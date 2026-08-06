//! Issue #2308 functional regression for the #2195 sparse width overlay.
//!
//! Page-count pins do not catch a width-scale consumer that drifts only the split
//! height of a nested 1×1 table. The two continuation fragments are pinned after
//! direct comparison with the HWP 2024/Hancom PDF fixture: the second fragment
//! begins at the page's content top, rather than retaining the pre-#3637 stale
//! cell-local offset.

use rhwp::document_core::DocumentCore;
use rhwp::renderer::render_tree::{RenderNode, RenderNodeType};
use std::fs;
use std::path::Path;

fn nested_one_by_one_tables(node: &RenderNode, table_depth: usize, out: &mut Vec<(f64, f64)>) {
    let next_depth = if let RenderNodeType::Table(table) = &node.node_type {
        if table_depth >= 1 && table.row_count == 1 && table.col_count == 1 {
            out.push((node.bbox.y, node.bbox.height));
        }
        table_depth + 1
    } else {
        table_depth
    };
    for child in &node.children {
        nested_one_by_one_tables(child, next_depth, out);
    }
}

fn find_table_with_owner_para(node: &RenderNode, para_index: usize) -> Option<&RenderNode> {
    if matches!(
        &node.node_type,
        RenderNodeType::Table(table) if table.para_index == Some(para_index)
    ) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_table_with_owner_para(child, para_index))
}

fn find_nested_single_cell_table(node: &RenderNode) -> Option<&RenderNode> {
    if matches!(
        &node.node_type,
        RenderNodeType::Table(table) if table.row_count == 1 && table.col_count == 1
    ) {
        return Some(node);
    }
    node.children.iter().find_map(find_nested_single_cell_table)
}

fn collect_visible_text_line_rights(node: &RenderNode, rights: &mut Vec<f64>) {
    if !node.visible {
        return;
    }
    if matches!(&node.node_type, RenderNodeType::TextLine(_)) {
        rights.push(node.bbox.x + node.bbox.width);
    }
    for child in &node.children {
        collect_visible_text_line_rights(child, rights);
    }
}

#[test]
fn issue_2308_sparse_width_overlay_keeps_nested_fragment_geometry() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/76076_regulatory_analysis.hwp");
    let bytes = fs::read(path).expect("read #2195 authority fixture");
    let core = DocumentCore::from_bytes(&bytes).expect("parse #2195 authority fixture");

    let expected = [(32, 351.1, 636.8), (33, 77.1, 406.1)];
    for (page, expected_y, expected_height) in expected {
        let tree = core
            .build_page_render_tree(page)
            .unwrap_or_else(|error| panic!("render page {}: {error}", page + 1));
        let mut fragments = Vec::new();
        nested_one_by_one_tables(&tree.root, 0, &mut fragments);
        assert!(
            fragments.iter().any(|(y, height)| {
                (y - expected_y).abs() <= 0.2 && (height - expected_height).abs() <= 0.2
            }),
            "page {} nested fragment must preserve Hancom-aligned geometry \
             y={expected_y:.1} h={expected_height:.1}; got {fragments:?}",
            page + 1
        );
    }
}

/// HWP 2024 PDF p34의 1×1 중첩 표는 `inMargin=(0,0,141,141)`이더라도
/// 저장된 셀 좌우 여백(510HU)을 유지한다. 이 예외를 놓치면 문단의 paint
/// viewport가 우측 테두리까지 확장되어 한컴 출력과 달리 글자가 선을 침범한다.
#[test]
fn issue_2308_nested_non_tac_table_keeps_saved_horizontal_cell_margin() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/76076_regulatory_analysis.hwp");
    let bytes = fs::read(path).expect("read #2195 authority fixture");
    let core = DocumentCore::from_bytes(&bytes).expect("parse #2195 authority fixture");
    let tree = core.build_page_render_tree(33).expect("render HWP PDF p34");

    let outer = find_table_with_owner_para(&tree.root, 325)
        .expect("p34 outer activity-cost table (pi=325)");
    let nested =
        find_nested_single_cell_table(outer).expect("p34 nested single-cell rationale table");
    let mut rights = Vec::new();
    collect_visible_text_line_rights(nested, &mut rights);
    let rightmost = rights.into_iter().fold(f64::NEG_INFINITY, f64::max);
    let border_right = nested.bbox.x + nested.bbox.width;
    assert!(
        rightmost <= border_right - 6.0,
        "p34 nested-table text paint reaches the right border: text_right={rightmost:.1}, \
         border_right={border_right:.1}; HWP PDF retains the saved 510HU cell margin"
    );
}
