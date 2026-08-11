---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3566 리뷰 — 그룹 자식 목록 종류 선언 정정 (한컴 개방 실패 해소)

- PR: [#3566](https://github.com/edwardkim/rhwp/pull/3566) / Closes [#3565](https://github.com/edwardkim/rhwp/issues/3565)
- 작성자: `planet6897` (재기여자) — 상호보완 쌍: [#3581](https://github.com/edwardkim/rhwp/pull/3581)(추적 도구)과 함께 접수
- 역할: maintainer 일반 경로 + local_validation

## 라우팅과 작성 시점

```text
base route: maintainer_general.md / modifiers: intake_and_review.md, local_validation.md
current head: 34d2a5ebf / MERGEABLE / behind (참고값)
규모: 2 files, +264/−2 — serializer/control.rs(+14/−2) + 회귀 테스트(+250)
```

## 변경 범위와 수용 판단

그룹(`$con`) SHAPE_COMPONENT 의 자식 종류 목록이 `serialize_group_child` 실제 방출과
어긋나던 두 분기를 정정 — 연결선(`$lin`→`$col`, `connector.is_some()` 동일 술어),
중첩 그룹(`gso `→`$con`, 동일 상수). 한컴은 이 목록으로 자식 트리를 세우므로 어긋나면
문서를 열지 못하며, rhwp 자기 파서는 목록을 참조하지 않아 `convert --verify` 로는
잡히지 않는 계열이다(자기 검증 ≠ 한컴 호환의 전형).

**수용 판단: merge 권고.** 목록 판정과 실제 방출이 같은 술어·같은 상수를 쓰도록 코드
수준에서 불변식을 확인했다(control.rs:1598↔1641, :1612↔1589).

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| 충돌 simulation (devel merge) | clean | behind 는 충돌 아님 |
| focused 2계약 (release-test, 통합 트리) | 2 passed | 자식 종류 존재 단언 포함(표본 무력화 방지) |
| red-check (수정 전 serializer 복원) | **2/2 FAILED → 원복 후 green** | 테스트가 실제 축을 문다 |
| `cargo test --profile release-test --tests` | 371 바이너리 전부 ok (exit 0) | 전체 회귀 없음 |
| fmt / clippy `-D warnings` | 둘 다 통과 | — |
| PR head CI | 전 check green | — |
| **한컴 실물 검증 (기여자, 정답지 이분법)** | 387쪽 편람 저장본 정상 개방(384쪽 = 정답지 일치), 반증 가설 11개 기록 | 판정자=한컴 요건 충족 |

시각 검증 비적용 — serializer 저장 축, 렌더 비접촉.

## 부수 기록

조사 중 발견 별건 5종(PARA_RANGE_TAG 소실, 탭 매개변수 소실, secd 크기, BorderFill
초과, 구역7 크기 불일치)은 PR 범위 밖으로 분리, 기여자가 별도 이슈 등록 예정 — 후속
접수 시 추적.

## 최종 권고

**merge 권고.** merge 후 #3565 auto-close 확인(Closes, devel 대상이라
close-issues-on-devel-push 워크플로 경유), contributor comment, 별건 5종 이슈 등록 추적.
