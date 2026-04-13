'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function printUsageAndExit() {
  console.error(
    [
      'usage: node ./scripts/zstd-hdiff-report.cjs <pairs-root> [--hdiff-module <path-or-package>] [--zstd-bin <path>] [--levels <csv>]',
      '',
      'The pairs root should contain child directories with v1/ and v2/ subdirectories.',
      'Default zstd levels are 3 and 19.',
    ].join('\n'),
  );
  process.exit(2);
}

function parseArgs(argv) {
  const positional = [];
  let hdiffModule;
  let zstdBin = process.env.CHIFF_ZSTD_BIN || 'zstd';
  let levels = [3, 19];

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--hdiff-module') {
      i += 1;
      hdiffModule = argv[i];
    } else if (arg.startsWith('--hdiff-module=')) {
      hdiffModule = arg.slice('--hdiff-module='.length);
    } else if (arg === '--zstd-bin') {
      i += 1;
      zstdBin = argv[i];
    } else if (arg.startsWith('--zstd-bin=')) {
      zstdBin = arg.slice('--zstd-bin='.length);
    } else if (arg === '--levels') {
      i += 1;
      levels = parseLevels(argv[i]);
    } else if (arg.startsWith('--levels=')) {
      levels = parseLevels(arg.slice('--levels='.length));
    } else if (arg.startsWith('--')) {
      printUsageAndExit();
    } else {
      positional.push(arg);
    }
  }

  if (positional.length !== 1 || !zstdBin || levels.length === 0) {
    printUsageAndExit();
  }

  return {
    pairsRoot: path.resolve(positional[0]),
    hdiffModule,
    zstdBin,
    levels,
  };
}

function parseLevels(value) {
  return String(value)
    .split(',')
    .map((part) => Number(part.trim()))
    .filter((level) => Number.isInteger(level) && level >= 1 && level <= 22);
}

function resolveModuleCandidate(candidate) {
  if (candidate.startsWith('.') || path.isAbsolute(candidate)) {
    return path.resolve(candidate);
  }
  return candidate;
}

function loadHdiffPatch(requestedModule) {
  const candidates = [
    requestedModule,
    process.env.CHIFF_HDIFFPATCH_MODULE,
    'node-hdiffpatch',
    path.resolve(__dirname, '../../../../node-hdiffpatch'),
  ].filter(Boolean);
  const failures = [];

  for (const candidate of candidates) {
    const resolved = resolveModuleCandidate(candidate);
    try {
      const mod = require(resolved);
      if (typeof mod.diff !== 'function') {
        failures.push(`${candidate}: missing diff`);
        continue;
      }
      return { mod, modulePath: resolved };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      failures.push(`${candidate}: ${message}`);
    }
  }

  throw new Error(
    [
      'Unable to load a node-hdiffpatch module with diff.',
      'Use --hdiff-module <path-or-package> or CHIFF_HDIFFPATCH_MODULE.',
      ...failures.map((failure) => `- ${failure}`),
    ].join('\n'),
  );
}

function measure(fn) {
  const start = process.hrtime.bigint();
  const value = fn();
  const elapsedMs = Number(process.hrtime.bigint() - start) / 1_000_000;
  return { value, elapsedMs };
}

function sameBytes(left, right) {
  return left.length === right.length && Buffer.compare(left, right) === 0;
}

function toFixedMs(value) {
  return value.toFixed(3);
}

function filesForPair(pairRoot) {
  return [
    ['text', 'text/index.android.bundle'],
    ['hermes', 'hermes/index.android.hbc'],
  ]
    .map(([kind, relative]) => ({
      kind,
      relative,
      oldPath: path.join(pairRoot, 'v1', relative),
      newPath: path.join(pairRoot, 'v2', relative),
    }))
    .filter((entry) => fs.existsSync(entry.oldPath) && fs.existsSync(entry.newPath));
}

function zstdLevelArgs(level) {
  if (level <= 19) {
    return [`-${level}`];
  }
  return ['--ultra', `-${level}`];
}

