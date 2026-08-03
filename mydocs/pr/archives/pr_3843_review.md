---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3843 검토 — 단일 왕복 `export-agent-manifest`

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3843](https://github.com/edwardkim/rhwp/pull/3843) / @kevin9327 |
| 원 head · 적용 source | `027d415c3` · `376b9a790`, `027d415c3` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |
| 메인터너 보정 | 자기서술 전용 tool로 `meta_only_by_design`에 명시 |

## 검토

agent bootstrap에 필요했던 capabilities·provenance 정보를 하나의 manifest로 묶되, 문서를
변경하거나 추가 권한을 발급하지 않는다. provenance sweep이 manifest의 파생 self-description
성격을 알고 면제하도록 만든 설계도 일관된다. 업무 profile에 억지로 넣지 않고 meta-only로
분류해 라우터의 목적별 경계를 보존했다.

## 누적 검증과 판정

`provenance_contract` 7/7, profile router 8/8, 전체 release-test `exit-0` 통과.
**통합 수용 권고.**
