# Stage 1 — task_m100_pua_f02fb 구현·집중 검증

- **대상 샘플**: `samples/basic/pau-004.hwp`
- **계획서**: [`mydocs/plans/task_m100_pua_f02fb.md`](../plans/task_m100_pua_f02fb.md)
- **브랜치**: `task_m100_pua_f02fb_small_triangle`
- **stack 기준**: `task_m100_4158_char_overlap_boxed_pua` `27932685b`
- **devel 기준**: `upstream/devel` `5a4f26d0d`
- **구현 커밋**: `3f0974dc8` (`fix(renderer): map U+F02FB to small right triangle`)
- **작업 시각**: 2026-08-08 KST

## 1. 진단

샘플은 1쪽·1문단이며 raw 본문은 `U+F02FB`와 `아름다운`이다. `U+F02FB`는 Unicode
category `Co`, Supplementary Private Use Area-A의 한컴 전용 문자이고 일반 `TextRun`으로
저장된다. `CharOverlap`이나 #4158의 사각 숫자 계열이 아니다.

Windows의 실제 `함초롬돋움(HCR Dotum)` cmap은 이 코드를 glyph `uF02FB`로 보유하며, 렌더
외곽은 작은 검은 오른쪽 방향 삼각형이다. 작업지시자가 한컴 표시 의미를 확인했다. 기존 검증 표에는
인접한 `U+F02FC → ►`만 있고 `U+F02FB`가 없어 raw PUA가 공개 글꼴까지 전달된 것이 tofu의
직접 원인이다.

## 2. RED와 구현

실제 샘플의 raw IR·표시 문자열·SVG를 고정한 통합 테스트 2건을 먼저 실행해 모두 실패했다.
`src/renderer/hancom_pua.rs`의 검증된 한컴 PUA 표에 `U+F02FB → U+25B8(▸)` 한 항목을
추가했다. IR과 저장 텍스트는 바꾸지 않고 Canvas2D·SVG·Native Skia의 기존
`expand_pua_render_text` paint 경로 및 텍스트 표면에서만 투영한다.

## 3. 래칫

- 실제 `pau-004.hwp` raw IR은 `U+F02FB아름다운`을 보존한다.
- paint·텍스트 표면은 `▸아름다운`을 반환한다.
- SVG와 Canvas2D에는 `▸`가 존재하고 raw `U+F02FB`는 없다.
- Canvas2D 글자 호출 순서는 `▸`, `아`, `름`, `다`, `운`이다.
- 인접 `U+F02FC → ►`와 기존 검증 PUA는 그대로 유지한다.

## 4. 집중 검증 결과

| 검증 | 결과 |
| --- | --- |
| 실제 샘플 RED | 예상 실패, Rust·SVG 2 failed |
| `cargo test --test pua_f02fb_small_right_triangle` | PASS, 2 passed |
| 검증 PUA 표 단위 테스트 | PASS, 1 passed |
| Native Skia feature release-test | PASS, 실제 샘플 2 passed |
| 인접 PUA·텍스트 표면 통합 테스트 | PASS, 13 passed |
| `cargo clippy --lib -- -D warnings` | PASS |
| `cargo fmt --check`, `git diff --check` | PASS |
| release WASM build | PASS, wasm-bindgen·wasm-opt·`pkg` packaging 완료 |
| `npm run e2e:pua-f02fb` | PASS, 6개 Canvas2D 계약 |
| `npm run e2e:manifest-check` | PASS, stacked head tracked 88개 / manifest 88행 |
| Native Skia `export-png --font-path ttfs/opensource` | PASS, 1쪽 PNG 출력 |

## 5. 시각 증적

`output/pau-004/`에 다음을 남겼다.

- `pau004_p001_canvas2d.png` — 새 WASM Canvas2D 전체 쪽
- `pau004_p001_f02fb_crop.png` — `▸아름다운` 대상 crop
- `render_tree_001.json` — raw `U+F02FB`가 남은 render tree
- `native-skia/pau-004.png` — 공개 폰트 지정 Native Skia 전체 쪽
- `native-skia/pau004_p001_f02fb_crop.png` — Native Skia 대상 crop
- `u-f02fb-handotum.png` — 로컬 한컴 폰트의 원 PUA glyph 진단본(커밋 제외)

Canvas2D와 Native Skia 모두 tofu 없이 작은 오른쪽 방향 삼각형을 출력한다. 최종 한컴 호환 시각
판정 권위는 작업지시자에게 있다.

## 6. #4158 stack 통합 재검증

최초 삼각형 branch가 `upstream/devel`에서 독립 분기되어, 그 상태로 만든 WASM에는 아직 원격에 없는
#4158 사각 번호 구현이 포함되지 않았다. 이는 두 렌더 구현의 회귀나 충돌이 아니라 branch 구성 누락이다.
삼각형 커밋을 #4158 시각 승인 head `27932685b` 위로 재배치하고 충돌한 E2E 스크립트 항목을 모두
보존했다.

재배치된 동일 head에서 `cargo build --release --bin rhwp`와 release `wasm-pack build`를 완료했다.
새 WASM으로 #4158 물리 10쪽 사각 번호 E2E 7개 계약과 `pau-004` 삼각형 E2E 6개 계약이 모두
통과했다. 따라서 통합 산출물은 사각형 안 숫자 `1`과 작은 오른쪽 방향 삼각형 `▸`를 함께 렌더링한다.

## 7. 원격 상태

GitHub 이슈 생성, push, PR 생성은 수행하지 않았다. 현재 head의 전체 PR 게이트도 집중 결과 보고 뒤
별도 승인받아 실행한다. 이 브랜치는 #4158 head를 base로 하는 PR stack이며, 통합 WASM에서는
#4158 사각 번호와 `U+F02FB` 삼각형을 함께 검증한다.
