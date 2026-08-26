//! html_export_tests — tests/mod.rs 에서 무변동 이동
use super::*;

#[test]
fn test_render_empty_page_html() {
    let doc = HwpDocument::create_empty();
    let html = doc.render_page_html_native(0);
    assert!(html.is_ok());
    let html = html.unwrap();
    assert!(html.contains("hwp-page"));
}



#[test]
fn test_css_color_rgba_and_border_width_keywords() {
    // rgba() 색상: 브라우저는 반투명/알파 포함 색을 rgba(r, g, b, a)로 직렬화한다.
    assert_eq!(
        super::css_color_to_hwp_bgr("rgba(255, 0, 0, 1)"),
        Some(0x0000FF),
        "rgba() 불투명 빨강 → BGR"
    );
    assert_eq!(
        super::css_color_to_hwp_bgr("rgba(0, 128, 255, 0.5)"),
        Some(0xFF8000),
        "rgba() 반투명 색도 RGB 성분은 파싱되어야 함"
    );
    // 완전 투명(alpha=0)은 색 없음으로 처리
    assert_eq!(
        super::css_color_to_hwp_bgr("rgba(255, 0, 0, 0)"),
        None,
        "rgba() alpha=0 → 색 없음"
    );

    // border 축약형의 rgba() 색상
    let (w, c, s) = super::parse_css_border_shorthand("1px solid rgba(255, 0, 0, 1)");
    assert!((w - 0.75).abs() < 0.01, "border width 1px -> 0.75pt");
    assert_eq!(c, 0x0000FF, "border rgba() 색상 빨강 (BGR)");
    assert_eq!(s, 1, "border style solid");

    // CSS 표준 border-width 키워드: thin(1px)/medium(3px)/thick(5px)
    // 키워드를 인식하지 못하면 width 0 → 테두리 전체가 소실된다.
    let (w_thin, _, s_thin) = super::parse_css_border_shorthand("thin solid #000000");
    assert!((w_thin - 0.75).abs() < 0.01, "thin = 1px = 0.75pt");
    assert_eq!(s_thin, 1);

    let (w_med, c_med, _) = super::parse_css_border_shorthand("medium solid #ff0000");
    assert!((w_med - 2.25).abs() < 0.01, "medium = 3px = 2.25pt");
    assert_eq!(c_med, 0x0000FF);

    let (w_thick, _, _) = super::parse_css_border_shorthand("thick solid #000000");
    assert!((w_thick - 3.75).abs() < 0.01, "thick = 5px = 3.75pt");
}



