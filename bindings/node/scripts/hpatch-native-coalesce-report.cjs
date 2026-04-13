'use strict';

const fs = require('node:fs');
const path = require('node:path');
const chiff = require('../index.cjs');

function printUsageAndExit() {
  console.error(
    [
      'usage: node ./scripts/hpatch-native-coalesce-report.cjs <pairs-root> [--hdiff-module <path-or-package>]',
      '',
      'The pairs root should contain child directories with v1/ and v2/ subdirectories.',
      'This fast report compares native hdiff, native cover coalescing, and coarse approximate Hermes merge.',
    ].join('\n'),
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
      hdiffModule = argv[i];
    } else if (arg.startsWith('--hdiff-module=')) {
      hdiffModule = arg.slice('--hdiff-module='.length);
    } else if (arg.startsWith('--')) {
      printUsageAndExit();
    } else {
      positional.push(arg);
    }
  }

  if (positional.length !== 1) {
    printUsageAndExit();
  }

  return {
    pairsRoot: path.resolve(positional[0]),
    hdiffModule,
  };
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
      if (typeof mod.diff !== 'function' || typeof mod.diffWithCovers !== 'function') {
        failures.push(`${candidate}: missing diff/diffWithCovers`);
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
      'Unable to load a node-hdiffpatch module with diffWithCovers.',
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

const { pairsRoot, hdiffModule } = parseArgs(process.argv.slice(2));
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
  nativeBytes: 0,
  coalescedBytes: 0,
  approximateBytes: 0,
  selectedBytes: 0,
  coalescedSmaller: 0,
  coalescedEqual: 0,
  coalescedLarger: 0,
  approximateSmaller: 0,
  approximateEqual: 0,
  approximateLarger: 0,
  selectedCoalesced: 0,
  selectedApproximate: 0,
  selectedNative: 0,
  nativeMs: 0,
  coalescedMs: 0,
  approximatePlanMs: 0,
  approximateDiffMs: 0,
};
let failedRoundtrip = false;

console.error(`hdiff_module=${modulePath}`);
console.log(
  [
    'pair',
    'kind',
    'native_patch_bytes',
    'coalesced_patch_bytes',
    'approx_merged_patch_bytes',
    'coalesced_delta_bytes',
    'approx_merged_delta_bytes',
    'winner',
    'native_cover_count',
    'coalesced_cover_count',
    'approx_cover_count',
    'approx_merge_final_cover_count',
    'approx_merge_extra_cover_count',
    'native_diff_ms',
    'coalesced_diff_ms',
    'approx_plan_ms',
    'approx_merged_diff_ms',
    'coalesced_roundtrip',
    'approx_merged_roundtrip',
  ].join('\t'),
);

