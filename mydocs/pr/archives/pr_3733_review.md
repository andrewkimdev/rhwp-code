---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3733_review.md
last_verified: 2026-08-02
---

# PR #3733 검토 — HWP5 probe 8종의 실패 exit code와 stdout 경계

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3733](https://github.com/edwardkim/rhwp/pull/3733) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/diagnostics-exit-codes</code> <code>8e04861a65d99fcdc5adcef8d254b2ef5b7d3d6b</code> |
| 누적 반영 | <code>92d5c9cff3a37efad4ef6ab498a7b9a6330992d5</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

<code>hwp5-*</code> probe 8종이 실패에도 exit 0을 내거나 stdout을 오염시키던
diagnostic 경계를 정리한다. 각 probe의 error propagation과 CLI exit-code regression을
누적 반영했으며 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
