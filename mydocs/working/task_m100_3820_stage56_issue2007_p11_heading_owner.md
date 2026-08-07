---
kind: investigation
status: in_progress
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-08
---

# Task #3820 Stage 56 — issue2007 p11→p12 제목 소유권

## 목적

PR 후보 `829d6420e`에서 다시 발견된
`issue2007_nested_cell_pagination_42065.hwp` p11→p12 경계를 한컴 2020 PDF와 맞춘다.
Stage 55의 PR 준비는 이 결함을 해결하고 정확한 새 HEAD를 검증할 때까지 보류한다.

## 재현과 정답지

- 입력: `samples/basic/issue2007_nested_cell_pagination_42065.hwp`
- 기준: `pdf/basic/issue2007_nested_cell_pagination_42065-2020.pdf`
- 기준 등급: Hancom Office 2020 PDF oracle
- 현재/기준 쪽수: 17/17
- 비교 범위: p11–p12, 144dpi

PDF p11은 `국세기본법`의 마지막 문장으로 끝나고, p12가
`3 중앙선거관리위원회`로 시작한다. 현재 rhwp는 제목과 다음 점선 표 상단선을 p11 하단에
미리 그리며 p12를 `공직선거법` 본문부터 시작한다. 따라서 쪽수 정합만으로는 통과시킬 수
없는 source-owner 회귀다.

## 구조 증거

- 바깥 continuation: `PartialTable(pi=7, ci=1)`
- p11 cut: `[168] -> [226]`
- p12 cut: `[226] -> [271]`
- p11에 잘못 남은 제목: source paragraph `pi=89`, `y=978.1`, `h=17.3`
- 제목 직후 다음 중첩 표 상단 조각: `y=997.3`, `h=3.8`
- p11 visual accuracy proxy: `6.63872%`
- p12 visual accuracy proxy: `6.25419%`

`fidelity_compare --layout-ledger`는 p11의 `table_footer=1`과 p11→p12의 동일 source 표
fragment를 후보로 잡았다. 반면 `visual_sweep`의 구조 heuristic은 두 쪽을 `flagged=0`으로
놓쳤다.

## 기존 회귀의 오판

`issue_2007_single_cell_continuation_does_not_repaint_boundary_fragments`는 p12에서
`contains_painted_text(..., "중앙선거관리위원회")`를 검사한다. p12 본문의
`중앙선거관리위원회규칙`도 substring으로 일치하므로 실제 제목이 없어도 통과한다.

회귀는 다음 exact owner 계약으로 바꾼다.

1. p11에는 trim한 정확한 TextRun `중앙선거관리위원회`가 없어야 한다.
2. p12에는 trim한 정확한 TextRun `중앙선거관리위원회`가 있어야 한다.
3. 기존 p12 선행 문장 비재도색 및 p16→p17 소유권 계약은 유지한다.

exact helper와 p11 negative assertion을 먼저 추가한 뒤 focused test를 실행했다. 기존
구현에서는 다음과 같이 의도대로 실패했다.

```text
test issue_2007_single_cell_continuation_does_not_repaint_boundary_fragments ... FAILED
p11 must not paint the p12-owned heading after an explicit page break
```

따라서 새 회귀는 기존 substring 오탐을 제거하고 실제 결함을 재현한다.

## 원인 분석

source의 제목 앞 `cp88`은 단순 여백이 아니라 `ColumnBreakType::Page`인 명시적 쪽
나누기 문단이다. 순서는 `cp87` 국세청 중첩 표, `cp88` 빈 쪽 나누기 문단, `cp89`
중앙선거관리위원회 제목, `cp90` 공직선거법 중첩 표다.

일반 body typesetter는 문단의 `ColumnBreakType::Page`를 강제 쪽 경계로 처리한다. 그러나
`cell_units_uncached`는 셀 문단의 `column_type`을 확인하지 않고 저장 vpos reset만으로
`hard_break_before`를 만든다. 게다가 Task #1488의 빈 overlay 보호 조건이 비가시 빈 문단의
reset을 제거한다. 그 결과 명시적 쪽 나누기인 `cp88`도 장식용 빈 overlay처럼 접히며,
`cp89`와 `cp90`의 3.76px sliver가 p11에서 소비된다.

이 결함은 기존 `RecursiveBlockPreludeRole`의 fit 보정 이전에 지켜야 할 명시적 source
경계를 잃은 문제다. 명시적 Page/Section break를 일반 vpos reset과 별도 strict metadata로
CellUnit 및 재귀 mixed projection에 보존해야 한다.

## 기각한 가정

셀의 명시적 Page/Section 표식을 별도 strict 경계로 전파하는 첫 구현을 시험했다. 새 단위
회귀와 #1488은 통과했지만 issue2007 전체 쪽수가 17에서 19로 늘고 p15–p17의 기존 PDF
소유권 계약이 깨졌다. 1×1 RowBreak로 범위를 좁혀도 결과는 같았다. 이 표식은 저장된
물리 프레임 원장과 중복되어 새 물리 쪽을 추가하므로 채택하지 않는다.

## 구현 계획

실제 p11 walk는 separator와 제목뿐 아니라 다음 재귀 block의 3.76px 첫 prefix까지 현재
쪽에 넣은 뒤, 그 다음 재귀 unit에서 overflow한다. 기존 prelude rewind는 pending unit
바로 앞이 제목일 때만 작동해 이 경우를 놓친다.

1. pending unit에서 뒤로 연속된 `recursive + nontrailing + role=None` prefix를 건너뛴다.
2. 가장 가까운 `OneLineHeadingBeforeSingleCellTable`과 바로 앞 `EmptySeparator`를 요구한다.
3. separator부터 이미 들어간 재귀 prefix까지 함께 되감는다.
4. hard break·trailing·비재귀 unit을 넘지 않아 일반 문단과 다른 표에는 적용하지 않는다.

수정은 파일명·페이지·문구 특례 없이 source 구조와 실제 fit만 사용한다. focused 회귀를
먼저 실패시키고 최소 구현 보정 후 p11–p12 PDF 대조를 다시 수행한다.

## 증적

- [p11 review before](../pr/assets/task_m100_3820_stage55_pr_readiness/review_p011_before.png)
- [p12 review before](../pr/assets/task_m100_3820_stage55_pr_readiness/review_p012_before.png)
- [overlay metrics before](../pr/assets/task_m100_3820_stage55_pr_readiness/overlay_metrics_before.json)
- [layout ledger before](../pr/assets/task_m100_3820_stage55_pr_readiness/layout_candidates_before.tsv)
