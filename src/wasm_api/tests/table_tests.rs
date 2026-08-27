//! table_tests — tests/mod.rs 에서 무변동 이동
use super::*;

/// [#1386] createEmpty는 구역 1개 + 빈 문단 1개를 포함해 생성 직후
/// 편집/조회/내보내기가 가능해야 한다 (구역 0개 → 모든 API 실패 회귀 방지).
#[test]
fn test_create_empty_document_is_editable() {
    let mut doc = HwpDocument::create_empty();
    assert_eq!(doc.get_section_count(), 1, "기본 구역 1개");

    // 편집: 구역 0 / 문단 0에 텍스트 삽입이 성공해야 한다
    doc.insert_text_native(0, 0, 0, "새 문서 첫 문단")
        .expect("createEmpty 문서에 insertText가 동작해야 한다 (#1386)");

    // 조회: 삽입한 텍스트가 읽혀야 한다
    let text = doc
        .get_text_range_native(0, 0, 0, 8)
        .expect("getTextRange가 동작해야 한다");
    assert!(
        text.contains("새 문서"),
        "삽입 텍스트가 조회되어야 한다: {text}"
    );

    // 내보내기: HWP/HWPX 직렬화가 모두 성공해야 한다
    let hwp = doc
        .export_hwp_with_adapter()
        .expect("createEmpty 문서 exportHwp");
    assert!(!hwp.is_empty());
    let hwpx = doc
        .export_hwpx_native()
        .expect("createEmpty 문서 exportHwpx");
    assert!(!hwpx.is_empty());

    // 재파싱: 내보낸 HWP가 다시 열리고 텍스트가 보존되어야 한다
    let reparsed =
        crate::document_core::DocumentCore::from_bytes(&hwp).expect("exportHwp 결과 재파싱");
    assert!(
        reparsed.document().sections[0]
            .paragraphs
            .iter()
            .any(|p| p.text.contains("새 문서")),
        "재파싱 문서에 삽입 텍스트가 보존되어야 한다"
    );
}


#[test]
fn test_insert_text_in_cell() {
    let mut doc = create_doc_with_table();
    let result = doc.insert_text_in_cell_native(0, 0, 0, 0, 0, 1, "추가");
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"charOffset\":3"));
    assert!(
        !json.contains("cellFlowChanged"),
        "immediate insert response schema must remain unchanged"
    );

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells[0].paragraphs[0].text, "셀추가A");
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn deferred_cell_replace_applies_ime_atomically() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    fn contains_text(node: &RenderNode, needle: &str) -> bool {
        if let RenderNodeType::TextRun(run) = &node.node_type {
            if run.text.contains(needle) {
                return true;
            }
        }
        node.children
            .iter()
            .any(|child| contains_text(child, needle))
    }

    let mut doc = create_doc_with_table();
    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 2, "ㅎ")
        .expect("seed composition");
    doc.build_page_render_tree(0).expect("warm page tree");

    let raw = doc
        .replace_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 2, 1, "하")
        .expect("atomic composition replace");
    let result: Value = serde_json::from_str(&raw).expect("replace result json");

    assert_eq!(result["charOffset"].as_u64(), Some(3));
    assert_eq!(result["cellFlowChanged"].as_bool(), Some(false));
    match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => {
            let para = &table.cells[0].paragraphs[0];
            assert_eq!(para.text, "셀A하");
            assert_eq!(para.char_count, 3);
            assert_eq!(para.char_offsets, make_char_offsets("셀A하"));
        }
        other => panic!("table control expected: {other:?}"),
    }

    let transient_tree = doc.build_page_render_tree(0).expect("transient page tree");
    assert!(
        contains_text(&transient_tree.root, "하"),
        "warm page tree must expose the final composition before pagination"
    );
    assert_eq!(doc.event_log.len(), 2, "seed insert + atomic replace");
    assert!(matches!(
        doc.event_log.last(),
        Some(crate::model::event::DocumentEvent::CellTextChanged {
            section: 0,
            para: 0,
            ctrl: 0,
            cell: 0,
        })
    ));
}



#[test]
fn deferred_cell_replace_reports_real_flow_boundary() {
    use crate::model::shape::{Caption, CaptionDirection};

    let mut doc = create_doc_with_table();
    match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => {
            table.caption = Some(Caption {
                direction: CaptionDirection::Bottom,
                width: 2_000,
                max_width: 2_000,
                paragraphs: vec![Paragraph {
                    text: "가".to_string(),
                    char_count: 1,
                    char_offsets: make_char_offsets("가"),
                    line_segs: vec![LineSeg {
                        line_height: 400,
                        baseline_distance: 320,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            });
        }
        other => panic!("table control expected: {other:?}"),
    }
    doc.reflow_cell_paragraph(0, 0, 0, 65534, 0);

    let raw = doc
        .replace_text_in_cell_native_deferred_pagination(
            0,
            0,
            0,
            65534,
            0,
            0,
            1,
            "가나다라마바사아",
        )
        .expect("caption boundary replace");
    let result: Value = serde_json::from_str(&raw).expect("boundary result json");
    assert_eq!(result["cellFlowChanged"].as_bool(), Some(true));
    match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => assert!(
            table.caption.as_ref().expect("table caption").paragraphs[0]
                .line_segs
                .len()
                > 1,
            "replacement must cross a line-flow boundary"
        ),
        other => panic!("table control expected: {other:?}"),
    }
}



#[test]
fn deferred_cell_replace_preserves_clickhere_range_and_offsets() {
    let mut doc = create_doc_with_table();
    let mut legacy = create_doc_with_table();
    doc.insert_click_here_field_at_in_cell(0, 0, 0, 0, 0, 2, false, "안내", "메모", "이름", true)
        .expect("insert empty ClickHere");
    legacy
        .insert_click_here_field_at_in_cell(0, 0, 0, 0, 0, 2, false, "안내", "메모", "이름", true)
        .expect("insert legacy empty ClickHere");
    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 2, "ㅎ")
        .expect("seed field composition");
    legacy
        .insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 2, "ㅎ")
        .expect("seed legacy field composition");
    doc.event_log.clear();
    legacy.event_log.clear();

    doc.replace_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 2, 1, "하")
        .expect("replace field composition");
    legacy
        .delete_text_in_cell_native(0, 0, 0, 0, 0, 2, 1)
        .expect("legacy field composition delete");
    legacy
        .insert_text_in_cell_native(0, 0, 0, 0, 0, 2, "하")
        .expect("legacy field composition insert");

    let para = match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => &table.cells[0].paragraphs[0],
        other => panic!("table control expected: {other:?}"),
    };
    let legacy_para = match &legacy.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => &table.cells[0].paragraphs[0],
        other => panic!("legacy table control expected: {other:?}"),
    };
    assert_eq!(para.text, "셀A하");
    assert_eq!(para.char_offsets, legacy_para.char_offsets);
    assert_eq!(para.field_ranges.len(), 1);
    assert_eq!(
        para.field_ranges[0].control_idx,
        legacy_para.field_ranges[0].control_idx
    );
    assert_eq!(
        para.field_ranges[0].start_char_idx,
        legacy_para.field_ranges[0].start_char_idx
    );
    assert_eq!(
        para.field_ranges[0].end_char_idx,
        legacy_para.field_ranges[0].end_char_idx
    );
    assert_eq!(para.field_ranges[0].start_char_idx, 2);
    assert_eq!(para.field_ranges[0].end_char_idx, 3);
    assert_eq!(
        doc.event_log.len(),
        1,
        "replace emits only final cell state"
    );
    assert_eq!(
        legacy.event_log.len(),
        2,
        "legacy delete+insert exposes two intermediate events"
    );
}



#[test]
fn deferred_cell_replace_rejects_invalid_input_before_mutation() {
    let mut doc = create_doc_with_table();
    let before = match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => table.cells[0].paragraphs[0].text.clone(),
        other => panic!("table control expected: {other:?}"),
    };

    let result = doc.replace_text_in_cell_native_deferred_pagination(
        0,
        0,
        0,
        0,
        0,
        2,
        1,
        "가나다라마바사아자",
    );

    assert!(
        result.is_err(),
        "more than eight replacement chars must fail"
    );
    match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => assert_eq!(table.cells[0].paragraphs[0].text, before),
        other => panic!("table control expected: {other:?}"),
    }
}



#[test]
fn test_delete_text_in_cell() {
    let mut doc = create_doc_with_table();
    let result = doc.delete_text_in_cell_native(0, 0, 0, 1, 0, 0, 1);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells[1].paragraphs[0].text, "B");
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn test_table_transpose_clipboard_native_api() {
    let mut doc = create_doc_with_table();
    assert!(!doc.has_table_transpose_clipboard_native());

    let copy = doc
        .copy_table_cells_transposed_native(0, 0, 0, 0, 0, 1, 1)
        .unwrap();
    let copy_json: Value = serde_json::from_str(&copy).unwrap();
    assert_eq!(copy_json["ok"], true);
    assert_eq!(copy_json["sourceRows"], 2);
    assert_eq!(copy_json["sourceCols"], 2);
    assert!(doc.has_table_transpose_clipboard_native());

    let paste = doc
        .paste_table_cells_transposed_native(0, 0, 0, 0, 0)
        .unwrap();
    let paste_json: Value = serde_json::from_str(&paste).unwrap();
    assert_eq!(paste_json["ok"], true);
    assert_eq!(paste_json["targetRows"], 2);
    assert_eq!(paste_json["targetCols"], 2);

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells[0].paragraphs[0].text, "셀A");
        assert_eq!(table.cells[1].paragraphs[0].text, "셀C");
        assert_eq!(table.cells[2].paragraphs[0].text, "셀B");
        assert_eq!(table.cells[3].paragraphs[0].text, "셀D");
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
    assert!(matches!(
        doc.event_log.last(),
        Some(crate::model::event::DocumentEvent::TableCellsTransposed {
            section: 0,
            para: 0,
            ctrl: 0,
        })
    ));
}



#[test]
fn test_table_transpose_in_place_native_api() {
    let mut doc = create_doc_with_table();

    let result = doc.transpose_table_cells_in_place_native(0, 0, 0).unwrap();
    let json: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["sourceRows"], 2);
    assert_eq!(json["sourceCols"], 2);
    assert_eq!(json["targetRows"], 2);
    assert_eq!(json["targetCols"], 2);

    assert_eq!(doc.document.sections[0].paragraphs.len(), 1);
    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.row_count, 2);
        assert_eq!(table.col_count, 2);
        assert_eq!(table.cells[0].paragraphs[0].text, "셀A");
        assert_eq!(table.cells[1].paragraphs[0].text, "셀C");
        assert_eq!(table.cells[2].paragraphs[0].text, "셀B");
        assert_eq!(table.cells[3].paragraphs[0].text, "셀D");
    } else {
        panic!("행/열이 바뀐 기존 표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn test_table_transpose_paste_as_new_table_native_api() {
    let mut doc = create_doc_with_table();
    doc.copy_table_cells_transposed_native(0, 0, 0, 0, 0, 1, 1)
        .unwrap();

    let paste = doc
        .paste_table_cells_transposed_as_new_table_native(0, 0, 0)
        .unwrap();
    let paste_json: Value = serde_json::from_str(&paste).unwrap();
    assert_eq!(paste_json["ok"], true);
    assert_eq!(paste_json["paraIdx"], 1);
    assert_eq!(paste_json["controlIdx"], 0);
    assert_eq!(paste_json["targetRows"], 2);
    assert_eq!(paste_json["targetCols"], 2);

    if let Some(Control::Table(source_table)) =
        doc.document.sections[0].paragraphs[0].controls.first()
    {
        assert_eq!(source_table.cells[0].paragraphs[0].text, "셀A");
        assert_eq!(source_table.cells[1].paragraphs[0].text, "셀B");
        assert_eq!(source_table.cells[2].paragraphs[0].text, "셀C");
        assert_eq!(source_table.cells[3].paragraphs[0].text, "셀D");
    } else {
        panic!("원본 표 컨트롤을 찾을 수 없음");
    }

    if let Some(Control::Table(target_table)) =
        doc.document.sections[0].paragraphs[1].controls.first()
    {
        assert_eq!(target_table.row_count, 2);
        assert_eq!(target_table.col_count, 2);
        assert_eq!(target_table.cells[0].paragraphs[0].text, "셀A");
        assert_eq!(target_table.cells[1].paragraphs[0].text, "셀C");
        assert_eq!(target_table.cells[2].paragraphs[0].text, "셀B");
        assert_eq!(target_table.cells[3].paragraphs[0].text, "셀D");
    } else {
        panic!("행/열 바꿈 붙여넣기 표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn test_cell_text_edit_invalid_indices() {
    let mut doc = create_doc_with_table();

    let result = doc.insert_text_in_cell_native(0, 0, 0, 99, 0, 0, "X");
    assert!(result.is_err());

    let result = doc.insert_text_in_cell_native(0, 0, 5, 0, 0, 0, "X");
    assert!(result.is_err());

    let result = doc.insert_text_in_cell_native(99, 0, 0, 0, 0, 0, "X");
    assert!(result.is_err());
}



#[test]
fn test_cell_text_layout_contains_cell_info() {
    let doc = create_doc_with_table();
    let layout = doc.get_page_text_layout_native(0);
    assert!(layout.is_ok());
    let json = layout.unwrap();

    assert!(json.contains("\"parentParaIdx\":"));
    assert!(json.contains("\"controlIdx\":"));
    assert!(json.contains("\"cellIdx\":"));
    assert!(json.contains("\"cellParaIdx\":"));
}



#[test]
fn test_insert_and_delete_roundtrip_in_cell() {
    let mut doc = create_doc_with_table();

    let result = doc.insert_text_in_cell_native(0, 0, 0, 2, 0, 2, "테스트");
    assert!(result.is_ok());

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells[2].paragraphs[0].text, "셀C테스트");
    }

    let result = doc.delete_text_in_cell_native(0, 0, 0, 2, 0, 2, 3);
    assert!(result.is_ok());

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells[2].paragraphs[0].text, "셀C");
    }
}



#[test]
fn test_svg_render_with_table_after_cell_edit() {
    let mut doc = create_doc_with_table();

    doc.insert_text_in_cell_native(0, 0, 0, 3, 0, 2, "수정됨")
        .unwrap();
    // 삽입 후 셀 텍스트 확인
    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells[3].paragraphs[0].text, "셀D수정됨");
    }
    let svg = doc.render_page_svg_native(0);
    assert!(svg.is_ok());
    let svg = svg.unwrap();
    // 언어별 폰트 분기로 "셀", "D", "수정됨"이 별도 text run으로 분리될 수 있으므로
    // 각 부분이 SVG에 포함되는지 확인
    // 문자별 개별 렌더링이므로 개별 문자 존재 확인
    assert!(svg.contains(">수</text>"), "SVG에 '수' 없음");
    assert!(svg.contains(">정</text>"), "SVG에 '정' 없음");
    assert!(svg.contains(">됨</text>"), "SVG에 '됨' 없음");
}



#[test]
fn test_get_page_control_layout_with_table() {
    let doc = create_doc_with_table();
    let result = doc.get_page_control_layout_native(0);
    assert!(result.is_ok());
    let json = result.unwrap();

    // 표 컨트롤이 포함되어야 함
    assert!(json.contains("\"type\":\"table\""));
    assert!(json.contains("\"rowCount\":"));
    assert!(json.contains("\"colCount\":"));
    // 문서 좌표 포함
    assert!(json.contains("\"secIdx\":"));
    assert!(json.contains("\"paraIdx\":"));
    assert!(json.contains("\"controlIdx\":"));
    // 셀 정보 포함
    assert!(json.contains("\"cells\":["));
    assert!(json.contains("\"cellIdx\":"));
    assert!(json.contains("\"row\":"));
    assert!(json.contains("\"col\":"));
}



#[test]
fn test_control_layout_cell_bounding_boxes() {
    let doc = create_doc_with_table();
    let result = doc.get_page_control_layout_native(0);
    assert!(result.is_ok());
    let json = result.unwrap();

    // JSON 파싱 검증: 표 바운딩 박스가 유효한 크기를 가짐
    assert!(json.contains("\"w\":"));
    assert!(json.contains("\"h\":"));

    // 셀이 4개 (2x2 표)
    let cell_count = json.matches("\"cellIdx\":").count();
    assert_eq!(cell_count, 4, "2x2 표에는 4개의 셀이 있어야 합니다");
}

// === 표 구조 편집 테스트 ===



#[test]
fn test_insert_table_row_below() {
    let mut doc = create_doc_with_table();
    let result = doc.insert_table_row_native(0, 0, 0, 0, true);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"rowCount\":3"));
    assert!(json.contains("\"colCount\":2"));

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.row_count, 3);
        assert_eq!(table.cells.len(), 6);
        // 원래 첫 행의 셀A는 여전히 행 0
        assert_eq!(table.cells[0].row, 0);
        assert_eq!(table.cells[0].paragraphs[0].text, "셀A");
        // 새 행은 행 1 (빈 문단)
        assert_eq!(table.cells[2].row, 1);
        assert!(table.cells[2].paragraphs[0].text.is_empty());
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn test_insert_table_column_right() {
    let mut doc = create_doc_with_table();
    let result = doc.insert_table_column_native(0, 0, 0, 0, true);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"rowCount\":2"));
    assert!(json.contains("\"colCount\":3"));

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.col_count, 3);
        assert_eq!(table.cells.len(), 6);
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn test_merge_table_cells() {
    let mut doc = create_doc_with_table();
    // 첫 행의 2개 셀 병합
    let result = doc.merge_table_cells_native(0, 0, 0, 0, 0, 0, 1);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"cellCount\":3")); // 비주 셀 1개 제거

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells.len(), 3); // 비주 셀 제거됨
        let merged = &table.cells[0];
        assert_eq!(merged.col_span, 2);
        assert_eq!(merged.row_span, 1);
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



