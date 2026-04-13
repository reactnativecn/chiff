# Hpatch-Compatible Optimization Analysis

This document tracks the highest-leverage optimization points that still preserve
HDiffPatch / hpatch apply-side compatibility.

## Compatibility Boundary

The hpatch-compatible lane cannot require a custom patch applier. The generated
artifact must apply to the original old bytes and reconstruct the original new
bytes through the existing HDiffPatch single-compressed apply path.

That does not mean covers must be exact byte-equality copies. In HDiffPatch's
single-compressed format, a cover selects an old/new byte range pair, then the
serializer stores:

- cover control data: old-position delta, new-position delta, and length
- `newDataDiff`: bytes in new data that are not covered
- `newDataSubDiff`: byte-wise deltas for covered ranges, RLE-compressed

So a cover can be approximate as long as the old/new ranges are valid. The patch
remains correct because HDiffPatch stores the byte deltas for the covered region.
`chiff` currently exports only exact `Copy` regions from its internal IR, which
is a self-imposed restriction rather than a hpatch format requirement.

## Current Measurements

For the real `bundle-label-copy-edit` Hermes pair:

- native HDiffPatch: `797` bytes
- exact `chiff_structured` cover replacement: `17068` bytes
- merged native-plus-`chiff` covers: `797` bytes
- section/body approximate-cover merge: `797` bytes
- native same-delta small-gap coalescing: `783` bytes
- `chiff` exact covers: `39`
- exact covered new bytes: `2658597`
- exact uncovered new bytes: `38915`
- native HDiffPatch covers: `6`
- native covered new bytes: `2697479`
- native uncovered new bytes: `33`
- exact `chiff` cover bytes that fall in native gaps: `0`
- merge final cover count: `6`
- merge native cover capacity: `6`

Filtering exact `chiff` covers by length did not recover the regression. With
minimum cover lengths from `1` through `65536`, replacement mode stayed much
larger than native, while merge mode stayed equal to native. That means the
current exact covers are not adding valuable coverage in native uncovered gaps.
The native HDiffPatch matcher is already using approximate covers to cover almost
the entire new file on this pair.

Approximate whole-file cover experiments also did not win:

- one full-file approximate replacement cover: about `81196` bytes
- one near-full-file approximate replacement cover: about `81868` bytes
- those same approximate covers in merge mode: `797` to `798` bytes

This shows approximate covers are allowed, but they must be costed. Blindly
covering a large mismatched region can expand `newDataSubDiff` enough to lose.
On the current real Hermes pair, the remaining hpatch-compatible opportunity is
therefore not "cover more bytes"; it is either to choose a different approximate
alignment that compresses `newDataSubDiff` better than native, or to find a
different corpus where native leaves meaningful gaps.

The first useful serialized-size win came from post-processing HDiffPatch's own
native cover list instead of replacing it with `chiff` covers. On the real
Hermes pair, two adjacent native covers had the same old/new offset delta and
were separated by a 9-byte gap. Coalescing across that small same-delta gap
reduced the serialized payload from `797` to `783` bytes while staying a normal
hpatch payload. Trying to coalesce across different deltas expanded the patch to
`14901` or `81391` bytes in one-off tests, so this must remain constrained and
costed.

For the current 7-pair mixed corpus:

- native HDiffPatch total: `348` bytes
- exact `chiff_structured` replacement total: `355` bytes
- merged native-plus-`chiff` total: `347` bytes
- section/body approximate-cover merge total: `354` bytes
- native same-delta small-gap coalescing total: `348` bytes
- serialized costed selection total: `346` bytes
- selected winners: native `6`, structured `1`, merged `0`

The current hpatch-compatible lane is safe only when serialized-size gated.

## Hermes Issue 208

