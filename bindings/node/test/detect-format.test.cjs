'use strict';

const assert = require('node:assert/strict');
const chiff = require('../index.cjs');

const HERMES_MAGIC = 0x1F1903C103BC1FC6n;

function hermesBytes(version) {
  const bytes = Buffer.alloc(64);
  bytes.writeBigUInt64LE(HERMES_MAGIC, 0);
  bytes.writeUInt32LE(version, 8);
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
