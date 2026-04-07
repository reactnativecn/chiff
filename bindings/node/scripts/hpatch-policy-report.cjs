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

function packedUIntSize(value, tagBits = 0) {
  let current = BigInt(value);
  const maxValueWithTag = (1n << BigInt(7 - tagBits)) - 1n;
  let size = 1;
  while (current > maxValueWithTag) {
    size += 1;
    current >>= 7n;
  }
  return size;
}

function hpatchCoverControlBytes(covers) {
  let lastOldEnd = 0n;
  let lastNewEnd = 0n;
  let total = 0;

  for (const cover of covers) {
    const oldPos = BigInt(cover.oldPos);
    const newPos = BigInt(cover.newPos);
    const len = BigInt(cover.len);
    total += packedUIntSize(
      oldPos >= lastOldEnd ? oldPos - lastOldEnd : lastOldEnd - oldPos,
      1,
    );
    total += packedUIntSize(newPos - lastNewEnd);
    total += packedUIntSize(len);
    lastOldEnd = oldPos + len;
    lastNewEnd = newPos + len;
  }

  return total;
}

function coverLength(covers) {
  return covers.reduce((total, cover) => total + BigInt(cover.len), 0n);
}

function uncoveredNewBytes(newSize, covers) {
  return BigInt(newSize) - coverLength(covers);
}

function coverNewEnd(cover) {
  return BigInt(cover.newPos) + BigInt(cover.len);
}

function bytesOutsideBlockers(covers, blockers) {
  let total = 0n;
  let blockerIndex = 0;

  for (const cover of covers) {
    let cursor = BigInt(cover.newPos);
    const coverEnd = coverNewEnd(cover);

    while (
      blockerIndex < blockers.length &&
      coverNewEnd(blockers[blockerIndex]) <= cursor
    ) {
      blockerIndex += 1;
    }

    let scanIndex = blockerIndex;
    while (
      scanIndex < blockers.length &&
      BigInt(blockers[scanIndex].newPos) < coverEnd
    ) {
      const blockerStart = BigInt(blockers[scanIndex].newPos);
      const blockerEnd = coverNewEnd(blockers[scanIndex]);
      if (blockerStart > cursor) {
        total += (blockerStart < coverEnd ? blockerStart : coverEnd) - cursor;
      }
      if (blockerEnd > cursor) {
        cursor = blockerEnd;
        if (cursor >= coverEnd) {
          break;
        }
      }
      scanIndex += 1;
    }

    if (cursor < coverEnd) {
      total += coverEnd - cursor;
    }
  }

  return total;
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
  mergedPatchBytes: 0,
  approximateMergedPatchBytes: 0,
  coalescedNativePatchBytes: 0,
  selectedPatchBytes: 0,
  structuredSmaller: 0,
  structuredEqual: 0,
  structuredLarger: 0,
  mergedSmaller: 0,
  mergedEqual: 0,
  mergedLarger: 0,
  approximateMergedSmaller: 0,
  approximateMergedEqual: 0,
  approximateMergedLarger: 0,
  coalescedNativeSmaller: 0,
  coalescedNativeEqual: 0,
  coalescedNativeLarger: 0,
  selectedNative: 0,
  selectedStructured: 0,
  selectedMerged: 0,
  selectedApproximateMerged: 0,
  selectedCoalescedNative: 0,
  nativeDiffMs: 0,
  structuredPlanMs: 0,
  approximatePlanMs: 0,
  structuredDiffMs: 0,
  mergedDiffMs: 0,
  approximateMergedDiffMs: 0,
  coalescedNativeDiffMs: 0,
  nativePatchMs: 0,
  structuredPatchMs: 0,
  mergedPatchMs: 0,
  approximateMergedPatchMs: 0,
  coalescedNativePatchMs: 0,
  structuredCoveredBytes: 0n,
  structuredUncoveredNewBytes: 0n,
  structuredCoverControlBytes: 0,
  approximateCoveredBytes: 0n,
  approximateUncoveredNewBytes: 0n,
  approximateCoverControlBytes: 0,
  nativeCoveredBytes: 0n,
  nativeUncoveredNewBytes: 0n,
  chiffCoverNativeGapBytes: 0n,
  approximateCoverNativeGapBytes: 0n,
  finalCoveredBytes: 0n,
  approximateFinalCoveredBytes: 0n,
  mergeExtraCoverCount: 0,
  approximateMergeExtraCoverCount: 0,
  coalescedNativeCoverCount: 0,
};
let failedRoundtrip = false;

