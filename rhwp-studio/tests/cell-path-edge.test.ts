import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// [2차 god-file 분할 신뢰 보강] engine/command/cell-path.ts(P4 분리)는 중첩 표 좌표의
// '축 규약' 단일 진실 원천이다(cellParaIndexOf=최내곽 문단, cellAxisPath=전체 경로,
// [#2756] 계약). 기존 커버리지는 원문 가드·간접 경로뿐 — 여기서 빈 경로·flat 합성·
// 경계값(MAX_PAGE_LOCAL_TEXT_EDIT_CHARS=8, 서로게이트 charCount)·불변식을 직접 검증한다.
// 드라이버 패턴은 selection-ordering-nested-cell.test.ts 를 따른다.

const studioRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const workDir = mkdtempSync(path.join(tmpdir(), 'rhwp-cell-path-'));
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

const m = await import(srcRoot + 'engine/command/cell-path.ts');

const flatCell = { sectionIndex: 0, paragraphIndex: 5, charOffset: 0, parentParaIndex: 2, controlIndex: 7, cellIndex: 3, cellParaIndex: 1 };
const body = { sectionIndex: 0, paragraphIndex: 4, charOffset: 0 };
const depth1 = { ...flatCell, cellPath: [{ controlIndex: 7, cellIndex: 3, cellParaIndex: 1 }] };
const nested = {
  ...flatCell,
  cellPath: [
    { controlIndex: 7, cellIndex: 3, cellParaIndex: 0 },
    { controlIndex: 12, cellIndex: 5, cellParaIndex: 2 },
  ],
};

const out = {
  isCell: [m.isCell(flatCell), m.isCell(depth1), m.isCell(nested), m.isCell(body), m.isCell({ ...body, parentParaIndex: 0 })],
  isNestedCell: [m.isNestedCell(nested), m.isNestedCell(depth1), m.isNestedCell(flatCell)],
  insert: [
    [flatCell, 'abc'], [nested, 'abc'], [body, 'abc'], [flatCell, ''], [flatCell, 'a'.repeat(8)],
    [flatCell, 'a'.repeat(9)], [flatCell, 'a\\tb'], [flatCell, 'a\\nb'],
  ].map(([p, t]) => m.canUseDeferredCellTextInsert(p, t)),
  del: [
    [flatCell, 8], [flatCell, 9], [flatCell, 0], [flatCell, -1], [flatCell, 1.5], [nested, 1],
  ].map(([p, c]) => m.canUseDeferredCellTextDelete(p, c)),
  replaceCell: [
    [flatCell, 1, 'ab'], [flatCell, 0, 'ab'], [flatCell, 1, ''], [flatCell, 9, 'ab'],
    [flatCell, 1, 'a'.repeat(8)], [flatCell, 1, 'a'.repeat(9)], [flatCell, 1, '😀'.repeat(8)],
    [flatCell, 1, '😀'.repeat(9)], [flatCell, 1, 'a\\rb'], [nested, 1, 'ab'],
  ].map(([p, d, t]) => m.canUseDeferredCellTextReplace(p, d, t)),
  replaceBody: [
    [body, 0, 'a'], [body, 0, ''], [body, 8, ''], [body, 9, 'a'], [flatCell, 1, 'a'],
    [body, 1, '\\t'], [body, 0, '😀'.repeat(8)], [body, 0, '😀'.repeat(9)],
  ].map(([p, d, t]) => m.canUseLocalBodyTextReplace(p, d, t)),
  pathJson: [m.cellPathJson(body), m.cellPathJson(flatCell), m.cellPathJson(depth1)],
  pathJsonForPara: [
    m.cellPathJsonForPara(nested, 9),
    m.cellPathJsonForPara(flatCell, 4),
    m.cellPathJsonForPara(body, 4),
  ],
  paraIndexOf: [m.cellParaIndexOf(nested), m.cellParaIndexOf(depth1), m.cellParaIndexOf(flatCell)],
  axisPath: [m.cellAxisPath(nested), m.cellAxisPath(depth1), m.cellAxisPath(flatCell), m.cellAxisPath(body)],
  paragraphPosition: (() => {
    const moved = m.cellParagraphPosition(nested, 7, 12);
    return {
      lastPara: moved.cellPath[moved.cellPath.length - 1].cellParaIndex,
      firstUntouched: moved.cellPath[0].cellParaIndex,
      flatPara: moved.cellParaIndex,
      paragraphIndex: moved.paragraphIndex,
      charOffset: moved.charOffset,
      cursorRect: moved.cursorRect,
    };
  })(),
  charCount: [m.charCount(''), m.charCount('abc'), m.charCount('😀😀'), m.charCount('한글a')],
  // 불변식의 정의역은 셀 좌표 — 본문(경로·flat 셀 필드 모두 없음)은 축 계약 밖이고
  // cellParaIndexOf 가 undefined 를 돌려주는 것 자체가 올바른 거동이다.
  invariant: [nested, depth1, flatCell].map(
    (p) => m.cellParaIndexOf(p) === m.cellAxisPath(p)[m.cellAxisPath(p).length - 1].cellParaIndex,
  ),
};

