# Task #3822 Stage 2 — 최신 stack 줄바꿈 검증

- 이슈: [#3822](https://github.com/edwardkim/rhwp/issues/3822)
- 브랜치: stack/issue-3822-overlong-token-wrap
- 최신 기준: upstream/devel 8d7bc622e
- code candidate: e4cf29df1
- 작성일: 2026-08-04

## 최신 기준 focused 결과

- cargo test renderer::composer::tests --lib: 52 / 52 통과
- #3822 전용 Latin·숫자·잔여 폭·hanging indent: 4 / 4 통과
- git diff --check: 통과

최종 검증 뒤 추가된 devel 19커밋은 composer 파일과 겹치지 않았다. 8d7bc622e로
다시 rebase한 최상단에서 composer focused spot-check를 반복한다.

## 기존 실제 문서 증적

2026-08-03 production WASM snapshot에서 다음을 확인했다.

- HWP/HWPX 두 번째 숫자 줄바꿈: 2 / 2
- line count 5 → 6, caret 665.4 / cell right 672.8, overflow false
- HWP/HWPX × digits, Latin, 완료 한글→digits 저장·재열기: 6 / 6
- 실제 IME→공백→두 번의 숫자 wrap: 2 / 2
- #3822 미적용 control은 숫자 79번째에서 cellOverflowed=true

문자 수와 실제 높이가 충분히 늘어 page count가 115 → 116이 되는 것은 숨은 overflow가
정상 line으로 복원된 결과다.

## 귀속 정정

기존 integration gate의 advance 상한은 TextRun origin과 bbox가 셀 안에 있는지를 검증했지만
glyph outline의 실제 가로 비율을 직접 증명하지 않았다. 따라서 다음처럼 분리한다.

- #3822: token 재분할과 overflow 해결
- #3937: Canvas/SVG 영문·숫자 glyph outline 확대 해결

최신 최상단 stack에서 같은 combined E2E를 한 번 실행해 두 정확성 수정과 #3815 scheduler의
조합을 확인한다.
