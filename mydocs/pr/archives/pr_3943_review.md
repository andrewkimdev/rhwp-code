---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3943 검토 — Stage 3 frontend unit/package/render 활성화

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, multi_pr_update_branch.md
current remote head: 1990916ab85c33b6aaa7237892d2293a4ae52ac1
                     (보정 push 전 작성 시점 참고값)
local correction commit: fc1c10450
local devel merge: d4ec7e93d
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#3943](https://github.com/edwardkim/rhwp/pull/3943) |
| 작성자 | `postmelee` (collaborator self-merge) |
| 대상 / head | `devel` / `codex/issue-3790-stage3-frontend` |
| 작성 시점 원격 상태 | draft, `MERGEABLE`, `BEHIND`; 보정 push 뒤 재확인 필요 |
| 보정 전 원격 head | `1990916ab85c33b6aaa7237892d2293a4ae52ac1` |
| 최신 동기화 기준 | `upstream/devel` `f864e851a98f` |
| 로컬 devel merge | `d4ec7e93d2565b001787e9e3609c990c9c6022ec` |
| review correction | `fc1c10450` |
| 작성 전 PR 고유 규모 | 14 files, +612 / -325 |
| 관련 issue | [#3790](https://github.com/edwardkim/rhwp/issues/3790) (open) |
| metadata | label·assignee·review request 없음 |

draft, mergeability, head SHA와 CI는 변할 수 있는 작성 시점 참고값이다. 최종 merge 조건은 최신 PR head의
GitHub Actions 통과와 작업지시자 승인이다. 사용자 지시에 따라 reviewer request를 보내지 않았고,
ready 전환과 merge도 이 review commit에서는 수행하지 않는다.

## 변경 범위와 목적

Stage 2.5까지 관찰만 하던 trusted-base classifier의 `frontend_mode`와 `render_required`를 실제 CI 조건에
연결한다. frontend-only PR에서 Rust lint·세 builder·네 worker·Native Skia·Rust/Python CodeQL은 아직
그대로 실행하며, 이번 Stage 3은 다음 두 비용만 먼저 줄인다.

- `unit`: Studio 전체 TypeScript와 759개 unit test를 실행하되 fresh WASM·Vite·extension build는 생략한다.
- `package`: fresh WASM 뒤 Studio test/build와 shared·extension package gate를 유지한다.
- `render_required=false`: Render Diff preflight 기록은 유지하고 Canvas job만 생략한다.

workflow·classifier·Cargo·WASM 경계, rename, 파일 수 경계, 미분류와 API 오류는 full로 닫힌다. Rust,
Native Skia와 CodeQL 축의 실제 조건화는 각각 Stage 4·5의 별도 PR 범위다.

## 리뷰 지적 대응

[리뷰 코멘트](https://github.com/edwardkim/rhwp/pull/3943#issuecomment-5177251126)의 7개 항목을 다음처럼
반영했다.

### 1–2. label 이벤트와 존재하지 않는 `ci:full`

`labeled|unlabeled`는 `ci:full`이 아닌 일반 label 변경에도 workflow를 다시 실행한다. 기존 concurrency는
진행 중인 같은 PR run을 취소하므로 불필요한 중복 실행뿐 아니라 정상 검증을 끊을 수 있다. 단순히
`cancel-in-progress=false`로 바꾸면 새 중복 run은 남고, label event를 성공 fast-pass로 처리하면 같은
required check 이름을 잘못 충족할 위험이 있다.

보정 commit `fc1c10450`에서 CI와 Render Diff의 label activity 및 label 검사 코드를 제거했다. 저장소에
없는 `ci:full` label은 만들지 않는다. 강제 전체 검증은 기존 `workflow_dispatch`가 fail-closed full로
수행한다. label 기반 강제 실행은 post-main trusted controller가 required policy로 채택될 때 별도
설계한다.

### 3. 넓은 Studio runtime의 Vite build 생략

초안에서는 `rhwp-studio/src` 218개 중 explicit package 경로 38개를 제외한 약 180개가 unit으로 들어갈
수 있었다. TypeScript가 잡지 못하는 Vite plugin·asset resolution·production bundle 오류를 놓칠 범위가
너무 넓었다.

최초 unit 소스 범위를 historical fixture로 검증한 `src/command/**`, `src/engine/command.ts`로 제한했다.
일반 Studio test는 unit/no-render를 유지하지만 `src/view/**`, `src/ui/**`, 그 밖의 runtime은
package+render로 시작한다. 따라서 #3785·#3656의 단축키·command 사례는 절감하고, #3670 hwpctl과 #3672
page renderer는 실제 WASM/Vite package gate를 유지한다.

### 4. unit/none 실제 실행 근거

Stage 3 workflow가 아직 devel base에 없으므로 별도 devel 대상 임시 PR은 새 classifier를 사용할 수 없다.
workflow 변경을 함께 넣으면 fail-closed full이 되어 unit/none canary가 되지 않는다. 검증용 override를
workflow에 추가하는 대신 unit 범위를 위의 두 소스 경계로 좁히고 historical fixture·로컬 unit lane을
고정했다.

첫 실제 canary는 Stage 3 merge 직후, Stage 4보다 먼저 수행한다. frontend-only PR의 selective run과 같은
commit의 수동 `workflow_dispatch` full run을 대조해 unit/none의 `success|skipped`, wall time과
runner-minute를 기록한다. false negative가 나오면 Stage 4로 진행하지 않고 unit 경계를 package로
되돌린다.

### 5. WASM unit stub 안전성

`@wasm/rhwp.js`를 직접 import하는 Studio 파일은 `src/core/wasm-bridge.ts`와 `src/hwpctl/index.ts`이며 둘 다
package lane으로 분류된다. 이 근거를 stub 주석에 추가했다. CI 전용 tsconfig·stub 자체 변경은 계속
`fail-closed:frontend-unit-contract`로 full 처리한다.

### 6. test-only render 축

피시험 코드와 테스트 파일명을 다르게 취급하던 heuristic 제거는 유지했다. Studio test 변경은 전체 759개
test를 unit gate에서 실행하며, test 파일명만으로 Canvas를 강제하지 않는다.

### 7. 절차와 기록

- 최신 `upstream/devel` `f864e851a`를 conflict 없이 merge했다.
- Stage 3 작업 기록과 #3790 계획·오늘할일을 현재 PR 상태와 canary 우선순서로 갱신했다.
- 이 review 문서를 `mydocs/pr/archives/pr_3943_review.md`에 추가했다.
- 보정은 단일 정책 commit과 devel merge로 추적 가능해 별도 `pr_3943_review_impl.md`는 만들지 않았다.

## 검증

최신 devel merge head `d4ec7e93d`에서 다음을 순차 실행해 통과했다.

| 검증 | 결과 |
| --- | --- |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 24 passed / 0 failed |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py scripts/tests/test_render_diff_workflow.py` | 14 passed |
| `actionlint .github/workflows/ci.yml .github/workflows/render-diff.yml` | 통과, 진단 없음 |
| `npx --prefix rhwp-studio tsc --project rhwp-studio/tsconfig.ci-unit.json --noEmit` | 통과 |
| `npm --prefix rhwp-studio run test` | 759 passed / 0 failed |
| `npm --prefix rhwp-studio run e2e:renderer-contract` | 통과 |
| `wasm-pack build --target web --dev` | fresh WASM package 생성 성공 |
| `npm --prefix rhwp-studio run build` | fresh WASM 선언 기반 TypeScript·Vite production build 성공 |
| `git diff --check upstream/devel...HEAD` | 통과 |

보정 전 원격 head `1990916ab`에서는 CodeQL 3언어, Canvas, lint, frontend package, Native Skia, 세 builder,
네 worker와 `Build & Test`가 모두 성공했다. 이는 이전 head의 역사 근거이며 최종 merge 근거로 재사용하지
않는다. 이번 push도 workflow/classifier 변경 때문에 full lane으로 닫히므로 최신 head에서 같은 required
CI를 다시 통과해야 한다.

## 시각·fixture 판정

별도 시각 증적은 적용하지 않았다. PR 고유 변경은 CI workflow, classifier, 계약 테스트, CI 전용 TypeScript
stub과 계획·검토 기록이며 renderer, layout, paint, pagination, sample, golden과 기준 PDF를 바꾸지 않는다.
Canvas 실행 조건 자체는 최신 head GitHub Actions에서 확인한다.

## 위험과 후속 조건

- Stage 3 merge 전에는 devel 대상 다른 PR에서 새 unit/none lane을 실측할 수 없다. merge 직후 canary가
  Stage 4 착수의 선행 조건이다.
- `src/command/**`에 WASM 직접 import나 package 경계 변경이 들어오면 classifier 규칙을 먼저 package로
  승격해야 한다.
- #3789가 완료되기 전에는 `src/main.rs`를 render 포함 full 경계로 유지한다.
- #3790은 Stage 4–7과 post-main enforcement가 남으므로 이 PR merge로 close하지 않는다.
- 최신 head CI가 통과해도 ready 전환·merge는 사용자가 직접 수행한다.

**현재 권고: 최신 head CI 대기.** 리뷰 1–7 대응과 최신 devel 기준 로컬 검증은 완료됐다. 이 문서를
포함한 head의 GitHub Actions가 모두 성공하면 collaborator self-merge 후보로 판단할 수 있다.
