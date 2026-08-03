---
kind: pr-review
status: active
canonical: mydocs/pr/archives/pr_3732_review.md
last_verified: 2026-08-02
---

# PR #3732 검토 — gen-pua 보존성 및 CLI 종료 코드 계약

## 라우팅과 원본

외부 contributor 누적 체리픽 검토 경로(<code>intake_and_review</code>,
<code>local_validation</code>, <code>multi_pr_update_branch</code>)로 처리한다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3732](https://github.com/edwardkim/rhwp/pull/3732) / @kevin9327 |
| 원 base / head | <code>devel</code> / <code>pr/cli-exit-contract</code> <code>31b9f01938b9d6607d2f15ab2e3545178c6f7ae5</code> |
| 누적 반영 | <code>7fc39d63cb3d5b806bba0c8a7c322084867cfc10</code> |
| 통합 후보 | [#3742](https://github.com/edwardkim/rhwp/pull/3742) |

## 범위와 처분

조사용 <code>gen-pua</code> 호출이 원본을 덮어쓰던 보존성 위반과
<code>test-field</code> panic 등 종료 코드 계약 3건을 고친다. diagnostics error
분류와 CLI contract 회귀를 통해 성공·실패 표면을 고정했으며 **수용 후보**다.

## 누적 검증과 merge 전 조건

- 누적 branch에서 <code>CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile release-test --tests</code>는 최종 exit code 0이며, <code>cargo fmt --all -- --check</code>, <code>cargo clippy --all-targets -- -D warnings</code>, <code>git diff --check</code>도 통과했다.
- code candidate <code>b1e9619433bd9f068a361ddfb42ea0138f0077d1</code>의 [Actions run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)는 전체 성공이다.
- 이 기록은 docs-only tail에 들어간다. 실제 merge 전 최신 tail head의 preflight와 <code>Build & Test</code> aggregate fast-pass, <code>CLEAN</code>·<code>MERGEABLE</code> 재확인이 필요하다.