/// [merge stale local-resize] 병합으로 셀 배열 인덱스가 바뀌면
/// local_resize_cell_widths의 cell 인덱스 참조가 stale 해진다.
///
/// 2×2 표에서 셀 3(row=1,col=1)에 로컬 resize 폭을 저장해 둔 뒤 (0,0)~(0,1)을 병합하면
/// Table::merge_cells()가 비주 셀 하나를 retain()으로 제거해 cells.len()이 4→3으로
/// 줄어든다. local_resize_cell_widths가 갱신되지 않으면 이제 존재하지 않는 인덱스 3을
/// 계속 가리켜, 이 값을 cells[idx]로 읽는 렌더링/직렬화 경로가 범위를 벗어나거나
/// 병합 후 엉뚱한 셀에 로컬 resize 폭을 적용하게 된다.
#[test]
fn test_merge_table_cells_clears_stale_local_resize_widths() {
    let mut doc = create_doc_with_table();
    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first_mut()
    {
        // 병합 전: 셀 인덱스 3(row=1,col=1)에 로컬 resize 폭 저장.
        table.local_resize_cell_widths.push((3, 1234));
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }

    // (0,0)~(0,1) 병합 — 비주 셀 하나 제거, cells.len() 4→3.
    doc.merge_table_cells_native(0, 0, 0, 0, 0, 0, 1).unwrap();

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(
            table.cells.len(),
            3,
            "병합으로 비주 셀 하나가 제거돼야 함(전제 확인)"
        );
        assert!(
            table.local_resize_cell_widths.is_empty(),
            "병합 후 셀 인덱스가 재배치되므로 local_resize_cell_widths의 stale 참조(인덱스 3)가 \
             비워져야 한다"
        );
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



/// [delete_row stale local-resize] 행 삭제로 셀 배열 인덱스가 바뀌면
/// local_resize_cell_heights의 cell 인덱스 참조가 stale 해진다.
///
/// 3×2 표(row 0,1,2 × col 0,1)에서 셀 인덱스 2(row=1,col=0)에 로컬 resize 높이를
/// 저장해 둔 뒤 row 0을 삭제하면 Table::delete_row()가 row 0의 셀 2개를 retain()으로
/// 제거해 cells.len()이 6→4로 줄고, 남은 셀을 sort_by_key(row, col)로 재정렬한다.
/// local_resize_cell_heights가 갱신되지 않으면 이제 존재하지 않거나(범위 초과) 엉뚱한
/// 셀을 가리키는 stale 참조가 남아, 이 값을 cells[idx]로 읽는 렌더링/직렬화 경로가
/// 패닉하거나 삭제 후 남은 엉뚱한 셀에 잘못된 로컬 resize 높이를 적용하게 된다.
#[test]
fn test_delete_table_row_clears_stale_local_resize_heights() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc.create_table_native(0, 0, 0, 3, 2).expect("3x2 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[table_para_idx]
        .controls
        .first_mut()
    {
        // 삭제 전: 셀 인덱스 2(row=1,col=0)에 로컬 resize 높이 저장.
        table.local_resize_cell_heights.push((2, 5678));
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }

    // row 0 삭제 — 셀 2개 제거, cells.len() 6→4.
    doc.delete_table_row_native(0, table_para_idx, 0, 0)
        .expect("행 삭제");

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[table_para_idx]
        .controls
        .first()
    {
        assert_eq!(
            table.cells.len(),
            4,
            "행 삭제로 셀 2개가 제거돼야 함(전제 확인)"
        );
        assert!(
            table.local_resize_cell_heights.is_empty(),
            "행 삭제 후 셀 인덱스가 재배치되므로 local_resize_cell_heights의 stale 참조(인덱스 2)가 \
             비워져야 한다"
        );
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



/// [insert_row/insert_column stale local-resize] 행/열 삽입으로 셀 배열 인덱스가
/// 바뀌면 local_resize_cell_widths/heights의 cell 인덱스 참조가 stale 해진다.
///
/// Table::insert_row()/insert_column()은 새 셀을 push()한 뒤 sort_by_key(row, col)로
/// 전체 셀 배열을 재정렬한다(delete_row가 retain()+정렬로 stale을 만드는 것과 같은
/// 근본 원인). 3×2 표에서 셀 인덱스 2에 로컬 resize 값을 저장해 둔 뒤 행을 삽입하면
/// 재정렬로 인덱스 2가 더 이상 같은 셀을 가리키지 않으므로, 이 값을 cells[idx]로
/// 읽는 렌더링/직렬화 경로가 엉뚱한 셀에 잘못된 로컬 resize 값을 적용하게 된다.
/// 열 삽입도 동일 원인으로 같은 결과를 낳는다.
#[test]
fn test_insert_table_row_and_column_clear_stale_local_resize() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc.create_table_native(0, 0, 0, 3, 2).expect("3x2 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[table_para_idx]
        .controls
        .first_mut()
    {
        table.local_resize_cell_widths.push((2, 1234));
        table.local_resize_cell_heights.push((2, 5678));
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }

    doc.insert_table_row_native(0, table_para_idx, 0, 0, true)
        .expect("행 삽입");

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[table_para_idx]
        .controls
        .first()
    {
        assert!(
            table.local_resize_cell_widths.is_empty() && table.local_resize_cell_heights.is_empty(),
            "행 삽입 후 셀 인덱스가 재배치되므로 local_resize_cell_widths/heights의 \
             stale 참조(인덱스 2)가 비워져야 한다"
        );
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[table_para_idx]
        .controls
        .first_mut()
    {
        table.local_resize_cell_widths.push((2, 1234));
        table.local_resize_cell_heights.push((2, 5678));
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }

    doc.insert_table_column_native(0, table_para_idx, 0, 0, true)
        .expect("열 삽입");

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[table_para_idx]
        .controls
        .first()
    {
        assert!(
            table.local_resize_cell_widths.is_empty() && table.local_resize_cell_heights.is_empty(),
            "열 삽입 후에도 local_resize_cell_widths/heights의 stale 참조가 비워져야 한다"
        );
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn test_split_table_cell() {
    let mut doc = create_doc_with_table();
    // 먼저 병합
    doc.merge_table_cells_native(0, 0, 0, 0, 0, 0, 1).unwrap();
    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells.len(), 3);
    }

    // 나누기
    let result = doc.split_table_cell_native(0, 0, 0, 0, 0);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"cellCount\":4"));

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert_eq!(table.cells.len(), 4);
        let cell = &table.cells[0];
        assert_eq!(cell.col_span, 1);
        assert_eq!(cell.row_span, 1);
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



/// [delete_table_column/split_table_cell stale local-resize] #2832/#2843/#2853과
/// 동일한 버그 클래스의 마지막 두 인스턴스. Table::delete_column()/split_cell()이
/// cells 배열의 인덱스 배치를 바꾸므로, local_resize_cell_widths/heights가 물고 있던
/// 이전 cell_idx는 정리되지 않으면 stale 참조로 남는다.
#[test]
fn test_delete_table_column_and_split_cell_clear_stale_local_resize() {
    let mut doc = create_doc_with_table();

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first_mut()
    {
        table.local_resize_cell_widths.push((1, 1234));
        table.local_resize_cell_heights.push((1, 5678));
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
    doc.delete_table_column_native(0, 0, 0, 0).expect("열 삭제");
    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert!(
            table.local_resize_cell_widths.is_empty() && table.local_resize_cell_heights.is_empty(),
            "열 삭제 후 local_resize_cell_widths/heights의 stale 참조가 비워져야 한다"
        );
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first_mut()
    {
        table.local_resize_cell_widths.push((0, 1234));
        table.local_resize_cell_heights.push((0, 5678));
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
    doc.merge_table_cells_native(0, 0, 0, 0, 0, 1, 0)
        .expect("병합");
    doc.split_table_cell_native(0, 0, 0, 0, 0).expect("분할");
    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert!(
            table.local_resize_cell_widths.is_empty() && table.local_resize_cell_heights.is_empty(),
            "셀 분할 후 local_resize_cell_widths/heights의 stale 참조가 비워져야 한다"
        );
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



/// [split_table_cells_in_range stale local-resize] split_table_cell_native/
/// split_table_cell_into_native와 동일하게 split_table_cells_in_range_native도
/// Table::split_cells_in_range()가 내부적으로 split_cell_into()를 반복 호출해
/// cells 배열의 인덱스 배치를 바꾼다. 그런데 이 커맨드만 local_resize_cell_widths/
/// heights를 비우지 않아 stale 참조가 남는다.
#[test]
fn test_split_table_cells_in_range_clears_stale_local_resize() {
    let mut doc = create_doc_with_table();

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first_mut()
    {
        table.local_resize_cell_widths.push((1, 1234));
        table.local_resize_cell_heights.push((1, 5678));
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }

    doc.split_table_cells_in_range_native(0, 0, 0, 0, 0, 1, 1, 2, 2, false)
        .expect("범위 분할");

    if let Some(Control::Table(table)) = doc.document.sections[0].paragraphs[0].controls.first() {
        assert!(
            table.local_resize_cell_widths.is_empty() && table.local_resize_cell_heights.is_empty(),
            "범위 분할 후 local_resize_cell_widths/heights의 stale 참조가 비워져야 한다"
        );
    } else {
        panic!("표 컨트롤을 찾을 수 없음");
    }
}



#[test]
fn test_insert_table_row_invalid_index() {
    let mut doc = create_doc_with_table();
    let result = doc.insert_table_row_native(0, 0, 0, 99, true);
    assert!(result.is_err());
}



#[test]
fn test_table_structure_edit_roundtrip() {
    let mut doc = create_doc_with_table();
    // 행 삽입
    doc.insert_table_row_native(0, 0, 0, 0, true).unwrap();
    // 열 삽입
    doc.insert_table_column_native(0, 0, 0, 0, true).unwrap();

    // 직렬화 → 재파싱
    let bytes = doc.export_hwp_native();
    assert!(bytes.is_ok(), "행/열 삽입 후 직렬화 실패");
    let bytes = bytes.unwrap();
    assert!(!bytes.is_empty());

    // 재파싱 가능 여부 확인
    let reparsed = crate::parser::parse_hwp(&bytes);
    assert!(reparsed.is_ok(), "재파싱 실패: {:?}", reparsed.err());
}



#[test]
fn test_real_hwp_table_insert_row_roundtrip() {
    use crate::parser::record::Record;
    use std::path::Path;

    let path = Path::new("samples/hwp_table_test.hwp");
    if !path.exists() {
        eprintln!("hwp_table_test.hwp 없음 — 건너뜀");
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = crate::parser::parse_hwp(&data).unwrap();

    // 원본 BodyText 레코드
    let mut cfb = crate::parser::cfb_reader::CfbReader::open(&data).unwrap();
    let orig_bt = cfb
        .read_body_text_section(0, doc.header.compressed, false)
        .unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();

    // 원본 표 부분 레코드 추출 (CTRL_HEADER tbl ~ 다음 레벨0 레코드)
    let mut table_start = 0;
    let mut table_end = 0;
    for (i, rec) in orig_recs.iter().enumerate() {
        if rec.tag_id == crate::parser::tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes(rec.data[0..4].try_into().unwrap());
            if ctrl_id == crate::parser::tags::CTRL_TABLE {
                table_start = i;
                // 표 끝 찾기
                table_end = orig_recs.len();
                for j in (i + 1)..orig_recs.len() {
                    if orig_recs[j].level <= rec.level {
                        table_end = j;
                        break;
                    }
                }
                break;
            }
        }
    }

    eprintln!(
        "=== 원본 표 레코드: [{}..{}] ({} records) ===",
        table_start,
        table_end,
        table_end - table_start
    );
    for i in table_start..table_end {
        let r = &orig_recs[i];
        let tag = crate::parser::tags::tag_name(r.tag_id);
        eprintln!(
            "  [{}] {} L{} {}B {:02X?}",
            i,
            tag,
            r.level,
            r.data.len(),
            &r.data[..r.data.len().min(16)]
        );
    }

    // 행 삽입 후 내보내기
    let mut hwp_doc = HwpDocument::create_empty();
    hwp_doc.set_document(crate::parser::parse_hwp(&data).unwrap());
    hwp_doc.insert_table_row_native(0, 3, 0, 0, true).unwrap();

    let exported = hwp_doc.export_hwp_native().unwrap();
    let mut cfb2 = crate::parser::cfb_reader::CfbReader::open(&exported).unwrap();
    let new_doc = crate::parser::parse_hwp(&exported).unwrap();
    let new_bt = cfb2
        .read_body_text_section(0, new_doc.header.compressed, false)
        .unwrap();
    let new_recs = Record::read_all(&new_bt).unwrap();

    // 수정 후 표 레코드
    let mut new_table_start = 0;
    let mut new_table_end = 0;
    for (i, rec) in new_recs.iter().enumerate() {
        if rec.tag_id == crate::parser::tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes(rec.data[0..4].try_into().unwrap());
            if ctrl_id == crate::parser::tags::CTRL_TABLE {
                new_table_start = i;
                new_table_end = new_recs.len();
                for j in (i + 1)..new_recs.len() {
                    if new_recs[j].level <= rec.level {
                        new_table_end = j;
                        break;
                    }
                }
                break;
            }
        }
    }

    eprintln!(
        "\n=== 수정 후 표 레코드: [{}..{}] ({} records) ===",
        new_table_start,
        new_table_end,
        new_table_end - new_table_start
    );
    for i in new_table_start..new_table_end {
        let r = &new_recs[i];
        let tag = crate::parser::tags::tag_name(r.tag_id);
        eprintln!(
            "  [{}] {} L{} {}B {:02X?}",
            i,
            tag,
            r.level,
            r.data.len(),
            &r.data[..r.data.len().min(16)]
        );
    }

    // 원본 빈 셀(1,0)과 새 셀 LIST_HEADER 바이트 비교
    let orig_table_recs = &orig_recs[table_start..table_end];
    let new_table_recs = &new_recs[new_table_start..new_table_end];

    // LIST_HEADER 레코드 모두 추출
    eprintln!("\n=== 원본 LIST_HEADER 바이트 ===");
    for (i, r) in orig_table_recs.iter().enumerate() {
        if r.tag_id == crate::parser::tags::HWPTAG_LIST_HEADER {
            eprintln!(
                "  [{}] {}B: {:02X?}",
                table_start + i,
                r.data.len(),
                &r.data
            );
        }
    }
    eprintln!("\n=== 수정 후 LIST_HEADER 바이트 ===");
    for (i, r) in new_table_recs.iter().enumerate() {
        if r.tag_id == crate::parser::tags::HWPTAG_LIST_HEADER {
            eprintln!(
                "  [{}] {}B: {:02X?}",
                new_table_start + i,
                r.data.len(),
                &r.data
            );
        }
    }

    // PARA_HEADER 바이트 비교
    eprintln!("\n=== 원본 PARA_HEADER (표 내부) ===");
    for (i, r) in orig_table_recs.iter().enumerate() {
        if r.tag_id == crate::parser::tags::HWPTAG_PARA_HEADER {
            eprintln!(
                "  [{}] {}B: {:02X?}",
                table_start + i,
                r.data.len(),
                &r.data
            );
        }
    }
    eprintln!("\n=== 수정 후 PARA_HEADER (표 내부) ===");
    for (i, r) in new_table_recs.iter().enumerate() {
        if r.tag_id == crate::parser::tags::HWPTAG_PARA_HEADER {
            eprintln!(
                "  [{}] {}B: {:02X?}",
                new_table_start + i,
                r.data.len(),
                &r.data
            );
        }
    }

    // TABLE 레코드 비교
    eprintln!("\n=== TABLE 레코드 비교 ===");
    for r in orig_table_recs.iter() {
        if r.tag_id == crate::parser::tags::HWPTAG_TABLE {
            eprintln!("  원본: {}B: {:02X?}", r.data.len(), &r.data);
        }
    }
    for r in new_table_recs.iter() {
        if r.tag_id == crate::parser::tags::HWPTAG_TABLE {
            eprintln!("  수정: {}B: {:02X?}", r.data.len(), &r.data);
        }
    }
}



#[test]
/// 실제 HWP 파일에서 셀 병합 후 포괄적 바이너리 비교
fn test_merge_cells_roundtrip_real_hwp() {
    use crate::parser::record::Record;
    use std::path::Path;

    let orig_path = Path::new("samples/hwp_table_test.hwp");
    if !orig_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();

    // 1) 원본 → 수정 없이 라운드트립 (기준선)
    let mut baseline_doc = HwpDocument::from_bytes(&orig_data).unwrap();
    let baseline_exported = baseline_doc.export_hwp_native().unwrap();

    // 2) 원본 → 병합 후 내보내기
    let mut merged_doc = HwpDocument::from_bytes(&orig_data).unwrap();
    merged_doc
        .merge_table_cells_native(0, 3, 0, 2, 0, 2, 1)
        .unwrap();
    let merged_exported = merged_doc.export_hwp_native().unwrap();

    // 검증용 파일 저장
    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/merge_test_baseline.hwp", &baseline_exported).unwrap();
    std::fs::write("output/merge_test_programmatic.hwp", &merged_exported).unwrap();
    eprintln!("검증 파일 저장: output/merge_test_baseline.hwp, output/merge_test_programmatic.hwp");

    // 기준선 BodyText
    let baseline_parsed = crate::parser::parse_hwp(&baseline_exported).unwrap();
    let mut baseline_cfb = crate::parser::cfb_reader::CfbReader::open(&baseline_exported).unwrap();
    let baseline_bt = baseline_cfb
        .read_body_text_section(0, baseline_parsed.header.compressed, false)
        .unwrap();
    let baseline_recs = Record::read_all(&baseline_bt).unwrap();

    // 병합 BodyText
    let merged_parsed = crate::parser::parse_hwp(&merged_exported).unwrap();
    let mut merged_cfb = crate::parser::cfb_reader::CfbReader::open(&merged_exported).unwrap();
    let merged_bt = merged_cfb
        .read_body_text_section(0, merged_parsed.header.compressed, false)
        .unwrap();
    let merged_recs = Record::read_all(&merged_bt).unwrap();

    eprintln!(
        "기준선 레코드: {}, 병합 레코드: {}",
        baseline_recs.len(),
        merged_recs.len()
    );

    // 표 범위 찾기
    let find_table = |recs: &[Record]| -> (usize, usize) {
        for (i, rec) in recs.iter().enumerate() {
            if rec.tag_id == crate::parser::tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
                let ctrl_id = u32::from_le_bytes(rec.data[0..4].try_into().unwrap());
                if ctrl_id == crate::parser::tags::CTRL_TABLE {
                    let mut end = recs.len();
                    for j in (i + 1)..recs.len() {
                        if recs[j].level <= rec.level {
                            end = j;
                            break;
                        }
                    }
                    return (i, end);
                }
            }
        }
        (0, 0)
    };

    let (bt_start, bt_end) = find_table(&baseline_recs);
    let (mt_start, mt_end) = find_table(&merged_recs);
    eprintln!(
        "기준선 표: [{}..{}] ({} recs), 병합 표: [{}..{}] ({} recs)",
        bt_start,
        bt_end,
        bt_end - bt_start,
        mt_start,
        mt_end,
        mt_end - mt_start
    );

    // 표 앞쪽 레코드 비교 (동일해야 함)
    let pre_count = bt_start.min(mt_start);
    for i in 0..pre_count {
        if baseline_recs[i].tag_id != merged_recs[i].tag_id
            || baseline_recs[i].data != merged_recs[i].data
        {
            let tag = crate::parser::tags::tag_name(baseline_recs[i].tag_id);
            eprintln!("!! 표 앞 [{}] {} 차이:", i, tag);
            eprintln!(
                "  기준: {:02X?}",
                &baseline_recs[i].data[..baseline_recs[i].data.len().min(40)]
            );
            eprintln!(
                "  병합: {:02X?}",
                &merged_recs[i].data[..merged_recs[i].data.len().min(40)]
            );
        }
    }

    // 표 뒤쪽 레코드 비교
    let bt_after = &baseline_recs[bt_end..];
    let mt_after = &merged_recs[mt_end..];
    if bt_after.len() != mt_after.len() {
        eprintln!(
            "!! 표 뒤 레코드 수 차이: {} vs {}",
            bt_after.len(),
            mt_after.len()
        );
    }
    for i in 0..bt_after.len().min(mt_after.len()) {
        if bt_after[i].tag_id != mt_after[i].tag_id || bt_after[i].data != mt_after[i].data {
            let tag = crate::parser::tags::tag_name(bt_after[i].tag_id);
            eprintln!("!! 표 뒤 [{}] {} 차이:", i, tag);
            eprintln!(
                "  기준: {:02X?}",
                &bt_after[i].data[..bt_after[i].data.len().min(40)]
            );
            eprintln!(
                "  병합: {:02X?}",
                &mt_after[i].data[..mt_after[i].data.len().min(40)]
            );
        }
    }

    // 표 내부 레코드 전체 출력
    eprintln!("\n=== 기준선 표 레코드 ===");
    for i in bt_start..bt_end {
        let r = &baseline_recs[i];
        let tag = crate::parser::tags::tag_name(r.tag_id);
        eprintln!(
            "  [{}] {} L{} {}B {:02X?}",
            i,
            tag,
            r.level,
            r.data.len(),
            &r.data[..r.data.len().min(50)]
        );
    }
    eprintln!("\n=== 병합 표 레코드 ===");
    for i in mt_start..mt_end {
        let r = &merged_recs[i];
        let tag = crate::parser::tags::tag_name(r.tag_id);
        eprintln!(
            "  [{}] {} L{} {}B {:02X?}",
            i,
            tag,
            r.level,
            r.data.len(),
            &r.data[..r.data.len().min(50)]
        );
    }

    // DocInfo 스트림 비교
    let mut baseline_cfb2 = crate::parser::cfb_reader::CfbReader::open(&baseline_exported).unwrap();
    let mut merged_cfb2 = crate::parser::cfb_reader::CfbReader::open(&merged_exported).unwrap();
    let baseline_di = baseline_cfb2
        .read_doc_info(baseline_parsed.header.compressed)
        .unwrap();
    let merged_di = merged_cfb2
        .read_doc_info(merged_parsed.header.compressed)
        .unwrap();
    if baseline_di == merged_di {
        eprintln!("\nDocInfo: 동일 ({}B)", baseline_di.len());
    } else {
        eprintln!(
            "\n!! DocInfo 차이: {}B vs {}B",
            baseline_di.len(),
            merged_di.len()
        );
        for i in 0..baseline_di.len().min(merged_di.len()) {
            if baseline_di[i] != merged_di[i] {
                eprintln!(
                    "  offset {}: {:02X} vs {:02X}",
                    i, baseline_di[i], merged_di[i]
                );
                if i > 5 {
                    eprintln!("  ... (더 있을 수 있음)");
                    break;
                }
            }
        }
    }

    // FileHeader 비교
    let baseline_hdr = &baseline_exported[0..256.min(baseline_exported.len())];
    let merged_hdr = &merged_exported[0..256.min(merged_exported.len())];
    if baseline_hdr != merged_hdr {
        eprintln!("\n!! FileHeader 차이 (첫 256바이트)");
    }

    eprintln!(
        "\n파일 크기: 기준선={}B, 병합={}B",
        baseline_exported.len(),
        merged_exported.len()
    );
}



#[test]
fn test_merge_cells_then_render() {
    let mut doc = create_doc_with_table();
    // 전체 병합
    doc.merge_table_cells_native(0, 0, 0, 0, 0, 1, 1).unwrap();

    // SVG 렌더링 성공 확인
    let svg = doc.render_page_svg_native(0);
    assert!(svg.is_ok());
    assert!(svg.unwrap().contains("<svg"));
}



#[test]
fn test_clipboard_copy_control_cell_path_json_arg() {
    // [Task #1161] copyControl 래퍼의 cell_path_json 인자: 빈 문자열/"[]" 는 본문.
    // (에러 경로는 JsValue 를 구성하므로 native 테스트에서 호출 불가 → OK 경로만 검증.
    //  cell 경로 자체는 tests/issue_1161_copy_picture_in_cell.rs 의 native 테스트로 가드.)
    let mut doc = create_doc_with_table();

    // 빈 문자열 = 본문 → 표 복사
    let r_empty = doc.copy_control(0, 0, "", 0);
    assert!(r_empty.is_ok(), "빈 cell_path_json 본문 복사 실패");
    assert!(r_empty.unwrap().contains("[표]"));

    // "[]" 도 본문
    let r_arr = doc.copy_control(0, 0, "[]", 0);
    assert!(r_arr.is_ok(), "[] cell_path_json 본문 복사 실패");
    assert!(r_arr.unwrap().contains("[표]"));
}



/// 섹션 0에서 첫 번째 표 컨트롤의 (para_idx, ctrl_idx)를 찾는다.
fn find_table_pos(doc: &HwpDocument) -> (usize, usize) {
    use crate::model::control::Control;
    for (pi, p) in doc.document.sections[0].paragraphs.iter().enumerate() {
        for (ci, c) in p.controls.iter().enumerate() {
            if matches!(c, Control::Table(_)) {
                return (pi, ci);
            }
        }
    }
    panic!("표 컨트롤 없음");
}



/// #1323: 표 셀 안 이미지 붙여넣기 — merge_from 컨트롤 병합으로 그림·CTRL_DATA가
/// 보존되어야 한다. 수정 전에는 에러 없이 조용히 누락되었다.
#[test]
fn test_paste_picture_into_table_cell() {
    use crate::model::control::Control;

    let mut doc = create_doc_with_floating_picture(true, 0, 0);
    // CTRL_DATA 인덱스 정렬 검증용 레코드 부여
    doc.document.sections[0].paragraphs[0].ctrl_data_records = vec![Some(vec![7, 7, 7])];
    doc.copy_control_native(0, 0, &[], 0).expect("그림 복사");

    doc.create_table_ex_native(0, 1, 0, 2, 2, true, None, None)
        .expect("표 생성");
    let (t_para, t_ctrl) = find_table_pos(&doc);

    doc.paste_internal_in_cell_native(0, t_para, t_ctrl, 0, 0, 0)
        .expect("셀에 그림 붙여넣기");

    let table = match &doc.document.sections[0].paragraphs[t_para].controls[t_ctrl] {
        Control::Table(t) => t,
        other => panic!("표가 아님: {other:?}"),
    };
    let mut found = None;
    for p in &table.cells[0].paragraphs {
        for (i, c) in p.controls.iter().enumerate() {
            if matches!(c, Control::Picture(_)) {
                found = Some((p, i));
            }
        }
    }
    let (cell_para, pic_idx) = found.expect("셀 안에 그림 컨트롤이 보존되어야 한다 (#1323)");
    assert_eq!(
        cell_para.ctrl_data_records.get(pic_idx).cloned().flatten(),
        Some(vec![7, 7, 7]),
        "CTRL_DATA가 controls 인덱스 정렬을 유지한 채 보존되어야 한다"
    );
}



/// #1323: path 기반 셀 붙여넣기(paste_internal_in_cell_by_path)도 동일하게 그림을 보존한다.
#[test]
fn test_paste_picture_into_cell_by_path() {
    use crate::model::control::Control;

    let mut doc = create_doc_with_floating_picture(true, 0, 0);
    doc.copy_control_native(0, 0, &[], 0).expect("그림 복사");

    doc.create_table_ex_native(0, 1, 0, 2, 2, true, None, None)
        .expect("표 생성");
    let (t_para, t_ctrl) = find_table_pos(&doc);

    // path = [(ctrl_idx, cell_idx, cell_para_idx)] — 셀 1의 문단 0에 붙여넣기
    doc.paste_internal_in_cell_by_path_native(0, t_para, &[(t_ctrl, 1, 0)], 0)
        .expect("path 기반 셀 붙여넣기");

    let table = match &doc.document.sections[0].paragraphs[t_para].controls[t_ctrl] {
        Control::Table(t) => t,
        other => panic!("표가 아님: {other:?}"),
    };
    let pic_count: usize = table.cells[1]
        .paragraphs
        .iter()
        .map(|p| {
            p.controls
                .iter()
                .filter(|c| matches!(c, Control::Picture(_)))
                .count()
        })
        .sum();
    assert_eq!(
        pic_count, 1,
        "path 기반 붙여넣기에서도 그림 컨트롤이 보존되어야 한다 (#1323)"
    );
}



/// #1323 부수 해소: 셀 문단 시작 Backspace 병합(merge_paragraph_in_cell) 시
/// 병합 대상 셀 문단의 컨트롤이 보존되어야 한다.
#[test]
fn test_merge_paragraph_in_cell_preserves_controls() {
    use crate::model::control::Control;

    let mut doc = create_doc_with_floating_picture(true, 0, 0);
    doc.create_table_ex_native(0, 1, 0, 2, 2, true, None, None)
        .expect("표 생성");
    let (t_para, t_ctrl) = find_table_pos(&doc);

    // 셀 0에 그림 문단을 두 번째 문단으로 구성
    let pic_para = doc.document.sections[0].paragraphs[0].clone();
    match &mut doc.document.sections[0].paragraphs[t_para].controls[t_ctrl] {
        Control::Table(t) => t.cells[0].paragraphs.push(pic_para),
        other => panic!("표가 아님: {other:?}"),
    }

    doc.merge_paragraph_in_cell_native(0, t_para, t_ctrl, 0, 1)
        .expect("셀 문단 병합");

    let table = match &doc.document.sections[0].paragraphs[t_para].controls[t_ctrl] {
        Control::Table(t) => t,
        other => panic!("표가 아님: {other:?}"),
    };
    assert_eq!(
        table.cells[0].paragraphs.len(),
        1,
        "셀 문단이 병합되어야 한다"
    );
    assert_eq!(
        table.cells[0].paragraphs[0]
            .controls
            .iter()
            .filter(|c| matches!(c, Control::Picture(_)))
            .count(),
        1,
        "셀 백스페이스 병합 시 그림 컨트롤이 보존되어야 한다 (#1323)"
    );
}



/// #1323: 셀에 그림을 붙여넣은 문서가 HWP5 직렬화 → 재파싱 후에도 그림을 보존한다.
/// (char_count 역산·char_offsets 갭 인코딩이 직렬화 계약과 정합함을 검증)
#[test]
fn test_paste_picture_into_table_cell_hwp5_roundtrip() {
    use crate::model::control::Control;

    let mut doc = create_doc_with_floating_picture(true, 0, 0);
    doc.copy_control_native(0, 0, &[], 0).expect("그림 복사");
    doc.create_table_ex_native(0, 1, 0, 2, 2, true, None, None)
        .expect("표 생성");
    let (t_para, t_ctrl) = find_table_pos(&doc);
    doc.paste_internal_in_cell_native(0, t_para, t_ctrl, 0, 0, 0)
        .expect("셀에 그림 붙여넣기");

    let bytes = doc.export_hwp_native().expect("HWP5 직렬화");
    let doc2 = HwpDocument::from_bytes(&bytes).expect("재파싱");

    let mut found = false;
    for p in &doc2.document.sections[0].paragraphs {
        for c in &p.controls {
            if let Control::Table(t) = c {
                for cell_para in t.cells.iter().flat_map(|cl| cl.paragraphs.iter()) {
                    if cell_para
                        .controls
                        .iter()
                        .any(|cc| matches!(cc, Control::Picture(_)))
                    {
                        found = true;
                    }
                }
            }
        }
    }
    assert!(
        found,
        "HWP5 round-trip 후에도 셀 안 그림 컨트롤이 보존되어야 한다 (#1323)"
    );
}



/// #1323 시각 검증 보조: 표 셀/글상자에 붙여넣은 그림이 SVG 렌더링에 실제
/// `<image>` 요소로 나타나는지 검증한다. BinData를 실제 등록(insert_picture)하여
/// 렌더러가 data URI 이미지를 방출하는 경로를 그대로 사용한다.
#[test]
fn test_paste_picture_into_cell_and_textbox_renders_in_svg() {
    fn minimal_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x00, 0x00, 0x00,
            0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }
    fn parse_idx(res: &str, key: &str) -> usize {
        res.split(&format!("\"{}\":", key))
            .nth(1)
            .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("missing {key} in {res}"))
    }
    fn count_images(svg: &str) -> usize {
        svg.matches("<image").count()
    }

    let mut doc = create_doc_with_floating_picture(true, 0, 0);
    // 헬퍼의 기본 그림은 BinData가 없으므로 실제 그림을 별도 삽입해 사용한다
    let res = doc
        .insert_picture_native(
            0,
            1,
            0,
            &[],
            &minimal_png(),
            5000,
            5000,
            1,
            1,
            "png",
            "",
            None,
            None,
        )
        .expect("본문 그림 삽입");
    let pic_para = parse_idx(&res, "paraIdx");
    let pic_ctrl = parse_idx(&res, "controlIdx");

    let svg_before = doc.render_page_svg_native(0).expect("기준 SVG 렌더");
    let base = count_images(&svg_before);
    assert!(base >= 1, "본문 그림이 SVG에 렌더되어야 한다: {base}");

    doc.copy_control_native(0, pic_para, &[], pic_ctrl)
        .expect("그림 복사");

    // 표 셀에 붙여넣기 → <image> 1개 증가
    // (기존 문단은 모두 그림 컨트롤을 보유하므로 표 전용 빈 문단을 추가)
    doc.document.sections[0]
        .paragraphs
        .push(Paragraph::default());
    let empty_para = doc.document.sections[0].paragraphs.len() - 1;
    doc.create_table_ex_native(0, empty_para, 0, 2, 2, true, None, None)
        .expect("표 생성");
    let (t_para, t_ctrl) = find_table_pos(&doc);
    doc.paste_internal_in_cell_native(0, t_para, t_ctrl, 0, 0, 0)
        .expect("셀에 그림 붙여넣기");

    let svg_cell = doc.render_page_svg_native(0).expect("셀 paste 후 SVG 렌더");
    assert_eq!(
        count_images(&svg_cell),
        base + 1,
        "셀에 붙여넣은 그림이 SVG에 렌더되어야 한다 (#1323)"
    );

    // 글상자에 붙여넣기 → <image> 1개 더 증가
    let tb_res = doc
        .create_shape_control_native(
            0,
            t_para,
            0,
            21600,
            7200,
            0,
            0,
            true,
            "TopAndBottom",
            "textbox",
            false,
            false,
            &[],
        )
        .expect("글상자 생성");
    let tb_para = parse_idx(&tb_res, "paraIdx");
    let tb_ctrl = parse_idx(&tb_res, "controlIdx");
    doc.paste_internal_in_cell_native(0, tb_para, tb_ctrl, 0, 0, 0)
        .expect("글상자에 그림 붙여넣기");

    let svg_tb = doc
        .render_page_svg_native(0)
        .expect("글상자 paste 후 SVG 렌더");
    assert_eq!(
        count_images(&svg_tb),
        base + 2,
        "글상자에 붙여넣은 그림이 SVG에 렌더되어야 한다 (#1323)"
    );
}



#[test]
fn test_export_control_html_table() {
    let mut doc = create_doc_with_table();

    let result = doc.export_control_html_native(0, 0, &[], 0);
    assert!(result.is_ok());
    let html = result.unwrap();

    assert!(html.contains("<table"));
    assert!(html.contains("</table>"));
    assert!(html.contains("<td"));
    assert!(html.contains("<tr>"));
}

// === HTML 붙여넣기 테스트 ===



#[test]
fn test_paste_html_table_as_control() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    document
        .doc_info
        .border_fills
        .push(crate::model::style::BorderFill::default());
    let mut para = Paragraph::default();
    para.text = "".to_string();
    para.char_count = 0;
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // 2×2 표 HTML
    let html = r#"<html><body><!--StartFragment-->
            <table><tr><td>셀1</td><td>셀2</td></tr><tr><td>셀3</td><td>셀4</td></tr></table>
        <!--EndFragment--></body></html>"#;

    let result = doc.paste_html_native(0, 0, 0, html);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));

    // Table Control이 삽입되었는지 확인
    let paras = &doc.document.sections[0].paragraphs;
    let table_para = paras.iter().find(|p| !p.controls.is_empty());
    assert!(
        table_para.is_some(),
        "Table Control을 포함하는 문단이 있어야 함"
    );

    let table_para = table_para.unwrap();
    assert!(
        table_para.text.is_empty(),
        "컨트롤 문단의 text는 비어있어야 함"
    );
    assert_eq!(table_para.controls.len(), 1);

    if let Control::Table(ref tbl) = table_para.controls[0] {
        assert_eq!(tbl.row_count, 2, "행 수 2");
        assert_eq!(tbl.col_count, 2, "열 수 2");
        assert_eq!(tbl.cells.len(), 4, "셀 4개");

        // 셀 내용 확인
        let cell_texts: Vec<String> = tbl
            .cells
            .iter()
            .map(|c| {
                c.paragraphs
                    .iter()
                    .map(|p| p.text.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();
        assert!(cell_texts.iter().any(|t| t.contains("셀1")), "셀1 포함");
        assert!(cell_texts.iter().any(|t| t.contains("셀2")), "셀2 포함");
        assert!(cell_texts.iter().any(|t| t.contains("셀3")), "셀3 포함");
        assert!(cell_texts.iter().any(|t| t.contains("셀4")), "셀4 포함");

        // 정상 파일 패턴과 일치하는 속성값 검증
        assert_eq!(tbl.attr, 0x082A2311, "table.attr = 0x082A2311");
        assert_eq!(
            tbl.raw_table_record_attr, 0x04000006,
            "raw_table_record_attr (DIFF-5: 셀분리금지 항상 설정)"
        );
        assert_eq!(tbl.padding.left, 510, "table padding left");
        assert_eq!(tbl.padding.right, 510, "table padding right");
        assert_eq!(tbl.padding.top, 141, "table padding top");
        assert_eq!(tbl.padding.bottom, 141, "table padding bottom");

        // 셀 속성 검증
        for cell in &tbl.cells {
            assert_eq!(
                cell.vertical_align,
                crate::model::table::VerticalAlign::Center,
                "Cell({},{}) v_align=Center",
                cell.row,
                cell.col
            );
            assert!(cell.raw_list_extra.len() >= 2, "raw_list_extra >= 2 bytes");
        }

        // table_para 속성 검증
        assert_eq!(table_para.char_count, 9, "table para char_count=9");
        assert_eq!(table_para.control_mask, 0x00000800, "control_mask=0x800");
        assert!(
            table_para.raw_header_extra.len() >= 10,
            "raw_header_extra >= 10"
        );
        let inst = u32::from_le_bytes([
            table_para.raw_header_extra[6],
            table_para.raw_header_extra[7],
            table_para.raw_header_extra[8],
            table_para.raw_header_extra[9],
        ]);
        assert_eq!(inst, 0x80000000, "table para instance_id=0x80000000");

        // DIFF-7: CTRL_HEADER instance_id (raw_ctrl_data[32..36]) 가 0이 아닌지 검증
        assert!(tbl.raw_ctrl_data.len() >= 36, "raw_ctrl_data >= 36 bytes");
        let common = parse_common_obj_attr(&tbl.raw_ctrl_data);
        assert_eq!(
            common.attr, tbl.attr,
            "HTML table raw_ctrl_data[0..4] must carry CommonObjAttr attr"
        );
        assert_eq!(
            (common.width, common.height),
            (
                tbl.get_column_widths().iter().sum(),
                tbl.get_row_heights().iter().sum()
            ),
            "HTML table raw_ctrl_data width/height offsets must match parser layout"
        );
        assert_eq!(
            (
                common.margin.left,
                common.margin.right,
                common.margin.top,
                common.margin.bottom
            ),
            (
                tbl.outer_margin_left,
                tbl.outer_margin_right,
                tbl.outer_margin_top,
                tbl.outer_margin_bottom
            ),
            "HTML table raw_ctrl_data margin offsets must match parser layout"
        );
        let ctrl_instance_id = common.instance_id;
        assert_ne!(
            ctrl_instance_id, 0,
            "DIFF-7: CTRL_HEADER instance_id != 0 (got 0x{:08X})",
            ctrl_instance_id
        );
    } else {
        panic!("첫 번째 컨트롤이 Table이어야 함");
    }
}



/// DIFF-1 검증: &nbsp; 만 있는 빈 셀이 char_count=1, has_para_text=false 인지 확인
#[test]
fn test_diff1_empty_cell_nbsp() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    document
        .doc_info
        .border_fills
        .push(crate::model::style::BorderFill::default());
    let mut para = Paragraph::default();
    para.text = "".to_string();
    para.char_count = 0;
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    document.sections.push(crate::model::document::Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.document = document;

    // &nbsp; 만 포함된 셀이 있는 2×2 표 (셀2, 셀4는 빈 셀)
    let html = r#"<table><tr><td>내용1</td><td>&nbsp;</td></tr><tr><td>내용2</td><td>&nbsp;&nbsp;&nbsp;</td></tr></table>"#;
    let mut paragraphs = Vec::new();
    doc.parse_table_html(&mut paragraphs, html);

    assert_eq!(paragraphs.len(), 1, "표 문단 1개");
    if let crate::model::control::Control::Table(ref tbl) = paragraphs[0].controls[0] {
        assert_eq!(tbl.cells.len(), 4, "4 셀");
        // 셀[0]: "내용1" → 텍스트 있음
        assert!(
            !tbl.cells[0].paragraphs[0].text.is_empty(),
            "셀[0] 텍스트 있음"
        );
        // 셀[1]: &nbsp; → 빈 셀
        let empty1 = &tbl.cells[1].paragraphs[0];
        assert_eq!(empty1.char_count, 1, "DIFF-1: &nbsp; 셀은 char_count=1");
        assert!(empty1.text.is_empty(), "DIFF-1: &nbsp; 셀은 text 비어있음");
        assert!(
            !empty1.has_para_text,
            "DIFF-1: &nbsp; 셀은 has_para_text=false"
        );
        // 셀[3]: &nbsp;&nbsp;&nbsp; → 빈 셀
        let empty2 = &tbl.cells[3].paragraphs[0];
        assert_eq!(
            empty2.char_count, 1,
            "DIFF-1: 다중 &nbsp; 셀은 char_count=1"
        );
        assert!(
            empty2.text.is_empty(),
            "DIFF-1: 다중 &nbsp; 셀은 text 비어있음"
        );
        assert!(
            !empty2.has_para_text,
            "DIFF-1: 다중 &nbsp; 셀은 has_para_text=false"
        );
    } else {
        panic!("Table 컨트롤이어야 함");
    }
}



#[test]
fn test_paste_html_table_with_colspan_rowspan() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    document
        .doc_info
        .border_fills
        .push(crate::model::style::BorderFill::default());
    let mut para = Paragraph::default();
    para.text = "".to_string();
    para.char_count = 0;
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // colspan=2, rowspan=2 포함 표
    let html = r#"<html><body><!--StartFragment-->
            <table>
                <tr><td colspan="2">병합열</td><td>C</td></tr>
                <tr><td rowspan="2">병합행</td><td>B2</td><td>C2</td></tr>
                <tr><td>B3</td><td>C3</td></tr>
            </table>
        <!--EndFragment--></body></html>"#;

    let result = doc.paste_html_native(0, 0, 0, html);
    assert!(result.is_ok());

    let paras = &doc.document.sections[0].paragraphs;
    let table_para = paras.iter().find(|p| !p.controls.is_empty());
    assert!(table_para.is_some(), "Table Control 문단이 있어야 함");

    if let Control::Table(ref tbl) = table_para.unwrap().controls[0] {
        assert_eq!(tbl.row_count, 3, "행 수 3");
        assert_eq!(tbl.col_count, 3, "열 수 3");

        // colspan=2인 셀 확인
        let merged_col = tbl.cells.iter().find(|c| c.col_span == 2);
        assert!(merged_col.is_some(), "colspan=2 셀이 있어야 함");
        assert_eq!(merged_col.unwrap().row, 0);

        // rowspan=2인 셀 확인
        let merged_row = tbl.cells.iter().find(|c| c.row_span == 2);
        assert!(merged_row.is_some(), "rowspan=2 셀이 있어야 함");
        assert_eq!(merged_row.unwrap().col, 0);
        assert_eq!(merged_row.unwrap().row, 1);
    } else {
        panic!("Table Control이어야 함");
    }
}



#[test]
fn test_paste_html_table_with_css_styles() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    document
        .doc_info
        .border_fills
        .push(crate::model::style::BorderFill::default());
    let mut para = Paragraph::default();
    para.text = "".to_string();
    para.char_count = 0;
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // CSS 스타일 포함 표
    let html = r#"<html><body><!--StartFragment-->
            <table style="border-collapse:collapse;">
                <tr>
                    <td style="width:38.50pt;height:21.31pt;border-top:solid #000000 0.28pt;border-bottom:solid #000000 0.28pt;border-left:solid #000000 0.28pt;border-right:solid #000000 0.28pt;padding:1.41pt 5.10pt;">데이터1</td>
                    <td style="width:50pt;height:21.31pt;background-color:#FFFF00;">데이터2</td>
                </tr>
            </table>
        <!--EndFragment--></body></html>"#;

    let result = doc.paste_html_native(0, 0, 0, html);
    assert!(result.is_ok());

    let paras = &doc.document.sections[0].paragraphs;
    let table_para = paras.iter().find(|p| !p.controls.is_empty());
    assert!(table_para.is_some());

    if let Control::Table(ref tbl) = table_para.unwrap().controls[0] {
        assert_eq!(tbl.row_count, 1);
        assert_eq!(tbl.col_count, 2);
        assert_eq!(tbl.cells.len(), 2);

        // 첫 번째 셀: width=38.50pt → 3850 HWPUNIT
        let cell0 = &tbl.cells[0];
        assert!(
            cell0.width > 3800 && cell0.width < 3900,
            "셀 폭 ~3850, 실제: {}",
            cell0.width
        );

        // 두 번째 셀: background-color → BorderFill에 등록
        let cell1 = &tbl.cells[1];
        assert!(cell1.border_fill_id > 0, "border_fill_id가 설정되어야 함");

        // 패딩 확인 (1.41pt ≈ 141, 5.10pt ≈ 510)
        assert!(
            cell0.padding.top > 130 && cell0.padding.top < 150,
            "상단 패딩 ~141, 실제: {}",
            cell0.padding.top
        );
        assert!(
            cell0.padding.left > 500 && cell0.padding.left < 520,
            "좌측 패딩 ~510, 실제: {}",
            cell0.padding.left
        );
    } else {
        panic!("Table Control이어야 함");
    }
}



#[test]
fn test_paste_html_table_with_th_header() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    document
        .doc_info
        .border_fills
        .push(crate::model::style::BorderFill::default());
    let mut para = Paragraph::default();
    para.text = "".to_string();
    para.char_count = 0;
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // <th> 헤더 포함 표
    let html = r#"<html><body><!--StartFragment-->
            <table>
                <tr><th>이름</th><th>나이</th></tr>
                <tr><td>홍길동</td><td>30</td></tr>
            </table>
        <!--EndFragment--></body></html>"#;

    let result = doc.paste_html_native(0, 0, 0, html);
    assert!(result.is_ok());

    let paras = &doc.document.sections[0].paragraphs;
    let table_para = paras.iter().find(|p| !p.controls.is_empty());
    assert!(table_para.is_some());

    if let Control::Table(ref tbl) = table_para.unwrap().controls[0] {
        assert_eq!(tbl.row_count, 2);
        assert_eq!(tbl.col_count, 2);
        assert!(tbl.repeat_header, "헤더 반복 활성화");

        // 첫 행 셀이 is_header=true
        let header_cells: Vec<_> = tbl.cells.iter().filter(|c| c.is_header).collect();
        assert_eq!(header_cells.len(), 2, "헤더 셀 2개");
    } else {
        panic!("Table Control이어야 함");
    }
}