console.log(JSON.stringify(out));
`);

const run = spawnSync(process.execPath, ['--experimental-transform-types', driverPath], {
  encoding: 'utf8',
});
rmSync(workDir, { recursive: true, force: true });
if (run.status !== 0) throw new Error(`드라이버 실패: ${run.stderr}`);
const o: any = JSON.parse(run.stdout);

test('isCell — parentParaIndex 유무만 본다(0 도 셀)', () => {
  assert.equal(o.isCell[0], true, 'flat 셀');
  assert.equal(o.isCell[1], true, 'depth1 경로 셀');
  assert.equal(o.isCell[2], true, '중첩 셀');
  assert.equal(o.isCell[3], false, '본문');
  assert.equal(o.isCell[4], true, 'parentParaIndex=0 도 셀(undefined 아님)');
});

test('isNestedCell — 경로 깊이 2 이상만', () => {
  assert.equal(o.isNestedCell[0], true, 'depth 2');
  assert.equal(o.isNestedCell[1], false, 'depth 1');
  assert.equal(o.isNestedCell[2], false, '경로 없음(flat)');
});

test('canUseDeferredCellTextInsert — 상한 8자·제어문자·중첩 거부', () => {
  assert.deepEqual(o.insert, [true, false, false, false, true, false, false, false]);
});

test('canUseDeferredCellTextDelete — 정수 양수 상한', () => {
  assert.deepEqual(o.del, [true, false, false, false, false, false]);
});

test('canUseDeferredCellTextReplace — charCount(유니코드 스칼라) 기준 상한', () => {
  assert.deepEqual(o.replaceCell, [
    true,   // 1 삭제 + 'ab' 삽입
    false,  // deleteCount 0
    false,  // 빈 삽입
    false,  // deleteCount 9 > 8
    true,   // 8자 삽입
    false,  // 9자 삽입
    true,   // 😀×8 — charCount 8(UTF-16 length 16 이지만 통과)
    false,  // 😀×9 — charCount 9
    false,  // \\r 포함
    false,  // 중첩 셀
  ]);
});

test('canUseLocalBodyTextReplace — 본문 전용·0삭제+빈삽입 거부·서로게이트', () => {
  assert.deepEqual(o.replaceBody, [
    true,   // 순수 삽입
    false,  // 0삭제 + 빈 삽입 = 무연산
    true,   // 순수 삭제 8
    false,  // 삭제 9
    false,  // 셀 위치는 로컬 본문 경로 거부
    false,  // 탭 포함
    true,   // 😀×8 — charCount 8
    false,  // 😀×9 — charCount 9
  ]);
});

test('cellPathJson — 경로 없음은 [] 직렬화', () => {
  assert.equal(o.pathJson[0], '[]');
  assert.equal(JSON.parse(o.pathJson[1]).length, 0, 'flat 도 cellPath 없음');
  assert.deepEqual(JSON.parse(o.pathJson[2]), [{ controlIndex: 7, cellIndex: 3, cellParaIndex: 1 }]);
});

test('cellPathJsonForPara — 마지막 엔트리만 교체', () => {
  const nested = JSON.parse(o.pathJsonForPara[0]);
  assert.equal(nested[1].cellParaIndex, 9, '마지막 엔트리 교체');
  assert.equal(nested[0].cellParaIndex, 0, '바깥 엔트리 무변경');
  assert.deepEqual(JSON.parse(o.pathJsonForPara[1]), [], 'flat(경로 없음)은 [] 그대로');
  assert.deepEqual(JSON.parse(o.pathJsonForPara[2]), [], '본문도 []');
});

test('cellParaIndexOf — 최내곽(마지막) 엔트리 우선, flat 폴백', () => {
  assert.equal(o.paraIndexOf[0], 2, '중첩: 안쪽 cellParaIndex(2), flat 값(1) 아님');
  assert.equal(o.paraIndexOf[1], 1, 'depth 1: 경로 값 === flat 값');
  assert.equal(o.paraIndexOf[2], 1, '경로 없음: flat 폴백');
});

test('cellAxisPath — 경로 있으면 원본, 없으면 flat 으로 1-depth 합성', () => {
  assert.equal(o.axisPath[0].length, 2, '중첩 경로 통과');
  assert.deepEqual(o.axisPath[1], [{ controlIndex: 7, cellIndex: 3, cellParaIndex: 1 }]);
  assert.deepEqual(o.axisPath[2], [{ controlIndex: 7, cellIndex: 3, cellParaIndex: 1 }], 'flat 합성 — depth 1 과 동일');
  assert.deepEqual(o.axisPath[3], [{ controlIndex: 0, cellIndex: 0, cellParaIndex: 0 }], '본문(필드 없음)은 0 기본합성');
});

test('cellParagraphPosition — 안쪽 문단·flat·본문 문단 일괄 정렬, cursorRect 해제', () => {
  const p = o.paragraphPosition;
  assert.equal(p.lastPara, 7, '경로 마지막 엔트리 cellParaIndex=7');
  assert.equal(p.firstUntouched, 0, '바깥 엔트리 무변경');
  assert.equal(p.flatPara, 7, 'flat cellParaIndex 동기화');
  assert.equal(p.paragraphIndex, 7, '본문 paragraphIndex 도 같은 값으로');
  assert.equal(p.charOffset, 12);
  assert.equal(p.cursorRect, undefined, '낡은 cursorRect 해제');
});

test('charCount — 유니코드 스칼라 단위(#2337-review)', () => {
  assert.deepEqual(o.charCount, [0, 3, 2, 3], '😀😀 는 2(UTF-16 length 4 아님)');
});

test('[#2756] cellParaIndexOf === cellAxisPath[last].cellParaIndex — 셀 좌표 3축 불변식', () => {
  assert.ok(o.invariant.every(Boolean));
});
