import type { WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition, CharProperties } from '@/core/types';
import { IMMEDIATE_TEXT_MUTATION_EFFECTS, type TextMutationEffects } from './types';
import {
  canUseDeferredCellTextDelete,
  canUseDeferredCellTextInsert,
  canUseLocalBodyTextReplace,
  cellPathJson,
  focusedCellCursorGeometryFromResult,
  focusedPagePatchFromResult,
  isCell,
  isNestedCell,
} from './cell-path';

export function insertTextWithMutationEffects(
  wasm: WasmBridge,
  pos: DocumentPosition,
  text: string,
): TextMutationEffects {
  if (isNestedCell(pos)) {
    wasm.insertTextInCellByPath(pos.sectionIndex, pos.parentParaIndex!, cellPathJson(pos), pos.charOffset, text);
  } else if (isCell(pos)) {
    if (canUseDeferredCellTextInsert(pos, text)) {
      const result = wasm.insertTextInCellDeferredPagination(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex!, pos.cellParaIndex!, pos.charOffset, text);
      const focusedCursorGeometry = focusedCellCursorGeometryFromResult(pos, result);
      const focusedPagePatch = focusedPagePatchFromResult(result);
      return {
        documentPaginationPending: result.paginationDeferred,
        flowChanged: result.cellFlowChanged,
        paginationCompleted: !result.paginationDeferred,
        ...(focusedCursorGeometry ? { focusedCursorGeometry } : {}),
        ...(focusedPagePatch ? { focusedPagePatch } : {}),
      };
    } else {
      wasm.insertTextInCell(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex!, pos.cellParaIndex!, pos.charOffset, text);
    }
  } else if (canUseLocalBodyTextReplace(pos, 0, text)) {
    return replaceBodyTextWithMutationEffects(wasm, pos, 0, text);
  } else {
    wasm.insertText(pos.sectionIndex, pos.paragraphIndex, pos.charOffset, text);
  }
  return IMMEDIATE_TEXT_MUTATION_EFFECTS;
}

export function replaceBodyTextWithMutationEffects(
  wasm: WasmBridge,
  pos: DocumentPosition,
  deleteCount: number,
  text: string,
): TextMutationEffects {
  const result = wasm.replaceBodyTextLocal(
    pos.sectionIndex,
    pos.paragraphIndex,
    pos.charOffset,
    deleteCount,
    text,
  );
  return {
    documentPaginationPending: result.documentPaginationPending,
    flowChanged: result.flowChanged,
    paginationCompleted: !result.documentPaginationPending,
  };
}

export function replaceCellTextWithMutationEffects(
  wasm: WasmBridge,
  pos: DocumentPosition,
  deleteCount: number,
  text: string,
): TextMutationEffects {
  const result = wasm.replaceTextInCellDeferredPagination(
    pos.sectionIndex,
    pos.parentParaIndex!,
    pos.controlIndex!,
    pos.cellIndex!,
    pos.cellParaIndex!,
    pos.charOffset,
    deleteCount,
    text,
  );
  const focusedCursorGeometry = focusedCellCursorGeometryFromResult(pos, result);
  const focusedPagePatch = focusedPagePatchFromResult(result);
  return {
    documentPaginationPending: result.paginationDeferred,
    flowChanged: result.paginationDeferred && result.cellFlowChanged,
    paginationCompleted: !result.paginationDeferred,
    ...(focusedCursorGeometry ? { focusedCursorGeometry } : {}),
    ...(focusedPagePatch ? { focusedPagePatch } : {}),
  };
}

/** undo/구조 명령의 full-refresh 복원은 flat cell에서도 immediate pagination을 사용한다. */
export function doInsertTextImmediate(wasm: WasmBridge, pos: DocumentPosition, text: string): void {
  if (isNestedCell(pos)) {
    wasm.insertTextInCellByPath(pos.sectionIndex, pos.parentParaIndex!, cellPathJson(pos), pos.charOffset, text);
  } else if (isCell(pos)) {
    wasm.insertTextInCell(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex!, pos.cellParaIndex!, pos.charOffset, text);
  } else {
    wasm.insertText(pos.sectionIndex, pos.paragraphIndex, pos.charOffset, text);
  }
}

