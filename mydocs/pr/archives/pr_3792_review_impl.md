---
kind: plan
status: active
pr: 3792
---

# PR #3792 review 보정 구현 기록

## 목적과 경계

shadow classifier의 실제 skip 비활성 안전 경계는 유지하면서 review에서 확인한 preflight 직렬 비용,
계약 테스트 취약성, checkout authority 오표기와 계획 문서의 오래된 #3684 상태를 보정한다. Render Diff
분류와 frontend worker 분리는 이 PR에서 활성화하지 않고 후속 PR의 명시적 선행 조건으로 남긴다.

## 적용 commit

| commit | 역할 |
| --- | --- |
| `1a1dc9756` | shadow impact classifier와 historical fixture 추가 |
| `43650312a` | review-only fast-pass에서 advisory checkout 생략 |
| `aa81855e6` | sparse checkout, authority, 계약 테스트와 활성화 계획 보정 |
| `1064aadae` | 최신 `upstream/devel@6ab503fe9` update merge |

중간 update-branch merge는 최신 base를 포함하기 위한 동기화 commit이며 기능 보정은 `aa81855e6`에
분리돼 있다.

## 실행 순서

1. PR metadata와 review 코멘트를 대조하고 formal reviewer로 `edwardkim`을 지정했다.
2. correction commit에서 workflow·classifier·계약 테스트·계획 문서를 보정했다.
3. focused 검증 20+6건, actionlint와 PR diff check를 실행해 통과했다.
4. 최신 `devel` merge simulation에서 충돌이 없음을 확인하고 update merge를 확정했다.
5. correction candidate를 fork head에 push했고 full CI와 CodeQL이 모두 통과했다. sparse checkout 보정으로
   CI preflight는 39초에서 9초, advisory checkout은 30초에서 1초로 줄었다.
6. 통과 결과를 확정한 이 review와 review_impl을 single-parent mydocs-only commit으로 추가한다.
7. trailing head의 preflight와 `Build & Test` fast-pass를 확인한다.
8. 최신 head·base 포함·mergeability·required check를 다시 확인하고 작업지시자 승인 전에는 merge하지
   않는다.

## 후속 활성화 조건

- `unit`: Studio 전역 `tsc --noEmit` + 전체 Studio unit test
- `package`: unit 계약 + Vite·extension·package build
- `render`: Canvas visual diff와 CanvasKit readiness의 영향축 분리 또는 보수적 합집합
- trusted execution: PR merge ref의 advisory classifier가 아니라 base의 신뢰된 classifier
- cache: #3810의 정리 후 4.73GB 기준선 대비 회귀 확인

## 롤백

merge 전에는 PR #3792를 merge하지 않으면 `devel`에 영향이 없다. merge 후 classifier에 문제가 있더라도
Stage 1은 worker skip을 활성화하지 않으므로 기존 CI 검증력은 유지된다. shadow 기록만 비활성화해야 하면
preflight의 checkout·수집·분류·summary와 `shadow_*` output을 함께 되돌리고, 기존
`frontend_required`와 `Build & Test` 계약은 그대로 둔다.
