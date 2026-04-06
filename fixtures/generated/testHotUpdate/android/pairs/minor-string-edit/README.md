# testHotUpdate Android Pair: minor-string-edit

This fixture pair was generated from:

- `/Users/sunny/Documents/workspace/react-native-update/Example/testHotUpdate`

The base app uses:

- `react-native` `0.84.1`
- Hermes bytecode version `98`

## Mutation

The pair models a minimal UI-text change in:

- `/Users/sunny/Documents/workspace/react-native-update/Example/testHotUpdate/src/index.tsx:115`

Mutation applied for `v2` generation:

- `这是版本一` -> `这是版本二`

The source file in the example project was restored after generation.

## Pair Layout

- `v1/text/index.android.bundle`
- `v2/text/index.android.bundle`
- `v1/hermes/index.android.hbc`
- `v2/hermes/index.android.hbc`

Machine-readable artifact metadata is recorded in `metadata.json`.

## Current Chiff Results

Release-mode stats from the current `chiff` implementation:

Text bundle:

- `old_size=2093750`
- `new_size=2093750`
- `op_count=3`
- `copy_op_count=2`
- `insert_op_count=1`
- `copied_bytes=2093748`
- `inserted_bytes=2`

Hermes bundle:

- `old_size=2697461`
- `new_size=2697468`
- `op_count=12`
- `copy_op_count=6`
- `insert_op_count=6`
- `copied_bytes=2697407`
- `inserted_bytes=61`

## Current Finding

`hermes_region_report` on this pair now shows:

- Hermes function layout now parses successfully on the real bundle.
- Function bodies are unchanged for this mutation.
- `string_storage` only changes locally.
- The global `debug_data` payload is unchanged.
- The remaining Hermes churn is concentrated in `debug_info` header metadata, filename storage, and the trailing footer bytes.

`chiff` now parses `debug_info` and diffs it by subregion, including stream-aware handling of `debug_data`.
