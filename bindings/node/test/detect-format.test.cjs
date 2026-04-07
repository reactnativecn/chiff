'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const chiff = require('../index.cjs');

const HERMES_MAGIC = 0x1F1903C103BC1FC6n;

function hermesBytes(version) {
  const bytes = Buffer.alloc(64);
  bytes.writeBigUInt64LE(HERMES_MAGIC, 0);
  bytes.writeUInt32LE(version, 8);
  return bytes;
}

function hermesHeaderBytes(version) {
  const bytes = Buffer.alloc(128);
  bytes.writeBigUInt64LE(HERMES_MAGIC, 0);
  bytes.writeUInt32LE(version, 8);
  bytes.writeUInt32LE(128, 32);
  bytes.writeUInt32LE(128, 108);
  return bytes;
}

const textResult = chiff.detectFormat(Buffer.from('const answer = 42;\n'));
assert.deepEqual(textResult, { kind: 'text_utf8' });

const hermesResult = chiff.detectFormat(hermesBytes(99));
assert.deepEqual(hermesResult, {
  kind: 'hermes_bytecode',
  version: 99,
  form: 'execution',
});

const diffStatsResult = chiff.diffStats(
  Buffer.from('abcXYZdef'),
  Buffer.from('abc123def'),
);
assert.deepEqual(diffStatsResult, {
  opCount: 3,
  copyOpCount: 2,
  insertOpCount: 1,
  copiedBytes: 6,
  insertedBytes: 3,
});

assert.equal(
  chiff.selectEngineName(Buffer.from('const a = 1;\n'), Buffer.from('const a = 2;\n')),
  'text',
);
assert.equal(chiff.structuredHermesCompatible(hermesBytes(99)), false);
assert.deepEqual(
  chiff.selectEngineDecisionResult(
    Buffer.from([0x00, 0xff, 0x10, 0x00, 0x7f]),
    Buffer.from([0x00, 0xff, 0x20, 0x00, 0x7f]),
  ),
  { kind: 'generic_binary', reason: 'binary_pair' },
);
assert.deepEqual(
  chiff.selectEngineDecisionResult(Buffer.from('const a = 1;\n'), Buffer.from('const a = 2;\n')),
  { kind: 'text', reason: 'text_pair' },
);
assert.deepEqual(
  chiff.analyzeDiffResult(
    Buffer.from([0x00, 0xff, 0x10, 0x00, 0x7f]),
    Buffer.from([0x00, 0xff, 0x20, 0x00, 0x7f]),
  ),
  {
    engineKind: 'generic_binary',
    engineReason: 'binary_pair',
    oldStructuredHermesSupport: 'not_hermes',
    newStructuredHermesSupport: 'not_hermes',
    opCount: 3,
    copyOpCount: 2,
    insertOpCount: 1,
    copiedBytes: 4,
    insertedBytes: 1,
  },
);
assert.deepEqual(
  chiff.analyzeDiffResult(Buffer.from('abcXYZdef'), Buffer.from('abc123def')),
  {
    engineKind: 'text',
    engineReason: 'text_pair',
    oldStructuredHermesSupport: 'not_hermes',
    newStructuredHermesSupport: 'not_hermes',
    opCount: 3,
    copyOpCount: 2,
    insertOpCount: 1,
    copiedBytes: 6,
    insertedBytes: 3,
  },
);
assert.deepEqual(
  chiff.hpatchCompatiblePlanResult(
    Buffer.from('abcXYZdef'),
    Buffer.from('abc123def'),
  ),
  {
    outputMode: 'hpatch_compatible',
    coverPolicy: 'chiff_structured',
    oldSize: '9',
    newSize: '9',
    coverCount: 2,
    coveredBytes: '6',
    uncoveredNewBytes: '3',
    covers: [
      { oldPos: '0', newPos: '0', len: '3' },
      { oldPos: '6', newPos: '6', len: '3' },
    ],
  },
);
assert.deepEqual(chiff.structuredHermesSupport(hermesHeaderBytes(99)), {
  status: 'supported',
  version: 99,
  form: 'execution',
});
assert.deepEqual(chiff.structuredHermesSupport(hermesBytes(99)), {
  status: 'invalid_header',
});

const workspaceRoot = path.resolve(__dirname, '../../..');
const mixedCorpus = chiff.analyzeDirectoryPairResult(
  path.join(workspaceRoot, 'fixtures/corpus/mixed-baseline/old'),
  path.join(workspaceRoot, 'fixtures/corpus/mixed-baseline/new'),
);
assert.equal(mixedCorpus.summary.paired, 7);
assert.deepEqual(
  mixedCorpus.summary.reasonCounts.map((entry) => [entry.key, entry.count]),
  [
    ['binary_pair', 1],
    ['hermes_form_mismatch', 1],
    ['hermes_old_invalid_header', 1],
    ['hermes_old_unsupported_version', 1],
    ['hermes_structured', 1],
    ['hermes_version_mismatch', 1],
    ['text_pair', 1],
  ],
);
