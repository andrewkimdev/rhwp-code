---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3758 검토 기록

## 판정

[PR #3758](https://github.com/edwardkim/rhwp/pull/3758)은 HWPX OLE/차트/도형 offset 음수값이
unsigned 변환 중 유실되지 않도록 한다. 두 기능 commit을 `51adfaa69`, `cb0461308`로 적층했다.

## 범위와 검증

- 작성자·base: `@kevin9327` / `devel`; 기준 `upstream/devel@a8d7bdfbf`.
- 동일 parser 경로의 `parse_i32_wrapping`을 유지하고 signed 및 wrapped-unsigned regression을
  함께 보존했다.
- source branch에 포함된 #3766 BrokenPipe commit은 이미 #3778로 merge됐으므로 이중 적용하지
  않았다.
- HWPX parser focused test와 release-test 전체를 적용하며 시각 증적은 불필요하다.

적층 경계는 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)에 기록한다.
