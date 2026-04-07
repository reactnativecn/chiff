# Mixed Baseline Corpus

This corpus combines seven representative old/new pairs in a single directory
layout:

- one real structured Hermes pair
- one real text pair
- one synthetic arbitrary binary pair
- one synthetic Hermes `version_mismatch` fallback pair
- one synthetic Hermes `form_mismatch` fallback pair
- one synthetic Hermes `unsupported_version` fallback pair
- one synthetic Hermes `invalid_header` fallback pair

Directory layout:

- `old/real/hermes/index.android.hbc`
- `new/real/hermes/index.android.hbc`
- `old/real/text/index.android.bundle`
- `new/real/text/index.android.bundle`
- `old/fallback/arbitrary-binary/blob.bin`
- `new/fallback/arbitrary-binary/blob.bin`
- `old/fallback/version-mismatch/index.android.hbc`
- `new/fallback/version-mismatch/index.android.hbc`
- `old/fallback/form-mismatch/index.android.hbc`
- `new/fallback/form-mismatch/index.android.hbc`
- `old/fallback/unsupported-version/index.android.hbc`
- `new/fallback/unsupported-version/index.android.hbc`
- `old/fallback/invalid-header/index.android.hbc`
- `new/fallback/invalid-header/index.android.hbc`

Current `chiff` summary:

- `paired=7`
- `total_copy_ops=18`
- `total_insert_ops=17`
- `total_copied_bytes=4791603`
- `total_inserted_bytes=113`

Expected reason counts:

- `binary_pair=1`
- `hermes_form_mismatch=1`
- `hermes_structured=1`
- `hermes_version_mismatch=1`
- `text_pair=1`
- `hermes_old_unsupported_version=1`
- `hermes_old_invalid_header=1`

Expected support counts for both old and new:

- `supported=3`
- `not_hermes=2`
- `unsupported_version=1`
- `invalid_header=1`

Current hpatch-compatible report:

- native hdiff total: `348` bytes
- `chiff_structured` replacement total: `355` bytes
- merged native-plus-`chiff` total: `347` bytes
- approximate section/body merge total: `354` bytes
- native same-delta gap coalescing total: `348` bytes
- serialized costed selection total: `346` bytes
- selected winners: native `6`, structured `1`, merged `0`,
  coalesced native `0`

The merged candidate lowers the aggregate merged total below native, but the
per-file serialized winner is still mostly native. This corpus should remain a
guardrail against enabling structured covers without a serialized-size policy.
The coalescing candidate is neutral on this mixed corpus and remains useful
because it wins on the larger `bundle-label-copy-edit` real Hermes pair.
