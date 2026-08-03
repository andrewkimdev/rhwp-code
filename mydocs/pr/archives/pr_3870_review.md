---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3870 검토 — 표 셀 검색 hit의 native·WASM·Studio 연결

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3870](https://github.com/edwardkim/rhwp/pull/3870) / @kevin9327 |
| 원 head · 적용 source | `a546f798b` · `8bbf7968f`, `a546f798b` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |
| 메인터너 보정 | table-cell opt-in에서 textbox·equation hit을 제외하는 명시적 type 경계 추가 |

## 검토

일반 본문뿐 아니라 표 셀의 hit을 native query, WASM DTO, Studio Find/F3 경로로 보존해 실제
셀 위치로 이동·치환하게 한다. 누적 코드 검토에서 textbox/equation hit에 `cellContext`가 있으면
Studio가 부모 셀 replace로 오인할 위험을 발견했다. `SearchHit.is_text_box`와 equation filter로
ordinary table-cell text만 opt-in으로 한정해, 새 기능과 기존 textbox/equation 경로를 분리했다.

## 누적 검증과 판정

`issue_3865_search_text_in_table_cells` 4/4(신규 textbox 음성 회귀 포함), full release-test
`exit-0`, `npx tsc --noEmit`, Studio test 749/749, WASM build `exit-0`을 확인했다.
renderer 출력 변경이 아닌 search/navigation DTO 변경이므로 별도 PDF visual sweep 대상은 아니다.
**통합 수용 권고.**
