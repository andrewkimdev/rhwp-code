/**
 * hwpx-template-engine 마커 authoring 도킹 패널.
 *
 * status bar의 eventBus 구독 패턴(main.ts의 `#sb-field` 배선)을 따른다 — 모달이
 * 아니라 `#editor-area` 안에 항상 존재하는 `<aside>`이고, 표시/숨김은
 * `template:toggle-panel` 커맨드(`command/commands/template.ts`)가 `hidden`
 * 속성으로 토글한다. 내부 DOM은 command-palette.ts처럼 전부 여기서 만든다 —
 * index.html에는 빈 컨테이너만 있다.
 *
 * `command-state-changed`는 커서 이동마다(사실상 매 클릭/키 입력) 발생하므로,
 * 패널이 숨겨진 동안은 `refresh()`가 즉시 반환해 불필요한 재렌더링을 피한다.
 *
 * 역할 선택은 라디오 그룹 2개("일반"/"반복 블록") + 중첩 체크박스 1개로
 * 구성한다(11개 항목을 한 줄로 나열하던 select 대신) — `-NESTED:` 4종은 실제로는
 * 별도 역할이 아니라 "반복 블록 역할 + 이미 있는 `#REPEAT-BODY:<name>` 표 아래
 * 중첩"이라는 수정자이므로, 체크박스로 표현하는 쪽이 실제 마커 문법과 더 정확히
 * 대응하고 전체 어휘도 한눈에 보인다. 문서에 `#REPEAT-BODY:<name>` 표가 하나도
 * 없으면(`availableNestedParentBlockNames`, table-outline.ts) 중첩 체크박스
 * 자체를 감춘다(`updateNestedRoleAvailability`) — 골라도 만들 수 없는 옵션이라서다.
 * v1은 존재 여부만 확인한다 — 정확한 인접성 재검증은 `#6. "Validate now"`의 실제
 * lint가 최종 권위다.
 *
 * 각 역할에는 `hwpx-template-engine/docs/TEMPLATE_MARKER_SYNTAX.md` §3/§3e 요약을
 * 한 줄로 압축한 설명이 붙는다 — 라디오 `label`의 `title`(hover)과, 선택된 역할이
 * 바뀔 때마다 갱신되는 `roleDescEl`(항상 보이는 캡션) 두 군데에 쓴다.
 *
 * 역할 라디오 1~7과 중첩 체크박스(N)에는 숫자/문자 키보드 단축키가 있다 —
 * `#template-panel` 컨테이너의 `keydown`에서 처리하며(전역 단축키가 아니다),
 * 블록명 입력/부모 블록 select에 포커스가 있을 때는 숫자를 그대로 입력해야 하므로
 * 그 두 컨트롤에 포커스가 있으면 무시한다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { EventBus } from '@/core/event-bus';
import type { CommandDispatcher } from '@/command/dispatcher';
import type { InputHandler } from '@/engine/input-handler';
import {
  listTopLevelTables,
  availableNestedParentBlockNames,
  findCellIndexForRowCol,
  type TableOutlineEntry,
} from '@/core/table-outline';
import { buildTableRoleMarkerText, type TemplateTableRole } from '@/command/commands/template';

interface GeneralRoleOption {
  value: 'HEADER' | 'FOOTER' | 'PAGENO';
  label: string;
  description: string;
}

interface RepeatRoleOption {
  value: 'REPEAT_TITLE' | 'REPEAT_HEADER' | 'REPEAT_BODY' | 'REPEAT_FOOTER';
  nestedValue: 'REPEAT_TITLE_NESTED' | 'REPEAT_HEADER_NESTED' | 'REPEAT_BODY_NESTED' | 'REPEAT_FOOTER_NESTED';
  label: string;
  description: string;
}

const GENERAL_ROLES: readonly GeneralRoleOption[] = [
  { value: 'HEADER', label: '#HEADER', description: '표의 역할을 문서화만 합니다 — 렌더링에는 영향을 주지 않습니다.' },
  { value: 'FOOTER', label: '#FOOTER', description: '표의 역할을 문서화만 합니다 — 렌더링에는 영향을 주지 않습니다.' },
  { value: 'PAGENO', label: '#PAGENO', description: '표 전체를 반복 꼬리말로 승격해 모든 페이지에 쪽 번호를 표시합니다. 문서당 최대 1개.' },
];

const REPEAT_ROLES: readonly RepeatRoleOption[] = [
  { value: 'REPEAT_TITLE', nestedValue: 'REPEAT_TITLE_NESTED', label: '#REPEAT-TITLE:', description: '반복 블록 전체의 제목 표 — 문서에서 한 번만 렌더됩니다. (선택)' },
  { value: 'REPEAT_HEADER', nestedValue: 'REPEAT_HEADER_NESTED', label: '#REPEAT-HEADER:', description: '컬럼 라벨 표 — 블록 맨 앞에 한 번, 항목이 페이지를 넘기면 다음 페이지 첫머리에도 다시 찍힙니다. (선택)' },
  { value: 'REPEAT_BODY', nestedValue: 'REPEAT_BODY_NESTED', label: '#REPEAT-BODY:', description: '항목 1개 분량의 표 — 데이터 개수만큼 복제됩니다. (필수, 블록당 정확히 1개)' },
  { value: 'REPEAT_FOOTER', nestedValue: 'REPEAT_FOOTER_NESTED', label: '#REPEAT-FOOTER:', description: '항목들 뒤에 한 번만 렌더되는 합계 등 요약 표. (선택)' },
];

/** 역할 값 → 설명 한 줄. 라디오 label의 title(hover)과 상시 캡션(roleDescEl)이 공유한다. */
const ROLE_DESCRIPTIONS: Readonly<Record<string, string>> = Object.fromEntries(
  [...GENERAL_ROLES, ...REPEAT_ROLES].map(r => [r.value, r.description]),
);

