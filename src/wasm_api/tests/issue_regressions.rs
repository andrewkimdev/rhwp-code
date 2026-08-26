//! issue_regressions — tests/mod.rs 에서 무변동 이동
use super::*;







fn issue_1481_find_table_and_host_mark_y(
    node: &crate::renderer::render_tree::RenderNode,
    para_idx: usize,
    table_y: &mut Option<f64>,
    mark_y: &mut Option<f64>,
) {
    use crate::renderer::render_tree::RenderNodeType;

    match &node.node_type {
        RenderNodeType::Table(table) if table.para_index == Some(para_idx) => {
            *table_y = Some(node.bbox.y);
        }
        RenderNodeType::TextRun(run)
            if run.para_index == Some(para_idx)
                && run.cell_context.is_none()
                && run.text.is_empty()
                && run.is_para_end =>
        {
            *mark_y = Some(node.bbox.y);
        }
        _ => {}
    }

    for child in &node.children {
        issue_1481_find_table_and_host_mark_y(child, para_idx, table_y, mark_y);
    }
}


fn issue_1481_collect_outside_empty_para_marks(
    node: &crate::renderer::render_tree::RenderNode,
    marks: &mut Vec<(usize, f64)>,
) {
    use crate::renderer::render_tree::RenderNodeType;

    if let RenderNodeType::TextRun(run) = &node.node_type {
        if let Some(para_idx) = run.para_index {
            if run.cell_context.is_none() && run.text.is_empty() && run.is_para_end {
                marks.push((para_idx, node.bbox.y));
            }
        }
    }

    for child in &node.children {
        issue_1481_collect_outside_empty_para_marks(child, marks);
    }
}


fn issue_1481_collect_layer_control_mark_y(value: &Value, marks: &mut Vec<f64>) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("textControlMark")
                && map.get("isParaEnd").and_then(Value::as_bool) == Some(true)
            {
                if let Some(y) = map
                    .get("bbox")
                    .and_then(|bbox| bbox.get("y"))
                    .and_then(Value::as_f64)
                {
                    marks.push(y);
                }
            }
            for child in map.values() {
                issue_1481_collect_layer_control_mark_y(child, marks);
            }
        }
        Value::Array(items) => {
            for item in items {
                issue_1481_collect_layer_control_mark_y(item, marks);
            }
        }
        _ => {}
    }
}


fn issue_1481_layer_control_mark_y(doc: &HwpDocument) -> Vec<f64> {
    let json = doc
        .get_page_layer_tree_native(0)
        .expect("PageLayerTree JSON");
    let parsed: Value = serde_json::from_str(&json).expect("PageLayerTree JSON 파싱");
    let mut marks = Vec::new();
    issue_1481_collect_layer_control_mark_y(&parsed, &mut marks);
    marks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    marks
}



#[test]
fn issue_1470_style_update_reflows_and_keeps_margin_unit() {
    use crate::model::style::{CharShape, ParaShape, Style};

    let mut doc = HwpDocument::create_empty();
    doc.document.doc_info.char_shapes.push(CharShape::default());
    doc.document.doc_info.para_shapes.push(ParaShape::default());
    doc.document.doc_info.styles.push(Style {
        local_name: "바탕글".to_string(),
        english_name: "Normal".to_string(),
        lang_id: 1042,
        para_shape_id: 0,
        char_shape_id: 0,
        ..Default::default()
    });
    doc.insert_text_native(0, 0, 0, "스타일 줄간격 검증")
        .expect("텍스트 입력");

    let style_id = doc.create_style(
        r#"{"name":"검증 스타일","englishName":"Issue1470","type":0,"nextStyleId":0,"baseParaShapeId":0,"baseCharShapeId":0}"#,
    );
    assert!(style_id >= 0, "스타일 생성");
    doc.apply_style_native(0, 0, style_id as usize)
        .expect("스타일 적용");

    let before_spacing = doc.document.sections[0].paragraphs[0]
        .line_segs
        .first()
        .map(|ls| ls.line_spacing)
        .unwrap_or_default();

    assert!(
        doc.update_style_shapes(
            style_id as u32,
            "{}",
            r#"{"marginLeft":3000,"lineSpacing":300,"lineSpacingType":"Percent"}"#,
        ),
        "스타일 문단 모양 수정"
    );

    let para = &doc.document.sections[0].paragraphs[0];
    let ps = &doc.document.doc_info.para_shapes[para.para_shape_id as usize];
    assert_eq!(para.style_id, style_id as u8);
    assert_eq!(
        ps.margin_left, 3000,
        "15pt raw(2x) 여백이 30pt로 중복 변환되면 안 됨"
    );
    assert_eq!(ps.line_spacing, 300);
    let after_spacing = para
        .line_segs
        .first()
        .map(|ls| ls.line_spacing)
        .unwrap_or_default();
    assert_ne!(
        after_spacing, before_spacing,
        "스타일 줄간격 변경 후 LineSeg가 즉시 재계산되어야 한다"
    );
}



#[test]
fn issue_1470_style_apply_preserves_direct_char_shape() {
    use crate::model::paragraph::CharShapeRef;
    use crate::model::style::{CharShape, ParaShape, Style};

    let mut doc = HwpDocument::create_empty();
    doc.document.doc_info.char_shapes = vec![
        CharShape {
            base_size: 1000,
            ..Default::default()
        },
        CharShape {
            base_size: 1200,
            ..Default::default()
        },
        CharShape {
            bold: true,
            ..Default::default()
        },
    ];
    doc.document.doc_info.para_shapes = vec![
        ParaShape::default(),
        ParaShape {
            margin_left: 1000,
            ..Default::default()
        },
    ];
    doc.document.doc_info.styles = vec![
        Style {
            local_name: "바탕글".to_string(),
            english_name: "Normal".to_string(),
            lang_id: 1042,
            para_shape_id: 0,
            char_shape_id: 0,
            ..Default::default()
        },
        Style {
            local_name: "새 문단 스타일".to_string(),
            english_name: "Issue1470Apply".to_string(),
            lang_id: 1042,
            para_shape_id: 1,
            char_shape_id: 1,
            ..Default::default()
        },
    ];
    doc.insert_text_native(0, 0, 0, "가나다라")
        .expect("텍스트 입력");

    let para = &mut doc.document.sections[0].paragraphs[0];
    para.style_id = 0;
    para.para_shape_id = 0;
    para.char_shapes = vec![CharShapeRef {
        start_pos: 0,
        char_shape_id: 0,
    }];
    para.apply_char_shape_range(1, 3, 2);

    doc.apply_style_native(0, 0, 1).expect("문단 스타일 적용");

    let para = &doc.document.sections[0].paragraphs[0];
    assert_eq!(para.style_id, 1);
    assert_eq!(para.para_shape_id, 1);
    let refs: Vec<(u32, u32)> = para
        .char_shapes
        .iter()
        .map(|cs| (cs.start_pos, cs.char_shape_id))
        .collect();
    assert_eq!(
        refs,
        vec![(0, 1), (1, 2), (3, 1)],
        "스타일 기본 글자 모양만 새 스타일로 바뀌고 직접 글자 모양 range는 유지되어야 한다"
    );
}



