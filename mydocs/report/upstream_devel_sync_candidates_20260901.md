# upstream devel 동기화 후보 조사 (2026-09-01)

- 비교 대상: `rhwp-code:main`(`.`) vs `origin/devel`(`../rhwp`, upstream `edwardkim/rhwp`).
- 동기화 지점(마지막 공유 커밋): `6f70cd1b6f25adc06bc6912251b683819626b35e`
  (`Merge pull request #4685 from edwardkim/pr/humdrum00001010-20260812`, 2026-08-12 23:06:32
  +0900). 이 커밋까지는 두 저장소에 커밋 오브젝트가 물리적으로 동일하게 존재한다
  (`git cat-file -t <hash>`가 양쪽 `.git`에서 같은 결과) — `rhwp-code`가 과거 upstream
  `devel`을 주기적으로 fetch+merge해왔기 때문이다. 이진 탐색으로 공유 이력의 정확한
  경계(인덱스 150/151)를 확인했다.
- upstream `main`은 `v0.8.4`(`496333b27`, 2026-08-12 09:09, "merge(release): v0.8.3 main
  계보를 devel에 동기화") 이후 정지해 있다. 실제 개발은 전부 `devel`에서 일어나므로
  이 문서는 `devel`을 기준으로 비교했다.
- 조사 시각: 2026-09-01. 방법: 위 이진 탐색으로 fork point 확정 →
  `git log 6f70cd1b6..origin/devel --oneline`/`--stat`/`--dirstat`로 전체 규모와 영역별
  분포 파악 → 영역별 대표 커밋을 `git show --stat`/`git log -p`로 실제 diff 확인.

## 1. 요약 수치

- upstream `devel`이 동기화 지점 이후 쌓은 커밋: **1,971개**, 기간 2026-08-12 ~ 09-01(약 20일).
- 전체 diff(`git diff --shortstat`): 16,812개 파일, +5,652,531 / -92,312줄(대용량 바이너리·
  코퍼스·시각 회귀 자산 포함). **`src/`만: 769개 파일, +238,446 / -90,014줄.**
  - `tests/`: 1,531개 파일, +202,109/-5,303줄
  - `rhwp-studio/`(UI): 279개 파일, +46,715/-2,887줄
  - `crates/`(신규): 14개 파일, +12,499줄
  - `mydocs/`: 4,299개 파일, +993,492줄 (PR별 셀프리뷰/보고서 아카이브 문화)
  - `.claude/skills/`: 2,880개 파일, +176,985줄
- 커밋 메시지 prefix 분포: fix 476, docs 430, feat 262, test 180, chore 37, refactor 36,
  ci 22, style 19, perf 16, merge 16.
- 상위 scope: `fix(renderer)` 94, `docs(report)` 62, `fix(studio)` 57, `docs(review)` 52,
  `docs(font)` 49, `feat(font)` 45, `fix(ci)` 35, `feat(gym)` 35, `test(font)` 31,
  `docs(pr)` 31, `feat(cli)` 30, `refactor(cli)` 28, `fix(test)` 27, `feat(agent)` 27,
  `fix(hwpx)` 24, `fix(layout)` 23, `feat(studio)` 21, `fix(serializer/hwpx)` 11,
  `fix(typeset)` 10, `fix(parser/hwp3)` 7, `fix(hwp3)` 7.
- `src/` 내부 라인 비중(parser): `hwpx/` 43.2%, `parser/`(공통) 20.7%, `hwp3/` 16.5%,
  `body_text/` 12.8%, `hml/` 3.4%, `control/` 3.1%. renderer는 `src/` 내부 라인 기준
  37.2%로 가장 큰 영역(`layout/` 2.8%, `composer/` 1.3%, `font_rule_projections/` 1.5% 등
  세분).
- 참고: `rhwp-code:main`도 같은 동기화 지점 이후 **자체 104개 커밋**을 쌓았다 — 7절 참조.

## 2. 방법론 주의사항

- **1,971개 커밋을 전수 검토하는 것은 비현실적**이다. 아래 티어를 파도(wave) 단위 검토
  순서로 쓴다: Tier 1부터 개별 diff를 확인하고, 필요·여력에 따라 Tier 2, Tier 3로 내려간다.
- `rhwp-code`와 upstream은 **같은 대형 파일들을 서로 다른 방식으로 쪼개는 리팩터**를
  각자 독자적으로 진행했다(`main.rs`, `table_layout.rs`, `wasm_api.rs`, `hwpx/section.rs`
  등). 이런 파일이 걸린 upstream 커밋은 `git cherry-pick`을 시도하면 높은 확률로 충돌하거나
  엉뚱한 곳에 적용된다. **커밋의 의미(diff)를 읽고 현재 rhwp-code의 분할 구조에 맞는
  위치에 수동으로 재적용**하는 방식을 권장한다. 이는 이미 `mydocs/manual/
  patch_stack_upstream_sync.md`가 다루는 것과 같은 원칙이다.
- 커밋 해시는 모두 `../rhwp` 저장소(`origin/devel`) 기준이다.

## 3. Tier 1 (High) — 우리 도메인과 직접 겹치고 이식/대조 가치가 큼

| 커밋 | 메시지 | 주요 경로 | 왜 중요한가 |
|---|---|---|---|
| `6e1a4e629` | fix(serializer/hwpx): curve를 hp:seg 체인으로 저장 — 한글 크래시 (#4676) | `src/serializer/hwpx/*` | ✅ **완료** — `main`에 `9a2dc2e88`로 이미 이식됨(2026-09-01). `curve_segs_xml()`이 `<hp:seg type="LINE\|CURVE">` 체인을 방출하고, `<hc:pt>` 미방출을 검증하는 회귀 테스트(`issue4676_curve_emits_seg_chain_not_pts`, `src/serializer/hwpx/section.rs`)까지 존재. HWPX 파서 쪽도 `segment_types` 왕복 확인됨. 후속 작업 없음 — 실물 curve 샘플 픽스처가 `samples/`에 없어 유닛 테스트 수준 커버리지뿐이라는 점만 참고(원하면 실물 샘플 추가 가능, 급하지 않음). |
| `718ce06d0` | fix(renderer): overlay 표 필러 흐름 복원과 상향 클램프 해제 — 6쪽 표 겹침 소멸 (#4514) | `src/renderer/*` | ✅ **완료** — 이 커밋 자체가 `main`의 조상(2026-08-11, `d522719a1a`로 양쪽에 동일 해시 존재 — 별도 이식이 아니라 원래 공유 이력의 일부). `typeset.rs`의 `overlay_shape_shortcut_para`, `table_layout.rs`의 `overlay_multirow_rowbreak` 클램프 우회 전부 현재 코드에 그대로 있음. `LAYOUT_TABLE_OVERLAP` 진단(`#4515`, `db62298e6`)과 회귀 테스트(`tests/issue_4515_table_overlap_diag.rs`), 리포 샘플(`samples/issue4514/sample1-repro.hwp`)까지 이미 존재. 후속 `#4568`(페이지 분할 리페인트) 알려진 한계는 별도 추적 중. |
| `bd8919896` 외(#6303 계열) | fix(layout): 셀 자동 축소 자간을 안쪽 폭에 수렴시킨다 | `src/renderer/layout/*` | ✅ **완료** — `main`에 `657d8a21e`로 이식됨(2026-09-01, upstream #6196/#6389/#6303 통합). `paragraph_layout.rs`의 `converge_cell_overflow_char_spacing()`이 최대 4회 반복 수렴. upstream 자신도 최초 무조건 버전을 되돌리고 `suppress_cell_overflow_spacing`이 없을 때만(=셀 내용이 이미 정확히 안쪽 폭에 맞춰 저장된 경우만) 적용하도록 게이팅했는데, 이식본도 동일하게 게이팅되어 있음. 회귀 테스트 `issue_6303_cell_shrink_convergence_tests`(`paragraph_layout.rs`), `tests/issue_6389_cell_stored_ladder_compresses_to_fit.rs` 존재. **미해결 잔여**: 포팅 시 제외된 `#6196` 원본 샘플(`samples/issue6196/cell_char_spacing_fit.hwp`)이 같은 페이지 다른 셀에 별개의 overflow 버그가 있어 아직 미조사 — 별도 이슈로 추적할 것. |
| `41475dd5e` / `94036e467` / `6f7f5c56f` (#4318) | fix(layout): 미주 다단 마지막 단 줄넘김 연작 | `src/renderer/layout/*` | ✅ **완료** — `main`에 `616767315`(`6f1deb5c5`와 동일 메시지, 스쿼시 이식)로 2026-09-01 이식됨. upstream 5개 커밋(본 수정 + rustfmt + 프로파일 한정 + vpos=0 리셋 인접 처리 + 리셋-인접 처리를 진짜 오버플로로 한정)을 전부 검토해 하나로 통합. `ENDNOTE_LAST_COLUMN_SPLIT_BLEED_PX`(24→4px) 도입, `tests/issue_4318_endnote_last_column_frame.rs` 신규, 기존 `#1355`/`#1082` 관련 테스트 임계값·회귀 가드 유지 확인. |
| `3431a7727` (#5251) | fix(hwpx): HWP3 원본 char_shapes 경계를 HWPX 재파싱에서 지킨다 | `src/parser/hwpx/*` | 🚫 **조사 완료, 이식 안 함 — rhwp-code에는 재현되는 버그가 없음(2026-09-03 재조사)**. 아래 별도 절 참고. |
| `d7f90eb00` (#5873) | fix(hwpx): 표 셀 안 구역 나누기를 secPr로 내보낸다 | `src/serializer/hwpx/*` | ✅ **완료** — `main`에 `033d1267c`로 이식됨(2026-09-01). |
| `d5dada7c1` (#5861) | fix(hwpx): 사용자 정의 기호 0xA807을 평면 15 사상표에 넣는다 | `src/parser/hwpx/*` | ✅ **완료** — `main`에 `3fffc4e58`로 이식됨(2026-09-01, upstream #5140/#5861 통합). |
| `ce8015138` (#6380) / `1d2674857` (#5860) | fix(parser/hwp3): 사적 문자 소실 방지 / 매핑 없는 사적 문자를 한글이 내는 글자로 해석 | `src/parser/hwp3/*` | ✅ **완료** — `main`에 `049210a0e`로 이식됨(2026-09-01). |
| `b14557e0a` (#5141) | fix(parser/hwp3): 묶음 개체 세부 길이 8바이트 오류로 인한 자식 도형 소실 방지 | `src/parser/hwp3/*` | ✅ **완료** — `main`에 `0dd135581`로 이식됨(2026-09-01). |

**요약(2026-09-03 재조사)**: Tier 1 9개 항목 중 8개가 이미 `main`에 이식 완료 상태였다 —
이 문서가 갱신되지 않아 최근까지 "미착수 후보"로 잘못 남아 있었다(`main`과 별개로
`upstream-sync/tier1-devel-20260901`라는 로컬 전용 브랜치가 같은 작업을 독자적으로
반복 수행했는데, 그 브랜치를 `main`에 합치치도 이 문서를 갱신하지도 않은 채 방치된
것으로 보인다 — 그 브랜치의 9개 커밋은 전부 `main`의 커밋과 메시지가 동일한 중복이라
고유 작업이 없음, 정리 대상). 실제로 남은 Tier 1 미해결 항목은 **`#5251` 단 하나**다.

### `#5251` (HWP3→HWPX char_shapes 경계) 재조사 결론 — 처음 진단은 틀렸다

2026-09-02 재보류 결론("네이티브 HWP3 파서의 단위 체계 자체를 확장 단위로 바꿔야 하는
더 큰 작업")은 **틀렸다**. 2026-09-03 재조사(upstream 4커밋 체인 전체 확인)로 밝혀진 것:

1. **upstream `3431a7727`은 upstream 자신의 최종 수정이 아니다.** 같은 날 이어지는
   4커밋 체인의 첫 번째일 뿐이다: `3431a7727`(초판, 전역 게이트) →
   `380525090`(upstream 자신이 "전역 hwp3-origin FFFC=8과 pageNum/footer 생략이
   #3532·#5542·char_count 왕복을 깨뜨렸다"고 명시하며 **좁은 게이트**
   `hwpx_hwp3_issue_5251_axis`로 교체 — 한 문단 안에 FFFC **와** `Footer` 컨트롤
   **과** `PageNumberPos`가 **동시에** 있을 때만 발동, `PAGE_FOOTER_SLOT_PART`
   sentinel + 위치 재사상 함수 사용) → `7dde31cfd`/`efa65a482`(부수 테스트 수정).
2. **2026-09-02 이식 시도는 이 4개 중 첫 번째(이미 알려진 결함이 있는 버전)만
   이식했다** — upstream 자신도 몇 시간 뒤 되돌린 "HWP3 origin이면 전역으로 FFFC=8·
   footer/pageNum 슬롯 생략" 게이트를 그대로 재현했다. 게다가 upstream의 가드
   구성 코드(`Hwp3OriginSourceGuard` RAII, `#4916`/`#3518` 계열) 자체가 rhwp-code에
   없어서 처음부터 새로 만들어야 했는데, 그때 좁은 조건이 아니라 "HWP3 origin
   마커가 있으면 무조건" 식의 전역 게이트로 구현된 것으로 보인다. `samples/issue_265.hwp`가
   회귀한 이유는 이 샘플이 좁은 `#5251` 패턴(FFFC+Footer+PageNumberPos 동시 존재)이
   아닌데도 전역 게이트에 걸려 영향을 받았기 때문이다.
3. `src/parser/hwp3/mod.rs:1922-1924`에 명시적 주석이 있다: NewNumber/PageNumberPos/
   PageHide/CharOverlap/Field/IndexMark/Outline/미인식 TOC 참조 등 FFFC를 내는 대부분의
   HWP3 컨트롤을 **의도적으로** 1유닛으로 유지한다 — 네이티브 HWP3 렌더링이 저장된
   `LineInfo`/`CharShape` 위치를 단일-마커 관례로 계산하기 때문에, 전역으로 8유닛으로
   넓히면 네이티브 HWP3 레이아웃 자체가 밀린다. **즉 "HWP3 파서가 항상 8유닛을 내게
   고친다"는 애초에 선택지가 아니다** — 이건 실수가 아니라 문서화된 의도적 트레이드오프다.
   `char_offsets`/`char_shapes`는 레이아웃·표·검색/커서·편집 커맨드 등 수십 개 파일에서
   원시 오프셋 산술로 직접 참조되므로(예: `src/model/paragraph.rs`에 `+8` 리터럴이
   그대로 박혀 있음), 네이티브 파서의 일반 단위 체계를 건드리는 건 여전히 위험 범위가
   넓다 — 하지만 **좁게 게이팅된 HWPX 재파싱 경로 안에서만** 보정하면 이 위험을 전부
   피할 수 있다(upstream의 최종 설계와 일치).

**실측(2026-09-03) — 좁은 게이트를 실제로 구현·검증한 결과: 이식하지 않기로 결정.**
위 분석대로 upstream 4커밋(`380525090`/`7dde31cfd`/`efa65a482` 포함, `Hwp3OriginSourceGuard`
스레드로컬 + `hwpx_hwp3_issue_5251_axis` 좁은 게이트 + `PAGE_FOOTER_SLOT_PART` sentinel +
`hwpx_map_std_pos_to_5251_axis` 위치 재사상)를 rhwp-code의 분할 구조(`section.rs`/
`section/paragraph_parsing.rs`)에 맞춰 실제로 구현했다. 컴파일 통과, `render-diff
samples/issue_265.hwp --via hwpx` PASS(0px, 회귀 없음) 확인 — 여기까지는 성공.

그런데 **저장소 전체 277개 `.hwp` 샘플을 HWP3 형식으로 필터링해 좁은 패턴(FFFC+Footer+
PageNumberPos 동시 존재)을 전수 스윕한 결과, 후보가 단 2건(`issue_265.hwp`,
`hwp3-sample.hwp` — 사실상 같은 문서의 사본으로 보임)뿐이었고, 이 2건 모두 **패치
적용 여부와 무관하게 이미 네이티브 파싱과 HWPX 왕복 사이에 문자 단위 char_shape_id
정렬이 완전히 일치**했다(직접 문자별 대조로 확인, `--verify`/`ir-diff` 우회). 패치가
실제로 다른 결과를 낼 수 있는 유일한 시나리오는 **진짜로 짝지어진 개체(Table/Picture/
Shape 컨트롤)가 Footer+PageNumberPos와 한 문단에 동시에 있는 경우**뿐인데(네이티브
파서가 이 경우 보이는 FFFC 문자 대신 "보이지 않는 8유닛 anchor gap"으로 처리하는 별도
경로, `hwp3/mod.rs:1907-1919` `preserve_invisible_anchor_gap`) — **이 조합은 저장소의
샘플 어디에도 존재하지 않는다.**

**결론**: upstream #5251은 upstream 자신의 네이티브 HWP3 파서가 FFFC를 (거의) 항상
8유닛으로 세는 설계라서 HWPX 재파싱 기본값(1유닛)과 실제로 어긋나는 것이지만,
rhwp-code의 네이티브 HWP3 파서는 시각적으로 보이는 FFFC 문자를 내는 모든 컨트롤에서
**이미 항상 1유닛**을 쓰고(`hwp3/mod.rs:1927`), HWPX 재파싱 기본값도 문자 그대로
1유닛이라 — **애초에 두 축이 어긋날 수가 없는 구조**다. 즉 `#5251`은 rhwp-code
아키텍처에는 적용되지 않는, upstream 고유의 결함이다. 구현했던 패치(스레드로컬 +
좁은 게이트 + 재사상 함수, ~150줄)는 되돌렸다(`git checkout`, 커밋 없음) — 확인된
버그가 전무한 상태에서 코드 복잡도만 늘리는 것은 이 프로젝트의 최소주의 원칙에
어긋난다. 향후 실제로 짝지어진 개체+Footer+PageNumberPos 조합을 가진 HWP3 실물
문서가 발견되면 이 절의 분석(특히 `hwp3/mod.rs:1907-1919`)을 재사용해 재평가할 것 —
그 전까지는 **Tier 1 전 항목이 완료 상태**다.

## 4. Tier 2 (Mid) — 가치는 있으나 이식 비용·구조 충돌 위험이 크거나 간접적

| 커밋 | 메시지 | 주요 경로 | 비고 |
|---|---|---|---|
| `193034df3` (#5511) | refactor: MCP metadata 모듈 추출 | `src/main.rs`(-4331줄) → `src/cli/metadata/mcp/{advanced,edit_content,edit_format,edit_structure,exchange,protocol,read,mod}.rs` | MCP 서버 구조 정리. rhwp-mcp-session 관련 작업 시 구조 참고 가치는 있으나, `main.rs` 분할 방향이 rhwp-code(`main/{batch,convert,edit,...}.rs`)와 달라 직접 이식은 어렵다 — 로직만 참고. |
| `9aa043c2c`~`e3e2aa21a` (#4100, Stage 1~6) | feat(ooxml_chart)/feat(serializer)/feat(cli): 차트 CSV 왕복 | `src/ooxml_chart/*`, `src/serializer/*`, `src/document_core/*`, `src/cli/commands/*` | ~~HWPX 임베드 OOXML 차트 값을 CSV로 뽑고(`chart-to-csv`) 되넣는(`csv-to-chart`) 완전히 새로운 기능. rhwp-code에 없는 기능 격차 — 31개 파일, +6,166줄로 이식 비용은 크지만 통째로 검토할 가치가 있다.~~ **정정(2026-09-01, Tier 2 착수 조사)**: 이 항목은 전제가 낡았다 — `9aa043c2c`~`e3e2aa21a`는 이미 rhwp-code HEAD의 조상(2026-08-11자, 동기화 지점 이전)이며 `src/ooxml_chart/*`와 `chart-to-csv`/`csv-to-chart` 명령이 이미 구현·문서화(`mydocs/manual/cli_commands.md`)되어 있다. **이번 Tier 2 라운드 대상에서 제외.** |
| `ba097d6bf` (#5185), `e0851908b` (#5192) 등 | feat(cli): 편집 명령 13건/29건 통합 | `src/cli/commands/edit/*`, `tests/cases/*_contract.rs` | `edit` 서브커맨드 42개+ 신규(`set-section-def`, `set-table-props`, `move-table`, `transpose-table`, `set-column-widths` 등). feature parity 가치는 크지만 rhwp-code CLI 구조가 이미 다르게 쪼개져 있어 명령 단위 개별 선별 이식을 권장. **진행(2026-09-02)**: `move-table`/`transpose-table`/`set-column-widths`/`set-table-props`/`set-section-def` 5건은 rhwp-code에 이미 있던 네이티브 함수(`table_ops.rs`의 `move_table_offset_native`/`transpose_table_cells_in_place_native`/`set_table_column_widths_native`/`set_table_properties_native`, `queries/rendering.rs`의 `set_section_def_native`)에 CLI 배선만 추가해 이식 완료. `set-table-props`/`set-section-def` 모두 착수 전 upstream 원 커밋(각각 `4909e3922`, `0e908e344` — `ba097d6bf` 통합 이전 원본)을 선확인한 결과, 이 라운드의 첫 보고서가 "신규 IR 필요"로 잘못 분류했던 것과 달리 rhwp-code에 이미 동형(또는 더 성숙한) 네이티브 함수가 있어 저위험으로 확정됐다 — 표본 5개 전부 결국 순수 CLI 배선 작업이었다. 나머지 ~37개는 다음 라운드 대상. |
| 신규 crate | `crates/rhwp-contracts` | `crates/rhwp-contracts/*`(14파일, +12,499줄) | ~~계약 테스트 인프라. rhwp-code의 work-receipt/캡슐 검증 체계(`rhwp replay/audit/lineage`)와 시너지 가능성이 있어 검토 가치.~~ **정정(2026-09-02, 3c 조사)**: 전제가 낡았다 — 실제 내용을 읽어보니 work-receipt/캡슐과 무관하고, `ir_schema`/`ontology`/`provenance`/`schema_registry` 4개 모듈을 별도 crate로 물리적으로 옮긴 것뿐이다(`lib.rs` 자체 주석: "공개 API를 보존하면서 해당 단위 테스트를 루트 rhwp 테스트 바이너리와 독립적으로 컴파일"하기 위한 **빌드 격리 리팩터**). rhwp-code는 이 4개 모듈을 이미 같은 이름(`src/{ir_schema,ontology,provenance,schema_registry}.rs`)으로 갖고 있다 — `ir_schema.rs`/`ontology.rs`는 줄 수까지 일치, `provenance.rs`/`schema_registry.rs`의 줄 수 차이는 내용 차이가 아니라 upstream이 그사이 늘린 명령 수(`charts`/`explore`/`word-count`/`bookmarks` 등, rhwp-code가 아직 안 가진 별개 기능들)만큼 출처 표지 항목이 늘어난 것이다. **포팅 가치 없음 — 이번 라운드에서 제외.** MCP metadata 리팩터(#5511)와 같은 성격(구조 정리, fidelity 무관)으로 재분류. |
| `61e439043` (#5932) | feat: info에 한컴오피스 마지막 저장 버전 표시 | `src/cli/*`, `src/document_core/*` | 소규모 실용 진단 기능. |

## 5. Tier 3 (Low) — 참고만, 우선순위 낮음

- `rhwp-q-*` 조회 전용 마이크로 CLI 54종(`2d897ca04`, #5674): `src/bin/rhwp-q-*`
  (예: `rhwp-q-cursor-model`, `rhwp-q-page-items`, `rhwp-q-char-shape` 등). 에이전트가
  문서 상태를 저비용으로 조회하도록 설계된 초소형 CLI 계열. 아이디어는 참고할 만하나
  코어 엔진 fidelity와 무관해 우선순위는 낮다.
- `gym/packs/*` 에이전트 훈련/평가 프레임워크(`28131af64` 등) — 코어 엔진과 무관.
- 폰트/글리프 CanvasKit 세로쓰기 프로젝트(~140개 커밋, #4969, 예: `c9b551909`) —
  대규모 아키텍처 변경. 세로쓰기/CanvasKit 요구가 당장 없다면 낮은 우선순위지만,
  커닝/폰트 렌더링 버그 수정이 섞여 있을 수 있어 필요 시에만 개별 검토.
- 브라우저 확장(chrome/vscode/firefox) 업데이트(13개 파일, +1,851/-415줄) — 소규모.
- `mydocs/` 문서·PR 셀프리뷰 아카이브 문화(4,299개 파일) — 코드 fidelity와 무관, 다만
  이 fork도 이미 유사한 문서 규율(canonical manifest, front matter)을 갖추고 있어
  구조적으로는 비교 대상이 아니라 참고 사례에 가깝다.

## 6. 대형 병합 지점 (참고)

- `13ae331db`: "통합: CI 통과 open PR 15건을 반영한다 (#6001)" — 병렬로 진행되던 다수
  작업 흐름이 devel에 합류하는 지점. upstream 워크플로 이해에 참고.

## 7. 참고 — rhwp-code 자체 발산 커밋

같은 동기화 지점(`6f70cd1b6`) 이후 `rhwp-code:main`은 upstream에는 없는 **자체 104개
커밋**을 쌓았다. 대표적으로:

- `4929e5d15`(현재 HEAD): fix(renderer): renormalize column widths on solver overshoot
- wasm_api 4분할 무변동 리팩터: `c42f66544`(api_queries), `bdf12a668`(api_export),
  `8f54711d9`(api_editing), `90a35506e`(api_clipboard)
- renderer 무변동 분리: `699224eb2`(svg/drawing.rs), `c5ad4e764`(renderer/tests.rs),
  `28824d531`(json/text_export.rs)
- parser 무변동 분리: `b068e14f7`(hwpx/section/{table_parsing,paragraph_parsing}.rs)
- `f9d08eb7c`: patch(composer) — CJK 줄 재래핑 임계값 ×1.15 실험 patch(main 병합 대상 아님)

이들은 대부분 "무변동(behavior-preserving) 파일 분할" 리팩터와 표 레이아웃 정밀도
작업이다. 위 Tier 1~2의 upstream 커밋을 실제로 이식할 때는, 이 104개 커밋이 건드린
동일 파일(특히 `wasm_api`, `renderer/table_layout`, `parser/hwpx/section`)과의 구조적
충돌 가능성을 별도로 확인해야 한다.

## 8. 부록 — 편집 명령 13건/29건(#5185/#5192) 나머지 43개 개별 트리아지 (2026-09-02)

4절의 "편집 명령 13건/29건 통합" 항목을 재조사했다. 먼저 정정: 이미 이식한
`set-table-props`/`set-section-def`/`move-table`/`transpose-table`/`set-column-widths`
5건은 **이 두 커밋(`ba097d6bf`/`e0851908b`)에 실제로는 포함되어 있지 않다** — 같은
시기의 별도 커밋(`4909e3922`, `0e908e344` 등)이었다. 따라서 `ba097d6bf`+`e0851908b`가
도입한 실제 43개 명령은 5건과 전혀 겹치지 않으며, 이번 부록이 그 43개 전부를 다룬다.

**방법**: 상용구(--table/--row/--col 류 인자, `-o`/`--dry-run`/`--verify`/`--json`)를
갖는 얇은 CLI 배선인지 판정하기 위해, 각 명령이 부를 만한 코어 네이티브 함수가
rhwp-code에 이미 있는지 이름 패턴으로 스윕했다(예: `insert-table-row` →
`insert_table_row_native`).

**결과: 43개 전부 rhwp-code에 동형 네이티브 함수가 이미 있다** — 신규 IR/편집
로직이 필요한 항목이 하나도 없다. `set-table-props`/`set-section-def`와 정확히 같은
패턴: rhwp-code와 upstream이 공유했던 성숙한 코어 위에 upstream만 나중에 CLI/MCP
배선을 추가한 것이다.

| upstream 명령 | 대응 네이티브 함수(rhwp-code, 위치 생략) |
|---|---|
| `insert-text`/`delete-text` | `insert_text_native`/`delete_text_native` |
| `insert-paragraph`/`delete-paragraph`/`merge-paragraph` | `insert_paragraph_native`/`delete_paragraph_native`/`merge_paragraph_native` |
| `insert-page-break`/`insert-column-break` | `insert_page_break_native`/`insert_column_break_native` |
| `insert-row`/`insert-col`/`delete-row`/`delete-col` (표) | `insert_table_row_native`/`insert_table_column_native`/`delete_table_row_native`/`delete_table_column_native` |
| `merge-cells`/`split-cell` | `merge_table_cells_native`/`split_table_cell_native` |
| `insert-footnote`/`insert-endnote`/`delete-footnote` | `insert_footnote_native`/`insert_endnote_native`/`delete_footnote_native` |
| `add-bookmark`/`delete-bookmark`/`rename-bookmark` | `add_bookmark_native`/`delete_bookmark_native`/`rename_bookmark_native` |
| `delete-table` | `delete_table_control_native` |
| `insert-header-footer` | `create_header_footer_native` |
| `insert-header-footer-text`/`delete-header-footer` | `insert_text_in_header_footer_native`/(삭제 경로 별도 확인 필요) |
| `set-header-footer-text` | `insert_text_in_header_footer_native`+`delete_text_in_header_footer_native` 조합으로 추정(전용 setter는 미확인) |
| `set-hf-picture` | `set_header_footer_picture_properties`(이미 `pub`, wasm_bindgen 래퍼만 있고 `_native` 접미 없음 — 이름 예외) |
| `apply-hf-template` | `apply_hf_template_native` |
| `delete-hf-text` | `delete_text_in_header_footer_native` |
| `insert-field-in-hf` | `insert_field_in_hf_native` |
| `split-paragraph-in-hf`/`merge-paragraph-in-hf` | `split_paragraph_in_header_footer_native`/`merge_paragraph_in_header_footer_native` |
| `toggle-hide-hf` | `toggle_hide_header_footer_native` |
| `apply-char-format`/`apply-para-format`/`apply-style` | `apply_char_format_native`/`apply_para_format_native`/`apply_style_native` |
| `split-paragraph` | `split_paragraph_native` |
| `set-numbering-restart` | `set_numbering_restart_native` |
| `apply-para-format-in-hf` | `apply_para_format_in_hf_native` |
| `apply-endnote-shape` | `apply_endnote_shape_native` |
| `insert-footnote-text` | `insert_text_in_footnote_native` |
| `delete-text-in-footnote`/`split-paragraph-in-footnote`/`merge-paragraph-in-footnote`/`apply-para-format-in-footnote` | 이름 그대로 대응하는 `*_in_footnote_native` 4종 |

**주의**: 이 표는 "함수 이름이 존재한다"까지만 확인한 1차 스윕이다. 실제 CLI 배선
착수 전에는 `set-table-props`/`set-section-def` 때처럼 **각 명령마다 upstream 원
diff를 `git show`로 읽어 인자 이름·의미가 정확히 대응하는지, 그리고 rhwp-code
쪽 함수의 시그니처(특히 좌표계 — 문단/컨트롤 인덱스 vs export-tables 격자 좌표)가
같은지** 재확인해야 한다(문서의 "후보 커밋 선확인 필수 규칙"). 특히
`insert-header-footer-text`/`set-header-footer-text`/`delete-header-footer`
3건은 이번 스윕에서 정확한 1:1 대응을 못 찾아 착수 전 별도 확인이 필요하다.

**다음 라운드 착수 순서 제안**(문서의 착수 순서 원칙 그대로): 43개 모두 저위험
와이어링이므로 순서보다 배치 크기가 관건이다 — 한 라운드에 5~8개씩 묶어(관련
영역별로: ~~표 편집 6종~~ → 각주/미주 7종 → 머리말/꼬리말 9종 → 서식/스타일 6종 →
책갈피/구조 5종 → 문단 기본 6종 → 나머지) PR을 나누는 편이 한 PR에 43개를 몰아
리뷰 난이도를 키우는 것보다 낫다.

**진행(2026-09-02)**: "표 편집 6종" 배치(`insert-row`/`insert-col`/`delete-row`/
`delete-col`/`merge-cells`/`split-cell`) 완료 — `insert_table_row_native`/
`insert_table_column_native`/`delete_table_row_native`/`delete_table_column_native`/
`merge_table_cells_native`/`split_table_cell_native`에 CLI 배선만 추가했다.

**진행(2026-09-02, 계속)**: "각주/미주 7종" 배치(`insert-footnote`/`insert-endnote`/
`delete-footnote`/`insert-footnote-text`/`delete-text-in-footnote`/
`split-paragraph-in-footnote`/`merge-paragraph-in-footnote`) 완료 —
`insert_footnote_native`/`insert_endnote_native`(둘 다 `object_ops/note.rs`)와
`delete_footnote_native`/`insert_text_in_footnote_native`/
`delete_text_in_footnote_native`/`split_paragraph_in_footnote_native`/
`merge_paragraph_in_footnote_native`(전부 `footnote_ops.rs`, 전부 이미 `pub`)에 CLI
배선만 추가했다. 부수 발견: `merge-paragraph-in-footnote`는 병합 경계 앞뒤가 같은
글자모양일 때 저장 후 재파싱에서 인접 동일 `charShape` 항목이 정리되어 `--verify`가
`diffCount:1`(무해, `render-diff`로 시각 회귀 없음 확인)을 보고하는 사전 존재
특성이 있다 — cli_commands.md에 기록, 코어 수정은 이번 라운드 범위 밖.

**진행(2026-09-02, 계속)**: "머리말/꼬리말 9종" 배치 중 8건 완료 —
`insert-header-footer`/`delete-header-footer`/`insert-header-footer-text`/
`set-header-footer-text`/`set-hf-picture`/`apply-hf-template`/`delete-hf-text`/
`insert-field-in-hf`가 각각 `create_header_footer_native`/
`delete_header_footer_native`/`insert_text_in_header_footer_native`/
(`get_header_footer_para_info_native`+`delete_text_in_header_footer_native`+
`insert_text_in_header_footer_native` 조합)/`set_header_footer_picture_properties_native`
(`object_ops/picture.rs`)/`apply_hf_template_native`/
`delete_text_in_header_footer_native`/`insert_field_in_hf_native`에 CLI 배선만
추가했다 — 착수 전 우려했던 "`insert-header-footer-text`/`set-header-footer-text`/
`delete-header-footer` 1:1 대응 불확실"은 upstream 실제 diff를 읽어 전부 해소됐다.

**9번째 `toggle-hide-hf`는 배선 도중 되돌렸다 — 실제 회귀 발견**: 코어
`toggle_hide_header_footer_native`가 다루는 `hidden_header_footer` 필드는
`DocumentCore`/`LayoutEngine`에만 존재하는 **세션 전용 렌더 캐시 힌트**이며(`grep`으로
전 사용처를 확인) 어떤 직렬화 코드에도 연결돼 있지 않다 — 문서를 새로 열 때마다
빈 `HashSet::new()`로 리셋된다. CLI로 "토글 → 저장 → 다시 열기 → 다시 토글"을 실제로
구동해 통합 테스트를 돌려 확인했다: 저장된 파일은 입력과 완전히 동일하고, 두 번째
토글도 첫 토글의 효과가 하나도 남지 않아 다시 "숨김"으로만 나온다(`hidden:true`
두 번 연속). `--verify`도 트리비얼하게 통과한다(애초에 IR을 바꾸지 않으므로).
즉 이 명령을 upstream처럼 "새 문서를 만드는 CLI 편집"으로 배선하면 **파일을 만드는
것처럼 보이지만 실제로는 아무것도 영구화하지 않는 오해 소지 있는 명령**이 된다.
코드는 되돌렸다(`git checkout` 없이 직접 제거, 순수 배선 코드라 커밋 이력에 남지
않음) — cli_commands.md에 이유를 기록했다. 향후 이 기능이 진짜 필요하면 이미
직렬화되는 `SectionDef.hide_header`/`hide_footer`(구역 전체, `set-section-def`로
이미 커버됨)나 `Control::PageHide`(문단 단위, `section.rs`의 `pageHiding` 파싱이
이미 지원) 중 하나를 실제로 조작하는 **새 코어 함수**를 설계해야 한다 — 이는
"기존 함수 배선"이 아니라 별도 기능 개발이라 이번 라운드 범위 밖이다.

**진행(2026-09-02, 계속)**: "문단 기본 6종" 배치(`insert-text`/`delete-text`/
`insert-paragraph`/`delete-paragraph`/`merge-paragraph`/`split-paragraph`) 완료 —
`text_editing.rs`의 `insert_text_native`/`delete_text_native`/
`insert_paragraph_native`/`delete_paragraph_native`/`merge_paragraph_native`/
`split_paragraph_native`(전부 이미 `pub`)에 CLI 배선만 추가했다. `insert-text`/
`insert-paragraph`는 upstream 원본대로 `--dry-run`에서도 구역/문단/오프셋 범위를
미리 검사한다(나머지 4개는 안 함). 부수 발견: `merge-paragraph`도 각주/미주
배치의 `merge-paragraph-in-footnote`와 동일하게, 병합되는 두 문단의 경계
`charShape` id가 우연히 같으면 저장 후 재파싱에서 인접 동일 항목이 정리돼
`--verify`가 `diffCount:1`(무해, `render-diff` PASS 0px로 시각 회귀 없음 확인)을
보고하는 사전 존재 특성이 재현된다 — cli_commands.md에 기록, 코어 수정은 이번
라운드 범위 밖.

**진행(2026-09-03)**: "서식/스타일 6종" 배치(`apply-char-format`/`apply-para-format`/
`apply-style`/`apply-para-format-in-hf`/`apply-para-format-in-footnote`/
`apply-endnote-shape`) 완료 — `formatting.rs`의 `apply_char_format_native`/
`apply_para_format_native`/`apply_style_native`, `header_footer_ops.rs`의
`apply_para_format_in_hf_native`, `footnote_ops.rs`의
`apply_para_format_in_footnote_native`, `object_ops/shape.rs`의
`apply_endnote_shape_native`(전부 이미 `pub`)에 CLI 배선만 추가했다. 인자·시맨틱은
upstream `origin/devel`(`../rhwp`)의 `src/cli/commands/edit/formatting.rs` /
`header_footer_properties.rs` / `note_content.rs`를 직접 읽어 확인했다(이 세
파일은 #5185/#5192 이후 upstream이 `src/main.rs` 단일 파일을 `src/cli/commands/edit/`
아래로 재구조화하면서 옮긴 위치 — rhwp-code는 이번 배치까지 재구조화하지 않고
`src/main/edit.rs` 단일 파일 관례를 유지한다). `apply-char-format`/`apply-para-format`/
`apply-style`은 upstream 원본대로 `--dry-run`에서도 범위(구역/문단/오프셋/스타일
인덱스)를 미리 검사하지만, `apply-para-format-in-hf`/`apply-endnote-shape`는
upstream 원본에 사전 JSON 검사가 없어 잘못된 `--props`도 조용히 무시되고 exit 0이
난다 — 43개 명령 전체에 걸친 기존 비대칭이라 이번 배치가 만든 문제가 아니며
cli_commands.md에 기록했다. upstream 최신본에는 이번 6종 외에도
`apply-char-format-in-cell`/`apply-cell-style`/`apply-para-format-in-cell`(표
셀 전용, #5185/#5192 이후 별도 PR로 추가된 것으로 보임 — 43개 표 대상 밖)이 있어
혼동하지 않도록 제외했다.

이 배치까지 완료/제외된 것은 6(표 편집)+7(각주/미주)+9(머리말/꼬리말, 8 완료+
`toggle-hide-hf` 제외)+6(문단 기본)+6(서식/스타일) = 34개. §8 표 43개 기준 나머지
9개(책갈피·구조/기타)는 `add-bookmark`/`delete-bookmark`/`rename-bookmark`/
`delete-table`/`insert-page-break`/`insert-column-break`/`split-paragraph-in-hf`/
`merge-paragraph-in-hf`/`set-numbering-restart` — 여전히 다음 라운드 대상.

**진행(2026-09-03, 계속)**: "책갈피/구조 5종" 배치(`add-bookmark`/`delete-bookmark`/
`rename-bookmark`/`delete-table`/`insert-page-break`) 완료 —
`bookmark_query.rs`의 `add_bookmark_native`/`delete_bookmark_native`/
`rename_bookmark_native`, `table_ops.rs`의 `delete_table_control_native`,
`text_editing.rs`의 `insert_page_break_native`(전부 이미 `pub`)에 CLI 배선만
추가했다. 인자·시맨틱은 upstream `origin/devel`(`../rhwp`)의
`src/cli/commands/edit/bookmarks.rs` / `tables/structure.rs` /
`document_text.rs`를 직접 읽어 확인했다 — 5개 전부 rhwp-code 네이티브 함수와
인자 이름·순서까지 1:1로 대응해 새 core 로직 없이 순수 배선으로 끝났다(`toggle-
hide-hf` 같은 함정 없음). 책갈피 3종(`add`/`delete`/`rename`)은 네이티브 함수가
`Err`가 아니라 반환 JSON의 `ok`/`error`로 실패를 알리고(이름 비어있음·중복·컨트롤이
책갈피 아님 등은 exit 1) **네이티브 호출 자체가 `--dry-run`에서 생략되므로 구역/
문단/컨트롤 범위를 미리 검사하지 않는다** — upstream 원본도 동일. 반대로
`delete-table`은 `resolve_table_index`가 dry-run 여부와 무관하게 항상 실행되어
표 번호 범위 초과가 `--dry-run`에서도 잡히고(exit 1), `insert-page-break`는
upstream 원본대로 `--section`/`--para` 범위를 네이티브 호출과 무관하게 인자
파싱 직후 무조건 검사한다(`--dry-run`에서도 exit 2, 단 `--offset`은 검사하지
않음) — 43개 명령 전체에 걸쳐 명령마다 실제로 다른 비대칭이 이번 5개에도
그대로 있었다. 부수 발견: `ir-diff`는 책갈피 이름 변경을 diff 카테고리로 잡지
않는다(저장본을 직접 재파싱해 이름을 확인해야 함) — 기존 도구 한계이며 이번
배치가 만든 문제가 아니다. 22개 신규 계약 테스트
(`tests/edit_bookmark_structure_contract.rs`) 추가, `cargo test --lib` 전체
(3634 passed) 및 신규 계약 스위트 통과 확인. 이 배치까지 완료/제외된 것은
34+5(책갈피/구조) = 39개. 나머지 4개(`insert-column-break`/
`split-paragraph-in-hf`/`merge-paragraph-in-hf`/`set-numbering-restart`)는
다음("나머지") 배치 대상.

**진행(2026-09-03, 계속)**: "나머지" 배치 중 3건 완료, 1건 제외 —
`text_editing.rs`의 `insert_column_break_native`, `header_footer_ops.rs`의
`split_paragraph_in_header_footer_native`/`merge_paragraph_in_header_footer_native`
(전부 이미 `pub`)에 CLI 배선만 추가했다(`insert-column-break`/
`split-paragraph-in-hf`/`merge-paragraph-in-hf`). 인자·시맨틱은 upstream
`origin/devel`(`../rhwp`)의 `src/cli/commands/edit/document_text.rs` /
`header_footer_content.rs`를 직접 읽어 확인했다. `insert-column-break`는 직전
배치의 `insert-page-break`와 완전히 같은 패턴(`--section`/`--para` 범위를
`--dry-run`에서도 무조건 검사, `--offset`은 검사 안 함). `merge-paragraph-in-hf`는
upstream 원본도 `--para` 기본값을 1로 잡는다(0은 첫 문단이라 항상 거부 —
`merge-paragraph-in-footnote`와 같은 관례, 직접 대조 확인). 표본
`samples/hwpx/143E433F503322BD33.hwpx`(머리말 컨트롤 1개, 문단 1개)로 split→merge
왕복 검증.

**`set-numbering-restart`는 배선하지 않았다 — 실제 회귀 발견, `toggle-hide-hf`와
같은 성격**: 코어 `set_numbering_restart_native`(`formatting.rs`)는 정상 동작해
`Paragraph.numbering_restart` 필드를 `{"ok":true}`로 설정하지만, 이 필드는
`src/parser`/`src/serializer` 어디에도 읽거나 쓰는 코드가 없다(전체 검색으로
확인) — 저장 후 다시 열면 **HWP5·HWPX 모두** 무조건 `None`으로 돌아온다.
직접 확인 절차: `set_numbering_restart_native(0,1,2,5)` 호출 직후 in-memory
필드는 `Some(NewStart(5))`였으나, `export_hwpx_native()`/`export_hwp_with_adapter()`
로 저장 후 재파싱하면 둘 다 `None`. 유일한 소비처는 `src/renderer/layout.rs`의
`NumberingCounter::advance()` — 라이브 세션 중 화면 번호 재계산에만 쓰는 **세션
전용 필드**다. 더 심각한 점: `--verify`와 `rhwp ir-diff`가 공유하는
`serializer/hwpx/roundtrip.rs`의 `diff_documents`가 애초에 이 필드를 비교
대상에서 빠뜨려(책갈피 이름과 같은 부류의 기존 커버리지 공백) `identical:true`를
보고한다 — 즉 이 명령을 배선하면 "저장했지만 아무 효과도 남지 않는" 명령인데도
자체 검증(`--verify`)조차 그 사실을 잡아내지 못한다. 코드는 작성하지 않았다
(CLI/MCP 배선 자체를 시도하지 않음, 되돌릴 커밋 없음). 향후 이 기능이 필요하면
HWP5 ParaHeader와 HWPX `<hp:p>`가 이 정보를 표현하는 바이트/속성을 먼저 조사해
직렬화기에 새로 연결해야 한다 — CLI 배선이 아니라 별도 기능 개발.

3개 신규 계약 테스트 그룹(`tests/edit_column_break_hf_numbering_contract.rs`,
실제로는 3개 명령만 다룸 — 파일명은 착수 당시 4개 예정이었던 흔적) 추가,
`cargo test --lib` 전체(3634 passed) 및 신규 계약 스위트 통과 확인. 이 배치까지
완료/제외된 것은 39+3(나머지 중 3건) = 42개, `set-numbering-restart` 1건은
새 core 작업(직렬화기 확장)이 필요해 **여전히 미착수** — upstream #5185/#5192
43개 명령의 CLI/MCP 이식은 42/43으로 사실상 마무리, 남은 1건은 별도 이슈로
분리해 추적할 것.

**진행(2026-09-03, 완료) — `set-numbering-restart` 43/43 완료, 단 "직렬화기
확장"이 아니라 기존 인프라 재사용으로 해결됨**: 위 §진행 항목이 "HWP5 ParaHeader와
HWPX `<hp:p>`에 새 바이트/속성을 조사해 직렬화기에 새로 연결해야 한다"고 예상한
것은 **틀렸다** — 실제로는 파서/직렬화기를 전혀 건드리지 않고 해결됐다. 렌더러의
`expand_numbering_format`(`renderer/layout/utils.rs`)이 화면 번호를
`(numbering.level_start_numbers[level]-1) + counters[level]`로 계산하는 것을
확인했는데, 새 `numbering_id`가 처음 등장하면 `counters[level]`이 0→1로
초기화되어 이 값이 그대로 `level_start_numbers[level]`이 된다 — 즉 "새 번호로
시작"은 문단이 참조하는 `ParaShape`를 (다른 `level_start_numbers`를 가진) 다른
`Numbering`으로 갈아 끼우는 것만으로 완전히 표현된다. 이 `ParaShape.numbering_id`
+`Numbering` 경로는 HWP5(`doc_info.rs`/`serializer/doc_info.rs`)·HWPX
(`hwpx/header.rs`/`serializer/hwpx/header.rs`) 양쪽 다 이미 완전히 파싱·직렬화되어
정상 왕복한다(일반 번호 매기기 문단이 이미 정상 동작하는 이유와 동일) — 새로
만든 것은 `document_core` 안의 조립 로직뿐이다.

핵심 안전장치(사전 설계에는 없었고 구현 중 발견): 대상 문단 **하나만** 새
`numbering_id`로 바꾸면, 뒤따르는 같은 목록 문단이 원래 `numbering_id`를 다시
참조할 때 `NumberingState`(`renderer/layout.rs`)의 history 복원이 옛 카운터를
되살려 번호가 한 문단만 튀었다가 복귀하는 회귀가 생긴다 — 그래서
`apply_numbering_new_start`(`formatting.rs`)는 대상 문단부터 유효 `numbering_id`가
같은 동안(그리고 `head_type`이 `Outline`/`Number`인 동안 — 번호 없는 일반
문단이 `numbering_id==0`으로 우연히 매치되는 것을 막는 가드) 뒤따르는 문단까지
같은 section 안에서 전진 전파한다. 표 셀·머리말/꼬리말·구역 경계는 넘지 않는다.

변경 내역: `model/style.rs`(`Numbering`/`NumberingHead`에 `raw_data`/
`raw_para_heads` 제외 수동 `PartialEq`), `model/document.rs`
(`find_or_create_numbering`, `find_or_create_tab_def` 패턴 재사용),
`document_core/commands/formatting.rs`(`set_numbering_restart_native` 재작성 +
`apply_numbering_new_start` 신규), `main.rs`/`main/edit.rs`/`main/mcp_meta.rs`
(CLI/MCP 배선, 다른 배치들과 동일 골격). 옛 `Paragraph.numbering_restart` 필드는
세터가 없어져 죽은 코드가 됐지만 split/merge 무결성 테스트 등 9곳 이상에 걸쳐
있어 이번 배치에서는 제거하지 않았다(제거 후보로 doc-comment에 명시만 함, 별도
정리 배치 대상).

검증: 실물 한컴 문서 `rhwp-studio/public/samples/para-head-num-2.hwp`(및 HWPX
사본 `samples/hwpx/para-head-num-2.hwpx`)로 신규
`tests/edit_numbering_restart_contract.rs`(9개 테스트) 작성 — HWP5/HWPX 각각
저장 후 재파싱해 `para_shape_id`/`Numbering.level_start_numbers`를 직접 단언
(`--verify`는 개수만 비교해 이 회귀를 못 잡으므로 의존하지 않음), 전진 전파가
다른 목록 경계에서 정확히 멈추는지 확인, mode=0/1이 데이터를 안 바꾸는지 확인.
`cargo test --lib` 전체(3635 passed, 0 failed), `cargo test --test
cli_json_contract`(31 passed) 전부 통과.

**부가 발견 — mode=1("이전 번호 목록에 이어")의 진짜 의미, 다음 이슈를 위한
실측 자료**: 렌더러의 `NumberingRestart::ContinuePrevious` 분기가 완전히 빈
코드라 mode=0/1은 현재 렌더 결과에 차이가 없어 이번 배치에서는 둘 다 no-op으로
남겨뒀다. 그런데 위 실물 샘플(`para-head-num-2.hwp`)이 정확히 이 3모드를
시연하도록 만들어진 것으로 보인다 — 문단1·2("가"/"나")는 `num_id=3` 공유, 문단3
("다", "새번호 목록 시작")은 `num_id=2`로 끊고, 문단4("라", "**이전 번호 목록에
이어**")는 다시 `num_id=3`으로 **되돌아간다**(그래서 `NumberingState`의 history
복원으로 문단2 다음 번호부터 자연스럽게 이어진다). 즉 실제 한컴 구현에서
"이전 번호 목록에 이어"는 새 플래그가 아니라 **끊기기 전 원래 numbering_id로
문단의 참조를 되돌리는 것**이다. 다만 "임의의 문단에서 어느 numbering_id가
'끊기기 전 원래 것'인지"를 현재 문서 상태만으로 일반화하는 규칙(가장 가까운
선행 동일 계열 목록을 찾는 휴리스틱 등)은 이번 배치에서 설계하지 않았다 — mode=1
을 mode=0과 실제로 구분되는 동작으로 만드는 것은 별도 이슈로 분리한다.

upstream #5185/#5192 43개 명령의 CLI/MCP 이식이 **43/43 전부 완료**됐다.
