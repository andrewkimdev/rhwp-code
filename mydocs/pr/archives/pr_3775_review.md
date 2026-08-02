---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3775 검토 기록

## 판정

[PR #3775](https://github.com/edwardkim/rhwp/pull/3775)는 Python M18 바인딩, `export-ir-schema`,
패키지 문서·CI를 추가한다. 기능과 문서 commit을 `6eac4249a`부터 `404288c6f`까지 순서대로
적층했다.

## 누적 보정

- #3779가 capabilities schema command를 추가하므로 Python public surface에도
  `capabilities_schema()`와 envelope API를 보완했다. integration test로 두 schema command가
  바인딩에서 누락되지 않음을 확인했다.
- 개발 도구는 Python 3.8 호환 mypy와 검토한 ruff 범위로 고정했고, generator와 생성 모델의
  type/lint 잔여를 보정했다.
- source cleanup이 제거한 #3761 dry-run 회귀는 별도 reviewer commit에서 복원했다.

## 검증

- Python 3.12에서 ruff, mypy, 단위·통합 254건, wheel/sdist build와 example을 실행해 모두 통과했다.
- public binding/API/CI 변경이지만 renderer 변경은 없어 시각 증적은 적용하지 않는다.

상세 보정과 최종 결과는 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)에 있다.
