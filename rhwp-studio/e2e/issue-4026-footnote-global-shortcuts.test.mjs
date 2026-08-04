/**
 * #4026: 각주 편집 중에도 전역 되돌리기와 찾아가기 단축키가 동작해야 한다.
 */
import { runTest, loadHwpFile, assert } from './helpers.mjs';

async function waitForUi(page) {
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 300)));
}

async function enterFirstFootnote(page) {
  await page.evaluate(() => {
    const handler = window.__inputHandler;
    if (!handler) throw new Error('input handler를 찾을 수 없습니다');

    handler.cursor.enterFootnoteMode(0, 3, 0, 0, 0);
    handler.eventBus.emit('footnoteModeChanged', true);
    handler.cursor.setFnCursorPosition(0, 0);
    handler.active = true;
    handler.focus();
    handler.updateCaret();
  });
  await waitForUi(page);
}

async function footnoteCursor(page) {
  return await page.evaluate(() => {
    const cursor = window.__inputHandler?.cursor;
    return {
      inFootnote: cursor?.isInFootnote?.() ?? false,
      innerParaIdx: cursor?.fnInnerParaIdx,
      charOffset: cursor?.fnCharOffset,
    };
  });
}

async function pressMetaZ(page) {
  await page.keyboard.down('Meta');
  await page.keyboard.press('KeyZ');
  await page.keyboard.up('Meta');
}

async function openGotoWithOptionG(page) {
  await page.keyboard.down('Alt');
  await page.keyboard.press('KeyG');
  await page.keyboard.up('Alt');
  await page.waitForSelector('.modal-overlay .goto-dialog-body', { timeout: 10_000 });
}

runTest('#4026 각주 편집 전역 단축키', async ({ page }) => {
  await loadHwpFile(page, 'footnote-01.hwp');
  await enterFirstFootnote(page);

  const beforeInput = await footnoteCursor(page);
  assert(beforeInput.inFootnote, '각주 편집 모드에 진입');

  await page.keyboard.type('X');
  await waitForUi(page);
  const afterInput = await footnoteCursor(page);
  assert(
    afterInput.charOffset === beforeInput.charOffset + 1,
    `각주 텍스트 입력 후 커서 이동 (${beforeInput.charOffset} -> ${afterInput.charOffset})`,
  );

  await pressMetaZ(page);
  await waitForUi(page);
  const afterUndo = await footnoteCursor(page);
  assert(afterUndo.inFootnote, 'Cmd+Z 후에도 각주 편집 모드 유지');
  assert(
    afterUndo.charOffset === beforeInput.charOffset,
    `Cmd+Z가 각주 입력을 되돌림 (${afterInput.charOffset} -> ${afterUndo.charOffset})`,
  );

  await openGotoWithOptionG(page);
  const gotoVisible = await page.$('.modal-overlay .goto-dialog-body');
  assert(gotoVisible !== null, 'Option+G가 각주 편집 중 찾아가기 대화상자를 표시');
  await page.keyboard.press('Escape');
  await page.waitForSelector('.modal-overlay', { hidden: true, timeout: 10_000 });
});
