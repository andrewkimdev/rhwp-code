---
kind: review
status: active
pr: 3792
---

# PR #3792 검토: CI 변경 영향축 shadow classifier

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           multi_pr_update_branch.md, review_only_fast_pass.md,
           rework_and_exceptions.md(대형 PR)
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, multi_pr_update_branch.md,
                  review_only_fast_pass.md, rework_and_exceptions.md
current head: 1064aadaefd675034dfea7f67b20b9ac6837096b (문서 작성 시점 참고값)
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#3792](https://github.com/edwardkim/rhwp/pull/3792) |
| 작성자 | `postmelee` |
| 대상 / head | `devel` / `codex/issue-3790-ci-impact-shadow` |
| 작성 시점 상태 | ready, `MERGEABLE`, `CLEAN` (correction candidate full CI 통과) |
| 변경 규모 | 9 files, +1,187 / -0 lines, 7 commits |
| 기준 devel | `6ab503fe97b7abfd1839800c5c018da9f9abf4c5` |
| correction candidate | `1064aadaefd675034dfea7f67b20b9ac6837096b` |

위 상태값은 문서 작성 시점 참고값이다. 최종 merge 조건은 최신 PR head의 GitHub Actions 통과와
작업지시자 승인이다. formal reviewer로 `edwardkim`을 지정했다.

## 변경 범위

이 PR은 frontend-only 변경에도 Rust·Native Skia·Canvas·모든 CodeQL 언어가 실행되는 비용을 줄이기 위한
Stage 1이다. 실제 worker 조건을 바꾸지 않고 다음 영향축을 순수 Node classifier로 계산해 preflight
output과 Job Summary에 advisory 값으로만 기록한다.

- `rust_required`
- `frontend_mode=none|unit|package`
- `render_required`
- `native_skia_required`
- `codeql_languages`
- `classification_status`, `classifier_version`, `reason`

#3785, #3656, #3670, #3672, #3690의 실제 변경 파일을 historical fixture로 고정했다. workflow,
classifier, Cargo, Rust toolchain, `src/main.rs`, WASM, rename, 파일 목록 오류·경계와 미분류 경로는
`full`로 닫는다.

## review 보정

[검토 코멘트](https://github.com/edwardkim/rhwp/pull/3792#issuecomment-5159164270)를 코드와 실제 run으로
대조한 뒤 다음을 correction commit `aa81855e6`에 반영했다.

- preflight에서 30초를 사용한 advisory checkout을 classifier 한 파일의 sparse checkout으로 좁혔다.
- worker가 `shadow_*` output을 소비하지 않는 계약을 marker 구간뿐 아니라 workflow 전체 참조로도
  검증한다.
- pull request 기본 checkout이 merge ref인 사실에 맞춰 authority를 `pr-merge-advisory`로 고쳤다.
- 존재하지 않는 `web/` frontend package prefix를 제거하고 새 경로는 fail-closed하도록 테스트했다.
- `frontend_mode=unit`이 Studio 전역 `tsc --noEmit`과 전체 unit test를 포함한다는 활성화 계약을
  계획서에 고정했다.
- Render Diff의 Canvas visual diff와 CanvasKit readiness 의존성을 분리하거나 보수적 합집합으로
  확정하기 전에는 `render_required`를 skip 근거로 쓰지 않도록 후속 조건을 명시했다.
- #3684가 #3810으로 완료돼 cache 기준선이 4.73GB로 고정된 사실을 반영하고, CodeQL 단계의 대기 조건을
  기준선 회귀 확인으로 바꿨다.

판정값은 classifier step의 JSON으로 출력되지 않지만 summary step의 `SHADOW_*` 환경값으로 Actions
로그에도 남는다. 따라서 stdout JSON 추가는 필수 보정에서 제외했다.

## 검증

### 로컬

correction commit을 최신 `devel`과 합친 `1064aadae`에서 다음 검증을 실행해 통과했다.

| 검증 | 결과 |
| --- | --- |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 20 passed / 0 failed |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py scripts/tests/test_render_diff_workflow.py` | 6 passed |
| `actionlint .github/workflows/ci.yml` | 통과, 경고 없음 |
| `git diff --check upstream/devel...HEAD` | 통과 |
| `git merge --no-commit --no-ff upstream/devel` | 충돌 없음; update merge로 확정 |

최신 `devel` 자체 evidence 파일의 기존 trailing whitespace는 simulation 중 확인했지만 PR diff에는 포함되지
않으며, `upstream/devel...HEAD` 범위의 diff check는 통과했다.

### GitHub Actions

- 이전 code head `37ee8845e`의 [CI](https://github.com/edwardkim/rhwp/actions/runs/30754352076)와
  [CodeQL](https://github.com/edwardkim/rhwp/actions/runs/30754352077)은 모두 통과했다.
- correction candidate `1064aadae`의 [full CI](https://github.com/edwardkim/rhwp/actions/runs/30756989300)와
  [CodeQL](https://github.com/edwardkim/rhwp/actions/runs/30756989297)은 모두 통과했다. CI에서는 lint,
  frontend package gates, test archive, Native Skia, default-feature 8개 shard와 `Build & Test` aggregate가
  모두 성공했고, CodeQL에서는 JavaScript/TypeScript, Python, Rust와 aggregate가 모두 성공했다.
- sparse checkout 보정 후 CI preflight는 이전 candidate의 39초에서 9초로, advisory checkout step은
  30초에서 1초로 줄었다. 이 수치는 동일 PR의 위 두 full CI run에서 확인한 관찰값이다.

### 시각·fixture 판정

시각 증적은 적용하지 않았다. 변경 범위는 CI workflow, 순수 Node classifier, 정적 계약 테스트와 계획
문서이며 renderer·layout·typeset·pagination·sample·golden·기준 PDF를 바꾸지 않는다.

## 위험과 후속 조건

- 현재 classifier는 advisory이고 기존 worker 조건은 `shadow_*`를 소비하지 않으므로 알려진 render
  분류 비대칭이 이 PR에서 검증을 생략시키지는 않는다.
- 실제 skip 활성화 전에는 trusted base classifier를 사용하고, Canvas visual diff와 CanvasKit readiness의
  실제 의존 경계를 확정하며, `canvaskit` 이름 heuristic의 피시험 코드/테스트 비대칭을 제거해야 한다.
- `frontend_mode=unit`은 전역 TypeScript 검사 없이 활성화할 수 없다.
- #3789가 완료되기 전에는 `src/main.rs`를 full 경계로 유지한다.
- 기존 `mydocs/orders/20260802.md`는 최초 PR 준비 기록을 이미 포함한다. PR 번호나 날짜만 보태기 위한
  재갱신은 하지 않는다.

**현재 권고: 조건부 merge 후보.** correction candidate의 full CI와 CodeQL은 모두 통과했다. 이 review
문서를 single-parent trailing commit으로 push한 뒤 최신 문서 전용 head의 review-only fast-pass와
`Build & Test` aggregate를 확인해야 한다. 그 뒤에도 작업지시자의 명시적 merge 요청 전에는 merge하지
않는다.
