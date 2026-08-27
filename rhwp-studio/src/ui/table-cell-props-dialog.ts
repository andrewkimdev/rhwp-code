import { ModalDialog } from './dialog';
import type { WasmBridge } from '@/core/wasm-bridge';
import type { CellProperties, TableProperties } from '@/core/types';
import type { EventBus } from '@/core/event-bus';
import type { CommandServices } from '@/command/types';
import { label } from './dialog-dom-helpers';
import {
  hwpunitToMm,
  mmToHwpunit,
  hwp16ToMm,
  mmToHwp16,
  DOC_PAPER_COLOR,
  PREVIEW_GUIDE_STROKE,
} from './table-cell-props-units';
import { buildCellTab } from './table-cell-props/tabs/cell';
import { buildTableTab } from './table-cell-props/tabs/table';
import { buildBasicTab } from './table-cell-props/tabs/basic';
import { buildMarginTab } from './table-cell-props/tabs/margin-caption';
import { buildBorderTab } from './table-cell-props/tabs/border';
import { buildBackgroundTab } from './table-cell-props/tabs/background';

/** 탭 정의 */
interface TabDef {
  id: string;
  label: string;
  builder: () => HTMLElement;
}

/**
 * 표/셀 속성 다이얼로그 (HWP 표준 6탭)
 */
export class TableCellPropsDialog extends ModalDialog {
  private wasm: WasmBridge;
  private eventBus: EventBus;
  private tableCtx: { sec: number; ppi: number; ci: number };
  private cellIdx: number;
  /** 'table' = 표 선택 (6탭), 'cell' = 셀 선택 (4탭: 테두리·배경 제외) */
  private mode: 'table' | 'cell';

  // ─── 탭 UI ───
  private tabs: HTMLButtonElement[] = [];
  private panels: HTMLDivElement[] = [];

  // ─── 셀 탭 필드 ───
  cellWidthInput!: HTMLInputElement;
  cellHeightInput!: HTMLInputElement;
  cellPaddingInputs!: Record<string, HTMLInputElement>;
  cellPaddingCheck!: HTMLInputElement;
  cellVAlignBtns!: HTMLButtonElement[];
  cellTextDirBtns!: HTMLButtonElement[];
  cellHeaderCheck!: HTMLInputElement;
  cellSingleLineCheck!: HTMLInputElement;
  cellProtectCheck!: HTMLInputElement;
  cellFieldNameInput!: HTMLInputElement;
  cellEditableCheck!: HTMLInputElement;
  cellApplySizeCheck!: HTMLInputElement;

  // ─── 표 탭 필드 ───
  tablePageBreakSelect!: HTMLSelectElement;
  tableRepeatHeaderCheck!: HTMLInputElement;
  tablePaddingInputs!: Record<string, HTMLInputElement>;
  tableAutoBorderCheck!: HTMLInputElement;
  tableAutoBorderFields!: HTMLDivElement;

  // ─── 테두리 탭 필드 ───
  borderCellSpacingInput!: HTMLInputElement;
  borderLineTypeGrid!: HTMLDivElement;
  borderSelectedLineType = 1;
  borderWidthSelect!: HTMLSelectElement;
  borderColorInput!: HTMLInputElement;
  borderPreviewSvg!: SVGSVGElement;
  borderApplyImmediateCheck!: HTMLInputElement;
  /** 4방향 테두리 편집 상태 */
  borderEdits!: { type: number; width: number; color: string }[];
  /** 적용 대상: 'cell' 또는 'table' */
  borderTarget!: string;
  /** 자동 경계선 설정 필드 */
  borderAutoBorderCheck!: HTMLInputElement;
  borderAutoBorderFields!: HTMLDivElement;

  // ─── 배경 탭 필드 ───
  bgNoneRadio!: HTMLInputElement;
  bgColorRadio!: HTMLInputElement;
  bgColorPicker!: HTMLInputElement;
  bgPatternColorPicker!: HTMLInputElement;
  bgPatternTypeSelect!: HTMLSelectElement;
  bgPreviewBox!: HTMLDivElement;
  /** 배경 적용 대상: 'cell' 또는 'table' */
  bgTarget!: string;

