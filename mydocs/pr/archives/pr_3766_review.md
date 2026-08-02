---
kind: review
status: superseded-by-merged-pr
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3766 검토 기록

## 판정

[PR #3766](https://github.com/edwardkim/rhwp/pull/3766)의 BrokenPipe stdin 계약 테스트는 이미
[#3778](https://github.com/edwardkim/rhwp/pull/3778)에서 `devel`에 merge된 동일 patch다. 따라서
`review/kevin9327-20260802`에는 다시 cherry-pick하지 않는다.

## 근거와 후속

- 작성자·base: `@kevin9327` / `devel`; 기준 `upstream/devel@a8d7bdfbf`.
- #3758, #3767, #3769 source branch에 포함된 같은 commit도 모두 이중 적용에서 제외했다.
- 별도 코드 변경·CI 재실행 대상이 아니며, 통합 PR merge 후 superseded 사유를 comment로 남기고
  원 PR을 close한다.

적용/제외 경계는 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)에 고정한다.
