/** input-handler field(누름틀)/click-here navigation methods — extracted from InputHandler class */
/* eslint-disable @typescript-eslint/no-explicit-any */

import { showConfirm } from '@/ui/confirm-dialog';
import type { DocumentPosition, CursorRect } from '@/core/types';
import type { WasmBridge } from '@/core/wasm-bridge';

/** 커서를 지정 위치로 이동하고 캐럿을 표시한다. 성공하면 true 반환. */
export function moveCursorTo(this: any, pos: DocumentPosition): boolean {
  // 이동 전 위치가 유효한지 사전 검증 (경고 로그 방지)
  try {
    const testRect = this.wasm.getCursorRect(pos.sectionIndex, pos.paragraphIndex, pos.charOffset);
    if (!testRect || testRect.pageIndex === undefined) return false;
  } catch {
    return false;
  }

  this.cursor.clearSelection();
  this.cursor.moveTo(pos);
  this.cursor.resetPreferredX();
  this.active = true;
  const rect = this.cursor.getRect();
  if (rect) {
    this.caret.show(rect, this.viewportManager.getZoom());
    this.updateCaret();
    this.focusTextarea();
    return true;
  }
  this.focusTextarea();
  return false;
}

/** 현재 커서 위치의 누름틀 필드와 내용을 제거한다. */
export function removeCurrentField(this: any, posOverride?: DocumentPosition): void {
  const pos = posOverride ?? this.cursor.getPosition();
  let restorePos: DocumentPosition | null = null;
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    if (fi.inField && fi.fieldType === 'clickhere') {
      restorePos = {
        ...pos,
        charOffset: fi.startCharIdx ?? pos.charOffset,
      };
    }
  } catch {
    restorePos = null;
  }

  try {
    // [Task #2377] 누름틀 제거는 필드+안내문 텍스트를 지운다(문자 수 변경) — 일반
    // 모드에선 snapshot 으로 기록해 undo 가능하게 한다. 아래 양식 모드 분기는 방어적이다:
    // field:remove는 canExecute에서, 키보드 경계 삭제는 tryConfirmRemove…에서 양식 모드를
    // 이미 막으므로 현재 도달 경로가 없다. 미래의 직접 호출이 생겨도 snapshot 게이트의
    // 무언 폐기를 피하려 기존 직접 경로를 보존한다(기록 역연산 설계는 명시적 범위 외).
    if (this.editMode === 'form') {
      const result = this.wasm.removeFieldAt(pos);
      if (!result.ok) return;
      if (restorePos) {
        this.cursor.clearSelection();
        this.cursor.moveTo(restorePos);
        this.cursor.resetPreferredX();
      }
      this.afterEdit();
    } else {
      this.cursor.clearSelection();
      this.executeOperation({
        kind: 'snapshot',
        operationType: 'removeField',
        operation: (wasm: WasmBridge) => {
          const result = wasm.removeFieldAt(pos);
          if (!result.ok) throw new Error('removeFieldAt not ok');
          return restorePos ?? pos;
        },
      });
      // 커서 이동·refresh 는 라우터가 수행.
    }
    this.fieldMarker.hide();
    this.fieldStartExitKey = null;
    this.fieldEndExitKey = null;
    this.wasm.clearActiveField();
    this.eventBus.emit('field-info-changed', null);
  } catch (err) {
    console.warn('[InputHandler] 누름틀 제거 실패:', err);
  }
}

/** 현재 커서 위치의 누름틀 제거를 한컴처럼 확인 후 수행한다. */
export function confirmRemoveCurrentField(this: any): boolean {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    if (!fi.inField || fi.fieldType !== 'clickhere') return false;
  } catch {
    return false;
  }

  void showConfirm('지우기', '[누름틀]을 지울까요?')
    .then((ok) => {
      if (ok) this.removeCurrentField(pos);
      this.focusTextarea();
    })
    .catch(() => {
      this.focusTextarea();
    });
  return true;
}

