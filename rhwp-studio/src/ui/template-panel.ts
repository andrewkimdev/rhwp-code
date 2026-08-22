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
 * `-NESTED:` 역할 4종은 문서에 `#REPEAT-BODY:<name>` 표가 하나도 없으면 역할
 * 선택지에서 감춘다(`updateNestedRoleAvailability`) — 고를 수 있어도 만들 수
 * 없는 옵션이라서다. 부모 후보 목록 자체는 `availableNestedParentBlockNames`
 * (table-outline.ts)가 낸다. v1은 존재 여부만 확인한다 — 정확한 인접성
 * 재검증은 `#6. "Validate now"`의 실제 lint가 최종 권위다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { EventBus } from '@/core/event-bus';
import type { CommandDispatcher } from '@/command/dispatcher';
import type { InputHandler } from '@/engine/input-handler';
import {
  listTopLevelTables,
  availableNestedParentBlockNames,
  type TableOutlineEntry,
} from '@/core/table-outline';
import { buildTableRoleMarkerText, type TemplateTableRole } from '@/command/commands/template';

interface RoleOption {
  value: TemplateTableRole;
  label: string;
  needsBlockName: boolean;
  needsNestedParent: boolean;
}

const ROLE_OPTIONS: readonly RoleOption[] = [
  { value: 'HEADER', label: '#HEADER', needsBlockName: false, needsNestedParent: false },
  { value: 'FOOTER', label: '#FOOTER', needsBlockName: false, needsNestedParent: false },
  { value: 'PAGENO', label: '#PAGENO', needsBlockName: false, needsNestedParent: false },
  { value: 'REPEAT_TITLE', label: '#REPEAT-TITLE:', needsBlockName: true, needsNestedParent: false },
  { value: 'REPEAT_HEADER', label: '#REPEAT-HEADER:', needsBlockName: true, needsNestedParent: false },
  { value: 'REPEAT_BODY', label: '#REPEAT-BODY:', needsBlockName: true, needsNestedParent: false },
  { value: 'REPEAT_FOOTER', label: '#REPEAT-FOOTER:', needsBlockName: true, needsNestedParent: false },
  { value: 'REPEAT_TITLE_NESTED', label: '#REPEAT-TITLE-NESTED: (중첩)', needsBlockName: true, needsNestedParent: true },
  { value: 'REPEAT_HEADER_NESTED', label: '#REPEAT-HEADER-NESTED: (중첩)', needsBlockName: true, needsNestedParent: true },
  { value: 'REPEAT_BODY_NESTED', label: '#REPEAT-BODY-NESTED: (중첩)', needsBlockName: true, needsNestedParent: true },
  { value: 'REPEAT_FOOTER_NESTED', label: '#REPEAT-FOOTER-NESTED: (중첩)', needsBlockName: true, needsNestedParent: true },
];

export class TemplatePanel {
  private emptyEl!: HTMLElement;
  private contentEl!: HTMLElement;
  private outlineEl!: HTMLElement;
  private hintEl!: HTMLElement;
  private roleSelect!: HTMLSelectElement;
  private nestedRoleOptionEls: HTMLOptionElement[] = [];
  private blockNameField!: HTMLElement;
  private blockNameInput!: HTMLInputElement;
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

    this.applyBtn.disabled = !canTag;
    this.clearBtn.disabled = !canTag;

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
   * `#REPEAT-BODY:<name>` 표가 문서에 하나도 없으면 `-NESTED:` 역할 옵션 자체를
   * 역할 선택지에서 감춘다 — 나열 가능한 부모가 없으므로 골라도 만들 수 없는
   * 옵션이라서다. 이미 그 역할이 선택된 채로 부모가 사라지면(마커 지우기 등)
   * 기본 역할로 되돌린다.
   */
  private updateNestedRoleAvailability(available: boolean): void {
    for (const el of this.nestedRoleOptionEls) el.hidden = !available;
    if (!available && this.currentRoleOption().needsNestedParent) {
      this.roleSelect.value = ROLE_OPTIONS[0].value;
      this.onRoleChanged();
    }
  }

  private currentRoleOption(): RoleOption {
    return ROLE_OPTIONS.find(o => o.value === this.roleSelect.value) ?? ROLE_OPTIONS[0];
  }

  private onRoleChanged(): void {
    const opt = this.currentRoleOption();
    this.blockNameField.style.display = opt.needsBlockName ? '' : 'none';
    this.nestedParentField.style.display = opt.needsNestedParent ? '' : 'none';
    this.updatePreview();
  }

  private buildParamsFromForm(): { role: TemplateTableRole; blockName?: string; nestedParent?: string } {
    const opt = this.currentRoleOption();
    return {
      role: opt.value,
      blockName: opt.needsBlockName ? (this.blockNameInput.value.trim() || undefined) : undefined,
      nestedParent: opt.needsNestedParent ? (this.nestedParentSelect.value || undefined) : undefined,
    };
  }

  private updatePreview(): void {
    try {
      this.previewEl.textContent = buildTableRoleMarkerText(this.buildParamsFromForm());
    } catch {
      this.previewEl.textContent = '(블록명을 입력하세요)';
    }
  }

  private applyTag(): void {
    this.dispatcher.dispatch('template:tag-selection', this.buildParamsFromForm());
    this.refresh();
  }

  private clearTag(): void {
    this.dispatcher.dispatch('template:clear-marker');
    this.refresh();
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

    // 역할 선택
    const roleField = document.createElement('div');
    roleField.className = 'tp-field';
    const roleLabel = document.createElement('label');
    roleLabel.className = 'tp-label';
    roleLabel.textContent = '역할';
    this.roleSelect = document.createElement('select');
    this.roleSelect.className = 'tp-select';
    for (const opt of ROLE_OPTIONS) {
      const el = document.createElement('option');
      el.value = opt.value;
      el.textContent = opt.label;
      this.roleSelect.appendChild(el);
      if (opt.needsNestedParent) this.nestedRoleOptionEls.push(el);
    }
    this.roleSelect.addEventListener('change', () => this.onRoleChanged());
    roleField.appendChild(roleLabel);
    roleField.appendChild(this.roleSelect);

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
    this.blockNameInput.addEventListener('input', () => this.updatePreview());
    this.blockNameField.appendChild(blockNameLabel);
    this.blockNameField.appendChild(this.blockNameInput);

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
    this.contentEl.appendChild(this.nestedParentField);
    this.contentEl.appendChild(this.previewEl);
    this.contentEl.appendChild(actions);

    body.appendChild(this.emptyEl);
    body.appendChild(this.contentEl);

    this.container.appendChild(header);
    this.container.appendChild(body);

    // #780과 같은 이유(icon-toolbar/style-bar) — 패널 안 select/input 클릭이
    // 에디터 캐럿에 영향을 주면 안 되지만, 텍스트 입력 자체는 정상 동작해야
    // 하므로 INPUT/SELECT는 제외하고 preventDefault한다.
    this.container.addEventListener('mousedown', (e) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag !== 'INPUT' && tag !== 'SELECT') e.preventDefault();
    });

    this.onRoleChanged();
  }
}
