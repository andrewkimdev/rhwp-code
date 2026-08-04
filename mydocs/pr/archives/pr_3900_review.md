---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3900 검토 — 저장 vpos 되돌아감을 쪽 경계로 판정

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3900](https://github.com/edwardkim/rhwp/pull/3900) / @planet6897 |
| 원 head | `d222a6d4947f2fe1d365e66445bb6d6109275214` |
| 누적 적용 commit | `685557488`, `8630d3f81` (`cherry-pick -x`) |
| 기준 / 누적 branch | `upstream/devel` `21affdafc` / `review/planet6897-20260804` |

## 검토

저장된 line-segment의 `vpos`가 앞 문단보다 되돌아가는 HWP에서는 그 되돌아감이 다음 쪽 시작을
뜻할 수 있다. 이 PR은 해당 신호를 쪽 경계로 인정하고, 적용 범위를 문단 시작으로 한정해 한 문단만
잘못 옮겨지던 축을 고친다. 새 실물 fixture
`samples/issue3837/stored_vpos_rewind_form.hwp`가 이를 고정하며, focused 회귀는 앞 문단의 큰
`vpos`와 되돌아간 항목이 각각 쪽 2에 있음을 확인한다.

누적 검토에서 synthetic line segment가 저장 위치처럼 선택될 수 있는 인접 경계를 발견했다. 별도
메인터너 보정 `b2905829a`는 이전·현재 문단 모두에서 synthetic segment를 제외해 실제 저장 위치만
비교하도록 좁혔다. contributor 원 변경을 rewrite하지 않은 추가 보정이다.

## 시각 증적

한컴 2020에서 같은 fixture를 PDF로 변환했다. 원본 HWP SHA-256은
`b1609c633619fdc5203c33ab542eca97b00211717d618f55ee2869106c895652`, PDF SHA-256은
`1d7b22cc462bcc77cd0f5577cf6e0671dd6c3c7970e9c0e8fc42c5e0097dd013`이며, 둘 다 4쪽이다.

- 기준 PDF: [stored_vpos_rewind_form-2020.pdf](../../../pdf/issue3837/stored_vpos_rewind_form-2020.pdf)
- review PNG: [쪽 2](../assets/pr_3900_stored_vpos_rewind_p002_review.png)
- 생성물: `output/review-planet6897-20260804/visual-issue3837/pr3900-stored-vpos-rewind/`
- sweep 결과: SVG/render tree/PDF raster 4/4쪽, overflow·equation·square-wrap·line-order·tail 후보 0건.
  자동 `column_text_flow_collapse` 후보는 2·4쪽뿐이었다.

쪽 2를 PDF와 사람 검토로 대조한 결과 표·문단 순서와 쪽 경계가 일치했다. `pixel_match` 94.742%와
ink proxy 7.2045%는 이 환경의 글꼴 metric 차이로 줄마다 약 13px 이동한 데 따른 보조 지표이며,
형상 붕괴 결함은 아니다.

## 판정

`issue_3837_stored_vpos_rewind_page_break` 2건과 synthetic-line focused 회귀가 통과했고,
전체 release-test·native-Skia·WASM 검증도 최신 누적 후보에서 성공했다. **메인터너 보정을 포함한
통합 수용 권고.**
