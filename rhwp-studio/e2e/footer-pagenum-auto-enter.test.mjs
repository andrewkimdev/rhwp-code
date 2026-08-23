/**
 * E2E 테스트 — 꼬리말 쪽번호/총쪽수/파일이름 필드 자동 진입 + 끝 위치 보정
 *
 * 회귀 배경: page:insert-field-pagenum 등 세 커맨드는 머리말/꼬리말 편집
 * 모드 밖에서는 insertHfField() 의 `!cursor.isInHeaderFooter()` 가드에 막혀
 * 조용히 no-op 이었다 — 숨겨진 꼬리말 도구모음으로 먼저 수동 진입해야만
 * 동작했다. 또한 HF 모드에서 End 로 커서를 옮기는 것도 실제로는 배선이
 * 안 되어 있어(Home/End 가 HF 키보드 블록의 무조건 return 에 막힘),
 * 빈 꼬리말에서만 우연히 동작해 보였다.
 *
 * 검증 항목:
 *   TC1. 빈 문서에서 수동 진입 없이 page:insert-field-pagenum 실행 →
 *        꼬리말이 생성되고 cursor.isInHeaderFooter()/headerFooterMode 가 올바름
 *   TC2. 이어서 두 번째 필드(총 쪽수) 삽입 → 처음으로 리셋되지 않고
 *        첫 필드 뒤에 이어 붙음(hfCharOffset 이 증가)
 *   TC3. 수동으로 꼬리말 진입 → 텍스트 입력 → End/Home 이 실제로 커서를 옮김
 *
 * 계획: 승인된 스펙 — 꼬리말 필드 삽입 자동 진입 + Home/End 배선 수정.
 */
import {
  runTest, createNewDocument, clickEditArea, typeText, screenshot, assert, setTestCase,
} from './helpers.mjs';

