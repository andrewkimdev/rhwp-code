import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// 실제 CursorState를 로드해 #3137 one-shot/revision/fallback 계약을 행위로 검증한다.
const studioRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const workDir = mkdtempSync(path.join(tmpdir(), 'rhwp-focused-cursor-'));
const driverPath = path.join(workDir, 'driver.mjs');

writeFileSync(driverPath, `
import { registerHooks } from 'node:module';
import { pathToFileURL } from 'node:url';

const srcRoot = ${JSON.stringify(pathToFileURL(path.join(studioRoot, 'src') + path.sep).href)};
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith('@/')) return nextResolve(srcRoot + specifier.slice(2) + '.ts', context);
    if (/^\\.{1,2}\\//.test(specifier) && !/\\.[a-z]+$/.test(specifier)) {
      return nextResolve(specifier + '.ts', context);
    }
    return nextResolve(specifier, context);
  },
});

const { CursorState } = await import(srcRoot + 'engine/cursor.ts');

const pathEntry = { controlIndex: 2, cellIndex: 2, cellParaIndex: 5 };
const position = (charOffset) => ({
  sectionIndex: 0,
  paragraphIndex: 5,
  charOffset,
  parentParaIndex: 0,
  controlIndex: 2,
  cellIndex: 2,
  cellParaIndex: 5,
  cellPath: [{ ...pathEntry }],
});
const rect = (x) => ({
  pageIndex: 0,
  x,
  y: 40,
  height: 12,
  cellBounds: { x: 5, y: 20, w: 300, h: 100 },
  cellOverflowed: false,
});

const wasm = {
  nearCalls: 0,
  vertical: 0,
  nextRect: rect(90),
  getCellTextDirection() { return this.vertical; },
  getCursorRectByPathNear() {
    this.nearCalls += 1;
    return { ...this.nextRect };
  },
  getCursorRectByPath() {
    this.nearCalls += 1;
    return { ...this.nextRect };
  },
};
const cursor = new CursorState(wasm);
cursor.moveToHit({ ...position(130), cursorRect: rect(100) });

const transition = (baseRevision, revision, sourceOffset, targetOffset, deltaX) => ({
  baseRevision,
  revision,
  source: position(sourceOffset),
  target: position(targetOffset),
  deltaX,
});

const result = {};
result.firstPrepared = cursor.prepareFocusedCellCursorGeometry(
  transition(0, 1, 130, 131, 7.25),
);
cursor.moveTo(position(131));
result.first = { rect: cursor.getRect(), calls: wasm.nearCalls };

result.secondPrepared = cursor.prepareFocusedCellCursorGeometry(
  transition(1, 2, 131, 132, 8.5),
);
cursor.moveTo(position(132));
result.second = { rect: cursor.getRect(), calls: wasm.nearCalls };

// fast revision 2 위에 base 1을 다시 적용할 수 없다. prepare가 invalid 처리하고 exact fallback.
wasm.nextRect = rect(140);
result.stalePrepared = cursor.prepareFocusedCellCursorGeometry(
  transition(1, 3, 132, 133, 9),
);
cursor.moveTo(position(133));
result.stale = { rect: cursor.getRect(), calls: wasm.nearCalls };

// sync flush/commit invalidation 뒤에는 준비했던 transition도 소비하지 않는다.
result.flushPrepared = cursor.prepareFocusedCellCursorGeometry(
  transition(3, 4, 133, 134, 10),
);
cursor.invalidateFocusedCellCursorGeometry();
wasm.nextRect = rect(155);
cursor.moveTo(position(134));
result.flushed = { rect: cursor.getRect(), calls: wasm.nearCalls };

// vertical cell은 Rust delta가 있어도 horizontal x fast path를 사용하지 않는다.
wasm.vertical = 1;
result.verticalPrepared = cursor.prepareFocusedCellCursorGeometry(
  transition(4, 5, 134, 135, 11),
);
wasm.nextRect = rect(170);
cursor.moveTo(position(135));
result.vertical = { rect: cursor.getRect(), calls: wasm.nearCalls };

process.stdout.write('###' + JSON.stringify(result) + '###');
`);

const run = spawnSync(
  process.execPath,
  ['--experimental-transform-types', '--no-warnings', driverPath],
  { cwd: studioRoot, encoding: 'utf8' },
);
rmSync(workDir, { recursive: true, force: true });

assert.equal(
  run.status,
  0,
  `focused cursor 행위 드라이버 실행 실패:\n${run.stdout}\n${run.stderr}`,
);
const captured = /###([\s\S]*)###/.exec(run.stdout);
assert.ok(captured, `focused cursor 결과 JSON 없음:\n${run.stdout}\n${run.stderr}`);
const observed = JSON.parse(captured[1]);

test('#3137 same-line transition은 exact query 없이 연속 revision으로 적용한다', () => {
  assert.equal(observed.firstPrepared, true);
  assert.equal(observed.first.calls, 0);
  assert.equal(observed.first.rect.x, 107.25);
  assert.equal(observed.secondPrepared, true);
  assert.equal(observed.second.calls, 0);
  assert.equal(observed.second.rect.x, 115.75);
});

test('#3137 revision mismatch와 flush/vertical 경계는 exact query로 fallback한다', () => {
  assert.equal(observed.stalePrepared, false);
  assert.equal(observed.stale.calls, 1);
  assert.equal(observed.stale.rect.x, 140);

  assert.equal(observed.flushPrepared, true);
  assert.equal(observed.flushed.calls, 2);
  assert.equal(observed.flushed.rect.x, 155);

  assert.equal(observed.verticalPrepared, false);
  assert.equal(observed.vertical.calls, 3);
  assert.equal(observed.vertical.rect.x, 170);
});
