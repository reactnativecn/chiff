# Hpatch Compatibility Architecture

## Goal

`chiff` should support an output mode whose final artifact is compatible with the
existing HDiffPatch / hpatch apply side used by `react-native-update`.

In this mode, Hermes-aware and text-aware logic may influence *which old/new byte
ranges are selected as covers*, but the patch artifact must still be a standard
HDiffPatch-compatible diff. The apply side must not need Hermes parsing, text
parsing, canonicalization, or a new patch container.

This is one output lane, not the only possible output lane. `chiff` should keep a
separate opt-in native format lane for cases where both producer and consumer can
change and we want to pursue the theoretical upper bound.

## Dual-Lane Output Strategy

`chiff` should keep these two lanes separate:

- `hpatch-compatible`: the migration/default lane for existing clients. It emits
  a standard HDiffPatch-compatible artifact, applies with the existing hpatch
  runtime, and only uses structure-aware analysis to choose better original-byte
  cover lines.
- `native-chiff`: the opt-in lane for maximum compression and future features.
  It may introduce a new container, Hermes-specific operations, symbolic
  references, section/function manifests, operand normalization, and custom
  validation metadata.

The core analyzer can be shared, but every optimization must be classified as
one of:

- compatible with original-byte cover emission
- native-only because it requires a custom apply-side transform

This classification should be explicit in tests and benchmarks.

## Compatibility Boundary

The stable boundary is:

1. `chiff` analyzes old/new bytes with format-aware logic.
2. `chiff` emits cover lines in HDiffPatch semantics:
   - `oldPos`
   - `newPos`
   - `length`
3. A HDiffPatch-compatible serializer turns those cover lines plus the original
   old/new bytes into the final `.hpatch`-compatible payload.
4. Existing `hpatch_by_file`-style clients apply the patch unchanged.

This means future algorithms must preserve a mapping back to original file byte
offsets. A normalized or symbolic comparison view is allowed only if it is used
to choose covers and every emitted cover still points into the original old file
and original new-file coordinate space.

## What Is Not Allowed In Hpatch-Compatible Mode

- Emitting Hermes-specific operations that the patch side must understand.
- Stripping, canonicalizing, or rewriting bytes before diff unless the final
  serialized patch applies to the original old bytes and reconstructs the
  original new bytes.
- Requiring a new native patch applier.
- Requiring React Native / Hermes version awareness on the patch side.

These restrictions do not apply to the opt-in `native-chiff` lane, but native
artifacts must never be advertised as hpatch-compatible.

## Current Implementation Hook

The first implementation hook is `build_hpatch_compatible_plan` in
[src/hpatch.rs](/Users/sunny/Documents/workspace/chiff/src/hpatch.rs). It
converts `chiff`'s exact `Copy/Insert` patch IR into
HDiffPatch cover-line coordinates.

The Node/Bun binding exposes the same seam as `hpatchCompatiblePlanResult` for
diagnostics and future integration work. It returns offsets as decimal strings
so 64-bit file positions are not truncated by JavaScript number precision.

The output-lane boundary is represented in
[src/output.rs](/Users/sunny/Documents/workspace/chiff/src/output.rs):

- `PatchOutputMode::HpatchCompatible` is the patch-side-compatible lane.
- `PatchOutputMode::NativeChiff` is the future opt-in custom lane.
- `OptimizationCompatibility::OriginalByteCover` marks optimizations that can
  emit original old/new byte coordinates and are allowed in both lanes.
- `OptimizationCompatibility::NativeOnly` marks optimizations that require a
  custom patch applier and must not be used in the hpatch-compatible lane.

This seam is now connected through `node-hdiffpatch.diffWithCovers`, which feeds
`chiff` cover lines into HDiffPatch's `ICoverLinesListener` and still lets
HDiffPatch serialize the final payload. The Rust crate still emits a cover plan
rather than a standalone hpatch file; the production-compatible artifact is
created by the Node bridge.

