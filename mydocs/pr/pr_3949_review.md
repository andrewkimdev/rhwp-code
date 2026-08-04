# PR #3949 검토

## 접수 정보

| 항목 | 내용 |
| --- | --- |
| PR | #3949 `fix(layout): 중첩 표를 부모 셀 안에서 시작하게` |
| 작성자 | `planet6897` (Jaeuk Ryu, 기존 기여자) |
| 대상 | `devel` |
| 최초 원 head | `54863489377712683bef3a083a7d5b5f9e292a03` |
| 최신 원 head | `ee1907f18054058447182220eead3e956ee747fe` |
| 검토 시작 기준 devel | `f864e851a98f30fef624976ce76f079fd5fe9eab` |
| 규모 | `src/renderer/layout/table_layout.rs` 1파일, 6행 추가 |
| 관련 이슈 | #3637 (closed) |

## 절차와 기준선

```text
base route: maintainer_general
modifiers: intake_and_review.md, local_validation.md,
           visual_fixture_evidence.md, rework_and_exceptions.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  intake_and_review.md, maintainer_general.md,
                  local_validation.md, visual_fixture_evidence.md,
                  rework_and_exceptions.md
current head: ee1907f18054058447182220eead3e956ee747fe
```

검토 시작 시 원 head는 기준 `devel`의 조상이 아니며 merge 상태도 `BEHIND`였다. 이후 작성자가
최신 `devel`을 병합한 `ee1907f`를 push했다. 원 브랜치를 rebase하거나 수정하지 않고,
`review/planet6897-20260804`에서 원 변경과 최신 source를 merge해 보존했다. 최신 `devel`과의
merge simulation은 충돌 없이 통과했고 `git diff --check`도 통과했다.

렌더러 레이아웃 변경이므로 local renderer 검증과 시각 증적 경로를 적용한다. 원 PR에는
재현 fixture, focused regression test, 기준 PDF, 안정 visual asset이 포함되어 있지 않다.

## 변경 요약

`Control::Table`의 비-TAC 중첩 표 anchor 후보 `nested_y`에 부모 셀 콘텐츠 하단 상한을
적용한다. 이 값은 이어서 `LayoutRect`의 y/height와 `layout_table`의 anchor로 직접 전달된다.

## 초기 검토 관찰

- PR 본문은 80550·49308 코퍼스 문서에서 쪽 밖 줄이 1,715에서 950으로 감소했다고 주장한다.
- 그러나 #3637의 2026-07-31 조사 기록에는 같은 `table_layout.rs`의 중첩 경로에서
  `nested_y`를 부모 셀 콘텐츠 하단으로 상한한 실험이 658개 쪽 밖 고유 줄 중 5개만
  움직였고, 프로브 문서 렌더 결과는 동일하여 철회됐다고 적혀 있다.
- 이 PR은 위 상충하는 측정의 원인, 사용한 정확한 revision/입력/명령, 또는 focused regression을
  제공하지 않는다. 최신 `devel`에서 재현 가능한 증거가 없으면 개선 주장을 수용할 수 없다.

## 검증 기록

- 최신 `devel`과 원 head를 `pr3949-merge-test` worktree에서 merge simulation했다.
  충돌 없이 적용됐고 `git diff --check`를 통과했다.
- 다음 focused regression은 patch 적용 상태에서 통과했다.

  ```bash
  CARGO_TARGET_DIR=/home/tsjang/rhwp/target/review-planet6897-20260804 \
    CARGO_INCREMENTAL=0 cargo test --profile release-test \
      --test issue_3637_split_cell_nested_table_vpos
  ```

- 같은 worktree에서 PR patch만 일시적으로 제거해 위 명령을 다시 실행해도 통과했다.
  이 테스트는 `table_partial.rs`의 분할 셀 경로를 검증하므로, 이번
  `table_layout.rs`의 일반 중첩 표 anchor 상한을 회귀로 검출하지 못한다.
- PR이 수치 근거로 든 80550·49308 코퍼스 원본과 `overflow_axis.py` 측정 산출물은 이
  서버와 저장소에 없었다. 따라서 주장한 1,715→950 감소를 독립 재현하거나 visual asset으로
  확인할 수 없었다.
- 최신 `devel`과의 merge result에 대한 release-test 전체, Native Skia, WASM build는
  재현 fixture와 효과 검증이 부재한 차단 사유 때문에 실행하지 않았다. 최초 원 head의 GitHub
  CI는 현재 merge 판단에 재사용하지 않는다. 최신 head CI는 진행 중이며 fixture 요청과 별개로
  완료 결과를 재확인해야 한다.

## 작성자 요청

2026-08-04에 PR comment로 원본 HWP fixture를 요청했다.

- comment: https://github.com/edwardkim/rhwp/pull/3949#issuecomment-5177241517
- 요청 내용: 80550 또는 49308 원본 `.hwp`, 또는 같은 일반 중첩 표 경로를 재현하는 최소 HWP,
  재현 쪽 번호·측정 revision·명령·기대 y 범위
- fixture가 도착하면 최신 `devel` + `ee1907f`에서 전후 SVG와 overflow를 다시 측정한다.

## 현재 권고

**재작업 요청.** 최신 `devel` 기준 재현 fixture와 이번 `table_layout.rs` 분기를 실제로
실패·성공시키는 regression test가 필요하다. 또한 #3637에 이미 남아 있는 같은 클램프의
무효 측정과 현재 수치가 달라진 이유를 revision, 입력, 명령, 전후 결과로 설명해야 한다.
그 뒤 최신 head 기준 GitHub Actions 완료도 다시 확인해야 한다. 최신 source는 `devel`에
정합됐지만 fixture 요청은 아직 대기 중이다.
