// E2E: #template-panel 의 "누름틀 만들기" — 행 선택 시 즉시 다건 생성
//
// 5555817.hwpx(법인설립허가신청서) 모양을 재현한 합성 표(3열: 섹션 앵커 col0/
// 라벨 col1/빈 값 col2, "신청인"(rowSpan 2)·"법인명"(rowSpan 3) 두 섹션 +
// 섹션 밖 한 행)에서 "누름틀 만들기"를 눌러 review list 없이 클릭 한 번으로
// 올바른 접두어 붙은 이름의 누름틀이 즉시 전부 생성되는지, 이미 필드가 있는
// 후보는 건너뛰고 메시지로 보고하는지 검증한다(field-name-suggest.test.ts의
// 순수 로직 단위 테스트를 실제 UI+wasm 경로로 재확인 — #template-panel 첫 e2e).
//
// 생성은 두 조건이 게이트다 — ① 역할 마커(#HEADER/#FOOTER/#PAGENO/#REPEAT-*)
// 가 지정된 표에서만, ② 검색 범위는 선택된 행(셀 선택 모드) 또는 커서 행만.
// 그래서 각 TC는 먼저 마커 없는 표에서 게이트 메시지가 나오는지 확인하고(TC-1b),
// 전체 행을 선택해 #HEADER 태깅한 뒤(TC-1c — 태그 지정은 선택 행만 표로 분리하므로
// 표 전체에 마커를 붙이려면 전체 범위 선택이 필요), 마커 행 다음 행부터 끝까지
// 다시 선택해 누름틀을 만든다.
//
// 클릭 1회 = 즉시 생성이므로(review list/"적용" 단계 없음) 개별 후보를 체크
// 해제하거나 이름을 편집하는 UX는 더 이상 없다 — 그 대신, 같은 행 선택으로
// 버튼을 두 번 누르면 두 번째는 모든 후보가 "이미 필드가 있음"으로 건너뛰어져야
// 한다는 것으로 skip 경로를 검증한다(TC-2b/TC-7).
//
// 실행: CHROME_PATH=... node e2e/field-suggest-panel.test.mjs --mode=headless

import { runTest, createNewDocument, setTestCase, screenshot, assert } from './helpers.mjs';

const sleep = (page, ms) => page.evaluate((t) => new Promise((r) => setTimeout(r, t)), ms);

