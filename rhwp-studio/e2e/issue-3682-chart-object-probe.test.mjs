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

  // ── P2: 속성 다이얼로그 (실제 진입: 선택 상태에서 단축키 P) ──
  // 단계 간 간섭 차단 — 매 단계 시작 시 선택을 재확립한다(P 키가 선택을 흐트러뜨림 실측).
  await page.mouse.click(pt.x, pt.y);
  await pause(page, 400);
  await page.keyboard.press('KeyP');
  await pause(page, 700);
  const dialogVisible = await page.evaluate(() =>
    !!document.querySelector('.picture-props-dialog, [data-dialog="picture-props"], dialog[open], .modal-overlay'));
  record('P2', '속성 다이얼로그(단축키 P)', dialogVisible ? '열림' : '안 됨',
    dialogVisible ? 'format:object-properties' : '선택 상태에서 P 무반응');
  if (dialogVisible) {
    await screenshot(page, '3682-p2-props-dialog');
    await page.keyboard.press('Escape');
    await pause(page, 400);
  }

  // ── P3: 이동 (드래그) ──
  await page.mouse.click(pt.x, pt.y);
  await pause(page, 400);
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

  // ── P5: z-order (브리지 changeShapeZOrder 직접 호출) ──
  await page.mouse.click(pt.x, pt.y);
  await pause(page, 300);
  const p5 = await page.evaluate(() => {
    try {
      const cur = window.__inputHandler?.cursor;
      const ref = cur?.getSelectedPictureRef?.();
      if (!ref) return { ok: false, why: '선택 없음' };
      const w = window.__wasm;
      if (typeof w?.changeShapeZOrder !== 'function') return { ok: false, why: 'changeShapeZOrder 부재' };
      const r = w.changeShapeZOrder(ref.sec, ref.ppi, ref.ci, 'front');
      const res = typeof r === 'string' ? JSON.parse(r) : r;
      return { ok: !!res?.ok, why: res?.ok ? `zOrder=${res.zOrder}` : (res?.error || JSON.stringify(res)) };
    } catch (e) { return { ok: false, why: e.message }; }
  });
  record('P5', 'z-order(front)', p5.ok ? '됨' : '안 됨', p5.why);

  console.log('\n=== #3682 P0~P5 행동 실측 요약 ===');
  for (const r of results) console.log(`${r.phase}\t${r.item}\t${r.status}\t${r.detail}`);
});
