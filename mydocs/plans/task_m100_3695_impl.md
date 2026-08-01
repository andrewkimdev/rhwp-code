# 구현계획서 — task_m100_3695

- **이슈**: #3695
- **상위 이슈**: #1528
- **선행 커밋**: #3693 `652e2ee27`
- **브랜치**: `codex/issue-3695-export-structure-auto`
- **수행계획서**: `mydocs/plans/task_m100_3695.md`

## Stage 1 — auto 증거 수집

1. 참조된 paragraph의 para shape를 순회한다.
2. `HeadType::Outline`은 즉시 authoritative outline으로 판정한다.
3. `HeadType::Number`는 약한 outline 증거로 기록한다.
4. 렌더링 질의와 같은 수식 포함 텍스트 조립기로 편·장·절·관·조 marker를 찾는다.

## Stage 2 — effective mode 우선순위

1. 명시적 Outline을 가장 먼저 선택한다.
2. Outline이 없으면 primary clause marker를 Number보다 우선한다.
3. primary clause marker가 없을 때만 Number로 outline을 선택한다.
4. 증거가 없으면 clause로 폴백한다.
5. explicit `StructureMode::Outline|Clause`는 selector를 우회한다.

## Stage 3 — synthetic·실문서 회귀 테스트

1. synthetic:
   - 조문 + 일반 Number 혼합 → auto clause
   - pure clause → auto clause
   - explicit Outline + 조문 모양 텍스트 → auto outline
   - single Number-only → auto outline
   - 같은 혼합 문서의 explicit outline/clause 선택 유지
2. 실제 sample:
   - `hwpctl_API_v2.4.hwp` 명시적 Outline 유지
   - `biz_plan.hwp` 복수 Number outline 유지
   - `추진일정.hwp` 단일 Number outline 유지
3. #3693 실문서 clause 회귀와 CLI JSON 계약을 다시 실행한다.

## Stage 4 — 문서·검증·커밋

1. CLI 매뉴얼의 auto 설명을 selector의 실제 우선순위와 맞춘다.
2. 수행계획서의 focused 명령을 순차 실행한다.
3. `mydocs/working/task_m100_3695_stage1.md`와
   `mydocs/report/task_m100_3695_report.md`에 red·green·실측 결과를 기록한다.
4. 오늘할일 #3695 상태를 갱신하고 #3693과 분리된 후속 커밋으로 고정한다.

## 비적용

- 확률값·튜닝 가능한 threshold 도입 없음
- 새 외부 의존성·fixture·baseline 추가 없음
- CLI parser, WASM binding, MCP schema 변경 없음
