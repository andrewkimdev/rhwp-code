/**
 * hwpx-template-engine 마커 authoring 도킹 패널 — 섹션 모듈들의 조립자.
 *
 * status bar의 eventBus 구독 패턴(main.ts의 `#sb-field` 배선)을 따른다 — 모달이
 * 아니라 `#editor-area` 안에 항상 존재하는 `<aside>`이고, 표시/숨김은
 * `template:toggle-panel` 커맨드(`command/commands/template.ts`)가 `hidden`
 * 속성으로 토글한다. 내부 DOM은 command-palette.ts처럼 전부 여기서 만든다 —
 * index.html에는 빈 컨테이너만 있다.
 *
 * 이 파일(panel.ts)은 패널 뼈대(빈/내용 게이트, 표 개요, 선택 힌트, 마커
 * 미리보기·액션 버튼)와 `refresh()` 오케스트레이션만 담는다. 각 섹션은 분리된
 * 모듈이다:
 * - roles.ts — 역할 라디오/블록명/중첩 폼(RoleForm)과 숫자 단축키(1~7, N)
 * - fieldsuggest.ts — 누름틀 만들기(FieldSuggestSection)
 * - entity-section.ts + entity-window.ts — Java 엔티티 생성
 * - row-range.ts — 힌트와 누름틀 생성이 공유하는 "선택된 행" 정의
 *
 * `command-state-changed`는 커서 이동마다(사실상 매 클릭/키 입력) 발생하므로,
 * 패널이 숨겨진 동안은 `refresh()`가 즉시 반환해 불필요한 재렌더링을 피한다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { EventBus } from '@/core/event-bus';
import type { CommandDispatcher } from '@/command/dispatcher';
import type { InputHandler } from '@/engine/input-handler';
import { listTopLevelTables, availableNestedParentBlockNames, type TableOutlineEntry } from '@/core/table-outline';
import { buildTableRoleMarkerText } from '@/core/template-marker';
import { RoleForm } from './roles';
import { FieldSuggestSection } from './fieldsuggest';
import { EntitySection } from './entity-section';
import { describeSelectedRows } from './row-range';

export class TemplatePanel {
  private emptyEl!: HTMLElement;
  private contentEl!: HTMLElement;
  private outlineEl!: HTMLElement;
  private hintEl!: HTMLElement;
  private previewEl!: HTMLElement;
  private applyBtn!: HTMLButtonElement;
  private clearBtn!: HTMLButtonElement;

  private roles!: RoleForm;
  private fieldSuggest!: FieldSuggestSection;
  private entity!: EntitySection;

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

    this.entity.refresh(this.wasm.getSourceFormat() === 'hwpx', this.wasm.fileName);

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
    this.roles.renderNestedParentOptions(nestedParentNames);
    this.roles.setNestedAvailable(nestedParentNames.length > 0);

    const inTable = pos?.parentParaIndex !== undefined && pos?.controlIndex !== undefined;
    const isNested = (pos?.cellPath?.length ?? 0) > 1;

    let hintText: string;
    let hintWarning = false;
    if (!inTable) {
      hintText = '표 안에 커서를 두면 태깅할 수 있습니다.';
    } else if (isNested) {
      hintText = '중첩 표는 아직 지원하지 않습니다.';
      hintWarning = true;
    } else {
      hintText = describeSelectedRows(this.wasm, ih, pos!);
    }
    this.hintEl.textContent = hintText;
    this.hintEl.classList.toggle('tp-hint--warning', hintWarning);

    this.updatePreview();
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

  /**
   * 마커 미리보기 텍스트를 갱신하고, 그 성공/실패를 그대로 '태그 지정' 버튼
   * 활성화 판정에도 쓴다 — `buildTableRoleMarkerText`가 던지는 예외
   * (`requireBlockName`/`requireNestedPair`, core/template-marker.ts)가 곧 실제
   * 커맨드 실행 시 필요한 조건과 동일하므로, 이중으로 검증 로직을 만들지 않는다.
   * 블록명/부모블록/역할/커서 위치 중 하나라도 바뀔 수 있는 모든 지점
   * (refresh, RoleForm의 역할 변경·블록명 input·부모블록 select)이 이미
   * onParamsChanged → updatePreview()를 호출하므로 버튼 상태도 그 지점들에서
   * 자동으로 갱신된다.
   */
  private updatePreview(): void {
    let markerValid = true;
    try {
      this.previewEl.textContent = buildTableRoleMarkerText(this.roles.buildParams());
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
    this.dispatcher.dispatch('template:tag-selection', this.roles.buildParams());
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

    // 섹션 모듈들 — 각자 자기 DOM을 만든다. RoleForm은 생성 마지막에
    // onRoleChanged → onParamsChanged → updatePreview를 호출하므로, 미리보기·
    // 버튼 DOM이 위에서 먼저 준비된 뒤에 생성해야 한다.
    this.roles = new RoleForm({
      wasm: this.wasm,
      getInputHandler: this.getInputHandler,
      onParamsChanged: () => this.updatePreview(),
      onApplyRequested: () => {
        if (!this.applyBtn.disabled) this.applyTag();
      },
    });
    this.fieldSuggest = new FieldSuggestSection({
      wasm: this.wasm,
      dispatcher: this.dispatcher,
      getInputHandler: this.getInputHandler,
      onApplied: () => this.refresh(),
    });
    this.entity = new EntitySection(this.wasm);

    this.contentEl.appendChild(outlineSection);
    this.contentEl.appendChild(this.hintEl);
    this.contentEl.appendChild(this.roles.roleFieldEl);
    this.contentEl.appendChild(this.roles.blockNameFieldEl);
    this.contentEl.appendChild(this.roles.nestedToggleFieldEl);
    this.contentEl.appendChild(this.roles.nestedParentFieldEl);
    this.contentEl.appendChild(this.previewEl);
    this.contentEl.appendChild(actions);
    this.contentEl.appendChild(this.fieldSuggest.fieldsetEl);
    this.contentEl.appendChild(this.entity.fieldsetEl);

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
    this.container.addEventListener('keydown', (e) => this.roles.handleShortcutKey(e));
  }
}
