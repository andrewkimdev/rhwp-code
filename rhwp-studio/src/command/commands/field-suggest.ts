/**
 * 누름틀 이름 자동 제안(`src/core/field-name-suggest.ts`, `src/core/selection-text.ts`)
 * review list의 "적용" — 표 인접 셀 기반 후보와 선택 텍스트 기반 후보를 한 번에
 * 삽입하는 단일 커맨드.
 *
 * `template.ts`의 `tagSelectionOperation`과 같은 이유로, N번의
 * `insertClickHereField` 호출을 하나의 `executeOperation({kind:'snapshot', ...})`
 * 안에 모아 undo 한 번으로 전체 배치가 되돌아가게 한다. 두 종류를 별도 커맨드로
 * 쪼개면 이 "적용 한 번 = undo 한 번" 보장이 깨지므로 확장하는 쪽을 택했다.
 */
import type { CommandDef } from '../types';
import type { DocumentPosition } from '@/core/types';
import type { WasmBridge } from '@/core/wasm-bridge';
import { CursorState } from '@/engine/cursor';

/** review list에서 체크된 한 행 — 최종(사용자가 손댔을 수 있는) 이름. */
export type FieldSuggestApplyItem =
  | { kind: 'cell'; cellIdx: number; name: string }
  | { kind: 'selection'; insertPos: DocumentPosition; name: string };

/**
 * `insertPos` 바로 앞 글자 1개를 읽는다(셀/본문 공통) — 없으면(문단 맨 앞이거나
 * 조회 실패) `null`. `selection-text.ts`의 `readSelectionText`와 같은 셀/본문
 * 분기를 쓴다.
 */
function readCharBefore(wasm: WasmBridge, pos: DocumentPosition): string | null {
  if (pos.charOffset <= 0) return null;
  try {
    if (pos.parentParaIndex !== undefined && pos.controlIndex !== undefined) {
      return wasm.getTextInCell(
        pos.sectionIndex,
        pos.parentParaIndex,
        pos.controlIndex,
        pos.cellIndex ?? 0,
        pos.cellParaIndex ?? 0,
        pos.charOffset - 1,
        1,
      );
    }
    return wasm.getTextRange(pos.sectionIndex, pos.paragraphIndex, pos.charOffset - 1, 1);
  } catch {
    return null;
  }
}

/** `pos` 위치에 스페이스 1글자를 삽입한다(셀/본문 공통, fire-and-forget). */
function insertSpaceAt(wasm: WasmBridge, pos: DocumentPosition): void {
  if (pos.parentParaIndex !== undefined && pos.controlIndex !== undefined) {
    wasm.insertTextInCell(
      pos.sectionIndex,
      pos.parentParaIndex,
      pos.controlIndex,
      pos.cellIndex ?? 0,
      pos.cellParaIndex ?? 0,
      pos.charOffset,
      ' ',
    );
  } else {
    wasm.insertText(pos.sectionIndex, pos.paragraphIndex, pos.charOffset, ' ');
  }
}

/**
 * 인라인 삽입(`kind:'selection'` — 규칙 4의 라벨 뒤 삽입, 텍스트 선택 삽입)은
 * 항상 이미 내용이 있는 위치에 필드를 붙인다. 그 직전 글자가 공백이 아니면
 * 스페이스 1글자를 구분자로 먼저 넣고, 필드 삽입 위치를 그만큼 밀어 돌려준다.
 * 이미 공백으로 끝나 있으면(트레일링 스페이스) 중복으로 넣지 않는다.
 * `kind:'cell'`(규칙 2/3/5)은 대상이 항상 빈 셀이라 호출하지 않는다.
 */
function withSpaceDelimiter(wasm: WasmBridge, pos: DocumentPosition): DocumentPosition {
  const before = readCharBefore(wasm, pos);
  if (before === null || /\s/.test(before)) return pos;
  insertSpaceAt(wasm, pos);
  return { ...pos, charOffset: pos.charOffset + 1 };
}