const HEADER_COLUMNS = [
  'path',
  'status',
  'old_format',
  'new_format',
  'engine_reason',
  'cover_count',
  'covered_bytes',
    'uncovered_new_bytes',
    'cover_control_bytes',
    'native_cover_count',
    'native_covered_bytes',
    'native_uncovered_new_bytes',
    'chiff_cover_native_gap_bytes',
    'final_covered_bytes',
    'native_patch_bytes',
  'structured_patch_bytes',
  'merged_patch_bytes',
  'approx_merged_patch_bytes',
  'coalesced_native_patch_bytes',
  'selected_patch_bytes',
  'structured_delta_bytes',
  'merged_delta_bytes',
  'approx_merged_delta_bytes',
  'coalesced_native_delta_bytes',
  'selected_delta_bytes',
  'winner',
  'native_diff_ms',
  'structured_plan_ms',
  'approx_plan_ms',
  'structured_diff_ms',
  'merged_diff_ms',
  'approx_merged_diff_ms',
  'coalesced_native_diff_ms',
  'native_file_patch_ms',
  'structured_file_patch_ms',
  'merged_file_patch_ms',
  'approx_merged_file_patch_ms',
  'coalesced_native_file_patch_ms',
  'native_file_roundtrip',
  'structured_file_roundtrip',
  'merged_file_roundtrip',
  'approx_merged_file_roundtrip',
  'coalesced_native_file_roundtrip',
  'used_covers',
  'merge_used_covers',
  'approx_merge_used_covers',
  'merge_native_cover_capacity',
  'merge_final_cover_count',
  'merge_extra_cover_count',
  'approx_cover_count',
  'approx_covered_bytes',
  'approx_uncovered_new_bytes',
  'approx_cover_control_bytes',
  'approx_chiff_cover_native_gap_bytes',
  'approx_final_covered_bytes',
  'approx_merge_final_cover_count',
  'approx_merge_extra_cover_count',
  'coalesced_native_cover_count',
];

console.error(`hdiff_module=${modulePath}`);
console.log(HEADER_COLUMNS.join('\t'));

