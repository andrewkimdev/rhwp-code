// F5 셀 선택 후 키보드 셀 크기 조절 3모드(한컴 table(size).htm)의
// resizeTableCells update 구성. WASM/DOM 의존이 없는 순수 로직이라
// 단위 테스트가 직접 검증한다 (tests/table-cell-resize-keyboard.test.ts).

import type { CellBbox } from '../core/types';

export type ResizeArrowKey = 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight';

export type CellSelectionRange = {
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
};

export type LocalResizeUpdate = {
  cellIdx: number;
  widthDelta?: number;
  heightDelta?: number;
  localResize?: boolean;
  renderWidth?: number;
  renderHeight?: number;
};

/** 키보드 셀 크기 조절: 1 키스트로크 당 이동량 (HWPUNIT, 약 1mm). */
const KEYBOARD_RESIZE_DELTA_HWP = 300;
/** px(96dpi) → HWPUNIT 환산 배율 (7200/96). getCellDisplaySize 와 동일 계약. */
const RESIZE_HWPUNIT_PER_PX = 75;
/** 셀 최소 크기 (HWPUNIT) — Rust MIN_CELL_SIZE(200)보다 보수적으로 300. */
const RESIZE_MIN_CELL_HWP = 300;

function resizeAxis(key: ResizeArrowKey): { isHoriz: boolean; delta: number } {
  const isHoriz = key === 'ArrowLeft' || key === 'ArrowRight';
  const delta =
    key === 'ArrowRight' || key === 'ArrowDown'
      ? KEYBOARD_RESIZE_DELTA_HWP
      : -KEYBOARD_RESIZE_DELTA_HWP;
  return { isHoriz, delta };
}

/** 페이지 fragment 중복을 제거하고 조절 축의 span==1 셀만 남긴다. */
function collectSpanOneCells(bboxes: CellBbox[], isHoriz: boolean): CellBbox[] {
  const seen = new Set<number>();
  const out: CellBbox[] = [];
  for (const b of bboxes) {
    if (seen.has(b.cellIdx)) continue;
    if (isHoriz ? b.colSpan !== 1 : b.rowSpan !== 1) continue;
    seen.add(b.cellIdx);
    out.push(b);
  }
  return out;
}

function cellSizeHwp(b: CellBbox, isHoriz: boolean): number {
  return Math.round((isHoriz ? b.w : b.h) * RESIZE_HWPUNIT_PER_PX);
}

function sizeUpdate(
  cellIdx: number,
  isHoriz: boolean,
  delta: number,
  renderSize: number,
): LocalResizeUpdate {
  return isHoriz
    ? { cellIdx, widthDelta: delta, localResize: true, renderWidth: renderSize }
    : { cellIdx, heightDelta: delta, localResize: true, renderHeight: renderSize };
}

/**
 * Ctrl/Cmd+방향키: 선택 칸(열)/줄(행) 전체에 같은 delta — 표 전체 크기가 변한다.
 *
 * 렌더 괘선은 열별 max 로 만드는 base grid 를 쓰므로, 셀 하나만 조절하면
 * 다행 표에서 열 max 가 그대로라 화면에 반영되지 않는다. 병합 셀은 걸친
 * 선택 칸/줄 수만큼 delta 를 곱해 저장 폭/높이를 동기화한다 — 빼놓으면
 * 저장·재열기 후 병합 셀 폭이 열 폭 합과 어긋난다.
 */
export function buildColumnResizeUpdates(
  bboxes: CellBbox[],
  range: CellSelectionRange,
  key: ResizeArrowKey,
): LocalResizeUpdate[] {
  const { isHoriz, delta } = resizeAxis(key);
  const updates: LocalResizeUpdate[] = [];
  const seen = new Set<number>();
  for (const b of bboxes) {
    if (seen.has(b.cellIdx)) continue;
    seen.add(b.cellIdx);
    // 셀이 걸친 선택 칸(열)/줄(행) 수 — span==1 이면 0 또는 1.
    const [lo, hi] = isHoriz ? [b.col, b.col + b.colSpan - 1] : [b.row, b.row + b.rowSpan - 1];
    const [selLo, selHi] = isHoriz ? [range.startCol, range.endCol] : [range.startRow, range.endRow];
    const overlap = Math.min(hi, selHi) - Math.max(lo, selLo) + 1;
    if (overlap <= 0) continue;
    const d = delta * overlap;
    updates.push(isHoriz ? { cellIdx: b.cellIdx, widthDelta: d } : { cellIdx: b.cellIdx, heightDelta: d });
  }
  return updates;
}

