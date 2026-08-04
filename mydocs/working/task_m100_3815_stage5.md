# Task #3815 Stage 5 — 최신 3단 stack 제한 검증

- 이슈: [#3815](https://github.com/edwardkim/rhwp/issues/3815)
- 브랜치: stack/issue-3815-pagination-coalescing
- 최신 기준: upstream/devel 8d7bc622e
- code candidate: fdebb38fb
- 작성일: 2026-08-04

## Stack

1. #3937 browser glyph 폭 보정
2. #3822 overlong token 반복 줄바꿈
3. #3815 pagination coalescing과 연속 입력 E2E

세 레이어를 최신 devel 위에 선형으로 재구성했고 integration merge commit이나 중복
cherry-pick은 포함하지 않았다.

## 최신 기준 검증

- renderer spacing focused: 42 / 42
- SVG renderer: 41 / 41
- composer: 52 / 52
- runner + InputHandler focused: 23 / 23
- Studio 전체 unit: 763 / 763
- npx tsc --noEmit: 통과
- production wasm-pack release build: 통과
- wasm32-unknown-unknown library check: 통과
- git diff --check: 통과

## HWP/HWPX combined smoke

production WASM과 새 headless Chrome에서 continuous-only 시나리오를 각 형식 한 번 실행했다.

| 형식 | 숫자 줄 전환 | 최종 숫자 | pending operation p95 | 결과 |
| --- | --- | ---: | ---: | --- |
| HWP | 11 / 69 | 73 | 49.3ms | GREEN |
| HWPX | 11 / 69 | 73 | 50.6ms | GREEN |

두 형식 모두 IME 조합 뒤 숫자가 두 번 줄바꿈되고 overflow 없이 최종 revision까지 게시됐다.
이전 동일 smoke의 50.2ms / 49.7ms 대비 변화는 각각 -1.8% / +1.8%로 ±10% gate 안이다.
따라서 2026-08-03에 완료한 current, 80ms, 250ms 형식별 3회 전체 측정은 반복하지 않았다.

이후 devel이 19커밋 전진해 최종 base를 8d7bc622e로 갱신했다. 추가 변경은
structure query와 문서이며 renderer, composer, Studio 제품 파일과 겹치지 않는다.
전체 browser smoke는 보존하고 spacing 42 / 42, composer 52 / 52, Studio focused
23 / 23과 TypeScript spot-check를 반복해 모두 통과했다.

## 기존 성능 근거

이전 production 측정에서 current 완료 중앙값은 HWP 1,581.6ms, HWPX 1,586.0ms였고,
input dispatch와 begin overlap, superseded publication과 flow sync flush는 모두 0이었다.
step p95 최대는 8.4ms였다.

남은 pending operation 비용은 exact cursor query가 지배하며 이번 scheduler PR의 비범위다.
flow가 안정되면 stable-tail fast path로 돌아간다.

## 게시 상태

renderer 이슈 #3937은 생성했지만 세 stack branch는 아직 원격에 push하지 않았고 Draft PR도
만들지 않았다. 세 본문을 작업지시자에게 먼저 보여준 뒤 승인받아 gh stack으로 제출한다.
