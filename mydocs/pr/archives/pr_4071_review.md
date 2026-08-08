# PR #4071 검토

## 결론

**수용 후보.** review-only trailing commit이 같은 PR의 검증 완료 code candidate를 재사용할 때,
현재 `devel`의 전진만으로 재실행하지 않도록 CI 정책을 보정했다. 재사용 대상은 경로만으로 판단하지 않고
같은 PR source repository·branch·candidate SHA·PR 생성 이후의 Actions run으로 고정한다.

최종 병합 조건은 이 검토·오늘 기록을 포함한 최신 PR head의 GitHub Actions 통과와 작업지시자 승인이다.

## 접수 및 기준

| 항목 | 내용 |
| --- | --- |
| PR | [#4071](https://github.com/edwardkim/rhwp/pull/4071) `ci: review-only trailing commit 재사용 조건 보정` |
| 작성자 | `jangster77` |
| 대상 | `devel` |
| 구현 head | `cb25176799d6f3381f363f8b8b1c920c93f1907b` |
| 구현 기준 devel | `efed25b43` |
| 관련 이슈 | [#4070](https://github.com/edwardkim/rhwp/issues/4070) |
| 구현 변경 규모 | 7 files, +150 / -125 |
| 코드 후보 전체 CI | CI `30999551801`, CodeQL `30999551184`, Render Diff `30999550538` 모두 성공 |
| 문서 작성 시점 현재 devel | `061778ff8` (PR #4048 병합 commit) |
| 문서 작성 시점 mergeable | `MERGEABLE` |
| 문서 작성 시점 merge 상태 | `CLEAN` |

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, rework_and_exceptions.md
```

## 변경 내용

- CI, CodeQL, Render Diff preflight에서 candidate가 현재 base를 조상으로 포함해야 한다는 조건을 제거했다.
  `devel` 전진만으로 contributor branch에 Update branch·merge·rebase를 요구하지 않는다.
- CI와 CodeQL은 generic check-run을 재사용하지 않는다. 같은 `pull_request` event의 workflow run에서
  candidate SHA, head branch, source repository, PR 생성 시각을 확인하고 해당 run의 aggregate job만 조회한다.
- fork PR에 `listWorkflowRuns`의 서버 측 branch filter를 보내면 일치하는 run이 누락되는 것을 확인해,
  API 응답을 받은 뒤 branch·repository·SHA를 직접 비교하도록 보정했다.
- 세 workflow의 정책 불일치를 막는 `test_review_only_fast_pass_workflows.py`를 추가하고, PR workflow 문서에
  trailing 문서 기록의 정확한 조건을 명시했다.
- 구현 후보가 전체 CI를 통과한 뒤 PR #4048이 `devel`에 병합되어 base가 `efed25b43`에서 `061778ff8`로
  전진했다. 이 검토 기록 commit은 그 뒤의 최신 base에서 reuse 동작을 검증하는 대상이다.

## 로컬 검증

아래 검증은 구현 head `cb2517679`에서 완료됐다.

| 검증 | 결과 |
| --- | --- |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py scripts/tests/test_render_diff_workflow.py scripts/tests/test_review_only_fast_pass_workflows.py` | 17 passed |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 24 passed |
| PyYAML로 CI·CodeQL·Render Diff workflow 파싱 | 3 files passed |
| `actionlint .github/workflows/ci.yml .github/workflows/codeql.yml .github/workflows/render-diff.yml` | 통과 |
| GitHub Script preflight 3개를 async wrapper로 구문 검사 | 통과 |
| 실제 PR #4048 source SHA의 CI·CodeQL Actions run 조회 | source repository·branch·SHA 일치, 두 workflow 모두 성공 run 확인 |
| `git diff --check` | 통과 |

## 범위와 위험

- source, parser, renderer, Studio, fixture는 변경하지 않았다. Canvas/PDF 시각 검증은 대상이 아니다.
- candidate SHA의 workflow가 누락·실패·미완료하거나 PR identity가 다르면 fail-closed로 full CI를 실행한다.
- review-only 범위 밖 파일, source·test·workflow 변경, 허용되지 않은 merge 형태도 fast-pass 대상이 아니다.
- 최신 base 병합을 강제하지 않으므로 merge 직전 GitHub의 최신 mergeable 상태와 required aggregate를 반드시
  다시 확인한다.

## 최종 권고

이 검토·오늘 기록만 담은 마지막 commit을 같은 PR head에 push한 뒤, 최신 base `061778ff8`에서도
heavy worker가 skip되고 aggregate가 성공하는지 확인한다. 이 확인과 작업지시자 승인이 끝난 뒤 PR #4071을 병합한다.
