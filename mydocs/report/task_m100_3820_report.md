---
kind: report
status: active
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-04
---

# Task #3820·#3821 — HWP 215쪽 전수 결함 종합 보고서

- **이슈**: #3820, #3821
- **기준 브랜치**: `task/3820-3821-fidelity` (2026-08-04 Stage 9 검증)
- **판정 대상**: `samples/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwp`
- **정답지**: `pdf/pr3740/hwp/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구)-2020.pdf` (한컴 2020 기준 PDF)

## 1. 결론

215쪽 전수 raster·overlay 검증은 **완료**했다. 비교 대상 PDF는 215쪽이고, 요청한 215쪽은 모두
SVG·PDF raster·compare·overlay·review PNG까지 생성됐다. 최초 inventory의 rhwp 전체 export는
219쪽으로 기준 PDF보다 4쪽 많았고, 이후 p168 표 fragment와 p118 그림 앞 본문 owner를 보정해 현재는
**218쪽(+3)** 이다. 이 차이는 전역 보정의 근거가 아니라, 뒤쪽의 페이지 경계/float/표 분할 원인을
계속 조사해야 한다는 강한 신호다.

이번 전수 결과는 다음 세 가지를 분명히 한다.

1. p118→p119의 `TopAndBottom` 그림 앞 본문 owner 이동과 p127 그림 56의 page-top geometry는
   Stage 8·9에서 해소했고 각각 focused PDF review와 회귀로 고정했다.
2. p168 부근부터 페이지 경계가 달라지고, p170 이후에는 같은 쪽번호에서 서로 다른 논리 내용이
   대조되는 **연쇄 pagination divergence**가 나타난다. p171~215의 대량 후보는 45개의 독립 결함이
   아니라 이 상류 결함의 연쇄 신호로 우선 다뤄야 한다.
3. p127의 이전 false negative 형상은 이제 `fidelity_compare`와 visual sweep의
   `deferred_square_picture_top_drift` 후보로 잡는다. 다만 자동 판정은 후보를 넓히는 도구이며,
   무결함 증명 도구는 아니다.

이 보고서는 현 상태의 inventory다. 이 단계에서는 renderer 동작을 바꾸지 않았다.

## 2. 전수 검증 완결성 및 재현

실행 명령:

```text
python3 scripts/visual_sweep.py \
  --key stage7-full-215 \
  --hwp 'samples/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwp' \
  --pdf 'pdf/pr3740/hwp/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구)-2020.pdf' \
  --pages 1-215 --dpi 144 \
  --rhwp-bin target/task-3820-3821-fidelity/release-test/rhwp \
  --out output/task-3820-3821-fidelity/stage7-full-sweep
```

| 항목 | 결과 |
| --- | ---: |
| 요청 / 완료 / 누락 | **215 / 215 / 0** |
| 기준 PDF / 이번 선택 SVG / render tree | **215 / 215 / 215** |
| compare / overlay / review PNG | **215 / 215 / 215** |
| rhwp 전체 export SVG / render tree | **219 / 219** |
| PDF 대비 rhwp 전체 page delta | **+4** |

전수 실행 원장은
`output/task-3820-3821-fidelity/stage7-full-sweep/summary.json`에 있고, 페이지별 증적은
`output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/{compare,overlay,review}/`에 있다.

Overlay의 평균 pixel match는 92.09%지만 평균 ink match는 16.50%다. 글꼴 rasterization,
anti-aliasing, 링크색 차이도 ink score를 크게 바꾸므로 이 숫자만으로 결함을 확정하지 않는다.

## 3. 확정·고우선순위 결함

### D-01 — p118→p119 그림 앞 본문 owner가 한 쪽 이르게 확정됨 — 해소

초기 inventory에서는 p119에서 rhwp가 절차 그림으로 바로 시작하지만, 한컴 PDF는 p118의 본문 뒷부분을 p119
상단에 먼저 배치한 뒤 그림을 둔다. `fidelity_compare`도 p118→p119에서
`rhwp_earlier_than_reference`, shared text 72자, 양쪽 coverage 1.000을 기록했다. 다음 p119 상단에는
Body `TopAndBottom` 그림(`pi=1276`, `bbox=94.5,83.2,448.5,359.0`)이 있어, 그림 앞 paragraph owner
결함으로 우선 분석해야 한다.

