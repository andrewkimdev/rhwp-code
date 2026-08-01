---
kind: review
status: active
canonical: mydocs/pr/archives/pr_3662_review.md
last_verified: 2026-08-01
---

# PR #3662 검토 기록 — #3315 이미지 바이트 계약 가드

## 라우팅

```text
base route: collaborator self-merge
modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, multi_pr_update_branch.md,
  review_only_fast_pass.md
integration branch: integrate/lpaiu-cs-20260801
integration PR: #3680
```

원 PR은 개별 merge하지 않고 최신 `upstream/devel`
`c588c8240331a181271c6551e124aa1ff770d900` 위 [#3680](https://github.com/edwardkim/rhwp/pull/3680)에
`-x`로 누적했다. #3672는 이 통합 검토 도중 별도로 merge되어 base에 포함됐으므로, 그 PR의 사후
검토와 RawSvg 안전 보정은 [#3672 review](pr_3672_review.md)에 분리한다.

## PR metadata

| 항목 | 값 |
| --- | --- |
| PR | [#3662](https://github.com/edwardkim/rhwp/pull/3662) |
| 작성자 | `@lpaiu-cs` (재기여자) |
| base / source head | `devel` / `0f6ccaa61a8d5bfe66ab423a0d6b5fc9bd0a77bb` (작성 시점 참고) |
| source 기능 commit | `f07d86a5c5c2fbcbffc27cd2044ebfbcd3ae0a80`, `0f6ccaa61a8d5bfe66ab423a0d6b5fc9bd0a77bb` |
| 통합 반영 | `fc06bf23dae767f51699e241c66bae3ab390f37b`, `2deb282081932d6ca9e459f9549cdd0830db9230` (`-x` 추적, author 보존) |
| reviewer | `@edwardkim` 요청 완료 |
| 관련 issue | [#3315](https://github.com/edwardkim/rhwp/issues/3315) — Track 1–4 umbrella, 열린 상태 유지 |

원 PR의 source base `3b28ab597`은 오래됐고, source head의 CI·mergeability는 작성 시점 참고값일 뿐이다.
최종 merge 근거는 최신 #3680 code candidate와 그 review-only tail의 CI다.

## 변경 범위와 검토 판정

- Rust paint JSON의 `imageBytes` 계약을 "바이트 동일"이라는 과도한 이름 대신 기존 필드 보존과
  schema minor 21의 additive metadata 계약으로 고정한다.
- PNG·BMP·TIFF·회색/색 JPEG·워터마크 JPEG·효과 PNG·손상 BMP rollback에 대해 source image key와
  방출 JSON 바이트가 같은 변환 결과를 가리키는지 검증한다. 실제 bake가 일어난 watermark case를
  만들고, 두 variant가 실제로 다르다는 전제까지 고정한 점이 특히 유효하다.
- `byKey` 모드의 최상위 `imageBytes`는 요청 모드이고, key 없는 합성 그림의 실제 payload 여부는
  op별 `imageBytesOmitted`가 말한다는 경계를 문서와 회귀로 분리한다.
- #3350 rollback은 nested catch의 discard가 아니라 성공 경로에서 discard 후 rethrow하는지를 좁혀
  검증한다. 이로써 성공 경로 누수 회귀를 놓치던 문자열 guard의 범위를 바로잡는다.

코드 검토에서 차단 결함은 발견하지 못했다. 다만 public fixture가 없는 PCX와 watermark 전체
round-trip은 이 PR의 공개 커버리지 밖이다. 따라서 현재 증거를 모든 이미지 형식·모든 watermark
경로의 완전한 동등성으로 확대하지 않는다.

## 로컬·시각 검증

| 검증 | 결과 |
| --- | --- |
| `cargo fmt --check` | 성공 |
| `cargo test --profile release-test --test issue_3315_image_bytes_by_key` | 7 passed |
| `cargo test --profile release-test --tests` | 최종 exit 0 |
| `cargo clippy --profile release-test --all-targets -- -D warnings` | 성공 |
| Native Skia 공식 3종 | 58/58, 2/2, 4/4 passed |
| Rust doc test | 4 passed, 2 ignored |
| `wasm-pack build --target web --out-dir pkg` | 성공 |
| Studio | `npx tsc --noEmit`, `npm test` 716 passed |

검토 전용 Cargo 실행은 `CARGO_INCREMENTAL=0`,
`CARGO_TARGET_DIR=target/review-lpaiu-cs-20260801`로 분리했다.

이 통합 후보는 `src/renderer`·WASM·Studio canvas에도 닿으므로 browser canvas 경로를 별도로 확인했다.
RawSvg는 400ms 후 색상 픽셀 1.757%로 임계 0.3%를 넘겼고, chart A/B는 각각 1.824%/2.697%, 같은
문서 `refreshPages()` 후 B도 2.697%였다. 이는 비동기 decode 재렌더를 직접 다루는 변경에 맞춘
Chrome E2E 근거다. HWP/HWPX fixture·page geometry·기준 PDF는 바뀌지 않았으므로 PDF/SVG visual
sweep을 수행하거나 전체 HWP/PDF 동등성을 주장하지 않았다. 최신 CI의 Canvas visual diff도 성공했다.

## CI와 권고

최신 code candidate `71ccfeaaa6c911340d18371e348a6b53ff33f4a0`의
[CI](https://github.com/edwardkim/rhwp/actions/runs/30687707964),
[CodeQL](https://github.com/edwardkim/rhwp/actions/runs/30687707945),
[Canvas visual diff](https://github.com/edwardkim/rhwp/actions/runs/30687707952)는 모두 성공했다.
CI에는 lint, frontend gate, Native Skia, test archive, default-feature 8 shards, `Build & Test`가
포함된다.

이 기록은 code CI 성공 뒤 same-PR head에 추가하는 single-parent review-only tail이다. 따라서
최종 merge 조건은 최신 #3680 head의 fast-pass preflight와 `Build & Test` aggregate 성공,
`CLEAN`·`MERGEABLE` 재확인, 그리고 작업지시자의 기존 자동 승인 범위다. 조건 충족 전 권고는 **보류**,
충족 후 권고는 **#3680 통합 merge**다.
