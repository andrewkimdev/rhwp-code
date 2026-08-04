---
kind: review
status: active
pr: 3823
---

# PR #3823 검토: Stage 2.5 trusted-base shadow 경계

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, multi_pr_update_branch.md
current remote head: 1e45c2ec2b0da7c2dce8ea22bfc49922af62a4d5
                     (보정 push 전 작성 시점 참고값)
local correction commit: 944d6fafe
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#3823](https://github.com/edwardkim/rhwp/pull/3823) |
| 작성자 | `postmelee` (collaborator self-merge) |
| 대상 / head | `devel` / `codex/issue-3790-shadow-observation` |
| 작성 시점 원격 상태 | draft, `MERGEABLE`, `BEHIND`; 최신 head push 뒤 재확인 필요 |
| 보정 전 원격 head | `1e45c2ec2b0da7c2dce8ea22bfc49922af62a4d5` |
| 최신 동기화 기준 | `upstream/devel` `2971a1d9a6ca1edbfe96d11c0fff29e09666cf4e` |
| devel merge commit | `7a191121328cd5b1418c73e7f321024fb3f39f09` |
| review correction | `944d6fafe` |
| 관련 issue | [#3790](https://github.com/edwardkim/rhwp/issues/3790) (open) |
| metadata | label `ci`, milestone `v1.0.0`, assignee `postmelee`, review request 없음 |

draft, mergeability, head SHA와 CI는 변할 수 있는 작성 시점 참고값이다. 최종 merge 조건은 최신 PR head의
GitHub Actions 통과와 작업지시자 승인이다. 사용자 지시에 따라 reviewer request는 보내지 않았고,
ready 전환과 merge도 이 문서 commit에서는 수행하지 않는다.

## 변경 범위와 목적

이 PR은 #3792의 Stage 1 shadow classifier를 실제 skip에 연결하지 않은 채, PR 판정에 사용하는 classifier
파일을 PR head가 아니라 `pull_request.base.sha`에서 sparse checkout하는 Stage 2.5다. 기존
`frontend_required`, worker `if`, Render Diff trigger와 CodeQL matrix는 그대로 유지한다.

Stage 2 실측은 최근 종료 PR 60건과 live run을 대조해 frontend-only 과잉 실행의 비용과 fail-closed
동작을 확인했다. 다만 trusted-base `classified` 운영 표본이 충분하지 않고 workflow 제어면 전체의 신뢰
경계도 아직 완성되지 않았으므로 이 PR은 worker skip을 활성화하지 않는다.

## 리뷰 지적 대응

[리뷰 코멘트](https://github.com/edwardkim/rhwp/pull/3823#issuecomment-5174364455)의 F1–F3를 다음처럼
반영했다.

### F1 — classifier 파일 부재 시 authority 강등

`actions/checkout`의 sparse checkout은 요청한 경로가 base SHA에 없어도 성공할 수 있다. checkout step의
`outcome`만 보던 기존 구현은 classifier 파일이 없는 오래된 base에서도
`pr-base-trusted-shadow`를 주장할 수 있었다.

보정 commit `944d6fafe`는 workspace의 `scripts/ci-impact-classifier.cjs` 존재를 `fs.existsSync`로 함께
확인한다. checkout이 실패하거나 파일이 없으면 authority를 `unavailable-advisory`로 낮추고,
classifier output은 기존 fail-closed 기본값인 모든 영향축 `full`을 유지한다. workflow 계약 테스트도
checkout 성공과 파일 존재가 함께 필요함을 고정했다.

### F2 — trusted 범위와 Stage 2.6 명시

Stage 2.5가 base SHA에 고정하는 것은 classifier 파일 하나뿐이다. `pull_request` workflow YAML, 인라인
collect script, classifier 호출부와 미래 worker `if`는 PR merge ref의 제어를 받는다. 계획서와 Stage 2
기록에서 `pr-base-trusted-shadow`를 classifier-source provenance로 한정했다.

실제 skip 전에는 base/default branch가 제어하는 controller 또는 required policy check를 확정하거나,
이를 적용하지 못하는 fork·untrusted head PR을 full로 유지하는 **Stage 2.6 trusted enforcement
boundary**를 추가했다. Stage 3는 Stage 2.5 표본과 Stage 2.6 실행 경계를 모두 충족하기 전에는 시작하지
않는다.

### F3 — 절차 문서 정합

- 이 review 문서를 `mydocs/pr/archives/pr_3823_review.md`에 추가했다.
- 2026-08-04 오늘할일의 공통 운영 작업에 #3790/#3823 진행 상태를 기록했다.
- Stage 2 최초 기준 SHA를 12자 `91f5131815dc`로 통일했다.

보정 범위가 단일 authority guard, 계약 테스트와 계획 정합화로 명확해 별도
`pr_3823_review_impl.md`는 만들지 않았다.

## #3892 반영 판정

[#3892](https://github.com/edwardkim/rhwp/pull/3892)는 default-feature CI를 세 builder와
`slow/1/2/3` 네 archive·네 worker로 재배치했다. full CI wall time은 개선하지만 frontend-only 변경에서
Rust worker를 실행하는 영향축 문제는 남으므로 #3790의 전략과 개선 가치는 유지된다.

후속 계획은 다음처럼 조정했다.

- Stage 4는 `build-test-archive-slow`, `build-test-archive-a`, `build-test-archive-b`와
  `test-slow-shard`, `test-regular-shard-1/2/3`을 함께 조건화한다.
- `Build & Test` aggregate는 세 builder·네 worker 각각의 `success|skipped` 진리표를 확인한다.
- Stage 6 artifact retry는 논리 label `slow/1/2/3`별 test archive, archive expected count와 worker run
  count를 함께 다룬다.
- Stage 2의 8-shard runner-minute는 historical 수치로 표시하고, 절감량은 #3892 topology에서 다시 잰다.
- cache는 #3810 정리 직후 4.73GB와 임의 시점 총량을 비교하지 않고 Stage 4 이후 다음 sweep 직후
  snapshot을 대조한다.

## 검증

### 로컬

최신 `upstream/devel` `2971a1d9a6ca`를 conflict 없이 merge한 뒤 correction commit `944d6fafe`에서
다음을 실행해 통과했다.

| 검증 | 결과 |
| --- | --- |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 20 passed / 0 failed |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py scripts/tests/test_render_diff_workflow.py` | 7 passed |
| `actionlint .github/workflows/ci.yml .github/workflows/build-nextest-archives.yml .github/workflows/run-nextest-archives.yml` | 통과, 진단 없음 |
| `git diff --check` | 통과 |

Rust·Studio 소스와 renderer 동작은 이 PR 고유 diff에서 바뀌지 않는다. 로컬 Cargo·Studio·시각 검증은
`local_validation.md` 4.3의 CI workflow 범위 밖이므로 생략하고, 최신 head GitHub Actions에서 실제
재사용 workflow와 전체 worker dependency를 확인한다.

### GitHub Actions

보정 전 원격 head `1e45c2ec2`의 CI와 CodeQL은 모두 통과했다. 이 결과는 이전 1-builder·8-worker
topology에서 얻은 역사 근거이며 새 head의 최종 merge 근거로 재사용하지 않는다.

이번 push는 `.github/workflows/ci.yml`을 포함하므로 classifier가 full로 닫고 #3892 이후의 lint,
frontend package gate, Native Skia, 세 builder, 네 worker, `Build & Test` aggregate와 CodeQL을 최신
head에서 다시 실행해야 한다. 그 CI 결과는 push 뒤 재확인한다.

### 시각·fixture 판정

시각 증적은 적용하지 않았다. PR 고유 변경은 CI workflow, Node classifier 계약 테스트와 계획·측정
문서이며 renderer, layout, paint, pagination, sample, golden과 기준 PDF를 바꾸지 않는다.

## 위험과 후속 조건

- `pr-base-trusted-shadow`는 worker skip 권한이 아니다. Stage 2.6 없이 Stage 3 조건에 직접 연결하면 안 된다.
- base에 classifier 파일이 없는 PR은 `unavailable-advisory`와 full 기본값으로 남아야 한다.
- #3789가 완료되기 전에는 `src/main.rs`를 render 포함 full 경계로 유지한다.
- #3790은 Stage 2.5 뒤에도 Stage 2.6–7이 남으므로 이 PR merge로 close하지 않는다.
- 최신 head의 CI가 통과해도 ready 전환·merge는 사용자가 직접 수행한다.

**현재 권고: 최신 head CI 대기.** F1–F3와 #3892 이후 계획 정합화는 로컬 검증을 통과했다. 이 review
문서와 오늘할일을 push한 뒤 최신 head의 CI·CodeQL이 모두 성공하면 merge 후보로 판단할 수 있다.
