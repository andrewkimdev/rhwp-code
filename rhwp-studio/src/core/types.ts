/**
 * 공유 타입 재수출 심 — 실제 선언은 `./types/` 관심사별 모듈에 있다.
 *
 * - `types/document.ts` — 문서 모델(WASM 쿼리 반환·편집 속성·표/셀·개체·검색)
 * - `types/preflight.ts` — CanvasKit 문서 사전점검(렌더 프로필·요약·차단 요인)
 * - `types/layers.ts` — Layer* 렌더 IR(레이어 트리·페인트 오퍼레이션·글리프 페이로드)
 *
 * 기존 `@/core/types` 임포터 56곳은 이 심을 그대로 쓴다(선언 내용은 동결 이동).
 */
export * from './types/document';
export * from './types/preflight';
export * from './types/layers';
