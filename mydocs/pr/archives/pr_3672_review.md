---
kind: review
status: active
canonical: mydocs/pr/archives/pr_3672_review.md
last_verified: 2026-08-01
---

# PR #3672 사후 검토 기록 — retry narrowing과 RawSvg 안전 경계

## 라우팅과 상태

```text
base route: collaborator self-merge
modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, multi_pr_update_branch.md,
  review_only_fast_pass.md
review correction PR: #3680
```

[#3672](https://github.com/edwardkim/rhwp/pull/3672)은 통합 검토 중
2026-08-01 06:12:20Z에 [merge commit `c588c8240`](https://github.com/edwardkim/rhwp/commit/c588c8240331a181271c6551e124aa1ff770d900)으로
별도 merge됐다. 따라서 이 문서는 원 기능의 사후 review와, 원 기능만으로 남는 RawSvg P1을
#3680에서 안전하게 보정하는 근거를 함께 남긴다. merge된 원 PR을 supersede close하거나 contributor
author를 다시 적용하지 않는다.

## PR metadata

| 항목 | 값 |
| --- | --- |
| PR | [#3672](https://github.com/edwardkim/rhwp/pull/3672) |
| 작성자 | `@lpaiu-cs` (재기여자) |
| source base / head | `5891600372d847fc0a000ba3f6ebb9e5861e1f03` / `81306ef0f26ff6c6d43dae65e8c9a177ad01b66f` (작성 시점 참고) |
| source 기능 commit | `81306ef0f26ff6c6d43dae65e8c9a177ad01b66f` |
| 실제 반영 | `c588c8240331a181271c6551e124aa1ff770d900` |
| review P1 보정 | #3680의 `01a572c32a9363ff2ebdfe1bf2dae1adcfb96529` |
| reviewer | `@edwardkim` 요청 완료 |
| 관련 issue | [#3315](https://github.com/edwardkim/rhwp/issues/3315) — 열린 umbrella 유지 |

## 변경 범위와 보정 판단

원 기능은 매 편집마다 `resetImageRetryState()`로 전체 retry state를 비우던 방식을 없애고, raster
image source key·문서 digest/generation을 retry key로 사용해 같은 그림 페이지의 중복 재렌더를 막는다.
판정 재료가 없을 때 `null`을 돌려 재사용을 포기하는 fail-closed 경계, document identity까지 키에
넣은 판단은 타당하다.

다만 RawSvg 차트/OLE는 raster source-image key 집합에 들어가지 않고, compact overlay는 RawSvg의
내용이 아니라 개수만 전달한다. 같은 문서·같은 개수에서 browser decode cache가 비워지거나 RawSvg가
갱신되면 기존 key가 같아 `scheduleReRender()`가 조기에 return할 수 있다. 첫 draw가 비동기 decode보다
앞서면 다음 안전망 없이 빈 canvas가 고착할 수 있으므로 이는 P1이다.

#3680의 `01a572c32`은 `rawSvgCount > 0`이면 retry key를 `null`로 해 해당 case의 timer/fallback을
항상 다시 무장한다. 일반 raster 이미지의 key 기반 재사용은 유지한다. 따라서 원 성능 개선을 넓게
되돌리지 않고 RawSvg가 판정 재료가 될 수 없는 경계만 fail closed한다.

잔여 P2는 `imageRetryCounts`가 page pool release에서는 지워지지 않고 `dispose()`에서만 정리되는 점이다.
긴 그림 문서 탐색에서 page key가 누적될 수 있으므로, page release cleanup 또는 bounded LRU는 후속으로
다뤄야 한다. 이 문서는 그 P2가 해소됐다고 주장하지 않는다.

## 로컬·시각 검증

검토 전용 target에서 `cargo fmt --check`, full
`cargo test --profile release-test --tests`(최종 exit 0), clippy, Native Skia 3종, Rust doc test,
WASM build, TypeScript와 Studio 716 test를 통과했다.

RawSvg 관련 사용자-visible 경로는 PDF/SVG fixture 비교 대신 browser Canvas E2E로 확인했다. 400ms 후
RawSvg의 색상 픽셀은 1.757%(임계 0.3% 초과)였고, chart 문서 A/B 및 같은 문서
`refreshPages()` 후에도 ink가 남았다(A 1.824%, B 2.697%). page geometry·기준 PDF·fixture를 바꾸는
PR이 아니므로 이 증적을 HWP/PDF 전체 동등성으로 확대하지 않는다. 최신 head의 Canvas visual diff도
성공했다.

## CI와 권고

#3672의 source 기능은 이미 `devel`에 merge됐지만, **RawSvg 보정이 포함된 #3680이 merge되기 전에는
원 기능을 완결로 선언하지 않는다.** 최신 code candidate
`71ccfeaaa6c911340d18371e348a6b53ff33f4a0`의 CI·CodeQL·Canvas visual diff는 모두 성공했다.
review-only tail의 fast-pass 성공과 latest head `CLEAN`·`MERGEABLE` 확인 뒤 #3680 merge를 권고한다.
