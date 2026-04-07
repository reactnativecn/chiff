'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const chiff = require('../index.cjs');

function printUsageAndExit() {
  console.error(
    'usage: node ./scripts/hpatch-policy-report.cjs <old-dir> <new-dir> [--hdiff-module <path-or-package>]',
  );
  process.exit(2);
}

function parseArgs(argv) {
  const positional = [];
  let hdiffModule;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--hdiff-module') {
      i += 1;
      if (!argv[i]) {
        printUsageAndExit();
      }
      hdiffModule = argv[i];
    } else if (arg.startsWith('--hdiff-module=')) {
      hdiffModule = arg.slice('--hdiff-module='.length);
    } else if (arg.startsWith('--')) {
      printUsageAndExit();
    } else {
      positional.push(arg);
    }
  }

  if (positional.length !== 2) {
    printUsageAndExit();
  }

  return {
    oldDir: path.resolve(positional[0]),
    newDir: path.resolve(positional[1]),
    hdiffModule,
  };
}

function assertDirectory(label, dir) {
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    console.error(`${label} is not a directory: ${dir}`);
    process.exit(2);
  }
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
      if (
        typeof mod.diff !== 'function' ||
        typeof mod.diffWithCovers !== 'function' ||
        typeof mod.patchSingleStream !== 'function'
      ) {
        failures.push(
          `${candidate}: missing diff/diffWithCovers/patchSingleStream`,
        );
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
      'Unable to load a node-hdiffpatch module with diffWithCovers and patchSingleStream.',
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

function toFixedMs(value) {
  return value.toFixed(3);
}

function patchBytesFromResult(result) {
  if (Buffer.isBuffer(result)) {
    return result;
  }
  if (Buffer.isBuffer(result?.diff)) {
    return result.diff;
  }
  throw new Error('diffWithCovers did not return a Buffer diff payload');
}

function sameBytes(left, right) {
  return left.length === right.length && Buffer.compare(left, right) === 0;
}

function applySinglePatchFile(hdiffpatch, oldPath, diffBytes, expectedBytes, label) {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'chiff-hpatch-report-'));
  const diffPath = path.join(tempDir, `${label}.hpatch`);
  const outPath = path.join(tempDir, `${label}.out`);

  try {
    fs.writeFileSync(diffPath, diffBytes);
    hdiffpatch.patchSingleStream(oldPath, diffPath, outPath);
    const actual = fs.readFileSync(outPath);
    return sameBytes(actual, expectedBytes);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

const { oldDir, newDir, hdiffModule } = parseArgs(process.argv.slice(2));
assertDirectory('old dir', oldDir);
assertDirectory('new dir', newDir);

const { mod: hdiffpatch, modulePath } = loadHdiffPatch(hdiffModule);
const analysis = chiff.analyzeDirectoryPairResult(oldDir, newDir);

const summary = {
  paired: 0,
  skipped: 0,
  nativePatchBytes: 0,
  structuredPatchBytes: 0,
  selectedPatchBytes: 0,
  structuredSmaller: 0,
  structuredEqual: 0,
  structuredLarger: 0,
  nativeDiffMs: 0,
  structuredPlanMs: 0,
  structuredDiffMs: 0,
  nativePatchMs: 0,
  structuredPatchMs: 0,
};
let failedRoundtrip = false;

console.error(`hdiff_module=${modulePath}`);
console.log(
  [
    'path',
    'status',
    'old_format',
    'new_format',
    'engine_reason',
    'cover_count',
    'native_patch_bytes',
    'structured_patch_bytes',
    'selected_patch_bytes',
    'delta_bytes',
    'winner',
    'native_diff_ms',
    'structured_plan_ms',
    'structured_diff_ms',
    'native_file_patch_ms',
    'structured_file_patch_ms',
    'native_file_roundtrip',
    'structured_file_roundtrip',
    'used_covers',
  ].join('\t'),
);

for (const entry of analysis.entries) {
  if (entry.status !== 'paired') {
    summary.skipped += 1;
    console.log(
      [
        entry.relativePath,
        entry.status,
        entry.oldFormat,
        entry.newFormat,
        '-',
        '-',
        '-',
        '-',
        '-',
        '-',
        'skipped',
        '-',
        '-',
        '-',
        '-',
        '-',
        '-',
        '-',
        '-',
      ].join('\t'),
    );
    continue;
  }

  const oldBytes = fs.readFileSync(path.join(oldDir, entry.relativePath));
  const oldPath = path.join(oldDir, entry.relativePath);
  const newBytes = fs.readFileSync(path.join(newDir, entry.relativePath));

  const nativeDiff = measure(() => hdiffpatch.diff(oldBytes, newBytes));
  const nativePatch = measure(() =>
    applySinglePatchFile(
      hdiffpatch,
      oldPath,
      nativeDiff.value,
      newBytes,
      'native',
    ),
  );
  const plan = measure(() =>
    chiff.hpatchCompatiblePlanResult(oldBytes, newBytes),
  );
  const structuredDiff = measure(() =>
    hdiffpatch.diffWithCovers(oldBytes, newBytes, plan.value.covers),
  );
  const structuredPatchBytes = patchBytesFromResult(structuredDiff.value);
  const structuredPatch = measure(() =>
    applySinglePatchFile(
      hdiffpatch,
      oldPath,
      structuredPatchBytes,
      newBytes,
      'structured',
    ),
  );

  const nativeOk = nativePatch.value;
  const structuredOk = structuredPatch.value;
  if (!nativeOk || !structuredOk) {
    failedRoundtrip = true;
  }

  const nativeBytes = nativeDiff.value.length;
  const structuredBytes = structuredPatchBytes.length;
  const deltaBytes = structuredBytes - nativeBytes;
  const selectedBytes = Math.min(nativeBytes, structuredBytes);
  const winner =
    deltaBytes < 0
      ? 'chiff_structured'
      : deltaBytes > 0
        ? 'hdiff_native'
        : 'tie';

  summary.paired += 1;
  summary.nativePatchBytes += nativeBytes;
  summary.structuredPatchBytes += structuredBytes;
  summary.selectedPatchBytes += selectedBytes;
  summary.nativeDiffMs += nativeDiff.elapsedMs;
  summary.structuredPlanMs += plan.elapsedMs;
  summary.structuredDiffMs += structuredDiff.elapsedMs;
  summary.nativePatchMs += nativePatch.elapsedMs;
  summary.structuredPatchMs += structuredPatch.elapsedMs;
  if (winner === 'chiff_structured') {
    summary.structuredSmaller += 1;
  } else if (winner === 'hdiff_native') {
    summary.structuredLarger += 1;
  } else {
    summary.structuredEqual += 1;
  }

  console.log(
    [
      entry.relativePath,
      entry.status,
      entry.oldFormat,
      entry.newFormat,
      entry.engineReason,
      plan.value.coverCount,
      nativeBytes,
      structuredBytes,
      selectedBytes,
      deltaBytes,
      winner,
      toFixedMs(nativeDiff.elapsedMs),
      toFixedMs(plan.elapsedMs),
      toFixedMs(structuredDiff.elapsedMs),
      toFixedMs(nativePatch.elapsedMs),
      toFixedMs(structuredPatch.elapsedMs),
      nativeOk,
      structuredOk,
      Boolean(structuredDiff.value.usedCovers),
    ].join('\t'),
  );
}

const totalDeltaBytes =
  summary.structuredPatchBytes - summary.nativePatchBytes;
const selectedDeltaBytes =
  summary.selectedPatchBytes - summary.nativePatchBytes;
console.log(
  [
    'TOTAL',
    `paired=${summary.paired}`,
    `skipped=${summary.skipped}`,
    `native_patch_bytes=${summary.nativePatchBytes}`,
    `structured_patch_bytes=${summary.structuredPatchBytes}`,
    `selected_patch_bytes=${summary.selectedPatchBytes}`,
    `delta_bytes=${totalDeltaBytes}`,
    `selected_delta_bytes=${selectedDeltaBytes}`,
    `structured_smaller=${summary.structuredSmaller}`,
    `structured_equal=${summary.structuredEqual}`,
    `structured_larger=${summary.structuredLarger}`,
    `native_diff_ms=${toFixedMs(summary.nativeDiffMs)}`,
    `structured_plan_ms=${toFixedMs(summary.structuredPlanMs)}`,
    `structured_diff_ms=${toFixedMs(summary.structuredDiffMs)}`,
    `native_file_patch_ms=${toFixedMs(summary.nativePatchMs)}`,
    `structured_file_patch_ms=${toFixedMs(summary.structuredPatchMs)}`,
  ].join('\t'),
);

if (failedRoundtrip) {
  process.exit(1);
}
