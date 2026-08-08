import type { InputHandler } from '@/engine/input-handler';

/**
 * [#4162] `getInputHandler()`가 null 인 채로 옵셔널 체이닝만 타면 커맨드가 조용히
 * 사라진다 — 사용자는 단축키/버튼을 눌렀는데 아무 일도 없었던 것으로 본다. 커맨드
 * 실행부는 이 helper로 InputHandler를 받아, 없으면 사유를 남기고 null을 돌려준다.
 *
 * 같은 commandId 로 반복 실패(빠른 연타, 초기화 전 클릭)해도 콘솔을 스팸하지 않도록
 * 최초 1회만 경고한다 — resetCommandFeedbackDedupe()로 초기화(테스트용).
 */
const warnedCommandIds = new Set<string>();

export function requireInputHandler(
  ctx: { getInputHandler: () => InputHandler | null },
  commandId: string,
): InputHandler | null {
  const handler = ctx.getInputHandler();
  if (handler) return handler;
  if (!warnedCommandIds.has(commandId)) {
    warnedCommandIds.add(commandId);
    console.warn(`[Command] ${commandId} 실행 불가 — InputHandler 가 아직 준비되지 않았습니다.`);
  }
  return null;
}

export function resetCommandFeedbackDedupe(): void {
  warnedCommandIds.clear();
}
