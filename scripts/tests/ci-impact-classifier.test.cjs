'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  classifyChanges,
  runCli,
} = require('../ci-impact-classifier.cjs');

const FIXTURE_PATH = path.join(
  __dirname,
  'fixtures',
  'ci-impact-classifier-prs.json',
);
const HISTORICAL_PRS = JSON.parse(fs.readFileSync(FIXTURE_PATH, 'utf8'));

for (const fixture of HISTORICAL_PRS) {
  test(`historical PR #${fixture.pr}: ${fixture.title}`, () => {
    assert.deepEqual(
      classifyChanges({ eventName: 'pull_request', files: fixture.files }),
      fixture.expected,
    );
  });
}

test('review-only changes require no code worker', () => {
  assert.deepEqual(
    classifyChanges({
      eventName: 'pull_request',
      files: [
        { filename: 'mydocs/orders/20260802.md', status: 'modified' },
        { filename: 'README.md', status: 'modified' },
      ],
    }),
    {
      rust_required: 'false',
      frontend_mode: 'none',
      render_required: 'false',
      native_skia_required: 'false',
      codeql_languages: 'none',
      classification_status: 'classified',
      classifier_version: '1',
      reason: 'classified:review-only',
    },
  );
});

test('mixed Studio package and Rust changes union modes and CodeQL languages', () => {
  assert.deepEqual(
    classifyChanges({
      eventName: 'pull_request',
      files: [
        { filename: 'rhwp-studio/src/hwpctl/action.ts', status: 'modified' },
        { filename: 'src/parser/hwpx/mod.rs', status: 'modified' },
      ],
    }),
    {
      rust_required: 'true',
      frontend_mode: 'package',
      render_required: 'false',
      native_skia_required: 'false',
      codeql_languages: 'javascript-typescript,rust',
      classification_status: 'classified',
      classifier_version: '1',
      reason: 'classified:rust+studio-package',
    },
  );
});

test('Rust renderer changes require Rust, Native Skia, Canvas, and Rust CodeQL', () => {
  const result = classifyChanges({
    eventName: 'pull_request',
    files: [{ filename: 'src/renderer/layout/table.rs', status: 'modified' }],
  });

  assert.equal(result.rust_required, 'true');
  assert.equal(result.frontend_mode, 'none');
  assert.equal(result.render_required, 'true');
  assert.equal(result.native_skia_required, 'true');
  assert.equal(result.codeql_languages, 'rust');
  assert.equal(result.classification_status, 'classified');
});

test('rename evaluates fail-closed before either path can be skipped', () => {
  const result = classifyChanges({
    eventName: 'pull_request',
    files: [{
      filename: 'rhwp-studio/src/command/new-name.ts',
      previous_filename: 'rhwp-studio/src/command/old-name.ts',
      status: 'renamed',
    }],
  });

  assert.equal(result.classification_status, 'full');
  assert.equal(result.reason, 'fail-closed:rename');
});

for (const [filename, expectedReason] of [
  ['Cargo.lock', 'fail-closed:cargo-contract'],
  ['.github/workflows/ci.yml', 'fail-closed:workflow-contract'],
  ['src/main.rs', 'fail-closed:main-render-boundary'],
  ['src/wasm_api.rs', 'fail-closed:wasm-contract'],
  ['scripts/ci-impact-classifier.cjs', 'fail-closed:classifier-contract'],
  ['unclassified/new-format.schema', 'fail-closed:unclassified-path'],
]) {
  test(`${filename} is fail-closed`, () => {
    const result = classifyChanges({
      eventName: 'pull_request',
      files: [{ filename, status: 'modified' }],
    });
    assert.equal(result.classification_status, 'full');
    assert.equal(result.reason, expectedReason);
    assert.equal(result.codeql_languages, 'javascript-typescript,python,rust');
  });
}

test('empty and forced file collection failures are full', () => {
  assert.equal(
    classifyChanges({ eventName: 'pull_request', files: [] }).reason,
    'fail-closed:file-list-empty',
  );
  assert.equal(
    classifyChanges({
      eventName: 'pull_request',
      files: [],
      forceFullReason: 'collection-error',
    }).reason,
    'fail-closed:collection-error',
  );
});

test('documented PR and push API boundaries are full', () => {
  const prFiles = Array.from(
    { length: 3000 },
    (_, index) => ({ filename: `mydocs/pr-${index}.md`, status: 'modified' }),
  );
  const pushFiles = Array.from(
    { length: 300 },
    (_, index) => ({ filename: `mydocs/push-${index}.md`, status: 'modified' }),
  );

  assert.equal(
    classifyChanges({ eventName: 'pull_request', files: prFiles }).reason,
    'fail-closed:pull_request-file-list-boundary',
  );
  assert.equal(
    classifyChanges({ eventName: 'push', files: pushFiles }).reason,
    'fail-closed:push-file-list-boundary',
  );
});

test('full fallback does not depend on input ordering', () => {
  const cargoFirst = [
    { filename: 'Cargo.toml', status: 'modified' },
    { filename: '.github/workflows/ci.yml', status: 'modified' },
  ];
  assert.deepEqual(
    classifyChanges({ eventName: 'pull_request', files: cargoFirst }),
    classifyChanges({ eventName: 'pull_request', files: cargoFirst.slice().reverse() }),
  );
});

test('CLI writes every classifier output to the GitHub output file', (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'rhwp-ci-impact-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const inputPath = path.join(directory, 'input.json');
  const outputPath = path.join(directory, 'github-output.txt');
  fs.writeFileSync(inputPath, JSON.stringify({
    eventName: 'pull_request',
    files: [{ filename: 'rhwp-studio/src/command/shortcut-map.ts', status: 'modified' }],
  }));

  const result = runCli(['--input', inputPath, '--github-output', outputPath]);
  const output = fs.readFileSync(outputPath, 'utf8');

  for (const [key, value] of Object.entries(result)) {
    assert.match(output, new RegExp(`^${key}=${value}$`, 'm'));
  }
});