runTest('꼬리말 필드 삽입 자동 진입 + 끝 위치 보정 + Home/End 배선', async ({ page }) => {
  await createNewDocument(page);
  await clickEditArea(page);
  await page.evaluate(() => new Promise(r => setTimeout(r, 300)));

  // ── TC1: 수동 진입 없이 쪽 번호 삽입 → 꼬리말 자동 생성 + 진입 ──────────
  setTestCase('TC1: 수동 진입 없이 쪽 번호 삽입');
  const tc1 = await page.evaluate(() => {
    const a = window.rhwpStudio.automation;
    const ih = window.__inputHandler;
    const cursor = ih?.cursor;
    const beforeInHf = !!cursor?.isInHeaderFooter?.();

    const result = a.execute('page:insert-field-pagenum');

    const afterInHf = !!cursor?.isInHeaderFooter?.();
    const mode = cursor?.headerFooterMode ?? null;
    const hf = window.__wasm?.getHeaderFooter?.(
      cursor?.hfSectionIdx ?? 0, false, cursor?.hfApplyTo ?? 0,
    );
    const exists = hf ? JSON.parse(hf).exists : null;

    return {
      result, beforeInHf, afterInHf, mode, exists,
      hfParaIdx: cursor?.hfParaIdx, hfCharOffset: cursor?.hfCharOffset,
    };
  });
  console.log('TC1:', JSON.stringify(tc1));
  assert(tc1.beforeInHf === false, 'TC1: 실행 전에는 HF 모드가 아니었다');
  assert(tc1.result?.ok === true, `TC1: execute 판정 ok (${JSON.stringify(tc1.result)})`);
  assert(tc1.afterInHf === true, 'TC1: 실행 후 자동으로 HF(꼬리말) 모드에 진입했다');
  assert(tc1.mode === 'footer', `TC1: headerFooterMode === 'footer' (실제: ${tc1.mode})`);
  assert(tc1.exists === true, 'TC1: 꼬리말 컨트롤이 실제로 생성됨');
  assert(typeof tc1.hfCharOffset === 'number' && tc1.hfCharOffset > 0,
    `TC1: 필드 삽입 후 커서가 마커 뒤로 이동함 (hfCharOffset=${tc1.hfCharOffset})`);
  await screenshot(page, 'footer-pagenum-01-auto-enter');

  // ── TC2: 이어서 총 쪽수 삽입 → 처음으로 리셋되지 않고 이어 붙는다 ──────
  setTestCase('TC2: 연속 삽입은 리셋 없이 이어 붙는다');
  const tc2 = await page.evaluate(() => {
    const a = window.rhwpStudio.automation;
    const cursor = window.__inputHandler?.cursor;
    const offsetBefore = cursor?.hfCharOffset ?? -1;
    const paraBefore = cursor?.hfParaIdx ?? -1;

    const result = a.execute('page:insert-field-totalpage');

    return {
      result,
      offsetBefore,
      paraBefore,
      offsetAfter: cursor?.hfCharOffset,
      paraAfter: cursor?.hfParaIdx,
      stillInHf: !!cursor?.isInHeaderFooter?.(),
    };
  });
  console.log('TC2:', JSON.stringify(tc2));
  assert(tc2.result?.ok === true, `TC2: execute 판정 ok (${JSON.stringify(tc2.result)})`);
  assert(tc2.stillInHf === true, 'TC2: 여전히 HF 모드');
  assert(tc2.paraAfter === tc2.paraBefore, 'TC2: 같은 문단에 이어 붙음(문단이 초기화되지 않음)');
  assert(tc2.offsetAfter > tc2.offsetBefore,
    `TC2: 두 번째 필드가 첫 필드 뒤에 이어 붙음 (offset ${tc2.offsetBefore} → ${tc2.offsetAfter})`);
  await screenshot(page, 'footer-pagenum-02-append');

  // ── TC3: 수동 진입 + 타이핑 후 End/Home 이 실제로 커서를 옮긴다 ────────
  setTestCase('TC3: HF 모드에서 End/Home 배선');
  // 새 문서로 초기화해 꼬리말 내용을 깨끗한 상태에서 검증한다.
  await createNewDocument(page);
  await clickEditArea(page);
  await page.evaluate(() => new Promise(r => setTimeout(r, 300)));

  const enter = await page.evaluate(() => window.rhwpStudio.automation.execute('page:footer-create'));
  assert(enter.ok === true, `TC3: page:footer-create 실행 (${JSON.stringify(enter)})`);
  await page.evaluate(() => new Promise(r => setTimeout(r, 200)));

  const typed = 'HOMEEND';
  await typeText(page, typed);
  const afterType = await page.evaluate(() => {
    const c = window.__inputHandler?.cursor;
    return { hfParaIdx: c?.hfParaIdx, hfCharOffset: c?.hfCharOffset, inHf: !!c?.isInHeaderFooter?.() };
  });
  console.log('TC3 afterType:', JSON.stringify(afterType));
  assert(afterType.inHf === true, 'TC3: 타이핑 후에도 HF 모드 유지');
  assert(afterType.hfCharOffset === typed.length,
    `TC3: 타이핑 직후 커서가 문자열 끝 (offset=${afterType.hfCharOffset}, 기대 ${typed.length})`);

  // Home → 0으로 이동
  await page.keyboard.press('Home');
  await page.evaluate(() => new Promise(r => setTimeout(r, 150)));
  const afterHome = await page.evaluate(() => window.__inputHandler?.cursor?.hfCharOffset);
  assert(afterHome === 0, `TC3: Home 이 커서를 문단 처음으로 옮김 (실제: ${afterHome})`);

  // End → 문자열 끝으로 이동
  await page.keyboard.press('End');
  await page.evaluate(() => new Promise(r => setTimeout(r, 150)));
  const afterEnd = await page.evaluate(() => window.__inputHandler?.cursor?.hfCharOffset);
  assert(afterEnd === typed.length,
    `TC3: End 가 커서를 문단 끝으로 옮김 (실제: ${afterEnd}, 기대 ${typed.length}) — 회귀 시 End 가 아무 것도 하지 않아 값이 그대로였다`);

  await screenshot(page, 'footer-pagenum-03-home-end');
});