  // ─── 기본 탭 필드 ───
  basicWidthInput!: HTMLInputElement;
  basicHeightInput!: HTMLInputElement;
  treatAsCharCheck!: HTMLInputElement;
  wrapBtns: HTMLButtonElement[] = [];
  private wrapValues = ['Square', 'TopAndBottom', 'BehindText', 'InFrontOfText'];
  horzRelSelect!: HTMLSelectElement;
  horzAlignSelect!: HTMLSelectElement;
  horzOffsetInput!: HTMLInputElement;
  vertRelSelect!: HTMLSelectElement;
  vertAlignSelect!: HTMLSelectElement;
  vertOffsetInput!: HTMLInputElement;
  posGroup!: HTMLDivElement;
  restrictInPageCheck!: HTMLInputElement;
  allowOverlapCheck!: HTMLInputElement;
  keepWithAnchorCheck!: HTMLInputElement;

  // ─── 여백/캡션 탭 필드 ───
  marginOuterInputs!: Record<string, HTMLInputElement>;
  captionDirSelect!: HTMLSelectElement;
  captionSpacingInput!: HTMLInputElement;
  captionWidthInput!: HTMLInputElement;
  captionExpandCheck!: HTMLInputElement;
  captionSection!: HTMLDivElement;
  captionPosBtns!: HTMLButtonElement[];
  captionFieldsWrap!: HTMLDivElement;

  // 현재 속성값 캐시
  private cellProps!: CellProperties;
  private tableProps!: TableProperties;
  /** undo 기록 라우팅용 (없으면 wasm 직접 호출 fallback). */
  private services: CommandServices | undefined;

  constructor(
    wasm: WasmBridge,
    eventBus: EventBus,
    tableCtx: { sec: number; ppi: number; ci: number },
    cellIdx: number,
    mode: 'table' | 'cell' = 'cell',
    services?: CommandServices,
  ) {
    super('표/셀 속성', 480);
    this.wasm = wasm;
    this.eventBus = eventBus;
    this.tableCtx = tableCtx;
    this.cellIdx = cellIdx;
    this.mode = mode;
    this.services = services;
  }

  show(): void {
    super.show();
    this.dialog.classList.add('tcp-dialog');
    // 속성 조회
    const { sec, ppi, ci } = this.tableCtx;
    this.cellProps = this.wasm.getCellProperties(sec, ppi, ci, this.cellIdx);
    this.tableProps = this.wasm.getTableProperties(sec, ppi, ci);
    this.populateFields();
  }

  protected createBody(): HTMLElement {
    const body = document.createElement('div');
    body.className = 'tcp-dialog-body';

    // 탭 정의: mode에 따라 테두리/배경 탭 포함 여부 결정
    const tabDefs: TabDef[] = [
      { id: 'basic', label: '기본', builder: () => this.buildBasicTab() },
      { id: 'margin', label: '여백/캡션', builder: () => this.buildMarginTab() },
      // 표 선택 시에만 테두리·배경 탭 표시 (셀 선택 시 별도 "셀 테두리/배경" 대화상자 사용)
      ...(this.mode === 'table' ? [
        { id: 'border', label: '테두리', builder: () => this.buildBorderTab() },
        { id: 'background', label: '배경', builder: () => this.buildBackgroundTab() },
      ] as TabDef[] : []),
      { id: 'table', label: '표', builder: () => this.buildTableTab() },
      { id: 'cell', label: '셀', builder: () => this.buildCellTab() },
    ];

    // 탭 헤더
    const tabBar = document.createElement('div');
    tabBar.className = 'dialog-tabs';

    const panelContainer = document.createElement('div');
    panelContainer.className = 'tcp-panel-container';

    for (let i = 0; i < tabDefs.length; i++) {
      const def = tabDefs[i];

      // 탭 버튼
      const btn = document.createElement('button');
      btn.className = 'dialog-tab';
      btn.textContent = def.label;
      btn.type = 'button';
      btn.addEventListener('click', () => this.switchTab(i));
      this.tabs.push(btn);
      tabBar.appendChild(btn);

      // 탭 패널
      const panel = document.createElement('div');
      panel.className = 'dialog-tab-panel';
      panel.appendChild(def.builder());
      this.panels.push(panel);
      panelContainer.appendChild(panel);
    }

    body.appendChild(tabBar);
    body.appendChild(panelContainer);

    // 기본 활성 탭: 표 선택 시 '기본' 탭, 셀 선택 시 '셀' 탭(마지막)
    this.switchTab(this.mode === 'table' ? 0 : tabDefs.length - 1);

    return body;
  }

  private switchTab(idx: number): void {
    for (let i = 0; i < this.tabs.length; i++) {
      this.tabs[i].classList.toggle('active', i === idx);
      this.panels[i].classList.toggle('active', i === idx);
    }
  }

  // ─── 셀 탭 ───────────────────────────────────────

