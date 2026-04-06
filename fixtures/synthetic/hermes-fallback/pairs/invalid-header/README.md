# Synthetic Hermes Pair: invalid-header

This pair uses Hermes magic and version `99`, but truncates the payload to
64 bytes so the header cannot be parsed structurally.

Expected behavior:

- input format detection still reports Hermes bytecode
- structured Hermes support reports `invalid_header`
- engine selection falls back to `generic_binary`
- patch application still round-trips exactly

Current `chiff` result:

- `selected_engine=generic_binary`
- `selected_engine_reason=hermes_old_invalid_header`
- `op_count=3`
- `copy_op_count=2`
- `insert_op_count=1`
- `copied_bytes=56`
- `inserted_bytes=8`
