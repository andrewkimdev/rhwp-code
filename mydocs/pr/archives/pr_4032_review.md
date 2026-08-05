---
kind: pr_review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-05
---

# PR #4032 검토 — Stage 4 Rust·Native Skia 영향축 활성화

## 결론

**review-only fast-pass 확인 전 merge 후보.** Stage 3의 frontend·render 조건화에 이어
`rust_required`와 `native_skia_required`를 lint·3 builders·4 workers·Native Skia job에 연결했다.
사용자 review의 F1–F4를 보정 commit `1f12a5fe0`에 반영했고, F5는 기존 결함
#4040으로 분리했으며, F6으로 이 review 문서와 오늘 기록을 추가한다.

보정 head, current-base head, 그리고 최종 code head `5eeab15fd` 셋 모두 full CI·Canvas·3개 언어
CodeQL이 통과했다. 최종 head aggregate에서 5,252건의 default-feature test가 누락·중복 없이
실행됐다. 이 문서 commit의 review-only fast-pass 통과와 작업지시자 승인을 최종 merge
조건으로 남긴다.

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           review_only_fast_pass.md, multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, review_only_fast_pass.md,
                  multi_pr_update_branch.md
reviewed user head: 61a616769c48a6124934a9ce4294e4ee143e1849
functional correction: 1f12a5fe0dbd8a39c7e585c56b4829affab65aef
current-base candidate: b0be8673149bbd00ebb67f6d5e62b70025cfa612
final code head: 5eeab15fd291b2b4b27d3b8a77498fcc0ca5723b
current devel: 955910136a33ff7d3438b661c27a1c114acf78bc
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#4032](https://github.com/edwardkim/rhwp/pull/4032) |
| 작성자 | `postmelee` (collaborator self-merge) |
| 대상 / head | `devel` / `postmelee:issue-3790-stage4-rust-native` (cross-repository) |
| 문서 작성 전 원격 상태 | draft, `MERGEABLE`, `CLEAN` |
| review | [pullrequestreview-4855664704](https://github.com/edwardkim/rhwp/pull/4032#pullrequestreview-4855664704), `COMMENTED` |
| review 대상 head | `61a616769c48a6124934a9ce4294e4ee143e1849` |
| 기능 보정 | `1f12a5fe0dbd8a39c7e585c56b4829affab65aef` |
| 첫 devel merge | `b0be8673149bbd00ebb67f6d5e62b70025cfa612` |
| 최종 code head | `5eeab15fd291b2b4b27d3b8a77498fcc0ca5723b` |
| 문서 작성 전 PR 고유 규모 | 13 files, +707 / -104 |
| 관련 issue | [#3790](https://github.com/edwardkim/rhwp/issues/3790), [#4040](https://github.com/edwardkim/rhwp/issues/4040) |
| metadata | label·milestone·assignee·review request 없음 |

draft, mergeability, head SHA와 CI는 작성 시점 참고값이다. review request는 보내지 않는다.
작업지시자 지시에 따라 이 문서 commit push와 함께 draft를 ready로 전환하고, merge는 이후
작업지시자의 별도 지시로 수행한다.

## 변경 범위와 안전 계약

- `rust_required=false`: Rust lint, 세 archive builder와 네 default-feature worker를 생략한다.
- `native_skia_required=false`: Native Skia job을 생략한다.
- frontend `none|unit|package`와 Rust·Native 축의 정확한 `success|skipped` 조합만 aggregate가
  수용하며 unknown, failure, cancellation, 예상 밖 실행을 실패시킨다.
- workflow·classifier·Cargo·WASM·rename·미분류·판정 오류는 full로 닫힌다.
- CodeQL 언어 조건화는 Stage 5 범위로 유지한다.

## review F1–F6 대응

### F1. Rust test-owned render input의 default-feature 테스트 보존

지적은 merge blocker로 판단했다. 다만 모든 `render-input`·`font-render-input`에
`rust_required=true`를 설정하면 Studio font asset·Python 생성 도구·문서까지 Rust lane을 과대
실행한다. 보정은 default-feature Rust 테스트가 직접 소비하는 다음 경계로 한정했다.

- `ttfs/**`와 `tests/fixtures/fonts/**`의 `.otf|.ttc|.ttf|.woff|.woff2`
- `samples/render-p35-font-native-bitmap.hwpx`

이 경로는 `rust=true`, `render=true`, `native=true`, `codeql=none`,
`reason=classified:rust-test-input`으로 고정했다. 반면 `assets/fonts/**`, render fixture 생성 Python,
render 문서는 `rust=false`, `render=true`, `native=true`를 유지하는 음성 fixture를 추가했다.

### F2. Native Skia frontend 진리표 가드 복구

workflow 계약 테스트가 Native Skia job의 `frontend-unit-gates`, `frontend-package-gates` 의존성과
`frontend_mode=none|unit|package` 조건을 모두 단언하게 보강했다. frontend 진리표를 제거하는
mutation은 이제 계약 테스트에서 실패한다.

### F3. aggregate harness step/job 경계 보정

`_step()`이 다음 `- name:`뿐 아니라 다음 top-level job 경계에서도 멈추게 했다.
`wasm-build:`와 `startsWith(github.ref` 문자열이 aggregate shell에 포함되지 않음을 단언하고,
GitHub 기본 shell과 같이 `bash -e -o pipefail -c`로 실행한다. 거부 테스트가 YAML 파싱
오류로 우연히 통과할 수 있던 경로를 제거했다.

### F4. canonical CI 의존 그래프 갱신

`mydocs/manual/pr_review_workflow.md` §3.1을 Stage 4 조건부 그래프로 갱신했다. lint는
Rust 필요 시만, Native는 자신의 축과 Rust lane의 lint `success|skipped`를 조건부로,
worker는 자기 builder와 Native `success|skipped`를 조건부로 확인한다는 실제 workflow를 반영했다.

### F5. 기존 Native Skia integration target 누락

`tests/issue_2293_chart_png_text.rs`는 `native-skia` cfg이지만 default-feature worker와 현재 Native job
모두에서 실행되지 않는 기존 결함이다. Stage 4의 신규 회귀가 아니므로 분리 이슈
[#4040](https://github.com/edwardkim/rhwp/issues/4040)을 등록했다. 이슈에 Native job target 추가,
workflow↔classifier 소유 목록 계약, classifier fixture 완료 조건을 기록했다.

### F6. collaborator self-merge review 기록

이 문서와 `mydocs/orders/20260805.md`, #3790 수행·구현·Stage 4 기록 갱신을 green code head
`5eeab15fd`의 단순 trailing review-only commit으로 올린다. 기능 보정은 단일 commit
`1f12a5fe0`으로 분리되어 별도 `pr_4032_review_impl.md`는 만들지 않는다.

## 검증

### RED 재현과 로컬 GREEN

| 검증 | 결과 |
| --- | --- |
| F1 추가 직후 Node classifier | 27건 중 신규 Rust test input 1건 실패로 RED 재현 |
| F3 추가 직후 Python workflow | aggregate script에 `wasm-build:` 포함을 검출해 RED 재현 |
| `actionlint .github/workflows/ci.yml .github/workflows/render-diff.yml` | 통과, 진단 없음 |
| `node --check scripts/ci-impact-classifier.cjs` | 통과 |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 27 passed / 0 failed |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py scripts/tests/test_render_diff_workflow.py` | 22 passed |
| `git diff --check` | 통과 |

### GitHub Actions

| head | CI | Render Diff | CodeQL | 결과 |
| --- | --- | --- | --- | --- |
| `1f12a5fe0` 기능 보정 | [30923071182](https://github.com/edwardkim/rhwp/actions/runs/30923071182) | [30923070493](https://github.com/edwardkim/rhwp/actions/runs/30923070493) | [30923070506](https://github.com/edwardkim/rhwp/actions/runs/30923070506) | full lane 전체 통과 |
| `b0be86731` 첫 devel merge | [30924641673](https://github.com/edwardkim/rhwp/actions/runs/30924641673) | [30924638772](https://github.com/edwardkim/rhwp/actions/runs/30924638772) | [30924638749](https://github.com/edwardkim/rhwp/actions/runs/30924638749) | full lane 전체 통과 |
| `5eeab15fd` 최종 code head | [31004297167](https://github.com/edwardkim/rhwp/actions/runs/31004297167) | [31004296886](https://github.com/edwardkim/rhwp/actions/runs/31004296886) | [31004296907](https://github.com/edwardkim/rhwp/actions/runs/31004296907) | full lane 전체 통과 |

첫 devel merge head `b0be86731`의 aggregate는 `fail-closed:classifier-contract`, `rust=true`,
`native=true`, `frontend=package`를 기록했다. shard별 `3698 + 693 + 840 + 1 = 5232`건이
archive expected `5232`건과 일치했다.

### 최종 code head `5eeab15fd`

이 문서 commit이 올라가는 base다. 세 workflow가 모두 `success`이고, 실패·취소된 job이 없다.

| 항목 | 값 |
| --- | --- |
| CI run 소요 | 830s (12:07:51Z → 12:21:41Z) |
| CodeQL run 소요 | 639s, `javascript-typescript` 121s / `python` 86s / `rust` 621s 모두 통과 |
| Render Diff run 소요 | 367s, Canvas visual diff 343s 통과 |
| preflight | `fast_pass=false`, `reason=no-trailing-review-only-commits`, `candidate_sha=` |
| 영향 축 | `rust_required=true`, `native_skia_required=true`, `render_required=true`, `frontend_mode=package` |
| 분류 근거 | `fail-closed:classifier-contract` |
| aggregate | `fast_pass=false`, `impact_reason=fail-closed:classifier-contract` |
| shard 합계 | `3714 + 753 + 784 + 1 = 5252`, archive expected `5252`와 일치 |
| skip된 job | Frontend unit gates (`frontend_mode=package`), WASM Build (tag 전용) |

`no-trailing-review-only-commits`는 이 head가 code를 포함한 update-branch merge이기 때문이며,
설계된 fail-closed 경로다. workflow·classifier 변경 PR이 스스로 full lane을 닫고 통과했다는 뜻이므로
Stage 4 조건화가 자기 자신을 우회하지 않았음을 확인한다.

### 이 문서 commit의 fast-pass 후보

[#4070·PR #4071](https://github.com/edwardkim/rhwp/pull/4071)이 current-base 조상 조건을 PR identity
조건으로 대체했으므로, base가 `955910136`(#4072 merge)까지 전진했다는 사실만으로는 재사용을 거부하지
않는다. `review_only_fast_pass.md` A 경로가 "base가 전진했더라도 source·test가 바뀌지 않았다면 문서
기록만을 위해 Update branch, merge, rebase를 수행하지 않는다"고 규정하므로 이 문서 commit에서는
devel을 다시 merge하지 않는다.

따라서 후보 순서는 `[이 문서 commit, 5eeab15fd]`이고, 앞의 후보는 자신의 run이 진행 중이라 pending으로
건너뛴 뒤 `5eeab15fd`의 완료된 `Build & Test` `success`를 재사용해야 한다. 기대 결과는
`fast_pass=true`, `reason=build-and-test-green:success`, `candidate_sha=5eeab15fd…`이며 heavy worker는
skipped다. 실제 결과가 다르면 fast-pass로 단정하지 않고 full CI aggregate를 기다린다.

## 시각·fixture 판단

별도 시각 증적은 적용하지 않았다. PR 고유 기능 변경은 CI workflow, classifier,
계약 테스트이며 renderer·layout·paint·pagination·golden 출력을 바꾸지 않는다. Render Diff
조건 변경의 full 경로는 세 head의 Canvas Actions 통과로 확인했다.

## 잔여 위험과 후속

- 이 PR의 full CI는 기존 검증의 무회귀와 fail-closed를 보장하지만 frontend-only skip 절감량은
  merge 뒤 canary에서 별도로 실측한다.
- #3810 직후 cache 기준선 4.73GB와 Stage 4 이후 sweep을 대조한다.
- Stage 5에서 CodeQL 언어를 분리하고 두 번째 canary로 총 절감량을 확정한다.
- #4029 cold release archive timeout과 #4040 Native target 누락은 이 PR과 분리해 추적한다.
- #3789 완료 전까지 `src/main.rs`는 render 포함 full 경계로 유지한다.
- #3790은 Stage 5–7과 post-main enforcement가 남아 이 PR merge로 close하지 않는다.

## 최종 권고

이 문서 commit을 push하면서 PR #4032를 draft에서 ready로 전환한다. 이후 review-only trailing
commit의 preflight fast-pass와 required `Build & Test` aggregate가 최신 head에서 성공하고 작업지시자가
승인하면 collaborator self-merge한다. 이 문서 commit에서는 review request와 merge를 수행하지 않는다.

merge 뒤 후속은 [post_merge.md](../../manual/pr_review/post_merge.md)를 따른다. #3790은 Stage 5–7이
남아 close하지 않고 진행 기록만 갱신하며, #4040은 이 PR과 무관하게 열어 둔다. frontend-only 절감량은
merge 뒤 최신 `devel` 기준의 별도 검증 PR에서 실행·skip job과 소요시간으로 실측한다.