#[test]
fn issue_1470_style_update_preserves_direct_char_shape() {
    use crate::model::paragraph::CharShapeRef;
    use crate::model::style::{CharShape, ParaShape, Style};

    let mut doc = HwpDocument::create_empty();
    doc.document.doc_info.char_shapes = vec![
        CharShape {
            base_size: 1000,
            ..Default::default()
        },
        CharShape {
            base_size: 1200,
            ..Default::default()
        },
        CharShape {
            bold: true,
            ..Default::default()
        },
    ];
    doc.document.doc_info.para_shapes = vec![
        ParaShape::default(),
        ParaShape {
            margin_left: 1000,
            ..Default::default()
        },
    ];
    doc.document.doc_info.styles = vec![
        Style {
            local_name: "바탕글".to_string(),
            english_name: "Normal".to_string(),
            lang_id: 1042,
            para_shape_id: 0,
            char_shape_id: 0,
            ..Default::default()
        },
        Style {
            local_name: "편집 대상 스타일".to_string(),
            english_name: "Issue1470Update".to_string(),
            lang_id: 1042,
            para_shape_id: 1,
            char_shape_id: 1,
            ..Default::default()
        },
    ];
    doc.insert_text_native(0, 0, 0, "가나다라")
        .expect("텍스트 입력");

    let para = &mut doc.document.sections[0].paragraphs[0];
    para.style_id = 1;
    para.para_shape_id = 1;
    para.char_shapes = vec![CharShapeRef {
        start_pos: 0,
        char_shape_id: 1,
    }];
    para.apply_char_shape_range(1, 3, 2);

    assert!(
        doc.update_style_shapes(1, r#"{"fontSize":1400}"#, "{}"),
        "스타일 글자 모양 수정"
    );

    let updated_csid = doc.document.doc_info.styles[1].char_shape_id as u32;
    assert_ne!(
        updated_csid, 1,
        "스타일 CharShape가 새 ID로 갱신되어야 한다"
    );

    let para = &doc.document.sections[0].paragraphs[0];
    let refs: Vec<(u32, u32)> = para
        .char_shapes
        .iter()
        .map(|cs| (cs.start_pos, cs.char_shape_id))
        .collect();
    assert_eq!(
        refs,
        vec![(0, updated_csid), (1, 2), (3, updated_csid)],
        "스타일 편집 전파 시 직접 글자 모양 range는 유지되어야 한다"
    );
}



/// [#4325] `table.dirty`가 문서 범위로 지워지고 구역 범위로 소비되는 스코프 불일치 회귀.
///
/// `updateStyleShapes`는 스타일을 쓰는 모든 구역의 표를 `mark_cell_control_dirty`로
/// dirty 마킹한 뒤, `0..num_sections`을 돌며 구역별로 `rebuild_section`을 호출한다.
/// `rebuild_section(0)`이 유발하는 `paginate()` 패스는 아직 `dirty_sections[1]`이
/// false라 구역 1을 건너뛰지만, 그 패스 끝에서 표 dirty 플래그를 지우는 루프가
/// 문서 전체 범위였다 — 건너뛴 구역 1의 표까지 dirty가 사라졌다. 이후
/// `rebuild_section(1)`이 구역 1을 실제로 재측정할 차례가 오면 `table.dirty`가
/// 이미 false라 `measure_section_incremental`이 변경 전 `MeasuredTable`을 그대로
/// clone해 구역 1의 표 높이가 영구히 stale로 남았다(원본 편집 전까지 복구 안 됨).
#[test]
fn issue_4325_style_update_second_section_table_not_left_stale() {
    use crate::model::style::{CharShape, ParaShape, Style};

    let mut doc = HwpDocument::create_empty();
    doc.document.doc_info.char_shapes.push(CharShape {
        base_size: 1000, // 10pt
        ..Default::default()
    });
    doc.document.doc_info.para_shapes.push(ParaShape::default());
    doc.document.doc_info.styles.push(Style {
        local_name: "바탕글".to_string(),
        english_name: "Normal".to_string(),
        lang_id: 1042,
        para_shape_id: 0,
        char_shape_id: 0,
        ..Default::default()
    });

    // 구역 0에 2x2 표 삽입 + 줄바꿈되는 셀 텍스트 (이슈 실측: "같은 표 + 줄바꿈되는 텍스트")
    doc.create_table_native(0, 0, 0, 2, 2)
        .expect("표 삽입 (구역0)");
    let wrap_text = "가".repeat(120);
    doc.insert_text_in_cell_native(0, 0, 0, 0, 0, 0, &wrap_text)
        .expect("셀 텍스트 삽입 (구역0)");

    // 구역 1: 구역 0과 완전히 같은 표+텍스트를 가진 구역을 추가한다 (이슈 실측: 2구역 문서).
    let mut two_section_doc = doc.document.clone();
    let section0 = two_section_doc.sections[0].clone();
    two_section_doc.sections.push(section0);
    doc.set_document(two_section_doc);

    assert_eq!(doc.document.sections.len(), 2, "구역 2개 문서 준비");
    assert_eq!(doc.measured_tables.len(), 2);
    assert_eq!(
        doc.measured_tables[0][0].total_height, doc.measured_tables[1][0].total_height,
        "복제 직후 두 구역의 표 높이는 같아야 한다 (기준선)"
    );
    assert!(
        doc.dirty_sections.iter().all(|d| !d),
        "set_document 직후 paginate가 두 구역을 모두 처리해야 한다"
    );
    let before_height = doc.measured_tables[0][0].total_height;

    // 스타일 글자모양을 36pt로 일괄 변경 (이슈 실측 시나리오)
    assert!(
        doc.update_style_shapes(0, r#"{"fontSize":3600}"#, "{}"),
        "스타일 글자모양 수정 (36pt)"
    );

    let after0 = doc.measured_tables[0][0].total_height;
    let after1 = doc.measured_tables[1][0].total_height;

    assert!(
        after0 > before_height,
        "구역0 표 높이는 36pt 반영으로 커져야 한다 (before={before_height}, after0={after0})"
    );
    assert_eq!(
        after0, after1,
        "issue #4325 회귀: 구역1 표가 구역0과 동일한 표/텍스트인데도 stale MeasuredTable로 \
         남았다 (before={before_height}, after0={after0}, after1={after1})"
    );
    assert_eq!(
        doc.measured_tables[0][0].row_heights, doc.measured_tables[1][0].row_heights,
        "issue #4325 회귀: 구역1 표의 행 높이가 구역0과 달라지면 안 된다"
    );
}



#[test]
fn issue_1470_character_style_does_not_replace_para_style() {
    use crate::model::paragraph::CharShapeRef;
    use crate::model::style::{CharShape, ParaShape, Style};

    let mut doc = HwpDocument::create_empty();
    doc.document.doc_info.char_shapes = vec![
        CharShape {
            base_size: 1000,
            ..Default::default()
        },
        CharShape {
            italic: true,
            ..Default::default()
        },
    ];
    doc.document.doc_info.para_shapes = vec![ParaShape::default()];
    doc.document.doc_info.styles = vec![
        Style {
            local_name: "바탕글".to_string(),
            english_name: "Normal".to_string(),
            lang_id: 1042,
            para_shape_id: 0,
            char_shape_id: 0,
            ..Default::default()
        },
        Style {
            local_name: "글자 스타일".to_string(),
            english_name: "Issue1470Char".to_string(),
            style_type: 1,
            lang_id: 1042,
            para_shape_id: 0,
            char_shape_id: 1,
            ..Default::default()
        },
    ];
    doc.insert_text_native(0, 0, 0, "글자스타일")
        .expect("텍스트 입력");

    let para = &mut doc.document.sections[0].paragraphs[0];
    para.style_id = 0;
    para.para_shape_id = 0;
    para.char_shapes = vec![CharShapeRef {
        start_pos: 0,
        char_shape_id: 0,
    }];

    doc.apply_style_native(0, 0, 1).expect("글자 스타일 적용");

    let para = &doc.document.sections[0].paragraphs[0];
    assert_eq!(
        para.style_id, 0,
        "글자 스타일은 문단 스타일 ID를 바꾸지 않는다"
    );
    assert_eq!(
        para.para_shape_id, 0,
        "글자 스타일은 문단 모양 ID를 바꾸지 않는다"
    );
    assert_eq!(
        para.char_shape_id_at(0),
        Some(1),
        "글자 스타일 CharShape는 글자 모양에 적용되어야 한다"
    );
}



#[test]
fn issue_1470_create_table_ex_applies_size_options() {
    use crate::model::control::Control;

    let mut doc = HwpDocument::create_empty();
    let col_widths = [4000, 6000];
    let row_heights = [3000, 5000];
    doc.create_table_ex_native(0, 0, 0, 2, 2, true, Some(&col_widths), Some(&row_heights))
        .expect("확장 표 생성");

    let table = doc.document.sections[0].paragraphs[0]
        .controls
        .iter()
        .find_map(|c| match c {
            Control::Table(t) => Some(t),
            _ => None,
        })
        .expect("표 컨트롤");
    assert!(
        table.common.treat_as_char,
        "상세 옵션의 글자처럼 취급이 반영되어야 한다"
    );
    assert_eq!(table.common.width, 10000);
    assert_eq!(table.common.height, 8000);
    assert_eq!(table.cells[0].width, 4000);
    assert_eq!(table.cells[1].width, 6000);
    assert_eq!(table.cells[0].height, 3000);
    assert_eq!(table.cells[2].height, 5000);
}



#[test]
fn issue_1481_create_table_keeps_first_line_mark_for_escape() {
    use crate::model::control::Control;

    let mut doc = HwpDocument::create_empty();
    let table_result = doc
        .create_table_ex_native(0, 0, 1, 3, 5, false, None, None)
        .expect("상세 대화상자 경로의 일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");
    let section = &doc.document.sections[0];

    assert_eq!(table_para_idx, 0, "새 문서 첫 표는 첫 줄에 만들어져야 한다");
    assert!(
        section.paragraphs.len() >= 2,
        "표 뒤 빈 문단은 유지되어야 한다"
    );
    assert!(section.paragraphs[0].text.is_empty());
    assert_eq!(section.paragraphs[0].char_count, 9);
    assert!(section.paragraphs[0].has_para_text);
    assert!(!section.paragraphs[0].line_segs.is_empty());
    assert!(matches!(
        section.paragraphs[table_para_idx].controls.first(),
        Some(Control::Table(_))
    ));
    assert!(section.paragraphs[table_para_idx + 1].text.is_empty());
    assert!(section.paragraphs[table_para_idx + 1].controls.is_empty());
    assert_eq!(section.paragraphs[table_para_idx + 1].char_count, 1);
    assert!(!section.paragraphs[table_para_idx + 1].line_segs.is_empty());

    let moved = doc
        .move_vertical(0, 0, 0, -1, 0.0, table_para_idx as u32, 0, 0, 0)
        .expect("첫 셀에서 위쪽 이동");
    let moved: Value = serde_json::from_str(&moved).expect("moveVertical JSON");
    assert_eq!(
        moved["paragraphIndex"].as_u64(),
        Some(table_para_idx as u64)
    );
    assert_eq!(moved["charOffset"].as_u64(), Some(0));
    assert!(
        moved.get("parentParaIndex").is_none(),
        "첫 셀 위쪽 이동은 같은 첫 줄의 표 밖 조판부호 위치로 나가야 한다"
    );

    let tree = issue_1481_first_page_render_tree(&doc);
    let mut table_y = None;
    let mut host_mark_y = None;
    issue_1481_find_table_and_host_mark_y(
        &tree.root,
        table_para_idx,
        &mut table_y,
        &mut host_mark_y,
    );
    let table_y = table_y.expect("표 렌더 노드 y");
    let host_mark_y = host_mark_y.expect("표 host 문단부호 y");
    assert!(
        (table_y - host_mark_y).abs() < 1.0,
        "기본 자리차지 표의 첫 조판부호는 빈 줄이 아니라 표 상단과 겹쳐야 한다: table_y={table_y}, mark_y={host_mark_y}"
    );
    doc.set_show_paragraph_marks(true);
    let layer_marks = issue_1481_layer_control_mark_y(&doc);
    let layer_marks_above_table = layer_marks
        .iter()
        .filter(|y| **y < table_y - 1.0)
        .copied()
        .collect::<Vec<_>>();
    assert!(
        layer_marks_above_table.is_empty(),
        "새 문서 빈 문단 끝에서 표를 만들 때 생성 경로가 빈 줄을 남기면 안 된다: table_y={table_y}, layer_marks_above={layer_marks_above_table:?}, all={layer_marks:?}"
    );
    doc.set_show_paragraph_marks(false);

    let mut outside_marks = Vec::new();
    issue_1481_collect_outside_empty_para_marks(&tree.root, &mut outside_marks);
    let marks_above_table = outside_marks
        .iter()
        .filter(|(_, y)| *y < table_y - 1.0)
        .copied()
        .collect::<Vec<_>>();
    assert!(
        marks_above_table.is_empty(),
        "표 생성 직후 표 위에 별도 빈 줄 조판부호가 있으면 안 된다: table_y={table_y}, marks={marks_above_table:?}, all={outside_marks:?}"
    );

    let enter_result = doc
        .split_paragraph_native(0, table_para_idx, 0, None)
        .expect("표 앞 조판부호 위치 Enter");
    let enter_para_idx = issue_1481_json_usize(&enter_result, "paraIdx");
    assert_eq!(
        enter_para_idx,
        table_para_idx + 1,
        "자리차지 표 앞 Enter는 표 아래 문단으로 커서를 보내야 한다"
    );
    let section_after_enter = &doc.document.sections[0];
    assert!(matches!(
        section_after_enter.paragraphs[table_para_idx]
            .controls
            .first(),
        Some(Control::Table(_))
    ));
    assert_eq!(
        section_after_enter.paragraphs[table_para_idx].char_count, 9,
        "Enter 후에도 표 host 문단은 빈 문단으로 분리되면 안 된다"
    );
    assert!(section_after_enter.paragraphs[enter_para_idx]
        .text
        .is_empty());
    assert!(section_after_enter.paragraphs[enter_para_idx]
        .controls
        .is_empty());
    assert_eq!(section_after_enter.paragraphs[enter_para_idx].char_count, 1);
    assert!(section_after_enter
        .paragraphs
        .get(enter_para_idx + 1)
        .map(|p| p.text.is_empty() && p.controls.is_empty())
        .unwrap_or(false));

    let tree_after_enter = issue_1481_first_page_render_tree(&doc);
    let mut table_y_after = None;
    let mut host_mark_y_after = None;
    issue_1481_find_table_and_host_mark_y(
        &tree_after_enter.root,
        table_para_idx,
        &mut table_y_after,
        &mut host_mark_y_after,
    );
    let table_y_after = table_y_after.expect("Enter 후 표 렌더 노드 y");
    let host_mark_y_after = host_mark_y_after.expect("Enter 후 표 host 문단부호 y");
    assert!(
        (table_y_after - host_mark_y_after).abs() < 1.0,
        "Enter 후에도 표 host 조판부호는 표 상단과 겹쳐야 한다: table_y={table_y_after}, mark_y={host_mark_y_after}"
    );
    let mut outside_marks_after = Vec::new();
    issue_1481_collect_outside_empty_para_marks(&tree_after_enter.root, &mut outside_marks_after);
    let marks_above_table_after = outside_marks_after
        .iter()
        .filter(|(_, y)| *y < table_y_after - 1.0)
        .copied()
        .collect::<Vec<_>>();
    assert!(
        marks_above_table_after.is_empty(),
        "Enter 후에도 표 위에 별도 빈 줄 조판부호가 있으면 안 된다: table_y={table_y_after}, marks={marks_above_table_after:?}, all={outside_marks_after:?}"
    );
}



#[test]
fn issue_1481_create_table_preserves_user_blank_line_above() {
    use crate::model::control::Control;

    let mut doc = HwpDocument::create_empty();
    doc.split_paragraph_native(0, 0, 0, None)
        .expect("사용자가 만든 빈 줄");
    let table_result = doc
        .create_table_ex_native(0, 1, 1, 3, 5, false, None, None)
        .expect("두 번째 빈 문단에 일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");
    let section = &doc.document.sections[0];

    assert_eq!(
        table_para_idx, 1,
        "사용자가 표 위에 만든 빈 문단은 삭제하지 않고 현재 빈 문단만 표 host로 교체해야 한다"
    );
    assert!(section.paragraphs[0].text.is_empty());
    assert!(section.paragraphs[0].controls.is_empty());
    assert_eq!(section.paragraphs[0].char_count, 1);
    assert!(matches!(
        section.paragraphs[table_para_idx].controls.first(),
        Some(Control::Table(_))
    ));
}



#[test]
fn issue_1481_create_table_empty_para_ignores_stale_offset() {
    use crate::model::control::Control;

    let mut doc = HwpDocument::create_empty();
    let table_result = doc
        .create_table_ex_native(0, 0, 2, 3, 5, false, None, None)
        .expect("빈 문단의 초과 offset에서 일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");
    let section = &doc.document.sections[0];

    assert_eq!(
        table_para_idx, 0,
        "빈 문단 offset이 초과되어도 표 위에 생성 경로의 빈 줄을 남기면 안 된다"
    );
    assert!(matches!(
        section.paragraphs[table_para_idx].controls.first(),
        Some(Control::Table(_))
    ));

    let tree = issue_1481_first_page_render_tree(&doc);
    let mut table_y = None;
    let mut host_mark_y = None;
    issue_1481_find_table_and_host_mark_y(
        &tree.root,
        table_para_idx,
        &mut table_y,
        &mut host_mark_y,
    );
    let table_y = table_y.expect("표 렌더 노드 y");
    let host_mark_y = host_mark_y.expect("표 host 문단부호 y");
    assert!(
        (table_y - host_mark_y).abs() < 1.0,
        "빈 문단 초과 offset에서도 첫 조판부호는 표 상단과 겹쳐야 한다: table_y={table_y}, mark_y={host_mark_y}"
    );
}



#[test]
fn issue_1481_blank_template_create_table_has_no_generated_blank_above() {
    use crate::model::control::Control;

    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document_native()
        .expect("Studio 새 문서 템플릿 생성");
    let table_result = doc
        .create_table_ex_native(0, 0, 2, 3, 5, false, None, None)
        .expect("blank2010 기반 빈 문단 초과 offset에서 일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");
    let table_control_idx = issue_1481_json_usize(&table_result, "controlIdx");
    let section = &doc.document.sections[0];

    assert_eq!(
        table_para_idx, 0,
        "Studio 새 문서 템플릿에서도 첫 표는 첫 줄에 만들어져야 한다"
    );
    assert_eq!(
        table_control_idx, 2,
        "blank2010의 SectionDef/ColumnDef 구조 컨트롤 뒤에 표 컨트롤이 보존되어야 한다"
    );
    assert!(matches!(
        section.paragraphs[table_para_idx]
            .controls
            .get(table_control_idx),
        Some(Control::Table(_))
    ));

    let tree = issue_1481_first_page_render_tree(&doc);
    let mut table_y = None;
    let mut host_mark_y = None;
    issue_1481_find_table_and_host_mark_y(
        &tree.root,
        table_para_idx,
        &mut table_y,
        &mut host_mark_y,
    );
    let table_y = table_y.expect("표 렌더 노드 y");
    let host_mark_y = host_mark_y.expect("표 host 문단부호 y");
    assert!(
        (table_y - host_mark_y).abs() < 1.0,
        "blank2010 경로에서도 첫 조판부호는 표 상단과 겹쳐야 한다: table_y={table_y}, mark_y={host_mark_y}"
    );

    doc.set_show_paragraph_marks(true);
    let layer_marks = issue_1481_layer_control_mark_y(&doc);
    let layer_marks_above_table = layer_marks
        .iter()
        .filter(|y| **y < table_y - 1.0)
        .copied()
        .collect::<Vec<_>>();
    assert!(
        layer_marks_above_table.is_empty(),
        "blank2010 경로에서도 표 위에 생성 경로 빈 줄을 남기면 안 된다: table_y={table_y}, layer_marks_above={layer_marks_above_table:?}, all={layer_marks:?}"
    );
    doc.set_show_paragraph_marks(false);
}



#[test]
fn issue_1481_insert_column_keeps_create_table_height() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc
        .create_table_native(0, 0, 0, 3, 5)
        .expect("일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    let (original_width, original_height, original_raw_height, row_height_sum) = {
        let table = issue_1481_table(&doc, table_para_idx);
        let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);
        (
            table.common.width,
            table.common.height,
            raw_common.height,
            table.get_row_heights().iter().sum::<u32>(),
        )
    };

    assert!(
        original_height > row_height_sum,
        "일반 표는 셀 저장 height 합보다 큰 외곽 height를 가진다"
    );

    doc.insert_table_column_native(0, table_para_idx, 0, 0, false)
        .expect("왼쪽 열 추가");

    let table = issue_1481_table(&doc, table_para_idx);
    let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);

    assert_eq!(table.col_count, 6);
    assert!(
        table.common.width > original_width,
        "열 추가 후 표 폭은 기준 열 폭만큼 증가해야 한다"
    );
    assert_eq!(
        table.common.height, original_height,
        "열 추가는 행 수를 바꾸지 않으므로 표 외곽 height를 보존해야 한다"
    );
    assert_eq!(
        raw_common.height, original_raw_height,
        "직렬화 원본 raw height도 표 외곽 height와 함께 보존해야 한다"
    );
    assert_eq!(
        raw_common.height, table.common.height,
        "raw height와 in-memory common height는 동기화되어야 한다"
    );
}