runTest('누름틀 만들기 — 행 선택 시 즉시 생성/skip', async ({ page }) => {
  // ── TC-1: 신청인/법인명 모양의 표 생성 ──────────────────────
  setTestCase('TC-1: 합성 표 생성');
  await createNewDocument(page);

  const tbl = await page.evaluate(() => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    let info = null;
    ih.executeOperation({
      kind: 'snapshot',
      operationType: 'createTable',
      operation: () => {
        const ret = wasm.createTable(0, 0, 0, 6, 3);
        info = typeof ret === 'string' ? JSON.parse(ret) : ret;
        return ih.getCursorPosition();
      },
    });
    const ppi = info.paraIdx;
    const ci = info.controlIdx;

    // 신청인(row0-1, col0) / 법인명(row2-4, col0) 섹션 앵커 병합.
    wasm.mergeTableCells(0, ppi, ci, 0, 0, 1, 0);
    wasm.mergeTableCells(0, ppi, ci, 2, 0, 4, 0);

    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };

    const labels = [
      [0, 0, '신청인'], [0, 1, '주소'],
      [1, 1, '전화번호'],
      [2, 0, '법인명'], [2, 1, '명칭'],
      [3, 1, '전화번호'],
      [4, 1, '소재지'],
      [5, 1, '담당자'],
    ];
    for (const [row, col, text] of labels) {
      const cellIdx = findCellIdx(row, col);
      wasm.insertTextInCell(0, ppi, ci, cellIdx, 0, 0, text);
    }

    // 커서를 표 안(빈 값 칸)으로 이동 — 패널이 "현재 표"로 인식하게 한다.
    const blankCellIdx = findCellIdx(0, 2);
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: blankCellIdx,
    });
    window.__eventBus?.emit('command-state-changed');

    return { ppi, ci };
  });
  assert(tbl.ci !== undefined, `표 생성 및 커서 진입 완료 (ppi=${tbl.ppi}, ci=${tbl.ci})`);
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-01-table-ready');

  // ── TC-1b: 마커 없는 표에서는 게이트 메시지만, 필드는 생성되지 않는다 ──
  setTestCase('TC-1b: 마커 게이트');
  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-btn').click();
  });
  await sleep(page, 200);
  const gateState = await page.evaluate(() => ({
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
    fieldCount: window.__wasm.getFieldList().length,
  }));
  assert(
    gateState.message.includes('역할 마커'),
    `마커 없는 표에서 게이트 메시지가 나온다: ${JSON.stringify(gateState.message)}`,
  );
  assert(gateState.fieldCount === 0, '마커 없는 표에서는 필드가 생성되지 않는다');
  await screenshot(page, 'field-suggest-01b-gated');

  // ── TC-1c: 전체 행 선택 → #HEADER 태깅 → 내용 행 재선택 ──────
  setTestCase('TC-1c: #HEADER 태깅');
  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    // 태그 지정(template:tag-selection)은 선택된 행만 표로 분리해 마커를 붙인다 —
    // 표 전체에 마커를 붙이려면 전체 행을 선택한 상태로 태깅해야 한다.
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(5, 0); // 6행 전체 (rows 0~5)
    window.__eventBus?.emit('command-state-changed');
    document.querySelector('#template-panel .tp-actions .tp-btn--primary').click(); // "태그 지정" (기본 역할 #HEADER)
  }, { ppi: tbl.ppi, ci: tbl.ci });
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-01c-tagged');

  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    // 태깅으로 전체 폭 마커 행이 row 0에 삽입됐다 — 생성 범위는 마커 행 다음
    // 행(row 1)부터 끝(row 6)까지 다시 선택한다.
    ih.cursor.exitCellSelectionMode();
    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(1, 2),
    });
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(5, 0); // rows 1~6 (마커 행 제외한 내용 전체)
    window.__eventBus?.emit('command-state-changed');
  }, { ppi: tbl.ppi, ci: tbl.ci });
  await sleep(page, 200);

  const taggedMarker = await page.evaluate(({ ppi, ci }) => {
    const wasm = window.__wasm;
    const dims = wasm.getTableDimensions(0, ppi, ci);
    const len = wasm.getCellParagraphLength(0, ppi, ci, 0, 0);
    return {
      rowCount: dims.rowCount,
      marker: len > 0 ? wasm.getTextInCell(0, ppi, ci, 0, 0, 0, len).trim() : '',
      hint: document.querySelector('#template-panel .tp-hint')?.textContent ?? '',
    };
  }, { ppi: tbl.ppi, ci: tbl.ci });
  assert(taggedMarker.rowCount === 7, `마커 행 삽입으로 7행: ${taggedMarker.rowCount}행`);
  assert(taggedMarker.marker === '#HEADER', `첫 셀에 #HEADER 마커: ${taggedMarker.marker}`);
  assert(
    taggedMarker.hint.includes('2~7'),
    `hint가 선택된 행(2~7)을 표시한다: ${taggedMarker.hint}`,
  );

  // ── TC-2: "누름틀 만들기" 클릭 → 후보 6개가 review list 없이 즉시 생성 ──
  setTestCase('TC-2: 즉시 생성');
  const createBtnFound = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-btn');
    if (!btn || btn.disabled) return false;
    btn.click();
    return true;
  });
  assert(createBtnFound, '"누름틀 만들기" 버튼이 활성 상태로 존재한다');
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-02-created');

  const afterCreate = await page.evaluate(() => ({
    fields: window.__wasm.getFieldList().map((f) => ({ name: f.name, guide: f.guide })),
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
  }));
  console.log('생성 후:', JSON.stringify(afterCreate));
  const fieldNames = afterCreate.fields.map((f) => f.name).sort();
  const expected = ['신청인_주소', '신청인_전화번호', '법인명_명칭', '법인명_전화번호', '법인명_소재지', '담당자'].sort();
  assert(
    JSON.stringify(fieldNames) === JSON.stringify(expected),
    `클릭 한 번으로 후보 6개가 전부 생성된다: ${JSON.stringify(fieldNames)} (기대: ${JSON.stringify(expected)})`,
  );
  assert(
    afterCreate.fields.every((f) => f.guide === f.name),
    `안내문(guide)이 이름과 동기화된다: ${JSON.stringify(afterCreate.fields)}`,
  );
  assert(
    afterCreate.message.includes('6개'),
    `메시지가 생성 개수(6개)를 보고한다: ${afterCreate.message}`,
  );

  // ── TC-2b: 같은 행을 다시 선택해 한 번 더 클릭 → 전부 "이미 필드가 있음"으로 skip ──
  setTestCase('TC-2b: 재클릭 시 전량 skip');
  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(1, 2),
    });
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(5, 0); // rows 1~6, TC-2와 동일한 범위
    window.__eventBus?.emit('command-state-changed');
  }, { ppi: tbl.ppi, ci: tbl.ci });
  await sleep(page, 200);
  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-btn').click();
  });
  await sleep(page, 300);

  const afterRetry = await page.evaluate(() => ({
    fieldCount: window.__wasm.getFieldList().length,
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
  }));
  console.log('재클릭 후:', JSON.stringify(afterRetry));
  assert(afterRetry.fieldCount === 6, `중복 생성 없이 필드 6개 그대로 유지된다: ${afterRetry.fieldCount}개`);
  assert(
    afterRetry.message.includes('이미 필드가 있') && afterRetry.message.includes('6'),
    `이미 태그된 6개를 모두 건너뛰었다는 메시지가 나온다: ${afterRetry.message}`,
  );

  // ── TC-5: 인적사항/그 밖의 특이사항 모양의 표 생성(새 문서) ────
  // 17856415.hwp(의무경찰 지원서) row5-8(인적사항 섹션)과 row9-10(그 밖의 특이사항)
  // 모양을 압축 재현한 2열 4행 합성 표에서 labelAboveBlankRule/labelInlineRoomRule을
  // 실제 wasm 경로로 검증한다. 위 fake wasm 단위 테스트(field-name-suggest.test.ts)는
  // 순수 로직만 확인하므로, 인라인 삽입(insertAt → 'selection' kind)이 실제
  // `insertClickHereFieldInCell`을 통해 라벨 텍스트를 보존한 채 그 뒤에 정확히
  // 삽입되는지는 이 TC만이 검증할 수 있다.
  setTestCase('TC-5: 인적사항/그 밖의 특이사항 모양의 표 생성');
  await createNewDocument(page);

  const tbl2 = await page.evaluate(() => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    let info = null;
    ih.executeOperation({
      kind: 'snapshot',
      operationType: 'createTable',
      operation: () => {
        const ret = wasm.createTable(0, 0, 0, 4, 2);
        info = typeof ret === 'string' ? JSON.parse(ret) : ret;
        return ih.getCursorPosition();
      },
    });
    const ppi = info.paraIdx;
    const ci = info.controlIdx;

    // row0-1 col0 = "인적사항" 섹션 앵커(rowSpan2).
    wasm.mergeTableCells(0, ppi, ci, 0, 0, 1, 0);
    // row2, row3은 각각 두 칸을 합쳐 전체 폭 라벨/빈 행으로 만든다(그 밖의 특이사항 모양).
    wasm.mergeTableCells(0, ppi, ci, 2, 0, 2, 1);
    wasm.mergeTableCells(0, ppi, ci, 3, 0, 3, 1);

    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };

    const labels = [
      [0, 0, '인적사항'], [0, 1, '성명'],
      [1, 1, '병적지청'],
      [2, 0, '그 밖의 특이사항'],
      // row3은 빈 채로 둔다(labelAboveBlankRule의 대상).
    ];
    for (const [row, col, text] of labels) {
      const cellIdx = findCellIdx(row, col);
      wasm.insertTextInCell(0, ppi, ci, cellIdx, 0, 0, text);
    }

    const anchorCellIdx = findCellIdx(0, 0);
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: anchorCellIdx,
    });
    window.__eventBus?.emit('command-state-changed');

    return { ppi, ci };
  });
  assert(tbl2.ci !== undefined, `표 생성 및 커서 진입 완료 (ppi=${tbl2.ppi}, ci=${tbl2.ci})`);
  await sleep(page, 300);

  // TC-1과 같은 게이트 흐름 — 전체 행(4행) 선택 → #HEADER 태깅 → 마커 행 다음
  // (row 1)부터 끝(row 4)까지 재선택.
  setTestCase('TC-5b: #HEADER 태깅');
  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(3, 1); // 4행 2열 전체 (rows 0~3)
    window.__eventBus?.emit('command-state-changed');
    document.querySelector('#template-panel .tp-actions .tp-btn--primary').click();
  }, { ppi: tbl2.ppi, ci: tbl2.ci });
  await sleep(page, 300);

  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    ih.cursor.exitCellSelectionMode();
    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(1, 1), // "성명" 행
    });
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(3, 0); // rows 1~4 (마커 행 제외)
    window.__eventBus?.emit('command-state-changed');
  }, { ppi: tbl2.ppi, ci: tbl2.ci });
  await sleep(page, 200);
  await screenshot(page, 'field-suggest-inline-01-tagged');

  setTestCase('TC-6: 즉시 생성(인라인 삽입 포함)');
  const createBtnFound2 = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-btn');
    if (!btn || btn.disabled) return false;
    btn.click();
    return true;
  });
  assert(createBtnFound2, '"누름틀 만들기" 버튼이 활성 상태로 존재한다');
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-inline-02-created');

  const result = await page.evaluate(({ ppi, ci }) => {
    const wasm = window.__wasm;
    const fields = wasm.getFieldList().map((f) => ({ name: f.name, guide: f.guide }));
    const dims = wasm.getTableDimensions(0, ppi, ci);
    const cellTexts = [];
    for (let idx = 0; idx < dims.cellCount; idx++) {
      const paraCount = wasm.getCellParagraphCount(0, ppi, ci, idx);
      let text = '';
      for (let cpi = 0; cpi < paraCount; cpi++) {
        const len = wasm.getCellParagraphLength(0, ppi, ci, idx, cpi);
        if (len > 0) text += wasm.getTextInCell(0, ppi, ci, idx, cpi, 0, len);
      }
      cellTexts.push(text);
    }
    return {
      fields,
      cellTexts,
      message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
    };
  }, { ppi: tbl2.ppi, ci: tbl2.ci });
  console.log('생성 후 필드 목록:', JSON.stringify(result.fields));
  console.log('생성 후 셀 텍스트:', JSON.stringify(result.cellTexts));

  const fieldNames2 = result.fields.map((f) => f.name).sort();
  const expectedFieldNames = ['인적사항_성명', '인적사항_병적지청', '그밖의특이사항'].sort();
  assert(
    JSON.stringify(fieldNames2) === JSON.stringify(expectedFieldNames),
    `클릭 한 번으로 필드 3개(인라인 삽입 포함)가 즉시 생성된다: ${JSON.stringify(fieldNames2)}`,
  );
  assert(result.message.includes('3개'), `메시지가 생성 개수(3개)를 보고한다: ${result.message}`);
  // 인라인 삽입(성명/병적지청)은 라벨 텍스트를 지우지 않고 그 뒤에 필드를 붙인다 —
  // 셀 텍스트가 라벨 문자열로 "시작"해야 한다(필드 자체는 getTextInCell 평문에
  // 나타나지 않을 수 있으므로 완전 일치가 아니라 startsWith로 확인한다).
  assert(
    result.cellTexts.some((t) => t.startsWith('성명')),
    `"성명" 셀 텍스트가 라벨로 시작한다(라벨 보존): ${JSON.stringify(result.cellTexts)}`,
  );
  assert(
    result.cellTexts.some((t) => t.startsWith('병적지청')),
    `"병적지청" 셀 텍스트가 라벨로 시작한다(라벨 보존): ${JSON.stringify(result.cellTexts)}`,
  );
  // label-above-blank(그 밖의 특이사항)는 빈 셀 채우기이므로 그 라벨 자신의 셀
  // 텍스트는 그대로, "빈 행"이었던 셀에 필드가 들어간다 — 라벨 셀 텍스트는 안 바뀐다.
  assert(
    result.cellTexts.some((t) => t.startsWith('그 밖의 특이사항') || t === '그 밖의 특이사항'),
    `"그 밖의 특이사항" 라벨 셀 텍스트가 보존된다: ${JSON.stringify(result.cellTexts)}`,
  );

  // ── TC-7: 같은 행을 다시 선택해 한 번 더 클릭 → 인라인 삽입 후보도 전량 skip ──
  setTestCase('TC-7: 재클릭 시 전량 skip(인라인 삽입 포함)');
  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(1, 1),
    });
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(3, 0); // rows 1~4, TC-6과 동일한 범위
    window.__eventBus?.emit('command-state-changed');
  }, { ppi: tbl2.ppi, ci: tbl2.ci });
  await sleep(page, 200);
  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-btn').click();
  });
  await sleep(page, 300);

  const afterRetry2 = await page.evaluate(() => ({
    fieldCount: window.__wasm.getFieldList().length,
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
  }));
  console.log('재클릭 후:', JSON.stringify(afterRetry2));
  assert(afterRetry2.fieldCount === 3, `중복 생성 없이 필드 3개 그대로 유지된다: ${afterRetry2.fieldCount}개`);
  assert(
    afterRetry2.message.includes('이미 필드가 있') && afterRetry2.message.includes('3'),
    `이미 태그된 3개(인라인 삽입 포함)를 모두 건너뛰었다는 메시지가 나온다: ${afterRetry2.message}`,
  );

  // ── TC-8: 규칙 5(repeat-header-column-match) — 5446234.hwp "변경 사항" 모양 ──
  // 2행 3열 표(row0="구분"/"변경내용"/"변경일", row1=빈칸 3개)를 만든 뒤 row0을
  // REPEAT_HEADER, row1을 REPEAT_BODY(같은 블록명 "변경사항")로 각각 태깅한다.
  // `template:tag-selection`은 태깅한 범위를 `splitTable`로 별도 최상위 표로
  // 떼어내므로(template.ts의 tagSelectionOperation), 이 시점에 문서에는 실제로
  // 서로 다른 두 표(REPEAT-HEADER 표, REPEAT-BODY 표)가 인접해 존재한다 —
  // field-name-suggest.test.ts의 fake wasm 단위 테스트는 이 표-간 관계를 손으로
  // 구성하지만, 이 TC는 실제 태깅 커맨드가 만들어낸 표 배치로 규칙 5가 wasm 경로
  // 전체를 통해 정상 동작하는지 검증한다.
  setTestCase('TC-8: 규칙 5 — REPEAT-HEADER/REPEAT-BODY 열 매칭');
  await createNewDocument(page);

  const tbl3 = await page.evaluate(() => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    let info = null;
    ih.executeOperation({
      kind: 'snapshot',
      operationType: 'createTable',
      operation: () => {
        const ret = wasm.createTable(0, 0, 0, 2, 3);
        info = typeof ret === 'string' ? JSON.parse(ret) : ret;
        return ih.getCursorPosition();
      },
    });
    const ppi = info.paraIdx;
    const ci = info.controlIdx;

    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };

    // row0 = 헤더 라벨, row1은 빈 채로 둔다(REPEAT-BODY 콘텐츠).
    [[0, 0, '구분'], [0, 1, '변경내용'], [0, 2, '변경일']].forEach(([row, col, text]) => {
      wasm.insertTextInCell(0, ppi, ci, findCellIdx(row, col), 0, 0, text);
    });

    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(0, 0),
    });
    window.__eventBus?.emit('command-state-changed');
    return { ppi, ci };
  });
  assert(tbl3.ci !== undefined, `표 생성 완료 (ppi=${tbl3.ppi}, ci=${tbl3.ci})`);
  await sleep(page, 200);

  // row0(헤더 라벨 행)만 선택 → REPEAT_HEADER:변경사항으로 태깅.
  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(0, 0),
    });
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(0, 2); // row0만, col0~2
    window.__eventBus?.emit('command-state-changed');
    document.querySelector('#template-panel input[name="tp-role"][value="REPEAT_HEADER"]').click();
    const blockNameInput = document.querySelector('#template-panel .tp-input');
    blockNameInput.value = '변경사항';
    blockNameInput.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('#template-panel .tp-actions .tp-btn--primary').click();
  }, { ppi: tbl3.ppi, ci: tbl3.ci });
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-08-header-tagged');

  // 태깅으로 원래 표가 (헤더 표, 나머지 표)로 splitTable됐다 — 문서의 최상위
  // 표를 순서대로 훑어 "나머지" 표(원래 row1, 아직 태깅 안 됨)의 (para, ci)를 찾는다
  // (table-outline.ts의 listTopLevelTables와 동일한 순회 로직).
  const bodyLoc = await page.evaluate(() => {
    const wasm = window.__wasm;
    const paraCount = wasm.getParagraphCount(0);
    const tables = [];
    let searchPara = 0;
    let searchOffset = 0;
    while (searchPara < paraCount) {
      const result = wasm.findNearestControlForward(0, searchPara, searchOffset, true);
      if (!result || result.type === 'none') break;
      if (result.type === 'table') tables.push({ para: result.para, ci: result.ci });
      if (result.para < searchPara) break;
      searchPara = result.para + 1;
      searchOffset = 0;
    }
    return tables;
  });
  assert(bodyLoc.length === 2, `태깅 후 최상위 표가 2개(헤더/나머지)여야 한다: ${JSON.stringify(bodyLoc)}`);
  const bodyPpi = bodyLoc[1].para;
  const bodyCi = bodyLoc[1].ci;

  // 나머지 표(원래 row1, 지금은 그 표의 유일한 행 row0)를 REPEAT_BODY:변경사항으로 태깅.
  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };
    ih.cursor.exitCellSelectionMode();
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(0, 0),
    });
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(0, 2); // 이 표의 유일한 행 전체
    window.__eventBus?.emit('command-state-changed');
    document.querySelector('#template-panel input[name="tp-role"][value="REPEAT_BODY"]').click();
    const blockNameInput = document.querySelector('#template-panel .tp-input');
    blockNameInput.value = '변경사항';
    blockNameInput.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('#template-panel .tp-actions .tp-btn--primary').click();
  }, { ppi: bodyPpi, ci: bodyCi });
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-08-body-tagged');

  // 마커 행(row0) 다음 콘텐츠 행(row1)만 선택 → "누름틀 만들기".
  await page.evaluate(({ ppi, ci }) => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    const findCellIdx = (row, col) => {
      const dims = wasm.getTableDimensions(0, ppi, ci);
      for (let idx = 0; idx < dims.cellCount; idx++) {
        const c = wasm.getCellInfo(0, ppi, ci, idx);
        if (row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan) return idx;
      }
      return -1;
    };
    ih.cursor.exitCellSelectionMode();
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: findCellIdx(1, 0),
    });
    ih.cursor.enterCellSelectionMode();
    ih.cursor.expandCellSelection(1, 2); // 콘텐츠 행(row1)만, col0~2
    window.__eventBus?.emit('command-state-changed');
  }, { ppi: bodyPpi, ci: bodyCi });
  await sleep(page, 200);

  const createBtnFound3 = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-btn');
    if (!btn || btn.disabled) return false;
    btn.click();
    return true;
  });
  assert(createBtnFound3, '"누름틀 만들기" 버튼이 활성 상태로 존재한다');
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-08-created');

  const afterRule5 = await page.evaluate(() => ({
    fields: window.__wasm.getFieldList().map((f) => f.name).sort(),
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
  }));
  console.log('규칙 5 생성 후:', JSON.stringify(afterRule5));
  const expectedRule5 = ['변경사항_구분', '변경사항_변경내용', '변경사항_변경일'].sort();
  assert(
    JSON.stringify(afterRule5.fields) === JSON.stringify(expectedRule5),
    `REPEAT-HEADER 열 텍스트로 REPEAT-BODY 빈 칸 3개가 제안·생성된다: ${JSON.stringify(afterRule5.fields)} (기대: ${JSON.stringify(expectedRule5)})`,
  );
  assert(afterRule5.message.includes('3개'), `메시지가 생성 개수(3개)를 보고한다: ${afterRule5.message}`);
});
