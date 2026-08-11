---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3769 검토 기록

## 판정

[PR #3769](https://github.com/edwardkim/rhwp/pull/3769)은 HWP5 표 셀의 `row_span`/`col_span`이
0일 때 underflow panic을 피한다. `ed339e78f`로 적층했다.

## 범위와 검증

- 작성자·base: `@kevin9327` / `devel`; 기준 `upstream/devel@a8d7bdfbf`.
- source에 포함된 #3766 duplicate은 이미 merge돼 제외했다.
- HWP5 table span zero regression과 release-test 전체를 실행해 통과했다.
- renderer/golden/기준 PDF 변경이 없어 시각 증적은 요구하지 않는다.

최종 반영 조건은 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)의 최신 CI 성공이다.
