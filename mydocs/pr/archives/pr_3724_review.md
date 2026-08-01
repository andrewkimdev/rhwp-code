---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3724_review.md
last_verified: 2026-08-02
---

# PR #3724 검토 — MCP 선언 인자 10건을 CLI까지 배선

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3724](https://github.com/edwardkim/rhwp/pull/3724) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/mcp-declared-args-wiring</code> <code>200bc52a8e502f19f512fa6ef8efae77f6d0e4cf</code> |
| 누적 반영 | <code>e22e60dfcd8120ce3e873e35ee0531781a46eeb1</code>, 보고서·전후 asset <code>ccf029914580e43fc50ca400a0207451cac78c75</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

MCP schema에는 선언됐지만 CLI 실행까지 닿지 않던 입력 인자 10개를 연결한다. 특히
<code>dryRun:true</code>인데 원본을 쓰던 경로를 계약으로 막고 server regression과 실측
보고서를 함께 보존한다. 누적 반영은 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