/** 누름틀 끝에서 오른쪽 이동 시 같은 charOffset을 필드 밖 위치로 취급한다. */
export function tryExitCurrentFieldEnd(this: any): boolean {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    const start = fi.startCharIdx ?? -1;
    const end = fi.endCharIdx ?? -1;
    if (!fi.inField || fi.fieldType !== 'clickhere' || start < 0 || end < 0) return false;
    if (this.isAtExitedFieldEnd(pos, fi)) return false;
    if (pos.charOffset < end) return false;
    this.fieldStartExitKey = null;
    this.fieldEndExitKey = this.fieldBoundaryKey(pos, fi.fieldId, end);
    this.fieldMarker.hide();
    this.wasm.clearActiveField();
    this.eventBus.emit('field-info-changed', null);
    this.eventBus.emit('document-changed');
    this.updateCaret(true);
    requestAnimationFrame(() => this.updateCaret(true));
    return true;
  } catch {
    return false;
  }
}

/** 누름틀 시작에서 왼쪽 이동 시 같은 charOffset을 필드 밖 위치로 취급한다. */
export function tryExitCurrentFieldStart(this: any): boolean {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    const start = fi.startCharIdx ?? -1;
    const end = fi.endCharIdx ?? -1;
    if (!fi.inField || fi.fieldType !== 'clickhere' || start < 0 || end < 0) return false;
    if (this.isAtExitedFieldStart(pos, fi)) return false;
    if (start === end || pos.charOffset > start) return false;
    this.fieldEndExitKey = null;
    this.fieldStartExitKey = this.fieldBoundaryKey(pos, fi.fieldId, start);
    this.fieldMarker.hide();
    this.wasm.clearActiveField();
    this.eventBus.emit('field-info-changed', null);
    this.eventBus.emit('document-changed');
    return true;
  } catch {
    return false;
  }
}

/** 누름틀 시작 밖 위치에서 오른쪽 이동하면 같은 charOffset의 필드 내부 시작으로 들어간다. */
export function tryEnterExitedFieldStart(this: any): boolean {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    if (!fi.inField || fi.fieldType !== 'clickhere' || !this.isAtExitedFieldStart(pos, fi)) {
      return false;
    }
    this.fieldStartExitKey = null;
    this.updateFieldMarkers();
    return true;
  } catch {
    return false;
  }
}

/** 누름틀 끝 밖 위치에서 왼쪽 이동하면 같은 charOffset의 필드 내부 끝으로 들어간다. */
export function tryEnterExitedFieldEnd(this: any): boolean {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    if (!fi.inField || fi.fieldType !== 'clickhere' || !this.isAtExitedFieldEnd(pos, fi)) {
      return false;
    }
    this.fieldEndExitKey = null;
    this.updateFieldMarkers();
    return true;
  } catch {
    return false;
  }
}

/** Home 이동 결과가 누름틀 시작이면 한컴처럼 누름틀 이전 위치로 취급한다. */
export function markCurrentFieldStartOutside(this: any): boolean {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    const start = fi.startCharIdx ?? -1;
    const end = fi.endCharIdx ?? -1;
    if (!fi.inField || fi.fieldType !== 'clickhere' || start < 0 || end < 0) return false;
    if (start === end || pos.charOffset !== start) return false;
    this.fieldEndExitKey = null;
    this.fieldStartExitKey = this.fieldBoundaryKey(pos, fi.fieldId, start);
    this.fieldMarker.hide();
    this.wasm.clearActiveField();
    this.eventBus.emit('field-info-changed', null);
    this.eventBus.emit('document-changed');
    this.updateCaret(true);
    requestAnimationFrame(() => this.updateCaret(true));
    return true;
  } catch {
    return false;
  }
}

/** End 이동 결과가 누름틀 끝이면 한컴처럼 누름틀 이후 위치로 취급한다. */
export function markCurrentFieldEndOutside(this: any): boolean {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    const start = fi.startCharIdx ?? -1;
    const end = fi.endCharIdx ?? -1;
    if (!fi.inField || fi.fieldType !== 'clickhere' || start < 0 || end < 0) return false;
    if (pos.charOffset !== end) return false;
    this.fieldStartExitKey = null;
    this.fieldEndExitKey = this.fieldBoundaryKey(pos, fi.fieldId, end);
    this.fieldMarker.hide();
    this.wasm.clearActiveField();
    this.eventBus.emit('field-info-changed', null);
    this.eventBus.emit('document-changed');
    this.updateCaret(true);
    requestAnimationFrame(() => this.updateCaret(true));
    return true;
  } catch {
    return false;
  }
}

