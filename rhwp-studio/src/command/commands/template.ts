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
import {
  createSavePayload,
  downloadBlob,
  flushDeferredPaginationBeforeExplicitOutput,
  showExportContentLoss,
  tryFileSystemSave,
} from './file.ts';
import { persistDownloadWithContentLoss, persistWithContentLoss } from '../../core/export-content-loss.ts';
import type { FileSystemFileHandleLike } from '../file-system-access.ts';
import { templateFileName } from '../template-save-name.ts';

export type { TemplateTableRole, TagSelectionParams } from '../../core/template-marker.ts';
export { templateFileName } from '../template-save-name.ts';

/**
 * "템플릿으로 저장" 클릭 사이에 저장 위치를 기억해 두 번째 클릭부터는 대화상자
 * 없이 그 자리에 덮어쓴다(파일:저장의 currentFileHandle과 같은 아이디어). 문서가
 * 바뀌면(`documentGeneration` 증가 — 새 문서 열기/새로 만들기) 무효화된다 — 다른
 * 문서의 이전 저장 위치에 실수로 덮어쓰지 않기 위해서다.
 */
let templateSaveHandleCache: { generation: number; handle: FileSystemFileHandleLike } | null = null;

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
  {
    // 템플릿 패널 상단 "템플릿으로 저장" 버튼 — 원본 형식과 무관하게 항상 HWPX로,
    // 원본 문서와 같은 폴더에 저장한다. 첫 클릭만 저장 위치를 고르는 대화상자를
    // 띄우고(원본 파일과 같은 폴더에서 시작 — startIn), 그 뒤로는 같은 문서
    // 세션 안에서 대화상자 없이 그 자리에 덮어쓴다(templateSaveHandleCache).
    // File System Access API 미지원 브라우저에서는 대화상자/폴더 지정 없이
    // 바로 다운로드하는 이전 동작으로 폴백한다.
    id: 'template:save-as-template',
    label: '템플릿으로 저장',
    canExecute: (ctx) => ctx.hasDocument,
    async execute(services) {
      try {
        flushDeferredPaginationBeforeExplicitOutput(services, 'save-as-template');
        const payload = createSavePayload(services, 'hwpx');
        const suggestedName = templateFileName(services.wasm.fileName);
        const generation = services.wasm.documentGeneration;
        const cachedHandle = templateSaveHandleCache?.generation === generation
          ? templateSaveHandleCache.handle
          : null;

        const result = await persistWithContentLoss(
          payload.contentLoss,
          () => tryFileSystemSave(
            services,
            'hwpx',
            payload.blob,
            suggestedName,
            !cachedHandle,
            cachedHandle,
            cachedHandle ? undefined : services.wasm.currentFileHandle,
          ),
          (saveResult) => saveResult !== 'cancelled' && saveResult.method !== 'fallback',
          showExportContentLoss,
        );
        if (result === 'cancelled') return;

        if (result.method === 'fallback') {
          // 캐시된 위치에 더 이상 쓸 수 없었거나(권한 취소 등) API 미지원 —
          // 다음 클릭은 새로 위치를 고르도록 캐시를 비운다.
          if (cachedHandle) templateSaveHandleCache = null;
          persistDownloadWithContentLoss(
            payload.contentLoss,
            () => downloadBlob(payload.blob, suggestedName),
            showExportContentLoss,
          );
          showToast({ message: `${suggestedName} 로 저장했습니다.`, durationMs: 2200 });
          return;
        }

        templateSaveHandleCache = { generation, handle: result.handle! };
        showToast({ message: `${result.fileName} 로 저장했습니다.`, durationMs: 2200 });
      } catch (err) {
        console.error('[template:save-as-template] 저장 실패:', err);
        const msg = err instanceof Error ? err.message : String(err);
        showToast({ message: `템플릿 저장에 실패했습니다: ${msg}`, durationMs: 4000 });
      }
    },
  },
];
