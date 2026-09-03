import test from 'node:test';
import assert from 'node:assert/strict';

import {
  readTableGrid,
  buildSectionPrefixMap,
  suggestFieldNames,
  isRepeatTaggedTable,
  isTemplateTableMarkerText,
  isRepeatBodyMarkerText,
  isRepeatFooterMarkerText,
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
function makeFakeTableWasm(
  cells: FakeCell[],
  existingFieldNames: string[] = [],
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  getFieldInfoAt?: (pos: any) => { inField: boolean },
) {
  const rowCount = Math.max(0, ...cells.map((c) => c.row + c.rowSpan));
  const colCount = Math.max(0, ...cells.map((c) => c.col + c.colSpan));
  return {
    // 규칙 5(repeat-header-column-match)가 `listTopLevelTables`로 문서를 훑는다 —
    // 이 fake는 표가 (sec=0, ppi=0, ci=0) 하나뿐이라고 가정하므로, 그 표 하나만
    // 돌려주면 충분하다(직전 표가 없으니 규칙 5는 항상 후보 0개로 조용히 빠진다).
    getParagraphCount: (_sec: number) => 1,
    findNearestControlForward: (_sec: number, para: number, _charOffset: number, _inclusive = false) =>
      para > 0 ? { type: 'none' } : { type: 'table', sec: 0, para: 0, ci: 0 },
    getFieldInfoAt: getFieldInfoAt ?? (() => ({ inField: false })),
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

// buildGovFormFixtureCells를 #HEADER 태깅한 모양 — template:tag-selection의
// setTableRoleMarker(command/commands/template.ts)는 표 전체 폭을 병합한 마커
// 행을 row 0에 삽입하므로 기존 내용은 한 행씩 내려간다.
function buildTaggedGovFormFixtureCells(markerText = '#HEADER'): FakeCell[] {
  return [
    { row: 0, col: 0, rowSpan: 1, colSpan: 3, text: markerText },
    // 신청인 (rowSpan 2)
    { row: 1, col: 0, rowSpan: 2, colSpan: 1, text: '신\n청\n인' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '주소' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 2, col: 1, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 2, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    // 법인명 (rowSpan 3)
    { row: 3, col: 0, rowSpan: 3, colSpan: 1, text: '법\n인\n명' },
    { row: 3, col: 1, rowSpan: 1, colSpan: 1, text: '명칭' },
    { row: 3, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 4, col: 1, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 4, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 5, col: 1, rowSpan: 1, colSpan: 1, text: '소재지' },
    { row: 5, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    // 섹션 밖(부모 없음) — 접두어가 붙지 않아야 한다.
    { row: 6, col: 1, rowSpan: 1, colSpan: 1, text: '담당자' },
    { row: 6, col: 2, rowSpan: 1, colSpan: 1, text: '' },
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

test('suggestFieldNames: 빈 셀 채우기 후보도 실제 삽입 지점에 이미 필드가 있으면(getFieldInfoAt) 감지한다', () => {
  // insertClickHereField로 실제 생성되는 누름틀은 셀의 `fieldName` 속성(표 셀 속성
  // 대화상자의 별개 기능)이 아니라 그 셀 첫 문단(charOffset 0)에 들어간다 — 같은
  // 행을 다시 스캔했을 때 getCellProperties().fieldName만 보면 놓치므로, apply가
  // 실제로 쓰는 지점(cellParaIndex 0, charOffset 0)에 getFieldInfoAt으로 물어본다.
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '' }, // fieldName 속성은 없음
  ];
  const wasm = makeFakeTableWasm(cells, [], (pos) =>
    pos.cellIndex === 1 && pos.charOffset === 0 && pos.cellParaIndex === 0
      ? { inField: true }
      : { inField: false },
  );
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  assert.equal(suggestions.length, 1);
  assert.equal(suggestions[0].alreadyHasField, true);
});

test('suggestFieldNames: 라벨 옆에 빈 칸이 없으면 후보를 만들지 않는다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 1, text: '전화번호' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '이미 채워짐' },
  ];
  const wasm = makeFakeTableWasm(cells);
  assert.deepEqual(suggestFieldNames(wasm, 0, 0, 0), []);
});

// 17856415.hwp(의무경찰 지원서)의 row9/10 모양 — 라벨이 오른쪽이 아니라 "아래"의
// 같은 colSpan 빈 행에 답을 쓴다.
test('labelAboveBlankRule: 라벨 바로 아래 같은 colSpan의 빈 행을 후보로 삼는다(그 밖의 특이사항)', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 14, text: '그 밖의 특이사항' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 14, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  assert.equal(suggestions.length, 1);
  assert.equal(suggestions[0].leafText, '그밖의특이사항'); // strippedText — 공백 제거
  assert.equal(suggestions[0].suggestedName, '그밖의특이사항');
  // 빈 셀 채우기 후보다 — 인라인 삽입이 아니므로 insertAt이 없어야 한다.
  assert.equal(suggestions[0].insertAt, undefined);
});

test('labelAboveBlankRule: 아래 빈 셀의 colSpan이 라벨과 다르면 후보를 만들지 않는다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 14, text: '그 밖의 특이사항' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 7, text: '' },
    { row: 1, col: 7, rowSpan: 1, colSpan: 7, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  assert.deepEqual(suggestFieldNames(wasm, 0, 0, 0), []);
});

// 17856415.hwp의 row5-7 모양 — "인적사항"(rowSpan3) 섹션 아래 6개 라벨이 각각 넓은
// 셀 안에 혼자 있고, 별도 빈 셀도 아래 빈 행도 없다 — 라벨 뒤 인라인 여백에 답을
// 쓴다. row3(=실제 문서의 row8)은 전체 폭 빈 여백 행 — 겹치는 셀이 "하나뿐"이므로
// 가드 5(2개 이상 겹칠 때만 제외)의 경계 케이스를 함께 검증한다.
test('labelInlineRoomRule: 인적사항 섹션의 라벨+인라인 여백 6칸을 후보로 삼는다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 3, colSpan: 1, text: '인적사항' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 9, text: '성명' },
    { row: 0, col: 10, rowSpan: 1, colSpan: 4, text: '병적지청' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 9, text: '주민등록번호' },
    { row: 1, col: 10, rowSpan: 1, colSpan: 4, text: '전자우편주소' },
    { row: 2, col: 1, rowSpan: 1, colSpan: 9, text: '전화번호' },
    { row: 2, col: 10, rowSpan: 1, colSpan: 4, text: '휴대전화번호' },
    { row: 3, col: 0, rowSpan: 1, colSpan: 14, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  const byLeaf = new Map(suggestions.map((s) => [s.leafText, s]));
  assert.equal(suggestions.length, 6);
  for (const leaf of ['성명', '병적지청', '주민등록번호', '전자우편주소', '전화번호', '휴대전화번호']) {
    const s = byLeaf.get(leaf);
    assert.ok(s, `${leaf} 후보가 있어야 한다`);
    assert.equal(s?.suggestedName, `인적사항_${leaf}`);
    assert.equal(s?.sectionPrefix, '인적사항');
    assert.ok(s?.insertAt, `${leaf}는 insertAt이 있어야 한다`);
    assert.equal(s?.insertAt?.cellParaIndex, 0);
    assert.equal(s?.insertAt?.charOffset, leaf.length);
  }
});

test('labelInlineRoomRule: 오른쪽에 진짜 빈 셀이 있으면 규칙 2와 중복 후보를 만들지 않는다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 2, colSpan: 1, text: '인적사항' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '성명' },
    { row: 0, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '주소' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  assert.equal(suggestions.length, 2);
  assert.deepEqual(suggestions.map((s) => s.leafText).sort(), ['성명', '주소']);
  assert.ok(suggestions.every((s) => s.insertAt === undefined));
});

// 17856415.hwp의 row13 모양을 "인적사항"류 섹션 안에 재현 — "지원자" 뒤 빈 칸은
// 규칙 2가 이미 채움 대상으로 찜하므로, 그 뒤에 오는 "(서명 또는 인)"은 라벨이
// 아니라 서명 안내문이다(가드 4).
test('labelInlineRoomRule: 이미 찜한 빈 칸 뒤의 서명 안내문("(서명 또는 인)")은 후보로 만들지 않는다', () => {
  const cells: FakeCell[] = [
    // rowSpan2 앵커 — row1에는 별도 셀을 두지 않는다(buildSectionPrefixMap은 앵커
    // 자신의 row/rowSpan만으로 매핑하므로, row1에 다른 셀이 없어도 prefixMap.has(1)은
    // 참이 된다 — 이 테스트에서 필요한 건 row0뿐).
    { row: 0, col: 0, rowSpan: 2, colSpan: 1, text: '인적사항' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 1, text: '지원자' },
    { row: 0, col: 2, rowSpan: 1, colSpan: 1, text: '' },
    { row: 0, col: 3, rowSpan: 1, colSpan: 1, text: '(서명 또는 인)' },
  ];
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  assert.equal(suggestions.length, 1);
  assert.ok(!suggestions.some((s) => s.leafText.includes('서명')));
  assert.deepEqual(suggestions.map((s) => s.leafText).sort(), ['지원자']);
});

// 17856415.hwp의 row3/4 모양(응시지역/모집분야/모집회차, colSpan [4,6,4]) 위에 그
// 답변이 [1,2,2,2,1,3,2,1] colSpan의 8개 서브셀로 갈라져 있어 경계가 어긋난다 —
// 섹션 앵커 안에서도(가드 6을 통과해도) 가드 5가 이 오정렬 답변 그리드를 걸러야
// 한다.
test('labelInlineRoomRule: 열 경계가 어긋난 답변 그리드(응시지역류)는 후보로 만들지 않는다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 2, colSpan: 1, text: '섹션' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 4, text: '응시지역' },
    { row: 0, col: 5, rowSpan: 1, colSpan: 6, text: '모집분야' },
    { row: 0, col: 11, rowSpan: 1, colSpan: 4, text: '모집회차' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 2, text: '' },
    { row: 1, col: 4, rowSpan: 1, colSpan: 2, text: '' },
    { row: 1, col: 6, rowSpan: 1, colSpan: 2, text: '' },
    { row: 1, col: 8, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 9, rowSpan: 1, colSpan: 3, text: '' },
    { row: 1, col: 12, rowSpan: 1, colSpan: 2, text: '' },
    { row: 1, col: 14, rowSpan: 1, colSpan: 1, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  assert.deepEqual(suggestFieldNames(wasm, 0, 0, 0), []);
});

test('labelInlineRoomRule: 삽입 지점에 이미 필드가 있으면 alreadyHasField로 표시하고 제외한다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 2, colSpan: 1, text: '인적사항' },
    { row: 0, col: 1, rowSpan: 1, colSpan: 9, text: '성명' },
    { row: 0, col: 10, rowSpan: 1, colSpan: 4, text: '병적지청' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 9, text: '주민등록번호' },
    { row: 1, col: 10, rowSpan: 1, colSpan: 4, text: '전자우편주소' },
  ];
  // cellIdx 1 == "성명"(cells 배열의 인덱스) — 그 삽입 지점(charOffset === "성명".length)에
  // 이미 필드가 있다고 가정한다.
  const wasm = makeFakeTableWasm(cells, [], (pos: { cellIndex?: number; charOffset: number }) => ({
    inField: pos.cellIndex === 1 && pos.charOffset === 2,
  }));
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  const seongmyeong = suggestions.find((s) => s.leafText === '성명');
  assert.ok(seongmyeong);
  assert.equal(seongmyeong?.alreadyHasField, true);
  const others = suggestions.filter((s) => s.leafText !== '성명');
  assert.equal(others.length, 3);
  assert.ok(others.every((s) => s.alreadyHasField === false));
});

