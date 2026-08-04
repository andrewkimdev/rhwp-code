---
kind: working
status: active
canonical: mydocs/plans/task_m100_3674.md
last_verified: 2026-08-04
---

# Task #3674 Stage 3 보고 — 후보 1 구현 (진행 중)

계측 단계(3-1)는 [stage2 8~11보](task_m100_3674_stage2.md)에 기록됨. 본 문서는
승인된 **후보 1**(RowBreak 다행 표의 declared 선점 이월을 분할 스캔 진입으로 교정)
구현 단계를 기록한다.

## 승인된 작업 순서

1. `saved_anchor_splits_here` 정의부 판독 → 되감김 형상(다음 문단 stored vpos 되감김)
   인지 추가
2. red-check — 수정 되돌리면 pi=14 재이월
3. 구역 10 쪽 시작 서열 재계측 → 한컴(11쪽) 수렴 확인
4. release-test 전체 + 10k 서베이 회귀 게이트
5. 시각 스왑 세트 B쌍 재생성 → 작업지시자 시각 판정

## 진행 기록

(이하 순서대로 추가)
### 1. 구현 (2026-08-04)

`saved_anchor_splits_here`(typeset.rs ~18067)에 **되감김 형상**을 OR 로 추가:

- `saved_next_para_rewind_tail`: row_count>1 · RowBreak · 다음 문단 첫 실측 seg vpos 가
  현재 앵커보다 되감김 · 꼬리 px < **table_total**(실측 표 높이).
- 앵커 심도 >100px(한컴이 현재 쪽에 남긴 증거) + 흐름 정합 허용 48px(누적 드리프트).
- 1×1 그림 표(#874·#2439)는 row_count>1 로 배제. 기존 형상(#2097)은 불변.

**구현 중 결함 1건 자체 검출·정정**: 첫 판에서 꼬리를 `declared_total`(최소 셀합,
175.2px)과 비교해 항상 탈락(꼬리 376px). 실측 `table_total`(554.6px) 비교로 정정 —
393→392(잘못된 판)→**391**(정정 후).

### 실측 (재저장본)

| | 수정 전 | 수정 후 |
|---|---|---|
| 쪽수 | 393 | **391** (−2, 한컴 383 방향) |
| pi=14 | fit 분기 cur_h=0(선이월) | **PRESPLIT 도달**(remaining 174.1, blk 81.1) |