#[test]
fn test_html_utility_functions() {
    // decode_html_entities
    assert_eq!(super::decode_html_entities("&amp;&lt;&gt;"), "&<>");
    assert_eq!(super::decode_html_entities("&nbsp;"), " ");

    // html_strip_tags
    assert_eq!(super::html_strip_tags("<b>bold</b>"), "bold");
    assert_eq!(super::html_strip_tags("<p>text<br/>more</p>"), "textmore");

    // html_to_plain_text
    assert_eq!(
        super::html_to_plain_text("<p>hello &amp; world</p>"),
        "hello & world"
    );

    // parse_inline_style
    assert_eq!(
        super::parse_inline_style(r#"<p style="text-align:center;font-size:12pt;">"#),
        "text-align:center;font-size:12pt;"
    );

    // parse_css_value
    assert_eq!(
        super::parse_css_value("text-align:center;font-size:12pt;", "text-align"),
        Some("center".to_string())
    );
    assert_eq!(
        super::parse_css_value("font-size:12pt;", "font-size"),
        Some("12pt".to_string())
    );

    // parse_pt_value
    assert_eq!(super::parse_pt_value("10.0pt"), Some(10.0));
    assert_eq!(super::parse_pt_value("12px"), Some(9.0)); // 12 * 0.75

    // css_color_to_hwp_bgr
    assert_eq!(super::css_color_to_hwp_bgr("#ff0000"), Some(0x0000FF)); // red → BGR
    assert_eq!(super::css_color_to_hwp_bgr("#00ff00"), Some(0x00FF00)); // green
    assert_eq!(
        super::css_color_to_hwp_bgr("rgb(255, 0, 0)"),
        Some(0x0000FF)
    );
}



#[test]
fn test_hml_source_format_is_reported_without_reusing_hwp_save_path() {
    let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<HWPML Style="embed" SubVersion="9.0.1.0" Version="2.9">
  <HEAD SecCnt="1" />
  <BODY><SECTION Id="0"><P ParaShape="0" Style="0"><TEXT CharShape="0"><CHAR>HML</CHAR></TEXT></P></SECTION></BODY>
  <TAIL />
</HWPML>"#;
    let doc = HwpDocument::new(bytes).expect("HML 문서를 열어야 한다");

    assert_eq!(doc.get_source_format(), "hml");
}



#[test]
fn test_hml_save_state_is_one_canonical_dto_for_hml_non_hml_and_unknown_equation() {
    let lawful = br#"<HWPML Version="2.91"><HEAD/><BODY><SECTION><P><TEXT><CHAR>ok</CHAR></TEXT></P></SECTION></BODY><TAIL/></HWPML>"#;
    let mut doc = HwpDocument::new(lawful).expect("lawful HML");
    let state: Value = serde_json::from_str(&doc.get_hml_save_state()).expect("save state JSON");
    assert_eq!(
        state,
        serde_json::json!({
            "sourceFormat": "hml",
            "hmlSavable": true,
            "blockers": [],
        })
    );

    doc.create_blank_document_native()
        .expect("non-HML blank document");
    let state: Value = serde_json::from_str(&doc.get_hml_save_state()).expect("save state JSON");
    assert_eq!(
        state,
        serde_json::json!({
            "sourceFormat": "hwp",
            "hmlSavable": false,
            "blockers": [{
                "code": "HML_SOURCE_REQUIRED",
                "xmlPath": "/HWPML",
                "message": "HML 원본 문서만 HML로 저장할 수 있습니다",
                "preserved": false,
            }],
        })
    );

    let unknown = br#"<HWPML Version="2.91"><HEAD/><BODY><SECTION><P><TEXT><EQUATION FutureAttr="1"><SCRIPT>x</SCRIPT><FUTURE/></EQUATION></TEXT></P></SECTION></BODY><TAIL/></HWPML>"#;
    let doc = HwpDocument::new(unknown).expect("unknown equation semantics remain readable");
    let state: Value = serde_json::from_str(&doc.get_hml_save_state()).expect("save state JSON");
    assert_eq!(state["hmlSavable"], false);
    assert_eq!(state["sourceFormat"], "hml");
    assert_eq!(state["blockers"].as_array().map(Vec::len), Some(2));
    for blocker in state["blockers"].as_array().unwrap() {
        assert_eq!(blocker["code"], "HML_UNSUPPORTED_EQUATION_SEMANTICS");
        assert_eq!(blocker["preserved"], false);
    }
}



#[test]
fn test_hml_open_metadata_exposes_import_warnings() {
    let bytes = include_bytes!("../../../samples/hml/formatting_table.hml");
    let doc = HwpDocument::new(bytes).expect("real HML fixture should open");
    let metadata: Value =
        serde_json::from_str(&doc.get_hml_open_metadata()).expect("HML metadata JSON");

    assert_eq!(metadata["format"], "hml");
    assert_eq!(metadata["hwpmlVersion"], "2.91");
    assert_eq!(metadata["encoding"], "utf-8");
    assert_eq!(metadata["resourceCount"], 0);
    assert_eq!(metadata["hmlSavable"], true);
    assert_eq!(metadata["saveBlockers"], serde_json::json!([]));
    assert!(metadata["warnings"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|warning| warning["xmlPath"] == "/HWPML/TAIL/SCRIPTCODE")
    }));
}



#[test]
fn test_hml_open_metadata_escapes_special_characters_as_valid_json() {
    let bytes = include_bytes!("../../../samples/hml/formatting_table.hml");
    let mut doc = HwpDocument::new(bytes).expect("real HML fixture should open");
    let special = "2.91\"\\\n한글\t";
    let metadata = doc
        .core
        .hml_metadata
        .as_mut()
        .expect("HML metadata should exist");
    metadata.hwpml_version = Some(special.to_string());
    metadata.warnings[0].message = special.to_string();

    let json: Value = serde_json::from_str(&doc.get_hml_open_metadata())
        .expect("metadata must remain valid JSON");

    assert_eq!(json["hwpmlVersion"], special);
    assert_eq!(json["warnings"][0]["message"], special);
}