#[test]
fn test_table_utility_functions() {
    // parse_css_dimension_pt
    assert!((super::parse_css_dimension_pt("width:38.50pt", "width") - 38.5).abs() < 0.01);
    assert!((super::parse_css_dimension_pt("width:100px", "width") - 75.0).abs() < 0.01);
    assert!((super::parse_css_dimension_pt("height:1cm", "height") - 28.3465).abs() < 0.1);
    assert_eq!(super::parse_css_dimension_pt("width:auto", "width"), 0.0);

    // parse_css_padding_pt
    let p = super::parse_css_padding_pt("padding:1.41pt 5.10pt");
    assert!((p[0] - 5.10).abs() < 0.01, "left = 5.10"); // left
    assert!((p[1] - 5.10).abs() < 0.01, "right = 5.10"); // right
    assert!((p[2] - 1.41).abs() < 0.01, "top = 1.41"); // top
    assert!((p[3] - 1.41).abs() < 0.01, "bottom = 1.41"); // bottom

    // parse_css_border_shorthand
    let (w, c, s) = super::parse_css_border_shorthand("solid #000000 0.28pt");
    assert!((w - 0.28).abs() < 0.01, "border width 0.28pt");
    assert_eq!(c, 0x000000, "border color black");
    assert_eq!(s, 1, "border style solid");

    let (w2, _, s2) = super::parse_css_border_shorthand("none");
    assert_eq!(w2, 0.0);
    assert_eq!(s2, 0);

    // rgb() 내부에 공백이 있어도 색상 토큰이 쪼개지지 않아야 한다.
    let (w3, c3, s3) = super::parse_css_border_shorthand("1px solid rgb(255, 0, 0)");
    assert!((w3 - 0.75).abs() < 0.01, "border width 1px -> 0.75pt");
    assert_eq!(c3, 0x0000FF, "border color red (BGR)");
    assert_eq!(s3, 1, "border style solid");

    // css_border_width_to_hwp
    assert_eq!(super::css_border_width_to_hwp(0.28), 0); // 0.28pt ≈ 0.1mm → index 0
    assert!(super::css_border_width_to_hwp(1.0) >= 5); // 1.0pt ≈ 0.35mm → index 5+

    // parse_html_attr_u16
    assert_eq!(
        super::parse_html_attr_u16(r#"<td colspan="3">"#, "colspan"),
        Some(3)
    );
    assert_eq!(super::parse_html_attr_u16(r#"<td>"#, "colspan"), None);
}



#[test]
fn test_pasted_table_structure_analysis() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;
    use std::path::Path;

    let orig_path = Path::new("pasts/20250130-hongbo-p2.hwp");
    let saved_path = Path::new("pasts/20250130-hongbo_saved-rp-003.hwp");
    if !orig_path.exists() || !saved_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();

    // Parse file headers to get compression flags
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== PASTED TABLE STRUCTURE ANALYSIS ===");
    eprintln!(
        "Original: {} ({} bytes)",
        orig_path.display(),
        orig_data.len()
    );
    eprintln!(
        "Saved:    {} ({} bytes)",
        saved_path.display(),
        saved_data.len()
    );
    eprintln!("{}", "=".repeat(120));

    // Read raw Section0 bytes
    let mut orig_cfb = CfbReader::open(&orig_data).unwrap();
    let mut saved_cfb = CfbReader::open(&saved_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();

    let orig_recs = Record::read_all(&orig_bt).unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    eprintln!("\nOriginal records: {}", orig_recs.len());
    eprintln!("Saved records:    {}", saved_recs.len());

    // Helper: hex dump with printable ASCII
    fn hex_dump(data: &[u8], max_bytes: usize) -> String {
        let show = data.len().min(max_bytes);
        let mut s = String::new();
        for (i, chunk) in data[..show].chunks(16).enumerate() {
            s.push_str(&format!("    {:04X}: ", i * 16));
            for b in chunk {
                s.push_str(&format!("{:02X} ", b));
            }
            // Pad for alignment
            for _ in 0..(16 - chunk.len()) {
                s.push_str("   ");
            }
            s.push_str(" |");
            for b in chunk {
                if *b >= 0x20 && *b < 0x7F {
                    s.push(*b as char);
                } else {
                    s.push('.');
                }
            }
            s.push_str("|\n");
        }
        if data.len() > max_bytes {
            s.push_str(&format!(
                "    ... ({} more bytes)\n",
                data.len() - max_bytes
            ));
        }
        s
    }

    // Helper: read ctrl_id from CTRL_HEADER record data
    fn get_ctrl_id(data: &[u8]) -> u32 {
        if data.len() >= 4 {
            u32::from_le_bytes([data[0], data[1], data[2], data[3]])
        } else {
            0
        }
    }

    // Helper: check if ctrl_id is table ("tbl " = 0x74626C20 big-endian)
    // In file: DWORD LE → bytes [0x20, 0x6C, 0x62, 0x74]
    // u32::from_le_bytes gives 0x74626C20
    fn is_table_ctrl(data: &[u8]) -> bool {
        if data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            ctrl_id == tags::CTRL_TABLE
        } else {
            false
        }
    }

    // Struct to hold a table's record cluster
    struct TableCluster {
        ctrl_header_idx: usize,
        ctrl_header: Record,
        table_rec: Option<(usize, Record)>,
        list_headers: Vec<(usize, Record)>,
        para_headers: Vec<(usize, Record)>,
        // All records in this table's scope
        all_records: Vec<(usize, Record)>,
    }

    // Find table clusters in a record list
    fn find_table_clusters(recs: &[Record]) -> Vec<TableCluster> {
        let mut clusters = Vec::new();
        let mut i = 0;
        while i < recs.len() {
            if recs[i].tag_id == tags::HWPTAG_CTRL_HEADER && is_table_ctrl(&recs[i].data) {
                let ctrl_level = recs[i].level;
                let mut cluster = TableCluster {
                    ctrl_header_idx: i,
                    ctrl_header: recs[i].clone(),
                    table_rec: None,
                    list_headers: Vec::new(),
                    para_headers: Vec::new(),
                    all_records: vec![(i, recs[i].clone())],
                };
                // Collect all child records (level > ctrl_level)
                let mut j = i + 1;
                while j < recs.len() && recs[j].level > ctrl_level {
                    cluster.all_records.push((j, recs[j].clone()));
                    if recs[j].tag_id == tags::HWPTAG_TABLE && cluster.table_rec.is_none() {
                        cluster.table_rec = Some((j, recs[j].clone()));
                    }
                    if recs[j].tag_id == tags::HWPTAG_LIST_HEADER {
                        cluster.list_headers.push((j, recs[j].clone()));
                    }
                    if recs[j].tag_id == tags::HWPTAG_PARA_HEADER {
                        cluster.para_headers.push((j, recs[j].clone()));
                    }
                    j += 1;
                }
                clusters.push(cluster);
                i = j;
            } else {
                i += 1;
            }
        }
        clusters
    }

    // Debug: list all CTRL_HEADER records and their ctrl_ids
    eprintln!("\n--- All CTRL_HEADER records in Original ---");
    for (i, r) in orig_recs.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes(r.data[0..4].try_into().unwrap());
            let ctrl_bytes = &r.data[0..4];
            let is_tbl = ctrl_id == tags::CTRL_TABLE;
            eprintln!("  [{}] CTRL_HEADER L{} {}B ctrl_id=0x{:08X} bytes=[{:02X} {:02X} {:02X} {:02X}] name={} is_table={}",
                    i, r.level, r.data.len(), ctrl_id,
                    ctrl_bytes[0], ctrl_bytes[1], ctrl_bytes[2], ctrl_bytes[3],
                    tags::ctrl_name(ctrl_id), is_tbl);
        }
    }
    eprintln!("\n--- All CTRL_HEADER records in Saved ---");
    for (i, r) in saved_recs.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes(r.data[0..4].try_into().unwrap());
            let ctrl_bytes = &r.data[0..4];
            let is_tbl = ctrl_id == tags::CTRL_TABLE;
            eprintln!("  [{}] CTRL_HEADER L{} {}B ctrl_id=0x{:08X} bytes=[{:02X} {:02X} {:02X} {:02X}] name={} is_table={}",
                    i, r.level, r.data.len(), ctrl_id,
                    ctrl_bytes[0], ctrl_bytes[1], ctrl_bytes[2], ctrl_bytes[3],
                    tags::ctrl_name(ctrl_id), is_tbl);
        }
    }

    let orig_tables = find_table_clusters(&orig_recs);
    let saved_tables = find_table_clusters(&saved_recs);

    eprintln!("\n--- Table Count ---");
    eprintln!("Original tables: {}", orig_tables.len());
    eprintln!("Saved tables:    {}", saved_tables.len());

    // Print summary of each table
    fn print_table_summary(label: &str, tables: &[TableCluster]) {
        eprintln!("\n--- {} Table Summary ---", label);
        for (ti, t) in tables.iter().enumerate() {
            let ctrl_id = get_ctrl_id(&t.ctrl_header.data);
            let ctrl_id_bytes = ctrl_id.to_le_bytes();
            let ctrl_str: String = ctrl_id_bytes
                .iter()
                .rev()
                .map(|b| {
                    if *b >= 0x20 && *b < 0x7F {
                        *b as char
                    } else {
                        '?'
                    }
                })
                .collect();
            eprintln!("  Table[{}] at rec[{}]: ctrl_id=0x{:08X} '{}' level={} total_children={} cells(LIST_HEADER)={} paras(PARA_HEADER)={}",
                    ti, t.ctrl_header_idx, ctrl_id, ctrl_str,
                    t.ctrl_header.level,
                    t.all_records.len() - 1,
                    t.list_headers.len(),
                    t.para_headers.len()
                );
            // Table record info
            if let Some((idx, ref tr)) = t.table_rec {
                eprintln!(
                    "    TABLE record at [{}]: size={} bytes",
                    idx,
                    tr.data.len()
                );
                if tr.data.len() >= 8 {
                    let flags = u32::from_le_bytes(tr.data[0..4].try_into().unwrap());
                    let nrows = u16::from_le_bytes(tr.data[4..6].try_into().unwrap());
                    let ncols = u16::from_le_bytes(tr.data[6..8].try_into().unwrap());
                    eprintln!(
                        "    TABLE: flags=0x{:08X} nrows={} ncols={} (expected cells={})",
                        flags,
                        nrows,
                        ncols,
                        nrows as u32 * ncols as u32
                    );
                }
            } else {
                eprintln!("    TABLE record: MISSING!");
            }
        }
    }

    print_table_summary("Original", &orig_tables);
    print_table_summary("Saved", &saved_tables);

    // Detailed analysis of each table
    fn dump_table_detail(label: &str, t: &TableCluster) {
        eprintln!(
            "\n  === {} Table at rec[{}] DETAILED ===",
            label, t.ctrl_header_idx
        );

        // 1. CTRL_HEADER full dump
        eprintln!(
            "\n  [CTRL_HEADER] rec[{}] level={} size={} bytes:",
            t.ctrl_header_idx,
            t.ctrl_header.level,
            t.ctrl_header.data.len()
        );
        eprintln!("{}", hex_dump(&t.ctrl_header.data, 256));

        // Parse CTRL_HEADER fields
        if t.ctrl_header.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes(t.ctrl_header.data[0..4].try_into().unwrap());
            eprintln!("    ctrl_id = 0x{:08X}", ctrl_id);
        }
        if t.ctrl_header.data.len() >= 8 {
            let obj_attr = u32::from_le_bytes(t.ctrl_header.data[4..8].try_into().unwrap());
            eprintln!("    obj_attr = 0x{:08X}", obj_attr);
            let vert_offset = obj_attr & 0x3;
            let horiz_offset = (obj_attr >> 2) & 0x3;
            let vert_rel = (obj_attr >> 4) & 0x3;
            let horiz_rel = (obj_attr >> 7) & 0x3;
            let flow_with_text = (obj_attr >> 10) & 0x1;
            let allow_overlap = (obj_attr >> 11) & 0x1;
            let wid_criterion = (obj_attr >> 12) & 0x7;
            let hgt_criterion = (obj_attr >> 15) & 0x3;
            let protect_size = (obj_attr >> 17) & 0x1;
            let text_flow = (obj_attr >> 21) & 0x7;
            let text_arrange = (obj_attr >> 24) & 0x3;
            eprintln!(
                "      vert_offset={} horiz_offset={} vert_rel={} horiz_rel={}",
                vert_offset, horiz_offset, vert_rel, horiz_rel
            );
            eprintln!(
                "      flow_with_text={} allow_overlap={} wid_criterion={} hgt_criterion={}",
                flow_with_text, allow_overlap, wid_criterion, hgt_criterion
            );
            eprintln!(
                "      protect_size={} text_flow={} text_arrange={}",
                protect_size, text_flow, text_arrange
            );
        }
        if t.ctrl_header.data.len() >= 12 {
            let vert_pos = u32::from_le_bytes(t.ctrl_header.data[8..12].try_into().unwrap());
            eprintln!("    vert_offset_value = {} hwpunit", vert_pos);
        }
        if t.ctrl_header.data.len() >= 16 {
            let horiz_pos = u32::from_le_bytes(t.ctrl_header.data[12..16].try_into().unwrap());
            eprintln!("    horiz_offset_value = {} hwpunit", horiz_pos);
        }
        if t.ctrl_header.data.len() >= 20 {
            let width = u32::from_le_bytes(t.ctrl_header.data[16..20].try_into().unwrap());
            eprintln!(
                "    width = {} hwpunit ({:.2} mm)",
                width,
                width as f64 / 7200.0 * 25.4
            );
        }
        if t.ctrl_header.data.len() >= 24 {
            let height = u32::from_le_bytes(t.ctrl_header.data[20..24].try_into().unwrap());
            eprintln!(
                "    height = {} hwpunit ({:.2} mm)",
                height,
                height as f64 / 7200.0 * 25.4
            );
        }
        if t.ctrl_header.data.len() >= 28 {
            let zorder = i32::from_le_bytes(t.ctrl_header.data[24..28].try_into().unwrap());
            eprintln!("    z_order = {}", zorder);
        }

        // 2. TABLE record full dump
        if let Some((idx, ref tr)) = t.table_rec {
            eprintln!(
                "\n  [TABLE] rec[{}] level={} size={} bytes:",
                idx,
                tr.level,
                tr.data.len()
            );
            eprintln!("{}", hex_dump(&tr.data, 512));

            // Parse TABLE fields
            if tr.data.len() >= 8 {
                let flags = u32::from_le_bytes(tr.data[0..4].try_into().unwrap());
                let nrows = u16::from_le_bytes(tr.data[4..6].try_into().unwrap());
                let ncols = u16::from_le_bytes(tr.data[6..8].try_into().unwrap());
                eprintln!("    flags=0x{:08X} nrows={} ncols={}", flags, nrows, ncols);

                // Cell spacing (4 bytes)
                if tr.data.len() >= 12 {
                    let cell_spacing = u32::from_le_bytes(tr.data[8..12].try_into().unwrap());
                    eprintln!("    cell_spacing={}", cell_spacing);
                }

                // Margins: left, right, top, bottom (each 2 bytes)
                if tr.data.len() >= 20 {
                    let ml = u16::from_le_bytes(tr.data[12..14].try_into().unwrap());
                    let mr = u16::from_le_bytes(tr.data[14..16].try_into().unwrap());
                    let mt = u16::from_le_bytes(tr.data[16..18].try_into().unwrap());
                    let mb = u16::from_le_bytes(tr.data[18..20].try_into().unwrap());
                    eprintln!(
                        "    margins: left={} right={} top={} bottom={}",
                        ml, mr, mt, mb
                    );
                }

                // Row sizes (nrows * 2 bytes starting at offset 20)
                let row_sizes_offset = 20;
                let row_sizes_end = row_sizes_offset + (nrows as usize * 2);
                if tr.data.len() >= row_sizes_end {
                    let mut row_sizes = Vec::new();
                    for r in 0..nrows as usize {
                        let off = row_sizes_offset + r * 2;
                        let rs = u16::from_le_bytes(tr.data[off..off + 2].try_into().unwrap());
                        row_sizes.push(rs);
                    }
                    eprintln!("    row_sizes: {:?}", row_sizes);
                }

                // Border fill ID after row sizes
                let bf_offset = row_sizes_end;
                if tr.data.len() >= bf_offset + 2 {
                    let bf_id =
                        u16::from_le_bytes(tr.data[bf_offset..bf_offset + 2].try_into().unwrap());
                    eprintln!("    border_fill_id={}", bf_id);
                }

                // Remaining bytes after all parsed fields
                let parsed_end = bf_offset + 2;
                if tr.data.len() > parsed_end {
                    eprintln!(
                        "    remaining {} bytes after parsed fields:",
                        tr.data.len() - parsed_end
                    );
                    eprintln!("{}", hex_dump(&tr.data[parsed_end..], 128));
                }
            }
        }

        // 3. LIST_HEADER records (cells) - first 5
        let cell_count = t.list_headers.len().min(5);
        eprintln!(
            "\n  [LIST_HEADER / Cells] total={}, showing first {}:",
            t.list_headers.len(),
            cell_count
        );
        for ci in 0..cell_count {
            let (idx, ref lh) = t.list_headers[ci];
            eprintln!(
                "\n    Cell[{}] LIST_HEADER rec[{}] level={} size={} bytes:",
                ci,
                idx,
                lh.level,
                lh.data.len()
            );
            eprintln!("{}", hex_dump(&lh.data, 256));

            // Parse LIST_HEADER cell fields
            if lh.data.len() >= 2 {
                let num_paras = u16::from_le_bytes(lh.data[0..2].try_into().unwrap());
                eprintln!("      num_paras = {}", num_paras);
            }
            if lh.data.len() >= 6 {
                let prop = u32::from_le_bytes(lh.data[2..6].try_into().unwrap());
                eprintln!("      property = 0x{:08X}", prop);
            }
            // Cell-specific fields (after list header base)
            // The cell list header typically has: nParagraphs(2), property(4),
            // then cell-specific: col_addr(2), row_addr(2), col_span(2), row_span(2),
            // width(4), height(4), margins(4*2=8), border_fill_id(2)
            if lh.data.len() >= 8 {
                let col_addr = u16::from_le_bytes(lh.data[6..8].try_into().unwrap());
                eprintln!("      col_addr = {}", col_addr);
            }
            if lh.data.len() >= 10 {
                let row_addr = u16::from_le_bytes(lh.data[8..10].try_into().unwrap());
                eprintln!("      row_addr = {}", row_addr);
            }
            if lh.data.len() >= 12 {
                let col_span = u16::from_le_bytes(lh.data[10..12].try_into().unwrap());
                eprintln!("      col_span = {}", col_span);
            }
            if lh.data.len() >= 14 {
                let row_span = u16::from_le_bytes(lh.data[12..14].try_into().unwrap());
                eprintln!("      row_span = {}", row_span);
            }
            if lh.data.len() >= 18 {
                let width = u32::from_le_bytes(lh.data[14..18].try_into().unwrap());
                eprintln!(
                    "      width = {} hwpunit ({:.2} mm)",
                    width,
                    width as f64 / 7200.0 * 25.4
                );
            }
            if lh.data.len() >= 22 {
                let height = u32::from_le_bytes(lh.data[18..22].try_into().unwrap());
                eprintln!(
                    "      height = {} hwpunit ({:.2} mm)",
                    height,
                    height as f64 / 7200.0 * 25.4
                );
            }
            // Margins
            if lh.data.len() >= 30 {
                let ml = u16::from_le_bytes(lh.data[22..24].try_into().unwrap());
                let mr = u16::from_le_bytes(lh.data[24..26].try_into().unwrap());
                let mt = u16::from_le_bytes(lh.data[26..28].try_into().unwrap());
                let mb = u16::from_le_bytes(lh.data[28..30].try_into().unwrap());
                eprintln!(
                    "      margins: left={} right={} top={} bottom={}",
                    ml, mr, mt, mb
                );
            }
            if lh.data.len() >= 32 {
                let bf_id = u16::from_le_bytes(lh.data[30..32].try_into().unwrap());
                eprintln!("      border_fill_id = {}", bf_id);
            }
            if lh.data.len() > 32 {
                eprintln!("      remaining {} bytes:", lh.data.len() - 32);
                eprintln!("{}", hex_dump(&lh.data[32..], 64));
            }
        }

        // 4. PARA_HEADER records - first 5
        let para_count = t.para_headers.len().min(5);
        eprintln!(
            "\n  [PARA_HEADER] total={}, showing first {}:",
            t.para_headers.len(),
            para_count
        );
        for pi in 0..para_count {
            let (idx, ref ph) = t.para_headers[pi];
            eprintln!(
                "\n    Para[{}] PARA_HEADER rec[{}] level={} size={} bytes:",
                pi,
                idx,
                ph.level,
                ph.data.len()
            );
            eprintln!("{}", hex_dump(&ph.data, 128));

            // Parse PARA_HEADER fields
            if ph.data.len() >= 4 {
                let nchars = u32::from_le_bytes(ph.data[0..4].try_into().unwrap());
                let n_char_shapes = if ph.data.len() >= 6 {
                    u16::from_le_bytes(ph.data[4..6].try_into().unwrap())
                } else {
                    0
                };
                let n_line_segs = if ph.data.len() >= 8 {
                    u16::from_le_bytes(ph.data[6..8].try_into().unwrap())
                } else {
                    0
                };
                let n_range_tags = if ph.data.len() >= 10 {
                    u16::from_le_bytes(ph.data[8..10].try_into().unwrap())
                } else {
                    0
                };
                let n_controls = if ph.data.len() >= 12 {
                    u16::from_le_bytes(ph.data[10..12].try_into().unwrap())
                } else {
                    0
                };
                let para_shape_id = if ph.data.len() >= 14 {
                    u16::from_le_bytes(ph.data[12..14].try_into().unwrap())
                } else {
                    0
                };
                let style_id = if ph.data.len() >= 15 { ph.data[14] } else { 0 };
                eprintln!(
                    "      nchars={} n_char_shapes={} n_line_segs={} n_range_tags={} n_controls={}",
                    nchars, n_char_shapes, n_line_segs, n_range_tags, n_controls
                );
                eprintln!(
                    "      para_shape_id={} style_id={}",
                    para_shape_id, style_id
                );
            }
        }

        // 5. Full record type breakdown
        let mut tag_counts: std::collections::BTreeMap<u16, usize> =
            std::collections::BTreeMap::new();
        for (_, rec) in &t.all_records {
            *tag_counts.entry(rec.tag_id).or_insert(0) += 1;
        }
        eprintln!("\n  Record type breakdown:");
        for (tag, count) in &tag_counts {
            eprintln!(
                "    {} (tag={}): {} records",
                tags::tag_name(*tag),
                tag,
                count
            );
        }
    }

    // Dump all original tables
    for (ti, t) in orig_tables.iter().enumerate() {
        eprintln!("\n{}", "=".repeat(100));
        eprintln!("ORIGINAL Table[{}]", ti);
        dump_table_detail("ORIG", t);
    }

    // Dump all saved tables
    for (ti, t) in saved_tables.iter().enumerate() {
        eprintln!("\n{}", "=".repeat(100));
        eprintln!("SAVED Table[{}]", ti);
        dump_table_detail("SAVED", t);
    }

    // Special comparison: last saved table (pasted) vs first original table
    if !saved_tables.is_empty() && !orig_tables.is_empty() {
        let pasted = saved_tables.last().unwrap();
        let orig_first = &orig_tables[0];

        eprintln!("\n{}", "=".repeat(120));
        eprintln!("=== COMPARISON: PASTED TABLE (last saved) vs FIRST ORIGINAL TABLE ===");
        eprintln!("{}", "=".repeat(120));

        // Compare CTRL_HEADER
        eprintln!("\n--- CTRL_HEADER comparison ---");
        eprintln!("ORIG size: {} bytes", orig_first.ctrl_header.data.len());
        eprintln!("PASTED size: {} bytes", pasted.ctrl_header.data.len());
        if orig_first.ctrl_header.data == pasted.ctrl_header.data {
            eprintln!("CTRL_HEADER: IDENTICAL");
        } else {
            let min_len = orig_first
                .ctrl_header
                .data
                .len()
                .min(pasted.ctrl_header.data.len());
            let mut diffs = Vec::new();
            for i in 0..min_len {
                if orig_first.ctrl_header.data[i] != pasted.ctrl_header.data[i] {
                    diffs.push((
                        i,
                        orig_first.ctrl_header.data[i],
                        pasted.ctrl_header.data[i],
                    ));
                }
            }
            eprintln!("CTRL_HEADER byte diffs ({}):", diffs.len());
            for (off, a, b) in &diffs {
                eprintln!("  offset {}: orig=0x{:02X} pasted=0x{:02X}", off, a, b);
            }
            if orig_first.ctrl_header.data.len() != pasted.ctrl_header.data.len() {
                eprintln!(
                    "  SIZE DIFFERENCE: orig={} pasted={}",
                    orig_first.ctrl_header.data.len(),
                    pasted.ctrl_header.data.len()
                );
            }
        }

        // Compare TABLE record
        eprintln!("\n--- TABLE record comparison ---");
        match (&orig_first.table_rec, &pasted.table_rec) {
            (Some((_, ref ot)), Some((_, ref pt))) => {
                eprintln!("ORIG TABLE size: {} bytes", ot.data.len());
                eprintln!("PASTED TABLE size: {} bytes", pt.data.len());
                if ot.data == pt.data {
                    eprintln!("TABLE: IDENTICAL");
                } else {
                    let min_len = ot.data.len().min(pt.data.len());
                    let mut diffs = Vec::new();
                    for i in 0..min_len {
                        if ot.data[i] != pt.data[i] {
                            diffs.push((i, ot.data[i], pt.data[i]));
                        }
                    }
                    eprintln!("TABLE byte diffs ({}):", diffs.len());
                    for (off, a, b) in &diffs {
                        eprintln!("  offset {}: orig=0x{:02X} pasted=0x{:02X}", off, a, b);
                    }
                    if ot.data.len() != pt.data.len() {
                        eprintln!(
                            "  SIZE DIFFERENCE: orig={} pasted={}",
                            ot.data.len(),
                            pt.data.len()
                        );
                    }
                }
            }
            _ => {
                eprintln!("One or both TABLE records MISSING!");
            }
        }

        // Compare LIST_HEADER records (cells)
        eprintln!("\n--- Cell LIST_HEADER comparison ---");
        eprintln!("ORIG cells: {}", orig_first.list_headers.len());
        eprintln!("PASTED cells: {}", pasted.list_headers.len());
        let compare_count = orig_first
            .list_headers
            .len()
            .min(pasted.list_headers.len())
            .min(10);
        for ci in 0..compare_count {
            let (_, ref olh) = orig_first.list_headers[ci];
            let (_, ref plh) = pasted.list_headers[ci];
            if olh.data == plh.data {
                eprintln!("  Cell[{}]: IDENTICAL ({} bytes)", ci, olh.data.len());
            } else {
                let min_len = olh.data.len().min(plh.data.len());
                let mut diffs = Vec::new();
                for i in 0..min_len {
                    if olh.data[i] != plh.data[i] {
                        diffs.push((i, olh.data[i], plh.data[i]));
                    }
                }
                eprintln!(
                    "  Cell[{}]: {} byte diffs, orig_size={} pasted_size={}",
                    ci,
                    diffs.len(),
                    olh.data.len(),
                    plh.data.len()
                );
                for (off, a, b) in diffs.iter().take(5) {
                    eprintln!("    offset {}: orig=0x{:02X} pasted=0x{:02X}", off, a, b);
                }
            }
        }

        // Compare record sequences
        eprintln!("\n--- Record sequence comparison ---");
        eprintln!("ORIG records in table: {}", orig_first.all_records.len());
        eprintln!("PASTED records in table: {}", pasted.all_records.len());
        let seq_len = orig_first.all_records.len().min(pasted.all_records.len());
        let mut first_mismatch = None;
        for i in 0..seq_len {
            let (_, ref orec) = orig_first.all_records[i];
            let (_, ref prec) = pasted.all_records[i];
            if orec.tag_id != prec.tag_id || orec.level != prec.level {
                if first_mismatch.is_none() {
                    first_mismatch = Some(i);
                }
                eprintln!(
                    "  [{}] MISMATCH: orig={}(tag={},L{},{}B) vs pasted={}(tag={},L{},{}B)",
                    i,
                    tags::tag_name(orec.tag_id),
                    orec.tag_id,
                    orec.level,
                    orec.data.len(),
                    tags::tag_name(prec.tag_id),
                    prec.tag_id,
                    prec.level,
                    prec.data.len()
                );
            }
        }
        if first_mismatch.is_none() && orig_first.all_records.len() == pasted.all_records.len() {
            eprintln!("  Record sequences: IDENTICAL structure");
        }
    }

    // Check level integrity of pasted table
    if !saved_tables.is_empty() {
        let pasted = saved_tables.last().unwrap();
        eprintln!("\n--- Level Integrity Check (Pasted Table) ---");
        let mut prev_level: i32 = -1;
        let mut issues = 0;
        for (idx, rec) in &pasted.all_records {
            let curr_level = rec.level as i32;
            if prev_level >= 0 && curr_level > prev_level + 1 {
                issues += 1;
                if issues <= 10 {
                    eprintln!(
                        "  LEVEL JUMP at [{}]: prev={} curr={} tag={} size={}",
                        idx,
                        prev_level,
                        curr_level,
                        tags::tag_name(rec.tag_id),
                        rec.data.len()
                    );
                }
            }
            prev_level = curr_level;
        }
        eprintln!("  Total level issues: {}", issues);
    }

    // Context: dump records around the pasted table
    if !saved_tables.is_empty() {
        let pasted = saved_tables.last().unwrap();
        let start_idx = if pasted.ctrl_header_idx > 5 {
            pasted.ctrl_header_idx - 5
        } else {
            0
        };
        let end_idx = (pasted.ctrl_header_idx + pasted.all_records.len() + 5).min(saved_recs.len());

        eprintln!(
            "\n--- Context: Records around pasted table (rec[{}..{}]) ---",
            start_idx, end_idx
        );
        for i in start_idx..end_idx {
            let r = &saved_recs[i];
            let marker = if i == pasted.ctrl_header_idx {
                " <<< PASTED TABLE START"
            } else if i == pasted.ctrl_header_idx + pasted.all_records.len() - 1 {
                " <<< PASTED TABLE END"
            } else {
                ""
            };
            if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
                let ctrl_id = u32::from_le_bytes(r.data[0..4].try_into().unwrap());
                eprintln!(
                    "  [{}] {} L{} {}B ctrl=0x{:08X} ({}){}",
                    i,
                    tags::tag_name(r.tag_id),
                    r.level,
                    r.data.len(),
                    ctrl_id,
                    tags::ctrl_name(ctrl_id),
                    marker
                );
            } else {
                eprintln!(
                    "  [{}] {} L{} {}B{}",
                    i,
                    tags::tag_name(r.tag_id),
                    r.level,
                    r.data.len(),
                    marker
                );
            }
        }
    }

    // Check all records after the last table in saved to see if there's corruption
    if !saved_tables.is_empty() {
        let pasted = saved_tables.last().unwrap();
        let after_idx = pasted.ctrl_header_idx + pasted.all_records.len();
        if after_idx < saved_recs.len() {
            eprintln!(
                "\n--- Records AFTER pasted table ({} remaining) ---",
                saved_recs.len() - after_idx
            );
            for i in after_idx..(after_idx + 20).min(saved_recs.len()) {
                let r = &saved_recs[i];
                if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
                    let ctrl_id = u32::from_le_bytes(r.data[0..4].try_into().unwrap());
                    eprintln!(
                        "  [{}] {} L{} {}B ctrl=0x{:08X} ({})",
                        i,
                        tags::tag_name(r.tag_id),
                        r.level,
                        r.data.len(),
                        ctrl_id,
                        tags::ctrl_name(ctrl_id)
                    );
                } else {
                    eprintln!(
                        "  [{}] {} L{} {}B first16: {:02X?}",
                        i,
                        tags::tag_name(r.tag_id),
                        r.level,
                        r.data.len(),
                        &r.data[..r.data.len().min(16)]
                    );
                }
            }
        } else {
            eprintln!("\n--- No records after pasted table (table is last content) ---");
        }
    }

    // Overall record comparison: saved vs original
    eprintln!("\n--- Overall Record Count by Type ---");
    let mut orig_counts: std::collections::BTreeMap<u16, usize> = std::collections::BTreeMap::new();
    let mut saved_counts: std::collections::BTreeMap<u16, usize> =
        std::collections::BTreeMap::new();
    for r in &orig_recs {
        *orig_counts.entry(r.tag_id).or_insert(0) += 1;
    }
    for r in &saved_recs {
        *saved_counts.entry(r.tag_id).or_insert(0) += 1;
    }
    let all_tags: std::collections::BTreeSet<u16> = orig_counts
        .keys()
        .chain(saved_counts.keys())
        .copied()
        .collect();
    eprintln!("{:<25} {:>6} {:>6} {:>6}", "Tag", "Orig", "Saved", "Diff");
    for tag in &all_tags {
        let oc = orig_counts.get(tag).copied().unwrap_or(0);
        let sc = saved_counts.get(tag).copied().unwrap_or(0);
        let diff = sc as i64 - oc as i64;
        if diff != 0 {
            eprintln!(
                "{:<25} {:>6} {:>6} {:>+6}",
                tags::tag_name(*tag),
                oc,
                sc,
                diff
            );
        }
    }

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== PASTED TABLE STRUCTURE ANALYSIS COMPLETE ===");
    eprintln!("{}", "=".repeat(120));
}



#[test]
fn test_table3_deep_comparison() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;
    use std::path::Path;

    let orig_path = Path::new("pasts/20250130-hongbo-p2.hwp");
    let saved_path = Path::new("pasts/20250130-hongbo_saved-rp-003.hwp");
    if !orig_path.exists() || !saved_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();

    let mut orig_cfb = CfbReader::open(&orig_data).unwrap();
    let mut saved_cfb = CfbReader::open(&saved_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();

    let orig_recs = Record::read_all(&orig_bt).unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== TABLE[3] DEEP COMPARISON (21x7 table) ===");
    eprintln!("{}", "=".repeat(120));

    fn is_table_ctrl(data: &[u8]) -> bool {
        if data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            ctrl_id == tags::CTRL_TABLE
        } else {
            false
        }
    }

    // Find the 4th table (index 3) in each file
    fn find_nth_table(recs: &[Record], n: usize) -> Option<(usize, usize)> {
        let mut table_count = 0;
        let mut i = 0;
        while i < recs.len() {
            if recs[i].tag_id == tags::HWPTAG_CTRL_HEADER && is_table_ctrl(&recs[i].data) {
                if table_count == n {
                    let ctrl_level = recs[i].level;
                    let start = i;
                    let mut j = i + 1;
                    while j < recs.len() && recs[j].level > ctrl_level {
                        j += 1;
                    }
                    return Some((start, j));
                }
                table_count += 1;
            }
            i += 1;
        }
        None
    }

    let (orig_start, orig_end) =
        find_nth_table(&orig_recs, 3).expect("Original Table[3] not found");
    let (saved_start, saved_end) =
        find_nth_table(&saved_recs, 3).expect("Saved Table[3] not found");

    let orig_table_recs = &orig_recs[orig_start..orig_end];
    let saved_table_recs = &saved_recs[saved_start..saved_end];

    eprintln!(
        "Original Table[3]: recs[{}..{}] ({} records)",
        orig_start,
        orig_end,
        orig_table_recs.len()
    );
    eprintln!(
        "Saved Table[3]:    recs[{}..{}] ({} records)",
        saved_start,
        saved_end,
        saved_table_recs.len()
    );

    // Compare TABLE record flags
    let orig_tbl = orig_table_recs
        .iter()
        .find(|r| r.tag_id == tags::HWPTAG_TABLE)
        .unwrap();
    let saved_tbl = saved_table_recs
        .iter()
        .find(|r| r.tag_id == tags::HWPTAG_TABLE)
        .unwrap();
    eprintln!(
        "\nOriginal TABLE flags: 0x{:08X}",
        u32::from_le_bytes(orig_tbl.data[0..4].try_into().unwrap())
    );
    eprintln!(
        "Saved TABLE flags:    0x{:08X}",
        u32::from_le_bytes(saved_tbl.data[0..4].try_into().unwrap())
    );
    let orig_flags = u32::from_le_bytes(orig_tbl.data[0..4].try_into().unwrap());
    let saved_flags = u32::from_le_bytes(saved_tbl.data[0..4].try_into().unwrap());
    let diff_bits = orig_flags ^ saved_flags;
    eprintln!("Flags diff bits: 0x{:08X}", diff_bits);
    // bit 1 = split page by cell, bit 2 = repeat header
    eprintln!(
        "  bit 1 (split_page_by_cell): orig={} saved={}",
        (orig_flags >> 1) & 1,
        (saved_flags >> 1) & 1
    );
    eprintln!(
        "  bit 2 (repeat_header): orig={} saved={}",
        (orig_flags >> 2) & 1,
        (saved_flags >> 2) & 1
    );
    eprintln!(
        "  bit 3 (?): orig={} saved={}",
        (orig_flags >> 3) & 1,
        (saved_flags >> 3) & 1
    );

    // Compare record-by-record, finding first divergence
    eprintln!("\n--- Record-by-record comparison ---");
    let min_len = orig_table_recs.len().min(saved_table_recs.len());
    let mut first_diverge = None;
    let mut extra_saved_recs = Vec::new();

    // Use alignment: match tag+level sequences
    let mut oi = 0usize;
    let mut si = 0usize;
    let mut matched = 0;
    let mut mismatched = 0;

    while oi < orig_table_recs.len() && si < saved_table_recs.len() {
        let o = &orig_table_recs[oi];
        let s = &saved_table_recs[si];
        if o.tag_id == s.tag_id && o.level == s.level {
            // Same tag and level - compare data
            if o.data != s.data && mismatched < 20 {
                eprintln!(
                    "  [O{}/S{}] {} L{}: DATA DIFFERS (orig={}B, saved={}B)",
                    orig_start + oi,
                    saved_start + si,
                    tags::tag_name(o.tag_id),
                    o.level,
                    o.data.len(),
                    s.data.len()
                );
                // Show specific differences for important records
                if o.tag_id == tags::HWPTAG_PARA_TEXT {
                    eprintln!(
                        "    ORIG PARA_TEXT: {:02X?}",
                        &o.data[..o.data.len().min(40)]
                    );
                    eprintln!(
                        "    SAVED PARA_TEXT: {:02X?}",
                        &s.data[..s.data.len().min(40)]
                    );
                }
                mismatched += 1;
            }
            matched += 1;
            oi += 1;
            si += 1;
        } else {
            // Divergence found
            if first_diverge.is_none() {
                first_diverge = Some((oi, si));
                eprintln!(
                    "\n  FIRST DIVERGENCE at orig[{}]/saved[{}]:",
                    orig_start + oi,
                    saved_start + si
                );
                eprintln!(
                    "    ORIG: {} L{} {}B",
                    tags::tag_name(o.tag_id),
                    o.level,
                    o.data.len()
                );
                eprintln!(
                    "    SAVED: {} L{} {}B",
                    tags::tag_name(s.tag_id),
                    s.level,
                    s.data.len()
                );
            }

            // Try to re-align: is the saved record an insertion?
            // Check if orig[oi] matches saved[si+1]
            if si + 1 < saved_table_recs.len()
                && orig_table_recs[oi].tag_id == saved_table_recs[si + 1].tag_id
                && orig_table_recs[oi].level == saved_table_recs[si + 1].level
            {
                extra_saved_recs.push((saved_start + si, s.clone()));
                si += 1;
                continue;
            }
            // Check if saved[si] matches orig[oi+1]
            if oi + 1 < orig_table_recs.len()
                && orig_table_recs[oi + 1].tag_id == saved_table_recs[si].tag_id
                && orig_table_recs[oi + 1].level == saved_table_recs[si].level
            {
                eprintln!(
                    "    ORIG record [{}] has no match in saved (deleted?)",
                    orig_start + oi
                );
                oi += 1;
                continue;
            }
            // Both advance
            oi += 1;
            si += 1;
        }
    }

    // Print remaining saved records
    while si < saved_table_recs.len() {
        extra_saved_recs.push((saved_start + si, saved_table_recs[si].clone()));
        si += 1;
    }

    eprintln!("\nMatched: {}, Data diffs: {}", matched, mismatched);
    eprintln!("Extra records in saved: {}", extra_saved_recs.len());
    if !extra_saved_recs.is_empty() {
        eprintln!("\n  Extra records in saved (not in original):");
        for (idx, rec) in extra_saved_recs.iter().take(30) {
            eprintln!(
                "    [{}] {} L{} {}B",
                idx,
                tags::tag_name(rec.tag_id),
                rec.level,
                rec.data.len()
            );
            if rec.tag_id == tags::HWPTAG_PARA_TEXT {
                // Decode as UTF-16LE text
                let text: String = rec
                    .data
                    .chunks(2)
                    .filter_map(|c| {
                        if c.len() == 2 {
                            let code = u16::from_le_bytes([c[0], c[1]]);
                            if code == 0x000D || code == 0x000A {
                                return Some('\n');
                            }
                            if code < 0x20 {
                                return None;
                            }
                            char::from_u32(code as u32)
                        } else {
                            None
                        }
                    })
                    .collect();
                eprintln!("      text: '{}'", &text[..text.len().min(80)]);
            }
        }
    }

    // Now compare record types breakdown
    let mut orig_types: std::collections::BTreeMap<u16, usize> = std::collections::BTreeMap::new();
    let mut saved_types: std::collections::BTreeMap<u16, usize> = std::collections::BTreeMap::new();
    for r in orig_table_recs {
        *orig_types.entry(r.tag_id).or_insert(0) += 1;
    }
    for r in saved_table_recs {
        *saved_types.entry(r.tag_id).or_insert(0) += 1;
    }
    eprintln!("\n--- Record type breakdown for Table[3] ---");
    eprintln!("{:<25} {:>6} {:>6} {:>6}", "Tag", "Orig", "Saved", "Diff");
    let all_tags: std::collections::BTreeSet<u16> = orig_types
        .keys()
        .chain(saved_types.keys())
        .copied()
        .collect();
    for tag in &all_tags {
        let oc = orig_types.get(tag).copied().unwrap_or(0);
        let sc = saved_types.get(tag).copied().unwrap_or(0);
        let diff = sc as i64 - oc as i64;
        eprintln!(
            "{:<25} {:>6} {:>6} {:>+6}",
            tags::tag_name(*tag),
            oc,
            sc,
            diff
        );
    }

    // Check cells with different nParagraphs
    eprintln!("\n--- Cells (LIST_HEADER) paragraph count comparison ---");
    let orig_cells: Vec<&Record> = orig_table_recs
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_LIST_HEADER)
        .collect();
    let saved_cells: Vec<&Record> = saved_table_recs
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_LIST_HEADER)
        .collect();
    eprintln!(
        "Original cells: {}, Saved cells: {}",
        orig_cells.len(),
        saved_cells.len()
    );

    let cell_count = orig_cells.len().min(saved_cells.len());
    let mut cells_with_diff = Vec::new();
    for ci in 0..cell_count {
        let o_nparas = if orig_cells[ci].data.len() >= 2 {
            u16::from_le_bytes(orig_cells[ci].data[0..2].try_into().unwrap())
        } else {
            0
        };
        let s_nparas = if saved_cells[ci].data.len() >= 2 {
            u16::from_le_bytes(saved_cells[ci].data[0..2].try_into().unwrap())
        } else {
            0
        };
        if o_nparas != s_nparas {
            cells_with_diff.push((ci, o_nparas, s_nparas));
        }
    }
    if cells_with_diff.is_empty() {
        eprintln!("All cells have same nParagraphs!");
    } else {
        eprintln!("Cells with different nParagraphs:");
        for (ci, o, s) in &cells_with_diff {
            eprintln!(
                "  Cell[{}]: orig={} saved={} (diff={})",
                ci,
                o,
                s,
                *s as i32 - *o as i32
            );
        }
    }

    // Check if saved cells have different data
    eprintln!("\n--- Cell data comparison (first diff bytes) ---");
    for ci in 0..cell_count {
        if orig_cells[ci].data != saved_cells[ci].data {
            let min_len = orig_cells[ci].data.len().min(saved_cells[ci].data.len());
            let mut diffs = Vec::new();
            for i in 0..min_len {
                if orig_cells[ci].data[i] != saved_cells[ci].data[i] {
                    diffs.push((i, orig_cells[ci].data[i], saved_cells[ci].data[i]));
                }
            }
            if !diffs.is_empty() || orig_cells[ci].data.len() != saved_cells[ci].data.len() {
                eprintln!(
                    "  Cell[{}]: {} byte diffs, size orig={} saved={}",
                    ci,
                    diffs.len(),
                    orig_cells[ci].data.len(),
                    saved_cells[ci].data.len()
                );
                for (off, a, b) in diffs.iter().take(5) {
                    eprintln!("    offset {}: orig=0x{:02X} saved=0x{:02X}", off, a, b);
                }
            }
        }
    }

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== TABLE[3] DEEP COMPARISON COMPLETE ===");
    eprintln!("{}", "=".repeat(120));
}



#[test]
fn test_model_table3_cell_text_check() {
    use std::path::Path;

    let orig_path = Path::new("pasts/20250130-hongbo-p2.hwp");
    let saved_path = Path::new("pasts/20250130-hongbo_saved-rp-003.hwp");
    if !orig_path.exists() || !saved_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== MODEL-LEVEL TABLE[3] CELL PARAGRAPH CHECK ===");
    eprintln!("{}", "=".repeat(120));

    // Find Table[3] in the model (it should be the 4th table control)
    fn find_tables(
        paras: &[crate::model::paragraph::Paragraph],
    ) -> Vec<(usize, usize, &crate::model::table::Table)> {
        let mut tables = Vec::new();
        for (pi, para) in paras.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                if let crate::model::control::Control::Table(t) = ctrl {
                    tables.push((pi, ci, t.as_ref()));
                }
            }
        }
        tables
    }

    let orig_tables = find_tables(&orig_doc.sections[0].paragraphs);
    let saved_tables = find_tables(&saved_doc.sections[0].paragraphs);

    eprintln!("Original tables: {}", orig_tables.len());
    eprintln!("Saved tables:    {}", saved_tables.len());

    if orig_tables.len() > 3 && saved_tables.len() > 3 {
        let (_, _, orig_t3) = orig_tables[3];
        let (_, _, saved_t3) = saved_tables[3];

        eprintln!(
            "\nOriginal Table[3]: rows={} cols={} cells={}",
            orig_t3.row_count,
            orig_t3.col_count,
            orig_t3.cells.len()
        );
        eprintln!(
            "Saved Table[3]:    rows={} cols={} cells={}",
            saved_t3.row_count,
            saved_t3.col_count,
            saved_t3.cells.len()
        );

        // Check paragraphs in cells
        let mut orig_para_with_text = 0;
        let mut orig_para_without_text = 0;
        let mut orig_para_with_flag = 0;
        let mut saved_para_with_text = 0;
        let mut saved_para_without_text = 0;
        let mut saved_para_with_flag = 0;

        eprintln!("\n--- Original Table[3] cell paragraphs ---");
        for (ci, cell) in orig_t3.cells.iter().enumerate() {
            for (pi, para) in cell.paragraphs.iter().enumerate() {
                let has_text = !para.text.is_empty();
                if has_text {
                    orig_para_with_text += 1;
                } else {
                    orig_para_without_text += 1;
                }
                if para.has_para_text {
                    orig_para_with_flag += 1;
                }
                if !has_text && !para.has_para_text {
                    // Only show the first few empty paragraphs
                    if ci < 3 {
                        eprintln!("  cell[{}] para[{}]: text='{}' has_para_text={} char_count={} controls={}",
                                ci, pi, &para.text[..para.text.len().min(20)],
                                para.has_para_text, para.char_count, para.controls.len());
                    }
                }
            }
        }

        eprintln!("\n--- Saved Table[3] cell paragraphs ---");
        for (ci, cell) in saved_t3.cells.iter().enumerate() {
            for (pi, para) in cell.paragraphs.iter().enumerate() {
                let has_text = !para.text.is_empty();
                if has_text {
                    saved_para_with_text += 1;
                } else {
                    saved_para_without_text += 1;
                }
                if para.has_para_text {
                    saved_para_with_flag += 1;
                }
                if ci < 3 {
                    eprintln!(
                        "  cell[{}] para[{}]: text='{}' has_para_text={} char_count={} controls={}",
                        ci,
                        pi,
                        &para.text[..para.text.len().min(40)],
                        para.has_para_text,
                        para.char_count,
                        para.controls.len()
                    );
                }
            }
        }

        eprintln!("\n--- Summary ---");
        eprintln!(
            "Original: {} with text, {} without text, {} with has_para_text flag",
            orig_para_with_text, orig_para_without_text, orig_para_with_flag
        );
        eprintln!(
            "Saved:    {} with text, {} without text, {} with has_para_text flag",
            saved_para_with_text, saved_para_without_text, saved_para_with_flag
        );

        // Find cells where text content differs
        let cell_count = orig_t3.cells.len().min(saved_t3.cells.len());
        let mut text_diff_cells = Vec::new();
        for ci in 0..cell_count {
            let opara_count = orig_t3.cells[ci].paragraphs.len();
            let spara_count = saved_t3.cells[ci].paragraphs.len();
            if opara_count != spara_count {
                text_diff_cells.push((
                    ci,
                    format!("para count differs: {} vs {}", opara_count, spara_count),
                ));
                continue;
            }
            for pi in 0..opara_count {
                let op = &orig_t3.cells[ci].paragraphs[pi];
                let sp = &saved_t3.cells[ci].paragraphs[pi];
                if op.text != sp.text || op.has_para_text != sp.has_para_text {
                    text_diff_cells.push((
                        ci,
                        format!(
                            "para[{}]: orig_text='{}' orig_flag={} saved_text='{}' saved_flag={}",
                            pi,
                            &op.text[..op.text.len().min(30)],
                            op.has_para_text,
                            &sp.text[..sp.text.len().min(30)],
                            sp.has_para_text
                        ),
                    ));
                }
            }
        }
        eprintln!(
            "\nCells with text/flag differences: {}",
            text_diff_cells.len()
        );
        for (ci, desc) in text_diff_cells.iter().take(20) {
            eprintln!("  cell[{}]: {}", ci, desc);
        }
    }

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== MODEL-LEVEL CHECK COMPLETE ===");
    eprintln!("{}", "=".repeat(120));
}



