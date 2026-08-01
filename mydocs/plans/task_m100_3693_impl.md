# 구현계획서 — task_m100_3693

- **이슈**: #3693
- **상위 이슈**: #1528
- **브랜치**: `codex/issue-3693-export-structure-clause`
- **수행계획서**: `mydocs/plans/task_m100_3693.md`

## Stage 1 — clause marker 파싱

1. `classify_clause()`의 `제N조` marker 조립 뒤 선택적인 `의M` suffix를 소비한다.
2. 숫자/한글 marker의 구분자를 `.` 또는 `)`로 인식한다.
3. 기존 `kind`/`level`/`heading`/좌표 JSON은 변경하지 않는다.

## Stage 2 — 구조 문맥 수용

1. `build_structure()`에서 clause 후보를 stack 문맥으로 검증한다.
2. `호`는 열린 `조|항`, `목`은 열린 `호`가 있을 때만 heading으로 채택한다.
3. 거부된 후보 문단은 기존 비제목 경로로 보내 preamble 또는 현재 제목의 `body`에 보존한다.
4. strong marker와 explicit `outline` mode 경로는 변경하지 않는다.

## Stage 3 — 단위·실문서 통합 테스트

1. 모듈 단위 테스트:
   - `제1조의2` marker 전체 보존
   - `1)`/`가)` 후보 검출
   - synthetic `조 → 항 → 호 → 목` 트리
   - standalone 숫자 후보 거부
2. `tests/issue_3693_structure_clause_context.rs`:
   - `hwp3-sample16-hwp5.hwp`의 협정서 구간에서 `제1조` 아래 `1.`~`3.` 보존
   - `2022년 국립국어원 업무계획.hwp`의 `2022. 1.`이 clause node가 아님
3. 기존 `cli_json_contract`의 기본/봉투 테스트로 JSON 호환을 확인한다.

## Stage 4 — 검증·문서·커밋

1. 수행계획서의 focused 명령을 순차 실행한다.
2. `mydocs/working/task_m100_3693_stage1.md`와
   `mydocs/report/task_m100_3693_report.md`에 기준선·변경·실측 결과를 기록한다.
3. 오늘할일 #3693 상태를 완료로 갱신하고 단계 변경 전 커밋한다.
4. #3695는 #3693 커밋 이후 별도 브랜치·문서 단계로 시작한다.

## 비적용

- 새 정규식 의존성 도입 없음
- 새 fixture·golden·baseline 추가 없음
- CLI parser, WASM binding, MCP schema 변경 없음