#[test]
fn issue_1481_insert_row_keeps_create_table_display_height() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc
        .create_table_native(0, 0, 0, 3, 5)
        .expect("일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    let (original_height, original_raw_height, original_row_height_sum) = {
        let table = issue_1481_table(&doc, table_para_idx);
        let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);
        (
            table.common.height,
            raw_common.height,
            table.get_row_heights().iter().sum::<u32>(),
        )
    };

    assert!(
        original_height > original_row_height_sum,
        "일반 표는 셀 저장 height 합보다 큰 외곽 height를 가진다"
    );

    doc.insert_table_row_native(0, table_para_idx, 0, 0, false)
        .expect("위쪽 줄 추가");

    let table = issue_1481_table(&doc, table_para_idx);
    let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);
    let expected_height = original_height + (original_height / 3);

    assert_eq!(table.row_count, 4);
    assert_eq!(
        table.common.height, expected_height,
        "줄 추가는 한 행의 표시 높이만큼 표 외곽 height를 늘려야 한다"
    );
    assert!(
        table.common.height > original_raw_height,
        "줄 추가 후 표 높이는 기존 외곽 height보다 커야 한다"
    );
    assert_eq!(
        raw_common.height, table.common.height,
        "raw height와 in-memory common height는 동기화되어야 한다"
    );
}



#[test]
fn issue_1481_delete_row_keeps_create_table_display_height() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc
        .create_table_native(0, 0, 0, 3, 5)
        .expect("일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    let (original_height, original_row_height_sum) = {
        let table = issue_1481_table(&doc, table_para_idx);
        (
            table.common.height,
            table.get_row_heights().iter().sum::<u32>(),
        )
    };

    assert!(
        original_height > original_row_height_sum,
        "일반 표는 셀 저장 height 합보다 큰 외곽 height를 가진다"
    );

    doc.delete_table_row_native(0, table_para_idx, 0, 0)
        .expect("줄 지우기");

    let table = issue_1481_table(&doc, table_para_idx);
    let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);
    let expected_height = original_height - (original_height / 3);

    assert_eq!(table.row_count, 2);
    assert_eq!(
        table.common.height, expected_height,
        "줄 삭제는 삭제 행의 표시 높이만큼 표 외곽 height를 줄여야 한다"
    );
    assert!(
        table.common.height > table.get_row_heights().iter().sum::<u32>(),
        "삭제 후에도 일반 표의 표시 height가 셀 저장 height 합으로 붕괴하면 안 된다"
    );
    assert_eq!(
        raw_common.height, table.common.height,
        "raw height와 in-memory common height는 동기화되어야 한다"
    );
}



#[test]
fn issue_1481_delete_column_keeps_create_table_height() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc
        .create_table_native(0, 0, 0, 3, 5)
        .expect("일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    let (original_width, original_height, original_raw_height, row_height_sum) = {
        let table = issue_1481_table(&doc, table_para_idx);
        let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);
        (
            table.common.width,
            table.common.height,
            raw_common.height,
            table.get_row_heights().iter().sum::<u32>(),
        )
    };

    assert!(
        original_height > row_height_sum,
        "일반 표는 셀 저장 height 합보다 큰 외곽 height를 가진다"
    );

    doc.delete_table_column_native(0, table_para_idx, 0, 0)
        .expect("칸 지우기");

    let table = issue_1481_table(&doc, table_para_idx);
    let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);

    assert_eq!(table.col_count, 4);
    assert!(
        table.common.width < original_width,
        "열 삭제 후 표 폭은 삭제 열 폭만큼 줄어야 한다"
    );
    assert_eq!(
        table.common.height, original_height,
        "열 삭제는 행 수를 바꾸지 않으므로 표 외곽 height를 보존해야 한다"
    );
    assert_eq!(
        raw_common.height, original_raw_height,
        "직렬화 원본 raw height도 표 외곽 height와 함께 보존해야 한다"
    );
    assert_eq!(
        raw_common.height, table.common.height,
        "raw height와 in-memory common height는 동기화되어야 한다"
    );
}