export function isAtExitedFieldStart(this: any, pos: DocumentPosition, fi?: { fieldId?: number; startCharIdx?: number }): boolean {
  const start = fi?.startCharIdx ?? pos.charOffset;
  return this.fieldStartExitKey === this.fieldBoundaryKey(pos, fi?.fieldId, start);
}

export function isExitedFieldStartPosition(this: any, pos: DocumentPosition): boolean {
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    return fi.inField
      && fi.fieldType === 'clickhere'
      && this.isAtExitedFieldStart(pos, fi);
  } catch {
    return false;
  }
}

export function isAtExitedFieldEnd(this: any, pos: DocumentPosition, fi?: { fieldId?: number; endCharIdx?: number }): boolean {
  const end = fi?.endCharIdx ?? pos.charOffset;
  return this.fieldEndExitKey === this.fieldBoundaryKey(pos, fi?.fieldId, end);
}

/** 빈 누름틀 안내문 클릭 후 첫 입력 위치를 실제 field start로 정규화한다. */
export function prepareClickHereInputPosition(this: any): DocumentPosition {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    const start = fi.startCharIdx ?? -1;
    if (!fi.inField || fi.fieldType !== 'clickhere' || !fi.isGuide || start < 0) {
      return pos;
    }

    const normalized = { ...pos, charOffset: start };
    this.fieldStartExitKey = null;
    this.fieldEndExitKey = null;
    this.cursor.clearSelection();
    if (pos.charOffset !== start) {
      this.cursor.moveTo(normalized);
    }
    this.wasm.setActiveField(normalized);
    return normalized;
  } catch {
    return pos;
  }
}

/** 마우스로 누름틀 위치를 직접 클릭하면 키보드 경계 이탈 상태를 해제한다. */
export function prepareClickHerePointerEntry(this: any, pageX?: number): void {
  const pos = this.cursor.getPosition();
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    const guidePos = this.findEmptyClickHereGuideHitPosition(pos);
    if (guidePos) {
      this.fieldStartExitKey = null;
      this.fieldEndExitKey = null;
      this.cursor.moveTo(guidePos);
      const fieldChanged = this.wasm.setActiveField(guidePos);
      if (fieldChanged) this.eventBus.emit('document-changed');
      return;
    }

    if (!fi.inField || fi.fieldType !== 'clickhere') {
      return;
    }

    if (typeof pageX === 'number' && this.prepareClickHerePointerBoundaryExit(pos, fi, pageX)) {
      return;
    }

    this.fieldStartExitKey = null;
    this.fieldEndExitKey = null;

    if (!fi.isGuide || fi.startCharIdx === undefined) return;

    const normalized = { ...pos, charOffset: fi.startCharIdx };
    if (pos.charOffset !== fi.startCharIdx) {
      this.cursor.moveTo(normalized);
    }
    const fieldChanged = this.wasm.setActiveField(normalized);
    if (fieldChanged) this.eventBus.emit('document-changed');
  } catch {
    // 클릭 hit-test 직후 필드 조회 실패는 일반 클릭 처리로 흘려보낸다.
  }
}

export function prepareClickHerePointerBoundaryExit(this: any, pos: DocumentPosition, fi: any, pageX: number): boolean {
  const start = fi.startCharIdx ?? -1;
  const end = fi.endCharIdx ?? -1;
  if (start < 0 || end < 0 || start === end) return false;

  const rects = this.getClickHereBoundaryRects(pos, start, end);
  if (!rects) return false;

  const tolerance = 1;
  if (pos.charOffset <= start && pageX < rects.startRect.x - tolerance) {
    this.fieldEndExitKey = null;
    this.fieldStartExitKey = this.fieldBoundaryKey(pos, fi.fieldId, start);
    this.fieldMarker.hide();
    this.wasm.clearActiveField();
    this.eventBus.emit('field-info-changed', null);
    return true;
  }

  if (pos.charOffset >= end && pageX > rects.endRect.x + tolerance) {
    this.fieldStartExitKey = null;
    this.fieldEndExitKey = this.fieldBoundaryKey(pos, fi.fieldId, end);
    this.fieldMarker.hide();
    this.wasm.clearActiveField();
    this.eventBus.emit('field-info-changed', null);
    return true;
  }

  return false;
}

