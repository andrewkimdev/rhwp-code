//! pagination_diag — tests/mod.rs 에서 무변동 이동
use super::*;

fn table_control_paras(doc: &HwpDocument) -> Vec<usize> {
    use crate::model::control::Control;
    doc.document.sections[0]
        .paragraphs
        .iter()
        .enumerate()
        .filter(|(_, p)| p.controls.iter().any(|c| matches!(c, Control::Table(_))))
        .map(|(i, _)| i)
        .collect()
}



#[test]
fn split_table_divides_rows_and_inherits_attrs() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    let (orig_width, orig_border) = {
        let t = issue_1481_table(&doc, para_idx);
        (t.common.width, t.border_fill_id)
    };

    doc.split_table_native(0, para_idx, 0, 2)
        .expect("표 나누기");

    let tables = table_control_paras(&doc);
    assert_eq!(tables.len(), 2, "나누면 표가 두 개여야 한다");

    let front = issue_1481_table(&doc, tables[0]);
    assert_eq!(front.row_count, 2, "앞 표는 커서 행 이전까지");
    assert_eq!(front.cells.len(), 6);
    assert!(front.cells.iter().all(|c| c.row < 2));

    let back = issue_1481_table(&doc, tables[1]);
    assert_eq!(back.row_count, 2, "뒤 표는 커서 행부터");
    assert_eq!(back.cells.len(), 6);
    assert!(
        back.cells.iter().all(|c| c.row < 2),
        "뒤 표 셀 row 는 0 부터 재배열되어야 한다"
    );
    assert_eq!(back.common.width, orig_width, "뒤 표는 앞 표 폭 상속");
    assert_eq!(back.border_fill_id, orig_border, "뒤 표는 앞 표 속성 상속");
}



#[test]
fn split_preserves_width_when_column_loses_its_only_unmerged_row() {
    // 회귀 재현 (5446234.hwp 실측): 열이 일부 행에서는 col_span==1 이고 다른
    // 행에서는 병합돼 사라지는 표를, 그 열의 유일한 col_span==1 대표 행이 반대
    // 반쪽으로 가도록 나누면, update_ctrl_dimensions() 로 폭을 재계산하는 옛
    // 방식은 대표 행 없는 반쪽에서 그 열을 1800 HU 기본값으로 축소해 표 폭이
    // 줄어들었다. 나누기는 폭을 그대로 보존해야 한다(함수 doc comment).
    use crate::model::control::Control;
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    let orig_width = issue_1481_table(&doc, para_idx).common.width;

    // 행 1~3의 열1·열2를 colspan=2 로 병합한다 — 열2는 행0에만 col_span==1 로
    // 남고, 나머지 행(1~3)에는 대표 셀이 없다.
    for c in &mut doc.document.sections[0].paragraphs[para_idx].controls {
        if let Control::Table(t) = c {
            let (col1_width, col2_width) = {
                let c1 = t.cells.iter().find(|c| c.row == 0 && c.col == 1).unwrap();
                let c2 = t.cells.iter().find(|c| c.row == 0 && c.col == 2).unwrap();
                (c1.width, c2.width)
            };
            for r in 1..4u16 {
                if let Some(cell) = t.cells.iter_mut().find(|c| c.row == r && c.col == 1) {
                    cell.col_span = 2;
                    cell.width = col1_width + col2_width;
                }
                t.cells.retain(|c| !(c.row == r && c.col == 2));
            }
        }
    }

    // 행 1 부터 나눠, 열2의 유일한 col_span==1 대표 행(행0)은 앞 표에만 남고
    // 뒤 표(원 행 1~3)에는 남지 않게 한다.
    doc.split_table_native(0, para_idx, 0, 1)
        .expect("표 나누기");

    let tables = table_control_paras(&doc);
    assert_eq!(tables.len(), 2, "나누면 표가 두 개여야 한다");
    let front = issue_1481_table(&doc, tables[0]);
    let back = issue_1481_table(&doc, tables[1]);
    assert_eq!(
        front.common.width, orig_width,
        "앞 표 폭은 나누기 전과 같아야 한다"
    );
    assert_eq!(
        back.common.width, orig_width,
        "뒤 표 폭은 나누기 전과 같아야 한다 (열2 대표 행이 앞 표에만 남아도)"
    );
}



#[test]
fn tagging_a_fully_merged_row_preserves_table_width() {
    // 회귀 재현 (#4478 — "Table Marker Drift" 조사): rhwp-studio의
    // setTableRoleMarker()(template.ts)가 표 마커를 달 때 실제로 호출하는 순서
    // 그대로 — insertTableRow(0, false) 로 마커 행을 삽입한 뒤, 열이 2개
    // 이상이면 mergeTableCells 로 그 행 전체를 하나의 셀로 합친다.
    //
    // 태깅 대상 표가 이미 "행 전체가 하나로 병합된 셀"만으로 이뤄져 있으면
    // (예: #REPEAT-TITLE/#REPEAT-HEADER 로 태깅하는 소제목 행), col_span==1인
    // 대표 셀이 단 하나도 없다. 옛 get_column_widths()는 이런 열을 전부 1800
    // HWPUNIT 고정값으로 채웠고, insert_row()의 update_ctrl_dimensions() 가 그
    // 잘못된 합계로 common.width를 덮어써 표가 옆으로 밀렸다(마커 삽입만으로
    // 폭이 바뀌면 안 된다).
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 1, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    let orig_width = issue_1481_table(&doc, para_idx).common.width;

    // 유일한 행을 하나의 셀로 완전 병합 — 이 시점부터 col_span==1 대표 셀이
    // 전혀 없다(소제목/구분 행의 실제 모양).
    doc.merge_table_cells_native(0, para_idx, 0, 0, 0, 0, 2)
        .expect("행 전체 병합");
    assert_eq!(
        issue_1481_table(&doc, para_idx).common.width,
        orig_width,
        "사전 조건: 병합 자체는 표 폭을 바꾸지 않는다"
    );

    // setTableRoleMarker()와 동일한 시퀀스: 마커 행 삽입 후 그 행 전체 병합.
    doc.insert_table_row_native(0, para_idx, 0, 0, false)
        .expect("마커 행 삽입");
    let dims_cols = issue_1481_table(&doc, para_idx).col_count;
    doc.merge_table_cells_native(0, para_idx, 0, 0, 0, 0, dims_cols - 1)
        .expect("마커 행 병합");

    assert_eq!(
        issue_1481_table(&doc, para_idx).common.width,
        orig_width,
        "완전 병합된 표에 마커 행을 달아도 표 폭은 그대로여야 한다"
    );
}



/// 비회귀 가드: 열만 병합해 결과가 row_span==1로 남는 경우, fix A의
/// `end_row > start_row` 조건이 새지 않아 기존 "height==0 == 자동맞춤" 보존
/// 의미론이 그대로 유지되어야 한다 (병합 자체는 원래부터 표 폭/높이를
/// 바꾸지 않는 게 맞다 — 이 테스트는 회귀 재현이 아니라 가드다).
#[test]
fn merge_cells_across_columns_only_preserves_auto_fit_zero_height() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 1, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    match &mut doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            for cell in &mut table.cells {
                cell.height = 0;
            }
        }
        _ => panic!("target table"),
    }

    // 가로(열)만 병합 — 결과는 row_span==1로 남는다.
    doc.merge_table_cells_native(0, para_idx, 0, 0, 0, 0, 2)
        .expect("가로 병합");

    match &doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            assert_eq!(table.cells.len(), 1);
            assert_eq!(
                table.cells[0].row_span, 1,
                "사전 조건: 열만 병합, row_span==1"
            );
            assert_eq!(
                table.cells[0].height, 0,
                "열만 병합한 경우 자동맞춤(height==0)이 그대로 보존되어야 한다"
            );
        }
        _ => panic!("target table"),
    }
}



