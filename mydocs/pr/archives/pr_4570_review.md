---
kind: pr-review
status: pending-ci-release-hold
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-12
---

# PR #4570 리뷰 - 자리차지 표 앵커 줄 재배치

| 항목 | 문서 작성 시점 참고값 |
| --- | --- |
| PR | [#4570](https://github.com/edwardkim/rhwp/pull/4570) |
| 작성자 | `planet6897` |
| base / 원 head | `devel` / `0d9d0d5acd6e46be1715d154450ed3142c917dc5` |
| 원 변경 규모 | 8 files, +450/-80 |
| 통합 적용 | `79311639c`부터 `76efeef1e`까지 9개 기능·golden commit |
| 관련 이슈 | [#4533](https://github.com/edwardkim/rhwp/issues/4533) |

비-TAC TopAndBottom 표의 앵커 줄만 저장 사다리 증거가 있을 때 밴드 아래로 옮긴다. 후속 문단과
밴드 자체를 움직이지 않고, HWP5/HWP3/HWPX 계보별 조건을 분리해 광범위한 vpos snap으로 확대되지 않게 했다.
golden `issue-157` 갱신도 같은 앵커 줄 좌표 변화에만 한정됐다.

통합 HEAD의 release-test 전체, Native Skia 3종, WASM build, Clippy와 focused provenance stage-1 6건을
통과했다. 렌더 영향 변경이므로 GitHub 통합 PR의 최신 head Full CI와 Render Diff를 merge 전 다시 확인한다.
릴리스 hold 동안 원 PR을 merge 또는 close하지 않는다.
