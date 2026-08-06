import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  excludedCellKey,
  selectCellIndicesInRange,
  paraFormatTargetsForCellBlock,
} from '../src/engine/cell-block-format.ts';

// F5 셀 블록 선택은 cellAnchor/cellFocus 축이라 텍스트 선택(cursor.anchor)을 만들지 않는다.
// 그래서 서식 경로가 getSelectionOrdered() 만 보면 커서가 있는 앵커 셀 하나만 대상이 된다.
// 실측(2x2 표, 전체 셀 선택 후 가운데 정렬): 4칸 중 1칸(앵커)만 변경.

const rootDir = dirname(dirname(fileURLToPath(import.meta.url)));
const source = (path: string): string => readFileSync(join(rootDir, path), 'utf8');

/** 행 우선으로 배치된 rows x cols 표의 cellIdx -> {row, col} */
function gridCellInfo(cols: number) {
  return (cellIdx: number) => ({ row: Math.floor(cellIdx / cols), col: cellIdx % cols });
}

test('선택 범위에 드는 셀만 고른다', () => {
  // 3x3 표에서 (0,0)~(1,1) 블록
  const picked = selectCellIndicesInRange(
    9, gridCellInfo(3),
    { startRow: 0, startCol: 0, endRow: 1, endCol: 1 },
    new Set(),
  );
  assert.deepEqual(picked, [0, 1, 3, 4]);
});

test('블록 전체 선택은 모든 셀을 고른다', () => {
  const picked = selectCellIndicesInRange(
    4, gridCellInfo(2),
    { startRow: 0, startCol: 0, endRow: 1, endCol: 1 },
    new Set(),
  );
  assert.deepEqual(picked, [0, 1, 2, 3]);
});

test('Ctrl+클릭으로 제외한 셀은 대상에서 빠진다', () => {
  const picked = selectCellIndicesInRange(
    4, gridCellInfo(2),
    { startRow: 0, startCol: 0, endRow: 1, endCol: 1 },
    new Set([excludedCellKey(1, 0)]),
  );
  assert.deepEqual(picked, [0, 1, 3]);
});

test('제외 셀 키 형식은 CursorState 가 조립하는 것과 같은 함수여야 한다', () => {
  // 조립하는 쪽과 조회하는 쪽이 형식을 따로 갖고 있으면 한쪽만 바뀌어도 조회가 조용히
  // 빗나가 제외 셀이 무시된다. 리터럴 재조립을 금지하고 단일 함수를 쓰는지 본다.
  const cursor = source('src/engine/cursor.ts');
  assert.match(cursor, /const key = excludedCellKey\(row, col\)/,
    'CursorState.ctrlToggleCell 이 공유 키 함수를 쓰지 않는다');
  assert.doesNotMatch(cursor, /const key = `\$\{row\},\$\{col\}`/,
    'CursorState 가 제외 셀 키를 리터럴로 재조립한다');
});

test('셀 블록 안 모든 셀의 모든 문단이 문단 서식 대상이 된다', () => {
  const targets = paraFormatTargetsForCellBlock(
    { sec: 0, ppi: 3, ci: 1, cellIndices: [0, 1, 2, 3] },
    () => 1,
  );
  assert.equal(targets.length, 4, '4칸 x 문단 1개 = 대상 4개여야 한다');
  assert.deepEqual(
    targets.map((t) => (t.kind === 'cell' ? t.cellIdx : -1)),
    [0, 1, 2, 3],
  );
  assert.deepEqual(targets[2], {
    kind: 'cell', sec: 0, parentPara: 3, controlIdx: 1, cellIdx: 2, cellParaIdx: 0,
  });
});

test('셀마다 문단 수가 다르면 각 셀의 문단 수만큼 대상이 만들어진다', () => {
  const paraCounts = [1, 3, 2];
  const targets = paraFormatTargetsForCellBlock(
    { sec: 0, ppi: 0, ci: 0, cellIndices: [0, 1, 2] },
    (cellIdx) => paraCounts[cellIdx],
  );
  assert.equal(targets.length, 6);
  assert.deepEqual(
    targets.map((t) => (t.kind === 'cell' ? `${t.cellIdx}:${t.cellParaIdx}` : '')),
    ['0:0', '1:0', '1:1', '1:2', '2:0', '2:1'],
  );
});

test('선택된 셀이 없으면 대상도 없다', () => {
  const targets = paraFormatTargetsForCellBlock(
    { sec: 0, ppi: 0, ci: 0, cellIndices: [] },
    () => 5,
  );
  assert.deepEqual(targets, []);
});

test('문단 서식 대상 산출은 텍스트 선택보다 셀 블록을 먼저 본다', () => {
  // 셀 블록 선택은 getSelectionOrdered() 가 null 이므로, 분기가 뒤에 오면 도달하지 못하고
  // getParaFormatTargetsForRange(pos, pos) 로 떨어져 앵커 셀 하나만 대상이 된다.
  const ih = source('src/engine/input-handler.ts');
  const start = ih.indexOf('private getParaFormatTargetsAtCursor()');
  assert.notEqual(start, -1, 'getParaFormatTargetsAtCursor 를 찾지 못했다');
  const body = ih.slice(start, ih.indexOf('\n  }', start));

  const blockAt = body.indexOf('this.getSelectedCellBlock()');
  const selAt = body.indexOf('this.cursor.getSelectionOrdered()');
  assert.notEqual(blockAt, -1, '셀 블록 분기가 없다');
  assert.notEqual(selAt, -1, '텍스트 선택 분기가 없다');
  assert.ok(blockAt < selAt, '셀 블록 분기가 텍스트 선택 분기보다 뒤에 있다');
  assert.match(body, /return this\.getParaFormatTargetsForCellBlock\(block\)/,
    '셀 블록 대상 산출을 호출하지 않는다');
});

test('모양 붙여넣기도 같은 셀 산출 함수를 쓴다', () => {
  // 같은 블록을 대상으로 하는 두 경로가 필터를 각자 갖고 있으면 한쪽만 고쳐진다.
  const ih = source('src/engine/input-handler.ts');
  const start = ih.indexOf('private applyCopiedCellPropsToSelection');
  assert.notEqual(start, -1, 'applyCopiedCellPropsToSelection 을 찾지 못했다');
  const body = ih.slice(start, ih.indexOf('\n  }\n', start));
  assert.match(body, /selectCellIndicesInRange\(/,
    '모양 붙여넣기가 셀 산출을 따로 구현하고 있다');
});
