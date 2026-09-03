/**
 * 템플릿 패널의 "계산 필드 삽입" 섹션(panel.ts에서 분리) — `#seq:<라벨>`/`#sum:<필드명>`
 * 계산 필드(TEMPLATE_MARKER_SYNTAX.md §3b/§3d)를 커서 위치에 즉시 삽입한다.
 *
 * "누름틀 만들기"(fieldsuggest.ts)와 다른 authoring 표면이다 — 그쪽은 표 셀의 기존
 * 라벨 텍스트를 스캔해 이름을 자동 유추하지만, `#seq:`/`#sum:`은 라벨 텍스트가 아니라
 * 사용자가 고르는 계산 규칙(어떤 필드를 합산할지 등)이라 자동 유추 대상이 없다 —
 * 그래서 라벨/대상 필드명을 직접 입력받는다. 실제 뮤테이션은 이 파일이 아니라
 * `template:insert-computed-field` 커맨드(`command/commands/template.ts`, 연산부는
 * `template-ops.ts`)가 한다 — 이 섹션은 게이트 판정과 이름 조합, dedup만 한다
 * (`fieldsuggest.ts`의 `createFieldFromSelectionText`와 같은 역할 분담).
 *
 * 두 행(`#seq:`/`#sum:`) 모두 입력창+버튼이라 자리를 차지하는데, 동시에 둘 다 유효한
 * 위치란 애초에 없다(`#seq:`는 REPEAT-BODY 표에서만, `#sum:`은 REPEAT-FOOTER 표에서만) —
 * 그래서 커서 컨텍스트에 따라 관련 없는 행을 숨긴다(`refresh()`, `RoleForm`의 중첩/
 * 바닥고정 체크박스와 같은 패턴). fieldset 자체(legend)는 항상 남겨 패널 레이아웃이
 * 통째로 나타났다 사라지지 않게 한다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { CommandDispatcher } from '@/command/dispatcher';
import type { InputHandler } from '@/engine/input-handler';
import { readTableMarkerText } from '@/core/table-outline';
import { isRepeatBodyMarkerText, isRepeatFooterMarkerText } from '@/core/field-name-suggest';
import { resolveUniqueName } from '@/core/field-name-dedup';

export interface ComputedFieldSectionDeps {
  wasm: WasmBridge;
  dispatcher: CommandDispatcher;
  getInputHandler: () => InputHandler | null;
  /** 계산 필드를 만든 뒤 패널 전체를 다시 그린다(panel.refresh). */
  onApplied: () => void;
}

/** `#seq:` 라벨을 비워두면 쓰는 기본값 — 값 자체는 장식용이라(항목마다 1부터 자동
 * 채번) 사용자가 굳이 입력하지 않아도 되게 한다. */
const DEFAULT_SEQ_LABEL = '순번';

export class ComputedFieldSection {
  readonly fieldsetEl: HTMLFieldSetElement;
  private messageEl: HTMLElement;
  private placeholderEl: HTMLElement;
  private seqRowEl!: HTMLDivElement;
  private sumRowEl!: HTMLDivElement;
  private seqLabelInput!: HTMLInputElement;
  private sumTargetInput!: HTMLInputElement;

  constructor(private deps: ComputedFieldSectionDeps) {
    this.fieldsetEl = document.createElement('fieldset');
    this.fieldsetEl.className = 'tp-role-group';
    const legend = document.createElement('legend');
    legend.className = 'tp-role-group-legend';
    legend.textContent = '계산 필드 삽입';
    this.fieldsetEl.appendChild(legend);

    this.seqRowEl = this.buildSeqRow();
    this.fieldsetEl.appendChild(this.seqRowEl);
    this.sumRowEl = this.buildSumRow();
    this.fieldsetEl.appendChild(this.sumRowEl);

    this.placeholderEl = document.createElement('div');
    this.placeholderEl.className = 'tp-hint';
    this.placeholderEl.textContent =
      '#REPEAT-BODY: 표 안에서는 일련번호(#seq:) 삽입 버튼이, #REPEAT-FOOTER: 표 안에서는 합계(#sum:) 삽입 버튼이 여기 나타납니다.';
    this.fieldsetEl.appendChild(this.placeholderEl);

    this.messageEl = document.createElement('div');
    this.messageEl.className = 'tp-hint tp-computed-field-message';
    this.fieldsetEl.appendChild(this.messageEl);

    const refHint = document.createElement('div');
    refHint.className = 'tp-hint';
    refHint.textContent =
      '현재_페이지/전체_페이지는 #PAGENO 태깅 시 자동으로 채워집니다. "<필드명>통화"로 끝나는 ' +
      '필드 이름은 화폐 짝 필드로 자동 인식됩니다(직접 입력, 삽입 버튼 없음).';
    this.fieldsetEl.appendChild(refHint);

    this.refresh();
  }

