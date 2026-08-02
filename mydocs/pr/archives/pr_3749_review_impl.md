# PR #3749 리뷰 보정 구현 기록

- **PR**: [#3749](https://github.com/edwardkim/rhwp/pull/3749)
- **Issue**: [#3695](https://github.com/edwardkim/rhwp/issues/3695)
- **리뷰**: [pullrequestreview-4838218628](https://github.com/edwardkim/rhwp/pull/3749#pullrequestreview-4838218628)
- **보정 전 head**: `4df21a021`
- **최신 devel**: `3d4863a0d`

## 1. 커밋 경계

| 순서 | commit | 역할 |
| ---: | --- | --- |
| 1 | `f2b93b7ee` | 최신 devel `3d4863a0d` 무충돌 통합 |
| 2 | `fd45184f1` | auto 조 제목 confidence와 회귀 테스트 보정 |
| 3 | 이 문서를 포함한 후속 commit | review·Stage 5·manual·보고·오늘할일 기록 |

원 WIP·계획·이전 단계 commit은 rewrite하지 않는다.

## 2. 실행 순서

1. 리뷰 High 두 건을 별도 테스트로 재현하고 red 9/3을 기록한다.
2. Number 충돌 시 제목형 조만 인정하고 목차 tail·조사형 상호참조를 배제한다.
3. 실제 시장구조조사 negative와 실제 협정서 기반 positive를 green으로 고정한다.
4. top-level·recursive corpus를 이전 devel auto와 비교한다.
5. focused, full release-test, fmt, diff, clippy를 순차 검증한다.
6. review·review_impl·오늘할일을 같은 PR diff에 포함한다.
7. fork head에 push하고 PR 코멘트로 보정과 검증을 공유한다.
8. 최신 head GitHub Actions와 mergeable을 확인한 뒤 다음 승인 지점에서 중지한다.

## 3. rollback 경계

- selector 보정은 `fd45184f1` 단일 commit으로 분리되어 있다.
- 문서 기록은 별도 후속 commit으로 분리한다.
- push 전에는 두 commit 모두 로컬에서만 되돌릴 수 있으나, push 뒤에는 원 이력을 rewrite하지 않고 추가
  correction commit만 사용한다.
- #3744 범위나 explicit clause 정책을 변경하지 않으므로 해당 후속 이슈에는 rollback 영향이 없다.

## 4. 최종 조건

- 보정 head의 GitHub Actions 통과
- PR latest head와 remote fork ref 일치
- review 코멘트 실제 줄바꿈 확인
- 작업지시자 승인 전 draft 해제·approval·merge 금지