  private buildCellTab(): HTMLElement {
    return buildCellTab(this);
  }

  // ─── 표 탭 ───────────────────────────────────────

  private buildTableTab(): HTMLElement {
    return buildTableTab(this);
  }

  // ─── 기본 탭 ──────────────────────────────

  private buildBasicTab(): HTMLElement {
    return buildBasicTab(this);
  }

  selectOptions(items: string[][]): HTMLSelectElement {
    const sel = document.createElement('select');
    sel.className = 'dialog-select';
    for (const [value, text] of items) {
      const opt = document.createElement('option');
      opt.value = value;
      opt.textContent = text;
      sel.appendChild(opt);
    }
    return sel;
  }

  updatePositionVisibility(): void {
    const disabled = this.treatAsCharCheck.checked;
    this.posGroup.classList.toggle('disabled', disabled);
  }

  updateCellPaddingState(): void {
    const enabled = this.cellPaddingCheck.checked;
    for (const input of Object.values(this.cellPaddingInputs)) {
      input.disabled = !enabled;
    }
  }

  updateCellSizeState(): void {
    const enabled = this.cellApplySizeCheck.checked;
    this.cellWidthInput.disabled = !enabled;
    this.cellHeightInput.disabled = !enabled;
  }

  selectWrap(idx: number): void {
    this.wrapBtns.forEach((b, i) => b.classList.toggle('active', i === idx));
  }

  private getSelectedWrap(): string {
    const idx = this.wrapBtns.findIndex(b => b.classList.contains('active'));
    return idx >= 0 ? this.wrapValues[idx] : 'Square';
  }

  // ─── 여백/캡션 탭 ──────────────────────────

  private buildMarginTab(): HTMLElement {
    return buildMarginTab(this);
  }

  /** 위/아래 선택 시 캡션 크기 비활성, 좌/우 선택 시 활성 */
  updateCaptionWidthState(): void {
    const activeBtn = this.captionPosBtns.find(b => b.classList.contains('active'));
    const dir = activeBtn ? parseInt(activeBtn.dataset.dir!, 10) : -1;
    // dir 2=위, 3=아래 → 캡션 크기 비활성 (위/아래는 표 너비와 동일)
    const isTopBottom = dir === 2 || dir === 3;
    this.captionWidthInput.disabled = isTopBottom;
    if (isTopBottom) {
      this.captionWidthInput.style.opacity = '0.5';
    } else {
      this.captionWidthInput.style.opacity = '';
    }
  }

  // ─── 테두리 탭 ──────────────────────────────────

  private buildBorderTab(): HTMLElement {
    return buildBorderTab(this);
  }

  /** 현재 선택된 선 종류/굵기/색을 지정 방향에 적용 */
  applyBorderToDirection(dirIdx: number): void {
    const lineType = this.borderSelectedLineType;
    const width = parseInt(this.borderWidthSelect.value, 10);
    const color = this.borderColorInput.value;
    const val = { type: lineType, width, color };
    if (dirIdx === 4) { // 모두
      this.borderEdits = [val, val, val, val];
    } else {
      this.borderEdits[dirIdx] = val;
    }
    this.updateBorderPreview();
  }

