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
 *
 * 호출부(`template-panel.ts`)는 두 가지로 범위를 좁혀 부른다 — ① 마커 게이트:
 * 역할 마커(#HEADER/#FOOTER/#PAGENO/#REPEAT-*:)가 지정된 표에서만 제안을
 * 만들고(`isTemplateTableMarkerText`), ② 행 범위: 표 전체가 아니라 사용자가
 * 선택한 행(또는 커서가 있는 행)만 검색한다(`SuggestFieldNamesOptions.rowRange`).
 */
import type { WasmBridge } from './wasm-bridge';
import { resolveUniqueName } from './field-name-dedup.ts';

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
  /** 텍스트가 있는 마지막 문단의 인덱스(0-based). 텍스트가 없으면 0. */
  lastNonEmptyParaIndex: number;
  /** `lastNonEmptyParaIndex` 문단의 원문 글자 수(그 문단 끝 charOffset). 텍스트가 없으면 0. */
  lastNonEmptyParaLength: number;
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
    let lastNonEmptyParaIndex = 0;
    let lastNonEmptyParaLength = 0;
    for (let cpi = 0; cpi < paraCount; cpi++) {
      const len = wasm.getCellParagraphLength(sec, parentPara, controlIndex, cellIdx, cpi);
      if (len > 0) {
        paraTexts.push(wasm.getTextInCell(sec, parentPara, controlIndex, cellIdx, cpi, 0, len));
        lastNonEmptyParaIndex = cpi;
        lastNonEmptyParaLength = len;
      }
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
      lastNonEmptyParaIndex,
      lastNonEmptyParaLength,
    });
  }
  return grid;
}

const REPEAT_MARKER_PATTERN = /^#REPEAT-(BODY|HEADER|FOOTER|TITLE)(-NESTED)?:/;
const GENERAL_MARKER_PATTERN = /^#(HEADER|FOOTER|PAGENO)$/;

/**
 * 텍스트가 템플릿 역할 마커 어휘(#HEADER/#FOOTER/#PAGENO/#REPEAT-*:) 그 자체인지
 * 판정한다. 두 곳에서 쓰인다:
 *
 * 1. **게이트** — 호출부(`template-panel.ts`)는 현재 표의 첫 셀 텍스트
 *    (`readTableMarkerText`)로 이 함수를 먼저 호출해, 마커가 지정된 표에서만
 *    `suggestFieldNames`를 실행한다. 제안 생성이 마커 authoring("태그 지정")의
 *    다음 단계가 되게 하는 게이트다. 단순 `#` 접두사가 아니라 어휘 전체로
 *    매칭하므로(`#HEADER2` 따위는 거짓), 첫 셀에 우연히 `#`으로 시작하는 원본
 *    텍스트가 있어도 게이트가 열리지 않는다.
 * 2. **마커 셀 제외** — 태깅된 표의 마커 행(row 0, 전체 폭 병합 셀)은
 *    authoring 주석이지 라벨이 아니므로, 규칙 2/3/4의 라벨 후보에서 제외한다.
 */
export function isTemplateTableMarkerText(text: string | null): boolean {
  if (text === null) return false;
  const stripped = text.replace(/\s+/g, '');
  return GENERAL_MARKER_PATTERN.test(stripped) || REPEAT_MARKER_PATTERN.test(stripped);
}

/**
 * 반복 블록(`#REPEAT-*:`) 마커가 붙은 표인지 판정한다. 이런 표는 행마다 같은
 * 이름이 반복되는 게 의도된 설계다(fill 시점에 `이름[N]`으로 disambiguate,
 * `rhwp-form-fill` 규약). 과거에는 제안 생성 자체를 막았지만, 제안 검색 범위가
 * "선택된 행"으로 좁아진 지금은 마커 게이트(`isTemplateTableMarkerText`)가
 * 허용하는 표 중 하나로 남는다 — 어느 행의 후보를 뽑을지 사용자가 행 선택으로
 * 직접 정하므로 "행마다 반복되는 같은 라벨" 모호성이 더 이상 발생하지 않는다.
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
  /** 채울 대상 셀 — `insertAt`이 없으면 빈 셀, 있으면 라벨 셀 자신. */
  cellIdx: number;
  row: number;
  col: number;
  leafText: string;
  sectionPrefix: string | null;
  /** 있으면 "빈 셀 채우기"가 아니라 "cellIdx 셀 자신의 텍스트 끝에 인라인 삽입" —
   * 별도 빈 셀 없이 라벨 뒤 여백에 답을 쓰는 모양(예: 인적사항 성명/병적지청)을
   * 위한 필드다. 없으면(undefined) 기존 동작(빈 셀 처음, cellParaIndex:0/charOffset:0)과
   * 같다. */
  insertAt?: { cellParaIndex: number; charOffset: number };
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
    if (isTemplateTableMarkerText(cell.strippedText)) continue; // 마커 행은 라벨이 아니다
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

