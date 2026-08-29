/** 표/셀 속성 대화상자 — 테두리 탭 빌더 (table-cell-props-dialog.ts에서 분리) */
import type { TableCellPropsDialog } from '../../table-cell-props-dialog';
import { row, label, unit } from '../../dialog-dom-helpers';
import { LINE_SAMPLE_STROKE } from '../../table-cell-props-units';

export function buildBorderTab(dlg: TableCellPropsDialog): HTMLElement {
  const frag = document.createElement('div');
  frag.className = 'tcp-tab-content';

  dlg.borderTarget = 'table';

  // ── 선 종류 시각적 격자 ──
  const lineSection = dlg.createSection('선 종류(Y)');
  dlg.borderLineTypeGrid = document.createElement('div');
  dlg.borderLineTypeGrid.className = 'tcp-line-type-grid';
  const lineTypeDefs = [
    { type: 0, label: '없음' },
    { type: 1, dash: '' },        // 실선
    { type: 2, dash: '6,3' },     // 파선
    { type: 3, dash: '2,2' },     // 점선
    { type: 4, dash: '8,3,2,3' }, // 일점쇄선
    { type: 5, dash: '8,3,2,3,2,3' }, // 이점쇄선
    { type: 6, dash: '12,3' },    // 긴 파선
    { type: 8, label: '이중' },   // 이중 실선 (HWP Double=8)
  ];
  lineTypeDefs.forEach(def => {
    const item = document.createElement('div');
    item.className = 'tcp-line-type-item';
    if (def.type === 1) item.classList.add('active');
    if (def.type === 0) {
      const span = document.createElement('span');
      span.className = 'tcp-line-type-none';
      span.textContent = '없음';
      item.appendChild(span);
    } else if (def.type === 8) {
      // 이중 실선 SVG
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('viewBox', '0 0 48 10');
      const l1 = document.createElementNS('http://www.w3.org/2000/svg', 'line');
      l1.setAttribute('x1', '0'); l1.setAttribute('y1', '3');
      l1.setAttribute('x2', '48'); l1.setAttribute('y2', '3');
      l1.setAttribute('stroke', LINE_SAMPLE_STROKE); l1.setAttribute('stroke-width', '1');
      const l2 = document.createElementNS('http://www.w3.org/2000/svg', 'line');
      l2.setAttribute('x1', '0'); l2.setAttribute('y1', '7');
      l2.setAttribute('x2', '48'); l2.setAttribute('y2', '7');
      l2.setAttribute('stroke', LINE_SAMPLE_STROKE); l2.setAttribute('stroke-width', '1');
      svg.appendChild(l1); svg.appendChild(l2);
      item.appendChild(svg);
    } else {
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('viewBox', '0 0 48 10');
      const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
      line.setAttribute('x1', '0'); line.setAttribute('y1', '5');
      line.setAttribute('x2', '48'); line.setAttribute('y2', '5');
      line.setAttribute('stroke', LINE_SAMPLE_STROKE); line.setAttribute('stroke-width', '1.5');
      if (def.dash) line.setAttribute('stroke-dasharray', def.dash);
      svg.appendChild(line);
      item.appendChild(svg);
    }
    item.addEventListener('click', () => {
      dlg.borderLineTypeGrid.querySelectorAll('.tcp-line-type-item').forEach(el =>
        el.classList.remove('active'));
      item.classList.add('active');
      dlg.borderSelectedLineType = def.type;
    });
    dlg.borderLineTypeGrid.appendChild(item);
  });
  lineSection.appendChild(dlg.borderLineTypeGrid);
  frag.appendChild(lineSection);

  // ── 굵기 + 색 ──
  const attrSection = dlg.createSection('선 속성');
  const widthRow = row();
  widthRow.appendChild(label('굵기'));
  dlg.borderWidthSelect = document.createElement('select');
  dlg.borderWidthSelect.className = 'dialog-select';
  const widths = ['0.1mm', '0.12mm', '0.15mm', '0.2mm', '0.25mm', '0.3mm', '0.4mm'];
  widths.forEach((text, i) => {
    const opt = document.createElement('option');
    opt.value = String(i); opt.textContent = text;
    dlg.borderWidthSelect.appendChild(opt);
  });
  widthRow.appendChild(dlg.borderWidthSelect);
  attrSection.appendChild(widthRow);

  const colorRow = row();
  colorRow.appendChild(label('색'));
  dlg.borderColorInput = document.createElement('input');
  dlg.borderColorInput.type = 'color';
  dlg.borderColorInput.value = '#000000';
  dlg.borderColorInput.style.width = '40px';
  dlg.borderColorInput.style.height = '22px';
  colorRow.appendChild(dlg.borderColorInput);
  attrSection.appendChild(colorRow);
  frag.appendChild(attrSection);

  // ── 미리보기 + 방향 버튼 (그리드 배치) ──
  const previewSection = dlg.createSection('미리 보기');
  const previewWrap = document.createElement('div');
  previewWrap.className = 'tcp-border-preview-wrap';

  // 방향 버튼: 모두(좌상), 위(상중), 왼(좌중), SVG(중중), 오른(우중), 아래(하중)
  const mkDirBtn = (text: string, cls: string, dirIdx: number) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = `tcp-dir-btn ${cls}`;
    btn.textContent = text;
    btn.addEventListener('click', () => dlg.applyBorderToDirection(dirIdx));
    return btn;
  };
  previewWrap.appendChild(mkDirBtn('O', 'tcp-dir-all', 4));   // 모두
  previewWrap.appendChild(mkDirBtn('▲', 'tcp-dir-top', 2));   // 위
  previewWrap.appendChild(document.createElement('span'));      // 우상 빈칸
  previewWrap.appendChild(mkDirBtn('◀', 'tcp-dir-left', 0));  // 왼
  // SVG 미리보기
  dlg.borderPreviewSvg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  dlg.borderPreviewSvg.classList.add('tcp-border-preview-svg');
  dlg.borderPreviewSvg.setAttribute('viewBox', '0 0 120 100');
  previewWrap.appendChild(dlg.borderPreviewSvg);
  previewWrap.appendChild(mkDirBtn('▶', 'tcp-dir-right', 1)); // 오른
  previewWrap.appendChild(document.createElement('span'));      // 좌하 빈칸
  previewWrap.appendChild(mkDirBtn('▼', 'tcp-dir-bottom', 3));// 아래
  previewSection.appendChild(previewWrap);

  // 선 모양 바로 적용
  const immediateRow = row();
  dlg.borderApplyImmediateCheck = dlg.checkbox('선 모양 바로 적용(I)');
  immediateRow.appendChild(dlg.borderApplyImmediateCheck.parentElement!);
  previewSection.appendChild(immediateRow);

  frag.appendChild(previewSection);

  // ── 셀 간격 ──
  const spacingSection = dlg.createSection('셀 간격');
  const spacingRow = row();
  spacingRow.appendChild(label('셀 간격'));
  dlg.borderCellSpacingInput = dlg.tabNumberInput();
  spacingRow.appendChild(dlg.borderCellSpacingInput);
  spacingRow.appendChild(unit('mm'));
  spacingSection.appendChild(spacingRow);

  const noteDiv = document.createElement('div');
  noteDiv.className = 'tcp-note';
  noteDiv.textContent = '※ 표 테두리는 [셀 간격]에 값을 입력해야 나타납니다';
  spacingSection.appendChild(noteDiv);
  frag.appendChild(spacingSection);

  // ── 자동 나뉜 표 경계선 설정 ──
  const abSection = dlg.createSection('자동 경계선');
  const abRow = row();
  dlg.borderAutoBorderCheck = dlg.checkbox('자동으로 나뉜 표의 경계선 설정(J)');
  abRow.appendChild(dlg.borderAutoBorderCheck.parentElement!);
  abSection.appendChild(abRow);

  dlg.borderAutoBorderFields = document.createElement('div');
  dlg.borderAutoBorderFields.className = 'tcp-disabled';
  const abLineRow = row();
  abLineRow.appendChild(label('종류'));
  const abLineType = dlg.selectOptions([
    ['0', '없음'], ['1', '실선'], ['2', '파선'], ['3', '점선'],
    ['4', '일점쇄선'], ['5', '이점쇄선'], ['6', '긴 파선'], ['7', '이중 실선'],
  ]);
  abLineType.disabled = true;
  abLineRow.appendChild(abLineType);
  dlg.borderAutoBorderFields.appendChild(abLineRow);
  const abWidthRow = row();
  abWidthRow.appendChild(label('굵기'));
  const abWidth = dlg.selectOptions([
    ['0', '0.1mm'], ['1', '0.12mm'], ['2', '0.15mm'], ['3', '0.2mm'],
    ['4', '0.25mm'], ['5', '0.3mm'], ['6', '0.4mm'],
  ]);
  abWidth.disabled = true;
  abWidthRow.appendChild(abWidth);
  dlg.borderAutoBorderFields.appendChild(abWidthRow);
  const abColorRow = row();
  abColorRow.appendChild(label('색'));
  const abColor = document.createElement('input');
  abColor.type = 'color'; abColor.value = '#000000';
  abColor.disabled = true;
  abColor.style.width = '40px'; abColor.style.height = '22px';
  abColorRow.appendChild(abColor);
  dlg.borderAutoBorderFields.appendChild(abColorRow);
  abSection.appendChild(dlg.borderAutoBorderFields);

  dlg.borderAutoBorderCheck.addEventListener('change', () => {
    const en = dlg.borderAutoBorderCheck.checked;
    dlg.borderAutoBorderFields.classList.toggle('tcp-disabled', !en);
    abLineType.disabled = !en; abWidth.disabled = !en; abColor.disabled = !en;
  });
  frag.appendChild(abSection);

  // 초기 편집 상태
  dlg.borderEdits = [
    { type: 1, width: 0, color: '#000000' },
    { type: 1, width: 0, color: '#000000' },
    { type: 1, width: 0, color: '#000000' },
    { type: 1, width: 0, color: '#000000' },
  ];

  return frag;
}