for (const pairName of pairNames) {
  const pairRoot = path.join(pairsRoot, pairName);
  for (const entry of filesForPair(pairRoot)) {
    const oldBytes = fs.readFileSync(entry.oldPath);
    const newBytes = fs.readFileSync(entry.newPath);
    const native = measure(() => hdiffpatch.diff(oldBytes, newBytes));
    const coalesced = measure(() =>
      hdiffpatch.diffWithCovers(oldBytes, newBytes, [], {
        mode: 'native-coalesce',
        debugCovers: true,
      }),
    );
    const coalescedDiff = patchBytesFromResult(coalesced.value);
    const roundtrip = sameBytes(hdiffpatch.patch(oldBytes, coalescedDiff), newBytes);
    const approximatePlan = measure(() =>
      chiff.hpatchApproximatePlanResult(oldBytes, newBytes),
    );
    const approximateMerged =
      approximatePlan.value.coverCount > 0
        ? measure(() =>
            hdiffpatch.diffWithCovers(
              oldBytes,
              newBytes,
              approximatePlan.value.covers,
              {
                mode: 'merge',
                debugCovers: true,
              },
            ),
          )
        : { value: native.value, elapsedMs: 0 };
    const approximateMergedDiff = patchBytesFromResult(approximateMerged.value);
    const approximateRoundtrip = sameBytes(
      hdiffpatch.patch(oldBytes, approximateMergedDiff),
      newBytes,
    );
    if (!roundtrip) {
      failedRoundtrip = true;
    }
    if (!approximateRoundtrip) {
      failedRoundtrip = true;
    }

    const coalescedDeltaBytes = coalescedDiff.length - native.value.length;
    const approximateDeltaBytes =
      approximateMergedDiff.length - native.value.length;
    const winner =
      approximateMergedDiff.length < coalescedDiff.length &&
      approximateMergedDiff.length < native.value.length
        ? 'chiff_approx_merged'
        : coalescedDiff.length < native.value.length
          ? 'native-coalesce'
          : 'hdiff_native';
    summary.files += 1;
    summary.nativeBytes += native.value.length;
    summary.coalescedBytes += coalescedDiff.length;
    summary.approximateBytes += approximateMergedDiff.length;
    summary.selectedBytes += Math.min(
      native.value.length,
      coalescedDiff.length,
      approximateMergedDiff.length,
    );
    summary.nativeMs += native.elapsedMs;
    summary.coalescedMs += coalesced.elapsedMs;
    summary.approximatePlanMs += approximatePlan.elapsedMs;
    summary.approximateDiffMs += approximateMerged.elapsedMs;
    if (coalescedDeltaBytes < 0) {
      summary.coalescedSmaller += 1;
    } else if (coalescedDeltaBytes > 0) {
      summary.coalescedLarger += 1;
    } else {
      summary.coalescedEqual += 1;
    }
    if (approximateDeltaBytes < 0) {
      summary.approximateSmaller += 1;
    } else if (approximateDeltaBytes > 0) {
      summary.approximateLarger += 1;
    } else {
      summary.approximateEqual += 1;
    }
    if (winner === 'chiff_approx_merged') {
      summary.selectedApproximate += 1;
    } else if (winner === 'native-coalesce') {
      summary.selectedCoalesced += 1;
    } else {
      summary.selectedNative += 1;
    }

    console.log(
      [
        pairName,
        entry.kind,
        native.value.length,
        coalescedDiff.length,
        approximateMergedDiff.length,
        coalescedDeltaBytes,
        approximateDeltaBytes,
        winner,
        coalesced.value.nativeCoverCapacity ?? '-',
        coalesced.value.finalCoverCount ?? '-',
        approximatePlan.value.coverCount,
        approximateMerged.value.finalCoverCount ?? '-',
        Math.max(
          0,
          (approximateMerged.value.finalCoverCount ?? 0) -
            (approximateMerged.value.nativeCoverCapacity ?? 0),
        ),
        toFixedMs(native.elapsedMs),
        toFixedMs(coalesced.elapsedMs),
        toFixedMs(approximatePlan.elapsedMs),
        toFixedMs(approximateMerged.elapsedMs),
        roundtrip,
        approximateRoundtrip,
      ].join('\t'),
    );
  }
}

console.log(
  [
    'TOTAL',
    `files=${summary.files}`,
    `native_patch_bytes=${summary.nativeBytes}`,
    `coalesced_patch_bytes=${summary.coalescedBytes}`,
    `approx_merged_patch_bytes=${summary.approximateBytes}`,
    `selected_patch_bytes=${summary.selectedBytes}`,
    `coalesced_delta_bytes=${summary.coalescedBytes - summary.nativeBytes}`,
    `approx_merged_delta_bytes=${summary.approximateBytes - summary.nativeBytes}`,
    `selected_delta_bytes=${summary.selectedBytes - summary.nativeBytes}`,
    `coalesced_smaller=${summary.coalescedSmaller}`,
    `coalesced_equal=${summary.coalescedEqual}`,
    `coalesced_larger=${summary.coalescedLarger}`,
    `approx_merged_smaller=${summary.approximateSmaller}`,
    `approx_merged_equal=${summary.approximateEqual}`,
    `approx_merged_larger=${summary.approximateLarger}`,
    `selected_native=${summary.selectedNative}`,
    `selected_coalesced=${summary.selectedCoalesced}`,
    `selected_approx_merged=${summary.selectedApproximate}`,
    `native_diff_ms=${toFixedMs(summary.nativeMs)}`,
    `coalesced_diff_ms=${toFixedMs(summary.coalescedMs)}`,
    `approx_plan_ms=${toFixedMs(summary.approximatePlanMs)}`,
    `approx_merged_diff_ms=${toFixedMs(summary.approximateDiffMs)}`,
  ].join('\t'),
);

if (failedRoundtrip) {
  process.exit(1);
}
