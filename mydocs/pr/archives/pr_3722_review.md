---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3722_review.md
last_verified: 2026-08-02
---

# PR #3722 검토 — 음수 HWPX offset의 u32 wraparound 방출

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3722](https://github.com/edwardkim/rhwp/pull/3722) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>task/3544-hwpx-unsigned-coords</code> <code>2af7d5c7ffd92511d9682e75ba690ad90fd1c7e8</code> |
| 누적 반영 | <code>3eb9191a07f9584d8fb182fd4a6185591cdf162a</code>, maintainer baseline 보정 <code>c3f235ea6b7cbdec0775939c8e6d141fccbb103f</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위·baseline A/B·처분

음수 <code>hp:offset</code>을 XML 부호 문자열로 내보내지 않고 HWPX unsigned 좌표 계약의
u32 wraparound로 직렬화한다. 새 실제 HWPX 회귀가 이 계약을 고정한다.

원 PR의 IR field sweep은 전역 <code>MAX_DIVERGENCES=2000</code> 상한 때문에 처음에는
baseline 변경처럼 보였다. 현재 후보 A는 805 samples, 588 paths, 110,707 rows이고, 수정
revert B는 611 paths, 110,738 rows다. offset 결함 행 23개가 사라져 기존 cap 뒤에 가려졌던
<code>raw_header</code> 4건이 드러난 것이며, baseline은 무비판적으로 덮지 않고 4 additions /
27 deletions로 재생성했다. 따라서 A/B는 #3722의 의도된 관측면 변경을 설명하며, **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
