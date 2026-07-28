---
kind: pr_review_impl
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-29
---

# PR #3529 구현 보정 기록

## 보정 내역

- HWP3 rectangle의 `0x10000000` no-fill marker와 line color의 동일 no-line marker를 IR 기본값과
  구분해, 채움·테두리를 임의로 그리지 않게 했다.
- HWP3 floating object의 reference position 1을 column origin으로 매핑하고, 암호 원본의
  Square-wrap line box만 문단 inset·기본 gap 계약을 적용했다.
- `PageBackgroundImage`의 raw legacy brightness/contrast를 화면 표현용 순서로 투영하고,
  일반 `RealPic` 배경을 watermark opacity로 낮추지 않도록 SVG·Canvas·Skia를 일치시켰다.
- `scaled_canvas_extent()`는 truncation 대신 ceil을 써 A4 fractional CSS pixel의 우·하단 clipping을
  방지하고 Studio E2E로 1191×1684 bitmap을 확인했다.
- CanvasKit `PageRenderer`와 결과 비교 창도 같은 ceil bitmap 경계를 적용했다. 이는 Canvas2D의
  `794×1123`와 CanvasKit의 `793×1122` 불일치로 readiness corpus 전체가 비교 오류가 된 CI 결과를
  직접 보정한 것이며, 7개 representative corpus의 Canvas2D↔CanvasKit readiness 재현으로 확인했다.

## baseline 갱신 사유

`samples/HWP5-nopassword-123456.hwp`가 field-sweep corpus에 들어온 뒤 baseline에 없던 HWP5
round-trip divergence 경로와, 현재 HWPX 정규화의 stable 결과를 792행 TSV로 재생성했다. rebase 후
`ir_field_sweep_baseline`을 다시 실행해 baseline 초과가 없음을 확인했다.

## 수용하지 않은 변경

PDF p1의 한컴 glyph와 공개 old-Hangul font의 외관 차이는 parser 원문 오류로 증명되지 않았다. 전역
`ᄒᆞᆫ → 한` 치환, font asset 교체, p3 표의 추정 보정은 이 PR에 넣지 않는다.
