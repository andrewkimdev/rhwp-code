# PR #4041 검토

## 결론

**통합 PR로 수용한다.** 저장 사다리의 tail-fit이 누적 초과를 모르고 여러 줄을 통과시키던 문제에
연쇄 상한을 추가한다. #4077과 같은 루프를 수정하므로 메인터너가 두 조건을 순수 경계 테스트로
고정했으며, 최신 통합 head의 전체 release-test가 통과했다.

| 항목 | 내용 |
| --- | --- |
| 작성자·대상 | `planet6897`, `devel` |
| source head | `44daea010019c0e7602a95cd90e844bebae432af` |
| 통합 적용 | `c506c949a` |
| 관련 이슈 | #4024 |
| 변경 | 저장 꼬리줄 연쇄 상한 2를 도입해 쪽 밖 소실을 줄인다. |

## 검증과 위험

- `saved_tail_fit_chain` 2개와 기존 multiline saved-vpos 회귀가 통과했다.
- 메인터너 보정은 첫 두 줄 허용·세 번째 분할과 HWP 권위/native HWP5 예외를 함께 검증한다.
- 전체 `release-test --tests` 467개가 최신 rebase head에서 성공했다.
- 상한 값은 표본 관찰에서 나왔으므로, 이후 fidelity 변화는 #4024의 후속 계측으로 추적한다.

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, visual_fixture_evidence.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  maintainer_general.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
```
