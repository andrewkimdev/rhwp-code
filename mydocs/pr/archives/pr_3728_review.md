---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3728_review.md
last_verified: 2026-08-02
---

# PR #3728 검토 — MCP set-cell을 CLI 입력 계약과 동형화

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3728](https://github.com/edwardkim/rhwp/pull/3728) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/mcp-setcell-isomorphism</code> <code>3d34d8006c1b8fc6e4b4b22d901212ce87e740bb</code> |
| 누적 반영 | <code>5c91b849eb355831f2b6c1045e58895d6f3d1b4b</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

CLI가 거부하는 raw 개행·탭을 <code>hwp_doc_set_cell</code>만 통과시켜 셀에 주입하던
불일치를 제거한다. MCP도 같은 입력 문법으로 거부하도록 하고 세션 set-cell 계약 회귀를
추가했으며 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
