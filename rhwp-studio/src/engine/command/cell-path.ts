import type {
  DeferredCellTextMutationResult,
  DeferredFocusedPagePatch,
} from '@/core/wasm-bridge';
import type { DocumentPosition, CellPathEntry } from '@/core/types';
import { MAX_PAGE_LOCAL_TEXT_EDIT_CHARS } from '../input-edit-invalidation';
import type { FocusedCellCursorGeometry } from './types';

// ─── 본문/셀 분기 헬퍼 ────────────────────────────────

export function isCell(pos: DocumentPosition): boolean {
  return pos.parentParaIndex !== undefined;
}

/** 중첩 표(depth > 1)인지 확인 */
export function isNestedCell(pos: DocumentPosition): boolean {
  return (pos.cellPath?.length ?? 0) > 1;
}

export function canUseDeferredCellTextInsert(pos: DocumentPosition, text: string): boolean {
  if (!isCell(pos) || isNestedCell(pos)) return false;
  if (text.length === 0 || text.length > MAX_PAGE_LOCAL_TEXT_EDIT_CHARS) return false;
  if (/[\r\n\t]/.test(text)) return false;
  return true;
}

export function canUseDeferredCellTextDelete(pos: DocumentPosition, count: number): boolean {
  if (!isCell(pos) || isNestedCell(pos)) return false;
  return Number.isInteger(count) && count > 0 && count <= MAX_PAGE_LOCAL_TEXT_EDIT_CHARS;
}

export function canUseDeferredCellTextReplace(
  pos: DocumentPosition,
  deleteCount: number,
  text: string,
): boolean {
  if (!isCell(pos) || isNestedCell(pos)) return false;
  if (
    !Number.isInteger(deleteCount) ||
    deleteCount < 1 ||
    deleteCount > MAX_PAGE_LOCAL_TEXT_EDIT_CHARS
  ) {
    return false;
  }
  const textChars = charCount(text);
  if (textChars < 1 || textChars > MAX_PAGE_LOCAL_TEXT_EDIT_CHARS) return false;
  if (/[\r\n\t]/.test(text)) return false;
  return true;
}

export function canUseLocalBodyTextReplace(
  pos: DocumentPosition,
  deleteCount: number,
  text: string,
): boolean {
  if (isCell(pos)) return false;
  if (!Number.isInteger(deleteCount) || deleteCount < 0 || deleteCount > MAX_PAGE_LOCAL_TEXT_EDIT_CHARS) {
    return false;
  }
  if (deleteCount === 0 && text.length === 0) return false;
  if (charCount(text) > MAX_PAGE_LOCAL_TEXT_EDIT_CHARS) return false;
  if (/[\r\n\t]/.test(text)) return false;
  return true;
}

/** cellPath를 WASM용 JSON 문자열로 변환 */
export function cellPathJson(pos: DocumentPosition): string {
  return JSON.stringify(pos.cellPath ?? []);
}

/** cellPath 의 최내곽(마지막) 엔트리의 cellParaIndex 를 지정 값으로 바꾼 pathJson.
 *  중첩 셀의 문단별 ByPath 호출(삭제 undo 저장과 문자 서식 적용/undo)에 쓴다. */
export function cellPathJsonForPara(pos: DocumentPosition, cellParaIndex: number): string {
  const path = (pos.cellPath ?? []).map((e) => ({ ...e }));
  if (path.length > 0) path[path.length - 1].cellParaIndex = cellParaIndex;
  return JSON.stringify(path);
}

/**
 * 셀 문단 인덱스 — cellPath 가 있으면 마지막(가장 안쪽) 엔트리에서 읽는다.
 *
 * hit-test 는 flat 필드(controlIndex/cellIndex/cellParaIndex)를 `cellPath[0]`, 즉 **최외곽**
 * 엔트리에서 채운다(cursor_rect.rs 의 `outer = &ctx.path[0]`). 그래서 중첩 셀에서
 * `pos.cellParaIndex` 는 바깥 셀의 문단 인덱스이고, 안쪽 셀의 값은
 * `cellPath[last].cellParaIndex` 다. 이를 섞으면 ...ByPath API 에 바깥 축의 인덱스를 넘겨
 * 엉뚱한 문단을 병합/분할한다.
 *
 * cursor.ts(:399) 와 input-handler-text.ts(:307) 의 `useCellPath` 분기와 동일한 규칙이다.
 * depth 1 에서는 `cellPath[0]` 이 곧 최외곽이라 flat 값과 같으므로 동작 변화가 없다.
 *
 * [#2717] 셀 문단 경계 판정(호출자 가드)도 같은 축이어야 해서 export 한다 —
 * 축 유도를 복제하면 한쪽만 고쳐지는 회귀가 재발한다
 * (tests/undo-nested-cell-merge-offset.test.ts).
 */
