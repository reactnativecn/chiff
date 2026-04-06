'use strict';

const fs = require('node:fs');
const path = require('node:path');
const chiff = require('../index.cjs');

function printUsageAndExit() {
  console.error('usage: node ./scripts/corpus-diff-stats.cjs <old-dir> <new-dir>');
  process.exit(2);
}

const [, , oldDirArg, newDirArg, ...rest] = process.argv;
if (!oldDirArg || !newDirArg || rest.length > 0) {
  printUsageAndExit();
}

const oldDir = path.resolve(oldDirArg);
const newDir = path.resolve(newDirArg);

if (!fs.existsSync(oldDir) || !fs.statSync(oldDir).isDirectory()) {
  console.error(`old dir is not a directory: ${oldDir}`);
  process.exit(2);
}

if (!fs.existsSync(newDir) || !fs.statSync(newDir).isDirectory()) {
  console.error(`new dir is not a directory: ${newDir}`);
  process.exit(2);
}

const analysis = chiff.analyzeDirectoryPairResult(oldDir, newDir);

console.log(
  'path\tstatus\told_format\tnew_format\tselected_engine\tselected_engine_reason\told_structured_hermes_support\tnew_structured_hermes_support\top_count\tcopy_ops\tinsert_ops\tcopied_bytes\tinserted_bytes',
);

for (const entry of analysis.entries) {
  console.log(
    [
      entry.relativePath,
      entry.status,
      entry.oldFormat,
      entry.newFormat,
      entry.engineKind,
      entry.engineReason,
      entry.oldStructuredHermesSupport,
      entry.newStructuredHermesSupport,
      entry.opCount,
      entry.copyOpCount,
      entry.insertOpCount,
      entry.copiedBytes,
      entry.insertedBytes,
    ].join('\t'),
  );
}

console.log(
  [
    'TOTAL',
    `paired=${analysis.summary.paired}`,
    '-',
    '-',
    '-',
    '-',
    '-',
    '-',
    '-',
    analysis.summary.copyOps,
    analysis.summary.insertOps,
    analysis.summary.copiedBytes,
    analysis.summary.insertedBytes,
  ].join('\t'),
);

for (const entry of analysis.summary.reasonCounts) {
  console.log(['SUMMARY', 'reason', entry.key, entry.count].join('\t'));
}
for (const entry of analysis.summary.oldSupportCounts) {
  console.log(['SUMMARY', 'old_support', entry.key, entry.count].join('\t'));
}
for (const entry of analysis.summary.newSupportCounts) {
  console.log(['SUMMARY', 'new_support', entry.key, entry.count].join('\t'));
}
