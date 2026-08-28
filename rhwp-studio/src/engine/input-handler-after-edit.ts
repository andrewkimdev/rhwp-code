/** input-handler post-edit/deferred-pagination methods — extracted from InputHandler class */
/* eslint-disable @typescript-eslint/no-explicit-any */

import { isPageLocalTextEditCommand, type PageLocalTextEditOptions } from './input-edit-invalidation';
import type { DocumentPosition } from '@/core/types';
import type { RefreshPolicy, TextMutationEffects } from './command';

const DOCUMENT_PAGINATION_IDLE_FLUSH_DELAY_MS = 120;
// 최초 입력의 paint 기회를 확보하고 반복 입력이 이 추가 예약 지연을 연장하지 않게 한다.
const DOCUMENT_PAGINATION_INITIAL_START_DELAY_MS = 100;
// #3794: 250ms cadence의 obsolete work를 줄이되 최신 job 완료의 10% 회귀 상한 안에 둔다.
const DOCUMENT_PAGINATION_RESTART_COALESCE_DELAY_MS = 200;
// 첫 fragment 하나 뒤 다음 입력과 후속 step이 겹치지 않게 하는 짧은 settle gap.
const DOCUMENT_PAGINATION_POST_FIRST_STEP_DELAY_MS = 25;
/**
 * [#3412] idle 자동 flush 대상 문서 크기 상한.
 *
 * #3248 이 idle 병합을 도입하면서 이 게이트(#2010 의 30쪽 상한)를 함께 지워 모든 문서가
 * 120ms 정지마다 동기 전체 pagination 을 하게 됐다. 대형 문서에서는 그 flush 자체가
 * 결함이다 — 115쪽 문서 실측으로 메인 스레드를 839ms 막고, #2214 의 재개형 러너를
 * 취소해 페이지-로컬 리페인트 계약(flush 0)을 깬다. 큰 문서는 러너와 명시 boundary
 * flush(undo/redo/navigation/blur/저장·인쇄)로 마감한다.
 */
const DOCUMENT_PAGINATION_IDLE_FLUSH_PAGE_LIMIT = 30;

/** textarea에 포커스를 설정한다 (iOS 호환) */
export function focusTextarea(this: any): void {
  this.textarea.focus();
}

/** 편집 후 처리: 재렌더링 + 캐럿 갱신 */
export function afterEdit(this: any, flushDeferredPagination = true): void {
  this.pendingFocusedPagePatch = null;
  if (flushDeferredPagination) {
    this.flushDeferredPaginationIfNeeded('before-full-edit', false);
  } else if (this.deferredPaginationPending) {
    // 경계 pre-flush 후 추가된 stable raw 입력은 즉시 재-flush하지 않고
    // 기존 작은 문서 idle 정책으로만 마무리한다.
    this.scheduleDeferredPaginationFlush();
  }
  this.lastCellKey = null; // 편집 후 셀 bbox 캐시 무효화
  this.protectedCellHitCache = null;
  // 표 구조 편집(줄/칸 삽입·삭제, 셀 합치기·나누기)은 cachedCellBboxes 의 기하와 cellIdx
  // 번호를 모두 바꾸지만, cachedTableRef 는 {sec, ppi, ci} 라 표 "정체성"만 담아 신선도
  // 검사를 그대로 통과한다. 지우지 않으면 hover marker 가 옛 경계에 그려지고
  // resolveTableResizeHit → startResizeDrag 가 옛 번호의 cellIdx 로 엉뚱한 행을 리사이즈한다.
  // undo/redo 경로가 이미 같은 이유로 이 루틴을 부른다.
  this.clearTableResizeRuntimeCache();
  this.eventBus.emit('document-mutated', 'input-handler-edit');
  this.eventBus.emit('document-changed');
  this.updateCaret();
}

/** 셀 내부 단일 텍스트 편집 후 처리: 현재 페이지 canvas만 갱신한다. */
export function afterPageLocalEdit(this: any): void {
  const focusedPagePatch = this.pendingFocusedPagePatch;
  this.pendingFocusedPagePatch = null;
  if (this.flushDeferredPaginationForCellOverflow()) return;

  // 텍스트 입력은 셀 폭을 바꾸지 않으므로 눈금자 셀 bbox 캐시를 무효화하지 않는다.
  this.protectedCellHitCache = null;
  this.eventBus.emit('document-mutated', 'input-handler-edit');
  const pageIndex = this.cursor.getRect()?.pageIndex;
  if (typeof pageIndex === 'number' && Number.isInteger(pageIndex) && pageIndex >= 0) {
    this.eventBus.emit('document-page-invalidated', {
      pageIndex,
      reason: 'text-edit',
      ...(focusedPagePatch?.pageIndex === pageIndex ? { focusedPagePatch } : {}),
    });
  } else {
    this.eventBus.emit('document-changed');
  }
  if (this.deferredPaginationPending) {
    this.scheduleDeferredPaginationFlush();
  }
  this.updateCaret();
}

