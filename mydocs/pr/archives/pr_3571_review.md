---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3571 리뷰 — mcp-serve: rhwp 를 실제 MCP 서버로 (#3140)

- PR: [#3571](https://github.com/edwardkim/rhwp/pull/3571) / Related [#3140](https://github.com/edwardkim/rhwp/issues/3140)
- 작성자: `kevin9327` — [#3577](https://github.com/edwardkim/rhwp/pull/3577)(MCP 통합 가이드)이 이 브랜치 위에 적층
- 역할: maintainer 일반 경로 + local_validation (4.3 Rust/CLI 행)

## 라우팅과 작성 시점

```text
base route: maintainer_general.md / modifiers: intake_and_review.md, local_validation.md
current head: fef859a7b / behind (참고값)
규모: 5 files, +788/−20 — mcp_serve.rs 신규(392줄) + main.rs 배선 + 계약 테스트 6종
  + cli_commands.md + task report
```

## 변경 범위와 수용 판단

`rhwp mcp-serve` — MCP 표준 stdio 전송(줄 단위 JSON-RPC 2.0)으로 initialize →
tools/list → tools/call 을 직접 처리. #3140 의 마지막 층(실행)을 채운다.

1. **단일 출처** — `mcp_tool_definitions()` 를 선언(`capabilities --mcp`)과 서버가 공유,
   드리프트는 계약 테스트가 고정.
2. **무상태 13종은 자기 자신을 서브프로세스로 실행** — #2707 종료 코드·stdout 순수성
   계약 재사용. 인자 치환은 shell 경유 없이 argv 원소로 개별 전달되어 메타문자 주입
   불가(스키마 밖 옵션 주입은 로컬 stdio 신뢰 모델상 수용).
3. **세션 3종**(hwp_open/doc_text/close) — CLI 로 원리적으로 불가능한 재파싱 없는 반복
   조회 공백을 docId 핸들로 채움. 닫힌 핸들 재사용은 isError.
4. **의존성 무추가** — serde_json 만 사용, WASM 비포함.

**수용 판단: merge 권고.**

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| 충돌 simulation (devel merge) | clean | — |
| focused 계약 6종 (release-test) | 6 passed | 핸드셰이크·선언 정합·무상태·세션·오류 2종 |
| **실 프로토콜 스모크** (빌드 바이너리, JSON-RPC 대화) | 도구 16종 노출, KTX.hwp open(docId·27쪽)→doc_text(p1 텍스트)→close→**닫힌 핸들 isError:true**, 무상태 hwp_info structuredContent 확인 | end-to-end 실동작 |
| stdout 순수성 | 진단(LAYOUT_OVERFLOW)이 stderr 로 분리, stdout 은 JSON-RPC 만 | MCP 전송 요건 충족 |
| `cargo test --profile release-test --tests` | 373 바이너리 전부 ok (exit 0) | 전체 회귀 없음 |
| fmt / clippy `-D warnings` | 둘 다 통과 | — |
| PR head CI | 전 check green | — |

## 최종 권고

**merge 권고.** merge 후 #3577 은 docs-only 잔여 diff 로 축소되므로 이어서 처리.
#3140 close 여부는 이 PR 이 Closes 를 쓰지 않았으므로 별도 판단(0단계 #3263 + 본 실행
층으로 이슈 범위 충족 가능성 — 작업지시자 결정 대상).
