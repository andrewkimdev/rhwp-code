/** 개체 속성 대화상자 — 그림자 탭 빌더 (picture-props-dialog.ts에서 분리) */
import type { PicturePropsDialog } from '../../picture-props-dialog';
import { fieldset, row, label, unit, numberInput, colorInput } from '../../dialog-dom-helpers';

export function buildShadowTab(dlg: PicturePropsDialog): HTMLDivElement {
  const panel = document.createElement('div');
  panel.className = 'dialog-tab-panel';

  // ── 종류 ──
  const typeFs = fieldset('종류');
  panel.appendChild(typeFs);

  const grid = document.createElement('div');
  grid.className = 'pp-shadow-grid';
  dlg.shadowTypeBtns = [];
  // 10개 그림자 유형 (2×5): 없음 + 9가지 방향/스타일
  const shadowLabels = [
    '없음', '왼쪽 위', '위', '오른쪽 위', '오른쪽',
    '왼쪽', '왼쪽 아래', '아래', '오른쪽 아래', '양쪽',
  ];
  const shadowIcons = [
    '□', '◰', '◱', '◲', '◳',
    '◰', '◱', '◲', '◳', '▣',
  ];
  shadowLabels.forEach((lbl, i) => {
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn pp-shadow-type-btn';
    btn.textContent = shadowIcons[i];
    btn.title = lbl;
    btn.addEventListener('click', () => {
      dlg.shadowTypeBtns.forEach((b, j) => b.classList.toggle('active', j === i));
      const enabled = i > 0;
      dlg.shadowColorInput.disabled = !enabled;
      dlg.shadowHInput.disabled = !enabled;
      dlg.shadowVInput.disabled = !enabled;
      dlg.shadowDirBtns.forEach(b => b.disabled = !enabled);
      // 타입 선택 시 기본 오프셋 자동 설정
      if (enabled) {
        // 방향별 기본 오프셋 (mm)
        const offsets: [number,number][] = [
          [0,0],       // 0: 없음
          [-1.2,-1.2], // 1: 왼쪽 위
          [0,-1.2],    // 2: 위
          [1.2,-1.2],  // 3: 오른쪽 위
          [1.2,0],     // 4: 오른쪽
          [-1.2,0],    // 5: 왼쪽
          [-1.2,1.2],  // 6: 왼쪽 아래
          [0,1.2],     // 7: 아래
          [1.2,1.2],   // 8: 오른쪽 아래
          [1.2,1.2],   // 9: 양쪽
        ];
        const [dx, dy] = offsets[i] ?? [1.2, 1.2];
        dlg.shadowHInput.value = dx.toFixed(1);
        dlg.shadowVInput.value = dy.toFixed(1);
      }
    });
    grid.appendChild(btn);
    dlg.shadowTypeBtns.push(btn);
  });
  typeFs.appendChild(grid);

  // ── 그림자 ──
  const shadowFs = fieldset('그림자');
  panel.appendChild(shadowFs);

  const cRow = row();
  cRow.appendChild(label('그림자 색(C):'));
  dlg.shadowColorInput = colorInput('#b2b2b2');
  dlg.shadowColorInput.disabled = true; // 초기 비활성 (타입 선택 시 활성)
  cRow.appendChild(dlg.shadowColorInput);
  shadowFs.appendChild(cRow);

  const hRow = row();
  hRow.appendChild(label('가로 방향 이동(H):'));
  dlg.shadowHInput = numberInput();
  dlg.shadowHInput.value = '0.0';
  dlg.shadowHInput.disabled = true;
  hRow.appendChild(dlg.shadowHInput);
  hRow.appendChild(unit('mm'));

  // 8방향 버튼 (3×3 - 중앙 제외)
  const dirGrid = document.createElement('div');
  dirGrid.className = 'pp-direction-grid';
  dlg.shadowDirBtns = [];
  const dirIcons = ['↖', '↑', '↗', '←', '', '→', '↙', '↓', '↘'];
  dirIcons.forEach((icon, i) => {
    if (i === 4) {
      // 중앙 빈칸
      const spacer = document.createElement('div');
      spacer.className = 'pp-dir-spacer';
      spacer.textContent = '✕';
      dirGrid.appendChild(spacer);
      return;
    }
    const btn = document.createElement('button');
    btn.className = 'pp-wrap-btn pp-dir-btn';
    btn.textContent = icon;
    btn.disabled = true; // 초기 비활성
    btn.addEventListener('click', () => {
      dlg.shadowDirBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      // 방향에 따라 offset 자동 설정
      const offsets: [number,number][] = [[-1,-1],[0,-1],[1,-1],[-1,0],[0,0],[1,0],[-1,1],[0,1],[1,1]];
      const dirIdx = [0,1,2,3,/*4skip*/5,6,7,8][dlg.shadowDirBtns.indexOf(btn)] ?? 8;
      const [dx, dy] = offsets[dirIdx] ?? [1, 1];
      dlg.shadowHInput.value = (dx * 1.2).toFixed(1);
      dlg.shadowVInput.value = (dy * 1.2).toFixed(1);
    });
    dirGrid.appendChild(btn);
    dlg.shadowDirBtns.push(btn);
  });
  hRow.appendChild(dirGrid);
  shadowFs.appendChild(hRow);

  const vRow = row();
  vRow.appendChild(label('세로 방향 이동(V):'));
  dlg.shadowVInput = numberInput();
  dlg.shadowVInput.value = '0.0';
  dlg.shadowVInput.disabled = true;
  vRow.appendChild(dlg.shadowVInput);
  vRow.appendChild(unit('mm'));
  shadowFs.appendChild(vRow);

  // ── 투명도 설정 ──
  const transFs = fieldset('투명도 설정');
  panel.appendChild(transFs);
  const transRow = row();
  transRow.appendChild(label('투명도(I):'));
  dlg.shadowTransInput = numberInput(0, 100, 1);
  dlg.shadowTransInput.value = '0';
  dlg.shadowTransInput.disabled = true;
  transRow.appendChild(dlg.shadowTransInput);
  transRow.appendChild(unit('%'));
  transFs.appendChild(transRow);

  return panel;
}
