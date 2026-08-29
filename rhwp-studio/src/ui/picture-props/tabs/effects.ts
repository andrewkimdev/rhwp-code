/** 개체 속성 대화상자 — 반사/네온/열은 테두리 탭 + 미구현 스텁 빌더 (picture-props-dialog.ts에서 분리) */
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { fieldset, row, label, unit, numberInput, colorInput as mkColorInput, checkboxLabel } from '../../dialog-dom-helpers';

export function buildReflectionTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  const fs = fieldset('반사 효과');
  panel.appendChild(fs);

  const noneLabel = checkboxLabel('반사 없음');
  const noneCheck = noneLabel.querySelector('input') as HTMLInputElement;
  noneCheck.checked = true;
  fs.appendChild(noneLabel);

  // 3×5 프리셋 그리드 (비활성)
  const grid = document.createElement('div');
  grid.className = 'pp-preset-grid pp-reflect-grid';
  for (let i = 0; i < 15; i++) {
    const btn = document.createElement('button');
    btn.className = 'pp-preset-btn';
    btn.textContent = '🖼';
    btn.disabled = true;
    grid.appendChild(btn);
  }
  fs.appendChild(grid);

  // 속성
  const sizeRow = row();
  sizeRow.appendChild(label('크기'));
  const sizeSlider = document.createElement('input');
  sizeSlider.type = 'range';
  sizeSlider.className = 'pp-slider';
  sizeSlider.disabled = true;
  sizeRow.appendChild(sizeSlider);
  const sizeInput = numberInput(0, 100, 1);
  sizeInput.disabled = true;
  sizeRow.appendChild(sizeInput);
  fs.appendChild(sizeRow);

  const distRow = row();
  distRow.appendChild(label('거리'));
  const distSlider = document.createElement('input');
  distSlider.type = 'range';
  distSlider.className = 'pp-slider';
  distSlider.disabled = true;
  distRow.appendChild(distSlider);
  const distInput = numberInput(0, 100, 1);
  distInput.disabled = true;
  distRow.appendChild(distInput);
  distRow.appendChild(unit('pt'));
  fs.appendChild(distRow);

  return panel;
}

export function buildGlowTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  const fs = fieldset('네온 효과');
  panel.appendChild(fs);

  const noneLabel = checkboxLabel('네온 없음');
  const noneCheck = noneLabel.querySelector('input') as HTMLInputElement;
  noneCheck.checked = true;
  fs.appendChild(noneLabel);

  // 3×6 프리셋 그리드
  const grid = document.createElement('div');
  grid.className = 'pp-preset-grid pp-glow-grid';
  for (let i = 0; i < 18; i++) {
    const btn = document.createElement('button');
    btn.className = 'pp-preset-btn';
    btn.textContent = '🖼';
    btn.disabled = true;
    grid.appendChild(btn);
  }
  fs.appendChild(grid);

  // 속성
  const colorRow = row();
  colorRow.appendChild(label('색'));
  const colorInput = mkColorInput('#ffff00');
  colorInput.disabled = true;
  colorRow.appendChild(colorInput);
  fs.appendChild(colorRow);

  const transRow = row();
  transRow.appendChild(label('투명도'));
  const transSlider = document.createElement('input');
  transSlider.type = 'range';
  transSlider.className = 'pp-slider';
  transSlider.disabled = true;
  transRow.appendChild(transSlider);
  const transInput = numberInput(0, 100, 1);
  transInput.disabled = true;
  transRow.appendChild(transInput);
  fs.appendChild(transRow);

  const sizeRow = row();
  sizeRow.appendChild(label('크기'));
  const sizeSlider = document.createElement('input');
  sizeSlider.type = 'range';
  sizeSlider.className = 'pp-slider';
  sizeSlider.disabled = true;
  sizeRow.appendChild(sizeSlider);
  const sizeInput = numberInput(0, 100, 1);
  sizeInput.disabled = true;
  sizeRow.appendChild(sizeInput);
  sizeRow.appendChild(unit('pt'));
  fs.appendChild(sizeRow);

  return panel;
}

export function buildSoftEdgeTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  const fs = fieldset('열은 테두리 효과');
  panel.appendChild(fs);

  const noneLabel = checkboxLabel('열은 테두리 없음');
  fs.appendChild(noneLabel);

  // 6개 프리셋 버튼
  const grid = document.createElement('div');
  grid.className = 'pp-preset-grid pp-softedge-grid';
  for (let i = 0; i < 6; i++) {
    const btn = document.createElement('button');
    btn.className = 'pp-preset-btn';
    btn.textContent = '🖼';
    btn.disabled = true;
    grid.appendChild(btn);
  }
  fs.appendChild(grid);

  // 크기 슬라이더
  const sizeRow = row();
  sizeRow.appendChild(label('크기'));
  const sizeSlider = document.createElement('input');
  sizeSlider.type = 'range';
  sizeSlider.className = 'pp-slider';
  sizeSlider.min = '0';
  sizeSlider.max = '50';
  sizeSlider.value = '3';
  sizeSlider.disabled = true;
  sizeRow.appendChild(sizeSlider);
  const sizeInput = numberInput(0, 50, 0.1);
  sizeInput.value = '3.0';
  sizeInput.disabled = true;
  sizeRow.appendChild(sizeInput);
  sizeRow.appendChild(unit('pt'));
  fs.appendChild(sizeRow);

  return panel;
}

/** 미구현 탭 스텁 패널 */
export function buildStubTab(dlg: PicturePropsDialog, name: string): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';
  const msg = document.createElement('div');
  msg.className = 'pp-stub-msg';
  msg.textContent = `[${name}] 탭은 추후 구현 예정입니다.`;
  panel.appendChild(msg);
  return panel;
}
