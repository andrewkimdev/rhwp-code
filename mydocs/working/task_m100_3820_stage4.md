---
kind: analysis
status: active
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-04
---

# Task #3820·#3821 Stage 4 — fidelity의 Square 그림 본문 접촉 후보화

## 기존 #3740 detector의 범위 확인

PR #3740의 Stage 23/31은 `tools/fidelity_compare/fidelity_compare.py`의
`square_wrap_text_overlap_candidates()`를 visual sweep이 직접 재사용하도록 연결했다.
이는 `Square`/`Tight`/`Through` image 폭의 절반 이상을 가로지르는 visible Body
`TextLine`이 3행 이상일 때 `square_wrap_text_overlap` 후보를 낸다. intentional
`BehindText`/`InFrontOfText`는 제외한다. 따라서 **명백히 물리 box를 가로지르는** p127
유형은 자동 후보화한다.

그러나 p156은 Stage 3 보정 전 render-tree에서 image left와 p1697 TextLine right가 모두
`429.7px`로 정확히 맞닿았다. 양의 horizontal intersection이 `0px`이므로 기존의
`image 폭 1/2 이상` 조건은 성립하지 않는다. 보정 전 tree
`output/task-3820-3821-fidelity/stage3-p156-tree/render_tree_156.json`에 현재 detector를
실행한 결과도 `[]`였다. 즉 pixel score, text owner, 기존 physical-overlap 모두 이
“outer margin 소실 → 문단과 그림의 접촉” 결함을 놓친다.

## 자동 후보 규칙

기존 public 함수와 `layout-candidates.tsv` column 이름은 호환을 유지한다. 단, 한 image의
candidate kind를 다음 둘 중 하나로 기록한다.

1. `physical_overlap`: 기존과 동일하게 image 폭 절반 이상을 가로지르는 visible Body line이
   3행 이상인 경우.
2. `edge_clearance_loss`: image의 수직 band에서, 그림 **바로 왼쪽 또는 오른쪽**의 visible
   Body line 끝이 image edge로부터 `≤1px`이고 그런 line이 3행 이상인 경우. 이미지 내부를
   넓게 교차한 경우는 1번이 우선이다.

두 경우 모두 PDF 정답 판정이 아니라 visual review 후보이다. zero-margin source가 의도된
문서는 false positive일 수 있으므로, `candidate_kind`, `edge`, `min_clearance_px`를 ledger와
sweep metrics에 남긴다. 1px는 render-tree stroke/rounding noise를 넘기지 않으면서 p156의
0px contact를 포착하는 최소 tolerance다.

## 수용 기준

- p156 보정 전 tree는 `edge_clearance_loss`, `pi=1692`, `ci=1`, 9개의 contact line 후보가 된다.
- 5px 이상 clearance를 둔 동일 synthetic tree는 후보가 아니다.
- 기존 physical overlap positive와 `InFrontOfText` negative, visual sweep bridge의 기존 API가
  계속 통과한다.
- Stage 3 post-fix p156 tree는 candidate 0이다. (수정 후 gap `6.8px`)

이 분석 문서를 커밋한 뒤에만 detector·test·README를 수정한다.
