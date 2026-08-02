---
kind: pr-review-implementation-plan
status: code-ci-success-docs-tail-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# Kevin #3747-#3779 통합 검토 계획

## 라우팅과 기준선

```text
base route: collaborator 매개 외부 PR (Route B: 통합 PR)
modifiers: 접수·리뷰 기록, 로컬 검증, 다수 PR·update branch, 재작업·예외
loaded documents: pr_review_workflow.md, pr_review/README.md,
  collaborator_external_pr.md, intake_and_review.md, local_validation.md,
  multi_pr_update_branch.md, rework_and_exceptions.md
integration branch: review/kevin9327-20260802
base: upstream/devel@a8d7bdfbf54dfa31f2cfe3b05d5f1e0c0cb18c99
```

@kevin9327의 열린 PR 18건은 원 source branch를 rewrite, rebase, amend, force-push하지 않는다.
최신 `devel` 위 통합 branch에서 contributor 기능 commit만 `git cherry-pick -x`로 적층하고,
최종 통합 PR로 `devel`에 반영한다. 각 원 PR의 review 문서는 별도로 남긴다.

## 적용 경계와 순서

| 순서 | 원 PR | 적용 범위 | 비고 |
| ---: | --- | --- | --- |
| 1 | #3747 | MCP 인자 타입 계약 회귀 | 구현은 #3742에 반영됨 |
| 2 | #3748, #3750, #3752 | CLI/MCP 오류·저장 계약 | 독립 소형 수정 |
| 3 | #3753, #3754 | HWPX/HML 회귀 계약 | 구현 변경 없음 |
| 4 | #3756, #3757, #3758, #3760, #3764, #3768, #3769 | HWP3/HWP5/HWPX 정수 경계 | parser/serializer 집중 검증 |
| 5 | #3761 | `rhwp run --dry-run` | plan 실행의 무변경 계약 |
| 6 | #3767 | HWP5 BMP 밖 문자 보존 | serializer 집중 검증 |
| 7 | #3775 | M18 Python 바인딩·IR schema | 대형 PR 별도 package 검증 |
| 8 | #3779 | M19 Node 바인딩·capabilities schema | #3775 이후, 대형 PR 별도 package 검증 |

#3766의 BrokenPipe 테스트 커밋과 #3758/#3767/#3769에 중복으로 포함된 동일 커밋은
이미 #3778로 `devel`에 들어갔으므로 적용하지 않는다. #3779의 `Merge branch 'devel'` commit도
기능 commit이 아니므로 제외한다.

## 단계와 종료 조건

## 실제 적층과 충돌 해소

18개 원 PR에는 REST reviewer assignment를 완료했다. `gh pr edit`은 Projects Classic GraphQL field 오류로
실패했으나, reviewer 지정은 `POST /repos/edwardkim/rhwp/pulls/<N>/requested_reviewers`로 정상 반영됐다.

| 원 PR | 누적 commit | 처리 |
| --- | --- | --- |
| #3747 | `62a9e5d85` | MCP 인자 타입 회귀 |
| #3748 | `659e2a13e` | dash 검색어 CLI 계약 |
| #3750 | `d11831798` | MCP 저장 확장자 |
| #3752 | `f756f6b9e` | HWP5 CLI exit code |
| #3753 | `ccc478560` | HWPX attribute 회귀 |
| #3754 | `6618f7012` | HML rectangle 회귀 |
| #3756 | `cdd55c838`, `8aa27b19b` | HWP3 padding/HWPX OLE signed 값 |
| #3757 | `e288b0a7f` | HWP3 margin overflow |
| #3758 | `51adfaa69`, `cb0461308` | HWPX 음수 offset |
| #3760 | `6bcbadcd1` | HWP3 drawing overflow |
| #3761 | `c40a2e5d0` | run dry-run |
| #3764 | `77627e953` | HWP3 padding overflow regression |
| #3766 | 제외 | #3778에 이미 merge된 BrokenPipe duplicate |
| #3767 | `0b72d3bf2`, `d6e2c7f03` | HWP5 비-BMP 문자 |
| #3768 | `80b572c19` | HWPX OLE BinData ID |
| #3769 | `ed339e78f` | HWP5 table span zero |
| #3775 | `6eac4249a`..`404288c6f` | M18 Python binding |
| #3779 | `9ecca3bae`..`106ff3780` | M19 Node binding; devel merge commit 제외 |

충돌은 다음 원칙으로 해소했다.

