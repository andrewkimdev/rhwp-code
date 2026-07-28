---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-29
---

# PR #3529 리뷰 — HWP3 암호 문서 렌더링 정합

- PR: [#3529](https://github.com/edwardkim/rhwp/pull/3529)
- 관련 이슈: [#3486](https://github.com/edwardkim/rhwp/issues/3486) (`Closes` 미사용)
- 역할: `jangster77` collaborator self-review
- 작성 시점 source head: `f049683a8671197320b47873d1821c769887aa8a`

## 라우팅과 merge 조건

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, multi_pr_update_branch.md
```

최신 `upstream/devel` 위로 rebase한 뒤 PR을 만들었다. 이 self-review는 독립 승인과 메인터너 검토를
대체하지 않는다. merge 전에는 이 review·오늘할일을 포함한 최신 head의 required check 성공,
`MERGEABLE` 재확인, 메인터너 검토와 작업지시자 승인이 모두 필요하다.

## 변경 범위와 수용 판단

1. HWP3 암호 원본의 Square-wrap 도형은 column 기준 좌표, no-fill/no-line sentinel, 문단 inset과
   기본 text gap을 보존한다. 이 계약은 일반 HWP3와 다르므로 복호화 원본 경로로 한정했다.
2. 페이지 배경 이미지는 raw legacy brightness/contrast의 저장 순서와 화면 투영 순서를 분리하고,
   일반 `RealPic` 배경을 watermark opacity로 잘못 낮추지 않는다. SVG·Web Canvas·Skia가 같은 규칙을
   사용한다.
3. 실제 Studio Canvas bitmap 경계는 fractional CSS A4 크기를 올림해 144dpi에서 마지막 물리 pixel을
   보존한다.
4. 새 HWP5 비교 fixture와 현재 HWPX IR 정규화 결과를 field-sweep baseline에 등록했다.

**조건부 수용 권고.** 실제 fixture의 암호 열기·도형·배경·A4 경계 계약은 수용 가능하나, 한컴 전용
옛한글 glyph/metric과 전수 PDF fidelity는 #3486의 열린 후속 범위다.

## 시각 검증

- 입력: `samples/HWP3-password-123456.hwp`
  - SHA-256: `db743d084efc9e08e839a5b4d978b16b8676434011776e090e4cda43e57304be`
- 기준 PDF: `pdf/HWP3-password-123456.pdf` (24쪽)
  - SHA-256: `3ced5ad95ad30331e2756b5b34509c1ac91dfe3c72013c8e14f2556ca6bd5776`
- 방식: Studio password-open 경로로 같은 fixture를 연 뒤 144dpi Canvas와 PDF p1을 대조했다.
- 결과: p1의 배경·아이콘·Square-wrap 본문·차례 도형과 A4 우·하단 경계가 증적에 실제로 보인다.
  제목의 `ᄒᆞᆫ` glyph 외관은 PDF와 다르며 Stage 9 판정에 따라 의도적으로 현대 음절로 치환하지 않았다.

![HWP3 암호 문서 p1 — Hancom PDF와 Studio Canvas 실제 대조](../assets/pr_3529_hwp3_password_p001_canvas_review.png)

## 검증 기록

모든 Cargo 검증은 `CARGO_TARGET_DIR=target/task_3486_render_v2`, `CARGO_INCREMENTAL=0`에서 실행했다.
공유 `target/debug`, `target/release`, `target/release-test`, `target/wasm32-unknown-unknown`은 삭제하거나
검증 결과에 섞지 않았다.

| 검증 | 결과 |
| --- | --- |
| `cargo fmt --check`, `git diff --check` | passed |
| `cargo clippy --all-targets -- -D warnings` | passed |
| `hwp3_password_fixture` | 8 passed |
| `ir_field_sweep_baseline` | 2 passed; 792행 baseline 확인 |
| `test_scaled_canvas_extent_keeps_fractional_a4_edge` | passed |
| native-Skia 3개 게이트 | passed |
| `wasm-pack build --target web --out-dir pkg` | passed |
| Chrome·Firefox extension `npm run build` | passed |
| Studio `npm run e2e:hwp-password-open` | passed; HWP3 144dpi A4 경계 확인 |

`cargo test --profile release-test --tests`는 #1692의 3개 기존 실패에서 중단했다. 같은 실패
(`line_box_reflects_para_margins`, `answer_endnote_pages_match_pdf_ranges`,
`page22_relationship_box_uses_table_flow`)는 현재 보정 전 Stage 9 head `e94194556`의 깨끗한 별도
worktree에서도 동일하게 재현했다. 이번 PR 회귀로 판정하지 않으며 #3486 수용 근거에도 사용하지 않는다.

## 위험과 후속 보완

- 제목의 한컴 전용 옛한글 glyph/advance와 일부 본문 font metric은 PDF와 아직 다르다.
- p3 표·bullet/폭 같은 전수 시각 차이는 후속 Stage에서 별도의 재현·원인·회귀로 처리한다.
- Stage 11은 구현 없는 다음 분석 계획이므로 이 PR에 포함하지 않는다.
