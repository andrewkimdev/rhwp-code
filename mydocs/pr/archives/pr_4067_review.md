# PR #4067 검토

## 결론

**통합 PR로 수용한다.** 표 셀에서 `Square` 어울림 그림이 문단 advance 뒤의 좌표를 사용해 아래로
밀리던 문제를 실제 문단 top 기준으로 보정한다.

| 항목 | 내용 |
| --- | --- |
| 작성자·대상 | `planet6897`, `devel` |
| source head | `7a6b8003875ace30013b57f9656dc651e9c806c1` |
| 통합 적용 | `a67ee894a` |
| 관련 이슈 | #4059 |
| fixture | `samples/156457624_210622 7월부터 해외직구 구매대행업체 등록제 시행.hwp` |
| 기준 PDF | `pdf/156457624_210622-2020.pdf`, SHA-256 `35b258a38db083046e3b94cfb5ae1db57aba33123bdfaa7cbe90a83af7208832`, 5쪽 A4 |

## 시각 검증

- HWP 2020 MCP `PrintToPDFEx`, `PrintMethod=0`으로 기준 PDF를 새로 만들었다. job은
  `4ea2c109-8bb7-459d-938a-329f5b950978`, `run_status=0`, `validation=ok`다.
- 첫 페이지 144dpi 수동 대조는 pixel match 89.065%, visual accuracy proxy 90.915%다. 폰트
  두께·자간 차이 때문에 이를 전체 fidelity 합격선으로 쓰지 않았다.
- 사람이 한컴 기준과 rhwp 모두에서 상단 표 우측 `한국판뉴딜` Square 그림이 셀 경계를 넘지 않고
  보이는 것을 확인했다. 이는 이 PR의 사용자-visible 변경 범위에 직접 대응한다.
- 대표 asset: `mydocs/pr/assets/planet6897_prs_20260806/pr4067_hancom2020_p001.png`,
  `pr4067_rhwp_p001.png`, `pr4067_compare_p001.png`, `pr4067_overlay_p001.png`.

## 로컬 검증과 위험

- `cell_square_picture_anchor` 1개, 전체 release-test 467개, Native Skia 공식 58/2/4가 성공했다.
- `ir_field_sweep_baseline.tsv` 충돌은 새·기존 fixture 행을 모두 이름순으로 유지해 해결했다.
- center valign 등 다른 wrap 계열의 anchor 계약을 넓히지 않았으며, 남은 약 5px 축은 별도 fidelity 과제로 둔다.

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, visual_fixture_evidence.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  maintainer_general.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
```