/** 셀 안 새 줄이 기존 가시 높이를 넘으면 즉시 전체 표 레이아웃을 다시 계산한다. */
export function flushDeferredPaginationForCellOverflow(this: any): boolean {
  if (!this.cursor.getRect()?.cellOverflowed) return false;

  this.cancelDeferredPaginationFlush();
  this.deferredPaginationRunner.cancel();
  try {
    this.wasm.flushDeferredPagination();
    this.deferredPaginationPending = false;
    this.cursor.invalidateFocusedCellCursorGeometry();
    this.lastCellKey = null;
    this.protectedCellHitCache = null;
    this.eventBus.emit('document-mutated', 'input-handler-cell-overflow');
    this.eventBus.emit('document-changed', 'cell-overflow-pagination');
    this.cursor.moveTo(this.cursor.getPosition());
    this.updateCaret();
    return true;
  } catch (err) {
    console.warn('[InputHandler] 셀 overflow 페이지네이션 flush 실패:', err);
    return false;
  }
}

export function scheduleDeferredPaginationFlush(this: any): void {
  this.cancelDeferredPaginationFlush();
  this.deferredPaginationPending = true;
  if (!this.shouldAutoFlushDeferredPagination()) return;
  this.deferredPaginationFlushTimer = setTimeout(() => {
    this.flushDeferredPaginationIfNeeded('idle-auto');
  }, DOCUMENT_PAGINATION_IDLE_FLUSH_DELAY_MS);
}

/**
 * [#3412] idle 자동 flush 대상 여부.
 *
 * 전진 중인 재개형 잡이 있으면 idle flush 는 그 잡을 취소하고 같은 일을 동기로 다시
 * 하는 셈이라 예약하지 않는다. 문서 크기 상한은 위 상수 주석 참조.
 */
export function shouldAutoFlushDeferredPagination(this: any): boolean {
  if (this.deferredPaginationRunner.hasPendingWork()) return false;
  return this.wasm.pageCount <= DOCUMENT_PAGINATION_IDLE_FLUSH_PAGE_LIMIT;
}

export function cancelDeferredPaginationFlush(this: any): void {
  if (this.deferredPaginationFlushTimer) {
    clearTimeout(this.deferredPaginationFlushTimer);
    this.deferredPaginationFlushTimer = null;
  }
}

/** deferred mutation을 cursor lookup 전에 등록하고 flow 경계에서는 resumable job을 시작한다. */
export function prepareTextMutationBeforeCursor(this: any, effects: TextMutationEffects): boolean {
  this.pendingFocusedPagePatch = effects.focusedPagePatch
    ? { ...effects.focusedPagePatch }
    : null;
  const hasTextMutation = effects.documentPaginationPending
    || effects.flowChanged
    || effects.paginationCompleted;
  if (effects.focusedCursorGeometry) {
    this.cursor.prepareFocusedCellCursorGeometry(effects.focusedCursorGeometry);
  } else if (hasTextMutation) {
    this.cursor.invalidateFocusedCellCursorGeometry();
  }

  if (effects.paginationCompleted) {
    this.cancelDeferredPaginationFlush();
    this.deferredPaginationRunner.cancel();
    this.deferredPaginationPending = false;
  }
  if (effects.flowChanged && effects.paginationCompleted) return true;
  if (!effects.documentPaginationPending) return false;

  const replacesPendingJob = this.deferredPaginationRunner.hasPendingWork();
  this.cancelDeferredPaginationFlush();
  this.deferredPaginationPending = true;
  if (!effects.flowChanged && !replacesPendingJob) return false;

  // 최초 admission은 고정 timer target을 유지하고, active restart만 마지막 입력까지 합친다.
  this.deferredPaginationRunner.requestStart(
    DOCUMENT_PAGINATION_RESTART_COALESCE_DELAY_MS,
    DOCUMENT_PAGINATION_INITIAL_START_DELAY_MS,
    DOCUMENT_PAGINATION_POST_FIRST_STEP_DELAY_MS,
  );
  return true;
}

export function completeResumablePagination(this: any, _pageCount: number): void {
  this.cancelDeferredPaginationFlush();
  this.deferredPaginationPending = false;
  this.lastCellKey = null;
  this.protectedCellHitCache = null;
  this.eventBus.emit('document-mutated', 'input-handler-resumable-pagination');
  this.eventBus.emit('document-changed', 'deferred-pagination-complete');
  const position = this.cursor.getPosition();
  this.cursor.invalidateFocusedCellCursorGeometry();
  this.cursor.moveTo(position);
  this.updateCaret();
}

