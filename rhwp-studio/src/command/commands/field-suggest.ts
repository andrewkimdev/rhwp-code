/**
 * 누름틀 이름 자동 제안(`src/core/field-name-suggest.ts`) review list의
 * "적용" — 여러 빈 셀에 한 번에 누름틀을 삽입하는 단일 커맨드.
 *
 * `template.ts`의 `tagSelectionOperation`과 같은 이유로, N번의
 * `insertClickHereField` 호출을 하나의 `executeOperation({kind:'snapshot', ...})`
 * 안에 모아 undo 한 번으로 전체 배치가 되돌아가게 한다.
 */
import type { CommandDef } from '../types';

/** review list에서 체크된 한 행 — 최종(사용자가 손댔을 수 있는) 이름. */
export interface FieldSuggestApplyItem {
  cellIdx: number;
  name: string;
}

export const fieldSuggestCommands: CommandDef[] = [
  {
    id: 'field-suggest:apply',
    label: '누름틀 이름 제안 적용',
    canExecute: (ctx) => ctx.hasDocument && ctx.inTable,
    execute(services, params) {
      const ih = services.getInputHandler();
      if (!ih) return;
      const pos = ih.getCursorPosition();
      if (pos.parentParaIndex === undefined || pos.controlIndex === undefined) return;
      // 중첩 표는 template:tag-selection과 같은 이유로 아직 지원하지 않는다.
      if ((pos.cellPath?.length ?? 0) > 1) return;

      const items = params?.items as FieldSuggestApplyItem[] | undefined;
      if (!items || items.length === 0) return;

      const sec = pos.sectionIndex, ppi = pos.parentParaIndex, ci = pos.controlIndex;

      try {
        ih.executeOperation({
          kind: 'snapshot',
          operationType: 'fieldSuggestApply',
          operation: (wasm) => {
            for (const item of items) {
              // 안내문(guide)을 이름과 동기화한다 — review list에서 편집한 이름이
              // 그대로 셀에 표시돼야, 채워 넣기 전에도 어떤 값이 들어갈 칸인지
              // 한눈에 보인다("입력하세요" 같은 범용 문구는 8개 필드가 전부 같은
              // 문구로 보여 구분이 안 된다).
              const result = wasm.insertClickHereField(
                {
                  sectionIndex: sec,
                  paragraphIndex: 0,
                  charOffset: 0,
                  parentParaIndex: ppi,
                  controlIndex: ci,
                  cellIndex: item.cellIdx,
                  cellParaIndex: 0,
                },
                item.name,
                '',
                item.name,
                true,
              );
              if (!result.ok) throw new Error(`[field-suggest:apply] 삽입 실패: ${item.name}`);
            }
            return {
              sectionIndex: sec,
              paragraphIndex: 0,
              charOffset: 0,
              parentParaIndex: ppi,
              controlIndex: ci,
              cellIndex: 0,
              cellParaIndex: 0,
            };
          },
        });
      } catch (err) {
        console.error('[field-suggest:apply] 실패:', err);
      }
    },
  },
];
