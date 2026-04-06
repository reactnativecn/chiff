# Synthetic Hermes Pair: form-mismatch

This pair uses structurally valid Hermes-like headers on the same explicitly
supported version, but the old artifact is execution form and the new artifact
is delta form.

Expected behavior:

- input format detection reports Hermes bytecode on both sides
- structured Hermes support reports `supported` on both sides
- engine selection falls back to `generic_binary`
- the fallback reason is `hermes_form_mismatch`
- patch application still round-trips exactly

Current `chiff` result:

- `selected_engine=generic_binary`
- `selected_engine_reason=hermes_form_mismatch`
- `op_count=5`
- `copy_op_count=2`
- `insert_op_count=3`
- `copied_bytes=125`
- `inserted_bytes=17`
