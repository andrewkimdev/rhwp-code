# 구현계획서 — task_m100_3790

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **수행계획서**: `mydocs/plans/task_m100_3790.md`
- **브랜치**: Stage 1 `codex/issue-3790-ci-impact-shadow`, Stage 2
  `codex/issue-3790-shadow-observation`
- **절차 상태**: Stage 2 1차 실측 완료, live frontend `unit|package`·render 표본 대기

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
Stage 3는 활성화하지 않는다. historical replay 60건은 경로 규칙과 비용의 보조 근거로만 사용한다.

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

1. Rust 비영향 PR에서 lint/archive/shard를 생략한다.
2. Rust 변경 중 render 비영향 경로는 Native Skia를 생략한다.
3. Rust formatter·passthrough invalidation·IR baseline 회귀가 필요한 경로는 기존 전체 검증을 유지한다.
4. #3684를 완료한 #3810의 정리 후 cache 기준선 4.73GB와 조건화 이후 총량을 대조한다.

## Stage 5 이후

- #3810의 4.73GB cache 기준선 회귀 여부를 확인하며 CodeQL 언어별 matrix를 활성화한다.
- shard count artifact 재시도와 draft 경량화는 각각 독립 PR로 진행한다.
- #3789가 완료되기 전에는 `src/main.rs`의 Render Diff trigger를 좁히지 않는다.

## Stage 1 집중 검증

```bash
node --test scripts/tests/ci-impact-classifier.test.cjs
python3 -m unittest scripts/tests/test_ci_impact_workflow.py
python3 -m unittest scripts/tests/test_render_diff_workflow.py
git diff --check
```

검증 결과는 `mydocs/working/task_m100_3790_stage1.md`에 명령과 종료 상태를 기록한다.
