//! `template_entity` 계약 회귀 테스트 — hwpx-template-engine의 `TemplateEntityGenerator`가
//! 실제로 낸 golden(`tests/fixtures/template-entity/golden/`)과 바이트 단위로 비교해
//! 패리티를 고정한다.
//!
//! 픽스처는 `scripts/template_entity_fixtures.py`로 재생성한다. `code`는 각 픽스처
//! 디렉터리명, `package`는 `com.example.fix` — Java 생성기를 이 값으로 돌려 golden을 냈다.
#![cfg(not(target_arch = "wasm32"))]

use rhwp::document_core::DocumentCore;
use std::path::{Path, PathBuf};

const PACKAGE: &str = "com.example.fix";

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template-entity")
}

fn core_of(code: &str) -> DocumentCore {
    let path = fixtures_dir().join(format!("{code}.hwpx"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path:?} 읽기 실패: {e}"));
    DocumentCore::from_bytes(&bytes).unwrap_or_else(|e| panic!("{path:?} 파싱 실패: {e}"))
}

fn golden(case: &str, file: &str) -> String {
    let path = fixtures_dir().join("golden").join(case).join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?} 읽기 실패: {e}"))
}

/// 성공 픽스처 1건 — Java golden과 두 소스 모두 바이트 단위로 같아야 한다.
fn assert_matches_golden(code: &str, data_class_name: &str, module_class_name: &str) {
    let core = core_of(code);
    let result = core.template_entity(code, PACKAGE);
    assert!(
        result.errors.is_empty(),
        "'{code}' 는 에러 없이 방출되어야 합니다: {:?}",
        result.errors
    );
    assert_eq!(result.data_class_name, data_class_name, "data class 이름");
    assert_eq!(
        result.module_class_name, module_class_name,
        "module class 이름"
    );
    assert_eq!(
        result.data_class_source,
        golden(code, &format!("{data_class_name}.java")),
        "'{code}' data class 소스가 Java golden과 다릅니다"
    );
    assert_eq!(
        result.module_class_source,
        golden(code, &format!("{module_class_name}.java")),
        "'{code}' module class 소스가 Java golden과 다릅니다"
    );
}

#[test]
fn fix_flat_matches_java_golden() {
    assert_matches_golden("fix-flat", "FixFlatData", "FixFlatTemplateModule");
}

#[test]
fn fix_nested_matches_java_golden() {
    assert_matches_golden("fix-nested", "FixNestedData", "FixNestedTemplateModule");
}

#[test]
fn fix_lenient_matches_java_golden() {
    assert_matches_golden("fix-lenient", "FixLenientData", "FixLenientTemplateModule");
}

/// 마커 없는 문서(fix-lenient)는 관대한 폴백 — 모든 필드가 top-level, 에러 없음.
#[test]
fn fix_lenient_has_no_blocks() {
    let core = core_of("fix-lenient");
    let result = core.template_entity("fix-lenient", PACKAGE);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        !result.data_class_source.contains("List<"),
        "마커 없는 문서는 반복 블록(List<...>)이 없어야 합니다:\n{}",
        result.data_class_source
    );
}

/// BODY 마커 2개 — 검증 실패. errors 에 Java와 같은 한국어 메시지가 그대로 실린다.
#[test]
fn fix_error_two_bodies_reports_java_message() {
    let core = core_of("fix-error-two-bodies");
    let result = core.template_entity("fix-error-two-bodies", PACKAGE);
    let expected = golden("fix-error-two-bodies", "error.txt");
    assert_eq!(
        result.errors,
        vec![expected.trim_end().to_string()],
        "에러 메시지가 Java golden과 다릅니다"
    );
    assert!(
        result.data_class_source.is_empty() && result.module_class_source.is_empty(),
        "에러가 있으면 소스를 방출하지 않아야 합니다"
    );
}