// 실제 버그 리포트에 대한 직접적 회귀 가드 — 17856415.hwp(의무경찰 지원서) 본문
// 표의 43개 셀을 그대로 옮긴다. `rhwp export-tables 17856415.hwp --json`으로 확인한
// 실측 그리드다(rowCount 25 x colCount 14).
test('suggestFieldNames: 17856415.hwp(의무경찰 지원서) 표 전체 — 지원자/그 밖의 특이사항/인적사항 6칸만 제안된다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 8, text: '■ 의무경찰대 설치 및 운영에 관한 법률 시행령 [별지 제2호의2서식] <개정 2020. 12. 31.>' },
    { row: 0, col: 8, rowSpan: 1, colSpan: 6, text: '대한민국 의무경찰 홈페이지(www.ap.police.go.kr)에서도 지원할 수 있습니다.' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 14, text: '의무경찰 지원서' },
    { row: 2, col: 0, rowSpan: 1, colSpan: 14, text: '※ 작성방법을 읽고 정확히 작성하여 주시기 바랍니다.' },
    { row: 3, col: 0, rowSpan: 1, colSpan: 4, text: '응시지역' },
    { row: 3, col: 4, rowSpan: 1, colSpan: 6, text: '모집분야' },
    { row: 3, col: 10, rowSpan: 1, colSpan: 4, text: '모집회차' },
    { row: 4, col: 0, rowSpan: 1, colSpan: 1, text: '' },
    { row: 4, col: 1, rowSpan: 1, colSpan: 2, text: '' },
    { row: 4, col: 3, rowSpan: 1, colSpan: 2, text: '' },
    { row: 4, col: 5, rowSpan: 1, colSpan: 2, text: '' },
    { row: 4, col: 7, rowSpan: 1, colSpan: 1, text: '' },
    { row: 4, col: 8, rowSpan: 1, colSpan: 3, text: '' },
    { row: 4, col: 11, rowSpan: 1, colSpan: 2, text: '' },
    { row: 4, col: 13, rowSpan: 1, colSpan: 1, text: '' },
    { row: 5, col: 0, rowSpan: 3, colSpan: 1, text: '인적사항' },
    { row: 5, col: 1, rowSpan: 1, colSpan: 9, text: '성명' },
    { row: 5, col: 10, rowSpan: 1, colSpan: 4, text: '병적지청' },
    { row: 6, col: 1, rowSpan: 1, colSpan: 9, text: '주민등록번호' },
    { row: 6, col: 10, rowSpan: 1, colSpan: 4, text: '전자우편주소' },
    { row: 7, col: 1, rowSpan: 1, colSpan: 9, text: '전화번호' },
    { row: 7, col: 10, rowSpan: 1, colSpan: 4, text: '휴대전화번호' },
    { row: 8, col: 0, rowSpan: 1, colSpan: 14, text: '' },
    { row: 9, col: 0, rowSpan: 1, colSpan: 14, text: '그 밖의 특이사항' },
    { row: 10, col: 0, rowSpan: 1, colSpan: 14, text: '' },
    { row: 11, col: 0, rowSpan: 1, colSpan: 14, text: '「병역법」 제20조 및 「의무경찰대 설치 및 운영에 관한 법률 시행령」 제9조에 따라 지원서를 제출합니다.' },
    { row: 12, col: 0, rowSpan: 1, colSpan: 14, text: '년          월          일장' },
    { row: 13, col: 0, rowSpan: 1, colSpan: 6, text: '' },
    { row: 13, col: 6, rowSpan: 1, colSpan: 3, text: '지원자' },
    { row: 13, col: 9, rowSpan: 1, colSpan: 3, text: '' },
    { row: 13, col: 12, rowSpan: 1, colSpan: 2, text: '(서명 또는 인)' },
    { row: 14, col: 0, rowSpan: 1, colSpan: 14, text: '경찰청장 또는 해양경찰청장 귀하' },
    { row: 15, col: 0, rowSpan: 1, colSpan: 2, text: '첨부서류\n(해당하는 사람만 제출합니다)' },
    { row: 15, col: 2, rowSpan: 1, colSpan: 12, text: '1. 병역판정 신체검사를 받은 사람의 경우 병역판정 신체검사결과 통보서' },
    { row: 16, col: 0, rowSpan: 1, colSpan: 14, text: '' },
    { row: 17, col: 0, rowSpan: 1, colSpan: 14, text: '행정정보 공동이용 동의서' },
    { row: 18, col: 0, rowSpan: 1, colSpan: 14, text: '본인은 이 건의 업무처리와 관련하여 담당 공무원이 「전자정부법」 제36조제1항에 따른 행정정보의 공동이용을 통하여 아래 항목의 자료를 확인하는 것에 동의합니다.' },
    { row: 19, col: 0, rowSpan: 1, colSpan: 14, text: '' },
    { row: 20, col: 0, rowSpan: 1, colSpan: 14, text: '작성방법' },
    { row: 21, col: 0, rowSpan: 1, colSpan: 14, text: '1. 응시지역란은 병적지와 관계없이 면접전형 등을 받을 시ㆍ도경찰청 또는 지방해양경찰청을 적습니다.' },
    { row: 22, col: 0, rowSpan: 1, colSpan: 14, text: '' },
    { row: 23, col: 0, rowSpan: 1, colSpan: 14, text: '의무경찰 지원서 접수증' },
    { row: 24, col: 0, rowSpan: 1, colSpan: 14, text: '210㎜×297㎜[백상지 80g/㎡(재활용품)]' },
  ];
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0);
  const leafTexts = suggestions.map((s) => s.leafText).sort();
  assert.deepEqual(
    leafTexts,
    ['그밖의특이사항', '병적지청', '성명', '전자우편주소', '전화번호', '주민등록번호', '지원자', '휴대전화번호'].sort(),
  );
  const byLeaf = new Map(suggestions.map((s) => [s.leafText, s]));
  assert.equal(byLeaf.get('지원자')?.sectionPrefix, null);
  assert.equal(byLeaf.get('그밖의특이사항')?.sectionPrefix, null);
  assert.equal(byLeaf.get('성명')?.suggestedName, '인적사항_성명');
  assert.equal(byLeaf.get('병적지청')?.suggestedName, '인적사항_병적지청');
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

