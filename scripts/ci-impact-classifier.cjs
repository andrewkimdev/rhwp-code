'use strict';

const fs = require('node:fs');

const CLASSIFIER_VERSION = '1';
const CODEQL_LANGUAGE_ORDER = ['javascript-typescript', 'python', 'rust'];
const FRONTEND_MODE_RANK = { none: 0, unit: 1, package: 2 };

const RENDER_RUST_PREFIXES = [
  'src/paint/',
  'src/model/',
  'src/renderer/',
];

const RENDER_RUST_FILES = new Set([
  'src/document_core/queries/rendering.rs',
]);

const RENDER_TOOL_PATHS = new Set([
  'scripts/renderer_baseline.py',
  'scripts/renderer_baseline_manifest.json',
  'scripts/generate_font_glyph_payload_fixture.py',
  'scripts/generate_exact_face_collection_fixture.py',
  'scripts/generate_font_native_hwpx_fixture.py',
  'scripts/requirements-font-fixtures.txt',
  'samples/render-p35-font-native-bitmap.hwpx',
  'docs/canvaskit-parity-implementation.md',
  'docs/text-ir-v2.md',
]);

const FRONTEND_PACKAGE_PREFIXES = [
  'rhwp-chrome/',
  'rhwp-firefox/',
  'rhwp-safari/',
  'rhwp-vscode/',
  'rhwp-shared/',
  'npm/editor/',
  'typescript/',
];

function fullResult(reason) {
  return {
    rust_required: 'true',
    frontend_mode: 'package',
    render_required: 'true',
    native_skia_required: 'true',
    codeql_languages: CODEQL_LANGUAGE_ORDER.join(','),
    classification_status: 'full',
    classifier_version: CLASSIFIER_VERSION,
    reason: `fail-closed:${reason}`,
  };
}

function normalizeFile(file) {
  if (typeof file === 'string') {
    return { filename: file, previous_filename: '', status: 'modified' };
  }

  const status = String(file?.status || file?.changeType || 'modified').toLowerCase();
  return {
    filename: String(file?.filename || file?.path || ''),
    previous_filename: String(file?.previous_filename || file?.previousPath || ''),
    status,
  };
}

function isReviewOnlyPath(filename) {
  return (
    filename.startsWith('mydocs/')
    || filename.startsWith('docs/')
    || filename === 'LICENSE'
    || filename === 'SECURITY.md'
    || filename === 'CONTRIBUTING.md'
    || filename === 'CHANGELOG.md'
    || filename === 'CHANGELOG_EN.md'
    || filename === 'README.md'
    || filename === 'README_EN.md'
    || filename === 'AGENTS.md'
    || filename === 'CLAUDE.md'
    || filename === 'llms.txt'
  );
}

function failClosedPathReason(filename) {
  if (filename.startsWith('.github/')) {
    return 'workflow-contract';
  }
  if (filename === 'Cargo.toml' || filename === 'Cargo.lock') {
    return 'cargo-contract';
  }
  if (filename === 'rust-toolchain.toml' || filename.startsWith('.cargo/')) {
    return 'rust-toolchain-contract';
  }
  if (filename === 'src/main.rs') {
    return 'main-render-boundary';
  }
  if (filename === 'src/wasm_api.rs' || filename.startsWith('src/wasm_api/')) {
    return 'wasm-contract';
  }
  if (
    filename === 'scripts/ci-impact-classifier.cjs'
    || filename === 'scripts/tests/test_ci_impact_workflow.py'
    || filename === 'scripts/tests/ci-impact-classifier.test.cjs'
    || filename.startsWith('scripts/tests/fixtures/ci-impact-classifier-')
  ) {
    return 'classifier-contract';
  }
  return '';
}

function isRenderRustPath(filename) {
  return (
    RENDER_RUST_FILES.has(filename)
    || RENDER_RUST_PREFIXES.some((prefix) => filename.startsWith(prefix))
  );
}

function isRustPath(filename) {
  return filename.endsWith('.rs') || filename === 'build.rs';
}

function isStudioRenderTest(filename) {
  if (!filename.startsWith('rhwp-studio/tests/')) {
    return false;
  }
  const basename = filename.slice('rhwp-studio/tests/'.length).toLowerCase();
  return (
    basename.startsWith('render-')
    || basename.includes('renderer')
    || basename.includes('canvaskit')
    || basename.startsWith('issue-3315-')
  );
}

