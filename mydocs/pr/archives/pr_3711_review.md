---
kind: pr_review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3711 검토 — Unicode confusable 누름틀 이름 방어

## 라우팅

base route: maintainer_general.md. 적용 보조 절차는 intake_and_review.md,
local_validation.md, multi_pr_update_branch.md이며, 누적 통합 PR은 #3742.

## 메타데이터와 적용 경계

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3711](https://github.com/edwardkim/rhwp/pull/3711) / @kevin9327 |
| 원 head | 8c453b69136ed138543b6d33a7f7bc4ebd08a10b |
| 원 기능 commit | 832d79bfb3667d29442f9d6c328c82fbd69004e6 |
| 통합 적용 | e1b5431db, 증적 30d88fddc |
| 적용 판정 | 수용. 필드 탐색·채움과 MCP 표면의 confusable 경계를 함께 누적했다. |

## 범위와 판정

화면상 같은 이름의 누름틀이 다른 유니코드 문자열일 때 엉뚱한 칸을 채우고 성공으로 보이던
보안 결함을 탐지한다. field query와 fill 응답이 모호성 정보를 보존하며, 후속 run 계획의
fill_fields도 같은 text_security 판단을 재사용하도록 통합 보정했다.

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
