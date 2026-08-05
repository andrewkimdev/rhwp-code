# task_m100_3790 Stage 4 — Rust·Native Skia 조건화

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **브랜치**: `issue-3790-stage4-rust-native`
- **최신 동기화 기준**: `upstream/devel` `d3fb9de7c0c0648e3d8126c25467e2c78a054337`
- **첫 devel merge head**: `b0be8673149bbd00ebb67f6d5e62b70025cfa612`
- **최종 code head**: `5eeab15fd291b2b4b27d3b8a77498fcc0ca5723b`
- **상태**: PR #4032 review F1–F6 보정 완료, 최종 code head full CI 통과, ready 전환과 review-only
  기록 push 단계
- **기록일**: 2026-08-05 KST

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

두 파일은 `rust_required=true`, `native_skia_required=true`, `render_required=false`로 판정한다.

review F1에서 default-feature 테스트가 직접 소비하는 데이터 경계를 추가로 확인했다.

- `ttfs/**`·`tests/fixtures/fonts/**`의 `.otf|.ttc|.ttf|.woff|.woff2`
- `samples/render-p35-font-native-bitmap.hwpx`

이 경로는 `rust=true`, `native=true`, `render=true`, data-only이므로 `codeql=none`으로 판정한다.
`assets/fonts/**`, render 생성 Python, render 문서는 `rust=false`, `native=true`를 유지해 불필요하게
Rust lane을 넓히지 않는다. workflow·classifier·Cargo·WASM·rename·미분류 변경은 계속 full로 닫힌다.

## review F1–F6 보정

| 항목 | 대응 |
| --- | --- |
| F1 | Rust test-owned font/HWPX 입력을 `rust-test-input`으로 분리하고 과대 분류 방지 테스트를 추가 |
| F2 | Native Skia job의 frontend `none|unit|package` 진리표 정적 단언 복구 |
| F3 | aggregate harness를 다음 step 또는 job 경계에서 자르고 GitHub과 같이 `bash -e -o pipefail`로 실행 |
| F4 | canonical `pr_review_workflow.md` §3.1을 Stage 4 조건부 그래프로 갱신 |
| F5 | 기존 Native Skia test 누락을 [#4040](https://github.com/edwardkim/rhwp/issues/4040)으로 분리 |
| F6 | `mydocs/pr/archives/pr_4032_review.md`와 2026-08-05 오늘 기록을 trailing commit으로 준비 |

## 검증

| 검증 | 결과 |
| --- | --- |
| `actionlint .github/workflows/ci.yml .github/workflows/render-diff.yml` | 통과 |
| `node --check scripts/ci-impact-classifier.cjs` | 통과 |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 27/27 통과 |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py scripts/tests/test_render_diff_workflow.py` | 22/22 통과 |
| aggregate shell 진리표 실행 — frontend-only, Rust 비렌더, Rust render, 비-Rust Native | 모두 통과 |
| aggregate shell 불일치·unknown 축 입력 | 모두 의도대로 실패 |
| `git diff --check` | 통과 |

장시간 Rust 전체 CI는 workflow/classifier 변경이 `fail-closed:classifier-contract`로 진입한 원격
full lane에서 확인했다.

- 보정 head `1f12a5fe0`: CI 30923071182, Render Diff 30923070493, CodeQL 30923070506 통과
- 첫 devel merge head `b0be86731`: CI 30924641673, Render Diff 30924638772, CodeQL 30924638749 통과
- 그 aggregate: shard `3698+693+840+1=5232`, expected runnable `5232` 일치
- 최종 code head `5eeab15fd`: CI 31004297167, Render Diff 31004296886, CodeQL 31004296907 통과
- 최종 aggregate: shard `3714+753+784+1=5252`, expected runnable `5252` 일치, preflight
  `no-trailing-review-only-commits`로 full lane 진입, `frontend_mode=package`로 Frontend unit gates만 skip

## 다음 단계

1. review-only trailing commit push와 ready 전환 뒤 preflight fast-pass와 required `Build & Test`
   성공을 확인한다. 기대 candidate는 `5eeab15fd`다.
2. 사용자가 CI 통과 후 요청하면 collaborator self-merge 절차를 진행한다.
3. merge 뒤 frontend-only canary에서 Rust lint·세 builder·네 worker·Native Skia가 모두 `skipped`되고
   `Build & Test` aggregate가 성공하는지 실측한다.
4. #3810 직후 4.73GB cache 기준선과 다음 cache sweep 직후 총량을 대조한 뒤 Stage 5 CodeQL 언어
   조건화로 진행한다.
