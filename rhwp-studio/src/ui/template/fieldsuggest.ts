/**
 * 템플릿 패널의 "누름틀 만들기" 섹션(panel.ts에서 분리) — "템플릿" 마커
 * authoring과는 다른 개념(누름틀 필드)이지만 같은 "현재 표"를 다루므로 같은
 * 패널에 별도 fieldset으로 둔다. 행 기반 스캔과 텍스트 선택 기반 삽입 모두
 * review list 없이 항상 즉시 생성한다(createFields). mouseup(드래그-선택 종료)이
 * command-state-changed를 발생시키지 않으므로 활성 상태를 캐시하지 않고,
 * 문서가 열려 있으면 항상 활성으로 두고 실제 선택 유효성은 클릭 시점에 검증한다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { CommandDispatcher } from '@/command/dispatcher';
import type { InputHandler } from '@/engine/input-handler';
import { readTableMarkerText } from '@/core/table-outline';
import { suggestFieldNames, isTemplateTableMarkerText, type FieldNameSuggestion } from '@/core/field-name-suggest';
import { extractSelectedLabel } from '@/core/selection-text';
import { resolveUniqueName } from '@/core/field-name-dedup';
import { MAX_FIELD_NAME_LEN } from '@/ui/field-edit-dialog';
import type { FieldSuggestApplyItem } from '@/command/commands/field-suggest';
import { getSelectedRowRange, formatRowRange } from './row-range';

export interface FieldSuggestSectionDeps {
  wasm: WasmBridge;
  dispatcher: CommandDispatcher;
  getInputHandler: () => InputHandler | null;
  /** 누름틀을 만든 뒤 패널 전체를 다시 그린다(panel.refresh). */
  onApplied: () => void;
}

export class FieldSuggestSection {
  readonly fieldsetEl: HTMLFieldSetElement;
  private messageEl: HTMLElement;

  constructor(private deps: FieldSuggestSectionDeps) {
    this.fieldsetEl = document.createElement('fieldset');
    this.fieldsetEl.className = 'tp-role-group';
    const legend = document.createElement('legend');
    legend.className = 'tp-role-group-legend';
    legend.textContent = '누름틀 만들기';
    this.fieldsetEl.appendChild(legend);

    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'tp-btn tp-fieldsuggest-btn';
    btn.textContent = '누름틀 만들기';
    btn.title =
      '행을 선택했거나 커서가 역할 마커가 지정된 표 안에 있으면 그 행(들)에서, 텍스트를 선택했으면 그 텍스트로 누름틀을 즉시 만듭니다.';
    btn.addEventListener('click', () => this.createFields());
    this.fieldsetEl.appendChild(btn);

    this.messageEl = document.createElement('div');
    this.messageEl.className = 'tp-hint tp-fieldsuggest-message';
    this.fieldsetEl.appendChild(this.messageEl);
  }

  /**
   * "누름틀 만들기" 버튼의 단일 진입점. 셀 선택 모드(명시적 행/범위 선택)와
   * 텍스트 선택은 cursor.ts에서 서로 다른 상태(cellAnchor/cellFocus vs
   * anchor/position)로 관리되어 겹치지 않는다 — 셀 선택 모드가 있으면 그
   * 범위, 없으면 텍스트 선택, 그것도 없으면 커서가 있는 행 순으로 처리한다.
   * 텍스트 선택이 있는데 유효성 검사에 실패하면(셀/문단 경계를 넘는 등) 그
   * 오류를 그대로 보여주고 행 기반으로 조용히 대체하지 않는다 — 명시적으로
   * 선택한 것을 다른 것으로 재해석하면 사용자가 의도하지 않은 필드가 생길 수
   * 있다.
   */
  private createFields(): void {
    const ih = this.deps.getInputHandler();
    if (!ih) return;

    if (ih.isInCellSelectionMode()) {
      this.createFieldsFromRows(ih);
      return;
    }
    const sel = ih.getSelection();
    if (sel) {
      this.createFieldFromSelectionText(ih, sel);
      return;
    }
    this.createFieldsFromRows(ih);
  }

