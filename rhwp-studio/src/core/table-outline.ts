/**
 * hwpx-template-engine 마커 authoring 을 위한 읽기 전용 표 개요(outline) 헬퍼.
 *
 * 문서에 존재하는 최상위(top-level) 표를 문서 순서대로 나열하고, 각 표의 첫 행
 * 첫 셀에 이미 적혀 있는 역할 마커 텍스트(`#HEADER`/`#REPEAT-BODY:<name>` 등,
 * 아직 없으면 null)를 읽어 온다. 여기 있는 함수는 전부 조회 전용이다 —
 * `mutation-method-registry.ts`의 뮤테이터 원장에 걸리지 않으므로 `src/core`에
 * 둬도 안전하다. 실제 마커를 쓰거나 표를 나누는 로직은 `src/command/commands/template.ts`에
 * 있다(그쪽은 뮤테이션 표면 원장이 스캔하는 `src/command/` 아래에 있어야 한다).
 */
import type { WasmBridge } from './wasm-bridge';

export interface TableOutlineEntry {
  sec: number;
  parentPara: number;
  controlIndex: number;
  rowCount: number;
  colCount: number;
  /** 첫 행 첫 셀 텍스트가 `#`로 시작하면 그 원문, 아니면 null. */
  markerText: string | null;
}

/** (row, col)을 포함하는 셀의 flat cellIdx를 찾는다. 없으면 null. */
export function findCellIndexForRowCol(
  wasm: WasmBridge,
  sec: number,
  parentPara: number,
  controlIndex: number,
  row: number,
  col: number,
): number | null {
  const dims = wasm.getTableDimensions(sec, parentPara, controlIndex);
  for (let cellIdx = 0; cellIdx < dims.cellCount; cellIdx++) {
    const info = wasm.getCellInfo(sec, parentPara, controlIndex, cellIdx);
    if (
      row >= info.row && row < info.row + info.rowSpan &&
      col >= info.col && col < info.col + info.colSpan
    ) {
      return cellIdx;
    }
  }
  return null;
}

/** 표의 첫 행 첫 셀 텍스트를 읽는다. 비어 있거나 표가 비정상이면 null. */
export function readTableMarkerText(
  wasm: WasmBridge,
  sec: number,
  parentPara: number,
  controlIndex: number,
): string | null {
  const dims = wasm.getTableDimensions(sec, parentPara, controlIndex);
  if (dims.rowCount === 0 || dims.colCount === 0) return null;
  const cellIdx = findCellIndexForRowCol(wasm, sec, parentPara, controlIndex, 0, 0);
  if (cellIdx === null) return null;
  const len = wasm.getCellParagraphLength(sec, parentPara, controlIndex, cellIdx, 0);
  if (len <= 0) return null;
  const text = wasm.getTextInCell(sec, parentPara, controlIndex, cellIdx, 0, 0, len);
  const trimmed = text.trim();
  return trimmed.length > 0 ? trimmed : null;
}

/**
 * 지정 section의 최상위 표를 문서 순서대로 나열한다.
 *
 * `findNearestControlForward`가 컨트롤을 문단 단위로 찾아 주는 것을 이용한다 —
 * 최상위 컨트롤(표 포함)은 자기 전용 문단을 차지하므로, 찾은 컨트롤의 문단
 * 바로 다음(`result.para + 1`)부터 다시 탐색하면 같은 컨트롤을 다시 찾지 않고
 * 전진한다(Shift+F11 핸들러, `input-handler-keyboard.ts`의 `handleShiftF11`와
 * 같은 전진 패턴).
 */
export function listTopLevelTables(wasm: WasmBridge, sec: number): TableOutlineEntry[] {
  const entries: TableOutlineEntry[] = [];
  const paraCount = wasm.getParagraphCount(sec);
  let searchPara = 0;
  let searchCharOffset = 0;
  while (searchPara < paraCount) {
    const result = wasm.findNearestControlForward(sec, searchPara, searchCharOffset);
    if (!result || result.type === 'none') break;
    if (result.type === 'table') {
      const dims = wasm.getTableDimensions(result.sec, result.para, result.ci);
      entries.push({
        sec: result.sec,
        parentPara: result.para,
        controlIndex: result.ci,
        rowCount: dims.rowCount,
        colCount: dims.colCount,
        markerText: readTableMarkerText(wasm, result.sec, result.para, result.ci),
      });
    }
    // 컨트롤은 자기 문단을 차지하므로 다음 문단부터 다시 찾는다. 전진하지
    // 않으면(비정상 응답 등) 무한루프가 되므로 안전장치로 멈춘다.
    if (result.para < searchPara) break;
    searchPara = result.para + 1;
    searchCharOffset = 0;
  }
  return entries;
}

const REPEAT_BODY_PATTERN = /^#REPEAT-BODY:(.+)$/;

/**
 * `-NESTED:` 마커를 authoring할 수 있는 부모 블록명 목록.
 *
 * 엔진의 `TableRoleMarker.findChildRepeatBlockGroup`은 자식 `-NESTED:` 표 그룹이
 * 부모의 `#REPEAT-BODY:<name>` 표 바로 뒤에 연속한 최상위 형제로 와야 한다고
 * 요구한다. 이 도구는 사용자가 지금 편집 중인 자리에 표를 만들기 때문에 위치
 * 자체는 항상 올바르지만(사용자가 정확히 그 지점에서 태깅함), 어떤 이름을
 * 부모로 선택할 수 있는지는 문서에 실제로 `#REPEAT-BODY:<name>` 표가 있는지로
 * 걸러야 한다 — v1은 이 존재 여부만 확인한다(정확한 인접성 재검증은 `#6.
 * "Validate now"`의 실제 lint가 최종 권위다).
 */
export function availableNestedParentBlockNames(entries: readonly TableOutlineEntry[]): string[] {
  const names = new Set<string>();
  for (const entry of entries) {
    const match = entry.markerText?.match(REPEAT_BODY_PATTERN);
    if (match) names.add(match[1]);
  }
  return [...names];
}
