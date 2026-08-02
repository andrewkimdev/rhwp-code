---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3750 검토 기록

## 판정

[PR #3750](https://github.com/edwardkim/rhwp/pull/3750)는 MCP `hwp_doc_save`가 요청한 output
확장자를 무시하지 않도록 저장 포맷 결정을 보정한다. `d11831798`로 적층했다.

## 범위와 검증

- 작성자·base: `@kevin9327` / `devel`; 기준 `upstream/devel@a8d7bdfbf`.
- `mcp_session_edit_contract` 37건과 누적 release-test 전체로 HWP/HWPX 저장 계약을 확인했다.
- 문서 변환 결과를 새 fixture나 기준 PDF로 주장하지 않으므로 시각 증적은 적용하지 않는다.
- local 검토 판정은 반영 가능이다. 원격 CI만 push 뒤 최신 integration head에서 확인한다.

상세 반영 SHA와 전체 결과는 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)에 기록한다.
