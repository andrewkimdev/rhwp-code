---
kind: working
status: active
canonical: mydocs/plans/task_m100_3682.md
last_verified: 2026-08-04
---

# Task #3682 Stage 1 보고 — P0~P5 행동 실측

CDP e2e 프로브 `rhwp-studio/e2e/issue-3682-chart-object-probe.test.mjs` 신설,
`samples/chart/세로막대형/묶은세로막대형.hwp` 로 실측(dev 7700, 호스트 Chrome CDP).

## 현황표 — 이슈 전제("Track A 미착수")는 **틀렸다**

| 단계 | 동작 | 실측 | 비고 |
|---|---|---|---|
| P0 | ole 레이아웃 방출 | **됨** | `{x:113.4,y:132.3,w:430,h:250,secIdx:0,paraIdx:0,controlIdx:2}` |
| P1 | 클릭 선택 | **됨** | `type=ole ref={sec:0,ppi:0,ci:2}` |
| P2 | 속성 다이얼로그 | **미검증** | 프로브가 커맨드 레지스트리 전역 진입점을 못 찾음 — 코드에는 존재(`format.ts:476` PicturePropsDialog) |
| P3 | 드래그 이동 | **됨** | Δx=60 Δy=40 (지시대로 이동) |
| P4 | 복사·붙여넣기 | **됨** | ole 1→2 |
| P4 | **삭제** | **안 됨** | ole 2→2 |
| P4 | undo | (삭제 미발생이라 무의미) | 2→1 은 붙여넣기 취소 |
| P5 | z-order | **미검증** | `input-handler` 전역 함수는 없으나 브리지 `changeShapeZOrder` 존재 |

## 확정된 진짜 갭 — P4 삭제

경로 추적으로 근인 확정:

```
input-handler-keyboard.ts:147 deleteSelectedObject()
  ref.type === 'image'    → deletePictureControl
  ref.type === 'equation' → deleteEquationControl
  그 외(= 'ole' 포함)      → deleteShapeControl        ← 여기로 샌다
```

```rust
// object_ops/shape.rs:1017
if !matches!(&para.controls[control_idx], Control::Shape(_)) {
    Err("지정된 컨트롤이 Shape이 아닙니다")
}
```

**차트(OLE)는 `Control::Shape` 이 아니므로 코어가 거부**한다. studio 는 반환값을
확인하지 않아 조용히 실패한다. `delete_ole_control` 계열 코어 API 는 **부재**
(`grep pub fn delete.*ole` 0건).

## 방법 교훈 — 프로브 자체 검증 필요

1차 프로브는 P1·P3 를 "안 됨"으로 잘못 보고했다. 원인은 내가 추측한 API 이름
(`pictureObjectSelection`)이 실제(`cursor.isInPictureObjectSelection()`)와 달랐기
때문. **가짜 갭이 진짜 갭 목록을 오염시킬 뻔했다** — 실패 항목은 코드에서 실제
API 존재를 확인한 뒤에만 갭으로 확정한다(#3681 프록시 함정과 동족).

## Stage 2 제안

- 실제 갭은 **P4 삭제 1건 + 미검증 2건(P2·P5)**. 이슈의 "전부 불가"와 거리가 크다.
- 다음: P2·P5 를 실제 진입점으로 재측정 → 확정 갭 목록 → 작업지시자와 수정 범위 결정.
- 수정 후보(P4): 코어 `delete_ole_control_native` 신설 + studio `deleteSelectedObject`
  에 `'ole'` 분기. 실패 시 조용히 넘어가지 않도록 반환값 검사도 함께.
