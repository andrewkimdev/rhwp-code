---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3945 검토 — prior break 뒤 긴 token 반복 줄바꿈

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           visual_fixture_evidence.md, multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
remote head before correction: a5d26c2d8fc5433d4ec2558c33821f5526a29cd2
reviewed local layer head: 8c9c35e9fa3518527c1d5aca28adfdf210022fe1
parent layer head: 3534eb843ae07edf60f061a9b245958c8b7d2cf0
upstream/devel: aeb5805cb93c92b8c44036f4c1fe1f2df420119f
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#3945](https://github.com/edwardkim/rhwp/pull/3945) |
| 작성자 | `postmelee` (collaborator self-merge) |
| 대상 / head | `stack/issue-3937-distribution-glyph-width` / `stack/issue-3822-overlong-token-wrap` |
| Stack 위치 | 2 / 3 — 부모 [#3944](https://github.com/edwardkim/rhwp/pull/3944), 자식 [#3946](https://github.com/edwardkim/rhwp/pull/3946) |
| 작성 시점 원격 상태 | draft, `MERGEABLE`; 보정 push와 restack 뒤 재확인 필요 |
| 보정 전 원격 head | `a5d26c2d8fc5433d4ec2558c33821f5526a29cd2` |
| 리뷰 보정 layer head | `8c9c35e9fa3518527c1d5aca28adfdf210022fe1` |
| 부모 레이어 head | `3534eb843ae07edf60f061a9b245958c8b7d2cf0` |
| 최신 공통 기준 | `upstream/devel` `aeb5805cb93c92b8c44036f4c1fe1f2df420119f` |
| review 문서 작성 전 PR 고유 규모 | 5 files, +230 / -22 |
| 관련 issue | [#3822](https://github.com/edwardkim/rhwp/issues/3822) |

draft, mergeability, 원격 head와 CI는 작성 시점 참고값이다. 최종 merge 조건은 부모 #3944를 먼저
통합한 뒤 갱신된 #3945 head에서 required CI를 다시 통과하는 것과 작업지시자의 명시적 승인이다. 이
review 단계에서는 ready 전환과 merge를 수행하지 않는다.

## 변경 범위와 목적

표 셀에서 앞선 break point 뒤의 긴 무공백 token이 다음 줄로 이동한 뒤에도 그 줄 폭을 초과하면 기존
구현은 token 전체 폭을 더하고 즉시 `continue`했다. 기존 문자 단위 fallback에 도달하지 못해
Latin·숫자·한글 어절이 셀 오른쪽으로 넘치고 caret와 입력 글자가 숨을 수 있었다.

이번 레이어는 자연 폭, 배분 축소 폭, 15 HWPUNIT 허용 오차와 condense pull 조건을 공통 fit 판정으로
모은다. 이전 break에서 줄을 확정한 뒤 새 줄의 실제 잔여 폭과 공백 절감량을 복원하고, 현재 token을
후속 줄의 실제 폭과 hanging indent로 다시 평가한다. 새 줄에도 들어가지 않으면 token 폭을 중복
합산하지 않고 기존 문자 단위 fallback으로 보내 필요한 만큼 반복 분할한다. 단일 CJK/한글 break 경로와
UTF-16 offset 계약은 유지한다.

이 PR은 token 재분할과 overflow 해결만 담당한다. browser glyph 윤곽 확대는 부모 #3944,
pagination 재시작·게시 스케줄링은 자식 #3946의 책임이다. CellFlowTree, PageCheckpoint와 viewport
DisplaySnapshot은 #3743 후속 아키텍처 범위로 남긴다.

## 리뷰 지적 대응

[리뷰 코멘트](https://github.com/edwardkim/rhwp/pull/3945#issuecomment-5177714900)를 검토하고 다음처럼
보정했다.

### 1. 새 #3822 경로의 폭 이중 합산 여부

새 경로는 이전 break 뒤 현재 token 이전의 잔여 폭만 복원한다. token이 후속 줄에도 들어가지 않으면
그 token 폭을 더하지 않고 문자 단위 fallback으로 넘어가므로 이중 합산하지 않는다. hanging indent도
첫 줄이 아닌 후속 줄 폭으로 판정하며 전용 회귀가 이 계약을 고정한다.

### 2. 한글 무공백 어절 회귀 보강

보정 commit `8c9c35e9f`에서 `korean_break_unit == 0`의 다중 문자 한글 어절을 별도 회귀로 추가했다.

```text
입력: "A 가나다라마바사"
예상 line starts: [0, 2, 4, 6, 8]
```

prior break 뒤 반복 분할 계약은 Latin·한글·숫자 모두 고정됐다. composer 단위 테스트는 52건에서
53건, #3822 전용 회귀는 4건에서 5건으로 늘었다.

### 3. 기존 condense 회계 경로

리뷰가 식별한 `break_token_idx == ti`의 폭 재계상과 char-level fallback 뒤
`line_space_savings = 0` 초기화는 이번 변경이 만든 동작이 아니라 기존 condense 계약이다. 이를 함께
바꾸면 #3822 prior-break 다중 token 수정 범위를 넘어가므로 현재 동작을 보존한다. condense 문서에서
조기 줄바꿈 재현이 확보되면 두 항목을 함께 후속 점검한다.

같은 지점의 `eff_w(false)`와 `eff_w(is_first_line)`은 동일한 값이다. 기능 차이가 없어 기존 일반
fallback 표현을 유지한다.

### 4. 쪽수 115 → 116

숨겨졌던 overflow가 실제 줄로 복원되면서 문서 높이와 쪽수가 115에서 116으로 늘어나는 것은 의도된
정확성 변화다. 다른 Studio E2E의 `115`는 좌표·mm 값으로 page count 계약과 무관하다. 변경된 쪽수는
자식 #3946의 통합 E2E에서 검증한다.

### 5. collaborator review 기록

이 문서를 `mydocs/pr/archives/pr_3945_review.md`로 추가한다. 보정은 한글 회귀와 관련 계획·작업 기록으로
명확하므로 별도 `review_impl` 문서는 만들지 않는다.

## 검증

리뷰 보정 layer head에서 다음 로컬 결과를 확인했다.

| 검증 | 결과 |
| --- | --- |
| `env CARGO_INCREMENTAL=0 cargo test issue_3822_reflow_overlong_korean_word_after_prior_break --lib` | 1 passed / 0 failed |
| `env CARGO_INCREMENTAL=0 cargo test renderer::composer::tests --lib` | 53 passed / 0 failed |
| #3822 전용 Latin·한글·숫자·잔여 폭·hanging indent | 5 passed / 0 failed |
| `env CARGO_INCREMENTAL=0 cargo fmt --check` | 통과 |
| `git diff --check` | 통과 |

## 실제 HWP/HWPX 증적

2026-08-03 production WASM snapshot에서 HWP/HWPX 두 번째 숫자 줄바꿈 2 / 2, line count 5 → 6,
caret 665.4 / cell right 672.8, `overflow=false`를 확인했다. HWP/HWPX × digits, Latin, 완료
한글→digits 저장·재열기 6 / 6과 실제 IME→공백→두 번의 숫자 wrap 2 / 2도 통과했다. #3822 미적용
control은 숫자 79번째에서 `cellOverflowed=true`였다.

최상단 combined production WASM E2E에서도 HWP/HWPX 모두 숫자 줄 전환 11 / 69, 최종 숫자 73,
최종 쪽수 116과 synchronous flush 0을 확인했다. 이 결과는 전체 Stack 통합 증적이며 #3945 단독
최신 원격 head의 CI를 대신하지 않는다.

## 시각·fixture 판정

이 레이어는 glyph outline이나 paint scaling을 바꾸지 않고 line start와 overflow 판정을 바꾼다. 의도된
시각 변화는 숨겨졌던 token이 다음 줄들에 나타나고 필요하면 전체 쪽수가 늘어나는 것이다. 별도 SVG
golden 갱신은 필요하지 않다.

부모 레이어에 보존한 `mydocs/pr/assets/pr_3944_issue1949_combined_hwp_final.png`는 최상단 Stack의 HWP
완료 상태에서 긴 숫자가 반복 줄바꿈되고 셀 안에 남는 것을 보여준다. 이는 #3945 단독 oracle이 아닌
combined 증적이라는 한계를 명시한다.

![PR 3945 combined HWP final](../assets/pr_3944_issue1949_combined_hwp_final.png)

## 위험과 알려진 한계

- 기존 단일 CJK/한글 condense 경로의 폭 재계상과 char-level fallback의 공백 절감량 초기화는 유지한다.
- 15 HWPUNIT 허용 오차와 기존 greedy line-breaking 정책 자체는 바꾸지 않는다.
- 긴 token을 정상 줄로 복원하면 line count와 page count가 늘 수 있다. 이는 숨은 overflow보다 정확하다.
- glyph 확대, pagination scheduling과 영속 대형 셀 아키텍처는 각각 #3944, #3946, #3743의 책임이다.
- 최종 merge 근거는 restack 뒤 최신 #3945 head의 CI여야 한다.

## Stack merge 순서와 최종 조건

1. 부모 [#3944](https://github.com/edwardkim/rhwp/pull/3944)를 먼저 `devel`에 통합한다.
2. #3945를 최신 `devel` 위로 restack하고 새 head를 확인한다.
3. 갱신된 #3945 head에서 GitHub Actions, Render Diff와 CodeQL을 다시 통과한다.
4. 작업지시자의 명시적 승인 뒤에만 #3945를 ready/merge한다.
5. 이후 자식 [#3946](https://github.com/edwardkim/rhwp/pull/3946)을 같은 방식으로 restack·검증한다.

**현재 권고: 보정 push와 최신-head CI 대기.** 코드 검토의 blocker는 없고 리뷰가 요구한 한글 어절
회귀와 review 기록은 준비됐다. 부모 #3944 이후 restack한 최신 head의 CI 성공과 작업지시자 승인이
확인되면 collaborator self-merge 후보로 판단할 수 있다.
