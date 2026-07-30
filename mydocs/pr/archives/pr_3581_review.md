---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3581 리뷰 — 한컴 개방 실패 추적 도구 2종 (extract-pages CLI + 레코드 이식 하니스)

- PR: [#3581](https://github.com/edwardkim/rhwp/pull/3581) / Refs [#3565](https://github.com/edwardkim/rhwp/issues/3565)
- 작성자: `planet6897` — [#3566](https://github.com/edwardkim/rhwp/pull/3566)(원인 수정)과 상호보완 쌍
- 역할: maintainer 일반 경로 + local_validation

## 라우팅과 작성 시점

```text
base route: maintainer_general.md / modifiers: intake_and_review.md, local_validation.md
current head: 7c112eb6d / MERGEABLE / behind (참고값)
규모: 9 files, +858/−0 — document_core 명령 + CLI + 테스트 2본 + Python 하니스 + 문서
```

## 변경 범위와 수용 판단

`convert --verify` 가 잡지 못하는 계열(자기 파서가 자기 산출물을 되읽음 — 판정자는
한컴뿐)의 원인 추적 수단 2종.

1. **`extract-pages` CLI** (`page_extract.rs` +118, main.rs +129): 쪽 범위만 남겨 저장 —
   재현 최소화 도구. 쪽 단위로 자르되 문단 단위로 삭제, 한계(구역·DocInfo·BinData 잔존)를
   문서에 명시. 삭제는 `delete_paragraph_native` 위임이라 raw_stream 무효화 계약 준수 —
   #2724 가드 원장에 정당한 면제 등록(+8)이며 위임 주장 자체를 가드가 기계 검증한다.
2. **`tools/hwp_open_bisect/`** (Python 3본, +438): 한컴 저장본을 정답지로 두고 rhwp
   산출물 레코드를 이식하며 개방 여부로 이분 — #3566 근인 특정에 실제 사용된 하니스.
   Windows COM 전용이라 CI 비대상, 코드 리뷰로만 확인(빌드 산출물에 미포함, 위험 없음).
3. `cli_commands.md` 갱신(+10) 포함 — 명령 문서 정합.

**수용 판단: merge 권고.** 비기능 DX 투자 지침("반복 마찰은 구조화로")에 정확히
부합 — 한컴-전용 결함 추적이라는 반복 마찰을 재사용 가능한 도구로 구조화했다.

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| 충돌 simulation (devel merge) | clean | — |
| focused (extract 5 + #2724 가드 5) | 10 passed | 면제 등록의 위임 무효화까지 기계 검증 |
| CLI 실동작 smoke | 35쪽 → 5쪽(3~5쪽 요청), 문단 61 보존/569 제거, 산출물 정상 로드 | end-to-end 동작 확인 |
| `cargo test --profile release-test --tests` | 371 바이너리 전부 ok (exit 0) | 전체 회귀 없음 |
| fmt / clippy `-D warnings` | 둘 다 통과 | — |
| PR head CI | 전 check green | — |

시각 검증 비적용 — 도구·CLI 축, 렌더 비접촉.

## 최종 권고

**merge 권고.** #3566 과 같은 배치로 처리하되 merge 판단은 PR 별 분리. merge 후
contributor comment 는 #3566 과 연계해 작성.
