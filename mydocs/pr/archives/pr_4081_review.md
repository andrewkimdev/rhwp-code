---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-06
---

# PR #4081 검토 — #3790 cache 기준선 대조 결과

## 결론

**문서 전용 merge 후보.** #3790 실행 계획 9번 "cache 회귀 확인"을 수행하고 결과를 계획·구현·작업
문서에 확정한다. 변경이 전부 `mydocs/**`라 review-only fast-pass B 경로
(`all-review-only-no-code-impact`) 대상이다.

판정은 **기준선 회귀 있음, 단 Stage 4 무관**이다. 회귀 대응은 [#4080](https://github.com/edwardkim/rhwp/issues/4080)으로
분리했고 Stage 5의 게이트에서 4.73GB를 제거했다.

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: review_only_fast_pass.md (B 경로), post_merge.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, review_only_fast_pass.md,
                  post_merge.md, local_validation.md
base: 202c18e91f3f8384902d6aba2076c54d0be7bc76
record commit: a7d805b79
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#4081](https://github.com/edwardkim/rhwp/pull/4081) |
| 작성자 | `postmelee` (collaborator self-merge) |
| 대상 / head | `devel` / `task_m100_3790_cache_record` (upstream branch) |
| 관련 issue | [#3790](https://github.com/edwardkim/rhwp/issues/3790), [#4080](https://github.com/edwardkim/rhwp/issues/4080), [#3684](https://github.com/edwardkim/rhwp/issues/3684) |
| metadata | label·milestone·assignee·review request 없음 |

## 변경 범위

- `mydocs/plans/task_m100_3790.md` — 9번 항목을 수행 결과로 확정하고, 10번 Stage 5의 판단 기준에서
  4.73GB 게이트를 제거한다.
- `mydocs/plans/task_m100_3790_impl.md` — 구현 항목 6에 수행 결과를 붙이고 "Stage 5 이후"의 cache
  기준을 #4080으로 넘긴다.
- `mydocs/working/task_m100_3790_stage4.md` — 스윕 시계열표와 원인 분석 절을 추가하고 완료된 항목을
  다음 단계에서 뺀다.
- `mydocs/pr/archives/pr_4081_review.md` — 이 문서.

## 기록한 대조 결과

| 시점 (UTC) | 정리 전 | 정리 대상 | 정리 후 |
| --- | --- | --- | --- |
| #3810 기준선 08-02 14:43 수동 | 42개 / 10.13GB | 18개 / 5.40GB | 24개 / 4.73GB |
| 08-02 19:39 cron | 31개 / 5.91GB | 1개 / 0.06GB | 30개 / 5.85GB |
| 08-03 20:03 cron | 50개 / 10.24GB | 10개 / 3.08GB | 40개 / 7.16GB |
| 08-04 20:03 cron | 50개 / 8.64GB | 1개 / 0.18GB | 49개 / 8.46GB |
| 08-05 17:28 dry-run | 53개 / 10.01GB | 3개 / 1.17GB | 50개 / 8.84GB |

기준선 대비 +4.11GB(+87%), 무료 한도 10GB의 88%다.

Stage 4 귀속을 기각한 근거는 세 가지다. ① Stage 4 merge는 08-05 16:42 UTC인데 회귀 추세는 그 전에
완성됐다. ② Stage 4는 frontend-only PR에서 Rust lane job을 생략하므로 캐시 생성을 오히려 줄이며,
canary #4078에서 해당 job이 실행되지 않았음을 확인했다. ③ 세대 상한이 정상 동작 중이다 —
(그룹, ref) 쌍 42개 중 2세대 초과는 3개뿐이고 전부 마지막 스윕 이후 생성분이다.

실제 원인은 쌍 수 증가로 KEEP=2의 하한이 올라간 것(`refs/heads/devel` 한 ref만 19개 5.84GB로 기준선
전체 초과)과 삭제된 브랜치의 고아 캐시가 (그룹, ref)별 최신 2세대 규칙 때문에 영구히 정리 대상이
아닌 것(약 0.53GB)이다.

## 검증

| 검증 | 결과 |
| --- | --- |
| `git diff --check` | 통과 |
| 변경 파일 경로 | 전부 `mydocs/**` — fast-pass 허용 범위 |
| 스윕 시계열 원본 | run 30763873837·30848508775·30946087506 로그, #3684 보고서 |
| Stage 4 이후 값 | dry-run run [31030157435](https://github.com/edwardkim/rhwp/actions/runs/31030157435), `dry_run=true`로 삭제 없음 |
| 캐시 인벤토리 | `GET /repos/edwardkim/rhwp/actions/caches` 페이지네이션 전량, 53개 10.01GB |

Cargo·npm 게이트는 적용하지 않았다. `local_validation.md` 4.3의 "mydocs만 변경" 범위다.

## 시각·fixture 판단

시각 증적 없음. renderer·layout·paint·pagination·golden 출력과 무관한 문서 변경이다.

## 잔여 위험과 후속

- 실제 cron 스윕이 dry-run 예상치 8.84GB를 확정하는지는 다음 스케줄 실행에서 확인한다. 예상치는
  `before - staleBytes` 계산값이므로 동시 생성분에 따라 소폭 달라질 수 있다.
- 캐시 총량이 한도의 88%라 #4080은 조기 처리 대상이다.
- #3790은 Stage 5–7과 post-main enforcement가 남아 close하지 않는다.

## 최종 권고

최신 head의 preflight fast-pass와 required `Build & Test` aggregate가 성공하면 collaborator
self-merge한다. merge 뒤에는 `post_merge.md`에 따라 devel sync와 branch·worktree 정리만 수행하고
issue comment와 오늘할일은 반복하지 않는다.