export function findEmptyClickHereGuideHitPosition(this: any, pos: DocumentPosition): DocumentPosition | null {
  try {
    const fields = this.wasm.getFieldList()
      .filter((field: any) =>
        field.fieldType === 'clickhere'
        && typeof field.startCharIdx === 'number'
        && field.startCharIdx === field.endCharIdx)
      .map((field: any) => {
        const fieldPos = this.formFieldPosition(field);
        if (!fieldPos || !this.isSameTextContainer(pos, fieldPos)) return null;
        const guideLen = Array.from(field.guide ?? '').length;
        if (guideLen <= 0) return null;
        const start = field.startCharIdx;
        const guideEnd = start + guideLen;
        if (pos.charOffset < start || pos.charOffset > guideEnd) return null;
        return fieldPos;
      })
      .filter((fieldPos: DocumentPosition | null): fieldPos is DocumentPosition => fieldPos !== null)
      .sort((a: DocumentPosition, b: DocumentPosition) => b.charOffset - a.charOffset);
    return fields[0] ?? null;
  } catch {
    return null;
  }
}

/** 현재 위치가 빈 누름틀 안내문 영역인지 확인한다. */
export function isClickHereGuidePosition(this: any, pos: DocumentPosition): boolean {
  try {
    const fi = this.wasm.getFieldInfoAt(pos);
    return fi.inField && fi.fieldType === 'clickhere' && fi.isGuide === true;
  } catch {
    return false;
  }
}

/** 빈 누름틀 첫 입력 직후 안내문/마커 캐시를 새 field value 기준으로 다시 잡는다. */
export function refreshClickHereAfterFirstInput(this: any): void {
  this.lastCellKey = null;
  this.fieldStartExitKey = null;
  this.fieldEndExitKey = null;
  this.fieldMarker.hide();
  this.wasm.clearActiveField();
  this.eventBus.emit('document-changed');
  requestAnimationFrame(() => {
    this.updateCaret();
    this.eventBus.emit('document-changed');
  });
}

export function fieldBoundaryKey(this: any, pos: DocumentPosition, fieldId: number | undefined, charOffset: number): string {
  const path = JSON.stringify(pos.cellPath ?? []);
  return [
    pos.sectionIndex,
    pos.parentParaIndex ?? -1,
    pos.paragraphIndex,
    pos.controlIndex ?? -1,
    pos.cellIndex ?? -1,
    pos.cellParaIndex ?? -1,
    pos.isTextBox ? 1 : 0,
    path,
    fieldId ?? -1,
    charOffset,
  ].join(':');
}

export function getClickHereBoundaryRects(this: any, pos: DocumentPosition, start: number, end: number): { startRect: CursorRect; endRect: CursorRect } | null {
  try {
    if ((pos.cellPath?.length ?? 0) > 1 && pos.parentParaIndex !== undefined) {
      const pathJson = JSON.stringify(pos.cellPath);
      return {
        startRect: this.wasm.getCursorRectByPath(
          pos.sectionIndex, pos.parentParaIndex, pathJson, start,
        ),
        endRect: this.wasm.getCursorRectByPath(
          pos.sectionIndex, pos.parentParaIndex, pathJson, end,
        ),
      };
    }

    if (pos.parentParaIndex !== undefined) {
      return {
        startRect: this.wasm.getCursorRectInCell(
          pos.sectionIndex, pos.parentParaIndex, pos.controlIndex!,
          pos.cellIndex!, pos.cellParaIndex!, start,
        ),
        endRect: this.wasm.getCursorRectInCell(
          pos.sectionIndex, pos.parentParaIndex, pos.controlIndex!,
          pos.cellIndex!, pos.cellParaIndex!, end,
        ),
      };
    }

    return {
      startRect: this.wasm.getCursorRect(pos.sectionIndex, pos.paragraphIndex, start),
      endRect: this.wasm.getCursorRect(pos.sectionIndex, pos.paragraphIndex, end),
    };
  } catch {
    return null;
  }
}

