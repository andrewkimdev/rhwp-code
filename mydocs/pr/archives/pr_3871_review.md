---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3871 검토 — 다문서 `batch extract-data`

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3871](https://github.com/edwardkim/rhwp/pull/3871) / @kevin9327 |
| 원 head · 적용 source | `a1c5b82c7` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |

## 검토

폴더의 HWP/HWPX 문서에서 날짜·금액·수량을 정해진 schema로 모으고, profile에도 batch 작업을
명시한다. 문서별 실패를 전체 batch의 무언 손실로 바꾸지 않도록 결과 envelope에 보존하는 경계와
입력 순서의 결정성이 계약으로 고정돼 있다.

## 누적 검증과 판정

`batch_extract_data_contract` 8/8, profile router 8/8, 전체 release-test `exit-0` 통과.
**통합 수용 권고.**
