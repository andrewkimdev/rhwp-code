---
kind: review
status: code-ci-success-review-only-fast-pass-pending
canonical: mydocs/pr/archives/pr_3685_review.md
last_verified: 2026-08-01
---

# PR #3685 검토·메인터너 보정 기록 — HWP3 변환본 한컴 열기 저장 계약

## 결론과 범위

[PR #3685](https://github.com/edwardkim/rhwp/pull/3685)는 HWP3→HWP5 변환본을 실제 한글이
열 수 있게 하는 저장 계약 보정이다. 작성자 `@planet6897`은 재기여자이며 관련 이슈는
[#3676](https://github.com/edwardkim/rhwp/issues/3676)이다.

최종 source/test candidate `C` = `1aa0aadbe9c5136ea16a2fbcd0745ffad841492b`는 로컬
focused·전체 integration·Clippy, Windows의 실제 한글 외부 오라클, GitHub의 최신 full CI를
모두 통과했다. 이 문서와 구현 기록, 오늘할일만 담은 후속 Markdown tail을 원 contributor
브랜치에 올린 뒤에는 **review-only fast-pass A**의 preflight와 required `Build & Test`
aggregate까지 성공해야 승인·merge할 수 있다.

시각 sweep/PDF 증적은 적용하지 않는다. 변경 대상은 renderer·typesetter·페이지 layout이 아니라 HWP
바이너리 저장 계약과 Windows 한글 COM 외부 오라클이다. 이 경우의 정답지는 PDF 겹침 비교가 아니라
실제 한글의 독립 인스턴스 열기다.

## PR metadata와 commit 경계

| 항목 | 값 |
| --- | --- |
| PR / 작성자 | [#3685](https://github.com/edwardkim/rhwp/pull/3685) / `@planet6897` |
| contributor 원 source head | `2f81e673308b5f253528541c3963e452e1cf2e41` |
| 최종 code candidate `C` | `1aa0aadbe9c5136ea16a2fbcd0745ffad841492b` |
| `C` 직전 원 contributor remote head | `de75f2d5df508d07447f9e26d213d2f38e09817c` |
| base | `devel` `f80b910aabeda5939972752719b0916129eb3a53` (이 기록 작성 시점 참고값) |
| code CI | [CI run 30701824447](https://github.com/edwardkim/rhwp/actions/runs/30701824447) success |
| CodeQL | [CodeQL run 30701824446](https://github.com/edwardkim/rhwp/actions/runs/30701824446) success |

`C`에는 공개 HWP3 저장 경로의 byte contract를 고정한 `7cbaee46c`, HWPX PBF overlay를 저장 뒤
복원하는 `9204055a2`, 평문·비밀번호 HWP 저장 경로가 adapter 위임을 명시적으로 유지하게 한
`1aa0aadbe`가 포함된다. 그 이전 maintainer tail의 Windows 프로세스 격리, HWP3 PBF·중첩
geometry/local-file-version 보정도 그대로 보존한다. contributor의 원 commit을 rewrite하거나
force-push하지 않았고, `C`는 `de75f2d5`의 descendant로 원 contributor head에 fast-forward했다.

## 발견한 경계와 최종 보정

### HWP 출력의 세 PAGE_BORDER_FILL과 HWPX live IR 보존을 함께 만족해야 한다

중간 보정은 HWPX의 원래 단일 `BOTH` `pageBorderFill` XML을 지키기 위해 세
`PAGE_BORDER_FILL` materialization을 HWP3 source로만 제한했다. 그러나 정확한 Windows 외부
오라클에서 HWPX→HWP 출력도 세 record가 필요하다는 반례가 나왔다. 내부
`--verify --verify-pages`는 성공했지만, 당시 HWPX 변환본을 한글로 열면 `RESULT 0 -1 -1`이었다.

최종 보정은 출처와 관계없이 **HWP 파일에는 구역마다 세 PBF record**를 materialize한다. 동시에
HWPX source라면 저장 직전에 root `SectionDef`와 serializer가 읽는 모든
`Control::SectionDef`의 PBF extras를 snapshot하고, serialize 성공·실패와 무관하게 저장 직후
원래 overlay를 restore한다. 따라서 HWP 출력은 한글 호환 계약을 충족하고, 같은 `DocumentCore`를
이어 HWPX로 저장해도 단일 `BOTH` 문서에 없던 EVEN/ODD XML을 남기지 않는다.

평문과 비밀번호 HWP public export는 모두 이 adapter를 직접 호출한다. 저장 뒤 live IR 불변과
passthrough invalidation의 정적 delegation contract도 별도 회귀로 고정했다.

### 기존 P1 보정도 최종 candidate에 포함한다

- HWP3 실제 paragraph container 안의 caption·HiddenComment·master page·Chart/OLE caption까지
  geometry/crop/local-file-version 정규화 walker가 도달하도록 보정했다.
- Windows batch 도구는 `Hwp(new=True, visible=False)`만 만들며 전역 `taskkill`이나 기존 사용자
  한글 인스턴스 attach를 사용하지 않도록 보정했다.

## 검증 근거

| 항목 | 결과 |
| --- | --- |
| PBF·공개 저장 경로 regression | `CARGO_TARGET_DIR=target/review-planet6897-20260801-final CARGO_INCREMENTAL=0 cargo test --profile release-test --test issue_3676_hwp3_convert_hancom_openable -- --nocapture` → 5 passed, 0 failed |
| passthrough invalidation static contract | `CARGO_TARGET_DIR=target/review-planet6897-20260801-final CARGO_INCREMENTAL=0 cargo test --profile release-test --test issue_2724_passthrough_invalidation_guard -- --nocapture` → 5 passed, 0 failed |
| 전체 Rust integration | `CARGO_TARGET_DIR=target/review-planet6897-20260801-final CARGO_INCREMENTAL=0 cargo test --profile release-test --tests` → 최종 exit 0 |
| Clippy | `CARGO_TARGET_DIR=target/review-planet6897-20260801-final CARGO_INCREMENTAL=0 cargo clippy --all-targets -- -D warnings` → exit 0 |
| Windows 도구 안전 회귀 | `python3 -m unittest scripts/tests/test_hwp3_convert_openable.py` → 2 passed |
| Windows release-test binary | exact `C`에서 `cargo build --profile release-test --bin rhwp` → exit 0 |
| HWP3 외부 오라클 | `samples/hwp3-sample.hwp` → HWP5 `--verify --verify-pages` 성공, 16쪽·IR diff 없음, 독립 `Hwp(new=True)` 실제 열기 `RESULT 1 16 16` |
| HWPX PBF 외부 오라클 | `samples/task2093/saved_single_line_spacing_after.hwpx` → HWP5 `--verify --verify-pages` 성공, 1쪽·IR diff 없음, 독립 `Hwp(new=True)` 실제 열기 `RESULT 1 1 1` |
| 사용자 한글 프로세스 보호 | `win10-ted`에서 기존 `Hwp`/`HwpApp` process 수 0을 전후 대조했고 전역 `taskkill` 없이 0 유지 |
| GitHub code CI | CI preflight, lint, build archive, Native Skia, default-feature 8 shards, `Build & Test` 모두 success; CodeQL Python/Rust/JavaScript 분석도 success |
| LFS 사전 판독 | code/test 변경 3개 파일의 `filter`가 모두 `unspecified`, `git lfs status` 대상 없음 |

## 현재 판단

**승인 권고, 단 Markdown tail의 fast-pass 성공 전에는 merge 보류.** `C`의 source/test 검증은
완료됐고, 이후 tail은 이 archive review·implementation record·오늘할일만 바꾸는 single-parent
문서 commit이다. push 직전 contributor remote head와 LFS 속성을 다시 고정하고, push 뒤 candidate
`C`의 green CI 및 새 docs head의 preflight·required aggregate·`CLEAN`/`MERGEABLE`을 재확인한다.
그 조건을 충족하면 LF가 실제로 들어 있는 body file로 approve review를 게시하고 merge한다.
