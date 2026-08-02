//! Issue #3738 Stage 9: 작은 RowBreak 표의 cell-footnote 전체 선예약이
//! 첫 fragment를 통째로 다음 쪽으로 미는 회귀를 실제 HWP로 고정한다.
//!
//! 한컴오피스 2020 기준 PDF p66에는 표 23의 0–4행(Organ Donation까지)과
//! 각주 76·77이 있고, p67은 Stephanie 행부터 이어진다. 표 전체 각주를
//! 첫 행 전부터 예약하면 p66 표가 전부 이월되어 이후 문단까지 한 쪽씩 밀린다.

use std::fs;
use std::path::Path;

use rhwp::renderer::render_tree::{RenderNode, RenderNodeType};
use rhwp::wasm_api::HwpDocument;

const SAMPLE: &str =
    "samples/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwp";
const PAGE_66: u32 = 65;
const PAGE_67: u32 = 66;
const PAGE_30: u32 = 29;
const PAGE_31: u32 = 30;
const PAGE_32: u32 = 31;
const PAGE_68: u32 = 67;
const PAGE_69: u32 = 68;
const PAGE_58: u32 = 57;
const PAGE_59: u32 = 58;
const PAGE_76: u32 = 75;
const PAGE_77: u32 = 76;
const PAGE_78: u32 = 77;
const PAGE_79: u32 = 78;

fn page_text(doc: &HwpDocument, page: u32) -> String {
    doc.extract_page_text_native(page)
        .unwrap_or_else(|e| panic!("extract physical page {}: {e}", page + 1))
}

fn subtree_bottom(node: &RenderNode) -> f64 {
    node.children
        .iter()
        .fold(node.bbox.y + node.bbox.height, |bottom, child| {
            bottom.max(subtree_bottom(child))
        })
}

fn footnote_and_footer(
    node: &RenderNode,
    footnote_bottom: &mut Option<f64>,
    footer_top: &mut Option<f64>,
) {
    match node.node_type {
        RenderNodeType::FootnoteArea => *footnote_bottom = Some(subtree_bottom(node)),
        RenderNodeType::Footer => *footer_top = Some(node.bbox.y),
        _ => {}
    }
    for child in &node.children {
        footnote_and_footer(child, footnote_bottom, footer_top);
    }
}

fn paragraph_bottom(node: &RenderNode, para_index: usize, bottom: &mut Option<f64>) {
    if let RenderNodeType::TextLine(line) = &node.node_type {
        if line.para_index == Some(para_index) {
            let candidate = node.bbox.y + node.bbox.height;
            *bottom = Some(bottom.map_or(candidate, |current| current.max(candidate)));
        }
    }
    for child in &node.children {
        paragraph_bottom(child, para_index, bottom);
    }
}

fn footnote_separator_top(node: &RenderNode, top: &mut Option<f64>) {
    if matches!(node.node_type, RenderNodeType::FootnoteArea) {
        for child in &node.children {
            if matches!(child.node_type, RenderNodeType::Line(_)) {
                *top = Some(child.bbox.y);
                return;
            }
        }
    }
    for child in &node.children {
        footnote_separator_top(child, top);
    }
}

#[test]
fn rowbreak_table_cell_footnotes_keep_the_pdf_fragment_boundary() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage9 HWP evidence fixture");

    assert!(
        doc.page_count() <= 224,
        "표 23의 전체 table-footnote 선예약이 되살아 HWP가 225쪽 이상으로 과페이지화됨: {}쪽",
        doc.page_count()
    );

    let p66 = page_text(&doc, PAGE_66);
    let p67 = page_text(&doc, PAGE_67);
    assert!(
        p66.contains("National Organ Transplant Act") && p66.contains("Organ Donation"),
        "p66에 기준 PDF의 표 23 0–4행이 함께 남아야 함: {p66}"
    );
    assert!(
        !p66.contains("Stephanie Tubbs Jones"),
        "p66은 기준 PDF처럼 Stephanie 행 이전에서 끝나야 함: {p66}"
    );
    assert!(
        p67.contains("Stephanie Tubbs Jones") && p67.contains("OPTN policy 14"),
        "p67은 기준 PDF처럼 표 23의 남은 5–6행에서 재개해야 함: {p67}"
    );

    let tree = doc
        .build_page_render_tree(PAGE_67)
        .unwrap_or_else(|e| panic!("render physical page 67: {e}"));
    let mut footnote_bottom = None;
    let mut footer_top = None;
    footnote_and_footer(&tree.root, &mut footnote_bottom, &mut footer_top);
    let footnote_bottom = footnote_bottom.expect("p67 footnote area");
    let footer_top = footer_top.expect("p67 footer");
    assert!(
        footnote_bottom <= footer_top + 1.0,
        "p67 각주 실제 하단({footnote_bottom:.1}px)이 footer 시작({footer_top:.1}px)을 넘어선다"
    );
}

