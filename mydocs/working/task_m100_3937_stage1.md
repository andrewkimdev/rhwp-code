# Task #3937 Stage 1 — 배분 간격과 browser glyph 폭 분리

- 이슈: [#3937](https://github.com/edwardkim/rhwp/issues/3937)
- 브랜치: stack/issue-3937-distribution-glyph-width
- 최신 기준: upstream/devel 8d7bc622e
- code candidate: 6d9b0f7f4
- 작성일: 2026-08-04

## 원인과 수정

compute_char_positions의 cluster advance에는 다음 문자의 origin을 옮기는
extra_char_spacing이 포함된다. 기존 WebCanvas ASCII scaleX와 SVG textLength는 이 전체
advance를 glyph-fit 폭으로 사용해 배분 간격만큼 glyph 윤곽까지 확대했다.

TextStyle에 layout advance에서 양수 배분 간격만 제거하는 공통 계산을 추가했다.
Canvas2D와 SVG는 이 계산을 사용하고, 문자 origin과 layout advance는 바꾸지 않는다.
음수 간격은 원래 폭을 안전하게 복원할 수 없어 일반 fit을 생략하며 반각 CJK 인용부호의
전용 textLength 계약은 유지한다.

## 최신 기준 검증

- cargo test spacing --lib: 42 / 42 통과
- cargo test renderer::svg::tests --lib: 41 / 41 통과
- cargo check --target wasm32-unknown-unknown --lib: 통과
- git diff --check: 통과

최종 검증 뒤 devel이 19커밋 전진해 8d7bc622e로 다시 rebase했다. 추가 변경은
structure query와 문서뿐이고 renderer 제품 파일과 겹치지 않는다. 최상단에서 focused
spot-check를 다시 실행한다.

이전 integration snapshot에서는 production WASM HWP/HWPX 연속 IME→숫자 E2E 2 / 2와
사용자 브라우저 시각 판정을 통과했다. 최상단 stack revision에서 같은 combined smoke를
한 번 더 실행한다.

## 범위 경계

- #3937: Canvas/SVG glyph 윤곽 폭
- #3822: 이전 break 뒤 긴 token의 반복 줄바꿈
- #3815: deferred pagination 시작 coalescing

세 변경은 제품 코드상 분리돼 있으며, 최상단 E2E가 실제 연속 입력 조합을 함께 검증한다.
