// E2E: #template-panel의 "누름틀 만들기" — 텍스트 선택 시 그 텍스트로 즉시 삽입
//
// "신청인" 텍스트 뒤에 긴 공백, 그 뒤에 "(인)"이 오는 패턴(5555817.hwp 모양)은
// 표의 두 셀로 나뉘지 않고 본문 문단 또는 표 셀 하나 안에 통짜 텍스트로 들어있어
// field-name-suggest.ts의 "라벨 셀 + 인접 빈 셀" 자동 스캔이 잡아내지 못한다.
// 이 테스트는 "신청인"을 직접 드래그 선택 → "누름틀 만들기" 버튼 1클릭 →
// review list를 거치지 않고 그 자리에서 즉시 누름틀이 생기는지, 라벨은
// 그대로 두고 그 뒤에 삽입되는지를 본문 텍스트와 표 셀 내부 텍스트 양쪽에서 검증한다
// (field-suggest-panel.test.mjs의 셀-인접 스캔 경로는 후보가 여러 개일 수 있지만,
// 그쪽도 이제 review list 없이 한 클릭으로 즉시 전부 생성한다 — 같은 버튼, 같은
// 즉시-생성 모델. 이 테스트는 그중 "텍스트 선택" 트리거 경로만 검증한다).
//
// 실행: CHROME_PATH=... node e2e/field-suggest-selection.test.mjs --mode=headless

import { runTest, createNewDocument, setTestCase, screenshot, assert } from './helpers.mjs';

const sleep = (page, ms) => page.evaluate((t) => new Promise((r) => setTimeout(r, t)), ms);