/**
 * Alt+방향키: 선택 칸만 조절, 표 전체 크기 유지 — 같은 줄(가로)/칸(세로)의
 * 나머지 셀이 반대 방향으로 흡수한다. 흡수량 합계는 delta 합계와 정확히
 * 일치시킨다(나머지는 뒤쪽 셀부터 1씩 배분). 최소 크기 미달이 생기는
 * 줄/칸은 통째로 건너뛴다.
 */
export function buildLocalResizeUpdates(
  bboxes: CellBbox[],
  range: CellSelectionRange,
  key: ResizeArrowKey,
): LocalResizeUpdate[] {
  const { isHoriz, delta } = resizeAxis(key);
  const cells = collectSpanOneCells(bboxes, isHoriz);
  const inRange = (b: CellBbox) =>
    isHoriz
      ? b.col >= range.startCol && b.col <= range.endCol
      : b.row >= range.startRow && b.row <= range.endRow;
  const laneOf = (b: CellBbox) => (isHoriz ? b.row : b.col);
  const inLaneRange = (b: CellBbox) =>
    isHoriz
      ? b.row >= range.startRow && b.row <= range.endRow
      : b.col >= range.startCol && b.col <= range.endCol;
  const lanes = new Set(cells.filter(b => inLaneRange(b) && inRange(b)).map(laneOf));

  const updates: LocalResizeUpdate[] = [];
  for (const lane of lanes) {
    const laneCells = cells.filter(b => laneOf(b) === lane);
    const targets = laneCells.filter(inRange);
    const others = laneCells.filter(b => !inRange(b));
    if (targets.length === 0 || others.length === 0) continue; // 홑칸 줄 — 유지 불가

    // 흡수량 배분: 합계를 정확히 delta*targets 로 맞춘다. 균등 몫에서 남는
    // 나머지는 뒤쪽 셀부터 1 HWPUNIT 씩 얹는다 (반올림 누적으로 표 폭이
    // 새는 것을 막는다).
    const total = delta * targets.length;
    const base = Math.trunc(total / others.length);
    const remainder = total - base * others.length; // |remainder| < others.length
    const extra = Math.abs(remainder);
    const sign = Math.sign(remainder);
    const absorbs = others.map((_, i) => base + (others.length - 1 - i < extra ? sign : 0));

    if (targets.some(t => cellSizeHwp(t, isHoriz) + delta < RESIZE_MIN_CELL_HWP)) continue;
    if (others.some((o, i) => cellSizeHwp(o, isHoriz) - absorbs[i] < RESIZE_MIN_CELL_HWP)) continue;

    for (const t of targets) {
      updates.push(sizeUpdate(t.cellIdx, isHoriz, delta, cellSizeHwp(t, isHoriz) + delta));
    }
    others.forEach((o, i) => {
      updates.push(sizeUpdate(o.cellIdx, isHoriz, -absorbs[i], cellSizeHwp(o, isHoriz) - absorbs[i]));
    });
  }
  return updates;
}

/**
 * Shift+방향키: 경계 이동 — 셀이 커진 만큼 이웃이 작아진다. 표 전체 크기 유지.
 *
 * 정책: 항상 선택 끝(오른쪽/아래) 경계를 움직인다. →/↓ 는 바깥으로(셀 +,
 * 이웃 −), ←/↑ 는 안으로(셀 −, 이웃 +). 마지막 칸/줄은 이웃이 없어 no-op.
 */
export function buildBoundaryResizeUpdates(
  bboxes: CellBbox[],
  range: CellSelectionRange,
  key: ResizeArrowKey,
): LocalResizeUpdate[] {
  const { isHoriz, delta } = resizeAxis(key);
  const cells = collectSpanOneCells(bboxes, isHoriz);

  const updates: LocalResizeUpdate[] = [];
  const laneStart = isHoriz ? range.startRow : range.startCol;
  const laneEnd = isHoriz ? range.endRow : range.endCol;
  for (let lane = laneStart; lane <= laneEnd; lane++) {
    const at = (main: number, cross: number) =>
      cells.find(b => (isHoriz ? b.row === cross && b.col === main : b.col === cross && b.row === main));
    const edge = isHoriz ? range.endCol : range.endRow;
    const target = at(edge, lane);
    const neighbor = at(edge + 1, lane);
    if (!target || !neighbor) continue; // 마지막 칸/줄 — 이웃 없음
    if (cellSizeHwp(target, isHoriz) + delta < RESIZE_MIN_CELL_HWP) continue;
    if (cellSizeHwp(neighbor, isHoriz) - delta < RESIZE_MIN_CELL_HWP) continue;
    updates.push(sizeUpdate(target.cellIdx, isHoriz, delta, cellSizeHwp(target, isHoriz) + delta));
    updates.push(sizeUpdate(neighbor.cellIdx, isHoriz, -delta, cellSizeHwp(neighbor, isHoriz) - delta));
  }
  return updates;
}

