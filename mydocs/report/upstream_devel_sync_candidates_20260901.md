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
| `3431a7727` (#5251) | fix(hwpx): HWP3 원본 char_shapes 경계를 HWPX 재파싱에서 지킨다 | `src/parser/hwpx/*` | HWP3→HWPX 경로의 char_shapes 경계 보존. 우리 fork의 HWP3 격리/충실도 원칙(`mydocs/tech/parser_architecture.md`)과 정확히 같은 문제의식. |
| `d7f90eb00` (#5873) | fix(hwpx): 표 셀 안 구역 나누기를 secPr로 내보낸다 | `src/serializer/hwpx/*` | 표 셀 내부 구역 나누기 시 secPr 누락 수정 — HWPX 저장 규격 준수성. |
| `d5dada7c1` (#5861) | fix(hwpx): 사용자 정의 기호 0xA807을 평면 15 사상표에 넣는다 | `src/parser/hwpx/*` | 사용자 정의 기호 매핑 결손 수정. |
| `ce8015138` (#6380) / `1d2674857` (#5860) | fix(parser/hwp3): 사적 문자 소실 방지 / 매핑 없는 사적 문자를 한글이 내는 글자로 해석 | `src/parser/hwp3/*` | HWP3 고전 인코딩(사적 영역 문자) 손실 방지 — 우리 HWP3 파서 작업과 직접 겹침. |
| `b14557e0a` (#5141) | fix(parser/hwp3): 묶음 개체 세부 길이 8바이트 오류로 인한 자식 도형 소실 방지 | `src/parser/hwp3/*` | HWP3 도형 파싱 손실 버그. |

## 4. Tier 2 (Mid) — 가치는 있으나 이식 비용·구조 충돌 위험이 크거나 간접적

| 커밋 | 메시지 | 주요 경로 | 비고 |
|---|---|---|---|
| `193034df3` (#5511) | refactor: MCP metadata 모듈 추출 | `src/main.rs`(-4331줄) → `src/cli/metadata/mcp/{advanced,edit_content,edit_format,edit_structure,exchange,protocol,read,mod}.rs` | MCP 서버 구조 정리. rhwp-mcp-session 관련 작업 시 구조 참고 가치는 있으나, `main.rs` 분할 방향이 rhwp-code(`main/{batch,convert,edit,...}.rs`)와 달라 직접 이식은 어렵다 — 로직만 참고. |
| `9aa043c2c`~`e3e2aa21a` (#4100, Stage 1~6) | feat(ooxml_chart)/feat(serializer)/feat(cli): 차트 CSV 왕복 | `src/ooxml_chart/*`, `src/serializer/*`, `src/document_core/*`, `src/cli/commands/*` | HWPX 임베드 OOXML 차트 값을 CSV로 뽑고(`chart-to-csv`) 되넣는(`csv-to-chart`) 완전히 새로운 기능. rhwp-code에 없는 기능 격차 — 31개 파일, +6,166줄로 이식 비용은 크지만 통째로 검토할 가치가 있다. |
| `ba097d6bf` (#5185), `e0851908b` (#5192) 등 | feat(cli): 편집 명령 13건/29건 통합 | `src/cli/commands/edit/*`, `tests/cases/*_contract.rs` | `edit` 서브커맨드 42개+ 신규(`set-section-def`, `set-table-props`, `move-table`, `transpose-table`, `set-column-widths` 등). feature parity 가치는 크지만 rhwp-code CLI 구조가 이미 다르게 쪼개져 있어 명령 단위 개별 선별 이식을 권장. |
| 신규 crate | `crates/rhwp-contracts` | `crates/rhwp-contracts/*`(14파일, +12,499줄) | 계약 테스트 인프라. rhwp-code의 work-receipt/캡슐 검증 체계(`rhwp replay/audit/lineage`)와 시너지 가능성이 있어 검토 가치. |
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
