---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3897 검토 — diagnostics CLI의 미지 flag·stdout 계약

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3897](https://github.com/edwardkim/rhwp/pull/3897) / @kevin9327 |
| 원 head | `0c057605f1ed14e0c68fb10c09ebb22566c6c4fa` |
| 누적 적용 commit | `919bec6f0` (`cherry-pick -x`) |
| 기준 / 누적 branch | `upstream/devel` `ad67e5a63` / `review/kevin9327-20260804` |

## 검토

`bench`, dump 계열과 diagnostics 명령이 미지 flag를 성공으로 삼키지 않게 하고, failure diagnostics가
stdout 계약을 오염시키지 않도록 한다. 자동화가 실패를 성공으로 오인하지 않게 하는 중요한 경계다.

누적 검토에서는 flag가 option value 자리에 들어가거나 내부 도구의 positional처럼 해석되는 인접
경계도 발견했다. 별도 메인터너 보정 `fb33dfd72`, `45e53d42e`에서 exact numeric parsing, option-in-value
거절, `--` 뒤 dash-leading text의 명시적 허용을 추가했다. 기존 정상 입력과 CLI exit-code 계약은
회귀로 보존했다.

## 판정

diagnostics focused 28건, preflight, 전체 release-test가 종료 코드 0으로 성공했다. **통합 수용 권고.**