/**
 * "그럴듯한 라벨"로 볼 수 있는 최대 길이(공백 제거 후) — `field-edit-dialog.ts`의
 * `MAX_FIELD_NAME_LEN`(필드 이름 기술적 상한, 250자)과는 다른 목적이다. 이 상수는
 * "라벨 아래/안 빈 공간" 규칙이 실제 라벨(성명, 그 밖의 특이사항 등 — 실측 최대
 * 7자)과 전체 폭 문단(작성방법 안내문처럼 40자 이상인 서술문)을 그리드 모양만으로
 * 구분하지 못해, 문단 바로 아래의 여백용 빈 행까지 "라벨+빈칸"으로 오탐하는 것을
 * 막기 위한 휴리스틱 상한이다.
 */
const MAX_PLAUSIBLE_LABEL_LEN = 20;

/**
 * label-above-blank: 라벨 텍스트가 있는 셀 바로 아래(`row + 1`, 같은 col)가 라벨과
 * 정확히 같은 col/colSpan을 가진 rowSpan===1 빈 셀이면, 그 빈 셀을 후보로 삼는다.
 * leafLabelAdjacentBlankRule의 자매 규칙 — "오른쪽" 대신 "아래"만 다르다(예: 전체
 * 폭 라벨 행 바로 아래 전체 폭 빈 행 — "그 밖의 특이사항"). `MAX_PLAUSIBLE_LABEL_LEN`
 * 가드는 전체 폭 문단 바로 아래의 여백용 빈 행(예: 작성방법 안내문 아래 spacer)을
 * "문단 = 라벨"로 오탐하지 않기 위함이다.
 */
const labelAboveBlankRule: RowPatternRule = (grid, prefixMap) => {
  const candidates: RowPatternCandidate[] = [];
  for (const cell of grid) {
    if (!cell.strippedText) continue;
    if (isTemplateTableMarkerText(cell.strippedText)) continue; // 마커 행은 라벨이 아니다
    if (cell.col === 0 && cell.rowSpan > 1) continue;
    if (cell.strippedText.length > MAX_PLAUSIBLE_LABEL_LEN) continue;
    const below = findGridCellAt(grid, cell.row + 1, cell.col);
    if (!below || below.col !== cell.col || below.colSpan !== cell.colSpan) continue;
    if (below.rowSpan !== 1 || below.strippedText !== '') continue;
    candidates.push({
      cellIdx: below.cellIdx,
      row: below.row,
      col: below.col,
      leafText: cell.strippedText,
      sectionPrefix: prefixMap.get(cell.row) ?? null,
    });
  }
  return candidates;
};

/** 같은 행(rowSpan으로 덮는 행 포함)에 속하는 모든 그리드 셀. */
function cellsInRow(grid: readonly GridCellText[], row: number): GridCellText[] {
  return grid.filter((c) => row >= c.row && row < c.row + c.rowSpan);
}

/** rowSpan===1이고 텍스트가 완전히 비어 있는 셀인지. */
function isBlankCell(cell: GridCellText): boolean {
  return cell.rowSpan === 1 && cell.strippedText === '';
}

/** [a.col, a.col+a.colSpan)과 [b.col, b.col+b.colSpan)이 겹치는지(경계 접촉은 겹침 아님). */
function colRangesOverlap(a: GridCellText, b: GridCellText): boolean {
  return a.col < b.col + b.colSpan && b.col < a.col + a.colSpan;
}

/**
 * label-inline-room: 라벨 뒤에 별도 빈 셀도, 바로 아래에 빈 행도 없는 "같은 셀
 * 안 여백" 모양 — 라벨 텍스트 바로 뒤(그 셀 마지막 문단 끝)에 인라인으로 채워
 * 넣는다(예: 인적사항 섹션의 "성명", "병적지청"). 아래 가드들은 "(서명 또는 인)"
 * 같은 서명 안내문이나 열 경계가 어긋난 오정렬 답변 그리드(응시지역류), 그리고
 * 이미 실제 값이 채워진 두 셀짜리 일반 행(예: "전화번호"/"이미 채워짐")을
 * 오탐하지 않기 위함이다 — 근거는 `mydocs/manual/field_naming_heuristics.md` 규칙 4.
 */