/// 회귀 재현: 병합 셀이 걸친 위치에 insert_row()로 새 행을 끼워 넣으면
/// row_span은 늘지만 height는 그대로 남아, merge_cells()가 구운 총합(옛
/// row_span개 행의 합)이 이제 (row_span+1)개 행을 나타내야 하는데도
/// stale하게 남는다. 렌더러(table_layout.rs)는 이 총합을 span에 걸친 행들에
/// 나눠 맞추므로, 새로 끼어든 행이 예산을 나눠 가지면서 편집과 무관한 기존
/// 걸침 행(대개 마지막 행)이 오히려 줄어드는 왜곡이 생긴다 — delete_row 쪽
/// 하이라인 붕괴(위 테스트)의 거울상. grow_row_span_height()가 shrink_row_
/// span_height()와 대칭으로 height를 비례 확장해 이를 막는다.
#[test]
fn insert_row_into_merged_span_grows_height_proportionally() {
    let mut doc = HwpDocument::create_empty();
    // 단일 열: "다른 열이 max()로 가려주는" 탈출구를 제거해 버그를 노출시킨다.
    let created = doc.create_table_native(0, 0, 0, 4, 1).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    // HWPX에서 가져온 "자동 맞춤" 표를 재현: 모든 셀 height를 0으로 강제
    // (create_table_native는 항상 0이 아닌 height를 채우므로 직접 주입해야 한다).
    match &mut doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            for cell in &mut table.cells {
                cell.height = 0;
            }
        }
        _ => panic!("target table"),
    }

    // 자동맞춤(height==0) 3행을 세로 병합 -> merge_cells()의 0->400 fallback으로
    // height=1200(400*3)이 구워진다 (row_span=3, 4행째는 병합 밖에 남는다).
    doc.merge_table_cells_native(0, para_idx, 0, 0, 0, 2, 0)
        .expect("세로 병합");
    match &doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            assert_eq!(table.cells.len(), 2, "병합 후 셀은 2개(병합 셀 + 4행)");
            let merged = table
                .cells
                .iter()
                .find(|c| c.row_span > 1)
                .expect("병합 셀");
            assert_eq!(merged.row_span, 3, "사전 조건: row_span==3");
            assert_eq!(merged.height, 1200, "사전 조건: 0->400 fallback 합 1200");
        }
        _ => panic!("target table"),
    }

    // 병합 span(행 0~2) 내부(행 1 아래, target_row=2)에 새 행을 끼워 넣는다.
    doc.insert_table_row_native(0, para_idx, 0, 1, true)
        .expect("병합 span 내부에 행 삽입");

    match &doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            assert_eq!(table.row_count, 5, "행이 하나 늘어 5행이어야 한다");
            let merged = table
                .cells
                .iter()
                .find(|c| c.row_span > 1)
                .expect("병합 셀");
            assert_eq!(merged.row_span, 4, "row_span이 4로 늘어야 한다");
            assert_eq!(
                merged.height, 1600,
                "#(insert_row 비대칭) 회귀: height가 옛 row_span(3) 기준 1행 몫(400)만큼 \
                 비례 확장되어 1600이 되어야 하는데, {}로 stale하게 남았다 — 렌더러가 이 \
                 총합을 4행에 나눠 맞추면서 편집과 무관한 기존 걸침 행이 줄어든다.",
                merged.height
            );
        }
        _ => panic!("target table"),
    }
}



/// 비회귀 가드: row_span>1인데 height==0(자동맞춤)인 셀에 insert_row()로 새
/// 행이 끼어들어도 height==0(자동맞춤) 의미론이 그대로 보존되어야 한다 —
/// shrink_row_span_height()가 delete_row() 쪽에서 0을 방어적으로 되살리는
/// 것과 반대로, grow_row_span_height()는 0을 그대로 둔다(렌더러가 항상
/// 형제 열의 콘텐츠 기준 높이로 채우므로 고정값으로 바꿀 이유가 없다).
#[test]
fn insert_row_into_merged_span_preserves_auto_fit_zero_height() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    doc.merge_table_cells_native(0, para_idx, 0, 0, 0, 2, 0)
        .expect("세로 병합");
    // 실제 파일에서 파싱된 row_span>1·height==0(자동맞춤) 상태를 직접 주입한다
    // (merge_cells()는 항상 0->400 fallback을 굽기 때문에 이 상태를 자체적으로
    // 만들 수 없다 -- 손상/레거시 문서 방어 경로를 재현하는 것).
    match &mut doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            let merged = table
                .cells
                .iter_mut()
                .find(|c| c.row_span > 1)
                .expect("병합 셀");
            merged.height = 0;
        }
        _ => panic!("target table"),
    }

    doc.insert_table_row_native(0, para_idx, 0, 1, true)
        .expect("병합 span 내부에 행 삽입");

    match &doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            let merged = table
                .cells
                .iter()
                .find(|c| c.row_span > 1)
                .expect("병합 셀");
            assert_eq!(merged.row_span, 4, "row_span이 4로 늘어야 한다");
            assert_eq!(
                merged.height, 0,
                "자동맞춤(height==0)은 insert_row() 이후에도 보존되어야 한다"
            );
        }
        _ => panic!("target table"),
    }
}



#[test]
fn split_table_at_first_row_is_rejected() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 3, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    assert!(
        doc.split_table_native(0, para_idx, 0, 0).is_err(),
        "첫 행에서는 표 나누기가 거부되어야 한다 (한컴 동일)"
    );
}



#[test]
fn split_then_merge_round_trips_rows() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 5, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    doc.split_table_native(0, para_idx, 0, 3).expect("나누기");
    let tables = table_control_paras(&doc);
    assert_eq!(tables.len(), 2);

    doc.merge_table_with_next_native(0, tables[0], 0)
        .expect("붙이기");

    let tables = table_control_paras(&doc);
    assert_eq!(tables.len(), 1, "붙이면 표가 하나여야 한다");
    let t = issue_1481_table(&doc, tables[0]);
    assert_eq!(t.row_count, 5, "행 수 원복");
    assert_eq!(t.cells.len(), 15);
    // 이어붙인 행들의 row 재배열 검증
    for r in 0..5u16 {
        assert_eq!(
            t.cells.iter().filter(|c| c.row == r).count(),
            3,
            "행 {r} 셀 수"
        );
    }
}



#[test]
fn merge_rejects_when_paragraph_between_has_content() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, para_idx, 0, 2).expect("나누기");

    let tables = table_control_paras(&doc);
    let between = tables[0] + 1;
    assert!(between < tables[1], "나눈 사이에 문단이 있어야 한다");
    doc.document.sections[0].paragraphs[between]
        .text
        .push_str("사이 내용");

    assert!(
        doc.merge_table_with_next_native(0, tables[0], 0).is_err(),
        "표 사이에 내용이 있으면 붙이기가 거부되어야 한다 (한컴 동일)"
    );
}



#[test]
fn split_table_recomputes_corrupted_row_sizes() {
    // 파싱이 불완전한 문서는 row_sizes 가 row_count 와 어긋날 수 있다. 그 상태로
    // 나누면 산술 분할(drain/truncate)은 어긋남을 양쪽 표로 전파해 직렬화를
    // 깨뜨린다 — 나누기는 양쪽 row_sizes 를 실제 셀에서 재계산해야 한다.
    use crate::model::control::Control;
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    for c in &mut doc.document.sections[0].paragraphs[para_idx].controls {
        if let Control::Table(t) = c {
            t.row_sizes = vec![1]; // 손상 재현: 4행인데 항목 1개
        }
    }

    doc.split_table_native(0, para_idx, 0, 2)
        .expect("표 나누기");

    for &p in &table_control_paras(&doc) {
        let t = issue_1481_table(&doc, p);
        assert_eq!(
            t.row_sizes.len(),
            t.row_count as usize,
            "row_sizes 길이는 row_count 와 일치해야 한다"
        );
        for r in 0..t.row_count {
            assert_eq!(
                t.row_sizes[r as usize] as usize,
                t.cells.iter().filter(|c| c.row == r).count(),
                "행 {r} 의 row_sizes 는 실제 셀 수여야 한다"
            );
        }
    }
}



