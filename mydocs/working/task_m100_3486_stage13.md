---
kind: investigation
status: active
canonical: mydocs/manual/codex/docs_and_git_workflow.md
last_verified: 2026-07-31
---

# Task #3486 Stage 13 — HWP3 p3 표 글머리표·세로 흐름 원인 분리

- 이슈: [#3486](https://github.com/edwardkim/rhwp/issues/3486)
- 기준 `devel`: `665982920b5d4739a58485060043a2a25b09c5e7`
- 기준 입력: `samples/HWP3-password-123456.hwp` (24쪽)
- 비교 입력: `samples/HWP5-nopassword-123456.hwp` (동일 문서의 한컴 HWP5 변환본, 24쪽)
- 기준 오라클: `pdf/HWP3-password-123456.pdf` (24쪽)
- 선행 결론: [Stage 12](task_m100_3486_stage12.md)의 제품명 display projection은 유지한다.

## 재현

현재 `devel` source를 `CARGO_TARGET_DIR=target/issue-3486-p3-20260731`,
`CARGO_INCREMENTAL=0`으로 `release-test` profile에 빌드했다. 비밀번호 값은 프로세스 인자,
출력, 이 문서에 기록하지 않고 local stdin launcher로만 HWP3에 공급했다.

```bash
python3 scripts/task1274_visual_sweep.py \
  --key hwp3-password-p3-current-devel \
  --hwp samples/HWP3-password-123456.hwp \
  --pdf pdf/HWP3-password-123456.pdf \
  --pages 3 --dpi 144 \
  --rhwp-bin <local-password-stdin-launcher> \
  --out /private/tmp/rhwp-issue-3486-p3-20260731/sweep
```

| 입력 | 요청/완료 | pixel match | ink proxy | 구조 후보 |
| --- | --- | ---: | ---: | --- |
| HWP3 원본 p3 | `[3]` / `[3]` | 93.48494% | 6.92495% | `content_bottom_drift` |
| HWP5 변환본 p3 | `[3]` / `[3]` | 93.35590% | 6.49419% | 없음 |

두 실행 모두 SVG와 render tree는 24쪽을 export했지만 raster/overlay/review는 p3만
완료했다. 따라서 전수 raster sweep 또는 전체 fidelity 합격을 주장하지 않는다. pixel/ink 값은
글꼴 raster 차이를 포함한 후보 지표이며, HWP5의 더 낮은 ink 값은 구조 fidelity의 우열을 뜻하지
않는다.

## 확인된 차이

한컴 PDF와 HWP5 변환본 p3의 4×2 표 우측 셀에는 `▸` 글머리표가 있다. HWP3 원본의 같은
셀은 원문 IR에서 그 marker가 없고 선행 공백만 남는다. HWP3 render tree도 marker를 paint하지
않는다. 이는 font raster 차이가 아니라 HWP3 source → IR 단계의 사용자 가시 소실이다.

| 대상 | HWP3 원본 | HWP5 변환본 |
| --- | --- | --- |
| p3 첫 제목 `LINE_SEG` 간격 | `ls=600` | `ls=960` |
| p3 본문 pi=24 첫 `vpos` | `2624` | `2984` |
| p3 폴더 표 pi=30 첫 `vpos` | `28540` | `29292` |
| p3 표 뒤 본문 pi=32 첫 `vpos` | `48464` | `49816` |
| 4×2 표 우측 셀 내용 | 글머리표 없음 | `▸` 글머리표 보존 |

HWP3 p3 render tree에서 첫 표는 `y=203.7px`, 폴더 표는 `y=514.7px`, 표 뒤 본문의
첫 기준선은 `y=778.5px`이다. PDF raster와 대조하면 첫 표부터 누적된 간격 차이가 표 뒤
본문에서 약 17px의 상향 배치로 보이며, 이는 visual sweep의 `content_bottom_drift`와 일치한다.

## 현재 판단과 다음 조사

`src/parser/hwp3/paragraph.rs`는 `special_char_flags`를 읽지만, 현재 parser에서 이 값이
HWP3 표 셀 글머리표를 복원하는 데 쓰이는지는 확인되지 않았다. 즉 이 필드는 **조사 후보**일 뿐
원인으로 확정하지 않는다.

다음 단계는 HWP3 table-box 내부 문단의 raw 문자/제어 데이터, `special_char_flags`, HWP5의
동일 셀 IR을 문단 단위로 대조해 `▸`의 원천을 확정하는 것이다. 원천이 확정되기 전에는 전역
문자 치환, renderer 전용 보정, HWP3 외 문서에 영향을 주는 line-spacing 변경을 하지 않는다.

확정 시에는 HWP3 parser에서만 최소 보정하고, 실제 HWP3 p3 PDF review와 원문 IR 보존/음성
회귀를 함께 추가한다.
