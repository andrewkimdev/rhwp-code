---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3923 검토 — HWP3 문서 파생 slice 시작 OOB 차단

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3923](https://github.com/edwardkim/rhwp/pull/3923) / @kevin9327 |
| 원 head | `7fdfc2a768bb5ef95dd42dd7b491a0895c024529` |
| 누적 적용 commit | `19aaf9398` (`cherry-pick -x`) |
| 현재 PR 기준 / 누적 branch | `upstream/devel` `874dae394` / `review/kevin9327-20260804` |

## 검토

HWP3 `info_block_length`가 파일 끝 이후의 body start를 가리킬 때와 24~31 byte image block에서
`data[32..]`를 자를 때 발생하던 slice OOB panic을 각각 checked slice와 빈 image fallback으로 바꾼다.
신뢰 경계 밖 HWP가 라이브러리·MCP·WASM 소비자를 process panic으로 멈추지 않고 parse error로 끝나게
하는 안전성 경계다.

메인터너 보정 `38d3b3b75`은 malformed `info_block_length=0xffff` 회귀가 단순 no-panic이 아니라
반드시 `Err`를 반환함을 확인하도록 oracle을 강화했다.

## 판정

유효 HWP3 양성 대조와 malformed 입력 오류 계약을 모두 focused 회귀 2건으로 고정했다. 전체 누적 회귀
종료 코드 0을 확인했다. **통합 수용 권고.**
