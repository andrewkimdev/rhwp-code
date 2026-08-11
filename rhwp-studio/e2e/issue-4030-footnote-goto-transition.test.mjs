/**
 * #4030: 각주 편집 중에도 대형 문서의 찾아가기가 본문 모드로 전환되어야 한다.
 */
import { runTest, loadHwpFile, assert } from './helpers.mjs';

const SAMPLE = '정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwp';
const FOOTNOTE = { sectionIndex: 0, paragraphIndex: 216, controlIndex: 0, footnoteIndex: 0 };
const TARGET_PAGE = 200;

async function waitForUi(page) {
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 300)));
}

async function enterKnownFootnote(page) {
  const entered = await page.evaluate((target) => {
    const handler = window.__inputHandler;
    const wasm = window.__wasm;
    if (!handler || !wasm) throw new Error('input handler 또는 wasm을 찾을 수 없습니다');

    const info = wasm.getFootnoteInfo(target.sectionIndex, target.paragraphIndex, target.controlIndex);
    if (!info?.ok || info.number !== 1) throw new Error('대형 HWP의 각주 1번 fixture를 찾을 수 없습니다');
    const pageResult = wasm.getPageOfPosition(target.sectionIndex, target.paragraphIndex);
    if (!pageResult?.ok || pageResult.page == null) throw new Error('각주 anchor의 본문 쪽을 찾을 수 없습니다');

    handler.cursor.moveTo({
      sectionIndex: target.sectionIndex,
      paragraphIndex: target.paragraphIndex,
      charOffset: 0,
    });
    handler.cursor.enterFootnoteMode(
      target.sectionIndex,
      target.paragraphIndex,
      target.controlIndex,
      target.footnoteIndex,
      pageResult.page,
    );
    handler.eventBus.emit('footnoteModeChanged', true);
    handler.cursor.setFnCursorPosition(0, 2);
    handler.active = true;
    handler.focus();
    handler.updateCaret();
    return {
      inFootnote: handler.cursor.isInFootnote(),
      footnoteNumber: info.number,
      sourcePage: pageResult.page + 1,
    };
  }, FOOTNOTE);
  await waitForUi(page);
  return entered;
}

async function openGotoWithOptionG(page) {
  await page.keyboard.down('Alt');
  await page.keyboard.press('KeyG');
  await page.keyboard.up('Alt');
  await page.waitForSelector('.modal-overlay .goto-dialog-body', { timeout: 10_000 });
}

async function setGotoPage(page, value) {
  await page.$eval('.goto-dialog-body input[type="number"]', (input, pageValue) => {
    input.value = String(pageValue);
  }, value);
  await page.keyboard.press('Enter');
}

runTest('#4030 각주에서 대형 문서 찾아가기 본문 전환', async ({ page }) => {
  const loaded = await loadHwpFile(page, SAMPLE);
  assert(loaded.pageCount >= TARGET_PAGE, `대형 HWP가 ${TARGET_PAGE}쪽 이상으로 로드됨 (${loaded.pageCount}쪽)`);

  const entered = await enterKnownFootnote(page);
  assert(entered.inFootnote, `각주 1번 편집 모드 진입 (${entered.sourcePage}쪽)`);
  assert(entered.footnoteNumber === 1, '대형 HWP의 실제 각주 fixture 확인');

  await openGotoWithOptionG(page);
  await setGotoPage(page, TARGET_PAGE);
  await page.waitForSelector('.modal-overlay', { hidden: true, timeout: 10_000 });
  await page.waitForFunction((target) => {
    const text = document.getElementById('sb-page')?.textContent ?? '';
    return text.startsWith(`${target} / `);
  }, { timeout: 10_000 }, TARGET_PAGE);

  const moved = await page.evaluate((target) => {
    const handler = window.__inputHandler;
    const wasm = window.__wasm;
    const position = handler?.getCursorPosition?.() ?? null;
    const expected = wasm?.getPositionOfPage?.(target - 1) ?? null;
    return {
      inFootnote: handler?.cursor?.isInFootnote?.() ?? true,
      status: document.getElementById('sb-page')?.textContent ?? '',
      scrollTop: document.getElementById('scroll-container')?.scrollTop ?? 0,
      targetOffset: handler?.virtualScroll?.getPageOffset?.(target - 1) ?? null,
      position,
      expected,
    };
  }, TARGET_PAGE);
  assert(!moved.inFootnote, '찾아가기 완료 후 각주 편집 모드 종료');
  assert(moved.status.startsWith(`${TARGET_PAGE} / `), `상태 표시줄이 ${TARGET_PAGE}쪽을 표시 (${moved.status})`);
  assert(
    moved.targetOffset != null && Math.abs(moved.scrollTop - moved.targetOffset) < 250,
    `viewport가 대상 ${TARGET_PAGE}쪽 근처 offset으로 이동 (${moved.scrollTop} / ${moved.targetOffset})`,
  );
  assert(
    moved.expected?.ok && moved.position?.sectionIndex === moved.expected.sec && moved.position?.paragraphIndex === moved.expected.para,
    `대상 쪽의 본문 cursor 배치 (${JSON.stringify(moved.position)})`,
  );

  await openGotoWithOptionG(page);
  const reopened = await page.$('.modal-overlay .goto-dialog-body');
  assert(reopened !== null, '본문 전환 뒤 Option+G로 찾아가기 다시 열기');
  await page.keyboard.press('Escape');
  await page.waitForSelector('.modal-overlay', { hidden: true, timeout: 10_000 });
});
