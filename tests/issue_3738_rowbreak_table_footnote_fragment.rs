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
const PAGE_80: u32 = 79;
const PAGE_37: u32 = 36;
const PAGE_25: u32 = 24;
const PAGE_154: u32 = 153;
const PAGE_155: u32 = 154;
const PAGE_157: u32 = 156;
const PAGE_158: u32 = 157;

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

fn table_bottom(node: &RenderNode, para_index: usize, bottom: &mut Option<f64>) {
    if let RenderNodeType::Table(table) = &node.node_type {
        if table.para_index == Some(para_index) {
            let candidate = node.bbox.y + node.bbox.height;
            *bottom = Some(bottom.map_or(candidate, |current| current.max(candidate)));
        }
    }
    for child in &node.children {
        table_bottom(child, para_index, bottom);
    }
}

fn images_for_control(
    node: &RenderNode,
    para_index: usize,
    control_index: usize,
    positions: &mut Vec<(f64, f64)>,
) {
    if let RenderNodeType::Image(image) = &node.node_type {
        if image.para_index == Some(para_index) && image.control_index == Some(control_index) {
            positions.push((node.bbox.x, node.bbox.y));
        }
    }
    for child in &node.children {
        images_for_control(child, para_index, control_index, positions);
    }
}

fn images_for_table(node: &RenderNode, para_index: usize, positions: &mut Vec<(f64, f64)>) {
    if let RenderNodeType::Table(table) = &node.node_type {
        if table.para_index == Some(para_index) {
            fn collect_images(node: &RenderNode, positions: &mut Vec<(f64, f64)>) {
                if matches!(node.node_type, RenderNodeType::Image(_)) {
                    positions.push((node.bbox.x, node.bbox.y));
                }
                for child in &node.children {
                    collect_images(child, positions);
                }
            }
            collect_images(node, positions);
            return;
        }
    }
    for child in &node.children {
        images_for_table(child, para_index, positions);
    }
}

