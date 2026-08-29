/** 표/셀 속성 대화상자 — 기본 탭 빌더 (table-cell-props-dialog.ts에서 분리) */
import type { TableCellPropsDialog } from '../../table-cell-props-dialog';
import { row, label, unit } from '../../dialog-dom-helpers';

export function buildBasicTab(dlg: TableCellPropsDialog): HTMLElement {
  const frag = document.createElement('div');
  frag.className = 'tcp-tab-content';

  // ── 크기 ──
  const sizeSection = dlg.createSection('크기');
  const sizeRow = row();
  sizeRow.appendChild(label('너비'));
  dlg.basicWidthInput = dlg.tabNumberInput();
  sizeRow.appendChild(dlg.basicWidthInput);
  sizeRow.appendChild(unit('mm'));
  sizeRow.appendChild(label('높이'));
  dlg.basicHeightInput = dlg.tabNumberInput();
  sizeRow.appendChild(dlg.basicHeightInput);
  sizeRow.appendChild(unit('mm'));
  sizeSection.appendChild(sizeRow);
  const sizeNote = document.createElement('div');
  sizeNote.className = 'tcp-note';
  sizeNote.textContent = '※ 표 크기는 읽기 전용입니다 (셀 크기의 합)';
  sizeSection.appendChild(sizeNote);
  frag.appendChild(sizeSection);

  // ── 위치 ──
  const posSection = dlg.createSection('위치');

  // 글자처럼 취급 체크박스
  const tacRow = row();
  dlg.treatAsCharCheck = dlg.checkbox('글자처럼 취급');
  tacRow.appendChild(dlg.treatAsCharCheck.parentElement!);
  posSection.appendChild(tacRow);
  dlg.treatAsCharCheck.addEventListener('change', () => dlg.updatePositionVisibility());

  // ── 본문과의 배치 그룹 (글자처럼 취급 해제 시 활성) ──
  dlg.posGroup = document.createElement('div');
  dlg.posGroup.className = 'dialog-pos-group';

  // 본문과의 배치 (버튼 4개)
  const wrapRow = row();
  wrapRow.appendChild(label('본문과의 배치'));
  const wrapGroup = document.createElement('div');
  wrapGroup.className = 'dialog-btn-group';
  dlg.wrapBtns = [];
  const wrapLabels = ['어울림', '자리 차지', '글 뒤로', '글 앞으로'];
  wrapLabels.forEach((text, i) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'dialog-btn';
    btn.textContent = text;
    btn.addEventListener('click', () => dlg.selectWrap(i));
    wrapGroup.appendChild(btn);
    dlg.wrapBtns.push(btn);
  });
  wrapRow.appendChild(wrapGroup);
  dlg.posGroup.appendChild(wrapRow);

  // 가로 위치
  const hRow = row();
  hRow.appendChild(label('가로'));
  dlg.horzRelSelect = dlg.selectOptions([
    ['Paper', '종이'], ['Page', '쪽'], ['Column', '단'], ['Para', '문단'],
  ]);
  hRow.appendChild(dlg.horzRelSelect);
  hRow.appendChild(unit('의'));
  dlg.horzAlignSelect = dlg.selectOptions([
    ['Left', '왼쪽'], ['Center', '가운데'], ['Right', '오른쪽'],
    ['Inside', '안쪽'], ['Outside', '바깥쪽'],
  ]);
  hRow.appendChild(dlg.horzAlignSelect);
  hRow.appendChild(unit('기준'));
  dlg.horzOffsetInput = dlg.tabNumberInput();
  hRow.appendChild(dlg.horzOffsetInput);
  hRow.appendChild(unit('mm'));
  dlg.posGroup.appendChild(hRow);

  // 세로 위치
  const vRow = row();
  vRow.appendChild(label('세로'));
  dlg.vertRelSelect = dlg.selectOptions([
    ['Paper', '종이'], ['Page', '쪽'], ['Para', '문단'],
  ]);
  vRow.appendChild(dlg.vertRelSelect);
  vRow.appendChild(unit('의'));
  dlg.vertAlignSelect = dlg.selectOptions([
    ['Top', '위'], ['Center', '가운데'], ['Bottom', '아래'],
    ['Inside', '안쪽'], ['Outside', '바깥쪽'],
  ]);
  vRow.appendChild(dlg.vertAlignSelect);
  vRow.appendChild(unit('기준'));
  dlg.vertOffsetInput = dlg.tabNumberInput();
  vRow.appendChild(dlg.vertOffsetInput);
  vRow.appendChild(unit('mm'));
  dlg.posGroup.appendChild(vRow);

  // 체크박스 옵션들
  const optRow = row();
  dlg.restrictInPageCheck = dlg.checkbox('쪽 영역 안으로 제한');
  optRow.appendChild(dlg.restrictInPageCheck.parentElement!);
  dlg.allowOverlapCheck = dlg.checkbox('서로 겹침 허용');
  optRow.appendChild(dlg.allowOverlapCheck.parentElement!);
  dlg.posGroup.appendChild(optRow);

  const anchorRow = row();
  dlg.keepWithAnchorCheck = dlg.checkbox('개체와 조판부호를 항상 같은 쪽에 놓기');
  anchorRow.appendChild(dlg.keepWithAnchorCheck.parentElement!);
  dlg.posGroup.appendChild(anchorRow);

  posSection.appendChild(dlg.posGroup);
  frag.appendChild(posSection);

  // ── 개체 회전 ──
  const rotSection = dlg.createSection('개체 회전');
  const rotRow = row();
  rotRow.appendChild(label('회전각'));
  const rotInput = dlg.tabNumberInput();
  rotInput.disabled = true;
  rotInput.value = '0';
  rotRow.appendChild(rotInput);
  rotRow.appendChild(unit('°'));
  rotSection.appendChild(rotRow);
  frag.appendChild(rotSection);

  // ── 기울이기 ──
  const skewSection = dlg.createSection('기울이기');
  const skewRow = row();
  skewRow.appendChild(label('가로'));
  const skewH = dlg.tabNumberInput();
  skewH.disabled = true;
  skewH.value = '0';
  skewRow.appendChild(skewH);
  skewRow.appendChild(unit('°'));
  skewRow.appendChild(label('세로'));
  const skewV = dlg.tabNumberInput();
  skewV.disabled = true;
  skewV.value = '0';
  skewRow.appendChild(skewV);
  skewRow.appendChild(unit('°'));
  skewSection.appendChild(skewRow);
  frag.appendChild(skewSection);

  // ── 기타 ──
  const etcSection = dlg.createSection('기타');
  const etcRow = row();
  etcRow.appendChild(label('번호 종류'));
  const numSelect = dlg.selectOptions([['Table', '표']]);
  numSelect.disabled = true;
  etcRow.appendChild(numSelect);
  etcSection.appendChild(etcRow);
  frag.appendChild(etcSection);

  return frag;
}
