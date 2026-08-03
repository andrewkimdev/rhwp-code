---
kind: review
status: pending-ci
canonical: mydocs/pr/archives/pr_3741_review.md
last_verified: 2026-08-03
---

# PR #3741 검토 - CanvasKit exact GlyphRun 글꼴 재생

## 라우팅

```text
base route: maintainer_general
modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, rework_and_exceptions.md
review branch: review/seo-rii-20260803
review head: bc2b3f801ca72b60c8c823bc2a5feef1a541aa1b
review base: 9095cd52d035d07c68872b41be7d792d101c5586
feature visibility ref: review/seo-rii-20260803-p42 (ac5262956)
```

## PR metadata

| 항목 | 값 |
| --- | --- |
| PR | [#3741](https://github.com/edwardkim/rhwp/pull/3741) |
| 관련 issue | [#536](https://github.com/edwardkim/rhwp/issues/536) P42 exact embedded font GlyphRun 후속 단계 |
| 작성자 | `@seo-rii` |
| base / head | `devel` / `render-p42` |
| 원격 변경 규모 | 22 files, +2,171 / -165 |
| review 문서 작성 시점 원격 상태 | open, ready, mergeable 재확인 필요 |

## 변경 범위

- Layer schema 1.22와 resource table 1.6에 inline `fontBlobs`와 content-addressed key를 추가한다.
- Rust normal lowering이 exact embedded face의 수평 nominal GlyphRun을 TextRun fallback과 함께 만든다.
- CanvasKit이 BLAKE3/길이 검증, TTC v1/v2 face 추출, typeface/font cache, `drawGlyphs` direct replay를 수행한다.
- 기본 renderer는 Canvas2D로 유지한다.

renderer/paint/WASM 변경이므로 focused Rust, Studio, Native Skia, WASM, release-test와 visual evidence를
검토 범위로 둔다. 기존 exact TTC face pixel E2E는 direct path의 실행 근거지만, 아래 strict gate 결함으로
최종 시각 merge 근거로는 사용하지 않는다. 검토 중 contributor가 `devel` merge commit을 push했으므로 이전
`cc3b1bb2` 결과는 역사 기록으로만 남기고, 최종 판단은 `bc2b3f80`의 재실행 결과만 사용한다.

## 발견 사항

### 높음 - strict GlyphRun consumer가 synthetic/bidi/writing-mode 전제조건을 검증하지 않는다

`rhwp-studio/src/view/canvaskit/glyph-run-fonts.ts`의 `replayStatus`는 embedded font proof, variation,
orientation, geometry를 확인하지만 `shapeKey.fontInstance.syntheticBold`,
`shapeKey.fontInstance.syntheticItalic`, `direction`, `shapeKey.direction`, `writingMode`,
`shapeKey.writingMode`를 strict gate에서 거부하지 않는다. 이후 `font()`은 synthetic bold와 italic을
CanvasKit `setEmbolden`/`setSkewX`로 적용한다.

PR 문서와 `docs/text-ir-v2.md`는 synthetic style, bidi, vertical writing을 strict portable GlyphRun
범위 밖으로 두고 TextRun fallback을 요구한다. 그러나 검증된 TTC face와 BLAKE3 resource를 사용한
독립 probe에서 아래 세 입력 모두 `replayable: true`가 됐다.

```text
synthetic=true
bidi=true
verticalWritingMode=true
```

재검토 결과, 현재 Rust producer는 해당 필드를 false/LTR/horizontal로만 내보내므로 이 결함이 지금의
일반 HWP/HWPX 입력에서 곧바로 잘못된 PDF나 화면 출력을 만들지는 않는다. 따라서 이것을 외부 입력에서
즉시 악용되는 보안 결함으로 분류하지 않는다.

그러나 P42의 PR 본문은 Rust strict selection, Text IR diagnostics, browser variant selection이 같은 조건을
적용하고 synthetic/bidi/vertical을 strict GlyphRun 범위에서 제외한다고 명시한다. 실제로는 Rust
`glyph_run_is_strict`, renderer selection, browser `replayStatus` 모두 이 조건을 확인하지 않고, browser
`font()`은 synthetic 값을 CanvasKit 효과로 적극 적용한다. 즉 producer-only 불변식에 기대는 상태이며,
향후 producer 확장이나 직렬화된 Layer payload의 비정상 값에서 fallback 대신 direct replay를 고른다.
명시한 fail-closed protocol 계약을 완료하지 못한 정확성 결함이므로 이 PR 범위에서는 재작업 요청 사유가
된다.

수정 요구:

1. Rust `VariantRejectReason`과 browser `replayStatus` 모두 synthetic style, bidi direction,
   horizontal writing mode 불일치를 명시적으로 reject한다.
2. TextRun fallback 선택을 확인하는 Rust와 Studio 회귀 테스트를 각각 추가한다.
3. exact TTC pixel E2E에는 정상 control과 위 reject case를 함께 둔다.

## 메인터너 보정과 검증

| 검증 | 결과 |
| --- | --- |
| `git diff --check` | 메인터너 보정 후 통과 |
| `cargo test --profile release-test renderer::layer_renderer::tests --lib` | 44 / 44 통과 |
| `cargo test --profile release-test paint::text_v2::tests --lib` | 19 / 19 통과 |
| `cargo test --profile release-test paint::json::tests::serializes_ --lib` | 14 / 14 통과 |
| `npm --prefix rhwp-studio test` | 759 / 759 통과 |
| `npm --prefix rhwp-studio run e2e:renderer-contract` | 통과 |
| `npm --prefix rhwp-studio run e2e:canvaskit-font-coverage` | 통과, exact TTC direct replay와 reject fallback 확인 |
| `npm --prefix rhwp-studio run e2e:manifest-check` | 80 / 80 통과 |
| synthetic/bidi/writing-mode independent probe | 모두 잘못된 strict replay 허용, 실패 재현 |
| `cargo test --profile release-test --tests` | library 3,182 통과, ignore 7 유지, 전체 종료 코드 0 |
| Native Skia 3종 | 58 / 58, 2 / 2, 4 / 4 통과 |
| `cargo fmt --check`, clippy | 통과 |
| Studio production build | 통과 |
| WASM package | release compile과 wasm-opt까지 정상 진행 후 작업지시자 지시로 완료 대기 중지 |

## CI 참고값

이전 `cc3b1bb2`의 완료 CI는 최신 판단에 사용하지 않는다. 최신 `bc2b3f80`의 CI preflight, CodeQL,
Render Diff, JavaScript/Python/Rust Analyze, Canvas visual diff, lint, frontend package gates, Native Skia,
8개 default-feature shard, Build & Test가 모두 통과했고 merge state는 `CLEAN`이다. 다만 CI도 정상
producer가 만드는 false/LTR/horizontal payload만 다루므로, 독립 probe로 확인한 strict consumer gate
누락을 발견하지 못한다.

## 최종 권고

**메인터너 보정 완료, 최신 CI 대기.** 초기 발견은 현 문서가 즉시 깨지는 회귀가 아니라 P42가 약속한
strict fail-closed GlyphRun protocol의 범위 결함이었다. contributor 원 commit은 유지하고 그 위에
synthetic/bidi/writing-mode consumer gate와 회귀 테스트를 추가했다. 로컬 전체 검증은 통과했으며,
보정 commit을 push한 뒤 최신 GitHub full CI가 통과하면 merge한다.