const labelInlineRoomRule: RowPatternRule = (grid, prefixMap) => {
  const claimedBlankIdxs = new Set(
    [...leafLabelAdjacentBlankRule(grid, prefixMap), ...labelAboveBlankRule(grid, prefixMap)].map(
      (c) => c.cellIdx,
    ),
  );
  const candidates: RowPatternCandidate[] = [];

  for (const cell of grid) {
    if (!cell.strippedText) continue;
    // 가드 8: 태깅된 표의 마커 행 셀은 authoring 주석이지 라벨이 아니다.
    if (isTemplateTableMarkerText(cell.strippedText)) continue;
    // 가드 2: column-0 rowSpan>1 앵커 자신은 라벨이 아니라 접두어 출처다.
    if (cell.col === 0 && cell.rowSpan > 1) continue;
    // 가드 7: 라벨치고 너무 길면(전체 폭 문단 등) 대상이 아니다 — labelAboveBlankRule과
    // 같은 이유(`MAX_PLAUSIBLE_LABEL_LEN` 참고).
    if (cell.strippedText.length > MAX_PLAUSIBLE_LABEL_LEN) continue;
    // 가드 6: 섹션 접두어 앵커가 덮는 행이 아니면 대상이 아니다 — "라벨 여러 개가
    // 나란히 있는 행"만으로는 그중 하나가 진짜 라벨(뒤에 값을 채워야 함)인지
    // 이미 값이 채워진 일반 텍스트인지 그리드만으로 구분할 수 없다("전화번호"
    // 옆의 "이미 채워짐"이 그 예). rowSpan>1 앵커가 있는 섹션(예: "인적사항")은
    // 명시적으로 "라벨+값 쌍의 묶음"이라는 저자 의도가 구조적으로 드러난
    // 경우이므로, 이 신호가 있을 때만 인라인 후보를 만든다.
    if (!prefixMap.has(cell.row)) continue;
    // 가드 1: 이 행에 그리드 셀이 하나뿐이면(전체 폭 제목/문단 행) 대상이 아니다.
    if (cellsInRow(grid, cell.row).length <= 1) continue;
    // 가드 3: 오른쪽 바로 옆이 빈 셀이면 leafLabelAdjacentBlankRule이 이미 채운다.
    const right = findGridCellAt(grid, cell.row, cell.col + cell.colSpan);
    if (right && isBlankCell(right)) continue;
    // 가드 4: 왼쪽 바로 옆이 "다른 후보가 이미 채우기로 찜한 빈 셀"이면, 이 셀은
    // 라벨이 아니라 그 빈 칸에 대한 안내문(예: "(서명 또는 인)")일 가능성이 높다.
    if (cell.col > 0) {
      const left = findGridCellAt(grid, cell.row, cell.col - 1);
      if (left && isBlankCell(left) && claimedBlankIdxs.has(left.cellIdx)) continue;
    }
    // 가드 5: 아래 행에 이 라벨의 열 범위와 겹치는 셀이 2개 이상이면(경계가 어긋난
    // 답변 그리드) 어느 서브셀이 이 라벨의 답인지 안전하게 정할 수 없다 — 자동
    // 인라인 후보로 삼지 않는다(수동 "태그 지정" 몫).
    const overlappingBelow = cellsInRow(grid, cell.row + 1).filter((b) => colRangesOverlap(cell, b));
    if (overlappingBelow.length > 1) continue;

    candidates.push({
      cellIdx: cell.cellIdx,
      row: cell.row,
      col: cell.col,
      leafText: cell.strippedText,
      sectionPrefix: prefixMap.get(cell.row) ?? null,
      insertAt: { cellParaIndex: cell.lastNonEmptyParaIndex, charOffset: cell.lastNonEmptyParaLength },
    });
  }
  return candidates;
};

/** 순서 있는 규칙 목록 — 새 레이아웃을 지원하려면 여기에 규칙을 추가한다. */
export const ROW_PATTERN_RULES: readonly RowPatternRule[] = [
  leafLabelAdjacentBlankRule,
  labelAboveBlankRule,
  labelInlineRoomRule,
];

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
  /** 있으면 "빈 셀 채우기"가 아니라 "cellIdx 셀 자신의 텍스트 끝에 인라인 삽입" —
   * `RowPatternCandidate.insertAt` 참고. UI는 이 필드의 유무로 `field-suggest:apply`에
   * `'cell'`/`'selection'` 중 어느 kind로 넘길지 정한다. */
  insertAt?: { cellParaIndex: number; charOffset: number };
}

