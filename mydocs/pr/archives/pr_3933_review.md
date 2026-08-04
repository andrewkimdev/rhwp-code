# PR #3933 검토 — clause 문맥 confidence 경계

- **PR**: [#3933](https://github.com/edwardkim/rhwp/pull/3933)
- **Issue**: [#3744](https://github.com/edwardkim/rhwp/issues/3744)
- **상위 이슈**: [#1528](https://github.com/edwardkim/rhwp/issues/1528)
- **작성자**: `postmelee` (collaborator self PR)
- **base / head**: `devel` / `task_m100_3744`
- **기능·보고 candidate head**: `532df0f13` (작성 시점 참고값)
- **기준 devel**: `2971a1d9a`
- **작성 시점 상태**: draft, `MERGEABLE` / `BLOCKED`, GitHub Actions 진행 중
- **작성 시점 규모**: 9 files, +1,233 / -10, 5 commits

## 1. 절차 경로

- base route: `collaborator_self_merge.md`
- modifiers: `intake_and_review.md`, `local_validation.md`, `rework_and_exceptions.md` 대형 PR 경로
- loaded documents: `pr_review_workflow.md`, `pr_review/README.md`, 위 기본·보조 문서
- visual modifier: 비대상. query-only 변경이며 renderer/layout/paint, sample, golden을 바꾸지 않는다.

PR 작성자가 현재 로그인한 collaborator와 같아 자기 자신을 reviewer로 요청하지 않았다. 별도 code
review와 작업지시자 승인을 merge 조건으로 남긴다. 이 review tail이 push되면 PR head가 바뀌므로,
mergeable·CI·head SHA는 최신 값으로 다시 확인해야 한다.

## 2. 관련 이슈와 변경 범위

#3744는 #1528의 세 번째 하위 작업으로, #3693의 clause marker/stack과 #3695의 auto 선택 정책 뒤에
남은 explicit clause confidence 경계를 다룬다.

- 오래된 `조|항` anchor 뒤의 SQL·일반 숫자 목록 `호` 오탐 제거
- 조문 안 `YYYY. M. D.` 날짜 `호` 오탐 제거 및 body 보존
- `장|절` 아래 실제 `가.`~`하.` direct `목` 회복
- 영구 회귀 8건과 Stage 1~4 계획·근거·최종 corpus 보고

공개 JSON 필드·봉투, CLI option·exit code, explicit outline, auto selector, parser, serializer,
renderer는 범위 밖이며 최종 diff에서도 바뀌지 않았다.

## 3. 계획·구현 독립 대조

Stage 3에서 구현 commit을 계획과 분리해 대조했다.

| 승인 정책 | 구현 대조 | 판정 |
| --- | --- | --- |
| `N.N`에서 nearest anchor 만료 | `ClauseGateState`가 anchor 식별자별 만료만 보관하며 새 `조|항`에 전파하지 않음 | 충족 |
| 달력형 lexical reject | 월 1~12·일 1~31인 `YYYY. M. D.`만 거부하고 anchor는 만료하지 않음 | 충족 |
| direct `목` confidence | 열린 `호`가 없고 `장|절` 아래에서 tail·margin·indent·level 조건을 모두 확인 | 충족 |
| 거부 문단 보존 | 기존 body/preamble 경로로 보내는 회귀 단언 포함 | 충족 |
| 공개 계약 무변경 | upstream의 `nodeCount` rename을 보존하고 CLI 계약 test 통과 | 충족 |

파일명, style/shape ID, paragraph 좌표를 제품 판정에 하드코딩하지 않았다. 기존 수행·구현계획서가
승인 경계와 rollback 범위를 충분히 정의하므로 별도 review_impl 문서는 필요하지 않다.

## 4. corpus 영향 검토

baseline/current checkout에 분리된 Cargo target을 사용해 artifact 혼입 없이 비교했다.

| 범위 | 후보 | parse 성공 | parse 차이 | 변경 문서 | `호` 증감 | `목` 증감 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| top-level | 353 | 350 | 0 | 10 | -4,351 | +144 |
| recursive | 673 | 670 | 0 | 11 | -4,351 | +167 |

recursive 변경 11문서는 오래된 anchor 제거 6개와 실제 direct 제목 회복 5개로 모두 분류됐다.
편·장·절·조·항 수와 parse 성공/실패는 변하지 않았고 미분류 변화는 없다. Oracle 경계 전 네 `호`,
더 깊은 shape의 direct `목`, 월별 실제 일수까지 검증하지 않는 날짜 문법은 승인된 잔여 trade-off다.

## 5. 로컬 검증

모든 Cargo 명령은 최신 `devel` `2971a1d9a` 결합 checkout에서 `CARGO_INCREMENTAL=0`과 전용 target을
사용해 순차 실행했다.

| 게이트 | 결과 |
| --- | --- |
| structure 단위 | 8 passed |
| `issue_3744_structure_clause_confidence` | 8 passed |
| `issue_3693_structure_clause_context` | 3 passed |
| `issue_3695_structure_auto_policy` | 13 passed |
| `cli_json_contract export_structure_` | 4 passed |
| `cargo test --profile release-test --tests` | 전체 test target exit 0, 실패 0 |
| `cargo fmt --check` / `git diff --check` | 통과 |
| `cargo clippy --all-targets -- -D warnings` | 통과 |

시각 검증은 structure query의 분류 결과만 바뀌고 renderer/layout/paint·fixture에 영향이 없어 생략했다.

## 6. 규모와 risk

1,000줄을 넘지만 제품 코드는 `structure.rs` 1개 파일의 제한된 내부 gate 변경이고, 나머지 대부분은
영구 회귀 8건과 하이퍼-워터폴 계획·단계·최종 보고다. 그래도 대형 PR 예외 규칙에 따라 즉시 merge하지
않고 별도 code review cycle과 최신 head CI를 모두 요구한다.

관찰할 핵심 risk는 정상 조문 반복 목록의 과도한 제거와 direct `목`의 일반 목록 오인이다. 전체 corpus
변경 문서 분류와 #3693·#3695 무회귀 test가 현재 정책 범위에서는 두 위험을 제한한다.

## 7. 작성 시점 CI와 권고

candidate head `532df0f13`에서 CI preflight와 CodeQL preflight는 성공했고 나머지 GitHub Actions는
진행 중이었다. review 문서 push 뒤 새 head의 required checks를 처음부터 다시 확인해야 한다.

현재 권고는 **draft 유지·검토 계속**이다. 최신 head의 GitHub Actions 통과, 별도 code review와
작업지시자 승인 전에는 ready 전환·merge하지 않는다. merge 뒤에는 #3744 auto-close 상태와 결과 comment,
상위 #1528의 세 하위 작업 통합 검증·종료 여부를 별도 후속 절차에서 확인한다.
