/** input-handler text-selection drag + autoscroll methods — extracted from InputHandler class */
/* eslint-disable @typescript-eslint/no-explicit-any */

const DRAG_SCROLL_EDGE_PX = 48;
const DRAG_SCROLL_MIN_STEP_PX = 2;
const DRAG_SCROLL_MAX_STEP_PX = 20;

/** 텍스트 선택 드래그를 시작한다 */
export function startTextSelectionDrag(this: any, e: MouseEvent): void {
  this.isDragging = true;
  this.dragLastClientX = e.clientX;
  this.dragLastClientY = e.clientY;
  document.addEventListener('mousemove', this.onMouseMoveBound);
}

/** 텍스트 선택 드래그 포인터 좌표를 갱신한다 */
export function updateTextSelectionDragPointer(this: any, e: MouseEvent): void {
  this.dragLastClientX = e.clientX;
  this.dragLastClientY = e.clientY;
  this.updateTextSelectionDragAutoScroll();
}

/** 마지막 포인터 좌표 기준으로 드래그 선택 focus를 갱신한다 */
export function updateTextSelectionDragFromPointer(this: any): void {
  if (!this.isDragging) return;

  if (this.cursor.isInFootnote()) {
    const fnHit = this.footnoteHitTestFromClientPoint(this.dragLastClientX, this.dragLastClientY);
    if (
      fnHit?.hit.hit &&
      fnHit.hit.footnoteIndex === this.cursor.fnFootnoteIndex &&
      fnHit.hit.fnParaIndex !== undefined &&
      fnHit.hit.charOffset !== undefined
    ) {
      this.cursor.setFnCursorPosition(fnHit.hit.fnParaIndex, fnHit.hit.charOffset);
      this.updateCaretDuringDrag();
    }
    return;
  }

  const hit = this.hitTestFromClientPoint(this.dragLastClientX, this.dragLastClientY);
  if (hit && hit.paragraphIndex < 0xFFFFFF00) {
    // [Issue #669] 셀 내부 드래그: anchor와 같은 셀 컨텍스트인 경우만 커서 이동.
    // 셀↔본문 혼합은 선택 렌더링 불가이므로 무시 (셀 내 선택 유지).
    const sel = this.cursor.getSelection();
    if (sel) {
      const anchorInCell = sel.anchor.parentParaIndex !== undefined;
      const hitInSameCell = anchorInCell &&
        hit.parentParaIndex === sel.anchor.parentParaIndex &&
        hit.controlIndex === sel.anchor.controlIndex &&
        hit.cellIndex === sel.anchor.cellIndex;
      if (anchorInCell && !hitInSameCell) {
        return;
      }
    }
    this.cursor.moveToHit(hit);
    this.updateCaretDuringDrag();
  }
}

/** 텍스트 선택 드래그를 종료한다 */
export function stopTextSelectionDrag(this: any): void {
  this.isDragging = false;
  this.cellSelectionDragCandidate = null;
  document.removeEventListener('mousemove', this.onMouseMoveBound);
  this.stopTextSelectionDragAutoScroll();
}

export function getTextSelectionDragScrollDeltaY(this: any): number {
  const rect = this.container.getBoundingClientRect();
  const topEdge = rect.top + DRAG_SCROLL_EDGE_PX;
  const bottomEdge = rect.top + this.container.clientHeight - DRAG_SCROLL_EDGE_PX;
  const clientY = this.dragLastClientY;

  if (clientY < topEdge) {
    return -this.scaleTextSelectionDragScrollStep(topEdge - clientY);
  }
  if (clientY > bottomEdge) {
    return this.scaleTextSelectionDragScrollStep(clientY - bottomEdge);
  }
  return 0;
}

export function scaleTextSelectionDragScrollStep(this: any, distance: number): number {
  const ratio = Math.min(1, Math.max(0, distance / DRAG_SCROLL_EDGE_PX));
  return Math.round(DRAG_SCROLL_MIN_STEP_PX + (DRAG_SCROLL_MAX_STEP_PX - DRAG_SCROLL_MIN_STEP_PX) * ratio);
}

export function updateTextSelectionDragAutoScroll(this: any): void {
  if (!this.isDragging) {
    this.stopTextSelectionDragAutoScroll();
    return;
  }
  if (this.getTextSelectionDragScrollDeltaY() === 0) {
    this.stopTextSelectionDragAutoScroll();
    return;
  }
  if (!this.dragAutoScrollRafId) {
    this.dragAutoScrollRafId = requestAnimationFrame(() => this.runTextSelectionDragAutoScroll());
  }
}

export function runTextSelectionDragAutoScroll(this: any): void {
  this.dragAutoScrollRafId = 0;
  if (!this.isDragging) return;

  const deltaY = this.getTextSelectionDragScrollDeltaY();
  if (deltaY === 0) return;

  const before = this.container.scrollTop;
  const maxScrollTop = Math.max(0, this.container.scrollHeight - this.container.clientHeight);
  this.container.scrollTop = Math.max(0, Math.min(maxScrollTop, before + deltaY));

  if (this.container.scrollTop === before) return;

  this.updateTextSelectionDragFromPointer();
  this.dragAutoScrollRafId = requestAnimationFrame(() => this.runTextSelectionDragAutoScroll());
}

export function stopTextSelectionDragAutoScroll(this: any): void {
  if (this.dragAutoScrollRafId) {
    cancelAnimationFrame(this.dragAutoScrollRafId);
    this.dragAutoScrollRafId = 0;
  }
}