`react-native-update-cli` can use that bridge behind the existing `hdiff`
commands without adding server task types. The bridge is intentionally opt-in:
set `RNU_CHIFF_HPATCH_POLICY=costed` to generate the native hdiff payload,
the `chiff` cover replacement payload, and the merged native-plus-`chiff`
payload, then keep the smallest standard hpatch-compatible artifact. With the
default policy, the CLI keeps using native hdiff only. Even in `costed` mode,
the CLI skips structured planning when the native hdiff payload is below
`RNU_CHIFF_HPATCH_MIN_NATIVE_BYTES`; the default threshold is `4096` bytes.

For corpus evaluation, the Node/Bun binding includes:

```bash
cd /Users/sunny/Documents/workspace/chiff/bindings/node
npm run hpatch:report:node -- ../../fixtures/corpus/mixed-baseline/old ../../fixtures/corpus/mixed-baseline/new --hdiff-module /Users/sunny/Documents/workspace/node-hdiffpatch
```

The current mixed baseline shows why the costed wrapper is necessary:
`hdiff_native` is smaller on most current pairs, while `chiff_structured` wins on
one synthetic form-mismatch pair. On the current 7-pair mixed corpus, native
hdiff totals 348 bytes, structured covers total 355 bytes, merged covers total
347 bytes, coarse approximate covers total 354 bytes, native same-delta
coalescing totals 348 bytes, and the costed selection totals 346 bytes. That is
expected at this stage because the current hpatch-compatible lane lowers exact
`chiff` copy regions into hpatch covers, while HDiffPatch's native matcher can
sometimes find approximate covers that serialize smaller.

The newer generated `testHotUpdate` corpus is a stronger warning against
unconditional enablement. The `bundle-label-copy-edit` Hermes pair has native
hdiff at 797 bytes, exact `chiff_structured` at 17068 bytes, and exact merge back
at 797 bytes. `native-coalesce` improves the same pair to 783 bytes by
coalescing adjacent HDiffPatch-native covers with identical old/new offset delta
across small gaps. Across the current 12 generated text/Hermes files,
`native-coalesce` is smaller on 1 file, equal on 11 files, and larger on none,
for a total reduction of 14 bytes.

The high-churn `test-id-edit` Hermes pair shows a different useful path. Native
hdiff is 31840 bytes, exact `chiff_structured` is much worse, and exact merge is
slightly worse than native. The coarse approximate Hermes cover plan merged into
native gaps is 30463 bytes and takes about 117ms to plan in the current debug
run. That makes approximate merge a better default experiment than exact
structured covers.

The fast production-candidate report over those 12 generated files totals
37380 bytes for native hdiff, 37366 bytes for native coalescing, 36032 bytes for
blind approximate merge, and 35989 bytes for serialized costed selection.
Approximate merge is smaller on 1 file, equal on 7 files, and larger on 4 files,
so it must remain costed rather than unconditional.

The same corpus was also compared against zstd `--patch-from`. Zstd `-3` totals
324437 bytes and zstd `-19` totals 125637 bytes. Both variants round-trip
correctly but are much larger than native hdiff on this small OTA workload.
This is expected because zstd patching is dictionary compression, not a
HDiffPatch-compatible copy/insert patch format.

Production server usage should still remain opt-in, serialized-size gated, and
threshold-gated until a broader corpus proves it non-regressive. With the
default 4096-byte native-payload threshold, small native patches skip approximate
and exact structured planning entirely, while `native-coalesce` can still be
tried cheaply.

The report validates both candidates through `node-hdiffpatch.patchSingleStream`,
which applies the same single-compressed hpatch payload family generated by
`diff` and `diffWithCovers`. This is intentionally separate from
`node-hdiffpatch.patchStream`, which applies HDiffPatch's stream-diff format.

A future Rust implementation of the HDiffPatch single-compressed serializer can
replace that bridge, but it must keep the same original-byte cover contract.

