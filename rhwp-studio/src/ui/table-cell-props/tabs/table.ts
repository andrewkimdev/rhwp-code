/** 표/셀 속성 대화상자 — 표 탭 빌더 (table-cell-props-dialog.ts에서 분리) */
import type { TableCellPropsDialog } from '../../table-cell-props-dialog';
import { row, label, unit } from '../../dialog-dom-helpers';

export function buildTableTab(dlg: TableCellPropsDialog): HTMLElement {
  const frag = document.createElement('div');
  frag.className = 'tcp-tab-content';

  // 여러 쪽 지원
  const pageSection = dlg.createSection('여러 쪽 지원');

  const pbRow = row();
  pbRow.appendChild(label('쪽 경계에서(Q)'));
  dlg.tablePageBreakSelect = dlg.selectOptions([
    ['2', '나눔'], ['1', '셀 단위로 나눔'], ['0', '나누지 않음'],
  ]);
  pbRow.appendChild(dlg.tablePageBreakSelect);
  pageSection.appendChild(pbRow);

  const rhRow = row();
  dlg.tableRepeatHeaderCheck = dlg.checkbox('제목 줄 자동 반복');
  rhRow.appendChild(dlg.tableRepeatHeaderCheck.parentElement!);
  pageSection.appendChild(rhRow);

  // 자동으로 나뉜 표의 경계선 설정
  const abRow = row();
  dlg.tableAutoBorderCheck = dlg.checkbox('자동으로 나뉜 표의 경계선 설정(J)');
  abRow.appendChild(dlg.tableAutoBorderCheck.parentElement!);
  pageSection.appendChild(abRow);

  dlg.tableAutoBorderFields = document.createElement('div');
  dlg.tableAutoBorderFields.className = 'tcp-disabled';
  const abLineRow = row();
  abLineRow.appendChild(label('종류(N)'));
  const abLineType = dlg.selectOptions([
    ['0', '없음'], ['1', '실선'], ['2', '파선'], ['3', '점선'],
    ['4', '일점쇄선'], ['5', '이점쇄선'], ['6', '긴 파선'], ['7', '이중 실선'],
  ]);
  abLineType.disabled = true;
  abLineRow.appendChild(abLineType);
  dlg.tableAutoBorderFields.appendChild(abLineRow);
  const abWidthRow = row();
  abWidthRow.appendChild(label('굵기(H)'));
  const abWidth = dlg.selectOptions([
    ['0', '0.1mm'], ['1', '0.12mm'], ['2', '0.15mm'], ['3', '0.2mm'],
    ['4', '0.25mm'], ['5', '0.3mm'], ['6', '0.4mm'],
  ]);
  abWidth.disabled = true;
  abWidthRow.appendChild(abWidth);
  dlg.tableAutoBorderFields.appendChild(abWidthRow);
  const abColorRow = row();
  abColorRow.appendChild(label('색(S)'));
  const abColor = document.createElement('input');
  abColor.type = 'color';
  abColor.value = '#000000';
  abColor.disabled = true;
  abColor.style.width = '40px';
  abColor.style.height = '22px';
  abColorRow.appendChild(abColor);
  dlg.tableAutoBorderFields.appendChild(abColorRow);
  pageSection.appendChild(dlg.tableAutoBorderFields);

  dlg.tableAutoBorderCheck.addEventListener('change', () => {
    const enabled = dlg.tableAutoBorderCheck.checked;
    dlg.tableAutoBorderFields.classList.toggle('tcp-disabled', !enabled);
    abLineType.disabled = !enabled;
    abWidth.disabled = !enabled;
    abColor.disabled = !enabled;
  });

  frag.appendChild(pageSection);

  // 모든 셀 안 여백
  const padSection = dlg.createSection('모든 셀의 안 여백');
  const padRow = document.createElement('div');
  padRow.className = 'tcp-margin-row';
  const padGrid = document.createElement('div');
  padGrid.className = 'dialog-margin-grid';
  dlg.tablePaddingInputs = {};
  for (const [key, text] of [['left', '왼쪽'], ['right', '오른쪽'], ['top', '위쪽'], ['bottom', '아래쪽']] as const) {
    padGrid.appendChild(label(text));
    dlg.tablePaddingInputs[key] = dlg.tabNumberInput();
    padGrid.appendChild(dlg.tablePaddingInputs[key]);
    padGrid.appendChild(unit('mm'));
  }
  padRow.appendChild(padGrid);
  padRow.appendChild(dlg.buildAllSpinner(dlg.tablePaddingInputs));
  padSection.appendChild(padRow);
  frag.appendChild(padSection);

  return frag;
}
