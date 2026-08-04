/**
 * [#3682] 차트 개체 1급화 — P1~P5 행동 실측 프로브.
 *
 * 목적은 통과/실패 판정이 아니라 **현황 수집**이다. 각 동작을 시도하고 결과를
 * 콘솔에 표로 남긴다(실패해도 다음 단계 계속). 이슈 전제("Track A 미착수")가
 * 낡았을 가능성이 실측으로 드러난 상태라(P0·studio 배선 존재), 어느 동작이
 * 실제로 되고 안 되는지의 목록을 만든다.
 */
import { runTest, loadHwpFile, screenshot } from './helpers.mjs';

const SAMPLE = 'chart/세로막대형/묶은세로막대형.hwp';

const results = [];
function record(phase, item, status, detail) {
  results.push({ phase, item, status, detail });
  console.log(`  [${phase}] ${item}: ${status}${detail ? ` — ${detail}` : ''}`);
}

async function pause(page, ms = 300) {
  await page.evaluate(d => new Promise(r => setTimeout(r, d)), ms);
}

/** 차트 OLE 레이아웃 좌표를 화면 클릭 좌표로 변환 */
async function oleClickPoint(page) {
  return page.evaluate(() => {
    const layout = window.__wasm.getPageControlLayout(0);
    const ole = (layout?.controls || []).find(c => c.type === 'ole');
    if (!ole) return null;
    const el = document.querySelector('#scroll-content');
    const rect = el.getBoundingClientRect();
    const scale = window.__canvasView?.scale ?? 1;
    return {
      x: rect.left + (ole.x + ole.w / 2) * scale,
      y: rect.top + (ole.y + ole.h / 2) * scale,
      layout: { x: ole.x, y: ole.y, w: ole.w, h: ole.h, secIdx: ole.secIdx, paraIdx: ole.paraIdx, controlIdx: ole.controlIdx },
    };
  });
}

async function selectionState(page) {
  return page.evaluate(() => {
    const cur = window.__inputHandler?.cursor;
    const on = !!cur?.isInPictureObjectSelection?.();
    const ref = cur?.getSelectedPictureRef?.() ?? null;
    return {
      hasSelection: on,
      type: ref?.type ?? null,
      ref: ref ? { sec: ref.sec, ppi: ref.ppi, ci: ref.ci } : null,
    };
  });
}

runTest('#3682 차트 개체 1급화 — P1~P5 행동 실측', async ({ page }) => {
  page.on('console', m => {
    const t = m.text();
    if (/error|실패|失敗/i.test(t)) console.log(`    (console) ${t.slice(0, 160)}`);
  });

  const info = await loadHwpFile(page, SAMPLE);
  console.log(`문서 로드: ${info.pageCount}쪽\n`);

  // ── P0: 레이아웃 방출 ──
  const pt = await oleClickPoint(page);
  if (!pt) {
    record('P0', 'ole 레이아웃 방출', '없음', 'getPageControlLayout 에 type:"ole" 부재');
    console.log('\nP0 부재 — 이후 단계 측정 불가');
    return;
  }
  record('P0', 'ole 레이아웃 방출', '있음', JSON.stringify(pt.layout));

  // ── P1: 클릭 선택 ──
  await page.mouse.click(pt.x, pt.y);
  await pause(page, 500);
  let sel = await selectionState(page);
  record('P1', '클릭 선택', sel.hasSelection ? '됨' : '안 됨',
    sel.hasSelection ? `type=${sel.type} ref=${JSON.stringify(sel.ref)}` : '선택 상태 없음');
  await screenshot(page, '3682-p1-click-select');

  // ── P2: 속성 다이얼로그 ──
  const p2 = await page.evaluate(() => {
    try {
      const cur = window.__inputHandler?.cursor;
      if (!cur?.isInPictureObjectSelection?.()) return { ok: false, why: '선택 상태 아님(P1 선행 실패)' };
      const cmd = window.__commandRegistry ?? window.__app?.commands;
      const run = cmd?.execute ?? cmd?.run;
      if (typeof run !== 'function') return { ok: false, why: '커맨드 레지스트리 없음' };
      run.call(cmd, 'format.pictureProps');
      return { ok: true };
    } catch (e) { return { ok: false, why: e.message }; }
  });
  await pause(page, 500);
  const dialogVisible = await page.evaluate(() =>
    !!document.querySelector('.picture-props-dialog, [data-dialog="picture-props"], dialog[open]'));
  record('P2', '속성 다이얼로그', dialogVisible ? '열림' : (p2.ok ? '호출됐으나 미표시' : '안 됨'), p2.why || '');
  if (dialogVisible) {
    await screenshot(page, '3682-p2-props-dialog');
    await page.keyboard.press('Escape');
    await pause(page);
  }

  // ── P3: 이동 (드래그) ──
  const before = (await oleClickPoint(page))?.layout;
  await page.mouse.move(pt.x, pt.y);
  await page.mouse.down();
  await page.mouse.move(pt.x + 60, pt.y + 40, { steps: 8 });
  await page.mouse.up();
  await pause(page, 700);
  const after = (await oleClickPoint(page))?.layout;
  const movedX = after && before ? +(after.x - before.x).toFixed(1) : null;
  const movedY = after && before ? +(after.y - before.y).toFixed(1) : null;
  record('P3', '드래그 이동', (movedX || movedY) ? '됨' : '안 됨',
    `Δx=${movedX} Δy=${movedY}`);
  await screenshot(page, '3682-p3-drag-move');

  // ── P4: 복사/삭제/undo ──
  await page.mouse.click(pt.x, pt.y);
  await pause(page, 300);
  const countOle = () => page.evaluate(() =>
    (window.__wasm.getPageControlLayout(0)?.controls || []).filter(c => c.type === 'ole').length);
  const n0 = await countOle();

  await page.keyboard.down('Control'); await page.keyboard.press('KeyC'); await page.keyboard.up('Control');
  await pause(page, 400);
  await page.keyboard.down('Control'); await page.keyboard.press('KeyV'); await page.keyboard.up('Control');
  await pause(page, 800);
  const n1 = await countOle();
  record('P4', '복사·붙여넣기', n1 > n0 ? '됨' : '안 됨', `ole 개수 ${n0}→${n1}`);

  await page.keyboard.press('Delete');
  await pause(page, 600);
  const n2 = await countOle();
  record('P4', '삭제', n2 < n1 ? '됨' : '안 됨', `ole 개수 ${n1}→${n2}`);

  await page.keyboard.down('Control'); await page.keyboard.press('KeyZ'); await page.keyboard.up('Control');
  await pause(page, 700);
  const n3 = await countOle();
  record('P4', 'undo', n3 > n2 ? '됨' : '안 됨', `ole 개수 ${n2}→${n3}`);
  await screenshot(page, '3682-p4-clipboard-undo');

  // ── P5: z-order API 존재 ──
  const p5 = await page.evaluate(() => {
    const ih = window.__inputHandler;
    const names = ['bringToFront', 'sendToBack', 'bringForward', 'sendBackward'];
    return names.filter(n => typeof ih?.[n] === 'function');
  });
  record('P5', 'z-order 명령', p5.length ? '함수 존재' : '안 됨', p5.join(',') || 'none');

  console.log('\n=== #3682 P0~P5 행동 실측 요약 ===');
  for (const r of results) console.log(`${r.phase}\t${r.item}\t${r.status}\t${r.detail}`);
});
