/**
 * 누름틀 이름 자동 제안 — 표의 "라벨 셀 + 인접 빈 셀" 모양에서 self-describing한
 * 누름틀 이름을 제안한다. 읽기 전용(wasm 쓰기 없음) — `src/core/table-outline.ts`와
 * 같은 이유로 `src/core/`에 둔다(`mutation-routing-guard.test.ts`가 스캔하는
 * `src/command/` 밖).
 *
 * 라벨을 그대로 이름으로 쓰면(예: "전화번호") 같은 라벨이 문서 안에 두 번 이상
 * 나올 때(신청인 섹션과 법인명 섹션에 각각 "전화번호") 충돌한다. 이를 trial-and-error가
 * 아니라 구조적으로 풀기 위해, column-0에 있고 `rowSpan > 1`인 부모 셀(예:
 * "신\n청\n인")의 텍스트를 그 rowSpan이 덮는 모든 행에 접두어로 붙인다
 * ("신청인_전화번호"). 접두어가 없는 표(부모 셀이 없는 단순 표)는 라벨을 그대로 쓴다.
 *
 * 라벨→빈 셀 관계를 찾는 규칙은 `RowPatternRule[]`로 뽑아 뒀다 — v1은
 * "라벨 오른쪽 바로 옆 빈 셀" 한 가지뿐이지만, 새 서식에서 다른 모양(라벨 아래
 * 빈칸, 라벨+콜론 한 셀 등)이 나오면 이 배열에 규칙을 추가하기만 하면 된다.
 * 자세한 설명은 `mydocs/manual/field_naming_heuristics.md`.
 */
import type { WasmBridge } from './wasm-bridge';

/** 표의 한 셀에서 읽은 텍스트/서식 정보 (제안 계산의 기초 자료). */
export interface GridCellText {
  cellIdx: number;
  row: number;
  col: number;
  rowSpan: number;
  colSpan: number;
  /** 모든 문단을 이어붙인 원문. */
  rawText: string;
  /** `rawText`에서 공백(줄바꿈 포함)을 전부 제거한 텍스트 — 음절별 줄바꿈 같은
   * 레이아웃 잡음을 없앤 비교/이름 후보 기준. */
  strippedText: string;
  /** 이 셀에 이미 누름틀/셀 필드가 있으면 그 이름, 없으면 null. */
  existingFieldName: string | null;
}

/** 표 하나의 전체 셀 그리드를 읽는다 (row/col/rowSpan/colSpan + 텍스트 + 기존 필드명). */
export function readTableGrid(
  wasm: WasmBridge,
  sec: number,
  parentPara: number,
  controlIndex: number,
): GridCellText[] {
  const dims = wasm.getTableDimensions(sec, parentPara, controlIndex);
  const grid: GridCellText[] = [];
  for (let cellIdx = 0; cellIdx < dims.cellCount; cellIdx++) {
    const info = wasm.getCellInfo(sec, parentPara, controlIndex, cellIdx);
    const paraCount = wasm.getCellParagraphCount(sec, parentPara, controlIndex, cellIdx);
    const paraTexts: string[] = [];
    for (let cpi = 0; cpi < paraCount; cpi++) {
      const len = wasm.getCellParagraphLength(sec, parentPara, controlIndex, cellIdx, cpi);
      if (len > 0) paraTexts.push(wasm.getTextInCell(sec, parentPara, controlIndex, cellIdx, cpi, 0, len));
    }
    const rawText = paraTexts.join('');
    let existingFieldName: string | null = null;
    try {
      existingFieldName = wasm.getCellProperties(sec, parentPara, controlIndex, cellIdx).fieldName ?? null;
    } catch {
      existingFieldName = null;
    }
    grid.push({
      cellIdx,
      row: info.row,
      col: info.col,
      rowSpan: info.rowSpan,
      colSpan: info.colSpan,
      rawText,
      strippedText: rawText.replace(/\s+/g, ''),
      existingFieldName,
    });
  }
  return grid;
}

const REPEAT_MARKER_PATTERN = /^#REPEAT-(BODY|HEADER|FOOTER|TITLE)(-NESTED)?:/;

/**
 * 반복 블록(`#REPEAT-*:`) 마커가 붙은 표인지 판정한다. 이런 표는 행마다 같은
 * 이름이 반복되는 게 의도된 설계이므로(fill 시점에 `이름[N]`으로 disambiguate,
 * `rhwp-form-fill` 규약) 이름 제안 생성 자체를 막아야 한다 — 호출부
 * (`template-panel.ts`)가 `suggestFieldNames`를 부르기 전에 이 함수로 먼저 걸러낸다.
 */
export function isRepeatTaggedTable(markerText: string | null): boolean {
  return markerText !== null && REPEAT_MARKER_PATTERN.test(markerText);
}

function findGridCellAt(grid: readonly GridCellText[], row: number, col: number): GridCellText | undefined {
  return grid.find((c) => row >= c.row && row < c.row + c.rowSpan && col >= c.col && col < c.col + c.colSpan);
}

/**
 * column-0에 있고 rowSpan > 1인 앵커 셀의 텍스트를, 그 rowSpan이 덮는 모든 행에
 * 접두어로 매핑한다. 새 앵커 모양(예: row-0 헤더 병합)이 생기면 이 함수만
 * 바꾸거나 나란히 새 함수를 추가하면 된다 — `suggestFieldNames`는 결과 Map만 본다.
 */
