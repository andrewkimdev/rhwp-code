import test from 'node:test';
import assert from 'node:assert/strict';

import {
  findCellIndexForRowCol,
  readTableMarkerText,
  listTopLevelTables,
  availableNestedParentBlockNames,
  type TableOutlineEntry,
} from '../src/core/table-outline.ts';
import { buildTableRoleMarkerText } from '../src/command/commands/template.ts';

// ─── buildTableRoleMarkerText: hwpx-template-engine 이 실제로 읽는 문법과
// 정확히 같은 문자열을 만드는지 확인한다(docs/TEMPLATE_MARKER_SYNTAX.md §3/§3e). ───

test('buildTableRoleMarkerText: 인자 없는 마커', () => {
  assert.equal(buildTableRoleMarkerText({ role: 'HEADER' }), '#HEADER');
  assert.equal(buildTableRoleMarkerText({ role: 'FOOTER' }), '#FOOTER');
  assert.equal(buildTableRoleMarkerText({ role: 'PAGENO' }), '#PAGENO');
});

test('buildTableRoleMarkerText: 블록명이 필요한 REPEAT-* 마커', () => {
  assert.equal(
    buildTableRoleMarkerText({ role: 'REPEAT_BODY', blockName: '품목내역' }),
    '#REPEAT-BODY:품목내역',
  );
  assert.equal(
    buildTableRoleMarkerText({ role: 'REPEAT_TITLE', blockName: '품목내역' }),
    '#REPEAT-TITLE:품목내역',
  );
  assert.equal(
    buildTableRoleMarkerText({ role: 'REPEAT_HEADER', blockName: '품목내역' }),
    '#REPEAT-HEADER:품목내역',
  );
  assert.equal(
    buildTableRoleMarkerText({ role: 'REPEAT_FOOTER', blockName: '품목내역' }),
    '#REPEAT-FOOTER:품목내역',
  );
});

test('buildTableRoleMarkerText: 블록명 없이 REPEAT-* 마커를 요청하면 던진다', () => {
  assert.throws(() => buildTableRoleMarkerText({ role: 'REPEAT_BODY' }));
});

test('buildTableRoleMarkerText: -NESTED 마커는 부모/자식 두 이름을 모두 요구한다', () => {
  assert.equal(
    buildTableRoleMarkerText({ role: 'REPEAT_BODY_NESTED', nestedParent: '수입물품내역', blockName: '물품상세내역' }),
    '#REPEAT-BODY-NESTED:수입물품내역/물품상세내역',
  );
  assert.throws(() => buildTableRoleMarkerText({ role: 'REPEAT_BODY_NESTED', blockName: '물품상세내역' }));
  assert.throws(() => buildTableRoleMarkerText({ role: 'REPEAT_BODY_NESTED', nestedParent: '수입물품내역' }));
});

// ─── table-outline: 최소 fake WasmBridge로 순수 조회 로직을 검증한다. ───

interface FakeCell {
  row: number;
  col: number;
  rowSpan: number;
  colSpan: number;
  text: string;
}

interface FakeTable {
  para: number;
  ci: number;
  /** char_offsets가 비어 있는 문단의 정밀도 손실 분기를 흉내: 문단 내 흐름상 anchor 위치. 기본 0. */
  pos?: number;
  rowCount: number;
  colCount: number;
  cells: FakeCell[];
}

/**
 * table-outline.ts가 실제로 쓰는 wasm 메서드만 구현한 최소 fake.
 *
 * `findNearestControlForward`는 실제 엔진의 same-paragraph exclusive/inclusive 경계를
 * 그대로 모델링한다 — 같은 문단(t.para === para)에서는 `inclusive`에 따라
 * `pos >= charOffset`/`pos > charOffset`을 적용하고, 이후 문단(t.para > para)은 실제
 * 엔진의 2·3단계처럼 offset 검사 없이 무조건 통과시킨다. 이 구분이 없으면 문단 0에
 * position 0으로 눌려 담긴 표를 놓치는 실제 버그를 이 fake가 재현하지 못해
 * 회귀 테스트가 거짓으로 통과한다.
 */
function makeFakeWasm(paragraphCount: number, tables: FakeTable[]) {
  return {
    getParagraphCount: (_sec: number) => paragraphCount,
    findNearestControlForward: (_sec: number, para: number, charOffset: number, inclusive = false) => {
      const candidates = tables.filter((t) => {
        if (t.para < para) return false;
        if (t.para > para) return true;
        const pos = t.pos ?? 0;
        return inclusive ? pos >= charOffset : pos > charOffset;
      });
      const next = candidates.sort((a, b) => a.para - b.para)[0];
      if (!next) return { type: 'none' };
      return { type: 'table', sec: 0, para: next.para, ci: next.ci };
    },
    getTableDimensions: (_sec: number, para: number, ci: number) => {
      const t = tables.find((x) => x.para === para && x.ci === ci)!;
      return { rowCount: t.rowCount, colCount: t.colCount, cellCount: t.cells.length };
    },
    getCellInfo: (_sec: number, para: number, ci: number, cellIdx: number) => {
      const t = tables.find((x) => x.para === para && x.ci === ci)!;
      const { row, col, rowSpan, colSpan } = t.cells[cellIdx];
      return { row, col, rowSpan, colSpan };
    },
    getCellParagraphLength: (_sec: number, para: number, ci: number, cellIdx: number, _cellParaIdx: number) => {
      const t = tables.find((x) => x.para === para && x.ci === ci)!;
      return t.cells[cellIdx].text.length;
    },
    getTextInCell: (_sec: number, para: number, ci: number, cellIdx: number) => {
      const t = tables.find((x) => x.para === para && x.ci === ci)!;
      return t.cells[cellIdx].text;
    },
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
}

test('findCellIndexForRowCol: colSpan으로 병합된 셀 안의 임의 열도 같은 cellIdx로 찾는다', () => {
  const wasm = makeFakeWasm(1, [{
    para: 0, ci: 0, rowCount: 1, colCount: 3,
    cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 3, text: '#HEADER' }],
  }]);
  assert.equal(findCellIndexForRowCol(wasm, 0, 0, 0, 0, 0), 0);
  assert.equal(findCellIndexForRowCol(wasm, 0, 0, 0, 0, 1), 0);
  assert.equal(findCellIndexForRowCol(wasm, 0, 0, 0, 0, 2), 0);
});