1. #3756/#3757/#3764는 공통 HWP3 padding helper 하나를 유지하고 각각의 음수·overflow regression을 보존했다.
2. #3758은 signed parser와 wrapped unsigned compatibility path를 모두 남겼다.
3. `src/main.rs`는 M18 `export-ir-schema`와 M19 `export-capabilities-schema` dispatch를 모두 보존했다.
4. #3775 cleanup이 #3761의 `run_plan_dry_run_contract`를 누적 tree에서 제거한 것을 발견해, reviewer 보정으로
   테스트 파일을 복원한다. 기능을 되돌린 것이 아니라 독립 PR 계약을 다시 보존한 것이다.

## 누적 검토 보정

원 PR을 개별 상태로만 보면 드러나지 않는 public surface와 재현성 결함을 다음처럼 보정한다.

- Node generated envelope가 M18/M19의 새 schema command 둘을 빠뜨렸다. generator를 실행해
  `ExportIrSchemaEnvelope`, `RenderDiffEnvelope`와 command map을 동기화했다.
- Node package에 lockfile이 없고 CI 네 job이 모두 unfrozen `npm install`을 사용했다. lockfile을 추적하고
  `npm ci` 및 setup-node npm cache를 적용했다.
- Python M18 binding에는 M19 capabilities schema command의 public API가 없었다. Python wrapper, model,
  schema helper, API 문서, unit·integration contract를 함께 추가했다.
- Python dev tool 최신판은 project가 보장하는 Python 3.8 분석을 중단했다. mypy와 ruff 범위를 호환되는
  검토 버전으로 제한하고, 실제 type/lint 잔여와 generator template을 보정했다.
- `bindings/node/tools/gen-types.ts`는 1,002줄이었다. 기능 변경 없이 빈 줄 둘을 제거해 1,000줄 이하 정책을
  지켰다. `src/parser/hwp3/mod.rs`는 누적 test formatting만 `cargo fmt` 결과로 정리한다.

위 보정은 contributor 원 commit과 분리해 다음 일반 commit으로 고정했다.

| commit | 보정 |
| --- | --- |
| `a6f453fb3` | #3775 cleanup으로 사라진 #3761 dry-run 계약 test 복원 |
| `90a208916` | Node generated envelope·lock/CI와 Python capabilities schema public API 보정 |
| `3bba6f628` | 누적 HWP3 regression test의 `cargo fmt` 정리 |

## 현재 로컬 검증

공유 target을 피하기 위해 모든 Cargo 검증에 `CARGO_INCREMENTAL=0`과
`CARGO_TARGET_DIR=target/review-kevin9327-20260802`를 사용했다. renderer/layout 또는 sample/golden을
변경하는 PR이 없으므로 기준 PDF·pixel 검증은 적용하지 않는다. Native Skia와 WASM은 CI 게이트 동등성
확인을 위해 실행한다.

| 영역 | 명령 또는 범위 | 결과 |
| --- | --- | --- |
| Rust focused | MCP/CLI/HML/dry-run/IR/capabilities/render-diff 9 target | 108 passed |
| Rust 전체 | `cargo test --profile release-test --tests` | exit 0; `overflow_cell_baseline` 포함 |
| Rust 정적 | `cargo fmt --check`, `git diff --check`, clippy `-D warnings` | 모두 통과 |
| Node 설치·형식 | `npm ci`, typecheck, `npm run build` | 통과 |
| Node test | unit 389, native integration 425, generator drift check | 모두 통과 |
| Node package | `npm pack --json` | 37 files; dist·README·CHANGELOG·package metadata만 포함 |
| Python 정적 | ruff, mypy 1.10.1 | 모두 통과 |
| Python test | `RHWP_BIN=... pytest tests -q` | 254 passed |
| Python package | example 2건, wheel/sdist build | 통과 |
| Native Skia | library 58건, missing-picture 2건, direct-PDF 4건 | 모두 통과 |
| WASM | `wasm-pack build --target web` | 통과; `rhwp@0.8.2`, wasm 7,438,103 bytes |

## 다음 순서

1. 원 PR별 review 18개, 이 기록, `mydocs/orders/20260802.md`를 docs tail commit으로 고정한다.
2. LFS 대상 판독 뒤 `upstream` temporary head를 push하고 `devel` 대상 통합 PR을 만든다.
3. 최신 integration head의 CI·mergeability를 재확인한 뒤, 작업지시자 승인 범위에서 merge 및 원 PR
   supersede close·devel sync·worktree/target 정리를 수행한다.

최종 merge 조건은 최신 integration head의 GitHub Actions 성공, `MERGEABLE` 상태, 작업지시자의 승인이다.
