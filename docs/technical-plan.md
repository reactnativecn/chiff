# Chiff Technical Plan

## Goal

`chiff` is a Rust diff library aimed at two primary input classes:

- Hermes bytecode bundles (`.hbc`)
- UTF-8 text bundles, especially JavaScript bundle artifacts

The project goal is not to invent a single universal binary diff algorithm and force every format through it.
The goal is to build a format-aware diff engine that:

- preserves semantic structure when possible
- falls back safely to generic byte diff when structure cannot be trusted
- distinguishes explicit generic-binary cases from true mixed-format cases
- remains easy to bind into Node and Bun through one shared native addon
- can optionally emit hpatch-compatible artifacts so existing patch clients do
  not need to change
- can also support an opt-in native format lane for maximum Hermes-aware
  compression when patch clients are allowed to change

Today that also means the Hermes-aware path is intentionally conservative:

- structured Hermes diff is only enabled for explicitly validated bytecode versions
- both sides must have the same Hermes bytecode version and the same bytecode form
- if header parsing or later structural parsing fails, patch generation falls back to generic binary diff
- engine selection and Hermes compatibility are exposed as reason/status codes, not just booleans

The current surrounding ecosystem baseline is documented in [baselines.md](/Users/sunny/Documents/workspace/chiff/docs/baselines.md).
The hpatch-compatible output boundary is documented in
[hpatch-compatibility.md](/Users/sunny/Documents/workspace/chiff/docs/hpatch-compatibility.md).
The HDiffPatch listener bridge is documented in
[hpatch-listener-bridge.md](/Users/sunny/Documents/workspace/chiff/docs/hpatch-listener-bridge.md).

## Non-goals

The current phase explicitly does not try to do the following:

- require a brand-new patch container format for the default compatibility path
- replace every mature binary diff implementation in all workloads
- support speculative footer metadata that is not actually present in current bundles
- fully reconstruct Hermes VM-level semantics from bytecode instructions

## Design Principles

### 1. Structure first, bytes second

For Hermes inputs, raw byte diff is a fallback, not the primary model.
The engine should first identify stable regions:

- file header
- structured metadata sections
- function bodies
- function info blocks
- debug info

Only after those regions are identified should byte-level diffing run inside each region.

### 2. Safe degradation

Whenever structural parsing is incomplete or uncertain, `chiff` must degrade to a coarser strategy:

- function-aware
- section-aware
- generic prefix/suffix byte diff
- generic binary diff for unknown or unsupported Hermes versions/forms

This keeps correctness separate from optimization.

### 3. TDD over speculative optimization

Each new parsing or diff refinement stage should be introduced by tests that demonstrate a concrete preservation property, for example:

- unchanged Hermes sections survive offset shifts
- unchanged functions survive preceding function growth
- overflowed function headers are still resolved to the correct function body ranges

### 4. Thin bindings

Node and Bun integration should stay thin.
The Rust crate owns:

- format detection
- Hermes parsing
- diff generation
- patch application

Bindings should only expose stable library APIs.

## Current Architecture

### Crate layout