test('isTemplateTableMarkerText: 게이트 어휘(#BLOCK/#PAGENO/#REPEAT-*, 및 폐지된 동의어 #HEADER/#FOOTER)만 참으로 판정한다', () => {
  assert.equal(isTemplateTableMarkerText('#BLOCK'), true);
  assert.equal(isTemplateTableMarkerText('#HEADER'), true);
  assert.equal(isTemplateTableMarkerText('#FOOTER'), true);
  assert.equal(isTemplateTableMarkerText('#PAGENO'), true);
  assert.equal(isTemplateTableMarkerText('#BLOCK-BOTTOM'), true);
  assert.equal(isTemplateTableMarkerText('#NOTE'), true);
  assert.equal(isTemplateTableMarkerText('#REPEAT-BODY:품목내역'), true);
  assert.equal(isTemplateTableMarkerText('#REPEAT-BODY-BOTTOM:품목내역'), true);
  assert.equal(isTemplateTableMarkerText('#REPEAT-TITLE-NESTED:부모/자식'), true);
  assert.equal(isTemplateTableMarkerText(null), false);
  assert.equal(isTemplateTableMarkerText('평범한 표 제목'), false);
  assert.equal(isTemplateTableMarkerText(''), false);
  // 어휘 밖 '#...' — 첫 셀 텍스트가 우연히 #으로 시작하는 원본 내용은 게이트를 열지 않는다.
  assert.equal(isTemplateTableMarkerText('#BLOCK2'), false);
  assert.equal(isTemplateTableMarkerText('#HEADER2'), false);
  assert.equal(isTemplateTableMarkerText('#NOTE2'), false);
});

