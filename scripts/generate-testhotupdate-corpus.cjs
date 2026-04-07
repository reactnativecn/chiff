#!/usr/bin/env node
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '..');
const defaultSourceProject =
  process.env.CHIFF_TESTHOTUPDATE_PROJECT ||
  '/Users/sunny/Documents/workspace/react-native-update/Example/testHotUpdate';
const defaultOutputRoot = path.join(
  repoRoot,
  'fixtures/generated/testHotUpdate/android',
);

const pairs = {
  'minor-string-edit': {
    title: 'minor-string-edit',
    file: 'src/index.tsx',
    from: '这是版本一',
    to: '这是版本二',
    summary: 'Minimal UI text change in the main instructions block.',
  },
  'bundle-label-copy-edit': {
    title: 'bundle-label-copy-edit',
    file: 'src/index.tsx',
    from: '<Text testID="bundle-label">bundleLabel: {bundleLabel}</Text>',
    to: '<Text testID="bundle-label">bundleLabelV2: {bundleLabel}</Text>',
    summary: 'Minimal ASCII UI text change in the bundle-label element.',
  },
};

function printUsageAndExit() {
  console.error(
    [
      'usage: node scripts/generate-testhotupdate-corpus.cjs --pair <name> [--force]',
      '       [--source-project <path>] [--output-root <path>]',
      '',
      `known pairs: ${Object.keys(pairs).join(', ')}`,
    ].join('\n'),
  );
  process.exit(2);
}

function parseArgs(argv) {
  let pairName;
  let sourceProject = defaultSourceProject;
  let outputRoot = defaultOutputRoot;
  let force = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--pair') {
      i += 1;
      pairName = argv[i];
    } else if (arg.startsWith('--pair=')) {
      pairName = arg.slice('--pair='.length);
    } else if (arg === '--source-project') {
      i += 1;
      sourceProject = argv[i];
    } else if (arg.startsWith('--source-project=')) {
      sourceProject = arg.slice('--source-project='.length);
    } else if (arg === '--output-root') {
      i += 1;
      outputRoot = argv[i];
    } else if (arg.startsWith('--output-root=')) {
      outputRoot = arg.slice('--output-root='.length);
    } else if (arg === '--force') {
      force = true;
    } else {
      printUsageAndExit();
    }
  }

  if (!pairName || !pairs[pairName]) {
    printUsageAndExit();
  }

  if (!sourceProject || !outputRoot) {
    printUsageAndExit();
  }

  return {
    pairName,
    sourceProject: path.resolve(sourceProject),
    outputRoot: path.resolve(outputRoot),
    force,
  };
}

function run(command, args, cwd) {
  console.error(`$ ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, {
    cwd,
    env: { ...process.env, CI: process.env.CI || '1' },
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status}`);
  }
}

