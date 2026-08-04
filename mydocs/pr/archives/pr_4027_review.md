# PR #4027 검토

## 결론

**수용 후보.** 각주 편집 모드가 전용 키 처리 뒤 무조건 반환하던 탓에 `Cmd/Ctrl+Z`와
`Option+G`가 공통 단축키 라우팅에 도달하지 못했다. 서브모드에서 안전한 세 명령만
dispatcher로 전달해 각주 입력·이동·삭제·Escape 동작은 그대로 유지한다.

최종 병합 조건은 review 문서 push 뒤 최신 PR head의 GitHub Actions 통과와 작업지시자 승인이다.

## 접수 및 기준

| 항목 | 내용 |
| --- | --- |
| PR | [#4027](https://github.com/edwardkim/rhwp/pull/4027) `fix(studio): 각주 편집 전역 단축키 복구` |
| 작성자 | `jangster77` |
| 대상 | `devel` |
| 검토한 구현 head | `2d236c3f85cc49a48fa89c25d3438cb2dee0d781` |
| 구현 기준 devel | `4473112d9` |
| 관련 이슈 | [#4026](https://github.com/edwardkim/rhwp/issues/4026) |
| 구현 변경 규모 | 4 files, +155 / -0 |
| 작성 시점 mergeable | `MERGEABLE` |
| 작성 시점 merge 상태 | `BLOCKED` (GitHub Actions 진행 중) |

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md
```

## 변경 내용

- `input-handler-keyboard.ts`에 서브모드 전역 단축키 게이트를 추가했다.
- 게이트는 `edit:undo`, `edit:redo`, `edit:goto`만 통과시킨다. 서식·클립보드·파일 명령처럼
  각주 전용 편집 모델과 별도 검토가 필요한 명령은 통과시키지 않는다.
- 머리말/꼬리말과 각주 전용 분기 시작점에 같은 게이트를 적용했다.
- `footnote-01.hwp`를 실제로 열어 각주 입력 뒤 macOS `Cmd+Z`와 `Option+G`를 검증하는 E2E를
  추가했다.
- 새 #4026 E2E와 최신 `devel`에 누락돼 있던 #3953 찾아가기 E2E를 MANIFEST 단일 권위 표에
  등록했다.

## 로컬 검증

아래 검증은 구현 head `2d236c3f8`에서 완료됐다.

| 검증 | 결과 |
| --- | --- |
| `git diff --check` | 통과 |
| `cd rhwp-studio && npx tsc --noEmit` | 통과 |
| `cd rhwp-studio && npm test` | 763/763 통과 |
| `VITE_URL=http://127.0.0.1:7700 node e2e/issue-4026-footnote-global-shortcuts.test.mjs --mode=headless` | 실제 `footnote-01.hwp`에서 각주 입력 뒤 `Cmd+Z` 커서 `1 -> 0`, 각주 모드 유지, `Option+G` 대화상자 표시 통과 |
| `cd rhwp-studio && npm run e2e:manifest-check` | 83개 파일 / 83개 행, 이상 없음 |

## 시각·fixture 판단

이 PR은 Canvas/render, 레이아웃, pagination, HWP/HWPX fixture를 변경하지 않는다. 키보드
이벤트 라우팅과 E2E 목록만 변경하므로 기준 PDF나 visual sweep은 적용하지 않았다. 대신 실제
브라우저 E2E가 각주 컨텍스트에서 명령 실행과 대화상자 표시를 검증한다.

## 범위 밖 변경 및 잔여 위험

- 각주 텍스트 입력, 방향키, Enter, Backspace/Delete, Escape의 기존 전용 분기는 변경하지 않았다.
- `Cmd/Ctrl+Z`, redo, `Option+G` 외의 전역 단축키는 서브모드에서 계속 기존 동작을 유지한다.
- #3953 MANIFEST 행은 최신 `devel`에 이미 존재하던 E2E의 목록 누락만 복구하며 runtime 동작을
  바꾸지 않는다.

## 최종 권고

review 문서와 오늘 기록을 추가한 최신 PR head의 required GitHub Actions가 통과하고 작업지시자가
승인하면 PR #4027을 병합한다. 병합 뒤 #4026 자동 close 상태와 원격 branch 정리를 확인한다.
