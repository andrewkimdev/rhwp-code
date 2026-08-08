---
kind: pr_review
status: ci-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-08
---

# PR #4266 검토 — #4150 IME 조합 오버레이 stale 좌표 + HF/FN 재정박 offset

## 결론

**Open PR 생성, 로컬 TypeScript·전체 studio 테스트·`git diff --check` 통과.** 커밋 1(`e6a20f92d`)은
IME 조합 중 같은 페이지 안 reflow가 일어나면 조합 오버레이가 옛 좌표에 그려져 실제 캔버스 텍스트와
겹치던 결함(#4150)을 `compositionAnchorRect` 캐시 제거로 고치고, 조합 replace가 wasm deferred replace
범위 가드에 거부될 때 `onInput` 밖으로 예외가 던져져 조합 추적이 wedge되던 경로를 try/catch로 방어한다.
커밋 2(`c03ee47a0`)는 이 리뷰 과정에서 발견한 회귀 — 위 catch 재정박이 머리말/꼬리말·각주 모드에서
`cursor.getPosition()`의 stale 본문 offset을 그대로 썼던 것 — 을 `onCompositionStart`와 동일한
`hfCharOffset`/`fnCharOffset` override로 고쳤다.

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md
devel base: e919655a7 (upstream/devel HEAD at PR 생성 시점)
validated code head: c03ee47a0142154c737c3a7c7a6da857fb4de764
```

원 후보는 `integration/all-works`(로컬, #4180/#4179 등 다른 이슈 커밋과 커밋되지 않은 무관한 변경이
섞인 통합 branch)에서 나왔다. 그 branch를 그대로 push하면 base가 크게 뒤처지고(분기 이후 devel
185커밋 진행) 무관한 이슈가 같은 PR에 섞이므로, `e6a20f92d`/`c03ee47a0` 두 커밋만 최신
`upstream/devel` 위 별도 worktree에 cherry-pick해 이 PR로 분리했다. cherry-pick은 두 커밋 모두
conflict 없이 적용됐다(`input-handler.ts` auto-merge).

별도 `review_impl` 문서는 만들지 않았다. 단일 이슈의 2-커밋 fix이고 실행 순서·rollback 경계가 이 문서
하나로 충분히 명확하다.

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#4266](https://github.com/edwardkim/rhwp/pull/4266) |
| 관련 이슈 | #4150 |
| 작성자 | `humdrum00001010` (fork 기반 — `upstream` 직접 push 권한 없음, 403 확인 후 `origin` fork로 push) |
| reviewer | 최근 동일 작성자 PR(#4259–#4262)과 동일하게 `jangster77` 지정 시도 — 권한 부족으로 `gh pr edit --add-reviewer` 실패(`RequestReviewsByLogin` 권한 없음). 작업지시자가 수동으로 지정 필요 |
| 대상 / head | `devel` / `humdrum00001010:task_m100_4150` (fork branch) |
| 생성 상태 | open, non-draft |
| 생성 시점 규모 | 5 files, +253 / -87, 2 commits |
| 생성 시점 merge 상태 | mergeable, `BLOCKED` — CI 진행 중 참고값 |

위 head·규모·merge 상태는 PR 생성 직후 참고값이다. merge 직전에 다시 확인한다.

## 렌더 영향 판정

시각·fixture 증적 보조 경로는 적용하지 않는다. 조합 블랙박스 오버레이(`caret-renderer.ts`의 `compEl`)는
canvas paint가 아니라 절대좌표 DOM 엘리먼트 위치 계산이고, 이 PR은 `src/renderer`, `wasm_api.rs`,
golden/fixture, HWP/HWPX sample을 전혀 건드리지 않는다.

## 로컬 검증

cherry-pick한 code head(`c03ee47a0`)를 최신 `upstream/devel` 기준 별도 worktree에서 검증했다.
Rust/wasm 변경이 없는 rhwp-studio 전용 PR이라 [4.3 표](../../manual/pr_review/local_validation.md#43-변경-범위별-기본-검증)의
"rhwp-studio만 변경" 행을 따랐다.

| 검증 | 결과 |
| --- | --- |
| `npx tsc --noEmit` | PASS |
| `node --test tests/*.test.ts ../npm/editor/tests/*.test.mjs` | 805/805 pass |
| `git diff --check` (devel..HEAD) | PASS |
| 새 행위 테스트 회귀 확인 | `composition-hf-fn-reanchor.runner.mjs`를 수정 전 소스로 되돌려 재실행 → HF 케이스에서 의도대로 fail(`expected 0, actual 1`), 원복 후 재실행 → pass. `git diff`로 원복 무결성 확인 |

wasm 타입 선언은 이 worktree에 fresh `wasm-pack build`를 새로 돌리지 않고, 이 PR과 무관한 Rust 소스로
만든 기존 `pkg/`(원 checkout, 오늘자 빌드)를 복사해 재사용했다 — 이 PR이 `.rs` 파일을 전혀 바꾸지 않아
API 표면이 같으므로 `tsc` 타입체크 목적에는 안전하다고 판단했다. 실제 IME 조합(OS 후보창)을 통한
수동 재현은 자동화 범위 밖이라 생략했고, 대신 `:7701` 실행 중인 dev 서버에서 synthetic
compositionstart/update/end 이벤트로 골든 패스 무오류를 확인했다(이전 리뷰 기록).

## 발견한 문제

없음. `e6a20f92d`(1차 커밋) 리뷰에서 지적한 HF/FN 재정박 offset 결함은 `c03ee47a0`(2차 커밋)이
`onCompositionStart`와 동일 규약으로 정확히 고쳤고, 그 수정을 검증하는 행위 테스트가 수정 전 상태에서
실패함을 직접 확인했다.

## GitHub Actions와 남은 게이트

- PR 생성 직후 `CI preflight`/`CodeQL preflight`/`Render Diff preflight`는 성공, `Frontend package gates`
  등 나머지는 진행 중이었다(참고값, 재확인 필요).
- `mergeStateStatus: BLOCKED`는 CI 진행 중 참고값이며 최신 head의 required check 전체 성공을 merge
  직전에 다시 확인해야 한다.
- reviewer 지정은 권한 부족으로 실패했다 — 작업지시자가 GitHub에서 직접 `jangster77`(또는 다른
  reviewer)을 지정해야 한다.

## 최종 권고

로컬 TypeScript·전체 테스트·diff 검사는 통과했다. 최신 head의 required CI 성공과 reviewer 지정·승인
확인 후 merge를 권고한다.
