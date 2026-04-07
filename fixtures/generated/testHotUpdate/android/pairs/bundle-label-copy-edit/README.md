# testHotUpdate Android Pair: bundle-label-copy-edit

This fixture pair was generated from:

- `/Users/sunny/Documents/workspace/react-native-update/Example/testHotUpdate`

## Mutation

Minimal ASCII UI text change in the bundle-label element.

Source file:

- `/Users/sunny/Documents/workspace/react-native-update/Example/testHotUpdate/src/index.tsx:122`

Mutation applied for `v2` generation:

- `<Text testID="bundle-label">bundleLabel: {bundleLabel}</Text>` -> `<Text testID="bundle-label">bundleLabelV2: {bundleLabel}</Text>`

The source file in the example project is restored after generation by `scripts/generate-testhotupdate-corpus.cjs`.
The generator uses a stable work directory under `target/generated-fixtures`
so Hermes debug metadata does not depend on a random temporary path.

## Pair Layout

- `v1/text/index.android.bundle`
- `v2/text/index.android.bundle`
- `v1/hermes/index.android.hbc`
- `v2/hermes/index.android.hbc`

Machine-readable artifact metadata is recorded in `metadata.json`.

Relative fixture root:

- `fixtures/generated/testHotUpdate/android/pairs/bundle-label-copy-edit`

## Current Chiff Results

Release-mode stats from the current `chiff` implementation:

Text bundle:

- `old_size=2093750`
- `new_size=2093752`
- `op_count=15`
- `copy_op_count=8`
- `insert_op_count=7`
- `copied_bytes=2093744`
- `inserted_bytes=8`

Hermes bundle:

- `old_size=2697512`
- `new_size=2697512`
- `op_count=75`
- `copy_op_count=39`
- `insert_op_count=36`
- `copied_bytes=2658597`
- `inserted_bytes=38915`

## Current Hpatch-Compatible Finding

With `node-hdiffpatch` as the serializer, this pair is a regression case for
blindly using `chiff_structured` covers:

- text: native hdiff `60` bytes, `chiff_structured` `63` bytes
- Hermes: native hdiff `797` bytes, `chiff_structured` `17068` bytes,
  merged native-plus-`chiff` covers `797` bytes, approximate section/body
  covers `797` bytes, native same-delta gap coalescing `783` bytes
- total: native hdiff `857` bytes, costed selection `843` bytes
- Hermes `chiff_structured` plan time: about `137s` in the latest debug run
- Hermes native HDiffPatch covers already cover `2697479 / 2697512` new bytes,
  leaving only `33` uncovered new bytes
- exact `chiff` covers contribute `0` bytes inside native uncovered gaps
- coarse approximate covers contribute `13` bytes inside native uncovered gaps,
  but the serialized payload remains equal to native hdiff
- native cover coalescing reduces the Hermes cover count from `6` to `5` by
  merging a small same-delta gap, and is currently the only hpatch-compatible
  candidate that wins on this pair

The region report shows the Hermes churn is concentrated in `small_string_table`
and `debug_data`, while function bodies remain unchanged. This is a useful
corpus case for improving string-table/debug-stream matching. The merged
hpatch-compatible mode avoids the serialized-size blow-up on this pair, but it
still pays the structured planning cost. More exact covers are unlikely to help
this pair; the strongest hpatch-compatible lead is now costed native-cover
coalescing. The bridge should remain opt-in, costed, and threshold-gated.