  /** SVG 기반 테두리 미리보기 갱신 (십자선 포함) */
  private updateBorderPreview(): void {
    const svg = this.borderPreviewSvg;
    if (!svg) return;
    // clear
    while (svg.firstChild) svg.removeChild(svg.firstChild);

    const ns = 'http://www.w3.org/2000/svg';
    // 배경
    const bg = document.createElementNS(ns, 'rect');
    bg.setAttribute('x', '0'); bg.setAttribute('y', '0');
    bg.setAttribute('width', '120'); bg.setAttribute('height', '100');
    bg.style.setProperty('fill', DOC_PAPER_COLOR);
    svg.appendChild(bg);

    // 십자선 (셀 구분선) — 연한 회색 점선
    const cross1 = document.createElementNS(ns, 'line');
    cross1.setAttribute('x1', '60'); cross1.setAttribute('y1', '5');
    cross1.setAttribute('x2', '60'); cross1.setAttribute('y2', '95');
    cross1.style.setProperty('stroke', PREVIEW_GUIDE_STROKE); cross1.setAttribute('stroke-width', '0.5');
    cross1.setAttribute('stroke-dasharray', '3,2');
    svg.appendChild(cross1);
    const cross2 = document.createElementNS(ns, 'line');
    cross2.setAttribute('x1', '5'); cross2.setAttribute('y1', '50');
    cross2.setAttribute('x2', '115'); cross2.setAttribute('y2', '50');
    cross2.style.setProperty('stroke', PREVIEW_GUIDE_STROKE); cross2.setAttribute('stroke-width', '0.5');
    cross2.setAttribute('stroke-dasharray', '3,2');
    svg.appendChild(cross2);

    // 4방향 테두리 선
    const drawBorder = (x1: number, y1: number, x2: number, y2: number, b: { type: number; width: number; color: string }) => {
      if (b.type === 0) return;
      const w = Math.max(0.5, (b.width + 1) * 0.7);
      const dashMap: Record<number, string> = {
        2: '6,3', 3: '2,2', 4: '8,3,2,3', 5: '8,3,2,3,2,3', 6: '12,3',
      };
      if (b.type === 7) {
        // 이중선
        const offset = w * 0.8;
        for (const off of [-offset, offset]) {
          const line = document.createElementNS(ns, 'line');
          const isVert = x1 === x2;
          line.setAttribute('x1', String(x1 + (isVert ? off : 0)));
          line.setAttribute('y1', String(y1 + (isVert ? 0 : off)));
          line.setAttribute('x2', String(x2 + (isVert ? off : 0)));
          line.setAttribute('y2', String(y2 + (isVert ? 0 : off)));
          line.setAttribute('stroke', b.color); line.setAttribute('stroke-width', String(w * 0.5));
          svg.appendChild(line);
        }
      } else {
        const line = document.createElementNS(ns, 'line');
        line.setAttribute('x1', String(x1)); line.setAttribute('y1', String(y1));
        line.setAttribute('x2', String(x2)); line.setAttribute('y2', String(y2));
        line.setAttribute('stroke', b.color); line.setAttribute('stroke-width', String(w));
        if (dashMap[b.type]) line.setAttribute('stroke-dasharray', dashMap[b.type]);
        svg.appendChild(line);
      }
    };

    // left, right, top, bottom
    drawBorder(2, 2, 2, 98, this.borderEdits[0]);     // 왼쪽
    drawBorder(118, 2, 118, 98, this.borderEdits[1]);  // 오른쪽
    drawBorder(2, 2, 118, 2, this.borderEdits[2]);     // 위
    drawBorder(2, 98, 118, 98, this.borderEdits[3]);   // 아래
  }

  /** 적용 대상(셀/표) 전환 시 해당 속성으로 테두리 편집 상태 갱신 */
  private populateBorderFromTarget(): void {
    const props = this.borderTarget === 'table' ? this.tableProps : this.cellProps;
    const dirs = ['borderLeft', 'borderRight', 'borderTop', 'borderBottom'] as const;
    for (let i = 0; i < 4; i++) {
      const b = props[dirs[i]];
      if (b) {
        this.borderEdits[i] = { type: b.type, width: b.width, color: b.color };
      }
    }
    // 굵기/색/선종류 컨트롤을 대표 테두리(왼쪽)로 동기화한다. 이 컨트롤들은
    // applyBorderToDirection()이 '현재 값'으로 그대로 읽어 wasm.set*Properties에
    // 전달하므로, 미리보기만 문서 값을 반영하고 컨트롤은 하드코딩 기본값(0.1mm/검정/실선)에
    // 머무르면 방향 버튼 재적용 시 기존 서식이 조용히 유실된다 (#2908).
    const rep = this.borderEdits[0];
    if (rep) {
      this.borderSelectedLineType = rep.type;
      this.borderWidthSelect.value = String(rep.width);
      this.borderColorInput.value = rep.color;
      this.borderLineTypeGrid.querySelectorAll('.tcp-line-type-item').forEach((el, idx) => {
        const lineTypeDefs = [0, 1, 2, 3, 4, 5, 6, 8];
        el.classList.toggle('active', lineTypeDefs[idx] === rep.type);
      });
    }
    this.updateBorderPreview();
  }

  // ─── 배경 탭 ──────────────────────────────────

  private buildBackgroundTab(): HTMLElement {
    return buildBackgroundTab(this);
  }

  /** 배경 대상(셀/표) 전환 시 해당 속성으로 배경 상태 갱신 */
  private populateBgFromTarget(): void {
    const props = this.bgTarget === 'table' ? this.tableProps : this.cellProps;
    if (props.fillType === 'solid' && props.fillColor) {
      this.bgColorRadio.checked = true;
      this.bgColorPicker.value = props.fillColor;
      this.bgPatternColorPicker.value = props.patternColor ?? '#000000';
      this.bgPatternTypeSelect.value = props.patternType != null ? String(props.patternType) : '0';
    } else {
      this.bgNoneRadio.checked = true;
    }
    this.updateBgPreview();
  }

