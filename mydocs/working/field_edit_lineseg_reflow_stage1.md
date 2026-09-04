# Stage 1 보고서 — 필드 편집 후 line_segs 미재계산(stale) 결함 수정

- 이슈: (미등록 — 발견 경위는 `sync/upstream-devel-9to11` 세션의 11순위 조사 중
  범위 밖에서 발견한 별개 결함. 인계 문서:
  `handover_field_edit_stale_lineseg_20260904.md`, 이번 세션이 착수 전 확인
  사항을 모두 해소하고 구현까지 완료)
- 작성일: 2026-09-04
- 브랜치: `fix/field-edit-lineseg-reflow` (base: `main`)

## 1. 작업 요약

`src/document_core/queries/field_query.rs`의 필드 값 쓰기 경로 두 곳
(`set_cell_field_text`, `set_field_text_at`)이 텍스트를 바꾼 뒤
`line_segs`(줄바꿈 경계)를 재계산하지 않던 결함을 고쳤다. `insert_text_at`/
`delete_text_at`는 기존 `line_segs`의 `text_start`만 시프트할 뿐 줄 수·폭을
다시 계산하지 않으므로, 값 길이가 바뀌어 줄바꿈 경계 자체가 달라져야 하는
편집에서도 `line_segs`가 stale한 채로 저장 파일에 그대로 직렬화됐다.

**심각도 재평가**: rhwp의 HWP5/HWPX 파서는 파일에 저장된 `line_segs`를 그대로
신뢰하고 로드한다(빈 값·zero-height 같은 퇴화 케이스만 예외적으로 재계산).
따라서 이 결함은 한컴 전용 문제가 아니라 **rhwp 자신도 "필드 편집 → 저장 →
재오픈" 후 잘못된 줄 배치를 보인다** — 최초 조사(낮음)보다 심각도가 올라간다.

## 2. 변경 파일

| 파일 | 내용 |
|------|------|
| `src/document_core/queries/field_query.rs` | `set_cell_field_text`/`set_field_text_at`을 얇은 래퍼로 바꾸고, 기존 본문은 `_raw`로 이름을 바꿔 위임. 래퍼가 성공 후 `reflow_cell_paragraph_by_path`(중첩 경로) 또는 `reflow_paragraph`(top-level)를 호출해 `line_segs`를 재계산한다. `NestedEntry → (usize,usize,usize)` 변환 helper `nested_path_to_tuple_path` 추가. 회귀 테스트 2건 추가. |

`src/document_core/commands/text_editing.rs`는 읽기만 했고 수정하지 않았다 —
기존 `reflow_cell_paragraph_by_path`/`reflow_paragraph`를 그대로 재사용했다.

## 3. 구현 세부

- **구조 확인(착수 전 필수 1번, 해소)**: `NestedEntry::TableCell{control_index,
  cell_index, para_index}` → `(control_index, cell_index, para_index)`로,
  `NestedEntry::TextBox{control_index, para_index}` → `(control_index, 0,
  para_index)`로 그대로 옮길 수 있다. `get_cell_paragraphs_mut_by_path`가
  글상자 항목의 cell_index를 항상 0으로 검증하는 것과 동일한 규약이라 별도
  변형 없이 기존 `reflow_cell_paragraph_by_path`를 그대로 재사용했다.
- **top-level 처리(착수 전 필수 2번, 해소)**: `nested_path`가 빈 경우
  (본문 최상위 ClickHere 필드)는 `reflow_cell_paragraph_by_path`로 덮이지
  않으므로, 삽입 경로(`insert_click_here_field_at`)가 이미 쓰는
  `reflow_paragraph(section_idx, para_idx)`를 그대로 재사용했다.
- **#1380 게이트와의 관계(착수 전 필수 3번, 실측 완료)**: `edit.rs`의
  `--verify`는 `diff_documents(doc.document(), reloaded.document())`로
  "편집 후 메모리 상태" vs "그 결과를 저장한 파일을 재파싱한 상태"를 비교한다.
  직렬화는 메모리의 `line_segs`를 그대로 파일에 쓰고, 파서는 그 값을 그대로
  신뢰해 재파싱하므로, 수정 전(stale-but-shifted)이든 수정 후(올바르게 재계산)
  든 두 쪽이 항상 자기 일치해 `diff_linesegs`가 잡지 못한다 — 게이트 무관 확인.
  `batch_fill_contract`/`edit_verify_contract` 전수 실행으로 실측 확인함(§4).
- **실제 체감 영향(착수 전 필수 4번, 실측 완료)**: 서브 에이전트 조사로
  HWP5(`src/parser/body_text.rs:158`)와 HWPX(`src/parser/hwpx/section/
  paragraph_parsing.rs:238,524`) 파서 모두 `line_segs`를 파일 그대로 신뢰하는
  것을 확인했다. `reflow_zero_height_paragraphs`(`src/document_core/commands/
  document.rs`)만 빈 값/zero-height 퇴화 케이스에 한해 로드 직후 재계산한다 —
  정상적으로 채워진 stale line_segs는 재파싱에서도 그대로 살아남는다. 즉 이
  결함은 **rhwp 자신에게도 재현되는 회귀**다(심각도 중간으로 재평가).

