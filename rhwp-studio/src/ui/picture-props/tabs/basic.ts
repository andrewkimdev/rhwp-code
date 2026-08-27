/** 개체 속성 대화상자 — 기본 탭 빌더 (picture-props-dialog.ts에서 분리) */
import { userSettings } from '@/core/user-settings';
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { fieldset, row, label, unit, numberInput, selectEl, sizeTypeSelect, checkboxLabel } from '../../dialog-dom-helpers';

export function buildBasicTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  // ── 크기 ──
  const sizeFs = fieldset('크기');
  panel.appendChild(sizeFs);

  // 너비
  const wRow = row();
  wRow.appendChild(label('너비(W)'));
  const widthTypeSelect = sizeTypeSelect();
  dlg.sizeLockControls.push(widthTypeSelect);
  wRow.appendChild(widthTypeSelect);
  dlg.widthInput = numberInput(0);
  dlg.sizeLockControls.push(dlg.widthInput);
  wRow.appendChild(dlg.widthInput);
  wRow.appendChild(unit('mm'));
  sizeFs.appendChild(wRow);

  // 높이
  const hRow = row();
  hRow.appendChild(label('높이(H)'));
  const heightTypeSelect = sizeTypeSelect();
  dlg.sizeLockControls.push(heightTypeSelect);
  hRow.appendChild(heightTypeSelect);
  dlg.heightInput = numberInput(0);
  dlg.sizeLockControls.push(dlg.heightInput);
  hRow.appendChild(dlg.heightInput);
  hRow.appendChild(unit('mm'));
  // 크기 고정
  const sfLabel = checkboxLabel('크기 고정(S)');
  dlg.sizeFixedCheck = sfLabel.querySelector('input') as HTMLInputElement;
  hRow.appendChild(sfLabel);
  const krLabel = checkboxLabel('비율 유지');
  dlg.keepRatioCheck = krLabel.querySelector('input') as HTMLInputElement;
  dlg.keepRatioCheck.checked = userSettings.getPicturePropsKeepRatio();
  dlg.sizeLockControls.push(dlg.keepRatioCheck);
  hRow.appendChild(krLabel);
  sizeFs.appendChild(hRow);

  dlg.sizeFixedCheck.addEventListener('change', () => dlg.updateSizeProtectControls());

  // 비율 유지 이벤트
  dlg.keepRatioCheck.addEventListener('change', () => {
    userSettings.setPicturePropsKeepRatio(dlg.keepRatioCheck.checked);
  });
  dlg.widthInput.addEventListener('input', () => {
    if (dlg.keepRatioCheck.checked && !dlg.syncingBasicSize && dlg.originalWidth > 0) {
      const ratio = dlg.originalHeight / dlg.originalWidth;
      const w = parseFloat(dlg.widthInput.value) || 0;
      dlg.syncingBasicSize = true;
      try {
        dlg.heightInput.value = (w * ratio).toFixed(2);
      } finally {
        dlg.syncingBasicSize = false;
      }
    }
  });
  dlg.heightInput.addEventListener('input', () => {
    if (dlg.keepRatioCheck.checked && !dlg.syncingBasicSize && dlg.originalHeight > 0) {
      const ratio = dlg.originalWidth / dlg.originalHeight;
      const h = parseFloat(dlg.heightInput.value) || 0;
      dlg.syncingBasicSize = true;
      try {
        dlg.widthInput.value = (h * ratio).toFixed(2);
      } finally {
        dlg.syncingBasicSize = false;
      }
    }
  });

  // ── 위치 ──
  const posFs = fieldset('위치');
  panel.appendChild(posFs);

  // 글자처럼 취급
  const tacRow = row();
  const tacLabel = checkboxLabel('글자처럼 취급(C)');
  dlg.treatAsCharCheck = tacLabel.querySelector('input') as HTMLInputElement;
  tacRow.appendChild(tacLabel);
  posFs.appendChild(tacRow);
  dlg.treatAsCharCheck.addEventListener('change', () => dlg.updatePositionVisibility());

  // 본문과의 배치 (아이콘 버튼 5개) + 본문 위치 드롭다운
  const wrapRow = row();
  wrapRow.classList.add('pp-pos-detail');
  wrapRow.appendChild(label('본문과의 배치:'));
  const wrapIcons = ['⬒', '⬓', '⬔', '⬕', '⬖'];
  const wrapTitles = ['자리 차지', '어울림', '빈 공간 채움', '글 뒤로', '글 앞으로'];
  dlg.wrapBtns = [];
  wrapTitles.forEach((title, i) => {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn';
    btn.textContent = wrapIcons[i];
    btn.title = title;
    btn.addEventListener('click', () => dlg.selectWrap(i));
    wrapRow.appendChild(btn);
    dlg.wrapBtns.push(btn);
  });
  // 본문 위치(P)
  wrapRow.appendChild(label('본문 위치(P):'));
  dlg.bodyPosSelect = selectEl([
    ['Both', '양쪽'], ['Left', '왼쪽'], ['Right', '오른쪽'],
    ['Larger', '큰 쪽'], ['Smaller', '작은 쪽'],
  ]);
  dlg.bodyPosSelect.disabled = true;
  wrapRow.appendChild(dlg.bodyPosSelect);
  posFs.appendChild(wrapRow);
  dlg.posDetailEls.push(wrapRow);

  // 가로
  const hPosRow = row();
  hPosRow.classList.add('pp-pos-detail');
  hPosRow.appendChild(label('가로(I):'));
  // [Task #1282] 한컴은 자리차지(TopAndBottom) 그림의 가로 기준 칸에
  // 실제 HorzRelTo 대신 "자리 차지"를 표시한다. 저장값은 textWrap 이므로
  // OK 시에는 HorzRelTo 로 넘기지 않는다.
  dlg.horzRelSelect = selectEl([
    ['TakePlace', '자리 차지'],
    ['Paper', '종이'], ['Page', '쪽'], ['Column', '단'], ['Para', '문단'],
  ]);
  hPosRow.appendChild(dlg.horzRelSelect);
  hPosRow.appendChild(unit('의'));
  dlg.horzAlignSelect = selectEl([
    ['Left', '왼쪽'], ['Center', '가운데'], ['Right', '오른쪽'], ['Outside', '바깥쪽'],
  ]);
  hPosRow.appendChild(dlg.horzAlignSelect);
  hPosRow.appendChild(unit('기준'));
  dlg.horzOffsetInput = numberInput();
  hPosRow.appendChild(dlg.horzOffsetInput);
  hPosRow.appendChild(unit('mm'));
  posFs.appendChild(hPosRow);
  dlg.posDetailEls.push(hPosRow);

  // 세로
  const vPosRow = row();
  vPosRow.classList.add('pp-pos-detail');
  vPosRow.appendChild(label('세로(V):'));
  dlg.vertRelSelect = selectEl([
    ['Paper', '종이'], ['Page', '쪽'], ['Para', '문단'],
  ]);
  vPosRow.appendChild(dlg.vertRelSelect);
  vPosRow.appendChild(unit('의'));
  dlg.vertAlignSelect = selectEl([
    ['Top', '위'], ['Center', '가운데'], ['Bottom', '아래'],
  ]);
  vPosRow.appendChild(dlg.vertAlignSelect);
  vPosRow.appendChild(unit('기준'));
  dlg.vertOffsetInput = numberInput();
  vPosRow.appendChild(dlg.vertOffsetInput);
  vPosRow.appendChild(unit('mm'));
  posFs.appendChild(vPosRow);
  dlg.posDetailEls.push(vPosRow);

  // 쪽 영역 안으로 제한 / 서로 겹침 허용
  const optRow = row();
  optRow.classList.add('pp-pos-detail');
  const palLabel = checkboxLabel('쪽 영역 안으로 제한(B)');
  dlg.pageAreaLimitCheck = palLabel.querySelector('input') as HTMLInputElement;
  dlg.pageAreaLimitCheck.addEventListener('change', () => dlg.updateOverlapOption());
  optRow.appendChild(palLabel);
  const oaLabel = checkboxLabel('서로 겹침 허용(L)');
  dlg.overlapAllowCheck = oaLabel.querySelector('input') as HTMLInputElement;
  optRow.appendChild(oaLabel);
  posFs.appendChild(optRow);
  dlg.posDetailEls.push(optRow);

  // 개체와 조판 부호를 항상 같은 쪽에 놓기
  const spRow = row();
  spRow.classList.add('pp-pos-detail');
  const spLabel = checkboxLabel('개체와 조판 부호를 항상 같은 쪽에 놓기(A)');
  dlg.samePageCheck = spLabel.querySelector('input') as HTMLInputElement;
  dlg.samePageCheck.disabled = true;
  spRow.appendChild(spLabel);
  posFs.appendChild(spRow);
  dlg.posDetailEls.push(spRow);

  // ── 개체 회전 ──
  const rotFs = fieldset('개체 회전/대칭');
  panel.appendChild(rotFs);
  const rotRow = row();
  rotRow.appendChild(label('회전각(E):'));
  dlg.rotationInput = numberInput(-360, 360, 1);
  dlg.rotationInput.disabled = true;
  rotRow.appendChild(dlg.rotationInput);
  rotRow.appendChild(unit('°'));
  // 회전 프리뷰 원
  const rotPreview = document.createElement('div');
  rotPreview.className = 'pp-rot-preview';
  const rotLine = document.createElement('div');
  rotLine.className = 'pp-rot-line';
  rotPreview.appendChild(rotLine);
  rotRow.appendChild(rotPreview);
  rotFs.appendChild(rotRow);
  // 대칭 체크박스
  const flipRow = row();
  dlg.horzFlipCheck = document.createElement('input');
  dlg.horzFlipCheck.type = 'checkbox';
  dlg.horzFlipCheck.disabled = true;
  const horzLabel = label('좌우 대칭');
  horzLabel.style.cursor = 'pointer';
  horzLabel.prepend(dlg.horzFlipCheck);
  flipRow.appendChild(horzLabel);
  dlg.vertFlipCheck = document.createElement('input');
  dlg.vertFlipCheck.type = 'checkbox';
  dlg.vertFlipCheck.disabled = true;
  const vertLabel = label('상하 대칭');
  vertLabel.style.cursor = 'pointer';
  vertLabel.style.marginLeft = '12px';
  vertLabel.prepend(dlg.vertFlipCheck);
  flipRow.appendChild(vertLabel);
  rotFs.appendChild(flipRow);

  // ── 기울이기 ──
  const skewFs = fieldset('기울이기');
  panel.appendChild(skewFs);
  const skewRow = row();
  skewRow.appendChild(label('가로(Y):'));
  dlg.skewHInput = numberInput(0, 45, 1);
  dlg.skewHInput.disabled = true;
  skewRow.appendChild(dlg.skewHInput);
  skewRow.appendChild(unit('°'));
  skewRow.appendChild(label('세로(U):'));
  dlg.skewVInput = numberInput(0, 45, 1);
  dlg.skewVInput.disabled = true;
  skewRow.appendChild(dlg.skewVInput);
  skewRow.appendChild(unit('°'));
  skewFs.appendChild(skewRow);

  // ── 기타 ──
  const etcFs = fieldset('기타');
  panel.appendChild(etcFs);
  const etcRow = row();
  etcRow.appendChild(label('번호 종류(N):'));
  const numTypeSelect = selectEl([['Picture', '그림']]);
  numTypeSelect.disabled = true;
  etcRow.appendChild(numTypeSelect);
  // 개체 보호하기
  const protLabel = checkboxLabel('개체 보호하기(K)');
  dlg.protectCheck = protLabel.querySelector('input') as HTMLInputElement;
  dlg.protectCheck.disabled = true;
  etcRow.appendChild(protLabel);
  const descBtn = document.createElement('button');
  descBtn.className = 'dialog-btn pp-desc-btn';
  descBtn.textContent = '개체 설명문(X)...';
  descBtn.addEventListener('click', () => dlg.showDescriptionPrompt());
  etcRow.appendChild(descBtn);
  etcFs.appendChild(etcRow);

  // 개체 설명 값 (숨김)
  dlg.descInput = document.createElement('input');
  dlg.descInput.type = 'hidden';
  panel.appendChild(dlg.descInput);

  return panel;
}