#[test]
fn issue_1481_resize_bottom_row_keeps_create_table_display_height() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc
        .create_table_native(0, 0, 0, 3, 5)
        .expect("일반 표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    let (original_height, original_raw_height, original_row_height_sum, last_row_cells) = {
        let table = issue_1481_table(&doc, table_para_idx);
        let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);
        let last_row = table.row_count - 1;
        (
            table.common.height,
            raw_common.height,
            table.get_row_heights().iter().sum::<u32>(),
            table
                .cells
                .iter()
                .enumerate()
                .filter_map(|(idx, cell)| (cell.row == last_row).then_some(idx))
                .collect::<Vec<_>>(),
        )
    };

    assert!(
        original_height > original_row_height_sum,
        "일반 표는 셀 저장 height 합보다 큰 외곽 height를 가진다"
    );
    assert_eq!(original_height, original_raw_height);
    assert_eq!(last_row_cells.len(), 5);

    let updates = last_row_cells
        .iter()
        .map(|cell_idx| format!(r#"{{"cellIdx":{},"heightDelta":300}}"#, cell_idx))
        .collect::<Vec<_>>()
        .join(",");
    doc.resize_table_cells_native(0, table_para_idx, 0, &format!("[{}]", updates))
        .expect("하단 행 resize");

    let table = issue_1481_table(&doc, table_para_idx);
    let raw_common = parse_common_obj_attr(&table.raw_ctrl_data);
    let row_height_sum = table.get_row_heights().iter().sum::<u32>();

    assert_eq!(
        table.common.height,
        original_height + 300,
        "하단선 resize는 기존 표시 height에 실제 행 높이 변화량만 반영해야 한다"
    );
    assert!(
        table.common.height > row_height_sum,
        "resize 후에도 생성 직후 표의 표시 height가 셀 저장 height 합으로 붕괴하면 안 된다"
    );
    assert_eq!(
        raw_common.height, table.common.height,
        "raw height와 in-memory common height는 동기화되어야 한다"
    );
}


fn issue_1470_count_rendered_tables(
    doc: &HwpDocument,
    para_idx: usize,
    control_idx: usize,
) -> usize {
    let layout = doc
        .get_page_control_layout_native(0)
        .expect("페이지 컨트롤 레이아웃");
    let parsed: Value = serde_json::from_str(&layout).expect("레이아웃 JSON");
    parsed["controls"]
        .as_array()
        .expect("controls 배열")
        .iter()
        .filter(|control| {
            control["type"] == "table"
                && control["paraIdx"].as_u64() == Some(para_idx as u64)
                && control["controlIdx"].as_u64() == Some(control_idx as u64)
        })
        .count()
}


fn issue_1470_table_caption_number(doc: &HwpDocument, control_idx: usize) -> Option<(u16, u16)> {
    use crate::model::control::Control;

    let table = match doc.document.sections[0].paragraphs[0]
        .controls
        .get(control_idx)?
    {
        Control::Table(t) => t,
        _ => return None,
    };
    table
        .caption
        .as_ref()?
        .paragraphs
        .first()?
        .controls
        .iter()
        .find_map(|c| match c {
            Control::AutoNumber(an) => Some((an.assigned_number, an.number)),
            _ => None,
        })
}


fn issue_1470_picture_caption_number(doc: &HwpDocument, control_idx: usize) -> Option<(u16, u16)> {
    use crate::model::control::Control;

    let picture = match doc.document.sections[0].paragraphs[0]
        .controls
        .get(control_idx)?
    {
        Control::Picture(p) => p,
        _ => return None,
    };
    picture
        .caption
        .as_ref()?
        .paragraphs
        .first()?
        .controls
        .iter()
        .find_map(|c| match c {
            Control::AutoNumber(an) => Some((an.assigned_number, an.number)),
            _ => None,
        })
}



#[test]
fn issue_1470_create_table_ex_tac_renders_once() {
    let mut doc = HwpDocument::create_empty();
    doc.insert_text_native(0, 0, 0, "본문 앞")
        .expect("본문 텍스트 입력");
    let insert_at = doc
        .get_paragraph_length_native(0, 0)
        .expect("문단 길이 조회");
    let created = doc
        .create_table_ex_native(
            0,
            0,
            insert_at,
            2,
            2,
            true,
            Some(&[4000, 6000]),
            Some(&[3000, 5000]),
        )
        .expect("TAC 표 생성");
    let created: Value = serde_json::from_str(&created).expect("생성 결과 JSON");
    let control_idx = created["controlIdx"].as_u64().expect("controlIdx") as usize;

    assert_eq!(
        issue_1470_count_rendered_tables(&doc, 0, control_idx),
        1,
        "문단 레이아웃에서 이미 그린 TAC 표를 PageItem 경로가 다시 그리면 안 된다"
    );
}



#[test]
fn issue_1470_create_table_ex_tac_caption_renders_once() {
    let mut doc = HwpDocument::create_empty();
    doc.insert_text_native(0, 0, 0, "캡션 표")
        .expect("본문 텍스트 입력");
    let insert_at = doc
        .get_paragraph_length_native(0, 0)
        .expect("문단 길이 조회");
    let created = doc
        .create_table_ex_native(0, 0, insert_at, 1, 1, true, None, None)
        .expect("TAC 표 생성");
    let created: Value = serde_json::from_str(&created).expect("생성 결과 JSON");
    let control_idx = created["controlIdx"].as_u64().expect("controlIdx") as usize;
    doc.set_table_properties_native(0, 0, control_idx, r#"{"hasCaption":true}"#)
        .expect("캡션 생성");

    assert_eq!(
        issue_1470_count_rendered_tables(&doc, 0, control_idx),
        1,
        "캡션이 있는 TAC 표도 같은 컨트롤이 한 번만 렌더되어야 한다"
    );
}



#[test]
fn issue_1470_picture_caption_can_be_removed_and_renumbers() {
    use crate::model::control::Control;

    fn minimal_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x00, 0x00, 0x00,
            0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    let mut doc = HwpDocument::create_empty();
    let image = minimal_png();
    let first = doc
        .insert_picture_native(
            0,
            0,
            0,
            &[],
            &image,
            5000,
            5000,
            1,
            1,
            "png",
            "first",
            None,
            None,
        )
        .expect("첫 번째 그림 삽입");
    let first_idx = issue_1481_json_usize(&first, "controlIdx");
    let second = doc
        .insert_picture_native(
            0,
            0,
            0,
            &[],
            &image,
            5000,
            5000,
            1,
            1,
            "png",
            "second",
            None,
            None,
        )
        .expect("두 번째 그림 삽입");
    let second_idx = issue_1481_json_usize(&second, "controlIdx");

    for control_idx in [first_idx, second_idx] {
        doc.set_picture_properties_native(0, 0, control_idx, r#"{"hasCaption":true}"#)
            .expect("그림 캡션 생성");
    }

    assert_eq!(
        issue_1470_picture_caption_number(&doc, first_idx),
        Some((1, 1))
    );
    assert_eq!(
        issue_1470_picture_caption_number(&doc, second_idx),
        Some((2, 2))
    );

    doc.set_picture_properties_native(0, 0, first_idx, r#"{"hasCaption":false}"#)
        .expect("그림 캡션 삭제");

    let first_picture = match &doc.document.sections[0].paragraphs[0].controls[first_idx] {
        Control::Picture(p) => p,
        other => panic!("첫 번째 컨트롤이 그림이 아님: {other:?}"),
    };
    assert!(
        first_picture.caption.is_none(),
        "hasCaption=false는 그림 캡션 슬롯을 삭제해야 한다"
    );
    assert_eq!(
        first_picture.common.attr & (1 << 29),
        0,
        "그림 캡션 attr bit도 내려야 한다"
    );
    let props: Value = serde_json::from_str(
        &doc.get_picture_properties_native(0, 0, first_idx)
            .expect("그림 속성 조회"),
    )
    .expect("그림 속성 JSON");
    assert_eq!(
        props["hasCaption"], false,
        "그림 속성창의 중앙 캡션 없음 선택은 hasCaption=false로 되돌아와야 한다"
    );
    assert_eq!(
        issue_1470_picture_caption_number(&doc, second_idx),
        Some((1, 1)),
        "앞 그림 캡션 삭제 후 뒤 그림 캡션 번호가 1로 재배정되어야 한다"
    );
}



#[test]
fn issue_1470_picture_caption_path_cursor_and_control_paste() {
    use crate::model::control::Control;

    fn minimal_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x00, 0x00, 0x00,
            0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    let mut doc = HwpDocument::create_empty();
    let image = minimal_png();
    let inserted = doc
        .insert_picture_native(
            0,
            0,
            0,
            &[],
            &image,
            5000,
            5000,
            1,
            1,
            "png",
            "caption-path",
            None,
            None,
        )
        .expect("그림 삽입");
    let pic_idx = issue_1481_json_usize(&inserted, "controlIdx");
    doc.set_picture_properties_native(0, 0, pic_idx, r#"{"hasCaption":true}"#)
        .expect("그림 캡션 생성");

    let path = [(pic_idx, 0usize, 0usize)];
    let path_json = format!(
        r#"[{{"controlIndex":{},"cellIndex":0,"cellParaIndex":0}}]"#,
        pic_idx
    );
    let rect = doc.get_cursor_rect_by_path_native(0, 0, &path_json, 0);
    assert!(
        rect.is_ok(),
        "그림 캡션 cellPath도 커서 좌표를 찾아야 한다: {:?}",
        rect.err()
    );

    doc.copy_control_native(0, 0, &[], pic_idx)
        .expect("그림 개체 복사");
    let pasted = doc.paste_internal_in_cell_by_path_native(0, 0, &path, 0);
    assert!(
        pasted.is_ok(),
        "그림 캡션 위치에도 내부 그림 클립보드를 붙여넣을 수 있어야 한다: {:?}",
        pasted.err()
    );
    let picture = match &doc.document.sections[0].paragraphs[0].controls[pic_idx] {
        Control::Picture(p) => p,
        other => panic!("그림 컨트롤이 아님: {other:?}"),
    };
    let caption = picture.caption.as_ref().expect("그림 캡션 존재");
    assert!(
        caption.paragraphs[0]
            .controls
            .iter()
            .any(|control| matches!(control, Control::Picture(_))),
        "그림 caption path 붙여넣기는 캡션 문단 안에 그림 컨트롤을 보존해야 한다"
    );
}



#[test]
fn issue_1470_table_caption_keeps_autonumber_and_can_be_removed() {
    use crate::model::control::Control;

    let mut doc = HwpDocument::create_empty();
    doc.create_table_ex_native(0, 0, 0, 1, 1, true, None, None)
        .expect("표 생성");

    doc.set_table_properties_native(0, 0, 0, r#"{"hasCaption":true}"#)
        .expect("캡션 생성");
    let table = match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(t) => t,
        other => panic!("표가 아님: {other:?}"),
    };
    let caption = table.caption.as_ref().expect("캡션 존재");
    let cap_para = caption.paragraphs.first().expect("캡션 문단");
    assert_eq!(cap_para.text, "표  ");
    assert_eq!(cap_para.char_count, 13);
    assert_eq!(cap_para.char_offsets, vec![0, 1, 2, 11]);
    assert!(
        cap_para
            .controls
            .iter()
            .any(|c| matches!(c, Control::AutoNumber(_))),
        "표 캡션 번호는 literal 텍스트가 아니라 AutoNumber 컨트롤로 유지되어야 한다"
    );

    doc.set_table_properties_native(0, 0, 0, r#"{"hasCaption":false}"#)
        .expect("캡션 삭제");
    let table = match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(t) => t,
        other => panic!("표가 아님: {other:?}"),
    };
    assert!(
        table.caption.is_none(),
        "hasCaption=false가 기존 캡션을 삭제해야 한다"
    );
    assert_eq!(table.attr & (1 << 29), 0, "캡션 attr bit도 내려야 한다");
}



#[test]
fn issue_1470_table_caption_renumbers_after_delete() {
    let mut doc = HwpDocument::create_empty();
    for _ in 0..3 {
        doc.create_table_ex_native(0, 0, 0, 1, 1, true, None, None)
            .expect("표 생성");
    }
    for control_idx in 0..3 {
        doc.set_table_properties_native(0, 0, control_idx, r#"{"hasCaption":true}"#)
            .expect("캡션 생성");
    }

    assert_eq!(issue_1470_table_caption_number(&doc, 0), Some((1, 1)));
    assert_eq!(issue_1470_table_caption_number(&doc, 1), Some((2, 2)));
    assert_eq!(issue_1470_table_caption_number(&doc, 2), Some((3, 3)));

    doc.set_table_properties_native(0, 0, 1, r#"{"hasCaption":false}"#)
        .expect("중간 캡션 삭제");
    doc.set_table_properties_native(
        0,
        0,
        2,
        r#"{"captionDirection":0,"captionVertAlign":1,"captionWidth":2400,"captionSpacing":600}"#,
    )
    .expect("뒤 캡션 속성 수정");

    assert_eq!(
        issue_1470_table_caption_number(&doc, 0),
        Some((1, 1)),
        "앞 표 캡션 번호는 1을 유지해야 한다"
    );
    assert_eq!(
        issue_1470_table_caption_number(&doc, 1),
        None,
        "삭제한 중간 표 캡션은 없어야 한다"
    );
    assert_eq!(
        issue_1470_table_caption_number(&doc, 2),
        Some((2, 2)),
        "중간 캡션 삭제 후 뒤 표 캡션의 assigned_number/number가 2로 재배정되어야 한다"
    );

    let svg = doc.render_page_svg_native(0).expect("SVG 렌더링");
    assert!(
        svg.contains(">표<") && svg.contains(">1<") && svg.contains(">2<") && !svg.contains(">3<"),
        "렌더링 결과도 중간 캡션 삭제 후 표 1, 표 2만 표시해야 한다"
    );
}



#[test]
fn issue_1470_table_caption_edit_keeps_autonumber() {
    use crate::model::control::Control;
    use crate::model::shape::{CaptionDirection, CaptionVertAlign};

    let mut doc = HwpDocument::create_empty();
    doc.create_table_ex_native(0, 0, 0, 1, 1, true, None, None)
        .expect("표 생성");
    doc.set_table_properties_native(0, 0, 0, r#"{"hasCaption":true}"#)
        .expect("캡션 생성");

    doc.set_table_properties_native(
        0,
        0,
        0,
        r#"{"captionDirection":0,"captionVertAlign":1,"captionWidth":2400,"captionSpacing":600}"#,
    )
    .expect("캡션 속성 수정");

    let table = match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(t) => t,
        other => panic!("표가 아님: {other:?}"),
    };
    let caption = table.caption.as_ref().expect("캡션 존재");
    assert_eq!(caption.direction, CaptionDirection::Left);
    assert_eq!(caption.vert_align, CaptionVertAlign::Center);
    assert_eq!(caption.width, 2400);
    assert_eq!(caption.spacing, 600);

    let cap_para = caption.paragraphs.first().expect("캡션 문단");
    assert_eq!(cap_para.text, "표  ");
    assert_eq!(cap_para.char_offsets, vec![0, 1, 2, 11]);
    assert!(
        cap_para.controls.iter().any(
            |c| matches!(c, Control::AutoNumber(an) if an.assigned_number == 1 && an.number == 1)
        ),
        "캡션 속성 수정 후에도 AutoNumber 컨트롤과 번호가 유지되어야 한다"
    );
}



#[test]
fn issue2424_deferred_delete_preserves_immediate_schema_and_tracks_ime_revision() {
    let mut immediate = create_doc_with_table();
    let immediate_raw = immediate
        .delete_text_in_cell_native(0, 0, 0, 0, 0, 1, 1)
        .expect("immediate cell delete");
    let immediate_result: Value =
        serde_json::from_str(&immediate_raw).expect("immediate delete json");
    assert_eq!(immediate_result["charOffset"], 1);
    assert!(
        immediate_result.get("cellFlowChanged").is_none(),
        "existing immediate response schema must remain unchanged"
    );

    let mut doc = create_doc_with_table();
    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 1, "ㅎ")
        .expect("first IME insert");
    let first_revision = doc
        .deferred_pagination_descriptor
        .as_ref()
        .expect("first IME descriptor")
        .revision;

    let delete_raw = doc
        .delete_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 1, 1)
        .expect("IME replacement delete");
    let delete_result: Value = serde_json::from_str(&delete_raw).expect("deferred delete json");
    assert_eq!(delete_result["charOffset"], 1);
    assert!(delete_result["cellFlowChanged"].is_boolean());
    let delete_revision = doc
        .deferred_pagination_descriptor
        .as_ref()
        .expect("delete descriptor")
        .revision;
    assert!(delete_revision > first_revision);

    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 1, "하")
        .expect("second IME insert");
    let final_descriptor = doc
        .deferred_pagination_descriptor
        .as_ref()
        .expect("latest IME descriptor");
    assert!(final_descriptor.revision > delete_revision);
    assert_eq!(
        (
            final_descriptor.section_index,
            final_descriptor.para_index,
            final_descriptor.control_index,
            final_descriptor.cell_index,
            final_descriptor.cell_para_index,
        ),
        (0, 0, 0, 0, 0)
    );
    match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => assert_eq!(table.cells[0].paragraphs[0].text, "셀하A"),
        other => panic!("table control expected: {other:?}"),
    }

    doc.flush_deferred_pagination().expect("IME output barrier");
    assert!(doc.deferred_pagination_descriptor.is_none());
}



#[test]
fn issue2424_page_count_is_held_until_shadow_layout_commits() {
    let mut doc = create_doc_with_page_count_boundary_table();
    let initial_page_count = doc.page_count();
    assert_eq!(initial_page_count, 1, "fixture must begin on one page");

    let inserted = "가".repeat(48);
    let edit_raw = doc
        .insert_text_in_cell_native_deferred_pagination(0, 0, 0, 12, 0, 1, &inserted)
        .expect("deferred boundary insert");
    let edit: Value = serde_json::from_str(&edit_raw).expect("edit json");
    assert_eq!(edit["cellFlowChanged"], true, "fixture must add cell lines");
    assert_eq!(
        doc.page_count(),
        initial_page_count,
        "deferred edit must keep the public page count"
    );

    let begin = doc.core.begin_deferred_pagination(1);
    assert_eq!(begin.state, DeferredPaginationJobState::Pending);
    assert_eq!(begin.page_count, initial_page_count);

    let completed = loop {
        let step = doc.core.step_deferred_pagination(1);
        match step.state {
            DeferredPaginationJobState::Pending => {
                assert_eq!(
                    step.page_count, initial_page_count,
                    "incomplete shadow fragments must not publish a page count"
                );
            }
            DeferredPaginationJobState::Complete => break step,
            state => panic!("unexpected shadow status: {state:?}"),
        }
    };
    assert!(
        completed.page_count > initial_page_count,
        "final shadow commit must publish the added page: {completed:?}"
    );
    assert_eq!(doc.page_count(), completed.page_count);
}



#[test]
fn issue2214_deferred_table_caption_reports_flow_change() {
    use crate::model::shape::{Caption, CaptionDirection};

    fn caption_paragraph(doc: &HwpDocument) -> &Paragraph {
        match &doc.document.sections[0].paragraphs[0].controls[0] {
            Control::Table(table) => &table.caption.as_ref().expect("table caption").paragraphs[0],
            other => panic!("table control expected: {other:?}"),
        }
    }

    fn relative_flow(paragraph: &Paragraph) -> Option<i64> {
        let first = paragraph.line_segs.first()?;
        let last = paragraph.line_segs.last()?;
        Some(
            i64::from(last.vertical_pos)
                + i64::from(last.line_height)
                + i64::from(last.line_spacing)
                - i64::from(first.vertical_pos),
        )
    }

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

    let mut saw_boundary = false;
    for inserted in 0..32 {
        let before = relative_flow(caption_paragraph(&doc));
        let raw = doc
            .insert_text_in_cell_native_deferred_pagination(0, 0, 0, 65534, 0, 1 + inserted, "가")
            .expect("deferred caption insert");
        let after = relative_flow(caption_paragraph(&doc));
        let result: Value = serde_json::from_str(&raw).expect("caption edit result json");
        let reported = result["cellFlowChanged"]
            .as_bool()
            .expect("caption flow result");
        assert_eq!(
            reported,
            before != after,
            "caption input {} flow signal",
            inserted + 1
        );
        if reported {
            saw_boundary = true;
            assert!(
                caption_paragraph(&doc).line_segs.len() > 1,
                "caption flow boundary must add a line"
            );
            break;
        }
    }
    assert!(
        saw_boundary,
        "caption deferred input must report a wrapping flow boundary"
    );
}



#[test]
fn issue2424_deferred_pagination_descriptor_tracks_latest_edit_until_flush() {
    let mut doc = create_doc_with_table();
    assert!(doc.deferred_pagination_descriptor.is_none());

    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 1, "x")
        .expect("first deferred insert");
    let first = doc
        .deferred_pagination_descriptor
        .clone()
        .expect("first target descriptor");
    assert_eq!(first.revision, 1);
    assert_eq!(
        (
            first.section_index,
            first.para_index,
            first.control_index,
            first.cell_index,
            first.cell_para_index,
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(first.target_first_page, Some(0));
    assert_ne!(first.table_structure_fingerprint, 0);
    assert_eq!(
        doc.deferred_pagination_target_status(&first),
        crate::document_core::DeferredPaginationTargetStatus::Current
    );

    // 앞선 입력에서 이미 flow boundary가 있었다고 가정하면 같은 target의 후속 stable
    // 입력이 descriptor의 pending boundary를 지우면 안 된다.
    doc.deferred_pagination_descriptor
        .as_mut()
        .expect("pending descriptor")
        .cell_flow_changed = true;

    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 2, "y")
        .expect("replacement deferred insert");
    let second = doc
        .deferred_pagination_descriptor
        .as_ref()
        .expect("replacement target descriptor");
    assert_eq!(second.revision, 2);
    assert!(second.cell_flow_changed);
    assert_eq!(
        second.table_structure_fingerprint, first.table_structure_fingerprint,
        "text-only edit must preserve the target table structure"
    );
    assert_eq!(
        doc.deferred_pagination_target_status(&first),
        crate::document_core::DeferredPaginationTargetStatus::Superseded,
        "a newer deferred edit must invalidate an older job revision"
    );
    let second = second.clone();
    assert_eq!(
        doc.deferred_pagination_target_status(&second),
        crate::document_core::DeferredPaginationTargetStatus::Current
    );

    let removed_paragraph = match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => table.cells[0]
            .paragraphs
            .pop()
            .expect("target cell paragraph"),
        _ => panic!("target table"),
    };
    assert_eq!(
        doc.deferred_pagination_target_status(&second),
        crate::document_core::DeferredPaginationTargetStatus::TargetMissing
    );
    match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => table.cells[0].paragraphs.push(removed_paragraph),
        _ => panic!("target table"),
    }

    match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => table.row_count = table.row_count.saturating_add(1),
        _ => panic!("target table"),
    }
    assert_eq!(
        doc.deferred_pagination_target_status(&second),
        crate::document_core::DeferredPaginationTargetStatus::StructureChanged
    );
    match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => table.row_count = table.row_count.saturating_sub(1),
        _ => panic!("target table"),
    }
    assert_eq!(
        doc.deferred_pagination_target_status(&second),
        crate::document_core::DeferredPaginationTargetStatus::Current
    );

    match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => table.cells[0].paragraphs[0]
            .controls
            .push(Control::Bookmark(Default::default())),
        _ => panic!("target table"),
    }
    assert_eq!(
        doc.deferred_pagination_target_status(&second),
        crate::document_core::DeferredPaginationTargetStatus::StructureChanged,
        "cell paragraph control structure changes must invalidate the descriptor"
    );
    match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Table(table) => {
            table.cells[0].paragraphs[0].controls.pop();
        }
        _ => panic!("target table"),
    }
    assert_eq!(
        doc.deferred_pagination_target_status(&second),
        crate::document_core::DeferredPaginationTargetStatus::Current
    );

    let third_raw = doc
        .insert_text_in_cell_native_deferred_pagination(0, 0, 0, 1, 0, 0, "z")
        .expect("different target deferred insert");
    let third_result: Value = serde_json::from_str(&third_raw).expect("different target result");
    let third = doc
        .deferred_pagination_descriptor
        .as_ref()
        .expect("different target descriptor");
    assert_eq!(third.revision, 3);
    assert_eq!(third.cell_index, 1);
    assert_eq!(
        third.cell_flow_changed,
        third_result["cellFlowChanged"].as_bool().unwrap(),
        "a different target must not inherit the previous flow signal"
    );

    doc.flush_deferred_pagination().expect("full flush");
    assert!(
        doc.deferred_pagination_descriptor.is_none(),
        "successful full pagination must consume the pending descriptor"
    );
}



