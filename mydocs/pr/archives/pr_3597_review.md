---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-31
---

# PR #3597 리뷰 — 산출물 축 `--json` 3종과 MCP 도구 노출

- PR: [#3597](https://github.com/edwardkim/rhwp/pull/3597)
- Related issue: [#3596](https://github.com/edwardkim/rhwp/issues/3596) (`Closes #3596` 표기는 merge 뒤 실제 close 여부를 확인)
- 작성자: `kevin9327` (fork `pr/task-output-axis-json`)
- 역할: collaborator 매개 외부 PR — `collaborator_external_pr`, `intake_and_review`,
  `local_validation`, `multi_pr_update_branch` 적용

## 작성 시점 상태와 변경 범위

| 항목 | 값 |
| --- | --- |
| contributor 원 head | `c930cac3cbedacdf0354fccaeb139b695c60db0d` |
| 작성 시점 PR 상태 | `MERGEABLE` / `BEHIND` (원 PR은 통합 PR merge 뒤 supersede close 대상) |
| 원 변경 규모 | 5 files, +603 / -34 |
| 통합 branch 반영 | `f1b5c0012` (원 commit), `395039cf2` (계약 test rustfmt) |
| 통합 merge 조건 | 통합 PR 최신 head full CI 통과, mergeable 및 작업지시자 자동 승인 확인 |

`export-pdf`, `export-markdown`은 stdout JSON envelope으로 산출물 목록·매니페스트를 돌려주고,
`export-hwpx`는 검증 결과와 기존 종료 코드(검증 실패 포함)를 보존한 envelope을 돌려준다. 성공 데이터와
진단 stderr를 분리해 자동화 소비자가 stdout을 안전하게 파싱할 수 있게 한 계약이다. 같은 선언을
`capabilities --mcp`와 `mcp-serve`가 공유하도록 노출했으며, 계약 테스트는 success·오류·재파싱 실패와
도구 선언을 고정한다.

renderer, typeset, layout, paint, pagination, visual fixture 원본은 변경하지 않는다. 따라서 visual sweep은
적용 대상이 아니다. 다만 PR에 포함된 실물 산출물 evidence(문서 JSON, HWPX 재검증, MCP 응답)는 열람해
명령·응답과 문서화된 계약이 일치함을 확인했다.

## 누적 검토와 로컬 검증

최신 `upstream/devel` `96892e31c8e5cdfd208e1c2f9fdd0faa25c5b6dd` 위의
`integrate/kevin9327-20260731`에서 다음 순서로 cherry-pick했다. PR 안의 `devel` merge commit은
제외했고 충돌은 없었다. 이 branch가 유일한 code candidate이며, 원 PR별 source branch는 추가 push·개별
merge 대상으로 사용하지 않는다.

| 순서 | PR | 적용 contributor commit |
| --- | --- | --- |
| 1 | #3597 | `c930cac3c` |
| 2 | #3599 | `52462c9c2`, `dc078646f` |
| 3 | #3602 | `c01b3956b`, `c61447fb2` |
| 4 | #3607 | `9a1d0f2d2` |
| 5 | #3610 | `19e16734f` |

| 검증 | 결과 |
| --- | --- |
| focused 신규 계약 5 바이너리 | 28 passed |
| 기존 `cli_json_contract` | 22 passed |
| `cargo test --profile release-test --tests` | 누적 통합 트리 exit 0 (완료까지 대기해 확인) |
| `cargo fmt --all -- --check` | 원 head test 세 곳 → `7d6c4a5bf`, 최신 CI가 지적한 `src/main.rs` 한 블록 → `71c798465`, 보정 후 passed |
| `cargo clippy --all-targets -- -D warnings` | passed |
| Markdown 링크·metadata | passed |

모든 Cargo 검증은 전용 `target/review-kevin9327-20260731`, `CARGO_INCREMENTAL=0`으로 실행했다.
전체 release-test는 서식 보정 전의 동일 프로그램 동작을 검증했고, 보정 뒤에도 전체 `release-test`를
재실행해 `exit 0`을 확인했다. 이어 focused 계약·fmt·clippy도 재실행했다. 보정에 production 의미 변경은
없지만, 통합 code candidate에는 test commit이 추가됐으므로 최신 **통합 PR** head의 full CI를 merge 전
필수 조건으로 둔다.

## 발견 사항과 수용 판단

원 #3597 source branch의 첫 CI 실패는 `tests/output_axis_json_contract.rs`의 rustfmt 세 곳이었다.
원 commit을 보존한 채 test-only commit `395039cf2`로 정렬했다. 이후 source branch를 최신화하며 발견된
`src/main.rs` closure 서식 보정은 통합 base에서는 이미 적용된 상태라 cherry-pick이 빈 변경으로 판정됐다.
두 경우 모두 production 동작이나 JSON 계약의 결함이 아니다.

**통합 PR merge 권고.** #3597은 #3599, #3602, #3607, #3610과 함께 하나의 최신 `devel` 대상 통합 PR로
검증·merge한다. 통합 PR CI·mergeability를 재확인한 뒤 그 PR만 merge하고, #3596의 자동 close를 확인한다.
원 #3597은 contributor 이력과 검토 기록을 보존한 채 통합 PR 링크를 남겨 supersede close한다. 전체 실행과
rollback 경계는 `pr_3597_review_impl.md`에 기록한다.
