---
kind: pr_review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3689 검토 — digest v2 절 청킹과 연속 쪽 창

## 라우팅

base route: maintainer_general.md. 적용 보조 절차는 intake_and_review.md,
local_validation.md, multi_pr_update_branch.md이며, 누적 통합 PR은 #3742.

## 메타데이터와 적용 경계

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3689](https://github.com/edwardkim/rhwp/pull/3689) / @kevin9327 |
| 원 head | 2f98d01803b5912a7a3d352b9def42116a377db2 (devel merge commit) |
| 원 기능 commit | df2558dd793c714beae18469f0aec7972e931a63, a29afa34edb7d748308a27944bc01d47af30e9fd |
| 통합 적용 | 5d35bbdde, 0fd231b94 및 digest fixture 보정 b1e961943 |
| 적용 판정 | 수용. 원 head의 devel merge commit은 적용하지 않고 contributor 기능 commit만 누적했다. |

## 범위와 판정

digest v2가 주소를 잃지 않게 절 단위 청킹과 연속 쪽 범위 창을 제공한다. 옵션 조합과 실제
조문 fixture를 계약으로 고정해, 대형 문서 탐색의 반환 단위가 기존 단일 요약 경로와 섞이지 않게 했다.

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
