---
kind: guide
status: active
canonical: mydocs/manual/rust_file_splitting_guide.md
last_verified: 2026-08-26
---

# Rust 소스 파일 분할 판단 가이드

이 문서는 "이 Rust 파일을 지금 여러 파일로 쪼개도 되는가"를 판단하는 절차를 다룬다. 파일이 길다는
사실 하나만으로 분할을 결정하면 두 가지 별도의 제약을 놓친다 — 하나는 이미 문서화된
[2026 리팩터링 계획](../plans/refactoring_plan_2026.md)의 Phase 1 가드레일이고, 다른 하나는
[로컬 패치 스택 upstream 재적용 절차](patch_stack_upstream_sync.md)가 전제하는 cherry-pick 기반
동기화 비용이다. 두 제약을 함께 확인하지 않으면, 로컬 작업 편의(에이전트 토큰 절감, IDE 반응성,
학습 목적의 작은 파일)를 위해 분할한 파일이 매번 upstream 동기화 때마다 수동 충돌 해결을 요구하게
되어 오히려 전체 비용이 늘어난다.

## 왜 줄 수 기준만으로 부족한가

[코드 품질 대시보드](dashboard.md)의 1,200줄 상한(`font_metrics_data.rs` 제외)은 복잡도 관리
목표일 뿐, 어떤 순서로 어떤 파일부터 쪼개야 안전한지는 말하지 않는다. 실제 판단에는 두 축이 더
필요하다.

1. **가드레일 축** — 해당 파일/구간이 [2026 리팩터링 계획](../plans/refactoring_plan_2026.md) §1
   "금지 목록"에 올라 있는가. 올라 있다면 Phase P(SourceProvenance/LayoutCompatibilityProfile)가
   끝나기 전까지 분할 대상에서 제외한다.
2. **upstream 동기화 축** — 이 저장소(`origin` = 이 fork)는 `upstream`(edwardkim/rhwp)의
   `devel`을 정기적으로 병합해 온다. [로컬 패치 스택 upstream 재적용 절차](patch_stack_upstream_sync.md)가
   기술하듯 이 저장소의 패치 반영은 `git cherry-pick`/merge로 이루어지므로, upstream이 자주 건드리는
   파일을 로컬에서 분할하면 그 파일을 건드리는 모든 미래의 upstream 패치가 매번 "코드가 새 위치로
   옮겨졌다"는 충돌을 만든다. 분할 자체가 문제가 아니라, **분할 후에도 upstream 패치가 계속
   원래 파일 경로/줄 번호를 기준으로 도착한다**는 점이 문제다.

## 판단 절차

### 1. 가드레일 확인

[2026 리팩터링 계획](../plans/refactoring_plan_2026.md) §1 "금지 B"에 이름이 올라 있는 파일/구간인지
확인한다. 이 문서 작성 시점(2026-08-26) 기준 목록:

- `src/renderer/typeset.rs` — HWP3 변형 흐름 계산 계열의 소스분기 밀집 구간
- `src/renderer/layout/paragraph_layout.rs` — "D 블록"(tac 개체 라인, `hwp3_variant` 스케일 분기)
- `src/renderer/layout.rs`, `src/renderer/height_cursor.rs` — HWP3-origin 예외 경로

이 목록에 있는 파일은 §5 "예외 심사제"로 작업지시자 승인을 받지 않는 한 분할하지 않는다. 목록은
라운드 재평가마다 바뀔 수 있으므로 실행 전 원문을 다시 확인한다.

### 2. upstream 동기화 비용 확인

가드레일에 없는 파일이라도, 실제로 upstream이 그 파일을 자주 건드리는지 확인한다.

```bash
# 전체 커밋 이력에서 이 파일이 얼마나 자주 바뀌었는지
git log --oneline -- <path> | wc -l

# upstream/devel 동기화 병합이 최근 얼마나 잦은지 (전체 이력 기준 참고용)
git log --all --grep="upstream/devel" -i --oneline | wc -l
git log --all --grep="upstream/devel" -i --format="%ad" --date=short | sort -u | tail -10
```

두 수치를 동시에 참고한다 — 커밋 이력이 길어도 대부분 이 fork 자체의 로컬 작업(fork-local 기능,
CLI, 테스트)이라면 upstream 충돌 위험은 낮다. 반대로 커밋 수가 적어도 그 파일이 upstream 핵심
렌더링/파싱 경로에 있다면 향후 병합 시 충돌 위험이 크다.

### 3. 분할 실행

1·2단계를 통과했다면 분할한다. 분할 시:

- **기존 모듈 패턴을 따른다** — `task_142`가 확립한 `foo.rs` → `foo/mod.rs` + 하위 모듈 구조를
  재사용한다 (`src/renderer/layout/`가 이미 이 구조다). 새로운 분할 관례를 만들지 않는다.
