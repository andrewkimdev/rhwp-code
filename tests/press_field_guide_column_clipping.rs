//! 좁은 표 열의 빈 누름틀(CLICK_HERE) 안내문이 셀 경계를 넘어 잘리던 회귀 가드.
//!
//! 재현 문서: `samples/hwpx/appccr1dlm.hwpx` (공공 정부 서식) 의
//! `#REPEAT-BODY:품목` 표. 이 표의 수량/단가/총액 열은 오른쪽 정렬, 국내제작여부
//! 열은 가운데 정렬이며 각각 빈 누름틀 필드의 안내문("수량"/"단가"/"총액"/
//! "국내제작여부")을 담는다.
//!
//! 근본 원인: 빈 필드 안내문은 `comp_line.runs` 밖의 별도 마커 노드로 그려지는데,
//! 줄의 자연 폭 계산(`total_text_width`)은 `comp_line.runs`만 보므로 안내문 전용
//! 줄은 폭이 0으로 계산됐다. 그 결과 정렬 오프셋이 어긋나 RIGHT 정렬 열은 안내문이
//! 셀 오른쪽 끝에서 시작해 잘리고, CENTER 정렬 열은 폭의 절반만큼 밀려 잘렸다.
#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::path::Path;

use rhwp::renderer::render_tree::{PageRenderTree, RenderNode, RenderNodeType, TextRunNode};
use rhwp::wasm_api::HwpDocument;

/// `guide_display_style` 이 안내문에 덮어쓰는 색 — 정적 라벨(검정)과 구분하는
/// 유일한 표지다. `src/renderer/layout/paragraph_layout.rs` 의 값과 동기화 유지.
const GUIDE_COLOR: u32 = 0x0000FF;

fn load_tree() -> PageRenderTree {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/hwpx/appccr1dlm.hwpx");
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse sample");
    doc.build_page_render_tree(0).expect("render page 1")
}

/// 안내문 TextRun 과 그 안내문을 담는 가장 가까운 TableCell 조상을 함께 찾는다.
/// 정적 라벨(예: `#REPEAT-HEADER:품목`의 "수량")은 `GUIDE_COLOR`가 아니므로
/// 걸러진다.
fn find_guide_run_and_cell<'a>(
    node: &'a RenderNode,
    current_cell: Option<&'a RenderNode>,
    text: &str,
) -> Option<(&'a RenderNode, &'a RenderNode)> {
    let cell_here = if matches!(node.node_type, RenderNodeType::TableCell(_)) {
        Some(node)
    } else {
        current_cell
    };
    if let RenderNodeType::TextRun(TextRunNode {
        text: run_text,
        style,
        ..
    }) = &node.node_type
    {
        if run_text == text && style.color == GUIDE_COLOR {
            if let Some(cell) = cell_here {
                return Some((node, cell));
            }
        }
    }
    for child in &node.children {
        if let Some(found) = find_guide_run_and_cell(child, cell_here, text) {
            return Some(found);
        }
    }
    None
}

/// 안내문이 자신을 담은 셀의 가로 범위 안에 들어와야 한다 — 잘림 재발 가드.
/// 셀 좌우 padding 만큼 여유(2px)를 둔다.
fn assert_guide_fits_in_cell(field_name: &str, tree: &PageRenderTree) {
    let (guide_run, cell) =
        find_guide_run_and_cell(&tree.root, None, field_name).unwrap_or_else(|| {
            panic!("빈 누름틀 안내문 \"{field_name}\" 을(를) 표 안에서 찾지 못함")
        });

    let guide_left = guide_run.bbox.x;
    let guide_right = guide_run.bbox.x + guide_run.bbox.width;
    let cell_left = cell.bbox.x;
    let cell_right = cell.bbox.x + cell.bbox.width;
    const SLACK: f64 = 2.0;

    assert!(
        guide_left >= cell_left - SLACK && guide_right <= cell_right + SLACK,
        "\"{field_name}\" 안내문이 셀 경계를 벗어남: guide=[{guide_left:.2}, {guide_right:.2}], \
         cell=[{cell_left:.2}, {cell_right:.2}]"
    );
}

#[test]
fn right_aligned_guides_stay_inside_cell() {
    let tree = load_tree();
    for field in ["수량", "단가", "총액"] {
        assert_guide_fits_in_cell(field, &tree);
    }
}

#[test]
fn center_aligned_wide_guide_stays_inside_cell() {
    let tree = load_tree();
    assert_guide_fits_in_cell("국내제작여부", &tree);
}
