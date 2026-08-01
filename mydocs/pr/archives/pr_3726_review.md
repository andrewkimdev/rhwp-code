---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3726_review.md
last_verified: 2026-08-02
---

# PR #3726 검토 — MCP split의 페이지 기준을 CLI와 일치

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3726](https://github.com/edwardkim/rhwp/pull/3726) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/mcp-split-page-base</code> <code>39687254a1c85b81b457b73043b8e4d7eb441d91</code> |
| 누적 반영 | <code>f89f88689f3b58f1918bd6a4fab01b354f11f939</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

<code>hwp_split_document</code>의 page-base가 CLI와 어긋나 한 페이지 밀린 문서를
조용히 만들던 경로를 일치시키고, CLI·agent knowledge map 문서와 계약 회귀를 함께 갱신한다.
누적 반영은 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
