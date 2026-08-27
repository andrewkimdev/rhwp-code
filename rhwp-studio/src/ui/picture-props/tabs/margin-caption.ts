/** 개체 속성 대화상자 — 여백/캡션 탭 빌더 (picture-props-dialog.ts에서 분리) */
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { fieldset, row, label, unit, numberInput, checkboxLabel } from '../../dialog-dom-helpers';

export function buildMarginCaptionTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  // ── 바깥 여백 ──
  const marginFs = fieldset('바깥 여백');
  panel.appendChild(marginFs);

  const row1 = row();
  row1.appendChild(label('왼쪽(L):'));
  dlg.outerMarginLeftInput = numberInput(0);
  dlg.outerMarginLeftInput.value = '0.00';
  row1.appendChild(dlg.outerMarginLeftInput);
  row1.appendChild(unit('mm'));
  row1.appendChild(label('위쪽(T):'));
  dlg.outerMarginTopInput = numberInput(0);
  dlg.outerMarginTopInput.value = '0.00';
  row1.appendChild(dlg.outerMarginTopInput);
  row1.appendChild(unit('mm'));
  // 모두(A) — ▲▼ 화살표만 있는 동기 스피너
  row1.appendChild(label('모두'));
  const syncWrap = document.createElement('div');
  syncWrap.className = 'pp-sync-arrows';
  const syncUp = document.createElement('button');
  syncUp.className = 'pp-sync-arrow-btn';
  syncUp.textContent = '▲';
  syncUp.title = '모두 증가';
  syncUp.addEventListener('click', () => {
    [dlg.outerMarginLeftInput, dlg.outerMarginRightInput,
     dlg.outerMarginTopInput, dlg.outerMarginBottomInput].forEach(inp => {
      inp.value = (parseFloat(inp.value || '0') + 0.5).toFixed(2);
    });
  });
  const syncDown = document.createElement('button');
  syncDown.className = 'pp-sync-arrow-btn';
  syncDown.textContent = '▼';
  syncDown.title = '모두 감소';
  syncDown.addEventListener('click', () => {
    [dlg.outerMarginLeftInput, dlg.outerMarginRightInput,
     dlg.outerMarginTopInput, dlg.outerMarginBottomInput].forEach(inp => {
      const v = parseFloat(inp.value || '0') - 0.5;
      inp.value = Math.max(0, v).toFixed(2);
    });
  });
  syncWrap.appendChild(syncUp);
  syncWrap.appendChild(syncDown);
  row1.appendChild(syncWrap);
  marginFs.appendChild(row1);

  const row2 = row();
  row2.appendChild(label('오른쪽(R):'));
  dlg.outerMarginRightInput = numberInput(0);
  dlg.outerMarginRightInput.value = '0.00';
  row2.appendChild(dlg.outerMarginRightInput);
  row2.appendChild(unit('mm'));
  row2.appendChild(label('아래쪽(B):'));
  dlg.outerMarginBottomInput = numberInput(0);
  dlg.outerMarginBottomInput.value = '0.00';
  row2.appendChild(dlg.outerMarginBottomInput);
  row2.appendChild(unit('mm'));
  marginFs.appendChild(row2);

  // ── 캡션 ──
  const captionFs = fieldset('캡션');
  panel.appendChild(captionFs);

  // 가로 배치: 그리드(왼) + 속성(오)
  const capLayout = document.createElement('div');
  capLayout.className = 'pp-caption-layout';

  // 3×3 캡션 위치 그리드
  const grid = document.createElement('div');
  grid.className = 'pp-caption-grid';
  dlg.captionBtns = [];
  const capTitles = [
    '왼쪽 위', '위', '오른쪽 위',
    '왼쪽', '가운데', '오른쪽',
    '왼쪽 아래', '아래', '오른쪽 아래',
  ];
  const capIcons = [
    '┌가1', '가1─', '가1┐',
    '│가1', '□', '가1│',
    '└가1', '가1─', '가1┘',
  ];
  capTitles.forEach((title, i) => {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn pp-caption-btn';
    btn.textContent = capIcons[i];
    btn.title = title;
    btn.disabled = true;
    btn.addEventListener('click', () => {
      dlg.captionBtns.forEach((b, j) => b.classList.toggle('active', j === i));
    });
    grid.appendChild(btn);
    dlg.captionBtns.push(btn);
  });
  capLayout.appendChild(grid);

  // 오른쪽 속성 영역
  const capRight = document.createElement('div');
  capRight.className = 'pp-caption-attrs';

  // 크기
  const capRow1 = row();
  capRow1.appendChild(label('크기(S):'));
  dlg.captionSizeInput = numberInput(0);
  dlg.captionSizeInput.value = '30.00';
  dlg.captionSizeInput.disabled = true;
  capRow1.appendChild(dlg.captionSizeInput);
  capRow1.appendChild(unit('mm'));
  capRight.appendChild(capRow1);

  // 개체와의 간격
  const capRow2 = row();
  capRow2.appendChild(label('개체와의 간격(G):'));
  dlg.captionGapInput = numberInput(0);
  dlg.captionGapInput.value = '3.00';
  dlg.captionGapInput.disabled = true;
  capRow2.appendChild(dlg.captionGapInput);
  capRow2.appendChild(unit('mm'));
  capRight.appendChild(capRow2);

  // 체크박스
  const ceLabel = checkboxLabel('여백 부분까지 너비 확대(W)');
  dlg.captionExpandCheck = ceLabel.querySelector('input') as HTMLInputElement;
  dlg.captionExpandCheck.disabled = true;
  capRight.appendChild(ceLabel);
  const cslLabel = checkboxLabel('한 줄로 입력(O)');
  dlg.captionSingleLineCheck = cslLabel.querySelector('input') as HTMLInputElement;
  dlg.captionSingleLineCheck.disabled = true;
  capRight.appendChild(cslLabel);

  capLayout.appendChild(capRight);
  captionFs.appendChild(capLayout);

  return panel;
}
