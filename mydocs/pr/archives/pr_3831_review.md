---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3831 검토 — Studio 표 나누기·붙이기

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3831](https://github.com/edwardkim/rhwp/pull/3831) / @enigma-jerry72 |
| 원 head | `97be5b3627e8e2acb42ccfc14a25167d1ae71d52` |
| 최신 기준 devel | `301d0fe5f` |
| 검토 branch | `review/enigma-jerry72-20260804` |
| 메인터너 보정 | `ab9baf359` |

## 검토와 보정

PR은 Studio의 `표 나누기`와 `표 붙이기` 명령, WASM 경계, undo snapshot, 한글 IME
chord, 표 행·셀·zone 재구성을 함께 추가한다. 원 코드의 분할 전 검증은 `u16`의
`row + row_span` 덧셈을 사용해 손상된 표에서 debug panic 또는 release wrap 가능성이 있었다.
메인터너 보정은 범위를 `u32`으로 계산해 문서 변형 전에 명시 오류로 거부한다.

또한 Studio local-resize 힌트는 HWP/HWPX 직렬화에 보존되지 않는다. 저장 후 재열기에서는
힌트가 없어도 셀 폭으로 추론한 base-grid outlier 행을 폭 재계산에서 제외해야 렌더러와
`common.width`가 일치한다. `base_grid_column_widths()`의 조기 반환을 제거하고, 실제 샘플
기반 회귀에서 런타임 힌트를 비운 상태를 고정했다.

## 검증

| 검증 | 결과 |
| --- | --- |
| `split_rejects_corrupted_cell_row_span_overflow_before_mutation` | 최신 head + 보정에서 1 passed |
| `split_after_local_resize_keeps_render_grid` | 최신 head + 보정에서 1 passed |
| 최초 전체 `cargo test --profile release-test --tests` | 보정 포함 이전 PR head에서 실패 표식 없이 전 항목 통과 로그 확인 |
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | 보정 포함 검토 기준에서 성공 |
| Native Skia 3종 | 58 + 2 + 4 passed |
| `wasm-pack build --target web` | 성공, 격리된 검토 산출물 생성 |
| Studio fresh-WASM TypeScript + `npm test` | 759 passed |
| Studio `npm run build` | 성공 |
| headless browser E2E | 4x2 표: 나누기 후 2행+2행, 붙이기 후 4행·8셀, undo snapshot 2건 확인 |
| 최신 원 PR CI | CodeQL, Lint, Native Skia, Frontend, Render Diff, 4개 test shard 모두 통과 |

`npx tsc --noEmit`을 WASM 생성 없이 단독 실행하면 기존 `pkg/rhwp.d.ts`가 이전 생성물이어서
새 API를 찾지 못한다. 이는 CI도 먼저 `wasm-pack build`를 실행하는 생성 순서 문제이며, fresh
WASM 선언을 참조한 TypeScript 검사와 실제 Studio build는 정상이다.

## 판정

**수용 권고.** 현재 원격 PR은 clean하며 최신 `devel`을 포함한다. 다만 메인터너 보정
`ab9baf359`은 아직 원격 PR head에 push하지 않았다. 해당 commit과 이 검토 기록을 동일 PR
branch에 push하고 required CI가 다시 통과한 뒤 merge해야 한다.
