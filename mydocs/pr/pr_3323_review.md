---
kind: reference
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-26
---

# PR #3323 검토 기록 — 머리말/꼬리말 필드의 표시·모델 좌표 분리

## 메타

| 항목 | 값 |
| --- | --- |
| 원 PR | [#3323](https://github.com/edwardkim/rhwp/pull/3323) |
| 작성자 | `lpaiu-cs` |
| 관련 이슈 | #3216 (`closes #3216`은 통합 PR merge 뒤에만 실제 close 여부 확인) |
| 원 head | `ca8219d232dcce2a081e1a6dccd789a4482bb16c` |
| 원 base / 상태 | `devel` / `BEHIND` (검토 착수 시점) |
| 원 변경 | 30 파일, +367/-80 |
| 검토 branch | `review/lpaiu-cs-hf-field-20260726` |
| 검토 base | `upstream/devel` `99732b2a1189` |
| 적용한 contributor commits | `d9c5b325` → `9e78b00c6`, `ca8219d232` → `07bbc1492` |
| 검토 라우트 | `maintainer_general` + `intake_and_review`, `local_validation`, `visual_fixture_evidence`, `rework_and_exceptions` |

원 tip만 체리픽하면 이미 revert된 #3212 보정이 빠져 충돌한다. 따라서 PR 범위의 두 commit을 최신
`devel` 위에 순서대로 적용했으며, 누적 적용은 clean이다. reviewer는 `jangster77`로 요청했다.

## 원 변경 검토와 메인터너 보정

원 변경은 머리말/꼬리말의 페이지·전체 페이지·파일명 marker를 모델 문자열(`text`)에서 1자로
유지하고, 화면 표시만 `display_text`로 확장한다. 이 규약은 hit test와 편집이 표시 문자열 길이가 아니라
모델 오프셋을 쓰도록 만드는 핵심이다.

검토 중 다음 두 결함을 확인해 메인터너 보정으로 현재 branch에 추가했다.

1. `AutoNumber(Page)`가 있는 문단에서 기존 blanket `U+0015` 치환이 명시 쪽번호 필드까지 다시
   표시 문자열로 바꿔 `text`/`char_start` 정합을 깨뜨렸다. AutoNumber 컨트롤이 가리키는 위치만
   별도 run으로 분리해 marker 1자를 유지하고 `display_text`만 설정하도록 고쳤다.
2. Studio history가 `result.charOffset - 요청 cursor offset`으로 marker 길이를 추정했다. inline
   control 뒤 cursor에서는 native의 실제 text 삽입 위치와 달라 음수 길이 또는 잘못된 undo 범위가 될 수
   있었다. native 응답에 `insertedAt`·`insertedLength`를 명시하고, redo에는 원 cursor 좌표를,
   undo에는 실제 모델 범위를 사용하도록 고쳤다.

세부 적용·rollback은 [implementation 계획](pr_3323_review_impl.md)에 기록한다.

## 시각 검증

변경은 머리말/꼬리말의 실제 그리기와 hit-test 좌표를 함께 바꾼다. 원 PR은 새 HWP/HWPX fixture 또는
기준 PDF를 제공하지 않았으므로, 외부 기준 PDF 대조 대상은 없다. 회귀 테스트는 파일명 필드가 화면에는
`displayText`로 보이되 layer tree의 raw `text`는 marker 1자로 남는 것을 검증한다.

메인터너 보정 뒤 Native Skia로 `samples/SO-SUEOP.hwpx` 5쪽(0-based page `4`)을 렌더링했다.
아래 실제 산출물에서 머리말 제목·밑줄과 꼬리말의 학교명·AutoNumber(Page) `5`가 모두 보인다.
명시 field marker의 모델 1자 보존은 PNG만으로 판별할 수 없으므로, `issue_3216_hf_field_display_space`
및 이 PR에서 추가한 AutoNumber/필드 단위 테스트로 함께 검증했다.

![SO-SUEOP HWPX 5쪽 — Native Skia 머리말/꼬리말과 쪽번호 표시 검증](assets/pr_3323_lpaiu-cs_issue3216_p005_review.png)

- 생성 명령: `target/review-lpaiu-cs-hf-field-20260726/release/rhwp export-png samples/SO-SUEOP.hwpx --page 4 --output mydocs/pr/assets --max-dimension 1600`
- 산출물: `794 × 1123` RGBA PNG, SHA-256 `c4aa6cd11853ccc2a59db16631e5d04d58a2f19980b6c8d0860e579ff75211da`
- 기준 PDF는 원 PR에 없으므로 PDF 대조를 주장하지 않는다.

이 기록을 작성하는 시점에는 아직 원격 push나 GitHub comment를 하지 않았다.

## 로컬 검증

검토 전용 target은 `target/review-lpaiu-cs-hf-field-20260726`이며 모든 Cargo 검증에
`CARGO_INCREMENTAL=0`을 사용한다. WASM build는 작업지시자가 수동 검증하는 범위라 실행하지 않는다.

| 항목 | 결과 |
| --- | --- |
| `git diff --check` | 통과 |
| `cargo fmt --check` | 통과 |
| 원 PR 집중 Rust: `issue_3216_hf_field_display_space` | 보정 전·후 통과 (3 tests) |
| 원 PR 보조 Rust: `issue_1144` | 통과 (4 tests) |
| AutoNumber placeholder 회귀: `issue_1113_header_autonum_placeholder` | 통과 (1 test) |
| 전체 Rust: `cargo test --profile release-test --tests` | 통과 |
| Studio `npm run build` | 통과 |
| Studio `npm test` | 통과 (637 tests) |
| 메인터너 AutoNumber/필드 회귀 | 통과 (1 test) |
| 메인터너 inline-control history 회귀 | 통과 (1 test) |
| Native Skia lib: `--features native-skia skia --lib` | 통과 (57 tests) |
| Native Skia placeholder: `issue_2225_missing_picture_placeholder` | 통과 (2 tests) |
| Native Skia PDF: `render_p37_direct_pdf_export` | 통과 (4 tests) |

## 현재 권고

**메인터너 보정 포함 후 최신 head 검증·시각 증적이 완료되면 merge 준비 가능.** 원 #3323은 stale
base이므로, 원 contributor branch의 update 대신 이 최신 `devel` 기반 검토 branch에서 contributor credit과
메인터너 보정을 포함한 통합 PR로 처리한다. 원격 push, PR 생성·comment, merge·close는 아직 하지 않았다.
