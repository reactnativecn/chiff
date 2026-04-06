# Synthetic Binary Pair: arbitrary-binary

This pair contains arbitrary non-text binary data with no Hermes structure.

Expected behavior:

- input format detection reports `Binary` on both sides
- structured Hermes support reports `not_hermes` on both sides
- engine selection uses `generic_binary`
- the reason code is `binary_pair`
- patch application still round-trips exactly

Current `chiff` result:

- `selected_engine=generic_binary`
- `selected_engine_reason=binary_pair`
- `op_count=2`
- `copy_op_count=1`
- `insert_op_count=1`
- `copied_bytes=2`
- `inserted_bytes=8`
