---
kind: review
status: active
pr: 3816
---

# PR #3816 검토: Kevin 기능 PR 13개 누적 통합

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           multi_pr_update_branch.md, rework_and_exceptions.md(대형 PR)
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, multi_pr_update_branch.md,
                  rework_and_exceptions.md
current head: dce827e96e5d613bbef3d2a65ce750ac33ec2d21 (문서 작성 시점 참고값)
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#3816](https://github.com/edwardkim/rhwp/pull/3816) |
| 작성자 | `jangster77` |
| 대상 / head | `devel` / `review/kevin9327-20260803-integration` |
| 작성 시점 상태 | draft, `MERGEABLE`, `BLOCKED` (CI 진행 중) |
| 변경 규모 | 68 files, +38,013 / -7,103 lines, 20 commits |
| 기준 devel | `3b0349a5670bf22e9043fced2064bf78935b0161` |
| 원 기여자 | `kevin9327` (13개 원 PR) |

위 상태값은 문서 작성 시점 참고값이다. merge 판단에는 최신 PR head의 GitHub Actions와
작업지시자 승인이 필요하다.

## 변경 범위

이 PR은 Kevin의 독립 기능 PR을 최신 `devel` 위에 누적하고, 통합 시 드러난 CLI/MCP/
출처 표지 계약 누락을 별도 collaborator commit으로 보정한다. 원 contributor branch는
rewrite하거나 push하지 않았다.

| 원 PR | 고정 source head | 핵심 범위 |
| --- | --- | --- |
| #3788 | `0c738cc4464ca89b008f669234082a27cd3eefdd` | 세션 편집 `changedPages` |
| #3791 | `d4ea13209e70f259afe8ea5056918ecc4d9aab37` | 표 CSV 왕복 |
| #3795 | `c0afa9d89d75d8f8a017f4fa6d07351ee21a0f94` | agent preflight |
| #3797 | `09a8337e1e6bf53a31f457e0b55fc6f995ee9788` | 이미지 삽입 |
| #3799 | `4d6d3567d0e0b2ead28f725f71bf87b3197fd688` | batch fill |
| #3800 | `6f33ad95ca0a27d7c97128bb7d24d20098a010f4` | 에이전트 보안 문서 |
| #3802 | `c81984d1718dbc19919d1bd535c23ab806faf974` | 에이전트 경계 계약 |
| #3804 | `ae41ab04ede64ed4d93aec3b12205cc3a9c3799e` | JSON 출처 표지 |
| #3805 | `ee55903c4101588943cc40ea4609b875acc3d168` | redact/sanitize |
| #3806 | `2a2c9b7710fd6ec3c28bc60967c4cb9376f005fe` | 날짜·금액·수량 추출 |
| #3807 | `6e6e012a6a675eea97f098dcb111a2dc5c7e5c76` | 주입 신호 검사 |
| #3809 | `2e7720807bd95fcbad3fbd37d53fe6c355641415` | 은닉 텍스트 검사 |
| #3811 | `d1cb0be6b9ca02bc8b3d88cc07a29fae65ad4e7e` | 유니코드 기만 검사 |

통합 보정 commit은 다음과 같이 contributor 원 기능과 분리했다.

- `6fcee0b26`: JSON 출처 표지 누락 CLI 보완
- `84db8b4d0`: `inspect` 봉투 출처 표지 보완
- `2972ee1c3`: `extract-data` 출처 봉투 직렬화 보완
- `c652fefbd`: `inspect` CLI/MCP/capability 계약 정렬
- `40871f3a1`, `5a852062a`, `dce827e96`: capability·출처 지도·보안 계약 테스트 보정

## 검증

### 로컬

- 누적 검토 R2에서 release-test 전체와 Native Skia 3종을 완료했다.
- 최신 `devel` rebase 대상 `3b0349a56`은 문서 변경만 포함하므로, 코드 검증 결과를
  무효화하지 않는다.
- 이번 통합 보정 후 다음 영향 범위 검증을 통과했다.
  - `unicode_deception_contract`: 14 passed
  - `mcp_password_contract`: 4 passed
  - `mcp_server_contract`: 22 passed
  - `provenance_contract`: 7 passed
  - `injection_scan_contract::capabilities_and_mcp_declare_the_command_consistently`: 1 passed
  - `capabilities_schema_contract`: 17 passed
  - `extract_data_contract`: 18 passed
  - `hidden_text_contract`: 24 passed
- `cargo fmt --check`, `git diff --check`를 통과했다.

### Node binding CI 실패 보정 (2026-08-03)

GitHub Actions의 `생성 타입 최신 검사`와 `통합 (rhwp 빌드)` 실패를 로컬에서 재현해
수정했다.

- 원인: capabilities가 26개에서 31개 봉투로 늘었지만 `bindings/node/src/envelopes.ts`가
  생성되지 않았고, `export-provenance-map`, `table-to-csv`, `csv-to-table`,
  `extract-data`, `inspect`의 1층 Node 래퍼가 빠져 parity 계약이 깨졌다.
- 보정: 다섯 래퍼와 하위 `inspect` 명령별 타입/argv 단위 테스트를 추가하고,
  `gen:types`로 31개 봉투를 재생성했다. 필수 `--table`은 선택 플래그 공통 검사에서
  제외해 기존 `setCell`과 CSV 편집의 올바른 호출을 보존했다.
- 로컬 결과: `gen:check`, `typecheck`, `vitest run`(17 files, 439 tests), `npm run build`,
  `npm pack --dry-run --json`, `git diff --check`를 통과했다.
- 별도 원인: `Cancel stale PR runs`가 종료 직전 run을 force-cancel해 GitHub API 409으로
  실패했다. 409이면 대상 run을 재조회하고 `completed`일 때만 정상 경과로 처리하도록
  고쳤으며, workflow YAML Prettier 검사와 GitHub Script JavaScript 구문 검사를 통과했다.

### 시각·fixture 판정

시각 증적 경로는 적용하지 않았다. 이 통합에는 `src/renderer`, typeset, pagination,
golden, HWP/HWPX sample, 기준 PDF 변경이 없다. 이미지 삽입의 직렬화 정합 보정은
기존 비율 계산을 바꾸지 않으며, 관련 계약 테스트로 저장·재파싱 정합을 확인한다.

`pdf-large/hwpx/2026_oss_rst.pdf`의 기존 LFS pointer 경고는 이 PR의 변경 대상이 아니며,
LFS 사전 판독에서도 새 LFS object 또는 변경 경로가 없었다.

## 위험과 판단

- 이 PR은 1,000줄을 크게 넘는 누적 통합이다. 기능별 원 PR은 분리된 source head로
  보존했지만, 최종 판단은 통합 head 전체에 대한 CI여야 한다.
- `inspect`는 세 하위 명령의 capability flag 합집합을 광고한다. 통합 보정은 각
  flag를 실제 하위 명령으로 라우팅해 허위 사용법 오류가 나지 않도록 했다.
- MCP password는 의도적으로 argv에 넣지 않는다. `passwordStdin` 계약을 검증해
  프로세스 목록 노출을 막는다.
- stale run 취소의 409은 대상이 이미 완료됐음을 재조회로 확인한 경우에만 무시한다.
  active run을 성공으로 기록하지 않으므로 최신 head 보호 경계는 유지된다.

**현재 권고: merge 보류.** 최신 PR head의 CI가 모두 통과하고, merge 직전 mergeability 및
source PR head 변동을 다시 확인한 뒤 작업지시자 승인 범위에서 merge를 판단한다.