#[test]
fn test_roundtrip_empty_cell_corruption() {
    use crate::parser::record::Record;
    use crate::parser::tags;
    use std::path::Path;

    let orig_path = Path::new("pasts/20250130-hongbo-p2.hwp");
    if !orig_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== ROUND-TRIP EMPTY CELL CORRUPTION TEST ===");
    eprintln!("{}", "=".repeat(120));

    // Find Table[3] in the original model
    fn find_tables(
        paras: &[crate::model::paragraph::Paragraph],
    ) -> Vec<(usize, usize, &crate::model::table::Table)> {
        let mut tables = Vec::new();
        for (pi, para) in paras.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                if let crate::model::control::Control::Table(t) = ctrl {
                    tables.push((pi, ci, t.as_ref()));
                }
            }
        }
        tables
    }

    let orig_tables = find_tables(&orig_doc.sections[0].paragraphs);
    assert!(orig_tables.len() > 3, "Need at least 4 tables");
    let (_, _, orig_t3) = orig_tables[3];

    // Check original model: char_count values for empty cells
    eprintln!("\n--- Original Table[3] empty cell char_count/msb/text analysis ---");
    let mut empty_cell_count = 0;
    let mut empty_cell_char_counts = std::collections::HashMap::new();
    for (ci, cell) in orig_t3.cells.iter().enumerate() {
        for (pi, para) in cell.paragraphs.iter().enumerate() {
            if para.text.is_empty() && !para.has_para_text {
                empty_cell_count += 1;
                *empty_cell_char_counts.entry(para.char_count).or_insert(0) += 1;
                if ci < 5 {
                    eprintln!("  cell[{}] para[{}]: text='{}' has_para_text={} char_count={} char_count_msb={} controls={} raw_header_extra_len={}",
                            ci, pi, para.text, para.has_para_text, para.char_count, para.char_count_msb,
                            para.controls.len(), para.raw_header_extra.len());
                    if para.raw_header_extra.len() >= 10 {
                        eprintln!("    raw_header_extra: {:02x?}", &para.raw_header_extra);
                    }
                }
            }
        }
    }
    eprintln!("\nEmpty cells: {}", empty_cell_count);
    for (cc, count) in &empty_cell_char_counts {
        eprintln!("  char_count={}: {} cells", cc, count);
    }

    // Now do a round-trip: serialize from model (bypassing raw_stream)
    eprintln!("\n--- Round-trip: serialize section from model ---");
    // Build records manually from the model paragraphs (bypassing raw_stream check)
    let mut records_from_model = Vec::new();
    for para in &orig_doc.sections[0].paragraphs {
        crate::serializer::body_text::serialize_paragraph_list(
            std::slice::from_ref(para),
            0,
            &mut records_from_model,
        );
    }
    let serialized = crate::serializer::record_writer::write_records(&records_from_model);
    eprintln!("Serialized section size: {} bytes", serialized.len());

    // Parse the serialized records
    let records = Record::read_all(&serialized).unwrap();
    eprintln!("Total records: {}", records.len());

    // Count PARA_TEXT records
    let para_text_count = records
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_PARA_TEXT)
        .count();
    let para_header_count = records
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_PARA_HEADER)
        .count();
    eprintln!("PARA_HEADER: {}", para_header_count);
    eprintln!("PARA_TEXT: {}", para_text_count);

    // Also count from original raw_stream for comparison
    let orig_raw = orig_doc.sections[0].raw_stream.as_ref().unwrap();
    let orig_records = Record::read_all(orig_raw).unwrap();
    let orig_pt_count = orig_records
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_PARA_TEXT)
        .count();
    let orig_ph_count = orig_records
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_PARA_HEADER)
        .count();
    eprintln!("\nOriginal raw:");
    eprintln!("  PARA_HEADER: {}", orig_ph_count);
    eprintln!("  PARA_TEXT: {}", orig_pt_count);

    eprintln!("\nDelta:");
    eprintln!(
        "  PARA_HEADER: {} (expected 0)",
        para_header_count as i32 - orig_ph_count as i32
    );
    eprintln!(
        "  PARA_TEXT: {} (expected 0)",
        para_text_count as i32 - orig_pt_count as i32
    );

    // Check specific records inside Table[3] area
    // Find all CTRL_HEADER records with Table ctrl_id
    let mut table_idx = 0;
    for (ri, rec) in records.iter().enumerate() {
        if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
            if ctrl_id == tags::CTRL_TABLE {
                if table_idx == 3 {
                    // Found Table[3] in serialized output - count children
                    let table_level = rec.level;
                    let mut child_pt_count = 0;
                    let mut child_ph_count = 0;
                    for child_rec in &records[ri + 1..] {
                        if child_rec.level <= table_level {
                            break;
                        }
                        if child_rec.tag_id == tags::HWPTAG_PARA_TEXT {
                            child_pt_count += 1;
                        }
                        if child_rec.tag_id == tags::HWPTAG_PARA_HEADER {
                            child_ph_count += 1;
                        }
                    }
                    eprintln!("\nSerialized Table[3] children:");
                    eprintln!("  PARA_HEADER: {}", child_ph_count);
                    eprintln!("  PARA_TEXT: {}", child_pt_count);

                    // Do the same for original
                    let mut orig_table_idx2 = 0;
                    for (ori, orec) in orig_records.iter().enumerate() {
                        if orec.tag_id == tags::HWPTAG_CTRL_HEADER && orec.data.len() >= 4 {
                            let cid = u32::from_le_bytes([
                                orec.data[0],
                                orec.data[1],
                                orec.data[2],
                                orec.data[3],
                            ]);
                            if cid == tags::CTRL_TABLE {
                                if orig_table_idx2 == 3 {
                                    let olevel = orec.level;
                                    let mut o_pt = 0;
                                    let mut o_ph = 0;
                                    for child_rec in &orig_records[ori + 1..] {
                                        if child_rec.level <= olevel {
                                            break;
                                        }
                                        if child_rec.tag_id == tags::HWPTAG_PARA_TEXT {
                                            o_pt += 1;
                                        }
                                        if child_rec.tag_id == tags::HWPTAG_PARA_HEADER {
                                            o_ph += 1;
                                        }
                                    }
                                    eprintln!("\nOriginal Table[3] children:");
                                    eprintln!("  PARA_HEADER: {}", o_ph);
                                    eprintln!("  PARA_TEXT: {}", o_pt);
                                    break;
                                }
                                orig_table_idx2 += 1;
                            }
                        }
                    }

                    // Check each PARA_TEXT in serialized Table[3] - look for 5-space entries
                    eprintln!("\nSerialized Table[3] PARA_TEXT analysis:");
                    let mut five_space_count = 0;
                    for child_rec in &records[ri + 1..] {
                        if child_rec.level <= table_level {
                            break;
                        }
                        if child_rec.tag_id == tags::HWPTAG_PARA_TEXT {
                            let data = &child_rec.data;
                            // 5 spaces + terminator = [0x20,0x00, 0x20,0x00, 0x20,0x00, 0x20,0x00, 0x20,0x00, 0x0D,0x00]
                            if data.len() == 12 {
                                let is_five_spaces = data
                                    == &[
                                        0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00, 0x20, 0x00,
                                        0x0D, 0x00,
                                    ];
                                if is_five_spaces {
                                    five_space_count += 1;
                                } else {
                                    eprintln!("  12-byte PARA_TEXT (not 5-spaces): {:02x?}", data);
                                }
                            }
                        }
                    }
                    eprintln!("  5-space PARA_TEXT entries: {}", five_space_count);

                    break;
                }
                table_idx += 1;
            }
        }
    }

    // Check model-level data that was used for serialization
    eprintln!("\n--- Model data used for serialization ---");
    let tables_check = find_tables(&orig_doc.sections[0].paragraphs);
    if tables_check.len() > 3 {
        let (_, _, t3) = tables_check[3];
        let mut model_empty = 0;
        let mut model_with_text = 0;
        for cell in &t3.cells {
            for para in &cell.paragraphs {
                if para.text.is_empty() && !para.has_para_text {
                    model_empty += 1;
                } else {
                    model_with_text += 1;
                }
            }
        }
        eprintln!("Model Table[3] paragraphs (from original parse):");
        eprintln!(
            "  Empty paragraphs (text='' && has_para_text=false): {}",
            model_empty
        );
        eprintln!(
            "  Paragraphs with text or has_para_text: {}",
            model_with_text
        );
    }

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== ROUND-TRIP TEST COMPLETE ===");
    eprintln!("{}", "=".repeat(120));
}



#[test]
fn test_saved_file_table_flags_and_origin() {
    use std::path::Path;

    let orig_path = Path::new("pasts/20250130-hongbo-p2.hwp");
    let saved_path = Path::new("pasts/20250130-hongbo_saved-rp-003.hwp");
    if !orig_path.exists() || !saved_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== SAVED FILE TABLE FLAGS AND ORIGIN ANALYSIS ===");
    eprintln!("{}", "=".repeat(120));

    fn find_tables(
        paras: &[crate::model::paragraph::Paragraph],
    ) -> Vec<(usize, usize, &crate::model::table::Table)> {
        let mut tables = Vec::new();
        for (pi, para) in paras.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                if let crate::model::control::Control::Table(t) = ctrl {
                    tables.push((pi, ci, t.as_ref()));
                }
            }
        }
        tables
    }

    let orig_tables = find_tables(&orig_doc.sections[0].paragraphs);
    let saved_tables = find_tables(&saved_doc.sections[0].paragraphs);

    eprintln!(
        "\nOriginal tables: {} | Saved tables: {}",
        orig_tables.len(),
        saved_tables.len()
    );

    // Compare all tables: rows, cols, cells, flags, para positions
    for i in 0..orig_tables.len().max(saved_tables.len()) {
        eprintln!("\n--- Table[{}] ---", i);
        if i < orig_tables.len() {
            let (pi, ci, t) = orig_tables[i];
            let total_paras: usize = t.cells.iter().map(|c| c.paragraphs.len()).sum();
            let text_paras: usize = t
                .cells
                .iter()
                .map(|c| c.paragraphs.iter().filter(|p| !p.text.is_empty()).count())
                .sum();
            eprintln!("  Original: para_pos={} ctrl_pos={} rows={} cols={} cells={} total_paras={} text_paras={} flags=0x{:08x} page_break={:?}",
                    pi, ci, t.row_count, t.col_count, t.cells.len(), total_paras, text_paras, t.attr, t.page_break);
        } else {
            eprintln!("  Original: MISSING");
        }
        if i < saved_tables.len() {
            let (pi, ci, t) = saved_tables[i];
            let total_paras: usize = t.cells.iter().map(|c| c.paragraphs.len()).sum();
            let text_paras: usize = t
                .cells
                .iter()
                .map(|c| c.paragraphs.iter().filter(|p| !p.text.is_empty()).count())
                .sum();
            eprintln!("  Saved:    para_pos={} ctrl_pos={} rows={} cols={} cells={} total_paras={} text_paras={} flags=0x{:08x} page_break={:?}",
                    pi, ci, t.row_count, t.col_count, t.cells.len(), total_paras, text_paras, t.attr, t.page_break);
        } else {
            eprintln!("  Saved: MISSING");
        }
    }

    // Detailed check of Table[3]: compare cell-by-cell
    if orig_tables.len() > 3 && saved_tables.len() > 3 {
        let (_, _, ot) = orig_tables[3];
        let (_, _, st) = saved_tables[3];

        eprintln!("\n--- Table[3] cell-by-cell comparison ---");
        let cell_count = ot.cells.len().min(st.cells.len());
        let mut diffs = Vec::new();
        for ci in 0..cell_count {
            let oc = &ot.cells[ci];
            let sc = &st.cells[ci];
            let mut cell_diffs = Vec::new();

            // Compare cell structure
            if oc.col != sc.col
                || oc.row != sc.row
                || oc.col_span != sc.col_span
                || oc.row_span != sc.row_span
            {
                cell_diffs.push(format!(
                    "position: ({},{}) cs={}x{} vs ({},{}) cs={}x{}",
                    oc.col,
                    oc.row,
                    oc.col_span,
                    oc.row_span,
                    sc.col,
                    sc.row,
                    sc.col_span,
                    sc.row_span
                ));
            }
            if oc.width != sc.width || oc.height != sc.height {
                cell_diffs.push(format!(
                    "size: {}x{} vs {}x{}",
                    oc.width, oc.height, sc.width, sc.height
                ));
            }
            if oc.paragraphs.len() != sc.paragraphs.len() {
                cell_diffs.push(format!(
                    "para_count: {} vs {}",
                    oc.paragraphs.len(),
                    sc.paragraphs.len()
                ));
            }
            // Compare paragraph text
            for pi in 0..oc.paragraphs.len().min(sc.paragraphs.len()) {
                let op = &oc.paragraphs[pi];
                let sp = &sc.paragraphs[pi];
                if op.text != sp.text {
                    cell_diffs.push(format!(
                        "para[{}] text: '{}' vs '{}'",
                        pi,
                        &op.text[..op.text.len().min(30)],
                        &sp.text[..sp.text.len().min(30)]
                    ));
                }
                if op.char_count != sp.char_count {
                    cell_diffs.push(format!(
                        "para[{}] char_count: {} vs {}",
                        pi, op.char_count, sp.char_count
                    ));
                }
                if op.has_para_text != sp.has_para_text {
                    cell_diffs.push(format!(
                        "para[{}] has_para_text: {} vs {}",
                        pi, op.has_para_text, sp.has_para_text
                    ));
                }
            }

            if !cell_diffs.is_empty() {
                diffs.push((ci, cell_diffs));
            }
        }

        eprintln!(
            "Cells with differences: {} out of {}",
            diffs.len(),
            cell_count
        );
        for (ci, cell_diffs) in &diffs {
            eprintln!(
                "  cell[{}] (row={}, col={}):",
                ci, ot.cells[*ci].row, ot.cells[*ci].col
            );
            for d in cell_diffs {
                eprintln!("    {}", d);
            }
        }

        // Show the specific text content of first 5 differing cells
        eprintln!("\n--- First 5 differing cells detail ---");
        for (ci, _) in diffs.iter().take(5) {
            let oc = &ot.cells[*ci];
            let sc = &st.cells[*ci];
            eprintln!("  cell[{}] (row={}, col={}):", ci, oc.row, oc.col);
            for pi in 0..oc.paragraphs.len().min(sc.paragraphs.len()) {
                let op = &oc.paragraphs[pi];
                let sp = &sc.paragraphs[pi];
                eprintln!("    orig para[{}]: text={:?} char_count={} msb={} has_pt={} char_offsets={:?} char_shapes_len={}",
                        pi, &op.text, op.char_count, op.char_count_msb, op.has_para_text, &op.char_offsets, op.char_shapes.len());
                eprintln!("    saved para[{}]: text={:?} char_count={} msb={} has_pt={} char_offsets={:?} char_shapes_len={}",
                        pi, &sp.text, sp.char_count, sp.char_count_msb, sp.has_para_text, &sp.char_offsets, sp.char_shapes.len());
            }
        }
    }

    // Also check: which paragraph does Table[3] belong to, and what else changed in the document?
    eprintln!("\n--- Document-level comparison ---");
    let orig_para_count = orig_doc.sections[0].paragraphs.len();
    let saved_para_count = saved_doc.sections[0].paragraphs.len();
    eprintln!(
        "Section[0] paragraph count: orig={} saved={}",
        orig_para_count, saved_para_count
    );

    // Check non-table paragraphs for text differences
    let min_paras = orig_para_count.min(saved_para_count);
    let mut non_table_diffs = 0;
    for pi in 0..min_paras {
        let op = &orig_doc.sections[0].paragraphs[pi];
        let sp = &saved_doc.sections[0].paragraphs[pi];
        if op.text != sp.text || op.controls.len() != sp.controls.len() {
            non_table_diffs += 1;
            if non_table_diffs <= 5 {
                eprintln!("  para[{}] differs: orig_text_len={} saved_text_len={} orig_ctrls={} saved_ctrls={}",
                        pi, op.text.len(), sp.text.len(), op.controls.len(), sp.controls.len());
            }
        }
    }
    eprintln!("Non-table paragraph differences: {}", non_table_diffs);
    if saved_para_count > orig_para_count {
        eprintln!(
            "Extra paragraphs in saved: {}",
            saved_para_count - orig_para_count
        );
        for pi in orig_para_count..saved_para_count {
            let sp = &saved_doc.sections[0].paragraphs[pi];
            eprintln!(
                "  para[{}]: text_len={} controls={}",
                pi,
                sp.text.len(),
                sp.controls.len()
            );
        }
    }

    // Detailed check of para[8] and para[10]
    eprintln!("\n--- Detailed check of para[8] and para[10] ---");
    for pi in [8, 9, 10, 11] {
        if pi < orig_doc.sections[0].paragraphs.len() {
            let p = &orig_doc.sections[0].paragraphs[pi];
            eprintln!(
                "  ORIG para[{}]: text_len={} text={:?} ctrls={} ctrl_types={:?}",
                pi,
                p.text.len(),
                &p.text.chars().take(30).collect::<String>(),
                p.controls.len(),
                p.controls
                    .iter()
                    .map(|c| match c {
                        crate::model::control::Control::Table(_) => "Table",
                        crate::model::control::Control::Shape(_) => "Shape",
                        crate::model::control::Control::Footnote(_) => "Footnote",
                        crate::model::control::Control::Endnote(_) => "Endnote",
                        crate::model::control::Control::Header(_) => "Header",
                        crate::model::control::Control::Footer(_) => "Footer",
                        crate::model::control::Control::SectionDef(_) => "SectionDef",
                        crate::model::control::Control::ColumnDef(_) => "ColumnDef",
                        crate::model::control::Control::Picture(_) => "Picture",
                        _ => "Other",
                    })
                    .collect::<Vec<_>>()
            );
        }
        if pi < saved_doc.sections[0].paragraphs.len() {
            let p = &saved_doc.sections[0].paragraphs[pi];
            eprintln!(
                "  SAVED para[{}]: text_len={} text={:?} ctrls={} ctrl_types={:?}",
                pi,
                p.text.len(),
                &p.text.chars().take(30).collect::<String>(),
                p.controls.len(),
                p.controls
                    .iter()
                    .map(|c| match c {
                        crate::model::control::Control::Table(_) => "Table",
                        crate::model::control::Control::Shape(_) => "Shape",
                        crate::model::control::Control::Footnote(_) => "Footnote",
                        crate::model::control::Control::Endnote(_) => "Endnote",
                        crate::model::control::Control::Header(_) => "Header",
                        crate::model::control::Control::Footer(_) => "Footer",
                        crate::model::control::Control::SectionDef(_) => "SectionDef",
                        crate::model::control::Control::ColumnDef(_) => "ColumnDef",
                        crate::model::control::Control::Picture(_) => "Picture",
                        _ => "Other",
                    })
                    .collect::<Vec<_>>()
            );
        }
    }

    // Check if Table[3] in saved is the same table (same col/row structure) as original
    // Or if it's a newly created table from paste
    eprintln!("\n--- Table[3] structural identity check ---");
    if orig_tables.len() > 3 && saved_tables.len() > 3 {
        let (_, _, ot) = orig_tables[3];
        let (_, _, st) = saved_tables[3];
        eprintln!(
            "  Same row_count: {} ({}=={})",
            ot.row_count == st.row_count,
            ot.row_count,
            st.row_count
        );
        eprintln!(
            "  Same col_count: {} ({}=={})",
            ot.col_count == st.col_count,
            ot.col_count,
            st.col_count
        );
        eprintln!(
            "  Same cell count: {} ({}=={})",
            ot.cells.len() == st.cells.len(),
            ot.cells.len(),
            st.cells.len()
        );
        eprintln!(
            "  Same attr: {} (0x{:08x}==0x{:08x})",
            ot.attr == st.attr,
            ot.attr,
            st.attr
        );
        eprintln!(
            "  Same border_fill_id: {} ({}=={})",
            ot.border_fill_id == st.border_fill_id,
            ot.border_fill_id,
            st.border_fill_id
        );

        // Compare cells with text - are the actual text values the same?
        let mut text_match_count = 0;
        let mut text_mismatch_count = 0;
        for ci in 0..ot.cells.len().min(st.cells.len()) {
            for pi in 0..ot.cells[ci]
                .paragraphs
                .len()
                .min(st.cells[ci].paragraphs.len())
            {
                let op = &ot.cells[ci].paragraphs[pi];
                let sp = &st.cells[ci].paragraphs[pi];
                if !op.text.is_empty() && op.text == sp.text {
                    text_match_count += 1;
                } else if !op.text.is_empty() && op.text != sp.text {
                    text_mismatch_count += 1;
                }
            }
        }
        eprintln!("  Cells with original text preserved: {}", text_match_count);
        eprintln!(
            "  Cells with original text changed: {}",
            text_mismatch_count
        );
    }

    eprintln!("\n{}", "=".repeat(120));
}



/// 테이블 paste 후 재직렬화 유효성 검증
#[test]
fn test_paste_table_then_export_validation() {
    use crate::parser::record::Record;
    use crate::parser::tags;

    let orig_path = "pasts/20250130-hongbo-p2.hwp";
    if !std::path::Path::new(orig_path).exists() {
        eprintln!("SKIP: 파일 없음");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    // 원본 레코드 수 저장
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();
    let orig_para_text_count = orig_recs
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_PARA_TEXT)
        .count();
    let orig_para_count = orig_recs
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_PARA_HEADER)
        .count();
    eprintln!(
        "원본: {} records, {} PARA_HEADER, {} PARA_TEXT",
        orig_recs.len(),
        orig_para_count,
        orig_para_text_count
    );

    // 간단한 HTML 테이블 paste
    let simple_table_html = r#"<table><tr><td>Cell A</td><td>Cell B</td></tr><tr><td>Cell C</td><td>Cell D</td></tr></table>"#;
    let last_para = doc.document.sections[0].paragraphs.len() - 1;
    let result = doc.paste_html_native(0, last_para, 0, simple_table_html);
    match &result {
        Ok(r) => eprintln!("Paste result: {}", r),
        Err(e) => {
            eprintln!("Paste failed: {:?}", e);
            return;
        }
    }

    // Export
    let saved_data = doc.export_hwp_native().unwrap();
    eprintln!("재직렬화(with paste): {} bytes", saved_data.len());

    // Re-parse
    let doc2 = HwpDocument::from_bytes(&saved_data);
    match &doc2 {
        Ok(d) => eprintln!(
            "재파싱 성공: {} sections, {} paragraphs",
            d.document().sections.len(),
            d.document().sections[0].paragraphs.len()
        ),
        Err(e) => {
            eprintln!("재파싱 실패: {:?}", e);
            // 실패시에도 record-level 분석 진행
        }
    }

    // Record level 분석
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    eprintln!("\n=== Record count comparison (with paste) ===");
    let count_tag = |recs: &[Record], tag: u16| recs.iter().filter(|r| r.tag_id == tag).count();
    let tags_to_check: [(u16, &str); 7] = [
        (tags::HWPTAG_PARA_HEADER, "PARA_HEADER"),
        (tags::HWPTAG_PARA_TEXT, "PARA_TEXT"),
        (tags::HWPTAG_PARA_CHAR_SHAPE, "PARA_CHAR_SHAPE"),
        (tags::HWPTAG_PARA_LINE_SEG, "PARA_LINE_SEG"),
        (tags::HWPTAG_CTRL_HEADER, "CTRL_HEADER"),
        (tags::HWPTAG_LIST_HEADER, "LIST_HEADER"),
        (tags::HWPTAG_TABLE, "TABLE"),
    ];
    for (tag, name) in &tags_to_check {
        let orig_cnt = count_tag(&orig_recs, *tag);
        let saved_cnt = count_tag(&saved_recs, *tag);
        let diff = saved_cnt as i64 - orig_cnt as i64;
        eprintln!(
            "  {}: {} → {} ({}{}){}",
            name,
            orig_cnt,
            saved_cnt,
            if diff > 0 { "+" } else { "" },
            diff,
            if diff != 0 { " ← DIFF" } else { "" }
        );
    }

    // PARA_HEADER/PARA_TEXT consistency
    eprintln!("\n=== Consistency check ===");
    let mut issues = 0;
    let mut i = 0;
    while i < saved_recs.len() {
        if saved_recs[i].tag_id == tags::HWPTAG_PARA_HEADER {
            let ph_data = &saved_recs[i].data;
            let ph_level = saved_recs[i].level;
            let nchars = if ph_data.len() >= 4 {
                u32::from_le_bytes([ph_data[0], ph_data[1], ph_data[2], ph_data[3]]) & 0x7FFFFFFF
            } else {
                0
            };

            // numCharShapes from para_header
            let n_cs = if ph_data.len() >= 14 {
                u16::from_le_bytes([ph_data[12], ph_data[13]])
            } else {
                0
            };

            // Count actual PARA_CHAR_SHAPE entries
            let mut actual_cs = 0u32;
            let mut j = i + 1;
            while j < saved_recs.len() && saved_recs[j].level > ph_level {
                if saved_recs[j].tag_id == tags::HWPTAG_PARA_CHAR_SHAPE
                    && saved_recs[j].level == ph_level + 1
                {
                    actual_cs = (saved_recs[j].data.len() / 8) as u32;
                }
                j += 1;
            }

            if n_cs as u32 != actual_cs && actual_cs > 0 {
                eprintln!(
                    "  rec[{}] PARA_HEADER: numCharShapes={} but actual PARA_CHAR_SHAPE entries={}",
                    i, n_cs, actual_cs
                );
                issues += 1;
            }

            // Check if nchars > 1 but no PARA_TEXT
            let has_text = i + 1 < saved_recs.len()
                && saved_recs[i + 1].tag_id == tags::HWPTAG_PARA_TEXT
                && saved_recs[i + 1].level == ph_level + 1;
            if nchars > 1 && !has_text {
                eprintln!(
                    "  rec[{}] PARA_HEADER nchars={} but NO PARA_TEXT!",
                    i, nchars
                );
                issues += 1;
            }
        }
        i += 1;
    }
    eprintln!("  Total issues: {}", issues);

    // Dump pasted table records
    eprintln!("\n=== Pasted table structure ===");
    let tables: Vec<usize> = saved_recs
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 && {
                let ctrl_id = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                ctrl_id == tags::CTRL_TABLE
            }
        })
        .map(|(i, _)| i)
        .collect();
    eprintln!("Total tables: {}", tables.len());
    if let Some(&last_tbl_idx) = tables.last() {
        let tbl_level = saved_recs[last_tbl_idx].level;
        let mut end = last_tbl_idx + 1;
        while end < saved_recs.len() && saved_recs[end].level > tbl_level {
            end += 1;
        }
        eprintln!(
            "Last (pasted) table: rec[{}..{}] ({} records)",
            last_tbl_idx,
            end,
            end - last_tbl_idx
        );
        for ri in last_tbl_idx..end.min(last_tbl_idx + 40) {
            let r = &saved_recs[ri];
            let tag_name = tags::tag_name(r.tag_id);
            eprintln!("  [{}] {} L{} {}B", ri, tag_name, r.level, r.data.len());
        }
    }

    // Check TABLE record extra bytes in original tables
    eprintln!("\n=== Original TABLE record sizes ===");
    for (ri, r) in orig_recs.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_TABLE {
            let data = &r.data;
            if data.len() >= 8 {
                let nrows = u16::from_le_bytes([data[4], data[5]]);
                let ncols = u16::from_le_bytes([data[6], data[7]]);
                let expected_min = 4 + 2 + 2 + 2 + 8 + (nrows as usize) * 2 + 2;
                let extra = data.len().saturating_sub(expected_min);
                let extra_bytes: Vec<u8> = data[expected_min..].to_vec();
                eprintln!("  rec[{}] TABLE {}B (nrows={} ncols={} expected_min={} extra={} extra_bytes={:02X?})",
                        ri, data.len(), nrows, ncols, expected_min, extra, extra_bytes);
            }
        }
    }

    // 저장 (수동 확인용)
    let out_dir = std::path::Path::new("output");
    if out_dir.exists() {
        std::fs::write(out_dir.join("hongbo_paste_test.hwp"), &saved_data).unwrap();
        eprintln!("\n저장: output/hongbo_paste_test.hwp");
    }
}



