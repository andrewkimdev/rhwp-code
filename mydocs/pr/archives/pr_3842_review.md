---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3842 검토 — 검색 hit의 문맥 문단 반환

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3842](https://github.com/edwardkim/rhwp/pull/3842) / @kevin9327 |
| 원 head · 적용 source | `b83953c15` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |

## 검토

`--context`가 검색 hit 앞뒤 문단을 같은 질의 응답에 넣어, 후속 grep 왕복 없이 판정할 수 있게
한다. CLI와 MCP의 동일 query 계층을 사용하고 context 미지정 시 기존 envelope를 유지하므로
호환성 경계가 명확하다.

## 누적 검증과 판정

전체 release-test `exit-0`, clippy `exit-0`, agent preflight의 CLI/MCP 커버 검사를 통과했다.
**통합 수용 권고.**
