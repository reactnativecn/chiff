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

const oldFiles = collectRelativeFiles(oldDir);
const newFiles = collectRelativeFiles(newDir);
const relativePaths = Array.from(new Set([...oldFiles, ...newFiles])).sort();

let totalPairs = 0;
let totalCopyOps = 0;
let totalInsertOps = 0;
let totalCopiedBytes = 0;
let totalInsertedBytes = 0;

console.log(
  'path\tstatus\told_format\tnew_format\tselected_engine\tselected_engine_reason\told_structured_hermes_support\tnew_structured_hermes_support\top_count\tcopy_ops\tinsert_ops\tcopied_bytes\tinserted_bytes',
);

for (const relativePath of relativePaths) {
  const oldPath = path.join(oldDir, relativePath);
  const newPath = path.join(newDir, relativePath);
  const oldExists = fs.existsSync(oldPath) && fs.statSync(oldPath).isFile();
  const newExists = fs.existsSync(newPath) && fs.statSync(newPath).isFile();

  let status = 'paired';
  let oldBytes = null;
  let newBytes = null;

  if (oldExists) {
    oldBytes = fs.readFileSync(oldPath);
  }
  if (newExists) {
    newBytes = fs.readFileSync(newPath);
  }

  if (!oldExists) {
    status = 'missing_in_old';
  } else if (!newExists) {
    status = 'missing_in_new';
  }

  const oldFormat = oldBytes ? formatName(chiff.detectFormat(oldBytes)) : '-';
  const newFormat = newBytes ? formatName(chiff.detectFormat(newBytes)) : '-';

  let stats = {
    opCount: 0,
    copyOpCount: 0,
    insertOpCount: 0,
    copiedBytes: 0,
    insertedBytes: 0,
  };
  let selectedEngine = '-';
  let selectedEngineReason = '-';

  if (oldBytes && newBytes) {
    const decision = chiff.selectEngineDecisionResult(oldBytes, newBytes);
    selectedEngine = decision.kind;
    selectedEngineReason = decision.reason;
    stats = chiff.diffStats(oldBytes, newBytes);
    totalPairs += 1;
    totalCopyOps += stats.copyOpCount;
    totalInsertOps += stats.insertOpCount;
    totalCopiedBytes += stats.copiedBytes;
    totalInsertedBytes += stats.insertedBytes;
  }

  console.log(
    [
      relativePath,
      status,
      oldFormat,
      newFormat,
      selectedEngine,
      selectedEngineReason,
      oldBytes ? chiff.structuredHermesSupport(oldBytes).status : '-',
      newBytes ? chiff.structuredHermesSupport(newBytes).status : '-',
      stats.opCount,
      stats.copyOpCount,
      stats.insertOpCount,
      stats.copiedBytes,
      stats.insertedBytes,
    ].join('\t'),
  );
}

console.log(
  [
    'TOTAL',
    `paired=${totalPairs}`,
    '-',
    '-',
    '-',
    '-',
    '-',
    '-',
    '-',
    totalCopyOps,
    totalInsertOps,
    totalCopiedBytes,
    totalInsertedBytes,
  ].join('\t'),
);

function collectRelativeFiles(rootDir) {
  const files = [];
  collectRelativeFilesRecursive(rootDir, rootDir, files);
  return files;
}

function collectRelativeFilesRecursive(rootDir, currentDir, files) {
  const entries = fs.readdirSync(currentDir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(currentDir, entry.name);
    if (entry.isDirectory()) {
      collectRelativeFilesRecursive(rootDir, fullPath, files);
    } else if (entry.isFile()) {
      files.push(path.relative(rootDir, fullPath));
    }
  }
}

function formatName(result) {
  if (result.kind === 'hermes_bytecode') {
    return `${result.kind}:${result.form}@${result.version}`;
  }
  return result.kind;
}
