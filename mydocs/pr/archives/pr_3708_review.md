---
kind: pr_review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3708 검토 — export-doclang JSON/MCP 표면

## 라우팅

base route: maintainer_general.md. 적용 보조 절차는 intake_and_review.md,
local_validation.md, multi_pr_update_branch.md이며, 누적 통합 PR은 #3742.

## 메타데이터와 적용 경계

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3708](https://github.com/edwardkim/rhwp/pull/3708) / @kevin9327 |
| 원 head / 기능 commit | 9d66cf8f7b7714f586fe5b39262e6b3e3fa3aaa8 |
| 통합 적용 | ef186322f |
| 적용 판정 | 수용. CLI JSON envelope와 MCP 도구를 같은 계약으로 누적했다. |

## 범위와 판정

export-doclang의 구조화된 JSON 결과와 hwp_export_doclang MCP 도구를 추가한다. CLI 명령,
capabilities, MCP 표면이 서로 다른 이름이나 결과 구조를 광고하지 않도록 contract test와
사용 문서를 함께 바꾼 변경이다.

## 공통 통합 후보와 검증

현재 유일한 code candidate는 [#3742](https://github.com/edwardkim/rhwp/pull/3742)의
b1e9619433bd9f068a361ddfb42ea0138f0077d1이며,
[CI run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 성공했다.
로컬에서는 CARGO_INCREMENTAL=0 및 target/review-kevin9327-20260801로 전체
cargo test --profile release-test --tests, fmt check, clippy -D warnings, git diff --check를
통과했다. 원 PR head의 상태는 이 누적 후보의 merge 근거를 대체하지 않는다.

## merge 전 조건

**조건부 수용.** 이 archive 기록·통합 계획·오늘할일을 같은 #3742에 docs-only tail로 올린 뒤,
LFS 대상 여부 판독과 review-only fast-pass의 preflight 및 Build & Test aggregate 성공,
최신 head의 MERGEABLE 상태를 재확인해야 한다. 그 전에는 merge나 원 PR 코멘트·supersede 처리를 하지 않는다.