#[test]
fn merge_failure_leaves_back_table_intact() {
    // 검증이 뒤 표 제거 이후에 이뤄지면, 실패한 붙이기가 뒤 표를 문서에서
    // 지워 버린다 — 모든 검증은 문서 변형 전에 끝나야 한다.
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, para_idx, 0, 2).expect("나누기");
    assert_eq!(table_control_paras(&doc).len(), 2);

    assert!(
        doc.merge_table_with_next_native(0, para_idx, 7).is_err(),
        "잘못된 control_idx 는 거부되어야 한다"
    );
    assert_eq!(
        table_control_paras(&doc).len(),
        2,
        "실패한 붙이기가 뒤 표를 지우면 안 된다"
    );
    let back = issue_1481_table(&doc, table_control_paras(&doc)[1]);
    assert_eq!(back.row_count, 2, "뒤 표 내용 보존");
}



#[test]
fn merge_rejects_corrupted_back_row_overflow_before_mutation() {
    // 손상 문서의 뒤 표에 row=u16::MAX 셀이 있으면 `cell.row += front_rows` 가
    // debug panic / release wraparound 를 일으킨다. row_count 합 검증만으로는
    // 못 걸러내므로, 셀·zone 실측 최대 행 기준으로 변형 전에 거부해야 한다.
    use crate::model::control::Control;
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, para_idx, 0, 2).expect("나누기");
    let tables = table_control_paras(&doc);
    assert_eq!(tables.len(), 2);

    for c in &mut doc.document.sections[0].paragraphs[tables[1]].controls {
        if let Control::Table(t) = c {
            t.cells[0].row = u16::MAX; // 손상 재현
        }
    }

    assert!(
        doc.merge_table_with_next_native(0, tables[0], 0).is_err(),
        "행 오버플로를 일으킬 손상 표는 거부되어야 한다"
    );
    assert_eq!(
        table_control_paras(&doc).len(),
        2,
        "거부된 붙이기가 뒤 표를 지우면 안 된다"
    );
}



#[test]
fn split_connects_new_paragraph_vpos_to_flow() {
    // [Task #2299 계약] 새로 삽입한 문단(사이 빈 문단·뒤 표 host)의 LineSeg 가
    // placeholder vpos(0)로 남으면, 직렬화 시 가짜 단/쪽 경계로 기록되고 이후
    // 편집의 vpos 재계산이 이를 저장 경계로 오인해 고착시킨다.
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    doc.split_table_native(0, para_idx, 0, 2)
        .expect("표 나누기");

    let paras = &doc.document.sections[0].paragraphs;
    for (label, idx) in [("사이 문단", para_idx + 1), ("뒤 표 host", para_idx + 2)] {
        let vpos = paras[idx].line_segs.first().map(|l| l.vertical_pos);
        assert!(
            vpos.is_some_and(|v| v > 0),
            "{label}(문단 {idx})의 vertical_pos 는 흐름에 연결되어야 한다 (실제 {vpos:?})"
        );
    }
}



#[test]
fn split_updates_dimensions_without_raw_ctrl_data() {
    // HWPX 파서는 표의 raw_ctrl_data 를 비워 두므로, 크기 갱신이 raw 존재에
    // 의존하면 나뉜 두 표가 모두 원본 전체 크기(common.height)로 남는다.
    use crate::model::control::Control;
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    let orig_height = issue_1481_table(&doc, para_idx).common.height;
    for c in &mut doc.document.sections[0].paragraphs[para_idx].controls {
        if let Control::Table(t) = c {
            t.raw_ctrl_data = Vec::new(); // HWPX 파스 문서 재현
        }
    }

    doc.split_table_native(0, para_idx, 0, 2)
        .expect("표 나누기");

    let tables = table_control_paras(&doc);
    let front = issue_1481_table(&doc, tables[0]);
    let back = issue_1481_table(&doc, tables[1]);
    assert!(
        front.common.height < orig_height && back.common.height < orig_height,
        "raw_ctrl_data 가 없어도 나뉜 표의 common.height 는 재계산되어야 한다 \
         (원본 {orig_height}, 앞 {}, 뒤 {})",
        front.common.height,
        back.common.height
    );
}



#[test]
fn split_assigns_unique_nonzero_instance_ids() {
    // instance_id 는 저장소 계약상 고유 비-0 이어야 한다 (create_table_native 의
    // "비-0 필수", html_table_import 의 "고유한 비-0 값"). 0 으로 두면 두 번
    // 나눴을 때 0 짜리 표 두 개가 생겨 동일-ID 충돌이 재현된다.
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 6, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    doc.split_table_native(0, para_idx, 0, 4)
        .expect("1차 나누기");
    doc.split_table_native(0, para_idx, 0, 2)
        .expect("2차 나누기");

    fn stored_instance_id(t: &crate::model::table::Table) -> u32 {
        use crate::model::shape::common_obj_offsets;
        if t.raw_ctrl_data.len() >= common_obj_offsets::INSTANCE_ID.end {
            u32::from_le_bytes(
                t.raw_ctrl_data[common_obj_offsets::INSTANCE_ID]
                    .try_into()
                    .unwrap(),
            )
        } else {
            t.common.instance_id
        }
    }
    let ids: Vec<u32> = table_control_paras(&doc)
        .iter()
        .map(|&p| stored_instance_id(issue_1481_table(&doc, p)))
        .collect();
    assert_eq!(ids.len(), 3);
    for id in &ids {
        assert_ne!(*id, 0, "instance_id 는 비-0 이어야 한다: {ids:?}");
    }
    let unique: std::collections::HashSet<u32> = ids.iter().copied().collect();
    assert_eq!(unique.len(), 3, "instance_id 는 서로 달라야 한다: {ids:?}");
}



#[test]
fn split_row_index_conversion_rejects_u16_overflow() {
    // WASM 경계의 u32 → u16 캐스팅이 묵시적 절단이면 65537 이 1 로 바뀌어
    // 요청 밖 행에서 표가 나뉜다 — 명시적 오류여야 한다.
    assert!(super::row_index_from_u32(65537).is_err());
    assert!(super::row_index_from_u32(u32::MAX).is_err());
    assert_eq!(super::row_index_from_u32(3).unwrap(), 3u16);
    assert_eq!(super::row_index_from_u32(65535).unwrap(), u16::MAX);
}



#[test]
fn split_table_rejects_when_vertical_merge_crosses_cut() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    // row1~row2 를 세로 병합 (col0) — 분할선(2)을 가로지르게 만든다.
    doc.merge_table_cells_native(0, para_idx, 0, 1, 0, 2, 0)
        .expect("세로 병합");

    assert!(
        doc.split_table_native(0, para_idx, 0, 2).is_err(),
        "세로 병합 셀이 걸친 위치에서는 나누기가 거부되어야 한다 (한컴 동일)"
    );
    // 걸치지 않는 위치(1)는 허용
    doc.split_table_native(0, para_idx, 0, 3)
        .expect("병합 아래 경계에서는 나뉘어야 한다");
}