fn footnote_text(node: &RenderNode, in_footnote: bool, text: &mut String) {
    let in_footnote = in_footnote || matches!(node.node_type, RenderNodeType::FootnoteArea);
    if in_footnote {
        if let RenderNodeType::TextRun(run) = &node.node_type {
            text.push_str(&run.text);
        }
    }
    for child in &node.children {
        footnote_text(child, in_footnote, text);
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
fn native_hwp5_repeated_empty_guide_lines_emit_tac_picture_once() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage18 HWP evidence fixture");
    let tree = doc
        .build_page_render_tree(PAGE_37)
        .expect("render physical page 37");

    // pi=463 control 1은 그림 37이다. text-start가 같은 빈 guide 줄이 둘이지만
    // 이 control은 하나뿐이므로 첫 줄에만 귀속되어야 한다.
    let mut positions = Vec::new();
    images_for_control(&tree.root, 463, 1, &mut positions);
    assert_eq!(
        positions.len(),
        1,
        "p37 그림 37은 한 번만 방출되어야 한다: {positions:?}"
    );
    let (x, y) = positions[0];
    assert!(
        x < 350.0 && y < 800.0,
        "그림 37은 PDF처럼 좌측의 두-그림 band에 있어야 하며 페이지 하단 fallback으로 새면 안 된다: x={x:.1}, y={y:.1}"
    );
}

#[test]
fn native_hwp5_same_page_stale_rowbreak_picture_keeps_figure_25_visible() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage19 HWP evidence fixture");
    let p25 = page_text(&doc, PAGE_25);
    assert!(
        p25.contains("그림 25.") && p25.contains("그림 26."),
        "p25에는 PDF처럼 그림 25와 그림 26의 caption이 함께 있어야 한다: {p25}"
    );

    let tree = doc
        .build_page_render_tree(PAGE_25)
        .expect("render physical page 25");
    let mut positions = Vec::new();
    // pi=357은 그림 25를 담은 빈 1×1 RowBreak 표다. stale -50000 HU를 그대로
    // 적용하면 Image가 p25 위쪽 밖(y<0)으로 나가 PDF에 있는 첫 그림이 사라진다.
    images_for_table(&tree.root, 357, &mut positions);
    assert_eq!(
        positions.len(),
        1,
        "p25 그림 25 표는 Image를 정확히 하나 방출해야 한다: {positions:?}"
    );
    let (x, y) = positions[0];
    assert!(
        x > 100.0 && y >= 240.0 && y < 360.0,
        "그림 25는 PDF처럼 p25 표 frame 내부에 있어야 한다: x={x:.1}, y={y:.1}"
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

#[test]
fn native_hwp5_large_rowbreak_table_keeps_its_first_fragment_before_cell_footnotes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage17 HWP evidence fixture");

    let p78 = page_text(&doc, PAGE_78);
    let p79 = page_text(&doc, PAGE_79);
    let p80 = page_text(&doc, PAGE_80);
    assert!(
        p78.contains("Convention") && p78.contains("Directive"),
        "p78은 표 25의 Convention·Directive first fragment를 보유해야 함: {p78}"
    );
    assert!(
        p79.contains("Recommendation") && p79.contains("CM/Res(2017)1"),
        "p79는 표 25의 Resolution/Recommendation continuation을 보유해야 함: {p79}"
    );
    assert!(
        p80.contains("유럽의회(European Parliament)") && !p80.contains("시행법은"),
        "p80은 표 25 continuation이 아니라 PDF처럼 본문으로 재개해야 함: {p80}"
    );

    // 표 25의 URL 각주는 source-cell 순서가 아니라 실제 물리 fragment page별로
    // 분할된다. p78의 기존 105·106, p79의 107–111, p80의 112–124 경계를
    // 고정해 한 fragment에 과예약해 다음 본문을 밀어내는 회귀를 막는다.
    let p78_tree = doc
        .build_page_render_tree(PAGE_78)
        .expect("render physical page 78");
    let p79_tree = doc
        .build_page_render_tree(PAGE_79)
        .expect("render physical page 79");
    let p80_tree = doc
        .build_page_render_tree(PAGE_80)
        .expect("render physical page 80");
    let mut p78_notes = String::new();
    let mut p79_notes = String::new();
    let mut p80_notes = String::new();
    footnote_text(&p78_tree.root, false, &mut p78_notes);
    footnote_text(&p79_tree.root, false, &mut p79_notes);
    footnote_text(&p80_tree.root, false, &mut p80_notes);
    for number in [105, 106] {
        assert!(
            p78_notes.contains(&format!("{number})")),
            "p78 각주 {number} 누락: {p78_notes}"
        );
    }
    assert!(
        !p78_notes.contains("107)"),
        "p78에는 표 cell 각주 107이 앞당겨지면 안 됨: {p78_notes}"
    );
    for number in 107..=111 {
        assert!(
            p79_notes.contains(&format!("{number})")),
            "p79 각주 {number} 누락: {p79_notes}"
        );
    }
    assert!(
        !p79_notes.contains("112)"),
        "p79에는 각주 112가 앞당겨지면 안 됨: {p79_notes}"
    );
    for number in 112..=124 {
        assert!(
            p80_notes.contains(&format!("{number})")),
            "p80 각주 {number} 누락: {p80_notes}"
        );
    }

    for (page, tree) in [(78, &p78_tree), (79, &p79_tree)] {
        let mut table = None;
        let mut separator = None;
        table_bottom(&tree.root, 885, &mut table);
        footnote_separator_top(&tree.root, &mut separator);
        assert!(
            table.expect("표 25") <= separator.expect("표 25 각주 separator") + 0.5,
            "p{page} 표 25 하단과 각주 separator가 겹치면 안 됨: table_bottom={table:?}, separator={separator:?}"
        );
    }
    assert!(
        p80.contains("유럽평의회는 2007년 5월 30일")
            && p80.contains("2007년 커뮤니케이션에 대한 대응으로"),
        "p80의 두 후속 본문이 각주 112–124 예약 때문에 p81로 밀리면 안 됨: {p80}"
    );
    let mut p80_body_bottom = None;
    let mut p80_separator = None;
    paragraph_bottom(&p80_tree.root, 889, &mut p80_body_bottom);
    footnote_separator_top(&p80_tree.root, &mut p80_separator);
    assert!(
        p80_body_bottom.expect("p80 para 889")
            <= p80_separator.expect("p80 footnote separator") + 0.5,
        "p80 표 25 뒤 본문과 각주 112 separator가 겹치면 안 됨: body_bottom={p80_body_bottom:?}, separator={p80_separator:?}"
    );
}

#[test]
fn native_hwp5_empty_rowbreak_table_uses_the_actual_existing_footnote_boundary() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage21 HWP evidence fixture");

    // pi=1682는 본문 각주 210 위에 통째로 들어간다. 40px safety margin을
    // 기계적으로 남기면 마지막 두 줄만 p155로 밀려 이후 물리 페이지가 전부 +1
    // shift 된다. Hancom PDF처럼 p154에서 표와 각주가 함께 끝나야 한다.
    let p154 = page_text(&doc, PAGE_154);
    let p155 = page_text(&doc, PAGE_155);
    assert!(
        p154.contains("생존 기증자가 모든 위험과 이익"),
        "p154에는 pi=1682의 마지막 셀 문단이 각주 210 위에 남아야 함: {p154}"
    );
    assert!(
        p155.trim_start().starts_with("(3) 평가 절차")
            && !p155.contains("생존 기증자가 모든 위험과 이익"),
        "p155는 pi=1682 tail 전용 페이지가 아니라 다음 절로 시작해야 함: {p155}"
    );

    let tree = doc
        .build_page_render_tree(PAGE_154)
        .expect("render physical page 154");
    let mut table = None;
    let mut separator = None;
    table_bottom(&tree.root, 1682, &mut table);
    footnote_separator_top(&tree.root, &mut separator);
    assert!(
        table.expect("p154 pi=1682") <= separator.expect("p154 footnote separator") + 0.5,
        "p154 pi=1682 하단과 기존 각주 separator가 겹치면 안 됨: table={table:?}, separator={separator:?}"
    );

    let p155_tree = doc
        .build_page_render_tree(PAGE_155)
        .expect("render physical page 155");
    let mut stale_tail = None;
    table_bottom(&p155_tree.root, 1682, &mut stale_tail);
    assert!(
        stale_tail.is_none(),
        "p155에는 pi=1682의 tail fragment가 남으면 안 됨: {stale_tail:?}"
    );
}