for (const entry of analysis.entries) {
  if (entry.status !== 'paired') {
    summary.skipped += 1;
    const row = new Array(HEADER_COLUMNS.length).fill('-');
    row[0] = entry.relativePath;
    row[1] = entry.status;
    row[2] = entry.oldFormat;
    row[3] = entry.newFormat;
    row[16] = 'skipped';
    console.log(row.join('\t'));
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
  const approximatePlan = measure(() =>
    chiff.hpatchApproximatePlanResult(oldBytes, newBytes),
  );
  const structuredDiff = measure(() =>
    hdiffpatch.diffWithCovers(oldBytes, newBytes, plan.value.covers, {
      mode: 'replace',
    }),
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
  const mergedDiff = measure(() =>
    hdiffpatch.diffWithCovers(oldBytes, newBytes, plan.value.covers, {
      mode: 'merge',
      debugCovers: true,
    }),
  );
  const mergedPatchBytes = patchBytesFromResult(mergedDiff.value);
  const mergedPatch = measure(() =>
    applySinglePatchFile(
      hdiffpatch,
      oldPath,
      mergedPatchBytes,
      newBytes,
      'merged',
    ),
  );
  const approximateMergedDiff =
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
      : { value: nativeDiff.value, elapsedMs: 0 };
  const approximateMergedPatchBytes = patchBytesFromResult(
    approximateMergedDiff.value,
  );
  const approximateMergedPatch =
    approximatePlan.value.coverCount > 0
      ? measure(() =>
          applySinglePatchFile(
            hdiffpatch,
            oldPath,
            approximateMergedPatchBytes,
            newBytes,
            'approx-merged',
          ),
        )
      : { value: nativePatch.value, elapsedMs: 0 };
  const nativeBytes = nativeDiff.value.length;
  const structuredBytes = structuredPatchBytes.length;
  const mergedBytes = mergedPatchBytes.length;
  const approximateMergedBytes = approximateMergedPatchBytes.length;
  const structuredCoveredBytes = BigInt(plan.value.coveredBytes);
  const structuredUncoveredNewBytes = BigInt(plan.value.uncoveredNewBytes);
  const structuredCoverControlBytes = hpatchCoverControlBytes(
    plan.value.covers,
  );
  const approximateCoveredBytes = BigInt(approximatePlan.value.coveredBytes);
  const approximateUncoveredNewBytes = BigInt(
    approximatePlan.value.uncoveredNewBytes,
  );
  const approximateCoverControlBytes = hpatchCoverControlBytes(
    approximatePlan.value.covers,
  );
  const nativeCovers = mergedDiff.value.nativeCovers ?? [];
  const finalCovers = mergedDiff.value.finalCovers ?? [];
  const approximateFinalCovers = approximateMergedDiff.value.finalCovers ?? [];
  const coalescedNativeDiff = measure(() =>
    hdiffpatch.diffWithCovers(oldBytes, newBytes, [], {
      mode: 'native-coalesce',
      debugCovers: true,
    }),
  );
  const coalescedNativePatchBytes = patchBytesFromResult(
    coalescedNativeDiff.value,
  );
  const coalescedNativePatch = measure(() =>
    applySinglePatchFile(
      hdiffpatch,
      oldPath,
      coalescedNativePatchBytes,
      newBytes,
      'coalesced-native',
    ),
  );
  const nativeOk = nativePatch.value;
  const structuredOk = structuredPatch.value;
  const mergedOk = mergedPatch.value;
  const approximateMergedOk = approximateMergedPatch.value;
  const coalescedNativeOk = coalescedNativePatch.value;
  if (
    !nativeOk ||
    !structuredOk ||
    !mergedOk ||
    !approximateMergedOk ||
    !coalescedNativeOk
  ) {
    failedRoundtrip = true;
  }
  const nativeCoveredBytes = coverLength(nativeCovers);
  const nativeUncoveredNewBytes = uncoveredNewBytes(
    newBytes.length,
    nativeCovers,
  );
  const chiffCoverNativeGapBytes = bytesOutsideBlockers(
    plan.value.covers,
    nativeCovers,
  );
  const approximateCoverNativeGapBytes = bytesOutsideBlockers(
    approximatePlan.value.covers,
    nativeCovers,
  );
  const finalCoveredBytes = coverLength(finalCovers);
  const approximateFinalCoveredBytes = coverLength(approximateFinalCovers);
  const mergeNativeCoverCapacity =
    mergedDiff.value.nativeCoverCapacity ?? 0;
  const mergeFinalCoverCount = mergedDiff.value.finalCoverCount ?? 0;
  const mergeExtraCoverCount = Math.max(
    0,
    mergeFinalCoverCount - mergeNativeCoverCapacity,
  );
  const approximateMergeFinalCoverCount =
    approximateMergedDiff.value.finalCoverCount ?? nativeCovers.length;
  const approximateMergeExtraCoverCount = Math.max(
    0,
    approximateMergeFinalCoverCount - (approximateMergedDiff.value.nativeCoverCapacity ?? nativeCovers.length),
  );
  const structuredDeltaBytes = structuredBytes - nativeBytes;
  const mergedDeltaBytes = mergedBytes - nativeBytes;
  const approximateMergedDeltaBytes = approximateMergedBytes - nativeBytes;
  const coalescedNativeBytes = coalescedNativePatchBytes.length;
  const coalescedNativeDeltaBytes = coalescedNativeBytes - nativeBytes;
  const candidates = [
    { name: 'hdiff_native', bytes: nativeBytes },
    { name: 'chiff_structured', bytes: structuredBytes },
    { name: 'chiff_merged', bytes: mergedBytes },
    { name: 'chiff_approx_merged', bytes: approximateMergedBytes },
    { name: 'hdiff_native_coalesced', bytes: coalescedNativeBytes },
  ];
  const selected = candidates.reduce((best, candidate) =>
    candidate.bytes < best.bytes ? candidate : best,
  );
  const selectedBytes = selected.bytes;
  const selectedDeltaBytes = selectedBytes - nativeBytes;
  const winner = selected.name;

  summary.paired += 1;
  summary.nativePatchBytes += nativeBytes;
  summary.structuredPatchBytes += structuredBytes;
  summary.mergedPatchBytes += mergedBytes;
  summary.approximateMergedPatchBytes += approximateMergedBytes;
  summary.coalescedNativePatchBytes += coalescedNativeBytes;
  summary.selectedPatchBytes += selectedBytes;
  summary.nativeDiffMs += nativeDiff.elapsedMs;
  summary.structuredPlanMs += plan.elapsedMs;
  summary.approximatePlanMs += approximatePlan.elapsedMs;
  summary.structuredDiffMs += structuredDiff.elapsedMs;
  summary.mergedDiffMs += mergedDiff.elapsedMs;
  summary.approximateMergedDiffMs += approximateMergedDiff.elapsedMs;
  summary.coalescedNativeDiffMs += coalescedNativeDiff.elapsedMs;
  summary.nativePatchMs += nativePatch.elapsedMs;
  summary.structuredPatchMs += structuredPatch.elapsedMs;
  summary.mergedPatchMs += mergedPatch.elapsedMs;
  summary.approximateMergedPatchMs += approximateMergedPatch.elapsedMs;
  summary.coalescedNativePatchMs += coalescedNativePatch.elapsedMs;
  summary.structuredCoveredBytes += structuredCoveredBytes;
  summary.structuredUncoveredNewBytes += structuredUncoveredNewBytes;
  summary.structuredCoverControlBytes += structuredCoverControlBytes;
  summary.approximateCoveredBytes += approximateCoveredBytes;
  summary.approximateUncoveredNewBytes += approximateUncoveredNewBytes;
  summary.approximateCoverControlBytes += approximateCoverControlBytes;
  summary.nativeCoveredBytes += nativeCoveredBytes;
  summary.nativeUncoveredNewBytes += nativeUncoveredNewBytes;
  summary.chiffCoverNativeGapBytes += chiffCoverNativeGapBytes;
  summary.approximateCoverNativeGapBytes += approximateCoverNativeGapBytes;
  summary.finalCoveredBytes += finalCoveredBytes;
  summary.approximateFinalCoveredBytes += approximateFinalCoveredBytes;
  summary.mergeExtraCoverCount += mergeExtraCoverCount;
  summary.approximateMergeExtraCoverCount += approximateMergeExtraCoverCount;
  summary.coalescedNativeCoverCount +=
    coalescedNativeDiff.value.finalCoverCount ?? 0;
  if (structuredDeltaBytes < 0) {
    summary.structuredSmaller += 1;
  } else if (structuredDeltaBytes > 0) {
    summary.structuredLarger += 1;
  } else {
    summary.structuredEqual += 1;
  }
  if (mergedDeltaBytes < 0) {
    summary.mergedSmaller += 1;
  } else if (mergedDeltaBytes > 0) {
    summary.mergedLarger += 1;
  } else {
    summary.mergedEqual += 1;
  }
  if (approximateMergedDeltaBytes < 0) {
    summary.approximateMergedSmaller += 1;
  } else if (approximateMergedDeltaBytes > 0) {
    summary.approximateMergedLarger += 1;
  } else {
    summary.approximateMergedEqual += 1;
  }
  if (coalescedNativeDeltaBytes < 0) {
    summary.coalescedNativeSmaller += 1;
  } else if (coalescedNativeDeltaBytes > 0) {
    summary.coalescedNativeLarger += 1;
  } else {
    summary.coalescedNativeEqual += 1;
  }
  if (winner === 'chiff_structured') {
    summary.selectedStructured += 1;
  } else if (winner === 'chiff_merged') {
    summary.selectedMerged += 1;
  } else if (winner === 'chiff_approx_merged') {
    summary.selectedApproximateMerged += 1;
  } else if (winner === 'hdiff_native_coalesced') {
    summary.selectedCoalescedNative += 1;
  } else {
    summary.selectedNative += 1;
  }

  console.log(
    [
      entry.relativePath,
      entry.status,
      entry.oldFormat,
      entry.newFormat,
      entry.engineReason,
      plan.value.coverCount,
      structuredCoveredBytes.toString(),
      structuredUncoveredNewBytes.toString(),
      structuredCoverControlBytes,
      nativeCovers.length,
      nativeCoveredBytes.toString(),
      nativeUncoveredNewBytes.toString(),
      chiffCoverNativeGapBytes.toString(),
      finalCoveredBytes.toString(),
      nativeBytes,
      structuredBytes,
      mergedBytes,
      approximateMergedBytes,
      coalescedNativeBytes,
      selectedBytes,
      structuredDeltaBytes,
      mergedDeltaBytes,
      approximateMergedDeltaBytes,
      coalescedNativeDeltaBytes,
      selectedDeltaBytes,
      winner,
      toFixedMs(nativeDiff.elapsedMs),
      toFixedMs(plan.elapsedMs),
      toFixedMs(approximatePlan.elapsedMs),
      toFixedMs(structuredDiff.elapsedMs),
      toFixedMs(mergedDiff.elapsedMs),
      toFixedMs(approximateMergedDiff.elapsedMs),
      toFixedMs(coalescedNativeDiff.elapsedMs),
      toFixedMs(nativePatch.elapsedMs),
      toFixedMs(structuredPatch.elapsedMs),
      toFixedMs(mergedPatch.elapsedMs),
      toFixedMs(approximateMergedPatch.elapsedMs),
      toFixedMs(coalescedNativePatch.elapsedMs),
      nativeOk,
      structuredOk,
      mergedOk,
      approximateMergedOk,
      coalescedNativeOk,
      Boolean(structuredDiff.value.usedCovers),
      Boolean(mergedDiff.value.usedCovers),
      Boolean(approximateMergedDiff.value.usedCovers),
      mergeNativeCoverCapacity,
      mergeFinalCoverCount,
      mergeExtraCoverCount,
      approximatePlan.value.coverCount,
      approximateCoveredBytes.toString(),
      approximateUncoveredNewBytes.toString(),
      approximateCoverControlBytes,
      approximateCoverNativeGapBytes.toString(),
      approximateFinalCoveredBytes.toString(),
      approximateMergeFinalCoverCount,
      approximateMergeExtraCoverCount,
      coalescedNativeDiff.value.finalCoverCount ?? 0,
    ].join('\t'),
  );
}

const totalDeltaBytes =
  summary.structuredPatchBytes - summary.nativePatchBytes;
const mergedDeltaBytes = summary.mergedPatchBytes - summary.nativePatchBytes;
const approximateMergedDeltaBytes =
  summary.approximateMergedPatchBytes - summary.nativePatchBytes;
const coalescedNativeDeltaBytes =
  summary.coalescedNativePatchBytes - summary.nativePatchBytes;
const selectedDeltaBytes =
  summary.selectedPatchBytes - summary.nativePatchBytes;
console.log(
  [
    'TOTAL',
    `paired=${summary.paired}`,
    `skipped=${summary.skipped}`,
    `native_patch_bytes=${summary.nativePatchBytes}`,
    `structured_patch_bytes=${summary.structuredPatchBytes}`,
    `merged_patch_bytes=${summary.mergedPatchBytes}`,
    `approx_merged_patch_bytes=${summary.approximateMergedPatchBytes}`,
    `coalesced_native_patch_bytes=${summary.coalescedNativePatchBytes}`,
    `selected_patch_bytes=${summary.selectedPatchBytes}`,
    `structured_covered_bytes=${summary.structuredCoveredBytes.toString()}`,
    `structured_uncovered_new_bytes=${summary.structuredUncoveredNewBytes.toString()}`,
    `structured_cover_control_bytes=${summary.structuredCoverControlBytes}`,
    `approx_covered_bytes=${summary.approximateCoveredBytes.toString()}`,
    `approx_uncovered_new_bytes=${summary.approximateUncoveredNewBytes.toString()}`,
    `approx_cover_control_bytes=${summary.approximateCoverControlBytes}`,
    `native_covered_bytes=${summary.nativeCoveredBytes.toString()}`,
    `native_uncovered_new_bytes=${summary.nativeUncoveredNewBytes.toString()}`,
    `chiff_cover_native_gap_bytes=${summary.chiffCoverNativeGapBytes.toString()}`,
    `approx_chiff_cover_native_gap_bytes=${summary.approximateCoverNativeGapBytes.toString()}`,
    `final_covered_bytes=${summary.finalCoveredBytes.toString()}`,
    `approx_final_covered_bytes=${summary.approximateFinalCoveredBytes.toString()}`,
    `merge_extra_cover_count=${summary.mergeExtraCoverCount}`,
    `approx_merge_extra_cover_count=${summary.approximateMergeExtraCoverCount}`,
    `coalesced_native_cover_count=${summary.coalescedNativeCoverCount}`,
    `structured_delta_bytes=${totalDeltaBytes}`,
    `merged_delta_bytes=${mergedDeltaBytes}`,
    `approx_merged_delta_bytes=${approximateMergedDeltaBytes}`,
    `coalesced_native_delta_bytes=${coalescedNativeDeltaBytes}`,
    `selected_delta_bytes=${selectedDeltaBytes}`,
    `structured_smaller=${summary.structuredSmaller}`,
    `structured_equal=${summary.structuredEqual}`,
    `structured_larger=${summary.structuredLarger}`,
    `merged_smaller=${summary.mergedSmaller}`,
    `merged_equal=${summary.mergedEqual}`,
    `merged_larger=${summary.mergedLarger}`,
    `approx_merged_smaller=${summary.approximateMergedSmaller}`,
    `approx_merged_equal=${summary.approximateMergedEqual}`,
    `approx_merged_larger=${summary.approximateMergedLarger}`,
    `coalesced_native_smaller=${summary.coalescedNativeSmaller}`,
    `coalesced_native_equal=${summary.coalescedNativeEqual}`,
    `coalesced_native_larger=${summary.coalescedNativeLarger}`,
    `selected_native=${summary.selectedNative}`,
    `selected_structured=${summary.selectedStructured}`,
    `selected_merged=${summary.selectedMerged}`,
    `selected_approx_merged=${summary.selectedApproximateMerged}`,
    `selected_coalesced_native=${summary.selectedCoalescedNative}`,
    `native_diff_ms=${toFixedMs(summary.nativeDiffMs)}`,
    `structured_plan_ms=${toFixedMs(summary.structuredPlanMs)}`,
    `approx_plan_ms=${toFixedMs(summary.approximatePlanMs)}`,
    `structured_diff_ms=${toFixedMs(summary.structuredDiffMs)}`,
    `merged_diff_ms=${toFixedMs(summary.mergedDiffMs)}`,
    `approx_merged_diff_ms=${toFixedMs(summary.approximateMergedDiffMs)}`,
    `coalesced_native_diff_ms=${toFixedMs(summary.coalescedNativeDiffMs)}`,
    `native_file_patch_ms=${toFixedMs(summary.nativePatchMs)}`,
    `structured_file_patch_ms=${toFixedMs(summary.structuredPatchMs)}`,
    `merged_file_patch_ms=${toFixedMs(summary.mergedPatchMs)}`,
    `approx_merged_file_patch_ms=${toFixedMs(summary.approximateMergedPatchMs)}`,
    `coalesced_native_file_patch_ms=${toFixedMs(summary.coalescedNativePatchMs)}`,
  ].join('\t'),
);

if (failedRoundtrip) {
  process.exit(1);
}
