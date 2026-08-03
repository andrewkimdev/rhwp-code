---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3727_review.md
last_verified: 2026-08-02
---

# PR #3727 검토 — MCP 입력 검증 3건으로 조용한 오답 차단

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3727](https://github.com/edwardkim/rhwp/pull/3727) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/mcp-arg-validation</code> <code>fe3f1a1fc25cc23864b3c0e9ed9fb1cbafedc294</code> |
| 누적 반영 | <code>ffeaa00e310a08aab4171a3abdb932813529fed7</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

<code>page:-1</code>을 문서 전체 성공으로 해석하던 입력과 <code>-</code>로 시작하는
검색어가 옵션으로 오인되던 경로 등 세 검증 누락을 명시 오류로 바꾼다. CLI·MCP 경계의
실제 계약을 회귀로 고정했으며 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
