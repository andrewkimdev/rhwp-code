---
kind: review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/pr/archives/pr_3718_review.md
last_verified: 2026-08-01
---

# PR #3718 검토 기록 — 중첩 표 분할 끝 조각의 꼬리 문단 보존

## 결론과 적용 범위

[PR #3718](https://github.com/edwardkim/rhwp/pull/3718)는 `@kevin9327`의 중첩 표가
쪽을 넘을 때 마지막 조각 뒤 문단이 사라지던 layout 결함 보정이다. 원 source head
`83bdf97f8`의 기능은 통합 PR [#3742](https://github.com/edwardkim/rhwp/pull/3742)에
`cd12815e9`로 누적했다. `nested_table_tail_paragraph_is_rendered`와
`nested_table_sharing_a_paragraph_with_text_is_not_dropped`가 그 계약을 고정한다.

판정은 **승인 권고**다. 다만 이 기록·공유 구현 기록·오늘할일과 증적만 담은
single-parent 문서 tail의 fast-pass가 끝나기 전에는 merge하지 않는다.

## 검토 경계

| 항목 | 근거 |
| --- | --- |
| 라우팅 | `collaborator_self_merge` · `intake_and_review` · `local_validation` · `multi_pr_update_branch` · `visual_fixture_evidence` |
| code candidate | `b1e9619433bd9f068a361ddfb42ea0138f0077d1` |
| GitHub code CI | [CI run 30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379) — preflight, lint, Native Skia, 8 test shards, `Build & Test` success |
| 보조 CI | [CodeQL 30711901389](https://github.com/edwardkim/rhwp/actions/runs/30711901389), [Canvas visual diff 30711901404](https://github.com/edwardkim/rhwp/actions/runs/30711901404) success |
| 공통 로컬 검증 | focused 회귀, `cargo test --profile release-test --tests` 최종 exit 0, fmt, clippy `-D warnings` success |

## 현재-head 시각 증적

renderer/layout 변경이므로 단순 unit/integration 성공을 PDF 동등성 주장으로 확대하지 않았다.
현재 code candidate에서 `samples/task2097/75544_pii_bunseok.hwpx`를 HWP 2020으로 PDF로
변환하고, 결함이 관찰됐던 60쪽만 같은 head의 rhwp SVG와 대조했다.

| 항목 | 결과 |
| --- | --- |
| HWP 2020 기준 PDF | `pdf/task2097/75544_pii_bunseok-2020.pdf`, 66쪽 A4, 546,627 bytes, SHA-256 `3a8d42f2e55a64b9a35fee89dfaeab27f52603fe112d4241d0cf03bae2b4dc4e` |
| 변환 검증 | MCP `status: success`, `run_status: 0`, `validation: ok`, PDF page match 66/66, text validation `ok` |
| rhwp 출력 | 전용 `target/review-kevin9327-20260801/debug/rhwp`로 SVG/render tree 각 66쪽 export |
| 선택 대조 | p060만 raster/overlay/review; frame-tail overflow 0, flagged page 0 |
| 영구 review PNG | [p060 review](../assets/pr_3742_issue_3658_terminal_tail_review_p060.png), SHA-256 `c29e08da6ea08f1f01476c3e7251739fed6f2ba5581ee9e83b5e830ccc82d48c` |
| overlay 참고값 | pixel match 91.04571%, ink match 6.81808%; 글꼴·전체 layout 차이를 포함하는 proxy이며 pass/fail 품질 등급으로 쓰지 않음 |

대조 이미지에서 rhwp 쪽 하단의 `○○금융회사 대표이사 △△△` 꼬리 문단이 남아 있음을 직접
확인했다. 이는 #3658의 **문단 소실 회귀가 재발하지 않음**을 보이는 증적이다. 두 renderer의
모든 글리프·간격이 PDF와 같다는 주장은 하지 않으며, overlay 차이는 별도 시각 정합 개선 과제로
남긴다. 임시 sweep 결과는 `/tmp/rhwp-pr3742-visual-sweep-b1e961/`에만 두고 merge 판단에
쓰는 PNG는 위 stable asset으로 고정했다. 재현용 임시 비교·overlay·review 경로는 각각
`/tmp/rhwp-pr3742-visual-sweep-b1e961/pr3742-issue3658/compare/compare_060.png`,
`/tmp/rhwp-pr3742-visual-sweep-b1e961/pr3742-issue3658/overlay/overlay_060.png`,
`/tmp/rhwp-pr3742-visual-sweep-b1e961/pr3742-issue3658/review/review_060.png`이다.

## Merge 전 조건과 후속

최신 #3742 head에 대해 review-only fast-pass A의 preflight와 required `Build & Test` aggregate가
success이고, PR 상태가 `CLEAN`·`MERGEABLE`이어야 한다. 그 뒤 통합 PR 하나만 merge하고 원 PR의
감사·supersede 안내 같은 외부 코멘트는 현재 지시대로 나중에 실제 줄바꿈 body로 게시한다.
