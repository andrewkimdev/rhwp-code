---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3924 검토 — r30 10k 서베이와 #3900 회귀 원인 기록

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3924](https://github.com/edwardkim/rhwp/pull/3924) / @planet6897 |
| 원 head | `751ddb23e0ac73e88cd894e1da6b016783132d52` |
| 누적 적용 commit | `b89da1e50`, `64c6a55d7` (`cherry-pick -x`) |
| 기준 / 누적 branch | `upstream/devel` `21affdafc` / `review/planet6897-20260804` |

## 검토

이 PR은 r30 10k survey 결과, #3900의 PI 변화, 그리고 devel에서 유입된 두 쪽수 회귀의 원인을
보고서로 남긴다. 문서는 원인 후보를 넓은 일반화로 확정하지 않고 bisect 결과와 표적 fixture 결과를
구분한다. 코드 동작을 변경하지 않으며, 이어지는 #3926의 gate 축소 판단에 필요한 근거를 보존한다.

10k survey 자체의 실행 환경·원시 산출물은 현 작업트리에 없으므로 이 검토에서 그 대규모 측정을
재실행했다고 주장하지 않는다. 대신 보고서의 결과는 historical evidence로 취급했고, 코드 수용의
근거는 현재 branch에서 독립적으로 실행한 focused·전체 회귀와 PDF 대조로 한정했다.

## 판정

보고서의 provenance와 범위 표기가 명확하고, 코드 변경을 동반하지 않는다. 현재 누적 후보에서 전체
release-test가 종료 코드 0으로 성공했다. **통합 수용 권고.**