#[test]
fn native_hwp5_footnote_reset_moves_only_the_overlapping_tail_to_the_next_page() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage12 HWP evidence fixture");

    let p30 = page_text(&doc, PAGE_30);
    let p31 = page_text(&doc, PAGE_31);
    let p32 = page_text(&doc, PAGE_32);
    assert!(
        p30.contains("10년 후 71.7%")
            && p30.contains("Dattani, Nikesh")
            && !p30.contains("문제가 나타남"),
        "p30은 각주 29와 그 위의 세 줄에서 끝나야 함: {p30}"
    );
    assert!(
        p31.contains("문제가 나타남")
            && p31.contains("5. 독일")
            && !p31.contains("Dattani, Nikesh"),
        "p31은 각주 29 없이 p30의 두 줄 tail 뒤에 독일 절로 이어져야 함: {p31}"
    );
    assert!(
        p32.contains("그림 35"),
        "각주 29를 p30으로 소급한 뒤에도 그림 35는 다음 페이지에 보존돼야 함: {p32}"
    );
}

#[test]
fn picture_caption_rowbreak_uses_the_actual_footnote_boundary_before_deferring() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage13 HWP evidence fixture");

    let p68 = page_text(&doc, PAGE_68);
    let p69 = page_text(&doc, PAGE_69);
    assert!(
        p68.contains("그림 49. OPTN 생존 장기기증 원칙"),
        "p68에는 그림 49와 caption이 각주 위에 남아야 함: {p68}"
    );
    assert!(
        !p69.contains("그림 49. OPTN 생존 장기기증 원칙")
            && p69.contains("나. 생존 장기기증 승인 절차"),
        "p69는 그림 49 없이 다음 본문으로 시작해야 함: {p69}"
    );
}

#[test]
fn native_hwp5_reset_tail_uses_the_actual_existing_footnote_boundary() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage14 HWP evidence fixture");

    let p58 = page_text(&doc, PAGE_58);
    let p59 = page_text(&doc, PAGE_59);
    assert!(
        p58.contains("호주 정부의 국민 건강 및 의료 연구 협의회")
            && p58.contains("Medical Research Council")
            && !p58.contains("독립적이며 적절한 지식과 기술"),
        "p58은 각주 70 위에 stored reset 전 세 줄을 보유해야 함: {p58}"
    );
    assert!(
        p59.contains("독립적이며 적절한 지식과 기술")
            && !p59.contains("호주 정부의 국민 건강 및 의료 연구 협의회"),
        "p59는 reset 뒤의 본문부터 재개해야 함: {p59}"
    );
}

#[test]
fn native_hwp5_rowbreak_tail_keeps_figure_51_with_its_pdf_page() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage15 HWP evidence fixture");

    let p76 = page_text(&doc, PAGE_76);
    let p77 = page_text(&doc, PAGE_77);
    let p78 = page_text(&doc, PAGE_78);
    let p79 = page_text(&doc, PAGE_79);
    assert!(
        p76.contains("생존 신장 기증자가") && p76.contains("위한 대기자 목록에 올라가거나,"),
        "p76은 표 24 row 4 reset 앞의 세 줄을 보유해야 함: {p76}"
    );
    assert!(
        p77.contains("투석을 시작하게 된 경우")
            && !p77.contains("후 2년 내에 신장 이식을 받기")
            && p77.contains("그림 51.")
            && !p77.contains("3. EU"),
        "p77은 표 24 row 4 tail 뒤에 그림 51을 각주 위에 포함해야 함: {p77}"
    );
    assert!(
        p78.contains("3. EU") && !p78.contains("그림 51."),
        "그림 51 단독 page가 제거되면 p78은 다음 본문으로 재개해야 함: {p78}"
    );
    assert!(
        !p79.trim().is_empty(),
        "p79은 연쇄 이월 때문에 빈 표 전용 page가 되어서는 안 됨"
    );
}

#[test]
fn native_hwp5_two_line_footnote_continues_after_the_reset_page() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage16 HWP evidence fixture");

    let p31 = page_text(&doc, PAGE_31);
    let p32 = page_text(&doc, PAGE_32);
    assert!(
        p31.contains("Aktuelle Entwicklungen") && !p31.contains("incentives"),
        "p31은 각주 30의 첫 줄만 보유해야 함: {p31}"
    );
    assert!(
        p32.contains("incentives") && !p32.contains("Aktuelle Entwicklungen"),
        "p32는 각주 30의 연속 tail만 보유해야 함: {p32}"
    );

    let tree = doc
        .build_page_render_tree(PAGE_31)
        .unwrap_or_else(|e| panic!("render physical page 31: {e}"));
    let mut body_bottom = None;
    let mut separator_top = None;
    paragraph_bottom(&tree.root, 421, &mut body_bottom);
    footnote_separator_top(&tree.root, &mut separator_top);
    assert!(
        body_bottom.expect("p31 para 421") <= separator_top.expect("p31 footnote separator") + 0.5,
        "p31 본문과 각주 separator가 겹치면 안 됨: body_bottom={body_bottom:?}, separator={separator_top:?}"
    );
}
