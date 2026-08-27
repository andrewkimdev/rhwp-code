/** 표/셀 속성 대화상자 — 셀 탭 빌더 (table-cell-props-dialog.ts에서 분리) */
import type { TableCellPropsDialog } from '../../table-cell-props-dialog';
import { row, label, unit } from '../../dialog-dom-helpers';

export function buildCellTab(dlg: TableCellPropsDialog): HTMLElement {
  const frag = document.createElement('div');
  frag.className = 'tcp-tab-content';

  // 셀 크기
  const sizeSection = dlg.createSection('셀 크기');
  const sizeCheck = row();
  dlg.cellApplySizeCheck = dlg.checkbox('셀 크기 적용');
  sizeCheck.appendChild(dlg.cellApplySizeCheck.parentElement!);
  sizeSection.appendChild(sizeCheck);

  const sizeRow = row();
  sizeRow.appendChild(label('너비'));
  dlg.cellWidthInput = dlg.tabNumberInput();
  sizeRow.appendChild(dlg.cellWidthInput);
  sizeRow.appendChild(unit('mm'));
  sizeRow.appendChild(label('높이'));
  dlg.cellHeightInput = dlg.tabNumberInput();
  sizeRow.appendChild(dlg.cellHeightInput);
  sizeRow.appendChild(unit('mm'));
  sizeSection.appendChild(sizeRow);
  dlg.cellApplySizeCheck.addEventListener('change', () => dlg.updateCellSizeState());
  frag.appendChild(sizeSection);

  // 안 여백
  const padSection = dlg.createSection('안 여백');
  const padCheck = row();
  dlg.cellPaddingCheck = dlg.checkbox('안 여백 지정');
  padCheck.appendChild(dlg.cellPaddingCheck.parentElement!);
  padSection.appendChild(padCheck);

  const padRow = document.createElement('div');
  padRow.className = 'tcp-margin-row';
  const padGrid = document.createElement('div');
  padGrid.className = 'dialog-margin-grid';
  dlg.cellPaddingInputs = {};
  for (const [key, text] of [['left', '왼쪽'], ['right', '오른쪽'], ['top', '위쪽'], ['bottom', '아래쪽']] as const) {
    padGrid.appendChild(label(text));
    dlg.cellPaddingInputs[key] = dlg.tabNumberInput();
    padGrid.appendChild(dlg.cellPaddingInputs[key]);
    padGrid.appendChild(unit('mm'));
  }
  padRow.appendChild(padGrid);
  padRow.appendChild(dlg.buildAllSpinner(dlg.cellPaddingInputs));
  padSection.appendChild(padRow);
  dlg.cellPaddingCheck.addEventListener('change', () => dlg.updateCellPaddingState());
  frag.appendChild(padSection);

  // 속성
  const attrSection = dlg.createSection('속성');

  // 세로 정렬
  const valignRow = row();
  valignRow.appendChild(label('세로 정렬'));
  const valignGroup = document.createElement('div');
  valignGroup.className = 'dialog-btn-group';
  dlg.cellVAlignBtns = ['위쪽', '가운데', '아래쪽'].map((text, i) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = text;
    btn.addEventListener('click', () => dlg.setButtonGroupActive(dlg.cellVAlignBtns, i));
    valignGroup.appendChild(btn);
    return btn;
  });
  valignRow.appendChild(valignGroup);
  attrSection.appendChild(valignRow);

  // 세로쓰기
  const tdirRow = row();
  tdirRow.appendChild(label('세로쓰기'));
  const tdirGroup = document.createElement('div');
  tdirGroup.className = 'dialog-btn-group';
  dlg.cellTextDirBtns = ['가로쓰기', '세로쓰기'].map((text, i) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = text;
    btn.addEventListener('click', () => {
      dlg.setButtonGroupActive(dlg.cellTextDirBtns, i);
      vertSubRow.classList.toggle('tcp-disabled', i === 0);
    });
    tdirGroup.appendChild(btn);
    return btn;
  });
  tdirRow.appendChild(tdirGroup);
  attrSection.appendChild(tdirRow);

  // 세로쓰기 하위: 문 눕힘/문 세움
  const vertSubRow = row();
  vertSubRow.className = 'dialog-row tcp-disabled';
  vertSubRow.appendChild(label(''));
  const vertSubGroup = document.createElement('div');
  vertSubGroup.className = 'dialog-btn-group';
  const vertSubLabels = ['문 눕힘(Q)', '문 세움(U)'];
  vertSubLabels.forEach((text, i) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = text;
    btn.addEventListener('click', () => {
      vertSubGroup.querySelectorAll('button').forEach((b, j) =>
        b.classList.toggle('active', j === i));
    });
    vertSubGroup.appendChild(btn);
  });
  // 기본: 문 눕힘 활성
  (vertSubGroup.firstChild as HTMLButtonElement)?.classList.add('active');
  vertSubRow.appendChild(vertSubGroup);
  attrSection.appendChild(vertSubRow);

  // 체크박스 옵션들
  const optRow1 = row();
  dlg.cellSingleLineCheck = dlg.checkbox('한 줄로 입력(S)');
  optRow1.appendChild(dlg.cellSingleLineCheck.parentElement!);
  dlg.cellProtectCheck = dlg.checkbox('셀 보호');
  optRow1.appendChild(dlg.cellProtectCheck.parentElement!);
  attrSection.appendChild(optRow1);

  const optRow2 = row();
  dlg.cellHeaderCheck = dlg.checkbox('제목 셀');
  optRow2.appendChild(dlg.cellHeaderCheck.parentElement!);
  attrSection.appendChild(optRow2);

  frag.appendChild(attrSection);

  // 필드
  const fieldSection = dlg.createSection('필드');
  const fieldRow = row();
  fieldRow.appendChild(label('필드 이름'));
  dlg.cellFieldNameInput = document.createElement('input');
  dlg.cellFieldNameInput.type = 'text';
  dlg.cellFieldNameInput.className = 'dialog-text-input';
  fieldRow.appendChild(dlg.cellFieldNameInput);
  fieldSection.appendChild(fieldRow);

  const fieldRow2 = row();
  dlg.cellEditableCheck = dlg.checkbox('양식 모드에서 편집 가능');
  fieldRow2.appendChild(dlg.cellEditableCheck.parentElement!);
  fieldSection.appendChild(fieldRow2);

  frag.appendChild(fieldSection);

  return frag;
}
