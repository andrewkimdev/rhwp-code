---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3838 검토 — 에이전트 profile 도구 등재 전수 감사

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3838](https://github.com/edwardkim/rhwp/pull/3838) / @kevin9327 |
| 원 head · 적용 source | `be8202517` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |
| 선행 의존 | #3832·#3843의 새 무상태 도구를 포함한 최종 tool set |

## 검토

프로필마다 도구가 빠져 routing 단계에서 보이지 않던 14건을 보완하고, 계약 테스트가 모든 무상태
도구의 귀속을 요구하도록 만든 변경이다. 누적 적용 뒤 새 `hwp_explain`과
`hwp_export_agent_manifest`가 다시 빠진 것을 발견해, 전자는 업무 profile에, 후자는 meta-only
설계 범주에 명시적으로 배치했다. 이는 profile 경계를 무분별하게 확장하지 않는 보정이다.

## 누적 검증과 판정

`agent_profile_router_contract` 8/8, 전체 release-test `exit-0`, agent preflight 전부 통과.
**통합 수용 권고.**
