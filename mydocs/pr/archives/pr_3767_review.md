---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3767 검토 기록

## 판정

[PR #3767](https://github.com/edwardkim/rhwp/pull/3767)은 HWP5 저장 때 BMP 밖 문자를 `u16`으로
축소해 손상시키던 경로를 보정한다. 기능·format commit을 `0b72d3bf2`, `d6e2c7f03`으로 적층했다.

## 범위와 검증

- 작성자·base: `@kevin9327` / `devel`; 기준 `upstream/devel@a8d7bdfbf`.
- source에 포함된 #3766 BrokenPipe duplicate은 제외했다.
- serializer의 비-BMP 문자 roundtrip regression과 release-test 전체를 실행해 통과했다.
- 새 PDF/HWP fixture baseline을 변경하지 않아 시각 증적은 적용하지 않는다.

최종 누적 gate는 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)에 따른다.
