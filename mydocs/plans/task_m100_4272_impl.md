# 구현계획 — task_m100_4272

- **Issue**: #4272
- **수행계획**: [task_m100_4272.md](task_m100_4272.md)
- **대상 브랜치**: `fix/issue-4272-nested-cell-text-selection`
- **기준 commit**: `828eabc19a4953a684e05d523a614256dae28b26`

## 1. Rust 선택 대상 일반화

`src/document_core/queries/cursor_nav.rs`에서 선택 대상 표현을 본문·평면 셀·경로 셀로 구분한다.

- 기존 `get_selection_rects_native()`는 호환 wrapper로 유지한다.
- 내부 공통 함수는 평면 셀 또는 `(parentPara, cellPath)`를 받아 동일한 페이지 계획과 사각형
  계산을 사용한다.
- 경로 셀은 중간 엔트리의 `cellParaIndex`까지 정확히 비교하고, 마지막 엔트리의 문단 인덱스만
  현재 선택 문단에 맞춘다.
- IR 문단 조회는 `resolve_paragraph_by_path()`를 사용한다.

## 2. WASM·Studio 경계

- `getSelectionRectsInCellByPath`와 page hint options 변형을 WASM에 노출한다.
- `selection-page-hints.ts`에 path query dispatch와 단위 테스트를 추가한다.
- `WasmBridge`에 경로 기반 메서드를 추가한다.
- `InputHandler.updateSelection()`은 시작·끝의 cell container path가 같은지 비교해 깊이 2 이상이면
  새 API를 호출한다. 깊이 1은 기존 API를 유지한다.

## 3. E2E·증적

- `issue-4272-nested-cell-text-selection.test.mjs`에서 물리 5쪽 `23,504`의 실제 bbox를 찾아
  브라우저 mouse drag를 수행한다.
- `23,504` 전체 selection offset `0 -> 6`, 깊이 3 경로, 한 drag event당 path API 최대 1회,
  highlight 1개 이상, 관련 console warning/error 0건을 고정한다.
- 결과 JSON과 screenshot은 `output/4272/`에 생성한다.

## 4. 검증 순서

1. focused Rust matcher/API 테스트
2. Studio selection dispatch 단위 테스트와 TypeScript 검사
3. `cargo fmt --all -- --check`, `git diff --check`
4. 프로젝트 표준 Docker WASM 빌드:
   `docker compose --env-file .env.docker run --rm wasm`
5. #4272 실제 browser E2E와 인접 #4252 E2E
6. 결과를 `mydocs/working/task_m100_4272_stage1.md`에 기록
