---
kind: analysis
status: active
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-04
---

# Task #3820 Stage 11 — p166 이후 여분 페이지 anchor 추적

## 목적

사용자 직접 비교에서 p166부터 기준 PDF와 rhwp의 흐름이 달라졌다. 전체 215쪽을 처음부터
다시 산출하지 않고, p166(0-based 165)부터 마지막 기준 PDF 페이지까지의 문자열 순서 anchor와
render-tree source `(pi, line range, control)`를 함께 사용해 **해당 범위의 첫 실제 owner 이탈**을
찾는다.

## 실행 계약

1. `fidelity_compare --text-only --layout-ledger 165 214`와 필요한 개별 `export-svg -p`만 사용해
   p166 이후의 text-owner·table 후보 원장을 갱신한다. 전체 SVG 재생성은 하지 않는다.
2. PDF와 rhwp의 p166 이후 page text를 순서 보존 anchor로 비교하여, 같은 물리 번호가 아닌 첫
   순서 이탈 위치를 정한다.
3. 해당 경계의 source paragraph/LINE_SEG, 각주, 표·그림 control을 `dump-pages`와 기준 PDF로
   대조한다.
4. 보정은 이 실제 source 계약을 focused regression으로 고정한 뒤에만 적용한다.

## 완료 기준

- p166 이후 범위에서 첫 owner 이탈 및 그 직접 원인이 재현 가능하게 기록된다.
- 이전 p118·p127·p156·p168 회귀를 침범하지 않는 좁은 보정 또는 기각 근거가 남는다.
- 수정 시 기준 PDF와 직접 비교한 시각 증적과 최신 page count 원장을 남긴다.
