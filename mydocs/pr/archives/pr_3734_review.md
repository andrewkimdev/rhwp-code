---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3734_review.md
last_verified: 2026-08-02
---

# PR #3734 검토 — 남은 diagnostic 명령의 종료 코드와 batch 보정

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3734](https://github.com/edwardkim/rhwp/pull/3734) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/diagnostics-exit-rest</code> <code>e9a7e6722f457a86124079dfe64a53f3bb069bba</code> |
| 누적 반영 | <code>35ffe2e2168872d37a2687705c5212090661cc8c</code>, 공유 maintainer 보정 <code>f0b96e87bebfcd8c93562222319a0693e4c0a89c</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

<code>core-pages</code>, <code>measure-width</code>, <code>bench</code>,
<code>test-shape</code>, <code>gen-table</code>의 실패 종료 코드 계약을 고정한다.
통합 audit는 <code>bench --batch</code>가 unreadable root·subdir·entry를 다른 파일이
있으면 조용히 성공으로 끝내는 문제를 찾아 공유 maintainer commit에서 runtime exit 1로
전파하도록 보정했다. <code>core-pages</code>의 관찰된 예외 경로는 정상 CLI에서 재현 불가여서
근거 없는 추가 수정은 하지 않았다. 결합 결과는 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