#[test]
fn issue2308_deferred_cell_edit_uses_path_revision_without_section_invalidation() {
    use crate::renderer::render_normalization::RenderPathEntry;

    let mut doc = create_doc_with_table();
    let section_revision_before = doc.render_normalization.section_revisions[0];

    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 0, 0, 0, 1, "가")
        .expect("deferred table-cell insert");

    assert_eq!(
        doc.render_normalization.section_revisions[0], section_revision_before,
        "a structure-stable cell edit must not invalidate the section projection"
    );
    let revision = doc
        .render_normalization
        .path_revisions
        .iter()
        .find_map(|(path, revision)| match path.entries.as_slice() {
            [RenderPathEntry::TableCell {
                control_index: 0,
                cell_index: 0,
                paragraph_index: 0,
            }] => Some(*revision),
            _ => None,
        });
    assert_eq!(revision, Some(1), "the edited logical path revision");
}



#[test]
fn issue2308_immediate_edit_rederives_existing_compat_projection() {
    use crate::model::image::Picture;
    use crate::model::shape::{CommonObjAttr, TextWrap};

    fn floating_picture() -> Control {
        Control::Picture(Box::new(Picture {
            common: CommonObjAttr {
                height: 50_000,
                text_wrap: TextWrap::Square,
                allow_overlap: false,
                treat_as_char: false,
                ..Default::default()
            },
            ..Default::default()
        }))
    }

    let mut doc = create_doc_with_table();
    let Control::Table(table) = &mut doc.document.sections[0].paragraphs[0].controls[0] else {
        panic!("table control");
    };
    table.cells[0].paragraphs[0] = Paragraph {
        controls: vec![floating_picture(), floating_picture()],
        line_segs: vec![LineSeg {
            line_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }],
        ..Default::default()
    };
    let document = doc.document.clone();
    doc.set_document(document);
    assert!(
        doc.render_normalization.sections[0].is_some(),
        "the synthetic cell image stack must create a #2004 compatibility projection"
    );
    let revision_before = doc.render_normalization.section_revisions[0];

    doc.insert_text_in_cell_native(0, 0, 0, 0, 0, 0, "x")
        .expect("immediate edit in a projected cell");

    assert_ne!(
        doc.render_normalization.section_revisions[0], revision_before,
        "an existing compatibility projection must be invalidated"
    );
    assert!(
        doc.render_normalization.sections[0].is_none(),
        "visible source text removes the stack gate, so no stale projection may survive"
    );
}



#[test]
fn issue2214_invalid_shape_cell_index_does_not_mutate_text() {
    let mut doc = HwpDocument::create_empty();
    let inserted = doc
        .create_shape_control_native(
            0,
            0,
            0,
            21_600,
            7_200,
            0,
            0,
            true,
            "TopAndBottom",
            "textbox",
            false,
            false,
            &[],
        )
        .expect("create textbox shape");
    let inserted: Value = serde_json::from_str(&inserted).expect("shape result json");
    let para_idx = inserted["paraIdx"].as_u64().expect("shape paraIdx") as usize;
    let control_idx = inserted["controlIdx"].as_u64().expect("shape controlIdx") as usize;
    let before = doc
        .get_cell_paragraph_ref(0, para_idx, control_idx, 0, 0)
        .expect("textbox paragraph")
        .text
        .clone();

    let result =
        doc.insert_text_in_cell_native_deferred_pagination(0, para_idx, control_idx, 1, 0, 0, "x");

    assert!(result.is_err(), "nonzero Shape cell index must fail");
    assert_eq!(
        doc.get_cell_paragraph_ref(0, para_idx, control_idx, 0, 0)
            .expect("textbox paragraph after invalid call")
            .text,
        before,
        "invalid Shape cell index must fail before mutation"
    );
}



#[derive(Clone, Debug, PartialEq, Eq)]
struct Issue2214TargetCut {
    page_index: u32,
    start_row: usize,
    end_row: usize,
    is_continuation: bool,
    start_cut: Vec<usize>,
    end_cut: Vec<usize>,
    is_block_split: bool,
}


