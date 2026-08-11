---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3916 검토 — 쪽수 실패와 IR 차이의 동시 보고

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3916](https://github.com/edwardkim/rhwp/pull/3916) / @kevin9327 |
| 원 head | `197391d9b063cf1388d6fc164dd062665966c51d` |
| 누적 적용 commit | `ae166a73c` (`cherry-pick -x`) |
| 현재 PR 기준 / 누적 branch | `upstream/devel` `874dae394` / `review/kevin9327-20260804` |

## 적용 범위와 중복 제외

원 PR의 첫 source commit `d0a1dc360`은 이미 `devel`에 들어온 #3884 diagnostics unknown-flag
계약을 다시 넣고, 같은 성격의 회귀를 중복 추가한다. 이 통합 후보에서는 그 commit을 적용하지 않았다.
문제 본체인 `197391d9b`만 `-x`로 적용해 author와 source SHA provenance를 보존했다.

## 검토

`export-hwpx`와 `convert`에서 `--verify-pages` 실패를 기록한 뒤에도 `--verify` IR 비교를 마저
수행한다. 두 축이 함께 실패하면 두 진단과 JSON `verifyPages`·`verify`를 모두 남기고, 기존 계약대로
쪽수 불일치 exit 4를 우선한다. 실패 중 통과 메시지를 내지 않는 경계와 단독 축·정상 문서도 회귀로
고정했다.

메인터너 보정 `38d3b3b75`은 신규 CLI test가 nextest archive의 runtime
`CARGO_BIN_EXE_rhwp` 주입 경로를 우선하도록 바꿔 로컬과 CI 실행 경로를 일치시켰다.

## 판정

focused 회귀 3건, 실제 JSON 출력, 전체 `cargo test --profile release-test --tests` 종료 코드 0을
확인했다. **통합 수용 권고.**
