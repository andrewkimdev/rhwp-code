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
| `6e1a4e629` | fix(serializer/hwpx): curve를 hp:seg 체인으로 저장 — 한글 크래시 (#4676) | `src/serializer/hwpx/*` | HWPX writer의 curve 직렬화 오류(`<hc:pt>` 단순 나열)로 한글(한컴오피스)이 파일을 여는 도중 프로세스가 죽는다(COM RPC 0x800706BE). 한글 2022 오라클 1만 건 전수검사로 확정. 동기화 지점 직후 첫 upstream 커밋이자 실제 크래시를 유발하는 상호운용성 결함 — 최우선 검토 대상. |
| `718ce06d0` | fix(renderer): overlay 표 필러 흐름 복원과 상향 클램프 해제 — 6쪽 표 겹침 소멸 (#4514) | `src/renderer/*` | 글앞/글뒤 표 anchor 처리 오류로 6쪽에 걸쳐 최대 555.5px 겹침(LAYOUT_TABLE_OVERLAP 8→0건). 지금 우리가 진행 중인 table-debugging/컬럼 폭 솔버 작업(`4929e5d15`)과 같은 영역이라 로직 대조 가치가 크다. |
| `bd8919896` 외(#6303 계열) | fix(layout): 셀 자동 축소 자간을 안쪽 폭에 수렴시킨다 | `src/renderer/layout/*` | 표 셀 자동축소 자간 계산이 발산하던 문제를 안쪽 폭 기준 수렴으로 교정. 우리 table_layout 리팩터(T1~T9)와 직접 겹치는 정밀도 이슈. |
| `41475dd5e` / `94036e467` / `6f7f5c56f` (#4318) | fix(layout): 미주 다단 마지막 단 줄넘김 연작 | `src/renderer/layout/*` | 다단 미주 마지막 단에서 줄이 본문 하단을 넘는 버그를 여러 단계로 정밀 수정. 다단/미주 조판이라는 까다로운 영역의 참고 가치가 크다. |
| `3431a7727` (#5251) | fix(hwpx): HWP3 원본 char_shapes 경계를 HWPX 재파싱에서 지킨다 | `src/parser/hwpx/*` | HWP3→HWPX 경로의 char_shapes 경계 보존. 우리 fork의 HWP3 격리/충실도 원칙(`mydocs/tech/parser_architecture.md`)과 정확히 같은 문제의식. **상태(2026-09-01)**: 이식 시도 중 rhwp-code 네이티브 HWP3 파서가 char_shapes 오프셋을 upstream과 다른 단위 체계(단순 문자 인덱스 vs PARA_TEXT 확장 단위)로 계산한다는 더 깊은 전제 불일치가 드러나 이 라운드 1차 착수에서는 보류됨(`mydocs/manual/upstream_devel_intake_strategy.md` "추적" 절 참조).

**재착수·재보류(2026-09-02, 3d)**: upstream 패치(`src/parser/hwpx/section.rs`의 `Hwp3OriginSourceGuard` 스레드로컬 + FFFC 8유닛/pageNum·footer 슬롯 생략)를 그대로 이식해 통합 테스트(`issue_5251_hwp3_char_shapes_hwpx_roundtrip.rs`, upstream 시험 그대로 복사)를 돌린 결과, **char_shapes 숫자는 그대로 어긋나고(원본 네이티브 파서 `(0,1,10,15,31)` vs 패치 후 재파싱 `(0,24,33,38,54)`) — rhwp-code 네이티브 HWP3 파서가 실제로 확장 단위가 아니라 단순 문자 인덱스를 쓴다는 가설이 확정됐다.** 더 결정적으로, `render-diff samples/issue_265.hwp --via hwpx`(자기 라운드트립 시각 정합성 게이트)가 **패치 전 PASS(0px)였다가 패치 후 STRUCT_MISMATCH(347px, TextRun 16→18)로 새로 깨졌다** — upstream 패치를 그대로 적용하면 지금 잘 작동하는 rhwp-code 렌더링에 실제 회귀를 낸다. 패치는 되돌렸다(`git checkout`).

**결론**: 이 항목은 "HWPX 파서 쪽 어댑터 몇 줄"로 끝나지 않는다. 진짜 원인은 `src/parser/hwp3/*`(네이티브 HWP3 파서)의 char_shapes/char_offsets 계산이 애초에 HWP5/HWPX가 공유하는 PARA_TEXT 확장 단위 체계(개체 U+FFFC=8유닛, 탭=8유닛 등)를 안 쓰고 단순 문자 인덱스를 쓴다는 데 있다 — 이는 렌더러가 각 문서 내부적으로는 자기 char_offsets와 char_shapes를 같은 단위로 일관되게 다뤄 지금까지 개별적으로는 문제없이 렌더링해 온 것과도 부합한다(네이티브 HWP3 렌더링 자체는 정상). 진짜 이식은 네이티브 HWP3 파서의 단위 체계 자체를 확장 단위로 바꾸는 작업이 되어야 하는데, 이는 HWP3 파서 전 소비처(레이아웃·표 계산 등 char_offsets/char_shapes를 참조하는 모든 곳)에 걸친 회귀 위험이 있는 더 큰 작업이다. **다시 보류 — 다음 라운드에서 네이티브 HWP3 파서의 char_offsets 단위 체계 자체를 별도 조사·설계해야 한다.** |
| `d7f90eb00` (#5873) | fix(hwpx): 표 셀 안 구역 나누기를 secPr로 내보낸다 | `src/serializer/hwpx/*` | 표 셀 내부 구역 나누기 시 secPr 누락 수정 — HWPX 저장 규격 준수성. |
| `d5dada7c1` (#5861) | fix(hwpx): 사용자 정의 기호 0xA807을 평면 15 사상표에 넣는다 | `src/parser/hwpx/*` | 사용자 정의 기호 매핑 결손 수정. |
| `ce8015138` (#6380) / `1d2674857` (#5860) | fix(parser/hwp3): 사적 문자 소실 방지 / 매핑 없는 사적 문자를 한글이 내는 글자로 해석 | `src/parser/hwp3/*` | HWP3 고전 인코딩(사적 영역 문자) 손실 방지 — 우리 HWP3 파서 작업과 직접 겹침. |
| `b14557e0a` (#5141) | fix(parser/hwp3): 묶음 개체 세부 길이 8바이트 오류로 인한 자식 도형 소실 방지 | `src/parser/hwp3/*` | HWP3 도형 파싱 손실 버그. |

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
`merge_table_cells_native`/`split_table_cell_native`에 CLI 배선만 추가했다. 나머지
37개(각주/미주 7종부터)는 여전히 다음 라운드 대상.