/** `suggestFieldNames`의 검색 범위 옵션. */
export interface SuggestFieldNamesOptions {
  /** 후보를 이 행 범위(양 끝 포함, 0-based)로 한정한다 — **후보 대상 셀**(빈 칸
   * 또는 인라인 삽입의 라벨 셀, `FieldNameSuggestion.row`)이 범위 밖이면 제외된다.
   * 규칙 3(label-above-blank)처럼 라벨 행과 대상 빈 칸이 서로 다른 행에 걸쳐 있을
   * 때는 "채워질 대상"이 있는 행을 기준으로 한다 — hint(`describeSelectedRows`)에
   * 보이는 행 범위와 항상 일치해야 사용자가 범위를 눈으로 검증할 수 있다. 생략하면
   * 표 전체를 스캔한다(단위 테스트가 쓰는 기본 동작). */
  rowRange?: { startRow: number; endRow: number };
}

/**
 * 현재 표에서 누름틀 이름 후보를 계산한다. 문서에 아무것도 쓰지 않는다.
 *
 * 마커 게이트는 여기가 아니라 호출부(`template-panel.ts`)에 있다 — 마커가
 * 지정되지 않은 표에서 이 함수를 부르지 않는 것이 호출부의 책임이다
 * (`isTemplateTableMarkerText`).
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
  options: SuggestFieldNamesOptions = {},
): FieldNameSuggestion[] {
  const grid = readTableGrid(wasm, sec, parentPara, controlIndex);
  const prefixMap = buildSectionPrefixMap(grid);
  const { rowRange } = options;
  // 범위 필터는 후보 단계에서 적용한다 — 그리드/접두어 계산은 표 전체 기하학을
  // 봐야 정확하다(인접 셀 탐색, rowSpan 앵커의 접두어 매핑).
  const inRowRange = (row: number) =>
    rowRange === undefined || (row >= rowRange.startRow && row <= rowRange.endRow);
  const candidates = ROW_PATTERN_RULES.flatMap((rule) => rule(grid, prefixMap)).filter((c) =>
    inRowRange(c.row),
  );

  const existingNames = new Set(wasm.getFieldList().map((f) => f.name));
  const usedInBatch = new Set<string>();
  const suggestions: FieldNameSuggestion[] = [];

  for (const candidate of candidates) {
    const gridCell = grid.find((c) => c.cellIdx === candidate.cellIdx);
    // 두 신호를 함께 본다: ① `getCellProperties(...).fieldName` — 셀 자체가
    // "셀 필드"로 지정된 경우(표 셀 속성 대화상자의 별개 기능, insertClickHereField와
    // 무관). ② 실제 삽입 지점(빈 셀 채우기는 그 셀의 첫 문단 charOffset 0, 인라인
    // 삽입은 `insertAt`이 가리키는 지점)에 이미 `insertClickHereField`로 넣은
    // 누름틀이 있는지 — apply가 실제로 쓰는 지점과 같은 지점을 물어봐야, 이미
    // 채워진 후보를 다시 스캔했을 때 중복 삽입 없이 정확히 건너뛴다.
    let alreadyHasField = Boolean(gridCell?.existingFieldName);
    if (!alreadyHasField) {
      try {
        alreadyHasField = wasm.getFieldInfoAt({
          sectionIndex: sec,
          paragraphIndex: 0,
          charOffset: candidate.insertAt?.charOffset ?? 0,
          parentParaIndex: parentPara,
          controlIndex,
          cellIndex: candidate.cellIdx,
          cellParaIndex: candidate.insertAt?.cellParaIndex ?? 0,
        }).inField;
      } catch {
        // 조회 실패는 무시 — existingFieldName 신호만 남는다.
      }
    }
    const baseName = candidate.sectionPrefix
      ? `${candidate.sectionPrefix}_${candidate.leafText}`
      : candidate.leafText;

    const suggestedName = alreadyHasField ? baseName : resolveUniqueName(baseName, existingNames, usedInBatch);
    if (!alreadyHasField) usedInBatch.add(suggestedName);

    suggestions.push({
      cellIdx: candidate.cellIdx,
      row: candidate.row,
      col: candidate.col,
      leafText: candidate.leafText,
      sectionPrefix: candidate.sectionPrefix,
      suggestedName,
      alreadyHasField,
      existingFieldName: gridCell?.existingFieldName ?? undefined,
      insertAt: candidate.insertAt,
    });
  }

  return suggestions;
}
