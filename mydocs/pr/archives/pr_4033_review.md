# PR #4033 검토

## 결론

**수용 후보.** 각주 편집 중 `Option+G`로 대형 문서의 대상 쪽을 찾을 때 본문 위치와 viewport는
이동했지만, 각주 전용 caret·상태 갱신이 최종 결과를 덮고 있었다. 페이지 화면 이동이 성공한 뒤에만
각주 컨텍스트를 종료하고 본문 cursor를 배치해, 실패 시의 각주 편집 상태는 유지하면서 정상 이동을
복구했다.

최종 병합 조건은 review 문서와 오늘 기록을 포함한 최신 PR head의 GitHub Actions 통과와 작업지시자
승인이다.

## 접수 및 기준

| 항목 | 내용 |
| --- | --- |
| PR | [#4033](https://github.com/edwardkim/rhwp/pull/4033) `fix(studio): 각주에서 찾아가기 본문 전환 (#4030)` |
| 작성자 | `jangster77` |
| 대상 | `devel` |
| 검토한 구현 head | `6388d2f0051978c92c6c44b6223098f8ab8e8f58` |
| 구현 기준 devel | `9aa0ec8b6` |
| 관련 이슈 | [#4030](https://github.com/edwardkim/rhwp/issues/4030) |
| 구현 변경 규모 | 5 files, +193 / -0 |
| 문서 작성 시점 mergeable | `MERGEABLE` |
| 문서 작성 시점 merge 상태 | `BLOCKED` (GitHub Actions 진행 중) |

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md
```

## 변경 내용

- `InputHandler`에 본문 탐색용 각주 모드 종료 API를 추가했다. 문서 mutation·history·dirty 상태는
  건드리지 않고 기존 `footnoteModeChanged=false` UI 계약만 발행한다.
- 페이지 찾아가기는 화면 이동이 성공한 뒤, `moveCursorTo()`의 본문 caret 배치 전에 위 API를 호출한다.
  화면 이동 실패 시에는 기존 각주 편집 상태를 보존한다.
- 실제 219쪽 HWP의 각주 1번에서 `Option+G → 200`을 수행하는 #4030 browser E2E와 E2E MANIFEST
  행을 추가했다.

## 로컬 검증

아래 검증은 구현 head `6388d2f00`에서 완료됐다.

| 검증 | 결과 |
| --- | --- |
| `git diff --check` | 통과 |
| `cd rhwp-studio && npx tsc --noEmit` | 통과 |
| `cd rhwp-studio && npm run e2e:manifest-check` | 84개 파일 / 84개 행, 이상 없음 |
| `VITE_URL=http://127.0.0.1:7700 node e2e/issue-4030-footnote-goto-transition.test.mjs --mode=headless` | 실제 219쪽 HWP의 각주 1번에서 200쪽 이동, 각주 모드 종료, `200 / 219 쪽`, viewport `225242 / 225377.5`, 문단 2191 cursor, `Option+G` 재호출 통과 |
| 기존 #3953 headless E2E | 158쪽 이동, 상태 표시줄 진입, 잘못된 입력 재입력 통과 |
| 기존 #4026 headless E2E | 각주 `Cmd+Z`와 `Option+G` 대화상자 표시 통과 |
| `cd rhwp-studio && npm test` | 763/763 통과 |

## 시각·fixture 판단

이 PR은 Canvas/render, 레이아웃, pagination 계산, HWP/HWPX fixture를 변경하지 않는다. 입력
컨텍스트 전환과 대화상자 이동 순서만 변경하므로 기준 PDF와 visual sweep은 적용하지 않았다. 대신
사용자가 보고한 실제 219쪽 HWP의 각주 상태에서 page status, viewport, 본문 cursor를 함께 보는
headless browser E2E를 실행했다.

## 범위 밖 변경 및 잔여 위험

- 머리말/꼬리말 및 책갈피 찾아가기는 이번 이슈 범위 밖으로 유지했다.
- `gotoPage()`가 실패하면 각주 모드를 종료하지 않는다.
- `getPositionOfPage()`가 본문 cursor를 둘 수 없는 표 위치를 반환할 때의 기존 ±5 문단 fallback은
  그대로 유지한다.

## 최종 권고

review 문서와 오늘 기록을 추가한 최신 PR head의 required GitHub Actions가 통과하고 작업지시자가
승인하면 PR #4033을 병합한다. 병합 뒤 #4030 자동 종료 상태와 원격 branch 정리를 확인한다.
