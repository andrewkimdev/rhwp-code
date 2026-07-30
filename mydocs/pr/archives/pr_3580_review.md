---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3580 리뷰 — delete_text_at 의 클램핑된 char_shape ref 중복 제거

- PR: [#3580](https://github.com/edwardkim/rhwp/pull/3580)
- Related issue: [#3576](https://github.com/edwardkim/rhwp/issues/3576) (`Closes` 사용 —
  auto-close 확인 필요, 부모 [#3552](https://github.com/edwardkim/rhwp/issues/3552)는
  post_merge.md 7.3.1 체크리스트에 따라 별도 판단)
- 작성자: `yuyu04` — **rhwp 첫 PR** (Discussion #3498 → #3576 등록 → 이 PR 사이클)
- 역할: maintainer 일반 경로 (maintainer_general + intake_and_review + local_validation + post_merge)

## 라우팅과 작성 시점

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md, multi_pr_update_branch.md(behind 참고)
current head: f81e24c26222363658b468e51d526b0298f6c0cd
mergeable / merge state: MERGEABLE / behind (작성 시점 참고값)
```

## PR metadata (작성 시점 참고값)

| 항목 | 값 |
| --- | --- |
| base → head | `devel` → `yuyu04:fix/issue-3576-char-shape-dedup` (fork) |
| 규모 | 2 files, +232 / −0 (모델 수정 실질 1줄 + 주석, 회귀 테스트 224줄) |
| milestone / assignee / reviewer | v1.0.0 / yuyu04 / edwardkim |
| CI | 첫 기여자 fork라 action_required → maintainer 승인으로 실행, 전체 통과 |

## 변경 범위와 수용 판단

1. `src/model/paragraph.rs` `delete_text_at`: 클램핑 루프 뒤
   `self.char_shapes.dedup_by_key(|cs| cs.start_pos)` 1줄 추가. 삭제 범위 안 ref 들이
   `utf16_start` 로 클램핑되고 범위 뒤 ref 가 시프트로 같은 위치에 합류하면서 남는
   중복을 제거한다. 변환이 단조라 오름차순 불변식이 유지되어 인접 dedup 으로 충분하고,
   첫 ref(주 글자모양)를 남기는 선택도 타당하다.
2. `tests/issue_3576_char_shape_dedup.rs`: 실제 편집 API 경로(`delete_text_in_cell` /
   `insert_text_in_cell` / `apply_char_format_in_cell`)로 구성한 3계약 — ①전체 삭제 후
   start_pos 유일 ②재삽입 텍스트가 주 글자모양 하나로 덮임 ③부분 삭제 생존 ref 보존
   (과잉 dedup 방지). 손제작 Paragraph 의 불변식 붕괴 함정을 피한 구성.

수용 판단: **merge 권고.** 모델 편집 경로의 국소 수정으로 renderer/serializer 미변경.
raw_ctrl_data 계약과 무관한 IR 내 정리라 저장 왕복 회귀 축 없음(전체 스위트로 확인).

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| 충돌 simulation (`devel` merge → `review/pr3580`) | clean merge | behind 는 상태일 뿐 충돌 없음 |
| focused `issue_3576_char_shape_dedup` (release-test) | 3 passed | 계약 3축 green |
| red-check (dedup 1줄 제거 후 focused) | ①② FAILED, ③ passed | 테스트가 실제 축을 물고 있음, 원복 확인 |
| `cargo test --profile release-test --tests` (통합 트리) | 369 바이너리 전부 ok, 실패 0 | 전체 회귀 없음 |
| `cargo fmt --check` | passed | — |
| `cargo clippy --all-targets -- -D warnings` | passed (경고 0) | — |
| PR head CI (승인 후) | 전 check success/skipped, 미완료 0 | 8-shard, Native Skia, Lint, Render Diff preflight 포함 |

시각 검증은 적용하지 않는다 — renderer/layout 미변경, IR 편집 경로 수정이며 계약 테스트가
발현 증상(재삽입 텍스트의 글자모양)을 직접 가드한다.

## 위험과 후속

- 부분 삭제에서 `utf16_start` 정확히 위치한 ref 가 문단 끝을 넘겨 남는 기존 동작(이 PR 이전부터
  존재)은 이번 범위 밖 — 발견 시 별도 이슈 대상.
- merge 후: #3576 auto-close 확인(2–3회 재조회), 부모 #3552 는 7.3.1 체크리스트
  (가드 테스트 PR merge 대기) 적용, 첫 PR contributor comment(환영 패턴).

## 최종 권고

**merge 권고.** 최신 head CI 전체 통과·로컬 게이트 전체 통과·red-check 실증까지 확인했다.
merge 는 작업지시자 승인 뒤 admin merge(`--merge`)로 진행한다.
