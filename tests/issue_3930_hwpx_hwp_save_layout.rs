//! Issue #3930 — HWPX 저장 뒤 표 분할과 바탕쪽 선택을 보존한다.

use std::fs;
use std::path::Path;

use rhwp::document_core::DocumentCore;
use rhwp::model::control::Control;
use rhwp::model::header_footer::{HeaderFooterApply, MasterPage};

const FIXTURE: &str = "samples/2025 행정업무운영 편람(최종).hwpx";

fn master_page_text(master_page: &MasterPage) -> String {
    let mut text = String::new();
    for paragraph in &master_page.paragraphs {
        text.push_str(&paragraph.text);
        for control in &paragraph.controls {
            let Control::Shape(shape) = control else {
                continue;
            };
            let Some(text_box) = shape
                .drawing()
                .and_then(|drawing| drawing.text_box.as_ref())
            else {
                continue;
            };
            for text_box_paragraph in &text_box.paragraphs {
                text.push_str(&text_box_paragraph.text);
            }
        }
    }
    text
}

#[test]
fn issue_3930_preserves_page_count_and_inherited_even_master_page() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let mut source = DocumentCore::from_bytes(&bytes).expect("HWPX fixture parse");

    assert_eq!(source.page_count(), 387, "원본 편람 쪽수");
    let saved = source.export_hwp_with_adapter().expect("HWP 저장");
    let reloaded = DocumentCore::from_bytes(&saved).expect("저장 HWP 재로드");

    assert_eq!(
        reloaded.page_count(),
        387,
        "p144 표가 다음 쪽으로 계속 이월돼야 한다"
    );

    let section = &reloaded.document().sections[2].section_def;
    let base_master_pages: Vec<&MasterPage> = section
        .master_pages
        .iter()
        .filter(|master_page| !master_page.is_extension)
        .collect();
    assert_eq!(base_master_pages.len(), 2, "HWP5 Both/Odd 저장 슬롯");
    assert_eq!(base_master_pages[0].apply_to, HeaderFooterApply::Both);
    assert!(
        master_page_text(base_master_pages[0]).contains("2025 행정업무운영 편람"),
        "p30 짝수 쪽은 앞 구역 책 제목 바탕쪽을 유지해야 한다"
    );
    assert_eq!(base_master_pages[1].apply_to, HeaderFooterApply::Odd);
    assert!(
        master_page_text(base_master_pages[1]).contains("제2장. 공문서 관리"),
        "홀수 쪽은 현재 구역 장 제목 바탕쪽을 사용해야 한다"
    );
}
