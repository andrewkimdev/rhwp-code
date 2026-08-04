# 구현계획서 — task_m100_3790

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **수행계획서**: `mydocs/plans/task_m100_3790.md`
- **브랜치**: Stage 1 `codex/issue-3790-ci-impact-shadow`, Stage 2
  `codex/issue-3790-shadow-observation`
- **절차 상태**: Stage 2.5 리뷰 보정 및 #3892 이후 CI topology 로컬 검증 완료, 최신 head CI 전

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

1차 실측 결과는 `mydocs/working/task_m100_3790_stage2.md`에 기록한다. merge 이후 live shadow는
고유 PR 4건뿐이고 frontend `unit|package` 및 `render_required=true`의 live non-full 표본이 없으므로
Stage 3는 활성화하지 않는다. 네 run은 모두 완료됐고 #3740에서 rename full fallback과 기존 Rust fmt
차단을 확인했다. historical replay 60건은 경로 규칙과 비용의 보조 근거로만 사용한다.

## Stage 2.5 — trusted-base shadow

1. PR에서는 `github.event.pull_request.base.sha`의 classifier만 sparse checkout한다.
2. push·manual 실행은 해당 실행의 `github.sha`를 사용한다.
3. PR authority를 `pr-base-trusted-shadow`로 기록하고 기존 merge-ref advisory 표본과 분리한다.
4. checkout credential을 저장하지 않고 classifier node step에는 토큰을 전달하지 않는다.
5. 기존 worker 조건이 shadow output을 소비하지 않는 정적 계약을 유지한다.
6. base SHA ref, classifier 파일 존재, authority, review-only fast-pass와 fail-open shadow 동작을
   workflow 테스트로 고정한다.

이 단계의 PR CI가 통과해도 worker skip은 활성화되지 않는다. merge 뒤 trusted authority의 completed
`classified` code run을 최소 5건 관찰하고 frontend `unit|package`와 render 표본을 확보한 뒤 Stage 3
착수 여부를 다시 판정한다.

Stage 2.5가 고정하는 신뢰 경계는 `scripts/ci-impact-classifier.cjs`의 출처뿐이다. `pull_request`의
workflow YAML, 인라인 collect script, classifier 실행 명령과 Stage 3에서 추가할 worker `if`는 PR merge
ref의 제어를 받는다. 따라서 `pr-base-trusted-shadow`는 classifier-source provenance이지 실제 skip을
허용하는 trusted execution 증명이 아니다.

## Stage 2.6 — trusted enforcement boundary

1. 실제 skip 판정과 required check를 base/default branch가 제어하는 controller 또는 동등한 policy
   check에서 산출하는 방안을 먼저 확정한다.
2. trusted controller를 적용하지 못하는 fork·untrusted head PR은 기존 full CI를 유지한다.
3. worker 조건은 Stage 2.5 authority 문자열만 소비하지 않으며, policy 결과가 없거나 실패하면 full로 닫는다.
4. trusted/untrusted PR, classifier 부재, API 실패와 workflow 자체 변경을 fixture 및 workflow 계약
   테스트로 고정한다.

Stage 3는 Stage 2.5 표본 게이트와 Stage 2.6 실행 경계를 모두 충족한 뒤에만 시작한다.

## Stage 3 — frontend unit/package/render 활성화

1. `unit`은 Studio 전체 `src`의 `tsc --noEmit`과 전체 Studio unit test를 실행한다.
2. `package`는 `unit` 계약에 Vite·extension·package build를 추가한다.
3. Render Diff의 Canvas visual diff와 CanvasKit readiness가 실제로 소비하는 경로를 각각 도출한다.
   영향축을 분리하지 않으면 두 gate 의존성의 보수적 합집합만 `render_required`에 연결한다.
4. `canvaskit` 파일명 heuristic처럼 피시험 코드와 테스트를 다르게 분류하는 규칙을 제거하고 계약
   fixture로 고정한다.
5. aggregate는 필요한 worker `success`, 불필요 worker `skipped`만 허용한다.
6. #3785/#3656은 unit, #3670은 package, #3672는 unit+render 경로를 실측한다.

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
`mydocs/working/task_m100_3790_stage2.md`에 명령과 종료 상태를 기록한다.