runTest('누름틀 만들기 — 텍스트 선택 시 즉시 삽입', async ({ page }) => {
  // ── TC-1: 본문에 "신청인" + 공백 + "(인)" 패턴 입력 후 "신청인" 선택 → 1클릭 삽입 ──
  setTestCase('TC-1: 본문 텍스트 선택 → 즉시 삽입');
  await createNewDocument(page);

  await page.evaluate(() => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    wasm.insertText(0, 0, 0, '신청인          (인)');
    // "신청인"(0~3) 범위를 선택 — moveTo(시작) → setAnchor → moveTo(끝).
    ih.cursor.moveTo({ sectionIndex: 0, paragraphIndex: 0, charOffset: 0 });
    ih.cursor.setAnchor();
    ih.cursor.moveTo({ sectionIndex: 0, paragraphIndex: 0, charOffset: 3 });
    window.__eventBus?.emit('command-state-changed');
  });
  await sleep(page, 200);
  await screenshot(page, 'field-suggest-selection-01-selected');

  const btnFound = await page.evaluate(() => {
    const btn = document.querySelector('#template-panel .tp-fieldsuggest-btn');
    if (!btn || btn.disabled) return false;
    btn.click();
    return true;
  });
  assert(btnFound, '"누름틀 만들기" 버튼이 활성 상태로 존재한다');
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-selection-02-inserted');

  const result = await page.evaluate(() => ({
    fields: window.__wasm.getFieldList().map((f) => ({ name: f.name, guide: f.guide, startCharIdx: f.startCharIdx })),
    paraText: window.__wasm.getTextRange(0, 0, 0, 4),
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
  }));
  console.log('삽입 후:', JSON.stringify(result));
  assert(result.fields.length === 1, `필드 1개 즉시 생성(review list/적용 클릭 없이): ${result.fields.length}개`);
  assert(result.fields[0]?.name === '신청인', `필드 이름=신청인: ${result.fields[0]?.name}`);
  assert(result.fields[0]?.guide === '신청인', `안내문(guide)이 이름과 동기화된다: ${result.fields[0]?.guide}`);
  assert(
    result.paraText === '신청인 ',
    `라벨 텍스트 "신청인"이 삭제되지 않고 그대로 남고, 필드 앞에 구분자 스페이스가 들어간다: ${JSON.stringify(result.paraText)}`,
  );
  assert(
    result.fields[0]?.startCharIdx === 4,
    `필드가 라벨+구분자 스페이스 뒤(charOffset 4)에 삽입된다(선택 텍스트 치환 아님): ${result.fields[0]?.startCharIdx}`,
  );

  // ── TC-2: 같은 이름이 이미 필드로 존재하면 조용히 _2로 조정된다 ──
  setTestCase('TC-2: 이름 충돌 시 자동 접미어');
  await page.evaluate(() => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    // para 0 끝에서 문단을 나눠 para 1을 만들고 같은 패턴을 다시 넣는다.
    wasm.splitParagraph(0, 0, wasm.getParagraphLength(0, 0));
    wasm.insertText(0, 1, 0, '신청인          (인)');
    ih.cursor.moveTo({ sectionIndex: 0, paragraphIndex: 1, charOffset: 0 });
    ih.cursor.setAnchor();
    ih.cursor.moveTo({ sectionIndex: 0, paragraphIndex: 1, charOffset: 3 });
    window.__eventBus?.emit('command-state-changed');
  });
  await sleep(page, 200);
  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-btn').click();
  });
  await sleep(page, 300);

  const afterCollision = await page.evaluate(() => ({
    fields: window.__wasm.getFieldList().map((f) => f.name).sort(),
    message: document.querySelector('#template-panel .tp-fieldsuggest-message')?.textContent ?? '',
  }));
  console.log('충돌 후:', JSON.stringify(afterCollision));
  assert(
    JSON.stringify(afterCollision.fields) === JSON.stringify(['신청인', '신청인_2']),
    `두 번째 "신청인"은 자동으로 "신청인_2"가 된다(문서에 이미 있는 이름과 충돌 검사): ${JSON.stringify(afterCollision.fields)}`,
  );
  assert(
    afterCollision.message.includes('신청인_2'),
    `조정된 최종 이름이 메시지에 그대로 보인다: ${afterCollision.message}`,
  );

  // ── TC-3: 표 셀 내부에 같은 패턴 — 통짜 텍스트 선택도 동일하게 1클릭 동작 ──
  setTestCase('TC-3: 표 셀 내부 텍스트 선택 → 즉시 삽입');
  await createNewDocument(page);

  const tbl = await page.evaluate(() => {
    const ih = window.__inputHandler;
    const wasm = window.__wasm;
    let info = null;
    ih.executeOperation({
      kind: 'snapshot',
      operationType: 'createTable',
      operation: () => {
        const ret = wasm.createTable(0, 0, 0, 1, 1);
        info = typeof ret === 'string' ? JSON.parse(ret) : ret;
        return ih.getCursorPosition();
      },
    });
    const ppi = info.paraIdx;
    const ci = info.controlIdx;
    wasm.insertTextInCell(0, ppi, ci, 0, 0, 0, '신청인          (인)');

    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 0,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: 0, cellParaIndex: 0,
    });
    ih.cursor.setAnchor();
    ih.cursor.moveTo({
      sectionIndex: 0, paragraphIndex: 0, charOffset: 3,
      parentParaIndex: ppi, controlIndex: ci, cellIndex: 0, cellParaIndex: 0,
    });
    window.__eventBus?.emit('command-state-changed');
    return { ppi, ci };
  });
  assert(tbl.ci !== undefined, `표 생성 및 셀 내부 선택 완료 (ppi=${tbl.ppi}, ci=${tbl.ci})`);
  await sleep(page, 200);

  await page.evaluate(() => {
    document.querySelector('#template-panel .tp-fieldsuggest-btn').click();
  });
  await sleep(page, 300);
  await screenshot(page, 'field-suggest-selection-03-cell-inserted');

  const cellResult = await page.evaluate(({ ppi, ci }) => ({
    fields: window.__wasm.getFieldList().map((f) => ({ name: f.name, guide: f.guide, startCharIdx: f.startCharIdx })),
    cellText: window.__wasm.getTextInCell(0, ppi, ci, 0, 0, 0, 4),
  }), { ppi: tbl.ppi, ci: tbl.ci });
  assert(
    cellResult.fields.length === 1 && cellResult.fields[0]?.name === '신청인' && cellResult.fields[0]?.guide === '신청인',
    `표 셀 내부 선택도 1클릭으로 즉시 필드 생성: ${JSON.stringify(cellResult.fields)}`,
  );
  assert(
    cellResult.cellText === '신청인 ',
    `셀 안에서도 라벨이 보존되고 필드 앞에 구분자 스페이스가 들어간다: ${JSON.stringify(cellResult.cellText)}`,
  );
  assert(
    cellResult.fields[0]?.startCharIdx === 4,
    `셀 안 필드도 라벨+구분자 스페이스 뒤(charOffset 4)에 삽입된다: ${cellResult.fields[0]?.startCharIdx}`,
  );
});