  /** 배경 미리보기 갱신 (무늬 패턴 포함) */
  updateBgPreview(): void {
    if (!this.bgColorRadio.checked) {
      this.bgPreviewBox.style.background = DOC_PAPER_COLOR;
      return;
    }
    const faceColor = this.bgColorPicker.value;
    const patType = parseInt(this.bgPatternTypeSelect.value, 10);
    if (patType === 0) {
      this.bgPreviewBox.style.background = faceColor;
      return;
    }
    const patColor = this.bgPatternColorPicker.value;
    // CSS repeating-linear-gradient 패턴
    const patternMap: Record<number, string> = {
      1: `repeating-linear-gradient(0deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 4px)`,  // 가로줄
      2: `repeating-linear-gradient(90deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 4px)`, // 세로줄
      3: `repeating-linear-gradient(135deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 5px)`,// 역슬래시
      4: `repeating-linear-gradient(45deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 5px)`, // 슬래시
      5: `repeating-linear-gradient(0deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 4px),repeating-linear-gradient(90deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 4px)`, // 십자
      6: `repeating-linear-gradient(45deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 5px),repeating-linear-gradient(135deg,${patColor} 0px,${patColor} 1px,transparent 1px,transparent 5px)`, // X자
    };
    this.bgPreviewBox.style.background = `${patternMap[patType] || ''},${faceColor}`;
  }

  // ─── 필드 채우기 ─────────────────────────────────

  private populateFields(): void {
    const cp = this.cellProps;
    const tp = this.tableProps;

    // 셀 탭
    this.cellWidthInput.value = hwpunitToMm(cp.width).toFixed(1);
    this.cellHeightInput.value = hwpunitToMm(cp.height).toFixed(1);
    this.updateCellSizeState();
    this.cellPaddingInputs['left'].value = hwp16ToMm(cp.paddingLeft).toFixed(1);
    this.cellPaddingInputs['right'].value = hwp16ToMm(cp.paddingRight).toFixed(1);
    this.cellPaddingInputs['top'].value = hwp16ToMm(cp.paddingTop).toFixed(1);
    this.cellPaddingInputs['bottom'].value = hwp16ToMm(cp.paddingBottom).toFixed(1);
    this.cellPaddingCheck.checked = cp.applyInnerMargin ?? false;
    this.updateCellPaddingState();
    this.setButtonGroupActive(this.cellVAlignBtns, cp.verticalAlign);
    this.setButtonGroupActive(this.cellTextDirBtns, cp.textDirection);
    this.cellHeaderCheck.checked = cp.isHeader;
    this.cellProtectCheck.checked = cp.cellProtect ?? false;
    this.cellFieldNameInput.value = cp.fieldName ?? '';
    this.cellEditableCheck.checked = cp.editableInForm ?? false;

    // 표 탭
    this.tablePageBreakSelect.value = String(tp.pageBreak ?? 0);
    this.tableRepeatHeaderCheck.checked = tp.repeatHeader;
    this.tablePaddingInputs['left'].value = hwp16ToMm(tp.paddingLeft).toFixed(1);
    this.tablePaddingInputs['right'].value = hwp16ToMm(tp.paddingRight).toFixed(1);
    this.tablePaddingInputs['top'].value = hwp16ToMm(tp.paddingTop).toFixed(1);
    this.tablePaddingInputs['bottom'].value = hwp16ToMm(tp.paddingBottom).toFixed(1);

    // 기본 탭
    if (tp.tableWidth != null) {
      this.basicWidthInput.value = hwpunitToMm(tp.tableWidth).toFixed(1);
      this.basicWidthInput.readOnly = true;
      this.basicWidthInput.style.removeProperty('background');
    }
    if (tp.tableHeight != null) {
      this.basicHeightInput.value = hwpunitToMm(tp.tableHeight).toFixed(1);
      this.basicHeightInput.readOnly = true;
      this.basicHeightInput.style.removeProperty('background');
    }
    this.treatAsCharCheck.checked = tp.treatAsChar ?? true;
    this.selectWrap(this.wrapValues.indexOf(tp.textWrap ?? 'Square'));
    this.horzRelSelect.value = tp.horzRelTo ?? 'Paper';
    this.horzAlignSelect.value = tp.horzAlign ?? 'Left';
    this.horzOffsetInput.value = hwpunitToMm(tp.horzOffset ?? 0).toFixed(1);
    this.vertRelSelect.value = tp.vertRelTo ?? 'Paper';
    this.vertAlignSelect.value = tp.vertAlign ?? 'Top';
    this.vertOffsetInput.value = hwpunitToMm(tp.vertOffset ?? 0).toFixed(1);
    this.restrictInPageCheck.checked = tp.restrictInPage ?? true;
    this.allowOverlapCheck.checked = tp.allowOverlap ?? false;
    this.keepWithAnchorCheck.checked = tp.keepWithAnchor ?? false;
    this.updatePositionVisibility();

    // 여백/캡션 탭
    this.marginOuterInputs['left'].value = hwp16ToMm(tp.outerLeft ?? 0).toFixed(1);
    this.marginOuterInputs['right'].value = hwp16ToMm(tp.outerRight ?? 0).toFixed(1);
    this.marginOuterInputs['top'].value = hwp16ToMm(tp.outerTop ?? 0).toFixed(1);
    this.marginOuterInputs['bottom'].value = hwp16ToMm(tp.outerBottom ?? 0).toFixed(1);

    // 기본값 설정
    this.captionSpacingInput.value = '3.0';
    this.captionWidthInput.value = '30.0';

    if (tp.hasCaption) {
      const dir = tp.captionDirection ?? 3;
      const va = tp.captionVertAlign ?? 0;
      this.captionDirSelect.value = String(dir);
      this.captionSpacingInput.value = hwp16ToMm(tp.captionSpacing ?? 0).toFixed(1);
      this.captionWidthInput.value = hwpunitToMm(tp.captionWidth ?? 0).toFixed(1);
      // 캡션 위치 아이콘 활성화 (dir + sub 매칭)
      const activeBtn = this.captionPosBtns.find(b =>
        b.dataset.dir === String(dir) && b.dataset.sub === String(va));
      if (activeBtn) activeBtn.classList.add('active');
      this.captionFieldsWrap.classList.remove('tcp-disabled');
    } else {
      // 가운데(캡션 없음) 버튼 활성화
      const noneBtn = this.captionPosBtns.find(b => b.dataset.dir === '-1');
      if (noneBtn) noneBtn.classList.add('active');
      this.captionFieldsWrap.classList.add('tcp-disabled');
    }
    this.updateCaptionWidthState();

    // 테두리 탭 (table 모드에서만 존재)
    if (this.borderCellSpacingInput) {
      this.borderCellSpacingInput.value = hwp16ToMm(tp.cellSpacing).toFixed(1);
      this.populateBorderFromTarget();
    }

    // 배경 탭 (table 모드에서만 존재)
    if (this.bgNoneRadio) {
      this.populateBgFromTarget();
    }
  }

