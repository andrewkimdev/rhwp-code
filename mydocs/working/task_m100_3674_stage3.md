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
