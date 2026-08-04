# 구현계획서 — task_m100_3790

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **수행계획서**: `mydocs/plans/task_m100_3790.md`
- **브랜치**: Stage 1 `codex/issue-3790-ci-impact-shadow`, Stage 2·2.5
  `codex/issue-3790-shadow-observation`, Stage 3 `codex/issue-3790-stage3-frontend`
- **절차 상태**: Stage 2.5 merge 완료, Stage 2.6 enforcement 분리 결정 및 Stage 3 로컬 구현 완료

## Stage 1 — shadow classifier

1. `scripts/ci-impact-classifier.cjs`에 부작용 없는 변경 집합 판정 함수를 둔다.
2. `scripts/tests/fixtures/ci-impact-classifier-prs.json`에 #3785, #3656, #3670, #3672, #3690의 실제
   변경 파일과 기대 출력을 고정한다.
3. 단위 테스트에서 historical fixture, mode 승격, 언어 집합, review-only, fail-closed 경계를 검증한다.
4. `CI preflight`가 PR/push 파일 목록을 수집해 classifier를 호출하고 `shadow_*` output과 Job Summary를
   기록하게 한다.
5. workflow 계약 테스트에서 shadow output이 기존 worker 조건에 사용되지 않음을 확인한다.

pull request에서는 checkout된 merge ref의 classifier가 실행되므로 Stage 1 결과는 advisory다. 실제 skip을
활성화하는 PR은 base SHA의 classifier를 사용하거나 동등한 trusted execution 경계를 먼저 구현해야 한다.

## Stage 2 — shadow 실측

1. draft PR의 각 run에서 shadow summary와 실제 변경 파일을 대조한다.
2. 분류 실패, API 경계, rename, mixed 변경의 full fallback을 확인한다.
3. 실제 worker duration과 예상 절감 runner-minute를 기록한다.
4. false negative가 있으면 규칙과 fixture를 먼저 보정하고 활성화를 연기한다.

1차 실측 결과는 `mydocs/working/task_m100_3790_stage2.md`에 기록했다. 네 live run은 모두 완료됐고
#3740에서 rename full fallback과 기존 Rust fmt 차단을 확인했다. historical replay 60건, 고정 fixture와
현재까지 관측 false negative 0건을 Stage 3 활성화 근거로 사용하며 자연 발생 frontend 표본 5건은 더
기다리지 않는다.

## Stage 2.5 — trusted-base shadow

1. PR에서는 `github.event.pull_request.base.sha`의 classifier만 sparse checkout한다.
2. push·manual 실행은 해당 실행의 `github.sha`를 사용한다.
3. PR authority를 `pr-base-trusted-shadow`로 기록하고 기존 merge-ref advisory 표본과 분리한다.
4. checkout credential을 저장하지 않고 classifier node step에는 토큰을 전달하지 않는다.
5. 기존 worker 조건이 shadow output을 소비하지 않는 정적 계약을 유지한다.
6. base SHA ref, classifier 파일 존재, authority, review-only fast-pass와 fail-open shadow 동작을
   workflow 테스트로 고정한다.

이 단계의 PR CI가 통과해도 worker skip은 활성화되지 않았다. #3823 merge 뒤 base SHA classifier 경계가
devel에 반영됐으며, Stage 3부터 이 출력을 실제 frontend gate에 연결한다.

Stage 2.5가 고정하는 신뢰 경계는 `scripts/ci-impact-classifier.cjs`의 출처뿐이다. `pull_request`의
workflow YAML, 인라인 collect script, classifier 실행 명령과 Stage 3에서 추가할 worker `if`는 PR merge
ref의 제어를 받는다. 따라서 `pr-base-trusted-shadow`는 classifier-source provenance이지 실제 skip을
허용하는 trusted execution 증명이 아니다.

## Stage 2.6 — devel 활성화와 post-main enforcement 분리

1. contributor/collaborator를 구분하지 않고 모든 genuine frontend-only PR에 기존 `pull_request`
   workflow의 선택 실행을 적용한다.
2. workflow/classifier/Cargo/WASM/rename/미분류 변경과 classifier/API 오류는 full로 닫는다.
3. 약 1,500줄 local controller 프로토타입은 원격에 게시하지 않고 Stage 3~5의 최종 job 진리표를 반영할
   후속 controller의 설계·테스트 근거로 보존한다.
4. Stage 3~5와 canary는 main 릴리즈 전에 devel 대상 PR에서 실행한다. controller는 별도 main PR로
   등록하지 않고 정상 `devel → main` 릴리즈를 기다린다.
