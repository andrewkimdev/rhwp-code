import test from 'node:test';
import assert from 'node:assert/strict';

import { ensurePageNoFields } from '../src/command/commands/template-ops.ts';
import { CURRENT_PAGE_FIELD_NAME, TOTAL_PAGES_FIELD_NAME } from '../src/core/template-marker.ts';

// `ensurePageNoFields`가 `#PAGENO` 태깅 후 표의 마지막 행에 `현재_페이지 / 전체_페이지`를
// 채우는지 검증한다. `setTableRoleMarker`가 이미 마커 행(row 0)을 만든 *이후* 상태를
// 직접 넣어주는 쪽이(전체 `tagSelectionOperation`을 거치지 않고) applyMarkerRowStyle의
// 스타일링 wasm 호출까지 fake로 재현할 필요가 없어 더 좁고 안정적이다.

interface FakeCell {
  row: number;
  col: number;
  rowSpan: number;
  colSpan: number;
  text: string;
}

interface Call {
  method: string;
  args: unknown[];
}

function makeFakeWasm(rowCount: number, colCount: number, cells: FakeCell[], fieldNames: string[] = []) {
  const state = {
    rowCount,
    colCount,
    cells: cells.map((c) => ({ ...c })),
    fieldNames: [...fieldNames],
  };
  const calls: Call[] = [];

  const wasm = {
    calls,
    getFieldList: () => state.fieldNames.map((name) => ({ name })),
    getTableDimensions: (_sec: number, _ppi: number, _ci: number) => ({
      rowCount: state.rowCount,
      colCount: state.colCount,
      cellCount: state.cells.length,
    }),
    getCellInfo: (_sec: number, _ppi: number, _ci: number, cellIdx: number) => {
      const c = state.cells[cellIdx];
      return { row: c.row, col: c.col, rowSpan: c.rowSpan, colSpan: c.colSpan };
    },
    getCellParagraphLength: (_sec: number, _ppi: number, _ci: number, cellIdx: number, _cellParaIdx: number) =>
      state.cells[cellIdx].text.length,
    mergeTableCells: (_sec: number, _ppi: number, _ci: number, r0: number, c0: number, r1: number, c1: number) => {
      calls.push({ method: 'mergeTableCells', args: [r0, c0, r1, c1] });
      const idxs = state.cells
        .map((_c, i) => i)
        .filter((i) => state.cells[i].row === r0 && state.cells[i].row + state.cells[i].rowSpan - 1 <= r1 && state.cells[i].col >= c0 && state.cells[i].col <= c1);
      const [first, ...rest] = idxs;
      state.cells[first].colSpan = c1 - c0 + 1;
      for (const i of [...rest].sort((a, b) => b - a)) state.cells.splice(i, 1);
    },
    insertTextInCell: (
      _sec: number, _ppi: number, _ci: number, cellIdx: number, _cellParaIdx: number, offset: number, text: string,
    ) => {
      calls.push({ method: 'insertTextInCell', args: [cellIdx, offset, text] });
      const cell = state.cells[cellIdx];
      cell.text = cell.text.slice(0, offset) + text + cell.text.slice(offset);
    },
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    insertClickHereField: (pos: any, guide: string, memo: string, name: string, editable: boolean) => {
      calls.push({ method: 'insertClickHereField', args: [pos.cellIndex, pos.charOffset, guide, name, editable] });
      state.fieldNames.push(name);
      return { ok: true, fieldId: state.fieldNames.length, charOffset: pos.charOffset };
    },
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
  return wasm;
}

test('ensurePageNoFields: 단일 열 표 — 마지막 행에 현재_페이지 / 전체_페이지를 순서대로 삽입한다', () => {
  const wasm = makeFakeWasm(2, 1, [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '#PAGENO' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '' },
  ]);
  ensurePageNoFields(wasm, 0, 0, 0);

  assert.deepEqual(
    wasm.calls.map((c: Call) => c.method),
    ['insertTextInCell', 'insertClickHereField', 'insertClickHereField'],
  );
  assert.deepEqual(wasm.calls[0].args, [1, 0, ' / ']);
  assert.deepEqual(wasm.calls[1].args, [1, 0, CURRENT_PAGE_FIELD_NAME, CURRENT_PAGE_FIELD_NAME, false]);
  assert.deepEqual(wasm.calls[2].args, [1, 3, TOTAL_PAGES_FIELD_NAME, TOTAL_PAGES_FIELD_NAME, false]);
});

test('ensurePageNoFields: 다열 표 — 마지막 행을 병합한 뒤 그 셀에 채운다', () => {
  const wasm = makeFakeWasm(2, 3, [
    { row: 0, col: 0, rowSpan: 1, colSpan: 3, text: '#PAGENO' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '' },
  ]);
  ensurePageNoFields(wasm, 0, 0, 0);

  assert.deepEqual(wasm.calls[0], { method: 'mergeTableCells', args: [1, 0, 1, 2] });
  assert.deepEqual(wasm.calls[1].args, [1, 0, ' / ']); // 병합 후 남은 셀 인덱스는 1(마커 셀이 0)
  assert.deepEqual(wasm.calls[2].args, [1, 0, CURRENT_PAGE_FIELD_NAME, CURRENT_PAGE_FIELD_NAME, false]);
  assert.deepEqual(wasm.calls[3].args, [1, 3, TOTAL_PAGES_FIELD_NAME, TOTAL_PAGES_FIELD_NAME, false]);
});

test('ensurePageNoFields: 현재_페이지가 이미 있으면(재태깅 등) 아무것도 하지 않는다', () => {
  const wasm = makeFakeWasm(2, 1, [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '#PAGENO' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '' },
  ], [CURRENT_PAGE_FIELD_NAME]);
  ensurePageNoFields(wasm, 0, 0, 0);
  assert.deepEqual(wasm.calls, []);
});

test('ensurePageNoFields: 마지막 행에 이미 내용이 있으면 건드리지 않는다(다열 병합도 하지 않는다)', () => {
  const wasm = makeFakeWasm(2, 2, [
    { row: 0, col: 0, rowSpan: 1, colSpan: 2, text: '#PAGENO' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '기존 내용' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
  ]);
  ensurePageNoFields(wasm, 0, 0, 0);
  assert.deepEqual(wasm.calls, []); // mergeTableCells조차 호출되지 않아야 한다 — 내용을 먼저 확인
});