test('readTableMarkerText: #로 시작하지 않는(또는 빈) 첫 셀은 null', () => {
  const wasm = makeFakeWasm(1, [{
    para: 0, ci: 0, rowCount: 1, colCount: 1,
    cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '평범한 표 제목' }],
  }]);
  assert.equal(readTableMarkerText(wasm, 0, 0, 0), '평범한 표 제목');
  const emptyWasm = makeFakeWasm(1, [{
    para: 0, ci: 0, rowCount: 1, colCount: 1,
    cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '' }],
  }]);
  assert.equal(readTableMarkerText(emptyWasm, 0, 0, 0), null);
});

test('listTopLevelTables: 문서 순서대로 표를 나열하고 각 표의 마커를 읽는다', () => {
  const tables: FakeTable[] = [
    { para: 2, ci: 0, rowCount: 1, colCount: 1, cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '#HEADER' }] },
    { para: 5, ci: 0, rowCount: 3, colCount: 2, cells: [
      { row: 0, col: 0, rowSpan: 1, colSpan: 2, text: '#REPEAT-BODY:품목내역' },
      { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '' },
      { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
      { row: 2, col: 0, rowSpan: 1, colSpan: 1, text: '' },
      { row: 2, col: 1, rowSpan: 1, colSpan: 1, text: '' },
    ] },
    { para: 9, ci: 0, rowCount: 1, colCount: 1, cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '' }] },
  ];
  const wasm = makeFakeWasm(12, tables);
  const outline = listTopLevelTables(wasm, 0);
  assert.equal(outline.length, 3);
  assert.deepEqual(outline.map((e: TableOutlineEntry) => e.parentPara), [2, 5, 9]);
  assert.deepEqual(outline.map((e: TableOutlineEntry) => e.markerText), ['#HEADER', '#REPEAT-BODY:품목내역', null]);
  assert.deepEqual(availableNestedParentBlockNames(outline), ['품목내역']);
});

// gov-form-simple-sample.hwp에서 실제로 재현된 버그: char_offsets가 비어 있는 문단(텍스트
// 없이 표만 있는 문단)에서 첫 컨트롤은 position 0으로 눌려 담긴다. 워커가 매 이터레이션을
// charOffset=0으로 시작하므로 exclusive 모드로는 그 표를 구조적으로 절대 찾지 못했다.

test('listTopLevelTables: char_offsets가 비어 있는 문단에서 position 0에 눌린 첫 표도 찾는다', () => {
  const wasm = makeFakeWasm(1, [
    { para: 0, ci: 0, pos: 0, rowCount: 30, colCount: 4, cells: [
      { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '' },
    ] },
  ]);
  const outline = listTopLevelTables(wasm, 0);
  assert.equal(outline.length, 1);
  assert.equal(outline[0].parentPara, 0);
});

test('listTopLevelTables: 문단 0의 표를 포함해도 이후 표들을 모두 정확한 순서로 찾는다', () => {
  const tables: FakeTable[] = [
    { para: 0, ci: 0, pos: 0, rowCount: 30, colCount: 4, cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '' }] },
    { para: 2, ci: 0, rowCount: 1, colCount: 1, cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '#HEADER' }] },
    { para: 5, ci: 0, rowCount: 1, colCount: 1, cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '' }] },
  ];
  const wasm = makeFakeWasm(6, tables);
  const outline = listTopLevelTables(wasm, 0);
  assert.deepEqual(outline.map((e: TableOutlineEntry) => e.parentPara), [0, 2, 5]);
});

test('listTopLevelTables: 연속된 문단이 각각 자기 문단 position 0에서 시작해도 매 반복에서 찾는다', () => {
  // "재개한 문단 자신이 position 0에서 시작하면 매 반복에서 재발"하는 시나리오.
  const wasm = makeFakeWasm(2, [
    { para: 0, ci: 0, pos: 0, rowCount: 1, colCount: 1, cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '' }] },
    { para: 1, ci: 0, pos: 0, rowCount: 1, colCount: 1, cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '' }] },
  ]);
  const outline = listTopLevelTables(wasm, 0);
  assert.deepEqual(outline.map((e: TableOutlineEntry) => e.parentPara), [0, 1]);
});
