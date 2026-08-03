---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3839 검토 — password CLI 선언과 stdin 배선 동등성

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3839](https://github.com/edwardkim/rhwp/pull/3839) / @kevin9327 |
| 원 head · 적용 source | `465704945` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |

## 검토

`--password`를 선언했지만 password stdin을 받지 못하던 7개 명령을 동일한 dispatch 경로로
배선하고, 선언·CLI parity를 전수 가드로 고정한다. 비밀값을 argv나 출력에 노출하는 우회가 아니라
기존 stdin 계약을 누락 명령까지 일관되게 적용한 변경이다.

## 누적 검증과 판정

`cli_password_stdin_command_parity_contract` 2/2 및 전체 release-test `exit-0`을 확인했다.
**통합 수용 권고.**