fn issue2214_target_cuts(doc: &HwpDocument) -> Vec<Issue2214TargetCut> {
    use crate::renderer::pagination::PageItem;

    let pages = doc
        .core
        .pagination
        .iter()
        .flat_map(|section| section.pages.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        pages.len(),
        doc.page_count() as usize,
        "pagination page coverage"
    );
    pages
        .into_iter()
        .enumerate()
        .map(|(global_page, page)| {
            assert_eq!(page.section_index, 0, "#2214 target section");
            assert_eq!(
                page.page_index as usize, global_page,
                "#2214 global page index"
            );
            let matches = page
                .column_contents
                .iter()
                .flat_map(|column| column.items.iter())
                .filter_map(|item| match item {
                    PageItem::PartialTable {
                        para_index: 0,
                        control_index: 2,
                        start_row,
                        end_row,
                        is_continuation,
                        start_cut,
                        end_cut,
                        is_block_split,
                        ..
                    } => Some(Issue2214TargetCut {
                        page_index: page.page_index,
                        start_row: *start_row,
                        end_row: *end_row,
                        is_continuation: *is_continuation,
                        start_cut: start_cut.clone(),
                        end_cut: end_cut.clone(),
                        is_block_split: *is_block_split,
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "page {global_page}: exactly one target PartialTable fragment"
            );
            matches.into_iter().next().expect("one target fragment")
        })
        .collect()
}


fn issue2214_assert_cut_continuity(label: &str, state: &str, cuts: &[Issue2214TargetCut]) {
    assert_eq!(cuts.len(), 115, "{label} {state}: target page coverage");
    assert!(!cuts[0].is_continuation, "{label} {state}: first fragment");
    assert!(
        cuts[0].start_cut.is_empty(),
        "{label} {state}: first fragment starts at row origin"
    );
    assert!(
        cuts.last().expect("last target cut").end_cut.is_empty(),
        "{label} {state}: final fragment consumes the target table"
    );
    assert!(
        cuts.iter().all(|cut| !cut.is_block_split),
        "{label} {state}: #2214 fixture must remain a non-block split chain"
    );
    for cut in cuts {
        assert!(
            cut.start_row < cut.end_row,
            "{label} {state}: page {} row range must advance",
            cut.page_index
        );
        if cut.start_row + 1 == cut.end_row && !cut.start_cut.is_empty() && !cut.end_cut.is_empty()
        {
            assert_eq!(
                cut.start_cut.len(),
                cut.end_cut.len(),
                "{label} {state}: page {} cut arity",
                cut.page_index
            );
            assert!(
                cut.start_cut
                    .iter()
                    .zip(&cut.end_cut)
                    .all(|(start, end)| end >= start),
                "{label} {state}: page {} cut components must not rewind",
                cut.page_index
            );
            assert!(
                cut.start_cut
                    .iter()
                    .zip(&cut.end_cut)
                    .any(|(start, end)| end > start),
                "{label} {state}: page {} cut must consume at least one unit",
                cut.page_index
            );
        }
    }
    for (page, pair) in cuts.windows(2).enumerate() {
        assert!(
            pair[1].is_continuation,
            "{label} {state}: page {} must be a continuation",
            page + 1
        );
        if pair[0].end_cut.is_empty() {
            assert!(
                pair[1].start_cut.is_empty(),
                "{label} {state}: page {} row boundary must restart without a cut",
                page + 1
            );
            assert_eq!(
                pair[1].start_row,
                pair[0].end_row,
                "{label} {state}: page {} row boundary must be contiguous",
                page + 1
            );
        } else {
            assert_eq!(
                pair[0].end_cut,
                pair[1].start_cut,
                "{label} {state}: page {} end_cut must equal page {} start_cut",
                page,
                page + 1
            );
            assert_eq!(
                pair[1].start_row,
                pair[0].end_row - 1,
                "{label} {state}: page {} split row must continue",
                page + 1
            );
        }
    }
}



/// #2430의 giant-cell target은 원본 형식마다 한컴 2020 편집 후 줄 경계가 다르다.
///
/// HWP 저장 LINE_SEG는 56번째 ASCII `1`에서 fifth line으로 전환하지만, HWPX는
/// 같은 한컴 2020 adapter-save oracle에서 61번째가 전환점이다 (Task #3820 Stage 86).
/// 이 값을 각 test에 따로 쓰면 HWPX의 실제 61회 경계를 다시 HWP 값으로 회귀시킨다.
fn issue2214_flow_boundary_insert_count(label: &str) -> usize {
    match label {
        "hwp" => 56,
        "hwpx" => 61,
        other => panic!("unknown #2214 fixture label: {other}"),
    }
}



/// #2214 Stage 3: scoped cache coherence는 deferred pagination geometry를 유지하면서
/// warm tree/cursor만 최신 edit으로 복구하고, explicit flush에서만 cut/bounds를 갱신한다.
#[test]
fn issue2214_scoped_cache_coherence_preserves_transient_pagination() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    fn target_tree_ranges(doc: &HwpDocument) -> Vec<(u32, usize, usize)> {
        fn visit(node: &RenderNode, page: u32, ranges: &mut Vec<(u32, usize, usize)>) {
            if let RenderNodeType::TextRun(run) = &node.node_type {
                if let (Some(start), Some(ctx)) = (run.char_start, run.cell_context.as_ref()) {
                    let target = ctx.parent_para_index == 0
                        && ctx.path.len() == 1
                        && ctx.path.first().is_some_and(|entry| {
                            entry.control_index == 2
                                && entry.cell_index == 2
                                && entry.cell_para_index == 5
                        });
                    if target {
                        assert!(run.char_overlap.is_none(), "target run must not overlap");
                        assert_eq!(
                            run.text.chars().count(),
                            run.text.encode_utf16().count(),
                            "fixture target run must be BMP"
                        );
                        let end = start + run.text.encode_utf16().count();
                        assert!(end > start, "target run must advance");
                        ranges.push((page, start, end));
                    }
                }
            }
            for child in &node.children {
                visit(child, page, ranges);
            }
        }

        let page = 0;
        let tree = doc
            .build_page_render_tree(page)
            .unwrap_or_else(|e| panic!("page {page} tree: {e}"));
        let mut ranges = Vec::new();
        visit(&tree.root, page, &mut ranges);
        ranges.sort_unstable_by_key(|(_, start, end)| (*start, *end));
        assert!(!ranges.is_empty(), "target paragraph ranges");
        let mut contiguous_end = 0;
        for (page, start, end) in &ranges {
            assert_eq!(
                *start, contiguous_end,
                "page {page}: target UTF-16 ranges must have no gap or overlap"
            );
            contiguous_end = *end;
        }
        ranges
    }

    for (label, relative) in [
        ("hwp", "samples/issue1949_giant_cell_nested_tables_perf.hwp"),
        (
            "hwpx",
            "samples/issue1949_giant_cell_nested_tables_perf.hwpx",
        ),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let bytes = std::fs::read(path).expect("read #2214 fixture");
        let mut doc = HwpDocument::from_bytes(&bytes).expect("load #2214 fixture");
        let boundary_inserts = issue2214_flow_boundary_insert_count(label);
        let target_end = 130 + boundary_inserts;

        // 실제 Studio처럼 편집 전에 페이지 트리/셀 유닛을 warm한다.
        let initial_ranges = target_tree_ranges(&doc);
        assert_eq!(
            initial_ranges.last().map(|(_, _, end)| *end),
            Some(130),
            "{label}: initial max char"
        );
        doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, 130)
            .expect("warm target cursor");
        let initial_cuts = issue2214_target_cuts(&doc);
        issue2214_assert_cut_continuity(label, "initial", &initial_cuts);

        // #2195 이후에도 44번째 입력은 target paragraph의 상대 flow advance를 바꾼다.
        // 다만 선언 셀 높이가 증가분을 흡수해 full pagination의 cut/bounds는 불변이다.
        // render_normalized warm tree는 flush 전에도 매 mutation을 즉시 반영해야 한다.
        // [#2430] HY/한양 ASCII advance는 0.497em이다. 그러나 저장 HWP LINE_SEG와
        // HWPX adapter layout의 실제 한컴 2020 전환점은 각각 56/61회다.
        for inserted in 0..boundary_inserts {
            let raw = doc
                .insert_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 130 + inserted, "1")
                .expect("deferred sequential insert");
            let result: Value = serde_json::from_str(&raw).expect("edit result json");
            assert_eq!(
                result["cellFlowChanged"].as_bool(),
                Some(inserted + 1 == boundary_inserts),
                "{label}: input {} flow signal",
                inserted + 1
            );
        }
        let transient_cuts = issue2214_target_cuts(&doc);
        issue2214_assert_cut_continuity(label, "transient", &transient_cuts);
        let transient_cut = transient_cuts[0].clone();
        let transient_ranges = target_tree_ranges(&doc);
        let transient_max = transient_ranges
            .last()
            .map(|(_, _, end)| *end)
            .expect("transient target end");
        let transient_rect = doc
            .get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, target_end)
            .expect("transient direct rect");

        doc.flush_deferred_pagination()
            .expect("explicit pagination control");
        let flushed_cuts = issue2214_target_cuts(&doc);
        issue2214_assert_cut_continuity(label, "full-flush", &flushed_cuts);
        let flushed_cut = flushed_cuts[0].clone();
        let flushed_ranges = target_tree_ranges(&doc);
        let flushed_max = flushed_ranges
            .last()
            .map(|(_, _, end)| *end)
            .expect("flushed target end");
        let flushed_rect = doc
            .get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, target_end)
            .expect("flushed direct rect");

        eprintln!(
            "#2214 {label}: transient max={transient_max} rect={transient_rect}; flushed max={flushed_max} rect={flushed_rect}; cuts transient={transient_cut:?} flushed={flushed_cut:?}"
        );

        assert_eq!(
            transient_max, target_end,
            "{label}: scoped warm tree coherence"
        );
        assert_eq!(flushed_max, target_end, "{label}: flush oracle");
        assert_eq!(
            transient_ranges, flushed_ranges,
            "{label}: transient target UTF-16 ranges must equal flush oracle"
        );
        assert_eq!(
            initial_cuts, transient_cuts,
            "{label}: scoped eviction must not change pagination fragments"
        );
        assert_eq!(transient_cut.start_cut, Vec::<usize>::new());
        assert_eq!(
            transient_cut.end_cut,
            vec![37],
            "{label}: transient page-zero cut"
        );
        assert_eq!(flushed_cut.start_cut, Vec::<usize>::new());
        assert_eq!(
            flushed_cut.end_cut,
            vec![37],
            "{label}: flushed page-zero cut"
        );
        assert_eq!(
            transient_cut, flushed_cut,
            "{label}: #2195 declared height must absorb the first-page advance"
        );
        let changed_pages = transient_cuts
            .iter()
            .zip(&flushed_cuts)
            .enumerate()
            .filter_map(|(page, (transient, flushed))| (transient != flushed).then_some(page))
            .collect::<Vec<_>>();
        eprintln!(
            "#2214 {label}: PartialTable fragments={} changed_after_flush_count={}",
            transient_cuts.len(),
            changed_pages.len(),
        );
        assert_eq!(
            transient_cuts.len(),
            flushed_cuts.len(),
            "{label}: page fingerprint count"
        );
        assert_eq!(
            changed_pages,
            (2..doc.page_count() as usize).collect::<Vec<_>>(),
            "{label}: flush must realign downstream continuation cuts"
        );
        let transient_rect_json: Value =
            serde_json::from_str(&transient_rect).expect("transient rect json");
        let flushed_rect_json: Value =
            serde_json::from_str(&flushed_rect).expect("flushed rect json");
        for key in ["pageIndex", "x", "y", "height", "cellOverflowed"] {
            assert_eq!(
                transient_rect_json.get(key),
                flushed_rect_json.get(key),
                "{label}: transient cursor field {key} must equal flush oracle"
            );
        }
        assert_eq!(
            transient_rect_json.get("cellBounds"),
            flushed_rect_json.get("cellBounds"),
            "{label}: absorbed flow boundary must preserve cell bounds"
        );
        let transient_bounds_h = transient_rect_json["cellBounds"]["h"]
            .as_f64()
            .expect("transient bounds h");
        let flushed_bounds_h = flushed_rect_json["cellBounds"]["h"]
            .as_f64()
            .expect("flushed bounds h");
        assert!(
            (transient_bounds_h - 945.9).abs() <= 0.2,
            "{label}: transient bounds h={transient_bounds_h}"
        );
        assert!(
            (flushed_bounds_h - 945.9).abs() <= 0.2,
            "{label}: flushed bounds h={flushed_bounds_h}"
        );
        assert_eq!(doc.page_count(), 115, "{label}: page count");
    }
}