[facebook/hermes#208](https://github.com/facebook/hermes/issues/208) is directly
relevant. The important conclusions are:

- `--base-bytecode` is the largest low-cost compatibility optimization reported
  there. It makes the new bytecode more diffable during compilation and still
  emits a normal executable HBC file.
- The main reason is string/identifier table stability. Hermes bytecode
  instructions refer to strings by table index; small JavaScript changes can
  reorder the table and cascade index changes across the instruction stream.
- `--base-bytecode` initializes the new string/identifier table from the base
  bytecode, so existing strings keep their order and indices.
- `hbc-deltaprep` converts execution bytecode into a more diffable delta form by
  rewriting some absolute values as relative/delta values, then requires a
  reverse conversion back to execution form.
- In that issue's reported 23 MB example, bsdiff went from `2.9 MB` naive to
  `1.0 MB` with `--base-bytecode`, `1.9 MB` with `hbc-deltaprep`, and `0.5 MB`
  with both.
- The issue also notes `hbc-deltaprep` was experimental and not actively
  supported, so it should not be assumed safe as a production patch-side step.

For `chiff`, this changes the priority order:

1. Prefer `--base-bytecode` whenever the bundle build step has access to a
   suitable base HBC. This is patch-side-compatible because the output remains a
   normal HBC.
2. Treat `hbc-deltaprep` as a source of algorithmic hints for native `chiff` or
   internal cover planning, not as a direct hpatch-compatible artifact path.
3. Inside hpatch-compatible mode, mimic the useful parts by choosing
   approximate covers across string tables, function offset tables, debug
   offsets, and other absolute-value regions, while still emitting covers in
   original execution-byte coordinates.

The immediate architectural question is where `--base-bytecode` can fit into
`react-native-update`'s package model. If there is one canonical new package
served to many old versions, compiling it against a single base version may help
all diffs but will not be optimal for every from-version. Compiling a different
new HBC per old version could improve individual diffs, but it changes artifact
identity and hash semantics and should be treated as a separate migration design.

If the final flow cannot provide a base HBC to `hermesc`, the implementation is
still useful as a guide. The compiler-side work that can be absorbed into
`chiff` falls into three buckets:

- String / identifier table reseeding. Hermes seeds the new string accumulator
  from the base bytecode, preserving old string order before adding new strings.
  In `chiff`, we cannot reorder the new HBC for hpatch-compatible output, but we
  can parse the old/new string tables and use string identity to propose
  approximate covers between corresponding table entries and string-use
  instruction operands.
- Literal value / object key buffer reseeding. Hermes reconstructs value-buffer
  and key-buffer entries from bytecode instructions and object shape metadata,
  then appends only new literal buffers after the base storage. In `chiff`, the
  equivalent is to parse or infer literal buffer records and align old/new
  records by content or use-site, instead of diffing the raw buffers as one
  shifted byte stream.
- Absolute-to-relative conversion from `hbc-deltaprep`. Hermes's converter
  relativizes selected instruction operands and table fields empirically known
  to reduce delta size. In hpatch-compatible mode we cannot replace the artifact
  with delta form, but we can use those fields to generate approximate covers or
  costed alignment anchors. In native `chiff`, the same idea can become an
  actual canonical comparison view with reverse transforms.

This means the next hpatch-compatible experiment should not try to literally
recreate `--base-bytecode` after compilation. It should mine the same stable
identities from old/new HBC and feed them into a serialized-cost-aware
approximate-cover planner.

## Highest-Leverage Hpatch-Compatible Optimizations

### 0. Build-Time Delta Optimizing Mode

If the build pipeline can provide an old HBC to `hermesc`, `--base-bytecode` is
the first hpatch-compatible experiment to run. It attacks the largest known
source of bytecode diff churn before the diff algorithm sees the bytes, and the
patch side does not need to change.

This is outside `chiff`'s byte-level diff algorithm, but it can dominate every
downstream cover-selection improvement. It should be benchmarked before spending
more time on lower-level hpatch cover policies.

### 1. Structured Approximate Covers

This is the largest theoretical hpatch-compatible optimization still untried.
Instead of exporting only exact `Copy` ranges, `chiff` can propose approximate
old/new range pairs where Hermes structure says the regions correspond:

- same section kind across old/new HBC files
- same function body identity or stable function order
- same debug-data stream identity
- same string/debug metadata subregion after offset drift

HDiffPatch can then encode byte deltas inside the cover. This preserves hpatch
compatibility while giving `chiff` a way to express semantic alignment that is
not byte-identical.

The risk is serialized cost. Approximate covers should be accepted only when the
estimated gain from reducing `newDataDiff` beats:

- cover control bytes
- `newDataSubDiff` RLE/compression cost
- extra fragmentation and worse compression context

The first implementation should be a separate hpatch-compatible planner, not a
change to the internal exact `Patch` IR.

The planner should be gap-aware from the start. If native HDiffPatch already
covers almost all new bytes, as in `bundle-label-copy-edit`, approximate covers
should be evaluated against serialized payload size and `newDataSubDiff`
compressibility rather than raw covered-byte count.

### 2. Native Gap-Aware Planning

The merge mode preserves native HDiffPatch covers and inserts external covers
only into uncovered new-file gaps. The real pairs currently show `merge` usually
adds no useful cover beyond native.

`node-hdiffpatch.diffWithCovers(..., { debugCovers: true })` now exposes native
and final cover coordinates. This lets `chiff` focus planning on native gaps
instead of spending time producing covers that native already covers.

This is still hpatch-compatible because the final output remains a standard
HDiffPatch payload.

### 3. Serialized-Cost-Aware Pruning

Simple cover-length pruning did not help the real Hermes pair. A better pruning
pass needs to model HDiffPatch's cost function:

- packed cover-control byte size
- uncovered literal byte contribution
- covered-region sub-diff RLE contribution
- compression effects, measured through serialized candidate payloads for now

This pass should decide whether to keep, merge, extend, or drop a candidate
cover before calling HDiffPatch.

The first practical version is `native-coalesce`: keep HDiffPatch's own cover
selection, then merge only adjacent native covers with identical old/new offset
delta across a small gap. This is generic, does not require Hermes parsing, and
already improved one real Hermes pair by `14` bytes. On the current generated
`testHotUpdate` corpus it is smaller on 1 file, equal on 11 files, and larger on
none. It should still be used behind costed selection until a broader corpus
proves it has no regressions.

### 4. HDiffPatch Parameter Tuning

Changing HDiffPatch search parameters may produce small wins, but it is unlikely
to be the main advantage. Native HDiffPatch already performs approximate cover
search, extension, and selection over raw bytes. Format knowledge is more likely
to matter in approximate cover proposals and gap targeting than in global
parameter changes.

### 5. Zstd `--patch-from` Baseline

Zstd's patch mode is a different model from hpatch-compatible output. According
to the upstream zstd wiki, `--patch-from` treats the old file as a dictionary for
compressing the new file, raises the dictionary limit to 2 GB, and can use the
long-range match finder when needed:
https://github.com/facebook/zstd/wiki/Zstandard-as-a-patching-engine

That model is attractive for speed and as a native `chiff` compression backend,
but it is not hpatch-compatible. A zstd patch requires zstd dictionary
decompression on the patch side:

```bash
zstd --patch-from=<oldfile> <newfile> -o <patchfile>
zstd -d --patch-from=<oldfile> <patchfile> -o <newfile>
```

On the current generated `testHotUpdate` corpus, the size baseline is not
competitive with native hdiff for small OTA bundle changes:

| Engine | Total patch bytes |
| --- | ---: |
| native hdiff | 37380 |
| zstd `-3 --patch-from` | 324437 |
| zstd `-19 --patch-from` | 125637 |
| current hpatch-compatible costed selection | 35989 |

The high-churn `test-id-edit` Hermes pair shows the same pattern:

| Engine | Patch bytes |
| --- | ---: |
| native hdiff | 31840 |
| zstd `-19 --patch-from` | 80416 |
| zstd `--ultra -22 --patch-from` | 79964 |
| approximate hpatch merge | 30463 |

So zstd is useful as a speed-oriented baseline and potential native-format
compression component, but it should not replace hdiff in the hpatch-compatible
lane for the current React Native / Hermes OTA workload.

### 6. More Exact Structural Segmentation

More exact segmentation is lower priority for hpatch-compatible output. It helps
the future native `chiff` format and internal diagnostics, but in hpatch mode it
can increase cover fragmentation and worsen serialized size.

## Decision Rule

The hpatch-compatible lane remains worth keeping for zero patch-side migration,
but it should not be the only long-term algorithm lane.

Continue hpatch-compatible work only if structured approximate covers plus
native gap-aware planning produce repeatable wins on a broader real corpus. A
reasonable gate is:

- at least `5%` serialized-size reduction on meaningful Hermes pairs, or
- no regression with a measurable server-time budget, and
- file-level hpatch roundtrip for every candidate.

If that gate is not met after the approximate/gap-aware experiments, keep
hpatch-compatible mode as a safe bridge and move compression work to the native
`chiff` format.

The native format is still required for the theoretical upper bound because it
can represent transformations hpatch cannot:

- canonicalized comparison views
- symbolic string/function/debug operands
- section/function manifests
- specialized debug-record encodings
- chunk-addressed OTA reuse

## Next Experiment

1. Expand the real corpus and keep measuring `native-coalesce` across more
   Hermes and text bundle pairs. It is currently non-regressive on the generated
   `testHotUpdate` corpus, but the absolute win is small.
2. Keep `native-coalesce` in the opt-in costed CLI policy because it is cheap and
   preserves the native hdiff cover search.
3. Prefer coarse approximate Hermes covers over exact structured covers for the
   next hpatch-compatible experiments. The `test-id-edit` Hermes pair improved
   from 31840 bytes to 30463 bytes when approximate covers were merged into
   native gaps, while exact structured covers were much worse and slow.
4. Keep approximate Hermes covers serialized-costed. The current generated
   corpus shows approximate merge is smaller on 1 file, equal on 7 files, and
   larger on 4 files; costed selection is 35989 bytes versus 37380 bytes for
   native hdiff.
5. Keep exact structured covers behind an explicit opt-in flag only. They are
   useful for diagnostics and native-format research, but not for default hpatch
   compatibility.