- [src/format.rs](/Users/sunny/Documents/workspace/chiff/src/format.rs): input detection
- [src/corpus.rs](/Users/sunny/Documents/workspace/chiff/src/corpus.rs): reusable directory-pair corpus analysis and summary aggregation
- [src/engine.rs](/Users/sunny/Documents/workspace/chiff/src/engine.rs): engine selection
- [src/hermes.rs](/Users/sunny/Documents/workspace/chiff/src/hermes.rs): Hermes structural parsing
- [src/patch.rs](/Users/sunny/Documents/workspace/chiff/src/patch.rs): patch IR, apply, and diff logic
- [src/output.rs](/Users/sunny/Documents/workspace/chiff/src/output.rs): output-lane and optimization-compatibility classification
- [src/hpatch.rs](/Users/sunny/Documents/workspace/chiff/src/hpatch.rs): hpatch-compatible cover-plan boundary
- [benches/diff_cases.rs](/Users/sunny/Documents/workspace/chiff/benches/diff_cases.rs): synthetic benchmark harness for diff/apply cases
- [benches/corpus_cases.rs](/Users/sunny/Documents/workspace/chiff/benches/corpus_cases.rs): Criterion harness for real mixed-corpus diff/apply cases
- [examples/diff_stats.rs](/Users/sunny/Documents/workspace/chiff/examples/diff_stats.rs): quick CLI-style inspection for format detection and patch stats
- [examples/corpus_diff_stats.rs](/Users/sunny/Documents/workspace/chiff/examples/corpus_diff_stats.rs): directory-pair runner for real corpus diff stats
- [examples/hermes_region_report.rs](/Users/sunny/Documents/workspace/chiff/examples/hermes_region_report.rs): Hermes-specific region diagnostics for section, gap, function, and info-block churn
- [bindings/node/native/src/lib.rs](/Users/sunny/Documents/workspace/chiff/bindings/node/native/src/lib.rs): Node-API binding
- [bindings/node/scripts/corpus-diff-stats.cjs](/Users/sunny/Documents/workspace/chiff/bindings/node/scripts/corpus-diff-stats.cjs): Node/Bun directory-pair runner built on the Rust `analyze_directory_pair` binding
- [fixtures/generated/testHotUpdate/android/README.md](/Users/sunny/Documents/workspace/chiff/fixtures/generated/testHotUpdate/android/README.md): real generated Android bundle fixtures copied from the `react-native-update` example app
- [fixtures/generated/testHotUpdate/android/pairs/minor-string-edit/README.md](/Users/sunny/Documents/workspace/chiff/fixtures/generated/testHotUpdate/android/pairs/minor-string-edit/README.md): real old/new Android fixture pair produced from a minimal UI-string change

### Public model

`chiff` currently exposes:

- input format detection
- engine selection
- unified diff analysis via `analyze_diff`
- directory-pair corpus analysis via `analyze_directory_pair`
- Node/Bun directory-pair corpus analysis via `analyzeDirectoryPairResult`
- Node/Bun hpatch cover-plan diagnostics via `hpatchCompatiblePlanResult`
- explicit output-lane classification via `PatchOutputMode`
- optimization lane gating via `OptimizationCompatibility`
- structured Hermes compatibility helpers:
  - `assess_structured_hermes`
  - `supports_structured_hermes_version`
  - `can_use_structured_hermes`
- Hermes header parsing
- Hermes section layout parsing
- Hermes function layout parsing
- patch statistics via `PatchStats`
- a first hpatch-compatible cover-plan boundary via `build_hpatch_compatible_plan`
- hpatch-compatible cover-quality floor stats via `HpatchCompatiblePlan::stats`
- minimal patch IR:
  - `Copy { offset, len }`
  - `Insert(bytes)`

This IR is intentionally small for the current phase.
It is enough to validate structural diff behavior before introducing more advanced operations.

For hpatch-compatible output, this IR must remain convertible to original-file
cover coordinates. Future semantic algorithms may use normalized internal views
to choose matches, but emitted covers must still point to original old/new byte
positions.

For the future native `chiff` format, this restriction can be relaxed, but only
behind an explicit output mode. Native-only optimizations must not silently leak
into the hpatch-compatible lane.

## Real Fixture Corpus

`chiff` now includes a real generated Android fixture pair copied from:

- `/Users/sunny/Documents/workspace/react-native-update/Example/testHotUpdate`

The corpus currently contains:

- a release-mode text bundle at [fixtures/generated/testHotUpdate/android/text/index.android.bundle](/Users/sunny/Documents/workspace/chiff/fixtures/generated/testHotUpdate/android/text/index.android.bundle)
- a Hermes bytecode bundle at [fixtures/generated/testHotUpdate/android/hermes/index.android.hbc](/Users/sunny/Documents/workspace/chiff/fixtures/generated/testHotUpdate/android/hermes/index.android.hbc)
- a real old/new pair at [fixtures/generated/testHotUpdate/android/pairs/minor-string-edit/README.md](/Users/sunny/Documents/workspace/chiff/fixtures/generated/testHotUpdate/android/pairs/minor-string-edit/README.md)
- synthetic fallback pairs at [fixtures/synthetic/hermes-fallback/README.md](/Users/sunny/Documents/workspace/chiff/fixtures/synthetic/hermes-fallback/README.md)
- synthetic arbitrary binary fixtures at [fixtures/synthetic/generic-binary/README.md](/Users/sunny/Documents/workspace/chiff/fixtures/synthetic/generic-binary/README.md)
- a mixed regression corpus at [fixtures/corpus/mixed-baseline/README.md](/Users/sunny/Documents/workspace/chiff/fixtures/corpus/mixed-baseline/README.md)

This corpus is not yet a versioned old/new diff benchmark set.
It is currently used for:

- real artifact format-detection validation
- real bundle size and checksum tracking
- future directory-pair diff-stat and patch-size regression runs

The first real mutation pair currently shows:

- text diff is already efficient for a minimal string edit
- Hermes function layout on real bundles required support for non-monotonic and duplicate overflowed bytecode offsets
- Hermes `debug_info` can now be parsed into header, filename table, filename storage, file-region table, and per-function debug-data streams
- the first real Hermes mutation pair now diffs down to a small patch because `debug_data` is preserved as `Copy`
- synthetic fallback pairs now lock down the generic-binary path for unsupported Hermes versions and invalid Hermes headers
- synthetic fixtures now also lock down Hermes version mismatch, Hermes form mismatch, and arbitrary binary fallback
- the mixed corpus now locks down aggregate reason/support counts across structured Hermes, text, arbitrary binary, unsupported-version fallback, invalid-header fallback, version mismatch, and form mismatch in one run

## Current Hermes Model

### Header parsing

`HermesHeader` tracks the important fields from `BytecodeFileHeader`, including:

- version and bytecode form
- function count
- string-related counts and sizes
- bigint / regexp sizes
- CommonJS and function source counts
- `debug_info_offset`
- bytecode options flags

This gives the diff engine enough global metadata to parse and validate the file layout.

`chiff` treats structured Hermes parsing as version-gated rather than open-ended.
The validated set is exposed as `SUPPORTED_STRUCTURED_HERMES_VERSIONS`, which currently contains `98` and `99`.
That matches the real fixture corpus in this repository and the current upstream Hermes source tree, where [BytecodeVersion.h](/Users/sunny/Documents/workspace/hermes/include/hermes/BCGen/HBC/BytecodeVersion.h) defines `BYTECODE_VERSION = 99`.

### Section layout

`HermesSectionLayout` currently follows the upstream structured segment order:

1. function headers
2. string kinds
3. identifier hashes
4. small string table
5. overflow string table
6. string storage
7. literal value buffer
8. object key buffer
9. object shape table
10. bigint table
11. bigint storage
12. regexp table
13. regexp storage
14. CommonJS module table
15. function source table

All sections are aligned to 4 bytes, matching Hermes upstream behavior.

### Function layout

`HermesFunctionLayout` currently supports:

- small function headers, where the function body offset and bytecode size are stored directly in the `SmallFuncHeader`
- small function info blocks, where `SmallFuncHeader.infoOffset` points directly at optional exception/debug payloads
- overflowed function headers, where the `SmallFuncHeader` stores the offset to a large `FunctionHeader`
- overflowed function info blocks, including large headers, optional exception tables, and optional debug-offset payloads

For each function, `chiff` currently derives:

- function index
- function header offset
- function body start
- bytecode size
- function body end

For overflowed functions, `chiff` also derives per-function info blocks:

- info block start
- large-header end
- optional exception-table range
- optional debug-offset range
- parsed payload end
- padded block end
- whether exception handlers are present
- whether debug offsets are present

The parser also computes the start and end of the entire bytecode region.

## Current Diff Strategy

### Generic path

Non-Hermes inputs currently use a conservative byte diff:

- preserve common prefix
- try to re-synchronize on a stable middle anchor
- replace changed middle
- preserve common suffix

This is still deliberately simple, but it can now preserve a stable interior block when both ends change.

### Hermes path

Hermes diff currently uses a cascading strategy:

1. Diff the fixed 128-byte file header.
2. Diff each structured section separately.
3. If function layout is available:
   - diff pre-bytecode gap
   - diff each function body separately
   - when a body can be decoded using the embedded Hermes 98/99 opcode size table, diff opcode instructions and switch-table tail segments separately
   - diff bytecode-to-info alignment gap separately
   - diff each overflowed function info block as subregions:
     - large header
     - exception-table payload
     - debug-offset payload
     - trailing padding
   - otherwise diff post-bytecode pre-debug gap
4. Otherwise:
   - diff the whole non-debug tail as one region
5. Diff debug info and trailing bytes separately.

This already yields a meaningful improvement over monolithic byte diff because unchanged sections and unchanged functions can remain `Copy` even when earlier regions shift.
The same middle-anchor resync is also used inside each diffed region, so large unchanged byte runs inside a changed function body can survive as `Copy`.

### Hermes compatibility and fallback policy

Engine selection is now deliberately stricter than format detection:

- two Hermes inputs only use the structured Hermes path if both are parseable headers
- both must be on the same Hermes bytecode version
- both must use the same bytecode form
- the version must be in `SUPPORTED_STRUCTURED_HERMES_VERSIONS`
- if later structural parsing still fails, `diff_bytes` falls back to generic binary diff before emitting the patch
- two arbitrary binary inputs now report `binary_pair` rather than the more ambiguous `mixed_formats`

This means Hermes version churn or partial format breakage should reduce patch quality, but should not compromise patch correctness.

### Hpatch-compatible output policy

`chiff` now treats hpatch compatibility as an output-mode constraint, not as a
patch-side feature. The format-aware algorithm may choose better cover lines, but
the final hpatch-compatible artifact must apply to the original old bytes and
reconstruct the original new bytes with an unmodified HDiffPatch / hpatch applier.

The first internal seam is `build_hpatch_compatible_plan`, which maps `chiff`'s
current `Copy/Insert` IR into HDiffPatch cover coordinates. The current
production-compatible serializer path is the `node-hdiffpatch.diffWithCovers`
bridge, which feeds those covers into HDiffPatch's `ICoverLinesListener` while
preserving the same patch-side compatibility contract.

The cover-plan seam now also exposes a cost floor through
`HpatchCompatiblePlan::stats`:

- `cover_count`
- `covered_bytes`
- `uncovered_new_bytes`

These values are useful for policy selection, but they are not a final serialized
patch-size estimate. HDiffPatch may still add metadata, encode sub-diffs, and
compress streams differently from this floor.

### Native output policy

The native `chiff` output lane is allowed to pursue the theoretical upper bound
with a custom format, but it must be opt-in. This lane may introduce
Hermes-specific operations, section/function manifests, symbolic references,
canonicalized comparison views, and custom validation metadata.

The key rule is separation: an optimization can be shared only if it can still
emit original-byte hpatch cover lines. If it requires patch-side Hermes knowledge
or reverse transforms, it belongs to the native lane.

This rule is represented in code by `OptimizationCompatibility`:

- `OriginalByteCover` optimizations are allowed in both hpatch-compatible and
  native output modes.
- `NativeOnly` optimizations are rejected from the hpatch-compatible output mode.

## What Is Implemented Today

The following milestones are complete:

- Hermes execution and delta magic detection
- Hermes header parsing
- Hermes structured section layout parsing
- small-header function layout parsing
- small-header function info block parsing
- overflowed large-header function layout parsing
- overflowed function info block parsing
- exception-table and debug-offset subrange parsing inside overflowed info blocks
- section-aware Hermes diff
- function-aware Hermes diff for both small and overflowed headers
- instruction-aware Hermes diff for decodable 98/99 function bodies, with safe fallback to coarse body diff when decoding fails
- per-info-block Hermes diff for overflowed function metadata
- per-subregion Hermes diff for overflowed info metadata, so unchanged exception tables can survive changed large-header/debug payloads
- middle-anchor resync inside diff regions, so unchanged interior byte runs can survive changed prefixes and suffixes
- support for real-world overflowed Hermes function headers where bytecode offsets are non-monotonic or duplicated across headers
- parsing of global Hermes `debug_info` layout and function debug-data streams using the upstream serialized format
- debug-info-aware diff that preserves unchanged filename tables, file regions, and per-function debug-data streams
- hpatch-compatible cover-plan export from the current exact `Copy/Insert` patch IR
- hpatch-compatible cover-plan cost-floor stats
- explicit output-mode and optimization-compatibility classification
- `node-hdiffpatch.diffWithCovers` bridge that emits standard
  hpatch-compatible payloads from `chiff` cover plans
- `react-native-update-cli` hdiff wrapper that optionally uses the bridge
  without adding server task types or changing the SDK patch side
- costed CLI selection between native hdiff and `chiff`-cover hpatch payloads,
  so an experimental cover plan does not enlarge the production artifact
- synthetic Criterion benchmark harness for text and Hermes diff/apply hot paths
- Criterion mixed-corpus benchmark harness covering real text/Hermes pairs plus generic-binary and Hermes fallback pairs
- Rust crate verification
- reusable Rust corpus aggregation API for fixtures, reports, and future benchmark tooling
- Node/Bun hpatch policy report comparing native HDiffPatch payloads against
  `chiff`-cover payloads with file-level single-compressed hpatch roundtrip
  validation
- Node and Bun smoke-test verification through one Node-API addon
- real generated text/Hermes Android fixtures and two real old/new mutation
  pairs from the `react-native-update` example app
- synthetic fixture coverage for version mismatch, form mismatch, invalid header, unsupported version, and arbitrary binary fallback cases

## Current Limits

The following areas are still intentionally incomplete:

### 1. Function info parsing is still range-based, not semantic

For overflowed functions, the parser now exposes:

- large-header range
- exception-table range
- debug-offset range
- padded info-block end

This is enough to preserve unchanged subregions inside a changed info block.
However, it still does not parse the internal meaning of those subregions, for example:

- individual exception entries
- specific debug-offset fields
- future metadata payload variants if Hermes changes the info layout

### 2. Opcode-aware diff is still shallow

Within a function body, `chiff` now has a shallow instruction-level split for
Hermes 98/99 using embedded opcode sizes. It can preserve:

- instruction boundaries
- `UIntSwitchImm` jump-table tails
- `StringSwitchImm` jump-table tails

However, it still does not understand:

- operand semantics beyond switch-table tail extraction
- operand classes
- Hermes delta-relative operand normalization

The current implementation is deliberately conservative:

- if bytecode decoding fails, body diff falls back to the coarse per-function region strategy
- switch tables are segmented structurally, but instruction operands are still compared as raw bytes
- final patch output is normalized globally, so adjacent `Copy` / `Insert` ops are coalesced before stats are reported or bindings consume the patch

### 3. Debug-info semantics are still coarse

`chiff` now parses the top-level `debug_info` layout, function debug-data stream
boundaries, SLEB128 unit boundaries inside each stream, coarse source-location
record boundaries, and the optional `address/line/column/statement/envIdx`
field ranges and decoded values inside records. However, it still does not parse the internal
semantics of:

- filename table entries
- file-region mappings beyond raw entries
- value normalization rules that would rewrite equivalent records into a canonical form

That means stream-level preservation is now finer-grained than before, but
intra-stream optimization is still value-aware for anchoring rather than value-normalizing.

### 4. Text diff is still conservative

UTF-8 text currently uses the same prefix/suffix byte strategy.
It now performs a conservative middle-anchor resync, but it still does not perform:

- line anchors
- token-aware matching
- multi-anchor / token-level re-synchronization

### 5. Hpatch-compatible generation still needs policy selection

`chiff` can export hpatch cover-plan coordinates from its internal patch IR, and
`node-hdiffpatch.diffWithCovers` can serialize those plans into a standard
HDiffPatch-compatible payload. `react-native-update-cli` can use that path inside
the existing `hdiff` commands when both `@chiff/node` and the enhanced
`node-hdiffpatch` are available. The current CLI integration is opt-in and
costed: `RNU_CHIFF_HPATCH_POLICY=costed` generates native hdiff and
`chiff`-cover hpatch payloads, then keeps the smaller payload. The default path
still uses native hdiff only. Even in `costed` mode, the CLI skips structured
planning when the native hdiff payload is below
`RNU_CHIFF_HPATCH_MIN_NATIVE_BYTES`, which defaults to 4096 bytes.

The compatibility path still needs:

- broader corpus reports before making `chiff_structured` the unconditional
  default
- a future `merged_costed` policy that can combine hdiff-native covers and
  `chiff` covers instead of choosing between two serialized payloads
- a faster hpatch-compatible planner, because the `bundle-label-copy-edit` real
  Hermes pair currently spends about 81.5s in `chiff_structured` planning and
  still serializes much larger than native hdiff
- threshold tuning for `RNU_CHIFF_HPATCH_MIN_NATIVE_BYTES` across a larger
  corpus, because small native hdiff patches have little room for meaningful
  structured-cover wins

The theoretical advantage over plain hdiff is cover selection, not container
format. On arbitrary binary data, plain hdiff may still produce better covers.
The hpatch-compatible mode must therefore be benchmark-gated against real
React Native / Hermes corpora.

### 6. Native format is not designed yet

The native lane is now explicit as `PatchOutputMode::NativeChiff`, but the
container is not specified yet. Open questions include:

- section/function manifest layout
- semantic operation set
- compression strategy
- apply-side validation and rollback
- migration rules from hpatch-compatible artifacts

### 7. Benchmarking is still limited

We now have both:

- a synthetic Criterion harness for focused text/Hermes micro-cases
- a mixed-corpus Criterion harness over real and synthetic regression fixtures
- a Node/Bun hpatch-compatible policy report that compares serialized native
  hdiff and `chiff`-cover payload sizes

However, we still do not have a broader corpus-driven benchmark suite that records:

- patch size
- generation time
- apply time
- memory trends

## Near-term Roadmap

### Stage 1: Function info layout

Completed:

- parse overflowed function info blocks as explicit regions
- teach Hermes diff to compare those info blocks per function instead of as one shared metadata tail

Why:

- Hermes metadata churn often sits in function info and debug-related regions
- per-function segmentation should preserve more unchanged metadata after earlier function shifts

### Stage 2: Function info subregions

Completed:

- parse exception-table subranges inside function info blocks
- parse debug-offset subranges explicitly
- allow per-function info diff to preserve unchanged subregions within a changed info block

Why it mattered:

- Hermes metadata churn often lands inside function info rather than across the whole block
- subregion segmentation preserves more unchanged metadata when only part of an info block changes

### Stage 3: Function body substructure

Partially completed:

- identify bytecode instruction boundaries for supported Hermes 98/99 bodies
- identify bytecode body vs switch-table tail for `UIntSwitchImm` and `StringSwitchImm`
- preserve aligned jump-table regions separately when possible

