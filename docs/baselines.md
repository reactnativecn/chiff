# Baselines

`chiff` is not trying to replace every existing diff algorithm with one generic strategy.
The current external baselines we should compare against are:

- `bsdiff` / `bsdiff4`: classic suffix-array based binary diff baseline.
- `HDiffPatch`: practical binary diff baseline already used in the surrounding ecosystem.
- `xdelta3` / `VCDIFF`: mainstream dictionary-style delta format and implementation baseline.
- `zstd --patch-from`: modern practical patch generation baseline for byte-oriented workloads.

For `chiff`, these are comparison targets, not implementation dependencies.
The goal is to beat or justify divergence from them on:

- patch size
- diff generation time
- patch apply time
- memory usage
- robustness on Hermes bytecode and UTF-8 text bundles