  protected onConfirm(): void {
    const { sec, ppi, ci } = this.tableCtx;

    // 셀 속성 수정
    const newCellProps: Record<string, unknown> = {};
    if (this.cellApplySizeCheck.checked) {
      newCellProps.width = mmToHwpunit(parseFloat(this.cellWidthInput.value) || 0);
      newCellProps.height = mmToHwpunit(parseFloat(this.cellHeightInput.value) || 0);
    }
    newCellProps.applyInnerMargin = this.cellPaddingCheck.checked;
    if (this.cellPaddingCheck.checked) {
      newCellProps.paddingLeft = mmToHwp16(parseFloat(this.cellPaddingInputs['left'].value) || 0);
      newCellProps.paddingRight = mmToHwp16(parseFloat(this.cellPaddingInputs['right'].value) || 0);
      newCellProps.paddingTop = mmToHwp16(parseFloat(this.cellPaddingInputs['top'].value) || 0);
      newCellProps.paddingBottom = mmToHwp16(parseFloat(this.cellPaddingInputs['bottom'].value) || 0);
    }
    const activeVAlign = this.cellVAlignBtns.findIndex(b => b.classList.contains('active'));
    if (activeVAlign >= 0) newCellProps.verticalAlign = activeVAlign;
    const activeTextDir = this.cellTextDirBtns.findIndex(b => b.classList.contains('active'));
    if (activeTextDir >= 0) newCellProps.textDirection = activeTextDir;
    newCellProps.isHeader = this.cellHeaderCheck.checked;
    newCellProps.cellProtect = this.cellProtectCheck.checked;
    newCellProps.fieldName = this.cellFieldNameInput.value;
    newCellProps.editableInForm = this.cellEditableCheck.checked;

    // 셀 테두리/배경 (cell 모드에서는 테두리/배경 탭이 없으므로 스킵)
    if (this.mode === 'table' && this.borderTarget === 'cell' && this.borderEdits) {
      newCellProps.borderLeft = this.borderEdits[0];
      newCellProps.borderRight = this.borderEdits[1];
      newCellProps.borderTop = this.borderEdits[2];
      newCellProps.borderBottom = this.borderEdits[3];
    }
    if (this.mode === 'table' && this.bgTarget === 'cell' && this.bgColorRadio) {
      if (this.bgColorRadio.checked) {
        newCellProps.fillType = 'solid';
        newCellProps.fillColor = this.bgColorPicker.value;
        newCellProps.patternColor = this.bgPatternColorPicker.value;
        newCellProps.patternType = parseInt(this.bgPatternTypeSelect.value, 10);
      } else {
        newCellProps.fillType = 'none';
      }
    }

    // 표 속성 수정
    const pbValue = parseInt(this.tablePageBreakSelect.value, 10);
    const newTableProps: Record<string, unknown> = {
      treatAsChar: this.treatAsCharCheck.checked,
      textWrap: this.getSelectedWrap(),
      vertRelTo: this.vertRelSelect.value,
      vertAlign: this.vertAlignSelect.value,
      vertOffset: mmToHwpunit(parseFloat(this.vertOffsetInput.value) || 0),
      horzRelTo: this.horzRelSelect.value,
      horzAlign: this.horzAlignSelect.value,
      horzOffset: mmToHwpunit(parseFloat(this.horzOffsetInput.value) || 0),
      restrictInPage: this.restrictInPageCheck.checked,
      allowOverlap: this.allowOverlapCheck.checked,
      keepWithAnchor: this.keepWithAnchorCheck.checked,
      pageBreak: pbValue,
      repeatHeader: this.tableRepeatHeaderCheck.checked,
      paddingLeft: mmToHwp16(parseFloat(this.tablePaddingInputs['left'].value) || 0),
      paddingRight: mmToHwp16(parseFloat(this.tablePaddingInputs['right'].value) || 0),
      paddingTop: mmToHwp16(parseFloat(this.tablePaddingInputs['top'].value) || 0),
      paddingBottom: mmToHwp16(parseFloat(this.tablePaddingInputs['bottom'].value) || 0),
      cellSpacing: this.borderCellSpacingInput ? mmToHwp16(parseFloat(this.borderCellSpacingInput.value) || 0) : this.tableProps.cellSpacing,
      // 바깥 여백
      outerLeft: mmToHwp16(parseFloat(this.marginOuterInputs['left'].value) || 0),
      outerRight: mmToHwp16(parseFloat(this.marginOuterInputs['right'].value) || 0),
      outerTop: mmToHwp16(parseFloat(this.marginOuterInputs['top'].value) || 0),
      outerBottom: mmToHwp16(parseFloat(this.marginOuterInputs['bottom'].value) || 0),
    };

    // 캡션 속성 (가운데 = 캡션 없음)
    const activeCapBtn = this.captionPosBtns.find(b => b.classList.contains('active'));
    const capDir = activeCapBtn ? parseInt(activeCapBtn.dataset.dir!, 10) : -1;
    newTableProps.hasCaption = capDir !== -1;
    if (capDir !== -1) {
      newTableProps.captionDirection = parseInt(this.captionDirSelect.value, 10);
      const activeCapBtn = this.captionPosBtns.find(b => b.classList.contains('active'));
      newTableProps.captionVertAlign = activeCapBtn ? parseInt(activeCapBtn.dataset.sub!, 10) : 0;
      newTableProps.captionSpacing = mmToHwp16(parseFloat(this.captionSpacingInput.value) || 0);
      newTableProps.captionWidth = mmToHwpunit(parseFloat(this.captionWidthInput.value) || 0);
    }

    // 표 테두리/배경 (table 모드에서만 테두리/배경 탭 존재)
    if (this.mode === 'table' && this.borderTarget === 'table' && this.borderEdits) {
      newTableProps.borderLeft = this.borderEdits[0];
      newTableProps.borderRight = this.borderEdits[1];
      newTableProps.borderTop = this.borderEdits[2];
      newTableProps.borderBottom = this.borderEdits[3];
    }
    if (this.mode === 'table' && this.bgTarget === 'table' && this.bgColorRadio) {
      if (this.bgColorRadio.checked) {
        newTableProps.fillType = 'solid';
        newTableProps.fillColor = this.bgColorPicker.value;
        newTableProps.patternColor = this.bgPatternColorPicker.value;
        newTableProps.patternType = parseInt(this.bgPatternTypeSelect.value, 10);
      } else {
        newTableProps.fillType = 'none';
      }
    }

    const applyProps = () => {
      this.wasm.setCellProperties(sec, ppi, ci, this.cellIdx, newCellProps as Partial<CellProperties>);
      this.wasm.setTableProperties(sec, ppi, ci, newTableProps as Partial<TableProperties>);
    };
    // 표/셀 속성 변경도 undo 대상이다 — 편집 라우터를 통과시켜 스냅샷으로
    // 기록한다 (#1320 계약, picture-props-dialog(#2027)와 동일 패턴).
    // services 미주입 환경에서만 직접 적용 fallback.
    const ih = this.services?.getInputHandler();
    if (ih) {
      ih.executeOperation({
        kind: 'snapshot',
        operationType: 'objectProps',
        operation: () => {
          applyProps();
          return ih.getCursorPosition();
        },
      });
    } else {
      applyProps();
      this.eventBus.emit('document-changed');
    }
  }