#[test]
fn split_preserves_local_resize_on_both_sides() {
    // Alt 로 조절한 행별 폭(local resize)이 나누기에서 소실되면 안 된다.
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 3).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    // 앞쪽(row1 col1, idx4)·뒤쪽(row3 col2, idx11) 셀에 localResize 등록
    let (w4, w11) = {
        let t = issue_1481_table(&doc, para_idx);
        (t.cells[4].width + 600, t.cells[11].width + 900)
    };
    doc.resize_table_cells_native(
        0,
        para_idx,
        0,
        &format!(
            r#"[{{"cellIdx":4,"widthDelta":600,"localResize":true,"renderWidth":{w4}}},{{"cellIdx":11,"widthDelta":900,"localResize":true,"renderWidth":{w11}}}]"#
        ),
    )
    .expect("local resize");

    doc.split_table_native(0, para_idx, 0, 2).expect("나누기");
    let tables = table_control_paras(&doc);

    let front = issue_1481_table(&doc, tables[0]);
    assert_eq!(
        front.local_resize_cell_widths,
        vec![(4usize, w4)],
        "앞 표는 자기 범위 항목을 그대로 보존해야 한다"
    );
    let back = issue_1481_table(&doc, tables[1]);
    assert_eq!(
        back.local_resize_cell_widths,
        vec![(11usize - 6, w11)],
        "뒤 표 항목은 앞 셀 수(6)만큼 당겨 재매핑되어야 한다"
    );

    // 붙이면 원래 배치로 돌아와야 한다
    doc.merge_table_with_next_native(0, tables[0], 0)
        .expect("붙이기");
    let tables = table_control_paras(&doc);
    let merged = issue_1481_table(&doc, tables[0]);
    let mut widths = merged.local_resize_cell_widths.clone();
    widths.sort();
    assert_eq!(widths, vec![(4usize, w4), (11usize, w11)], "붙이기 후 원복");
}



#[test]
fn split_table_survives_hwp_save_reload() {
    // 나눈 문서를 HWP 로 저장해 다시 열어도 표 두 개가 그대로여야 한다.
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 5, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, para_idx, 0, 2).expect("나누기");

    let bytes = doc.export_hwp_native().expect("HWP 저장");
    let reloaded = HwpDocument::from_bytes(&bytes).expect("재파싱");

    let tables = table_control_paras(&reloaded);
    assert_eq!(tables.len(), 2, "저장-재열기 후에도 표 두 개");
    let front = issue_1481_table(&reloaded, tables[0]);
    let back = issue_1481_table(&reloaded, tables[1]);
    assert_eq!((front.row_count, back.row_count), (2, 3));
    assert_eq!(front.cells.len(), 4);
    assert_eq!(back.cells.len(), 6);
}



#[test]
fn split_then_column_resize_still_works() {
    // 나눈 뒤에도 칸 전체 리사이즈(Ctrl 경로)가 각 표에 독립적으로 적용돼야 한다.
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, para_idx, 0, 2).expect("나누기");
    let tables = table_control_paras(&doc);

    let before = issue_1481_table(&doc, tables[1]).cells[0].width;
    // 뒤 표 col0 전체 +300
    let updates: Vec<String> = issue_1481_table(&doc, tables[1])
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.col == 0)
        .map(|(i, _)| format!(r#"{{"cellIdx":{i},"widthDelta":300}}"#))
        .collect();
    doc.resize_table_cells_native(0, tables[1], 0, &format!("[{}]", updates.join(",")))
        .expect("리사이즈");

    let back = issue_1481_table(&doc, tables[1]);
    assert_eq!(back.cells[0].width, before + 300, "뒤 표 리사이즈 반영");
    let front = issue_1481_table(&doc, tables[0]);
    assert_eq!(
        front.cells[0].width, before,
        "앞 표는 영향 없음 (따로 놀지도, 같이 끌려가지도 않게)"
    );
}

// ─── 표 나누기/붙이기 확장 케이스 매트릭스 ──────────────────────────



#[test]
fn split_at_last_row_leaves_one_row_back() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 3)
        .expect("마지막 행 나누기");
    let t = table_control_paras(&doc);
    assert_eq!(issue_1481_table(&doc, t[0]).row_count, 3);
    assert_eq!(issue_1481_table(&doc, t[1]).row_count, 1, "뒤 표 1행");
}



#[test]
fn split_two_row_table_minimal() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 2, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 1).expect("2행 표 나누기");
    let t = table_control_paras(&doc);
    assert_eq!(issue_1481_table(&doc, t[0]).row_count, 1);
    assert_eq!(issue_1481_table(&doc, t[1]).row_count, 1);
}



#[test]
fn split_single_row_table_always_rejected() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 1, 3).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    assert!(doc.split_table_native(0, p, 0, 0).is_err());
    assert!(
        doc.split_table_native(0, p, 0, 1).is_err(),
        "범위 초과도 거부"
    );
}



#[test]
fn split_preserves_cell_text_on_both_sides() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 3, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    {
        use crate::model::control::Control;
        let para = &mut doc.document.sections[0].paragraphs[p];
        if let Some(Control::Table(t)) = para.controls.get_mut(0) {
            t.cells[0].paragraphs[0].text = "앞머리".to_string();
            t.cells[5].paragraphs[0].text = "뒤꼬리".to_string();
        }
    }
    doc.split_table_native(0, p, 0, 1).expect("나누기");
    let t = table_control_paras(&doc);
    assert_eq!(
        issue_1481_table(&doc, t[0]).cells[0].paragraphs[0].text,
        "앞머리"
    );
    let back = issue_1481_table(&doc, t[1]);
    let tail = back
        .cells
        .iter()
        .find(|c| c.paragraphs[0].text == "뒤꼬리")
        .expect("뒤 표에 텍스트 보존");
    assert_eq!(tail.row, 1, "원래 row2 가 뒤 표 row1 로 재배열");
}



#[test]
fn split_allows_horizontal_merge_and_preserves_span() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 3, 3).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    // row2 에서 가로 병합 (분할과 무관한 축)
    doc.merge_table_cells_native(0, p, 0, 2, 0, 2, 1)
        .expect("가로 병합");
    doc.split_table_native(0, p, 0, 2)
        .expect("가로 병합은 나누기 허용");
    let t = table_control_paras(&doc);
    let back = issue_1481_table(&doc, t[1]);
    assert!(back.cells.iter().any(|c| c.col_span == 2), "colSpan 보존");
}



#[test]
fn split_allowed_when_vertical_merge_ends_at_cut() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.merge_table_cells_native(0, p, 0, 0, 0, 1, 0)
        .expect("row0~1 세로 병합");
    // 병합이 row1 에서 끝나므로 경계(2)는 가로지르지 않는다
    doc.split_table_native(0, p, 0, 2)
        .expect("경계 정확히 아래는 허용");
}



#[test]
fn split_twice_makes_three_tables() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 6, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 4).expect("1차");
    let t = table_control_paras(&doc);
    doc.split_table_native(0, t[0], 0, 2)
        .expect("2차 (앞 표 재분할)");
    let t = table_control_paras(&doc);
    assert_eq!(t.len(), 3);
    let rows: Vec<u16> = t
        .iter()
        .map(|p| issue_1481_table(&doc, *p).row_count)
        .collect();
    assert_eq!(rows, vec![2, 2, 2]);
}



#[test]
fn split_survives_snapshot_undo_roundtrip() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    let snap = doc.save_snapshot();
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    assert_eq!(table_control_paras(&doc).len(), 2);
    doc.restore_snapshot(snap).expect("스냅샷 복원");
    assert_eq!(
        table_control_paras(&doc).len(),
        1,
        "undo(스냅샷 복원) 후 원복"
    );
    assert_eq!(issue_1481_table(&doc, p).row_count, 4);
}



#[test]
fn split_inherits_repeat_header_and_keeps_caption_front_only() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 3, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    {
        use crate::model::control::Control;
        use crate::model::shape::Caption;
        let para = &mut doc.document.sections[0].paragraphs[p];
        if let Some(Control::Table(t)) = para.controls.get_mut(0) {
            t.repeat_header = true;
            t.caption = Some(Caption::default());
        }
    }
    doc.split_table_native(0, p, 0, 1).expect("나누기");
    let t = table_control_paras(&doc);
    let front = issue_1481_table(&doc, t[0]);
    let back = issue_1481_table(&doc, t[1]);
    assert!(
        front.repeat_header && back.repeat_header,
        "제목행 반복 속성 상속"
    );
    assert!(front.caption.is_some(), "캡션은 앞 표 유지");
    assert!(back.caption.is_none(), "뒤 표는 캡션 없음");
}



#[test]
fn split_row_sizes_partitioned() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 5, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    let orig = issue_1481_table(&doc, p).row_sizes.clone();
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    let t = table_control_paras(&doc);
    let front = issue_1481_table(&doc, t[0]);
    let back = issue_1481_table(&doc, t[1]);
    if !orig.is_empty() {
        assert_eq!(front.row_sizes.len(), 2);
        assert_eq!(back.row_sizes.len(), 3);
    }
}



