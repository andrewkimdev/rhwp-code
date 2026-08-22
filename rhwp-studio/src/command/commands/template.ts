/**
 * hwpx-template-engine 마커 authoring 커맨드.
 *
 * `table:split`/`table:attach`(`table.ts`)와 같은 방식으로 구현한다 — 여러
 * wasm 호출을 하나의 `executeOperation({kind:'snapshot', ...})` 안에 모아 한
 * 번의 undo 단위로 만든다. 표 마커 텍스트는 hwpx-template-engine 이 이미 읽는
 * 그대로의 평문 텍스트(`#HEADER`/`#REPEAT-BODY:<name>` 등)이므로, 이 파일은
 * "무엇을 쓸지"만 결정하고 실제 authoring 규칙(TITLE→HEADER→BODY→FOOTER 순서,
 * `-NESTED:` 부모 존재 등)의 최종 권위는 여전히 엔진의
 * `TableRoleMarkerLintValidator`다 — 여기서는 명백히 잘못된 입력만 막는다.
 *
 * 이 파일이 호출하는 wasm 뮤테이터(`insertTableRow`/`mergeTableCells`/
 * `splitTable`/`insertTextInCell`/`deleteTextInCell`/`deleteTableRow`)는 모두
 * `mutation-method-registry.ts`의 MUTATING_METHODS에 이미 등재돼 있다 — 새
 * 브리지 메서드를 추가하는 게 아니라 기존 메서드를 새 위치에서 호출하는
 * 것이므로 그 레지스트리 자체는 바뀔 필요가 없다. 다만 `tests/
 * mutation-routing-guard.test.ts`의 뮤테이션 표면 원장(BASELINE)은 파일별
 * 호출 수를 동결하므로, 이 파일이 새로 생기면 그 표에 새 항목을 추가해야
 * 한다(§verification 참고). 이 로직을 의도적으로 `src/core/table-outline.ts`가
 * 아니라 여기(`src/command/`)에 둔 것도 그 원장이 스캔하는 표면 밖으로
 * 뮤테이션을 숨기지 않기 위해서다.
 */
import type { CommandDef } from '../types';
import type { WasmBridge } from '../../core/wasm-bridge';
import { findCellIndexForRowCol, readTableMarkerText } from '../../core/table-outline.ts';

export type TemplateTableRole =
  | 'HEADER' | 'FOOTER' | 'PAGENO'
  | 'REPEAT_TITLE' | 'REPEAT_HEADER' | 'REPEAT_BODY' | 'REPEAT_FOOTER'
  | 'REPEAT_TITLE_NESTED' | 'REPEAT_HEADER_NESTED' | 'REPEAT_BODY_NESTED' | 'REPEAT_FOOTER_NESTED';

export interface TagSelectionParams {
  role: TemplateTableRole;
  /** REPEAT_* / REPEAT_*_NESTED 에서 자식(비중첩이면 그 블록 자체) 블록명. */
  blockName?: string;
  /** REPEAT_*_NESTED 에서만 필요한 부모 블록명. */
  nestedParent?: string;
}

function requireBlockName(blockName: string | undefined, role: string): string {
  if (!blockName) throw new Error(`[template] ${role} 마커에는 블록명이 필요합니다`);
  return blockName;
}

function requireNestedPair(nestedParent: string | undefined, blockName: string | undefined, role: string): string {
  if (!nestedParent || !blockName) {
    throw new Error(`[template] ${role} 마커에는 부모 블록명과 자식 블록명이 모두 필요합니다`);
  }
  return `${nestedParent}/${blockName}`;
}