export function buildSectionPrefixMap(grid: readonly GridCellText[]): Map<number, string> {
  const map = new Map<number, string>();
  for (const cell of grid) {
    if (cell.col !== 0 || cell.rowSpan <= 1) continue;
    if (!cell.strippedText) continue;
    for (let row = cell.row; row < cell.row + cell.rowSpan; row++) {
      map.set(row, cell.strippedText);
    }
  }
  return map;
}

/** 하나의 row-pattern 규칙이 찾아낸, "이 빈 셀에 이 라벨로 채워라" 후보. */
export interface RowPatternCandidate {
  /** 채울 대상 빈 셀. */
  cellIdx: number;
  row: number;
  col: number;
  leafText: string;
  sectionPrefix: string | null;
}

/**
 * row 모양 하나를 인식해 후보를 뽑는 순수 함수. 새 레이아웃이 나오면 이 시그니처로
 * 규칙을 하나 더 만들어 `ROW_PATTERN_RULES`에 추가한다.
 */
export type RowPatternRule = (
  grid: readonly GridCellText[],
  prefixMap: ReadonlyMap<number, string>,
) => RowPatternCandidate[];

/**
 * leaf-label-adjacent-blank: 라벨 텍스트가 있는 셀 바로 오른쪽(`col + colSpan`)이
 * rowSpan===1인 빈 셀이면, 그 빈 셀을 "라벨 텍스트로 채울 후보"로 삼는다.
 * column-0에 있고 rowSpan>1인 셀(섹션 접두어를 이미 제공하는 앵커 자신)은
 * 라벨 후보에서 제외한다 — 그 셀 자체는 채울 빈 칸이 아니라 접두어 출처다.
 */
const leafLabelAdjacentBlankRule: RowPatternRule = (grid, prefixMap) => {
  const candidates: RowPatternCandidate[] = [];
  for (const cell of grid) {
    if (!cell.strippedText) continue;
    if (cell.col === 0 && cell.rowSpan > 1) continue;
    const blank = findGridCellAt(grid, cell.row, cell.col + cell.colSpan);
    if (!blank || blank.rowSpan !== 1 || blank.strippedText !== '') continue;
    candidates.push({
      cellIdx: blank.cellIdx,
      row: blank.row,
      col: blank.col,
      leafText: cell.strippedText,
      sectionPrefix: prefixMap.get(cell.row) ?? null,
    });
  }
  return candidates;
};

/** 순서 있는 규칙 목록 — 새 레이아웃을 지원하려면 여기에 규칙을 추가한다. */
export const ROW_PATTERN_RULES: readonly RowPatternRule[] = [leafLabelAdjacentBlankRule];

/** 검토 목록 한 행 — UI가 그대로 렌더링하는 단위. */
export interface FieldNameSuggestion {
  cellIdx: number;
  row: number;
  col: number;
  leafText: string;
  sectionPrefix: string | null;
  /** 접두어 + 라벨, 문서 내 기존 필드명 및 같은 배치 안 다른 제안과 충돌하지 않도록
   * `_2`, `_3`, ... 접미어까지 반영된 최종 제안. */
  suggestedName: string;
  /** true면 대상 셀에 이미 필드가 있어 "삽입" 대상에서 제외된다(재검토 표시만). */
  alreadyHasField: boolean;
  existingFieldName?: string;
}

/**
 * 현재 표에서 누름틀 이름 후보를 계산한다. 문서에 아무것도 쓰지 않는다.
 *
 * 이미 필드가 있는 셀도 후보 목록에는 포함하되(`alreadyHasField: true`), 이름
 * 유일성 계산에는 참여시키지 않는다 — 그 셀은 v1에서 절대 새로 채워지지 않으므로
 * 다른 제안의 접미어 계산에 영향을 줄 이유가 없다.
 */
export function suggestFieldNames(
  wasm: WasmBridge,
  sec: number,
  parentPara: number,
  controlIndex: number,
): FieldNameSuggestion[] {
  const grid = readTableGrid(wasm, sec, parentPara, controlIndex);
  const prefixMap = buildSectionPrefixMap(grid);
  const candidates = ROW_PATTERN_RULES.flatMap((rule) => rule(grid, prefixMap));

  const existingNames = new Set(wasm.getFieldList().map((f) => f.name));
  const usedInBatch = new Set<string>();
  const suggestions: FieldNameSuggestion[] = [];

  for (const candidate of candidates) {
    const gridCell = grid.find((c) => c.cellIdx === candidate.cellIdx);
    const alreadyHasField = Boolean(gridCell?.existingFieldName);
    const baseName = candidate.sectionPrefix
      ? `${candidate.sectionPrefix}_${candidate.leafText}`
      : candidate.leafText;

    let suggestedName = baseName;
    if (!alreadyHasField) {
      let suffix = 2;
      while (existingNames.has(suggestedName) || usedInBatch.has(suggestedName)) {
        suggestedName = `${baseName}_${suffix}`;
        suffix++;
      }
      usedInBatch.add(suggestedName);
    }

    suggestions.push({
      cellIdx: candidate.cellIdx,
      row: candidate.row,
      col: candidate.col,
      leafText: candidate.leafText,
      sectionPrefix: candidate.sectionPrefix,
      suggestedName,
      alreadyHasField,
      existingFieldName: gridCell?.existingFieldName ?? undefined,
    });
  }

  return suggestions;
}
