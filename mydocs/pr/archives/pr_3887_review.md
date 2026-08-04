---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3887 검토 — agent preflight 검사 범위 확장

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3887](https://github.com/edwardkim/rhwp/pull/3887) / @kevin9327 |
| 원 head | `c5cfba6cda5754e477ddda55367f2b7e31b50f9b` |
| 누적 적용 commit | `8b4461124` (`cherry-pick -x`) |
| 기준 / 누적 branch | `upstream/devel` `ad67e5a63` / `review/kevin9327-20260804` |

## 검토

기존 세 경로에 고정됐던 preflight를 60여 internal command의 실제 계약으로 넓혀, 선언되지 않은
명령과 미지 flag의 침묵 무시를 자동으로 찾게 한다. 이 확장은 실제 누적 검토에서 6개 internal
diagnostic command가 미지 flag를 정상 종료로 처리하는 결함을 발견했다.

원 PR의 탐지 범위와 provenance는 보존했다. 별도 메인터너 보정 `fb33dfd72`는 runner 예외 설명과
공개 diagnostics flag 계약을 맞췄고, `45e53d42e`는 나머지 internal command의 flag-like positional,
option value, 잘못된 수치 입력을 명시적으로 거절하도록 했다. 이제 preflight의 30개 command 검사가
성공한다.

## 판정

focused CLI 계약 28건, preflight, 전체 release-test가 최신 누적 후보에서 성공했다. **통합 수용 권고.**
