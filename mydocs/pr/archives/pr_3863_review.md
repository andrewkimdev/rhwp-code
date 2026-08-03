---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3863 검토 - HWP5 번호 형식 문자열 HWPX 방출

## 접수와 적용

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3863](https://github.com/edwardkim/rhwp/pull/3863) / @planet6897 |
| 관련 이슈 | [#3862](https://github.com/edwardkim/rhwp/issues/3862) |
| 원 head / 적용 | 9bf8add89c05274784d22601a0a41d268a6c1e99 / ad10797df |
| base / 작성 시점 상태 | devel / MERGEABLE, BEHIND |
| 규모 / 충돌 | 55 additions, 18 deletions, 1 file / 없음 |

HWP5 경유 번호 매기기에서 paraHead의 번호 문자열이 attribute가 아니라 XML text node라는
스키마 계약을 복원한다. 비어 있는 format은 기존처럼 self-closing tag를 유지하고, 원 HWPX의
raw paraHead splice 경로는 변경하지 않는다.

## 검증

- write_numbering_skeleton_emits_level_format_string_as_text 1 / 1 통과:
  ^1.과 (^2)는 text node로, 빈 level은 self-closing으로 고정했다.
- samples/2022년 국립국어원 업무계획.hwp를 export-hwpx --verify --verify-pages로 실제 확인:
  IR diff 0, 35쪽 전후 동일. 이 표본의 level format은 비어 있어 nonempty path의 실물
  증명에는 사용하지 않고 unit fixture를 그 경계의 직접 근거로 기록한다.
- cargo fmt --check, diff --check, clippy -D warnings, doc test를 누적 후보에서 통과했다.

## 판정

**누적 통합 수용.** raw 보존 경로는 유지하면서 fallback skeleton의 정확한 XML 내용만 채운
변경이며, 빈/비빈 level 양쪽의 계약을 회귀로 고정했다.

