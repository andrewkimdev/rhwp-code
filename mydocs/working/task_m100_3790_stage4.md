# task_m100_3790 Stage 4 — Rust·Native Skia 조건화

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **브랜치**: `issue-3790-stage4-rust-native`
- **분기 기준**: `upstream/devel` `9aa0ec8b61fad2b1401341af36b562fae1529813`
- **상태**: 로컬 구현·집중 검증 완료, push·draft PR 생성 승인 대기
- **기록일**: 2026-08-04 KST

## 선행 canary

Stage 3 canary PR #3951의 selective run은 `frontend_mode=unit`, `render_required=false`를 판정했고 unit
gate만 59초에 실행했다. 같은 SHA의 수동 full에서 package gate는 2분 47초, Canvas는 5분 59초에
성공했으므로 Stage 3의 직접 runner time 절감은 7분 47초다. 수동 full 전체를 중단시킨 기존 cold
release archive 30분 timeout은 #4029에서 별도로 추적하며 Stage 4 영향축 판단 근거와 섞지 않는다.

## 변경 요약

preflight의 `rust_required`와 `native_skia_required`를 CI job 조건과 aggregate 진리표에 연결했다.

| 영향축 | Rust lint·3 builders·4 workers | Native Skia |
| --- | --- | --- |
| `rust=true`, `native=true` | 모두 `success` | `success` |
| `rust=true`, `native=false` | 모두 `success` | `skipped` |
| `rust=false`, `native=true` | 모두 `skipped` | `success` |
| `rust=false`, `native=false` | 모두 `skipped` | `skipped` |

worker는 해당 builder의 성공에 더해 Native job이 영향축과 일치하는 `success|skipped`인지 확인한 뒤
실행한다. aggregate는 각 job 결과를 개별적으로 검증하며 알 수 없는 축 값이나 부분 성공을 실패시킨다.
review-only fast-pass와 Stage 3의 frontend `none|unit|package` 진리표는 유지한다. CodeQL 언어 조건화는
Stage 5까지 기존 동작을 유지한다.

## 분류 경계 보완

Native Skia job의 실제 `cargo test --test` 대상과 classifier 경로를 전수 대조했다. 일반 Rust 경로로
분류되던 아래 두 통합 테스트 변경은 Native 검증을 건너뛸 수 있어 classifier v2의 독립 경계로 고정했다.

- `tests/issue_2225_missing_picture_placeholder.rs`
- `tests/render_p37_direct_pdf_export.rs`

두 파일은 `rust_required=true`, `native_skia_required=true`, `render_required=false`로 판정한다. font 같은
비-Rust render 입력도 Native Skia가 필요하므로 `rust=false`, `native=true` 조합은 정상 조합으로 지원한다.
workflow·classifier·Cargo·WASM·rename·미분류 변경은 계속 full로 닫히므로 Stage 4 PR 자체는 원격 CI에서
전체 lane을 실행한다.

## 검증

| 검증 | 결과 |
| --- | --- |
| `actionlint .github/workflows/ci.yml .github/workflows/render-diff.yml` | 통과 |
| `node --check scripts/ci-impact-classifier.cjs` | 통과 |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 25/25 통과 |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py scripts/tests/test_render_diff_workflow.py` | 21/21 통과 |
| aggregate shell 진리표 실행 — frontend-only, Rust 비렌더, Rust render, 비-Rust Native | 모두 통과 |
| aggregate shell 불일치·unknown 축 입력 | 모두 의도대로 실패 |
| `git diff --check` | 통과 |

장시간 Rust 전체 CI는 workflow/classifier 변경으로 인해 draft PR 자체가 fail-closed full lane에 들어가므로
원격에서 확인한다. 로컬에서는 새 조건과 aggregate 진리표, classifier/job 소유 경계에 집중했다.

## 다음 단계

1. 사용자 승인 뒤 브랜치를 push하고 Stage 4 draft PR을 만든다.
2. 원격 full CI에서 기존 Rust·frontend·Native Skia·Render Diff·CodeQL 전체 회귀가 없는지 확인한다.
3. collaborator 직접 merge 절차에 따라 review 문서를 PR head에 커밋하고 최신 CI를 통과시킨다.
4. merge 뒤 frontend-only canary에서 Rust lint·세 builder·네 worker·Native Skia가 모두 `skipped`되고
   `Build & Test` aggregate가 성공하는지 실측한다.
5. #3810 직후 4.73GB cache 기준선과 다음 cache sweep 직후 총량을 대조한 뒤 Stage 5 CodeQL 언어
   조건화로 진행한다.
