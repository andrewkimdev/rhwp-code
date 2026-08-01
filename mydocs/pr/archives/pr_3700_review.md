---
kind: pr_review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3700 검토 — MCP batch stdin 프로토콜 경계

## 라우팅

base route: maintainer_general.md. 적용 보조 절차는 intake_and_review.md,
local_validation.md, multi_pr_update_branch.md이며, 누적 통합 PR은 #3742.

## 메타데이터와 적용 경계

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3700](https://github.com/edwardkim/rhwp/pull/3700) / @kevin9327 |
| 원 head | 287bb08db28e33f6afd7e5c8d35f5609244a9e6a |
| 원 기능 commit | c01eb77167df497e9456802b2701f41177a10e2b |
| 통합 적용 | 106b34536, 증적 cd99dc216 |
| 적용 판정 | 수용. 기능과 실측 증적을 함께 누적했다. |

## 범위와 판정

MCP 서버가 hwp_batch 자식에게 JSON-RPC stdin을 상속시키지 않게 해, 요청 프레임이 파일 경로로
소비되어 응답이 사라지는 경계를 막는다. 자식 stdin은 명시적으로 안전한 입력으로 고정되고,
MCP server contract가 정상 요청과 실패 경계를 직접 확인한다.

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
