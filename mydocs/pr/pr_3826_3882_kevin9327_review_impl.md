---
kind: review_plan
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# kevin9327 누적 통합 검토·반영 계획 — PR #3826~#3882 (#3808 제외)

## 범위와 기준

기준은 `upstream/devel`의 `ac085e1d1e6caaea00a2151a60094b7098a96206`이다. 사용자가 기본
작업트리의 devel 위에서 진행 상황이 보이도록 지시한 데 따라, 가시성 branch
`review/kevin9327-20260803`에 열린 contributor PR을 오래된 번호 순으로 누적했다. 원 fork branch나
원 PR에는 push하지 않았다.

사용자 지시에 따라 draft [#3808](https://github.com/edwardkim/rhwp/pull/3808)은 누적에서
**제외**했다. 이 문서는 #3808을 수용·검토·merge 권고하는 기록이 아니다.

| 순서 | 원 PR | 적용한 contributor source SHA | 누적 적용 결과 / 비고 |
| --- | --- | --- | --- |
| 1 | #3826 | `0e2919a0e`, `2eabe53d3` | 결정 문서 2개 |
| 2 | #3827 | `440b6ba5e`, `786584516` | #3841과 CLI 문서 충돌 병합 |
| 3 | #3832 | `7dc857b44`, `6c441525a`, `f4a427e20`, `00c26ed90` | explain 4 commit |
| 4 | #3835 | `76360dc1b`, `f7af6a34f`, `bfba31702`, `b3d6ad8d1` | recipe·index 4 commit |
| 5 | #3836 | `56e75f895` | capabilities search |
| 6 | #3838 | `be8202517` | profile audit |
| 7 | #3839 | `465704945` | password stdin parity |
| 8 | #3841 | `017e883ce` | redact no-raw |
| 9 | #3842 | `b83953c15` | search context |
| 10 | #3843 | `376b9a790`, `027d415c3` | agent manifest |
| 11 | #3867 | `bfe56d4b2`, `dd26c9dc5`, `0916940c2` | source branch의 devel merge commit은 제외 |
| 12 | #3870 | `8bbf7968f`, `a546f798b` | table-cell search |
| 13 | #3871 | `a1c5b82c7` | batch extract-data |
| 14 | #3872 | `73d71c313` | preflight false-positive |
| 15 | #3873 | `eba956496` | M24 WASM 문서 |
| 16 | #3875 | `6173318d2` | WMF negative count |
| 17 | #3876 | `5e2d699a8` | runtime 문서 |
| 18 | #3877 | `9d70068e1` | fuzz 운영 문서 |
| 19 | #3878 | `6ec061d4e` | M25 문서 |
| 20 | #3879 | `3373ba877` | bindings 문서 |
| 21 | #3882 | `3478b2da8` | envelope integrity |

원 PR의 devel 병합 commit은 기능 source가 아니므로 #3867에서 누적하지 않았다. 나머지 적용 중
#3841, #3878, #3879의 문서 index 충돌은 각 선행 PR의 내용을 버리지 않고 함께 보존했다.

## 검토에서 발견해 추가한 메인터너 보정

| 누적 commit | 보정 | 근거 |
| --- | --- | --- |
| `a29e0591e` | visual regression recipe trailing whitespace 제거 | `git diff --check` 실패 2건 |
| `858a743e5` | `hwp_explain` profile 등재, manifest meta-only 분류, textbox/equation을 표 셀 navigation opt-in에서 제외 | profile contract 실패와 #3870의 parent-cell replace 범위 누수 |
| `8e80e4a47` | 존재하지 않는 recipe 링크를 CLI 레퍼런스로 교체, M24 문서 3건을 허용 `active` status로 보정 | Markdown link checker 1건 및 metadata checker의 새 오류 3건 |

`858a743e5`는 #3870의 ordinary table-cell search를 유지하면서 textbox와 equation hit이 Studio의
cell replacement로 잘못 해석되지 않게 `SearchHit` 경계를 명시했다. 신규 negative regression을
`issue_3865_search_text_in_table_cells`에 추가했다.

## 검증 결과

| 게이트 | 결과 |
| --- | --- |
| `explain_contract` | 13/13 통과 |
| `issue_3865_search_text_in_table_cells` | 4/4 통과, textbox 음성 회귀 포함 |
| `agent_profile_router_contract` | 8/8 통과 |
| `cli_json_contract` | 31/31 통과 |
| password/redact/batch/envelope/WMF focused | 2/2, 13/13, 8/8, 5/5, 3/3 통과 |
| `provenance_contract` | 7/7 통과 |
| `cargo test --profile release-test --tests` | 전용 `target/review-kevin9327-20260803`, `CARGO_INCREMENTAL=0`, 명시적 `exit-0` |
| `cargo fmt --check` / `git diff --check` | 통과 |
| `cargo clippy --all-targets -- -D warnings` | 명시적 `exit-0` |
| `python3 tools/agent_preflight.py --bin …/release-test/rhwp` | 111 flags, 63 commands 등 전부 통과 |
| `npx tsc --noEmit` / `npm test` | 통과 / 749/749 통과 |
| `wasm-pack build --target web` | 전용 `target/review-kevin9327-20260803/wasm-pkg`, 명시적 `exit-0` |
| Markdown link checker | 480개 문서 내부 상대 링크 이상 없음 |

문서 metadata 전체 검사에는 이 누적과 무관하게 이미 존재하던 `envelope_provenance.md`의
`kind: contract`와 `task_m100_3604_password_encryption_cpp_review.md`의 metadata 2건이 남아
있다. 이번에 추가된 M24 문서의 3건은 위 보정으로 해소했다. renderer/layout/typeset 출력 변경은
없으므로 HWP/PDF visual fixture 증적 대상은 아니다.

## 다음 단계

1. review 문서를 포함한 head에서 LFS 대상 여부와 staged diff를 확인한다.
2. 작업지시자 승인 뒤 `upstream`의 임시 head branch로 push하고 `devel` 대상 통합 PR을 만든다.
3. 통합 PR의 최신 head, mergeability, required Actions를 확인한다. 원 PR #3826·#3827·#3832·#3835·#3836·#3838·#3839·#3841·#3842·#3843·#3867·#3870·#3871·#3872·#3873·#3875·#3876·#3877·#3878·#3879·#3882도 최신 상태를 개별 재확인한다.
4. 최신 CI 성공과 작업지시자 승인 뒤 통합 PR만 merge하고, 실제 merge commit 및 원 PR close 상태를 확인한 뒤 post-merge 절차를 수행한다.
