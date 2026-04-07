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
- Hermes: native hdiff `797` bytes, `chiff_structured` `17068` bytes
- total: native hdiff `857` bytes, costed selection `857` bytes
- Hermes `chiff_structured` plan time: about `81.5s`

The region report shows the Hermes churn is concentrated in `small_string_table`
and `debug_data`, while function bodies remain unchanged. This is a useful
corpus case for improving string-table/debug-stream matching, but it also means
the hpatch-compatible bridge should remain opt-in and costed.