/** hwpx-template-engine `docs/TEMPLATE_MARKER_SYNTAX.md` §3/§3e 문법 그대로. */
export function buildTableRoleMarkerText(params: TagSelectionParams): string {
  const { role, blockName, nestedParent } = params;
  switch (role) {
    case 'HEADER': return '#HEADER';
    case 'FOOTER': return '#FOOTER';
    case 'PAGENO': return '#PAGENO';
    case 'REPEAT_TITLE': return `#REPEAT-TITLE:${requireBlockName(blockName, role)}`;
    case 'REPEAT_HEADER': return `#REPEAT-HEADER:${requireBlockName(blockName, role)}`;
    case 'REPEAT_BODY': return `#REPEAT-BODY:${requireBlockName(blockName, role)}`;
    case 'REPEAT_FOOTER': return `#REPEAT-FOOTER:${requireBlockName(blockName, role)}`;
    case 'REPEAT_TITLE_NESTED': return `#REPEAT-TITLE-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    case 'REPEAT_HEADER_NESTED': return `#REPEAT-HEADER-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    case 'REPEAT_BODY_NESTED': return `#REPEAT-BODY-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    case 'REPEAT_FOOTER_NESTED': return `#REPEAT-FOOTER-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    default: throw new Error(`[template] 알 수 없는 역할: ${role as string}`);
  }
}

/**
 * 표의 첫 행 첫 셀에 마커 텍스트를 쓴다. 이미 `#`로 시작하는 마커 행이 있으면
 * 그 셀 텍스트만 덮어쓰고, 없으면 Java `TableRoleMarkerInserter`가 하는 것과
 * 같은 모양(행 삽입 → 전체 열 병합 → 텍스트 입력)을 wasm 호출로 재현한다.
 */
/**
 * 마커 행이 원본 표에서 그대로 복제된 서식(원래 셀의 글자 크기·정렬·배경·행
 * 높이)을 물려받으면 눈에 잘 안 띈다 — 이 표가 authoring 마커라는 걸 한눈에
 * 알 수 있도록 고정된 시각 스타일을 강제한다: 18pt 굵게, 왼쪽 정렬, 검정
 * 글자, 옅은 회색 배경, 22px 행 높이, 세로 가운데 정렬.
 */
const MARKER_ROW_STYLE = {
  charFontSizeHwpUnit: 1800, // HWPUNIT: 1pt = 100 → 18pt
  charTextColor: '#000000',
  paraAlignment: 'left',
  cellFillColor: '#D9D9D9',
  cellHeightPx: 22,
  /** CellProperties.verticalAlign: 0=top, 1=center, 2=bottom */
  cellVerticalAlign: 1,
} as const;

/**
 * 96dpi 화면 px → HWPUNIT(1/7200인치) 환산 배율 — `table-resize-updates.ts`의
 * `RESIZE_HWPUNIT_PER_PX`(키보드 셀 크기 조절이 쓰는 것)와 동일 계약.
 */
const HWPUNIT_PER_PX = 75;

function applyMarkerRowStyle(
  wasm: WasmBridge,
  sec: number,
  ppi: number,
  ci: number,
  cellIdx: number,
  markerText: string,
): void {
  wasm.applyCharFormatInCell(sec, ppi, ci, cellIdx, 0, 0, markerText.length, JSON.stringify({
    fontSize: MARKER_ROW_STYLE.charFontSizeHwpUnit,
    bold: true,
    textColor: MARKER_ROW_STYLE.charTextColor,
  }));
  wasm.applyParaFormatInCell(sec, ppi, ci, cellIdx, 0, JSON.stringify({
    alignment: MARKER_ROW_STYLE.paraAlignment,
  }));
  wasm.setCellProperties(sec, ppi, ci, cellIdx, {
    fillType: 'solid',
    fillColor: MARKER_ROW_STYLE.cellFillColor,
    verticalAlign: MARKER_ROW_STYLE.cellVerticalAlign,
  });

  // `resizeTableCells`는 delta 기반이라(TableCellResizeUpdate) 목표 높이가
  // 아니라 "현재 대비 변화량"을 넘긴다 — getCellProperties()가 이미 HWPUNIT
  // 원본값을 주므로 px 환산은 목표값 쪽에만 필요하다.
  const targetHeightHwp = Math.round(MARKER_ROW_STYLE.cellHeightPx * HWPUNIT_PER_PX);
  const currentHeightHwp = wasm.getCellProperties(sec, ppi, ci, cellIdx).height;
  const heightDelta = targetHeightHwp - currentHeightHwp;
  if (heightDelta !== 0) {
    wasm.resizeTableCells(sec, ppi, ci, [{ cellIdx, heightDelta }]);
  }
}

