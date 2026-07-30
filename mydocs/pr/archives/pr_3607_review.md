---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-31
---

# PR #3607 리뷰 — 생성·미리보기 축 JSON 봉투와 MCP 도구

- PR: [#3607](https://github.com/edwardkim/rhwp/pull/3607)
- Related issue: [#3600](https://github.com/edwardkim/rhwp/issues/3600)
- 작성자: `kevin9327`
- 역할: collaborator 매개 외부 PR — `intake_and_review`, `local_validation`,
  `multi_pr_update_branch` 적용

## 변경과 누적 경계

원 contributor commit `9a1d0f2d2`는 #3597의 CLI JSON 공통 계약 위에 적층돼 있어, 최신
`upstream/devel`의 `integrate/kevin9327-20260731`에서 #3597 뒤에 반영했다. 원 변경은 5 files,
+1,012 / -60으로 1,000 lines를 넘으므로 API 선언·stdout envelope·오류 경계·계약 테스트를 분리해 검토했다.
충돌은 없었고 PR 내부의 `devel` merge commit은 제외했다.

`build-from-ingest --json`은 생성물 envelope을, `thumbnail --json`은 미리보기 envelope을 내보내며 MCP에도
동일 기능을 두 도구로 노출한다. 공용 JSON contract를 사용해 stdout의 기계 파싱과 stderr 진단을 분리한 점이
#3597의 축과 일관된다. renderer의 출력물을 호출하지만 renderer·typeset·layout 구현이나 golden은 변경하지
않으므로 visual sweep 대상이 아니다. 포함된 JSON 및 rhwp-rendered evidence는 명령 결과를 보조 확인했다.

## 검증과 판단

| 검증 | 결과 |
| --- | --- |
| `genpreview_json_contract` 포함 focused 계약 | 28 passed |
| 기존 `cli_json_contract` | 22 passed |
| `cargo test --profile release-test --tests` | 누적 통합 트리 exit 0 |
| `cargo fmt --check` / clippy `-D warnings` | passed |
| 관련 Markdown 링크·metadata | passed |

모든 Cargo 검증은 `CARGO_TARGET_DIR=target/review-kevin9327-20260731`, `CARGO_INCREMENTAL=0`으로 실행했다.

**통합 PR merge 권고.** 범위가 큰 변경이지만 계약·통합 검증에서 수용 기준을 충족했다. 단일 통합 PR의 full CI
통과 뒤 #3600 close 상태를 확인하고, 원 #3607에는 통합 PR 링크와 검토 결과를 남긴 뒤 supersede close한다.