test('isRepeatBodyMarkerText: #REPEAT-BODY:/-NESTED:/-BOTTOM:만 참으로 판정한다', () => {
  assert.equal(isRepeatBodyMarkerText('#REPEAT-BODY:품목내역'), true);
  assert.equal(isRepeatBodyMarkerText('#REPEAT-BODY-NESTED:부모/자식'), true);
  assert.equal(isRepeatBodyMarkerText('#REPEAT-BODY-BOTTOM:품목내역'), true);
  assert.equal(isRepeatBodyMarkerText('#REPEAT-HEADER:품목내역'), false);
  assert.equal(isRepeatBodyMarkerText('#REPEAT-FOOTER:품목내역'), false);
  assert.equal(isRepeatBodyMarkerText(null), false);
});

test('isRepeatFooterMarkerText: #REPEAT-FOOTER:/-NESTED:만 참으로 판정한다(바닥고정 동의어 없음)', () => {
  assert.equal(isRepeatFooterMarkerText('#REPEAT-FOOTER:품목내역'), true);
  assert.equal(isRepeatFooterMarkerText('#REPEAT-FOOTER-NESTED:부모/자식'), true);
  assert.equal(isRepeatFooterMarkerText('#REPEAT-BODY:품목내역'), false);
  assert.equal(isRepeatFooterMarkerText(null), false);
});

