# Synthetic Hermes Pair: version-mismatch

This pair uses structurally valid Hermes-like headers on two explicitly
supported versions, but the old and new bytecode versions differ.

Expected behavior:

- input format detection reports Hermes bytecode on both sides
- structured Hermes support reports `supported` on both sides
- engine selection falls back to `generic_binary`
- the fallback reason is `hermes_version_mismatch`
- patch application still round-trips exactly

Current `chiff` result:

- `selected_engine=generic_binary`
- `selected_engine_reason=hermes_version_mismatch`
- `op_count=6`
- `copy_op_count=3`
- `insert_op_count=3`
- `copied_bytes=134`
- `inserted_bytes=8`
