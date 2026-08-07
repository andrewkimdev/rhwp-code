# Stage 1 — task_m100_4159 진단·래칫 준비

- **이슈**: [#4159](https://github.com/edwardkim/rhwp/issues/4159)
- **계획서**: [`mydocs/plans/task_m100_4159.md`](../plans/task_m100_4159.md)
- **브랜치**: `task_m100_4159_nested_table_bottom_clip`
- **분기 기준**: `upstream/devel` `06f8ebcca`
- **작업 시각**: 2026-08-07 KST

## 1. 기준 진단

실제 fixture 물리 3쪽의 bottom `Line`은 생성돼 있지만 조상 분할 셀 clip 밖에 있다.

| 노드 | 하단 |
| --- | ---: |
| 조상 partial Table | 827.3px |
| 조상 `clip=true` TableCell | 824.88px |
| 종료 nested Table | 827.3px |
| nested bottom Line stroke | 827.27px |

재귀 자식 표가 조상 `inner_area.y = cell_y + pad_top`에서 시작하면서 조상 조각과 같은
711.5px fragment 높이를 쓴다. `layout_partial_table()`의 최종 자손 포섭은 Table bbox만
확장하므로 TableCell bbox를 clip으로 쓰는 SVG와 Canvas2D에서 선이 잘린다.

## 2. 구현 판정 기준

- `cell_cut_window`가 마지막 유닛까지 포함하는 종료 셀만 확장 후보로 삼는다.
- 실제 재귀 Table 자손의 하단이 셀 하단을 넘는 경우에만 필요한 만큼 확장한다.
- 다음 continuation이 있는 `eu < units.len()` 조각은 기존 셀 clip을 보존한다.
- cell background·외부 edge grid의 조판 위치는 바꾸지 않고 clip 포섭만 정합시킨다.

## 3. 다음 단계

실제 fixture 구조 래칫을 red로 고정한 뒤 최소 구현, SVG·Canvas2D 시각 증적, 기존 #2007
페이지네이션 회귀를 순서대로 수행한다.
