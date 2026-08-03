---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3872 검토 — preflight 선언 flag 오탐 제거

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3872](https://github.com/edwardkim/rhwp/pull/3872) / @kevin9327 |
| 원 head · 적용 source | `73d71c313` |
| 기준 / 누적 branch | `upstream/devel` `ac085e1d` / `review/kevin9327-20260803` |

## 검토

schema 출력 명령의 정상 stdout에 들어 있는 오류 사전 문구를 미구현 flag로 오인하던 검사다. 정상
exit code는 즉시 수용하고, 실패한 실행에서는 stderr만 unknown-option 신호로 보게 해 검사 대상의
정상 출력과 진단을 분리했다.

## 누적 검증과 판정

release-test binary로 `python3 tools/agent_preflight.py --bin …/rhwp`를 실행해 111 flag·63
command 상호 커버를 포함한 모든 검사 통과를 확인했다. **통합 수용 권고.**
