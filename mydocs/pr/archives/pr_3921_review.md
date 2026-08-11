---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3921 검토 — WMF 음수 StringLength 패닉 차단

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3921](https://github.com/edwardkim/rhwp/pull/3921) / @kevin9327 |
| 원 head | `96161b03e9b31eae9d9893390002ae92415bf179` |
| 누적 적용 commit | `85b06f958` (`cherry-pick -x`) |
| 현재 PR 기준 / 누적 branch | `upstream/devel` `874dae394` / `review/kevin9327-20260804` |

## 검토

신뢰할 수 없는 WMF의 `META_TEXTOUT`·`META_EXTTEXTOUT` signed `StringLength`가 음수일 때
`usize`로 확장돼 allocation panic으로 이어지던 DoS 경계를 두 레코드 모두에서 `Err`로 바꾼다.
`META_TEXTOUT`의 홀수 길이 alignment 계산도 먼저 `usize`로 변환해 `0x7fff` signed overflow를
피한다. 음수 세 경우와 정상 길이 대조를 한 focused 회귀가 두 분기를 함께 고정한다.

## 판정

파서 안전성 보정이며 렌더링 결과·fixture를 변경하지 않는다. focused 회귀 3건과 전체 누적 회귀
종료 코드 0을 확인했다. **통합 수용 권고.**
