// E2E: #template-panel 의 "누름틀 이름 제안" review list
//
// 5555817.hwpx(법인설립허가신청서) 모양을 재현한 합성 표(3열: 섹션 앵커 col0/
// 라벨 col1/빈 값 col2, "신청인"(rowSpan 2)·"법인명"(rowSpan 3) 두 섹션 +
// 섹션 밖 한 행)에서 "현재 표에서 제안 생성"을 눌러 review list가 올바른
// 접두어 붙은 이름을 렌더링하는지, 체크 해제/이름 편집이 반영되는지, "적용" 후
// 실제 문서에 정확한 누름틀만 생기는지 검증한다(field-name-suggest.test.ts의
// 순수 로직 단위 테스트를 실제 UI+wasm 경로로 재확인 — #template-panel 첫 e2e).
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

  // ── TC-2: "현재 표에서 제안 생성" 클릭 → review list 렌더링 ──
  setTestCase('TC-2: 제안 생성');
  const generateBtnFound = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-generate-btn');
    if (!btn || btn.disabled) return false;
    btn.click();
    return true;
  });
  assert(generateBtnFound, '"현재 표에서 제안 생성" 버튼이 활성 상태로 존재한다');
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
});
