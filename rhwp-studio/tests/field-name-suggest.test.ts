import test from 'node:test';
import assert from 'node:assert/strict';

import {
  readTableGrid,
  buildSectionPrefixMap,
  suggestFieldNames,
  isRepeatTaggedTable,
} from '../src/core/field-name-suggest.ts';

interface FakeCell {
  row: number;
  col: number;
  rowSpan: number;
  colSpan: number;
  text: string;
  fieldName?: string;
}

/**
 * field-name-suggest.ts가 실제로 쓰는 wasm 메서드만 구현한 최소 fake — 표는 항상
 * `(sec=0, ppi=0, ci=0)` 하나뿐이라고 가정한다(`template-marker-authoring.test.ts`의
 * makeFakeWasm과 달리 여러 표 탐색은 필요 없다 — 호출자가 이미 "현재 표"를 골랐다).
 */
function makeFakeTableWasm(cells: FakeCell[], existingFieldNames: string[] = []) {
  const rowCount = Math.max(0, ...cells.map((c) => c.row + c.rowSpan));
  const colCount = Math.max(0, ...cells.map((c) => c.col + c.colSpan));
  return {
    getTableDimensions: (_sec: number, _ppi: number, _ci: number) => ({
      rowCount,
      colCount,
      cellCount: cells.length,
    }),
    getCellInfo: (_sec: number, _ppi: number, _ci: number, cellIdx: number) => {
      const { row, col, rowSpan, colSpan } = cells[cellIdx];
      return { row, col, rowSpan, colSpan };
    },
    getCellParagraphCount: (_sec: number, _ppi: number, _ci: number, _cellIdx: number) => 1,
    getCellParagraphLength: (_sec: number, _ppi: number, _ci: number, cellIdx: number, _cpi: number) =>
      cells[cellIdx].text.length,
    getTextInCell: (_sec: number, _ppi: number, _ci: number, cellIdx: number) => cells[cellIdx].text,
    getCellProperties: (_sec: number, _ppi: number, _ci: number, cellIdx: number) => ({
      fieldName: cells[cellIdx].fieldName,
    }),
    getFieldList: () =>
      existingFieldNames.map((name, i) => ({
        fieldId: i,
        fieldType: 'ClickHere',
        cellField: false,
        name,
        guide: '',
        command: '',
        value: '',
        location: { sectionIndex: 0, paraIndex: 0 },
      })),
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
}

// 3열: col0=섹션 앵커(rowSpan>1이면 접두어 출처), col1=라벨, col2=값(빈 칸).
// 5555817.hwpx(법인설립허가신청서)의 "신\n청\n인"(rowSpan 2)/"법\n인\n명"(rowSpan 3)
// 모양을 재현 — 같은 라벨 "전화번호"가 두 섹션에 각각 나타나 접두어 없이는 충돌한다.
function buildGovFormFixtureCells(): FakeCell[] {
  return [
    // 신청인 (rowSpan 2)
    { row: 0, col: 0, rowSpan: 2, colSpan: 1, text: '신\n청\n인' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '주소' },
    { row: 0, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    // 법인명 (rowSpan 3)
    { row: 2, col: 0, rowSpan: 3, colSpan: 1, text: '법\n인\n명' },
    { row: 2, col: 1, rowSpan: 1, colSpan: 1, text: '명칭' },
    { row: 2, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 3, col: 1, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 3, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 4, col: 1, rowSpan: 1, colSpan: 1, text: '소재지' },
    { row: 4, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    // 섹션 밖(부모 없음) — 접두어가 붙지 않아야 한다.
    { row: 5, col: 1, rowSpan: 1, colSpan: 1, text: '담당자' },
    { row: 5, col: 2, rowSpan: 1, colSpan: 1, text: '' },
  ];
}

test('buildSectionPrefixMap: column-0 rowSpan>1 앵커의 텍스트를 rowSpan이 덮는 모든 행에 매핑한다', () => {
  const wasm = makeFakeTableWasm(buildGovFormFixtureCells());
  const grid = readTableGrid(wasm, 0, 0, 0);
  const prefixMap = buildSectionPrefixMap(grid);
  assert.equal(prefixMap.get(0), '신청인'); // 줄바꿈(음절별) 제거됨
  assert.equal(prefixMap.get(1), '신청인');
  assert.equal(prefixMap.get(2), '법인명');
  assert.equal(prefixMap.get(3), '법인명');
  assert.equal(prefixMap.get(4), '법인명');
  assert.equal(prefixMap.has(5), false);
});

test('suggestFieldNames: 같은 라벨이 서로 다른 섹션에 있으면 접두어로 구분한다', () => {
  const wasm = makeFakeTableWasm(buildGovFormFixtureCells());
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  const byRow = new Map(suggestions.map((s) => [s.row, s]));

  assert.equal(byRow.get(0)?.suggestedName, '신청인_주소');
  assert.equal(byRow.get(1)?.suggestedName, '신청인_전화번호');
  assert.equal(byRow.get(2)?.suggestedName, '법인명_명칭');
  assert.equal(byRow.get(3)?.suggestedName, '법인명_전화번호');
  assert.equal(byRow.get(4)?.suggestedName, '법인명_소재지');
  // 섹션 밖 필드는 접두어 없이 그대로.
  assert.equal(byRow.get(5)?.suggestedName, '담당자');
  assert.equal(suggestions.length, 6);
});

test('suggestFieldNames: 배치 내 이름 충돌은 _2, _3 접미어로 해소한다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '담당자' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '담당자' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
    { row: 2, col: 0, rowSpan: 1, colSpan: 1, text: '담당자' },
    { row: 2, col: 1, rowSpan: 1, colSpan: 1, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['담당자', '담당자_2', '담당자_3'],
  );
});

test('suggestFieldNames: 문서에 이미 있는 필드명과 충돌해도 접미어로 해소한다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '부서' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells, ['부서']);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  assert.equal(suggestions[0].suggestedName, '부서_2');
});

test('suggestFieldNames: 이미 필드가 있는 셀은 삽입 대상에서 빠지고 플래그만 남는다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '', fieldName: '전화번호_기존' },
  ];
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  assert.equal(suggestions.length, 1);
  assert.equal(suggestions[0].alreadyHasField, true);
  assert.equal(suggestions[0].existingFieldName, '전화번호_기존');
});

test('suggestFieldNames: 라벨 옆에 빈 칸이 없으면 후보를 만들지 않는다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '이미 채워짐' },
  ];
  const wasm = makeFakeTableWasm(cells);
  assert.deepEqual(suggestFieldNames(wasm, 0, 0, 0), []);
});

test('isRepeatTaggedTable: #REPEAT-* 계열 마커(및 -NESTED)만 참으로 판정한다', () => {
  assert.equal(isRepeatTaggedTable('#REPEAT-BODY:품목내역'), true);
  assert.equal(isRepeatTaggedTable('#REPEAT-HEADER:품목내역'), true);
  assert.equal(isRepeatTaggedTable('#REPEAT-FOOTER:품목내역'), true);
  assert.equal(isRepeatTaggedTable('#REPEAT-TITLE:품목내역'), true);
  assert.equal(isRepeatTaggedTable('#REPEAT-BODY-NESTED:수입물품내역/물품상세내역'), true);
  assert.equal(isRepeatTaggedTable('#HEADER'), false);
  assert.equal(isRepeatTaggedTable('평범한 표 제목'), false);
  assert.equal(isRepeatTaggedTable(null), false);
});
