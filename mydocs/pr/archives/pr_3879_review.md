---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3879 검토 — Python·Node 바인딩 동등성 계약

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3879](https://github.com/edwardkim/rhwp/pull/3879) / @kevin9327 |
| 원 head · 적용 source | `3373ba877` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |
| 충돌 | #3873 M24·#3878 M25 map 항목을 모두 보존하고 bindings 항목을 추가 |

## 검토

Python·Node에서 표면이 드리프트한 20개 항목을 parity contract, 새 바인딩 절차, 실측 대조로
정리한다. 현재 확인된 사실과 장래 binding 설계를 구분하고, 언어별 차이를 공통 JSON/envelope
계약으로 판단하게 한 점이 적절하다.

## 누적 검증과 판정

상대 링크 검사 480개와 `git diff --check` 통과. **통합 수용 권고.**