- **단일 목적·무변동 커밋으로 분리한다** — [2026 리팩터링 계획](../plans/refactoring_plan_2026.md) §1
  "금지 C"와 동일한 원칙이다: 구조 추출과 동작 변경(버그 픽스 포함)을 같은 커밋에 섞지 않는다. 이렇게
  하면 이후 upstream 충돌이 나더라도 "이 커밋은 순수 이동"이라는 사실이 diff만으로 분명해진다.
- **`git rerere`를 켜 둔다** — 분할한 파일에서 upstream 패치와 처음 충돌이 나면 그 해결을
  `git config rerere.enabled true`가 캐시해 두고, 동일한 충돌 패턴이 이후 동기화에서 다시 나타나면
  자동으로 재적용한다. 분할로 인한 반복 충돌 비용을 "이동당 1회"로 제한하는 실질적인 방법이다.

## 현재 실측 스냅샷 (2026-08-26)

아래는 위 절차를 이 문서 작성 시점에 실제로 적용한 결과다. **스냅샷일 뿐이므로 실행 전 1·2단계
명령을 다시 돌려 확인한다.** `table_layout.rs`·`main.rs`·`wasm_api/tests.rs`는 이 문서가 참고한
초기 조사 이후 별도 라운드에서 이미 분할이 끝났음을 재측정으로 확인했다 — 아래 행은 그 분할 완료
상태를 반영한 최신 수치다.

| 파일 | 줄 수 | 가드레일 | upstream 충돌 위험 | 판정 |
| --- | ---: | --- | --- | --- |
| `src/renderer/typeset.rs` | 25,253 | 금지 B 대상 | 커밋 이력 429건, 매우 활발 | 보류 |
| `src/renderer/layout.rs` | 11,617 | 금지 B 대상 | 커밋 이력 333건, 활발 | 보류 |
| `src/renderer/layout/paragraph_layout.rs` | 7,713 | 금지 B 대상(D 블록만) | 미확인 | D 블록 제외하고 판단 |
| `src/renderer/height_cursor.rs` | 2,411 | 금지 B 대상 | 미확인 | 보류 |
| `src/renderer/layout/table_layout.rs` | 3,107(루트) + 하위 9개 파일 11,386 = 14,493 | 목록에 없음 | 이미 `table_layout/{content_heights,cell_units,unit_row_cuts,cell_line_ranges,horizontal_cell,geometry,nested_split,nested_repair,row_cut_tests}.rs`로 분할 완료. hwp3 참조는 루트 1건·`content_heights.rs` 4건·`nested_split.rs` 1건(모두 유지) | 완료 — 재분할 불필요 |
| `src/main.rs` | 4,015(루트) + `src/main/{batch,convert,edit,export,governance,inspect,mcp_meta,schema}.rs` | 목록에 없음 | 이미 분할 완료. 루트에 hwp3 문자열 리터럴 7건 잔존(포맷 판정 분기 아님, `--expect-format`/매직 감지 상수) | 완료 — 재분할 불필요 |
| `src/wasm_api/tests.rs` | 해당 경로 없음 — 이미 `src/wasm_api/tests/{mod,table_tests,task_features,picture_tests,paste_clipboard_tests,fixtures,html_export_tests,issue_regressions,save_field_tests,pagination_diag}.rs` 9개 파일 총 29,407줄로 분산 | 목록에 없음 | hwp3 참조 0건(분할 전후 동일) | 완료 — 재분할 불필요 |
| `src/renderer/font_metrics_data.rs` | 46,464 | 대시보드 정책상 제외 | 자동 생성 파일 — `font-metric-gen`으로 재생성 | 수동 분할 대상 아님 |
| `src/renderer/pua_oldhangul.rs` | 5,797 | 목록에 없음 | 자동 생성 (`scripts/gen_pua_oldhangul_rs.py`) | 수동 분할 대상 아님 |
| `src/parser/hwp3/johab_map.rs` | 5,900 | 목록에 없음 | 정적 매핑 테이블(데이터) | 분할 가치 낮음 |

## 참고

- [2026 리팩터링 계획](../plans/refactoring_plan_2026.md) §1(금지 목록), §5(CC 예외 심사제)
- [로컬 패치 스택 upstream 재적용 절차](patch_stack_upstream_sync.md)
- [코드 품질 대시보드](dashboard.md)
- [포맷 파서와 공통 Document IR 경계](../tech/parser_architecture.md) — HWP3 전용 해석이
  `src/parser/hwp3/` 밖으로 새면 안 된다는 불변식. 금지 A/B가 이 불변식을 보호하는 임시 조치다.
