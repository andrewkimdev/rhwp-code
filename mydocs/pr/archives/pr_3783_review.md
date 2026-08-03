---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3783 검토 — TAC 앵커 migration의 packed attr 동기화

## 라우팅과 적용 경계

base route는 `maintainer_general.md`이며, `intake_and_review.md`,
`local_validation.md`, `visual_fixture_evidence.md`,
`multi_pr_update_branch.md`를 함께 적용했다.

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3783](https://github.com/edwardkim/rhwp/pull/3783) / @twoLoop-40 (Lee Joon ho) |
| 관련 이슈 | [#3781](https://github.com/edwardkim/rhwp/issues/3781) |
| 원 기능·head commit | `003483a5356afc804e055378d998dea762ff94c9` |
| 검토 기준 | `upstream/devel` `9095cd52d` |
| 가시성 통합 검토 브랜치 | `review/twoloop-40-20260803` |
| 누적 적용 commit | `003483a5` → `e11868d15` (`cherry-pick -x`) |
| 함께 누적한 PR | [#3782](https://github.com/edwardkim/rhwp/pull/3782) |
| 충돌 | 없음 |
| 작성 시점 원 PR 상태 | GitHub `mergeStateStatus=UNKNOWN`; 원 head CI 전체 성공, 단 최신 `devel`보다 이전 head |

reviewer는 `jangster77`로 지정했다. 원 contributor branch에는 push하지 않았다.

## 변경 검토

그림 삽입은 non-zero `CommonObjAttr.attr`에 floating anchor 비트를 시드하지만, TAC
토글과 `migrate_picture_floating_to_inline`은 enum 필드만 바꾸고 packed attr을 남겼다.
직렬화기는 non-zero raw attr을 우선하므로, HWP 왕복 후 `treatAsChar=1`과 Paper anchor가
공존하는 모순 상태가 다시 나타날 수 있었다.

`sync_anchor_bits`는 TAC bit 0, 세로 관계 bit 3–4, 가로 관계 bit 8–9만 지운 뒤 enum
값으로 다시 기록한다. criterion·wrap·flow를 포함한 나머지 raw bit는 보존한다. serializer의
실제 bit mapping(세로 Paper/Page/Para = 0/1/2, 가로 Paper/Page/Column/Para = 0/1/2/3)과
일치함을 확인했다.

호출은 본문 TAC 토글, header/footer의 동등한 토글 경로, floating-to-inline migration 끝에
있다. `attr == 0`은 건드리지 않아 serializer가 enum으로 새 값을 합성하는 기존 계약을 유지한다.
기존 TAC 통합 fixture와 새 stale-floating seed 회귀가 이 두 경계를 함께 검사한다.

## 로컬 검증

모든 Cargo 실행은 `CARGO_INCREMENTAL=0`,
`CARGO_TARGET_DIR=target/review-twoloop-40-20260803`으로 분리했다.

| 게이트 | 결과 |
| --- | --- |
| `sync_anchor_bits_` library unit | 2 / 2 통과 |
| `tac_toggle_body_floating_to_inline` | 1 / 1 통과 |
| `integration_tac_toggle_matches_hancom_scenario` | 4 / 4 통과 |
| 전체 library | 3,170 통과, 7 ignored, 실패 0 |
| 전체 integration | `cargo test --profile release-test --tests` 종료 코드 0 |
| SVG snapshot / visual baseline | 전체 integration 안에서 각각 8 / 8, 3 / 3 통과 |
| Native Skia library | 58 / 58 통과 |
| Native Skia placeholder / direct PDF | 각각 2 / 2, 4 / 4 통과 |
| WASM | `wasm-pack build --target web --out-dir pkg` 성공 |
| 형식·정적 검사 | `cargo fmt --check`, `git diff --check`, `cargo clippy --all-targets -- -D warnings` 모두 통과 |

## 시각 검증 범위

원 #3781의 실제 HWP는 비공개 자료여서 원본 파일과 한컴 기준 PDF가 제공되지 않았다. 따라서
이번 검토에는 독립 PDF/SVG visual sweep이나 한컴 화면 동일성 주장이 없다. 전체 integration의
TAC HWP/HWPX roundtrip·Hancom scenario 회귀와 SVG snapshot/visual baseline은 기존 그림·anchor
경로를 깨지 않았다는 자동 회귀 근거이며, 실제 깨진 그림의 외관 자체를 판별한 증적은 아니다.

비식별화된 HWP를 제공할 수 있으면 TAC 전환 전후의 HWP 저장·한컴 재열기·PDF 비교를 별도
fixture로 추가하는 것이 다음 증적 보강이다.

## 현재 판정

**통합 PR 수용 권고.** raw packed attr 우선 직렬화 계약과 enum mapping을 따라, 필요한 anchor
bit만 동기화하고 다른 bit와 `attr == 0` 합성 경로를 보존한다. 최신 `devel` 위 cumulative
검증도 성공했다. 실제 merge 전 통합 PR head의 최신 원격 CI와 작업지시자 승인을 다시 확인한다.

후속 [통합 PR #3881](https://github.com/edwardkim/rhwp/pull/3881)의 code head
`b88bd6e80`에서는 [CI](https://github.com/edwardkim/rhwp/actions/runs/30808394555),
[CodeQL](https://github.com/edwardkim/rhwp/actions/runs/30808395173),
[Render Diff](https://github.com/edwardkim/rhwp/actions/runs/30808395295)가 모두 성공했다.
