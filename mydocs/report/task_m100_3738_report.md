---
kind: investigation
status: active
canonical: mydocs/manual/codex/docs_and_git_workflow.md
last_verified: 2026-08-02
---

# Task #3738 결과 보고 — Stage 1–12 그림·RowBreak·각주 reservation 보정과 전체 pagination 잔여

- 이슈: [#3738](https://github.com/edwardkim/rhwp/issues/3738)
- 관련 PR: [#3740](https://github.com/edwardkim/rhwp/pull/3740)
- 상태: **Stage 8의 HWP p23 그림 21 caption, Stage 9의 HWP p66 table-footnote first fragment, Stage 11의 HWP p67 footer collision, Stage 12의 HWP p30 각주 29 reset collision은 복원. 전체 215쪽 pagination 정합은 계속 조사 중.**

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
10. Stage 8에서 native HWP p23의 그림 21/22가 Center cell 안에서 Bottom caption을 제외한 그림 본체만
    중앙 정렬해 caption이 약 50px 아래로 밀리는 원인을 고쳤다. Bottom caption height와 spacing을 그림
    본체와 하나의 시각 블록으로 Center/Bottom 정렬하되, Top caption·일반 picture·pagination은 바꾸지
    않았다. 그림 21 caption 첫 줄은 `544.7px → 494.7px`으로 이동했고 한컴 PDF의 `495.16px`와
    0.46px 차이다.
11. Stage 9에서 HWP p728 7×2 `RowBreak` 표의 모든 table-cell footnote `294.0px`를 fragment 전부터
    선예약해 p66 표 전체가 이월되던 흐름을 고쳤다. 작은 non-rowspan RowBreak 표만 fragment queue로
    좁히고, 확정한 fragment page에 들어가는 cell footnote를 순서대로 등록했다. HWP p66은 기준 PDF처럼
    표 0–4행(Organ Donation까지)과 각주 76·77을 보유하고 p67은 Stephanie/Policy 5–6행부터 재개한다.
    전체 HWP 쪽수는 225→224가 됐다.
12. Stage 10에서 p67 각주의 composed line spacing을 paginator까지 단순 전파한 후보는 224→226쪽으로
    악화해 커밋하지 않고 기각 근거만 남겼다.
13. Stage 11에서 p67 `FootnoteArea` reservation이 paint가 누적하는 trailing line-spacing을 빠뜨린
    것을 고쳤다. renderer-side area height만 exact paint 산식에 맞춰 `y=669.5, h=369.8px`에서
    `y=600.6, h=438.7px`으로 복원했고, actual footnote bottom은 footer top `1039.3px`에서 끝난다.
    paginator를 건드리지 않아 224쪽을 유지했고 p66 table fragment ownership도 회귀하지 않았다.
14. Stage 12에서 native HWP5 문단 407의 첫 각주 29와 `vpos=0` reset을 실제 composed footnote
    height로 대조했다. visible body line은 각주 위에 남되 trailing line-spacing만 각주 영역을 넘는
    경우에만 tail을 분리하도록 좁혀, p30의 두 줄/각주 overlap을 제거하고 p31의 tail·`5. 독일`,
    p32의 그림 35 page ownership을 기준 PDF처럼 복원했다. 넓은 reset 강제 split 후보는 226쪽
    regression을 일으켜 커밋하지 않았다.

## 미해결과 다음 회차

HWP 그림 23 p23–p24 체인, HWPX의 더 이른 그림 11 이월, HWPX p344 table의 페이지 소유권과 image/caption
offset, 그리고 HWP p23 그림 21 caption cell 정렬은 해당 선택 페이지 범위에서 해소됐다. HWPX p344는
24쪽 `y=90.6px`, image `y=92.5px`, Bottom caption 3줄 `y=434.4/455.7/477.1px`이고 23쪽에는 없다.
Stage 8 HWP 그림 21 caption 첫 줄은 PDF `495.16px` 대비 rhwp `494.7px`이다.

그러나 전체 문서는 아직 **HWP 224쪽/HWPX 224쪽, 한컴 PDF 215쪽**이다. Stage 12가 p30–p32의 각주
reset collision을 제거했어도, p68의 그림 49·caption `RowBreak` table은 첫 행 near-fit 계산이 실제
각주 경계보다 2.6px 보수적으로 잡혀 p69로 통째로 이월된다. PDF에서는 같은 table이 p68 하단에서
끝난다. 다음 Stage는 이 RowBreak table 분기를 별도 분석한다. 완료된 p30–p32, p66 소유권과 p67 footer
collision을 전체 pagination 완료로 표현하지 않는다.

원본 HWP·HWPX와 각각의 기준 PDF, review PNG는 모두
[`pdf/pr3740/README.md`](../../pdf/pr3740/README.md) 및 연결된 증적에 보관한다. 상세 페이지 비교와
자동 후보는
[Stage 1 visual sweep](../working/task_m100_3738_stage1_visual_sweep.md),
[Stage 2 visual sweep](../working/task_m100_3738_stage2_visual_sweep.md),
[Stage 3 visual sweep](../working/task_m100_3738_stage3_visual_sweep.md),
[Stage 4 visual sweep](../working/task_m100_3738_stage4_visual_sweep.md),
[Stage 5 visual sweep](../working/task_m100_3738_stage5_visual_sweep.md),
[Stage 6 visual sweep](../working/task_m100_3738_stage6_visual_sweep.md),
[Stage 7 visual sweep](../working/task_m100_3738_stage7_visual_sweep.md),
[Stage 8 visual sweep](../working/task_m100_3738_stage8_visual_sweep.md),
[Stage 9 visual sweep](../working/task_m100_3738_stage9_visual_sweep.md),
[Stage 11 visual sweep](../working/task_m100_3738_stage11_visual_sweep.md),
[Stage 12 visual sweep](../working/task_m100_3738_stage12_visual_sweep.md)에 기록했다.

현재 그림별 선택 페이지의 완료 판정과 전체 215쪽 pagination 완료 판정을 혼동하지 않는다. 전체 215쪽
raster sweep이나 전체 integration test는 Stage 8의 좁은 caption 보정 완료 근거로 사용하지 않았다.

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
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo test --profile release-test --test issue_3738_hwp_caption_cell_alignment` — 1 passed
- HWP p23, 144 DPI visual sweep — SVG/render tree 225쪽 생성, 선택 raster 1/1 완료; 그림 21 caption 직접 좌표와 PDF의 차이 0.46px
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo test --profile release-test --test issue_3738_rowbreak_table_footnote_fragment` — 1 passed
- HWP p66–p67, 144 DPI visual sweep — SVG/render tree 224쪽 생성, 선택 raster 2/2 완료; p66의 표 0–4행/각주 76·77 ownership 복원, p67 35px frame overflow 후보는 잔여로 기록
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo test --profile release-test --test issue_3738_rowbreak_table_footnote_fragment --test issue_3738_hwp_caption_cell_alignment` — 2 passed
- HWP p66–p67, 144 DPI visual sweep — SVG/render tree 224쪽 생성, 선택 raster 2/2 완료; p67 `FootnoteArea` actual bottom과 footer top 모두 1039.3px, structural 후보 0건
- `CARGO_TARGET_DIR=target/review-planet6897-20260802 CARGO_INCREMENTAL=0 cargo test --profile release-test --test issue_3738_rowbreak_table_footnote_fragment --test issue_3738_hwp_caption_cell_alignment` — 3 passed
- HWP p30–p32, 144 DPI visual sweep — SVG/render tree 224쪽 생성, 선택 raster 3/3 완료; structural 후보 0건, p30 각주 29 collision 제거 및 p31–p32 본문·그림 ownership 복원

전체 integration test와 215쪽 전체 raster sweep은 이 회차의 완료 근거로 사용하지 않았다. 전체
pagination 정합이 아직 남아 있으므로, 이 보고서는 선택 그림 흐름, Stage 9 p66 table-footnote
ownership, Stage 11 p67 footer collision만 완료로 표현한다.
