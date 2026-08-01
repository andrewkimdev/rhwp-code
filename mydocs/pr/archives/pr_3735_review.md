---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3735_review.md
last_verified: 2026-08-02
---

# PR #3735 검토 — batch convert 축과 누적 CLI 경계 보정

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3735](https://github.com/edwardkim/rhwp/pull/3735) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/batch-convert-axis</code> <code>68eafdb8a3fea5f23518c8f01ddb1940dcb1f830</code> |
| 누적 반영 | <code>9b78e156e9502e6d984733cf949d69b2ccd575e5</code>, 공유 maintainer 보정 <code>f0b96e87bebfcd8c93562222319a0693e4c0a89c</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

#3626의 폴더 일괄 convert·verify NDJSON 축을 누적 반영한다. 통합 audit에서 batch 호출이
global password stdin을 먼저 소비하거나 worker credential을 무시하고, <code>--out-dir</code>의
옵션값 오인·case collision이 output을 만들 수 있는 경계를 발견했다. 공유 maintainer 보정은
batch의 global auth를 명시 거부하고, out-dir 형태와 collision을 output 생성 전에 검증하며
계약 회귀를 추가한다. 기능 범위를 넓히지 않는 안전 보정이므로 결합 결과는 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
