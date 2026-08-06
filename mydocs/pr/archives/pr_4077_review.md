# PR #4077 검토

## 결론

**통합 PR로 수용한다.** 각주가 있는 쪽에서 저장 LineSeg가 각주 영역 위에 있고 흐름 커서도 그보다
위일 때만 안전마진을 조건부로 넘겨, 한컴보다 이르게 페이지를 끊던 문제를 줄인다.

| 항목 | 내용 |
| --- | --- |
| 작성자·대상 | `planet6897`, `devel` |
| source head | `fc70872cbbaf365871455a27c276499be8f388a3` |
| 통합 적용 | `e21ffecdd` |
| 관련 이슈 | #4054 |
| 변경 | 각주 안전마진의 저장 좌표·흐름 커서 조건부 예외 |

## 검증과 위험

- `saved_line_clears_footnote_area` 경계 테스트와 최신 전체 release-test 467개가 성공했다.
- #4041과의 같은 루프 변경은 메인터너 보정에서 함께 검증했다. 각주가 없거나 다단인 경우, 저장
  bounds가 본문 한계를 넘는 경우에는 예외를 열지 않는다.
- 작성자가 기록한 모집단에서 회귀 1건이 있어, 페이지수 fidelity의 추가 계측은 #4054 후속 범위로
  유지한다.

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, visual_fixture_evidence.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  maintainer_general.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
```
