# Hpatch Listener Bridge Plan

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
  currently passes `0` for the listener, so all cover selection is done by
  HDiffPatch's raw-byte matcher.

That means `chiff` can improve cover selection without changing the patch
consumer. The generated payload still comes from HDiffPatch and should still
apply with the existing `hpatch_by_file` runtime.

There is one important implementation constraint in the current HDiffPatch hook:
`diff.cpp` allocates the listener output buffer with the same size as
HDiffPatch's native cover list, then asserts `coverCount <= _covers.size()`.
So a listener cannot safely return more covers than the raw matcher already
found unless we either:

- coalesce or down-select `chiff` covers to fit the available capacity,
- merge into the native cover list rather than replacing it wholesale, or
- add a new HDiffPatch-side hook that allows dynamic cover-list sizing.

This constraint is a serializer integration detail, not a patch-side
compatibility issue. Any of the options above can still emit a standard
HDiffPatch-compatible payload.

## Proposed Bridge

Add a new generation path in `node-hdiffpatch`:

1. Keep the current `hdiff(old, oldsize, new, newsize, out)` path unchanged.
2. Add a second entry point, for example `hdiff_with_cover_listener(...)`.
3. The new entry point calls `chiff` or receives a precomputed
   `HpatchCompatiblePlan`.
4. Convert `HpatchCompatiblePlan.covers` to `hpatch_TCover` records.
5. Pass an `ICoverLinesListener` to `create_single_compressed_diff`.
6. Let HDiffPatch continue running `sub_cover` and its standard serializer.
7. Validate the result with `check_single_compressed_diff`, then with the
   existing file-level `hpatch_by_file` path used by `react-native-update`.

This should be implemented as a side-by-side mode first, not as a replacement.
The first implementation may intentionally fall back to `hdiff_native` when
`chiff` produces more covers than the current listener buffer can hold.

## Policy

The compatible bridge should eventually support three policies:

- `hdiff_native`: no listener, current behavior.
- `chiff_structured`: replace HDiffPatch's computed covers with `chiff` covers.
- `merged_costed`: compare or merge both cover sets, then use the lower-risk or
  lower-cost result.

The current `chiff` crate only implements the `chiff_structured` cover-plan
export. Production default should not switch to that blindly. It should be
chosen by corpus data because HDiffPatch's approximate raw-byte covers may beat
`chiff`'s exact covers on some arbitrary binary inputs.

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
