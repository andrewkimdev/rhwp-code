/**
 * #3953: 대형 HWP의 후반부 찾아가기와 상태 표시줄 진입점을 검증한다.
 */
import {
  runTest, loadHwpFile, clickEditArea, assert,
} from './helpers.mjs';

const SAMPLE = '정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwp';
const TARGET_PAGE = 158;

async function openGotoWithOptionG(page) {
  await page.keyboard.down('Alt');
  await page.keyboard.press('g');
  await page.keyboard.up('Alt');
  await page.waitForSelector('.modal-overlay .goto-dialog-body', { timeout: 10_000 });
}

async function setGotoPage(page, value) {
  await page.$eval('.goto-dialog-body input[type="number"]', (input, pageValue) => {
    input.value = String(pageValue);
  }, value);
  await page.keyboard.press('Enter');
}

runTest('#3953 대형 문서 찾아가기', async ({ page }) => {
  const loaded = await loadHwpFile(page, SAMPLE);
  assert(loaded.pageCount >= TARGET_PAGE, `대형 HWP가 ${TARGET_PAGE}쪽 이상으로 로드됨 (${loaded.pageCount}쪽)`);

  await clickEditArea(page);
  await openGotoWithOptionG(page);
  await setGotoPage(page, TARGET_PAGE);
  await page.waitForSelector('.modal-overlay', { hidden: true, timeout: 10_000 });
  await page.waitForFunction((target) => {
    const text = document.getElementById('sb-page')?.textContent ?? '';
    return text.startsWith(`${target} / `);
  }, { timeout: 10_000 }, TARGET_PAGE);

  const moved = await page.evaluate((target) => ({
    status: document.getElementById('sb-page')?.textContent ?? '',
    scrollTop: document.getElementById('scroll-container')?.scrollTop ?? 0,
    cursor: window.__inputHandler?.getCursorPosition?.() ?? null,
  }), TARGET_PAGE);
  assert(moved.scrollTop > 0, `대상 쪽으로 스크롤 이동 (${moved.scrollTop})`);
  assert(moved.cursor !== null, `대상 쪽 또는 인접 문단에 커서 배치 (${JSON.stringify(moved.cursor)})`);

  await page.click('#sb-page');
  await page.waitForSelector('.modal-overlay .goto-dialog-body', { timeout: 10_000 });
  await page.keyboard.press('Escape');
  await page.waitForSelector('.modal-overlay', { hidden: true, timeout: 10_000 });

  // 잘못된 입력으로 모달을 유지한 뒤에도 Option+G 경로가 키 이벤트를 가로채지 않는다.
  await openGotoWithOptionG(page);
  await setGotoPage(page, loaded.pageCount + 1);
  const invalidInputState = await page.$eval('.goto-dialog-body', (body) => ({
    error: body.textContent ?? '',
    inputFocused: document.activeElement === body.querySelector('input[type="number"]'),
  }));
  assert(invalidInputState.error.includes('범위의 쪽 번호'), '잘못된 쪽 번호는 모달 안에 오류를 표시');
  assert(invalidInputState.inputFocused, '잘못된 쪽 번호 뒤 입력칸을 다시 선택');
  await page.keyboard.press('Escape');
  await page.waitForSelector('.modal-overlay', { hidden: true, timeout: 10_000 });
  await openGotoWithOptionG(page);
  await page.keyboard.press('Escape');
});