5. main 등록 뒤 controller가 PR head code/artifact를 실행하지 않고 실제 job의 expected
   `success|skipped`를 독립 감사하게 한다. repository admin이 required status를 채택해야 merge
   enforcement가 완성된다.

controller 프로토타입은 대체 controller가 main에서 live audit까지 통과하거나 maintainer가 required
policy를 미채택하기로 결정할 때까지 보존한다. 이후 재사용할 설계·테스트 근거를 계획·보고서에 옮긴 뒤
사용자 승인으로 local branch/worktree를 정리한다.

## Stage 3 — frontend unit/package/render 활성화

1. `unit`은 Studio 전체 `src`의 `tsc --noEmit`과 전체 Studio unit test를 실행한다. fresh WASM build는
   생략하며 CI 전용 tsconfig가 `@wasm/rhwp.js`만 최소 stub으로 치환한다.
2. `package`는 `unit` 계약에 Vite·extension·package build를 추가한다.
3. Render Diff의 Canvas visual diff와 CanvasKit readiness가 실제로 소비하는 경로를 각각 도출한다.
   영향축을 분리하지 않으면 두 gate 의존성의 보수적 합집합만 `render_required`에 연결한다.
4. `canvaskit` 파일명 heuristic처럼 피시험 코드와 테스트를 다르게 분류하는 규칙을 제거하고 계약
   fixture로 고정한다.
5. aggregate는 필요한 worker `success`, 불필요 worker `skipped`만 허용한다.
6. #3785/#3656은 unit, #3670은 package, #3672는 unit+render 경로를 실측한다.
7. `ci:full` label은 모든 축을 full로 승격한다. `labeled|unlabeled` 이벤트로 같은 SHA를 full/selective
   두 번 실행할 수 있게 하고, canary에서는 full 완료 뒤 selective를 실행한다.
8. 작성자 association은 선택 실행 조건으로 사용하지 않는다. 외부 fork의 정상 frontend-only PR도 같은
   영향축을 사용하며, workflow 변경은 author와 무관하게 full이다.
9. WASM binding을 직접 소비하는 `src/core/**`, `src/embed/**`, `src/main.ts`, `public/**`, `src/hwpctl/**`은
   package lane으로 승격한다. CI 전용 tsconfig·stub 자체 변경도 full로 닫는다.

## Stage 4 — Rust·Native Skia 조건화

1. Rust 비영향 PR에서 lint와 #3892의 `build-test-archive-slow`, `build-test-archive-a`,
   `build-test-archive-b` 세 builder를 생략한다.
2. 같은 영향축으로 `test-slow-shard`, `test-regular-shard-1`, `test-regular-shard-2`,
   `test-regular-shard-3` 네 worker를 조건화한다.
3. aggregate는 세 builder와 네 worker 각각에 필요한 `success` 또는 불필요한 `skipped`만 허용한다.
4. Rust 변경 중 render 비영향 경로는 Native Skia를 생략한다.
5. Rust formatter·passthrough invalidation·IR baseline 회귀가 필요한 경로는 기존 전체 검증을 유지한다.
6. #3684를 완료한 #3810의 정리 직후 cache 기준선 4.73GB와 조건화 이후 다음 sweep 직후 총량을
   같은 시점 조건으로 대조한다.

## Stage 5 이후

- #3810의 4.73GB cache 기준선 회귀 여부를 확인하며 CodeQL 언어별 matrix를 활성화한다.
- Stage 3·4 merge 뒤 첫 canary, Stage 5 merge 뒤 두 번째 canary에서 동일 SHA full/selective를 대조한다.
- default-branch controller는 Stage 3~5 진리표가 확정된 뒤 축소 구현하고 정상 릴리즈로 main에 등록한다.
- artifact 재시도는 #3892의 논리 label `slow/1/2/3`별 test archive, archive expected count와 worker run
  count를 함께 다루고, draft 경량화와 별도 PR로 진행한다.
- #3789가 완료되기 전에는 `src/main.rs`의 Render Diff trigger를 좁히지 않는다.

## 집중 검증

```bash
node --test scripts/tests/ci-impact-classifier.test.cjs
python3 -m unittest scripts/tests/test_ci_impact_workflow.py
python3 -m unittest scripts/tests/test_render_diff_workflow.py
git diff --check
```

Stage 1 검증 결과는 `mydocs/working/task_m100_3790_stage1.md`, Stage 2·2.5 결과는
`mydocs/working/task_m100_3790_stage2.md`, Stage 3 결과는
`mydocs/working/task_m100_3790_stage3.md`에 명령과 종료 상태를 기록한다.
