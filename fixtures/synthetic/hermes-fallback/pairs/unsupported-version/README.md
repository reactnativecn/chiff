# Synthetic Hermes Pair: unsupported-version

This pair uses structurally valid Hermes-like headers with bytecode version
`100`, which is intentionally outside `chiff`'s current structured Hermes
allow-list.

Expected behavior:

- input format detection reports Hermes bytecode
- structured Hermes support reports `unsupported_version`
- engine selection falls back to `generic_binary`
- patch application still round-trips exactly

Current `chiff` result:

- `selected_engine=generic_binary`
- `selected_engine_reason=hermes_old_unsupported_version`
- `op_count=4`
- `copy_op_count=2`
- `insert_op_count=2`
- `copied_bytes=131`
- `inserted_bytes=9`
