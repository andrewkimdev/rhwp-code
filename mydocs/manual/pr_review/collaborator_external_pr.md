---
kind: guide
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-25
---

# Collaborator 매개 외부 PR 처리

이 경로는 repository collaborator가 외부 contributor PR을 검토하고, 필요하면 PR head에 review 기록이나
보정 commit을 더해 merge를 준비할 때 적용한다. maintainer 일반 경로를 대체하지 않는다.

## 9.1 적용 조건

- PR 작성자는 외부 contributor다.
- collaborator가 review, 문서화, merge 준비를 담당한다.
- maintainer_can_modify가 true이거나 contributor가 collaborator의 source branch push를 허용했다.
- review 문서만을 위한 별도 PR보다 현재 PR head에 운영 기록을 넣는 편이 단순하다.
- GitHub review/comment, 실제 remote push, ready 전환, merge는 각각 작업지시자 승인 뒤에 수행한다.

maintainer_can_modify가 false이면 이 경로를 쓰지 않는다. maintainer 일반 경로 또는 작업지시자가 승인한
별도 PR 경로로 전환한다.

## 9.2 문서와 오늘할일

현재 contributor PR head에 다음을 포함할 수 있다.

~~~text
mydocs/pr/archives/pr_N_review.md
mydocs/pr/archives/pr_N_review_impl.md     # 필요 시
mydocs/pr/archives/pr_N_report.md          # 필요 시, 사전 판단 보고서
mydocs/orders/YYYYMMDD.md                  # 갱신이 필요한 경우
~~~

오늘할일은 최초 조사나 local 검증 중에는 만들지 않는다. contributor PR에 넣을 최종 review 묶음을 작성할 때
같은 commit으로 생성·갱신한다. report를 쓰면 merge SHA, 실제 merge 시각, issue close 완료를 미리 단정하지
않고 수용·보류 판단, merge 전 조건, merge 뒤 확인 항목만 적는다.

## 로컬 검증 뒤 문서-only fast-pass

collaborator가 contributor의 **현재 코드 head를 local에서 검증한 뒤**, review 문서와 오늘할일만 같은
source branch에 추가하는 것은 허용된 기본 최적화다. 이 경우 full CI를 다시 기다리는 대신,
[CI workflow](../../../.github/workflows/ci.yml)의 review-only fast-pass가 작동할 수 있다.

이 경로는 다음을 모두 만족해야 한다.

1. 로컬 검증은 contributor의 현재 code head를 대상으로 완료하고, review 문서에 선택한 결과를 기록한다.
2. remote push로 새로 추가하는 trailing commit은 mydocs 아래의 review 문서·오늘할일 등 **문서만** 바꾼다.
   source, test, workflow, Cargo.lock, golden, baseline, 기존 sample·PDF를 함께 바꾸지 않는다.
3. 문서 commit 직전 후보 SHA는 최신 devel과 호환되는 현재 PR code head이며, 그 SHA의 Build & Test
   check가 success, skipped, neutral 중 하나다. local Cargo 성공만으로 이 조건을 대체하지 않는다.
4. 문서 commit은 single-parent로 추가한다. 직전 후보가 docs-only Update branch merge인 경우에는
   현재 base를 parent로 하는 CI가 확인된 merge 형태만 허용한다.
5. push 뒤 최신 PR head의 preflight와 branch protection이 요구하는 check를 다시 확인한다.
   heavy job이 skipped로 보이는 것은 정상 fast-pass 결과지만 pending 또는 failing이면 merge하지 않는다.

즉, **로컬 검증 + 직전 code SHA의 녹색 CI + 문서-only trailing commit + 최신 head preflight/merge 가능 상태**
가 모두 있어야 한다. 새 code 또는 test 보정을 한 경우에는 이 fast-pass를 쓰지 않고 최신 head 기준 full CI를
기다린다.

PR의 전체 diff에 contributor code가 이미 있어도, 뒤에 추가한 commit만 mydocs라면 workflow가 직전 code
candidate의 Build & Test를 재사용한다. 반대로 문서-only 후속 PR이라도 check 조회 실패, missing check,
failed check, 허용되지 않은 merge commit이면 보수적으로 full CI 경로다.

## 9.3 PR head push

contributor 원 commit을 rewrite하지 않는다. review 문서·오늘할일·보정 code는 별도 commit으로 나누고,
보정이 있으면 review 문서에 contributor 원 변경과 collaborator 추가 변경을 구분한다.

~~~bash
git fetch upstream pull/N/head:local/prN
git switch local/prN
# local 검증과 archive review·오늘할일 작성
git commit -m "docs: PR #N 검토 기록"
git push https://github.com/<contributor>/rhwp.git HEAD:<head-branch>
~~~

push 뒤 PR head SHA가 local HEAD와 같은지 확인하고, 위 fast-pass 또는 full CI 결과가 merge 가능한 상태인지
확인한다.

### 9.3.1 contributor PR head 직접 보정

차단 결함을 collaborator가 고치기로 하면 별도 통합 branch로 옮기지 않고 maintainer_can_modify가 true인
**현재 contributor PR head 위에만** 추가 commit을 만든다.

- push 직전 PR head SHA, git ls-remote SHA, fetch한 local branch SHA가 모두 같아야 한다.
- contributor commit은 rebase, amend, reset, force-push하지 않는다.
- code·regression test 보정과 review·오늘할일은 별도 commit으로 만든다.
- LFS object가 있으면 정상 LFS lock·upload 권한을 사용하며 GIT_LFS_SKIP_PUSH를 쓰지 않는다.
- LFS object가 전혀 없을 때만 dry-run과 실제 push에서 GIT_LFS_SKIP_PUSH로 ref write를 확인할 수 있다.
  core.hooksPath를 무력화해 전체 pre-push hook을 우회하지 않는다.

~~~bash
gh pr view N --repo edwardkim/rhwp \
  --json headRefName,headRefOid,headRepository,maintainerCanModify
git ls-remote --heads https://github.com/<contributor>/rhwp.git refs/heads/<head-branch>
git fetch https://github.com/<contributor>/rhwp.git \
  refs/heads/<head-branch>:refs/heads/review/prN-maintainer
git push --dry-run https://github.com/<contributor>/rhwp.git HEAD:<head-branch>
~~~

승인 뒤에도 마지막 remote SHA가 보정 시작 SHA와 같을 때만 push한다. code 또는 test commit이 하나라도
포함되면 문서-only fast-pass를 적용하지 않고 최신 head full CI를 기다린다.

## 9.4 merge 전 조건

- 최신 head의 full CI 또는 위 문서-only fast-pass가 branch protection을 만족한다.
- 필요한 review 문서와 오늘할일이 PR diff에 있다.
- report는 사전 판단 형식이다.
- contributor에게 review 또는 PR comment로 결과를 남긴다. 단, 이미 완료된 원 PR의 기록만 담는 별도
  fast-pass PR은 추가 contributor comment 대상이 아니다.
- 최신 mergeable 상태와 작업지시자 승인을 확인한다.

원 코드 PR을 merge한 뒤에는 [merge 후속 처리](post_merge.md)를 적용한다. 이미 완료된 원 PR의
review·asset·오늘할일만 반영한 별도 fast-pass PR은 issue close/comment와 오늘할일 생성을 반복하지 않되,
devel sync와 branch/worktree/target 정리는 수행한다.
