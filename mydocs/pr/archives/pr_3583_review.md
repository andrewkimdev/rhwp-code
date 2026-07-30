---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3583 리뷰 — 표 CommonObjAttr 저장 반영 무회귀 가드 5개 (테스트 전용)

- PR: [#3583](https://github.com/edwardkim/rhwp/pull/3583)
- Related issue: [#3552](https://github.com/edwardkim/rhwp/issues/3552) — `Refs` 사용
  (부모 추적 유지, close 는 post_merge 7.3.1 체크리스트로 별도 판단)
- 작성자: `yuyu04` — 두 번째 PR (#3580 merge 후, #3552 코멘트의 테스트-only PR 요청 이행)
- 역할: maintainer 일반 경로 (maintainer_general + intake_and_review + local_validation + post_merge)

## 라우팅과 작성 시점

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, local_validation.md
current head: 48a1020b84099c227ca5895f4efb40f122be86fa
mergeable / merge state: MERGEABLE / behind (작성 시점 참고값)
```

## PR metadata (작성 시점 참고값)

| 항목 | 값 |
| --- | --- |
| base → head | `devel` → `yuyu04:test/issue-3552-table-common-attr-guards` (fork) |
| 규모 | 1 file, +233 / −0 — `tests/issue_3552_table_common_attr_save.rs` 신규 (테스트 전용, src 비접촉) |
| milestone / assignee | v1.0.0 / yuyu04 |
| CI | 자동 실행(첫 기여자 게이트 해제 후), 전 check success/skipped |

## 변경 범위와 수용 판단

버그 수정이 아닌 **계약 고정** PR — #2055(902031fb)가 이미 닫은 "표 CommonObjAttr 편집의
raw_ctrl_data FLAGS 동기화" 축을 5개 계약으로 가드한다.

1. ① HWP5 저장→재파스 보존 / ② HWPX 보존(IR 파생 대조군)
2. ③ 잔여 CommonObjAttr 23축 보존 — 향후 무효화 계약 리팩터링 시 재합성 손실의 사전 가드
3. ④ CommonObjAttr 무관 편집(cellSpacing)의 raw 보존 — 과잉 무효화 방지
4. ⑤ 무편집 2-round 바이트 안정 — raw 재사용 경로 무회귀

전제 붕괴를 assert 로 명시(fixture 구조·raw 보유), IR 갱신 실패와 저장 유실의 층을
분리(`flip_treat_as_char` 내 즉시 assert), 공개 API 경로만 사용. #3552 등록 오류(코드
읽기 단정)의 재발을 테스트로 막는다는 취지가 파일 주석에 그대로 담겼다.

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| 충돌 simulation (`devel` merge → `review/pr3583`) | clean merge | behind 는 충돌 아님 |
| focused 5계약 (release-test, 통합 트리) | 5 passed | 전 계약 green |
| red-check (table_ops.rs:2250~2256 FLAGS 동기화 비활성) | **① 만 FAILED, ②③④⑤ passed** | 주장한 가드 의미론과 정확히 일치, 원복 확인 |
| `cargo test --profile release-test --tests` (통합 트리) | 370 바이너리 전부 ok, 실패 0 | 전체 회귀 없음 (신규 파일로 369→370) |
| `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | 둘 다 passed (clippy 경고 0) | — |
| PR head CI | 전 check success/skipped, 미완료 0 | 8-shard·Native Skia·Lint 포함 |

시각 검증 비적용 — 테스트 전용, renderer/layout/src 비접촉.

## merge 후 처리 (post_merge 연계)

이 PR merge 로 **#3552 의 7.3.1 close 체크리스트가 완성**된다:
① sub-issue #3576 close (완료, 2026-07-30) ② 가드 테스트 PR merge (본 PR)
③ 판정 경위 close comment ④ 작업지시자 승인. merge 후 ③·④를 진행한다.

## 최종 권고

**merge 권고.** 테스트 전용(src 비접촉) 신규 1파일, 최신 head CI 전체 통과, 통합 트리
로컬 게이트 전체 통과, red-check 으로 가드 의미론(①만 실패, ②~⑤ 통과)까지 실증했다.
merge 는 작업지시자 승인 뒤 admin merge(`--merge`)로 진행하고, 직후 #3552 close
체크리스트 ③(판정 경위 comment)·④(작업지시자 승인)를 밟는다.
