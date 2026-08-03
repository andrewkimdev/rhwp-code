---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3874 검토 - HWP5-origin HWPX stored line-start profile

## 접수와 적용

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3874](https://github.com/edwardkim/rhwp/pull/3874) / @planet6897 |
| 관련 이슈 | [#3837](https://github.com/edwardkim/rhwp/issues/3837) |
| 원 head / 적용 | bd06df336a486ef93858272d8160d84f1a5d69b6 / 51516daf8 |
| base / 작성 시점 상태 | devel / MERGEABLE, BEHIND |
| 규모 / 충돌 | 108 additions, 1 deletion, 2 files / 없음 |

HWP5에서 내보낸 HWPX도 HWP5 stored line-start profile을 사용하게 해, 저장된 줄 시작
좌표의 권위를 HWP5-origin 경로에서 잃지 않게 한다. original HWPX는 기존 경로를 유지한다.

## maintainer 보정과 검증

원 PR에는 작은 직접 fixture가 없어 maintainer commit a4ebe461e에서 profile predicate를
uses_hwp5_stored_line_start_profile로 분리하고 native HWP5, HWP5-origin HWPX, original HWPX의
세 경계를 unit test로 고정했다.

- hwp5_origin_hwpx_keeps_the_hwp5_stored_line_start_contract 1 / 1 통과.
- issue_1891 HWP5-origin HWPX export/reparse page count 1 / 1 통과.
- issue_1939 strict render-diff self consistency 1 / 1 통과.
- 2022년 국립국어원 업무계획 HWP→HWPX glyph 비교: 35/35쪽, 23,994 glyph, 차이 0.

기여자가 언급한 81MB 원 재현본과 한컴 기준 PDF는 PR에 제공되지 않아 그 문서와의 직접
fidelity 주장은 하지 않는다. 위 검증은 profile 경계와 기존 HWP5-origin fixture의
자기정합 근거다.

## 판정

**maintainer 보정을 포함해 누적 통합 수용.** 적용 범위를 HWP5-origin으로만 제한하고
original HWPX를 제외한 경계가 명시적으로 회귀 보호된다.

