---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3729_review.md
last_verified: 2026-08-02
---

# PR #3729 검토 — 세션 save의 IR 변경을 snapshot 저장으로 차단

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3729](https://github.com/edwardkim/rhwp/pull/3729) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/mcp-save-snapshot</code> <code>2552619e9b9ded84f632dfb5fcf40b4609266510</code> |
| 누적 반영 | <code>a4c4ab19ebfaf425de302f56740b8c744e7ff419</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

<code>hwp_doc_save</code>가 열린 handle의 IR을 바꾸어 한 번 저장할 때 본문 16줄을
잃을 수 있던 세션 편집 결함을 snapshot 저장으로 바꾼다. handle 상태와 저장 결과를
분리하는 contract 회귀가 있으므로 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
