---
kind: review-implementation
status: pending-ci
canonical: mydocs/pr/archives/pr_3741_review_impl.md
last_verified: 2026-08-03
---
# PR #3741 CanvasKit exact GlyphRun 메인터너 보정 기록

## 대상과 기준

- 대상 PR: [#3741](https://github.com/edwardkim/rhwp/pull/3741)
- contributor source head: `bc2b3f801ca72b60c8c823bc2a5feef1a541aa1b`
- 기준 `devel`: `9095cd52d035d07c68872b41be7d792d101c5586`
- 작업 branch: `review/seo-rii-20260803`
- 경로: `collaborator_external_pr` 9.3.1

`maintainer_can_modify=true`를 확인했다. contributor 원 commit은 rewrite하지 않고, 현재 가시성 review
branch 위에 메인터너 보정 commit만 추가한다. 보정을 위해 두 번째 local branch를 만들지 않는다.

## 문제

P42 producer는 synthetic style=false, LTR, horizontal-tb, bidi level 0인 GlyphRun만 만든다. 그러나 Rust
strict selection/Text IR diagnostics와 browser CanvasKit `replayStatus`는 이 전제조건을 다시 검사하지
않는다. browser는 synthetic 값이 true이면 실제 CanvasKit `setEmbolden`/`setSkewX`를 적용하므로, strict
범위 밖 payload가 TextRun fallback 대신 direct GlyphRun으로 선택될 수 있다.

현재 일반 HWP/HWPX producer 경로에서 즉시 재현되는 문서 회귀는 아니지만, P42가 명시한 strict
fail-closed Layer protocol 계약과 맞지 않는다.

## 구현과 검증 결과

1. Rust `VariantRejectReason`, renderer selection, Text IR strict 진단에 synthetic style, run/shape-key
   direction, bidi level, run/shape-key writing mode의 명시적 reject를 추가했다.
2. Studio CanvasKit `replayStatus`에도 같은 false/LTR/level-0/horizontal-tb 조건을 추가했다. 필드가
   누락된 JSON은 fail-closed로 거부한다.
3. Rust와 Studio 회귀 테스트에서 각 금지 조건이 `TextRun` fallback을 선택하고, 정상 exact TTC control은
   계속 direct replay되는 것을 확인했다.
4. 보정 후 JSON serializer fixture가 생략한 `bidi_level` 때문에 strict control 두 건이 실패했다. fixture를
   P42 정상값 `Some(0)`으로 명시한 뒤 관련 JSON 테스트 14건과 전체 회귀를 다시 통과했다.

| 검증 | 결과 |
| --- | --- |
| renderer focused | 44 / 44 통과 |
| Text IR focused | 19 / 19 통과 |
| JSON strict serializer focused | 14 / 14 통과 |
| Studio unit | 759 / 759 통과 |
| CanvasKit font coverage, renderer contract, E2E manifest | 모두 통과, manifest 80 / 80 |
| release-test 전체 | library 3,182 통과, ignore 7 유지, 전체 종료 코드 0 |
| Native Skia | library 58 / 58, placeholder 2 / 2, P37 direct PDF 4 / 4 통과 |
| format / diff / clippy | `cargo fmt --check`, `git diff --check`, `cargo clippy --all-targets -- -D warnings` 통과 |
| Studio production build | TypeScript와 Vite build 통과 |

`wasm-pack build`는 release compile과 wasm-opt 단계까지 정상 진행됐으나, 작업지시자가 CI 전 push를
우선하도록 지시해 최적화 완료 대기는 중지했다. 이 보정은 Rust와 Studio 양쪽을 수정하므로 원 PR의 최신
GitHub full CI가 WASM 포함 최종 merge 근거가 된다.

## 완료 조건

- strict selection, Text IR diagnostics, CanvasKit consumer가 같은 조건으로 false/LTR/level-0/horizontal-tb를
  요구한다.
- synthetic/bidi/writing-mode 변형은 fallback을 선택하고 정상 exact TTC control은 direct replay를 유지한다.
- code/test commit과 review·오늘할일·workflow 문서 commit을 분리한다.
- remote source SHA 재확인과 LFS 판독 뒤 승인받은 push를 수행하고, 최신 head full CI 통과를 확인한다.
