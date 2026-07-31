---
kind: review
status: active
canonical: mydocs/pr/archives/pr_3665_review.md
last_verified: 2026-08-01
---

# PR #3665 리뷰 기록

## 라우팅

```text
base route: collaborator self-merge
modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, multi_pr_update_branch.md
integration branch: integrate/lpaiu-planet-20260731
integration PR: #3671
```

원 PR은 개별 merge하지 않고, 최신 `upstream/devel`
`3b28ab597cb9c45c2d08c37fa16a4c9377db7d67` 위 통합 후보에 누적했다. 관련 실행 순서와
원 PR #3666의 경계 보정은 [공유 계획](pr_3665_review_impl.md)에 기록한다.

## PR metadata

| 항목 | 값 |
| --- | --- |
| PR | [#3665](https://github.com/edwardkim/rhwp/pull/3665) |
| 작성자 | `@lpaiu-cs` |
| base / source head | `devel` / `f4be36a1…` (작성 시점 참고) |
| 기능 commit | `24ad696b…`, 후속 P1 보정 `06d03b110…` |
| 통합 반영 | `9dac26c0e…`, `3a1db3234…` (`-x` 추적) |
| reviewer | `@edwardkim` 요청 완료 |
| 관련 issue | [#3315](https://github.com/edwardkim/rhwp/issues/3315), umbrella이므로 열린 상태 유지 |

source head의 CI는 lint, frontend gate, Native Skia, default-feature 8 shards, `Build & Test`,
CodeQL, Canvas visual diff가 성공한 상태로 확인했다. 이는 작성 시점 참고값이며, 통합 merge의
근거는 아래 #3671의 exact code candidate다.

## 변경 범위와 판정

- 좁은 flow-image 질의가 전체 render tree와 같은 canonical ancestor clip을 보존하도록 Rust query를
  보정한다. 좁은 질의가 없거나 해석에 실패하면 기존 full-tree 경로로 fail closed한다.
- Studio의 object URL cache를 문서 digest·generation에 귀속하고, 후속 commit은 URL 회수 책임을
  소비 시점이 아닌 `CanvasView.prepareDocumentLoad()`의 문서 (재)로드 경계로 옮긴다. 새 문서가
  flow 그림을 전혀 요청하지 않거나 digest가 없을 때도 이전 URL은 즉시 revoke하고, 같은 문서 재로드만
  캐시를 보존한다.
- cache 경계가 아직 정해지지 않으면 `urlFor()`는 `null`을 반환해 종전 base64/full-tree fallback을
  사용한다. 따라서 캐시 수명 보정이 그림을 조용히 누락시키는 경로가 되지 않는다.

독립 코드 리뷰에서 차단 결함은 발견하지 못했다. 다만 이후 새 UI가 `WasmBridge.loadDocument()`를 직접
호출한다면 같은 문서 경계 호출 계약을 지켜야 한다는 P2 유지보수 경계가 있다. 현행 load/create 경로는
그 순서를 지키며, source guard와 회귀가 이를 보호한다.

## 로컬 검증

| 검증 | 결과 |
| --- | --- |
| `cargo test --profile release-test --test issue_3315_flow_image_narrow_query` | 5 passed |
| `npx tsc --noEmit` (`rhwp-studio`) | 성공 |
| `npm test` (`rhwp-studio`) | 705 passed / 0 failed |
| `npm run build` (`rhwp-studio`) | 성공 |
| #3671 code candidate full CI | 성공 — frontend gate, Canvas visual diff 포함 |

Studio UI/WASM shell은 in-app browser에서 로드·접근성 snapshot까지 확인했다. 다만 이 환경의 browser
file chooser는 local fixture 주입을 허용하지 않아, 실제 A(대형 flow 그림)→B(그림 없는 문서) DOM 전환은
브라우저로 재현하지 못했다. 이를 성공으로 확대하지 않았고, no-query revoke·null identity·same identity
보존·generation 변화·fallback은 위 unit/Studio 회귀와 exact CI frontend gate로 확인했다.

PDF/SVG 기준 비교는 표 분할 layout을 바꾸는 #3666에 적용했다. #3665의 URL lifetime은 같은 문서의
시각 기하를 바꾸지 않는 Studio resource-lifecycle 변경이므로, 그 sweep을 이 PR의 PDF 정합 주장으로 쓰지
않는다.

## CI와 권고

통합 code candidate `3a1db3234ff80466bc8dfd49364de49f66db8e0d`의 CI preflight, lint, frontend
package gates, Native Skia, test archive, default-feature 8 shards, `Build & Test`, CodeQL,
Canvas visual diff는 모두 성공했다. 이 기록은 code CI 성공 뒤 추가하는 review-only tail이므로,
최종 merge 조건은 최신 #3671 head의 preflight·`Build & Test` aggregate 성공, `CLEAN`·`MERGEABLE`
재확인, 그리고 작업지시자의 자동 승인 범위다. 조건 충족 전 권고는 **보류**, 충족 후 권고는 **통합 merge**다.
