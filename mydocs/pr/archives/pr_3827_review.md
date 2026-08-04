---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3827 검토 — CLI 실물 명령 8종 레퍼런스 보강

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3827](https://github.com/edwardkim/rhwp/pull/3827) / @kevin9327 |
| 원 head · 적용 source | `786584516` · `440b6ba5e`, `786584516` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |
| 충돌 | #3841의 `edit redact` 설명을 함께 유지하도록 해소 |

## 검토

`table-to-csv`·`csv-to-table`·batch fill·`edit insert-image`와 보안 inspect/provenance 축을
`cli_commands.md`에 실제 옵션과 JSON 사용 예로 연결한다. 새 CLI를 주장하는 문서가 아니라 이미
배선된 명령의 발견성을 높이는 변경이며, 명령 이름과 출력 계약은 `cli_json_contract`으로 함께 확인했다.

## 누적 검증과 판정

focused `cli_json_contract` 31/31, 전체 release-test `exit-0`, 상대 링크 검사 480개 통과.
**통합 수용 권고.**