const NESTED_TOGGLE_DESCRIPTION =
  '이미 있는 #REPEAT-BODY:<부모블록> 표 바로 뒤에, 그 항목마다 자기만의 하위 반복 목록을 붙입니다(최대 1단계 중첩).';

const DEFAULT_ROLE: TemplateTableRole = GENERAL_ROLES[0].value;

export class TemplatePanel {
  private emptyEl!: HTMLElement;
  private contentEl!: HTMLElement;
  private outlineEl!: HTMLElement;
  private hintEl!: HTMLElement;
  private roleRadios: HTMLInputElement[] = [];
  private roleDescEl!: HTMLElement;
  private nestedRoleAvailable = false;
  private blockNameField!: HTMLElement;
  private blockNameInput!: HTMLInputElement;
  /** 사용자가 블록명을 직접 입력했는지 — 자동 제안이 이를 덮어쓰지 않게 막는다. */
  private blockNameManuallyEdited = false;
  private nestedToggleField!: HTMLElement;
  private nestedToggle!: HTMLInputElement;
  private nestedParentField!: HTMLElement;
  private nestedParentSelect!: HTMLSelectElement;
  private previewEl!: HTMLElement;
  private applyBtn!: HTMLButtonElement;
  private clearBtn!: HTMLButtonElement;

  constructor(
    private container: HTMLElement,
    private wasm: WasmBridge,
    private eventBus: EventBus,
    private dispatcher: CommandDispatcher,
    private getInputHandler: () => InputHandler | null,
  ) {
    this.build();
    this.eventBus.on('table-object-selection-changed', () => this.refresh());
    this.eventBus.on('command-state-changed', () => this.refresh());
    this.eventBus.on('template-panel-visibility-changed', () => this.refresh());
    // 패널이 기본으로 열려 있으므로, 문서 로드가 처음 refresh()를 유발하기 전에도
    // 빈 상태/내용 영역이 올바르게 갈라져 있어야 한다(둘 다 노출되는 걸 방지).
    this.refresh();
  }

