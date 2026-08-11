---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3748 검토 기록

## 판정

[PR #3748](https://github.com/edwardkim/rhwp/pull/3748)는 `search`에서 `-`로 시작하는 검색어를
`--` 뒤에 전달할 수 있게 한다. `659e2a13e`로 적층했으며, 기존 옵션 해석을 바꾸지 않는 작은
CLI 경계 수정이다.

## 범위와 검증

- 작성자·base: `@kevin9327` / `devel`; 기준 `upstream/devel@a8d7bdfbf`.
- `search_dash_query_contract` 4건과 release-test 전체 gate를 실행해 통과했다.
- parser/layout/fixture 변경이 없으므로 시각 증적은 불필요하다.
- local 검토 판정은 반영 가능이다. 원격 CI만 push 뒤 최신 integration head에서 확인한다.

누적 충돌·검증은 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)을 따른다.