/// rp-004 저장 파일의 BodyText 레코드를 분석하여 붙여넣기된 표의 구조적 문제를 찾는다.
#[test]
fn test_rp004_bodytext_table_analysis() {
    use crate::parser::record::Record;
    use crate::parser::tags;

    let orig_path = "pasts/20250130-hongbo-p2.hwp";
    let saved_path = "pasts/20250130-hongbo_saved-rp-004.hwp";

    if !std::path::Path::new(orig_path).exists() {
        eprintln!("SKIP: 원본 파일 없음 ({})", orig_path);
        return;
    }
    if !std::path::Path::new(saved_path).exists() {
        eprintln!("SKIP: 저장 파일 없음 ({})", saved_path);
        return;
    }

    // ============================================================
    // 1. 두 파일 파싱
    // ============================================================
    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();
    eprintln!("원본 파일 크기: {} bytes", orig_data.len());
    eprintln!("저장 파일 크기: {} bytes", saved_data.len());

    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();

    eprintln!(
        "원본: sections={}, compressed={}",
        orig_doc.sections.len(),
        orig_doc.header.compressed
    );
    eprintln!(
        "저장: sections={}, compressed={}",
        saved_doc.sections.len(),
        saved_doc.header.compressed
    );

    // ============================================================
    // 2. BodyText Section[0] raw stream 읽기 및 Record 스캔
    // ============================================================
    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();
    eprintln!(
        "\n원본 BodyText Section[0]: {} bytes, {} records",
        orig_bt.len(),
        orig_recs.len()
    );

    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();
    eprintln!(
        "저장 BodyText Section[0]: {} bytes, {} records",
        saved_bt.len(),
        saved_recs.len()
    );

    // ============================================================
    // 2a. 모든 레코드 목록 출력 (저장 파일)
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 저장 파일 전체 레코드 목록 (Section[0]) ===");
    eprintln!("{}", "=".repeat(120));
    for (i, r) in saved_recs.iter().enumerate() {
        let tname = tags::tag_name(r.tag_id);
        let extra = if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            let ctrl_bytes = [r.data[0], r.data[1], r.data[2], r.data[3]];
            let ctrl_str: String = ctrl_bytes
                .iter()
                .rev()
                .map(|&b| {
                    if b >= 0x20 && b <= 0x7e {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            format!(
                " ctrl_id=0x{:08X} \"{}\" ({})",
                ctrl_id,
                ctrl_str,
                tags::ctrl_name(ctrl_id)
            )
        } else if r.tag_id == tags::HWPTAG_PARA_HEADER && r.data.len() >= 4 {
            let nchars = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            let nchars_val = nchars & 0x7FFFFFFF;
            let control_mask = if r.data.len() >= 8 {
                u32::from_le_bytes([r.data[4], r.data[5], r.data[6], r.data[7]])
            } else {
                0
            };
            format!(
                " char_count={} (raw=0x{:08X}) control_mask=0x{:08X}",
                nchars_val, nchars, control_mask
            )
        } else if r.tag_id == tags::HWPTAG_TABLE && r.data.len() >= 8 {
            let flags = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            let nrows = u16::from_le_bytes([r.data[4], r.data[5]]);
            let ncols = u16::from_le_bytes([r.data[6], r.data[7]]);
            format!(" flags=0x{:08X} rows={} cols={}", flags, nrows, ncols)
        } else if r.tag_id == tags::HWPTAG_LIST_HEADER && r.data.len() >= 4 {
            let nparas = u16::from_le_bytes([r.data[0], r.data[1]]);
            let flags = u16::from_le_bytes([r.data[2], r.data[3]]);
            format!(" nparas={} flags=0x{:04X}", nparas, flags)
        } else {
            String::new()
        };
        eprintln!(
            "  [{:4}] tag=0x{:04X} {:30} L{:<3} {:6}B{}",
            i,
            r.tag_id,
            tname,
            r.level,
            r.data.len(),
            extra
        );
    }

    // ============================================================
    // 2b. 레코드 타입별 카운트 비교
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 레코드 타입별 카운트 비교 (원본 vs 저장) ===");
    eprintln!("{}", "=".repeat(120));
    let count_tag = |recs: &[Record], tag: u16| recs.iter().filter(|r| r.tag_id == tag).count();
    let tags_to_check: [(u16, &str); 10] = [
        (tags::HWPTAG_PARA_HEADER, "PARA_HEADER"),
        (tags::HWPTAG_PARA_TEXT, "PARA_TEXT"),
        (tags::HWPTAG_PARA_CHAR_SHAPE, "PARA_CHAR_SHAPE"),
        (tags::HWPTAG_PARA_LINE_SEG, "PARA_LINE_SEG"),
        (tags::HWPTAG_PARA_RANGE_TAG, "PARA_RANGE_TAG"),
        (tags::HWPTAG_CTRL_HEADER, "CTRL_HEADER"),
        (tags::HWPTAG_LIST_HEADER, "LIST_HEADER"),
        (tags::HWPTAG_TABLE, "TABLE"),
        (tags::HWPTAG_CTRL_DATA, "CTRL_DATA"),
        (tags::HWPTAG_PAGE_DEF, "PAGE_DEF"),
    ];
    for (tag, name) in &tags_to_check {
        let orig_cnt = count_tag(&orig_recs, *tag);
        let saved_cnt = count_tag(&saved_recs, *tag);
        let diff = saved_cnt as i64 - orig_cnt as i64;
        eprintln!(
            "  {:25} orig={:4}  saved={:4}  diff={}{:+}{}",
            name,
            orig_cnt,
            saved_cnt,
            if diff != 0 { "<<< " } else { "" },
            diff,
            if diff != 0 { " >>>" } else { "" }
        );
    }

    // ============================================================
    // 2c. 표(Table) 분석
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 표(Table) 분석 ===");
    eprintln!("{}", "=".repeat(120));

    // 원본의 표 찾기
    let orig_tables: Vec<usize> = orig_recs
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 && {
                let ctrl_id = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                ctrl_id == tags::CTRL_TABLE
            }
        })
        .map(|(i, _)| i)
        .collect();

    let saved_tables: Vec<usize> = saved_recs
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 && {
                let ctrl_id = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                ctrl_id == tags::CTRL_TABLE
            }
        })
        .map(|(i, _)| i)
        .collect();

    eprintln!("원본 표 개수: {}", orig_tables.len());
    eprintln!("저장 표 개수: {}", saved_tables.len());

    // 각 표의 구조 분석 함수
    let analyze_table = |recs: &[Record], tbl_start: usize, label: &str| {
        let tbl_level = recs[tbl_start].level;
        let mut tbl_end = tbl_start + 1;
        while tbl_end < recs.len() && recs[tbl_end].level > tbl_level {
            tbl_end += 1;
        }
        let tbl_record_count = tbl_end - tbl_start;

        eprintln!(
            "\n--- {} (rec[{}..{}], {} records) ---",
            label, tbl_start, tbl_end, tbl_record_count
        );

        // CTRL_HEADER 바이트 덤프 (처음 최대 50바이트)
        let ctrl_hdr = &recs[tbl_start];
        let dump_len = ctrl_hdr.data.len().min(50);
        eprintln!(
            "  CTRL_HEADER ({}B): {:02X?}",
            ctrl_hdr.data.len(),
            &ctrl_hdr.data[..dump_len]
        );

        // TABLE 레코드 찾기
        let mut table_rec_idx = None;
        let mut list_headers: Vec<usize> = Vec::new();

        for ri in tbl_start + 1..tbl_end {
            if recs[ri].tag_id == tags::HWPTAG_TABLE && recs[ri].level == tbl_level + 1 {
                table_rec_idx = Some(ri);
            }
            if recs[ri].tag_id == tags::HWPTAG_LIST_HEADER && recs[ri].level == tbl_level + 1 {
                list_headers.push(ri);
            }
        }

        if let Some(tri) = table_rec_idx {
            let td = &recs[tri].data;
            let dump_len2 = td.len().min(80);
            eprintln!(
                "  TABLE record (rec[{}], {}B): {:02X?}",
                tri,
                td.len(),
                &td[..dump_len2]
            );
            if td.len() >= 8 {
                let flags = u32::from_le_bytes([td[0], td[1], td[2], td[3]]);
                let nrows = u16::from_le_bytes([td[4], td[5]]);
                let ncols = u16::from_le_bytes([td[6], td[7]]);
                eprintln!(
                    "    flags=0x{:08X} rows={} cols={} (expected cells={})",
                    flags,
                    nrows,
                    ncols,
                    nrows as u32 * ncols as u32
                );

                if td.len() >= 10 {
                    let border_fill_id = u16::from_le_bytes([td[8], td[9]]);
                    eprintln!("    border_fill_id={}", border_fill_id);
                }
                if td.len() > 10 {
                    eprintln!(
                        "    remaining bytes (offset 10..): {:02X?}",
                        &td[10..td.len().min(80)]
                    );
                }
            }
        } else {
            eprintln!("  TABLE record: NOT FOUND!");
        }

        // 각 셀(LIST_HEADER) 분석
        eprintln!(
            "  셀 개수 (LIST_HEADER at tbl_level+1): {}",
            list_headers.len()
        );
        for (ci, &lhi) in list_headers.iter().enumerate() {
            let lh = &recs[lhi];
            let dump_len3 = lh.data.len().min(40);
            eprintln!(
                "  Cell[{}] LIST_HEADER (rec[{}], {}B): {:02X?}",
                ci,
                lhi,
                lh.data.len(),
                &lh.data[..dump_len3]
            );

            if lh.data.len() >= 4 {
                let nparas = u16::from_le_bytes([lh.data[0], lh.data[1]]);
                let flags = u16::from_le_bytes([lh.data[2], lh.data[3]]);
                eprintln!("    nparas={} flags=0x{:04X}", nparas, flags);
            }

            // 이 셀에 속하는 PARA_HEADER 찾기
            let cell_level = lh.level;
            let next_boundary = if ci + 1 < list_headers.len() {
                list_headers[ci + 1]
            } else {
                tbl_end
            };

            let mut para_count = 0;
            for ri2 in lhi + 1..next_boundary {
                if recs[ri2].tag_id == tags::HWPTAG_PARA_HEADER && recs[ri2].level == cell_level + 1
                {
                    let ph = &recs[ri2];
                    let nchars = if ph.data.len() >= 4 {
                        u32::from_le_bytes([ph.data[0], ph.data[1], ph.data[2], ph.data[3]])
                            & 0x7FFFFFFF
                    } else {
                        0
                    };
                    let control_mask = if ph.data.len() >= 8 {
                        u32::from_le_bytes([ph.data[4], ph.data[5], ph.data[6], ph.data[7]])
                    } else {
                        0
                    };
                    eprintln!(
                        "      PARA[{}] (rec[{}]) char_count={} control_mask=0x{:08X}",
                        para_count, ri2, nchars, control_mask
                    );
                    para_count += 1;
                }
            }
        }

        (tbl_start, tbl_end, tbl_record_count)
    };

    // 원본 표 분석
    for (ti, &tbl_start) in orig_tables.iter().enumerate() {
        analyze_table(&orig_recs, tbl_start, &format!("원본 표[{}]", ti));
    }

    // 저장 표 분석
    for (ti, &tbl_start) in saved_tables.iter().enumerate() {
        analyze_table(&saved_recs, tbl_start, &format!("저장 표[{}]", ti));
    }

    // ============================================================
    // 2d. 마지막 표 (붙여넣기된 것) 전체 레코드 덤프
    // ============================================================
    if let Some(&last_tbl_idx) = saved_tables.last() {
        let tbl_level = saved_recs[last_tbl_idx].level;
        let mut tbl_end = last_tbl_idx + 1;
        while tbl_end < saved_recs.len() && saved_recs[tbl_end].level > tbl_level {
            tbl_end += 1;
        }
        eprintln!("\n{}", "=".repeat(120));
        eprintln!(
            "=== 마지막(붙여넣기) 표: rec[{}..{}] 전체 레코드 덤프 ===",
            last_tbl_idx, tbl_end
        );
        eprintln!("{}", "=".repeat(120));
        for ri in last_tbl_idx..tbl_end {
            let r = &saved_recs[ri];
            let tname = tags::tag_name(r.tag_id);
            let dump_len = r.data.len().min(64);
            let extra_info = if r.tag_id == tags::HWPTAG_PARA_HEADER && r.data.len() >= 4 {
                let nchars = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                let nchars_val = nchars & 0x7FFFFFFF;
                let control_mask = if r.data.len() >= 8 {
                    u32::from_le_bytes([r.data[4], r.data[5], r.data[6], r.data[7]])
                } else {
                    0
                };
                format!(
                    " | char_count={} control_mask=0x{:08X}",
                    nchars_val, control_mask
                )
            } else if r.tag_id == tags::HWPTAG_PARA_TEXT {
                let u16_chars: Vec<u16> = r
                    .data
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let text = String::from_utf16_lossy(&u16_chars);
                let preview: String = text.chars().take(40).collect();
                format!(" | text_preview=\"{}\"", preview)
            } else {
                String::new()
            };
            eprintln!(
                "  [{:4}] {:30} L{:<3} {:6}B  data[..{}]: {:02X?}{}",
                ri,
                tname,
                r.level,
                r.data.len(),
                dump_len,
                &r.data[..dump_len],
                extra_info
            );
        }
    }

    // ============================================================
    // 3. 문단 일관성 검사 (PARA_HEADER <-> PARA_TEXT)
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 문단 일관성 검사 (저장 파일) ===");
    eprintln!("{}", "=".repeat(120));
    let mut mismatch_count = 0;
    let mut para_idx = 0;
    let mut i = 0;
    while i < saved_recs.len() {
        if saved_recs[i].tag_id == tags::HWPTAG_PARA_HEADER {
            let ph = &saved_recs[i];
            let ph_level = ph.level;
            let nchars = if ph.data.len() >= 4 {
                u32::from_le_bytes([ph.data[0], ph.data[1], ph.data[2], ph.data[3]]) & 0x7FFFFFFF
            } else {
                0
            };
            let control_mask = if ph.data.len() >= 8 {
                u32::from_le_bytes([ph.data[4], ph.data[5], ph.data[6], ph.data[7]])
            } else {
                0
            };

            // 다음 레코드가 PARA_TEXT인지 확인
            let has_text = i + 1 < saved_recs.len()
                && saved_recs[i + 1].tag_id == tags::HWPTAG_PARA_TEXT
                && saved_recs[i + 1].level == ph_level + 1;

            if has_text {
                let pt = &saved_recs[i + 1];
                let pt_byte_len = pt.data.len();
                let expected_byte_len = (nchars as usize) * 2;

                if pt_byte_len != expected_byte_len {
                    eprintln!("  MISMATCH para[{}] rec[{}]: char_count={} => expected PARA_TEXT={}B, actual={}B (diff={})",
                            para_idx, i, nchars, expected_byte_len, pt_byte_len,
                            pt_byte_len as i64 - expected_byte_len as i64);
                    // 텍스트 미리보기
                    let u16_chars: Vec<u16> = pt
                        .data
                        .chunks(2)
                        .filter(|c| c.len() == 2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    let text = String::from_utf16_lossy(&u16_chars);
                    let preview: String = text.chars().take(50).collect();
                    eprintln!("    text_preview: \"{}\"", preview);
                    mismatch_count += 1;
                }
            } else if nchars > 1 {
                eprintln!("  MISSING PARA_TEXT para[{}] rec[{}]: char_count={} control_mask=0x{:08X} but NO PARA_TEXT follows (next tag={})",
                        para_idx, i, nchars, control_mask,
                        if i + 1 < saved_recs.len() {
                            format!("0x{:04X} ({})", saved_recs[i+1].tag_id, tags::tag_name(saved_recs[i+1].tag_id))
                        } else { "EOF".to_string() }
                    );
                mismatch_count += 1;
            } else if nchars == 0 {
                // char_count=0인 PARA_HEADER (빈 문단) 확인
                if has_text {
                    eprintln!(
                        "  UNEXPECTED para[{}] rec[{}]: char_count=0 but PARA_TEXT exists ({}B)",
                        para_idx,
                        i,
                        saved_recs[i + 1].data.len()
                    );
                    mismatch_count += 1;
                }
            }

            para_idx += 1;
        }
        i += 1;
    }
    eprintln!("\n  총 문단 수: {}", para_idx);
    eprintln!("  불일치 개수: {}", mismatch_count);

    // ============================================================
    // 3b. 원본 파일도 동일 검사 (비교용)
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 문단 일관성 검사 (원본 파일) ===");
    eprintln!("{}", "=".repeat(120));
    let mut orig_mismatch_count = 0;
    let mut orig_para_idx = 0;
    i = 0;
    while i < orig_recs.len() {
        if orig_recs[i].tag_id == tags::HWPTAG_PARA_HEADER {
            let ph = &orig_recs[i];
            let ph_level = ph.level;
            let nchars = if ph.data.len() >= 4 {
                u32::from_le_bytes([ph.data[0], ph.data[1], ph.data[2], ph.data[3]]) & 0x7FFFFFFF
            } else {
                0
            };
            let control_mask = if ph.data.len() >= 8 {
                u32::from_le_bytes([ph.data[4], ph.data[5], ph.data[6], ph.data[7]])
            } else {
                0
            };

            let has_text = i + 1 < orig_recs.len()
                && orig_recs[i + 1].tag_id == tags::HWPTAG_PARA_TEXT
                && orig_recs[i + 1].level == ph_level + 1;

            if has_text {
                let pt = &orig_recs[i + 1];
                let pt_byte_len = pt.data.len();
                let expected_byte_len = (nchars as usize) * 2;

                if pt_byte_len != expected_byte_len {
                    eprintln!("  MISMATCH para[{}] rec[{}]: char_count={} => expected PARA_TEXT={}B, actual={}B (diff={})",
                            orig_para_idx, i, nchars, expected_byte_len, pt_byte_len,
                            pt_byte_len as i64 - expected_byte_len as i64);
                    orig_mismatch_count += 1;
                }
            } else if nchars > 1 {
                eprintln!(
                    "  MISSING PARA_TEXT para[{}] rec[{}]: char_count={} control_mask=0x{:08X}",
                    orig_para_idx, i, nchars, control_mask
                );
                orig_mismatch_count += 1;
            }

            orig_para_idx += 1;
        }
        i += 1;
    }
    eprintln!("\n  총 문단 수: {}", orig_para_idx);
    eprintln!("  불일치 개수: {}", orig_mismatch_count);

    // ============================================================
    // 요약
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 요약 ===");
    eprintln!("{}", "=".repeat(120));
    eprintln!(
        "원본 표: {}개, 저장 표: {}개 (차이: {})",
        orig_tables.len(),
        saved_tables.len(),
        saved_tables.len() as i64 - orig_tables.len() as i64
    );
    eprintln!(
        "원본 레코드: {}개, 저장 레코드: {}개 (차이: {})",
        orig_recs.len(),
        saved_recs.len(),
        saved_recs.len() as i64 - orig_recs.len() as i64
    );
    eprintln!(
        "원본 문단 불일치: {}, 저장 문단 불일치: {}",
        orig_mismatch_count, mismatch_count
    );
}



/// rp-005 저장 파일과 원본을 비교하여 붙여넣기된 표의 구조를 깊이 분석한다.
/// DocInfo 일관성, 표 구조, 문단 char_count vs PARA_TEXT 길이, CharShape ID 유효성 검사.
#[test]
fn test_rp005_pasted_table_analysis() {
    use crate::parser::record::Record;
    use crate::parser::tags;
    use std::collections::HashMap;

    let orig_path = "pasts/20250130-hongbo-p2.hwp";
    let saved_path = "pasts/20250130-hongbo_saved-rp-005.hwp";

    if !std::path::Path::new(orig_path).exists() {
        eprintln!("SKIP: 원본 파일 없음 ({})", orig_path);
        return;
    }
    if !std::path::Path::new(saved_path).exists() {
        eprintln!("SKIP: 저장 파일 없음 ({})", saved_path);
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();
    eprintln!("원본 파일 크기: {} bytes", orig_data.len());
    eprintln!("저장 파일 크기: {} bytes", saved_data.len());

    // ============================================================
    // 1. 두 파일 파싱 (고수준 IR)
    // ============================================================
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();

    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 1. DocInfo 비교 ===");
    eprintln!("{}", "=".repeat(120));

    let orig_cs = orig_doc.doc_info.char_shapes.len();
    let saved_cs = saved_doc.doc_info.char_shapes.len();
    let orig_ps = orig_doc.doc_info.para_shapes.len();
    let saved_ps = saved_doc.doc_info.para_shapes.len();
    let orig_bf = orig_doc.doc_info.border_fills.len();
    let saved_bf = saved_doc.doc_info.border_fills.len();
    let orig_st = orig_doc.doc_info.styles.len();
    let saved_st = saved_doc.doc_info.styles.len();

    eprintln!(
        "  CharShape:  orig={:<5} saved={:<5} diff={:+}",
        orig_cs,
        saved_cs,
        saved_cs as i64 - orig_cs as i64
    );
    eprintln!(
        "  ParaShape:  orig={:<5} saved={:<5} diff={:+}",
        orig_ps,
        saved_ps,
        saved_ps as i64 - orig_ps as i64
    );
    eprintln!(
        "  BorderFill: orig={:<5} saved={:<5} diff={:+}",
        orig_bf,
        saved_bf,
        saved_bf as i64 - orig_bf as i64
    );
    eprintln!(
        "  Styles:     orig={:<5} saved={:<5} diff={:+}",
        orig_st,
        saved_st,
        saved_st as i64 - orig_st as i64
    );

    // ID_MAPPINGS 일관성 (raw DocInfo 스트림에서 직접 파싱)
    eprintln!("\n--- ID_MAPPINGS consistency check ---");
    let check_id_mappings =
        |raw: &[u8], label: &str, cs_count: usize, ps_count: usize, bf_count: usize| {
            let recs = Record::read_all(raw).unwrap();
            let idm_rec = recs.iter().find(|r| r.tag_id == tags::HWPTAG_ID_MAPPINGS);
            if let Some(idm) = idm_rec {
                let d = &idm.data;
                // ID_MAPPINGS fields (u32 each):
                // [0]=BinData, [1..7]=FontFace(7lang), [8]=BorderFill, [9]=CharShape,
                // [10]=TabDef, [11]=Numbering, [12]=Bullet, [13]=ParaShape, [14]=Style
                if d.len() >= 60 {
                    let bf_map = u32::from_le_bytes([d[32], d[33], d[34], d[35]]);
                    let cs_map = u32::from_le_bytes([d[36], d[37], d[38], d[39]]);
                    let ps_map = u32::from_le_bytes([d[52], d[53], d[54], d[55]]);

                    let bf_actual = recs
                        .iter()
                        .filter(|r| r.tag_id == tags::HWPTAG_BORDER_FILL)
                        .count();
                    let cs_actual = recs
                        .iter()
                        .filter(|r| r.tag_id == tags::HWPTAG_CHAR_SHAPE)
                        .count();
                    let ps_actual = recs
                        .iter()
                        .filter(|r| r.tag_id == tags::HWPTAG_PARA_SHAPE)
                        .count();

                    eprintln!(
                        "  [{}] ID_MAPPINGS: BorderFill={}, CharShape={}, ParaShape={}",
                        label, bf_map, cs_map, ps_map
                    );
                    eprintln!(
                        "  [{}] actual recs: BorderFill={}, CharShape={}, ParaShape={}",
                        label, bf_actual, cs_actual, ps_actual
                    );
                    eprintln!(
                        "  [{}] model count: BorderFill={}, CharShape={}, ParaShape={}",
                        label, bf_count, ps_count, cs_count
                    );

                    let bf_ok = bf_map as usize == bf_actual && bf_actual == bf_count;
                    let cs_ok = cs_map as usize == cs_actual && cs_actual == cs_count;
                    let ps_ok = ps_map as usize == ps_actual && ps_actual == ps_count;
                    eprintln!(
                        "  [{}] consistency: BF={} CS={} PS={}",
                        label,
                        if bf_ok { "OK" } else { "MISMATCH!" },
                        if cs_ok { "OK" } else { "MISMATCH!" },
                        if ps_ok { "OK" } else { "MISMATCH!" }
                    );
                }
            } else {
                eprintln!("  [{}] ID_MAPPINGS not found!", label);
            }
        };

    if let Some(ref raw) = orig_doc.doc_info.raw_stream {
        check_id_mappings(raw, "orig", orig_cs, orig_ps, orig_bf);
    }
    if let Some(ref raw) = saved_doc.doc_info.raw_stream {
        check_id_mappings(raw, "saved", saved_cs, saved_ps, saved_bf);
    }

    // ============================================================
    // 2. BodyText raw records 읽기
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 2. BodyText 레코드 분석 ===");
    eprintln!("{}", "=".repeat(120));

    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();

    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    eprintln!(
        "  원본 BodyText: {} bytes, {} records",
        orig_bt.len(),
        orig_recs.len()
    );
    eprintln!(
        "  저장 BodyText: {} bytes, {} records",
        saved_bt.len(),
        saved_recs.len()
    );

    // ============================================================
    // 3. 모든 표 찾기
    // ============================================================
    let find_tables = |recs: &[Record]| -> Vec<usize> {
        recs.iter()
            .enumerate()
            .filter(|(_, r)| {
                r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 && {
                    let ctrl_id = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                    ctrl_id == tags::CTRL_TABLE
                }
            })
            .map(|(i, _)| i)
            .collect()
    };

    let orig_tables = find_tables(&orig_recs);
    let saved_tables = find_tables(&saved_recs);

    eprintln!("\n  원본 표 개수: {}", orig_tables.len());
    eprintln!("  저장 표 개수: {}", saved_tables.len());
    eprintln!(
        "  차이: {:+}",
        saved_tables.len() as i64 - orig_tables.len() as i64
    );

    // 붙여넣기된 표 = 저장에만 있는 표 (인덱스가 원본 표 개수 이상인 것)
    let pasted_table_indices: Vec<usize> = if saved_tables.len() > orig_tables.len() {
        saved_tables[orig_tables.len()..].to_vec()
    } else {
        vec![]
    };
    eprintln!("  붙여넣기된 표 시작 인덱스: {:?}", pasted_table_indices);

    // ============================================================
    // 4. 표 구조 분석 함수
    // ============================================================
    let analyze_table_deep = |recs: &[Record], tbl_start: usize, label: &str, cs_count: usize| {
        let tbl_level = recs[tbl_start].level;
        let mut tbl_end = tbl_start + 1;
        while tbl_end < recs.len() && recs[tbl_end].level > tbl_level {
            tbl_end += 1;
        }

        eprintln!(
            "\n  --- {} (rec[{}..{}], {} records, level={}) ---",
            label,
            tbl_start,
            tbl_end,
            tbl_end - tbl_start,
            tbl_level
        );

        // CTRL_HEADER dump
        let ctrl_hdr = &recs[tbl_start];
        let ctrl_id = u32::from_le_bytes([
            ctrl_hdr.data[0],
            ctrl_hdr.data[1],
            ctrl_hdr.data[2],
            ctrl_hdr.data[3],
        ]);
        let ctrl_bytes = [
            ctrl_hdr.data[3],
            ctrl_hdr.data[2],
            ctrl_hdr.data[1],
            ctrl_hdr.data[0],
        ];
        let ctrl_str: String = ctrl_bytes
            .iter()
            .map(|&b| {
                if b >= 0x20 && b <= 0x7e {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        let dump_len = ctrl_hdr.data.len().min(64);
        eprintln!(
            "    CTRL_HEADER ({}B): ctrl_id=0x{:08X} \"{}\"",
            ctrl_hdr.data.len(),
            ctrl_id,
            ctrl_str
        );
        eprintln!(
            "      data[..{}]: {:02X?}",
            dump_len,
            &ctrl_hdr.data[..dump_len]
        );

        // TABLE record
        let mut table_rec_idx = None;
        let mut list_headers: Vec<usize> = Vec::new();

        for ri in tbl_start + 1..tbl_end {
            if recs[ri].tag_id == tags::HWPTAG_TABLE && recs[ri].level == tbl_level + 1 {
                table_rec_idx = Some(ri);
            }
            if recs[ri].tag_id == tags::HWPTAG_LIST_HEADER && recs[ri].level == tbl_level + 1 {
                list_headers.push(ri);
            }
        }

        if let Some(tri) = table_rec_idx {
            let td = &recs[tri].data;
            eprintln!("    TABLE record (rec[{}], {}B):", tri, td.len());
            let dump_len2 = td.len().min(80);
            eprintln!("      data[..{}]: {:02X?}", dump_len2, &td[..dump_len2]);
            if td.len() >= 8 {
                let flags = u32::from_le_bytes([td[0], td[1], td[2], td[3]]);
                let nrows = u16::from_le_bytes([td[4], td[5]]);
                let ncols = u16::from_le_bytes([td[6], td[7]]);
                eprintln!(
                    "      flags=0x{:08X} rows={} cols={} (expected_cells={})",
                    flags,
                    nrows,
                    ncols,
                    nrows as u32 * ncols as u32
                );

                // Cell spacing, padding
                if td.len() >= 10 {
                    let cell_spacing = u16::from_le_bytes([td[8], td[9]]);
                    eprintln!("      cell_spacing={}", cell_spacing);
                }
                // padding: left, right, top, bottom (u16 each) at offset 10..18
                if td.len() >= 18 {
                    let pad_l = u16::from_le_bytes([td[10], td[11]]);
                    let pad_r = u16::from_le_bytes([td[12], td[13]]);
                    let pad_t = u16::from_le_bytes([td[14], td[15]]);
                    let pad_b = u16::from_le_bytes([td[16], td[17]]);
                    eprintln!(
                        "      padding: L={} R={} T={} B={}",
                        pad_l, pad_r, pad_t, pad_b
                    );
                }
                // Row sizes
                if td.len() >= 18 + nrows as usize * 2 {
                    let mut row_sizes = Vec::new();
                    for r in 0..nrows as usize {
                        let off = 18 + r * 2;
                        let rs = u16::from_le_bytes([td[off], td[off + 1]]);
                        row_sizes.push(rs);
                    }
                    eprintln!("      row_sizes: {:?}", row_sizes);
                }
                // border_fill_id
                let bf_off = 18 + nrows as usize * 2;
                if td.len() >= bf_off + 2 {
                    let bf_id = u16::from_le_bytes([td[bf_off], td[bf_off + 1]]);
                    eprintln!("      border_fill_id={}", bf_id);
                }
            }
        } else {
            eprintln!("    TABLE record: NOT FOUND!");
        }

        // LIST_HEADER (cells) and their paragraphs
        eprintln!("    셀 개수 (LIST_HEADER): {}", list_headers.len());

        let mut cell_issues: Vec<String> = Vec::new();

        for (ci, &lhi) in list_headers.iter().enumerate() {
            let lh = &recs[lhi];
            let cell_level = lh.level;
            let dump_len3 = lh.data.len().min(48);
            eprintln!(
                "\n    Cell[{}] LIST_HEADER (rec[{}], {}B, level={}):",
                ci,
                lhi,
                lh.data.len(),
                cell_level
            );
            eprintln!(
                "      data[..{}]: {:02X?}",
                dump_len3,
                &lh.data[..dump_len3]
            );

            if lh.data.len() >= 4 {
                let nparas = u16::from_le_bytes([lh.data[0], lh.data[1]]);
                let flags = u16::from_le_bytes([lh.data[2], lh.data[3]]);
                eprintln!("      nparas={} flags=0x{:04X}", nparas, flags);
            }
            // Cell-specific data: col, row, col_span, row_span, width, height at offsets in LIST_HEADER
            // After the generic LIST_HEADER (first ~14 bytes): col(u16) row(u16) col_span(u16) row_span(u16) width(u32) height(u32) padding(u16x4) border_fill_id(u16)
            if lh.data.len() >= 34 {
                let col_addr = u16::from_le_bytes([lh.data[14], lh.data[15]]);
                let row_addr = u16::from_le_bytes([lh.data[16], lh.data[17]]);
                let col_span = u16::from_le_bytes([lh.data[18], lh.data[19]]);
                let row_span = u16::from_le_bytes([lh.data[20], lh.data[21]]);
                let width =
                    u32::from_le_bytes([lh.data[22], lh.data[23], lh.data[24], lh.data[25]]);
                let height =
                    u32::from_le_bytes([lh.data[26], lh.data[27], lh.data[28], lh.data[29]]);
                eprintln!(
                    "      cell: col={} row={} col_span={} row_span={} width={} height={}",
                    col_addr, row_addr, col_span, row_span, width, height
                );

                let bf_id = u16::from_le_bytes([lh.data[32], lh.data[33]]);
                eprintln!("      border_fill_id={}", bf_id);
            }

            // Find paragraphs belonging to this cell
            let next_boundary = if ci + 1 < list_headers.len() {
                list_headers[ci + 1]
            } else {
                tbl_end
            };

            let mut para_count = 0;
            for ri2 in lhi + 1..next_boundary {
                if recs[ri2].tag_id == tags::HWPTAG_PARA_HEADER && recs[ri2].level == cell_level + 1
                {
                    let ph = &recs[ri2];
                    let raw_char_count = if ph.data.len() >= 4 {
                        u32::from_le_bytes([ph.data[0], ph.data[1], ph.data[2], ph.data[3]])
                    } else {
                        0
                    };
                    let char_count = raw_char_count & 0x7FFFFFFF;
                    let msb = raw_char_count >> 31;
                    let control_mask = if ph.data.len() >= 8 {
                        u32::from_le_bytes([ph.data[4], ph.data[5], ph.data[6], ph.data[7]])
                    } else {
                        0
                    };
                    let para_shape_id = if ph.data.len() >= 10 {
                        u16::from_le_bytes([ph.data[8], ph.data[9]])
                    } else {
                        0
                    };
                    let style_id = if ph.data.len() >= 11 { ph.data[10] } else { 0 };
                    let num_char_shapes = if ph.data.len() >= 14 {
                        u16::from_le_bytes([ph.data[12], ph.data[13]])
                    } else {
                        0
                    };

                    eprintln!("      PARA[{}] (rec[{}]): char_count={} (msb={}) control_mask=0x{:08X} para_shape_id={} style_id={} numCharShapes={}",
                            para_count, ri2, char_count, msb, control_mask, para_shape_id, style_id, num_char_shapes);

                    // para_shape_id validity
                    if (para_shape_id as usize) >= saved_ps {
                        let msg = format!(
                            "Cell[{}] PARA[{}] rec[{}]: para_shape_id={} >= para_shapes.len()={}",
                            ci, para_count, ri2, para_shape_id, saved_ps
                        );
                        eprintln!("        *** INVALID para_shape_id: {} ***", msg);
                        cell_issues.push(msg);
                    }

                    // PARA_TEXT check
                    let has_text = ri2 + 1 < next_boundary
                        && recs[ri2 + 1].tag_id == tags::HWPTAG_PARA_TEXT
                        && recs[ri2 + 1].level == cell_level + 2;

                    if has_text {
                        let pt = &recs[ri2 + 1];
                        let pt_u16_count = pt.data.len() / 2;
                        let expected_u16 = char_count as usize;
                        let u16_chars: Vec<u16> = pt
                            .data
                            .chunks(2)
                            .filter(|c| c.len() == 2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let text = String::from_utf16_lossy(&u16_chars);
                        let preview: String = text.chars().take(60).collect();

                        if pt_u16_count != expected_u16 {
                            let msg = format!("Cell[{}] PARA[{}] rec[{}]: char_count={} but PARA_TEXT has {} u16 units (diff={})",
                                    ci, para_count, ri2, char_count, pt_u16_count, pt_u16_count as i64 - expected_u16 as i64);
                            eprintln!("        *** MISMATCH: {} ***", msg);
                            cell_issues.push(msg);
                        }
                        eprintln!(
                            "        PARA_TEXT ({}B, {} u16): \"{}\"",
                            pt.data.len(),
                            pt_u16_count,
                            preview
                        );
                    } else if char_count > 0 {
                        // char_count > 0 but no PARA_TEXT (might have char_count=1 for empty para end marker only in HEADER)
                        if char_count > 1 {
                            let msg = format!(
                                "Cell[{}] PARA[{}] rec[{}]: char_count={} but NO PARA_TEXT",
                                ci, para_count, ri2, char_count
                            );
                            eprintln!("        *** MISSING PARA_TEXT: {} ***", msg);
                            cell_issues.push(msg);
                        }
                    }

                    // PARA_CHAR_SHAPE check
                    // Look for PARA_CHAR_SHAPE following PARA_TEXT (or PARA_HEADER if no text)
                    let mut pcs_idx = None;
                    for ri3 in ri2 + 1..next_boundary {
                        if recs[ri3].level <= cell_level + 1 {
                            break;
                        } // left this para's children
                        if recs[ri3].tag_id == tags::HWPTAG_PARA_CHAR_SHAPE
                            && recs[ri3].level == cell_level + 2
                        {
                            pcs_idx = Some(ri3);
                            break;
                        }
                    }

                    if let Some(pcs_ri) = pcs_idx {
                        let pcs = &recs[pcs_ri];
                        let num_entries = pcs.data.len() / 8;
                        eprintln!(
                            "        PARA_CHAR_SHAPE (rec[{}], {}B, {} entries):",
                            pcs_ri,
                            pcs.data.len(),
                            num_entries
                        );

                        for ei in 0..num_entries {
                            let off = ei * 8;
                            if off + 8 <= pcs.data.len() {
                                let start_pos = u32::from_le_bytes([
                                    pcs.data[off],
                                    pcs.data[off + 1],
                                    pcs.data[off + 2],
                                    pcs.data[off + 3],
                                ]);
                                let cs_id = u32::from_le_bytes([
                                    pcs.data[off + 4],
                                    pcs.data[off + 5],
                                    pcs.data[off + 6],
                                    pcs.data[off + 7],
                                ]);
                                let valid = (cs_id as usize) < cs_count;
                                eprintln!(
                                    "          [{}] start_pos={} char_shape_id={} {}",
                                    ei,
                                    start_pos,
                                    cs_id,
                                    if valid { "OK" } else { "*** INVALID ***" }
                                );
                                if !valid {
                                    cell_issues.push(format!("Cell[{}] PARA[{}] PARA_CHAR_SHAPE entry[{}]: char_shape_id={} >= {} (invalid)",
                                            ci, para_count, ei, cs_id, cs_count));
                                }
                            }
                        }
                    } else if num_char_shapes > 0 {
                        eprintln!(
                            "        PARA_CHAR_SHAPE: NOT FOUND (numCharShapes={})",
                            num_char_shapes
                        );
                    }

                    para_count += 1;
                }
            }
        }

        cell_issues
    };

    // ============================================================
    // 5. 모든 표 분석
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 3. 표(Table) 상세 분석 ===");
    eprintln!("{}", "=".repeat(120));

    for (ti, &tbl_start) in orig_tables.iter().enumerate() {
        analyze_table_deep(&orig_recs, tbl_start, &format!("원본 표[{}]", ti), orig_cs);
    }

    let mut all_pasted_issues: Vec<String> = Vec::new();
    for (ti, &tbl_start) in saved_tables.iter().enumerate() {
        let is_pasted = pasted_table_indices.contains(&tbl_start);
        let label = if is_pasted {
            format!("저장 표[{}] (*** PASTED ***)", ti)
        } else {
            format!("저장 표[{}]", ti)
        };
        let issues = analyze_table_deep(&saved_recs, tbl_start, &label, saved_cs);
        if is_pasted {
            all_pasted_issues.extend(issues);
        }
    }

    // ============================================================
    // 6. 전체 저장 파일 문단 일관성 검사
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 4. 전체 문단 일관성 검사 (저장 파일) ===");
    eprintln!("{}", "=".repeat(120));

    let mut total_paras = 0u32;
    let mut char_count_mismatches = 0u32;
    let mut missing_para_text = 0u32;
    let mut invalid_cs_refs = 0u32;
    let mut invalid_ps_refs = 0u32;
    let mut max_cs_id: u32 = 0;
    let mut max_ps_id: u16 = 0;

    let mut i = 0;
    while i < saved_recs.len() {
        if saved_recs[i].tag_id == tags::HWPTAG_PARA_HEADER {
            let ph = &saved_recs[i];
            let ph_level = ph.level;
            let raw_char_count = if ph.data.len() >= 4 {
                u32::from_le_bytes([ph.data[0], ph.data[1], ph.data[2], ph.data[3]])
            } else {
                0
            };
            let char_count = raw_char_count & 0x7FFFFFFF;
            let control_mask = if ph.data.len() >= 8 {
                u32::from_le_bytes([ph.data[4], ph.data[5], ph.data[6], ph.data[7]])
            } else {
                0
            };
            let para_shape_id = if ph.data.len() >= 10 {
                u16::from_le_bytes([ph.data[8], ph.data[9]])
            } else {
                0
            };
            let style_id = if ph.data.len() >= 11 { ph.data[10] } else { 0 };

            // para_shape_id validity
            if (para_shape_id as usize) >= saved_ps {
                eprintln!(
                    "  *** para[{}] rec[{}]: para_shape_id={} >= {} (INVALID) ***",
                    total_paras, i, para_shape_id, saved_ps
                );
                invalid_ps_refs += 1;
            }
            if para_shape_id > max_ps_id {
                max_ps_id = para_shape_id;
            }

            // PARA_TEXT check
            let has_text = i + 1 < saved_recs.len()
                && saved_recs[i + 1].tag_id == tags::HWPTAG_PARA_TEXT
                && saved_recs[i + 1].level == ph_level + 1;

            if has_text {
                let pt = &saved_recs[i + 1];
                let pt_u16_count = pt.data.len() / 2;
                let expected_u16 = char_count as usize;

                if pt_u16_count != expected_u16 {
                    eprintln!("  MISMATCH para[{}] rec[{}]: char_count={} but PARA_TEXT has {} u16 (diff={})",
                            total_paras, i, char_count, pt_u16_count, pt_u16_count as i64 - expected_u16 as i64);
                    let u16_chars: Vec<u16> = pt
                        .data
                        .chunks(2)
                        .filter(|c| c.len() == 2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    let text = String::from_utf16_lossy(&u16_chars);
                    let preview: String = text.chars().take(60).collect();
                    eprintln!("    text_preview: \"{}\"", preview);
                    char_count_mismatches += 1;
                }
            } else if char_count > 1 {
                eprintln!(
                    "  MISSING PARA_TEXT para[{}] rec[{}]: char_count={} control_mask=0x{:08X}",
                    total_paras, i, char_count, control_mask
                );
                missing_para_text += 1;
            }

            // PARA_CHAR_SHAPE check for all paragraphs
            for ri3 in i + 1..saved_recs.len() {
                if saved_recs[ri3].level <= ph_level {
                    break;
                }
                if saved_recs[ri3].tag_id == tags::HWPTAG_PARA_CHAR_SHAPE
                    && saved_recs[ri3].level == ph_level + 1
                {
                    let pcs = &saved_recs[ri3];
                    let num_entries = pcs.data.len() / 8;
                    for ei in 0..num_entries {
                        let off = ei * 8;
                        if off + 8 <= pcs.data.len() {
                            let cs_id = u32::from_le_bytes([
                                pcs.data[off + 4],
                                pcs.data[off + 5],
                                pcs.data[off + 6],
                                pcs.data[off + 7],
                            ]);
                            if cs_id > max_cs_id {
                                max_cs_id = cs_id;
                            }
                            if (cs_id as usize) >= saved_cs {
                                eprintln!("  *** para[{}] rec[{}] PARA_CHAR_SHAPE entry[{}]: char_shape_id={} >= {} (INVALID) ***",
                                        total_paras, i, ei, cs_id, saved_cs);
                                invalid_cs_refs += 1;
                            }
                        }
                    }
                    break;
                }
            }

            total_paras += 1;
        }
        i += 1;
    }

    // ============================================================
    // 7. 원본 파일도 동일 검사 (비교용)
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 5. 전체 문단 일관성 검사 (원본 파일) ===");
    eprintln!("{}", "=".repeat(120));

    let mut orig_total_paras = 0u32;
    let mut orig_char_count_mismatches = 0u32;
    let mut orig_invalid_cs_refs = 0u32;
    let mut orig_invalid_ps_refs = 0u32;

    i = 0;
    while i < orig_recs.len() {
        if orig_recs[i].tag_id == tags::HWPTAG_PARA_HEADER {
            let ph = &orig_recs[i];
            let ph_level = ph.level;
            let raw_char_count = if ph.data.len() >= 4 {
                u32::from_le_bytes([ph.data[0], ph.data[1], ph.data[2], ph.data[3]])
            } else {
                0
            };
            let char_count = raw_char_count & 0x7FFFFFFF;
            let para_shape_id = if ph.data.len() >= 10 {
                u16::from_le_bytes([ph.data[8], ph.data[9]])
            } else {
                0
            };

            if (para_shape_id as usize) >= orig_ps {
                eprintln!(
                    "  *** orig para[{}] rec[{}]: para_shape_id={} >= {} (INVALID) ***",
                    orig_total_paras, i, para_shape_id, orig_ps
                );
                orig_invalid_ps_refs += 1;
            }

            let has_text = i + 1 < orig_recs.len()
                && orig_recs[i + 1].tag_id == tags::HWPTAG_PARA_TEXT
                && orig_recs[i + 1].level == ph_level + 1;

            if has_text {
                let pt = &orig_recs[i + 1];
                let pt_u16_count = pt.data.len() / 2;
                if pt_u16_count != char_count as usize {
                    eprintln!(
                        "  MISMATCH orig para[{}] rec[{}]: char_count={} but PARA_TEXT has {} u16",
                        orig_total_paras, i, char_count, pt_u16_count
                    );
                    orig_char_count_mismatches += 1;
                }
            }

            // PARA_CHAR_SHAPE
            for ri3 in i + 1..orig_recs.len() {
                if orig_recs[ri3].level <= ph_level {
                    break;
                }
                if orig_recs[ri3].tag_id == tags::HWPTAG_PARA_CHAR_SHAPE
                    && orig_recs[ri3].level == ph_level + 1
                {
                    let pcs = &orig_recs[ri3];
                    let num_entries = pcs.data.len() / 8;
                    for ei in 0..num_entries {
                        let off = ei * 8;
                        if off + 8 <= pcs.data.len() {
                            let cs_id = u32::from_le_bytes([
                                pcs.data[off + 4],
                                pcs.data[off + 5],
                                pcs.data[off + 6],
                                pcs.data[off + 7],
                            ]);
                            if (cs_id as usize) >= orig_cs {
                                eprintln!("  *** orig para[{}] rec[{}] PARA_CHAR_SHAPE entry[{}]: char_shape_id={} >= {} (INVALID) ***",
                                        orig_total_paras, i, ei, cs_id, orig_cs);
                                orig_invalid_cs_refs += 1;
                            }
                        }
                    }
                    break;
                }
            }

            orig_total_paras += 1;
        }
        i += 1;
    }

    // ============================================================
    // 8. 레코드 타입별 카운트 비교
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== 6. 레코드 타입별 카운트 비교 ===");
    eprintln!("{}", "=".repeat(120));
    let count_tag = |recs: &[Record], tag: u16| recs.iter().filter(|r| r.tag_id == tag).count();
    let tags_to_check: [(u16, &str); 11] = [
        (tags::HWPTAG_PARA_HEADER, "PARA_HEADER"),
        (tags::HWPTAG_PARA_TEXT, "PARA_TEXT"),
        (tags::HWPTAG_PARA_CHAR_SHAPE, "PARA_CHAR_SHAPE"),
        (tags::HWPTAG_PARA_LINE_SEG, "PARA_LINE_SEG"),
        (tags::HWPTAG_PARA_RANGE_TAG, "PARA_RANGE_TAG"),
        (tags::HWPTAG_CTRL_HEADER, "CTRL_HEADER"),
        (tags::HWPTAG_LIST_HEADER, "LIST_HEADER"),
        (tags::HWPTAG_TABLE, "TABLE"),
        (tags::HWPTAG_CTRL_DATA, "CTRL_DATA"),
        (tags::HWPTAG_PAGE_DEF, "PAGE_DEF"),
        (tags::HWPTAG_SHAPE_COMPONENT, "SHAPE_COMPONENT"),
    ];
    for (tag, name) in &tags_to_check {
        let orig_cnt = count_tag(&orig_recs, *tag);
        let saved_cnt = count_tag(&saved_recs, *tag);
        let diff = saved_cnt as i64 - orig_cnt as i64;
        eprintln!(
            "  {:25} orig={:4}  saved={:4}  diff={}{:+}{}",
            name,
            orig_cnt,
            saved_cnt,
            if diff != 0 { "<<< " } else { "" },
            diff,
            if diff != 0 { " >>>" } else { "" }
        );
    }

    // ============================================================
    // 9. 요약
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("=== SUMMARY ===");
    eprintln!("{}", "=".repeat(120));
    eprintln!("  원본: tables={}, paragraphs={}, char_count_mismatches={}, invalid_cs_refs={}, invalid_ps_refs={}",
            orig_tables.len(), orig_total_paras, orig_char_count_mismatches, orig_invalid_cs_refs, orig_invalid_ps_refs);
    eprintln!("  저장: tables={}, paragraphs={}, char_count_mismatches={}, invalid_cs_refs={}, invalid_ps_refs={}, missing_para_text={}",
            saved_tables.len(), total_paras, char_count_mismatches, invalid_cs_refs, invalid_ps_refs, missing_para_text);
    eprintln!("  붙여넣기 표 issues: {}", all_pasted_issues.len());
    for (idx, issue) in all_pasted_issues.iter().enumerate() {
        eprintln!("    [{}] {}", idx, issue);
    }
    eprintln!(
        "  max CharShape ID referenced: {} (available: 0..{})",
        max_cs_id, saved_cs
    );
    eprintln!(
        "  max ParaShape ID referenced: {} (available: 0..{})",
        max_ps_id, saved_ps
    );

    // Assertions
    // We do NOT assert zero mismatches here because the purpose is analysis/reporting.
    // But we flag truly fatal issues.
    if invalid_cs_refs > 0 {
        eprintln!(
            "\n  *** FATAL: {} invalid CharShape ID references in saved file ***",
            invalid_cs_refs
        );
    }
    if invalid_ps_refs > 0 {
        eprintln!(
            "\n  *** FATAL: {} invalid ParaShape ID references in saved file ***",
            invalid_ps_refs
        );
    }
    if char_count_mismatches > 0 {
        eprintln!(
            "\n  *** WARNING: {} char_count vs PARA_TEXT mismatches in saved file ***",
            char_count_mismatches
        );
    }

    eprintln!("\n=== test_rp005_pasted_table_analysis complete ===");
}



#[test]
fn test_analyze_reference_table() {
    // 참조 파일 분석: HWP 프로그램으로 표 1개만 삽입한 파일
    use crate::parser::cfb_reader::LenientCfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let path = "output/1by1-table.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();

    // 표준 cfb 크레이트로 열기 시도, 실패하면 lenient 리더 사용
    let doc = match HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  표준 파서 실패 ({}), LenientCfbReader로 분석합니다.", e);
            // LenientCfbReader로 직접 스트림 추출 후 분석
            let lcfb = LenientCfbReader::open(&data).unwrap();

            eprintln!("\n  [LenientCFB 엔트리 목록]");
            for (name, start, size, otype) in lcfb.list_entries() {
                let tname = match otype {
                    1 => "storage",
                    2 => "stream",
                    5 => "root",
                    _ => "?",
                };
                eprintln!(
                    "  {:20} start={:5} size={:8} type={}",
                    name, start, size, tname
                );
            }

            // FileHeader 읽기
            let fh = lcfb.read_stream("FileHeader").unwrap();
            let compressed = fh.len() >= 37 && (fh[36] & 0x01) != 0;
            eprintln!(
                "\n  FileHeader: {} bytes, compressed={}",
                fh.len(),
                compressed
            );

            // DocInfo 읽기 & 파싱
            let di_data = lcfb.read_doc_info(compressed).unwrap();
            let di_recs = Record::read_all(&di_data).unwrap();
            eprintln!(
                "  DocInfo: {} bytes → {} 레코드",
                di_data.len(),
                di_recs.len()
            );

            // DocProperties (첫 번째 레코드)
            if let Some(dp_rec) = di_recs.first() {
                if dp_rec.tag_id == tags::HWPTAG_DOCUMENT_PROPERTIES && dp_rec.data.len() >= 26 {
                    let d = &dp_rec.data;
                    let caret_list_id = u32::from_le_bytes([d[14], d[15], d[16], d[17]]);
                    let caret_para_id = u32::from_le_bytes([d[18], d[19], d[20], d[21]]);
                    let caret_char_pos = u32::from_le_bytes([d[22], d[23], d[24], d[25]]);
                    eprintln!("\n  [캐럿 위치 (raw)]");
                    eprintln!("  caret_list_id:  {}", caret_list_id);
                    eprintln!("  caret_para_id:  {}", caret_para_id);
                    eprintln!("  caret_char_pos: {}", caret_char_pos);
                }
            }

            // ID_MAPPINGS (두 번째 레코드)
            if di_recs.len() > 1 && di_recs[1].tag_id == tags::HWPTAG_ID_MAPPINGS {
                let d = &di_recs[1].data;
                if d.len() >= 72 {
                    eprintln!("\n  [ID_MAPPINGS]");
                    let labels = [
                        "bin_data",
                        "font_kr",
                        "font_en",
                        "font_cn",
                        "font_jp",
                        "font_etc",
                        "font_sym",
                        "font_usr",
                        "border_fill",
                        "char_shape",
                        "tab_def",
                        "numbering",
                        "bullet",
                        "para_shape",
                        "style",
                        "memo_shape",
                        "trackchange",
                        "trackchange_author",
                    ];
                    for (i, label) in labels.iter().enumerate() {
                        let off = i * 4;
                        let val = u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]]);
                        if val > 0 {
                            eprintln!("  {:20}: {}", label, val);
                        }
                    }
                }
            }

            // BorderFill 레코드 덤프
            eprintln!("\n  [DocInfo BORDER_FILL 레코드]");
            for (i, r) in di_recs.iter().enumerate() {
                if r.tag_id == tags::HWPTAG_BORDER_FILL {
                    eprintln!(
                        "  [{:2}] BORDER_FILL size={} data: {:02x?}",
                        i,
                        r.data.len(),
                        &r.data[..r.data.len().min(60)]
                    );
                }
            }

            // BodyText/Section0 읽기
            let bt_data = lcfb.read_body_text_section(0, compressed).unwrap();
            let bt_recs = Record::read_all(&bt_data).unwrap();

            eprintln!("\n  [BodyText 레코드 덤프] ({} 개)", bt_recs.len());
            for (i, r) in bt_recs.iter().enumerate() {
                let tname = tags::tag_name(r.tag_id);
                let mut extra = String::new();
                if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
                    let cid = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                    extra = format!(" ctrl='{}'", tags::ctrl_name(cid));
                }
                eprintln!(
                    "  [{:2}] tag={:3}({:22}) level={} size={}{}",
                    i,
                    r.tag_id,
                    tname,
                    r.level,
                    r.data.len(),
                    extra
                );
                // 주요 레코드 데이터 덤프
                if matches!(
                    r.tag_id,
                    71 | 72 | 77 | // CTRL_HEADER, LIST_HEADER, TABLE
                        66 | 67 | 68 | 69 // PARA_HEADER, PARA_TEXT, PARA_CHAR_SHAPE, PARA_LINE_SEG
                ) {
                    let show = r.data.len().min(80);
                    eprintln!("        data[..{}]: {:02x?}", show, &r.data[..show]);
                }
            }

            // empty.hwp와 비교
            let empty_path = "template/empty.hwp";
            if std::path::Path::new(empty_path).exists() {
                let empty_data = std::fs::read(empty_path).unwrap();
                let empty_parsed = crate::parser::parse_hwp(&empty_data).unwrap();
                let mut empty_cfb =
                    crate::parser::cfb_reader::CfbReader::open(&empty_data).unwrap();
                let empty_bt = empty_cfb
                    .read_body_text_section(0, empty_parsed.header.compressed, false)
                    .unwrap();
                let empty_recs = Record::read_all(&empty_bt).unwrap();
                eprintln!(
                    "\n  [비교] empty.hwp={} 개, roundtrip.hwp={} 개 → 추가={} 개",
                    empty_recs.len(),
                    bt_recs.len(),
                    bt_recs.len() as i32 - empty_recs.len() as i32
                );
            }

            eprintln!("\n=== 참조 파일 분석 완료 (LenientCfbReader) ===");
            return;
        }
    };
    let doc = doc;

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  참조 파일 분석: {}", path);
    eprintln!("{}", "=".repeat(60));

    // 1. 캐럿 위치 정보
    let dp = &doc.document.doc_properties;
    eprintln!("\n  [캐럿 위치]");
    eprintln!("  caret_list_id:  {}", dp.caret_list_id);
    eprintln!("  caret_para_id:  {}", dp.caret_para_id);
    eprintln!("  caret_char_pos: {}", dp.caret_char_pos);

    // 2. 섹션/문단 구조
    for (si, sec) in doc.document.sections.iter().enumerate() {
        eprintln!("\n  [섹션 {}] 문단 수: {}", si, sec.paragraphs.len());
        for (pi, para) in sec.paragraphs.iter().enumerate() {
            eprintln!(
                "  문단[{}]: text='{}' char_count={} controls={} char_offsets={:?}",
                pi,
                para.text,
                para.char_count,
                para.controls.len(),
                para.char_offsets
            );
            eprintln!(
                "    control_mask=0x{:08X} para_shape_id={} style_id={}",
                para.control_mask, para.para_shape_id, para.style_id
            );
            eprintln!("    char_shapes: {:?}", para.char_shapes);
            eprintln!("    line_segs: {:?}", para.line_segs);
            eprintln!(
                "    raw_header_extra({} bytes): {:02x?}",
                para.raw_header_extra.len(),
                &para.raw_header_extra[..para.raw_header_extra.len().min(20)]
            );

            for (ci, ctrl) in para.controls.iter().enumerate() {
                match ctrl {
                    crate::model::control::Control::SectionDef(_) => {
                        eprintln!("    ctrl[{}]: SectionDef", ci)
                    }
                    crate::model::control::Control::ColumnDef(_) => {
                        eprintln!("    ctrl[{}]: ColumnDef", ci)
                    }
                    crate::model::control::Control::Table(t) => {
                        eprintln!(
                            "    ctrl[{}]: Table {}x{} cells={} attr=0x{:08X}",
                            ci,
                            t.row_count,
                            t.col_count,
                            t.cells.len(),
                            t.attr
                        );
                        eprintln!(
                            "      raw_ctrl_data({} bytes): {:02x?}",
                            t.raw_ctrl_data.len(),
                            &t.raw_ctrl_data[..t.raw_ctrl_data.len().min(40)]
                        );
                        eprintln!(
                            "      raw_table_record_attr=0x{:08X}",
                            t.raw_table_record_attr
                        );
                        eprintln!(
                            "      raw_table_record_extra({} bytes): {:02x?}",
                            t.raw_table_record_extra.len(),
                            &t.raw_table_record_extra[..t.raw_table_record_extra.len().min(20)]
                        );
                        eprintln!(
                            "      padding: l={} r={} t={} b={}",
                            t.padding.left, t.padding.right, t.padding.top, t.padding.bottom
                        );
                        eprintln!(
                            "      cell_spacing={} border_fill_id={} row_sizes={:?}",
                            t.cell_spacing, t.border_fill_id, t.row_sizes
                        );
                        for (celli, cell) in t.cells.iter().enumerate() {
                            eprintln!(
                                "      cell[{}]: col={} row={} span={}x{} w={} h={} bfid={}",
                                celli,
                                cell.col,
                                cell.row,
                                cell.col_span,
                                cell.row_span,
                                cell.width,
                                cell.height,
                                cell.border_fill_id
                            );
                            eprintln!(
                                "        padding: l={} r={} t={} b={}",
                                cell.padding.left,
                                cell.padding.right,
                                cell.padding.top,
                                cell.padding.bottom
                            );
                            eprintln!("        list_header_width_ref={} raw_list_extra({} bytes): {:02x?}",
                                    cell.list_header_width_ref, cell.raw_list_extra.len(),
                                    &cell.raw_list_extra[..cell.raw_list_extra.len().min(20)]);
                            for (cpi, cp) in cell.paragraphs.iter().enumerate() {
                                eprintln!(
                                    "        para[{}]: text='{}' cc={} cs={:?} ls={:?}",
                                    cpi, cp.text, cp.char_count, cp.char_shapes, cp.line_segs
                                );
                                eprintln!("          control_mask=0x{:08X} raw_header_extra({} bytes): {:02x?}",
                                        cp.control_mask, cp.raw_header_extra.len(),
                                        &cp.raw_header_extra[..cp.raw_header_extra.len().min(20)]);
                            }
                        }
                    }
                    _ => eprintln!("    ctrl[{}]: {:?}", ci, std::mem::discriminant(ctrl)),
                }
            }
        }
    }

    // 3. BodyText 레코드 덤프
    let mut cfb = crate::parser::cfb_reader::CfbReader::open(&data).unwrap();
    let parsed = crate::parser::parse_hwp(&data).unwrap();
    let bt = cfb
        .read_body_text_section(0, parsed.header.compressed, false)
        .unwrap();
    let recs = Record::read_all(&bt).unwrap();

    eprintln!("\n  [BodyText 레코드 덤프] ({} 개)", recs.len());
    for (i, r) in recs.iter().enumerate() {
        let tname = tags::tag_name(r.tag_id);
        let mut extra = String::new();
        if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let cid = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            extra = format!(" ctrl='{}'", tags::ctrl_name(cid));
        }
        eprintln!(
            "  [{:2}] tag={:3}({:22}) level={} size={}{}",
            i,
            r.tag_id,
            tname,
            r.level,
            r.data.len(),
            extra
        );
        // CTRL_HEADER, TABLE, LIST_HEADER 데이터 덤프
        if matches!(r.tag_id, 71 | 72 | 77) {
            // CTRL_HEADER, LIST_HEADER, TABLE
            let show = r.data.len().min(60);
            eprintln!("        data[..{}]: {:02x?}", show, &r.data[..show]);
        }
    }

    // 4. 원본 empty.hwp 레코드 수 비교
    let empty_path = "template/empty.hwp";
    if std::path::Path::new(empty_path).exists() {
        let empty_data = std::fs::read(empty_path).unwrap();
        let empty_parsed = crate::parser::parse_hwp(&empty_data).unwrap();
        let mut empty_cfb = crate::parser::cfb_reader::CfbReader::open(&empty_data).unwrap();
        let empty_bt = empty_cfb
            .read_body_text_section(0, empty_parsed.header.compressed, false)
            .unwrap();
        let empty_recs = Record::read_all(&empty_bt).unwrap();
        eprintln!(
            "\n  [비교] empty.hwp={} 개, roundtrip.hwp={} 개 → 차이={} 개",
            empty_recs.len(),
            recs.len(),
            recs.len() as i32 - empty_recs.len() as i32
        );
    }

    // 5. DocInfo 분석
    eprintln!("\n  [DocInfo]");
    eprintln!(
        "  bin_data_count: {}",
        doc.document.doc_info.bin_data_list.len()
    );
    eprintln!(
        "  border_fill_count: {}",
        doc.document.doc_info.border_fills.len()
    );
    eprintln!(
        "  char_shape_count: {}",
        doc.document.doc_info.char_shapes.len()
    );
    eprintln!(
        "  para_shape_count: {}",
        doc.document.doc_info.para_shapes.len()
    );

    // 6. BorderFill 상세 분석
    eprintln!("\n  [BorderFill 상세]");
    for (bi, bf) in doc.document.doc_info.border_fills.iter().enumerate() {
        eprintln!(
            "  bf[{}]: borders=[{:?}, {:?}, {:?}, {:?}] diag={:?}",
            bi, bf.borders[0], bf.borders[1], bf.borders[2], bf.borders[3], bf.diagonal
        );
        eprintln!("    attr={} fill={:?}", bf.attr, bf.fill);
        if let Some(ref raw) = bf.raw_data {
            let show = raw.len().min(60);
            eprintln!("    raw_data({} bytes): {:02x?}", raw.len(), &raw[..show]);
        }
    }

    eprintln!("\n=== 참조 파일 분석 완료 ===");
}