  /** 문서 로드 등 다른 트리거에서 강제로 다시 그리고 싶을 때 (main.ts에서 호출). */
  refresh(): void {
    if (this.container.hidden) return; // 숨겨진 동안은 갱신하지 않는다

    if (this.wasm.pageCount <= 0) {
      this.emptyEl.style.display = '';
      this.contentEl.style.display = 'none';
      return;
    }
    this.emptyEl.style.display = 'none';
    this.contentEl.style.display = '';

    const ih = this.getInputHandler();
    const pos = ih?.getCursorPosition();
    const sec = pos?.sectionIndex ?? 0;

    let entries: TableOutlineEntry[] = [];
    try {
      entries = listTopLevelTables(this.wasm, sec);
    } catch (err) {
      console.warn('[template-panel] 표 개요 조회 실패:', err);
    }

    this.renderOutline(entries, pos);
    const nestedParentNames = availableNestedParentBlockNames(entries);
    this.renderNestedParentOptions(nestedParentNames);
    this.updateNestedRoleAvailability(nestedParentNames.length > 0);

    const inTable = pos?.parentParaIndex !== undefined && pos?.controlIndex !== undefined;
    const isNested = (pos?.cellPath?.length ?? 0) > 1;
    const canTag = Boolean(inTable) && !isNested;

    let hintText: string;
    let hintWarning = false;
    if (!inTable) {
      hintText = '표 안에 커서를 두면 태깅할 수 있습니다.';
    } else if (isNested) {
      hintText = '중첩 표는 아직 지원하지 않습니다.';
      hintWarning = true;
    } else {
      hintText = this.describeSelectedRows(ih, pos!);
    }
    this.hintEl.textContent = hintText;
    this.hintEl.classList.toggle('tp-hint--warning', hintWarning);

    this.updatePreview();
  }

  private describeSelectedRows(
    ih: InputHandler | null,
    pos: NonNullable<ReturnType<InputHandler['getCursorPosition']>>,
  ): string {
    const range = ih?.isInCellSelectionMode() ? ih.getSelectedCellRange() : null;
    let startRow = range?.startRow;
    let endRow = range?.endRow;
    if (startRow === undefined && pos.cellIndex !== undefined) {
      try {
        const info = this.wasm.getCellInfo(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex);
        startRow = info.row;
        endRow = info.row;
      } catch {
        // 무시 — 아래 fallback 문구를 쓴다
      }
    }
    if (startRow === undefined || endRow === undefined) {
      return '표 안에 커서를 두면 태깅할 수 있습니다.';
    }
    return startRow === endRow ? `선택된 행: ${startRow + 1}` : `선택된 행: ${startRow + 1}~${endRow + 1}`;
  }

  /**
   * REPEAT_TITLE 역할일 때, 현재 셀(셀 선택 모드면 선택 범위 좌상단 셀)의
   * 텍스트에서 공백을 모두 제거해 블록명 입력란에 제안한다. 사용자가 이미
   * 손댔거나(blockNameManuallyEdited) 입력란에 값이 있으면 절대 덮어쓰지 않는다.
   * REPEAT_TITLE에만 한정하는 이유: 이 역할만 "셀 하나의 텍스트 = 블록명"이라는
   * 전제가 성립한다(HEADER는 여러 라벨 셀, BODY/FOOTER는 반복되는 데이터 행).
   */
  private suggestBlockNameFromCurrentCell(
    ih: InputHandler | null,
    pos: ReturnType<InputHandler['getCursorPosition']> | undefined,
  ): void {
    if (this.selectedRepeatRole()?.value !== 'REPEAT_TITLE') return;
    if (this.blockNameManuallyEdited) return;
    if (this.blockNameInput.value.trim() !== '') return;
    if (!pos || pos.parentParaIndex === undefined || pos.controlIndex === undefined) return;
    if ((pos.cellPath?.length ?? 0) > 1) return; // 중첩 표는 지원하지 않음

    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex;
    const ci = pos.controlIndex;
    try {
      const range = ih?.isInCellSelectionMode() ? ih.getSelectedCellRange() : null;
      const cellIdx = range
        ? findCellIndexForRowCol(this.wasm, sec, ppi, ci, range.startRow, range.startCol)
        : (pos.cellIndex ?? null);
      if (cellIdx === null) return;

      const len = this.wasm.getCellParagraphLength(sec, ppi, ci, cellIdx, 0);
      if (len <= 0) return;
      const text = this.wasm.getTextInCell(sec, ppi, ci, cellIdx, 0, 0, len);
      const stripped = text.replace(/\s+/g, ''); // trim보다 강함 — 텍스트 내부 공백까지 전부 제거
      if (!stripped) return;

      this.blockNameInput.value = stripped;
      this.updatePreview();
    } catch {
      // 조회 실패는 조용히 무시 — 제안은 optional UX이지 필수 경로가 아니다
    }
  }

