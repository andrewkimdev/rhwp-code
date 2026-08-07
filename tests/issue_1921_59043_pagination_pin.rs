//! Issue #1921 — 59043 규제영향분석서 페이지네이션 드리프트 핀.
//!
//! `samples/issue1921/59043_regulatory_analysis.hwp` — 부동(자리차지) 표·rowspan
//! 블록이 밀집한 규제영향분석서. PR #2092(RowBreak 블록컷 sliver 흡수)로
//! 48쪽 → 42쪽 (수정 전 pi=160 3×3 rowspan 블록에서 컷 진동 `+46,+1` 교대).
//!
//! 권위 정답지는 한글 2022 편집기 37쪽
//! (`pdf/issue1921/59043_regulatory_analysis-2022.pdf`, 편집기 PageCount=37 정합).
//! 잔여 +5는 2단 배치 밀도(부동 표 흐름 패킹) 축으로 #1921 후속 과제 — 본 테스트는
//! 현재 도달값 39를 핀해 개선(37 방향)과 회귀(40+)를 모두 표면화한다.

use std::fs;
use std::path::Path;

use rhwp::renderer::render_tree::{RenderNode, RenderNodeType};
use rhwp::wasm_api::HwpDocument;

const SAMPLE: &str = "samples/issue1921/59043_regulatory_analysis.hwp";
const PAGE_8: u32 = 7;
const PAGE_11: u32 = 10;

fn page_count_of(rel: &str) -> u32 {
    let repo_root = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(repo_root).join(rel);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let doc = HwpDocument::from_bytes(&bytes).unwrap_or_else(|e| panic!("parse {}: {:?}", rel, e));
    doc.page_count()
}

fn load_document() -> HwpDocument {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    HwpDocument::from_bytes(&bytes).unwrap_or_else(|e| panic!("parse {SAMPLE}: {e}"))
}

fn find_table<'a>(
    node: &'a RenderNode,
    para_index: usize,
    control_index: usize,
) -> Option<&'a RenderNode> {
    if let RenderNodeType::Table(table) = &node.node_type {
        if table.para_index == Some(para_index) && table.control_index == Some(control_index) {
            return Some(node);
        }
    }
    node.children
        .iter()
        .find_map(|child| find_table(child, para_index, control_index))
}

fn find_cell(node: &RenderNode, row: u16, col: u16) -> Option<&RenderNode> {
    node.children.iter().find(|child| {
        matches!(
            child.node_type,
            RenderNodeType::TableCell(ref cell) if cell.row == row && cell.col == col
        )
    })
}

fn collect_images<'a>(node: &'a RenderNode, images: &mut Vec<&'a RenderNode>) {
    if matches!(node.node_type, RenderNodeType::Image(_)) {
        images.push(node);
    }
    for child in &node.children {
        collect_images(child, images);
    }
}

#[test]
fn regulatory_59043_page_count_pin() {
    let pages = page_count_of("samples/issue1921/59043_regulatory_analysis.hwp");
    assert_eq!(
        pages, 39,
        "issue1921 59043 현 핀 39쪽 (한글 2022 정답지 37쪽, 잔여 +2=배치 밀도·페이지 소유 fidelity 축). \
         실측 {}p — 40p+면 과분할 회귀, 38p 이하면 한글 PDF 직접 대조 후 핀과 \
         잔여 정합성 기록을 갱신할 것.",
        pages
    );
}

/// 한컴 2022 PDF p8의 6행 왼쪽 셀에는 두 사진이 모두 들어간다. 특히 Square-wrap
/// 세로 사진은 cell bottom에서 다시 배치되면 다음 본문을 침범한다. 페이지 수만으로는
/// 이 소실/겹침을 감지하지 못하므로, 이미지의 물리 cell containment를 별도 고정한다.
#[test]
fn regulatory_59043_page8_square_picture_stays_in_its_table_cell() {
    let doc = load_document();
    let tree = doc
        .build_page_render_tree(PAGE_8)
        .unwrap_or_else(|e| panic!("render {SAMPLE} page 8: {e}"));
    let table = find_table(&tree.root, 73, 0).expect("p8 사진 표(pi=73, ci=0)");
    let cell = find_cell(table, 6, 0).expect("p8 사진 표의 6행 왼쪽 셀");
    let cell_bottom = cell.bbox.y + cell.bbox.height;
    let mut images = Vec::new();
    collect_images(cell, &mut images);
    assert_eq!(
        images.len(),
        2,
        "p8 6행 왼쪽 셀의 사진 두 개가 모두 렌더되어야 함"
    );
    for image in images {
        let bottom = image.bbox.y + image.bbox.height;
        assert!(
            bottom <= cell_bottom + 0.75,
            "p8 6행 왼쪽 셀의 사진이 cell 밖으로 돌출: image bottom={bottom:.1}, \
             cell bottom={cell_bottom:.1}"
        );
    }
}

/// 한컴 2022 PDF p11의 pi=98 row 2에는 Square 사진 두 개가 해당 cell 안에 배치된다.
/// 저장 vpos가 있는 RowBreak cell에서 그림 높이를 다시 일반 flow로 더하면 다음 fragment가
/// 소유해야 할 그림이 앞 page의 cell 밖으로 올라간다. 페이지 수 핀만으로는 이 소유권
/// 역전을 감지하지 못하므로, p11의 물리 cell containment를 고정한다.
#[test]
fn regulatory_59043_page11_square_pictures_stay_in_row2_fragment() {
    let doc = load_document();
    let tree = doc
        .build_page_render_tree(PAGE_11)
        .unwrap_or_else(|e| panic!("render {SAMPLE} page 11: {e}"));
    let table = find_table(&tree.root, 98, 0).expect("p11 SNS 사례 표(pi=98, ci=0)");
    let cell = find_cell(table, 2, 0).expect("p11 SNS 사례 표의 2행");
    let cell_top = cell.bbox.y;
    let cell_bottom = cell.bbox.y + cell.bbox.height;
    let mut images = Vec::new();
    collect_images(cell, &mut images);
    assert_eq!(
        images.len(),
        2,
        "p11 row 2의 시작 Square 사진 두 개가 같은 fragment에서 렌더되어야 함"
    );
    for image in images {
        let bottom = image.bbox.y + image.bbox.height;
        assert!(
            image.bbox.y >= cell_top - 0.75 && bottom <= cell_bottom + 0.75,
            "p11 row 2 Square 사진의 owner가 cell fragment와 다름: image={:?}, cell=({cell_top:.1}..{cell_bottom:.1})",
            image.bbox
        );
    }
}