#[test]
fn native_hwp5_oversized_single_rowbreak_table_splits_inside_the_page_frame() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc = HwpDocument::from_bytes(&bytes).expect("parse stage21 HWP evidence fixture");

    // pi=1723은 선언 높이 363.8px보다 셀 본문 측정 높이가 1163.8px인 1×1
    // RowBreak 표다. 선언 높이만 예약하는 빈-anchor fast lane을 타면 p158
    // frame 밖으로 700px 이상 새므로, p157/p158의 두 fragment로 이어져야 한다.
    let p157 = page_text(&doc, PAGE_157);
    let p158 = page_text(&doc, PAGE_158);
    assert!(
        p157.contains("<BTS Guideline>") && p157.contains("<OPTN policy>"),
        "p157에는 표 37의 첫 fragment가 있어야 함: {p157}"
    );
    assert!(
        p158.contains("<BC Canada>") && p158.contains("신체 검진은 체중"),
        "p158에는 표 37의 continuation과 뒤 본문이 함께 있어야 함: {p158}"
    );

    for (page, label) in [(PAGE_157, "p157"), (PAGE_158, "p158")] {
        let tree = doc
            .build_page_render_tree(page)
            .unwrap_or_else(|e| panic!("render physical {label}: {e}"));
        let mut table = None;
        let mut footnote = None;
        let mut footer = None;
        table_bottom(&tree.root, 1723, &mut table);
        footnote_and_footer(&tree.root, &mut footnote, &mut footer);
        assert!(
            table.expect("pi=1723 fragment") <= footer.expect("page footer") + 0.5,
            "{label} pi=1723 fragment가 footer 밖으로 넘으면 안 됨: table={table:?}, footer={footer:?}"
        );
    }
}
