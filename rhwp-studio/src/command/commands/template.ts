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
 * 마커 행이 원본 표에서 그대로 복제된 서식(원래 셀의 글꼴·자간·장평·안 여백·
 * 정렬·배경·행 높이)을 물려받으면 눈에 잘 안 띄고, 표마다 제각각으로 보인다
 * — 이 표가 authoring 마커라는 걸 한눈에 알 수 있도록, 그리고 어떤 원본 행에
 * 붙였든 모든 마커 행이 서로 동일하게 보이도록 고정된 시각 스타일을 강제한다:
 * 돋움체 18pt 굵게(자간·장평 100% 고정), 왼쪽 정렬, 검정 글자, 옅은 파란
 * 배경 + 왼쪽 강조선, 36px 행 높이, 세로 가운데 정렬, 4~6px 안 여백.
 *
 * [자체 발견] 글꼴·자간·장평·안 여백을 강제하지 않았을 때는 원본 셀에 이미
 * 있던 값을 그대로 물려받았다 — 이 문서 안에서는 우연히 대부분 일치했지만,
 * 새로 삽입된 마커 행(`insertTableRow`)의 문단이 원본 문서의 다른 행과 다른
 * 기본 서식(예: 자간이 들어간 글자 모양)을 물려받으면 같은 문서 안에서도
 * 마커 행끼리 눈에 띄게 다르게 렌더링됐다.
 *
 * [자체 발견 #2] `cellHeightPx: 22`는 18pt 텍스트 한 줄 자체의 콘텐츠 높이
 * (`table_layout.rs`가 셀의 마지막 줄은 line_spacing을 제외하므로, 필요한
 * 높이는 사실상 글자 크기 그대로 — 18pt = 1800 HWPUNIT ÷ `HWPUNIT_PER_PX`
 * = 24px)보다도 작았다 — 안 여백을 얼마로 주든 무조건 아래가 잘렸다. 22px는
 * "회색 배경 22px" 관행값을 그대로 물려받은 것일 뿐, 실측 없이 정해진 값이었다.
 */
const MARKER_ROW_STYLE = {
  charFontSizeHwpUnit: 1800, // HWPUNIT: 1pt = 100 → 18pt
  charTextColor: '#000000',
  /** 한컴 표준 산세리프 — 기존에 이미 쓰이던 서체를 명시적으로 고정한다. */
  charFontFace: '돋움체',
  /** CharShape.ratios/spacings 7언어 슬롯 전부에 강제할 장평/자간(기본값). */
  charWidthRatioPercent: 100,
  charLetterSpacingPercent: 0,
  paraAlignment: 'left',
  /** Tailwind blue-100 — 문서 본문과 뚜렷이 구분되는 "authoring 주석" 톤. */
  cellFillColor: '#DBEAFE',
  /** Tailwind blue-600 — 왼쪽 강조선 색. */
  accentBorderColor: '#2563EB',
  /** BorderLine.width 는 0~15 인덱스(`BORDER_WIDTHS` mm 표, style.rs:12) —
   * 10 = 1.0mm(~3.8px), 뚜렷하지만 과하지 않은 두께. */
  accentBorderWidthIndex: 10,
  /** 18pt 한 줄 콘텐츠 높이(24px, 아래 자체 발견 #2) + 안 여백 + 여유. */
  cellHeightPx: 36,
  /** CellProperties.verticalAlign: 0=top, 1=center, 2=bottom */
  cellVerticalAlign: 1,
  /** 안 여백(px) — 왼쪽은 강조선과 텍스트가 붙어 보이지 않도록 더 준다. */
  cellPaddingPx: { left: 6, right: 4, top: 4, bottom: 4 },
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
  // 글꼴은 이름이 아니라 fontId(문서 글꼴 표 인덱스)로만 적용된다 — 없으면
  // 새로 등록한다. char-shape-dialog.ts의 fontName→fontId 변환과 동일 계약.
  const fontId = wasm.findOrCreateFontId(MARKER_ROW_STYLE.charFontFace);
  const ratios = new Array(7).fill(MARKER_ROW_STYLE.charWidthRatioPercent);
  const spacings = new Array(7).fill(MARKER_ROW_STYLE.charLetterSpacingPercent);
  wasm.applyCharFormatInCell(sec, ppi, ci, cellIdx, 0, 0, markerText.length, JSON.stringify({
    fontSize: MARKER_ROW_STYLE.charFontSizeHwpUnit,
    bold: true,
    textColor: MARKER_ROW_STYLE.charTextColor,
    ...(fontId >= 0 ? { fontId } : {}),
    ratios,
    spacings,
  }));
  wasm.applyParaFormatInCell(sec, ppi, ci, cellIdx, 0, JSON.stringify({
    alignment: MARKER_ROW_STYLE.paraAlignment,
  }));
  // `setCellProperties`에 테두리 키를 하나라도 넣으면 네 변 전체가 다시
  // 만들어진다 — 언급 안 한 변은 "그대로 유지"가 아니라 기본값(사실상 테두리
  // 없음)으로 리셋된다(html_table_import.rs의 create_border_fill_from_json).
  // 왼쪽에만 강조선을 추가하려면 나머지 세 변을 현재 값 그대로 같이 보내야
  // 오른쪽/위/아래 테두리가 사라지지 않는다.
  const current = wasm.getCellProperties(sec, ppi, ci, cellIdx);
  wasm.setCellProperties(sec, ppi, ci, cellIdx, {
    fillType: 'solid',
    fillColor: MARKER_ROW_STYLE.cellFillColor,
    verticalAlign: MARKER_ROW_STYLE.cellVerticalAlign,
    // 표 기본 안 여백에 얹혀가지 않도록 셀 자체에 명시적으로 고정한다 —
    // 그래야 원본 표의 기본 안 여백이 뭐였든 마커 행은 항상 동일하다.
    applyInnerMargin: true,
    paddingLeft: MARKER_ROW_STYLE.cellPaddingPx.left * HWPUNIT_PER_PX,
    paddingRight: MARKER_ROW_STYLE.cellPaddingPx.right * HWPUNIT_PER_PX,
    paddingTop: MARKER_ROW_STYLE.cellPaddingPx.top * HWPUNIT_PER_PX,
    paddingBottom: MARKER_ROW_STYLE.cellPaddingPx.bottom * HWPUNIT_PER_PX,
    borderLeft: {
      type: 1, // SOLID
      width: MARKER_ROW_STYLE.accentBorderWidthIndex,
      color: MARKER_ROW_STYLE.accentBorderColor,
    },
    borderRight: current.borderRight,
    borderTop: current.borderTop,
    borderBottom: current.borderBottom,
  });

  // `resizeTableCells`는 delta 기반이라(TableCellResizeUpdate) 목표 높이가
  // 아니라 "현재 대비 변화량"을 넘긴다 — 위에서 이미 읽어둔 `current`의
  // HWPUNIT 원본 높이를 재사용한다(그 사이 setCellProperties 는 높이를 건드리지
  // 않으므로 다시 조회할 필요가 없다).
  const targetHeightHwp = Math.round(MARKER_ROW_STYLE.cellHeightPx * HWPUNIT_PER_PX);
  const heightDelta = targetHeightHwp - current.height;
  if (heightDelta !== 0) {
    wasm.resizeTableCells(sec, ppi, ci, [{ cellIdx, heightDelta }]);
  }
}

function setTableRoleMarker(wasm: WasmBridge, sec: number, ppi: number, ci: number, markerText: string): void {
  // `readTableMarkerText`는 표의 첫 행 첫 셀 텍스트를 무엇이든 그대로 돌려준다
  // (마커 여부를 스스로 판단하지 않는다) — "이미 마커 행이 있는가"는 그 텍스트가
  // `#`로 시작하는지로 여기서 직접 판정해야 한다. 이 판정 없이 existing이 그냥
  // non-null이기만 하면 재사용 분기를 타면, 아직 마커를 단 적 없는 표(첫 셀에
  // 원본 서식의 실제 라벨 텍스트, 예: "처분재산"이 있는 표)를 태깅할 때 그 라벨을
  // 마커 텍스트로 덮어써 원본 내용을 파괴한다 — 새 마커 행을 위에 삽입하는 대신.
  const existing = readTableMarkerText(wasm, sec, ppi, ci);
  const alreadyMarked = existing !== null && existing.startsWith('#');
  if (!alreadyMarked) {
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
  const existing = readTableMarkerText(wasm, sec, ppi, ci);
  if (existing === null || !existing.startsWith('#')) return; // 실제 마커 행이 아니면 지울 것이 없다
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
    // 문서가 없어도 열고 닫을 수 있어야 한다 — 패널이 기본으로 열려 있으므로
    // hasDocument 를 요구하면 문서를 열기 전에는 닫을 방법이 없어진다.
    // 문서가 없을 때의 내용은 TemplatePanel.refresh() 의 빈 상태 안내로 처리한다.
    canExecute: () => true,
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
