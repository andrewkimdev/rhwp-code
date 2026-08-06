# PR #4060 검토

## 결론

**통합 PR로 수용한다.** EMF 매직 판별과 standalone SVG 변환을 모든 이미지 방출 경로로 연결한다.
선행 #4057의 WMF commit은 중복 적용하지 않고 이 PR의 EMF 고유 commit만 통합했다.

| 항목 | 내용 |
| --- | --- |
| 작성자·대상 | `planet6897`, `devel` |
| source head | `9c9e67eb57ef1d5191345fa3f782b4cc1430d008` |
| 통합 적용 | `03a0de0a0`~`da2ea6479` |
| 관련 이슈 | EMF 이미지 발산 후속 |
| 변경 | EMF 판별·SVG 변환·DOM/SVG/canvas 배선과 sweep 가이드 |

## 검증과 위험

- WMF/EMF raw 방출 2개, image resolver 14개, 전체 release-test 467개가 성공했다.
- Native Skia 공식 58/2/4와 WASM package가 Stage 1에서 성공했다.
- EMF 매직은 확장자가 아니라 바이트로 판별하므로 잘못된 확장자 fixture도 회귀 범위에 포함된다.

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, visual_fixture_evidence.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  maintainer_general.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
```