test('suggestFieldNames(rowRange): 후보 대상 셀의 행이 범위 안일 때만 제안한다', () => {
  const wasm = makeFakeTableWasm(buildGovFormFixtureCells());
  // 기본(범위 생략)은 표 전체 — 기존 동작 그대로(단위 테스트 전체가 이 전제로 짜여 있다).
  assert.equal(suggestFieldNames(wasm, 0, 0, 0).length, 6);

  // 신청인 섹션만 (대상 셀 rows 0~1)
  const sin = suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 0, endRow: 1 } });
  assert.deepEqual(
    sin.map((s) => s.suggestedName),
    ['신청인_주소', '신청인_전화번호'],
  );

  // 법인명 섹션만 (대상 셀 rows 2~4)
  const corp = suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 2, endRow: 4 } });
  assert.deepEqual(
    corp.map((s) => s.suggestedName),
    ['법인명_명칭', '법인명_전화번호', '법인명_소재지'],
  );

  // 한 행만 (대상 row 3)
  const one = suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 3, endRow: 3 } });
  assert.deepEqual(
    one.map((s) => s.suggestedName),
    ['법인명_전화번호'],
  );

  // 표 밖 범위 — 대상이 없으면 제안 없음
  assert.deepEqual(suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 99, endRow: 99 } }), []);
});