  private renderOutline(
    entries: readonly TableOutlineEntry[],
    pos: ReturnType<InputHandler['getCursorPosition']> | undefined,
  ): void {
    this.outlineEl.textContent = '';
    if (entries.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'tp-outline-empty';
      empty.textContent = '최상위 표가 없습니다.';
      this.outlineEl.appendChild(empty);
      return;
    }
    entries.forEach((entry, idx) => {
      const item = document.createElement('div');
      const isCurrent = pos?.parentParaIndex === entry.parentPara && pos?.controlIndex === entry.controlIndex;
      item.className = isCurrent ? 'tp-outline-item tp-outline-item--active' : 'tp-outline-item';

      const indexEl = document.createElement('span');
      indexEl.className = 'tp-outline-index';
      indexEl.textContent = `#${idx + 1} · ${entry.rowCount}행 ${entry.colCount}열`;

      const markerEl = document.createElement('span');
      markerEl.className = entry.markerText ? 'tp-outline-marker' : 'tp-outline-marker tp-outline-marker--none';
      markerEl.textContent = entry.markerText ?? '역할 없음';

      item.appendChild(indexEl);
      item.appendChild(markerEl);
      this.outlineEl.appendChild(item);
    });
  }

  private renderNestedParentOptions(names: readonly string[]): void {
    const current = this.nestedParentSelect.value;
    this.nestedParentSelect.textContent = '';
    for (const name of names) {
      const opt = document.createElement('option');
      opt.value = name;
      opt.textContent = name;
      this.nestedParentSelect.appendChild(opt);
    }
    if (names.includes(current)) this.nestedParentSelect.value = current;
  }

  /**
   * `#REPEAT-BODY:<name>` 표가 문서에 하나도 없으면 중첩 체크박스 자체를 감추고
   * 해제한다 — 나열 가능한 부모가 없으므로 체크해도 만들 수 없는 옵션이라서다.
   */
  private updateNestedRoleAvailability(available: boolean): void {
    this.nestedRoleAvailable = available;
    if (!available && this.nestedToggle.checked) {
      this.nestedToggle.checked = false;
    }
    this.onRoleChanged();
  }

  private selectedRepeatRole(): RepeatRoleOption | null {
    const checked = this.roleRadios.find(r => r.checked)?.value;
    return REPEAT_ROLES.find(r => r.value === checked) ?? null;
  }

  private resolveSelectedRole(): TemplateTableRole {
    const repeat = this.selectedRepeatRole();
    if (repeat && this.nestedToggle.checked) return repeat.nestedValue;
    return (this.roleRadios.find(r => r.checked)?.value as TemplateTableRole | undefined) ?? DEFAULT_ROLE;
  }

  private onRoleChanged(): void {
    const repeat = this.selectedRepeatRole();
    this.blockNameField.style.display = repeat ? '' : 'none';
    const nestedTogglable = Boolean(repeat) && this.nestedRoleAvailable;
    this.nestedToggleField.style.display = nestedTogglable ? '' : 'none';
    if (!nestedTogglable) this.nestedToggle.checked = false;
    this.nestedParentField.style.display = (nestedTogglable && this.nestedToggle.checked) ? '' : 'none';
    const checkedValue = this.roleRadios.find(r => r.checked)?.value;
    this.roleDescEl.textContent = (checkedValue && ROLE_DESCRIPTIONS[checkedValue]) || '';
    const ih = this.getInputHandler();
    this.suggestBlockNameFromCurrentCell(ih, ih?.getCursorPosition());
    this.updatePreview();
  }

