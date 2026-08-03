---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3866 검토 - HWPX→HWP 표 flowWithText 보존

## 접수와 적용

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3866](https://github.com/edwardkim/rhwp/pull/3866) / @planet6897 |
| 관련 이슈 | [#3834](https://github.com/edwardkim/rhwp/issues/3834) |
| 원 head / 적용 | be6a2534352053e791cd4b97c9dfcc5de73fe667 |
| 누적 적용 | c8de4f2e6, c2ddb937 (원 기능 + rustfmt test) |
| base / 작성 시점 상태 | devel / MERGEABLE, BEHIND |
| 규모 / 충돌 | 193 additions, 16 deletions, 2 files / 없음 |

HWPX 표의 flowWithText=0을 HWP common object attribute bit 13에서 무조건 1로 OR하던
직렬화 오류를 제거한다. 원 IR의 boolean을 그대로 materialize하므로 0과 1 모두 보존한다.

## 검증

- issue_3834_flow_with_text_preserved 2 / 2 통과: zero fixture와 enabled fixture 모두
  HWP 결과를 다시 읽어 같은 IR boolean임을 확인했다.
- HWPX→HWP writer의 binary attr과 reparse를 한 integration test로 고정했다.
- cargo fmt --check, diff --check, clippy -D warnings, release-test 전체 회귀를 누적 후보에서 통과했다.

## 판정

**누적 통합 수용.** 특수한 take-place/flow 정책을 새로 정하는 변경이 아니라 source boolean
보존을 복구하는 serializer fix이며, 양쪽 값이 모두 회귀로 보호된다.

