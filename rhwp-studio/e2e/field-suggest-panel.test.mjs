// E2E: #template-panel 의 "누름틀 이름 제안" review list
//
// 5555817.hwpx(법인설립허가신청서) 모양을 재현한 합성 표(3열: 섹션 앵커 col0/
// 라벨 col1/빈 값 col2, "신청인"(rowSpan 2)·"법인명"(rowSpan 3) 두 섹션 +
// 섹션 밖 한 행)에서 "선택된 행에서 제안 생성"을 눌러 review list가 올바른
// 접두어 붙은 이름을 렌더링하는지, 체크 해제/이름 편집이 반영되는지, "적용" 후
// 실제 문서에 정확한 누름틀만 생기는지 검증한다(field-name-suggest.test.ts의
// 순수 로직 단위 테스트를 실제 UI+wasm 경로로 재확인 — #template-panel 첫 e2e).
//
// 제안 생성은 두 조건이 게이트다 — ① 역할 마커(#HEADER/#FOOTER/#PAGENO/#REPEAT-*)
// 가 지정된 표에서만, ② 검색 범위는 선택된 행(셀 선택 모드) 또는 커서 행만.
// 그래서 각 TC는 먼저 마커 없는 표에서 게이트 메시지가 나오는지 확인하고(TC-1b),
// 전체 행을 선택해 #HEADER 태깅한 뒤(TC-1c — 태그 지정은 선택 행만 표로 분리하므로
// 표 전체에 마커를 붙이려면 전체 범위 선택이 필요), 마커 행 다음 행부터 끝까지
// 다시 선택해 제안을 생성한다.
//
// 실행: CHROME_PATH=... node e2e/field-suggest-panel.test.mjs --mode=headless

import { runTest, createNewDocument, setTestCase, screenshot, assert } from './helpers.mjs';

const sleep = (page, ms) => page.evaluate((t) => new Promise((r) => setTimeout(r, t)), ms);