  /**
   * 커서가 있는 표의 마커에 따라 `#seq:`/`#sum:` 행을 보이거나 숨긴다 — panel.ts의
   * `refresh()`(command-state-changed/table-object-selection-changed마다 호출)가 다른
   * 섹션들과 같은 주기로 이 메서드도 부른다.
   */
  refresh(): void {
    const marker = this.currentTableMarker();
    const showSeq = isRepeatBodyMarkerText(marker);
    const showSum = isRepeatFooterMarkerText(marker);
    this.seqRowEl.style.display = showSeq ? '' : 'none';
    this.sumRowEl.style.display = showSum ? '' : 'none';
    this.placeholderEl.style.display = (showSeq || showSum) ? 'none' : '';
  }

  /** 커서가 있는(중첩 아닌) 표의 마커 텍스트 — 표 밖이거나 중첩 표면 null.
   * `refresh()`(행 표시/숨김)와 `insertField()`(클릭 시점 최종 게이트)가 공유한다. */
  private currentTableMarker(): string | null {
    const ih = this.deps.getInputHandler();
    if (!ih) return null;
    const pos = ih.getCursorPosition();
    if (!pos || pos.parentParaIndex === undefined || pos.controlIndex === undefined) return null;
    if ((pos.cellPath?.length ?? 0) > 1) return null; // 중첩 표는 지원하지 않음
    return readTableMarkerText(this.deps.wasm, pos.sectionIndex, pos.parentParaIndex, pos.controlIndex);
  }

  private buildSeqRow(): HTMLDivElement {
    const row = document.createElement('div');
    row.className = 'tp-field';
    this.seqLabelInput = document.createElement('input');
    this.seqLabelInput.type = 'text';
    this.seqLabelInput.className = 'tp-input';
    this.seqLabelInput.placeholder = `예: ${DEFAULT_SEQ_LABEL}`;
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'tp-btn tp-computed-field-btn';
    btn.textContent = '일련번호 필드 삽입 (#seq:)';
    btn.title =
      '커서가 #REPEAT-BODY: 표 안에 있어야 합니다. 라벨은 장식용이며 값 자체는 항목마다 1부터 자동 채번됩니다.';
    btn.addEventListener('click', () => this.insertSeqField());
    row.appendChild(this.seqLabelInput);
    row.appendChild(btn);
    return row;
  }

  private buildSumRow(): HTMLDivElement {
    const row = document.createElement('div');
    row.className = 'tp-field';
    this.sumTargetInput = document.createElement('input');
    this.sumTargetInput.type = 'text';
    this.sumTargetInput.className = 'tp-input';
    this.sumTargetInput.placeholder = '합산할 필드 이름(예: 단가)';
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'tp-btn tp-computed-field-btn';
    btn.textContent = '합계 필드 삽입 (#sum:)';
    btn.title =
      '커서가 #REPEAT-FOOTER: 표 안에 있어야 합니다. 대상 필드 이름은 같은 블록의 #REPEAT-BODY: 표에 있는 필드여야 합니다.';
    btn.addEventListener('click', () => this.insertSumField());
    row.appendChild(this.sumTargetInput);
    row.appendChild(btn);
    return row;
  }

  private insertSeqField(): void {
    const label = this.seqLabelInput.value.trim() || DEFAULT_SEQ_LABEL;
    this.insertField(isRepeatBodyMarkerText, '#REPEAT-BODY:', `#seq:${label}`);
  }

  private insertSumField(): void {
    const target = this.sumTargetInput.value.trim();
    if (!target) {
      this.messageEl.textContent = '합산할 필드 이름을 입력하세요.';
      return;
    }
    this.insertField(isRepeatFooterMarkerText, '#REPEAT-FOOTER:', `#sum:${target}`);
  }

  /**
   * 공통 삽입 경로 — 게이트 판정(커서가 있는 표의 마커가 `gate`를 통과하는지) →
   * dedup된 이름 조합 → `template:insert-computed-field` 디스패치. 실제 wasm
   * 뮤테이션은 그 커맨드(연산부: `template-ops.ts`의 `insertComputedField`)가 한다 —
   * 여기서는 직접 wasm을 변형하지 않는다(뮤테이션 표면 원장을 이 파일 밖에 둔다).
   * 해당 행이 `refresh()`로 이미 게이트를 통과했을 때만 보이므로 실전에서는 이 게이트가
   * 실패할 일이 거의 없지만, 안전망으로 남겨둔다(예: 입력 중 커서가 옮겨간 극히 드문 경합).
   */
  private insertField(
    gate: (markerText: string | null) => boolean,
    gateLabel: string,
    baseName: string,
  ): void {
    const marker = this.currentTableMarker();
    if (!gate(marker)) {
      this.messageEl.textContent = `${gateLabel} 마커가 지정된 표 안에 커서를 두세요.`;
      return;
    }

    const existingNames = new Set(this.deps.wasm.getFieldList().map((f) => f.name));
    const name = resolveUniqueName(baseName, existingNames, new Set());

    this.deps.dispatcher.dispatch('template:insert-computed-field', { name });
    this.messageEl.textContent = `누름틀 "${name}"을(를) 만들었습니다.`;
    this.deps.onApplied();
  }
}
