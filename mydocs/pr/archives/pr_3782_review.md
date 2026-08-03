---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3782 검토 — 재배치 줄 advance의 범위 방어

## 라우팅과 적용 경계

base route는 `maintainer_general.md`이며, `intake_and_review.md`,
`local_validation.md`, `visual_fixture_evidence.md`,
`multi_pr_update_branch.md`를 함께 적용했다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3782](https://github.com/edwardkim/rhwp/pull/3782) / @twoLoop-40 (Lee Joon ho) |
| 관련 이슈 | [#3780](https://github.com/edwardkim/rhwp/issues/3780) |
| 원 기능·head commit | `81d1b9bcce1f7f38eb0ddce65acdcef7896a07e8` |
| 검토 기준 | `upstream/devel` `9095cd52d` |
| 가시성 통합 검토 브랜치 | `review/twoloop-40-20260803` |
| 누적 적용 commit | `81d1b9bc` → `f46e15245` (`cherry-pick -x`) |
| 함께 누적한 PR | [#3783](https://github.com/edwardkim/rhwp/pull/3783) |
| 충돌 | 없음 |
| 작성 시점 원 PR 상태 | GitHub `mergeStateStatus=UNKNOWN`; 원 head CI 전체 성공, 단 최신 `devel`보다 이전 head |

reviewer는 `jangster77`로 지정했다. 원 contributor branch에는 push하지 않았다.

## 변경 검토

연속 쪽 재배치 뒤 기록된 줄 인덱스가 새 `FormattedParagraph` 줄 수와 어긋나면,
`line_advance`와 `line_advances_sum`의 직접 색인이 `len == index`에서 즉시 panic할 수 있었다.
변경은 존재하지 않는 줄의 advance를 `0.0`으로, 범위 합산은 실제 줄 개수 안으로
클램프한다.

상한을 `line_heights.len()` 하나가 아니라 `line_heights`와 `line_spacings`의 최소 길이로
정한 판단이 맞다. 두 벡터를 같은 인덱스로 읽는 함수이므로 한쪽만 방어하면 기존 panic
경로가 남는다. 경계 내부 advance의 계산은 바꾸지 않으며, 역전되거나 완전히 범위 밖인
range는 빈 범위로 정규화된다.

호출부의 stale `PartialParagraph` 줄 범위가 생기는 근본 원인은 별도로 추적할 수 있지만,
레이아웃 방어 계층에서 렌더를 중단시키지 않는 이 수정은 독립적으로 필요하다. 추가한
유닛 회귀는 실측 경계(`31`개 줄의 index `31`)와 range 상한 초과를 모두 고정한다.

## 로컬 검증

모든 Cargo 실행은 `CARGO_INCREMENTAL=0`,
`CARGO_TARGET_DIR=target/review-twoloop-40-20260803`으로 분리했다.

| 게이트 | 결과 |
| --- | --- |
| `issue_3780_line_advance_oob` library unit | 2 / 2 통과 |
| 전체 library | 3,170 통과, 7 ignored, 실패 0 |
| 전체 integration | `cargo test --profile release-test --tests` 종료 코드 0 |
| SVG snapshot / visual baseline | 전체 integration 안에서 각각 8 / 8, 3 / 3 통과 |
| Native Skia library | 58 / 58 통과 |
| Native Skia placeholder / direct PDF | 각각 2 / 2, 4 / 4 통과 |
| WASM | `wasm-pack build --target web --out-dir pkg` 성공 |
| 형식·정적 검사 | `cargo fmt --check`, `git diff --check`, `cargo clippy --all-targets -- -D warnings` 모두 통과 |

## 시각 검증 범위

변경은 typeset renderer의 페이지 advance에 닿으므로 시각 검증 대상이다. 다만 #3780의
실제 재현 문서는 실서비스 비공개 자료여서 원 HWP/HWPX와 한컴 기준 PDF가 제공되지 않았다.
따라서 본 검토는 독립 PDF/SVG visual sweep을 수행하거나 그 결과를 주장하지 않는다.

대신 최신 누적 head에서 SVG snapshot 8개, visual round-trip baseline 3개, pagination·TAC
경계 fixture를 포함한 전체 integration이 통과했음을 회귀 근거로 남긴다. 원본을 비식별화해
제공할 수 있게 되면, 같은 연속 재배치 구간의 한컴 PDF를 기준으로 별도 visual sweep을 추가해야 한다.

## 현재 판정

**통합 PR 수용 권고.** 최신 `devel` 위 누적 적용에서 범위 방어의 의미와 기존 계산 보존을
확인했고, focused·전체·native·WASM·정적 검증이 모두 성공했다. 원 PR의 CI 성공은 참고로만
기록하며, 실제 merge 전에는 통합 PR head의 최신 원격 CI와 작업지시자 승인을 다시 확인한다.