test('suggestFieldNames(rowRange): 규칙 3(라벨 위/빈 행 아래)은 "채워질 빈 칸"이 있는 행을 기준으로 범위를 판정한다', () => {
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 14, text: '그 밖의 특이사항' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 14, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  // 라벨 행(0)만 선택 → 대상 빈 칸은 행 1이라 범위 밖 → 제안 없음. review list의
  // R 표시가 항상 선택된 행 안에 들어오는 게 이 범위 정의의 목적이다.
  assert.deepEqual(suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 0, endRow: 0 } }), []);
  // 빈 칸 행(1)만 선택 → 대상이 범위 안 → 제안한다(라벨 행이 범위 밖이어도).
  const fromBlank = suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 1, endRow: 1 } });
  assert.equal(fromBlank.length, 1);
  assert.equal(fromBlank[0].suggestedName, '그밖의특이사항');
});

test('마커 행 셀은 라벨 후보가 되지 않는다 — 태깅된 표의 마커 텍스트를 이름으로 제안하지 않는다', () => {
  // 태깅은 전체 폭 마커 행을 row 0에 삽입한다. 바로 아래 행이 같은 폭의 빈 행이면
  // 규칙 3(label-above-blank)의 모양과 기하학적으로 동일하다 — 마커 셀 제외
  // 없이는 "#HEADER" 자체가 라벨로 제안된다.
  const cells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 2, text: '#HEADER' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 2, text: '' },
  ];
  const wasm = makeFakeTableWasm(cells);
  assert.deepEqual(suggestFieldNames(wasm, 0, 0, 0), []);

  // '#REPEAT-BODY:품목내역'(공백 제거 17자)은 MAX_PLAUSIBLE_LABEL_LEN(20) 가드도
  // 통과하므로 마커 어휘 제외가 유일한 방어선이다. 아래 내용 행의 정상 후보는
  // 그대로 나와야 한다.
  const repeatCells: FakeCell[] = [
    { row: 0, col: 0, rowSpan: 1, colSpan: 2, text: '#REPEAT-BODY:품목내역' },
    { row: 1, col: 0, rowSpan: 1, colSpan: 2, text: '' },
    { row: 2, col: 0, rowSpan: 1, colSpan: 2, text: '비고' },
    { row: 3, col: 0, rowSpan: 1, colSpan: 2, text: '' },
  ];
  const wasm2 = makeFakeTableWasm(repeatCells);
  const suggestions = suggestFieldNames(wasm2, 0, 0, 0, { rowRange: { startRow: 1, endRow: 3 } });
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['비고'],
  );
});

test('태깅된 표 전체 흐름: #HEADER 마커 행 + 마커 행 다음부터 끝까지 선택 → 내용 6칸만 제안된다', () => {
  const wasm = makeFakeTableWasm(buildTaggedGovFormFixtureCells());
  // e2e(field-suggest-panel)의 실제 흐름과 같다 — 태깅 후 마커 행(row 0)을 제외한
  // 내용 행 전체를 선택하고 제안을 생성한다.
  const suggestions = suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 1, endRow: 6 } });
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['신청인_주소', '신청인_전화번호', '법인명_명칭', '법인명_전화번호', '법인명_소재지', '담당자'],
  );
  // 마커 행 시프트는 접두어 계산에 영향을 주지 않는다.
  const byLeaf = new Map(suggestions.map((s) => [s.leafText, s]));
  assert.equal(byLeaf.get('주소')?.sectionPrefix, '신청인');
  assert.equal(byLeaf.get('담당자')?.sectionPrefix, null);
  // 마커 셀 텍스트는 후보에 절대 등장하지 않는다.
  assert.ok(!suggestions.some((s) => s.leafText.startsWith('#')));
});