#[test]
fn split_invalid_targets_rejected() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 3, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    assert!(
        doc.split_table_native(0, p + 99, 0, 1).is_err(),
        "없는 문단"
    );
    assert!(doc.split_table_native(0, p, 7, 1).is_err(), "없는 컨트롤");
    assert!(doc.split_table_native(3, p, 0, 1).is_err(), "없는 구역");
}



#[test]
fn split_survives_hwpx_save_reload() {
    // create_empty 문서는 HWPX ID 맵 미등록으로 원래 export 불가(기존 한계) —
    // 실물 빈 문서를 기반으로 검증한다.
    let root = env!("CARGO_MANIFEST_DIR");
    let bytes =
        std::fs::read(format!("{root}/samples/253E164F57A1BC6934-empty.hwp")).expect("샘플");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("파싱");
    let baseline = table_control_paras(&doc).len(); // 샘플 자체 표 수
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표 생성");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    let out = doc.export_hwpx_native().expect("HWPX 저장");
    let reloaded = HwpDocument::from_bytes(&out).expect("재파싱");
    let tables = table_control_paras(&reloaded);
    let shapes: Vec<(u16, u16, usize)> = tables
        .iter()
        .map(|p| {
            let t = issue_1481_table(&reloaded, *p);
            (t.row_count, t.col_count, t.cells.len())
        })
        .collect();
    assert_eq!(
        tables.len(),
        baseline + 2,
        "HWPX 왕복: 샘플 원본 표 + 나눈 두 표 전부 생존 — 실제: {shapes:?}"
    );
    assert_eq!(
        shapes.iter().filter(|s| **s == (2, 2, 4)).count(),
        2,
        "나눈 2×2 두 표가 보존되어야 한다: {shapes:?}"
    );
}



#[test]
fn merge_different_col_counts_keeps_rows_intact() {
    // 한컴 명세: 칸 수 달라도 붙는다. 각 행은 자기 칸 배치를 유지한다.
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 2, 2).expect("2열 표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    // 2열 표를 나눈 뒤, 뒤 표에 열 추가해 3열로 만들고 다시 붙인다
    doc.split_table_native(0, p, 0, 1).expect("나누기");
    let t = table_control_paras(&doc);
    doc.insert_table_column_native(0, t[1], 0, 1, true)
        .expect("열 추가");
    assert_eq!(issue_1481_table(&doc, t[1]).col_count, 3);
    doc.merge_table_with_next_native(0, t[0], 0)
        .expect("칸 수 다른 붙이기");
    let t = table_control_paras(&doc);
    let m = issue_1481_table(&doc, t[0]);
    assert_eq!(m.col_count, 3, "col_count 는 큰 쪽");
    assert_eq!(
        m.cells.iter().filter(|c| c.row == 0).count(),
        2,
        "앞 행은 2칸 유지"
    );
    assert_eq!(
        m.cells.iter().filter(|c| c.row == 1).count(),
        3,
        "뒤 행은 3칸 유지"
    );
}



#[test]
fn merge_three_tables_sequentially() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 6, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 4).expect("1차 나누기");
    let t = table_control_paras(&doc);
    doc.split_table_native(0, t[0], 0, 2).expect("2차 나누기");
    // 3개 → 앞에서 두 번 붙여 1개
    let t = table_control_paras(&doc);
    doc.merge_table_with_next_native(0, t[0], 0)
        .expect("1차 붙이기");
    let t = table_control_paras(&doc);
    doc.merge_table_with_next_native(0, t[0], 0)
        .expect("2차 붙이기");
    let t = table_control_paras(&doc);
    assert_eq!(t.len(), 1);
    assert_eq!(issue_1481_table(&doc, t[0]).row_count, 6, "6행 원복");
}



#[test]
fn merge_rejected_at_document_end_without_next_table() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 2, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    assert!(
        doc.merge_table_with_next_native(0, p, 0).is_err(),
        "다음 표 없음 → 거부"
    );
}



#[test]
fn merge_allows_whitespace_only_paragraph_between() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    let t = table_control_paras(&doc);
    // 사이 문단에 표(컨트롤)를 넣으면... 그 자체가 다음 표가 되므로,
    // 대신 텍스트 아닌 컨트롤 존재 케이스: 사이 문단에 빈 문자열 + 컨트롤 흉내로
    // 텍스트를 넣는 기존 케이스와 구분해 공백만 있는 문단은 허용되는지 확인.
    let between = t[0] + 1;
    doc.document.sections[0].paragraphs[between].text = "   ".to_string();
    doc.merge_table_with_next_native(0, t[0], 0)
        .expect("공백뿐인 문단은 빈 문단으로 취급 (한컴: 빈칸 허용)");
}



#[test]
fn merge_keeps_front_caption() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    {
        use crate::model::control::Control;
        use crate::model::shape::Caption;
        let para = &mut doc.document.sections[0].paragraphs[p];
        if let Some(Control::Table(t)) = para.controls.get_mut(0) {
            t.caption = Some(Caption::default());
        }
    }
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    let t = table_control_paras(&doc);
    doc.merge_table_with_next_native(0, t[0], 0)
        .expect("붙이기");
    let t = table_control_paras(&doc);
    assert!(
        issue_1481_table(&doc, t[0]).caption.is_some(),
        "앞 캡션 유지"
    );
}



#[test]
fn merge_then_split_again_roundtrip() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 5, 3).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    let t = table_control_paras(&doc);
    doc.merge_table_with_next_native(0, t[0], 0)
        .expect("붙이기");
    let t = table_control_paras(&doc);
    doc.split_table_native(0, t[0], 0, 2).expect("재나누기");
    let t = table_control_paras(&doc);
    assert_eq!(t.len(), 2);
    assert_eq!(issue_1481_table(&doc, t[1]).row_count, 3);
}



#[test]
fn merge_survives_hwp_save_reload() {
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    let t = table_control_paras(&doc);
    doc.merge_table_with_next_native(0, t[0], 0)
        .expect("붙이기");
    let bytes = doc.export_hwp_native().expect("저장");
    let reloaded = HwpDocument::from_bytes(&bytes).expect("재파싱");
    let t = table_control_paras(&reloaded);
    assert_eq!(t.len(), 1);
    assert_eq!(issue_1481_table(&reloaded, t[0]).row_count, 4);
}



#[test]
fn merge_rejects_when_nontable_control_between() {
    use crate::model::control::{Control, Hyperlink};
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    let p = issue_1481_json_usize(&created, "paraIdx");
    doc.split_table_native(0, p, 0, 2).expect("나누기");
    let t = table_control_paras(&doc);
    let between = t[0] + 1;
    doc.document.sections[0].paragraphs[between]
        .controls
        .push(Control::Hyperlink(Hyperlink {
            url: "https://example.com".into(),
            text: "링크".into(),
        }));
    assert!(
        doc.merge_table_with_next_native(0, t[0], 0).is_err(),
        "사이 문단에 컨트롤이 있으면 거부"
    );
}



#[test]
fn create_empty_table_hwpx_export_known_limitation() {
    // 기존 한계 고정: create_empty 문서는 HWPX ID 맵(charPr/paraPr/style 0)
    // 미등록으로 표 유무와 무관하게 export 가 거부된다. split 탓이 아님을
    // 대조 실험으로 봉인해 둔다 (해소되면 이 테스트를 뒤집을 것).
    let mut doc = HwpDocument::create_empty();
    doc.create_table_native(0, 0, 0, 4, 2).expect("표");
    assert!(doc.export_hwpx_native().is_err());
}



