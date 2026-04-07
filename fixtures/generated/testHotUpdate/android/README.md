# testHotUpdate Android Generated Fixtures

These fixtures were generated from the React Native example app at:

- `/Users/sunny/Documents/workspace/react-native-update/Example/testHotUpdate`

The source app currently uses:

- `react-native` `0.84.1`
- Hermes bytecode version `98` for the generated `.hbc` sample

## Generation Commands

Text bundle:

```bash
npx react-native bundle \
  --platform android \
  --dev false \
  --entry-file index.js \
  --bundle-output "$TMPDIR/index.android.bundle" \
  --assets-dest "$TMPDIR/assets"
```

Hermes bytecode bundle:

```bash
./node_modules/hermes-compiler/hermesc/osx-bin/hermesc \
  -O \
  -emit-binary \
  -out "$TMPDIR/index.android.hbc" \
  "$TMPDIR/index.android.bundle"
```

The resulting artifacts were copied into:

- `/Users/sunny/Documents/workspace/chiff/fixtures/generated/testHotUpdate/android/text/index.android.bundle`
- `/Users/sunny/Documents/workspace/chiff/fixtures/generated/testHotUpdate/android/hermes/index.android.hbc`

Additional old/new pairs can be generated with:

```bash
node scripts/generate-testhotupdate-corpus.cjs --pair <name>
```

Current generated pairs:

- `minor-string-edit`
- `bundle-label-copy-edit`

## Notes

- These fixtures intentionally exclude any speculative custom footer metadata.
- They are intended to serve as real-world corpus samples for format detection, diff statistics, and future patch-size regression tracking.
- File sizes and SHA-256 checksums are recorded in `metadata.json`.
