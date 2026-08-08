---
kind: investigation
status: in_progress
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-08
---

# Task #3820 Stage 60 — issue2007 p14 재귀 셀 clip 소실

## 문제

`samples/basic/issue2007_nested_cell_pagination_42065.hwp`의 rhwp 물리 p14 하단에서
금융위원회 항목 ⑦의 마지막 두 줄이 잘린다. 한컴오피스 2020 PDF는
`관계자에게 내보여야`와 `한다.`를 모두 p14에 표시하고 p15는 항목 ⑧로
시작한다.

수정 전 증적:

- [p14 rhwp·PDF·overlay](../pr/assets/task_m100_3820_stage60_issue2007_p14_ancestor_clip/review_p014_before.png)
- 원본: `samples/basic/issue2007_nested_cell_pagination_42065.hwp`
- 기준: `pdf/basic/issue2007_nested_cell_pagination_42065-2020.pdf`
- 재현: Stage 59 release-test binary로 `scripts/visual_sweep.py --pages 14-15 --dpi 180`

## 정확한 소실 범위

- p14 outer `PartialTable(pi=7,ci=1)` cursor: start `[282]`, end `[331]`
- p15 cursor: start `[331]`; 항목 ⑧이 소유
- p14 leaf 항목 ⑦ 첫 줄 bbox: `969.7..983.0px`
- p14 leaf `한다.` bbox: `987.0..1000.3px`
- leaf 금융위원회 cell clip bottom: `1006.213px`
- 중간 wrapper cell clip bottom: `979.067px`
- outer wrapper cell clip bottom: `974.947px`
- body clip bottom: `1009.120px`

따라서 page owner나 body 용량은 정상이고, 재귀 wrapper clip만 현재 자식 조각보다
약 31px 짧다. 첫 줄은 약 8px이 잘리고 `한다.`는 전부 잘리며, p15에서
재방출되지 않으므로 실제 콘텐츠 소실이다.

## 원인

Stage 48의 recursive child viewport 보정은 자식 `RowCut`에 속한 마지막 줄을
render tree에 복원했지만, 부모 flow height는 기존 `flow_visible`을 유지했다.
`expand_terminal_cell_clip_to_nested_table_descendants()`는 host가 terminal일 때만 셀 clip을
늘리므로, p14처럼 nonterminal host 안에 명시적 `recursive_cut`으로 소스 범위가
제한된 현재 자식 조각도 조상 clip에서 잘린다.

기존 `contains_painted_text()`는 text bbox와 clip이 조금이라도 교차하면 성공하고,
아예 clip 밖인 `한다.`를 검사하지 않아 이 결함을 놓쳤다. 기존
`LAYOUT_OVERFLOW_CELL`도 page bottom 초과를 보므로 page 안의 ancestor clip 소실은
검출하지 못한다.

## 수정 원칙

1. pagination cursor와 `flow_height`는 바꾸지 않는다.
2. nonterminal cell 전체를 무조건 확장하지 않는다.
3. `recursive_cut`이 source start/end를 명시적으로 제한한 현재 호출이 새로
   붙인 child subtree만 측정해 해당 조상 cell clip에 포섭한다.
4. scalar nonterminal child와 다음 쪽 source tail은 계속 숨긴다.
5. p14 항목 ⑦의 두 줄이 모든 조상 clip 안에 **전체** 포함되는지 회귀로
   고정하고, p15가 여전히 항목 ⑧로 시작함을 함께 검사한다.

## 완료 조건

- 수정 전 회귀는 `한다.` bottom `1000.3 > 974.947` 로 실패
- 수정 후 p14 항목 ⑦ 두 줄과 하단 frame이 모든 ancestor clip 안에 존재
- p15 항목 ⑧ 소유권과 17쪽 수 유지
- issue2007 focused 회귀와 p14-p15 PDF 재대조 통과
- 수정 커밋 후 새 PR gate stage에서 전체 회귀를 처음부터 순차 실행
