---
kind: pr-review-implementation-plan
status: code-ci-success-docs-tail-pending
pr: 3685
issue: 3676
last_verified: 2026-08-01
---

# PR #3685 메인터너 보정·반영 기록

## 기준점과 commit 경계

| 구분 | SHA | 역할 |
| --- | --- | --- |
| contributor 원 source head | `2f81e6733` | HWP3→HWP5 한글 열기 문제의 세 저장 계약과 Windows 검사 도구 |
| prior maintainer remote head | `de75f2d5d` | Windows 전용 도구를 Linux CI 의무 gate로 만들지 않는 현재 PR 기준점 |
| 공개 저장 경로 회귀 | `7cbaee46c` | `DocumentCore` public HWP3 export도 세 PBF record를 쓰는 byte contract 고정 |
| HWPX overlay 보정 | `9204055a2` | HWP 저장에는 세 PBF를 쓰되 HWPX live IR의 단일 BOTH overlay 복원 |
| final code candidate `C` | `1aa0aadbe` | 평문·비밀번호 HWP export의 adapter 위임을 명시적으로 유지 |

공통 adapter가 HWPX와 HWP3 모두의 HWP 저장 경로라는 점은 실물 Windows 한글 오라클로 다시
확인했다. HWPX PBF를 HWP3에만 materialize하던 중간 보정은 내부 검증은 통과했어도 실제 한글이
HWPX→HWP 산출물을 거부했으므로 final candidate에 포함하지 않았다. contributor commit은 rebase,
amend, force-push하지 않았으며, `C`를 기존 contributor fork head 위에 fast-forward했다.

## 완료한 검증과 반영

1. 별도 review worktree에서 `C`를 확정하고 unrelated #3486 변경을 candidate에서 제외했다.
2. focused PBF/public export regression 5/5, passthrough invalidation static contract 5/5,
   full `cargo test --profile release-test --tests` exit 0, Clippy exit 0을 전용 target에서 확인했다.
3. `win10-ted` 전용 checkout에서 `pyhwpx 1.7.2`를 설치·import하고 release-test binary를 만들었다.
   HWP3 16쪽과 HWPX PBF 1쪽 모두 `--verify --verify-pages`와 독립 `Hwp(new=True)` 실제 열기를
   통과했다. 기존 사용자 한글 프로세스에는 attach·종료·전역 `taskkill`을 하지 않았다.
4. code/test 3개 파일의 LFS 속성을 먼저 판독해 비-LFS임을 확인한 뒤,
   `GIT_LFS_SKIP_PUSH=1` dry-run과 일반 fast-forward push로 `C`를
   `planet6897/rhwp:fix/3676-hwp3-convert-hancom-openable`에 반영했다.
5. [CI run 30701824447](https://github.com/edwardkim/rhwp/actions/runs/30701824447)의
   required `Build & Test`와 [CodeQL run 30701824446](https://github.com/edwardkim/rhwp/actions/runs/30701824446)가
   모두 success임을 확인했다.

## 남은 반영 순서

1. 이 review, 이 implementation record, `mydocs/orders/20260801.md`만 담은 single-parent
   Markdown commit을 `C` 위에 만든다.
2. push 직전 PR head·fork ref가 계속 `C`인지, 문서 경로가 LFS 대상이 아닌지 다시 확인한다.
   비-LFS일 때만 `GIT_LFS_SKIP_PUSH=1` dry-run 후 같은 contributor branch에 push한다.
3. 이 문서-only tail은 review-only fast-pass **A**다. `C`가 current base의 ancestor이고 `C`의
   full CI가 green임을 유지하면서, 새 head의 preflight와 required `Build & Test` aggregate가
   success인지 확인한다. aggregate pending/fail, fast-pass 거부, base drift가 생기면 full CI로
   되돌린다.
4. 최신 docs head가 `CLEAN`/`MERGEABLE`이고 모든 required check가 녹색이면, 실제 LF body file을
   사용해 approve review를 게시하고 API body에 literal `\\n`이 없는지 확인한다. 그 뒤 merge한다.
5. merge 후에는 #3676 auto-close 상태를 확인해 필요하면 close와 maintainer comment를 남기고,
   merge SHA·검증·fast-pass 결과를 담은 contributor comment를 LF body file로 게시한다. 그 다음
   정확한 review worktree/전용 target만 정리하며 contributor fork remote branch는 삭제하지 않는다.

## 롤백 경계

- merge 전 결함은 contributor history를 rewrite하지 않는다. 현재 head 위에 정정 또는 revert commit을
  추가하고 full CI와 Windows 외부 오라클을 다시 실행한다.
- PBF 경계 결함은 `1aa0aadbe` → `9204055a2` → `7cbaee46c` 순으로 검토한다. HWP 출력의 세 record와
  HWPX live IR restore는 함께 유지해야 한다.
- 문서 tail 또는 fast-pass만 실패하면 code candidate `C`는 보존하고 문서 전용 follow-up으로 해결한다.
