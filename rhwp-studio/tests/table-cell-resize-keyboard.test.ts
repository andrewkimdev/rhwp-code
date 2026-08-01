import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildColumnResizeUpdates,
  buildLocalResizeUpdates,
  buildBoundaryResizeUpdates,
} from '../src/engine/table-resize-updates.ts';
import type { CellBbox } from '../src/core/types.ts';

// F5 셀 선택 후 키보드 셀 크기 조절 3모드(한컴 table(size).htm)의 update 구성 계약.
//
// - Ctrl/Cmd = 선택 칸(열)/줄(행) 전체에 같은 delta (표 전체 크기 변화)
// - Alt      = 선택 칸만 조절, 같은 줄 나머지가 흡수 (표 크기 유지)
// - Shift    = 선택 끝 경계 이동, 이웃이 반대로 조절 (표 크기 유지)
//
// 렌더 괘선은 열별 max base grid 를 쓰므로 Ctrl 이 단일 셀에만 delta 를 보내면
// 다행 표에서 화면에 반영되지 않는다 — 칸/줄 전체 적용이 계약의 핵심이다.

/** rows×cols 균일 그리드 bbox 생성 (w/h 는 px, 75×로 HWPUNIT 환산됨). */
function grid(rows: number, cols: number, wPx = 40, hPx = 20): CellBbox[] {
  const cells: CellBbox[] = [];
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      cells.push({
        cellIdx: r * cols + c,
        row: r,
        col: c,
        rowSpan: 1,
        colSpan: 1,
        pageIndex: 0,
        x: c * wPx,
        y: r * hPx,
        w: wPx,
        h: hPx,
      });
    }
  }
  return cells;
}

const cellAt = (cells: CellBbox[], row: number, col: number) =>
  cells.find(b => b.row === row && b.col === col)!;

function widthSum(updates: ReturnType<typeof buildColumnResizeUpdates>): number {
  return updates.reduce((s, u) => s + (u.widthDelta ?? 0), 0);
}

// ─── Ctrl: 칸/줄 전체 ─────────────────────────────────────────────

test('Ctrl 가로: 선택 열의 모든 행 셀에 같은 widthDelta 가 붙는다', () => {
  const cells = grid(5, 3);
  const range = { startRow: 2, startCol: 1, endRow: 2, endCol: 1 };
  const updates = buildColumnResizeUpdates(cells, range, 'ArrowRight');

  assert.equal(updates.length, 5, '열의 다섯 행 전부가 대상이어야 한다');
  assert.ok(updates.every(u => u.widthDelta === 300));
  assert.ok(updates.every(u => cells[u.cellIdx].col === 1), '다른 열이 섞이면 안 된다');
});

test('Ctrl 세로: 선택 행의 모든 열 셀에 heightDelta, ↑ 는 음수', () => {
  const cells = grid(4, 6);
  const range = { startRow: 1, startCol: 3, endRow: 1, endCol: 3 };
  const updates = buildColumnResizeUpdates(cells, range, 'ArrowUp');

  assert.equal(updates.length, 6);
  assert.ok(updates.every(u => u.heightDelta === -300));
});

test('Ctrl: 병합 셀은 걸친 선택 칸 수만큼 delta 를 받아 저장 폭이 동기화된다', () => {
  const cells = grid(3, 3);
  cellAt(cells, 1, 1).colSpan = 2; // col1~2 에 걸친 병합 셀
  const range = { startRow: 0, startCol: 1, endRow: 0, endCol: 1 };
  const updates = buildColumnResizeUpdates(cells, range, 'ArrowRight');

  assert.equal(updates.length, 3, '세 행 전부 대상 (병합 셀 포함)');
  const merged = updates.find(u => u.cellIdx === cellAt(cells, 1, 1).cellIdx)!;
  assert.equal(merged.widthDelta, 300, '선택 열과 한 칸 겹치므로 1×delta');
});

test('Ctrl: 선택 범위가 병합 셀을 두 칸 걸치면 2×delta', () => {
  const cells = grid(2, 4);
  cellAt(cells, 0, 1).colSpan = 2; // col1~2
  const range = { startRow: 0, startCol: 1, endRow: 0, endCol: 2 };
  const updates = buildColumnResizeUpdates(cells, range, 'ArrowRight');

  const merged = updates.find(u => u.cellIdx === cellAt(cells, 0, 1).cellIdx)!;
  assert.equal(merged.widthDelta, 600, '두 칸에 걸치므로 2×delta');
  const single = updates.find(u => u.cellIdx === cellAt(cells, 1, 1).cellIdx)!;
  assert.equal(single.widthDelta, 300);
});

// ─── Alt: 칸만 조절, 표 크기 유지 ─────────────────────────────────

