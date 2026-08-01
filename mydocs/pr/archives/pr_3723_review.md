---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3723_review.md
last_verified: 2026-08-02
---

# PR #3723 검토 — 조회 전용 프로필의 세션 쓰기 경계 차단

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3723](https://github.com/edwardkim/rhwp/pull/3723) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/mcp-profile-session-boundary</code> <code>7a517d04181c824ac06c219a4ca5c630e7c336b9</code> |
| 누적 반영 | <code>ec3ef4c35633374dbb063f992f4ba30e8d1af334</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

#3629 후속으로 읽기 전용 agent profile이 세션 편집 도구까지 열던 권한 경계 이탈을
profile router·CLI·MCP 표면에서 막고 계약 회귀를 추가한다. 누적 반영에서 권한을 넓히지
않았으며 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
