---
kind: review-plan
status: active
canonical: mydocs/pr/archives/pr_3745_review_impl.md
last_verified: 2026-08-02
---

# PR #3745 review 보정 반영 계획

## Commit 경계

| 순서 | commit | 역할 |
| --- | --- | --- |
| 의견 시점 | `a0891ab2b10e65f960bd8641d4133a35a7864162` | linked review comment 작성 시점 head |
| 기준 | `95dc3e1261b0de47e12d762428a842fe988c2b2a` | review 보정 시작/현재 원격 head |
| 1 | `23967640f7aaeb991eb1d2d48938b5c4ce469a4c` | margin guide patch clip |
| 2 | `b48ca8785439b1e373635aa0f55cb5de92748722` | conservative text replay bounds와 재측정 |
| 3 | `6dd0795af35fd030c2ef3fae0fb22cc28092d10c` | `ResolvedStyleSet` clone 제거 |
| 기록 | 이 문서·review·대표 PNG commit | 제한 검증·시각 증적·review 기록 |

## 진행 상태

1. 완료: review 발견 1 수정과 margin-guide focused/full/runtime 테스트.
2. 완료: 발견 2의 no-cull A/B 측정, 보수적 envelope 선택, 24개 행렬 재측정.
3. 완료: style clone 제거와 focused Rust 회귀.
4. 완료: 최종 source production WASM, Studio 관련 테스트, 80ms browser smoke, HWP/HWPX 시각 crop.
5. 대기: 작업지시자 확인 뒤 원 PR branch push.
6. 대기: 최신 head CI, mergeable 상태와 독립 review/작업지시자 승인 확인.
7. 대기: 승인 뒤 review comment reply·thread resolve. merge와 #3137 close는 별도 승인 대상이다.

## 선택 근거

no-cull은 정확성에는 안전하지만 6/6 시나리오에서 frame gate를 넘었으므로 채택하지 않았다.
독립 잉크 범위가 있는 op와 editor mark는 cull하지 않고, plain text만 두 line extent로 확장한
envelope가 clip과 겹치지 않을 때 제한적으로 건너뛴다. 구조 편집 architecture는 #3743으로 분리했다.

## 롤백 경계

- margin guide 회귀: `23967640f`의 helper·호출·테스트만 독립 revert한다.
- text replay 회귀: `b48ca8785`의 `partial_replay` helper와 WebCanvas 연결만 revert한다. Canvas
  clip 기반 correctness authority는 유지한다.
- style borrow 회귀: `6dd0795af`만 revert해 종전 clone 경로로 복귀할 수 있다.
- review 문서와 asset은 code candidate와 별도 commit으로 유지하거나 사실관계만 정정한다.

## 원격 상태 변경 게이트

현재 수행하지 않은 작업은 push, PR reply, thread resolve, approve, ready/draft 변경, merge,
#3137 close다. 모든 remote mutation은 작업지시자의 후속 승인을 필요로 한다. 원격 head
`95dc3e126`의 shard 3·aggregate 실패와 shard 4·5 취소는 보정 branch push 뒤 최신 CI로 다시
판정한다.