  // ─── "모두(A)" 일괄 여백 스피너 ─────────────────────

  /** 4방향 여백 입력을 일괄 조정하는 "모두(A)" 스피너 생성 */
  buildAllSpinner(inputs: Record<string, HTMLInputElement>): HTMLElement {
    const wrap = document.createElement('div');
    wrap.className = 'tcp-all-spinner';
    const lbl = label('모두(A)');
    wrap.appendChild(lbl);

    const setAll = (delta: number) => {
      for (const inp of Object.values(inputs)) {
        const cur = parseFloat(inp.value) || 0;
        inp.value = Math.max(0, cur + delta).toFixed(1);
      }
    };

    const upBtn = document.createElement('button');
    upBtn.type = 'button';
    upBtn.className = 'tcp-all-spinner-btn';
    upBtn.textContent = '▲';
    upBtn.addEventListener('click', () => setAll(0.5));

    const downBtn = document.createElement('button');
    downBtn.type = 'button';
    downBtn.className = 'tcp-all-spinner-btn';
    downBtn.textContent = '▼';
    downBtn.addEventListener('click', () => setAll(-0.5));

    wrap.appendChild(upBtn);
    wrap.appendChild(downBtn);

    return wrap;
  }

  // ─── DOM 헬퍼 ────────────────────────────────────

