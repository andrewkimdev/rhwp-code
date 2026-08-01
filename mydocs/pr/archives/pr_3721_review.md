---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3721_review.md
last_verified: 2026-08-02
---

# PR #3721 검토 — 초기 누름틀 안내문 본문을 HWPX 저장에 보존

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3721](https://github.com/edwardkim/rhwp/pull/3721) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>task/3545-hwpx-pressframe-text-loss</code> <code>7ab87291b4fce63e70e6f91dbd4efe16c3610cc2</code> |
| 누적 반영 | <code>5e6c2b71093c7f1d0f3a423060b05f8cf08a6c07</code>, 처리 기록 <code>7b58ad9a502785e514818e13ca4373835ba7d087</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

#3545의 초기 상태 누름틀에서 안내문 본문 run이 HWPX 재저장 때 영구 소실되던
parser·model·serializer 왕복을 보존한다. dirty roundtrip 회귀와 처리 기록까지 함께
적용했으며, 누적 코드에서 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
