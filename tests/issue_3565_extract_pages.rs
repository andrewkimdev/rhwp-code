//! `extract-pages` — 쪽 범위만 남겨 저장하는 진단 도구 (#3565).
//!
//! 387쪽 문서가 저장 후 한컴에서 열리지 않을 때, 절반씩 잘라 재현 여부를 보면 방아쇠를
//! 좁힐 수 있다. 그때 필요한 것이 이 기능이다.
//!
//! 계약은 셋이다.
//!
//! 1. 요청 범위에 걸친 문단은 남고, 나머지는 지워진다 (쪽수가 줄어든다).
//! 2. 잘라 낸 결과가 **다시 열리는 정상 문서**여야 한다 — 재파싱이 되어야 이분법이 성립한다.
//! 3. 범위가 잘못되면 조용히 넘어가지 않고 오류를 낸다.
//!
//! 결과 쪽수가 요청 범위와 정확히 같을 필요는 없다(잘라 낸 뒤 레이아웃이 다시 흐른다).
//! 목적은 재현 최소화이지 정밀한 페이지 오려내기가 아니다.
#![cfg(not(target_arch = "wasm32"))]

use rhwp::document_core::DocumentCore;

/// 여러 쪽·여러 구역이 있는 표본.
const SAMPLE: &str = "samples/issue2083_hide_fill_page.hwpx";

fn load() -> DocumentCore {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = std::fs::read(&path).expect("표본 읽기");
    DocumentCore::from_bytes(&bytes).expect("파싱")
}

fn total_paragraphs(core: &DocumentCore) -> usize {
    core.document()
        .sections
        .iter()
        .map(|s| s.paragraphs.len())
        .sum()
}

/// 첫 쪽만 남기면 쪽수와 문단 수가 함께 줄어든다.
#[test]
fn extracting_first_page_shrinks_the_document() {
    let mut core = load();
    let pages_before = core.page_count();
    let paras_before = total_paragraphs(&core);
    assert!(
        pages_before >= 2,
        "표본이 1쪽뿐이라 추출을 검증할 수 없다 — 표본이 바뀌었는지 확인하라"
    );

    let report = core.extract_page_range(1, 1).expect("1쪽 추출");

    assert_eq!(report.pages_before, pages_before);
    assert!(
        report.pages_after < pages_before,
        "쪽수가 줄지 않았다: {} → {}",
        report.pages_before,
        report.pages_after
    );
    assert!(report.removed > 0, "지운 문단이 없다");
    assert!(
        total_paragraphs(&core) < paras_before,
        "문단이 줄지 않았다: {paras_before} → {}",
        total_paragraphs(&core)
    );
}

/// 잘라 낸 결과가 다시 열려야 이분법이 성립한다.
#[test]
fn extracted_document_is_still_loadable() {
    let mut core = load();
    core.extract_page_range(1, 1).expect("1쪽 추출");

    let saved = core.export_hwp_native().expect("저장");
    let reloaded = DocumentCore::from_bytes(&saved).expect("잘라 낸 산출물 재파싱");
    assert!(
        reloaded
            .document()
            .sections
            .iter()
            .any(|s| !s.paragraphs.is_empty()),
        "재파싱 결과가 비어 있다"
    );
}

/// 전체 범위를 요청하면 아무것도 지우지 않는다.
#[test]
fn full_range_keeps_everything() {
    let mut core = load();
    let pages = core.page_count();
    let paras = total_paragraphs(&core);

    let report = core.extract_page_range(1, pages).expect("전체 범위");

    assert_eq!(report.removed, 0, "전체 범위인데 문단을 지웠다");
    assert_eq!(report.pages_after, pages);
    assert_eq!(total_paragraphs(&core), paras);
}

/// 잘못된 범위는 조용히 넘어가지 않고 오류를 낸다.
#[test]
fn invalid_ranges_are_rejected() {
    let mut core = load();
    let pages = core.page_count();

    assert!(core.extract_page_range(0, 1).is_err(), "0쪽 시작을 받았다");
    assert!(
        core.extract_page_range(3, 2).is_err(),
        "from > to 를 받았다"
    );
    assert!(
        core.extract_page_range(pages + 1, pages + 2).is_err(),
        "문서 쪽수를 넘는 시작 쪽을 받았다"
    );
}
