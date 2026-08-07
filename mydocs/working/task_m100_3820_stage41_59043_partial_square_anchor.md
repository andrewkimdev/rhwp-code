---
kind: analysis
status: in_progress
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-07
---

# Task #3820 Stage 41 — #1921 p11 첫 partial fragment의 Square anchor

## Stage 40 이후의 범위 재설정

Stage 40의 vpos-only flow 대체는 39→38쪽으로 줄였지만 PDF p11의 내용 자체를 훼손해 채택하지
않았다. 이 Stage는 그 변경을 되돌린 상태에서, p11의 **첫** p98 row 2 fragment가 왜 source picture를
자기 cell보다 위에 그리는지를 먼저 분리한다.

## 확정 관측

- p11 fragment의 cursor는 `start_cut=[0]`, `end_cut=[29]`이다. 즉 이 row 2의 앞 fragment는
  이전 page에서 소비한 unit이 없고, continuation origin을 적용할 이유가 없다.
- current render tree에서 physical row 2 cell bbox는 `y=351.9..843.9`이다.
- 같은 cell에 속한 Square image bbox는 `y=123.3..214.8`, `y=332.7..574.2`다. 첫 image는 cell
  위로 228px 이상 빠져 row 0의 동영상/소개 image와 겹친다.
- PDF p11에서는 이 row의 product pictures가 row 2 cell 안에 있다. 따라서 p12/p13의 page-owner
  지연은 먼저 발생한 p11 anchor failure의 누적 결과이며, p12만 억지로 앞당겨서는 해결되지 않는다.

## 코드 경로

이 형상은 일반 `layout_table_cells`가 아니라 `layout_partial_table_cells`를 탄다. 해당 경로는
`cut_units`로 visible paragraph를 선택한 뒤 Square picture에 `anchor_y = para_y`를 주고
`compute_object_position`에 전달한다. `CellUnit`의 generic flow unit을 바꾸는 것만으로는 이 p11
anchor가 고쳐지지 않는 이유다.

## 검증 계획

1. `RHWP_DIAG_CELLPIC` 일시 진단으로 p0/p6의 `para_y_before`, `anchor_y`, vertical align/offset,
   `compute_object_position` 결과와 physical cell bbox를 기록한다.
2. 첫 fragment(`su=0`)에만 적용할 수 있는 origin/clip 오류인지, object vertical-align 해석 오류인지
   구분한다.
3. 원인에 대응하는 최소 수정 후 p11 row2 containment gate를 통과시킨다.
4. 그 다음에만 p12--p13 picture owner와 PDF page content를 다시 대조한다.

진단은 환경변수에 한정하고, 정답 PDF와 맞지 않는 page-count 감소는 성공으로 취급하지 않는다.