function runZstdPatchFrom(zstdBin, level, oldPath, newPath, expectedBytes) {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'chiff-zstd-report-'));
  const patchPath = path.join(tempDir, 'patch.zst');
  const outPath = path.join(tempDir, 'out');

  try {
    const compress = measure(() =>
      spawnSync(
        zstdBin,
        [
          ...zstdLevelArgs(level),
          '--patch-from',
          oldPath,
          newPath,
          '-o',
          patchPath,
          '-q',
          '-f',
        ],
        { encoding: 'utf8' },
      ),
    );
    if (compress.value.status !== 0) {
      throw new Error(compress.value.stderr || `zstd level ${level} failed`);
    }

    const decompress = measure(() =>
      spawnSync(
        zstdBin,
        [
          '-d',
          '--patch-from',
          oldPath,
          patchPath,
          '-o',
          outPath,
          '-q',
          '-f',
        ],
        { encoding: 'utf8' },
      ),
    );
    if (decompress.value.status !== 0) {
      throw new Error(
        decompress.value.stderr || `zstd level ${level} decompression failed`,
      );
    }

    return {
      bytes: fs.statSync(patchPath).size,
      compressMs: compress.elapsedMs,
      decompressMs: decompress.elapsedMs,
      ok: sameBytes(fs.readFileSync(outPath), expectedBytes),
    };
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

const { pairsRoot, hdiffModule, zstdBin, levels } = parseArgs(
  process.argv.slice(2),
);
if (!fs.existsSync(pairsRoot) || !fs.statSync(pairsRoot).isDirectory()) {
  console.error(`pairs root is not a directory: ${pairsRoot}`);
  process.exit(2);
}

const { mod: hdiffpatch, modulePath } = loadHdiffPatch(hdiffModule);
const pairNames = fs
  .readdirSync(pairsRoot)
  .filter((name) => fs.statSync(path.join(pairsRoot, name)).isDirectory())
  .sort();
const summary = {
  files: 0,
  hdiffBytes: 0,
  hdiffMs: 0,
  zstd: new Map(levels.map((level) => [level, { bytes: 0, cMs: 0, dMs: 0 }])),
};
let failedRoundtrip = false;

console.error(`hdiff_module=${modulePath}`);
console.error(`zstd_bin=${zstdBin}`);
console.log(
  [
    'pair',
    'kind',
    'new_bytes',
    'hdiff_bytes',
    ...levels.map((level) => `zstd${level}_bytes`),
    ...levels.map((level) => `zstd${level}_delta_bytes`),
    'hdiff_ms',
    ...levels.map((level) => `zstd${level}_compress_ms`),
    ...levels.map((level) => `zstd${level}_decompress_ms`),
    'zstd_roundtrip',
  ].join('\t'),
);

for (const pairName of pairNames) {
  const pairRoot = path.join(pairsRoot, pairName);
  for (const entry of filesForPair(pairRoot)) {
    const oldBytes = fs.readFileSync(entry.oldPath);
    const newBytes = fs.readFileSync(entry.newPath);
    const hdiff = measure(() => hdiffpatch.diff(oldBytes, newBytes));
    const zstdResults = levels.map((level) => [
      level,
      runZstdPatchFrom(zstdBin, level, entry.oldPath, entry.newPath, newBytes),
    ]);
    const zstdOk = zstdResults.every(([, result]) => result.ok);
    if (!zstdOk) {
      failedRoundtrip = true;
    }

    summary.files += 1;
    summary.hdiffBytes += hdiff.value.length;
    summary.hdiffMs += hdiff.elapsedMs;
    for (const [level, result] of zstdResults) {
      const item = summary.zstd.get(level);
      item.bytes += result.bytes;
      item.cMs += result.compressMs;
      item.dMs += result.decompressMs;
    }

    console.log(
      [
        pairName,
        entry.kind,
        newBytes.length,
        hdiff.value.length,
        ...zstdResults.map(([, result]) => result.bytes),
        ...zstdResults.map(([, result]) => result.bytes - hdiff.value.length),
        toFixedMs(hdiff.elapsedMs),
        ...zstdResults.map(([, result]) => toFixedMs(result.compressMs)),
        ...zstdResults.map(([, result]) => toFixedMs(result.decompressMs)),
        zstdOk,
      ].join('\t'),
    );
  }
}

console.log(
  [
    'TOTAL',
    `files=${summary.files}`,
    `hdiff_bytes=${summary.hdiffBytes}`,
    ...levels.map((level) => `zstd${level}_bytes=${summary.zstd.get(level).bytes}`),
    ...levels.map(
      (level) =>
        `zstd${level}_delta_bytes=${
          summary.zstd.get(level).bytes - summary.hdiffBytes
        }`,
    ),
    `hdiff_ms=${toFixedMs(summary.hdiffMs)}`,
    ...levels.map((level) => `zstd${level}_compress_ms=${toFixedMs(summary.zstd.get(level).cMs)}`),
    ...levels.map((level) => `zstd${level}_decompress_ms=${toFixedMs(summary.zstd.get(level).dMs)}`),
  ].join('\t'),
);

if (failedRoundtrip) {
  process.exit(1);
}
