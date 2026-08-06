/**
 * F5 셀 블록 선택을 서식 적용 대상으로 바꾸는 순수 로직.
 *
 * 셀 블록 선택은 cellAnchor/cellFocus 축이라 텍스트 선택(cursor.anchor)을 만들지 않는다.
 * 그래서 대상 산출은 "행/열 범위 + 제외 셀"에서 셀 인덱스를 골라내는 별도 경로가 된다.
 * 이 모듈은 wasm/커서 상태 없이 그 산출만 담당해 단위 테스트가 가능하게 한다.
 */
export type CellRowCol = { row: number; col: number };

export type CellBlockRange = {
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
};

/**
 * 제외 셀 Set 의 키를 만든다.
 *
 * 키를 조립하는 쪽(CursorState.ctrlToggleCell)과 조회하는 쪽이 형식을 따로 갖고 있으면
 * 한쪽만 바뀌어도 조회가 조용히 빗나가 제외 셀이 무시된다. 형식은 여기 한 곳에서만 만든다.
 */
export function excludedCellKey(row: number, col: number): string {
  return `${row},${col}`;
}

/** 선택 범위에 드는 셀 인덱스를 고른다. 제외 셀(Ctrl+클릭)은 뺀다. */
export function selectCellIndicesInRange(
  cellCount: number,
  cellInfoAt: (cellIdx: number) => CellRowCol,
  range: CellBlockRange,
  excluded: ReadonlySet<string>,
): number[] {
  const cellIndices: number[] = [];
  for (let cellIdx = 0; cellIdx < cellCount; cellIdx++) {
    const info = cellInfoAt(cellIdx);
    if (info.row < range.startRow || info.row > range.endRow ||
        info.col < range.startCol || info.col > range.endCol) {
      continue;
    }
    if (excluded.has(excludedCellKey(info.row, info.col))) continue;
    cellIndices.push(cellIdx);
  }
  return cellIndices;
}
