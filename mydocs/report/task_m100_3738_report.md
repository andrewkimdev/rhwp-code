---
kind: investigation
status: active
canonical: mydocs/manual/codex/docs_and_git_workflow.md
last_verified: 2026-08-02
---

# Task #3738 결과 보고 — Stage 1–2 부분 보정 및 잔여 결함

- 이슈: [#3738](https://github.com/edwardkim/rhwp/issues/3738)
- 관련 PR: [#3740](https://github.com/edwardkim/rhwp/pull/3740)
- 상태: **미완료**

## 완료한 작업

1. 한컴 2020 MCP의 HWP/HWPX 기준 PDF를 준비하고, 그림 23의 p23–p24 페이지 소유권을
   visual sweep으로 재현했다.
2. native HWP의 빈 1×1 `RowBreak` 그림 표가 다음 문단의 `LINE_SEG` 되감기 신호를 가질 때,
   잔여 공간에 force-split하지 않고 fresh page로 이월하도록 `typeset.rs`를 보정했다.
3. HWP p23에서는 그림 23의 조기 배치가 사라진 것을 review PNG로 확인했다.
4. Stage 2에서 outer host의 page-boundary 상쇄값을 page-local picture offset으로 정규화하는
   후보를 검증했으나, 실제 셀의 `LINE_SEG.vpos`가 0임을 확인해 조건이 발동하지 않았음을 visual
   sweep과 render tree로 반증했다. 효과가 없는 후보 구현은 커밋하지 않고 증적·분석만 보존했다.

## 미해결과 다음 회차

이 보정은 완료 판정이 아니다. HWP p24에서 그림 23의 내부 picture bounds가 여전히 `y=-181.4px`로
남아 상단 잘림과 frame overflow가 남았고, HWPX p23–p24에도 독립적인 page-local anchor 차이가 있다.
상세 페이지 비교와 자동 후보는 [Stage 1 visual sweep](../working/task_m100_3738_stage1_visual_sweep.md) 및
[Stage 2 visual sweep](../working/task_m100_3738_stage2_visual_sweep.md)에 기록했다.

사용자 지시대로 이 부분 보정·분석·증적을 먼저 커밋한 뒤, 다음 회차에서는 다음을 새 문제로
분리해 조사한다.

- p24로 이월된 non-TAC 그림 표에서 outer host `vpos`가 셀 picture 배치 경로에 전달되지 않는 경로
- HWPX 저장 레이아웃에서 같은 그림 23이 HWP 기준과 다른 페이지 흐름을 쓰는 경로

## 검증 범위

- `cargo fmt`
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo build --profile release-test --bin rhwp`
- HWP 및 HWPX 각각 p23–p24, 144 DPI visual sweep — 선택 페이지 2/2 완료

전체 integration test와 215쪽 전체 raster sweep은 이 회차의 완료 근거로 사용하지 않았다. 현재
잔여 시각 결함이 있으므로, 이를 통과나 해결로 표현하지 않는다.