export function cellParaIndexOf(pos: DocumentPosition): number {
  const path = pos.cellPath;
  return (path?.length ?? 0) > 0 ? path![path!.length - 1].cellParaIndex : pos.cellParaIndex!;
}

/**
 * [#2756] 비교용 셀 경로 — `cellParaIndexOf` 와 같은 축 규약의 경로 전체 버전.
 *
 * `cellParaIndexOf` 가 최내곽 **문단 인덱스** 하나를 주는 것과 달리, 이쪽은 셀 **정체성**을
 * 깊이별로 비교해야 하는 곳(선택 영역 정렬)에 쓴다. 두 함수는 같은 규약을 공유한다 —
 * `cellParaIndexOf(pos) === cellAxisPath(pos)[last].cellParaIndex`.
 *
 * `cellPath` 가 없는 위치(레거시 flat 좌표, `applyNavResult` 산출물)는 flat 필드로 1-depth
 * 경로를 합성한다. hit-test 가 flat 을 `cellPath[0]`(최외곽)에서 채우므로, depth 1 에서는
 * 합성 경로가 실제 경로와 완전히 같아 **동작 변화가 없다**.
 */
export function cellAxisPath(pos: DocumentPosition): CellPathEntry[] {
  if ((pos.cellPath?.length ?? 0) > 0) return pos.cellPath!;
  return [{
    controlIndex: pos.controlIndex ?? 0,
    cellIndex: pos.cellIndex ?? 0,
    cellParaIndex: pos.cellParaIndex ?? 0,
  }];
}

/** 셀 문단 구조 편집 뒤 flat/path 커서 위치를 같은 문단으로 맞춘다. */
export function cellParagraphPosition(
  pos: DocumentPosition,
  cellParaIndex: number,
  charOffset: number,
): DocumentPosition {
  const cellPath = pos.cellPath?.map((entry, index, path) =>
    index + 1 === path.length ? { ...entry, cellParaIndex } : entry,
  );
  return {
    ...pos,
    paragraphIndex: cellParaIndex,
    cellParaIndex,
    cellPath,
    charOffset,
    cursorRect: undefined,
  };
}

export function focusedCellCursorGeometryFromResult(
  pos: DocumentPosition,
  result: DeferredCellTextMutationResult,
): FocusedCellCursorGeometry | undefined {
  const geometry = result.focusedCursorGeometry;
  if (
    !result.paginationDeferred
    || result.cellFlowChanged
    || !geometry
    || geometry.targetCharOffset !== result.charOffset
  ) {
    return undefined;
  }
  const cloneAt = (charOffset: number): DocumentPosition => ({
    ...pos,
    charOffset,
    cellPath: pos.cellPath?.map((entry) => ({ ...entry })),
    cursorRect: undefined,
  });
  return {
    baseRevision: geometry.baseRevision,
    revision: geometry.revision,
    source: cloneAt(geometry.sourceCharOffset),
    target: cloneAt(geometry.targetCharOffset),
    deltaX: geometry.deltaX,
  };
}

export function focusedPagePatchFromResult(
  result: DeferredCellTextMutationResult,
): DeferredFocusedPagePatch | undefined {
  if (
    !result.paginationDeferred
    || result.cellFlowChanged
    || !result.focusedPageTreePatched
    || !result.focusedPagePatch
  ) {
    return undefined;
  }
  return { ...result.focusedPagePatch };
}

/**
 * [Task #2337-review] WASM 삭제 count 는 Rust `Paragraph::delete_text_at` 의 char(Unicode
 * scalar) 단위다. JS `String.length`(UTF-16 code unit)를 넘기면 astral 문자(😀 등)에서
 * 실제보다 많이 삭제해 undo/redo 가 인접 문자를 잃는다 → 코드포인트 수로 계산한다.
 * (커서 오프셋은 studio 의 UTF-16 관례를 유지하므로 여기서만 char 단위를 쓴다.)
 */
export function charCount(s: string): number {
  return [...s].length;
}
