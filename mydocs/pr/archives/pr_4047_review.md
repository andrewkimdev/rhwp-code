# PR #4047 검토

## 결론

**통합 PR로 수용한다.** WASM도 `EmbeddedTextMeasurer`를 사용하도록 통일해 native와 SVG
텍스트 위치가 달라지던 경로를 제거하고, 비도달 WASM 측정기 구현을 정리한다.

| 항목 | 내용 |
| --- | --- |
| 작성자·대상 | `planet6897`, `devel` |
| source head | `b1ee94e9990de53a9a72613376dc9e1d55f45637` |
| 통합 적용 | `88c68492b`~`26d8fc5bd` |
| 관련 이슈 | #4046 |
| 변경 | EmbeddedTextMeasurer 통일, native↔WASM SVG 패리티 하네스·가이드 추가 |

## 검증과 위험

- `test_embedded_measurer` 3개와 최신 전체 `release-test --tests` 467개가 성공했다.
- `wasm-pack build --target web`이 통과했고 산출 wasm 모듈을 확인했다.
- 변경은 SVG/WASM 출력 경로다. studio canvas replay의 직접 동작 변경은 아니므로, browser UI 회귀는
  기존 renderer contract 및 원격 CI에서 계속 확인한다.

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  maintainer_general.md, intake_and_review.md,
                  local_validation.md, multi_pr_update_branch.md
```
