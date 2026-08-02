//! Issue #3738 Stage 9: 작은 RowBreak 표의 cell-footnote 전체 선예약이
//! 첫 fragment를 통째로 다음 쪽으로 미는 회귀를 실제 HWP로 고정한다.
//!
//! 한컴오피스 2020 기준 PDF p66에는 표 23의 0–4행(Organ Donation까지)과
//! 각주 76·77이 있고, p67은 Stephanie 행부터 이어진다. 표 전체 각주를
//! 첫 행 전부터 예약하면 p66 표가 전부 이월되어 이후 문단까지 한 쪽씩 밀린다.

use std::fs;
use std::path::Path;

use rhwp::wasm_api::HwpDocument;

const SAMPLE: &str =
    "samples/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적 선별기준 연구).hwp";
const PAGE_66: u32 = 65;
const PAGE_67: u32 = 66;

fn page_text(doc: &HwpDocument, page: u32) -> String {
    doc.extract_page_text_native(page)
        .unwrap_or_else(|e| panic!("extract physical page {}: {e}", page + 1))
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
}
