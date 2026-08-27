/** 개체 속성 대화상자 — 글상자 탭 빌더 (picture-props-dialog.ts에서 분리) */
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { fieldset, row, label, unit, numberInput, checkboxLabel } from '../../dialog-dom-helpers';

export function buildTextboxTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  // ── 글상자 여백 ──
  const marginFs = fieldset('글상자 여백');
  panel.appendChild(marginFs);

  const lRow = row();
  lRow.appendChild(label('왼쪽(L):'));
  dlg.tbMarginLeftInput = numberInput(0);
  lRow.appendChild(dlg.tbMarginLeftInput);
  lRow.appendChild(unit('mm'));
  lRow.appendChild(label('위쪽(T):'));
  dlg.tbMarginTopInput = numberInput(0);
  lRow.appendChild(dlg.tbMarginTopInput);
  lRow.appendChild(unit('mm'));
  // 모두(A) 동기 스피너
  lRow.appendChild(label('모두(A):'));
  const tbSyncAll = numberInput(0);
  tbSyncAll.className = 'dialog-input pp-sync-spinner';
  tbSyncAll.addEventListener('input', () => {
    const v = tbSyncAll.value;
    dlg.tbMarginLeftInput.value = v;
    dlg.tbMarginRightInput.value = v;
    dlg.tbMarginTopInput.value = v;
    dlg.tbMarginBottomInput.value = v;
  });
  lRow.appendChild(tbSyncAll);
  marginFs.appendChild(lRow);

  const rRow = row();
  rRow.appendChild(label('오른쪽(R):'));
  dlg.tbMarginRightInput = numberInput(0);
  rRow.appendChild(dlg.tbMarginRightInput);
  rRow.appendChild(unit('mm'));
  rRow.appendChild(label('아래쪽(B):'));
  dlg.tbMarginBottomInput = numberInput(0);
  rRow.appendChild(dlg.tbMarginBottomInput);
  rRow.appendChild(unit('mm'));
  marginFs.appendChild(rRow);

  // ── 속성 ──
  const attrFs = fieldset('속성');
  panel.appendChild(attrFs);

  // 세로 정렬 (아이콘 버튼 3개)
  const vaRow = row();
  vaRow.appendChild(label('세로 정렬:'));
  dlg.tbVertAlignBtns = [];
  const vaIcons = ['⬆', '⬌', '⬇'];
  const vaTitles = ['위', '가운데', '아래'];
  const vaValues = ['Top', 'Center', 'Bottom'];
  vaTitles.forEach((title, i) => {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn pp-valign-btn';
    btn.textContent = vaIcons[i];
    btn.title = title;
    btn.dataset.value = vaValues[i];
    btn.addEventListener('click', () => {
      dlg.tbVertAlignBtns.forEach((b, j) => b.classList.toggle('active', j === i));
    });
    vaRow.appendChild(btn);
    dlg.tbVertAlignBtns.push(btn);
  });

  // 세로쓰기
  const vwLabel = checkboxLabel('세로쓰기(E):');
  dlg.tbVertWriteCheck = vwLabel.querySelector('input') as HTMLInputElement;
  dlg.tbVertWriteCheck.disabled = true;
  vaRow.appendChild(vwLabel);
  attrFs.appendChild(vaRow);

  // 영문 눕힘/세움
  const engRow = row();
  engRow.appendChild(label(''));
  dlg.tbEngLay = document.createElement('button');
  dlg.tbEngLay.className = 'pp-wrap-btn pp-eng-btn';
  dlg.tbEngLay.textContent = '가\nA B';
  dlg.tbEngLay.title = '영문 눕힘(O)';
  dlg.tbEngLay.disabled = true;
  engRow.appendChild(dlg.tbEngLay);
  dlg.tbEngStand = document.createElement('button');
  dlg.tbEngStand.className = 'pp-wrap-btn pp-eng-btn';
  dlg.tbEngStand.textContent = '가\nA\nB';
  dlg.tbEngStand.title = '영문 세움(U)';
  dlg.tbEngStand.disabled = true;
  engRow.appendChild(dlg.tbEngStand);
  attrFs.appendChild(engRow);

  // 한 줄로 입력
  const slRow = row();
  const slLabel = checkboxLabel('한 줄로 입력(S)');
  dlg.tbSingleLineCheck = slLabel.querySelector('input') as HTMLInputElement;
  dlg.tbSingleLineCheck.disabled = true;
  slRow.appendChild(slLabel);
  attrFs.appendChild(slRow);

  // ── 필드 ──
  const fieldFs = fieldset('필드');
  panel.appendChild(fieldFs);

  const fnRow = row();
  fnRow.appendChild(label('필드 이름(N):'));
  dlg.tbFieldNameInput = document.createElement('input');
  dlg.tbFieldNameInput.type = 'text';
  dlg.tbFieldNameInput.className = 'dialog-input';
  dlg.tbFieldNameInput.style.flex = '1';
  dlg.tbFieldNameInput.disabled = true;
  fnRow.appendChild(dlg.tbFieldNameInput);
  fieldFs.appendChild(fnRow);

  const fmRow = row();
  const fmLabel = checkboxLabel('양식 모드에서 편집 가능(F)');
  dlg.tbFormModeCheck = fmLabel.querySelector('input') as HTMLInputElement;
  dlg.tbFormModeCheck.disabled = true;
  fmRow.appendChild(fmLabel);
  fieldFs.appendChild(fmRow);

  return panel;
}
