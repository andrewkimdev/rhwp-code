---
kind: pr_review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3692 검토 — 외부 바인딩 공통 기반 문서

## 라우팅

base route: maintainer_general.md. 적용 보조 절차는 intake_and_review.md,
local_validation.md, multi_pr_update_branch.md이며, 누적 통합 PR은 #3742.

## 메타데이터와 적용 경계

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3692](https://github.com/edwardkim/rhwp/pull/3692) / @kevin9327 |
| 원 head / 기능 commit | 6508dde2bf912c0488ac613bb299d15117aee8d8 |
| 통합 적용 | 0cc37c5fa |
| 적용 판정 | 수용. 독립 문서 변경으로 누적 code candidate에 그대로 적용했다. |

## 범위와 판정

IR 스키마 버저닝, 표면 판단 매트릭스, Python 바인딩 첫 명세를 기술 문서의 공통 기반으로
추가한다. 구현 경로나 renderer를 바꾸지 않으며, 뒤따르는 CLI/MCP 표면 확장의 계약 문맥을 정리한다.

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