The listener bridge plan is documented in
[hpatch-listener-bridge.md](/Users/sunny/Documents/workspace/chiff/docs/hpatch-listener-bridge.md).
The current optimization analysis and next experiment gate are documented in
[hpatch-compatible-optimization-analysis.md](/Users/sunny/Documents/workspace/chiff/docs/hpatch-compatible-optimization-analysis.md).

`HpatchCompatiblePlan::stats()` reports a compatibility-mode cost floor:

- `cover_count`
- `covered_bytes`
- `uncovered_new_bytes`

This is not the final compressed patch size. It is only a cover-quality signal
that can be compared before invoking the HDiffPatch serializer and compressor,
or used alongside serialized-size measurements in corpus reports.

## Theoretical Advantage Over Plain Hdiff

The compatibility mode does not claim an advantage from the patch container.
If the final payload is HDiffPatch-compatible, serialization, compression, and
apply behavior remain HDiffPatch's responsibility.

The only intended advantage is cover selection:

- plain hdiff searches byte similarity over the raw old/new byte streams
- `chiff` can use Hermes and text structure before choosing cover lines
- unchanged semantic regions can be selected even when surrounding offsets,
  varint lengths, debug-data records, or function metadata shift
- unsupported or uncertain structures can still fall back to the native hdiff
  cover search

This should be most useful for Hermes bytecode churn where small source changes
cause metadata or offset shifts that are meaningful to a parser but noisy to a
raw-byte matcher.

The expected advantage is not universal. For arbitrary binary data, or for cases
where hdiff's raw matcher already finds better approximate covers than `chiff`'s
exact `Copy` regions, plain hdiff may still win. The long-term implementation
should therefore compare costs and either:

- use `chiff` covers,
- use hdiff's native covers, or
- merge both cover sets before serialization.

The current `node-hdiffpatch` bridge already supports the merge option by
preserving HDiffPatch's native covers and inserting `chiff` covers only into
uncovered new-file gaps. This keeps the output hpatch-compatible, but it is
still a generated-payload candidate rather than a proof that structured planning
is cheap enough to enable globally.

The benchmark gate for this mode is direct comparison against the existing
HDiffPatch output on real React Native / Hermes corpora.

## Native Format Upper Bound

The native lane is where `chiff` can exceed the hpatch-compatible ceiling. It can
use techniques that are impossible to express as plain HDiffPatch cover lines,
for example:

- section-aware manifests instead of one monolithic byte stream
- function-level copy by stable identity instead of raw offset
- symbolic Hermes operands for string/function/debug references
- canonicalized comparison views with explicit reverse transforms
- specialized debug-data record encodings
- per-section compression and checksums
- chunk-addressed update manifests for OTA reuse

This lane can trade patch-side compatibility for better patch size or stronger
validation, but it should remain opt-in until the existing hpatch-compatible lane
has production-grade reports.

## Roadmap

1. Keep `chiff`'s core algorithm output convertible to HDiffPatch cover lines.
2. Add tests that verify cover coordinates match the current `Patch` replay
   order.
3. Use the `node-hdiffpatch.diffWithCovers` bridge for the current
   hpatch-compatible generation path.
4. Use a serialized-size policy in `react-native-update-cli` that picks the
   smallest payload between native hdiff, `chiff` cover replacement, and merged
   native-plus-`chiff` covers.
5. Keep refining the cover-selection policy:
   - `hdiff-native`
   - `chiff-structured`
   - `merged-costed`
6. Use `HpatchCompatiblePlan::stats()` as the first cover-quality signal, while
   treating it as a floor rather than a serialized size estimate.
7. Add or replace the serializer backend behind an explicit output mode:
   - `native-chiff` for future custom patch format
   - `hpatch-compatible` for existing patch clients
8. Validate generated hpatch-compatible payloads with a file-level
   single-compressed hpatch apply path.
9. Only after that, consider custom patch containers for clients that opt in.
