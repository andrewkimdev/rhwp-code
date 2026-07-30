---
kind: pr_review_plan
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-31
---

# PR #3597 및 kevin9327 적층 PR 처리 계획

## 입력과 commit 경계

| 구분 | SHA / 범위 | 처리 원칙 |
| --- | --- | --- |
| #3597 contributor 원 변경 | `c930cac3c` → 통합 `f1b5c0012` | rewrite·amend·force-push 금지 |
| #3599 contributor 원 변경 | `52462c9c2`, `dc078646f` | #3598 기능·증적을 순서대로 반영 |
| #3602 contributor 원 변경 | `c01b3956b`, `c61447fb2` | #3599 후속 기능·증적을 순서대로 반영 |
| #3607 contributor 원 변경 | `9a1d0f2d2` | #3597 후속 기능을 반영 |
| #3610 contributor 원 변경 | `19e16734f` | 독립 문서 변경을 마지막에 반영 |
| collaborator 보정 | `395039cf2` | #3597 계약 테스트 rustfmt만 별도 commit |
| review 묶음 | 원 PR별 review·오늘할일 | code candidate에 함께 기록 |

적층 관계는 `#3597 → #3607`, `#3599 → #3602`이며, #3610은 독립 문서 PR이다. 최신 `devel`의
`integrate/kevin9327-20260731`에 원 contributor commit만 누적했다. 원 PR source branch의 base update와
CI는 이미 시작된 실행을 보존할 뿐, 이 통합의 merge 판단이나 개별 merge의 근거로 사용하지 않는다.

## 실행 순서

1. 통합 branch의 source/local/remote SHA를 대조하고, `upstream/devel`부터 통합 HEAD까지의 변경 파일을
   `git check-attr filter` 및 `git lfs status`로 먼저 판독한다. LFS 대상이 없으면
   `GIT_LFS_SKIP_PUSH=1` dry-run과 실제 push를 사용한다.
2. `integrate/kevin9327-20260731`을 `devel` 대상 단일 PR로 연다. PR 본문에는 다섯 원 PR·원 contributor
   SHA·test-only style 보정 및 검증 결과를 명시해 저자성을 보존한다.
3. code·test가 포함됐으므로 fast-pass를 쓰지 않는다. 통합 PR 최신 head의 full CI, 정확한 head SHA,
   `MERGEABLE` 상태를 확인한 뒤 review·admin merge한다.
4. merge 뒤 #3596, #3598, #3601, #3600의 자동 close 상태를 재조회하고, 통합 PR merge SHA를 근거로
   필요한 issue comment를 게시한다.
5. 원 PR #3597, #3599, #3602, #3607, #3610에는 실제 LF를 가진 감사·검토 결과 comment와 통합 PR 링크를
   남긴 뒤 supersede close한다. contributor fork branch는 삭제하지 않는다.
6. 마지막으로 local `devel`을 동기화하고, 통합 local/remote branch와 전용 Cargo target을 후속 절차에 따라
   정리한다.

## 중단·rollback 기준

- 원 contributor SHA가 새로 변하면 통합 범위를 재검토한다. 이미 시작된 원 PR CI는 취소하지 않지만, 통합 PR
  CI와 혼동하지 않는다.
- 최신 통합 full CI 실패, aggregate pending/failure, mergeability 변화, LFS object/lock 필요이면 통합 PR에서
  멈추고 원인을 분리한다. contributor history를 rewrite하거나 원 PR을 개별 merge하지 않는다.
- collaborator 보정에 문제가 발견되면 `395039cf2`만 별도 revert할 수 있어 contributor 원 변경이 독립적으로
  보존된다.
- 통합 PR의 post-merge 확인이 끝난 뒤에만 전용 Cargo target과 통합 branch를 정리한다.
