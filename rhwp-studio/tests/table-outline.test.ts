import test from 'node:test';
import assert from 'node:assert/strict';

import { expandRowRangeForMerges } from '../src/core/table-outline.ts';
import type { CellBbox } from '../src/core/types.ts';

/** 테스트에 필요한 row/col/rowSpan/colSpan만 채우고 나머지 bbox 필드는 0으로 둔다. */
function cell(row: number, col: number, rowSpan = 1, colSpan = 1): CellBbox {
  return { cellIdx: 0, row, col, rowSpan, colSpan, pageIndex: 0, x: 0, y: 0, w: 0, h: 0 };
}

test('expandRowRangeForMerges: 병합 없는 표는 그대로 반환한다', () => {
  const cells = [cell(0, 0), cell(0, 1), cell(1, 0), cell(1, 1)];
  const result = expandRowRangeForMerges(cells, { startRow: 0, endRow: 0 });
  assert.deepEqual(result, { startRow: 0, endRow: 0 });
});

test('expandRowRangeForMerges: 끝쪽에 걸치는 rowSpan을 아래로 넓힌다', () => {
  // 선택은 row 0뿐이지만, col 1에 row0~1을 덮는 병합 셀이 있다.
  const cells = [cell(0, 0), cell(0, 1, 2), cell(1, 0)];
  const result = expandRowRangeForMerges(cells, { startRow: 0, endRow: 0 });
  assert.deepEqual(result, { startRow: 0, endRow: 1 });
});

test('expandRowRangeForMerges: 시작쪽에 걸치는 rowSpan을 위로 넓힌다', () => {
  // 선택은 row 1뿐이지만, col 1에 row0~1을 덮는 병합 셀이 있다.
  const cells = [cell(0, 0), cell(0, 1, 2), cell(1, 0)];
  const result = expandRowRangeForMerges(cells, { startRow: 1, endRow: 1 });
  assert.deepEqual(result, { startRow: 0, endRow: 1 });
});

test('expandRowRangeForMerges: 연쇄 확장이 필요한 경우 고정점까지 넓힌다', () => {
  // col1의 병합이 row0~1을 끌어들이고, 그 결과 row1과 겹치는 col2의 병합이
  // 다시 row1~2를 끌어들인다 — 최초 선택은 row0뿐.
  const cells = [
    cell(0, 0),
    cell(0, 1, 2), // rows 0-1
    cell(1, 2, 2), // rows 1-2
    cell(2, 0),
  ];
  const result = expandRowRangeForMerges(cells, { startRow: 0, endRow: 0 });
  assert.deepEqual(result, { startRow: 0, endRow: 2 });
});

test('expandRowRangeForMerges: 5446216.hwpx 헤더 레이아웃 — 일부 컬럼만 rowSpan:2', () => {
  // 번호/발급일/구분/단체명/대표자/수령인(col 0-4,6)은 rowSpan:2로 row2를 덮고,
  // "주소(대표자)"/"사업장 소재지"(col 5)만 row2/row3로 나뉜 별개 행이다.
  const cells = [
    cell(2, 0, 2), cell(2, 1, 2), cell(2, 2, 2), cell(2, 3, 2), cell(2, 4, 2),
    cell(2, 5, 1), // 주소(대표자)
    cell(3, 5, 1), // 사업장 소재지
    cell(2, 6, 2),
  ];
  // "번호"에서 "수령인"까지 드래그하면 anchor/focus가 모두 row 2에 걸려
  // {startRow:2, endRow:2}만 잡힌다 — row 3("사업장 소재지")이 빠진 상태.
  const result = expandRowRangeForMerges(cells, { startRow: 2, endRow: 2 });
  assert.deepEqual(result, { startRow: 2, endRow: 3 });
});
