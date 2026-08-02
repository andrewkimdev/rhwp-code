---
kind: investigation
status: active
canonical: mydocs/manual/codex/docs_and_git_workflow.md
last_verified: 2026-08-02
---

# Task #3738 결과 보고 — Stage 1–7 HWP/HWPX 그림 23 흐름 보정

- 이슈: [#3738](https://github.com/edwardkim/rhwp/issues/3738)
- 관련 PR: [#3740](https://github.com/edwardkim/rhwp/pull/3740)
- 상태: **조사 범위 해소 — HWP p23–p24 및 HWPX p13–p15·p23–p24 그림 흐름을 기준 PDF와 대조해 복원**

## 완료한 작업

1. 한컴 2020 MCP의 HWP/HWPX 기준 PDF를 준비하고, 그림 23의 p23–p24 페이지 소유권을
   visual sweep으로 재현했다.
2. native HWP의 빈 1×1 `RowBreak` 그림 표가 다음 문단의 `LINE_SEG` 되감기 신호를 가질 때,
   잔여 공간에 force-split하지 않고 fresh page로 이월하도록 `typeset.rs`를 보정했다.
3. HWP p23에서는 그림 23의 조기 배치가 사라진 것을 review PNG로 확인했다.
4. Stage 2에서 outer host의 page-boundary 상쇄값을 page-local picture offset으로 정규화하는
   후보를 검증했으나, 실제 셀의 `LINE_SEG.vpos`가 0임을 확인해 조건이 발동하지 않았음을 visual
   sweep과 render tree로 반증했다. 효과가 없는 후보 구현은 커밋하지 않고 증적·분석만 보존했다.
5. Stage 3에서 outer host의 first stored `LINE_SEG.vpos`를 root-body table 호출에서만 cell picture
   배치까지 전달했다. `52230 + 560 - 52790 = 0 HU`인 native HWP5 RowBreak 형상에만 picture를
   page-local content top으로 정규화해 HWP p24 Image bbox를 `-181.4px`에서 `92.5px`로 복원했다.
6. Stage 4에서 셀 안 non-inline picture의 Bottom caption을 기존 picture frame 및 caption spacing으로
   배치하고, 같은 native HWP5 empty `RowBreak` 형상에서만 table paint geometry를 유지한 채 다음 저장
   `LINE_SEG`로 outgoing flow cursor를 재설정했다. HWP p24에서 3줄 caption, `○ EU에서 …`, 표 4와
   후속 본문이 기준 PDF의 순서·page-local 위치로 돌아왔고 `frame_overflow_pixels`도 사라졌다.
7. Stage 5에서 HWPX p273 그림 11의 단일 `TopAndBottom` 표도 저장 anchor lane을 사용하도록, 기존 빈 host·
   단일 float·다음 vpos 증가·선언 bottom 적합 조건은 그대로 둔 채 original HWPX stored-layout에만 해당
   gate를 열었다. 일반 block fit의 4.7px 초과 이월이 사라져 그림 11과 후속 흐름은 기준 PDF와 같은 p13–p15에
   놓였다.
8. Stage 6에서 HWPX p344의 next-vpos rewind `RowBreak` 그림 표도 fresh page로 defer하도록 source
   profile gate를 최소 확장했다. p344 table은 p23에서 사라져 p24 `y=90.6px`으로 이동했고, p13–p15
   그림 11 흐름도 회귀하지 않았다.
9. Stage 7에서 HWPX p344의 image offset reset이 적용되지 않던 직접 원인(outer host stored vpos를
   HWP5에만 전달하던 caller gate)을 고쳤다. p344 picture는 p24 `y=92.5px`, Bottom caption은
   `y=434.4/455.7/477.1px`으로 복원됐고, p345 이후 flow도 기준 PDF 순서를 유지한다.

## 미해결과 다음 회차

HWP 그림 23 p23–p24 체인, HWPX의 더 이른 그림 11 이월, HWPX p344 table의 페이지 소유권과 image/caption
offset은 조사 범위에서 해소됐다. HWPX p344는 24쪽 `y=90.6px`, image `y=92.5px`, Bottom caption 3줄
`y=434.4/455.7/477.1px`이고 23쪽에는 없다. p24의 `question_marker_flow_drift` 자동 후보는 일반
`○` bullet을 exam marker로 오인한 것으로, review PNG와 render tree에서 그림 23 흐름 결함은 확인되지
않았다. 원본 HWP·HWPX와 각각의 기준 PDF, review PNG는 모두
[`pdf/pr3740/README.md`](../../pdf/pr3740/README.md) 및 연결된 증적에 보관한다. 상세 페이지 비교와
자동 후보는
[Stage 1 visual sweep](../working/task_m100_3738_stage1_visual_sweep.md),
[Stage 2 visual sweep](../working/task_m100_3738_stage2_visual_sweep.md),
[Stage 3 visual sweep](../working/task_m100_3738_stage3_visual_sweep.md),
[Stage 4 visual sweep](../working/task_m100_3738_stage4_visual_sweep.md),
[Stage 5 visual sweep](../working/task_m100_3738_stage5_visual_sweep.md),
[Stage 6 visual sweep](../working/task_m100_3738_stage6_visual_sweep.md),
[Stage 7 visual sweep](../working/task_m100_3738_stage7_visual_sweep.md)에 기록했다.

현재 확인한 HWP/HWPX 그림 23 흐름에는 추가 Stage를 열 필요가 없다. 전체 215쪽 raster sweep이나
전체 integration test는 이 좁은 수정의 완료 근거로 사용하지 않았다.

## 검증 범위

- `cargo fmt`
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo test relocated_hwp5_picture_caption_uses_next_saved_flow_anchor --lib --quiet`
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo build --profile release-test --bin rhwp`
- HWP 및 HWPX 각각 p23–p24, 144 DPI visual sweep — 선택 페이지 2/2 완료
- HWPX p13–p15, 144 DPI visual sweep — 선택 페이지 3/3 완료; 최초 그림 11 page drift 해소 확인
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo test stored_layout_relocated_empty_rowbreak_picture --lib --quiet`
- HWPX p23–p24, p13–p15, 144 DPI visual sweep — 각각 2/2, 3/3 완료; p344 table ownership만 해소 확인
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo test stored_layout_relocated --lib` — 2 passed
- HWPX p23–p24, p13–p15, 144 DPI visual sweep — 각각 2/2, 3/3 완료; p344 image/caption offset 복원 확인

전체 integration test와 215쪽 전체 raster sweep은 이 회차의 완료 근거로 사용하지 않았다. 현재
범위 밖 페이지와 글꼴 raster 차이는 남을 수 있으므로, 이 보고서는 선택 페이지의 그림 흐름 정합만
표현한다.
