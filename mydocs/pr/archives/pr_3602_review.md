---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-31
---

# PR #3602 리뷰 — MCP 세션 검색·누적 텍스트 치환

- PR: [#3602](https://github.com/edwardkim/rhwp/pull/3602)
- Related issue: [#3601](https://github.com/edwardkim/rhwp/issues/3601)
- 작성자: `kevin9327`
- 역할: collaborator 매개 외부 PR — `intake_and_review`, `local_validation`,
  `multi_pr_update_branch` 적용

## 변경과 누적 경계

원 contributor commit `c01b3956b`와 추가 렌더 증적 `c61447fb2`는 #3599의 세션 편집 API 위에 적층돼 있다.
따라서 최신 `upstream/devel` 위 `integrate/kevin9327-20260731`에서 #3599 다음으로 반영했다. 충돌은 없었고,
PR 내부의 `devel` merge commit은 제외했다.

`hwp_doc_search`는 주소 어휘 동형을 포함한 세션 문서 검색 결과를, `hwp_doc_replace_text`는 native
`replace_all` 경로의 누적 치환 결과를 MCP 응답으로 돌려준다. 계약 테스트는 검색 결과·치환 수·저장 후 재검색
경계를 고정한다. 구현은 session API와 native 문서 연산에 국한되며 renderer·layout 알고리즘을 변경하지 않아
visual sweep 대상이 아니다. 포함된 전/후 render 증적은 치환 효과가 실물 출력에도 반영되는지를 보조 확인한다.

## 검증과 판단

| 검증 | 결과 |
| --- | --- |
| `mcp_session_query_contract` 포함 focused 계약 | 28 passed |
| 기존 `cli_json_contract` | 22 passed |
| `cargo test --profile release-test --tests` | 누적 통합 트리 exit 0 |
| `cargo fmt --check` / clippy `-D warnings` | passed |
| 관련 Markdown 링크·metadata | passed |

모든 Cargo 검증은 `CARGO_TARGET_DIR=target/review-kevin9327-20260731`, `CARGO_INCREMENTAL=0`으로 실행했다.

**통합 PR merge 권고.** 이 PR은 #3599 세션 API를 포함한 단일 통합 PR의 full CI를 통과해야 한다. 통합 PR merge
뒤 #3601 close 상태를 확인하고, 원 #3602에는 통합 PR 링크와 검토 결과를 남긴 뒤 supersede close한다.
