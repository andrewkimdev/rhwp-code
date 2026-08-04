# PR #3954 검토

## 결론

**수용 후보.** 대형 HWP의 쪽 이동을 커서 배치 성공 여부와 분리하고, 표가 쪽 시작인 경우에는
인접 문단으로 커서를 옮기도록 보정했다. 실제 219쪽 HWP에서 158쪽 이동, 상태 표시줄 진입,
잘못된 입력의 재입력, 모달 종료 후 `Option+G` 재호출을 자동 검증했다.

최종 병합 조건은 최신 PR head의 GitHub Actions 통과와 작업지시자 승인이다.

## 접수 및 기준

| 항목 | 내용 |
| --- | --- |
| PR | [#3954](https://github.com/edwardkim/rhwp/pull/3954) `fix(studio): 대형 HWP 찾아가기 복구` |
| 작성자 | `jangster77` |
| 대상 | `devel` |
| 검토한 구현 head | `ad60775c35ddeac08117e675ff6d1d7269462ecb` |
| 구현 기준 devel | `cf5d462dc` |
| 관련 이슈 | [#3953](https://github.com/edwardkim/rhwp/issues/3953) |
| 구현 변경 규모 | 10 files, +199 / -11 |
| 작성 시점 mergeable | `MERGEABLE` |
| 작성 시점 merge 상태 | `BLOCKED` (GitHub Actions 대기) |

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md
```

## 변경 내용

- `CanvasView.gotoPage()`가 유효한 전역 쪽 번호의 virtual scroll offset으로 화면을 이동한다.
- `GotoDialog`는 페이지 입력을 확인한 뒤 화면 이동을 먼저 수행하고, 해당 위치에 직접 커서를
  놓지 못하면 앞뒤 다섯 문단에서 커서 배치 가능 위치를 찾는다.
- 커서 위치를 찾지 못한 경우에는 이유를 표시하고 모달을 유지하여 다시 입력할 수 있게 했다.
- 모달을 닫을 때 `InputHandler`에 포커스를 복구해 macOS `Option+G`를 포함한 편집 단축키가
  다음 입력에도 전달되도록 했다.
- 상태 표시줄의 `현재 쪽 / 전체 쪽`을 접근 가능한 버튼으로 바꾸고 `edit:goto` 명령을 호출하게 했다.

## 로컬 검증

아래 검증은 구현 head `ad60775c3`에서 완료됐다.

| 검증 | 결과 |
| --- | --- |
| `git diff --check` | 통과 |
| `(cd rhwp-studio && npx tsc --noEmit)` | 통과 |
| `npm --prefix rhwp-studio test` | 759 passed, 0 failed |
| `npm --prefix rhwp-studio run build` | 통과 |
| `issue-3953-large-document-goto.test.mjs` | 실제 HWP 219쪽 로드, 158쪽 이동, 상태 표시줄 클릭, 범위 오류 재입력, `Option+G` 재호출 통과 |

E2E 대상은 `samples/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwp`다.
158쪽 요청 뒤 상태 표시줄은 `158 / 219 쪽`, scrollTop은 `178015`였고 커서는 0-base 쪽 번호
`157`에 배치됐다.

## 시각·fixture 판단

이 PR은 Canvas render 출력, 레이아웃, pagination, HWP/HWPX fixture를 변경하지 않는다. 기존 문서의
virtual scroll 위치와 모달·상태 표시줄 상호작용만 변경하므로 기준 PDF나 visual sweep은 적용하지 않았다.
실제 브라우저 E2E로 화면 이동과 상태 표시줄 값을 확인했다.

## 범위 밖 변경 및 잔여 위험

- 문서 내용, 쪽 수 계산, layout 결과, 기존 HWP fixture는 변경하지 않았다.
- 페이지 시작 인근 다섯 문단 모두 커서 배치 불가인 비정상 문서는 화면 이동 후 모달을 유지하고
  오류를 표시한다. 잘못된 위치를 성공으로 닫지 않는다.
- `gotoPage()`는 유효 범위만 수락하므로 범위 밖 입력은 기존 쪽 번호 오류 처리로 차단된다.

## 최종 권고

최신 PR head의 required GitHub Actions가 통과하고 작업지시자가 승인하면 PR #3954를 병합한다.
병합 뒤 #3953을 닫고, 구현 중 커서 위치와 화면 이동을 분리한 이유를 후속 기록에 남긴다.