## 4. 검증

통과:

- `cargo check --lib --tests`
- `rustfmt --check --edition 2021 src/document_core/queries/field_query.rs`
  (수정한 두 블록만 재포맷 확인; 저장소 전역 CRLF 개행 경고는 기존 drift라
  무관 — `src/lib.rs` 등 미수정 파일에서도 동일하게 뜨는 것으로 확인)
- `cargo clippy --lib --tests -- -D warnings`
  — 21건 에러, 전부 `field_query.rs`/`text_editing.rs` 밖의 기존 baseline
  (`table_layout::CellUnit` 가시성, `len_zero`) — grep으로 내 변경 파일에는
  0건 확인
- `cargo test --lib document_core::queries::field_query::tests` — 16 passed
  (신규 회귀 테스트 2건 포함:
  `set_cell_field_text_reflows_line_segs_on_wrap_change`,
  `set_field_text_at_reflows_top_level_paragraph_on_wrap_change`)
- `cargo test --lib task1380` — 8 passed (#1380 게이트 전수)
- `cargo test --test batch_fill_contract --test edit_verify_contract` —
  25 + 4 = 29 passed (rhwp-form-fill/rhwp-safe-edit 핫 패스 회귀 스위트)
- `cargo test --lib`(전체) — 3637 passed, 0 failed, 13 ignored (221.82s)

## 5. 후속 — `NestedEntry::TextBox` 실제 문서 통합 테스트 (2026-09-04 추가)

Stage 1 종료 시점에 남겨둔 "글상자 내부 필드 실측 통합 테스트 없음" 격차를 채웠다.

- **샘플 확보**: `samples/*.hwp`/`*.hwpx` 78+277개를 `rhwp fields <파일> --json`으로
  전수 스캔해 글상자(`kind:"textBox"`) 안 필드가 있는 표본 6개를 찾았고, 이미
  `edit_verify_contract.rs`가 쓰는 `samples/field-01.hwp`를 골랐다(새 샘플 도입 없음).
  `목차1[0]` 필드가 section 1, paragraph 0, 글상자 컨트롤 `[3]` 안에 있고, `rhwp dump`로
  실측한 글상자 폭은 `max_width=30011 HWPUNIT`(margins 283×4)이다.
- **실측 재현(수정 전 코드로 직접 확인)**: `git checkout 9dfabe44a~1 -- \
  src/document_core/queries/field_query.rs`로 결함 커밋 직전 상태로 되돌려 같은
  시나리오(글상자 폭을 넘기는 40자 값)를 `edit fill-fields --verify`로 채운 뒤
  `rhwp dump`로 확인한 결과:
  - 수정 전: `ls_count=1`, `sw=29444`(옛 짧은 텍스트 폭 그대로) — 61자가 들어간
    문단인데 한 줄로 stale하게 남음. `verify.identical:true`는 그대로 찍힘
    (§3의 #1380 무관 결론을 실제 문서로도 재확인).
  - 수정 후: `ls_count=3`으로 올바르게 재계산됨. `verify.identical:true` 유지.
  - 수정을 원복(`git checkout HEAD -- ...`)한 뒤 정상 상태로 복귀 확인.
- **테스트 고정**: `tests/field_edit_textbox_lineseg_reflow_contract.rs` 추가.
  `edit fill-fields`로 `목차1[0]`을 40자 값으로 채우고 `--verify` 자기 일치를
  확인한 뒤, `rhwp dump`(사람이 읽는 디버그 텍스트라 스키마 계약이 아니므로
  최소 줄 단위 파싱만 함)로 컨트롤 `[3]`의 `ls_count`가 2줄 이상으로 재계산됐는지
  본다. 이 테스트는 수정 전 커밋에 대고 돌리면 **실패**함을 직접 확인했다
  (진짜 회귀 가드, 우연히 통과하는 테스트 아님).
- **검증**: `cargo test --test field_edit_textbox_lineseg_reflow_contract` — 1
  passed. `rustfmt --check`/`cargo clippy -- -D warnings` 새 파일 대상 0건.

## 6. 남은 범위 — 후속 이슈로 명시적으로 남김

- `samples/table-vpos-01.hwpx`의 `담당부서` 필드를 이용한 실측 통합 테스트
  (핸드오버가 제안한 실사례)는 다루지 않았다 — `field-01.hwp`의 `목차1[0]`이
  같은 결함 메커니즘(글상자 내부, 값 길이 변화)을 이미 실측으로 덮는다.
- `NestedEntry::TableCell`이 `TextBox` 안에 중첩된 경로(글상자 안의 표 셀
  필드)는 실측 통합 테스트가 아직 없다 — 구조적으로는 `nested_path_to_tuple_path`
  변환이 두 variant를 동일하게 다루므로 별도 분기가 필요 없다고 판단했지만,
  다중 중첩 실제 샘플로 고정하는 것은 후속으로 남긴다.
