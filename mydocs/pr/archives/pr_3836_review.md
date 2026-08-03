---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3836 검토 — `capabilities --search`로 명령 발견성 보완

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3836](https://github.com/edwardkim/rhwp/pull/3836) / @kevin9327 |
| 원 head · 적용 source | `56e75f895` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |

## 검토

명령 이름을 모르는 소비자가 capabilities의 이름·설명·키워드로 필요한 동사를 찾도록 하되, 기존
JSON envelope와 무검색 동작을 유지한다. 검색은 기능 권한을 새로 부여하지 않고 discovery만
개선하므로 capabilities schema 경계도 보존된다.

## 누적 검증과 판정

`cli_json_contract` 31/31, 전체 release-test `exit-0`, `agent_preflight.py`의
capabilities/help 상호 커버 63명령 통과. **통합 수용 권고.**
