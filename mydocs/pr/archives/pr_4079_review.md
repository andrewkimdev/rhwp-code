---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-06
---

# PR #4079 검토 — #3790 Stage 4 merge와 canary 실측 기록

## 결론

**문서 전용 merge 후보.** PR #4032 merge 사실과 canary PR #4078의 실측치를 #3790 계획·작업 문서에
반영한다. 변경이 전부 `mydocs/**`라 review-only fast-pass B 경로
(`all-review-only-no-code-impact`) 대상이며, source·test·workflow·fixture를 건드리지 않는다.

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: review_only_fast_pass.md (B 경로), post_merge.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, review_only_fast_pass.md,
                  post_merge.md, local_validation.md
base: 8d48a4c07fad6bcccebbc2adddef4685456bb313
record commit: b9219fd1f
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#4079](https://github.com/edwardkim/rhwp/pull/4079) |
| 작성자 | `postmelee` (collaborator self-merge) |
| 대상 / head | `devel` / `task_m100_3790_canary_record` (upstream branch) |
| 관련 PR | [#4032](https://github.com/edwardkim/rhwp/pull/4032) merge, [#4078](https://github.com/edwardkim/rhwp/pull/4078) canary close |
| 관련 issue | [#3790](https://github.com/edwardkim/rhwp/issues/3790) |
| metadata | label·milestone·assignee·review request 없음 |

## 변경 범위

- `mydocs/plans/task_m100_3790.md` — Stage 4 merge commit 기록, 실행 계획에 **Stage 4 canary** 항목을
  신설하고 이후 항목 번호를 재정렬, 절차 상태와 최신 동기화 기준 갱신.
- `mydocs/working/task_m100_3790_stage4.md` — review-only fast-pass 결과, devel push full lane 결과,
  canary job별 대조표와 workflow 합계표, Stage 5 관찰 추가.
- `mydocs/pr/archives/pr_4079_review.md` — 이 문서.

## 기록한 실측 근거

| workflow | #3951 runner | #4078 runner | #3951 wall | #4078 wall |
| --- | --- | --- | --- | --- |
| CI | 2,355s | 137s | 857s | 148s |
| CodeQL | 875s | 794s | 655s | 575s |
| Render Diff | 8s | 8s | 12s | 12s |
| 합계 | 3,238s | 939s | 857s | 575s |

Stage 4가 새로 생략한 9개 job의 직접 runner time은 2,279초다. CI workflow runner time −2,218초(94.2%),
세 workflow 합계 −2,299초(71.0%). Frontend unit gates의 59초→127초 증가와 CodeQL 편차는 조건화 결과가
아니므로 절감 계산에서 분리해 기록했다.

두 canary는 같은 2파일 frontend-only 변경(`rhwp-studio/src/command/shortcut-map.ts`,
`rhwp-studio/tests/shortcut-map.test.ts`)을 사용해 Stage 4만이 차이가 되도록 통제했다.

## 검증

| 검증 | 결과 |
| --- | --- |
| `git diff --check` | 통과 |
| 변경 파일 경로 | 전부 `mydocs/**` — fast-pass 허용 범위 |
| 실측 원본 대조 | Actions run 30902591087·30902590876·30902590721 (#3951), 31028405199·31028404752·31028404689 (#4078) |
| Stage 4 merge commit 확인 | `8d48a4c07`가 `upstream/devel`에 포함됨 |

Cargo·npm 게이트는 적용하지 않았다. `local_validation.md` 4.3의 "mydocs만 변경" 범위이며 PR 고유 변경이
문서뿐이다. canary의 Studio 로컬 검증(`npm test` 764 passed / 0 failed, `tsc --noEmit` 신규 오류 없음)은
#4078에서 이미 수행했고 그 결과만 인용한다.

## 시각·fixture 판단

시각 증적 없음. renderer·layout·paint·pagination·golden 출력과 무관한 문서 변경이다.

## 잔여 위험과 후속

- 이 PR은 완료된 작업의 기록이므로 merge 뒤 추가 문서 PR을 만들지 않는다.
- #3790은 cache 기준선 대조와 Stage 5–7이 남아 close하지 않는다.
- #4040 Native target 누락과 #4029 cold release archive timeout은 이 PR과 분리해 추적한다.

## 최종 권고

최신 head의 preflight fast-pass와 required `Build & Test` aggregate가 성공하면 collaborator
self-merge한다. merge 뒤에는 `post_merge.md`에 따라 devel sync와 branch·worktree 정리만 수행하고
issue comment와 오늘할일은 반복하지 않는다.
