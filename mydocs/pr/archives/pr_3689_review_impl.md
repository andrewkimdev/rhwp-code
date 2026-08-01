---
kind: pr-review-implementation-plan
status: code-ci-success-docs-tail-pending
integration_pr: 3742
canonical: mydocs/pr/archives/pr_3689_review_impl.md
last_verified: 2026-08-01
---

# Kevin #3689–#3735 · planet6897 #3736 통합 반영 기록

## 기준점과 후보 경계

최신 `upstream/devel` `fe9749d542f46643e408c23878229c326e341363` 위
`review/kevin9327-20260801`에 contributor 기능 commit만 누적했다. 최종 code candidate는
`b1e9619433bd9f068a361ddfb42ea0138f0077d1`이며, integration PR은
[#3742](https://github.com/edwardkim/rhwp/pull/3742)다. 원 contributor branch는 rewrite·삭제하지
않는다.

## 원 PR별 적용·제외 경계

| 원 PR | 처리 | #3742의 기능 경계 |
| --- | --- | --- |
| #3689 | 적용 | digest v2 재구성·sections/pages (`5d35bbdde`, `0fd231b94`); fixture 보정 `b1e961943` |
| #3692 | 적용 | bindings/IR versioning 문서 (`0cc37c5fa`) |
| #3698 | #3716으로 대체 | 독립 merge하지 않고 changedPages 재적층에 포함 |
| #3700 | 적용 | MCP stdin stream 경계 (`106b34536`, `cd99dc216`) |
| #3701 | #3716으로 대체 | nextCall stack은 #3716으로 대체 |
| #3704 | 적용 | 세션 재페이지네이션 (`c1dfa8c6`, `2d2c1ad0`, `474e5db59`) |
| #3705 | #3716으로 대체 | edit verify stack은 #3716으로 대체 |
| #3708 | 적용 | export-doclang JSON (`ef186322f`) |
| #3710 | #3716으로 대체 | run plan stack은 #3716으로 대체 |
| #3711 | 적용 | Unicode confusable 진단 (`e1b5431db`, `30d88fddc`) |
| #3713 | 적용 | HWP3 FileHeader version (`5f28d0147`) |
| #3714 | 적용 | dump-pages JSON (`9c042dc3e` 및 기록) |
| #3716 | 적용 | changedPages canonical stack (`e38b2d997`, `a45487bab`, `2ac80b996`) |
| #3717 | 적용 | info/batch title (`f3fd5fac5` 및 기록) |
| #3718 | 적용 | nested-table tail paragraph (`cd12815e9`) |
| #3720 | 적용 | MCP JSON-RPC frame/version (`762d761ae`) |
| #3721 | 적용 | ClickHere 저장 보존 (`5e6c2b710`) |
| #3722 | 적용 | signed `hp:offset` (`3eb9191a0`) 및 baseline 보정 |
| #3723 | 적용 | readonly source overwrite guard (`ec3ef4c35`) |
| #3724 | 적용 | declared MCP argument wiring (`e22e60dfc`) |
| #3725 | 제외 | #3724와 patch-equivalent duplicate |
| #3726 | 적용 | split page base (`f89f88689`) |
| #3727 | 적용 | page/search argument validation (`ffeaa00e3`) |
| #3728 | 적용 | set-cell newline/tab (`5c91b849e`) |
| #3729 | 적용 | save IR snapshot (`a4c4ab19e`) |
| #3730 | 적용 | MCP resources (`d5272294d`) |
| #3731 | 적용 | capabilities drift guard (`2d4c5d57e`) |
| #3732 | 적용 | gen-pua overwrite/diagnostic exit (`7fc39d63c`) |
| #3733 | 적용 | HWP5 probe exit contract (`92d5c9cff`) |
| #3734 | 적용 | remaining diagnostic exit contract (`35ffe2e21`) |
| #3735 | 적용 | batch convert NDJSON (`9b78e156e`) |
| #3736 | 적용 | `@planet6897` 별도 작성자의 HWP3 roundtrip/endnote (`527e0368f`) |

원 PR #3698·#3701·#3705·#3710은 기능을 잃지 않고 #3716의 canonical stack으로 재적층했으며,
#3725만 동일 patch라 이중 적용하지 않았다. 누적 중 textual conflict는 없었다.

## 메인터너 보정과 롤백 경계

- #3722의 unsigned offset 보정 뒤 IR sweep baseline이 실제 관찰 순서와 달라졌다. `c3f235ea6`는
  새 결과를 가리는 것이 아니라 4 additions/27 deletions의 A/B 차이를 고정한다.
- #3731 선언 capability와 실제 CLI의 drift는 `a0e548e17`와 `f0b96e87b`에서 기능과 별도로
  교정했다. #3734/#3735의 diagnostic exit, global auth, out-dir/verify, case collision도 같은
  `f0b96e87b` 경계에 명시한다.
- #3689 digest v2 test는 #3715 이후 실제 조문 fixture가 필요해 `b1e961943`로 보정했다. 이는
  원 `hwp3-sample.hwp`가 독립 표제를 clause로 오인하지 않는 현행 계약을 반영한다.
- renderer 회귀 #3718은 안정 PNG와 HWP 2020 PDF 근거를 `pr_3718_review.md`에 남긴다. 이 증적은
  tail paragraph 보존만 판단하며 전체 PDF 동등성 주장이 아니다.
- rollback은 contributor history를 rewrite하지 않고 #3742 head에 정정/revert commit을 더하는
  방식으로 한다. docs tail만 실패하면 code candidate는 보존하고 문서 전용 follow-up으로 고친다.

## 검증과 남은 순서

`CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/review-kevin9327-20260801 cargo test --profile
release-test --tests`는 최종 exit 0이었고, fmt·clippy `-D warnings`도 success다. [CI run
30711901379](https://github.com/edwardkim/rhwp/actions/runs/30711901379)의 lint, Native Skia,
default-feature 8 shards, `Build & Test`와 CodeQL/Canvas visual diff도 성공했다.

1. 이 archive set, 이 기록, 오늘할일, #3718 PDF/PNG만 담은 single-parent docs-only commit을
   `b1e961943` 위에 만든다.
2. candidate→docs head의 모든 path에 대해 LFS attribute와 `git lfs status`를 판독하고, 비-LFS일
   때만 `GIT_LFS_SKIP_PUSH=1` dry-run 뒤 같은 remote branch에 push한다.
3. 최신 docs head의 preflight와 required `Build & Test` aggregate가 review-only fast-pass A로
   success인지 확인한다. base drift·aggregate failure·fast-pass 거부면 full CI로 되돌린다.
4. `CLEAN`·`MERGEABLE`과 required checks를 재확인한 뒤 #3742 하나만 merge한다. 외부 review와
   contributor 감사/supersede comment는 현재 지시대로 이후 실제 LF body로 게시한다.
