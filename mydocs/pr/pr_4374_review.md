---
kind: pr_review
status: in-progress
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-10
---

# PR #4374 검토 - renderer: PartialTable 조각의 행 좌표계를 데이터로 고정하고 되추론 제거

## 라우팅

base route: `maintainer_general.md`. modifiers: `intake_and_review.md`, `local_validation.md`, `multi_pr_update_branch.md`, `visual_fixture_evidence.md`, `post_merge.md`.
loaded documents: `pr_review_workflow.md`, `pr_review/README.md`와 위 자식 문서.

## 접수 메타데이터

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#4374](https://github.com/edwardkim/rhwp/pull/4374) / @humdrum00001010 |
| 관련 issue | [#4326](https://github.com/edwardkim/rhwp/issues/4326) |
| base / 원 head | `devel` / `1525cf831f5f73d79dce8a5e1bc04edf17d668aa` |
| 규모 | +329 / -35, 파일 10개, commit 2개 |
| 작성 시점 상태 | `MERGEABLE` / `CLEAN`, latest code head required checks 성공 |
| reviewer | @edwardkim 지정 완료 |
| 누적 검토 | `review/humdrum00001010-20260810`, 기준 `e48fe86947fbf9a44b1b98c7037150751af541ab` |

## 변경 범위와 검증 상태

renderer: PartialTable 조각의 행 좌표계를 데이터로 고정하고 되추론 제거. 원 commit은 merge commit 없이 기준 devel에서 직접 분기했다. 상세 체리픽 순서와
충돌·적용 SHA는 [누적 구현 계획](pr_4313_review_impl.md)에 기록한다.

- GitHub CI: 원 head `1525cf831f5f73d79dce8a5e1bc04edf17d668aa`에서 성공을 확인했다. merge 전 최신 상태 재확인이 필요하다.
- 로컬 focused·누적 검증: 진행 전.
- 시각 검증: 사용자-visible layout/render 영향이 있어 누적 후보에서 수행해야 한다.

## 현재 판정

**보류.** 누적 체리픽, PR 고유 diff 검토와 로컬 검증이 끝난 뒤 최종 권고를 갱신한다.
