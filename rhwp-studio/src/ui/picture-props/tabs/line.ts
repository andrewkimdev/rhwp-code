/** 개체 속성 대화상자 — 선 탭 빌더 (picture-props-dialog.ts에서 분리) */
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { fieldset, row, label, unit, numberInput, colorInput, selectEl, checkboxLabel } from '../../dialog-dom-helpers';

export function buildLineTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  // ── 선 ──
  const lineFs = fieldset('선');
  panel.appendChild(lineFs);

  const row1 = row();
  row1.appendChild(label('색(C):'));
  dlg.lineColorInput = colorInput('#000000');
  row1.appendChild(dlg.lineColorInput);
  row1.appendChild(label('종류(L):'));
  // HWP 선 종류: attr bits 0-5 (0~17)
  dlg.lineTypeSelect = selectEl([
    ['0', '선 없음'], ['1', '실선'], ['2', '파선'], ['3', '점선'],
    ['4', '일점쇄선'], ['5', '이점쇄선'], ['6', '긴 파선'], ['7', '원형 점선'],
    ['8', '2중선'], ['9', '가는선-굵은선'], ['10', '굵은선-가는선'], ['11', '3중선'],
  ]);
  row1.appendChild(dlg.lineTypeSelect);
  lineFs.appendChild(row1);

  const row2 = row();
  row2.appendChild(label('끝 모양(E):'));
  // HWP 끝 모양: attr bits 6-9
  dlg.lineEndSelect = selectEl([
    ['0', '둥근'], ['1', '평면'],
  ]);
  row2.appendChild(dlg.lineEndSelect);
  row2.appendChild(label('굵기(T):'));
  dlg.lineWidthInput = numberInput(0, undefined, 0.01);
  dlg.lineWidthInput.value = '0.12';
  row2.appendChild(dlg.lineWidthInput);
  row2.appendChild(unit('mm'));
  lineFs.appendChild(row2);

  if (dlg.objectType === 'ole') return panel;

  // ── 화살표 ──
  const arrowFs = fieldset('화살표');
  panel.appendChild(arrowFs);

  const aRow1 = row();
  aRow1.appendChild(label('시작 모양(S):'));
  // HWP 화살표 모양: attr bits 10-15 / 16-21
  dlg.arrowStartSelect = selectEl([
    ['0', '없음'], ['1', '화살표'], ['2', '열린 화살표'],
    ['3', '꼬리 화살표'], ['4', '마름모'], ['5', '원형'], ['6', '사각형'],
  ]);
  aRow1.appendChild(dlg.arrowStartSelect);
  aRow1.appendChild(label('끝 모양(Y):'));
  dlg.arrowEndSelect = selectEl([
    ['0', '없음'], ['1', '화살표'], ['2', '열린 화살표'],
    ['3', '꼬리 화살표'], ['4', '마름모'], ['5', '원형'], ['6', '사각형'],
  ]);
  aRow1.appendChild(dlg.arrowEndSelect);
  arrowFs.appendChild(aRow1);

  const aRow2 = row();
  aRow2.appendChild(label('시작 크기(Z):'));
  // HWP 화살표 크기: attr bits 22-25 / 26-29 (0~8)
  dlg.arrowStartSizeSelect = selectEl([
    ['0', '작은×작은'], ['1', '작은×중간'], ['2', '작은×큰'],
    ['3', '중간×작은'], ['4', '중간×중간'], ['5', '중간×큰'],
    ['6', '큰×작은'], ['7', '큰×중간'], ['8', '큰×큰'],
  ]);
  aRow2.appendChild(dlg.arrowStartSizeSelect);
  aRow2.appendChild(label('끝 크기(N):'));
  dlg.arrowEndSizeSelect = selectEl([
    ['0', '작은×작은'], ['1', '작은×중간'], ['2', '작은×큰'],
    ['3', '중간×작은'], ['4', '중간×중간'], ['5', '중간×큰'],
    ['6', '큰×작은'], ['7', '큰×중간'], ['8', '큰×큰'],
  ]);
  aRow2.appendChild(dlg.arrowEndSizeSelect);
  arrowFs.appendChild(aRow2);

  // ── 사각형 모서리 곡률 ──
  const cornerFs = fieldset('사각형 모서리 곡률');
  panel.appendChild(cornerFs);

  const cRow = row();
  dlg.cornerBtns = [];
  const cornerIcons = ['▢', '▢̤', '⬭'];
  const cornerTitles = ['직각(G)', '둥근 모양(O)', '반원(M)'];
  cornerTitles.forEach((title, i) => {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn pp-corner-btn';
    btn.textContent = cornerIcons[i];
    btn.title = title;
    btn.addEventListener('click', () => {
      dlg.cornerBtns.forEach((b, j) => b.classList.toggle('active', j === i));
      if (dlg.cornerCustomRadio) dlg.cornerCustomRadio.checked = false;
    });
    cRow.appendChild(btn);
    dlg.cornerBtns.push(btn);
  });
  // 곡률 지정 라디오
  const crLabel = document.createElement('label');
  crLabel.className = 'dialog-checkbox';
  dlg.cornerCustomRadio = document.createElement('input');
  dlg.cornerCustomRadio.type = 'radio';
  dlg.cornerCustomRadio.name = 'corner-mode';
  crLabel.appendChild(dlg.cornerCustomRadio);
  crLabel.appendChild(document.createTextNode(' 곡률 지정(J):'));
  cRow.appendChild(crLabel);
  dlg.cornerCustomInput = numberInput(0, 100, 1);
  dlg.cornerCustomInput.value = '0';
  dlg.cornerCustomInput.disabled = true;
  cRow.appendChild(dlg.cornerCustomInput);
  cRow.appendChild(unit('%'));
  dlg.cornerCustomRadio.addEventListener('change', () => {
    dlg.cornerBtns.forEach(b => b.classList.remove('active'));
    dlg.cornerCustomInput.disabled = !dlg.cornerCustomRadio.checked;
  });
  cornerFs.appendChild(cRow);

  // ── 호 테두리 ──
  const arcFs = fieldset('호 테두리');
  panel.appendChild(arcFs);

  const arcRow = row();
  dlg.arcBtns = [];
  const arcTitles = ['호(A)', '부채꼴(B)', '활 모양(I)'];
  const arcIcons = ['⌒', '◔', '⌢'];
  arcTitles.forEach((title, i) => {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn';
    btn.textContent = arcIcons[i];
    btn.title = title;
    btn.disabled = true;
    btn.addEventListener('click', () => {
      dlg.arcBtns.forEach((b, j) => b.classList.toggle('active', j === i));
    });
    arcRow.appendChild(btn);
    dlg.arcBtns.push(btn);
  });
  arcFs.appendChild(arcRow);

  // ── 투명도 설정 + 기타 ──
  const transRow = row();
  transRow.appendChild(label('투명도(I):'));
  dlg.lineTransInput = numberInput(0, 100, 1);
  dlg.lineTransInput.value = '0';
  dlg.lineTransInput.disabled = true;
  transRow.appendChild(dlg.lineTransInput);
  transRow.appendChild(unit('%'));

  const liLabel = checkboxLabel('선 굵기 내부 적용(K)');
  dlg.lineInnerCheck = liLabel.querySelector('input') as HTMLInputElement;
  dlg.lineInnerCheck.disabled = true;
  transRow.appendChild(liLabel);
  panel.appendChild(transRow);

  return panel;
}
