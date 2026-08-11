---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3720_review.md
last_verified: 2026-08-02
---

# PR #3720 검토 — MCP JSON-RPC 무응답 프레임을 오류 응답으로 고정

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3720](https://github.com/edwardkim/rhwp/pull/3720) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/mcp-jsonrpc-conformance</code> <code>140162fec6074045ce37684c694d01da3f488128</code> |
| 누적 반영 | <code>762d761aed36199d1de4eedcfe8173389edfe1a9</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

버전 협상이 빠진 JSON-RPC 요청과 비객체 프레임이 무응답으로 증발하던
<code>mcp-serve</code> 경계를 <code>-32600</code> 오류 응답으로 고정하고 서버 계약 회귀를
추가한다. 원 기능을 patch 동등하게 누적 반영했으며, 별도 보정 없이 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