export const fieldSuggestCommands: CommandDef[] = [
  {
    id: 'field-suggest:apply',
    label: '누름틀 이름 제안 적용',
    // 선택 텍스트 기반 항목은 본문에서도 유효하므로 ctx.inTable을 요구하지 않는다.
    // 셀 기반 항목이 섞여 있는데 현재 커서가 표 밖이면 execute() 안에서 개별 처리한다.
    canExecute: (ctx) => ctx.hasDocument,
    execute(services, params) {
      const ih = services.getInputHandler();
      if (!ih) return;

      const items = params?.items as FieldSuggestApplyItem[] | undefined;
      if (!items || items.length === 0) return;

      const hasCellItems = items.some((i) => i.kind === 'cell');
      const pos = ih.getCursorPosition();
      let sec = pos.sectionIndex;
      let ppi: number | undefined;
      let ci: number | undefined;
      if (hasCellItems) {
        if (pos.parentParaIndex === undefined || pos.controlIndex === undefined) return;
        // 중첩 표는 template:tag-selection과 같은 이유로 아직 지원하지 않는다.
        if ((pos.cellPath?.length ?? 0) > 1) return;
        ppi = pos.parentParaIndex;
        ci = pos.controlIndex;
      }

      try {
        ih.executeOperation({
          kind: 'snapshot',
          operationType: 'fieldSuggestApply',
          operation: (wasm) => {
            // 각 항목의 최종 삽입 DocumentPosition을 먼저 전부 계산한 뒤, 문서 내
            // 위치가 뒤(늦은 offset)인 항목부터 삽입한다 — 같은 문단/셀에 선택 기반
            // 항목이 둘 이상 있을 때, 앞쪽 삽입이 뒤쪽 항목의 charOffset을 밀어버리는
            // 것을 막기 위함(셀 기반 항목끼리는 서로 다른 셀이라 이 문제가 없었다).
            const resolved = items.map((item) => ({
              item,
              insertPos:
                item.kind === 'cell'
                  ? ({
                      sectionIndex: sec,
                      paragraphIndex: 0,
                      charOffset: 0,
                      parentParaIndex: ppi,
                      controlIndex: ci,
                      cellIndex: item.cellIdx,
                      cellParaIndex: 0,
                    } satisfies DocumentPosition)
                  : item.insertPos,
            }));
            resolved.sort((a, b) => -CursorState.comparePositions(a.insertPos, b.insertPos));

            let lastPos: DocumentPosition = resolved[0]?.insertPos ?? pos;
            for (const { item, insertPos } of resolved) {
              // 'selection'(인라인 삽입/텍스트 선택)은 항상 이미 내용이 있는 위치에
              // 붙는다 — 직전 글자가 공백이 아니면 스페이스 구분자를 먼저 넣는다.
              // 'cell'(규칙 2/3/5)은 대상이 항상 빈 셀이라 그대로 둔다.
              const targetPos = item.kind === 'selection' ? withSpaceDelimiter(wasm, insertPos) : insertPos;
              // 안내문(guide)을 이름과 동기화한다 — review list에서 편집한 이름이
              // 그대로 셀/본문에 표시돼야, 채워 넣기 전에도 어떤 값이 들어갈 자리인지
              // 한눈에 보인다("입력하세요" 같은 범용 문구는 여러 필드가 전부 같은
              // 문구로 보여 구분이 안 된다).
              const result = wasm.insertClickHereField(targetPos, item.name, '', item.name, true);
              if (!result.ok) throw new Error(`[field-suggest:apply] 삽입 실패: ${item.name}`);
              lastPos = targetPos;
            }
            return lastPos;
          },
        });
        // 삽입 전 선택 anchor가 executeOperation의 cursor.moveTo(newPos) 이후에도
        // 살아남아(moveTo는 anchor를 지우지 않음) 방금 삽입한 필드까지 걸치는 유령
        // 선택을 만든다 — 명시적으로 지운다.
        ih.clearSelectionAnchor();
      } catch (err) {
        console.error('[field-suggest:apply] 실패:', err);
      }
    },
  },
];
