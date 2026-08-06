import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  excludedCellKey,
  selectCellIndicesInRange,
} from '../src/engine/cell-block-format.ts';

// F5 셀 블록 선택(cellAnchor/cellFocus)에서 "어느 셀이 대상인가"를 정하는 산출 로직.
// 같은 블록을 대상으로 하는 경로가 여럿이라 필터가 복제되면 한쪽만 고쳐진다.

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

test('모양 붙여넣기도 같은 셀 산출 함수를 쓴다', () => {
  // 같은 블록을 대상으로 하는 두 경로가 필터를 각자 갖고 있으면 한쪽만 고쳐진다.
  const ih = source('src/engine/input-handler.ts');
  const start = ih.indexOf('private applyCopiedCellPropsToSelection');
  assert.notEqual(start, -1, 'applyCopiedCellPropsToSelection 을 찾지 못했다');
  const body = ih.slice(start, ih.indexOf('\n  }\n', start));
  assert.match(body, /selectCellIndicesInRange\(/,
    '모양 붙여넣기가 셀 산출을 따로 구현하고 있다');
});
