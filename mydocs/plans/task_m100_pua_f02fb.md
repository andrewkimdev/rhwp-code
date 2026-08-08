# 수행계획서 — task_m100_pua_f02fb

- **대상 샘플**: `samples/basic/pau-004.hwp`
- **브랜치**: `task_m100_pua_f02fb_small_triangle`
- **stack 기준**: `task_m100_4158_char_overlap_boxed_pua` `27932685b`
- **devel 기준**: `upstream/devel` `5a4f26d0d`
- **기록 시각**: 2026-08-08 KST
- **원격 상태**: 이슈·push·PR 미수행
- **절차 상태**: 구현·집중 검증·#4158 stack 통합 WASM·작업지시자 시각 판정 완료 — 전체 PR 게이트 별도 승인 대기

## 1. 문제

한컴 문자표로 입력한 `U+F02FB`는 `함초롬돋움`에서 작은 오른쪽 방향 삼각형으로 보이지만,
rhwp-studio의 공개 글꼴에는 해당 Supplementary PUA-A 글리프가 없어 tofu로 출력된다.

## 2. 확인된 의미와 수정 계약

- 원문 IR과 저장 텍스트는 `U+F02FB`를 보존한다.
- 일반 `TextRun`의 paint·측정 표면에서는 표준 `U+25B8` `▸`로 투영한다.
- Canvas2D·SVG·Native Skia는 기존 `expand_pua_render_text` 공통 경로를 공유한다.
- 인접한 기존 `U+F02FC → ►`와 다른 PUA 동작은 바꾸지 않는다.

## 3. 래칫과 검증

1. 실제 `pau-004.hwp`의 raw IR, 표시 문자열, SVG 구조 테스트
2. 검증된 한컴 PUA 표의 정렬·인접 문자 보존 단위 테스트
3. Native Skia feature focused test
4. release WASM build와 실제 Canvas2D 호출·스크린샷 E2E
5. `output/pau-004/`에 render tree와 시각 증적 보존
6. 집중 결과 보고 뒤 전체 PR 게이트와 원격 단계는 별도 승인
