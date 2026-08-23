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
import type { CellBbox } from './types';

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
 *
 * `inclusive: true`로 호출한다 — 텍스트 없이 컨트롤만 있는 문단(`char_offsets`가
 * 비어 있는 문단)에서는 흐름 시작 전 첫 컨트롤이 항상 position 0으로 눌려 담기므로
 * (`Paragraph::control_text_positions`의 정밀도 손실 분기), `charOffset=0`으로 매
 * 이터레이션을 시작하는 이 워커는 exclusive 모드로는 그 자리의 표를 구조적으로
 * 절대 찾을 수 없다 — 워커 자신의 시작 문단(searchPara)에 있는 표라서 "이후 문단"
 * 폴백도 거치지 않는다. 이 문제는 최초 호출(문단 0)만이 아니라, 재개한 문단
 * 자신이 다시 position 0에서 시작하면 매 반복에서 재발할 수 있다.
 */
export function listTopLevelTables(wasm: WasmBridge, sec: number): TableOutlineEntry[] {
  const entries: TableOutlineEntry[] = [];
  const paraCount = wasm.getParagraphCount(sec);
  let searchPara = 0;
  let searchCharOffset = 0;
  while (searchPara < paraCount) {
    const result = wasm.findNearestControlForward(sec, searchPara, searchCharOffset, true);
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

/**
 * 드래그로 잡은 원시 행 범위를, 그 범위와 겹치는 세로 병합 셀(rowSpan)을 전부
 * 온전히 포함하도록 바깥쪽으로 확장한다.
 *
 * `getSelectedCellRange`(cursor.ts)는 anchor/focus 셀의 min/max일 뿐 rowSpan을
 * 모른다 — 헤더 블록의 일부 컬럼만 두 행에 걸쳐 병합되어 있으면(예:
 * 5446216.hwpx의 "번호"~"수령인"은 rowSpan:2인데 "사업장 소재지"만 별개 행인
 * 경우), 드래그가 병합된 컬럼의 anchor 행만 잡아도 실제로는 그 옆 병합 셀이
 * 덮는 모든 행을 포함해야 한다. 이 파일의 다른 함수와 달리 `wasm`을 직접 호출하지
 * 않고 이미 가져온 `CellBbox[]`를 받는다 — 순수 함수이므로 여전히 "조회 전용"
 * 불변조건은 지킨다.
 *
 * 확장된 범위가 다시 다른 병합 셀과 겹칠 수 있으므로(연쇄 확장) 고정점까지 반복한다.
 */
export function expandRowRangeForMerges(
  cells: readonly CellBbox[],
  range: { startRow: number; endRow: number },
): { startRow: number; endRow: number } {
  let { startRow, endRow } = range;
  let changed = true;
  while (changed) {
    changed = false;
    for (const cell of cells) {
      const cellEndRow = cell.row + cell.rowSpan - 1;
      const overlaps = cell.row <= endRow && cellEndRow >= startRow;
      if (!overlaps) continue;
      if (cell.row < startRow) {
        startRow = cell.row;
        changed = true;
      }
      if (cellEndRow > endRow) {
        endRow = cellEndRow;
        changed = true;
      }
    }
  }
  return { startRow, endRow };
}

const REPEAT_BODY_PATTERN = /^#REPEAT-BODY:(.+)$/;

// `REPEAT_BODY_PATTERN`과 별개 정규식이다 — `-NESTED`를 옵셔널 그룹으로 넣으면 이름
// capture group 위치가 바뀌므로(1→2), `availableNestedParentBlockNames`가 쓰는 그룹 번호를
// 깨지 않기 위해 새로 둔다.
const REPEAT_HEADER_MARKER_PATTERN = /^#REPEAT-HEADER(-NESTED)?:(.+)$/;
const REPEAT_BODY_MARKER_PATTERN = /^#REPEAT-BODY(-NESTED)?:(.+)$/;

/**
 * `bodyEntry`(`#REPEAT-BODY(-NESTED)?:<segment>` 표)의 문서 순서상 **바로 앞** 최상위
 * 표가 같은 `<segment>`(그리고 같은 nested 여부)를 가진 `#REPEAT-HEADER(-NESTED)?:` 표인지
 * 확인한다. 그렇지 않으면(앞 표가 없거나, 헤더 마커가 아니거나, segment가 다르면) `null`.
 *
 * "바로 앞"만 보는 이유: hwpx-template-engine 엔진(`TableRoleMarkerLintValidator`, Rust 포트
 * `src/document_core/queries/template_entity.rs`의 `find_group`)이 이미 TITLE→HEADER→
 * BODY→FOOTER를 연속한 최상위 형제 표로 요구한다 — 같은 반복 블록에 속하는
 * REPEAT-HEADER/REPEAT-BODY는 애초에 인접해야 하므로, 그 기존 불변조건을 그대로 재사용한다.
 */
export function findMatchingRepeatHeaderEntry(
  entries: readonly TableOutlineEntry[],
  bodyEntry: TableOutlineEntry,
): TableOutlineEntry | null {
  const bodyMatch = bodyEntry.markerText?.match(REPEAT_BODY_MARKER_PATTERN);
  if (!bodyMatch) return null;
  const [, bodyNested, bodySegment] = bodyMatch;

  const index = entries.findIndex(
    (e) => e.parentPara === bodyEntry.parentPara && e.controlIndex === bodyEntry.controlIndex,
  );
  if (index <= 0) return null;

  const prev = entries[index - 1];
  const headerMatch = prev.markerText?.match(REPEAT_HEADER_MARKER_PATTERN);
  if (!headerMatch) return null;
  const [, headerNested, headerSegment] = headerMatch;

  if (headerSegment !== bodySegment) return null;
  if (Boolean(headerNested) !== Boolean(bodyNested)) return null;
  return prev;
}

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
