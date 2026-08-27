/**
 * "선택된 행"의 정의 — 템플릿 패널의 힌트 표시와 누름틀 생성이 공유하는 순수 조회 유틸.
 *
 * 원래 TemplatePanel의 private 메서드였는데, 힌트(`describeSelectedRows`)와
 * 누름틀 생성(`createFieldsFromRows`, fieldsuggest.ts)이 같은 정의를 공유해야
 * "힌트에 보이는 행 = 실제로 생성되는 행"이 항상 성립하므로 두 모듈 사이의
 * 단일 소스로 분리했다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { InputHandler } from '@/engine/input-handler';
import { expandRowRangeForMerges } from '@/core/table-outline';

export interface RowRange {
  startRow: number;
  endRow: number;
}

type CursorPosition = NonNullable<ReturnType<InputHandler['getCursorPosition']>>;

/**
 * "선택된 행"을 (0-based, 양 끝 포함)으로 돌려준다 — 셀 선택 모드면 그 범위,
 * 아니면 커서가 있는 셀의 행 한 줄. `describeSelectedRows`(hint 표시)와
 * `createFieldsFromRows`(생성 검색 범위, fieldsuggest.ts)가 같은 정의를
 * 공유해야 "힌트에 보이는 행 = 실제로 생성되는 행"이 항상 성립한다.
 */
export function getSelectedRowRange(
  wasm: WasmBridge,
  ih: InputHandler | null,
  pos: CursorPosition,
): RowRange | null {
  const range = ih?.isInCellSelectionMode() ? ih.getSelectedCellRange() : null;
  let raw: RowRange | null = range
    ? { startRow: range.startRow, endRow: range.endRow }
    : null;
  if (!raw && pos.cellIndex !== undefined) {
    try {
      const info = wasm.getCellInfo(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex);
      raw = { startRow: info.row, endRow: info.row };
    } catch {
      // 무시 — null 반환 (호출부의 fallback 처리)
    }
  }
  if (!raw || pos.parentParaIndex === undefined || pos.controlIndex === undefined) return raw;
  // 드래그가 rowSpan을 몰라서 병합 셀 일부만 잡을 수 있다 — 태깅 시 실제로
  // 적용될 범위와 힌트/제안이 항상 일치하도록 여기서 한 번만 넓힌다
  // (template-ops.ts의 template:tag-selection 실행부도 같은 헬퍼를 쓴다).
  try {
    const bboxes = wasm.getTableCellBboxes(pos.sectionIndex, pos.parentParaIndex, pos.controlIndex);
    return expandRowRangeForMerges(bboxes, raw);
  } catch {
    return raw;
  }
}

/** 행 범위를 1-based 표기("3" 또는 "3~5")로 바꾼다 — hint 문구와 제안 메시지가 공유한다. */
export function formatRowRange(range: RowRange): string {
  return range.startRow === range.endRow
    ? `${range.startRow + 1}`
    : `${range.startRow + 1}~${range.endRow + 1}`;
}

/** 힌트용 한 줄 요약 — 행 범위를 잡지 못하면 안내 문구로 대체한다. */
export function describeSelectedRows(
  wasm: WasmBridge,
  ih: InputHandler | null,
  pos: CursorPosition,
): string {
  const rowRange = getSelectedRowRange(wasm, ih, pos);
  if (!rowRange) {
    return '표 안에 커서를 두면 태깅할 수 있습니다.';
  }
  return `선택된 행: ${formatRowRange(rowRange)}`;
}
