---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-31
---

# PR #3610 리뷰 — 에이전트 표면 playbook canonical화

- PR: [#3610](https://github.com/edwardkim/rhwp/pull/3610)
- 작성자: `kevin9327`
- 역할: collaborator 매개 외부 PR — `intake_and_review`, `local_validation`,
  `multi_pr_update_branch` 적용

원 contributor commit `19e16734f`는 CLI `--json`과 MCP 도구 추가 시의 구현·계약·증적 수용 기준을
`mydocs/manual/agent_surface_playbook.md`로 canonical화하고 manual index에서 연결한다. 기능 코드나 기존
문서 구조를 바꾸지 않으며, #3597·#3599·#3602·#3607에서 검토한 표면 계약을 재사용 가능한 절차로 정리한다.

최신 `upstream/devel`의 `integrate/kevin9327-20260731`에 다른 네 PR 뒤 마지막으로 cherry-pick했고 충돌은
없었다. 관련 Markdown link checker와 metadata scanner는 통과했다. 이 PR은 문서 전용이고 renderer·layout
변경이 없어 visual sweep 대상이 아니다.

**통합 PR merge 권고.** 문서만 별도로 fast-pass하지 않고, 다섯 원 PR을 담은 code 통합 PR의 full CI를
merge gate로 사용한다. 통합 PR merge 뒤 원 #3610에는 통합 PR 링크와 검토 결과를 남긴 뒤 supersede close한다.
