---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3926 검토 — HWPX raw anchor의 저장 사다리 gate

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3926](https://github.com/edwardkim/rhwp/pull/3926) / @planet6897 |
| 원 head | `ae00c04d3436640fee02756f50744f77a6cf28c3` |
| 누적 적용 commit | `bb4ad741d` (`cherry-pick -x`) |
| 기준 / 누적 branch | `upstream/devel` `21affdafc` / `review/planet6897-20260804` |

## 검토와 보정

PR의 목표는 raw HWPX anchor를 무조건 믿지 않고, 다음 저장 `vpos`가 table 높이와 outer margin을
수용할 여유를 보일 때만 적용하는 것이다. 이는 #3738 문서의 222쪽 유지와 #3925의 과도한 쪽 이동을
동시에 지키려는 좁은 계약이다.

원 PR 설명은 host 줄 높이 gate가 표적을 223쪽으로 악화시켜 배제됐고 저장 사다리 여유만 채택됐다고
기록한다. 그러나 원 구현은 둘을 OR로 결합해 host 줄 높이만으로도 gate를 통과시켰다. 설명과 실제
수용 조건의 불일치이며, 배제했던 223쪽 회귀를 되살릴 수 있다. 메인터너 보정 `b2905829a`에서 host
조건을 제거하고 **다음 저장 사다리 여유만** 판정하도록 맞췄다. 새 단위 회귀는 (1) 짧은 사다리 거절,
(2) 충분한 사다리 수용, (3) 큰 host 줄만으로는 거절을 모두 고정한다.

## 시각 증적

`정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwpx`의 한컴 2020
기준 PDF와 rhwp 222쪽 산출을 선택 범위 23–24쪽에서 대조했다.

- review PNG: [쪽 23](../assets/pr_3926_issue3738_hwpx_p023_review.png)
- 생성물: `output/review-planet6897-20260804/visual-issue3738/pr3926-issue3738-hwpx/`
- sweep 결과: 전체 SVG/render tree 222쪽, 선택 PDF/raster 23–24쪽, flag 0건.

사람 검토에서 쪽 23의 두 그림·caption·본문 흐름과 PDF의 geometry가 일치했다. `pixel_match`
91.013%, ink proxy 8.6258%는 glyph/font metric 차이의 보조 수치로만 기록하며 결함 판정에는 사용하지
않았다.

## 판정

원 기여의 방향은 타당하나, 수용은 위 메인터너 보정이 포함된 경우에 한정한다. focused predicate
회귀, #3738 선택 visual 대조, 전체 release-test·native-Skia·WASM 검증이 성공했다. **보정 포함 통합
수용 권고.**
