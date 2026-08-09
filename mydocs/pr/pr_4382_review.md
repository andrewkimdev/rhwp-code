---
kind: pr_review
status: in-progress
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-10
---

# PR #4382 검토 - renderer: 캡션 높이 산식을 렌더·측정 공유 함수로 통일

## 라우팅

base route: `maintainer_general.md`. modifiers: `intake_and_review.md`, `local_validation.md`, `multi_pr_update_branch.md`, `visual_fixture_evidence.md`, `post_merge.md`.
loaded documents: `pr_review_workflow.md`, `pr_review/README.md`와 위 자식 문서.

## 접수 메타데이터

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#4382](https://github.com/edwardkim/rhwp/pull/4382) / @humdrum00001010 |
| 관련 issue | [#4320](https://github.com/edwardkim/rhwp/issues/4320) |
| base / 원 head | `devel` / `f36adcae9ce47d507148686ffcd84c677868c6aa` |
| 규모 | +123 / -65, 파일 4개, commit 2개 |
| 작성 시점 상태 | `MERGEABLE` / `CLEAN`, latest code head required checks 성공 |
| reviewer | @edwardkim 지정 완료 |
| 누적 검토 | `review/humdrum00001010-20260810`, 기준 `e48fe86947fbf9a44b1b98c7037150751af541ab` |

## 변경 범위와 검증 상태

renderer: 캡션 높이 산식을 렌더·측정 공유 함수로 통일. 원 commit은 merge commit 없이 기준 devel에서 직접 분기했다. 상세 체리픽 순서와
충돌·적용 SHA는 [#4445 통합 구현 기록](archives/pr_4445_review_impl.md)에 기록한다.

- GitHub CI: 원 head `f36adcae9ce47d507148686ffcd84c677868c6aa`에서 성공을 확인했다. merge 전 최신 상태 재확인이 필요하다.
- 로컬 focused·누적 검증: focused 회귀와 누적 release-test 5,567/5,567,
  Native Skia 58+2+4, fmt·diff·clippy, 표준 Docker WASM을 통과했다. 적용 SHA와
  충돌 해소·시각 자료는 [#4445 통합 구현 기록](archives/pr_4445_review_impl.md)에 기록했다.
- 누적 시각 판정: 작업지시자가 2026-08-10 누적 후보를 직접 확인해 통과시켰다.
- GitHub 최종 재확인: 검토 시작 뒤 contributor 추가 push가 없고 원 head가 기록값과
  일치한다. `OPEN` / `CLEAN` / `MERGEABLE`, required checks `SUCCESS`다.

## 현재 판정

**통합 PR 수용 권고.** 이 PR의 변경은 승인된 충돌 해소와 메인테이너 보정을 포함한
누적 트리에서 검증됐다. 원 PR들을 개별 merge하면 그 트리를 재현하지 못하므로 현재
누적 branch를 별도 integration PR로 게시해 최신 head CI를 받은 뒤 통합하고, 이 원
PR은 기여 내역 안내와 함께 후속 close한다.