#[test]
fn test_save_table_1x1() {
    // 단계 3: 빈 HWP에 1×1 표 삽입 → 저장
    // 참조: output/1by1-table.hwp (HWP 프로그램으로 생성한 1x1 표)
    use crate::model::control::Control;
    use crate::model::table::{Cell, Table};
    use crate::model::Padding;
    use crate::parser::record::Record;

    let path = "template/empty.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let orig_data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  단계 3: 1×1 표 삽입 → 저장 (참조파일 기반)");
    eprintln!("{}", "=".repeat(60));

    // 참조 파일의 값을 사용하여 표 생성
    // cell_width=41954, cell_height=282 (참조 파일 기준)
    let table_width: u32 = 41954; // 참조 파일과 동일
    let table_height: u32 = 1282; // 참조 파일과 동일
    let cell_width: u32 = 41954;
    let cell_height: u32 = 282;

    // 셀 내부 문단: 빈 문단 (CR만, char_count=1, MSB set)
    let cell_seg_width = 40932; // 참조: cell_width - 패딩(510+510) - 2
    let cell_para = Paragraph {
        text: String::new(),
        char_count: 1,
        char_count_msb: true, // 참조: 0x80000001
        control_mask: 0,
        para_shape_id: 0, // empty.hwp의 기존 para_shape 사용
        style_id: 0,
        char_shapes: vec![crate::model::paragraph::CharShapeRef {
            start_pos: 0,
            char_shape_id: 0, // empty.hwp의 기존 char_shape 사용
        }],
        line_segs: vec![LineSeg {
            text_start: 0,
            vertical_pos: 0,
            line_height: 1000,
            text_height: 1000,
            baseline_distance: 850,
            line_spacing: 600,
            column_start: 0,
            segment_width: cell_seg_width,
            tag: LineSeg::TAG_SINGLE_SEGMENT_LINE,
        }],
        has_para_text: false, // 빈 문단: PARA_TEXT 없음
        raw_header_extra: vec![0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00],
        ..Default::default()
    };

    let cell = Cell {
        col: 0,
        row: 0,
        col_span: 1,
        row_span: 1,
        width: cell_width,
        height: cell_height,
        border_fill_id: 1,
        padding: Padding {
            left: 510,
            right: 510,
            top: 141,
            bottom: 141,
        }, // 참조값
        list_header_width_ref: 0,
        // raw_list_extra: 참조파일의 13바이트 (width + zeros)
        raw_list_extra: {
            let mut v = Vec::new();
            v.extend_from_slice(&cell_width.to_le_bytes()); // [e2,a3,00,00]
            v.extend_from_slice(&[0u8; 9]); // zeros
            v
        },
        paragraphs: vec![cell_para],
        ..Default::default()
    };

    // CommonObjAttr 바이너리 생성 (참조 파일의 raw_ctrl_data 38바이트)
    let raw_ctrl_data = {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes()); // y_offset = 0
        v.extend_from_slice(&0u32.to_le_bytes()); // x_offset = 0
        v.extend_from_slice(&table_width.to_le_bytes()); // width
        v.extend_from_slice(&table_height.to_le_bytes()); // height
        v.extend_from_slice(&1u32.to_le_bytes()); // z_order = 1
        v.extend_from_slice(&283u16.to_le_bytes()); // margin_left
        v.extend_from_slice(&283u16.to_le_bytes()); // margin_right
        v.extend_from_slice(&283u16.to_le_bytes()); // margin_top
        v.extend_from_slice(&283u16.to_le_bytes()); // margin_bottom
        v.extend_from_slice(&0x7C1E9738u32.to_le_bytes()); // instance_id
        v.extend_from_slice(&0u32.to_le_bytes()); // unknown1
        v.extend_from_slice(&0u16.to_le_bytes()); // unknown2
        v
    };

    // DocInfo에 실선 테두리 BorderFill 추가 (참조: bf[0])
    use crate::model::style::{
        BorderFill, BorderLine, BorderLineType, CenterLine, DiagonalLine, Fill,
    };
    let solid_border = BorderLine {
        line_type: BorderLineType::Solid,
        width: 1,
        color: 0,
    };
    let new_bf = BorderFill {
        raw_data: None,
        attr: 0,
        borders: [solid_border, solid_border, solid_border, solid_border],
        diagonal: DiagonalLine {
            diagonal_type: 1,
            width: 0,
            color: 0,
        },
        center_line: CenterLine::None,
        fill: Fill::default(),
        three_d: false,
    };
    doc.document.doc_info.border_fills.push(new_bf);
    let table_bf_id = doc.document.doc_info.border_fills.len() as u16; // 1-based ID

    let table = Table {
        attr: 0x082A2210, // 참조: CommonObjAttr flags
        row_count: 1,
        col_count: 1,
        cell_spacing: 0,
        padding: Padding {
            left: 510,
            right: 510,
            top: 141,
            bottom: 141,
        }, // 참조값
        row_sizes: vec![1],
        border_fill_id: table_bf_id,
        cells: {
            // cell의 border_fill_id도 갱신
            let mut c = cell;
            c.border_fill_id = table_bf_id;
            vec![c]
        },
        raw_ctrl_data,
        raw_table_record_attr: 6,                 // 참조: attr=6
        raw_table_record_extra: vec![0x00, 0x00], // 참조: 2바이트
        ..Default::default()
    };
    eprintln!(
        "  DocInfo: border_fill_count={}, table_bf_id={}",
        doc.document.doc_info.border_fills.len(),
        table_bf_id
    );

    // 첫 번째 문단에 Table 컨트롤 추가
    {
        let para = &mut doc.document.sections[0].paragraphs[0];
        para.controls.push(Control::Table(Box::new(table)));
        para.ctrl_data_records.push(None);
        para.char_count += 8; // 표 제어문자 8 code units
        para.control_mask = 0x00000804; // 참조: 표가 있는 문단의 control_mask

        // 표가 있는 문단의 segment_width는 0 (참조 파일)
        if let Some(ls) = para.line_segs.first_mut() {
            ls.segment_width = 0;
        }
    }

    // 두 번째 빈 문단 추가 (HWP는 표 삽입 시 아래에 빈 문단을 자동 추가)
    let empty_para = Paragraph {
        text: String::new(),
        char_count: 1,        // CR만
        char_count_msb: true, // 참조: 0x80000001
        control_mask: 0,
        para_shape_id: 0, // empty.hwp의 기존 para_shape 사용
        style_id: 0,
        char_shapes: vec![crate::model::paragraph::CharShapeRef {
            start_pos: 0,
            char_shape_id: 0,
        }],
        line_segs: vec![LineSeg {
            text_start: 0,
            vertical_pos: 1848, // 참조: 표 아래 위치
            line_height: 1000,
            text_height: 1000,
            baseline_distance: 850,
            line_spacing: 600,
            column_start: 0,
            segment_width: 42520, // 참조: 편집 영역 전체 너비
            tag: LineSeg::TAG_SINGLE_SEGMENT_LINE,
        }],
        has_para_text: false,
        raw_header_extra: vec![0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00],
        ..Default::default()
    };
    doc.document.sections[0].paragraphs.push(empty_para);

    // raw_stream 무효화 (재직렬화 유도)
    doc.document.sections[0].raw_stream = None;

    // 캐럿 위치: 두 번째 문단(표 아래 빈 줄) 시작
    doc.document.doc_properties.caret_list_id = 1; // 문단 인덱스 1
    doc.document.doc_properties.caret_para_id = 0;
    doc.document.doc_properties.caret_char_pos = 0;
    doc.document.doc_info.raw_stream = None;
    doc.document.doc_properties.raw_data = None;

    let para = &doc.document.sections[0].paragraphs[0];
    eprintln!(
        "  문단[0]: text='{}' char_count={} controls={} seg_width={}",
        para.text,
        para.char_count,
        para.controls.len(),
        para.line_segs
            .first()
            .map(|ls| ls.segment_width)
            .unwrap_or(-1)
    );
    let para1 = &doc.document.sections[0].paragraphs[1];
    eprintln!(
        "  문단[1]: text='{}' char_count={} vpos={}",
        para1.text,
        para1.char_count,
        para1
            .line_segs
            .first()
            .map(|ls| ls.vertical_pos)
            .unwrap_or(-1)
    );

    // HWP 저장
    let saved = doc.export_hwp_native();
    assert!(saved.is_ok(), "HWP 저장 실패: {:?}", saved.err());
    let saved_data = saved.unwrap();

    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/save_test_table_1x1.hwp", &saved_data).unwrap();
    eprintln!(
        "  저장: output/save_test_table_1x1.hwp ({} bytes)",
        saved_data.len()
    );

    // 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&saved_data);
    assert!(doc2.is_ok(), "재파싱 실패: {:?}", doc2.err());
    let doc2 = doc2.unwrap();

    // 표 컨트롤 존재 검증
    let para2 = &doc2.document.sections[0].paragraphs[0];
    eprintln!(
        "  재파싱: text='{}' char_count={} controls={}",
        para2.text,
        para2.char_count,
        para2.controls.len()
    );
    let table_found = para2
        .controls
        .iter()
        .any(|c| matches!(c, Control::Table(_)));
    assert!(table_found, "재파싱된 문서에 표 컨트롤이 없음");

    // 표 내용 검증
    if let Some(Control::Table(t)) = para2
        .controls
        .iter()
        .find(|c| matches!(c, Control::Table(_)))
    {
        eprintln!(
            "  표: {}×{} cells={}",
            t.row_count,
            t.col_count,
            t.cells.len()
        );
        for (ci, cell) in t.cells.iter().enumerate() {
            eprintln!(
                "  셀[{}]: col={} row={} w={} h={} text='{}'",
                ci,
                cell.col,
                cell.row,
                cell.width,
                cell.height,
                cell.paragraphs
                    .first()
                    .map(|p| p.text.as_str())
                    .unwrap_or("")
            );
        }
        assert_eq!(t.row_count, 1);
        assert_eq!(t.col_count, 1);
        assert_eq!(t.cells.len(), 1);
        assert_eq!(t.cells[0].paragraphs.len(), 1);
        // 빈 셀 확인 (참조 파일 기반)
        assert_eq!(t.cells[0].paragraphs[0].char_count, 1); // CR만
    }

    // 두 번째 문단 (표 아래 빈 줄) 검증
    assert!(
        doc2.document.sections[0].paragraphs.len() >= 2,
        "표 아래 빈 문단이 없음"
    );
    let para_below = &doc2.document.sections[0].paragraphs[1];
    eprintln!(
        "  문단[1]: char_count={} controls={}",
        para_below.char_count,
        para_below.controls.len()
    );

    // 저장 레코드 덤프 (참조 파일과 비교)
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    eprintln!("\n  --- 저장 레코드 덤프 ({} 개) ---", saved_recs.len());
    use crate::parser::tags as t;
    for (i, r) in saved_recs.iter().enumerate() {
        let tname = t::tag_name(r.tag_id);
        let mut extra = String::new();
        if r.tag_id == t::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let cid = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            extra = format!(" ctrl='{}'", t::ctrl_name(cid));
        }
        eprintln!(
            "  [{:2}] tag={:3}({:22}) level={} size={}{}",
            i,
            r.tag_id,
            tname,
            r.level,
            r.data.len(),
            extra
        );
    }

    // 참조 파일과 레코드 수 비교
    eprintln!("\n  [참조 비교] 참조=21개, 저장={}개", saved_recs.len());

    eprintln!("\n=== 단계 3 표 저장 검증 완료 ===");
}



