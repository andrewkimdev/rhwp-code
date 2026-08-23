/**
 * 선택 텍스트 기반 누름틀 이름 제안 — `ih.getSelection()`이 반환하는 임의의 텍스트
 * 선택 범위에서 평문을 읽어온다. 읽기 전용(wasm 쓰기 없음) — `field-name-suggest.ts`와
 * 같은 이유로 `src/core/`에 둔다(`mutation-routing-guard.test.ts`가 스캔하는
 * `src/command/` 밖).
 *
 * `field-name-suggest.ts`의 "라벨 셀 + 인접 빈 셀" 자동 스캔과는 별개의 후보 소스다
 * — 표 그리드를 전혀 읽지 않고, 사용자가 직접 드래그 선택한 범위 하나만 다룬다.
 * "신청인" 텍스트 뒤에 긴 공백, 그 뒤에 "(인)"이 오는 것처럼 표의 두 셀로 나뉘지
 * 않고 본문 문단 또는 표 셀 하나 안에 통짜로 들어있는 라벨 패턴을 겨냥한다.
 *
 * 본문/단일 표 셀 분기는 `WasmBridge.insertClickHereField`
 * (`wasm-bridge.ts:insertClickHereField`)와 `InputHandler.getParaFormatTargetsForRange`
 * 가 이미 쓰는 것과 같은 패턴을 따른다. 중첩 표(cellPath.length > 1)와 글상자는
 * 두 곳 모두와 마찬가지로 지원 범위 밖이다.
 */
import type { WasmBridge } from './wasm-bridge';
import type { DocumentPosition } from './types';

/**
 * 선택 범위(start~end, `ih.getSelection()` 결과 — 이미 start&lt;end 순서)가 본문의
 * 한 문단 또는 단일 표 셀의 한 문단 안에 완전히 들어있는지 판정한다. 들어있으면
 * 텍스트 조회에 필요한 `{pos, count}`를 반환하고, 아니면 `null`을 반환한다
 * (지원 범위 밖 — 중첩 표, 글상자, 문단/셀 경계를 넘는 선택, 본문↔셀 혼합,
 * 다른 섹션, 역방향/빈 범위).
 */
export function singleParagraphSelectionQuery(
  start: DocumentPosition,
  end: DocumentPosition,
): { pos: DocumentPosition; count: number } | null {
  if (start.isTextBox || end.isTextBox) return null;
  if ((start.cellPath?.length ?? 0) > 1 || (end.cellPath?.length ?? 0) > 1) return null;
  if (start.sectionIndex !== end.sectionIndex) return null;

  const startInCell = start.parentParaIndex !== undefined;
  const endInCell = end.parentParaIndex !== undefined;
  if (startInCell !== endInCell) return null;

  if (startInCell) {
    if (
      start.parentParaIndex !== end.parentParaIndex ||
      start.controlIndex !== end.controlIndex ||
      start.cellIndex !== end.cellIndex ||
      start.cellParaIndex !== end.cellParaIndex
    ) {
      return null;
    }
  } else if (start.paragraphIndex !== end.paragraphIndex) {
    return null;
  }

  const count = end.charOffset - start.charOffset;
  if (count <= 0) return null;

  return { pos: { ...start }, count };
}

/** `{pos,count}`에서 실제 텍스트를 읽는다 — 본문/단일 셀 2분기(중첩 표는 호출부에서 이미 배제됨). */
export function readSelectionText(wasm: WasmBridge, pos: DocumentPosition, count: number): string {
  if (pos.parentParaIndex !== undefined && pos.controlIndex !== undefined) {
    return wasm.getTextInCell(
      pos.sectionIndex,
      pos.parentParaIndex,
      pos.controlIndex,
      pos.cellIndex ?? 0,
      pos.cellParaIndex ?? 0,
      pos.charOffset,
      count,
    );
  }
  return wasm.getTextRange(pos.sectionIndex, pos.paragraphIndex, pos.charOffset, count);
}

/**
 * 선택 텍스트를 누름틀 이름 후보 원문으로 뽑는다. 문단/셀 경계를 넘거나
 * 공백만 있으면 `null`. 길이 상한(`MAX_FIELD_NAME_LEN`)은 여기서 강제하지
 * 않는다 — 호출부(UI)가 사용자에게 보이는 메시지로 안내해야 하므로.
 *
 * `insertPos`는 선택 영역의 끝(`end`)이다 — `singleParagraphSelectionQuery`가
 * 이미 start/end가 같은 문단/셀임을 보장하므로 별도 오프셋 계산이 필요 없다.
 * 라벨 텍스트는 그대로 두고 그 바로 뒤(공백이 시작되는 지점)에 필드를 삽입하기
 * 위함이다.
 */
export function extractSelectedLabel(
  wasm: WasmBridge,
  start: DocumentPosition,
  end: DocumentPosition,
): { text: string; insertPos: DocumentPosition } | null {
  const query = singleParagraphSelectionQuery(start, end);
  if (!query) return null;
  const text = readSelectionText(wasm, query.pos, query.count).trim();
  if (!text) return null;
  return { text, insertPos: { ...end } };
}
