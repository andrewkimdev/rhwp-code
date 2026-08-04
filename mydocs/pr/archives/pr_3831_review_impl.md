---
kind: pr_review_implementation
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3831 메인터너 보정 구현 기록

## 기준과 재배치

처음 검토한 source head `010055c` 검증 중 contributor가 최신 `devel`을 병합해 원격 head가
`97be5b3`으로 바뀌었다. PR 고유 변경 11개 파일의 diff는 변하지 않았고, 새 devel
`301d0fe`은 원 PR #3935의 완료 CI로 확인했다. 메인터너 보정은 같은 가시성 branch에서
최신 source head 위로 재배치했다.

```text
upstream/devel 301d0fe5f
upstream/pr3831-head 97be5b362
review/enigma-jerry72-20260804 ab9baf359
```

## 보정 내용

1. `split_table_native()`는 모든 셀의 `row_span`과 행 끝을 `u32`으로 검증한다. 0 span,
   table 범위를 넘는 span, 분할선을 넘는 세로 병합 셀은 문서 변경 전에 오류가 된다.
2. `base_grid_column_widths()`는 runtime local-resize 행이 비어도 outlier 행을 제외한다.
   이는 renderer의 기존 `resolve_column_widths()` 판단과 동일하다.
3. 실제 표 샘플 회귀는 저장·재열기 뒤 runtime hint가 없다는 상태를 재현한다. 추가 회귀는
   `u16::MAX - 2`에서 3행 span을 주어 panic 대신 오류와 무변경을 확인한다.

## Studio 실행 검증

fresh WASM package를 만든 뒤 Studio를 headless Chromium에서 실행했다. 임시 E2E는
`window.__inputHandler.dispatcher`로 실제 `table:split`, `table:attach` 명령을 호출했다.

```json
{
  "front": {"rowCount": 2, "colCount": 2, "cellCount": 4},
  "back": {"rowCount": 2, "colCount": 2, "cellCount": 4},
  "merged": {"rowCount": 4, "colCount": 2, "cellCount": 8},
  "parasAfterSplit": 4,
  "parasAfterAttach": 2,
  "undoDepth": 2
}
```

이 PR은 renderer golden 자체를 변경하지 않는다. 기존 visual baseline은 전체 release-test와 원 PR의
Canvas visual diff CI에서 통과했으며, 이번 검토에서는 기능 명령의 구조·undo 결과를 browser E2E로
확인했다.
