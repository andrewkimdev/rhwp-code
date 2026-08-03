---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3875 검토 — WMF 음수 point count 패닉 차단

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3875](https://github.com/edwardkim/rhwp/pull/3875) / @kevin9327 |
| 원 head · 적용 source | `6173318d2` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |

## 검토

WMF `POLYLINE`/`POLYGON`의 signed `NumberOfPoints`가 음수일 때 `usize` capacity로 변환되며
발생하던 overflow panic을 allocation 전에 오류로 반환하게 한다. 두 record parser에 같은 방어를
적용하고 red→green fixture로 panic 부재를 고정해 신뢰하지 않는 그림 입력 경계를 보강한다.

## 누적 검증과 판정

`wmf_poly_negative_point_count_no_panic` 3/3, 전체 release-test `exit-0`, clippy `exit-0`.
**통합 수용 권고.**