/**
 * 활성 필드를 무조건 해제하고 마커를 숨긴다 (안내문 다시 표시).
 *
 * 텍스트 편집 자체를 완전히 벗어나는 지점(blur, 표/글상자/보호 셀 객체 선택
 * 진입 등)에서 쓴다. `updateFieldMarkers` 처럼 커서 위치를 다시 조회해 같은
 * 필드인지 판정하지 않고 무조건 해제하므로, 커서 위치가 갱신되지 않는(또는
 * 의미 없는) 지점에서도 안전하다.
 *
 * [주의] `this.fieldMarker.isVisible` 는 실제 활성 필드 존재 여부의 신뢰할
 * 수 있는 대리 지표가 아니다 — `updateFieldMarkers` 의 선택 분기처럼 마커만
 * 숨기고 wasm 쪽 active_field 해제를 재렌더 없이 수행하는 경로가 있으면 두
 * 상태가 어긋난다. 그래서 `wasm.clearActiveField()` 자체의 반환값(실제로
 * 해제된 필드가 있었는지)으로 재렌더 필요 여부를 판정한다.
 */
export function clearActiveFieldMarker(this: any): void {
  this.fieldStartExitKey = null;
  this.fieldEndExitKey = null;
  if (this.fieldMarker.isVisible) this.fieldMarker.hide();
  const hadActiveField = this.wasm.clearActiveField();
  if (hadActiveField) this.eventBus.emit('document-changed');
  this.eventBus.emit('field-info-changed', null);
}

/** 커서 위치의 필드 상태에 따라 낫표 마커를 표시/숨김한다 */
export function updateFieldMarkers(this: any): void {
  const wasVisible = this.fieldMarker.isVisible;
  if (this.cursor.hasSelection()) {
    clearActiveFieldMarker.call(this);
    return;
  }
  try {
    const pos = this.cursor.getPosition();
    const fi = this.wasm.getFieldInfoAt(pos);
    if (fi.inField && fi.startCharIdx !== undefined && fi.endCharIdx !== undefined) {
      if (this.isAtExitedFieldStart(pos, fi) || this.isAtExitedFieldEnd(pos, fi)) {
        clearActiveFieldMarker.call(this);
        return;
      }
      this.fieldStartExitKey = null;
      this.fieldEndExitKey = null;
      // 활성 필드 설정 → 안내문 숨김 + 페이지 캐시 무효화
      const fieldChanged = this.wasm.setActiveField(pos);
      const zoom = this.viewportManager.getZoom();
      const rects = this.getClickHereBoundaryRects(pos, fi.startCharIdx, fi.endCharIdx);
      if (!rects) return;
      const { startRect, endRect } = rects;
      this.fieldMarker.show(startRect, endRect, zoom);
      // 필드 진입 또는 다른 필드로 전환 시 재렌더링 (안내문 표시/숨김 반영)
      if (!wasVisible || fieldChanged) {
        this.eventBus.emit('document-changed');
        // 재렌더링 후 캐럿 위치 재계산 (가이드 텍스트 제거로 좌표 변경됨)
        this.cursor.updateRect();
        this.updateCaret();
      }
      // 상태 표시줄에 필드 정보 표시
      this.eventBus.emit('field-info-changed', {
        fieldId: fi.fieldId, fieldType: fi.fieldType, guideName: fi.guideName,
      });
      return;
    }
  } catch (err) { console.warn('[updateFieldMarkers] 필드 마커 갱신 실패:', err); }
  // 필드 밖이면 마커 숨김 + 활성 필드 해제
  clearActiveFieldMarker.call(this);
}