// ─── 규칙 5 (repeat-header-column-match): #REPEAT-BODY 표는 다른 표(직전
// #REPEAT-HEADER 표)의 텍스트를 봐야 하므로, table-marker-authoring.test.ts의
// 다중 표 fake 패턴을 참고해 표 여러 개를 (para, ci)로 구분하는 fake를 쓴다. ───

interface MultiFakeCell {
  row: number;
  col: number;
  rowSpan: number;
  colSpan: number;
  text: string;
}

interface MultiFakeTable {
  para: number;
  ci: number;
  cells: MultiFakeCell[];
}

function makeFakeMultiTableWasm(tables: MultiFakeTable[]) {
  const findTable = (para: number, ci: number): MultiFakeTable => {
    const t = tables.find((x) => x.para === para && x.ci === ci);
    if (!t) throw new Error(`no fake table at para=${para} ci=${ci}`);
    return t;
  };
  return {
    getParagraphCount: (_sec: number) => Math.max(0, ...tables.map((t) => t.para)) + 1,
    findNearestControlForward: (_sec: number, para: number, charOffset: number, inclusive = false) => {
      const candidates = tables.filter((t) => {
        if (t.para < para) return false;
        if (t.para > para) return true;
        return inclusive ? 0 >= charOffset : 0 > charOffset;
      });
      const next = candidates.sort((a, b) => a.para - b.para)[0];
      return next ? { type: 'table', sec: 0, para: next.para, ci: next.ci } : { type: 'none' };
    },
    getTableDimensions: (_sec: number, para: number, ci: number) => {
      const t = findTable(para, ci);
      return {
        rowCount: Math.max(0, ...t.cells.map((c) => c.row + c.rowSpan)),
        colCount: Math.max(0, ...t.cells.map((c) => c.col + c.colSpan)),
        cellCount: t.cells.length,
      };
    },
    getCellInfo: (_sec: number, para: number, ci: number, cellIdx: number) => {
      const { row, col, rowSpan, colSpan } = findTable(para, ci).cells[cellIdx];
      return { row, col, rowSpan, colSpan };
    },
    getCellParagraphCount: (_sec: number, _para: number, _ci: number, _cellIdx: number) => 1,
    getCellParagraphLength: (_sec: number, para: number, ci: number, cellIdx: number, _cpi: number) =>
      findTable(para, ci).cells[cellIdx].text.length,
    getTextInCell: (_sec: number, para: number, ci: number, cellIdx: number) => findTable(para, ci).cells[cellIdx].text,
    getCellProperties: (_sec: number, _para: number, _ci: number, _cellIdx: number) => ({ fieldName: undefined }),
    getFieldList: () => [],
    getFieldInfoAt: () => ({ inField: false }),
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
}

/** 5446234.hwp "변경 사항" 표 모양 — REPEAT-HEADER(para 1) 바로 뒤 REPEAT-BODY(para 2). */
function buildRepeatHeaderBodyTables(
  headerRow: MultiFakeCell[],
  bodyRow: MultiFakeCell[],
  headerMarker = '#REPEAT-HEADER:변경사항',
  bodyMarker = '#REPEAT-BODY:변경사항',
): MultiFakeTable[] {
  const headerColCount = Math.max(0, ...headerRow.map((c) => c.col + c.colSpan));
  const bodyColCount = Math.max(0, ...bodyRow.map((c) => c.col + c.colSpan));
  return [
    {
      para: 1,
      ci: 0,
      cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: headerColCount, text: headerMarker }, ...headerRow],
    },
    {
      para: 2,
      ci: 0,
      cells: [{ row: 0, col: 0, rowSpan: 1, colSpan: bodyColCount, text: bodyMarker }, ...bodyRow],
    },
  ];
}

const CHANGE_HEADER_ROW: MultiFakeCell[] = [
  { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '구분' },
  { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '변경내용' },
  { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '변경일' },
];
const CHANGE_BODY_ROW: MultiFakeCell[] = [
  { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '' },
  { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
  { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '' },
];

test('규칙 5: REPEAT-HEADER 열 텍스트로 REPEAT-BODY 빈 칸 이름을 제안한다(5446234.hwp "변경 사항")', () => {
  const wasm = makeFakeMultiTableWasm(buildRepeatHeaderBodyTables(CHANGE_HEADER_ROW, CHANGE_BODY_ROW));
  const suggestions = suggestFieldNames(wasm, 0, 2, 0, { rowRange: { startRow: 1, endRow: 1 } });
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['변경사항_구분', '변경사항_변경내용', '변경사항_변경일'],
  );
});

