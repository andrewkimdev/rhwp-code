---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3736_review.md
last_verified: 2026-08-02
---

# PR #3736 검토 — HWP3 왕복본의 미주 페이지 흐름 보존

## 라우팅과 원본

이 PR은 @kevin9327 묶음과 다른 작성자지만, 작업지시자가 명시한 cutoff에 따라 같은 누적
검토 branch에 포함했다. 외부 contributor 누적 체리픽 및 renderer/typeset 보조 경로
(<code>intake_and_review</code>, <code>local_validation</code>,
<code>multi_pr_update_branch</code>, <code>visual_fixture_evidence</code>)를 적용한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3736](https://github.com/edwardkim/rhwp/pull/3736) / @planet6897 |
| 원 base / head | <code>devel</code> / <code>fix/3707-hwp3-roundtrip-endnote-columns</code> <code>df041c1c0adf87e005fef428cbaa0b31553fbc7c</code> |
| 누적 반영 | <code>527e0368f3ce3f1729a60e17844a981cc8ba617e</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

#3707의 HWP3 왕복이 page-break allowance를 잃어 미주가 뒤로 밀리던 converter·parser·typeset
결함을 고치고 전용 roundtrip endnote-columns 회귀를 추가한다. author는 @planet6897로
분리해 credit을 보존한다. page flow에 영향이 있으므로 최종 merge 전 visual fixture 기록의
적용 범위와 안정 증적을 함께 재확인하는 조건으로 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code>, 필요한 visual evidence 재확인이 필요하다.
