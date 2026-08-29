/** 표/셀 속성 대화상자 — 배경 탭 빌더 (table-cell-props-dialog.ts에서 분리) */
import type { TableCellPropsDialog } from '../../table-cell-props-dialog';
import { row, label } from '../../dialog-dom-helpers';

export function buildBackgroundTab(dlg: TableCellPropsDialog): HTMLElement {
  const frag = document.createElement('div');
  frag.className = 'tcp-tab-content';

  dlg.bgTarget = 'table';

  // 색 채우기
  const fillSection = dlg.createSection('채우기');

  const noneRow = row();
  dlg.bgNoneRadio = document.createElement('input');
  dlg.bgNoneRadio.type = 'radio';
  dlg.bgNoneRadio.name = 'bgFill';
  dlg.bgNoneRadio.checked = true;
  dlg.bgNoneRadio.addEventListener('change', () => dlg.updateBgPreview());
  noneRow.appendChild(dlg.bgNoneRadio);
  noneRow.appendChild(document.createTextNode(' 채우기 없음'));
  fillSection.appendChild(noneRow);

  const colorRow = row();
  dlg.bgColorRadio = document.createElement('input');
  dlg.bgColorRadio.type = 'radio';
  dlg.bgColorRadio.name = 'bgFill';
  dlg.bgColorRadio.addEventListener('change', () => dlg.updateBgPreview());
  colorRow.appendChild(dlg.bgColorRadio);
  colorRow.appendChild(document.createTextNode(' 색(Q)'));
  fillSection.appendChild(colorRow);

  // 면색 + 무늬색 + 무늬모양
  const colorFields = document.createElement('div');
  colorFields.style.marginLeft = '20px';

  const faceRow = row();
  faceRow.appendChild(label('면색(C)'));
  dlg.bgColorPicker = document.createElement('input');
  dlg.bgColorPicker.type = 'color';
  dlg.bgColorPicker.value = '#ffffff';
  dlg.bgColorPicker.style.width = '40px';
  dlg.bgColorPicker.style.height = '22px';
  dlg.bgColorPicker.addEventListener('input', () => {
    dlg.bgColorRadio.checked = true;
    dlg.updateBgPreview();
  });
  faceRow.appendChild(dlg.bgColorPicker);
  colorFields.appendChild(faceRow);

  const patColorRow = row();
  patColorRow.appendChild(label('무늬색(K)'));
  dlg.bgPatternColorPicker = document.createElement('input');
  dlg.bgPatternColorPicker.type = 'color';
  dlg.bgPatternColorPicker.value = '#000000';
  dlg.bgPatternColorPicker.style.width = '40px';
  dlg.bgPatternColorPicker.style.height = '22px';
  dlg.bgPatternColorPicker.addEventListener('input', () => {
    dlg.bgColorRadio.checked = true;
    dlg.updateBgPreview();
  });
  patColorRow.appendChild(dlg.bgPatternColorPicker);
  colorFields.appendChild(patColorRow);

  const patTypeRow = row();
  patTypeRow.appendChild(label('무늬모양(L)'));
  dlg.bgPatternTypeSelect = dlg.selectOptions([
    ['0', '없음'], ['1', '가로줄'], ['2', '세로줄'], ['3', '역슬래시'],
    ['4', '슬래시'], ['5', '십자'], ['6', 'X자'],
  ]);
  dlg.bgPatternTypeSelect.addEventListener('change', () => {
    dlg.bgColorRadio.checked = true;
    dlg.updateBgPreview();
  });
  patTypeRow.appendChild(dlg.bgPatternTypeSelect);
  colorFields.appendChild(patTypeRow);

  fillSection.appendChild(colorFields);

  // 미리보기
  dlg.bgPreviewBox = document.createElement('div');
  dlg.bgPreviewBox.className = 'tcp-bg-preview';
  fillSection.appendChild(dlg.bgPreviewBox);

  frag.appendChild(fillSection);

  // 그러데이션 (읽기 전용)
  const gradSection = dlg.createSection('그러데이션');
  gradSection.classList.add('disabled');
  const gradRow = row();
  gradRow.appendChild(label('유형'));
  const gradSelect = document.createElement('select');
  gradSelect.className = 'dialog-select';
  gradSelect.disabled = true;
  for (const text of ['선형', '방사형', '원뿔형', '사각형']) {
    const opt = document.createElement('option');
    opt.textContent = text;
    gradSelect.appendChild(opt);
  }
  gradRow.appendChild(gradSelect);
  gradSection.appendChild(gradRow);
  frag.appendChild(gradSection);

  // 그림 (읽기 전용)
  const imgSection = dlg.createSection('그림');
  imgSection.classList.add('disabled');
  const imgRow = row();
  imgRow.appendChild(label('그림 파일'));
  const imgBtn = document.createElement('button');
  imgBtn.type = 'button';
  imgBtn.className = 'dialog-btn';
  imgBtn.textContent = '열기...';
  imgBtn.disabled = true;
  imgRow.appendChild(imgBtn);
  imgSection.appendChild(imgRow);
  frag.appendChild(imgSection);

  return frag;
}
