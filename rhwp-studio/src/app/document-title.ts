/**
 * 브라우저 탭 제목 — 편집 중인 문서 파일명을 항상 탭에 노출한다.
 *
 * 파일명의 원천은 WasmBridge.fileName 이고, 변경 지점(문서 초기화·저장 이름 확정)과
 * document-dirty-changed 전이에서 applyDocumentTitle 을 호출하면 제목이 최신을 유지한다.
 * 인쇄(file.ts)는 직전 제목을 스냅샷해 복원하므로 동적 제목과도 정상 왕복한다.
 */

/** 순수 포맷터 — 단위 테스트 대상. */
export function documentTitleFor(fileName: string, dirty: boolean): string {
  return `${fileName}${dirty ? '*' : ''} | rhwp`;
}

export function applyDocumentTitle(fileName: string, dirty: boolean): void {
  document.title = documentTitleFor(fileName, dirty);
}
