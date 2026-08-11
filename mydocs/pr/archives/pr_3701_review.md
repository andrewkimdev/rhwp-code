---
kind: pr_review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3701 검토 — nextCall 교정 호출

## 라우팅

base route: maintainer_general.md. 적용 보조 절차는 intake_and_review.md,
local_validation.md, multi_pr_update_branch.md이며, 누적 통합 PR은 #3742.

## 메타데이터와 적용 경계

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3701](https://github.com/edwardkim/rhwp/pull/3701) / @kevin9327 |
| 원 head / 기능 commit | 6f8a8cdf2dfbfe6eaf437ae3eade84bcf3ad4e52 |
| canonical 후속 | #3716의 rebase 기능 commit cc81dbc7b10a6270c3f71a228e588ba1302b1f09 |
| 통합 적용 | c07bee779 (#3716 canonical stack) |
| 적용 판정 | 단독 적용하지 않음. #3716이 #3698 기반과 이 기능을 함께 대체한다. |

## 범위와 판정

오류 응답의 isError 정보에 다음 기계 호출을 넣어 자동 소비자가 추측하지 않게 한다. #3701은
#3698 위 적층 PR이었고 #3716의 canonical rebase가 두 기능과 그 뒤의 edit/run 흐름을 일관된
계약으로 재구성했다. 따라서 #3716만 통합 적용하며 #3701은 그 저자성·원 범위를 기록하는 superseded
source로 남긴다.

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