function setTableRoleMarker(wasm: WasmBridge, sec: number, ppi: number, ci: number, markerText: string): void {
  const existing = readTableMarkerText(wasm, sec, ppi, ci);
  if (existing === null) {
    wasm.insertTableRow(sec, ppi, ci, 0, false);
    const dims = wasm.getTableDimensions(sec, ppi, ci);
    if (dims.colCount > 1) {
      wasm.mergeTableCells(sec, ppi, ci, 0, 0, 0, dims.colCount - 1);
    }
  }
  const cellIdx = findCellIndexForRowCol(wasm, sec, ppi, ci, 0, 0) ?? 0;
  const len = wasm.getCellParagraphLength(sec, ppi, ci, cellIdx, 0);
  if (len > 0) {
    wasm.deleteTextInCell(sec, ppi, ci, cellIdx, 0, 0, len);
  }
  wasm.insertTextInCell(sec, ppi, ci, cellIdx, 0, 0, markerText);
  applyMarkerRowStyle(wasm, sec, ppi, ci, cellIdx, markerText);
}

function clearTableRoleMarker(wasm: WasmBridge, sec: number, ppi: number, ci: number): void {
  if (readTableMarkerText(wasm, sec, ppi, ci) === null) return;
  wasm.deleteTableRow(sec, ppi, ci, 0);
}

/**
 * 선택된 행 구간만 담은 독립 표를 만들고(필요한 만큼만 `splitTable`) 그 표에
 * 마커를 쓴다 — "행 선택 + 역할 선택 = 한 번의 클릭"의 실제 구현.
 *
 * `splitTable(sec, ppi, ci, atRow)`는 `atRow`부터의 뒤쪽 행을 새 최상위 표로
 * 떼어내고(`rhwp-code/src/document_core/commands/table_ops.rs`의
 * `split_table_native`), 그 새 표는 `res.backParaIdx`가 가리키는 새 문단의
 * controlIndex 0 이 된다(그 문단의 유일한 컨트롤이라서) — `table:split` 커맨드
 * 가 이미 이렇게 가정한다.
 */
function tagSelectionOperation(
  wasm: WasmBridge,
  sec: number,
  ppi: number,
  ci: number,
  startRow: number,
  endRow: number,
  markerText: string,
): { parentPara: number; controlIndex: number } {
  let targetPpi = ppi;
  let targetCi = ci;
  let localEnd = endRow;

  if (startRow > 0) {
    const split = wasm.splitTable(sec, targetPpi, targetCi, startRow);
    targetPpi = split.backParaIdx;
    targetCi = 0;
    localEnd = endRow - startRow;
  }

  const dims = wasm.getTableDimensions(sec, targetPpi, targetCi);
  if (localEnd < dims.rowCount - 1) {
    wasm.splitTable(sec, targetPpi, targetCi, localEnd + 1);
    // 뒤로 떨어져 나간 표(선택 이후의 나머지 행)는 여기서 더 다루지 않는다 —
    // 필요하면 사용자가 그 표를 다시 선택해 별도로 태깅한다.
  }

  setTableRoleMarker(wasm, sec, targetPpi, targetCi, markerText);
  return { parentPara: targetPpi, controlIndex: targetCi };
}

export const templateCommands: CommandDef[] = [
  {
    id: 'template:toggle-panel',
    label: '템플릿 패널',
    canExecute: (ctx) => ctx.hasDocument,
    // 표를 뮤테이션하지 않는 순수 UI 토글이라 executeOperation 이 필요 없다 —
    // view:toggle-grid(view.ts) 와 같은 패턴으로 DOM을 직접 다룬다. 패널
    // 내용 자체는 `ui/template-panel.ts`의 TemplatePanel 이
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
      const startRow = range?.startRow ?? cellInfo?.row ?? 0;
      const endRow = range?.endRow ?? cellInfo?.row ?? 0;

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
