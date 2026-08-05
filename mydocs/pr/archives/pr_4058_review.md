# PR #4058 검토

## 결론

**수용 후보.** #3930의 직접 수용 기준인 p144/145 표 이월과 p30 머리말 배정은 실제 편람
fixture 회귀로 고정됐다. 한컴 PDF 전수 fidelity의 잔여분은 이 PR의 완료 주장에 섞지 않고
[#3820](https://github.com/edwardkim/rhwp/issues/3820)으로 이관했다.

최종 병합 조건은 이 review·오늘 기록을 포함한 최신 PR head의 GitHub Actions 통과와 작업지시자
승인이다.

## 접수 및 기준

| 항목 | 내용 |
| --- | --- |
| PR | [#4058](https://github.com/edwardkim/rhwp/pull/4058) `fix: HWPX 저장 조판 입력 보존 (#3930)` |
| 작성자 | `jangster77` |
| 대상 | `devel` |
| 구현 head | `043a2e339d537cb068287160e89434be87e14a69` |
| 구현 기준 devel | `aebfcaa33` |
| 관련 이슈 | [#3930](https://github.com/edwardkim/rhwp/issues/3930), 후속 [#3820](https://github.com/edwardkim/rhwp/issues/3820) |
| 구현 변경 규모 | 25 files, +3,466 / -205 |
| 문서 작성 시점 mergeable | `MERGEABLE` |
| 문서 작성 시점 merge 상태 | `BLOCKED` (GitHub Actions 진행 중) |

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
           visual_fixture_evidence.md, rework_and_exceptions.md (대형 PR)
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_self_merge.md, intake_and_review.md,
                  local_validation.md, visual_fixture_evidence.md,
                  rework_and_exceptions.md
```

## 변경 내용

- HWPX 저장 계보의 표는 `treat_as_char && flow_with_text`일 때만 글자처럼 흐르게 해
  `flowWithText=0`의 p144/145 표 연속을 보존한다.
- 희소 HWPX 바탕쪽 상속을 HWP5 `Both`/`Odd` 슬롯으로 물질화해 p30 짝수 쪽의 책 제목과
  홀수 쪽의 장 제목을 구분한다.
- HWP5 DocInfo/BodyText/CHAR_SHAPE의 raw 계약을 분석하는 diagnostic probe와
  `hwp5-char-shape-audit` CLI를 추가했다. HWPX decoration signature는
  `unique_different`/`ambiguous`/`unmatched`를 함께 포함하므로 production serializer
  canonicalization 기준으로 쓰지 않는다.
- CLI 계약은 [CLI 명령 매뉴얼](../../manual/cli_commands.md#4-hwpxhwp-저장-계약-분석-hwp5--진단-도구)에
  등록했고, 조사 단계 1~6은 `mydocs/working/task_m100_3930_stage*.md`에 기록했다.

## 로컬 검증

아래 검증은 구현 head `043a2e339`에서 완료됐다.

| 검증 | 결과 |
| --- | --- |
| `CARGO_INCREMENTAL=0 cargo test --profile release-test --test issue_3930_hwpx_hwp_save_layout -- --nocapture` | 1 passed |
| `CARGO_INCREMENTAL=0 cargo test --lib diagnostics::hwp5_char_shape_audit -- --nocapture` | 5 passed |
| `target/release-test/rhwp hwp5-char-shape-audit <hancom.hwp> <generated.hwp> --source-hwpx <source.hwpx> --out <report.md>` | 실제 편람 fixture 보고서 생성 성공, source charPr 937개 확인 |
| `cargo fmt --check` | 통과 |
| `git diff --check` | 통과 |

전체 `cargo test`와 `cargo clippy`는 이번 최신 head에서 재실행하지 않았다. 대형 PR의 최종
전체 회귀는 최신 GitHub Actions 결과로 확인한다.

## 시각·fixture 판단

이 PR은 renderer/typeset과 HWPX→HWP 저장 경로를 변경하므로 fixture 검증이 필요하다.

- `samples/2025 행정업무운영 편람(최종).hwpx` 저장 후 재열기에서 387쪽, p144 `endCut=[21]`와
  p145 `startRow=2/startCut=[21]`의 표 연속, p30 책 제목 머리말을 focused release-test로 확인했다.
- 기준 HWP/HWPX/PDF 경로와 SHA-256은 [#3930 후속 기록](https://github.com/edwardkim/rhwp/issues/3930#issuecomment-5188976890) 및
  [#3820 이관 기록](https://github.com/edwardkim/rhwp/issues/3820#issuecomment-5189062021)에 남겼다.
- Hancom 2020 PDF 전수 비교의 production baseline에는 109쪽/392,833픽셀 차이가 남는다.
  이는 #3930 직접 수용 기준과 별개로 #3820에서 이어 가며, 이 PR의 visual pass라고 주장하지 않는다.

## 범위 밖 변경 및 잔여 위험

- CharShape sentinel probe의 7쪽/631픽셀 개선은 Hancom oracle에 의존한 진단 결과다. source-derived
  조건이 없으므로 production serializer는 Stage 2와 byte-identical 상태를 유지한다.
- 한컴 2020 `PrintToPDFEx`와 기준 PDF의 전체 시각 정합은 #3820에서 raw 저장 계약 축별로 재분석한다.
- `hwp5-char-shape-audit`의 HWPX source option은 진단 전용이며, input HWP/HWPX를 변경하지 않는다.

## 최종 권고

대형 PR 예외 절차에 따라 최신 head의 required GitHub Actions가 통과하고 작업지시자가 승인하면
PR #4058을 병합한다. 병합 시 PR 본문의 `Closes #3930`으로 직접 수용 기준 이슈를 닫고,
#3820은 열린 상태로 유지한다.
