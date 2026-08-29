/** CursorState F5 셀 블록·표 객체·그림 객체 선택 모드 메서드 — extracted from CursorState class */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { CellPathEntry, CellBbox } from '@/core/types';
// 제외 셀 Set 의 키 형식은 조립하는 쪽과 조회하는 쪽이 반드시 같아야 한다 → 단일 정의.
import { excludedCellKey } from './cell-block-format';
import type { CellSelectionReason, PictureSelectionRef } from './cursor';

// ─── F5 셀 블록 선택 모드 ─────────────────────────────────

/** 셀 선택 모드에 진입한다. 현재 셀의 row/col이 anchor/focus가 된다. */
export function enterCellSelectionMode(this: any, reason: CellSelectionReason = 'manual'): boolean {
  if (!this.isInCell() || this.isInTextBox()) return false;
  const { sectionIndex: sec, parentParaIndex: ppi, controlIndex: ci, cellIndex: cei, cellPath } = this.position;
  if (ppi === undefined || ci === undefined || cei === undefined) return false;

  try {
    let info, dims;
    if ((cellPath?.length ?? 0) > 0) {
      // cellPath가 있으면 1-depth 표도 경로 기반 API 사용
      const pathJson = JSON.stringify(cellPath);
      info = this.wasm.getCellInfoByPath(sec, ppi, pathJson);
      dims = this.wasm.getTableDimensionsByPath(sec, ppi, pathJson);
    } else {
      info = this.wasm.getCellInfo(sec, ppi, ci, cei);
      dims = this.wasm.getTableDimensions(sec, ppi, ci);
    }
    this.cellAnchor = { row: info.row, col: info.col };
    this.cellFocus = { row: info.row, col: info.col };
    this.cellTableCtx = { sec, ppi, ci, rowCount: dims.rowCount, colCount: dims.colCount, cellPath };
    this._cellSelectionMode = true;
    this._cellSelectionPhase = 1;
    this._cellSelectionReason = reason;
    return true;
  } catch (e) {
    console.warn('[CursorState] enterCellSelectionMode 실패:', e);
    return false;
  }
}

/** 셀 선택 모드를 종료한다. */
export function exitCellSelectionMode(this: any): void {
  this._cellSelectionMode = false;
  this._cellSelectionPhase = 1;
  this._cellSelectionReason = 'manual';
  this.cellAnchor = null;
  this.cellFocus = null;
  this.excludedCells.clear();
  this.cellTableCtx = null;
}

/** 범위 선택: anchor 고정, focus만 이동 (phase 2) */
export function expandCellSelection(this: any, deltaRow: number, deltaCol: number): void {
  if (!this._cellSelectionMode || !this.cellFocus || !this.cellTableCtx) return;
  const { rowCount, colCount } = this.cellTableCtx;
  this.cellFocus = {
    row: Math.max(0, Math.min(rowCount - 1, this.cellFocus.row + deltaRow)),
    col: Math.max(0, Math.min(colCount - 1, this.cellFocus.col + deltaCol)),
  };
}

/** 셀 선택을 화살표 방향으로 이동한다 (anchor/focus 함께 이동, 단일 셀 선택). */
export function moveCellSelection(this: any, deltaRow: number, deltaCol: number): void {
  if (!this._cellSelectionMode || !this.cellFocus || !this.cellTableCtx) return;
  const { rowCount, colCount } = this.cellTableCtx;
  const { sec, ppi, ci, cellPath } = this.cellTableCtx;

  // 현재 셀의 병합 정보 조회
  let bboxes: CellBbox[];
  try {
    bboxes = cellPath
      ? this.wasm.getTableCellBboxesByPath(sec, ppi, JSON.stringify(cellPath))
      : this.wasm.getTableCellBboxes(sec, ppi, ci!);
  } catch { bboxes = []; }

  const curCell = bboxes.find(b =>
    this.cellFocus!.row >= b.row && this.cellFocus!.row < b.row + b.rowSpan &&
    this.cellFocus!.col >= b.col && this.cellFocus!.col < b.col + b.colSpan
  );

  let newRow = this.cellFocus.row;
  let newCol = this.cellFocus.col;

  if (curCell) {
    if (deltaCol > 0) {
      // 오른쪽: 현재 셀의 오른쪽 끝 다음 열로 이동
      newCol = curCell.col + curCell.colSpan;
      if (newCol >= colCount) {
        // 오른쪽에 셀 없음 → 다음 행 첫 열
        newRow = curCell.row + curCell.rowSpan;
        newCol = 0;
      }
    } else if (deltaCol < 0) {
      // 왼쪽: 현재 셀 왼쪽 열로 이동
      newCol = curCell.col - 1;
      if (newCol < 0) {
        // 왼쪽에 셀 없음 → 이전 행 마지막 열
        newRow = curCell.row - 1;
        newCol = colCount - 1;
      }
    } else if (deltaRow > 0) {
      // 아래: 현재 셀 하단 다음 행으로 이동
      newRow = curCell.row + curCell.rowSpan;
    } else if (deltaRow < 0) {
      // 위: 현재 셀 위 행으로 이동
      newRow = curCell.row - 1;
    }
  } else {
    newRow += deltaRow;
    newCol += deltaCol;
  }

  // 범위 체크: 표 경계를 벗어나면 이동하지 않음
  if (newRow < 0 || newRow >= rowCount || newCol < 0 || newCol >= colCount) {
    return; // 표 끝 → 멈춤
  }

  this.cellAnchor = { row: newRow, col: newCol };
  this.cellFocus = { row: newRow, col: newCol };
  this.excludedCells.clear();

  // F5 단일 셀 선택의 화살표 이동은 하이라이트뿐 아니라 실제 편집 캐럿도 대상 셀 첫 위치로 옮긴다.
  // 그래야 셀 선택을 끝낸 직후의 입력·서식 명령이 표시된 셀에 적용된다.
  const targetCell = bboxes.find(b =>
    newRow >= b.row && newRow < b.row + b.rowSpan
      && newCol >= b.col && newCol < b.col + b.colSpan,
  );
  if (!targetCell) return;
  this.preferredX = null;
  this.atLineEnd = false;
  this.moveToCellByIndex(sec, ppi, ci, cellPath, targetCell.cellIdx, 'start');
  this.updateRect();
}