Why:

- function bodies often shift due to codegen changes, but jump tables may remain stable

### Stage 4: Opcode-aware normalization

Then:

- parse Hermes instructions
- identify operand classes that are good candidates for normalization
- optionally introduce a normalized internal comparison view for selected operands

This stage should be driven by real corpus evidence, not aesthetic preference.

### Stage 5: Text diff refinement

For text bundles:

- add line-anchor matching
- add token-aware middle-block matching
- retain generic byte fallback for pathological inputs

### Stage 6: Benchmarks and corpus evaluation

Add:

- fixture corpus management
- benchmark commands and report conventions
- artifact reports comparing `chiff` to HDiffPatch / bsdiff / xdelta3 / zstd patch-from

### Stage 7: Hpatch-compatible output

For existing `react-native-update` clients:

- feed `chiff` cover plans into HDiffPatch `ICoverLinesListener` through
  `node-hdiffpatch.diffWithCovers` (done)
- keep existing server task types (`hdiff`, `hdiffFrom*`, `phdiff`) and SDK
  hpatch apply code unchanged (done)
- compare native hdiff and `chiff`-cover serialized payloads and keep the
  smaller one in `react-native-update-cli` (done)
- compare `chiff` covers, hdiff-native covers, and merged-costed covers at the
  cover-policy level
- emit standard hpatch-compatible payloads (done through `node-hdiffpatch`)
- validate with a file-level single-compressed hpatch apply path (done through
  `node-hdiffpatch.patchSingleStream`)

### Stage 8: Native `chiff` format

For opt-in maximum compression:

- design a custom container separate from hpatch-compatible output
- support Hermes section/function/debug-data operations directly
- allow canonicalized comparison views with explicit reverse transforms
- benchmark native output against both hpatch-compatible `chiff` and plain hdiff

## Validation Strategy

Every parsing or diff refinement should satisfy three layers of validation:

### Unit tests

Validate:

- field extraction
- alignment handling
- section and function offsets
- invalid layout rejection

### Round-trip tests

Validate:

- `apply_patch(old, diff_bytes(old, new)) == new`

for:

- text
- generic binary
- Hermes small-header bundles
- Hermes overflowed-header bundles

### Behavioral preservation tests

Validate the optimization property, not just correctness:

- unchanged section remains copyable after earlier section growth
- unchanged function remains copyable after earlier function growth
- same property holds for overflowed large-header functions
- unchanged overflowed function info block remains copyable between changed neighboring info blocks
- unchanged exception table remains copyable inside a changed overflowed info block

## Compatibility Strategy

### Rust crate

The Rust API is the source of truth.
New public types should only be added when they represent stable structure that is likely to remain useful for bindings and benchmarks.

### Node / Bun

Bindings should stay compatibility-oriented:

- thin API surface
- no duplicated parsing logic
- no JS-side structural interpretation

That now includes corpus analysis as well: the Node/Bun runner delegates directory walking, pairing, and summary aggregation to Rust rather than re-implementing those rules in JavaScript.

If new APIs are exposed to Node/Bun, they should come from crate-level stable functions, not binding-only helpers.

## Immediate Next Tasks

The immediate next implementation step after this document is:

1. Add corpus reporting that compares `hdiff_native`, `chiff_structured`, and
   eventually `merged_costed` output. The first serialized-size report now
   exists; it still needs broader corpus input and CI-friendly saved outputs.
2. Continue Hermes/text algorithm work only after each optimization is classified
   as `OriginalByteCover` or `NativeOnly`.
3. Start the native `chiff` container design separately, so native-only
   transforms can pursue the upper bound without weakening hpatch compatibility.

After that, the most valuable branch point is:

- cover-policy selection for the hpatch-compatible bridge, or
- native `chiff` container MVP

That choice should be driven by real patch-size, diff-time, apply-time, and
rollback-risk measurements, not by intuition alone.
