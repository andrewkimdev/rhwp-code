---
kind: investigation
status: in_progress
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-08
---

# Task #3820 Stage 62 — issue2007 p14 중간 block 경계 간격

## 문제

Stage 60·61의 clip 및 제목 뒤 저장 간격 보정 뒤에도
`samples/basic/issue2007_nested_cell_pagination_42065.hwp`의 물리 p14에서
`제51조(벌칙)` 표 조각과 `6 금융위원회` block 사이의 세로 배치가 한컴오피스
2020 기준 PDF와 다르다. 전체 PR 게이트는 이 차이를 닫을 때까지 보류한다.

기준 PDF는 `pdf/basic/issue2007_nested_cell_pagination_42065-2020.pdf`이며,
Stage 61의 144dpi render tree와 PDF text 좌표를 96dpi CSS 좌표로 정규화했다.

| 기준점 | rhwp y | PDF y | rhwp - PDF |
|---|---:|---:|---:|
| `제51조(벌칙)` text top | `343.6px` | `351.1px` | `-7.5px` |
| 마지막 `제27조제4항을 위반한 자는` text top | `447.6px` | `454.7px` | `-7.1px` |
| `6 금융위원회` text top | `524.7px` | `497.6px` | `+27.1px` |
| `자본시장과 금융투자업에 관한 법률` text top | `554.3px` | `535.9px` | `+18.4px` |

마지막 벌칙 줄 bottom과 다음 제목 top의 공백은 PDF 약 `29.1px`인 반면 rhwp는
약 `63.8px`다. 단순히 제목 뒤 `780HU`를 다시 조정할 문제가 아니라, 앞 재귀 표
조각의 끝과 다음 separator/title owner 사이에서 약 `34.7px`가 추가된 상태다.

## 기존 수정과의 경계

- Stage 60의 p14 항목 ⑦ ancestor clip 확장과 `한다.` 표시를 유지한다.
- Stage 61의 p12·p15 제목 뒤 `780HU` 저장 간격을 유지한다.
- pagination cursor나 source page owner를 임의로 이동하지 않는다.
- p14의 앞 조각, 빈 separator, 제목, 뒤 1×1 표 각각의 source `LINE_SEG`
  좌표와 현재 `CellUnit` 범위를 확인한 뒤 가장 좁은 계약만 수정한다.

## 완료 조건

- p14 마지막 벌칙 줄→`6 금융위원회` 제목 간격이 PDF 좌표와 허용 오차 안에서 일치
- p14 제목→금융위원회 표 저장 간격과 항목 ⑦ 전체 clip 유지
- p15가 항목 ⑧부터 시작하고 전체 17쪽 유지
- p12·p14·p15 focused 회귀와 p14 PDF 직접 대조 통과
- 수정 결과와 before/after 증적을 이 문서에 기록하고 커밋한 뒤 다음 Stage에서
  전체 PR 게이트를 처음부터 실행
