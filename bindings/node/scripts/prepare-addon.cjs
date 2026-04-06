'use strict';

const fs = require('node:fs');
const path = require('node:path');

const profile = process.argv[2] === 'release' ? 'release' : 'debug';
const extension = process.platform === 'win32' ? '.dll' : process.platform === 'darwin' ? '.dylib' : '.so';
const source = path.join(__dirname, '..', '..', '..', 'target', profile, `libchiff_node${extension}`);
const target = path.join(__dirname, '..', 'chiff.node');

fs.copyFileSync(source, target);