/// 추가 검증: 표 안에 이미지 삽입 — 참조 파일 분석
/// output/pic-in-tb-01.hwp: 빈 문서 → 1×1 표 → 셀 안에 이미지 삽입
#[test]
fn test_analyze_pic_in_table() {
    use crate::model::control::Control;
    use crate::parser::cfb_reader::LenientCfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let path = "output/pic-in-tb-01.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();

    eprintln!("\n{}", "=".repeat(70));
    eprintln!("  표 안 이미지 참조 파일 분석: {}", path);
    eprintln!("  파일 크기: {} bytes", data.len());
    eprintln!("{}", "=".repeat(70));

    let doc = match HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  표준 파서 실패 ({}), LenientCfbReader로 분석합니다.", e);
            let lcfb = LenientCfbReader::open(&data).unwrap();

            eprintln!("\n  [LenientCFB 엔트리]");
            for (name, start, size, otype) in lcfb.list_entries() {
                let tname = match otype {
                    1 => "storage",
                    2 => "stream",
                    5 => "root",
                    _ => "?",
                };
                eprintln!(
                    "  {:30} start={:5} size={:8} type={}",
                    name, start, size, tname
                );
            }

            let fh = lcfb.read_stream("FileHeader").unwrap();
            let compressed = fh.len() >= 37 && (fh[36] & 0x01) != 0;

            // DocInfo
            let di_data = lcfb.read_doc_info(compressed).unwrap();
            let di_recs = Record::read_all(&di_data).unwrap();

            // 캐럿 위치
            if let Some(dp_rec) = di_recs.first() {
                if dp_rec.tag_id == tags::HWPTAG_DOCUMENT_PROPERTIES && dp_rec.data.len() >= 26 {
                    let d = &dp_rec.data;
                    eprintln!("\n  [캐럿 위치]");
                    eprintln!(
                        "  list_id={} para_id={} char_pos={}",
                        u32::from_le_bytes([d[14], d[15], d[16], d[17]]),
                        u32::from_le_bytes([d[18], d[19], d[20], d[21]]),
                        u32::from_le_bytes([d[22], d[23], d[24], d[25]])
                    );
                }
            }

            // ID_MAPPINGS
            if di_recs.len() > 1 && di_recs[1].tag_id == tags::HWPTAG_ID_MAPPINGS {
                let d = &di_recs[1].data;
                if d.len() >= 72 {
                    eprintln!("\n  [ID_MAPPINGS]");
                    let labels = [
                        "bin_data",
                        "font_kr",
                        "font_en",
                        "font_cn",
                        "font_jp",
                        "font_etc",
                        "font_sym",
                        "font_usr",
                        "border_fill",
                        "char_shape",
                        "tab_def",
                        "numbering",
                        "bullet",
                        "para_shape",
                        "style",
                        "memo_shape",
                        "trackchange",
                        "trackchange_author",
                    ];
                    for (i, label) in labels.iter().enumerate() {
                        let off = i * 4;
                        let val = u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]]);
                        if val > 0 {
                            eprintln!("  {:20}: {}", label, val);
                        }
                    }
                }
            }

            // BIN_DATA 레코드
            eprintln!("\n  [DocInfo BIN_DATA 레코드]");
            for (i, r) in di_recs.iter().enumerate() {
                if r.tag_id == tags::HWPTAG_BIN_DATA {
                    eprintln!(
                        "  [{:2}] BIN_DATA size={} data: {:02x?}",
                        i,
                        r.data.len(),
                        &r.data[..r.data.len().min(60)]
                    );
                }
            }

            // BodyText
            let bt_data = lcfb.read_body_text_section(0, compressed).unwrap();
            let bt_recs = Record::read_all(&bt_data).unwrap();

            eprintln!("\n  [BodyText 레코드] ({} 개)", bt_recs.len());
            for (i, r) in bt_recs.iter().enumerate() {
                let tname = tags::tag_name(r.tag_id);
                let mut extra = String::new();
                if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
                    let cid = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                    extra = format!(" ctrl='{}'", tags::ctrl_name(cid));
                }
                eprintln!(
                    "  [{:2}] tag={:3}({:22}) level={} size={}{}",
                    i,
                    r.tag_id,
                    tname,
                    r.level,
                    r.data.len(),
                    extra
                );
                if matches!(r.tag_id, 66 | 67 | 68 | 69 | 71 | 72 | 76 | 77 | 85) {
                    let show = r.data.len().min(100);
                    eprintln!("        data[..{}]: {:02x?}", show, &r.data[..show]);
                    if r.data.len() > 100 {
                        eprintln!("        total: {} bytes", r.data.len());
                    }
                }
            }

            eprintln!("\n=== 분석 완료 (LenientCfbReader) ===");
            return;
        }
    };

    // === 표준 파서 성공 ===

    // 1. 캐럿 위치
    let dp = &doc.document.doc_properties;
    eprintln!("\n  [캐럿 위치]");
    eprintln!(
        "  list_id={} para_id={} char_pos={}",
        dp.caret_list_id, dp.caret_para_id, dp.caret_char_pos
    );

    // 2. BinData
    eprintln!(
        "\n  [BinData] ({} 개)",
        doc.document.doc_info.bin_data_list.len()
    );
    for (i, bd) in doc.document.doc_info.bin_data_list.iter().enumerate() {
        eprintln!(
            "  [{}] attr=0x{:04X} type={:?} storage_id={} ext={:?}",
            i, bd.attr, bd.data_type, bd.storage_id, bd.extension
        );
    }
    eprintln!(
        "  [BinDataContent] ({} 개)",
        doc.document.bin_data_content.len()
    );
    for (i, bc) in doc.document.bin_data_content.iter().enumerate() {
        eprintln!(
            "  [{}] id={} ext='{}' size={}",
            i,
            bc.id,
            bc.extension,
            bc.data.len()
        );
    }

    // 3. 문단/컨트롤 구조 (재귀적)
    for (si, sec) in doc.document.sections.iter().enumerate() {
        eprintln!("\n  [섹션 {}] 문단: {}", si, sec.paragraphs.len());
        for (pi, para) in sec.paragraphs.iter().enumerate() {
            eprintln!(
                "  문단[{}]: cc={} msb={} ctrls={} mask=0x{:08X} ps={} ss={}",
                pi,
                para.char_count,
                para.char_count_msb,
                para.controls.len(),
                para.control_mask,
                para.para_shape_id,
                para.style_id
            );
            eprintln!("    cs={:?} ls={:?}", para.char_shapes, para.line_segs);
            eprintln!(
                "    raw_header_extra({} bytes): {:02x?}",
                para.raw_header_extra.len(),
                &para.raw_header_extra[..para.raw_header_extra.len().min(20)]
            );

            for (ci, ctrl) in para.controls.iter().enumerate() {
                match ctrl {
                    Control::SectionDef(_) => eprintln!("    ctrl[{}]: SectionDef", ci),
                    Control::ColumnDef(_) => eprintln!("    ctrl[{}]: ColumnDef", ci),
                    Control::Table(t) => {
                        eprintln!(
                            "    ctrl[{}]: Table {}×{} cells={} bfid={} attr=0x{:08X}",
                            ci,
                            t.row_count,
                            t.col_count,
                            t.cells.len(),
                            t.border_fill_id,
                            t.attr
                        );
                        eprintln!(
                            "      padding: l={} r={} t={} b={}",
                            t.padding.left, t.padding.right, t.padding.top, t.padding.bottom
                        );
                        eprintln!(
                            "      cell_spacing={} row_sizes={:?}",
                            t.cell_spacing, t.row_sizes
                        );
                        eprintln!(
                            "      raw_ctrl_data({} bytes): {:02x?}",
                            t.raw_ctrl_data.len(),
                            &t.raw_ctrl_data[..t.raw_ctrl_data.len().min(40)]
                        );
                        for (celli, cell) in t.cells.iter().enumerate() {
                            eprintln!(
                                "      cell[{}]: col={} row={} span={}×{} w={} h={} bfid={}",
                                celli,
                                cell.col,
                                cell.row,
                                cell.col_span,
                                cell.row_span,
                                cell.width,
                                cell.height,
                                cell.border_fill_id
                            );
                            eprintln!(
                                "        padding: l={} r={} t={} b={} paras={}",
                                cell.padding.left,
                                cell.padding.right,
                                cell.padding.top,
                                cell.padding.bottom,
                                cell.paragraphs.len()
                            );
                            // 셀 내 문단/컨트롤
                            for (cpi, cp) in cell.paragraphs.iter().enumerate() {
                                eprintln!(
                                    "        para[{}]: cc={} msb={} ctrls={} mask=0x{:08X}",
                                    cpi,
                                    cp.char_count,
                                    cp.char_count_msb,
                                    cp.controls.len(),
                                    cp.control_mask
                                );
                                eprintln!("          cs={:?}", cp.char_shapes);
                                eprintln!("          ls={:?}", cp.line_segs);
                                for (cci, cctrl) in cp.controls.iter().enumerate() {
                                    match cctrl {
                                        Control::Picture(pic) => {
                                            eprintln!(
                                                "          ctrl[{}]: Picture {}×{} bid={}",
                                                cci,
                                                pic.common.width,
                                                pic.common.height,
                                                pic.image_attr.bin_data_id
                                            );
                                            eprintln!("            attr=0x{:08X} z={} margins=({},{},{},{})",
                                                    pic.common.attr, pic.common.z_order,
                                                    pic.common.margin.left, pic.common.margin.right,
                                                    pic.common.margin.top, pic.common.margin.bottom);
                                            eprintln!("            shape: ctrl_id=0x{:08X} two={} orig={}×{} cur={}×{}",
                                                    pic.shape_attr.ctrl_id, pic.shape_attr.is_two_ctrl_id,
                                                    pic.shape_attr.original_width, pic.shape_attr.original_height,
                                                    pic.shape_attr.current_width, pic.shape_attr.current_height);
                                            eprintln!(
                                                "            border_x={:?} border_y={:?}",
                                                pic.border_x, pic.border_y
                                            );
                                            eprintln!(
                                                "            crop: l={} t={} r={} b={}",
                                                pic.crop.left,
                                                pic.crop.top,
                                                pic.crop.right,
                                                pic.crop.bottom
                                            );
                                            eprintln!("            raw_extra({} bytes) raw_rendering({} bytes) raw_pic_extra({} bytes)",
                                                    pic.common.raw_extra.len(), pic.shape_attr.raw_rendering.len(), pic.raw_picture_extra.len());
                                        }
                                        _ => eprintln!(
                                            "          ctrl[{}]: {:?}",
                                            cci,
                                            std::mem::discriminant(cctrl)
                                        ),
                                    }
                                }
                            }
                        }
                    }
                    Control::Picture(pic) => {
                        eprintln!(
                            "    ctrl[{}]: Picture {}×{} bid={}",
                            ci, pic.common.width, pic.common.height, pic.image_attr.bin_data_id
                        );
                    }
                    _ => eprintln!("    ctrl[{}]: {:?}", ci, std::mem::discriminant(ctrl)),
                }
            }
        }
    }

    // 4. BodyText 레코드 덤프
    let parsed = crate::parser::parse_hwp(&data).unwrap();
    let mut cfb = crate::parser::cfb_reader::CfbReader::open(&data).unwrap();
    let bt = cfb
        .read_body_text_section(0, parsed.header.compressed, false)
        .unwrap();
    let recs = Record::read_all(&bt).unwrap();

    eprintln!("\n  [BodyText 레코드] ({} 개)", recs.len());
    for (i, r) in recs.iter().enumerate() {
        let tname = tags::tag_name(r.tag_id);
        let mut extra = String::new();
        if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let cid = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            extra = format!(" ctrl='{}'", tags::ctrl_name(cid));
        }
        eprintln!(
            "  [{:2}] tag={:3}({:22}) level={} size={}{}",
            i,
            r.tag_id,
            tname,
            r.level,
            r.data.len(),
            extra
        );
        if matches!(r.tag_id, 66 | 67 | 68 | 69 | 71 | 72 | 76 | 77 | 85) {
            let show = r.data.len().min(100);
            eprintln!("        data[..{}]: {:02x?}", show, &r.data[..show]);
            if r.data.len() > 100 {
                eprintln!("        total: {} bytes", r.data.len());
            }
        }
    }

    // 5. 비교
    let empty_path = "template/empty.hwp";
    if std::path::Path::new(empty_path).exists() {
        let empty_data = std::fs::read(empty_path).unwrap();
        let empty_parsed = crate::parser::parse_hwp(&empty_data).unwrap();
        let mut empty_cfb = crate::parser::cfb_reader::CfbReader::open(&empty_data).unwrap();
        let empty_bt = empty_cfb
            .read_body_text_section(0, empty_parsed.header.compressed, false)
            .unwrap();
        let empty_recs = Record::read_all(&empty_bt).unwrap();
        eprintln!(
            "\n  [비교] empty.hwp={} 개, pic-in-tb={} 개 → 차이={} 개",
            empty_recs.len(),
            recs.len(),
            recs.len() as i32 - empty_recs.len() as i32
        );
    }

    // 6. 라운드트립 검증
    eprintln!("\n  [라운드트립 검증]");
    let mut doc_mut = HwpDocument::from_bytes(&data).unwrap();
    for sec in &mut doc_mut.document.sections {
        sec.raw_stream = None;
    }
    doc_mut.document.doc_info.raw_stream = None;
    doc_mut.document.doc_properties.raw_data = None;

    let saved = doc_mut.export_hwp_native();
    match saved {
        Ok(saved_data) => {
            let _ = std::fs::create_dir_all("output");
            std::fs::write("output/roundtrip_pic_in_tb.hwp", &saved_data).unwrap();
            eprintln!(
                "  저장: output/roundtrip_pic_in_tb.hwp ({} bytes)",
                saved_data.len()
            );

            // 재파싱
            match HwpDocument::from_bytes(&saved_data) {
                Ok(doc2) => {
                    eprintln!("  재파싱 성공 ✓");

                    // 레코드 비교
                    let saved_parsed = crate::parser::parse_hwp(&saved_data).unwrap();
                    let mut saved_cfb =
                        crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
                    let saved_bt = saved_cfb
                        .read_body_text_section(0, saved_parsed.header.compressed, false)
                        .unwrap();
                    let saved_recs = Record::read_all(&saved_bt).unwrap();

                    eprintln!("  레코드: 원본={} 저장={}", recs.len(), saved_recs.len());

                    let max = recs.len().max(saved_recs.len());
                    let mut diff_count = 0;
                    for i in 0..max {
                        if i < recs.len() && i < saved_recs.len() {
                            let o = &recs[i];
                            let s = &saved_recs[i];
                            let mark =
                                if o.tag_id == s.tag_id && o.level == s.level && o.data == s.data {
                                    "=="
                                } else if o.tag_id == s.tag_id && o.level == s.level {
                                    "~="
                                } else {
                                    "!="
                                };
                            if mark != "==" {
                                diff_count += 1;
                                if diff_count <= 10 {
                                    eprintln!(
                                        "  [{:2}] {} {}(lv{} sz{}) vs {}(lv{} sz{})",
                                        i,
                                        mark,
                                        tags::tag_name(o.tag_id),
                                        o.level,
                                        o.data.len(),
                                        tags::tag_name(s.tag_id),
                                        s.level,
                                        s.data.len()
                                    );
                                }
                            }
                        }
                    }
                    if diff_count > 10 {
                        eprintln!("  ... 외 {} 개", diff_count - 10);
                    }
                    eprintln!(
                        "  일치: {}/{} ({}%)",
                        max.saturating_sub(diff_count),
                        max,
                        if max > 0 {
                            (max.saturating_sub(diff_count)) * 100 / max
                        } else {
                            100
                        }
                    );

                    // 표 안 이미지 보존 확인
                    let mut pic_in_cell = false;
                    for sec in &doc2.document.sections {
                        for para in &sec.paragraphs {
                            for ctrl in &para.controls {
                                if let Control::Table(t) = ctrl {
                                    for cell in &t.cells {
                                        for cp in &cell.paragraphs {
                                            for cc in &cp.controls {
                                                if matches!(cc, Control::Picture(_)) {
                                                    pic_in_cell = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    eprintln!(
                        "  표 안 이미지 보존: {}",
                        if pic_in_cell { "✓" } else { "✗" }
                    );
                    assert!(pic_in_cell, "라운드트립 후 표 안 이미지가 사라짐!");
                }
                Err(e) => eprintln!("  재파싱 실패: {}", e),
            }
        }
        Err(e) => eprintln!("  저장 실패: {}", e),
    }

    eprintln!("\n=== 표 안 이미지 참조 파일 분석 완료 ===");
}



/// 추가 검증: 빈 HWP → 1×1 표 → 셀 안에 이미지 삽입 (FROM SCRATCH)
/// 참조: output/pic-in-tb-01.hwp (HWP 프로그램으로 생성)
#[test]
fn test_save_pic_in_table() {
    use crate::model::bin_data::{
        BinData, BinDataCompression, BinDataContent, BinDataStatus, BinDataType,
    };
    use crate::model::control::Control;
    use crate::model::paragraph::{CharShapeRef, LineSeg, Paragraph};
    use crate::model::table::{Cell, Table};
    use crate::model::Padding;
    use crate::parser::record::Record;
    use crate::parser::tags;

    eprintln!("\n=== 추가 검증: 표 안 이미지 저장 (FROM SCRATCH) ===");

    // 1. 참조 파일에서 Table, Picture, BinData 구조 추출
    let ref_path = "output/pic-in-tb-01.hwp";
    if !std::path::Path::new(ref_path).exists() {
        eprintln!("SKIP: {} 없음", ref_path);
        return;
    }
    let ref_data = std::fs::read(ref_path).unwrap();
    let ref_doc = HwpDocument::from_bytes(&ref_data).unwrap();

    // 참조 파일에서 Table 컨트롤 추출
    let ref_table = ref_doc.document.sections[0].paragraphs[0]
        .controls
        .iter()
        .find_map(|c| {
            if let Control::Table(t) = c {
                Some(t)
            } else {
                None
            }
        })
        .expect("참조 파일에 Table 컨트롤 없음");

    // 참조 파일에서 셀 안 Picture 컨트롤 추출
    let ref_pic = ref_table.cells[0].paragraphs[0]
        .controls
        .iter()
        .find_map(|c| {
            if let Control::Picture(p) = c {
                Some(p)
            } else {
                None
            }
        })
        .expect("참조 파일 셀 안에 Picture 컨트롤 없음");

    let ref_bindata = &ref_doc.document.doc_info.bin_data_list[0];
    let ref_bincontent = &ref_doc.document.bin_data_content[0];
    let ref_cell = &ref_table.cells[0];
    let ref_cell_para = &ref_cell.paragraphs[0];
    let ref_para0 = &ref_doc.document.sections[0].paragraphs[0];
    let ref_para1 = &ref_doc.document.sections[0].paragraphs[1];

    eprintln!(
        "  참조 Table: {}×{} bfid={} attr=0x{:08X}",
        ref_table.row_count, ref_table.col_count, ref_table.border_fill_id, ref_table.attr
    );
    eprintln!(
        "  참조 Cell: col={} row={} w={} h={} bfid={}",
        ref_cell.col, ref_cell.row, ref_cell.width, ref_cell.height, ref_cell.border_fill_id
    );
    eprintln!(
        "  참조 Cell 문단: cc={} msb={} mask=0x{:08X} ctrls={}",
        ref_cell_para.char_count,
        ref_cell_para.char_count_msb,
        ref_cell_para.control_mask,
        ref_cell_para.controls.len()
    );
    eprintln!(
        "  참조 Picture: {}×{} bid={} z={}",
        ref_pic.common.width,
        ref_pic.common.height,
        ref_pic.image_attr.bin_data_id,
        ref_pic.common.z_order
    );
    eprintln!(
        "  참조 캐럿: list_id={} para_id={} char_pos={}",
        ref_doc.document.doc_properties.caret_list_id,
        ref_doc.document.doc_properties.caret_para_id,
        ref_doc.document.doc_properties.caret_char_pos
    );

    // 2. empty.hwp 로드
    let empty_path = "template/empty.hwp";
    assert!(
        std::path::Path::new(empty_path).exists(),
        "template/empty.hwp 없음"
    );
    let empty_data = std::fs::read(empty_path).unwrap();
    let mut doc = HwpDocument::from_bytes(&empty_data).unwrap();

    // 3. DocInfo에 BinData 추가
    let bin_data_entry = BinData {
        attr: ref_bindata.attr,
        data_type: BinDataType::Embedding,
        compression: BinDataCompression::Default,
        status: BinDataStatus::NotAccessed,
        storage_id: 1,
        extension: Some(ref_bincontent.extension.clone()),
        raw_data: None,
        ..Default::default()
    };
    doc.document.doc_info.bin_data_list.push(bin_data_entry);

    // BinDataContent 추가
    doc.document.bin_data_content.push(BinDataContent {
        id: 1,
        data: ref_bincontent.data.clone(),
        extension: ref_bincontent.extension.clone(),
    });

    // 4. DocInfo에 BorderFill 추가 (표 테두리용)
    use crate::model::style::{
        BorderFill, BorderLine, BorderLineType, CenterLine, DiagonalLine, Fill,
    };
    let solid_border = BorderLine {
        line_type: BorderLineType::Solid,
        width: 1,
        color: 0,
    };
    let new_bf = BorderFill {
        raw_data: None,
        attr: 0,
        borders: [solid_border, solid_border, solid_border, solid_border],
        diagonal: DiagonalLine {
            diagonal_type: 1,
            width: 0,
            color: 0,
        },
        center_line: CenterLine::None,
        fill: Fill::default(),
        three_d: false,
    };
    doc.document.doc_info.border_fills.push(new_bf);
    let table_bf_id = doc.document.doc_info.border_fills.len() as u16;
    eprintln!(
        "  DocInfo: border_fill_count={}, table_bf_id={}",
        doc.document.doc_info.border_fills.len(),
        table_bf_id
    );

    // 5. Picture 컨트롤 구성 (참조 파일의 정확한 값 사용)
    let picture = crate::model::image::Picture {
        common: ref_pic.common.clone(),
        shape_attr: ref_pic.shape_attr.clone(),
        border_color: ref_pic.border_color,
        border_width: ref_pic.border_width,
        border_attr: ref_pic.border_attr.clone(),
        border_x: ref_pic.border_x,
        border_y: ref_pic.border_y,
        crop: ref_pic.crop.clone(),
        padding: ref_pic.padding.clone(),
        image_attr: ref_pic.image_attr.clone(),
        href: ref_pic.href.clone(),
        border_opacity: ref_pic.border_opacity,
        instance_id: ref_pic.instance_id,
        raw_picture_extra: ref_pic.raw_picture_extra.clone(),
        effects: ref_pic.effects.clone(),
        caption: None,
        img_dim: (0, 0),
        reverse: ref_pic.reverse,
        lock: false,
    };

    // 6. 셀 내부 문단 구성 (cc=9: gso(8)+CR(1), mask=0x00000800)
    let cell_para = Paragraph {
        text: String::new(),
        char_count: ref_cell_para.char_count,         // 9
        char_count_msb: ref_cell_para.char_count_msb, // true
        control_mask: ref_cell_para.control_mask,     // 0x00000800
        para_shape_id: 0,
        style_id: 0,
        raw_break_type: ref_cell_para.raw_break_type,
        char_shapes: vec![CharShapeRef {
            start_pos: 0,
            char_shape_id: 0,
        }],
        line_segs: vec![LineSeg {
            text_start: ref_cell_para.line_segs[0].text_start,
            vertical_pos: ref_cell_para.line_segs[0].vertical_pos,
            line_height: ref_cell_para.line_segs[0].line_height, // 15600 (= image height)
            text_height: ref_cell_para.line_segs[0].text_height,
            baseline_distance: ref_cell_para.line_segs[0].baseline_distance,
            line_spacing: ref_cell_para.line_segs[0].line_spacing,
            column_start: ref_cell_para.line_segs[0].column_start,
            segment_width: ref_cell_para.line_segs[0].segment_width, // 40932
            tag: ref_cell_para.line_segs[0].tag,
        }],
        has_para_text: true, // gso 제어문자 있으므로 PARA_TEXT 필요
        controls: vec![Control::Picture(Box::new(picture))],
        raw_header_extra: ref_cell_para.raw_header_extra.clone(),
        ..Default::default()
    };

    // 7. Cell 구성
    let cell = Cell {
        col: ref_cell.col,
        row: ref_cell.row,
        col_span: ref_cell.col_span,
        row_span: ref_cell.row_span,
        width: ref_cell.width,
        height: ref_cell.height,
        border_fill_id: table_bf_id,
        padding: Padding {
            left: ref_cell.padding.left,
            right: ref_cell.padding.right,
            top: ref_cell.padding.top,
            bottom: ref_cell.padding.bottom,
        },
        list_header_width_ref: ref_cell.list_header_width_ref,
        raw_list_extra: ref_cell.raw_list_extra.clone(),
        paragraphs: vec![cell_para],
        ..Default::default()
    };

    // 8. Table 구성
    let table = Table {
        attr: ref_table.attr,
        row_count: ref_table.row_count,
        col_count: ref_table.col_count,
        cell_spacing: ref_table.cell_spacing,
        padding: Padding {
            left: ref_table.padding.left,
            right: ref_table.padding.right,
            top: ref_table.padding.top,
            bottom: ref_table.padding.bottom,
        },
        row_sizes: ref_table.row_sizes.clone(),
        border_fill_id: table_bf_id,
        cells: vec![cell],
        raw_ctrl_data: ref_table.raw_ctrl_data.clone(),
        raw_table_record_attr: ref_table.raw_table_record_attr,
        raw_table_record_extra: ref_table.raw_table_record_extra.clone(),
        ..Default::default()
    };

    // 9. 첫 번째 문단에 Table 컨트롤 추가
    {
        let para = &mut doc.document.sections[0].paragraphs[0];
        para.controls.push(Control::Table(Box::new(table)));
        para.ctrl_data_records.push(None);
        para.char_count += 8; // 표 제어문자 8 code units
        para.control_mask = ref_para0.control_mask; // 0x00000804

        // 표가 있는 문단의 segment_width는 0 (참조 파일)
        if let Some(ls) = para.line_segs.first_mut() {
            ls.segment_width = 0;
        }
    }

    // 10. 두 번째 빈 문단 추가 (표 아래)
    let empty_para = Paragraph {
        text: String::new(),
        char_count: ref_para1.char_count,         // 1
        char_count_msb: ref_para1.char_count_msb, // true
        control_mask: ref_para1.control_mask,     // 0
        para_shape_id: 0,
        style_id: 0,
        raw_break_type: ref_para1.raw_break_type,
        char_shapes: vec![CharShapeRef {
            start_pos: 0,
            char_shape_id: 0,
        }],
        line_segs: vec![LineSeg {
            text_start: ref_para1.line_segs[0].text_start,
            vertical_pos: ref_para1.line_segs[0].vertical_pos, // 16448
            line_height: ref_para1.line_segs[0].line_height,
            text_height: ref_para1.line_segs[0].text_height,
            baseline_distance: ref_para1.line_segs[0].baseline_distance,
            line_spacing: ref_para1.line_segs[0].line_spacing,
            column_start: ref_para1.line_segs[0].column_start,
            segment_width: ref_para1.line_segs[0].segment_width,
            tag: ref_para1.line_segs[0].tag,
        }],
        has_para_text: false,
        raw_header_extra: ref_para1.raw_header_extra.clone(),
        ..Default::default()
    };
    doc.document.sections[0].paragraphs.push(empty_para);

    // 11. raw_stream 무효화 (재직렬화)
    doc.document.sections[0].raw_stream = None;
    doc.document.doc_info.raw_stream = None;
    doc.document.doc_properties.raw_data = None;

    // 캐럿 위치 (참조: list_id=0, para_id=1, char_pos=0)
    doc.document.doc_properties.caret_list_id = ref_doc.document.doc_properties.caret_list_id;
    doc.document.doc_properties.caret_para_id = ref_doc.document.doc_properties.caret_para_id;
    doc.document.doc_properties.caret_char_pos = ref_doc.document.doc_properties.caret_char_pos;

    let para = &doc.document.sections[0].paragraphs[0];
    eprintln!(
        "  구성 문단[0]: cc={} ctrls={} mask=0x{:08X} seg_w={}",
        para.char_count,
        para.controls.len(),
        para.control_mask,
        para.line_segs
            .first()
            .map(|ls| ls.segment_width)
            .unwrap_or(-1)
    );
    let para1 = &doc.document.sections[0].paragraphs[1];
    eprintln!(
        "  구성 문단[1]: cc={} vpos={}",
        para1.char_count,
        para1
            .line_segs
            .first()
            .map(|ls| ls.vertical_pos)
            .unwrap_or(-1)
    );

    // 12. 저장
    let saved = doc.export_hwp_native();
    assert!(saved.is_ok(), "HWP 저장 실패: {:?}", saved.err());
    let saved_data = saved.unwrap();

    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/save_test_pic_in_table.hwp", &saved_data).unwrap();
    eprintln!(
        "  저장: output/save_test_pic_in_table.hwp ({} bytes)",
        saved_data.len()
    );

    // 13. 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&saved_data);
    assert!(doc2.is_ok(), "재파싱 실패: {:?}", doc2.err());
    let doc2 = doc2.unwrap();

    // 표 컨트롤 존재 검증
    let para2 = &doc2.document.sections[0].paragraphs[0];
    let table_found = para2
        .controls
        .iter()
        .any(|c| matches!(c, Control::Table(_)));
    assert!(table_found, "재파싱된 문서에 표 컨트롤이 없음");

    // 표 안 이미지 보존 검증
    let mut pic_in_cell = false;
    if let Some(Control::Table(t)) = para2
        .controls
        .iter()
        .find(|c| matches!(c, Control::Table(_)))
    {
        eprintln!(
            "  재파싱 표: {}×{} cells={}",
            t.row_count,
            t.col_count,
            t.cells.len()
        );
        assert_eq!(t.row_count, 1);
        assert_eq!(t.col_count, 1);
        assert_eq!(t.cells.len(), 1);

        for cp in &t.cells[0].paragraphs {
            for cc in &cp.controls {
                if let Control::Picture(p) = cc {
                    pic_in_cell = true;
                    eprintln!(
                        "  셀 안 Picture: {}×{} bid={}",
                        p.common.width, p.common.height, p.image_attr.bin_data_id
                    );
                    assert_eq!(p.image_attr.bin_data_id, ref_pic.image_attr.bin_data_id);
                    assert_eq!(p.common.width, ref_pic.common.width);
                    assert_eq!(p.common.height, ref_pic.common.height);
                }
            }
        }
    }
    assert!(pic_in_cell, "재파싱 후 표 안 이미지가 없음");

    // BinData 검증
    assert_eq!(
        doc2.document.doc_info.bin_data_list.len(),
        1,
        "BinData 없음"
    );
    assert_eq!(
        doc2.document.doc_info.bin_data_list[0].data_type,
        BinDataType::Embedding
    );
    assert_eq!(
        doc2.document.bin_data_content.len(),
        1,
        "BinDataContent 없음"
    );
    assert_eq!(
        doc2.document.bin_data_content[0].data.len(),
        ref_bincontent.data.len(),
        "이미지 데이터 크기 불일치"
    );

    // 두 번째 문단 검증
    assert!(
        doc2.document.sections[0].paragraphs.len() >= 2,
        "표 아래 빈 문단이 없음"
    );

    // 캐럿 위치 검증
    eprintln!(
        "  캐럿: list_id={} para_id={} char_pos={}",
        doc2.document.doc_properties.caret_list_id,
        doc2.document.doc_properties.caret_para_id,
        doc2.document.doc_properties.caret_char_pos
    );

    // 14. 참조 파일과 레코드 비교
    let saved_parsed = crate::parser::parse_hwp(&saved_data).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_parsed.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    let ref_parsed = crate::parser::parse_hwp(&ref_data).unwrap();
    let mut ref_cfb = crate::parser::cfb_reader::CfbReader::open(&ref_data).unwrap();
    let ref_bt = ref_cfb
        .read_body_text_section(0, ref_parsed.header.compressed, false)
        .unwrap();
    let ref_recs = Record::read_all(&ref_bt).unwrap();

    eprintln!(
        "\n  --- 레코드 비교 (참조={} 개, 저장={} 개) ---",
        ref_recs.len(),
        saved_recs.len()
    );
    let max_recs = ref_recs.len().max(saved_recs.len());
    let mut diff_count = 0;
    for i in 0..max_recs {
        let ref_info = if i < ref_recs.len() {
            let r = &ref_recs[i];
            format!(
                "tag={:3}({:22}) lv={} sz={}",
                r.tag_id,
                tags::tag_name(r.tag_id),
                r.level,
                r.data.len()
            )
        } else {
            "---".to_string()
        };
        let saved_info = if i < saved_recs.len() {
            let r = &saved_recs[i];
            format!(
                "tag={:3}({:22}) lv={} sz={}",
                r.tag_id,
                tags::tag_name(r.tag_id),
                r.level,
                r.data.len()
            )
        } else {
            "---".to_string()
        };
        let match_mark = if i < ref_recs.len() && i < saved_recs.len() {
            let r = &ref_recs[i];
            let s = &saved_recs[i];
            if r.tag_id == s.tag_id && r.level == s.level && r.data == s.data {
                "=="
            } else if r.tag_id == s.tag_id && r.level == s.level {
                "~="
            } else {
                "!="
            }
        } else {
            "!="
        };
        if match_mark != "==" {
            diff_count += 1;
        }
        eprintln!("  [{:2}] {} {} | {}", i, match_mark, ref_info, saved_info);
    }

    // 차이 상세
    for i in 0..ref_recs.len().min(saved_recs.len()) {
        let r = &ref_recs[i];
        let s = &saved_recs[i];
        if r.tag_id == s.tag_id && r.data != s.data {
            eprintln!("\n  [차이 상세] 레코드 {}: {}", i, tags::tag_name(r.tag_id));
            let max_show = r.data.len().max(s.data.len()).min(120);
            eprintln!("    참조: {:02x?}", &r.data[..r.data.len().min(max_show)]);
            eprintln!("    저장: {:02x?}", &s.data[..s.data.len().min(max_show)]);
            for j in 0..r.data.len().min(s.data.len()) {
                if r.data[j] != s.data[j] {
                    eprintln!(
                        "    첫 차이: offset {} (참조=0x{:02x}, 저장=0x{:02x})",
                        j, r.data[j], s.data[j]
                    );
                    break;
                }
            }
        }
    }

    eprintln!("  일치: {}/{} 레코드", max_recs - diff_count, max_recs);

    // CFB 스트림 확인
    let streams = saved_cfb.list_streams();
    let has_bindata = streams
        .iter()
        .any(|s| s.contains("BinData") || s.contains("BIN"));
    assert!(has_bindata, "BinData 스트림이 없음");

    eprintln!("\n=== 표 안 이미지 저장 검증 완료 ===");
}



/// 타스크 41 단계 1: 기존 HWP에 프로그래밍 방식으로 2×2 표 삽입 → 저장
/// 직렬화 코드 자체의 정상 동작을 먼저 확인
#[test]
fn test_inject_table_into_existing() {
    use crate::model::control::Control;
    use crate::model::paragraph::{CharShapeRef, LineSeg, Paragraph};
    use crate::model::table::{Cell, Table};
    use crate::model::Padding;

    let path = "samples/20250130-hongbo.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  타스크 41 단계 1: 기존 HWP에 2×2 표 삽입");
    eprintln!("{}", "=".repeat(60));

    let orig_data = std::fs::read(path).unwrap();

    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    let sec = &doc.document.sections[0];
    let orig_para_count = sec.paragraphs.len();
    eprintln!(
        "  원본: {} 문단, {} 컨트롤",
        orig_para_count,
        sec.paragraphs
            .iter()
            .map(|p| p.controls.len())
            .sum::<usize>()
    );

    // 캐럿 위치 확인
    let caret_list_id = doc.document.doc_properties.caret_list_id;
    let caret_para_id = doc.document.doc_properties.caret_para_id;
    let caret_char_pos = doc.document.doc_properties.caret_char_pos;
    eprintln!(
        "  캐럿 위치: list_id={}, para_id={}, char_pos={}",
        caret_list_id, caret_para_id, caret_char_pos
    );

    // 삽입 위치: 캐럿이 가리키는 문단
    let insert_para_idx = caret_para_id as usize;
    assert!(
        insert_para_idx < orig_para_count,
        "캐럿 para_id({})가 문단 범위({})를 초과",
        insert_para_idx,
        orig_para_count
    );
    eprintln!("  삽입 위치: 문단[{}] (캐럿 기반)", insert_para_idx);

    // 삽입 위치 근처 문단 구조 출력
    let start = if insert_para_idx > 2 {
        insert_para_idx - 2
    } else {
        0
    };
    let end = (insert_para_idx + 4).min(orig_para_count);
    for i in start..end {
        let p = &sec.paragraphs[i];
        let ctrl_types: Vec<&str> = p
            .controls
            .iter()
            .map(|c| match c {
                Control::Table(_) => "Table",
                Control::Picture(_) => "Picture",
                _ => "Other",
            })
            .collect();
        let marker = if i == insert_para_idx {
            " ← 캐럿"
        } else {
            ""
        };
        eprintln!(
            "    문단[{}]: cc={} mask=0x{:08X} text='{}' ctrls={:?}{}",
            i,
            p.char_count,
            p.control_mask,
            if p.text.len() > 30 {
                &p.text[..30]
            } else {
                &p.text
            },
            ctrl_types,
            marker
        );
    }

    // === 방법: 기존 표 문단을 복제하여 삽입 (직렬화 문제 격리) ===
    // 문단[2]의 표를 그대로 복제
    let source_para_idx = 2;
    let table_para = doc.document.sections[0].paragraphs[source_para_idx].clone();
    eprintln!(
        "  복제 원본: 문단[{}] cc={} controls={}",
        source_para_idx,
        table_para.char_count,
        table_para.controls.len()
    );
    if let Some(Control::Table(t)) = table_para.controls.first() {
        eprintln!(
            "    표: {}×{} cells={} attr=0x{:08X}",
            t.row_count,
            t.col_count,
            t.cells.len(),
            t.attr
        );
    }

    // 캐럿 위치 뒤에 표 문단 삽입
    doc.document.sections[0]
        .paragraphs
        .insert(insert_para_idx + 1, table_para);

    // 기존 콘텐츠 사이 삽입 → 빈 문단 불필요 (기존 문단이 이어짐)

    // raw_stream 무효화: 섹션만 (DocInfo raw 유지 → 손상 방지)
    doc.document.sections[0].raw_stream = None;

    eprintln!(
        "  수정: {} 문단 (원본 {} + 표 문단 1개)",
        doc.document.sections[0].paragraphs.len(),
        orig_para_count
    );

    // 저장
    let saved = doc.export_hwp_native();
    assert!(saved.is_ok(), "HWP 저장 실패: {:?}", saved.err());
    let saved_data = saved.unwrap();

    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/save_test_table_inject.hwp", &saved_data).unwrap();
    eprintln!(
        "  저장: output/save_test_table_inject.hwp ({} bytes)",
        saved_data.len()
    );

    // 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&saved_data);
    assert!(doc2.is_ok(), "재파싱 실패: {:?}", doc2.err());
    let doc2 = doc2.unwrap();

    // 문단 수 검증 (표 문단 = +1)
    let new_para_count = doc2.document.sections[0].paragraphs.len();
    eprintln!("  재파싱: {} 문단", new_para_count);
    assert_eq!(new_para_count, orig_para_count + 1, "문단 수 불일치");

    // 삽입된 표 검증 (캐럿 문단 다음 위치)
    let table_para_idx = insert_para_idx + 1;
    let injected = &doc2.document.sections[0].paragraphs[table_para_idx];
    let table_found = injected
        .controls
        .iter()
        .any(|c| matches!(c, Control::Table(_)));
    assert!(
        table_found,
        "삽입된 표 컨트롤이 없음 (문단[{}])",
        table_para_idx
    );

    if let Some(Control::Table(t)) = injected
        .controls
        .iter()
        .find(|c| matches!(c, Control::Table(_)))
    {
        eprintln!(
            "  복제 표: {}×{} cells={} attr=0x{:08X}",
            t.row_count,
            t.col_count,
            t.cells.len(),
            t.attr
        );
    }

    // 기존 컨트롤 보존 검증
    let orig_doc = HwpDocument::from_bytes(&orig_data).unwrap();
    let mut orig_tables = 0;
    let mut orig_pics = 0;
    for para in &orig_doc.document.sections[0].paragraphs {
        for ctrl in &para.controls {
            match ctrl {
                Control::Table(_) => orig_tables += 1,
                Control::Picture(_) => orig_pics += 1,
                _ => {}
            }
        }
    }
    let mut new_tables = 0;
    let mut new_pics = 0;
    for para in &doc2.document.sections[0].paragraphs {
        for ctrl in &para.controls {
            match ctrl {
                Control::Table(_) => new_tables += 1,
                Control::Picture(_) => new_pics += 1,
                _ => {}
            }
        }
    }
    eprintln!(
        "  컨트롤 보존: Table {}→{}, Picture {}→{}",
        orig_tables, new_tables, orig_pics, new_pics
    );
    assert_eq!(new_tables, orig_tables + 1, "표 개수 불일치");
    assert_eq!(new_pics, orig_pics, "이미지 개수 변경됨");

    eprintln!("\n=== 타스크 41 단계 1 완료 ===");

    // === 진단: 저장된 파일에서 삽입된 표 제거 후 재저장 ===
    eprintln!("\n  [진단] 표 제거 후 재저장...");
    let mut doc3 = HwpDocument::from_bytes(&saved_data).unwrap();
    let para_count_before = doc3.document.sections[0].paragraphs.len();
    // 삽입된 표 문단 제거 (index = insert_para_idx + 1 = 9)
    doc3.document.sections[0]
        .paragraphs
        .remove(insert_para_idx + 1);
    doc3.document.sections[0].raw_stream = None;
    let saved3 = doc3.export_hwp_native().unwrap();
    std::fs::write("output/save_test_table_removed.hwp", &saved3).unwrap();
    eprintln!(
        "  [진단] 표 제거: {} → {} 문단, output/save_test_table_removed.hwp ({} bytes)",
        para_count_before,
        doc3.document.sections[0].paragraphs.len(),
        saved3.len()
    );
}



/// 진단: 복제 표 vs parse_table_html 표의 raw_ctrl_data 및 직렬화 바이트 비교
#[test]
fn test_diag_clone_vs_parsed_table() {
    use crate::model::control::Control;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let path = "samples/20250130-hongbo.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  진단: 복제 표 vs parse_table_html 표 비교");
    eprintln!("{}", "=".repeat(60));

    let orig_data = std::fs::read(path).unwrap();

    // === A: 복제 표 (정상 동작) ===
    let doc_a = HwpDocument::from_bytes(&orig_data).unwrap();
    let clone_para = doc_a.document.sections[0].paragraphs[2].clone();

    // === B: parse_table_html 표 (내용 사라짐) ===
    let mut doc_b = HwpDocument::from_bytes(&orig_data).unwrap();
    let table_html = r#"<table><tr><td style="border:1px solid black;">테스트A</td><td style="border:1px solid black;">&nbsp;</td></tr><tr><td style="border:1px solid black;">&nbsp;</td><td style="border:1px solid black;">테스트D</td></tr></table>"#;
    let mut parsed_paras = Vec::new();
    doc_b.parse_table_html(&mut parsed_paras, table_html);
    let parsed_para = &parsed_paras[0];

    // 문단 헤더 비교
    eprintln!("\n  [문단 헤더 비교]");
    eprintln!(
        "  복제: cc={} msb={} cm=0x{:08X} ps={} sid={} rhe={:02x?}",
        clone_para.char_count,
        clone_para.char_count_msb,
        clone_para.control_mask,
        clone_para.para_shape_id,
        clone_para.style_id,
        &clone_para.raw_header_extra
    );
    eprintln!(
        "  생성: cc={} msb={} cm=0x{:08X} ps={} sid={} rhe={:02x?}",
        parsed_para.char_count,
        parsed_para.char_count_msb,
        parsed_para.control_mask,
        parsed_para.para_shape_id,
        parsed_para.style_id,
        &parsed_para.raw_header_extra
    );

    // raw_ctrl_data 비교
    if let Some(Control::Table(ref t_a)) = clone_para.controls.first() {
        if let Some(Control::Table(ref t_b)) = parsed_para.controls.first() {
            eprintln!("\n  [table.attr 비교]");
            eprintln!("  복제: attr=0x{:08X}", t_a.attr);
            eprintln!("  생성: attr=0x{:08X}", t_b.attr);

            eprintln!("\n  [raw_ctrl_data 비교] (CommonObjAttr after attr)");
            eprintln!(
                "  복제 ({} bytes): {:02x?}",
                t_a.raw_ctrl_data.len(),
                &t_a.raw_ctrl_data
            );
            eprintln!(
                "  생성 ({} bytes): {:02x?}",
                t_b.raw_ctrl_data.len(),
                &t_b.raw_ctrl_data
            );

            // 필드별 해석
            fn read_i32(d: &[u8], o: usize) -> i32 {
                if o + 4 <= d.len() {
                    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
                } else {
                    0
                }
            }
            fn read_u32(d: &[u8], o: usize) -> u32 {
                if o + 4 <= d.len() {
                    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
                } else {
                    0
                }
            }
            fn read_i16(d: &[u8], o: usize) -> i16 {
                if o + 2 <= d.len() {
                    i16::from_le_bytes([d[o], d[o + 1]])
                } else {
                    0
                }
            }
            fn read_u16(d: &[u8], o: usize) -> u16 {
                if o + 2 <= d.len() {
                    u16::from_le_bytes([d[o], d[o + 1]])
                } else {
                    0
                }
            }

            for (label, d) in [
                ("복제", t_a.raw_ctrl_data.as_slice()),
                ("생성", t_b.raw_ctrl_data.as_slice()),
            ] {
                eprintln!("\n  [{}] CommonObjAttr 필드:", label);
                eprintln!("    [0..4] vert_offset  = {}", read_i32(d, 0));
                eprintln!("    [4..8] horz_offset  = {}", read_i32(d, 4));
                eprintln!("    [8..12] width       = {}", read_u32(d, 8));
                eprintln!("    [12..16] height     = {}", read_u32(d, 12));
                eprintln!("    [16..20] z_order    = {}", read_i32(d, 16));
                eprintln!("    [20..22] margin_l   = {}", read_i16(d, 20));
                eprintln!("    [22..24] margin_r   = {}", read_i16(d, 22));
                eprintln!("    [24..26] margin_t   = {}", read_i16(d, 24));
                eprintln!("    [26..28] margin_b   = {}", read_i16(d, 26));
                eprintln!("    [28..32] inst_id    = 0x{:08X}", read_u32(d, 28));
                eprintln!("    [32..34] desc_len   = {}", read_u16(d, 32));
                if d.len() > 34 {
                    eprintln!("    [34..] extra        = {:02x?}", &d[34..]);
                }
            }

            // 직렬화 바이트 비교 (CTRL_HEADER + TABLE + cells)
            eprintln!("\n  [직렬화 레코드 비교]");
            let mut recs_a: Vec<Record> = Vec::new();
            crate::serializer::control::serialize_control(
                &clone_para.controls[0],
                1,
                None,
                &mut recs_a,
            );
            let mut recs_b: Vec<Record> = Vec::new();
            crate::serializer::control::serialize_control(
                &parsed_para.controls[0],
                1,
                None,
                &mut recs_b,
            );

            eprintln!(
                "  복제: {} 레코드, 생성: {} 레코드",
                recs_a.len(),
                recs_b.len()
            );

            // 처음 5개 레코드 비교
            let max_show = recs_a.len().max(recs_b.len()).min(10);
            for i in 0..max_show {
                let a_info = if i < recs_a.len() {
                    format!(
                        "{:22} lv={} sz={}",
                        tags::tag_name(recs_a[i].tag_id),
                        recs_a[i].level,
                        recs_a[i].data.len()
                    )
                } else {
                    "---".to_string()
                };
                let b_info = if i < recs_b.len() {
                    format!(
                        "{:22} lv={} sz={}",
                        tags::tag_name(recs_b[i].tag_id),
                        recs_b[i].level,
                        recs_b[i].data.len()
                    )
                } else {
                    "---".to_string()
                };
                let status = if i < recs_a.len() && i < recs_b.len() {
                    if recs_a[i].tag_id == recs_b[i].tag_id && recs_a[i].data == recs_b[i].data {
                        "=="
                    } else if recs_a[i].tag_id == recs_b[i].tag_id {
                        "~="
                    } else {
                        "!="
                    }
                } else {
                    "!="
                };
                eprintln!("  [{:2}] {} | {} | {}", i, status, a_info, b_info);

                // 데이터 차이 상세
                if i < recs_a.len()
                    && i < recs_b.len()
                    && recs_a[i].tag_id == recs_b[i].tag_id
                    && recs_a[i].data != recs_b[i].data
                {
                    let max_d = recs_a[i].data.len().max(recs_b[i].data.len()).min(80);
                    eprintln!(
                        "       복제: {:02x?}",
                        &recs_a[i].data[..recs_a[i].data.len().min(max_d)]
                    );
                    eprintln!(
                        "       생성: {:02x?}",
                        &recs_b[i].data[..recs_b[i].data.len().min(max_d)]
                    );
                }
            }

            // raw_table_record_attr 비교
            eprintln!("\n  [TABLE record attr 비교]");
            eprintln!("  복제: tbl_rec_attr=0x{:08X}", t_a.raw_table_record_attr);
            eprintln!("  생성: tbl_rec_attr=0x{:08X}", t_b.raw_table_record_attr);
        }
    }

    // === 전체 문단 직렬화 비교 (PARA_HEADER + PARA_TEXT + ... + CTRL_HEADER + ...) ===
    eprintln!("\n  [전체 문단 직렬화 비교]");
    let mut full_recs_a: Vec<Record> = Vec::new();
    crate::serializer::body_text::serialize_paragraph_list(
        std::slice::from_ref(&clone_para),
        0,
        &mut full_recs_a,
    );
    let mut full_recs_b: Vec<Record> = Vec::new();
    crate::serializer::body_text::serialize_paragraph_list(
        std::slice::from_ref(parsed_para),
        0,
        &mut full_recs_b,
    );

    eprintln!(
        "  복제 전체: {} 레코드, 생성 전체: {} 레코드",
        full_recs_a.len(),
        full_recs_b.len()
    );
    let max = full_recs_a.len().max(full_recs_b.len());
    for i in 0..max {
        let a = full_recs_a.get(i);
        let b = full_recs_b.get(i);
        let a_info = a
            .map(|r| {
                format!(
                    "{:22} lv={} sz={}",
                    tags::tag_name(r.tag_id),
                    r.level,
                    r.data.len()
                )
            })
            .unwrap_or_else(|| "---".to_string());
        let b_info = b
            .map(|r| {
                format!(
                    "{:22} lv={} sz={}",
                    tags::tag_name(r.tag_id),
                    r.level,
                    r.data.len()
                )
            })
            .unwrap_or_else(|| "---".to_string());
        let status = match (a, b) {
            (Some(ra), Some(rb)) if ra.tag_id == rb.tag_id && ra.data == rb.data => "==",
            (Some(ra), Some(rb)) if ra.tag_id == rb.tag_id => "~=",
            _ => "!!",
        };
        eprintln!("  [{:2}] {} | {} | {}", i, status, a_info, b_info);

        // PARA_HEADER와 PARA_TEXT 데이터 상세
        if let (Some(ra), Some(rb)) = (a, b) {
            if ra.tag_id == rb.tag_id && ra.data != rb.data {
                if ra.tag_id == tags::HWPTAG_PARA_HEADER || ra.tag_id == tags::HWPTAG_PARA_TEXT {
                    eprintln!("       복제: {:02x?}", &ra.data[..ra.data.len().min(60)]);
                    eprintln!("       생성: {:02x?}", &rb.data[..rb.data.len().min(60)]);
                }
            }
        }
    }

    eprintln!("\n=== 진단 완료 ===");
}



/// 타스크 41 단계 3: parse_table_html()로 생성한 표를 기존 문서에 삽입 → 저장 → 검증
/// DIFF-1~8 수정 사항이 모두 반영된 통합 테스트
#[test]
fn test_parse_table_html_save() {
    use crate::model::control::Control;

    let path = "samples/20250130-hongbo.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  타스크 41 단계 3: parse_table_html 표 삽입 저장 검증");
    eprintln!("{}", "=".repeat(60));

    let orig_data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    let orig_para_count = doc.document.sections[0].paragraphs.len();
    let caret_para_id = doc.document.doc_properties.caret_para_id as usize;
    eprintln!(
        "  원본: {} 문단, 캐럿 위치: 문단[{}]",
        orig_para_count, caret_para_id
    );

    // HTML 표 생성 (2×2, 빈 셀 포함)
    let table_html = r#"<table style="border-collapse:collapse;">
            <tr>
                <td style="border:1px solid black; padding:5px; width:200px;">테스트 셀 A</td>
                <td style="border:1px solid black; padding:5px; width:200px;">&nbsp;</td>
            </tr>
            <tr>
                <td style="border:1px solid black; padding:5px;">&nbsp;&nbsp;</td>
                <td style="border:1px solid black; padding:5px;">테스트 셀 D</td>
            </tr>
        </table>"#;

    // parse_table_html으로 표 문단 생성
    let mut table_paragraphs = Vec::new();
    doc.parse_table_html(&mut table_paragraphs, table_html);
    assert_eq!(table_paragraphs.len(), 1, "표 문단 1개 생성");

    let table_para = &table_paragraphs[0];
    eprintln!(
        "  표 문단: cc={} msb={} cm=0x{:08X} cs={} ls={}",
        table_para.char_count,
        table_para.char_count_msb,
        table_para.control_mask,
        table_para.char_shapes.len(),
        table_para.line_segs.len()
    );

    // DIFF 검증
    if let Some(Control::Table(ref tbl)) = table_para.controls.first() {
        eprintln!(
            "  표: {}×{} cells={} attr=0x{:08X}",
            tbl.row_count,
            tbl.col_count,
            tbl.cells.len(),
            tbl.attr
        );
        eprintln!("  DIFF-5: tbl_rec_attr=0x{:08X}", tbl.raw_table_record_attr);
        assert_eq!(
            tbl.raw_table_record_attr, 0x04000006,
            "DIFF-5: 셀분리금지 항상 설정"
        );

        // DIFF-7: instance_id
        let inst = parse_common_obj_attr(&tbl.raw_ctrl_data).instance_id;
        eprintln!("  DIFF-7: instance_id=0x{:08X}", inst);
        assert_ne!(inst, 0, "DIFF-7: instance_id != 0");

        // DIFF-1: 빈 셀 검증
        for (i, cell) in tbl.cells.iter().enumerate() {
            let p = &cell.paragraphs[0];
            eprintln!(
                "  셀[{}]({},{}): cc={} text='{}' cs={} ls={} has_pt={}",
                i,
                cell.row,
                cell.col,
                p.char_count,
                if p.text.len() > 20 {
                    &p.text[..20]
                } else {
                    &p.text
                },
                p.char_shapes.len(),
                p.line_segs.len(),
                p.has_para_text
            );

            // DIFF-2: 모든 셀 문단은 char_shapes가 있어야 함
            assert!(
                !p.char_shapes.is_empty(),
                "DIFF-2: 셀[{}] char_shapes 비어있음",
                i
            );
            // DIFF-3: para_shape_id=0 (기본 본문 스타일)
            assert_eq!(p.para_shape_id, 0, "DIFF-3: 셀[{}] para_shape_id=0", i);
            // DIFF-6: line_segs의 tag
            if !p.line_segs.is_empty() {
                assert_eq!(
                    p.line_segs[0].tag,
                    LineSeg::TAG_SINGLE_SEGMENT_LINE,
                    "DIFF-6: 셀[{}] line_seg tag",
                    i
                );
                assert!(
                    p.line_segs[0].segment_width > 0,
                    "DIFF-6: 셀[{}] seg_width > 0",
                    i
                );
            }
        }

        // DIFF-1: 빈 셀 (셀[1], 셀[2]) 확인
        assert_eq!(
            tbl.cells[1].paragraphs[0].char_count, 1,
            "DIFF-1: 빈 셀[1] cc=1"
        );
        assert!(
            tbl.cells[1].paragraphs[0].text.is_empty(),
            "DIFF-1: 빈 셀[1] text empty"
        );
        assert_eq!(
            tbl.cells[2].paragraphs[0].char_count, 1,
            "DIFF-1: 빈 셀[2] cc=1"
        );
    }

    // DIFF-8: 표 컨테이너 문단 LineSeg
    assert!(
        !table_para.line_segs.is_empty(),
        "DIFF-8: 표 문단 line_segs 비어있음"
    );
    eprintln!(
        "  DIFF-8: line_seg h={} tw={} seg_w={} tag=0x{:08X}",
        table_para.line_segs[0].line_height,
        table_para.line_segs[0].text_height,
        table_para.line_segs[0].segment_width,
        table_para.line_segs[0].tag
    );
    assert!(
        table_para.line_segs[0].line_height > 0,
        "DIFF-8: line_height > 0"
    );
    assert!(
        table_para.line_segs[0].segment_width > 0,
        "DIFF-8: seg_width > 0"
    );
    assert_eq!(
        table_para.line_segs[0].tag,
        LineSeg::TAG_SINGLE_SEGMENT_LINE,
        "DIFF-8: tag=LineSeg::TAG_SINGLE_SEGMENT_LINE"
    );

    // 삽입 및 저장
    doc.document.sections[0]
        .paragraphs
        .insert(caret_para_id + 1, table_paragraphs.remove(0));
    doc.document.sections[0].raw_stream = None;

    let saved = doc.export_hwp_native();
    assert!(saved.is_ok(), "저장 실패: {:?}", saved.err());
    let saved_data = saved.unwrap();

    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/save_test_parsed_table.hwp", &saved_data).unwrap();
    eprintln!(
        "  저장: output/save_test_parsed_table.hwp ({} bytes)",
        saved_data.len()
    );

    // 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&saved_data);
    assert!(doc2.is_ok(), "재파싱 실패: {:?}", doc2.err());
    let doc2 = doc2.unwrap();
    let new_para_count = doc2.document.sections[0].paragraphs.len();
    eprintln!(
        "  재파싱: {} 문단 (원본 {} + 1)",
        new_para_count, orig_para_count
    );
    assert_eq!(new_para_count, orig_para_count + 1);

    // 삽입된 표 확인
    let injected = &doc2.document.sections[0].paragraphs[caret_para_id + 1];
    assert!(
        injected
            .controls
            .iter()
            .any(|c| matches!(c, Control::Table(_))),
        "삽입된 표 컨트롤 없음"
    );

    eprintln!("\n=== 타스크 41 단계 3 완료 ===");
    eprintln!("  output/save_test_parsed_table.hwp 를 HWP 프로그램에서 확인해 주세요");
}



/// 표 바운딩박스 조회 테스트
#[test]
fn test_get_table_bbox() {
    use std::path::Path;

    let path = Path::new("samples/hwp_table_test.hwp");
    if !path.exists() {
        eprintln!("hwp_table_test.hwp 없음 — 건너뜀");
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    let result = doc.get_table_bbox_native(0, 3, 0);
    assert!(result.is_ok(), "표 bbox 조회 실패: {:?}", result.err());

    let json = result.unwrap();
    assert!(json.contains("pageIndex"), "pageIndex 필드 존재 확인");
    assert!(json.contains("width"), "width 필드 존재 확인");
    assert!(json.contains("height"), "height 필드 존재 확인");
    eprintln!("표 bbox: {}", json);
}



/// #2400: page-local pointer 좌표는 같은 page 의 표 fragment bbox와 비교해야 한다.
#[test]
fn test_get_table_bbox_at_page_for_giant_multi_page_cell() {
    use std::path::Path;

    for path in [
        "rhwp-studio/public/samples/issue1949_giant_cell_nested_tables_perf.hwp",
        "samples/issue1949_giant_cell_nested_tables_perf.hwpx",
    ] {
        let data = std::fs::read(Path::new(path)).expect("#2400 권위 샘플 읽기");
        let doc = HwpDocument::from_bytes(&data).expect("#2400 권위 샘플 파싱");
        assert_eq!(doc.page_count(), 115, "{path}: page count");

        let legacy: Value = serde_json::from_str(
            &doc.get_table_bbox_native(0, 0, 2)
                .expect("legacy 첫 fragment bbox"),
        )
        .expect("legacy bbox JSON");
        let current: Value = serde_json::from_str(
            &doc.get_table_bbox_at_page_native(0, 0, 2, 113)
                .expect("page 113 fragment bbox"),
        )
        .expect("page-scoped bbox JSON");

        assert_eq!(legacy["pageIndex"].as_u64(), Some(0), "{path}: legacy page");
        assert_eq!(
            current["pageIndex"].as_u64(),
            Some(113),
            "{path}: current fragment page"
        );

        let click_y = 1057.3;
        let legacy_bottom = legacy["y"].as_f64().unwrap() + legacy["height"].as_f64().unwrap();
        let current_bottom = current["y"].as_f64().unwrap() + current["height"].as_f64().unwrap();
        assert!(
            (click_y - legacy_bottom).abs() <= 5.0,
            "{path}: 재현점은 첫 fragment 하단에 잘못 걸리는 전제"
        );
        assert!(
            (click_y - current_bottom).abs() > 5.0,
            "{path}: 현재 fragment에서는 실제 경계가 아님"
        );

        assert!(
            doc.get_table_bbox_at_page_native(0, 0, 2, 115).is_err(),
            "{path}: 범위 밖 page가 첫 fragment로 fallback하면 안 됨"
        );
    }
}



/// 표 컨트롤 삭제 테스트 (wasm_api 내부 접근)
#[test]
fn test_delete_table_control() {
    use std::path::Path;

    let path = Path::new("samples/hwp_table_test.hwp");
    if !path.exists() {
        eprintln!("hwp_table_test.hwp 없음 — 건너뜀");
        return;
    }

    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();
    let _ = doc.convert_to_editable_native();

    // 삭제 전 컨트롤 수 확인
    let before_count = doc.document.sections[0].paragraphs[3].controls.len();
    assert!(before_count > 0, "테스트 파일에 표가 없음");

    // 삭제 전 char_count
    let before_char_count = doc.document.sections[0].paragraphs[3].char_count;
    let before_next_vpos = doc.document.sections[0]
        .paragraphs
        .get(4)
        .and_then(|p| p.line_segs.first())
        .map(|ls| ls.vertical_pos);

    // 표 bbox 조회 성공 확인
    let bbox_result = doc.get_table_bbox_native(0, 3, 0);
    assert!(bbox_result.is_ok(), "삭제 전 bbox 조회 실패");

    // 표 삭제
    let result = doc.delete_table_control_native(0, 3, 0);
    assert!(result.is_ok(), "표 삭제 실패: {:?}", result.err());

    // 삭제 후 컨트롤 수 감소 확인
    let after_count = doc.document.sections[0].paragraphs[3].controls.len();
    assert_eq!(after_count, before_count - 1, "컨트롤 수 감소 확인");

    // char_count가 8 감소했는지 확인
    let after_char_count = doc.document.sections[0].paragraphs[3].char_count;
    assert_eq!(
        after_char_count,
        before_char_count - 8,
        "char_count 8 감소 확인"
    );

    if let (Some(before), Some(after)) = (
        before_next_vpos,
        doc.document.sections[0]
            .paragraphs
            .get(4)
            .and_then(|p| p.line_segs.first())
            .map(|ls| ls.vertical_pos),
    ) {
        assert!(
            after < before,
            "표 삭제 후 다음 문단 vpos가 위로 당겨져야 함: before={}, after={}",
            before,
            after
        );
    }

    eprintln!(
        "표 삭제: 컨트롤 {}→{}, char_count {}→{}",
        before_count, after_count, before_char_count, after_char_count
    );
}



#[test]
/// B6: 표 구조 변경 후 저장 시 빈 셀 문단의 PARA_TEXT/char_count/LineSeg 검증
fn test_table_modification_empty_cell_serialization() {
    use crate::parser::record::Record;
    use std::path::Path;

    let path = Path::new("samples/hwp_table_test.hwp");
    if !path.exists() {
        eprintln!("hwp_table_test.hwp 없음 — 건너뜀");
        return;
    }

    let data = std::fs::read(path).unwrap();

    // 행 추가 후 내보내기
    let mut doc = HwpDocument::from_bytes(&data).unwrap();
    doc.insert_table_row_native(0, 3, 0, 0, true).unwrap();
    let exported = doc.export_hwp_native().unwrap();

    // 재파싱
    let parsed = crate::parser::parse_hwp(&exported).unwrap();
    let mut cfb = crate::parser::cfb_reader::CfbReader::open(&exported).unwrap();
    let bt = cfb
        .read_body_text_section(0, parsed.header.compressed, false)
        .unwrap();
    let recs = Record::read_all(&bt).unwrap();

    // 표 범위 내 PARA_HEADER → PARA_TEXT 패턴 검사
    // cc=1인 문단(빈 셀)은 PARA_TEXT가 없어야 한다
    let mut empty_cell_count = 0;
    let mut violation_count = 0;

    for (i, rec) in recs.iter().enumerate() {
        if rec.tag_id == crate::parser::tags::HWPTAG_PARA_HEADER && rec.data.len() >= 4 {
            let cc_raw = u32::from_le_bytes(rec.data[0..4].try_into().unwrap());
            let cc = cc_raw & 0x7FFFFFFF;

            // 빈 문단 (cc == 0 또는 1)
            if cc <= 1 {
                empty_cell_count += 1;

                // 다음 레코드가 PARA_TEXT이면 안 됨
                if i + 1 < recs.len() && recs[i + 1].tag_id == crate::parser::tags::HWPTAG_PARA_TEXT
                {
                    violation_count += 1;
                    eprintln!(
                        "!! 위반: rec[{}] cc={} 다음에 PARA_TEXT({}B) 존재",
                        i,
                        cc,
                        recs[i + 1].data.len()
                    );
                }

                // cc=0이면 안 됨 (HWP 스펙: 최소 cc=1)
                if cc == 0 {
                    eprintln!("!! 위반: rec[{}] cc=0 (HWP 스펙 위반, 최소 1이어야 함)", i);
                    violation_count += 1;
                }

                // PARA_LINE_SEG가 존재해야 함 — PARA_CHAR_SHAPE 다음에
                let mut has_line_seg = false;
                for j in (i + 1)..recs.len() {
                    if recs[j].tag_id == crate::parser::tags::HWPTAG_PARA_HEADER
                        || recs[j].level <= rec.level
                    {
                        break;
                    }
                    if recs[j].tag_id == crate::parser::tags::HWPTAG_PARA_LINE_SEG {
                        has_line_seg = true;
                        break;
                    }
                }
                if !has_line_seg {
                    eprintln!("!! 위반: rec[{}] cc={} PARA_LINE_SEG 없음", i, cc);
                    violation_count += 1;
                }
            }
        }
    }

    eprintln!(
        "빈 문단 수: {}, 위반: {}",
        empty_cell_count, violation_count
    );
    assert!(
        empty_cell_count > 0,
        "빈 셀 문단이 없음 — 테스트 유효성 확인 필요"
    );
    assert_eq!(
        violation_count, 0,
        "빈 셀 문단 직렬화 위반이 {}건 발견됨",
        violation_count
    );
}



#[test]
fn test_find_next_editable_control_bookreview() {
    let data = std::fs::read("samples/basic/BookReview.hwp").expect("BookReview.hwp not found");
    let doc = HwpDocument::from_bytes(&data).unwrap();

    // Section 1, Para 0: controls 0-8 중 textbox는 ci=3,4,5,6,7,8
    // ci=3에서 앞으로 → ci=4 (textbox)
    let r = doc.find_next_editable_control_native(1, 0, 3, 1);
    println!("sec1 para0 ci=3 → next: {}", r);
    assert!(r.contains("\"type\":\"textbox\""));
    assert!(r.contains("\"ci\":4"));

    // ci=8에서 앞으로 → 같은 문단에 더 이상 없음 → 다음 문단/섹션
    let r = doc.find_next_editable_control_native(1, 0, 8, 1);
    println!("sec1 para0 ci=8 → next: {}", r);
    // section 1에 paragraph가 1개뿐이므로 다음 섹션도 없음 → none
    assert!(r.contains("\"type\":\"none\""));

    // ci=3에서 뒤로 → 같은 문단에 ci=3 이전 textbox 없음 → 이전 섹션
    let r = doc.find_next_editable_control_native(1, 0, 3, -1);
    println!("sec1 para0 ci=3 → prev: {}", r);
    // section 0의 마지막에서 편집 가능한 위치
    assert!(r.contains("\"sec\":0"));

    // Section 0에서 앞으로: section 0의 마지막 문단에서 section 1로 이동
    let sec0_paras = doc.core.document.sections[0].paragraphs.len();
    let r = doc.find_next_editable_control_native(0, sec0_paras - 1, -1, 1);
    println!("sec0 last_para body → next: {}", r);

    // ci=5에서 앞으로 → ci=6
    let r = doc.find_next_editable_control_native(1, 0, 5, 1);
    println!("sec1 para0 ci=5 → next: {}", r);
    assert!(r.contains("\"ci\":6"));

    // ci=6에서 앞으로 → ci=7
    let r = doc.find_next_editable_control_native(1, 0, 6, 1);
    println!("sec1 para0 ci=6 → next: {}", r);
    assert!(r.contains("\"ci\":7"));

    // ci=7에서 앞으로 → ci=8
    let r = doc.find_next_editable_control_native(1, 0, 7, 1);
    println!("sec1 para0 ci=7 → next: {}", r);
    assert!(r.contains("\"ci\":8"));

    // ci=8에서 뒤로 → ci=7
    let r = doc.find_next_editable_control_native(1, 0, 8, -1);
    println!("sec1 para0 ci=8 → prev: {}", r);
    assert!(r.contains("\"ci\":7"));
}



/// 12페이지 각 문단에서 엔터 후 13페이지 표 배치 검증
#[test]
fn test_page12_enter_table_placement_scan() {
    use crate::renderer::pagination::PageItem;

    // 12페이지의 각 문단 끝에서 엔터를 입력하는 시나리오
    for split_pi in [194, 196] {
        let bytes = std::fs::read("samples/kps-ai.hwp").expect("kps-ai.hwp 읽기 실패");
        let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
        doc.convert_to_editable_native().unwrap();
        doc.paginate();

        let text_len = doc.document.sections[0].paragraphs[split_pi]
            .text
            .chars()
            .count();
        let offset = text_len; // 문단 끝에서 분할

        eprintln!("\n=== split pi={} offset={} ===", split_pi, offset);

        // 분할 전 page 13 (idx=12) 확인
        let table_pi_before = 198; // 원래 pi=198의 표
        let p13_before = &doc.pagination[0].pages[12];
        let has_table_before = p13_before.column_contents[0].items.iter().any(
            |it| matches!(it, PageItem::Table { para_index, .. } if *para_index == table_pi_before),
        );
        eprintln!(
            "  before: pi={} table on page 13: {}",
            table_pi_before, has_table_before
        );

        let result = doc
            .split_paragraph_native(0, split_pi, offset, None)
            .unwrap();
        assert!(
            result.contains("\"ok\":true"),
            "split failed at pi={}: {}",
            split_pi,
            result
        );

        let pages_after = doc.pagination[0].pages.len();
        let table_pi_after = if split_pi < table_pi_before {
            table_pi_before + 1
        } else {
            table_pi_before
        };

        // 분할 후: 표가 어느 페이지에 있는지 탐색
        let mut table_page = None;
        for (pidx, page) in doc.pagination[0].pages.iter().enumerate() {
            for item in &page.column_contents[0].items {
                if matches!(item, PageItem::Table { para_index, .. } if *para_index == table_pi_after)
                {
                    table_page = Some(pidx);
                }
            }
        }
        eprintln!(
            "  after: pi={} table on page {} (total {})",
            table_pi_after,
            table_page.map(|p| p + 1).unwrap_or(0),
            pages_after
        );

        // 페이지 12-15 내용 출력
        for pidx in 11..15.min(pages_after) {
            let p = &doc.pagination[0].pages[pidx];
            eprintln!("  page {} items:", pidx + 1);
            for item in &p.column_contents[0].items {
                match item {
                    PageItem::Table {
                        para_index,
                        control_index,
                    } => {
                        let text = &doc.document.sections[0].paragraphs[*para_index].text;
                        eprintln!(
                            "    Table pi={} ci={} text='{}'",
                            para_index,
                            control_index,
                            &text[..text.len().min(30)]
                        );
                    }
                    PageItem::FullParagraph { para_index } => {
                        let text = &doc.document.sections[0].paragraphs[*para_index].text;
                        let display: String = if text.is_empty() {
                            "(빈)".to_string()
                        } else {
                            text.chars().take(40).collect()
                        };
                        eprintln!("    FullPara pi={} '{}'", para_index, display);
                    }
                    _ => eprintln!("    {:?}", item),
                }
            }
        }
    }
}



/// 12페이지 엔터 후 13페이지의 표 배치 검증
#[test]
fn test_page12_enter_table_placement() {
    use crate::renderer::pagination::PageItem;

    let bytes = std::fs::read("samples/kps-ai.hwp").expect("kps-ai.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    let pages_before = doc.pagination[0].pages.len();
    eprintln!("  pages_before = {}", pages_before);

    // page 12 (idx=11) 내용 확인
    let p12 = &doc.pagination[0].pages[11];
    eprintln!("  page 12 items:");
    for item in &p12.column_contents[0].items {
        eprintln!("    {:?}", item);
    }

    // page 13 (idx=12): pi=197(text), pi=198(table), pi=199(text)
    let p13_before = &doc.pagination[0].pages[12];
    eprintln!("  page 13 items (before):");
    for item in &p13_before.column_contents[0].items {
        eprintln!("    {:?}", item);
    }
    // pi=198 표가 page 13에 있는지 확인
    let has_table_198_on_p13 = p13_before.column_contents[0].items.iter().any(|it| {
        matches!(
            it,
            PageItem::Table {
                para_index: 198,
                ..
            }
        )
    });
    assert!(
        has_table_198_on_p13,
        "수정 전: pi=198 표가 page 13에 있어야 함"
    );

    // pi=199 앞에서 엔터 (pi=199를 분할하여 빈 문단 삽입)
    let result = doc.split_paragraph_native(0, 199, 0, None).unwrap();
    assert!(result.contains("\"ok\":true"), "split failed: {}", result);

    let pages_after = doc.pagination[0].pages.len();
    eprintln!("  pages_after = {}", pages_after);

    // page 13 (idx=12): pi=198 표가 여전히 page 13에 있어야 함
    if doc.pagination[0].pages.len() > 12 {
        let p13_after = &doc.pagination[0].pages[12];
        eprintln!("  page 13 items (after):");
        for item in &p13_after.column_contents[0].items {
            eprintln!("    {:?}", item);
        }
        let has_table_198_after = p13_after.column_contents[0].items.iter().any(|it| {
            matches!(
                it,
                PageItem::Table {
                    para_index: 198,
                    ..
                }
            )
        });

        // page 14도 확인
        if doc.pagination[0].pages.len() > 13 {
            let p14_after = &doc.pagination[0].pages[13];
            eprintln!("  page 14 items (after):");
            for item in &p14_after.column_contents[0].items {
                eprintln!("    {:?}", item);
            }
        }

        assert!(
            has_table_198_after,
            "pi=198 표가 page 13에 있어야 하지만 다음 페이지로 밀려남"
        );
    }
}



/// 논리적 오프셋: 인라인 TAC 표 뒤에서 텍스트 삽입 검증
#[test]
fn test_logical_offset_insert_after_inline_table() {
    let bytes = std::fs::read("saved/blank2010.hwp").expect("blank2010.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    // Enter로 새 문단 생성 (기존 컨트롤이 있는 pi=0 대신 깨끗한 pi=1 사용)
    doc.insert_text_native(0, 0, 0, "test").unwrap();
    doc.split_paragraph_native(0, 0, 4, None).unwrap();

    // pi=1에 "abc" 입력
    doc.insert_text_native(0, 1, 0, "abc").unwrap();
    let para = &doc.document.sections[0].paragraphs[1];
    assert_eq!(para.text, "abc");
    eprintln!("  pi=1 controls={} (표 삽입 전)", para.controls.len());

    // offset=3 위치에 인라인 TAC 2×2 표 삽입
    let result = doc
        .create_table_ex_native(0, 1, 3, 2, 2, true, Some(&[6777, 6777]), None)
        .unwrap();
    eprintln!("  createTableEx result: {}", result);
    // logicalOffset: "abc"(3) + [표](1) = 4
    assert!(
        result.contains("\"logicalOffset\":4"),
        "logicalOffset=4 예상: {}",
        result
    );

    let para = &doc.document.sections[0].paragraphs[1];
    eprintln!(
        "  text='{}' controls={} char_offsets={:?}",
        para.text,
        para.controls.len(),
        para.char_offsets
    );

    // 논리적 길이: "abc"(3) + [표](1) = 4
    let logical_len = crate::document_core::helpers::logical_paragraph_length(para);
    eprintln!("  논리적 길이: {}", logical_len);
    assert_eq!(logical_len, 4, "논리적 길이 4 예상, 실제: {}", logical_len);

    // 논리적 offset 4에 "XYZ" 삽입 → 표 뒤에 삽입되어야 함
    let (text_off, after_ctrl) = crate::document_core::helpers::logical_to_text_offset(para, 4);
    eprintln!(
        "  logical 4 → text_off={} after_ctrl={}",
        text_off, after_ctrl
    );
    assert_eq!(text_off, 3, "text_off=3 예상 (abc 뒤)");

    doc.insert_text_native(0, 1, text_off, "XYZ").unwrap();
    let para = &doc.document.sections[0].paragraphs[1];
    assert_eq!(
        para.text, "abcXYZ",
        "표 뒤에 XYZ 삽입 예상, 실제: '{}'",
        para.text
    );
    eprintln!("  삽입 후 text='{}' ✓", para.text);

    // 논리적 길이: "abcXYZ"(6) + [표](1) = 7
    let logical_len2 = crate::document_core::helpers::logical_paragraph_length(para);
    assert_eq!(
        logical_len2, 7,
        "논리적 길이 7 예상, 실제: {}",
        logical_len2
    );

    // logical offset 변환 검증 (삽입 후: "abcXYZ" + [표at3])
    // a(0) b(1) c(2) [표](3) X(4) Y(5) Z(6)
    let (t0, _) = crate::document_core::helpers::logical_to_text_offset(para, 0);
    let (t3, _) = crate::document_core::helpers::logical_to_text_offset(para, 3);
    let (t4, _) = crate::document_core::helpers::logical_to_text_offset(para, 4);
    let (t7, _) = crate::document_core::helpers::logical_to_text_offset(para, 7);
    eprintln!("  logical→text: 0→{} 3→{} 4→{} 7→{}", t0, t3, t4, t7);
    assert_eq!(t0, 0, "logical 0 → text 0");
    assert_eq!(
        t3, 3,
        "logical 3 → text 3 (표 위치, [표] = ctrl at text pos 3)"
    );
    assert_eq!(t4, 3 + 1, "logical 4 → text 4 (X, 표 뒤 첫 텍스트)");
    assert_eq!(t7, 6, "logical 7 → text 6 (끝)");

    // ── 핵심 검증: charOffset > text_len으로 직접 삽입 ──
    // 새 문서에서 "가나다" + [표] 구조 생성, charOffset=4로 삽입
    doc.split_paragraph_native(0, 1, 6, None).unwrap(); // pi=2 생성
    doc.insert_text_native(0, 2, 0, "가나다").unwrap();
    doc.create_table_ex_native(0, 2, 3, 1, 1, true, Some(&[5000]), None)
        .unwrap();
    let para2 = &doc.document.sections[0].paragraphs[2];
    let tl = para2.text.chars().count();
    eprintln!(
        "  pi=2: text='{}' len={} controls={}",
        para2.text,
        tl,
        para2.controls.len()
    );
    // charOffset=4 (> text_len=3) → 표 뒤에 삽입
    doc.insert_text_native(0, 2, 4, "라마바").unwrap();
    let para2 = &doc.document.sections[0].paragraphs[2];
    eprintln!("  charOffset=4 삽입 후: '{}'", para2.text);
    assert_eq!(
        para2.text, "가나다라마바",
        "표 뒤에 '라마바' 삽입, 실제: '{}'",
        para2.text
    );

    eprintln!("  논리적 오프셋 테스트 통과 ✓");
}



/// createTableEx: 빈 문서에서 인라인 TAC 표를 생성하여 tac-case-001.hwp와 동일한 구조 검증
#[test]
fn test_create_inline_tac_table() {
    let bytes = std::fs::read("saved/blank2010.hwp").expect("blank2010.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    // 1. pi=0에 "TC #20" 입력
    doc.insert_text_native(0, 0, 0, "TC #20").unwrap();
    // 2. Enter → pi=1 생성
    doc.split_paragraph_native(0, 0, 6, None).unwrap();
    // 3. pi=1에 "tacglkj 표 3 배치 시작" 입력
    doc.insert_text_native(0, 1, 0, "tacglkj 표 3 배치 시작")
        .unwrap();

    let text_len = doc.document.sections[0].paragraphs[1].text.chars().count();
    eprintln!(
        "  pi=1 text='{}' len={}",
        doc.document.sections[0].paragraphs[1].text, text_len
    );

    // 4. pi=1, char_offset=text_len 위치에 인라인 TAC 2×2 표 생성
    // 열 폭: 6777 HU × 2 = 13554 HU (tac-case-001.hwp과 동일)
    let result = doc
        .create_table_ex_native(0, 1, text_len, 2, 2, true, Some(&[6777, 6777]), None)
        .unwrap();
    eprintln!("  createTableEx result: {}", result);
    assert!(
        result.contains("\"ok\":true"),
        "createTableEx 실패: {}",
        result
    );

    // 5. 표 뒤에 "4 tacglkj 표 다음" 텍스트 추가
    let para = &doc.document.sections[0].paragraphs[1];
    let new_text_offset = para.text.chars().count();
    doc.insert_text_native(0, 1, new_text_offset, "4 tacglkj 표 다음")
        .unwrap();

    // 6. 검증
    let para = &doc.document.sections[0].paragraphs[1];
    eprintln!(
        "  pi=1 final text='{}' controls={}",
        para.text,
        para.controls.len()
    );

    // 표가 controls에 추가되었는지
    assert_eq!(para.controls.len(), 1, "pi=1에 표 컨트롤 1개 예상");
    if let crate::model::control::Control::Table(t) = &para.controls[0] {
        assert!(t.common.treat_as_char, "treat_as_char=true 예상");
        assert_eq!(t.row_count, 2, "행 수 2 예상");
        assert_eq!(t.col_count, 2, "열 수 2 예상");
        eprintln!(
            "  표: {}×{} tac={} width={} height={}",
            t.row_count, t.col_count, t.common.treat_as_char, t.common.width, t.common.height
        );
    } else {
        panic!("pi=1의 첫 컨트롤이 Table이 아님");
    }

    // 셀에 텍스트 입력
    doc.insert_text_in_cell_native(0, 1, 0, 0, 0, 0, "1")
        .unwrap();
    doc.insert_text_in_cell_native(0, 1, 0, 1, 0, 0, "2")
        .unwrap();
    doc.insert_text_in_cell_native(0, 1, 0, 2, 0, 0, "3 tacglkj")
        .unwrap();
    doc.insert_text_in_cell_native(0, 1, 0, 3, 0, 0, "4 tacglkj")
        .unwrap();

    // Enter → pi=2
    let pi1_len = crate::document_core::helpers::logical_paragraph_length(
        &doc.document.sections[0].paragraphs[1],
    );
    doc.split_paragraph_native(0, 1, pi1_len, None).unwrap();
    // pi=2에 텍스트
    doc.insert_text_native(0, 2, 0, "tacglkj 가나 옮").unwrap();

    // 페이지네이션
    doc.paginate();
    let page_count: usize = doc.pagination.iter().map(|r| r.pages.len()).sum();
    eprintln!("  최종 페이지 수: {}", page_count);
    assert_eq!(page_count, 1, "1페이지 문서 예상");

    // 텍스트에 표가 포함된 인라인 배치 확인
    let para = &doc.document.sections[0].paragraphs[1];
    assert!(!para.text.is_empty(), "pi=1에 텍스트가 있어야 함");
    assert_eq!(para.controls.len(), 1, "pi=1에 인라인 표 1개");

    // is_tac_table_inline 확인
    let seg_w = para.line_segs.first().map(|s| s.segment_width).unwrap_or(0);
    if let crate::model::control::Control::Table(t) = &para.controls[0] {
        let is_inline =
            crate::renderer::height_measurer::is_tac_table_inline_in_para(t, seg_w, para);
        eprintln!("  is_tac_table_inline: {} (seg_w={})", is_inline, seg_w);
        assert!(is_inline, "인라인 TAC 표로 판별되어야 함");
    }

    eprintln!("  인라인 TAC 표 생성 테스트 통과");
}



#[test]
fn local_body_replace_exposes_stable_edit_before_full_pagination() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    fn contains_text(node: &RenderNode, needle: &str) -> bool {
        if let RenderNodeType::TextRun(run) = &node.node_type {
            if run.text.contains(needle) {
                return true;
            }
        }
        node.children
            .iter()
            .any(|child| contains_text(child, needle))
    }

    let mut doc = HwpDocument::create_empty();
    doc.insert_text_native(0, 0, 0, "나")
        .expect("seed non-empty body paragraph");
    doc.build_page_render_tree(0).expect("warm page tree");

    let raw = doc
        .replace_body_text_local_native(0, 0, 1, 0, "가")
        .expect("stable local insert");
    let result: Value = serde_json::from_str(&raw).expect("local result json");

    assert_eq!(result["charOffset"].as_u64(), Some(2));
    assert_eq!(result["documentPaginationPending"].as_bool(), Some(true));
    assert_eq!(result["flowChanged"].as_bool(), Some(false));
    assert_eq!(
        doc.get_text_range_native(0, 0, 0, 2)
            .expect("immediate text"),
        "나가"
    );

    let transient_tree = doc.build_page_render_tree(0).expect("transient page tree");
    assert!(
        contains_text(&transient_tree.root, "가"),
        "warm page tree must expose the local edit before full pagination"
    );

    let deleted_raw = doc
        .replace_body_text_local_native(0, 0, 1, 1, "")
        .expect("stable local delete");
    let deleted: Value = serde_json::from_str(&deleted_raw).expect("delete result json");
    assert_eq!(deleted["charOffset"].as_u64(), Some(1));
    assert_eq!(deleted["documentPaginationPending"].as_bool(), Some(true));
    assert_eq!(deleted["flowChanged"].as_bool(), Some(false));
    assert_eq!(
        doc.get_text_range_native(0, 0, 0, 1).expect("deleted text"),
        "나"
    );
}