  private buildParamsFromForm(): { role: TemplateTableRole; blockName?: string; nestedParent?: string } {
    const repeat = this.selectedRepeatRole();
    const nested = Boolean(repeat) && this.nestedToggle.checked;
    return {
      role: this.resolveSelectedRole(),
      blockName: repeat ? (this.blockNameInput.value.trim() || undefined) : undefined,
      nestedParent: nested ? (this.nestedParentSelect.value || undefined) : undefined,
    };
  }

  /**
   * 마커 미리보기 텍스트를 갱신하고, 그 성공/실패를 그대로 '태그 지정' 버튼
   * 활성화 판정에도 쓴다 — `buildTableRoleMarkerText`가 던지는 예외
   * (`requireBlockName`/`requireNestedPair`, template.ts)가 곧 실제 커맨드
   * 실행 시 필요한 조건과 동일하므로, 이중으로 검증 로직을 만들지 않는다.
   * 블록명/부모블록/역할/커서 위치 중 하나라도 바뀔 수 있는 모든 지점
   * (refresh, onRoleChanged, 블록명 input, 부모블록 select)이 이미
   * updatePreview()를 호출하므로 버튼 상태도 그 지점들에서 자동으로 갱신된다.
   */
  private updatePreview(): void {
    let markerValid = true;
    try {
      this.previewEl.textContent = buildTableRoleMarkerText(this.buildParamsFromForm());
    } catch {
      this.previewEl.textContent = '(블록명을 입력하세요)';
      markerValid = false;
    }
    const ih = this.getInputHandler();
    const pos = ih?.getCursorPosition();
    const inTable = pos?.parentParaIndex !== undefined && pos?.controlIndex !== undefined;
    const isNested = (pos?.cellPath?.length ?? 0) > 1;
    const canTag = Boolean(inTable) && !isNested;
    this.clearBtn.disabled = !canTag;
    this.applyBtn.disabled = !canTag || !markerValid;
  }

  private applyTag(): void {
    this.dispatcher.dispatch('template:tag-selection', this.buildParamsFromForm());
    this.refresh();
  }

  private clearTag(): void {
    this.dispatcher.dispatch('template:clear-marker');
    this.refresh();
  }

  private buildRoleGroup(
    legendText: string,
    roles: readonly { value: string; label: string; description: string }[],
  ): HTMLFieldSetElement {
    const fieldset = document.createElement('fieldset');
    fieldset.className = 'tp-role-group';
    const legend = document.createElement('legend');
    legend.className = 'tp-role-group-legend';
    legend.textContent = legendText;
    fieldset.appendChild(legend);
    for (const role of roles) {
      const label = document.createElement('label');
      label.className = 'tp-radio-label';
      label.title = role.description;
      const input = document.createElement('input');
      input.type = 'radio';
      input.name = 'tp-role';
      input.value = role.value;
      input.checked = role.value === DEFAULT_ROLE;
      input.addEventListener('change', () => {
        this.blockNameManuallyEdited = false;
        this.onRoleChanged();
      });
      // 단축키 배지(1~7)는 buildRoleGroup이 두 그룹에 걸쳐 순서대로 불리므로
      // roleRadios.length(=push 전 현재 길이)가 곧 0-based 순번이다 —
      // 실제 키 처리는 이 순번 그대로 this.roleRadios를 인덱싱한다(handleShortcutKey).
      const keyBadge = document.createElement('span');
      keyBadge.className = 'tp-role-key';
      keyBadge.textContent = String(this.roleRadios.length + 1);
      this.roleRadios.push(input);
      label.appendChild(input);
      label.appendChild(document.createTextNode(role.label));
      label.appendChild(keyBadge);
      fieldset.appendChild(label);
    }
    return fieldset;
  }

