---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3864 검토 - #3798 쪽 끝 trailing spill 반증 기록

## 접수와 적용

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3864](https://github.com/edwardkim/rhwp/pull/3864) / @planet6897 |
| 관련 이슈 | [#3798](https://github.com/edwardkim/rhwp/issues/3798) |
| 원 head / 적용 | 2142c3eb0e2474de34b795105242554706755fe7 / 75937d2db |
| base / 작성 시점 상태 | devel / MERGEABLE, BEHIND |
| 규모 / 충돌 | 935 additions, 11 deletions, 6 files / 없음 |

이 PR은 renderer 수정을 주장하지 않는다. page-end trailing spacing을 제한하는 초기 가설을
fixture, 실험 patch, 비교표로 반증하고, 미해결 behavior를 ignored regression으로 보존한다.

## 검증과 범위

- issue_3798_page_end_trailing_spill: fixture shape 1 / 1 통과, 해결을 주장하는 test는
  명시적으로 ignored 상태임을 확인했다.
- trimcap_experiment.patch는 git apply --check를 통과했다.
- report의 결론은 해결이 아니라 순이득 음수로 해당 접근을 접고 후속 원인 분석을 유지하는 것이다.
- renderer code 변경이 없으므로 기준 PDF fidelity 개선을 주장하거나 visual pass로 표기하지 않는다.

## 판정

**누적 통합 수용.** 미해결 결함을 green test로 위장하지 않고 반증 자료·재현 fixture·ignored
회귀를 분리해 다음 해결 작업의 안전한 출발점을 제공한다.