/** Ctrl+클릭: 해당 셀을 선택에서 제외/복원 토글. */
export function ctrlToggleCell(this: any, row: number, col: number): void {
  if (!this._cellSelectionMode) return;
  const key = excludedCellKey(row, col);
  if (this.excludedCells.has(key)) {
    this.excludedCells.delete(key);
  } else {
    this.excludedCells.add(key);
  }
}

/** 선택된 셀 범위를 반환한다 (정렬된 start/end). */
export function getSelectedCellRange(this: any): { startRow: number; startCol: number; endRow: number; endCol: number } | null {
  if (!this._cellSelectionMode || !this.cellAnchor || !this.cellFocus) return null;
  return {
    startRow: Math.min(this.cellAnchor.row, this.cellFocus.row),
    startCol: Math.min(this.cellAnchor.col, this.cellFocus.col),
    endRow: Math.max(this.cellAnchor.row, this.cellFocus.row),
    endCol: Math.max(this.cellAnchor.col, this.cellFocus.col),
  };
}

/** 제외된 셀 목록을 반환한다. */
export function getExcludedCells(this: any): Set<string> {
  return this.excludedCells;
}

/** 현재 셀 선택의 표 컨텍스트를 반환한다 (셀 bbox 조회용). */
export function getCellTableContext(this: any): { sec: number; ppi: number; ci: number; cellPath?: CellPathEntry[] } | null {
  if (this.cellTableCtx) return this.cellTableCtx;
  if (!this.isInCell()) return null;

  const { sectionIndex: sec, parentParaIndex: ppi, controlIndex: ci, cellPath } = this.position;
  if (ppi === undefined || ci === undefined) return null;
  return { sec, ppi, ci, cellPath };
}

// ─── 표 객체 선택 모드 ─────────────────────────────────

/** 현재 셀 위치의 표를 객체 선택한다. 셀 내부가 아니면 false.
 *
 *  [Task #919] 글상자 안 표 셀 (isInTextBox + cellPath.length >= 2) 도
 *  허용 — 가장 안쪽 표를 객체 선택한다. 한컴 UX 정합 (글상자 안 표 Esc).
 *  글상자 직접 셀 (cellPath.length === 1, 글상자 자체) 은 표가 아니므로 제외.
 */
export function enterTableObjectSelection(this: any): boolean {
  if (!this.isInCell()) return false;
  const { sectionIndex: sec, parentParaIndex: ppi, controlIndex: ci, cellPath } = this.position;
  if (ppi === undefined || ci === undefined) return false;
  // 글상자 안 본문 (cellPath.length === 1, 글상자 자체) → 표 객체 선택 대상 아님
  if (this.isInTextBox() && (cellPath?.length ?? 0) < 2) return false;
  this._tableObjectSelected = true;
  if (cellPath && cellPath.length > 1) {
    // 중첩 표: 내부 표를 선택 (cellPath 포함)
    this.selectedTableRef = { sec, ppi, ci, cellPath };
  } else {
    this.selectedTableRef = { sec, ppi, ci };
  }
  return true;
}

/** 선택된 표의 참조 정보를 반환한다. */
export function getSelectedTableRef(this: any): { sec: number; ppi: number; ci: number; cellPath?: CellPathEntry[] } | null {
  return this.selectedTableRef;
}