  /**
   * 패널 안(블록명 입력/부모 블록 select 제외)에서 숫자 1~7 → 해당 역할 라디오,
   * `n` → 중첩 체크박스(보일 때만) 토글. 조합키가 있으면 무시해 브라우저/전역
   * 단축키와 겹치지 않게 한다. 전역 리스너가 아니라 `#template-panel`의
   * keydown이라 포커스가 패널 밖(에디터 캔버스 등)에 있을 때는 애초에 안 불린다.
   */
  private handleShortcutKey(e: KeyboardEvent): void {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target === this.blockNameInput || e.target === this.nestedParentSelect) return;

    const digit = Number(e.key);
    if (Number.isInteger(digit) && digit >= 1 && digit <= this.roleRadios.length) {
      e.preventDefault();
      const radio = this.roleRadios[digit - 1];
      radio.checked = true;
      radio.focus();
      this.onRoleChanged();
      return;
    }
    if (e.key.toLowerCase() === 'n' && this.nestedToggleField.style.display !== 'none') {
      e.preventDefault();
      this.nestedToggle.checked = !this.nestedToggle.checked;
      this.nestedToggle.focus();
      this.onRoleChanged();
    }
  }

  private build(): void {
    const header = document.createElement('div');
    header.className = 'tp-header';
    const title = document.createElement('span');
    title.className = 'tp-title';
    title.textContent = '템플릿';
    const closeBtn = document.createElement('button');
    closeBtn.type = 'button';
    closeBtn.className = 'tp-close';
    closeBtn.textContent = '×';
    closeBtn.title = '패널 닫기';
    closeBtn.addEventListener('click', () => this.dispatcher.dispatch('template:toggle-panel'));
    header.appendChild(title);
    header.appendChild(closeBtn);

    const body = document.createElement('div');
    body.className = 'tp-body';

    this.emptyEl = document.createElement('div');
    this.emptyEl.className = 'tp-empty';
    this.emptyEl.textContent = '문서를 열면 표 개요와 마커 지정 도구가 표시됩니다.';

    this.contentEl = document.createElement('div');
    this.contentEl.className = 'tp-content';

    // 표 개요
    const outlineSection = document.createElement('div');
    const outlineLabel = document.createElement('div');
    outlineLabel.className = 'tp-section-label';
    outlineLabel.textContent = '표 개요';
    this.outlineEl = document.createElement('div');
    this.outlineEl.className = 'tp-outline';
    outlineSection.appendChild(outlineLabel);
    outlineSection.appendChild(this.outlineEl);

    // 선택 상태 힌트
    this.hintEl = document.createElement('div');
    this.hintEl.className = 'tp-hint';

    // 역할 선택 — 라디오 그룹 2개("일반"/"반복 블록")
    const roleField = document.createElement('div');
    roleField.className = 'tp-field';
    const roleLabel = document.createElement('span');
    roleLabel.className = 'tp-label';
    roleLabel.textContent = '역할';
    roleField.appendChild(roleLabel);
    roleField.appendChild(this.buildRoleGroup('일반', GENERAL_ROLES));
    roleField.appendChild(this.buildRoleGroup('반복 블록', REPEAT_ROLES));
    this.roleDescEl = document.createElement('div');
    this.roleDescEl.className = 'tp-role-desc';
    roleField.appendChild(this.roleDescEl);

    // 블록명
    this.blockNameField = document.createElement('div');
    this.blockNameField.className = 'tp-field';
    const blockNameLabel = document.createElement('label');
    blockNameLabel.className = 'tp-label';
    blockNameLabel.textContent = '블록명';
    this.blockNameInput = document.createElement('input');
    this.blockNameInput.type = 'text';
    this.blockNameInput.className = 'tp-input';
    this.blockNameInput.placeholder = '예: 품목내역';
    this.blockNameInput.addEventListener('input', () => {
      this.blockNameManuallyEdited = true;
      this.updatePreview();
    });
    this.blockNameInput.addEventListener('keydown', (e) => {
      if (e.key !== 'Enter' || e.isComposing) return;
      e.preventDefault();
      if (this.applyBtn.disabled) return;
      this.applyTag();
    });
    this.blockNameField.appendChild(blockNameLabel);
    this.blockNameField.appendChild(this.blockNameInput);

    // 중첩 여부 체크박스 — REPEAT_* 역할에서만, 유효한 부모가 있을 때만 노출
    this.nestedToggleField = document.createElement('div');
    this.nestedToggleField.className = 'tp-field';
    const nestedToggleLabel = document.createElement('label');
    nestedToggleLabel.className = 'tp-checkbox-label';
    nestedToggleLabel.title = NESTED_TOGGLE_DESCRIPTION;
    this.nestedToggle = document.createElement('input');
    this.nestedToggle.type = 'checkbox';
    this.nestedToggle.className = 'tp-checkbox';
    this.nestedToggle.addEventListener('change', () => this.onRoleChanged());
    const nestedKeyBadge = document.createElement('span');
    nestedKeyBadge.className = 'tp-role-key';
    nestedKeyBadge.textContent = 'N';
    nestedToggleLabel.appendChild(this.nestedToggle);
    nestedToggleLabel.appendChild(document.createTextNode('중첩 자식 블록으로 지정'));
    nestedToggleLabel.appendChild(nestedKeyBadge);
    this.nestedToggleField.appendChild(nestedToggleLabel);

    // 중첩 부모 블록명
    this.nestedParentField = document.createElement('div');
    this.nestedParentField.className = 'tp-field';
    const nestedParentLabel = document.createElement('label');
    nestedParentLabel.className = 'tp-label';
    nestedParentLabel.textContent = '부모 블록 (중첩)';
    this.nestedParentSelect = document.createElement('select');
    this.nestedParentSelect.className = 'tp-select';
    this.nestedParentSelect.addEventListener('change', () => this.updatePreview());
    this.nestedParentField.appendChild(nestedParentLabel);
    this.nestedParentField.appendChild(this.nestedParentSelect);

    // 마커 미리보기
    this.previewEl = document.createElement('div');
    this.previewEl.className = 'tp-preview';

    // 액션 버튼
    const actions = document.createElement('div');
    actions.className = 'tp-actions';
    this.applyBtn = document.createElement('button');
    this.applyBtn.type = 'button';
    this.applyBtn.className = 'tp-btn tp-btn--primary';
    this.applyBtn.textContent = '태그 지정';
    this.applyBtn.addEventListener('click', () => this.applyTag());
    this.clearBtn = document.createElement('button');
    this.clearBtn.type = 'button';
    this.clearBtn.className = 'tp-btn';
    this.clearBtn.textContent = '마커 지우기';
    this.clearBtn.addEventListener('click', () => this.clearTag());
    actions.appendChild(this.applyBtn);
    actions.appendChild(this.clearBtn);

    this.contentEl.appendChild(outlineSection);
    this.contentEl.appendChild(this.hintEl);
    this.contentEl.appendChild(roleField);
    this.contentEl.appendChild(this.blockNameField);
    this.contentEl.appendChild(this.nestedToggleField);
    this.contentEl.appendChild(this.nestedParentField);
    this.contentEl.appendChild(this.previewEl);
    this.contentEl.appendChild(actions);

    body.appendChild(this.emptyEl);
    body.appendChild(this.contentEl);

    this.container.appendChild(header);
    this.container.appendChild(body);

    // #780과 같은 이유(icon-toolbar/style-bar) — 패널 안 컨트롤 클릭이 에디터
    // 캐럿에 영향을 주면 안 되지만, 그 컨트롤 자체의 입력/토글은 정상 동작해야
    // 하므로 INPUT/SELECT는 제외하고 preventDefault한다.
    this.container.addEventListener('mousedown', (e) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag !== 'INPUT' && tag !== 'SELECT') e.preventDefault();
    });
    this.container.addEventListener('keydown', (e) => this.handleShortcutKey(e));

    this.onRoleChanged();
  }
}
