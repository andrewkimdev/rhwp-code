---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3731_review.md
last_verified: 2026-08-02
---

# PR #3731 검토 — capabilities 자기서술 드리프트와 통합 보정

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3731](https://github.com/edwardkim/rhwp/pull/3731) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/capabilities-drift</code> <code>08ef9bb86aa08c7db44859cd1b164ece9c4dad68</code> |
| 누적 반영 | <code>2d4c5d57e9a59bcf2bd65ac2a7ce6191833844ee</code>, maintainer 보정 <code>a0e548e17c6f72bd93417a130813454a85c80308</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

CLI가 지원하지 않거나 자기서술이 없는 필드를 capabilities가 광고하던 5개 드리프트를
정리하고 재발 방지 guard 3종을 추가한다. 누적 audit에서 batch feature flag 선언이 다시
실제 capability 표면과 어긋난 것을 발견해 maintainer commit으로 동기화했다. 이는 기능
확장이 아니라 advertised contract를 실제 CLI와 맞춘 보정이며, 결합 결과를 **수용 후보**로
판정한다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