function ensureDirectory(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function replaceExactly(input, from, to) {
  const occurrences = input.split(from).length - 1;
  if (occurrences !== 1) {
    throw new Error(
      `expected exactly one mutation target, found ${occurrences}: ${from}`,
    );
  }
  return input.replace(from, to);
}

function lineNumberOf(input, needle) {
  const index = input.indexOf(needle);
  if (index < 0) {
    return null;
  }
  return input.slice(0, index).split('\n').length;
}

function sha256(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function loadChiffBinding() {
  try {
    return require(path.join(repoRoot, 'bindings/node'));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`warning: unable to load @chiff/node for stats: ${message}`);
    return null;
  }
}

function artifactMetadata(root, variant, relativePath, format) {
  const file = path.join(root, relativePath);
  return {
    variant,
    path: relativePath,
    format,
    size_bytes: fs.statSync(file).size,
    sha256: sha256(file),
  };
}

function detectTextFormat(chiff, file) {
  if (chiff) {
    return chiff.detectFormat(fs.readFileSync(file));
  }
  return { kind: 'text_utf8' };
}

function detectHermesFormat(chiff, file) {
  if (chiff) {
    return chiff.detectFormat(fs.readFileSync(file));
  }

  const bytes = fs.readFileSync(file);
  return {
    kind: 'hermes_bytecode',
    version: bytes.length >= 12 ? bytes.readUInt32LE(8) : 0,
    form: 'execution',
  };
}

function copyFile(source, destination) {
  ensureDirectory(path.dirname(destination));
  fs.copyFileSync(source, destination);
}

function buildVariant(sourceProject, hermesc, tempRoot) {
  const variantRoot = path.join(tempRoot, 'work');
  fs.rmSync(variantRoot, { recursive: true, force: true });

  const textPath = path.join(variantRoot, 'index.android.bundle');
  const hbcPath = path.join(variantRoot, 'index.android.hbc');
  const assetsPath = path.join(variantRoot, 'assets');

  ensureDirectory(path.dirname(textPath));
  ensureDirectory(path.dirname(hbcPath));

  run(
    'npx',
    [
      'react-native',
      'bundle',
      '--platform',
      'android',
      '--dev',
      'false',
      '--entry-file',
      'index.js',
      '--bundle-output',
      textPath,
      '--assets-dest',
      assetsPath,
    ],
    sourceProject,
  );

  run(hermesc, ['-O', '-emit-binary', '-out', hbcPath, textPath], sourceProject);

  return {
    textPath,
    hbcPath,
  };
}

function collectDiffStats(chiff, pairRoot) {
  if (!chiff) {
    return undefined;
  }

  const normalize = (stats) => ({
    op_count: stats.opCount,
    copy_op_count: stats.copyOpCount,
    insert_op_count: stats.insertOpCount,
    copied_bytes: stats.copiedBytes,
    inserted_bytes: stats.insertedBytes,
  });

  return {
    text: normalize(
      chiff.diffStats(
        fs.readFileSync(path.join(pairRoot, 'v1/text/index.android.bundle')),
        fs.readFileSync(path.join(pairRoot, 'v2/text/index.android.bundle')),
      ),
    ),
    hermes: normalize(
      chiff.diffStats(
        fs.readFileSync(path.join(pairRoot, 'v1/hermes/index.android.hbc')),
        fs.readFileSync(path.join(pairRoot, 'v2/hermes/index.android.hbc')),
      ),
    ),
  };
}

function writeReadme(pairName, config, sourceProject, sourceFile, line, pairRoot) {
  const relativePairRoot = path.relative(repoRoot, pairRoot);
  const readme = `# testHotUpdate Android Pair: ${pairName}

This fixture pair was generated from:

- \`${sourceProject}\`

## Mutation

${config.summary}

Source file:

- \`${sourceFile}${line ? `:${line}` : ''}\`

Mutation applied for \`v2\` generation:

- \`${config.from}\` -> \`${config.to}\`

The source file in the example project is restored after generation by \`scripts/generate-testhotupdate-corpus.cjs\`.
The generator uses a stable work directory under \`target/generated-fixtures\`
so Hermes debug metadata does not depend on a random temporary path.

## Pair Layout

- \`v1/text/index.android.bundle\`
- \`v2/text/index.android.bundle\`
- \`v1/hermes/index.android.hbc\`
- \`v2/hermes/index.android.hbc\`

Machine-readable artifact metadata is recorded in \`metadata.json\`.

Relative fixture root:

- \`${relativePairRoot}\`
`;

  fs.writeFileSync(path.join(pairRoot, 'README.md'), readme);
}

function writeMetadata(pairName, config, sourceProject, sourceFile, line, pairRoot) {
  const chiff = loadChiffBinding();
  const metadata = {
    source_project: sourceProject,
    platform: 'android',
    generated_by: 'scripts/generate-testhotupdate-corpus.cjs',
    mutation: {
      name: pairName,
      file: sourceFile,
      line,
      from: config.from,
      to: config.to,
    },
    artifacts: [
      artifactMetadata(pairRoot, 'v1', 'v1/text/index.android.bundle', detectTextFormat(chiff, path.join(pairRoot, 'v1/text/index.android.bundle'))),
      artifactMetadata(pairRoot, 'v2', 'v2/text/index.android.bundle', detectTextFormat(chiff, path.join(pairRoot, 'v2/text/index.android.bundle'))),
      artifactMetadata(pairRoot, 'v1', 'v1/hermes/index.android.hbc', detectHermesFormat(chiff, path.join(pairRoot, 'v1/hermes/index.android.hbc'))),
      artifactMetadata(pairRoot, 'v2', 'v2/hermes/index.android.hbc', detectHermesFormat(chiff, path.join(pairRoot, 'v2/hermes/index.android.hbc'))),
    ],
    current_diff_stats: collectDiffStats(chiff, pairRoot),
  };

  fs.writeFileSync(
    path.join(pairRoot, 'metadata.json'),
    `${JSON.stringify(metadata, null, 2)}\n`,
  );
}

function main() {
  const { pairName, sourceProject, outputRoot, force } = parseArgs(
    process.argv.slice(2),
  );
  const config = pairs[pairName];
  const sourceFile = path.join(sourceProject, config.file);
  const hermesc = path.join(
    sourceProject,
    'node_modules/hermes-compiler/hermesc/osx-bin/hermesc',
  );
  const pairRoot = path.join(outputRoot, 'pairs', pairName);

  if (!fs.existsSync(sourceFile)) {
    throw new Error(`source file does not exist: ${sourceFile}`);
  }
  if (!fs.existsSync(hermesc)) {
    throw new Error(`hermesc does not exist: ${hermesc}`);
  }
  if (fs.existsSync(pairRoot)) {
    if (!force) {
      throw new Error(`pair already exists; pass --force to overwrite: ${pairRoot}`);
    }
    fs.rmSync(pairRoot, { recursive: true, force: true });
  }

  const originalSource = fs.readFileSync(sourceFile, 'utf8');
  const mutatedSource = replaceExactly(originalSource, config.from, config.to);
  const line = lineNumberOf(originalSource, config.from);
  const tempRoot = path.join(
    repoRoot,
    'target/generated-fixtures/testHotUpdate/android',
    pairName,
  );
  const pairTempRoot = path.join(tempRoot, 'pair');
  fs.rmSync(tempRoot, { recursive: true, force: true });
  ensureDirectory(tempRoot);

  try {
    console.error(`building v1 from ${sourceProject}`);
    fs.writeFileSync(sourceFile, originalSource);
    const v1 = buildVariant(sourceProject, hermesc, tempRoot);
    copyFile(
      v1.textPath,
      path.join(pairTempRoot, 'v1/text/index.android.bundle'),
    );
    copyFile(v1.hbcPath, path.join(pairTempRoot, 'v1/hermes/index.android.hbc'));

    console.error(`building v2 with mutation ${pairName}`);
    fs.writeFileSync(sourceFile, mutatedSource);
    const v2 = buildVariant(sourceProject, hermesc, tempRoot);
    copyFile(
      v2.textPath,
      path.join(pairTempRoot, 'v2/text/index.android.bundle'),
    );
    copyFile(v2.hbcPath, path.join(pairTempRoot, 'v2/hermes/index.android.hbc'));

    fs.rmSync(pairRoot, { recursive: true, force: true });
    ensureDirectory(path.dirname(pairRoot));
    fs.renameSync(pairTempRoot, pairRoot);

    writeReadme(pairName, config, sourceProject, sourceFile, line, pairRoot);
    writeMetadata(pairName, config, sourceProject, sourceFile, line, pairRoot);
    console.error(`generated ${pairRoot}`);
  } finally {
    fs.writeFileSync(sourceFile, originalSource);
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

main();
