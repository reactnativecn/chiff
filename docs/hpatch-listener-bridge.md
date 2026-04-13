# Hpatch Listener Bridge

## Integration Target

The lowest-risk hpatch-compatible integration point is the existing
`ICoverLinesListener` hook in HDiffPatch:

- [diff.h](/Users/sunny/Documents/workspace/node-hdiffpatch/HDiffPatch/libHDiffPatch/HDiff/diff.h)
  defines `ICoverLinesListener::coverLines`.
- [diff.cpp](/Users/sunny/Documents/workspace/node-hdiffpatch/HDiffPatch/libHDiffPatch/HDiff/diff.cpp)
  calls the listener from `create_single_compressed_diff`, then continues through
  HDiffPatch's existing `sub_cover` and `serialize_single_compressed_diff`
  pipeline.
- [src/hdiff.cpp](/Users/sunny/Documents/workspace/node-hdiffpatch/src/hdiff.cpp)
  historically passed `0` for the listener, so all cover selection was done by
  HDiffPatch's raw-byte matcher.

That means `chiff` can improve cover selection without changing the patch
consumer. The generated payload still comes from HDiffPatch and should still
apply with the existing `hpatch_by_file` runtime.

There was one important implementation constraint in the original HDiffPatch
hook: `diff.cpp` allocated the listener output buffer with the same size as
HDiffPatch's native cover list, then asserted `coverCount <= _covers.size()`.
So a listener could not safely return more covers than the raw matcher already
found unless we either:

- coalesce or down-select `chiff` covers to fit the available capacity,
- merge into the native cover list rather than replacing it wholesale, or
- add a new HDiffPatch-side hook that allows dynamic cover-list sizing.

This constraint is a serializer integration detail, not a patch-side
compatibility issue. Any of the options above can still emit a standard
HDiffPatch-compatible payload.

The local `node-hdiffpatch` bridge now uses the dynamic-cover option: listener
implementations may set `coverCount` larger than the initial native capacity,
HDiffPatch resizes the temporary cover buffer, and then calls the listener again
to fill the larger list. The output is still serialized by HDiffPatch.

## Proposed Bridge

The generation path in `node-hdiffpatch` is side-by-side with the existing
path:

1. Keep the current `hdiff(old, oldsize, new, newsize, out)` path unchanged.
2. Add `hdiff_with_covers(...)` and expose it to JavaScript as `diffWithCovers`.
3. The caller passes a precomputed `HpatchCompatiblePlan` cover list from
   `chiff` or another cover selector.
4. Convert cover entries to `hpatch_TCover` records.
5. Pass an `ICoverLinesListener` to `create_single_compressed_diff`.
6. Let HDiffPatch continue running `sub_cover` and its standard serializer.
7. Validate the result with `check_single_compressed_diff`, then with the
   file-level `patchSingleStream` path that applies the same single-compressed
   hpatch payload family used by `react-native-update`.

The original `diff` API remains unchanged.

## React Native Update Integration

`react-native-update-cli` now keeps the existing `hdiff` and `hdiffFrom*`
commands and wraps their hdiff implementation internally:

1. Load `node-hdiffpatch` as before.
2. Optionally load `@chiff/node`.
3. If `RNU_CHIFF_HPATCH_POLICY=costed` is enabled and both `diffWithCovers` and
   `hpatchCompatiblePlanResult` are available, generate `chiff` cover lines and
   pass them to `diffWithCovers` in both `replace` and `merge` modes. The wrapper
   also generates the native hdiff payload and keeps the smallest standard
   hpatch-compatible output.
4. If the native hdiff payload is below `RNU_CHIFF_HPATCH_MIN_NATIVE_BYTES`
   (`4096` bytes by default), skip structured planning and keep native hdiff.
5. If loading, cover generation, or `diffWithCovers` fails, fall back to the
   original `node-hdiffpatch.diff`.

With the default policy, the wrapper only calls native `node-hdiffpatch.diff`.

This keeps the server task model unchanged. Existing `hdiff` and `phdiff`
tasks still produce standard hpatch-compatible payloads, so the SDK/native
patch side does not need to change.

## Policy

The compatible bridge now supports these serialized candidates:

- `hdiff_native`: no listener, current behavior.
- `chiff_structured`: replace HDiffPatch's computed covers with `chiff` covers.
- `merged_costed`: preserve HDiffPatch's native covers and insert `chiff` covers
  only into uncovered new-file gaps before serialization.
- `chiff_approx_merged`: preserve HDiffPatch's native covers and insert coarse
  Hermes approximate covers into uncovered new-file gaps.
- `native-coalesce`: preserve HDiffPatch's native cover selection but coalesce
  adjacent native covers with the same old/new offset delta across small gaps.

The current `chiff` crate exports exact structured and coarse approximate cover
plans, and the `node-hdiffpatch` bridge implements replacement, merge injection,
and native coalescing modes. Production default should not switch to these
blindly. They are currently opt-in because HDiffPatch's approximate raw-byte
covers may beat `chiff`'s exact covers on real Hermes inputs, and exact
structured planning paths are still too slow for server defaults.

The CLI costed policy should try `native-coalesce` first, then coarse
approximate merge when the native hdiff payload is above the configured
threshold. Exact structured covers require an explicit opt-in flag.

## Compatibility Constraints

The listener bridge must preserve these constraints:

- Cover coordinates are always in original old/new file byte space.
- The listener must not hand HDiffPatch a canonicalized or transformed byte
  stream unless the final serialized patch still applies to the original old
  bytes and reconstructs the original new bytes.
- Unsupported Hermes versions, invalid Hermes headers, mixed formats, and
  arbitrary binary data must remain eligible for `hdiff_native` fallback.
- Native-only `chiff` optimizations must not feed this bridge unless they can be
  lowered back to exact original-byte covers.

## Theoretical Advantage

In hpatch-compatible mode, `chiff` has no theoretical advantage from the
container, compressor, or applier because those remain HDiffPatch's. The
advantage can only come from better cover selection before serialization.

The advantage should exist when semantic structure reveals stable regions that a
raw-byte matcher sees as noisy, for example:

- Hermes debug-data records whose delta encoding changes but decoded locations
  still align.
- Function bodies or metadata whose surrounding offsets shift.
- Section-local changes that should not pollute unrelated sections.

The advantage should not be assumed for arbitrary binary data. The correct
production strategy is therefore benchmark-gated policy selection, not a global
replacement of HDiffPatch's matcher.