test('규칙 5: 열 개수가 다르면(구조 불일치) 후보를 만들지 않는다', () => {
  const bodyRow: MultiFakeCell[] = [
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
  ];
  const wasm = makeFakeMultiTableWasm(buildRepeatHeaderBodyTables(CHANGE_HEADER_ROW, bodyRow));
  const suggestions = suggestFieldNames(wasm, 0, 2, 0, { rowRange: { startRow: 1, endRow: 1 } });
  assert.deepEqual(suggestions, []);
});

test('규칙 5: 헤더 셀 텍스트가 비어 있는 열은 건너뛰고 나머지 열은 정상 제안한다', () => {
  const headerRow: MultiFakeCell[] = [
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '구분' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '변경일' },
  ];
  const wasm = makeFakeMultiTableWasm(buildRepeatHeaderBodyTables(headerRow, CHANGE_BODY_ROW));
  const suggestions = suggestFieldNames(wasm, 0, 2, 0, { rowRange: { startRow: 1, endRow: 1 } });
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['변경사항_구분', '변경사항_변경일'],
  );
});

test('규칙 5: 직전 표가 REPEAT-HEADER가 아니면(예: REPEAT-TITLE) 후보를 만들지 않는다', () => {
  const tables = buildRepeatHeaderBodyTables(CHANGE_HEADER_ROW, CHANGE_BODY_ROW, '#REPEAT-TITLE:변경사항');
  const wasm = makeFakeMultiTableWasm(tables);
  const suggestions = suggestFieldNames(wasm, 0, 2, 0, { rowRange: { startRow: 1, endRow: 1 } });
  assert.deepEqual(suggestions, []);
});

test('규칙 5 — seqno 특례: 헤더 첫 열이 "번호"로 시작하면 그 열만 #seqno:<segment>로 제안한다', () => {
  const headerRow: MultiFakeCell[] = [
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '번호' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: '변경내용' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '변경일' },
  ];
  const wasm = makeFakeMultiTableWasm(buildRepeatHeaderBodyTables(headerRow, CHANGE_BODY_ROW));
  const suggestions = suggestFieldNames(wasm, 0, 2, 0, { rowRange: { startRow: 1, endRow: 1 } });
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['#seqno:변경사항', '변경사항_변경내용', '변경사항_변경일'],
  );
});

test('규칙 5 — seqno 특례: 첫 열이 아닌 곳의 "No"로 시작하는 텍스트는 특례가 적용되지 않는다', () => {
  const headerRow: MultiFakeCell[] = [
    { row: 1, col: 0, rowSpan: 1, colSpan: 1, text: '구분' },
    { row: 1, col: 1, rowSpan: 1, colSpan: 1, text: 'Note' },
    { row: 1, col: 2, rowSpan: 1, colSpan: 1, text: '변경일' },
  ];
  const wasm = makeFakeMultiTableWasm(buildRepeatHeaderBodyTables(headerRow, CHANGE_BODY_ROW));
  const suggestions = suggestFieldNames(wasm, 0, 2, 0, { rowRange: { startRow: 1, endRow: 1 } });
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['변경사항_구분', '변경사항_Note', '변경사항_변경일'],
  );
});

test('태깅된 표 전체 흐름: #REPEAT-BODY 마커 표에서도 행 범위 제안이 동작한다(과거의 반복 표 제외 정책 폐지)', () => {
  // 반복 블록의 BODY 표 1행짜리 모양 — 라벨("품목명") 옆 빈 칸. 검색 범위가
  // 선택된 행이므로 "행마다 같은 라벨 반복" 모호성이 더 이상 없다.
  const cells = buildTaggedGovFormFixtureCells('#REPEAT-BODY:품목내역');
  const wasm = makeFakeTableWasm(cells);
  const suggestions = suggestFieldNames(wasm, 0, 0, 0, { rowRange: { startRow: 1, endRow: 2 } });
  assert.deepEqual(
    suggestions.map((s) => s.suggestedName),
    ['신청인_주소', '신청인_전화번호'],
  );
});
