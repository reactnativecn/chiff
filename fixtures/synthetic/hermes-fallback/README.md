# Synthetic Hermes Fallback Fixtures

This corpus contains intentionally minimal Hermes-like bytecode pairs used to
exercise `chiff` fallback behavior.

These fixtures are synthetic, not generated from a React Native app. They exist
to lock down correctness when the Hermes bytecode version changes or the bundle
shape is damaged.

Current pairs:

- `pairs/version-mismatch`
- `pairs/form-mismatch`
- `pairs/unsupported-version`
- `pairs/invalid-header`

The intended behavior is:

- detect the inputs as Hermes bytecode
- refuse structured Hermes diff
- fall back to generic binary diff
- still reconstruct the new bytes exactly

Each pair records machine-readable hashes and current `chiff` results in its
own `metadata.json`.