  createSection(title: string): HTMLDivElement {
    const sec = document.createElement('div');
    sec.className = 'dialog-section';
    const t = document.createElement('div');
    t.className = 'dialog-section-title';
    t.textContent = title;
    sec.appendChild(t);
    return sec;
  }

  private numberInput(): HTMLInputElement {
    const inp = document.createElement('input');
    inp.type = 'number';
    inp.className = 'dialog-input';
    inp.step = '0.1';
    inp.min = '0';
    // HTML min 속성은 .value 를 자동으로 clamp 하지 않는다(브라우저는 checkValidity()에서만
    // 검사) — 음수·비정상 값이 그대로 parseFloat 되어 wasm.setCellProperties/setTableProperties
    // 로 전달되는 것을 막는다. (#2838 번호매기기 시작 번호 clamp 누락과 동일 패턴)
    inp.addEventListener('change', () => {
      if (inp.value === '') return;
      const min = inp.min !== '' ? parseFloat(inp.min) : -Infinity;
      const max = inp.max !== '' ? parseFloat(inp.max) : Infinity;
      const v = parseFloat(inp.value);
      if (!Number.isFinite(v)) return;
      const clamped = Math.min(max, Math.max(min, v));
      if (clamped !== v) inp.value = String(clamped);
    });
    return inp;
  }

  /** 탭 빌더 모듈용 numberInput 공개 래퍼 — 본체는 tests/dialog-numberinput-min-clamp.test.ts
   *  가드가 private 원문을 고정하므로 private 을 유지한다. */
  tabNumberInput(): HTMLInputElement {
    return this.numberInput();
  }

  checkbox(text: string): HTMLInputElement {
    const lbl = document.createElement('label');
    lbl.className = 'dialog-checkbox';
    const inp = document.createElement('input');
    inp.type = 'checkbox';
    lbl.appendChild(inp);
    lbl.appendChild(document.createTextNode(text));
    return inp;
  }

  setButtonGroupActive(btns: HTMLButtonElement[], idx: number): void {
    btns.forEach((b, i) => b.classList.toggle('active', i === idx));
  }
}