export function fallbackFromResumablePagination(this: any): void {
  // 구버전 WASM 또는 fast-path 비대상 문서는 기존 동기 barrier 의미론을 유지한다.
  this.flushDeferredPaginationIfNeeded('resumable-fallback');
}

export function resetRawTextMutationEffects(this: any): void {
  this.rawTextMutationEffects.clear();
}

export function consumeRawTextMutationBeforeCursor(this: any): boolean {
  return this.prepareTextMutationBeforeCursor(this.rawTextMutationEffects.consume());
}

export function hasDeferredPaginationPending(this: any): boolean {
  return this.deferredPaginationPending;
}

export function flushDeferredPaginationIfNeeded(
  this: any,
  reason = 'manual',
  emitChange = true,
): boolean {
  const shouldFlush = this.deferredPaginationPending
    || this.deferredPaginationFlushTimer !== null
    || this.deferredPaginationRunner.hasPendingWork();
  this.cancelDeferredPaginationFlush();
  if (!shouldFlush) return false;

  try {
    this.deferredPaginationRunner.cancel();
    this.wasm.flushDeferredPagination();
    this.deferredPaginationPending = false;
    this.cursor.invalidateFocusedCellCursorGeometry();
    if (emitChange) {
      this.eventBus.emit('document-changed', `deferred-pagination-flush:${reason}`);
    }
    return true;
  } catch (err) {
    this.deferredPaginationPending = true;
    console.warn('[InputHandler] 지연 페이지네이션 flush 실패:', err);
    return false;
  }
}

/**
 * [#4031] 동기 full pagination을 소유하는 structural command(셀 Enter 분할)가 확정된
 * 경로에서, 곧 폐기될 stale deferred job을 계산 완료 없이 취소한다.
 * `wasm.flushDeferredPagination()`을 호출하지 않는 것이 flush 경로와의 유일한 차이다.
 * runner.cancel()이 전진 중인 WASM resumable job까지 취소한다.
 * `deferredPaginationPending`은 유지한다 — mutation이 실패하면 다음 boundary flush가
 * 기존 barrier 의미론으로 복구하도록 fail-closed로 남긴다.
 */
export function cancelDeferredPaginationForOwnedMutation(this: any): void {
  this.cancelDeferredPaginationFlush();
  this.deferredPaginationRunner.cancel();
}

/** raw IME/iOS 텍스트 입력처럼 command를 거치지 않는 경로의 갱신 라우터. */
export function afterTextInputEdit(
  this: any,
  beforePos: DocumentPosition,
  afterPos: DocumentPosition,
  pageLocalOptions: PageLocalTextEditOptions = {},
  boundaryHandled = false,
): void {
  if (boundaryHandled) {
    this.afterEdit(false);
    return;
  }
  if (this.shouldUsePageLocalRefresh('insertText', beforePos, afterPos, pageLocalOptions)) {
    this.afterPageLocalEdit();
  } else {
    this.afterEdit();
  }
}

export function refreshAfterOperation(
  this: any,
  requested: RefreshPolicy | undefined,
  fallback: RefreshPolicy,
  commandType: string,
  beforePos: DocumentPosition,
  afterPos: DocumentPosition,
  pageLocalOptions: PageLocalTextEditOptions = {},
  boundaryHandled = false,
): void {
  if (boundaryHandled) {
    this.afterEdit(false);
    return;
  }
  const policy = requested ?? fallback;
  switch (policy) {
    case 'none':
      return;
    case 'selectionOnly':
      this.updateCaret();
      return;
    case 'pageLocal':
      this.afterPageLocalEdit();
      return;
    case 'full':
      this.afterEdit();
      return;
    case 'auto':
    default:
      if (this.shouldUsePageLocalRefresh(commandType, beforePos, afterPos, pageLocalOptions)) {
        this.afterPageLocalEdit();
      } else {
        this.afterEdit();
      }
  }
}

export function shouldUsePageLocalRefresh(
  this: any,
  commandType: string,
  beforePos: DocumentPosition,
  afterPos: DocumentPosition,
  pageLocalOptions: PageLocalTextEditOptions = {},
): boolean {
  if (this.cursor.isInHeaderFooter() || this.cursor.isInFootnote()) return false;
  // page-local redraw는 pagination을 지연한 stable mutation에서만 안전하다.
  // immediate pagination은 후속 페이지 cut을 바꿀 수 있으므로 full 표시 무효화로 보낸다.
  if (!this.deferredPaginationPending) return false;
  return isPageLocalTextEditCommand(commandType, beforePos, afterPos, pageLocalOptions);
}
