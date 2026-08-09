---
kind: pr_review
status: in-progress
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-10
---

# PR #4426 검토 - core: 클립보드 HTML의 중첩 표·이미지 소실과 import 패닉 수정

## 라우팅

base route: `maintainer_general.md`. modifiers: `intake_and_review.md`, `local_validation.md`, `multi_pr_update_branch.md`, `post_merge.md`.
loaded documents: `pr_review_workflow.md`, `pr_review/README.md`와 위 자식 문서.

## 접수 메타데이터

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#4426](https://github.com/edwardkim/rhwp/pull/4426) / @humdrum00001010 |
| 관련 issue | [#4413](https://github.com/edwardkim/rhwp/issues/4413) |
| base / 원 head | `devel` / `772f89c968b8c1c67372c81c67810617da7cc376` |
| 규모 | +616 / -17, 파일 3개, commit 4개 |
| 작성 시점 상태 | `MERGEABLE` / `CLEAN`, latest code head required checks 성공 |
| reviewer | @edwardkim 지정 완료 |
| 누적 검토 | `review/humdrum00001010-20260810`, 기준 `e48fe86947fbf9a44b1b98c7037150751af541ab` |

## 변경 범위와 검증 상태

core: 클립보드 HTML의 중첩 표·이미지 소실과 import 패닉 수정. 원 commit은 merge commit 없이 기준 devel에서 직접 분기했다. 상세 체리픽 순서와
충돌·적용 SHA는 [누적 구현 계획](pr_4313_review_impl.md)에 기록한다.

- GitHub CI: 원 head `772f89c968b8c1c67372c81c67810617da7cc376`에서 성공을 확인했다. merge 전 최신 상태 재확인이 필요하다.
- 로컬 focused·누적 검증: focused 회귀와 누적 release-test 5,567/5,567,
  Native Skia 58+2+4, fmt·diff·clippy, 표준 Docker WASM을 통과했다. 적용 SHA와
  충돌 해소·시각 자료는 [누적 구현 계획](pr_4313_review_impl.md)에 기록했다.
- 시각 검증: 현재 변경 성격상 필수 대상이 아닌 것으로 접수했으며 diff 검토 뒤 재판정한다.

## 현재 판정

**누적 검증 통과, 최종 판정 대기.** 작업지시자 시각 판정과 merge 직전 GitHub 최신
head·mergeable·required checks 재확인 뒤 최종 권고를 갱신한다.