/// 코퍼스 전수 스윕: samples 의 모든 HWP 문서 × 모든 최상위 표 × 분할점
/// {1, 중간, 마지막} 에서 나누기→불변식 검증→붙이기→원본 대조.
///
/// 무겁기 때문에 기본 제외 — 실행: `cargo test ... corpus_split_join_sweep -- --ignored`
#[test]
#[ignore]
fn corpus_split_join_sweep_all_samples() {
    use crate::model::control::Control;

    let root = env!("CARGO_MANIFEST_DIR");
    let mut files: Vec<_> = std::fs::read_dir(format!("{root}/samples"))
        .expect("samples 디렉터리")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("hwp"))
        .collect();
    files.sort();

    #[derive(Clone, PartialEq, Debug)]
    struct TableSig {
        rows: u16,
        cols: u16,
        cells: Vec<(u16, u16, u16, u16, String)>, // (row, col, rspan, cspan, 첫문단 텍스트)
    }
    fn sig(t: &crate::model::table::Table) -> TableSig {
        TableSig {
            rows: t.row_count,
            cols: t.col_count,
            cells: t
                .cells
                .iter()
                .map(|c| {
                    (
                        c.row,
                        c.col,
                        c.row_span,
                        c.col_span,
                        c.paragraphs
                            .first()
                            .map(|p| p.text.clone())
                            .unwrap_or_default(),
                    )
                })
                .collect(),
        }
    }

    let (mut docs, mut tables, mut splits_ok, mut rejected, mut merged_ok) =
        (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut failures: Vec<String> = Vec::new();

    for path in files {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // 암호 문서 등 파싱 불가는 건너뜀
        let Ok(probe) = HwpDocument::from_bytes(&bytes) else {
            continue;
        };
        docs += 1;

        // 최상위 표 위치 수집 (섹션 0 한정 — 파일당 비용 통제)
        let table_locs: Vec<(usize, usize, u16)> = probe.document.sections[0]
            .paragraphs
            .iter()
            .enumerate()
            .flat_map(|(pi, para)| {
                para.controls.iter().enumerate().filter_map(move |(ci, c)| {
                    if let Control::Table(t) = c {
                        Some((pi, ci, t.row_count))
                    } else {
                        None
                    }
                })
            })
            .collect();

        for (pi, ci, rows) in table_locs {
            if rows < 2 {
                continue;
            }
            tables += 1;
            let cuts: Vec<u16> = {
                let mut c = vec![1, rows / 2, rows - 1];
                c.dedup();
                c.retain(|x| *x >= 1 && *x < rows);
                c
            };
            for cut in cuts {
                let mut doc = HwpDocument::from_bytes(&bytes).expect("재파싱");
                let orig = sig(
                    match doc.document.sections[0].paragraphs[pi].controls.get(ci) {
                        Some(Control::Table(t)) => t,
                        _ => continue,
                    },
                );
                match doc.split_table_native(0, pi, ci, cut) {
                    Err(e) => {
                        // 허용된 거부 사유만 인정
                        let msg = format!("{e:?}");
                        if msg.contains("세로로 합쳐진") || msg.contains("첫 번째 줄") {
                            rejected += 1;
                        } else {
                            failures.push(format!("{name} 표(p{pi}) cut{cut}: 예상 밖 거부 {msg}"));
                        }
                        continue;
                    }
                    Ok(_) => splits_ok += 1,
                }
                // 불변식: 나눈 두 표의 시그니처 합 = 원본
                let front = sig(
                    match doc.document.sections[0].paragraphs[pi].controls.get(ci) {
                        Some(Control::Table(t)) => t,
                        _ => {
                            failures.push(format!("{name} p{pi} cut{cut}: 앞 표 소실"));
                            continue;
                        }
                    },
                );
                if front.rows != cut {
                    failures.push(format!(
                        "{name} p{pi} cut{cut}: 앞 행수 {}≠{cut}",
                        front.rows
                    ));
                }
                // 붙여서 원본과 대조
                if let Err(e) = doc.merge_table_with_next_native(0, pi, ci) {
                    failures.push(format!("{name} p{pi} cut{cut}: 붙이기 실패 {e:?}"));
                    continue;
                }
                let merged = sig(
                    match doc.document.sections[0].paragraphs[pi].controls.get(ci) {
                        Some(Control::Table(t)) => t,
                        _ => {
                            failures.push(format!("{name} p{pi} cut{cut}: 병합 표 소실"));
                            continue;
                        }
                    },
                );
                if merged != orig {
                    failures.push(format!(
                        "{name} p{pi} cut{cut}: 라운드트립 불일치 (rows {}→{}, cells {}→{})",
                        orig.rows,
                        merged.rows,
                        orig.cells.len(),
                        merged.cells.len()
                    ));
                } else {
                    merged_ok += 1;
                }
            }
        }
    }

    eprintln!(
        "[corpus sweep] 문서 {docs} · 표 {tables} · 나누기 성공 {splits_ok} · 정당 거부 {rejected} · 라운드트립 일치 {merged_ok} · 실패 {}",
        failures.len()
    );
    for f in failures.iter().take(20) {
        eprintln!("  FAIL {f}");
    }
    assert!(failures.is_empty(), "{}건 실패 (위 로그)", failures.len());
}



#[test]
fn split_after_local_resize_keeps_render_grid() {
    // Alt(행별 폭, local resize)로 조절한 표를 나누면, base grid 재계산이
    // override 행을 제외하지 않을 경우 뒤 표의 common.width 가 override 폭만큼
    // 부풀어 전 행이 넓게 렌더된다 (override 행은 residual 몰아주기로 두 배).
    // 한컴 의미론: Alt 는 표 폭 유지, 나누기도 폭 불변 — 렌더 폭이 나누기
    // 전후로 같아야 한다.
    use crate::model::control::Control;
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/samples/21868765_별표2_보건소_분장사무.hwp"
    ))
    .expect("샘플");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("파싱");
    doc.paginate();
    let para_idx = doc.document.sections[0]
        .paragraphs
        .iter()
        .enumerate()
        .find(|(_, p)| p.controls.iter().any(|c| matches!(c, Control::Table(_))))
        .map(|(i, _)| i)
        .expect("표 문단");

    let bb = |d: &HwpDocument, p: usize| -> Vec<(u16, u16, f64)> {
        let json = d
            .get_table_cell_bboxes_by_path_native(
                0,
                p,
                r#"[{"controlIndex":0,"cellIndex":0,"cellParaIndex":0}]"#,
            )
            .expect("bbox");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["row"].as_u64().unwrap() as u16,
                    c["col"].as_u64().unwrap() as u16,
                    c["w"].as_f64().unwrap(),
                )
            })
            .collect()
    };
    let at = |v: &Vec<(u16, u16, f64)>, r: u16, c: u16| {
        v.iter()
            .find(|x| x.0 == r && x.1 == c)
            .map(|x| x.2)
            .unwrap()
    };

    // Alt+→ ×3 상당: (12,2) +900, 같은 줄 (12,0)/(12,1) 이 -450 씩 흡수
    let (i0, w0, i1, w1, i2, w2) = {
        let t = issue_1481_table(&doc, para_idx);
        let f = |r: u16, c: u16| {
            t.cells
                .iter()
                .position(|x| x.row == r && x.col == c && x.row_span == 1)
                .expect("셀")
        };
        let (a, b, c) = (f(12, 0), f(12, 1), f(12, 2));
        (
            a,
            t.cells[a].width,
            b,
            t.cells[b].width,
            c,
            t.cells[c].width,
        )
    };
    let payload = format!(
        r#"[{{"cellIdx":{i2},"widthDelta":900,"localResize":true,"renderWidth":{}}},{{"cellIdx":{i0},"widthDelta":-450,"localResize":true,"renderWidth":{}}},{{"cellIdx":{i1},"widthDelta":-450,"localResize":true,"renderWidth":{}}}]"#,
        w2 + 900,
        w0 - 450,
        w1 - 450,
    );
    doc.resize_table_cells_native(0, para_idx, 0, &payload)
        .expect("alt resize");

    let pre = bb(&doc, para_idx);
    let (pre_plain_c2, pre_override_c2) = (at(&pre, 11, 2), at(&pre, 12, 2));

    // HWP에는 Studio의 local-resize 런타임 힌트가 저장되지 않는다. 저장 후
    // 다시 연 상태와 같이 힌트를 비워도, 셀 폭에서 추론한 outlier 행은 base
    // grid와 표 전체 폭 재계산에서 똑같이 제외해야 한다.
    for control in &mut doc.document.sections[0].paragraphs[para_idx].controls {
        if let Control::Table(table) = control {
            table.local_resize_rows.clear();
            table.local_resize_cols.clear();
            table.local_resize_cell_widths.clear();
            table.local_resize_cell_heights.clear();
        }
    }

    doc.split_table_native(0, para_idx, 0, 10).expect("나누기");

    let tables: Vec<usize> = doc.document.sections[0]
        .paragraphs
        .iter()
        .enumerate()
        .filter(|(_, p)| p.controls.iter().any(|c| matches!(c, Control::Table(_))))
        .map(|(i, _)| i)
        .collect();
    let back = bb(&doc, tables[1]);

    // 원래 row 11/12/13 → 뒤 표 row 1/2/3
    assert!(
        (at(&back, 1, 2) - pre_plain_c2).abs() < 0.5,
        "나누기 후 일반 행 폭이 변하면 안 된다: {} → {}",
        pre_plain_c2,
        at(&back, 1, 2)
    );
    assert!(
        (at(&back, 3, 2) - pre_plain_c2).abs() < 0.5,
        "나누기 후 일반 행 폭이 변하면 안 된다: {} → {}",
        pre_plain_c2,
        at(&back, 3, 2)
    );
    assert!(
        (at(&back, 2, 2) - pre_override_c2).abs() < 0.5,
        "나누기 후 Alt 행 폭이 변하면 안 된다: {} → {}",
        pre_override_c2,
        at(&back, 2, 2)
    );
}