/// #3137 Stage 3: stable cell edit가 돌려준 local x delta는 직전 absolute rect에 적용했을 때
/// cache miss page-tree rebuild로 얻은 exact rect와 같아야 한다.
#[test]
fn issue3137_focused_cell_geometry_matches_exact_rect() {
    #[derive(Debug, PartialEq, Eq)]
    struct FocusedRunSnapshot {
        text: String,
        char_start: Option<usize>,
        char_shape_id: Option<u32>,
        para_shape_id: Option<u16>,
        is_para_end: bool,
        border_fill_id: u16,
        bbox_bits: [u64; 4],
        baseline_bits: u64,
        font_family: String,
        font_size_bits: u64,
        letter_spacing_bits: u64,
        ratio_bits: u64,
        line_x_offset_bits: u64,
        available_width_bits: u64,
    }

    fn focused_line_snapshot(
        tree: &crate::renderer::render_tree::PageRenderTree,
    ) -> ([u64; 4], Vec<FocusedRunSnapshot>) {
        use crate::renderer::render_tree::{RenderNode, RenderNodeType};

        fn visit(node: &RenderNode) -> Option<([u64; 4], Vec<FocusedRunSnapshot>)> {
            if let RenderNodeType::TextLine(line) = &node.node_type {
                if line.section_index == Some(0)
                    && line.para_index == Some(5)
                    && line.line_index == Some(3)
                    && node.children.iter().all(|child| {
                        matches!(
                            &child.node_type,
                            RenderNodeType::TextRun(run)
                                if run.cell_context.as_ref().is_some_and(|context| {
                                    context.parent_para_index == 0
                                        && context.path.len() == 1
                                        && context.path[0].control_index == 2
                                        && context.path[0].cell_index == 2
                                        && context.path[0].cell_para_index == 5
                                })
                        )
                    })
                {
                    let runs = node
                        .children
                        .iter()
                        .map(|child| {
                            let RenderNodeType::TextRun(run) = &child.node_type else {
                                unreachable!("focused line children were validated as TextRun");
                            };
                            FocusedRunSnapshot {
                                text: run.text.clone(),
                                char_start: run.char_start,
                                char_shape_id: run.char_shape_id,
                                para_shape_id: run.para_shape_id,
                                is_para_end: run.is_para_end,
                                border_fill_id: run.border_fill_id,
                                bbox_bits: [
                                    child.bbox.x.to_bits(),
                                    child.bbox.y.to_bits(),
                                    child.bbox.width.to_bits(),
                                    child.bbox.height.to_bits(),
                                ],
                                baseline_bits: run.baseline.to_bits(),
                                font_family: run.style.font_family.clone(),
                                font_size_bits: run.style.font_size.to_bits(),
                                letter_spacing_bits: run.style.letter_spacing.to_bits(),
                                ratio_bits: run.style.ratio.to_bits(),
                                line_x_offset_bits: run.style.line_x_offset.to_bits(),
                                available_width_bits: run.style.available_width.to_bits(),
                            }
                        })
                        .collect();
                    return Some((
                        [
                            node.bbox.x.to_bits(),
                            node.bbox.y.to_bits(),
                            node.bbox.width.to_bits(),
                            node.bbox.height.to_bits(),
                        ],
                        runs,
                    ));
                }
            }
            node.children.iter().find_map(visit)
        }

        visit(&tree.root).expect("focused final TextLine")
    }

    fn assert_cached_line_matches_fresh(
        doc: &HwpDocument,
        label: &str,
        operation: &str,
        page_index: u32,
    ) {
        let cached = {
            let cache = doc.core.page_tree_cache.borrow();
            focused_line_snapshot(
                cache
                    .get(page_index as usize)
                    .and_then(Option::as_ref)
                    .unwrap_or_else(|| panic!("focused page tree cache page={page_index}")),
            )
        };
        let fresh = focused_line_snapshot(
            &doc.build_page_render_tree(page_index)
                .expect("fresh focused page render tree"),
        );
        assert_eq!(
            cached, fresh,
            "{label} {operation}: patched TextLine must equal a fresh page build"
        );
    }

    fn focused_patch_page_index(mutation: &Value, label: &str, operation: &str) -> u32 {
        mutation["focusedPagePatch"]["pageIndex"]
            .as_u64()
            .unwrap_or_else(|| {
                panic!("{label} {operation}: missing focused patch page: {mutation}")
            }) as u32
    }

    fn rect_number(rect: &Value, key: &str) -> f64 {
        rect[key]
            .as_f64()
            .unwrap_or_else(|| panic!("cursor rect field {key}: {rect}"))
    }

    fn assert_geometry(
        label: &str,
        operation: &str,
        before_rect: &Value,
        mutation: &Value,
        after_rect: &Value,
        expected_source: u64,
        expected_target: u64,
    ) {
        assert_eq!(
            mutation["cellFlowChanged"].as_bool(),
            Some(false),
            "{label} {operation}: stable flow"
        );
        assert_eq!(
            mutation["focusedPageTreePatched"].as_bool(),
            Some(true),
            "{label} {operation}: focused page tree patch"
        );
        let page_patch = &mutation["focusedPagePatch"];
        assert!(
            page_patch.is_object(),
            "{label} {operation}: focused page repaint patch missing: {mutation}"
        );
        assert_eq!(
            page_patch["pageIndex"], before_rect["pageIndex"],
            "{label} {operation}: focused page repaint page"
        );
        for key in ["x", "y", "width", "height"] {
            let value = page_patch[key]
                .as_f64()
                .unwrap_or_else(|| panic!("{label} {operation}: page patch {key}"));
            assert!(
                value.is_finite(),
                "{label} {operation}: non-finite page patch {key}={value}"
            );
            if key == "width" || key == "height" {
                assert!(
                    value > 0.0,
                    "{label} {operation}: non-positive page patch {key}={value}"
                );
            }
        }
        let geometry = &mutation["focusedCursorGeometry"];
        assert!(
            geometry.is_object(),
            "{label} {operation}: focused geometry missing: {mutation}"
        );
        assert_eq!(
            geometry["sourceCharOffset"].as_u64(),
            Some(expected_source),
            "{label} {operation}: source offset"
        );
        assert_eq!(
            geometry["targetCharOffset"].as_u64(),
            Some(expected_target),
            "{label} {operation}: target offset"
        );
        assert!(
            geometry["revision"].as_u64().unwrap_or(0)
                > geometry["baseRevision"].as_u64().unwrap_or(u64::MAX),
            "{label} {operation}: revision chain"
        );

        for key in ["pageIndex", "y", "height", "cellOverflowed"] {
            assert_eq!(
                before_rect.get(key),
                after_rect.get(key),
                "{label} {operation}: stable cursor field {key}"
            );
        }
        assert_eq!(
            before_rect.get("cellBounds"),
            after_rect.get("cellBounds"),
            "{label} {operation}: stable cell bounds"
        );
        let predicted_x = rect_number(before_rect, "x") + rect_number(geometry, "deltaX");
        let exact_x = rect_number(after_rect, "x");
        assert!(
            // 공개 rect는 0.1px로 직렬화되므로 직전 rounded 원점의 최대 반올림 오차를 허용한다.
            (predicted_x - exact_x).abs() <= 0.051,
            "{label} {operation}: predicted x={predicted_x}, exact x={exact_x}, geometry={geometry}"
        );
    }

    for (label, relative) in [
        ("hwp", "samples/issue1949_giant_cell_nested_tables_perf.hwp"),
        (
            "hwpx",
            "samples/issue1949_giant_cell_nested_tables_perf.hwpx",
        ),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let bytes = std::fs::read(path).expect("read #3137 fixture");
        let mut doc = HwpDocument::from_bytes(&bytes).expect("load #3137 fixture");
        // HWPX는 paragraph layout에는 full reflow를 쓰지만, same-line 결과라면
        // cached tree tail patch는 fresh build와 동치여야 한다.
        let supports_tail_cache_patch = true;

        let rect_130: Value = serde_json::from_str(
            &doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, 130)
                .expect("initial exact rect"),
        )
        .expect("initial rect json");
        doc.get_cursor_rect_by_path_with_hint(
            0,
            0,
            r#"[{"controlIndex":2,"cellIndex":2,"cellParaIndex":5}]"#,
            130,
            Some(0),
        )
        .expect("warm initial page tree cache");
        let insert_1: Value = serde_json::from_str(
            &doc.insert_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 130, "1")
                .expect("first stable insert"),
        )
        .expect("first insert json");
        let rect_131: Value = serde_json::from_str(
            &doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, 131)
                .expect("first exact rect"),
        )
        .expect("first exact rect json");
        assert_eq!(
            insert_1["cellFlowChanged"].as_bool(),
            Some(false),
            "{label} first insert flow"
        );
        // 원본 LineSeg가 첫 local reflow에서 합성 metrics로 정규화되는 문서는 첫 입력을
        // exact fallback한다. 그 exact rect가 다음 revision의 재사용 기준점이 된다.
        if insert_1["focusedCursorGeometry"].is_object() {
            assert_geometry(label, "insert-1", &rect_130, &insert_1, &rect_131, 130, 131);
        }
        doc.get_cursor_rect_by_path_with_hint(
            0,
            0,
            r#"[{"controlIndex":2,"cellIndex":2,"cellParaIndex":5}]"#,
            131,
            Some(0),
        )
        .expect("warm post-normalization page tree cache");

        let insert_2: Value = serde_json::from_str(
            &doc.insert_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 131, "a")
                .expect("second stable insert"),
        )
        .expect("second insert json");
        assert_eq!(
            insert_2["focusedPageTreePatched"].as_bool(),
            Some(supports_tail_cache_patch),
            "{label} insert-a same-line tail-cache policy"
        );
        if supports_tail_cache_patch {
            assert_cached_line_matches_fresh(
                &doc,
                label,
                "insert-a",
                focused_patch_page_index(&insert_2, label, "insert-a"),
            );
        } else {
            assert!(
                doc.core
                    .page_tree_cache
                    .borrow()
                    .iter()
                    .all(Option::is_none),
                "{label} insert-a full reflow must invalidate cached page trees"
            );
        }
        let rect_132: Value = serde_json::from_str(
            &doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, 132)
                .expect("second exact rect"),
        )
        .expect("second exact rect json");
        if supports_tail_cache_patch {
            assert_geometry(label, "insert-a", &rect_131, &insert_2, &rect_132, 131, 132);
        }

        let replace: Value = serde_json::from_str(
            &doc.replace_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 131, 1, "한")
                .expect("stable IME replace"),
        )
        .expect("replace json");
        if supports_tail_cache_patch {
            assert_cached_line_matches_fresh(
                &doc,
                label,
                "replace-ime",
                focused_patch_page_index(&replace, label, "replace-ime"),
            );
        } else {
            assert_eq!(replace["focusedPageTreePatched"].as_bool(), Some(false));
            assert!(
                doc.core
                    .page_tree_cache
                    .borrow()
                    .iter()
                    .all(Option::is_none),
                "{label} replace-ime full reflow must invalidate cached page trees"
            );
        }
        let rect_replaced: Value = serde_json::from_str(
            &doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, 132)
                .expect("replace exact rect"),
        )
        .expect("replace exact rect json");
        if supports_tail_cache_patch {
            assert_geometry(
                label,
                "replace-ime",
                &rect_132,
                &replace,
                &rect_replaced,
                132,
                132,
            );
        }

        let delete: Value = serde_json::from_str(
            &doc.delete_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 131, 1)
                .expect("stable backspace"),
        )
        .expect("delete json");
        let delete_patched = delete["focusedPageTreePatched"].as_bool();
        if delete_patched == Some(false) {
            assert!(
                doc.core
                    .page_tree_cache
                    .borrow()
                    .iter()
                    .all(Option::is_none),
                "{label} delete-backward fallback must invalidate cached page trees"
            );
        }
        let rect_deleted: Value = serde_json::from_str(
            &doc.get_cursor_rect_in_cell_native(0, 0, 2, 2, 5, 131)
                .expect("delete exact rect"),
        )
        .expect("delete exact rect json");
        match delete_patched {
            Some(true) => {
                assert_cached_line_matches_fresh(
                    &doc,
                    label,
                    "delete-backward",
                    focused_patch_page_index(&delete, label, "delete-backward"),
                );
                assert_geometry(
                    label,
                    "delete-backward",
                    &rect_replaced,
                    &delete,
                    &rect_deleted,
                    132,
                    131,
                );
            }
            Some(false) => {
                assert_eq!(
                    rect_deleted["cellBounds"], rect_replaced["cellBounds"],
                    "{label} delete-backward fallback must keep cell bounds"
                );
            }
            other => panic!("{label} delete-backward patch flag must be boolean: {other:?}"),
        }

        // 중간 오프셋 편집은 후속 TextRun char_start까지 바꾸므로 보수적으로 전체
        // 캐시 무효화한다. 같은 text를 되돌린 뒤 page tree를 다시 warm한다.
        let middle_insert: Value = serde_json::from_str(
            &doc.insert_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 125, "x")
                .expect("middle insert fallback"),
        )
        .expect("middle insert json");
        assert_eq!(
            middle_insert["focusedPageTreePatched"].as_bool(),
            Some(false),
            "{label}: middle edit must not patch the cached tail line"
        );
        assert!(
            middle_insert["focusedPagePatch"].is_null(),
            "{label}: middle edit must not expose a repaint patch"
        );
        assert!(
            doc.core
                .page_tree_cache
                .borrow()
                .iter()
                .all(Option::is_none),
            "{label}: middle edit must invalidate cached page trees"
        );
        let middle_delete: Value = serde_json::from_str(
            &doc.delete_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 125, 1)
                .expect("restore middle insert"),
        )
        .expect("middle delete json");
        assert_eq!(
            middle_delete["focusedPageTreePatched"].as_bool(),
            Some(false),
            "{label}: uncached restore must keep the full invalidation fallback"
        );
        doc.get_cursor_rect_by_path_with_hint(
            0,
            0,
            r#"[{"controlIndex":2,"cellIndex":2,"cellParaIndex":5}]"#,
            131,
            Some(0),
        )
        .expect("rewarm after middle-edit fallback");

        // 이 시나리오는 앞서 IME replace/backspace를 거치므로 #2214의 pristine
        // 56/61 입력 경계를 재사용하지 않는다. 실제 첫 flow boundary까지 각 stable
        // 입력은 cached patch, boundary 입력은 full invalidation이어야 한다.
        let mut saw_flow_boundary = false;
        for inserted in 0..512 {
            let mutation: Value = serde_json::from_str(
                &doc.insert_text_in_cell_native_deferred_pagination(
                    0,
                    0,
                    2,
                    2,
                    5,
                    131 + inserted,
                    "1",
                )
                .expect("tail insert through flow boundary"),
            )
            .expect("tail boundary json");
            let boundary = mutation["cellFlowChanged"].as_bool().unwrap_or_else(|| {
                panic!(
                    "{label}: tail input {} must report a boolean cell-flow signal: {mutation}",
                    inserted + 2
                )
            });
            assert_eq!(
                mutation["focusedPageTreePatched"].as_bool(),
                Some(supports_tail_cache_patch && !boundary),
                "{label}: tail input {} patch signal",
                inserted + 2
            );
            assert_eq!(
                mutation["focusedPagePatch"].is_object(),
                supports_tail_cache_patch && !boundary,
                "{label}: tail input {} repaint patch signal",
                inserted + 2
            );
            if boundary {
                saw_flow_boundary = true;
                break;
            }
        }
        assert!(
            saw_flow_boundary,
            "{label}: IME-normalized tail input must eventually cross a line-flow boundary"
        );
        assert!(
            doc.core
                .page_tree_cache
                .borrow()
                .iter()
                .all(Option::is_none),
            "{label}: flow boundary must invalidate the focused page tree"
        );
    }
}



/// #2424 Stage D: 공개 pagination을 유지한 채 한 호출당 한 fragment만 전진하고,
/// 마지막 step에서만 full-pagination oracle과 같은 cut chain을 원자적으로 commit한다.
#[test]
fn issue2424_resumable_pagination_commits_only_after_final_fragment() {
    for (label, relative) in [
        ("hwp", "samples/issue1949_giant_cell_nested_tables_perf.hwp"),
        (
            "hwpx",
            "samples/issue1949_giant_cell_nested_tables_perf.hwpx",
        ),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let bytes = std::fs::read(path).expect("read #2424 fixture");
        let mut doc = HwpDocument::from_bytes(&bytes).expect("load #2424 fixture");
        let boundary_inserts = issue2214_flow_boundary_insert_count(label);

        for inserted in 0..boundary_inserts {
            doc.insert_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 130 + inserted, "1")
                .expect("deferred sequential insert");
        }
        let transient_cuts = issue2214_target_cuts(&doc);
        issue2214_assert_cut_continuity(label, "resumable-transient", &transient_cuts);

        let begin: Value = serde_json::from_str(
            &doc.begin_deferred_pagination(1)
                .expect("begin resumable pagination"),
        )
        .expect("begin result json");
        assert_eq!(begin["status"], "pending", "{label}: begin status");
        assert_eq!(
            issue2214_target_cuts(&doc),
            transient_cuts,
            "{label}: begin must not publish shadow pages"
        );

        let mut step_calls = 0usize;
        let mut fragments_processed = 0usize;
        loop {
            let step: Value = serde_json::from_str(
                &doc.step_deferred_pagination(1)
                    .expect("step resumable pagination"),
            )
            .expect("step result json");
            step_calls += 1;
            fragments_processed +=
                step["fragmentsProcessed"].as_u64().expect("fragment count") as usize;
            match step["status"].as_str() {
                Some("pending") => assert_eq!(
                    issue2214_target_cuts(&doc),
                    transient_cuts,
                    "{label}: step {step_calls} published an incomplete shadow result"
                ),
                Some("complete") => break,
                other => panic!("{label}: unexpected step status {other:?}: {step}"),
            }
        }

        assert_eq!(step_calls, 115, "{label}: one macrotask per fragment");
        assert_eq!(
            fragments_processed, 115,
            "{label}: every target fragment processed exactly once"
        );
        let committed_cuts = issue2214_target_cuts(&doc);
        issue2214_assert_cut_continuity(label, "resumable-committed", &committed_cuts);
        assert_eq!(
            transient_cuts
                .iter()
                .zip(&committed_cuts)
                .filter(|(before, after)| before != after)
                .count(),
            113,
            "{label}: committed cut chain must match the full-pagination oracle"
        );
    }
}



