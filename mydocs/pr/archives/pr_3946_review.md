---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3946 검토 — latest-revision deferred pagination 시작 병합

## 검토 경로

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           visual_fixture_evidence.md, multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  multi_pr_update_branch.md
remote head before correction: 07e775092d1c08a69a8b71b523a0648eb0d6e7a5
reviewed local layer head: f5c423a93c25d84c291e17a0b7462b29ec153585
parent layer head: 8a2a6b65052130d6b3849565db0662c71a9105f7
upstream/devel: aeb5805cb93c92b8c44036f4c1fe1f2df420119f
```

## 메타데이터

| 항목 | 값 |
| --- | --- |
| PR | [#3946](https://github.com/edwardkim/rhwp/pull/3946) |
| 작성자 | `postmelee` (collaborator self-merge) |
| Stack 위치 | 3 / 3 |
| 대상 / head | `stack/issue-3822-overlong-token-wrap` / `stack/issue-3815-pagination-coalescing` |
| 작성 시점 원격 상태 | draft, `MERGEABLE`; 보정 restack·push 뒤 재확인 필요 |
| 보정 전 원격 head | `07e775092d1c08a69a8b71b523a0648eb0d6e7a5` |
| 리뷰 보정 layer head | `f5c423a93c25d84c291e17a0b7462b29ec153585` |
| 부모 레이어 head | `8a2a6b65052130d6b3849565db0662c71a9105f7` |
| 최신 기준 | `upstream/devel` `aeb5805cb93c92b8c44036f4c1fe1f2df420119f` |
| review 문서 작성 전 PR 고유 규모 | 10 files, +895 / -80 |
| 관련 issue | [#3815](https://github.com/edwardkim/rhwp/issues/3815) |

draft, mergeability, head SHA와 CI는 변할 수 있는 작성 시점 참고값이다. 최종 merge 조건은 최신 PR head의
required CI 통과와 작업지시자 승인이다. 사용자 지시에 따라 reviewer request, ready 전환과 merge는 이
review commit에서 수행하지 않는다.

## 변경 범위와 목적

115쪽 거대 표 셀에서 flow를 바꾸는 입력마다 `beginDeferredPagination()`이 input dispatch 호출 스택
안에서 실행되고 진행 중 job을 즉시 취소·재시작하던 비용을 줄인다.

`DeferredPaginationRunner`를 `idle`, `begin-scheduled`, `stepping` 상태로 나누고 다음 계약을 적용한다.

- 최초 begin은 입력 paint 기회를 확보하는 고정 100ms target에서 input stack 밖에 실행한다.
- 최초 timer 대기 중 요청은 target을 연장하지 않고 begin 시점의 최신 descriptor를 쓴다.
- 전진 중이거나 restart 대기 중인 job은 마지막 요청 뒤 200ms trailing window로 합친다.
- begin 뒤 첫 fragment는 일반 cadence에서 전진하고 다음 step 한 번만 25ms settle gap을 둔다.
- generation guard가 취소·대체된 begin, step, settle callback과 stale publication을 차단한다.
- queued begin도 `hasPendingWork()`로 판단해 navigation, blur, 저장·인쇄 barrier가 회수한다.
- deferred job이 있으면 120ms idle flush가 같은 작업을 동기로 되풀이하지 않는다.

하위 Stack의 #3944는 Canvas/SVG glyph 폭, #3945는 prior-break 긴 token 반복 줄바꿈, #3946은 Studio
pagination 시작 병합을 담당한다. scheduler 제품 변경은 하위 renderer와 독립적이지만 실제 IME 뒤 긴
숫자의 두 번째 줄바꿈은 세 레이어가 모두 있는 최상단 revision에서 검증한다.

## 리뷰 지적 대응

[리뷰 코멘트](https://github.com/edwardkim/rhwp/pull/3946#issuecomment-5177715130)를 다음처럼 검토하고
보정했다.

### 1. 지속 입력 중 pagination 정지의 Known limit

200ms보다 짧은 간격의 입력이 계속되면 active restart timer가 반복 재예약된다. 이 구간에서는
pagination step과 공개 쪽수 갱신이 일시 정지하고 `hasPendingWork()` 때문에 120ms idle flush도
선점하지 않는다. 입력이 200ms 이상 쉬면 최신 revision에서 다시 시작하며 완료 뒤 stable-tail fast
path로 복귀한다. 이 사용자 관측 결과를 계획서와 Stage 5 기록에 명시했다.

pending 중 exact cursor query 약 49–51ms, Enter 전 navigation flush와 장기 대형 셀 아키텍처는 계속
이번 PR의 비범위다.

### 2. E2E MANIFEST·진입점·증적 성격

`issue-2214-page-local-repaint.test.mjs`의 MANIFEST 행을 #2214 focused 회귀와 #3815 Stack 통합
회귀를 함께 설명하도록 갱신했다. HWP/HWPX fixture, `e2e:issue-2214`, `e2e:issue-3815`, CI의
`node --check`, 로컬 production WASM·Chrome 증적을 구분했다. `rhwp-studio/package.json`에는 다음
재현 진입점을 추가했다.

```text
npm run e2e:issue-3815
```

파일 머리말에도 `--continuous-only`가 #3937/#3822 위의 #3815 로컬 통합 gate이고 CI는 구문만
검사한다고 기록했다. HWP/HWPX 2 / 2, p95, 115 → 116, revision 일치와 synchronous flush 0은
GitHub Actions 브라우저 결과가 아니라 로컬 production WASM + Chrome 근거다.

최신 devel에 존재했지만 MANIFEST에 빠져 있던 #3682 진단 프로브 행도 등록했다. 이는 새 MANIFEST
검사 정합성 보완이며 #3682 제품 동작은 바꾸지 않는다.

### 3. E2E 파일을 분리하지 않은 이유

`--continuous-only`는 #2214 fixture target, 문서 로딩, trace 설치·복구, cursor/tree snapshot,
composited canvas capture와 pagination 완료 판정을 그대로 사용한다. 시나리오만 옮기면 helper를 대량
복제하거나 공용 기반을 별도 리팩터링해야 해 이번 scheduler 보정의 위험과 범위를 넓힌다.

대신 같은 파일 안에서 모드를 분리하고 전용 npm script·MANIFEST 용도·header를 추가해 발견 가능성과
수명 차이를 기록했다. 계약이 독립 확대되거나 공용 helper 추출을 계획할 때 분리를 다시 검토한다.

### 4. `isActive()`와 delay 값 회귀 유지

`hasPendingWork()`는 production flush·restart 판단에 쓰며 queued begin과 stepping을 모두 포함한다.
`isActive()`는 stepping만 관찰해 queued begin과 실제 core 전진을 단위 테스트에서 구분한다. 의미가
달라 테스트 관찰 API로 유지했다.

100ms 최초 target, 200ms restart window, 25ms post-first-step gap은 성능·UX 측정과 Known limit을
정의한다. 값이 바뀌면 근거를 다시 검토해야 하므로 소스 회귀가 이름과 값을 함께 고정하도록 유지했다.
runner 단위 테스트는 실제 예약 delay와 상태별 적용 횟수를 별도로 검증한다.

### 5. cancel·publication 불변식과 절차 기록

`requestStart()`는 queued begin 전에 기존 core job을 취소한다. dequeue된 stale begin, active restart 뒤
stale step, settle callback, begin·step 예외의 단일 fallback을 단위 테스트가 고정한다. pending begin과
step은 공개 쪽수를 바꾸지 않고 complete에서만 115 → 116을 한 번 게시한다.

계획서·Stage 5, E2E MANIFEST/header/npm 진입점을 보정하고 이 review 문서를 추가했다. 보정이 단일
문서·E2E 배선 commit으로 추적 가능해 별도 `review_impl` 문서는 만들지 않는다.

## 검증

최신 부모 위의 리뷰 보정 layer head에서 다음 제한 검증을 통과했다.

| 검증 | 결과 |
| --- | --- |
| `node --test tests/deferred-pagination-runner.test.ts tests/input-edit-invalidation.test.ts` | 23 passed / 0 failed |
| `cd rhwp-studio && npx tsc --noEmit` | 통과 |
| `python3 scripts/check_e2e_manifest.py` | tracked 81 / manifest 81, 이상 없음 |
| `node --check rhwp-studio/e2e/issue-2214-page-local-repaint.test.mjs` | 통과 |
| `git diff --check` | 통과 |

Stack 전체 Studio unit 763 / 763도 이전 최신 기준 검증에서 통과했다. 이번 correction은 계획·작업 기록,
MANIFEST, E2E header와 npm 진입점만 바꾸며 scheduler 제품 코드는 변경하지 않는다.

## production WASM + Chrome 증적

최신 devel 기준 production WASM과 새 headless Chrome에서 HWP/HWPX `--continuous-only`를 각 한 번
실행했다.

| 형식 | 숫자 줄 전환 | 최종 숫자 | pending operation p95 | 최종 쪽수 | 결과 |
| --- | --- | ---: | ---: | ---: | --- |
| HWP | 11 / 69 | 73 | 50.1ms | 116 | GREEN |
| HWPX | 11 / 69 | 73 | 50.9ms | 116 | GREEN |

두 형식 모두 IME `ㅎ → 하 → 한` 뒤 긴 숫자가 pending 중 두 번 줄바꿈됐다. caret, model text,
visible ink와 layer tree가 최신 revision으로 일치했고 latest begin/final step revision은 132 / 132,
superseded publication과 synchronous flow flush는 0이었다. 이전 smoke의 50.2ms / 49.7ms 대비 -0.2% /
+2.4%로 ±10% gate 안이어서 형식별 current·80ms·250ms 3회 전체 측정은 반복하지 않았다.

이 수치는 로컬 production WASM + Chrome의 역사·회귀 근거다. Render Diff CI는 E2E의
`node --check`만 수행하므로 최신 PR head CI와 별도로 해석한다.

## 시각·fixture 판정

시나리오는 composited canvas에서 IME 확정 glyph, pending·complete 숫자 suffix, 셀 bounds와 line band를
검증한다. 부모 #3944에 보존한 대표 asset은 HWP에서 두 번째 숫자 줄바꿈과 최신 visible ink를 보여준다.

- `mydocs/pr/assets/pr_3944_issue1949_combined_hwp_final.png`
- SHA-256 `9329c26e6f4ce7d9a1e123928e360f94612e84ef2ecc07f10ff578dfb2fc33d2`

이번 correction은 renderer, layout, paint, sample과 golden을 바꾸지 않는다. 최신 PR head에서는 Render
Diff와 Canvas visual diff가 다시 통과해야 한다.

![PR 3946 combined HWP final](../assets/pr_3944_issue1949_combined_hwp_final.png)

## 위험과 후속 조건

- 200ms보다 짧은 입력 버스트 중 pagination step과 공개 쪽수 갱신이 멈춘다. 200ms 이상 쉬면 재개된다.
- pending operation 약 49–51ms의 exact cursor query는 이번 scheduler PR의 비범위다.
- Enter 전 navigation flush, 저장·인쇄 full pagination barrier는 정확성 경계로 유지한다.
- CellFlowTree, 영속 PageCheckpoint, viewport DisplaySnapshot과 복잡 HWP flow 확대는 #3743 후속이다.
- E2E는 #2214 helper에 결합돼 있다. 시나리오 확대나 공용 helper 추출 시 독립 파일을 재검토한다.
- `mydocs/orders/20260804.md`는 세 레이어가 순서대로 확장한다. devel 변경 시 상위 레이어를 restack한다.

## Stack merge 순서와 최종 조건

1. #3944를 `devel`에 merge한다.
2. #3945 base가 `devel`로 재지정된 뒤 최신 #3945 head의 CI와 mergeability를 다시 확인한다.
3. #3945를 merge한다.
4. #3946 base가 `devel`로 재지정된 뒤 최신 #3946 head의 CI, Render Diff, CodeQL과 mergeability를
   다시 확인한다.
5. 모든 최신-head gate가 성공하고 작업지시자가 승인한 뒤에만 #3946을 ready/merge한다.

이 PR의 통합 E2E는 세 레이어를 한 revision에서 함께 검증하는 근거이므로 중간 레이어를 건너뛰거나
순서를 바꾸지 않는다.

**현재 권고: 보정 restack·push 뒤 최신 head CI 대기.** scheduler 구현과 로컬 제한 검증은 통과했다.
최신 head의 GitHub Actions, Render Diff와 CodeQL이 모두 성공하고 작업지시자가 승인하면 collaborator
self-merge 후보로 판단할 수 있다.