  /**
   * 선택된 행(셀 선택 모드) 또는 커서가 있는 행에서 누름틀 후보를 계산해
   * review list 없이 즉시 전부 생성한다(이름은 `suggestFieldNames`가 이미
   * dedup해서 돌려준다). 순수 조회로 후보를 계산하므로
   * `template:tag-selection`과 달리 dispatcher를 거치지 않고 wasm을 직접
   * 읽는다(`suggestBlockNameFromCurrentCell`과 같은 전례). 두 조건이 함께
   * 게이트가 된다:
   *
   * 1. **마커 게이트** — 역할 마커(#HEADER/#FOOTER/#PAGENO/#REPEAT-*:)가 지정된
   *    표에서만 만든다. 마커 authoring("태그 지정")의 다음 단계다 — 이
   *    패널의 워크플로(표에 역할을 선언 → 그 표의 누름틀을 채운다)와 같은
   *    순서. 반복 블록(#REPEAT-*)도 예외가 아니다: 검색 범위가 "선택된 행"이므로
   *    과거 반복 표를 막았던 "행마다 같은 라벨이 반복돼 충돌로 오인" 모호성이
   *    발생하지 않는다.
   * 2. **행 범위** — 표 전체가 아니라 선택된 행(셀 선택 모드) 또는 커서가 있는
   *    행만 검색한다. hint(`describeSelectedRows`)에 보이는 행과 같은 정의
   *    (`getSelectedRowRange`, row-range.ts)를 쓴다.
   */
  private createFieldsFromRows(ih: InputHandler): void {
    const pos = ih.getCursorPosition();
    if (!pos || pos.parentParaIndex === undefined || pos.controlIndex === undefined) {
      this.messageEl.textContent = '표 안에 커서를 두거나 누름틀로 만들 텍스트를 선택하세요.';
      return;
    }
    if ((pos.cellPath?.length ?? 0) > 1) return; // 중첩 표는 지원하지 않음

    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex;
    const ci = pos.controlIndex;

    const marker = readTableMarkerText(this.deps.wasm, sec, ppi, ci);
    if (!isTemplateTableMarkerText(marker)) {
      this.messageEl.textContent =
        '역할 마커(#HEADER/#FOOTER/#PAGENO/#REPEAT-*)가 지정된 표에서만 누름틀을 만듭니다. 위 "태그 지정"으로 먼저 역할을 지정하세요.';
      return;
    }

    const rowRange = getSelectedRowRange(this.deps.wasm, ih, pos);
    if (!rowRange) {
      this.messageEl.textContent = '누름틀을 만들 행에 커서를 두거나 행을 선택하세요.';
      return;
    }

    let suggestions: FieldNameSuggestion[] = [];
    try {
      suggestions = suggestFieldNames(this.deps.wasm, sec, ppi, ci, { rowRange });
    } catch (err) {
      console.warn('[template-panel] 누름틀 생성 실패:', err);
    }

    if (suggestions.length === 0) {
      this.messageEl.textContent = `선택된 행(${formatRowRange(rowRange)})에서 누름틀을 만들 빈 칸을 찾지 못했습니다.`;
      return;
    }

    const skipped = suggestions.filter((s) => s.alreadyHasField).length;
    const items: FieldSuggestApplyItem[] = suggestions
      .filter((s) => !s.alreadyHasField)
      .map((s) => {
        const insertAt = s.insertAt;
        if (insertAt) {
          return {
            kind: 'selection' as const,
            insertPos: {
              sectionIndex: sec,
              paragraphIndex: 0,
              charOffset: insertAt.charOffset,
              parentParaIndex: ppi,
              controlIndex: ci,
              cellIndex: s.cellIdx,
              cellParaIndex: insertAt.cellParaIndex,
            },
            name: s.suggestedName,
          };
        }
        return { kind: 'cell' as const, cellIdx: s.cellIdx, name: s.suggestedName };
      });

    if (items.length === 0) {
      this.messageEl.textContent =
        `선택된 행(${formatRowRange(rowRange)})의 후보 ${skipped}개가 모두 이미 필드가 있어 건너뛰었습니다.`;
      return;
    }

    this.deps.dispatcher.dispatch('field-suggest:apply', { items });
    this.messageEl.textContent =
      skipped === 0
        ? `누름틀 ${items.length}개를 만들었습니다.`
        : `누름틀 ${items.length}개를 만들었습니다(이미 필드가 있는 ${skipped}개는 건너뛰었습니다).`;
    this.deps.onApplied();
  }

  /**
   * 현재 텍스트 선택 영역을 그 자리에서 바로 누름틀로 삽입한다. 표 인접 셀
   * 스캔(`createFieldsFromRows`)과 달리 후보가 항상 하나뿐이라 즉시 삽입 외에
   * 다른 단계가 없다 — 검증을 통과하면 곧바로 `field-suggest:apply`를 단일
   * 아이템으로 디스패치한다(undo 한 번, `field-suggest.ts` 참고).
   */
  private createFieldFromSelectionText(
    ih: InputHandler,
    sel: NonNullable<ReturnType<InputHandler['getSelection']>>,
  ): void {
    const extracted = extractSelectedLabel(this.deps.wasm, sel.start, sel.end);
    if (!extracted) {
      this.messageEl.textContent =
        '선택 영역이 비어있거나(공백만) 문단/셀 경계를 벗어났습니다(표 셀 경계나 문단 경계를 넘는 선택, 중첩 표, 글상자는 지원하지 않습니다).';
      return;
    }
    if (extracted.text.length > MAX_FIELD_NAME_LEN) {
      this.messageEl.textContent = `선택한 텍스트가 너무 깁니다(최대 ${MAX_FIELD_NAME_LEN}자).`;
      return;
    }
    // 이미 필드(안내문 등) 안의 선택은 막는다.
    try {
      if (this.deps.wasm.getFieldInfoAt(sel.start).inField || this.deps.wasm.getFieldInfoAt(sel.end).inField) {
        this.messageEl.textContent = '이미 필드 안의 텍스트는 선택할 수 없습니다.';
        return;
      }
    } catch {
      // 조회 실패는 무시하고 진행 — 최종 방어선은 wasm insert 실패 처리
    }

    // 배치 개념이 없으므로(항상 한 건씩 즉시 삽입) usedInBatch는 항상 빈 Set —
    // 문서에 이미 있는 이름과만 충돌 검사한다. 조정된 이름은 메시지로 보여준다.
    const existingNames = new Set(this.deps.wasm.getFieldList().map((f) => f.name));
    const name = resolveUniqueName(extracted.text, existingNames, new Set());

    this.deps.dispatcher.dispatch('field-suggest:apply', {
      items: [{ kind: 'selection', insertPos: extracted.insertPos, name }],
    });
    this.messageEl.textContent = `누름틀 "${name}"을(를) 만들었습니다.`;
    this.deps.onApplied();
  }
}