/// #2424 리뷰 보정: line 5→4가 되는 삭제도 incomplete shadow cut을 게시하지 않고,
/// 마지막 step에서 full-pagination oracle과 같은 continuation chain으로 돌아가야 한다.
#[test]
fn issue2424_resumable_delete_commits_only_after_final_fragment() {
    for (label, relative) in [
        ("hwp", "samples/issue1949_giant_cell_nested_tables_perf.hwp"),
        (
            "hwpx",
            "samples/issue1949_giant_cell_nested_tables_perf.hwpx",
        ),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let bytes = std::fs::read(path).expect("read #2424 fixture");
        let mut doc = HwpDocument::from_bytes(&bytes).expect("load #2424 fixture");
        let boundary_inserts = issue2214_flow_boundary_insert_count(label);
        doc.insert_text_in_cell_native_deferred_pagination(
            0,
            0,
            2,
            2,
            5,
            130,
            &"1".repeat(boundary_inserts),
        )
        .expect("prepare fifth cell line");
        doc.flush_deferred_pagination()
            .expect("commit expanded pagination");
        let expanded_cuts = issue2214_target_cuts(&doc);
        let expanded_line_starts = doc
            .core
            .get_cell_paragraph_ref(0, 0, 2, 2, 5)
            .expect("expanded target paragraph")
            .line_segs
            .iter()
            .map(|seg| seg.text_start)
            .collect::<Vec<_>>();

        let delete_raw = doc
            .delete_text_in_cell_native_deferred_pagination(
                0,
                0,
                2,
                2,
                5,
                129 + boundary_inserts,
                1,
            )
            .expect("deferred line-shrinking delete");
        let delete: Value = serde_json::from_str(&delete_raw).expect("delete result");
        let deleted_line_starts = doc
            .core
            .get_cell_paragraph_ref(0, 0, 2, 2, 5)
            .expect("deleted target paragraph")
            .line_segs
            .iter()
            .map(|seg| seg.text_start)
            .collect::<Vec<_>>();
        assert_eq!(
            delete["cellFlowChanged"], true,
            "{label}: delete must remove the fifth line; expanded={expanded_line_starts:?}, deleted={deleted_line_starts:?}"
        );
        let transient_cuts = issue2214_target_cuts(&doc);
        issue2214_assert_cut_continuity(label, "delete-transient", &transient_cuts);

        let begin: Value = serde_json::from_str(
            &doc.begin_deferred_pagination(1)
                .expect("begin delete pagination"),
        )
        .expect("begin delete json");
        assert_eq!(begin["status"], "pending", "{label}: delete begin");
        assert_eq!(
            issue2214_target_cuts(&doc),
            transient_cuts,
            "{label}: delete begin must not publish shadow pages"
        );

        let mut step_calls = 0usize;
        loop {
            let step: Value = serde_json::from_str(
                &doc.step_deferred_pagination(1)
                    .expect("step delete pagination"),
            )
            .expect("step delete json");
            step_calls += 1;
            match step["status"].as_str() {
                Some("pending") => assert_eq!(
                    issue2214_target_cuts(&doc),
                    transient_cuts,
                    "{label}: delete step {step_calls} published incomplete cuts"
                ),
                Some("complete") => break,
                other => panic!("{label}: unexpected delete step {other:?}: {step}"),
            }
        }
        assert_eq!(step_calls, 115, "{label}: delete fragment steps");
        let committed_cuts = issue2214_target_cuts(&doc);
        issue2214_assert_cut_continuity(label, "delete-committed", &committed_cuts);

        let mut oracle = HwpDocument::from_bytes(&bytes).expect("load delete oracle");
        oracle
            .insert_text_in_cell_native(0, 0, 2, 2, 5, 130, &"1".repeat(boundary_inserts - 1))
            .expect("full-pagination delete oracle state");
        assert_eq!(
            committed_cuts,
            issue2214_target_cuts(&oracle),
            "{label}: resumable delete must match full pagination"
        );
        assert_ne!(
            committed_cuts, expanded_cuts,
            "{label}: deleting the boundary character must change downstream cuts"
        );
    }
}



#[test]
fn issue2424_new_edit_stales_old_job_and_sync_flush_restarts_latest_revision() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("samples/issue1949_giant_cell_nested_tables_perf.hwp");
    let bytes = std::fs::read(path).expect("read #2424 fixture");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("load #2424 fixture");
    let boundary_inserts = issue2214_flow_boundary_insert_count("hwp");
    for inserted in 0..boundary_inserts {
        doc.insert_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 130 + inserted, "1")
            .expect("deferred sequential insert");
    }

    let begin: Value = serde_json::from_str(
        &doc.begin_deferred_pagination(1)
            .expect("begin first revision"),
    )
    .expect("begin json");
    assert_eq!(begin["status"], "pending");
    let first_revision = begin["revision"].as_u64().expect("first revision");
    let first_step: Value = serde_json::from_str(
        &doc.step_deferred_pagination(1)
            .expect("step first revision"),
    )
    .expect("step json");
    assert_eq!(first_step["status"], "pending");

    doc.insert_text_in_cell_native_deferred_pagination(0, 0, 2, 2, 5, 130 + boundary_inserts, "1")
        .expect("new edit supersedes first revision");
    let stale: Value = serde_json::from_str(
        &doc.step_deferred_pagination(1)
            .expect("reject stale first revision"),
    )
    .expect("stale json");
    assert_eq!(stale["status"], "stale");
    assert_eq!(stale["revision"].as_u64(), Some(first_revision));

    let replacement: Value = serde_json::from_str(
        &doc.begin_deferred_pagination(1)
            .expect("begin replacement revision"),
    )
    .expect("replacement json");
    assert_eq!(replacement["status"], "pending");
    assert!(
        replacement["revision"]
            .as_u64()
            .expect("replacement revision")
            > first_revision,
        "latest edit must own a newer job revision"
    );
    assert!(doc.cancel_deferred_pagination());
    assert!(!doc.cancel_deferred_pagination());

    let flushed: Value = serde_json::from_str(
        &doc.flush_deferred_pagination()
            .expect("sync barrier restarts latest revision"),
    )
    .expect("flush json");
    assert_eq!(flushed["status"], "complete");
    assert_eq!(flushed["pageCount"], 115);
    issue2214_assert_cut_continuity("hwp", "replacement-flushed", &issue2214_target_cuts(&doc));
}



/// [#4167] units 지문의 실문서 계약: 텍스트 문단 제자리 타이핑은 지문 불변(캐시
/// 보존 → find_pages µs 급), 공백 스페이서 문단에 글자 삽입은 클래스 전이로 지문
/// 변경(정당 evict). cp6 은 공백 5자 스페이서, cp29 는 다줄 텍스트 문단이다.
#[test]
fn issue4167_units_fingerprint_doc_contract() {
    use crate::renderer::layout::LayoutEngine;
    let bytes =
        std::fs::read("samples/issue1949_giant_cell_nested_tables_perf.hwp").expect("샘플 읽기");
    let mut doc = HwpDocument::from_bytes(&bytes).expect("파싱");
    let _ = doc.page_count();
    let para = |doc: &HwpDocument, cp: usize| -> Paragraph {
        match &doc.document.sections[0].paragraphs[0].controls[2] {
            Control::Table(t) => t.cells[2].paragraphs[cp].clone(),
            _ => panic!("표가 아님"),
        }
    };

    // 텍스트 문단: 삽입 전후 지문 불변 (원본 tag 잔여 비트는 마스킹됨)
    let text_before = para(&doc, 29);
    doc.insert_text_in_cell_deferred_pagination(0, 0, 2, 2, 29, 5, "X")
        .expect("insert");
    let text_after = para(&doc, 29);
    assert_eq!(
        LayoutEngine::cell_paragraph_units_fingerprint(&text_before),
        LayoutEngine::cell_paragraph_units_fingerprint(&text_after),
        "텍스트 문단 제자리 삽입은 units 지문 불변이어야 한다 (#4167 스킵 전제)"
    );

    // 스페이서 문단: 삽입이 trim-empty 클래스를 바꿔 지문 변경
    let spacer_before = para(&doc, 6);
    assert!(
        spacer_before.text.trim().is_empty(),
        "cp6 은 공백 스페이서 전제"
    );
    doc.insert_text_in_cell_deferred_pagination(0, 0, 2, 2, 6, 5, "X")
        .expect("insert");
    let spacer_after = para(&doc, 6);
    assert_ne!(
        LayoutEngine::cell_paragraph_units_fingerprint(&spacer_before),
        LayoutEngine::cell_paragraph_units_fingerprint(&spacer_after),
        "스페이서 → 텍스트 전이는 지문이 변해 evict 되어야 한다"
    );
}



/// [#4576] 핫패치 무효화 계약 — 원본 IR 이 그대로여도 파생본을 못 믿게 된 상황을
/// 표현할 수 있어야 한다.
///
/// 렌더러 코드가 교체되면 `pagination`·`composed`·측정 캐시에 남은 값은 패치 이전
/// 코드의 산출물이다. 원본 IR 은 한 글자도 안 바뀌었으므로 `dirty_sections` 도
/// `section_revisions` 도 오르지 않고, 그래서 억지로 `paginate()` 를 불러도 깨끗한
/// 구역을 건너뛴다 — 새 코드가 낸 숫자를 옛 코드가 잡은 페이지 박스에 그리는 혼합물이
/// 남는다.
///
/// 이 테스트는 "패치 이전 코드가 만든 파생본"을 원본을 건드리지 않고 흉내 낸다:
/// 메모된 조합·페이지네이션·측정 결과만 지금 코드가 같은 원본에서 절대 만들지 않을
/// 값으로 바꾼다. 그 상태에서 `paginate()` 는 아무것도 되돌리지 못해야 하고(게이트가
/// 살아 있다는 확인), `rebuild_derived_state()` 는 셋 다 원본에서 다시 만들어야 한다.
#[test]
fn issue_4576_rebuild_derived_state_recomputes_composition_and_pagination() {
    let mut doc = HwpDocument::create_empty();
    // 여러 쪽·여러 줄이 나오도록 본문을 채운다 (한 문단이 여러 페이지에 걸친다).
    let body = "가나다라마바사아자차카타파하".repeat(400);
    doc.insert_text_native(0, 0, 0, &body).expect("본문 삽입");

    let baseline_pages = doc.page_count();
    assert!(
        baseline_pages >= 2,
        "여러 쪽 문서를 전제로 한다 (실측 {baseline_pages}쪽)"
    );
    let baseline_tree = doc
        .build_page_tree(0)
        .expect("기준 페이지 트리")
        .root
        .to_json();
    let baseline_height = doc.measured_sections[0].fallback_paragraphs[0].total_height;
    let baseline_lines = doc.composed[0][0].lines.len();

    // --- 패치 이전 코드의 파생본을 흉내 낸다 (원본 IR 은 손대지 않는다) ---
    doc.composed[0][0].lines[0].runs[0].text = "옛 조합 규칙".to_string();
    doc.pagination[0].pages.pop().expect("마지막 쪽 제거");
    doc.measured_sections[0].fallback_paragraphs[0].total_height = 1.0;

    doc.invalidate_page_tree_cache();
    assert_ne!(
        doc.build_page_tree(0)
            .expect("오염된 페이지 트리")
            .root
            .to_json(),
        baseline_tree,
        "조합 오염이 페이지 트리에 실제로 보여야 테스트가 의미를 갖는다"
    );
    assert_eq!(
        doc.page_count(),
        baseline_pages - 1,
        "페이지네이션 오염이 쪽 수에 보여야 한다"
    );

    // 문서가 안 바뀌었으므로 강제 재조판은 아무것도 되돌리지 못한다 (#4576 의 전제).
    doc.paginate();
    assert_eq!(
        doc.page_count(),
        baseline_pages - 1,
        "깨끗한 구역을 건너뛰므로 paginate() 만으로는 페이지네이션이 복구되지 않는다"
    );
    assert_eq!(
        doc.composed[0][0].lines[0].runs[0].text, "옛 조합 규칙",
        "paginate() 는 조합을 다시 만들지 않는다"
    );
    assert_eq!(
        doc.measured_sections[0].fallback_paragraphs[0].total_height, 1.0,
        "paginate() 는 깨끗한 구역의 측정 캐시를 다시 만들지 않는다"
    );

    // --- 무효화 계약: 원본에서 파생 상태를 전부 다시 만든다 ---
    doc.core.rebuild_derived_state();

    assert_eq!(
        doc.page_count(),
        baseline_pages,
        "페이지네이션이 원본에서 다시 만들어져야 한다"
    );
    assert_ne!(
        doc.composed[0][0].lines[0].runs[0].text, "옛 조합 규칙",
        "조합이 원본에서 다시 만들어져야 한다"
    );
    assert_eq!(
        doc.composed[0][0].lines.len(),
        baseline_lines,
        "조합 결과의 줄 수가 원본 기준으로 돌아와야 한다"
    );
    assert_eq!(
        doc.measured_sections[0].fallback_paragraphs[0].total_height, baseline_height,
        "측정 캐시가 원본에서 다시 만들어져야 한다"
    );
    assert_eq!(
        doc.build_page_tree(0)
            .expect("복구 후 페이지 트리")
            .root
            .to_json(),
        baseline_tree,
        "페이지 트리가 기준선과 같아야 한다 (옛 코드 잔재 없음)"
    );
}
