//! [#3693] `export-structure --mode clause` marker·문맥 회귀 테스트.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};

use rhwp::document_core::queries::structure::{
    build_structure, StructureDoc, StructureMode, StructureNode,
};
use rhwp::wasm_api::HwpDocument;

fn sample_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn load_structure(relative: &str) -> StructureDoc {
    let path = sample_path(relative);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("sample 읽기 실패 ({}): {error}", path.display()));
    let document = HwpDocument::from_bytes(&bytes)
        .unwrap_or_else(|error| panic!("sample 파싱 실패 ({}): {error:?}", path.display()));
    build_structure(document.document(), StructureMode::Clause)
}

fn find_at(nodes: &[StructureNode], section: usize, paragraph: usize) -> Option<&StructureNode> {
    for node in nodes {
        if node.section == section && node.paragraph == paragraph {
            return Some(node);
        }
        if let Some(found) = find_at(&node.children, section, paragraph) {
            return Some(found);
        }
    }
    None
}

fn contains_marker(nodes: &[StructureNode], marker: &str) -> bool {
    nodes
        .iter()
        .any(|node| node.marker == marker || contains_marker(&node.children, marker))
}

#[test]
fn real_agreement_keeps_items_under_article() {
    let structure = load_structure("samples/hwp3-sample16-hwp5.hwp");
    let article = find_at(&structure.roots, 0, 945).expect("협정서 제1조");

    assert_eq!((article.kind, article.marker.as_str()), ("조", "제1조"));
    let child_markers: Vec<&str> = article
        .children
        .iter()
        .map(|node| node.marker.as_str())
        .collect();
    assert_eq!(child_markers, vec!["1.", "2.", "3."]);
}

#[test]
fn real_work_plan_date_is_not_a_clause_item() {
    let structure = load_structure("samples/2022년 국립국어원 업무계획.hwp");

    assert!(!contains_marker(&structure.roots, "2022."));
    assert!(find_at(&structure.roots, 0, 10).is_none());
}

#[test]
fn real_handbook_toc_numbers_are_not_clause_items() {
    let structure = load_structure("samples/2025 행정업무운영 편람(최종).hwp");

    assert_eq!(
        find_at(&structure.roots, 0, 8).map(|node| (node.kind, node.marker.as_str())),
        Some(("장", "제1장"))
    );
    assert_eq!(
        find_at(&structure.roots, 0, 14).map(|node| (node.kind, node.marker.as_str())),
        Some(("절", "제1절"))
    );
    assert!(find_at(&structure.roots, 0, 9).is_none());
    assert!(find_at(&structure.roots, 0, 15).is_none());
}
