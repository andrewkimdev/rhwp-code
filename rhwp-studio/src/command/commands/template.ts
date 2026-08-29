/**
 * hwpx-template-engine 마커 authoring 커맨드(정의부).
 *
 * 여러 wasm 호출을 하나의 `executeOperation({kind:'snapshot', ...})` 안에 모아 한
 * 번의 undo 단위로 만든다 — 실제 뮤테이션 연산부는 `template-ops.ts`에 있다
 * (뮤테이션 표면 원장이 그 파일의 호출 수를 동결한다). 마커 문법 문자열 빌더는
 * `core/template-marker.ts`, 패널 UI는 `ui/template/panel.ts`다.
 */
import type { CommandDef } from '../types';
import { expandRowRangeForMerges, readTableMarkerText } from '../../core/table-outline.ts';
import { showToast } from '../../ui/toast.ts';
import { buildTableRoleMarkerText, type TemplateTableRole } from '../../core/template-marker.ts';
import { tagSelectionOperation, clearTableRoleMarker } from './template-ops.ts';

export type { TemplateTableRole, TagSelectionParams } from '../../core/template-marker.ts';

export const templateCommands: CommandDef[] = [
  {
    id: 'template:toggle-panel',
    label: '템플릿 패널',
    // 문서가 없어도 열고 닫을 수 있어야 한다 — 패널이 기본으로 열려 있으므로
    // hasDocument 를 요구하면 문서를 열기 전에는 닫을 방법이 없어진다.
    // 문서가 없을 때의 내용은 TemplatePanel.refresh() 의 빈 상태 안내로 처리한다.
    canExecute: () => true,
    // 표를 뮤테이션하지 않는 순수 UI 토글이라 executeOperation 이 필요 없다 —
    // view:toggle-grid(view.ts) 와 같은 패턴으로 DOM을 직접 다룬다. 패널
    // 내용 자체는 `ui/template/panel.ts`의 TemplatePanel 이
    // `template-panel-visibility-changed` 를 구독해 채운다(지연 렌더링 —
    // 숨겨진 동안은 갱신하지 않는다).
    execute(services) {
      const panel = document.getElementById('template-panel');
      if (!panel) return;
      // HTMLElement.hidden 타입에 "until-found"도 있어(lib.dom) boolean으로 좁힌다 —
      // 이 패널은 그 값을 절대 쓰지 않으므로 참이면 곧 "숨김"으로 취급해도 안전하다.
      const willShow = panel.hidden === true;
      panel.hidden = !willShow;
      document.querySelectorAll('[data-cmd="template:toggle-panel"]').forEach(el => {
        el.classList.toggle('active', willShow);
      });
      services.eventBus.emit('template-panel-visibility-changed', willShow);
    },
  },
  {
    id: 'template:tag-selection',
    label: '템플릿 마커 지정',
    canExecute: (ctx) => ctx.inTable,
    execute(services, params) {
      const ih = services.getInputHandler();
      if (!ih) return;
      const pos = ih.getCursorPosition();
      if (pos.parentParaIndex === undefined || pos.controlIndex === undefined) return;
      // 중첩 표는 table:split 과 같은 이유로 아직 지원하지 않는다.
      if ((pos.cellPath?.length ?? 0) > 1) return;

      const role = params?.role as TemplateTableRole | undefined;
      const blockName = params?.blockName as string | undefined;
      const nestedParent = params?.nestedParent as string | undefined;
      if (!role) return;

      let markerText: string;
      try {
        markerText = buildTableRoleMarkerText({ role, blockName, nestedParent });
      } catch (err) {
        console.warn('[template:tag-selection]', err);
        return;
      }

      const sec = pos.sectionIndex, ppi = pos.parentParaIndex, ci = pos.controlIndex;
      const range = ih.isInCellSelectionMode?.() ? ih.getSelectedCellRange?.() : null;
      const cellInfo = pos.cellIndex !== undefined
        ? services.wasm.getCellInfo(sec, ppi, ci, pos.cellIndex)
        : null;
      const rawStartRow = range?.startRow ?? cellInfo?.row ?? 0;
      const rawEndRow = range?.endRow ?? cellInfo?.row ?? 0;
      // 드래그 선택은 anchor/focus 셀의 min/max일 뿐 rowSpan을 모른다 — 헤더
      // 블록의 일부 컬럼만 두 행에 걸쳐 병합돼 있으면(예: 5446216.hwpx의
      // "번호"~"수령인") 병합 셀이 실제로 덮는 행 전체로 넓혀야 splitTable이
      // 그 병합 셀을 가로지르며 거부되는 일이 없다.
      const bboxes = services.wasm.getTableCellBboxes(sec, ppi, ci);
      const { startRow, endRow } = expandRowRangeForMerges(bboxes, { startRow: rawStartRow, endRow: rawEndRow });

      try {
        ih.executeOperation({
          kind: 'snapshot',
          operationType: 'templateTagSelection',
          operation: (wasm) => {
            const target = tagSelectionOperation(wasm, sec, ppi, ci, startRow, endRow, markerText);
            return {
              sectionIndex: sec,
              paragraphIndex: 0,
              charOffset: 0,
              parentParaIndex: target.parentPara,
              controlIndex: target.controlIndex,
              cellIndex: 0,
              cellParaIndex: 0,
            };
          },
        });
      } catch (err) {
        console.error('[template:tag-selection] 실패:', err);
        const msg = err instanceof Error ? err.message : String(err);
        showToast({ message: `태그를 지정하지 못했습니다: ${msg}`, durationMs: 4000 });
      }
    },
  },
  {
    id: 'template:clear-marker',
    label: '템플릿 마커 지우기',
    canExecute: (ctx) => ctx.inTable,
    execute(services) {
      const ih = services.getInputHandler();
      if (!ih) return;
      const pos = ih.getCursorPosition();
      if (pos.parentParaIndex === undefined || pos.controlIndex === undefined) return;
      if ((pos.cellPath?.length ?? 0) > 1) return;
      const sec = pos.sectionIndex, ppi = pos.parentParaIndex, ci = pos.controlIndex;

      try {
        ih.executeOperation({
          kind: 'snapshot',
          operationType: 'templateClearMarker',
          operation: (wasm) => {
            if (readTableMarkerText(wasm, sec, ppi, ci) === null) return null; // no-op
            clearTableRoleMarker(wasm, sec, ppi, ci);
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
        console.error('[template:clear-marker] 실패:', err);
      }
    },
  },
];