runTest('누름틀 이름 제안 review list — 생성/편집/적용', async ({ page }) => {
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

  // ── TC-1b: 마커 없는 표에서는 게이트 메시지 + 빈 목록 ────────
  setTestCase('TC-1b: 마커 게이트');
  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-generate-btn').click();
  });
  await sleep(page, 200);
  const gateState = await page.evaluate(() => ({
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
    rowCount: document.querySelectorAll('#template-panel .tp-fieldsuggest-row').length,
  }));
  assert(
    gateState.message.includes('역할 마커'),
    `마커 없는 표에서 게이트 메시지가 나온다: ${JSON.stringify(gateState.message)}`,
  );
  assert(gateState.rowCount === 0, '마커 없는 표에서는 review list가 비어 있다');
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
    // 태깅으로 전체 폭 마커 행이 row 0에 삽입됐다 — 제안 범위는 마커 행 다음
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

  // ── TC-2: "선택된 행에서 제안 생성" 클릭 → review list 렌더링 ──
  setTestCase('TC-2: 제안 생성');
  const generateBtnFound = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-generate-btn');
    if (!btn || btn.disabled) return false;
    btn.click();
    return true;
  });
  assert(generateBtnFound, '"선택된 행에서 제안 생성" 버튼이 활성 상태로 존재한다');
  await sleep(page, 200);

  const rows = await page.evaluate(() =>
    Array.from(document.querySelectorAll('#template-panel .tp-fieldsuggest-row')).map((row) => ({
      loc: row.querySelector('.tp-fieldsuggest-row-loc')?.textContent ?? '',
      checked: row.querySelector('input[type="checkbox"]')?.checked ?? false,
      name: row.querySelector('.tp-fieldsuggest-row-input')?.value ?? '',
    })),
  );
  console.log('review list:', JSON.stringify(rows));
  assert(rows.length === 6, `review list 6행 렌더링: ${rows.length}행`);
  assert(rows[0]?.name === '신청인_주소', `R1=신청인_주소: ${rows[0]?.name}`);
  assert(rows[1]?.name === '신청인_전화번호', `R2=신청인_전화번호: ${rows[1]?.name}`);
  assert(rows[2]?.name === '법인명_명칭', `R3=법인명_명칭: ${rows[2]?.name}`);
  assert(rows[3]?.name === '법인명_전화번호', `R4=법인명_전화번호: ${rows[3]?.name}`);
  assert(rows[4]?.name === '법인명_소재지', `R5=법인명_소재지: ${rows[4]?.name}`);
  assert(rows[5]?.name === '담당자', `R6(섹션 밖)=담당자(접두어 없음): ${rows[5]?.name}`);
  assert(rows.every((r) => r.checked), '기본적으로 모든 행이 체크되어 있다');
  await screenshot(page, 'field-suggest-02-review-list');

  // ── TC-3: 한 행 체크 해제 + 다른 행 이름 편집 ────────────────
  setTestCase('TC-3: 체크 해제 + 이름 편집');
  await page.evaluate(() => {
    const rowEls = Array.from(document.querySelectorAll('#template-panel .tp-fieldsuggest-row'));
    // R4(법인명_전화번호) 체크 해제 — 적용에서 제외되어야 한다.
    const checkbox = rowEls[3].querySelector('input[type="checkbox"]');
    checkbox.checked = false;
    checkbox.dispatchEvent(new Event('change', { bubbles: true }));
    // R1(신청인_주소) 이름을 사용자가 직접 수정.
    const nameInput = rowEls[0].querySelector('.tp-fieldsuggest-row-input');
    nameInput.value = '신청인_거주지';
    nameInput.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await sleep(page, 100);

  const applyBtnEnabled = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-apply-btn');
    return !!btn && !btn.disabled;
  });
  assert(applyBtnEnabled, '체크된 행이 남아 있으므로 "적용" 버튼이 활성화된다');

  // ── TC-4: 적용 → 실제 문서에 정확한 누름틀만 생성 ────────────
  setTestCase('TC-4: 적용 및 결과 확인');
  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-apply-btn').click();
  });
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-03-applied');

  const fields = await page.evaluate(() =>
    window.__wasm.getFieldList().map((f) => ({ name: f.name, guide: f.guide })),
  );
  const fieldNames = fields.map((f) => f.name).sort();
  console.log('적용 후 필드 목록:', JSON.stringify(fields));
  const expected = ['담당자', '법인명_명칭', '법인명_소재지', '신청인_거주지', '신청인_전화번호'].sort();
  assert(
    JSON.stringify(fieldNames) === JSON.stringify(expected),
    `적용 후 필드 5개(체크 해제한 법인명_전화번호 제외, 편집한 이름 반영): ${JSON.stringify(fieldNames)}`,
  );
  assert(!fieldNames.includes('법인명_전화번호'), '체크 해제한 행은 삽입되지 않는다');
  assert(
    fields.every((f) => f.guide === f.name),
    `안내문(guide)이 이름과 동기화된다(범용 "입력하세요" 대신): ${JSON.stringify(fields)}`,
  );

  const rowsAfterApply = await page.evaluate(
    () => document.querySelectorAll('#template-panel .tp-fieldsuggest-row').length,
  );
  assert(rowsAfterApply === 0, '적용 후 review list가 비워진다(다음 배치 오적용 방지)');

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

  setTestCase('TC-6: 제안 생성');
  const generateBtnFound2 = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-generate-btn');
    if (!btn || btn.disabled) return false;
    btn.click();
    return true;
  });
  assert(generateBtnFound2, '"선택된 행에서 제안 생성" 버튼이 활성 상태로 존재한다');
  await sleep(page, 200);

  const rows2 = await page.evaluate(() =>
    Array.from(document.querySelectorAll('#template-panel .tp-fieldsuggest-row')).map((row) => ({
      loc: row.querySelector('.tp-fieldsuggest-row-loc')?.textContent ?? '',
      checked: row.querySelector('input[type="checkbox"]')?.checked ?? false,
      name: row.querySelector('.tp-fieldsuggest-row-input')?.value ?? '',
    })),
  );
  console.log('review list:', JSON.stringify(rows2));
  assert(rows2.length === 3, `review list 3행 렌더링(성명/병적지청/그 밖의 특이사항): ${rows2.length}행`);
  const names = rows2.map((r) => r.name).sort();
  const expectedNames = ['인적사항_성명', '인적사항_병적지청', '그밖의특이사항'].sort();
  assert(
    JSON.stringify(names) === JSON.stringify(expectedNames),
    `review list 이름: ${JSON.stringify(names)} (기대: ${JSON.stringify(expectedNames)})`,
  );
  await screenshot(page, 'field-suggest-inline-02-review-list');

  setTestCase('TC-7: 적용 및 결과 확인');
  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-apply-btn').click();
  });
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-inline-03-applied');

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
    return { fields, cellTexts };
  }, { ppi: tbl2.ppi, ci: tbl2.ci });
  console.log('적용 후 필드 목록:', JSON.stringify(result.fields));
  console.log('적용 후 셀 텍스트:', JSON.stringify(result.cellTexts));

  const fieldNames2 = result.fields.map((f) => f.name).sort();
  const expectedFieldNames = ['인적사항_성명', '인적사항_병적지청', '그밖의특이사항'].sort();
  assert(
    JSON.stringify(fieldNames2) === JSON.stringify(expectedFieldNames),
    `적용 후 필드 3개: ${JSON.stringify(fieldNames2)}`,
  );
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

  const rowsAfterApply2 = await page.evaluate(
    () => document.querySelectorAll('#template-panel .tp-fieldsuggest-row').length,
  );
  assert(rowsAfterApply2 === 0, '적용 후 review list가 비워진다(다음 배치 오적용 방지)');
});
