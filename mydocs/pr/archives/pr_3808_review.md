---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3808 검토 — `export-plan-schema`와 조건부 실행 계획 계약

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3808](https://github.com/edwardkim/rhwp/pull/3808) / @kevin9327 |
| 원 head | `e21240f08dc0576ee5a2d9db20a09bc567fceb26` |
| 누적 적용 commit | `b0876fef9` → `fd04add7b` (`cherry-pick -x`) |
| 기준 / 누적 branch | `upstream/devel` `ad67e5a63` / `review/kevin9327-20260804` |

## 검토

실행 계획의 `when` 조건을 JSON schema로 공개하고, schema 산출 명령을 provenance·profile 계약에
연결했다. schema가 허용하는 입력과 runner가 실제로 수용하는 입력이 달라지지 않도록 한 방향은
적절하며, 출처 표지와 profile 예외까지 함께 고정해 agent 소비자가 계약을 찾을 수 있게 했다.

누적 검토에서 `fieldExists`, `textFound`, `fieldEquals.name`의 빈 operand를 schema가 허용하지만
실행기는 거절하는 불일치를 발견했다. 원 기여 commit은 보존하고 별도 메인터너 보정
`fb33dfd72`에서 이 세 필드에 `minLength: 1`을 명시하고 계약 회귀 26건으로 고정했다.
`fieldEquals.value`의 빈 문자열은 유효한 비교값이므로 그대로 허용한다.

## 판정

원 head의 CI 성공을 intake 시 확인했다. 최신 누적 후보에서는 plan-schema 계약 26건과 전체
`cargo test --profile release-test --tests`가 종료 코드 0으로 통과했다. 렌더링 출력을 바꾸지 않는
CLI/schema 변경이므로 별도 PDF visual sweep 대상은 아니다. **통합 수용 권고**이며, 원격 통합 PR의
최신 required CI와 mergeability 확인 뒤에만 merge한다.
