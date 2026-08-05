# PR #4057 검토

## 결론

**통합 PR로 수용한다.** WMF를 DOM `<img>` 및 layer 경로에서 SVG로 방출해 브라우저가 raw WMF를
받아 깨지던 문제를 해결한다.

| 항목 | 내용 |
| --- | --- |
| 작성자·대상 | `planet6897`, `devel` |
| source head | `d3592c5582439bf4bd3d57d1952e368e50308c52` |
| 통합 적용 | `a4b1c79fb`~`6de709902` |
| 변경 | WMF 변환 배선·실 fixture 회귀 테스트·oracle 도구 경로 보정 |

## 검증과 위험

- WMF/EMF raw 방출 회귀 2개와 `wmf_flow_image_emitted_as_svg`가 성공했다.
- `image_resolver::tests` 14개와 최신 전체 `release-test --tests` 467개가 성공했다.
- #4060·#4066이 이 변경을 stack으로 포함하므로 WMF 원 commit은 통합에서 한 번만 적용했다.

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, visual_fixture_evidence.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  maintainer_general.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
```
