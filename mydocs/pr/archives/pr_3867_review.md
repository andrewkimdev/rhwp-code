---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3867 검토 — 실제 samples 기반 악성 코퍼스 회귀

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3867](https://github.com/edwardkim/rhwp/pull/3867) / @kevin9327 |
| 원 head · 적용 source | `0916940c2` · `bfe56d4b2`, `dd26c9dc5`, `0916940c2` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |

## 검토

실제 fixture에서 찾은 hidden text·unicode·injection 신호를 양성 코퍼스로 고정하고, 세 detector가
같은 부정 샘플 집합을 오탐 없이 통과하는지 검사한다. 허용목록의 존재만이 아니라 허용목록 자체를
검증해 보안 회귀가 테스트의 맹점을 통해 숨지 않게 한 점이 중요하다.

## 누적 검증과 판정

`security_corpus_regression`은 전체 release-test 내에서 실물 CLI sweep까지 완료했고 Cargo가
명시적 `exit-0`으로 종료했다. **통합 수용 권고.**
