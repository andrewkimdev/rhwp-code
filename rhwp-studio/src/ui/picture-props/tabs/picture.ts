/** 개체 속성 대화상자 — 그림 탭 빌더 (picture-props-dialog.ts에서 분리) */
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { fieldset, row, label, unit, numberInput, checkboxLabel } from '../../dialog-dom-helpers';

export function buildPictureTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  // ── 파일 이름 ──
  const fileFs = fieldset('파일 이름');
  panel.appendChild(fileFs);
  const fileRow = row();
  // [Task #741 후속] 외부 file path 그림 영역 dialog 표시 영역. populateFromProps 영역
  // 영역 props.externalPath 영역 보유 시 file path + embed=false 영역 갱신.
  dlg.picFileNameInput = document.createElement('input');
  dlg.picFileNameInput.type = 'text';
  dlg.picFileNameInput.className = 'dialog-input';
  dlg.picFileNameInput.style.width = '280px';
  dlg.picFileNameInput.readOnly = true;
  dlg.picFileNameInput.value = '(문서에 포함된 그림)';
  fileRow.appendChild(dlg.picFileNameInput);
  const embedLabel = checkboxLabel('문서에 포함');
  dlg.picEmbedCheck = embedLabel.querySelector('input') as HTMLInputElement;
  dlg.picEmbedCheck.checked = true;
  dlg.picEmbedCheck.disabled = true;
  fileRow.appendChild(embedLabel);
  fileFs.appendChild(fileRow);

  // ── 확대/축소 비율 ──
  const scaleFs = fieldset('확대/축소 비율');
  panel.appendChild(scaleFs);

  const sxRow = row();
  sxRow.appendChild(label('가로'));
  dlg.picScaleXInput = numberInput(1, 1000, 0.01);
  dlg.picScaleXInput.style.width = '70px';
  sxRow.appendChild(dlg.picScaleXInput);
  sxRow.appendChild(unit('%'));
  // 아이콘 버튼들
  const scalePresets = [
    { label: '🔍', title: '원래 크기로', pct: 100 },
    { label: '½', title: '1/2배', pct: 50 },
    { label: '⅔', title: '2/3배', pct: 67 },
    { label: '³⁄₂', title: '3/2배', pct: 150 },
    { label: '×2', title: '2배', pct: 200 },
  ];
  for (const p of scalePresets) {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn';
    btn.textContent = p.label;
    btn.title = p.title;
    btn.addEventListener('click', () => {
      dlg.picScaleXInput.value = String(p.pct);
      if (dlg.picKeepRatioCheck.checked) {
        dlg.picScaleYInput.value = String(p.pct);
      }
    });
    dlg.sizeLockControls.push(btn);
    sxRow.appendChild(btn);
  }
  scaleFs.appendChild(sxRow);

  const syRow = row();
  syRow.appendChild(label('세로'));
  dlg.picScaleYInput = numberInput(1, 1000, 0.01);
  dlg.picScaleYInput.style.width = '70px';
  dlg.sizeLockControls.push(dlg.picScaleXInput, dlg.picScaleYInput);
  syRow.appendChild(dlg.picScaleYInput);
  syRow.appendChild(unit('%'));
  scaleFs.appendChild(syRow);

  const ratioRow = row();
  const ratioLabel = checkboxLabel('가로 세로 같은 비율 유지');
  dlg.picKeepRatioCheck = ratioLabel.querySelector('input') as HTMLInputElement;
  dlg.sizeLockControls.push(dlg.picKeepRatioCheck);
  ratioRow.appendChild(ratioLabel);
  const resetBtn = document.createElement('button');
  resetBtn.className = 'dialog-btn';
  resetBtn.textContent = '원래 그림으로';
  resetBtn.style.marginLeft = '12px';
  resetBtn.addEventListener('click', () => {
    dlg.picScaleXInput.value = '100';
    dlg.picScaleYInput.value = '100';
    dlg.picCropLeftInput.value = '0.00';
    dlg.picCropTopInput.value = '0.00';
    dlg.picCropRightInput.value = '0.00';
    dlg.picCropBottomInput.value = '0.00';
    // 효과 초기화
    if (dlg.picEffectRadios[0]) dlg.picEffectRadios[0].checked = true;
    dlg.picBrightnessInput.value = '0';
    dlg.picContrastInput.value = '0';
    dlg.picTransparencyInput.value = '0';
  });
  dlg.sizeLockControls.push(resetBtn);
  ratioRow.appendChild(resetBtn);
  scaleFs.appendChild(ratioRow);

  // 비율 유지 이벤트
  dlg.picScaleXInput.addEventListener('input', () => {
    if (dlg.picKeepRatioCheck.checked) {
      dlg.picScaleYInput.value = dlg.picScaleXInput.value;
    }
  });
  dlg.picScaleYInput.addEventListener('input', () => {
    if (dlg.picKeepRatioCheck.checked) {
      dlg.picScaleXInput.value = dlg.picScaleYInput.value;
    }
  });

  // ── 그림 자르기 ──
  const cropFs = fieldset('그림 자르기');
  panel.appendChild(cropFs);
  const cropRow1 = row();
  cropRow1.appendChild(label('왼쪽'));
  dlg.picCropLeftInput = numberInput(0);
  dlg.picCropLeftInput.value = '0.00';
  cropRow1.appendChild(dlg.picCropLeftInput);
  cropRow1.appendChild(unit('mm'));
  cropRow1.appendChild(label('위쪽'));
  dlg.picCropTopInput = numberInput(0);
  dlg.picCropTopInput.value = '0.00';
  cropRow1.appendChild(dlg.picCropTopInput);
  cropRow1.appendChild(unit('mm'));
  // 모두 스피너
  cropRow1.appendChild(label('모두'));
  const cropSync = numberInput(0);
  cropSync.className = 'dialog-input pp-sync-spinner';
  cropSync.addEventListener('input', () => {
    const v = cropSync.value;
    dlg.picCropLeftInput.value = v;
    dlg.picCropTopInput.value = v;
    dlg.picCropRightInput.value = v;
    dlg.picCropBottomInput.value = v;
  });
  cropRow1.appendChild(cropSync);
  cropFs.appendChild(cropRow1);

  const cropRow2 = row();
  cropRow2.appendChild(label('오른쪽'));
  dlg.picCropRightInput = numberInput(0);
  dlg.picCropRightInput.value = '0.00';
  cropRow2.appendChild(dlg.picCropRightInput);
  cropRow2.appendChild(unit('mm'));
  cropRow2.appendChild(label('아래쪽'));
  dlg.picCropBottomInput = numberInput(0);
  dlg.picCropBottomInput.value = '0.00';
  cropRow2.appendChild(dlg.picCropBottomInput);
  cropRow2.appendChild(unit('mm'));
  cropFs.appendChild(cropRow2);

  // ── 그림 여백 ──
  const padFs = fieldset('그림 여백');
  panel.appendChild(padFs);
  const padRow1 = row();
  padRow1.appendChild(label('왼쪽'));
  dlg.picPadLeftInput = numberInput(0);
  dlg.picPadLeftInput.value = '0.00';
  padRow1.appendChild(dlg.picPadLeftInput);
  padRow1.appendChild(unit('mm'));
  padRow1.appendChild(label('위쪽'));
  dlg.picPadTopInput = numberInput(0);
  dlg.picPadTopInput.value = '0.00';
  padRow1.appendChild(dlg.picPadTopInput);
  padRow1.appendChild(unit('mm'));
  padRow1.appendChild(label('모두'));
  const padSync = numberInput(0);
  padSync.className = 'dialog-input pp-sync-spinner';
  padSync.addEventListener('input', () => {
    const v = padSync.value;
    dlg.picPadLeftInput.value = v;
    dlg.picPadTopInput.value = v;
    dlg.picPadRightInput.value = v;
    dlg.picPadBottomInput.value = v;
  });
  padRow1.appendChild(padSync);
  padFs.appendChild(padRow1);

  const padRow2 = row();
  padRow2.appendChild(label('오른쪽'));
  dlg.picPadRightInput = numberInput(0);
  dlg.picPadRightInput.value = '0.00';
  padRow2.appendChild(dlg.picPadRightInput);
  padRow2.appendChild(unit('mm'));
  padRow2.appendChild(label('아래쪽'));
  dlg.picPadBottomInput = numberInput(0);
  dlg.picPadBottomInput.value = '0.00';
  padRow2.appendChild(dlg.picPadBottomInput);
  padRow2.appendChild(unit('mm'));
  padFs.appendChild(padRow2);

  // ── 그림 효과 ──
  const effectFs = fieldset('그림 효과');
  panel.appendChild(effectFs);

  const effectMain = row();
  effectMain.style.alignItems = 'flex-start';

  // 좌측: 라디오 4개 (세로 배치)
  const radioCol = document.createElement('div');
  radioCol.className = 'pp-effect-radios';
  const effectNames = [
    { value: 'RealPic', label: '효과 없음' },
    { value: 'GrayScale', label: '회색조' },
    { value: 'BlackWhite', label: '흑백' },
    { value: 'Original', label: '원래 그림에서' },
  ];
  dlg.picEffectRadios = [];
  effectNames.forEach((e) => {
    const lbl = document.createElement('label');
    lbl.className = 'dialog-radio';
    const radio = document.createElement('input');
    radio.type = 'radio';
    radio.name = 'pp-pic-effect';
    radio.value = e.value;
    lbl.appendChild(radio);
    lbl.appendChild(document.createTextNode(` ${e.label}`));
    radioCol.appendChild(lbl);
    dlg.picEffectRadios.push(radio);
  });
  effectMain.appendChild(radioCol);

  // 우측: 밝기/대비/워터마크/반전
  const attrCol = document.createElement('div');
  attrCol.className = 'pp-effect-attrs';
  const brRow = row();
  brRow.appendChild(label('밝기'));
  dlg.picBrightnessInput = numberInput(-100, 100, 1);
  dlg.picBrightnessInput.value = '0';
  dlg.picBrightnessInput.style.width = '60px';
  brRow.appendChild(dlg.picBrightnessInput);
  brRow.appendChild(unit('%'));
  attrCol.appendChild(brRow);
  const ctRow = row();
  ctRow.appendChild(label('대비'));
  dlg.picContrastInput = numberInput(-100, 100, 1);
  dlg.picContrastInput.value = '0';
  dlg.picContrastInput.style.width = '60px';
  ctRow.appendChild(dlg.picContrastInput);
  ctRow.appendChild(unit('%'));
  attrCol.appendChild(ctRow);
  const wmLabel = checkboxLabel('워터마크 효과');
  dlg.picWatermarkCheck = wmLabel.querySelector('input') as HTMLInputElement;
  dlg.picWatermarkCheck.addEventListener('change', () => {
    if (dlg.picWatermarkCheck.checked) {
      dlg.picBrightnessInput.value = '70';
      dlg.picContrastInput.value = '-50';
    }
  });
  attrCol.appendChild(wmLabel);
  const invertLabel = checkboxLabel('그림 반전');
  const invertCheck = invertLabel.querySelector('input') as HTMLInputElement;
  invertCheck.disabled = true;
  attrCol.appendChild(invertLabel);
  effectMain.appendChild(attrCol);
  effectFs.appendChild(effectMain);

  // ── 투명도 설정 ──
  const transFs = fieldset('투명도 설정');
  panel.appendChild(transFs);
  const transRow = row();
  transRow.appendChild(label('투명도'));
  dlg.picTransparencyInput = numberInput(0, 100, 1);
  dlg.picTransparencyInput.value = '0';
  transRow.appendChild(dlg.picTransparencyInput);
  transRow.appendChild(unit('%'));
  transFs.appendChild(transRow);

  return panel;
}
