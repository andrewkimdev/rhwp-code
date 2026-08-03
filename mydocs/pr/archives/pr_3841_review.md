---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3841 검토 — `edit redact --no-raw` 개인정보 원문 비노출

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3841](https://github.com/edwardkim/rhwp/pull/3841) / @kevin9327 |
| 원 head · 적용 source | `017e883ce` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |
| 충돌 | #3827의 CLI 문서 보강과 함께 유지 |

## 검토

마스킹 전후를 검토하기 위한 findings envelope에서 원문 `raw`를 선택적으로 제외한다. 기본 동작과
실제 redaction 결과는 보존하면서, review JSON을 공유해야 하는 경우에만 최소 정보로 축소한다.
형식만 바꾸고 sanitizer 결과를 바꾸지 않는 경계가 계약 테스트로 고정돼 있다.

## 누적 검증과 판정

`redact_sanitize_contract` 13/13, 전체 release-test `exit-0`, clippy `exit-0` 통과.
**통합 수용 권고.**