test('Alt: 대상 +delta, 나머지가 흡수해 줄 합계가 0 (표 폭 유지)', () => {
  const cells = grid(1, 4);
  const range = { startRow: 0, startCol: 1, endRow: 0, endCol: 1 };
  const updates = buildLocalResizeUpdates(cells, range, 'ArrowRight');

  assert.equal(updates.length, 4);
  assert.equal(widthSum(updates), 0, '늘어난 만큼 정확히 흡수돼야 표 폭이 유지된다');
  const target = updates.find(u => u.cellIdx === cellAt(cells, 0, 1).cellIdx)!;
  assert.equal(target.widthDelta, 300);
  assert.ok(updates.every(u => u.localResize === true && typeof u.renderWidth === 'number'));
});

test('Alt: 나눠떨어지지 않는 흡수도 합계가 정확히 보존된다 (반올림 누수 금지)', () => {
  // 대상 2, 흡수 7 — 600/7 은 나눠떨어지지 않는다.
  const cells = grid(1, 9);
  const range = { startRow: 0, startCol: 0, endRow: 0, endCol: 1 };
  const updates = buildLocalResizeUpdates(cells, range, 'ArrowRight');

  assert.equal(updates.length, 9);
  assert.equal(widthSum(updates), 0, '2:7 배분에서도 표 폭이 한 단위도 새면 안 된다');
});

test('Alt: 흡수로 최소 크기 미달이 나는 줄은 통째로 건너뛴다', () => {
  const cells = grid(1, 2, 5); // 5px ≈ 375 HWPUNIT — 흡수하면 최소(300) 미달
  const range = { startRow: 0, startCol: 0, endRow: 0, endCol: 0 };
  const updates = buildLocalResizeUpdates(cells, range, 'ArrowRight');

  assert.equal(updates.length, 0);
});

test('Alt: 홑칸 줄(흡수할 이웃 없음)은 조절하지 않는다', () => {
  const cells = grid(1, 1);
  const range = { startRow: 0, startCol: 0, endRow: 0, endCol: 0 };
  assert.equal(buildLocalResizeUpdates(cells, range, 'ArrowRight').length, 0);
});

// ─── Shift: 경계 이동 ─────────────────────────────────────────────

test('Shift →: 끝 경계 이동 — 대상 +300, 오른쪽 이웃 −300', () => {
  const cells = grid(3, 3);
  const range = { startRow: 1, startCol: 1, endRow: 1, endCol: 1 };
  const updates = buildBoundaryResizeUpdates(cells, range, 'ArrowRight');

  assert.equal(updates.length, 2);
  assert.equal(widthSum(updates), 0, '경계 이동은 표 폭을 바꾸지 않는다');
  const target = updates.find(u => u.cellIdx === cellAt(cells, 1, 1).cellIdx)!;
  const neighbor = updates.find(u => u.cellIdx === cellAt(cells, 1, 2).cellIdx)!;
  assert.equal(target.widthDelta, 300);
  assert.equal(neighbor.widthDelta, -300);
});

test('Shift ←: 같은 경계가 안쪽으로 — 대상 −300, 이웃 +300', () => {
  const cells = grid(1, 3);
  const range = { startRow: 0, startCol: 1, endRow: 0, endCol: 1 };
  const updates = buildBoundaryResizeUpdates(cells, range, 'ArrowLeft');

  const target = updates.find(u => u.cellIdx === cellAt(cells, 0, 1).cellIdx)!;
  const neighbor = updates.find(u => u.cellIdx === cellAt(cells, 0, 2).cellIdx)!;
  assert.equal(target.widthDelta, -300);
  assert.equal(neighbor.widthDelta, 300);
});

test('Shift: 마지막 칸은 이웃이 없어 no-op', () => {
  const cells = grid(2, 3);
  const range = { startRow: 0, startCol: 2, endRow: 1, endCol: 2 };
  assert.equal(buildBoundaryResizeUpdates(cells, range, 'ArrowRight').length, 0);
});

test('Shift ↓: 세로 경계 — 대상 행 +300, 아래 행 −300, 열마다 한 쌍', () => {
  const cells = grid(3, 2);
  const range = { startRow: 1, startCol: 0, endRow: 1, endCol: 1 };
  const updates = buildBoundaryResizeUpdates(cells, range, 'ArrowDown');

  assert.equal(updates.length, 4, '두 열 각각 대상+이웃 한 쌍');
  assert.equal(updates.reduce((s, u) => s + (u.heightDelta ?? 0), 0), 0);
});

test('Shift: 이웃이 최소 크기 미달이 되면 그 줄은 건너뛴다', () => {
  const cells = grid(1, 2);
  cellAt(cells, 0, 1).w = 5; // ≈375 HWPUNIT — −300 하면 최소 미달
  const range = { startRow: 0, startCol: 0, endRow: 0, endCol: 0 };
  assert.equal(buildBoundaryResizeUpdates(cells, range, 'ArrowRight').length, 0);
});
