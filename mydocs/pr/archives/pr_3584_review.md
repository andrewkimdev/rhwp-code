---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3584 리뷰 — 쪽 번호 폭을 실제 폰트 메트릭으로 측정 (#3048)

- PR: [#3584](https://github.com/edwardkim/rhwp/pull/3584) / Related [#3048](https://github.com/edwardkim/rhwp/issues/3048)
- 작성자: `planet6897`
- 역할: maintainer 일반 경로 + local_validation + visual_fixture_evidence(수치 증적)

## 라우팅과 작성 시점

```text
base route: maintainer_general.md / modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md
current head: 7035573db / MERGEABLE / behind (참고값)
규모: 2 files, +16/−10 — renderer/layout.rs(+13/−7) + golden_svg/form-002(3줄)
```

## 변경 범위와 수용 판단

1. `build_page_number()` 의 폭 어림(`문자수 × 크기 × 0.6`)을 제거하고, 렌더에 쓰는
   `TextStyle` 을 먼저 만들어 **같은 스타일로** `estimate_text_width` 실측 — 측정과 렌더가
   같은 값을 쓰므로 구조적으로 어긋날 수 없다. 하드코딩 계수 제거라는 점에서 렌더링
   보정 원칙(근거는 문서 속성/실측)에 정확히 부합.
2. 크기 상수 10pt 는 유지 — 이슈의 "6.0~7.1pt 관측"이 쪽 번호 아닌 본문 숫자 오탐이었음을
   한컴 PDF 275건 재측정(쪽마다 +1 증가·x ±3pt 고정 계열만 인정)으로 확증.
3. golden form-002 갱신: `- 1 -` 3글리프 +1px(96dpi) = +0.75pt(72dpi) — PR 실측
   (282.64→283.39pt, 한컴 285.09pt 방향)과 산술 일치.

**수용 판단: merge 권고.**

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| 충돌 simulation (devel merge) | clean | — |
| `cargo test --profile release-test --tests` | 370 바이너리 전부 ok (exit 0) | golden 갱신 포함 전체 정합 |
| Native Skia 3종 (skia --lib / issue_2225 / render_p37) | 58 + 2 + 4 passed | renderer 공식 회귀 범위 green |
| wasm 빌드 (Docker 표준) | 성공 (exit 0) | WASM 경계 무회귀 |
| fmt / clippy `-D warnings` | 둘 다 통과 | — |
| PR head CI | 전 check green | — |
| 시각 증적 | 한컴 PDF 실측 좌표(biz_plan.hwp p3: 한컴 285.09 / 전 282.64 / 후 283.39pt) + 275건 오라클 재측정 + golden 정합 | 수치 증적 충분, 실물 시각 판정 필요 여부는 작업지시자 결정 |

## 최종 권고

**merge 권고.** ±1px 수준의 위치 정밀 수정으로 한컴 PDF 실측 좌표가 직접 증적 —
OVL-step 선택 기준상 별도 시각 판정 없이 수용 가능하다고 보나, 최종 판단은 작업지시자
권위에 따른다.