#[test]
fn test_export_hml_binding_preserves_edit_and_fragment() {
    let bytes = include_bytes!("../../../samples/hml/formatting_table.hml");
    let mut doc = HwpDocument::new(bytes).expect("real HML fixture should open");
    let (section_index, paragraph_index) = doc
        .document()
        .sections
        .iter()
        .enumerate()
        .find_map(|(section_index, section)| {
            section
                .paragraphs
                .iter()
                .position(|paragraph| !paragraph.text.is_empty())
                .map(|paragraph_index| (section_index, paragraph_index))
        })
        .expect("fixture should contain text");
    doc.insert_text_native(section_index, paragraph_index, 0, "WASM_EDIT_")
        .expect("apply public edit");

    let exported = doc.export_hml().expect("exportHml should succeed");

    assert_eq!(
        crate::parser::detect_format(&exported),
        crate::parser::FileFormat::Hml
    );
    assert!(String::from_utf8_lossy(&exported).contains("<SCRIPTCODE"));
    let reparsed = DocumentCore::from_bytes(&exported).expect("exportHml output should reparse");
    assert!(
        reparsed.document().sections[section_index].paragraphs[paragraph_index]
            .text
            .starts_with("WASM_EDIT_")
    );
}



#[test]
fn test_export_hml_error_message_exposes_blocker_codes() {
    let fixture = std::str::from_utf8(include_bytes!("../../../samples/hml/formatting_table.hml"))
        .expect("fixture is UTF-8");
    let lossy = fixture.replacen("Type=\"None\"", "Type=\"Dash\"", 1);
    let doc = HwpDocument::new(lossy.as_bytes()).expect("lossy HML should import");
    let error = doc
        .core
        .export_hml_native()
        .expect_err("lossy import must block exportHml");
    let blocker = &error.blockers()[0];

    let message = super::format_hml_export_error(&error);

    assert!(message.contains(blocker.code), "{message}");
    assert!(message.contains(&blocker.xml_path), "{message}");
    assert!(message.contains(&blocker.message), "{message}");
}



#[test]
fn test_hml_open_metadata_uses_shared_preflight_for_import_and_ir_loss() {
    let fixture = std::str::from_utf8(include_bytes!("../../../samples/hml/formatting_table.hml"))
        .expect("fixture is UTF-8");
    let lossy = fixture.replacen("Type=\"None\"", "Type=\"Dash\"", 1);
    let import_loss = HwpDocument::new(lossy.as_bytes()).expect("lossy HML should import");
    let import_json: Value =
        serde_json::from_str(&import_loss.get_hml_open_metadata()).expect("metadata JSON");
    assert_eq!(import_json["hmlSavable"], false);
    assert!(import_json["saveBlockers"]
        .as_array()
        .is_some_and(|blockers| {
            blockers.iter().any(|blocker| {
                blocker["code"] == "UNSUPPORTED_ATTRIBUTE"
                    && blocker["xmlPath"]
                        == "/HWPML/HEAD/MAPPINGTABLE/BORDERFILLLIST/BORDERFILL/LEFTBORDER"
                    && blocker["message"]
                        .as_str()
                        .is_some_and(|message| !message.is_empty())
            })
        }));

    let mut edited_ir = HwpDocument::new(fixture.as_bytes()).expect("lawful HML should import");
    edited_ir.document_mut().sections[0].paragraphs[0].column_type =
        crate::model::paragraph::ColumnBreakType::Section;
    let ir_json: Value =
        serde_json::from_str(&edited_ir.get_hml_open_metadata()).expect("metadata JSON");
    assert_eq!(ir_json["hmlSavable"], false);
    assert!(ir_json["saveBlockers"].as_array().is_some_and(|blockers| {
        blockers.iter().any(|blocker| {
            blocker["code"] == "HML_UNSUPPORTED_IR"
                && blocker["xmlPath"]
                    .as_str()
                    .is_some_and(|path| !path.is_empty())
                && blocker["message"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty())
        })
    }));
}



#[test]
fn test_hml_open_metadata_reports_mixed_import_and_ir_loss() {
    let fixture = std::str::from_utf8(include_bytes!("../../../samples/hml/formatting_table.hml"))
        .expect("fixture is UTF-8");
    let lossy = fixture.replacen("Type=\"None\"", "Type=\"Dash\"", 1);
    let mut doc = HwpDocument::new(lossy.as_bytes()).expect("lossy HML should import");
    doc.document_mut().sections[0].paragraphs[0].column_type =
        crate::model::paragraph::ColumnBreakType::Section;

    let metadata: Value =
        serde_json::from_str(&doc.get_hml_open_metadata()).expect("metadata JSON");
    let blockers = metadata["saveBlockers"]
        .as_array()
        .expect("save blockers array");

    assert_eq!(metadata["hmlSavable"], false);
    assert!(blockers
        .iter()
        .any(|blocker| blocker["code"] == "UNSUPPORTED_ATTRIBUTE"));
    assert!(blockers
        .iter()
        .any(|blocker| blocker["code"] == "HML_UNSUPPORTED_IR"));
}
