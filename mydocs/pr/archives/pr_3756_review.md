---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3756 검토 기록

## 판정

[PR #3756](https://github.com/edwardkim/rhwp/pull/3756)는 HWP3 셀 padding과 HWPX OLE 위치의
음수값을 signed 값으로 보존한다. 두 source commit을 `cdd55c838`, `8aa27b19b`로 순서대로
적층했다.

## 범위와 검증

- 작성자·base: `@kevin9327` / `devel`; 기준 `upstream/devel@a8d7bdfbf`.
- HWP3/HWPX parser 회귀와 release-test 전체를 실행해 signed 경계를 확인했다.
- #3757·#3764가 같은 padding helper를 건드려, 누적 tree에는 helper 하나와 각 PR의 negative
  regression을 함께 남겼다.
- renderer/golden 변경이 아니므로 시각 증적은 생략한다.

충돌 해소 근거는 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)에 있다.
