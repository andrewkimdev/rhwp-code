/** 표/셀 속성 대화상자 — 여백/캡션 탭 빌더 (table-cell-props-dialog.ts에서 분리) */
import type { TableCellPropsDialog } from '../../table-cell-props-dialog';
import { row, label, unit } from '../../dialog-dom-helpers';
import { appendSvgMarkup } from '../../dom-utils';

export function buildMarginTab(dlg: TableCellPropsDialog): HTMLElement {
  const frag = document.createElement('div');
  frag.className = 'tcp-tab-content';

  // 바깥 여백 (활성)
  const outerSection = dlg.createSection('바깥 여백');
  const outerRow = document.createElement('div');
  outerRow.className = 'tcp-margin-row';
  const outerGrid = document.createElement('div');
  outerGrid.className = 'dialog-margin-grid';
  dlg.marginOuterInputs = {};
  for (const [key, text] of [['left', '왼쪽'], ['right', '오른쪽'], ['top', '위쪽'], ['bottom', '아래쪽']] as const) {
    outerGrid.appendChild(label(text));
    dlg.marginOuterInputs[key] = dlg.tabNumberInput();
    outerGrid.appendChild(dlg.marginOuterInputs[key]);
    outerGrid.appendChild(unit('mm'));
  }
  outerRow.appendChild(outerGrid);
  outerRow.appendChild(dlg.buildAllSpinner(dlg.marginOuterInputs));
  outerSection.appendChild(outerRow);
  frag.appendChild(outerSection);

  // 캡션 넣기
  dlg.captionSection = dlg.createSection('캡션');

  // 캡션 하위 필드 래퍼 (가운데 선택 시 비활성)
  dlg.captionFieldsWrap = document.createElement('div');

  // 캡션 위치 3×3 아이콘 그리드 (가운데 = 캡션 없음)
  const captionGrid = document.createElement('div');
  captionGrid.className = 'tcp-caption-grid';
  dlg.captionPosBtns = [];
  // 3×3: 행(위/가운데/아래) × 열(왼쪽/가운데/오른쪽)
  // dir: 0=왼쪽, 1=오른쪽, 2=위, 3=아래, -1=없음(가운데)
  const capPositions = [
    { dir: 0, sub: 0, svg: captionSvg('left-top') },   // 왼쪽 위
    { dir: 2, sub: 0, svg: captionSvg('top') },        // 위
    { dir: 1, sub: 0, svg: captionSvg('right-top') },  // 오른쪽 위
    { dir: 0, sub: 1, svg: captionSvg('left-mid') },   // 왼쪽 가운데
    { dir: -1, sub: 0, svg: captionSvg('none') },      // 가운데 = 캡션 없음
    { dir: 1, sub: 1, svg: captionSvg('right-mid') },  // 오른쪽 가운데
    { dir: 0, sub: 2, svg: captionSvg('left-bot') },   // 왼쪽 아래
    { dir: 3, sub: 0, svg: captionSvg('bottom') },     // 아래
    { dir: 1, sub: 2, svg: captionSvg('right-bot') },  // 오른쪽 아래
  ];
  capPositions.forEach((pos) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'tcp-caption-item';
    appendSvgMarkup(btn, pos.svg);
    btn.dataset.dir = String(pos.dir);
    btn.dataset.sub = String(pos.sub);
    btn.addEventListener('click', () => {
      dlg.captionPosBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      const isNone = pos.dir === -1;
      dlg.captionFieldsWrap.classList.toggle('tcp-disabled', isNone);
      if (!isNone) {
        dlg.captionDirSelect.value = String(pos.dir);
      }
      dlg.updateCaptionWidthState();
    });
    captionGrid.appendChild(btn);
    dlg.captionPosBtns.push(btn);
  });
  dlg.captionSection.appendChild(captionGrid);

  // 숨겨진 방향 select (내부 값 관리용)
  dlg.captionDirSelect = document.createElement('select');
  dlg.captionDirSelect.className = 'dialog-select';
  dlg.captionDirSelect.style.display = 'none';
  const capDirs = [
    [0, '왼쪽'], [1, '오른쪽'], [2, '위쪽'], [3, '아래쪽'],
  ] as const;
  for (const [val, text] of capDirs) {
    const opt = document.createElement('option');
    opt.value = String(val);
    opt.textContent = text;
    dlg.captionDirSelect.appendChild(opt);
  }
  dlg.captionFieldsWrap.appendChild(dlg.captionDirSelect);

  const capGapRow = row();
  capGapRow.appendChild(label('간격'));
  dlg.captionSpacingInput = dlg.tabNumberInput();
  capGapRow.appendChild(dlg.captionSpacingInput);
  capGapRow.appendChild(unit('mm'));
  dlg.captionFieldsWrap.appendChild(capGapRow);

  const capSizeRow = row();
  capSizeRow.appendChild(label('캡션 크기(S)'));
  dlg.captionWidthInput = dlg.tabNumberInput();
  capSizeRow.appendChild(dlg.captionWidthInput);
  capSizeRow.appendChild(unit('mm'));
  dlg.captionFieldsWrap.appendChild(capSizeRow);

  const capExpandRow = row();
  dlg.captionExpandCheck = dlg.checkbox('여백 부분까지 너비 확대(W)');
  capExpandRow.appendChild(dlg.captionExpandCheck.parentElement!);
  dlg.captionFieldsWrap.appendChild(capExpandRow);

  dlg.captionSection.appendChild(dlg.captionFieldsWrap);
  frag.appendChild(dlg.captionSection);

  return frag;
}

/** 캡션 위치별 간단한 SVG 아이콘 */
function captionSvg(pos: string): string {
  const table = '<rect x="12" y="8" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>';
  const capH = '<rect fill="#ffd966" stroke="#c09000" stroke-width="0.5" rx="1"';
  const capV = '<rect fill="#ffd966" stroke="#c09000" stroke-width="0.5" rx="1"';
  const w = 54, h = 40;
  let inner = '';
  switch (pos) {
    case 'top':    inner = `${capH} x="12" y="2" width="30" height="5"/><rect x="12" y="10" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>`; break;
    case 'bottom': inner = `${table}${capH} x="12" y="32" width="30" height="5"/>`; break;
    case 'left-top':  inner = `<rect x="18" y="8" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>${capV} x="4" y="8" width="12" height="6"/>`; break;
    case 'left-mid':  inner = `<rect x="18" y="8" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>${capV} x="4" y="14" width="12" height="6"/>`; break;
    case 'left-bot':  inner = `<rect x="18" y="8" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>${capV} x="4" y="24" width="12" height="6"/>`; break;
    case 'right-top': inner = `<rect x="4" y="8" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>${capV} x="38" y="8" width="12" height="6"/>`; break;
    case 'right-mid': inner = `<rect x="4" y="8" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>${capV} x="38" y="14" width="12" height="6"/>`; break;
    case 'right-bot': inner = `<rect x="4" y="8" width="30" height="22" rx="1" fill="#d0d8e8" stroke="#6182d6" stroke-width="0.5"/>${capV} x="38" y="24" width="12" height="6"/>`; break;
    case 'none': inner = `${table}<line x1="10" y1="6" x2="44" y2="34" stroke="#c00" stroke-width="1.5"/>`; break;
    default: inner = table;
  }
  return `<svg viewBox="0 0 ${w} ${h}" xmlns="http://www.w3.org/2000/svg">${inner}</svg>`;
}