function classifyChanges(input = {}) {
  const eventName = String(input.eventName || 'pull_request');
  const forceFullReason = String(input.forceFullReason || '');
  if (forceFullReason) {
    return fullResult(forceFullReason);
  }
  if (eventName !== 'pull_request' && eventName !== 'push') {
    return fullResult('unsupported-event');
  }

  if (!Array.isArray(input.files) || input.files.length === 0) {
    return fullResult('file-list-empty');
  }
  const boundary = eventName === 'push' ? 300 : 3000;
  if (input.files.length >= boundary) {
    return fullResult(`${eventName}-file-list-boundary`);
  }

  const files = input.files.map(normalizeFile);
  if (files.some((file) => !file.filename)) {
    return fullResult('invalid-file-entry');
  }
  if (files.some((file) => file.status === 'renamed' || file.previous_filename)) {
    return fullResult('rename');
  }

  const failClosedReasons = files
    .map((file) => failClosedPathReason(file.filename))
    .filter(Boolean)
    .sort();
  if (failClosedReasons.length > 0) {
    return fullResult(failClosedReasons[0]);
  }

  let rustRequired = false;
  let frontendMode = 'none';
  let renderRequired = false;
  let nativeSkiaRequired = false;
  let reviewOnlyCount = 0;
  const codeqlLanguages = new Set();
  const reasons = new Set();

  function requireFrontend(mode, reason) {
    if (FRONTEND_MODE_RANK[mode] > FRONTEND_MODE_RANK[frontendMode]) {
      frontendMode = mode;
    }
    codeqlLanguages.add('javascript-typescript');
    reasons.add(reason);
  }

  for (const file of files.slice().sort((a, b) => a.filename.localeCompare(b.filename))) {
    const filename = file.filename;

    if (isRenderRustPath(filename)) {
      rustRequired = true;
      renderRequired = true;
      nativeSkiaRequired = true;
      codeqlLanguages.add('rust');
      reasons.add('rust-render');
      continue;
    }

    if (isRustPath(filename)) {
      rustRequired = true;
      codeqlLanguages.add('rust');
      reasons.add('rust');
      continue;
    }

    if (filename.startsWith('rhwp-studio/src/view/')) {
      requireFrontend('unit', 'studio-render');
      renderRequired = true;
      continue;
    }

    if (isStudioRenderTest(filename)) {
      requireFrontend('unit', 'studio-render-test');
      renderRequired = true;
      continue;
    }

    if (filename.startsWith('rhwp-studio/src/hwpctl/')) {
      requireFrontend('package', 'studio-package');
      continue;
    }

    if (
      filename.startsWith('rhwp-studio/src/core/')
      || filename.startsWith('rhwp-studio/src/embed/')
      || filename === 'rhwp-studio/src/main.ts'
      || filename.startsWith('rhwp-studio/public/')
    ) {
      requireFrontend('package', 'studio-package');
      continue;
    }

    if (filename.startsWith('rhwp-studio/src/') || filename.startsWith('rhwp-studio/tests/')) {
      requireFrontend('unit', 'studio-unit');
      continue;
    }

    if (filename.startsWith('rhwp-studio/')) {
      requireFrontend('package', 'studio-package');
      continue;
    }

    if (FRONTEND_PACKAGE_PREFIXES.some((prefix) => filename.startsWith(prefix))) {
      requireFrontend('package', 'frontend-package');
      continue;
    }

    if (filename.startsWith('scripts/frontend-') && filename.endsWith('.mjs')) {
      requireFrontend('package', 'frontend-package');
      continue;
    }

    if (filename.startsWith('assets/fonts/')) {
      requireFrontend('package', 'font-render-input');
      renderRequired = true;
      nativeSkiaRequired = true;
      continue;
    }

    if (
      filename.startsWith('ttfs/')
      || filename.startsWith('tests/fixtures/fonts/')
      || RENDER_TOOL_PATHS.has(filename)
    ) {
      renderRequired = true;
      nativeSkiaRequired = true;
      if (filename.endsWith('.py')) {
        codeqlLanguages.add('python');
      }
      reasons.add('render-input');
      continue;
    }

    if (isReviewOnlyPath(filename)) {
      reviewOnlyCount += 1;
      continue;
    }

    return fullResult('unclassified-path');
  }

  if (reasons.size === 0 && reviewOnlyCount === files.length) {
    reasons.add('review-only');
  }

  return {
    rust_required: rustRequired ? 'true' : 'false',
    frontend_mode: frontendMode,
    render_required: renderRequired ? 'true' : 'false',
    native_skia_required: nativeSkiaRequired ? 'true' : 'false',
    codeql_languages: CODEQL_LANGUAGE_ORDER
      .filter((language) => codeqlLanguages.has(language))
      .join(',') || 'none',
    classification_status: 'classified',
    classifier_version: CLASSIFIER_VERSION,
    reason: `classified:${Array.from(reasons).sort().join('+')}`,
  };
}

function parseCliArgs(argv) {
  const args = { input: '', githubOutput: '' };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--input') {
      args.input = argv[index + 1] || '';
      index += 1;
    } else if (argv[index] === '--github-output') {
      args.githubOutput = argv[index + 1] || '';
      index += 1;
    } else {
      throw new Error(`unknown argument: ${argv[index]}`);
    }
  }
  if (!args.input) {
    throw new Error('--input is required');
  }
  return args;
}

function runCli(argv) {
  const args = parseCliArgs(argv);
  const input = JSON.parse(fs.readFileSync(args.input, 'utf8'));
  const result = classifyChanges(input);

  if (args.githubOutput) {
    const lines = Object.entries(result).map(([key, value]) => `${key}=${value}`).join('\n');
    fs.appendFileSync(args.githubOutput, `${lines}\n`, 'utf8');
  } else {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  }
  return result;
}

if (require.main === module) {
  try {
    runCli(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`ci-impact-classifier: ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = {
  CLASSIFIER_VERSION,
  classifyChanges,
  fullResult,
  normalizeFile,
  runCli,
};
