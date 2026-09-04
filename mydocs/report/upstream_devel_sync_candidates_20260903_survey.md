# upstream devel 신규 동기화 후보 — 8클러스터 기술 서베이 (2026-09-03)

- 전제: `mydocs/report/upstream_devel_sync_candidates_20260901.md`(2026-09-01 스냅샷) 기준
  Tier 1~2 전 항목은 이미 완료/제외 처리됐다(`d52da303d`, `bbeb45f66` 참고). 이 문서는 그
  이후 `origin/devel`에 새로 쌓인 커밋을 다룬다.
- 비교 대상: `rhwp-code:main`(`.`) vs `origin/devel`(`../rhwp`, upstream `edwardkim/rhwp`).
- fork point: `6f70cd1b6f25adc06bc6912251b683819626b35e` (양쪽 저장소에 물리적으로 동일 커밋
  존재, `git cat-file -t`로 재확인 완료).
- 조사 시각: 2026-09-03. `origin/devel` HEAD `d770ef80e`(2026-09-03, PR #6677 병합).
  fork point 이후 신규 커밋 132개(2026-09-02~09-03) 중 gym/docs/release/ci 계열
  ~90개는 코어 엔진 fidelity와 무관해 제외(§9). 남은 8개 클러스터(37개 커밋)를
  서베이했다 — **이번 라운드는 서베이만, 이식은 아직 하지 않았다.**
- 방법론: 각 클러스터에 대해 (1) `git show`로 실제 diff 확인, (2) rhwp-code 대응 코드에
  같은 버그가 재현되는지 직접 대조, (3) rhwp-code 자체 발산 커밋(표 레이아웃 T1~T9 분할,
  컬럼 폭 솔버 `4929e5d15`/`e20b2457e` 등)과 파일 충돌 위험, (4) 이식 난이도.
  `#5251` 사례(패치를 먼저 짜고 나서야 재현 안 됨을 확인해 되돌린 낭비)를 반복하지
  않기 위해, 실제 이식은 이 서베이 이후 사용자 우선순위 결정을 거쳐 진행한다.

## 요약 우선순위 (권장)

| 순위 | 클러스터 | 재현 확인 | 이식 난이도 | 파일 충돌 위험 |
|---|---|---|---|---|
| 1 | ⑦ HWPX note/앵커 vpos (2건) | ✅ 재현됨 | 낮음 | 낮음 |
| 2 | ⑤ PDF 하위 SVG 비트맵 유지 (1건) | ✅ 재현됨 | 낮음(가장 쉬움) | 낮음 |
| 3 | ①-㉮ TAC/부동 개체 여백 미반영 (4건) | ✅ 재현됨(추정, 함수 동일 존재) | 낮음 | 낮음(발산 0건 파일) |
| 4 | ③ EMF PolylineTo16 등 4단 연쇄 | ✅ 재현됨 | 낮음~중 | 낮음 |
| 5 | ②-㉮ 인라인 TAC 표 정렬(#6601 net) | ✅ 재현됨(실측, 동일 버그 라인 확인) | 낮음(로직 그대로 포팅 가능) | 중(관련 함수 rhwp-code 자체 이력 있음) |
| 6 | ②-㉯ cellzone 테두리(#6619) | ✅ 재현됨 | 중 | 낮음(geometry.rs, T4 분리 산물) |
| 7 | ①-㉯ TAC 캡션/baseline 연작(5건) | 미확인(중간 위험 함수) | 중(순서 의존) | 중 |
| 8 | ④ WMF y축 뒤집기 3중 상쇄 | ✅ 재현됨 | 중(좌표계 핵심 로직) | 낮음 |
| 9 | ①-㉰ 셀·글상자 줄높이/오프셋(2건) | 미확인 | 낮음~중 | 낮음~중 |
| 10 | ⑥ 차트 XML 선언 반영(#6624, 2건) | ✅ 재현됨(추정, 하드코딩 확인) | 중~높음 | 낮음(단 기존 파서와 통합 필요) |
| 11 | ⑧ 필드 리플로우(2건, 하나의 논리 패치로 취급) | 미확인(시그니처 변경 전파 확인 필요) | 중 | 낮음 |
| 후순위 | ①-㉱/② 표 경계 잔여(`f4c0f7334e`,`208a18b8d7` 등) | — | — | **최고 위험**(`table_layout.rs` 자체 발산 19건) |

---

## ① TAC(글자처럼)/부동 개체 여백·baseline 클러스터 (17개, `jeong-sik` 연작)

`src/renderer/`, `src/renderer/layout/` 전역. 4개 하위그룹으로 묶임.

**㉮ 여백 미반영 (4건: `0b1062f391` `683a74e365` `197bbea933` `9c7caced20`)** — 부동/글자처럼
그림·도형이 개체 바깥여백을 무시하고 잉크 크기만으로 자리를 잡는 문제, 머리말·꼬리말
직접배치 그림 기준 틀 오류. `layout_body_picture`/`layout_shape_item`/
`layout_header_footer_picture`가 rhwp-code에도 동일 이름·위치로 존재, 관련 파일
(`picture_footnote.rs`) fork point 이후 rhwp-code 자체 발산 **0건** — 충돌 위험 거의 없음.
순수 로직 포팅.

**㉯ TAC 캡션/baseline 정렬 (5건: `098eda8ff5` `65df9ac15a` `53282aa5e8` `67bdcd8ae5`
`e86f64bf55`+`b63ab2cd68`)** — 캡션 붙은 TAC 그림 baseline 정렬, 빈 줄 상단 앉힘, 캡션을
그림 실제 바닥 기준으로 잡기, TAC 높이 보정이 문단 앞 간격(sb)을 되돌리는 버그.
`paragraph_layout.rs`/`layout.rs`에 집중(발산 4건/1건 — 직접 diff 대조 필요). 순차
디버깅 연작이라 순서대로 적용 필요.

**㉰ 셀·글상자 내부 줄높이/오프셋 (2건: `76c27710a4` `e0e07783dc`)** — 셀 안 글자+글자처럼
도형 혼재 줄의 줄높이 접힘, 글상자 문단 TAC 폭 이중 오프셋. `shape_layout.rs`(발산 0건,
저위험), `paragraph_layout.rs`(발산 4건).

**㉱ 표 경계 잔여 (4건: `f4c0f7334e` `208a18b8d7` `fab784334e` `cdf358d759`)** — TAC 표가
문단 흐름 시작에 겹쳐 그려지는 문제, 문단 기준 자리차지 표 세로 기준점 오류, 자리차지
밴드 이중 계상. `f4c0f7334e`는 `table_layout.rs`를 직접 건드려 **클러스터 ②와 경계가
흐림** — 통합 검토 필요. `table_layout.rs`는 rhwp-code 자체 발산 **19건**으로 8클러스터
전체 최고 충돌 위험 파일.

**실측 확인**: `fab784334e`(고정폭 빈칸 U+2007 전진폭 0.5em→0.25em) 대표 샘플 확인 결과
rhwp-code `text_measurement.rs`에 **동일 버그 재현**. 단 upstream은 단일 함수, rhwp-code는
같은 로직이 `char_width` 클로저 3곳(323/460/1216행)에 중복 — 기계적이나 단일 지점 아님.

통합 요약 통계: 발산 없는 저위험 파일(①/③/④/⑤ 등 총 6건) vs 표 레이아웃과 얽힌
고위험 항목(㉱ 4건).

---

## ② 표/셀 레이아웃 클러스터 (9개) — rhwp-code 자체 표 리팩터와 직접 겹침

`table_layout.rs` 분할(T1~T9)·컬럼 폭 솔버(`4929e5d15`/`e20b2457e`)와 같은 영역이라
충돌 위험이 8클러스터 중 가장 크다.

**인라인 TAC 표 정렬 (`f13cd7d53e` `b2a1550cb2` `74b2875a2f`, #6601 net effect)** —
`paragraph_layout.rs`의 `layout_inline_table_paragraph` 보정/되돌림 연작. 최종 상태:
(1) 정렬 폭은 열별 셀 폭 합이 아니라 표 **선언 폭**을 써야 함(병합 셀 많은 표에서 열
합산 과소계상), 줄넘김 판정(`should_wrap_middle_anchored_table`)의 `table_footprint`와
일치 필요(`#5785`/`is_tac_table_inline` 계약 강화). (2) 선행 컨트롤 개수는
`offsets[0]/8`(전체 컨트롤)이 아니라 문자 위치 0인 인라인 표 개수로 세야 함.
**실측: rhwp-code에 완전히 동일한 버그가 그대로 있음** — `is_tac_table_inline`
(`height_measurer.rs:20`), `should_wrap_middle_anchored_table`(`paragraph_layout.rs:59`)
동일 계약으로 존재, `layout_inline_table_paragraph`(`paragraph_layout.rs:1428`) 안에
`num_leading = (offsets[0]/8) as usize`(1562행)와 열합산 `total_width`(1663~1665행)이
수정 전 upstream과 동일하게 남아있음. 이 함수는 rhwp-code 자체 이력(`281f21939` #4370,
`8d25b718f`)도 있어 cherry-pick 불가·수동 재적용 필요하나 **로직 자체는 거의 그대로
포팅 가능** — 최상위 후보.

**cellzone 테두리 (`950300ea7b` `6161b4a571`, #6619)** — `hp:cellzone`이 배경·대각선만
그리고 네 변을 방출하지 않던 버그, 병합 칸 주소 오인 수정. rhwp-code도
`table_layout.rs`/`table_layout/geometry.rs`에서 배경·대각선만 처리, 네 변 방출 로직
**없음** — 동일 버그 재현. `apply_table_outer_border_fill`류 지점에 얹으면 되는 구조,
난이도 중간, geometry.rs는 T4 분리 산물이라 구조 안정적.

**중첩/1×1 표 (`88ad1549f2` `394ffc23ba` `9088bd705c` `018b643227`, #4915/#6621/#6630/#6648)**
— reset 없는 다쪽 중첩 표 canonical 투영, 1×1 상자 unwrap 시 여백 처리, 셀 첫 문단
vpos 정렬. **rhwp-code가 가장 오래·깊게 자체 발산한 영역** — `#1658`/`#3738`/`#4042`/
`#4069`/`#4277` 등 독자 1×1 래퍼·중첩 표 처리 이력이 두껍고 `compute_table_y_position`
등 핵심 함수 구조가 upstream과 갈렸을 가능성 큼. **재조사 우선, `#5251` 패턴 반복
위험** — 이식보다 rhwp-code 쪽 함수를 먼저 정독해 같은 증상이 있는지부터 확인할 것.

---

## ③ EMF 컨버터 클러스터 (5개, 실질 4개)

`c823cd04d3`(PolylineTo16/PolyBezierTo16) → `bd159be023`(스톡 오브젝트 SelectObject) →
`b8b52fc9e8`(EMR_EXTCREATEPEN) → `51d9b4142b`(월드 변환+클립) 4단 연쇄.
`dbe17794d7`는 문서 주석만 수정, 포팅 대상 제외.

**실측: rhwp-code에 4개 모두 동일 버그 존재** — `src/emf/parser/mod.rs` 디스패치
테이블에 `RT_POLYLINE_TO16`/`RT_POLYBEZIER_TO16` 자체가 없어 `Record::Unknown`으로
버려짐(`player.rs:120`), `select_object`에 스톡 오브젝트 분기 전무,
`SetWorldTransform`/`ModifyWorldTransform`은 `player.rs:103-106`에 "단계 12에서는
저장만" 주석과 함께 명시적 no-op, 클립 레코드 처리 전무. 부수 발견:
`record_type.rs`의 `RecordType` enum이 `EmrPolyBezierTo16=0x60`/`EmrPolyLineTo16=0x61`로
잘못 정의(MS-EMF 실제값 0x58/0x59)돼 있으나 실제 디스패처에서 미참조되는 죽은 코드라
기능 버그는 아님(정리 시 같이 고칠 것).

파일 충돌: EMF 관련 rhwp-code 커밋 11개뿐, 최근 활동은 rustfmt뿐 — 낮음. 이식 난이도:
순수 로직 포팅 가능(같은 파일 경로). 단 upstream 커밋 메시지 자체가 "WMF 안에 실린
EMF 배선은 아직 안 켰다"고 명시 — 포팅해도 즉시 효과는 직접 삽입 EMF(`image/x-emf`)에
한정.

---

## ④ WMF 클러스터 (1개, `cb1df750d5`, #6617)

WMF→SVG 변환기의 y축 뒤집기가 세 겹으로 중첩된 구조적 버그: `point_s_to_absolute_point`의
`.abs()` 트릭, Stage D의 `translate(0,vb_h) scale(1,-1)` 그룹 재반전, `#6140`의 음수 높이
DIB 정규화가 서로 상쇄/충돌 — y-up 창이나 x축 반전 창에서 그림 소실 또는 좌우 반전.
`Window::to_device`로 창 좌표 변환을 정정하고 그룹 반전을 제거하는 3파일 리팩터
(88+173+93줄).

**실측: rhwp-code에 동일 버그 패턴 존재** — `device_context.rs:177,184`에 수정 전과
동일한 `(point.x - self.window.origin_x).abs()`, `mod.rs:142`에 동일한
`translate(0,{vb_h}) scale(1,-1)` Stage D 그룹. 파일 충돌: 전체 역사 7개 커밋뿐, 낮음.
좌표계 핵심 로직이라 이식 난이도는 중간 — diff 그대로 적용보다 로직 이해 후 재작성 필요.

---

## ⑤ PDF 클러스터 (1개, `7946a457d9`, #6612)

`src/renderer/pdf.rs`에 커스텀 `image_href_resolver` 추가 — usvg 기본 하위-SVG 로더가
SVG 규격상 버리는 중첩 `<image>`(비트맵 포함 WMF를 SVG로 감싼 것)를 직접 파싱해 유지.
단일 함수 + 옵션 필드 대입, 28줄.

**실측: rhwp-code `pdf.rs:967`에 `usvg::Options::default()`만 쓰고
`image_href_resolver` 미설정 — 동일 버그**(비트맵 품은 WMF/EMF 그림이 PDF 내보내기 시
빈칸). 파일 충돌: 최근 폰트 캐싱/병렬화 작업이 있었으나 해당 라인과 무관, 낮음.
이식 난이도: **8클러스터 중 가장 쉬움** — 독립 헬퍼 함수 추가 + 한 줄 옵션 대입.

---

## ⑥ 차트 클러스터 (2개, #6624)

`b1091864df`(글꼴 크기·계열 선 굵기·bar gapWidth, 없으면 한/글 기본값) →
`a8b89b34c0`(격자선·축선·눈금·테두리를 `c:txPr`/`c:spPr`/`c:majorGridlines` 등 차트 XML
선언대로) 순차 확장. 각 767줄/488줄(신규 계약 테스트 포함).

rhwp-code `src/ooxml_chart/`는 upstream `crates/rhwp-ooxml-chart/`와 파일 구조가 거의
1:1(`parser.rs`/`renderer.rs`/`mod.rs`↔`lib.rs`). `bar_gap_width`/`up_down_gap_width`
파싱, `axPos` 기반 primary/secondary 축 매핑은 **이미 존재**하지만
`text_size_pt`/`title_size_pt`/`line_width_emu`, `OoxmlAxis{majorGridlines,
majorTickMark, deleted}`는 **없음** — `renderer.rs`에 `stroke-width="2"/"0.5"/"0.75"`
하드코딩이 그대로 있어 같은 종류의 결함(차트 XML 선언 무시) 존재 추정.

파일 충돌: 표 레이아웃 등과 무관, 낮음. 다만 rhwp-code 기존 `axPos` 파싱과 upstream의
새 `OoxmlAxis` 모델을 그대로 붙이면 중복/충돌 — 단순 포팅이 아니라 기존 파서에 맞춘
재작성 필요. 이식 난이도 중~높음(로직 명확하나 물량 큼). 렌더링 정확성 버그라기보다
시각 품질 개선 — 우선순위 중.

---

## ⑦ HWPX note/앵커 vpos 클러스터 (2개)

둘 다 매우 국소적, 같은 파일에 대응 함수 존재.

- `0f9bc5c5ae`(#6535 잔여) — `src/renderer/typeset.rs` 단일 단 리셋 판정
  (`cv==0 && pv>5000`)이 쪽-앵커 블록의 저장 vpos=0을 "쪽 끊김" 신호로 오판. rhwp-code에
  기존 예외 함수 `para_is_page_bottom_fixed_table_anchor`가 **이미 존재**(#6535 base는
  이미 이식됨) — 이번 커밋은 옆에 놓는 "잔여" 예외 하나뿐. 34줄, 매우 낮은 위험, 즉시
  포팅 가능.
- `194ab18188`(#6495) — `src/parser/hwpx/section.rs`의 `normalize_hwpx_note_line_vpos`가
  vpos=0을 조건 없이 `expected`(prev+line_height+line_spacing)로 **무조건 덮어쓰는**
  구버전 로직 그대로 — upstream이 고친 "되감김 판별자" 분기 없음. rhwp-code는 upstream
  수정 *전* 상태와 거의 동일해 같은 버그 재현 가능성 높음. 36줄, 낮은 위험.

파일 충돌: 표 레이아웃과 무관, 낮음. 이식 난이도: 낮음(거의 그대로 포팅 가능, 함수명·
구조 일치). **작고 안전하며 재현 가능성 높은 실버그 — 우선순위 높음.**

---

## ⑧ 필드 리플로우 클러스터 (2개)

`ce2fb30b86`(wip, #6628 작업 중 발견한 product-only 패치)과 `9bc475bfb0`("reflow every
supported field owner")는 같은 파일(`src/document_core/queries/field_query.rs`)을 이틀에
걸쳐 이어 완성한 **하나의 논리적 패치**다 — `is_cell_field`/
`validate_field_reflow_location` 추가, `reflow_field_location_after_edit`을 `Result`
반환으로 바꿔 필드 소유 문단 경로 검증 실패를 조용히 무시하지 않게 함. 별도 이슈 번호는
`9bc475bfb0` 본문에도 없음(내부 task 문서 `task_m100_6641`만 존재).

rhwp-code `field_query.rs`에 `NestedEntry::TableCell/TextBox`,
`get_cell_paragraph_mut_by_path`, `reflow_field_location_after_edit`,
`field_body_flow_end` 등 **동일 API 이미 존재** — 이식 가능성 있으나
`reflow_field_location_after_edit`의 현재 반환 타입(`()` vs `Result`)과 호출부 전체
확인 필요. 이식 난이도 중, 우선순위 낮음~중. **두 커밋을 개별 포팅하지 말고
`9bc475bfb0` 기준 net diff로 평가할 것.**

---

## 9. 제외 확정(재검토 불필요) — gym/docs/release/ci 등 ~90개

`docs(gym)` 12, `docs` 9, `docs(review)` 7, `docs(release)` 6, `docs(field)` 5,
`test(field)` 4, `fix(gym)` 4, `docs(pr)` 3, `test(renderer)` 2, `test(gym)` 2,
`ci(gym)` 2, 기타 단발성 gym/ci/release 커밋(`3a944a3133` `07ab264e85` `512f2d9845`
`fa194b820e` `84d89dc509` `ccb163ca25` `8605399f9c` `b03faa1fb1` `3eb2e0d5a0` 등). 전부
upstream 내부 에이전트 훈련(gym)/문서/릴리스 프로세스 — 코어 엔진 fidelity와 무관, 2026-09-01
보고서 Tier 3 선례와 같은 성격.

## 10. 방법론 메모

- `rhwp-code`와 upstream은 같은 대형 파일(`table_layout.rs` 등)을 각자 다르게 쪼개는
  리팩터를 진행 중 — `git cherry-pick`은 높은 확률로 충돌, diff를 읽고 rhwp-code
  구조에 맞춰 수동 재적용할 것.
- **패치를 짜기 전에 rhwp-code에서 실제로 재현되는지부터 확인**(`#5251` 선례).
  이번 서베이에서 이미 8클러스터 중 다수(⑦⑤①-㉮③②-㉮②-㉯④⑥)는 실측으로
  재현을 확인했으나, ①-㉯·①-㉰·⑧은 아직 미확인 — 실제 이식 착수 전 재확인 필요.
  ②의 중첩/1×1 표는 rhwp-code 자체 발산이 가장 깊어 재현 여부 자체가 불확실.
- upstream 커밋 해시는 전부 `../rhwp` 저장소(`origin/devel`) 기준.