export function deleteTextWithMutationEffects(
  wasm: WasmBridge,
  pos: DocumentPosition,
  count: number,
): TextMutationEffects {
  if (isNestedCell(pos)) {
    wasm.deleteTextInCellByPath(pos.sectionIndex, pos.parentParaIndex!, cellPathJson(pos), pos.charOffset, count);
  } else if (isCell(pos)) {
    if (canUseDeferredCellTextDelete(pos, count)) {
      const result = wasm.deleteTextInCellDeferredPagination(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex!, pos.cellParaIndex!, pos.charOffset, count);
      const focusedCursorGeometry = focusedCellCursorGeometryFromResult(pos, result);
      const focusedPagePatch = focusedPagePatchFromResult(result);
      return {
        documentPaginationPending: result.paginationDeferred,
        flowChanged: result.cellFlowChanged,
        paginationCompleted: !result.paginationDeferred,
        ...(focusedCursorGeometry ? { focusedCursorGeometry } : {}),
        ...(focusedPagePatch ? { focusedPagePatch } : {}),
      };
    }
    wasm.deleteTextInCell(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex!, pos.cellParaIndex!, pos.charOffset, count);
  } else if (canUseLocalBodyTextReplace(pos, count, '')) {
    return replaceBodyTextWithMutationEffects(wasm, pos, count, '');
  } else {
    wasm.deleteText(pos.sectionIndex, pos.paragraphIndex, pos.charOffset, count);
  }
  return IMMEDIATE_TEXT_MUTATION_EFFECTS;
}

export function doDeleteTextImmediate(wasm: WasmBridge, pos: DocumentPosition, count: number): void {
  if (isNestedCell(pos)) {
    wasm.deleteTextInCellByPath(pos.sectionIndex, pos.parentParaIndex!, cellPathJson(pos), pos.charOffset, count);
  } else if (isCell(pos)) {
    wasm.deleteTextInCell(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex!, pos.cellParaIndex!, pos.charOffset, count);
  } else {
    wasm.deleteText(pos.sectionIndex, pos.paragraphIndex, pos.charOffset, count);
  }
}

export function doGetTextRange(wasm: WasmBridge, pos: DocumentPosition, count: number): string {
  if (isNestedCell(pos)) {
    return wasm.getTextInCellByPath(pos.sectionIndex, pos.parentParaIndex!, cellPathJson(pos), pos.charOffset, count);
  } else if (isCell(pos)) {
    return wasm.getTextInCell(pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!, pos.cellIndex!, pos.cellParaIndex!, pos.charOffset, count);
  } else {
    return wasm.getTextRange(pos.sectionIndex, pos.paragraphIndex, pos.charOffset, count);
  }
}

/**
 * [#4162] 캐럿 대기 글자 모양(pending char shape) — 방금 삽입된 range 에 글자 서식을 건다.
 *
 * ApplyCharFormatCommand.execute() 의 셀/본문 분기와 같은 축이다(셀은 항상 ...ByPath).
 * from === to(빈 range)면 적용 대상이 없으므로 아무것도 하지 않는다.
 */
export function applyCharShapeModsToRange(
  wasm: WasmBridge,
  pos: DocumentPosition,
  from: number,
  to: number,
  props: Partial<CharProperties>,
): void {
  if (to <= from) return;
  const propsJson = JSON.stringify(props);
  if (isCell(pos)) {
    wasm.applyCharFormatInCellByPath(pos.sectionIndex, pos.parentParaIndex!, cellPathJson(pos), from, to, propsJson);
  } else {
    wasm.applyCharFormat(pos.sectionIndex, pos.paragraphIndex, from, to, propsJson);
  }
}