증적:

- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/review/review_118.png`
- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/review/review_119.png`
- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/compare/compare_118.png`, `compare_119.png`
- `output/task-3820-3821-fidelity/stage6-full-ledger/float-owner-shift-candidates.tsv`의 p118→p119 행

**해소 상태:** `309c5f123`가 p118의 `pi=1275`를 lines `0..8`, p119를 lines `9..10`과 그림 55로
분할해 PDF owner를 복원했다. 직접 증적은 [Stage 8 visual sweep](../working/task_m100_3820_stage8_visual_sweep.md)에 있다.

### D-02 — p127 본문과 그림 56의 폭/배치 관계가 PDF와 다름 — 해소

초기에는 **사용자 직접 확인 결함이자 자동판정 false negative**였다. p127에서 rhwp와 기준 PDF의 그림 56 주변
본문 행폭·완충 관계가 다르다. 이번 visual sweep의 `page_127.json`은 flag와
`square_wrap_text_overlap_candidates`를 모두 0건으로 기록했다. 즉 현 규칙은 실제 그림-본문 관계의
fidelity 저하를 아직 충분히 검출하지 못한다.

증적:

- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/review/review_127.png`
- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/compare/compare_127.png`
- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/overlay/overlay_127.png`

**해소 상태:** Stage 9는 deferred page-start Square 그림의 source `vertical_offset` 이중 적용을
제거해 frame top을 `130.7px`에서 body top `83.2px`로 복원했다. 이전 형상은 새
`deferred_square_picture_top_drift` detector가 후보화하고, 수정 후 p127/p156 직접 PDF review와
focused Rust·Python 회귀는 [Stage 9 visual sweep](../working/task_m100_3820_stage9_visual_sweep.md)에 있다.

### D-03 — p168 이후의 연쇄 pagination divergence

**확정된 고우선순위 결함군.** p168→p169에는 227자의 reciprocal owner-shift가 있고,
`text-owner-sequence-candidates.tsv`도 같은 경계의 47자 연속 문자열 이동을 기록한다. p170부터
동일 쪽번호 비교에서 서로 다른 논리 내용이 나타나며, p171~215에는 본문 흐름 붕괴 후보가 연속된다.
전수 overlay의 평균 visual proxy도 p1~167은 19.49%, p168~215는 6.11%로 급락했다.

이는 p170 이후의 모든 페이지를 개별 수정할 사안이 아니다. p168~170의 표·그림·문단 owner와
page-break 결정을 먼저 고쳐 같은 논리 흐름을 다시 정렬해야 한다.

**2026-08-04 Stage 7 상태:** 최초 p168 분기(표 44 `pi=1778`의 first fragment 누락)는
`319ed3dd4`에서 해소했다. p168은 `PartialTable` first fragment, p169은 continuation+그림 65,
p170은 `(라) 심혈관계 검사` 본문으로 다시 정렬됐고 rhwp 전체 쪽수는 219에서 218로 줄었다.
다만 기준 PDF의 215쪽보다 아직 3쪽 많고, 이 전수 inventory 자체는 수정 전 기준선이므로 D-03 전체를
해결로 재분류하지 않는다. focused 3-way 증적과 다음 조사 범위는
[Stage 7 visual sweep](../working/task_m100_3820_stage7_visual_sweep.md)에 기록했다.

증적:

- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/review/review_168.png` ~ `review_171.png`
- `output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/compare/compare_168.png` ~ `compare_171.png`
- `output/task-3820-3821-fidelity/stage6-full-ledger/text-owner-shift-candidates.tsv`의 p168→p169 행
- `output/task-3820-3821-fidelity/stage6-full-ledger/text-owner-sequence-candidates.tsv`의 p168→p169, p172→p173 행

## 4. 자동 검출 inventory

다음은 **결함 확정 목록이 아니라** PDF 대조를 우선해야 할 자동 후보 목록이다. 같은 원인으로
연쇄된 페이지는 한 묶음으로 해석한다.

### 4.1 Raster/구조 visual sweep 후보: 58쪽, 64개 flag

| 규칙 | 후보 쪽 | 해석 우선순위 |
| --- | --- | --- |
| `column_text_flow_collapse` (46) | 7, 9, 28, 75, 77, 119, 134, 171–175, 177–179, 182–185, 187–199, 201, 203–215 | p119 및 p171–215는 D-01/D-03과 결합. 나머지는 독립 PDF review 필요 |
| `line_order_overlap` (3) | 118, 129, 181 | p118은 D-01과 결합; p129·181은 후보 |
| `frame_overflow_pixels` (3) | 161, 167, 204 | 테두리/표 외곽선으로도 발생할 수 있어 candidate-only |
| `content_bottom_drift` (1) | 167 | p167 frame 후보와 함께 review |
| `column_line_band_drift` (1) | 181 | p181 line-order 후보와 함께 review |
| `question_marker_flow_drift` (9) | 20, 24, 42, 47, 68, 174, 176, 182, 183 | 앞 5쪽은 독립 review, 뒤 4쪽은 D-03 연쇄 가능성 |
| `endnote_separator_gap_drift` (1) | 27 | 각주 separator 간격 review |

세부 flag와 candidate 수는
`output/task-3820-3821-fidelity/stage7-full-sweep/stage7-full-215/analysis/page_*.json`에 있다.

### 4.2 Layout ledger 후보

| 후보 | 건수 / 쪽 경계 | 의미 |
| --- | --- | --- |
| reciprocal text owner shift | 8 | 74→75, 90→91, 118→119, 120→121, 129→130, 131→132, 166→167, 168→169 |
| order-preserving text sequence shift | 6 | 74→75, 131→132, 168→169, 172→173 (3행) |
| 상단 float와 결합한 owner shift | 2 | 74→75, **118→119** |
| table fragment | 15 | 66→67, 76→77, 78→79, 90→91, 94→95, 106→107, 157→158, 160→161, 161→162, 163→164, 164→165, 167→168, 176→177, 190, 215 |

원장 파일은 `output/task-3820-3821-fidelity/stage6-full-ledger/` 아래의
`text-owner-shift-candidates.tsv`, `text-owner-sequence-candidates.tsv`,
`float-owner-shift-candidates.tsv`, `table-fragment-candidates.tsv`다. table fragment는 동일
표의 인접 페이지 fragment를 찾는 규칙일 뿐, 행의 소유 페이지가 PDF와 다르다는 자동 확정은 아니다.

## 5. 자동 판정의 한계와 다음 수정 순서

1. **D-03의 최초 잔존 divergence를 최신 218쪽 renderer에서 다시 분석한다.** p171 이후 flow flag를 개별
   해결하지 않는다. 먼저 표/그림 float와 다음 본문 block의 owner·page-break 결정이 PDF와
   갈라지는 최초 지점을 확정한다.
2. 각 수정은 분석 문서 → 코드 → focused PDF review → 동일 215쪽 전수 재실행 순으로 분리한다.
   page count +3을 전역 강제 break로 상쇄하지 않는다.

## 6. 해소로 재분류한 과거 항목

이번 전수 run에서 p108 TIFF 그림 미출력과 p156 Square 그림 여백은 현재 우선 결함으로 분류하지
않았다. 이들은 이전 stage의 focused PDF review에서 각각 PNG 변환 및 outer-margin 보정 후 정상
확인된 항목이다. 다만 이후 regression은 위 전수 기준선을 다시 실행해 판정한다.

## 7. 검증 도구 상태

이번 inventory를 생성한 `scripts/visual_sweep.py`와 `tools/fidelity_compare/fidelity_compare.py`는
이미 p118→p119 owner shift를 상단 float와 연결하는 triage를 갖고 있고, Stage 9는 deferred Square
page-top offset drift도 공통 후보로 추가했다. 관련 Python 회귀는 다음으로 확인했다.

```text
python3 -m py_compile tools/fidelity_compare/fidelity_compare.py
python3 -m unittest scripts/tests/test_fidelity_compare.py scripts/tests/test_visual_sweep.py
# Ran 59 tests ... OK
```

수정 전 p127 geometry의 재발은 후보화하지만, D-03의 잔존 페이지 경계 결함을 포함해 이 통과는
검출기 구현 회귀 부재를 뜻할 뿐 이 215쪽 문서의 layout fidelity 보증은 뜻하지 않는다.