#[test]
fn split_rejects_corrupted_cell_row_span_overflow_before_mutation() {
    // `row + row_span`을 u16으로 더하면 debug에서는 panic, release에서는
    // wrap한다. 손상 입력은 나누기 전 명시적 오류로 막고 원본 표를 남겨야 한다.
    use crate::model::control::Control;
    let mut doc = HwpDocument::create_empty();
    let created = doc.create_table_native(0, 0, 0, 2, 2).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    for control in &mut doc.document.sections[0].paragraphs[para_idx].controls {
        if let Control::Table(table) = control {
            table.row_count = u16::MAX;
            let cell = table.cells.first_mut().expect("셀");
            cell.row = u16::MAX - 2;
            cell.row_span = 3;
        }
    }

    assert!(
        doc.split_table_native(0, para_idx, 0, u16::MAX - 1)
            .is_err(),
        "손상된 row/span은 panic 대신 오류여야 한다"
    );
    assert_eq!(
        table_control_paras(&doc),
        vec![para_idx],
        "실패는 표를 추가하지 않는다"
    );
    let table = issue_1481_table(&doc, para_idx);
    assert_eq!(table.row_count, u16::MAX, "실패는 기존 표를 바꾸지 않는다");
    assert_eq!(table.cells[0].row, u16::MAX - 2);
    assert_eq!(table.cells[0].row_span, 3);
}



/// [#4149 조사] Enter→Delete 왕복(내용 무변경 복원)이 셀 문단의 저장 line_segs 를
/// 보존하는지 계측한다. 두 원천을 분리한다:
///
///   A) 무편집 divergence — 로드 직후 저장 segs vs `reflow_cell_paragraph` 재계산.
///      다르면 그 문단은 "한 번이라도 건드리는 순간" 한컴 원본 줄바꿈 증거를 잃는
///      잠재 모집단이다.
///   B) 왕복 실측 — split→merge(내용 복원) 전후 저장 segs 대조. 불변식
///      "무변경 편집 왕복은 레이아웃 보존" 위반의 직접 증거.
///
/// 구조상 왕복 결과는 reflow 결과와 같아야 하므로 B의 변경 집합은 A의 divergence
/// 집합과 일치할 것으로 기대한다 — 일치하면 원인은 왕복 경로가 아니라 "reflow 가
/// 원본 줄바꿈과 다르다"로 좁혀진다. 요약은 --nocapture 로 출력.
#[test]
fn issue4149_cell_lineseg_roundtrip_survey() {
    let path = "samples/issue1949_giant_cell_nested_tables_perf.hwp";
    let bytes = std::fs::read(path).expect("issue1949 샘플 읽기");

    // (ppi, ci, cell, para) — 최외곽 표, 다줄(>=2 segs), UTF-16 4자 이상
    fn survey(doc: &HwpDocument) -> Vec<(usize, usize, usize, usize)> {
        let mut out = Vec::new();
        for (ppi, para) in doc.document.sections[0].paragraphs.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                if let Control::Table(t) = ctrl {
                    for (cell_idx, cell) in t.cells.iter().enumerate() {
                        for (cp, p) in cell.paragraphs.iter().enumerate() {
                            if p.line_segs.len() >= 2 && p.text.encode_utf16().count() >= 4 {
                                out.push((ppi, ci, cell_idx, cp));
                            }
                        }
                    }
                }
            }
        }
        out
    }
    fn cell_para<'a>(doc: &'a HwpDocument, t: (usize, usize, usize, usize)) -> &'a Paragraph {
        match &doc.document.sections[0].paragraphs[t.0].controls[t.1] {
            Control::Table(tbl) => &tbl.cells[t.2].paragraphs[t.3],
            _ => panic!("표가 아님"),
        }
    }
    // 줄바꿈 증거(text_start 수열)와 기하(vpos/width)를 분리해 본다
    fn breaks(p: &Paragraph) -> Vec<u32> {
        p.line_segs.iter().map(|s| s.text_start).collect()
    }
    fn geom(p: &Paragraph) -> Vec<(i32, i32, i32)> {
        p.line_segs
            .iter()
            .map(|s| (s.vertical_pos, s.segment_width, s.line_height))
            .collect()
    }

    // ── A) 무편집: 저장 segs vs reflow 재계산 ─────────────────────────────
    let mut doc_a = HwpDocument::from_bytes(&bytes).expect("파싱");
    let targets = survey(&doc_a);
    assert!(!targets.is_empty(), "다줄 셀 문단이 없으면 계측 무의미");
    let (mut a_same, mut a_breaks, mut a_geom_only) = (0usize, 0usize, 0usize);
    let mut a_break_examples = Vec::new();
    let mut a_divergent = std::collections::HashSet::new();
    for &t in &targets {
        let before = (breaks(cell_para(&doc_a, t)), geom(cell_para(&doc_a, t)));
        doc_a.reflow_cell_paragraph(0, t.0, t.1, t.2, t.3);
        let after = (breaks(cell_para(&doc_a, t)), geom(cell_para(&doc_a, t)));
        if before.0 != after.0 {
            a_breaks += 1;
            a_divergent.insert(t);
            if a_break_examples.len() < 3 {
                a_break_examples.push((t, before.0.clone(), after.0.clone()));
            }
        } else if before.1 != after.1 {
            a_geom_only += 1;
            a_divergent.insert(t);
        } else {
            a_same += 1;
        }
    }
    println!(
        "[#4149-A] 무편집 다줄 셀 문단 {}개: 보존 {} / 줄바꿈 divergence {} / 기하만 {}",
        targets.len(),
        a_same,
        a_breaks,
        a_geom_only
    );
    for (t, b, a) in &a_break_examples {
        println!(
            "[#4149-A] 예시 ppi{} ci{} cell{} para{}: 저장 {:?} → 재계산 {:?}",
            t.0, t.1, t.2, t.3, b, a
        );
    }

    // ── B) 왕복: split(중간)→merge 전후 저장 segs ────────────────────────
    let mut doc_b = HwpDocument::from_bytes(&bytes).expect("파싱");
    let roundtrip_targets: Vec<_> = targets.iter().copied().take(12).collect();
    let (mut b_same, mut b_breaks, mut b_geom_only) = (0usize, 0usize, 0usize);
    let mut b_matches_a = true;
    for &t in &roundtrip_targets {
        let p = cell_para(&doc_b, t);
        let text_before = p.text.clone();
        let (bk_before, gm_before) = (breaks(p), geom(p));
        let mid = (text_before.encode_utf16().count() / 2).max(1);
        doc_b
            .split_paragraph_in_cell_native(0, t.0, t.1, t.2, t.3, mid, None)
            .expect("split");
        doc_b
            .merge_paragraph_in_cell_native(0, t.0, t.1, t.2, t.3 + 1)
            .expect("merge");
        let p = cell_para(&doc_b, t);
        assert_eq!(
            p.text, text_before,
            "왕복 후 텍스트 복원은 전제 (ppi{} cell{})",
            t.0, t.2
        );
        let changed = if bk_before != breaks(p) {
            b_breaks += 1;
            true
        } else if gm_before != geom(p) {
            b_geom_only += 1;
            true
        } else {
            b_same += 1;
            false
        };
        if changed != a_divergent.contains(&t) {
            b_matches_a = false;
            println!(
                "[#4149-B] 예외: ppi{} ci{} cell{} para{} — 왕복 변경={} vs A divergence={}",
                t.0,
                t.1,
                t.2,
                t.3,
                changed,
                a_divergent.contains(&t)
            );
        }
    }
    println!(
        "[#4149-B] Enter→Delete 왕복 {}개: 보존 {} / 줄바꿈 변경 {} / 기하만 변경 {}",
        roundtrip_targets.len(),
        b_same,
        b_breaks,
        b_geom_only
    );
    println!(
        "[#4149-B] 왕복 변경 집합 == A divergence 집합: {} — true 면 왕복 자체는 reflow 를 \
         충실히 따르고, 원인은 reflow vs 한컴 원본 줄바꿈 차이로 좁혀진다",
        b_matches_a
    );
}