/** 표 객체 선택 상태에서 표 밖으로 커서를 이동한다. */
export function moveOutOfSelectedTable(this: any): void {
  if (!this.selectedTableRef) return;
  const { sec, ppi, cellPath } = this.selectedTableRef;

  if (cellPath && cellPath.length > 1) {
    // 중첩 표 객체 선택 → 외부 셀로 이동 (한 단계 위)
    // [Task #919] 글상자 안 표였으면 (가장 바깥이 글상자 = isTextBox) 유지.
    const wasInTextBox = this.position.isTextBox === true;
    const outerPath = cellPath.slice(0, -1);
    const lastOuter = outerPath[outerPath.length - 1];
    // outerPath.length === 1 이고 글상자였으면 isTextBox 유지 → 다음 Esc 시
    // 글상자 객체 선택으로 전이 가능
    const stillInTextBox = wasInTextBox && outerPath.length === 1;
    this.position = {
      sectionIndex: sec,
      paragraphIndex: lastOuter.cellParaIndex,
      charOffset: 0,
      parentParaIndex: ppi,
      controlIndex: outerPath[0].controlIndex,
      cellIndex: lastOuter.cellIndex,
      cellParaIndex: lastOuter.cellParaIndex,
      cellPath: outerPath,
      isTextBox: stillInTextBox ? true : undefined,
    };
  } else {
    // 단일 표 객체 선택 → 표 밖으로 이동
    const paraCount = this.wasm.getParagraphCount(sec);
    if (ppi + 1 < paraCount) {
      this.position = { sectionIndex: sec, paragraphIndex: ppi + 1, charOffset: 0 };
    } else if (ppi > 0) {
      const prevLen = this.wasm.getParagraphLength(sec, ppi - 1);
      this.position = { sectionIndex: sec, paragraphIndex: ppi - 1, charOffset: prevLen };
    }
  }
  this.exitTableObjectSelection();
  this.updateRect();
}

// ── 그림/글상자 객체 선택 모드 ─────────────────────────────────

/** 지정한 개체(그림/글상자/묶음)를 객체 선택한다.
 * [Task #825] `headerFooter` — 머리말/꼬리말 안 그림일 때 outer 위치 marker 보존. */
export function enterPictureObjectSelectionDirect(
  this: any,
  sec: number, ppi: number, ci: number,
  type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole' = 'image',
  cellIdx?: number, cellParaIdx?: number,
  headerFooter?: { kind: 'header' | 'footer'; outerParaIdx: number; outerControlIdx: number },
  outerTableControlIdx?: number,
  cellPath?: CellPathEntry[],
  noteRef?: any,
  missing?: boolean,
): void {
  this.exitTableObjectSelection();
  this._pictureObjectSelected = true;
  this.selectedPictureRef = { sec, ppi, ci, type, cellIdx, cellParaIdx, outerTableControlIdx, cellPath, noteRef, headerFooter, missing };
  this.selectedPictureRefs = [{ ...this.selectedPictureRef }];
}

/** Shift+클릭: 개체를 다중 선택에 추가/제거 (토글) */
export function togglePictureObjectSelection(
  this: any,
  refOrSec: PictureSelectionRef | number,
  ppi?: number,
  ci?: number,
  type?: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole',
): void {
  this.exitTableObjectSelection();
  this._pictureObjectSelected = true;
  const ref: PictureSelectionRef =
    typeof refOrSec === 'number'
      ? { sec: refOrSec, ppi: ppi!, ci: ci!, type: type! }
      : refOrSec;
  const idx = this.selectedPictureRefs.findIndex((r: any) =>
    r.sec === ref.sec &&
    r.ppi === ref.ppi &&
    r.ci === ref.ci &&
    JSON.stringify(r.cellPath ?? []) === JSON.stringify(ref.cellPath ?? []),
  );
  if (idx >= 0) {
    this.selectedPictureRefs.splice(idx, 1);
    if (this.selectedPictureRefs.length === 0) {
      this.exitPictureObjectSelection();
      return;
    }
  } else {
    this.selectedPictureRefs.push({ ...ref });
  }
  // 기본 ref는 마지막 선택된 개체
  const last = this.selectedPictureRefs[this.selectedPictureRefs.length - 1];
  this.selectedPictureRef = { ...last };
}

/** 개체 객체 선택 상태에서 개체 밖으로 커서를 이동한다. */
export function moveOutOfSelectedPicture(this: any): void {
  if (!this.selectedPictureRef) return;
  const { sec, ppi } = this.selectedPictureRef;
  const paraCount = this.wasm.getParagraphCount(sec);
  if (ppi + 1 < paraCount) {
    this.position = { sectionIndex: sec, paragraphIndex: ppi + 1, charOffset: 0 };
  } else if (ppi > 0) {
    const prevLen = this.wasm.getParagraphLength(sec, ppi - 1);
    this.position = { sectionIndex: sec, paragraphIndex: ppi - 1, charOffset: prevLen };
  }
  this.exitPictureObjectSelection();
  this.updateRect();
}
