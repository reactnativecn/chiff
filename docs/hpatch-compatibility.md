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

This is not a serialized hpatch file yet. It is the seam where the next step can
connect either:

- a Rust implementation of the HDiffPatch single-compressed serializer, or
- a thin FFI/Node bridge to HDiffPatch's `ICoverLinesListener`.

The listener bridge plan is documented in
[hpatch-listener-bridge.md](/Users/sunny/Documents/workspace/chiff/docs/hpatch-listener-bridge.md).

`HpatchCompatiblePlan::stats()` reports a compatibility-mode cost floor:

- `cover_count`
- `covered_bytes`
- `uncovered_new_bytes`

This is not the final compressed patch size. It is only a cover-quality signal
that can be compared before the HDiffPatch serializer and compressor are
introduced.

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
3. Add a cover-selection policy:
   - `hdiff-native`
   - `chiff-structured`
   - `merged-costed`
4. Use `HpatchCompatiblePlan::stats()` as the first cover-quality signal, while
   treating it as a floor rather than a serialized size estimate.
5. Add a serializer backend behind an explicit output mode:
   - `native-chiff` for future custom patch format
   - `hpatch-compatible` for existing patch clients
6. Validate generated hpatch-compatible payloads with the existing native
   `hpatch_by_file` apply path.
7. Only after that, consider custom patch containers for clients that opt in.