/// [조사] 거대 셀 문서 타이핑 지연의 진짜 병목 분해 — getCursorRectInCell 60ms 의
/// 내부 귀속. 브라우저 트레이스(조합 업데이트마다 getCursorRectInCell 56~60ms)의
/// wasm 안쪽을 페이즈별로 실측한다:
///   1) find_pages_for_cell_position (페이지 좁히기)
///   2) build_page_tree (uncached — LayoutEngine::build_render_tree 포함)
///   3) get_cursor_rect_in_cell_native 전체
///   4) 거대 표가 없는 쪽의 build_page_tree (차등 — 표 레이아웃 귀속 증거)
#[test]
fn issue4149_adjacent_giant_cell_cursor_rect_latency_decomposition() {
    use std::time::Instant;
    let bytes =
        std::fs::read("samples/issue1949_giant_cell_nested_tables_perf.hwp").expect("샘플 읽기");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("파싱");
    let pages = doc.page_count();
    println!("[cursor-rect] 총 {pages}쪽");

    // 캐럿 좌표: 브라우저 실측과 동일 (ppi0 ci2 cell2 para6 off5)
    let t = Instant::now();
    let cand = doc
        .find_pages_for_cell_position(0, 0, 2, 2, Some((6, 5)))
        .expect("페이지 좁히기");
    println!(
        "[cursor-rect] 1) find_pages_for_cell_position={:?} → {:?}",
        t.elapsed(),
        cand
    );

    let target_page = cand[0];
    for i in 0..3 {
        let t = Instant::now();
        let _ = doc.build_page_tree(target_page).expect("페이지 트리");
        println!(
            "[cursor-rect] 2) build_page_tree(p{target_page}) #{i}={:?}",
            t.elapsed()
        );
    }
    // find_page(구성 조회)만 분리 — 나머지가 LayoutEngine::build_render_tree 귀속
    for i in 0..2 {
        let t = Instant::now();
        let _ = doc.find_page(target_page).expect("find_page");
        println!(
            "[cursor-rect] 2b) find_page(p{target_page}) #{i}={:?}",
            t.elapsed()
        );
    }
    // 차등: 마지막 쪽 (다른 내용 구성)
    let last = pages - 1;
    let t = Instant::now();
    let _ = doc.build_page_tree(last).expect("페이지 트리");
    println!(
        "[cursor-rect] 4) build_page_tree(p{last})={:?}",
        t.elapsed()
    );

    for i in 0..3 {
        let t = Instant::now();
        let _ = doc
            .get_cursor_rect_in_cell_native(0, 0, 2, 2, 6, 5)
            .expect("커서 rect");
        println!(
            "[cursor-rect] 3) get_cursor_rect_in_cell_native #{i}={:?}",
            t.elapsed()
        );
    }
}



/// [조사] 프로파일링용 핫루프 — macOS `sample` 로 build_render_tree 내부 귀속.
/// `--ignored` 로만 실행.
#[test]
#[ignore]
fn issue4149_adjacent_cursor_rect_profile_loop() {
    let bytes =
        std::fs::read("samples/issue1949_giant_cell_nested_tables_perf.hwp").expect("샘플 읽기");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("파싱");
    let _ = doc.page_count();
    let _ = doc.find_pages_for_cell_position(0, 0, 2, 2, Some((6, 5)));
    eprintln!("[profile-loop] 시작 pid={}", std::process::id());
    for _ in 0..600 {
        let _ = doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 6, 5);
    }
    eprintln!("[profile-loop] 끝");
}



/// [조사] deferred 편집 직후 첫 캐럿 질의 ~20ms 의 귀속 — find_pages vs 나머지.
#[test]
fn issue4149_deferred_edit_first_cursor_query_decomposition() {
    use std::time::Instant;
    let bytes =
        std::fs::read("samples/issue1949_giant_cell_nested_tables_perf.hwp").expect("샘플 읽기");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("파싱");
    let _ = doc.page_count();
    // 웜업
    let _ = doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 6, 5);
    let t = Instant::now();
    let _ = doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 6, 5);
    println!("[deferred-q] 웜 질의={:?}", t.elapsed());

    // cp29: 다줄 텍스트 문단 (조사 실측 [0,46,84,...]) — 실타이핑 대표 사례.
    // cp6(공백 스페이서)은 삽입이 spacer 클래스를 바꿔 정당 evict 되는 반례다.
    doc.insert_text_in_cell_deferred_pagination(0, 0, 2, 2, 29, 5, "X")
        .expect("deferred insert");
    let t = Instant::now();
    let pages = doc
        .find_pages_for_cell_position(0, 0, 2, 2, Some((29, 6)))
        .expect("pages");
    println!(
        "[deferred-q] 편집 직후 find_pages={:?} → {:?}",
        t.elapsed(),
        pages
    );
    let t = Instant::now();
    let _ = doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 29, 6);
    println!("[deferred-q] 편집 직후 rect(질의1)={:?}", t.elapsed());
    let t = Instant::now();
    let _ = doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 29, 6);
    println!("[deferred-q] rect(질의2)={:?}", t.elapsed());
}



/// [조사] deferred 편집 후 cell_units 재계산 11ms 의 내부 귀속 — sample 용 핫루프.
#[test]
#[ignore]
fn issue4149_deferred_units_rebuild_profile_loop() {
    let bytes =
        std::fs::read("samples/issue1949_giant_cell_nested_tables_perf.hwp").expect("샘플 읽기");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("파싱");
    let _ = doc.page_count();
    let _ = doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 6, 5);
    eprintln!("[units-loop] 시작 pid={}", std::process::id());
    for i in 0..400 {
        let ch = if i % 2 == 0 { "X" } else { "" };
        if ch.is_empty() {
            let _ = doc.delete_text_in_cell_deferred_pagination(0, 0, 2, 2, 6, 5, 1);
        } else {
            let _ = doc.insert_text_in_cell_deferred_pagination(0, 0, 2, 2, 6, 5, ch);
        }
        let _ = doc.find_pages_for_cell_position(0, 0, 2, 2, Some((6, 5)));
    }
    eprintln!("[units-loop] 끝");
}
