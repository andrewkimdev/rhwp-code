# PR #4066 검토

## 결론

**통합 PR로 수용한다.** PCX v2.8, WMF 폰트 힌트, 팔레트 TIFF, 대형 BMP, DOS EPS preview의
이미지 발산 잔여를 각 포맷의 안전 경계 안에서 처리한다. #4057/#4060 선행 stack은 중복 적용하지 않았다.

| 항목 | 내용 |
| --- | --- |
| 작성자·대상 | `planet6897`, `devel` |
| source head | `f0631023a2dee6545fc7bc757edb86046a108bd3` |
| 통합 적용 | `126127997`~`8776da74b` |
| 관련 이슈 | #4063, #4064, #4065 |
| 변경 | 남은 이미지 형식 판별·변환과 r3 조사 기록 |

## 검증과 위험

- image resolver 14개, WMF/EMF 2개, 전체 release-test 467개가 성공했다.
- BMP는 CanvasKit 한도 안으로만 축소하고, 손상 헤더·텍스트 PostScript는 계속 거부한다.
- WMF 폰트 hint의 미지 값은 도형 좌표와 무관한 경우에만 DEFAULT로 허용한다.

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, visual_fixture_evidence.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  maintainer_general.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
```
