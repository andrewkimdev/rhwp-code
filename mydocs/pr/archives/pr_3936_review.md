---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3936 검토 — HWP3 body 구역 cold의 독립 보장

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3936](https://github.com/edwardkim/rhwp/pull/3936) / @planet6897 |
| 원 head | `d82114c7d8ee25f8df01fcfdc6a016d577581ae4` |
| 누적 적용 commit | `c81575ba4` (`cherry-pick -x`) |
| 기준 / 누적 branch | `upstream/devel` `21affdafc` / `review/planet6897-20260804` |

## 검토

`fixup_hwp3_notes`는 원래 미주가 있을 때만 body 첫 문단의 `ColumnDef`를 보장했다. 하지만 cold는
미주 존재와 별개인 body section의 직렬화 계약이다. PR은 `ensure_hwp3_initial_body_column_def`를
미주 조건 밖으로 옮겨, 미주 없는 HWP3도 첫 `ColumnDef`를 갖게 한다. 미주가 있는 기존 경로는 그대로
유지된다.

원 PR에는 이 독립 경계를 직접 고정한 회귀가 없었다. 별도 메인터너 test commit `c6f2ddcb3`에서
미주 없는 단일 section을 만들고, 첫 control이 `ColumnDef`인지와 두 번 실행해도 하나만 남는
idempotence를 검증했다. 기여자가 대조에 사용한 원본 HWP3 파일은 저장소에 포함되지 않아 여기서
재현하지 않았으며, 그 한계는 synthetic 경계 회귀와 전체 HWP3 fixture suite로 보완했다.

## 판정

새 focused HWP3 회귀 1건, 기존 `issue_3676_hwp3_convert_hancom_openable` 5건, 전체
release-test·native-Skia·WASM 검증이 성공했다. **메인터너 회귀 추가를 포함한 통합 수용 권고.**
