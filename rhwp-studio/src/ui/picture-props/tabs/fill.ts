/** 개체 속성 대화상자 — 채우기 탭 빌더 (picture-props-dialog.ts에서 분리) */
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { setAreaDisabled, fieldset, row, label, unit, numberInput, colorInput, selectEl, checkboxLabel } from '../../dialog-dom-helpers';

export function buildFillTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  // ── 채우기 ──
  const fillFs = fieldset('채우기');
  panel.appendChild(fillFs);

  const radioName = 'pp-fill-type';

  // 색 채우기 없음
  const noneRow = row();
  const noneLabel = document.createElement('label');
  noneLabel.className = 'dialog-checkbox';
  dlg.fillNoneRadio = document.createElement('input');
  dlg.fillNoneRadio.type = 'radio';
  dlg.fillNoneRadio.name = radioName;
  dlg.fillNoneRadio.checked = true;
  noneLabel.appendChild(dlg.fillNoneRadio);
  noneLabel.appendChild(document.createTextNode(' 색 채우기 없음(V)'));
  noneRow.appendChild(noneLabel);
  fillFs.appendChild(noneRow);

  // ◉ 색(O)
  const solidLabel = document.createElement('label');
  solidLabel.className = 'dialog-checkbox';
  dlg.fillSolidRadio = document.createElement('input');
  dlg.fillSolidRadio.type = 'radio';
  dlg.fillSolidRadio.name = radioName;
  solidLabel.appendChild(dlg.fillSolidRadio);
  solidLabel.appendChild(document.createTextNode(' 색(O)'));

  const solidHdr = row();
  solidHdr.appendChild(solidLabel);
  fillFs.appendChild(solidHdr);

  dlg.solidArea = document.createElement('div');
  dlg.solidArea.className = 'pp-fill-sub';
  const sRow = row();
  sRow.appendChild(label('면 색(C):'));
  dlg.solidFaceColor = colorInput('#ffffff');
  sRow.appendChild(dlg.solidFaceColor);
  sRow.appendChild(label('무늬 색(K):'));
  dlg.solidPatColor = colorInput('#000000');
  sRow.appendChild(dlg.solidPatColor);
  sRow.appendChild(label('무늬 모양(L):'));
  dlg.solidPatternSelect = selectEl([
    ['none', '없음'], ['hline', '수평선'], ['vline', '수직선'],
    ['dline1', '대각선1'], ['dline2', '대각선2'], ['cross', '격자'],
  ]);
  sRow.appendChild(dlg.solidPatternSelect);
  dlg.solidArea.appendChild(sRow);
  fillFs.appendChild(dlg.solidArea);

  // ○ 그러데이션(B)
  const gradLabel = document.createElement('label');
  gradLabel.className = 'dialog-checkbox';
  dlg.fillGradientRadio = document.createElement('input');
  dlg.fillGradientRadio.type = 'radio';
  dlg.fillGradientRadio.name = radioName;
  gradLabel.appendChild(dlg.fillGradientRadio);
  gradLabel.appendChild(document.createTextNode(' 그러데이션(B)'));

  const gradHdr = row();
  gradHdr.appendChild(gradLabel);
  fillFs.appendChild(gradHdr);

  dlg.gradientArea = document.createElement('div');
  dlg.gradientArea.className = 'pp-fill-sub';

  const gRow1 = row();
  gRow1.appendChild(label('시작 색(G):'));
  dlg.gradStartColor = colorInput('#ffffff');
  gRow1.appendChild(dlg.gradStartColor);
  gRow1.appendChild(label('끝 색(E):'));
  dlg.gradEndColor = colorInput('#000000');
  gRow1.appendChild(dlg.gradEndColor);
  dlg.gradientArea.appendChild(gRow1);

  const gRow2 = row();
  gRow2.appendChild(label('유형(T):'));
  dlg.gradTypeSelect = selectEl([
    ['linear', '소라'], ['horizontal', '수평'], ['rdiag', '오른쪽 대각선'],
    ['ldiag', '왼쪽 대각선'], ['center', '가운데에서'], ['classic', '클래식'],
    ['narcissus', '나르시스'],
  ]);
  gRow2.appendChild(dlg.gradTypeSelect);
  // 6방향 아이콘
  const dirGrid = document.createElement('div');
  dirGrid.className = 'pp-gradient-dir';
  dlg.gradDirBtns = [];
  const dirs = ['↗', '→', '↘', '↙', '←', '↖'];
  dirs.forEach((icon, i) => {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn pp-grad-dir-btn';
    btn.textContent = icon;
    btn.addEventListener('click', () => {
      dlg.gradDirBtns.forEach((b, j) => b.classList.toggle('active', j === i));
    });
    dirGrid.appendChild(btn);
    dlg.gradDirBtns.push(btn);
  });
  gRow2.appendChild(dirGrid);
  dlg.gradientArea.appendChild(gRow2);

  const gRow3 = row();
  gRow3.appendChild(label('가로 중심(W):'));
  dlg.gradCenterXInput = numberInput();
  dlg.gradCenterXInput.value = '0';
  gRow3.appendChild(dlg.gradCenterXInput);
  gRow3.appendChild(label('세로 중심(X):'));
  dlg.gradCenterYInput = numberInput();
  dlg.gradCenterYInput.value = '0';
  gRow3.appendChild(dlg.gradCenterYInput);
  dlg.gradientArea.appendChild(gRow3);

  const gRow4 = row();
  gRow4.appendChild(label('기울임(Y):'));
  dlg.gradTiltInput = numberInput();
  dlg.gradTiltInput.value = '0';
  gRow4.appendChild(dlg.gradTiltInput);
  gRow4.appendChild(label('번짐 정도(Z):'));
  dlg.gradBlurInput = numberInput(0, 100);
  dlg.gradBlurInput.value = '0';
  gRow4.appendChild(dlg.gradBlurInput);
  gRow4.appendChild(label('반전 중심(N):'));
  dlg.gradReverseCenterInput = numberInput();
  dlg.gradReverseCenterInput.value = '0';
  gRow4.appendChild(dlg.gradReverseCenterInput);
  dlg.gradientArea.appendChild(gRow4);

  fillFs.appendChild(dlg.gradientArea);

  // ☐ 그림(B)
  const imgHdr = row();
  const imgLabel = checkboxLabel('그림(B)');
  dlg.fillImageCheck = imgLabel.querySelector('input') as HTMLInputElement;
  imgHdr.appendChild(imgLabel);
  fillFs.appendChild(imgHdr);

  dlg.imageArea = document.createElement('div');
  dlg.imageArea.className = 'pp-fill-sub';

  const iRow1 = row();
  iRow1.appendChild(label('그림 파일(I):'));
  dlg.imageFileInput = document.createElement('input');
  dlg.imageFileInput.type = 'text';
  dlg.imageFileInput.className = 'dialog-input';
  dlg.imageFileInput.style.flex = '1';
  dlg.imageFileInput.disabled = true;
  iRow1.appendChild(dlg.imageFileInput);
  const browseBtn = document.createElement('button');
  browseBtn.className = 'dialog-btn';
  browseBtn.textContent = '...';
  browseBtn.disabled = true;
  iRow1.appendChild(browseBtn);
  const embedLabel = checkboxLabel('문서에 포함(J)');
  dlg.imageEmbedCheck = embedLabel.querySelector('input') as HTMLInputElement;
  dlg.imageEmbedCheck.disabled = true;
  iRow1.appendChild(embedLabel);
  dlg.imageArea.appendChild(iRow1);

  const iRow2 = row();
  iRow2.appendChild(label('채우기 유형(S):'));
  dlg.imageFillTypeSelect = selectEl([
    ['tile', '바둑판식으로-모두'], ['stretch', '크기에 맞추어'], ['center', '가운데로'],
  ]);
  dlg.imageFillTypeSelect.disabled = true;
  iRow2.appendChild(dlg.imageFillTypeSelect);
  iRow2.appendChild(label('밝기(H):'));
  dlg.imageBrightnessInput = numberInput(-100, 100);
  dlg.imageBrightnessInput.value = '0';
  dlg.imageBrightnessInput.disabled = true;
  iRow2.appendChild(dlg.imageBrightnessInput);
  iRow2.appendChild(unit('%'));
  dlg.imageArea.appendChild(iRow2);

  const iRow3 = row();
  iRow3.appendChild(label('그림 효과(E):'));
  dlg.imageEffectSelect = selectEl([
    ['none', '효과 없음'], ['gray', '회색조'], ['bw', '흑백'],
  ]);
  dlg.imageEffectSelect.disabled = true;
  iRow3.appendChild(dlg.imageEffectSelect);
  iRow3.appendChild(label('대비(I):'));
  dlg.imageContrastInput = numberInput(-100, 100);
  dlg.imageContrastInput.value = '0';
  dlg.imageContrastInput.disabled = true;
  iRow3.appendChild(dlg.imageContrastInput);
  iRow3.appendChild(unit('%'));
  dlg.imageArea.appendChild(iRow3);

  const iRow4 = row();
  const wmLabel = checkboxLabel('워터마크 효과(M)');
  dlg.imageWatermarkCheck = wmLabel.querySelector('input') as HTMLInputElement;
  dlg.imageWatermarkCheck.disabled = true;
  iRow4.appendChild(wmLabel);
  dlg.imageArea.appendChild(iRow4);

  fillFs.appendChild(dlg.imageArea);

  // ── 투명도 설정 ──
  const transFs = fieldset('투명도 설정');
  panel.appendChild(transFs);
  const transRow = row();
  transRow.appendChild(label('투명도(I):'));
  dlg.fillTransInput = numberInput(0, 100, 1);
  dlg.fillTransInput.value = '0';
  dlg.fillTransInput.disabled = true;
  transRow.appendChild(dlg.fillTransInput);
  transRow.appendChild(unit('%'));
  transFs.appendChild(transRow);

  // 라디오 전환 이벤트
  const updateFillVisibility = () => {
    const isSolid = dlg.fillSolidRadio.checked;
    const isGrad = dlg.fillGradientRadio.checked;
    dlg.solidArea.style.opacity = isSolid ? '1' : '0.4';
    dlg.gradientArea.style.opacity = isGrad ? '1' : '0.4';
    setAreaDisabled(dlg.solidArea, !isSolid);
    setAreaDisabled(dlg.gradientArea, !isGrad);
    // 투명도: 채우기 없음이면 비활성, 색/그러데이션이면 활성
    dlg.fillTransInput.disabled = !(isSolid || isGrad);
  };
  dlg.fillNoneRadio.addEventListener('change', updateFillVisibility);
  dlg.fillSolidRadio.addEventListener('change', updateFillVisibility);
  dlg.fillGradientRadio.addEventListener('change', updateFillVisibility);
  // 초기 상태
  setTimeout(updateFillVisibility, 0);

  return panel;
}
