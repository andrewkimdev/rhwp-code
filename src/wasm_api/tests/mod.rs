use super::*;
mod html_export_tests;
mod issue_regressions;
mod pagination_diag;
mod paste_clipboard_tests;
mod picture_tests;
mod save_field_tests;
mod table_tests;
mod task_features;
mod fixtures;
pub(crate) use fixtures::*;

use crate::model::document::{Document, Section};
use crate::model::paragraph::{LineSeg, Paragraph};
use crate::paint::{RenderProfile, LAYER_TREE_SCHEMA};
use crate::parser::control::parse_common_obj_attr;
use serde_json::Value;

#[test]
fn test_create_empty_document() {
    let doc = HwpDocument::create_empty();
    assert_eq!(doc.page_count(), 1);
}











#[test]
fn test_empty_document_info() {
    let doc = HwpDocument::create_empty();
    let info = doc.get_document_info();
    assert!(info.contains("\"pageCount\":1"));
    assert!(info.contains("\"encrypted\":false"));
}

#[test]
fn test_render_empty_page_svg() {
    let doc = HwpDocument::create_empty();
    let svg = doc.render_page_svg_native(0);
    assert!(svg.is_ok());
    let svg = svg.unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
}

#[test]
fn test_page_out_of_range() {
    let doc = HwpDocument::create_empty();
    let result = doc.render_page_svg_native(999);
    assert!(result.is_err());
    match result.unwrap_err() {
        HwpError::PageOutOfRange(n) => assert_eq!(n, 999),
        _ => panic!("Expected PageOutOfRange error"),
    }
}

#[test]
fn test_page_layer_tree_export_uses_schema_contract() {
    let doc = HwpDocument::create_empty();
    let json = doc
        .get_page_layer_tree_native(0)
        .expect("empty document layer tree should export");
    let parsed: Value = serde_json::from_str(&json).expect("PageLayerTree JSON");

    assert_eq!(
        parsed["schemaVersion"].as_u64(),
        Some(LAYER_TREE_SCHEMA.schema_version as u64)
    );
    assert_eq!(
        parsed["resourceTableVersion"].as_u64(),
        Some(LAYER_TREE_SCHEMA.resource_table_version as u64)
    );
    assert_eq!(parsed["unit"].as_str(), Some(LAYER_TREE_SCHEMA.unit));
    assert_eq!(
        parsed["coordinateSystem"].as_str(),
        Some(LAYER_TREE_SCHEMA.coordinate_system)
    );
    assert_eq!(parsed["profile"].as_str(), Some("screen"));
    assert!(parsed["buildOptions"].is_object());
    assert!(parsed["debugOptions"].is_object());
    assert!(parsed["outputOptions"].is_object());
}

#[test]
fn test_page_layer_tree_export_uses_requested_profile() {
    let doc = HwpDocument::create_empty();
    for (profile, expected) in [
        (RenderProfile::FastPreview, "fastPreview"),
        (RenderProfile::Screen, "screen"),
        (RenderProfile::Print, "print"),
        (RenderProfile::HighQuality, "highQuality"),
    ] {
        let json = doc
            .get_page_layer_tree_with_profile_native(0, profile)
            .expect("profiled layer tree should export");
        let parsed: Value = serde_json::from_str(&json).expect("PageLayerTree JSON");
        assert_eq!(parsed["profile"].as_str(), Some(expected));
    }
}

#[test]
fn test_page_layer_tree_export_preserves_output_options() {
    let mut doc = HwpDocument::create_empty();
    doc.set_show_paragraph_marks(true);
    doc.set_show_control_codes(true);
    doc.set_show_transparent_borders(true);
    doc.set_clip_enabled(false);
    doc.set_debug_overlay(true);

    let json = doc
        .get_page_layer_tree_native(0)
        .expect("layer tree should export output options");
    let parsed: Value = serde_json::from_str(&json).expect("PageLayerTree JSON");

    assert_eq!(
        parsed["buildOptions"]["showTransparentBorders"].as_bool(),
        Some(true)
    );
    assert_eq!(parsed["buildOptions"]["clipEnabled"].as_bool(), Some(false));
    assert_eq!(parsed["debugOptions"]["debugOverlay"].as_bool(), Some(true));
    assert_eq!(
        parsed["outputOptions"]["showParagraphMarks"].as_bool(),
        Some(true)
    );
    assert_eq!(
        parsed["outputOptions"]["showControlCodes"].as_bool(),
        Some(true)
    );
    assert_eq!(
        parsed["outputOptions"]["showTransparentBorders"].as_bool(),
        Some(true)
    );
    assert_eq!(
        parsed["outputOptions"]["clipEnabled"].as_bool(),
        Some(false)
    );
    assert_eq!(
        parsed["outputOptions"]["debugOverlay"].as_bool(),
        Some(true)
    );
}

#[test]
fn test_canvaskit_replay_plan_export_uses_mode_policy() {
    let doc = HwpDocument::create_empty();

    let default_json = doc
        .get_canvaskit_replay_plan_native(0, "default")
        .expect("empty document CanvasKit plan should export");
    assert!(default_json.contains("\"mode\":\"default\""));
    assert!(default_json.contains("\"hiddenCanvas2dOverlayAllowed\":false"));
    assert!(default_json.contains("\"directReplayRequired\":true"));
    assert!(default_json.contains("\"requiredFontFamilies\""));
    assert!(default_json.contains("\"requiredFontFamiliesComplete\":true"));

    let compat_json = doc
        .get_canvaskit_replay_plan_native(0, "compat")
        .expect("compat CanvasKit plan should export");
    assert!(compat_json.contains("\"mode\":\"compat\""));
    assert!(compat_json.contains("\"hiddenCanvas2dOverlayAllowed\":false"));
    assert!(compat_json.contains("\"directReplayRequired\":true"));

    let invalid = doc.get_canvaskit_replay_plan_native(0, "canvas2d");
    let error = invalid.expect_err("unsupported CanvasKit replay mode should fail");
    let message = error.to_string();
    assert!(message.contains("canvas2d"));
    assert!(message.contains("allowed modes: default, compat"));
}

#[test]
fn test_empty_document_canvaskit_preflight_api_schema() {
    let doc = HwpDocument::create_empty();

    let json = doc
        .get_canvaskit_document_preflight("default", "screen")
        .expect("empty document CanvasKit preflight should export");
    let parsed: Value = serde_json::from_str(&json).expect("CanvasKit preflight JSON");

    assert_eq!(parsed["schemaVersion"].as_u64(), Some(1));
    assert_eq!(parsed["mode"].as_str(), Some("default"));
    assert_eq!(parsed["profile"].as_str(), Some("screen"));
    assert!(matches!(
        parsed["status"].as_str(),
        Some("eligible" | "ineligible" | "incomplete")
    ));
    assert!(parsed["eligible"].is_boolean());
    assert!(parsed["complete"].is_boolean());
    assert_eq!(parsed["pageCount"].as_u64(), Some(1));
    assert!(parsed["scannedPages"].is_u64());
    assert!(parsed["scannedWorkUnits"].is_u64());
    assert_eq!(parsed["limits"]["maxPages"].as_u64(), Some(128));
    assert_eq!(parsed["limits"]["maxWorkUnits"].as_u64(), Some(50_000));
    assert_eq!(parsed["limits"]["maxBlockers"].as_u64(), Some(32));
    assert_eq!(
        parsed["limits"]["maxRequiredFontFamilies"].as_u64(),
        Some(256)
    );
    assert!(parsed["summary"]["totalItems"].is_u64());
    assert!(parsed["blockers"].is_array());
    assert!(parsed["requiredFontFamilies"].is_array());
    assert!(parsed["capabilityDigest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 71 && digest.starts_with("blake3:")));
    assert!(parsed.get("root").is_none());
    assert!(parsed.get("resources").is_none());
}

#[test]
fn test_normalize_canvas_scale_rejects_invalid_page_dimensions() {
    for (width, height) in [
        (0.0, 100.0),
        (100.0, 0.0),
        (-1.0, 100.0),
        (100.0, -1.0),
        (f64::NAN, 100.0),
        (100.0, f64::NAN),
        (f64::INFINITY, 100.0),
        (100.0, f64::INFINITY),
    ] {
        assert!(
            normalize_canvas_scale(width, height, 1.0).is_err(),
            "invalid page dimensions must fail: {width} x {height}"
        );
    }
}

#[test]
fn test_normalize_canvas_scale_clamps_request_and_canvas_extent() {
    assert_eq!(normalize_canvas_scale(100.0, 100.0, 0.0), Ok(1.0));
    assert_eq!(normalize_canvas_scale(100.0, 100.0, f64::NAN), Ok(1.0));
    assert_eq!(normalize_canvas_scale(100.0, 100.0, f64::INFINITY), Ok(1.0));
    assert_eq!(normalize_canvas_scale(100.0, 100.0, 0.1), Ok(0.25));
    assert_eq!(normalize_canvas_scale(100.0, 100.0, 20.0), Ok(12.0));

    let scale = normalize_canvas_scale(20_000.0, 10_000.0, 1.0)
        .expect("large finite page should be scaled down");
    assert!((scale - (16_384.0 / 20_000.0)).abs() < f64::EPSILON);
}

#[test]
fn test_scaled_canvas_extent_keeps_fractional_a4_edge() {
    // A4를 CSS 96dpi 좌표로 환산한 뒤 144dpi(1.5x) bitmap으로 옮기는 경계값이다.
    // `as u32` 절사 회귀 시 각각 1190 × 1683이 되어 마지막 물리 픽셀이 사라진다.
    assert_eq!(scaled_canvas_extent(793.700_787, 1.5), 1191);
    assert_eq!(scaled_canvas_extent(1_122.519_685, 1.5), 1684);
    assert_eq!(scaled_canvas_extent(16_384.25, 1.0), 16_384);
}

#[test]
fn test_document_with_paragraphs() {
    use crate::model::document::SectionDef;
    use crate::model::page::PageDef;

    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();

    // A4 크기 페이지 정의 (단위: HwpUnit, 1pt = 100)
    let page_def = PageDef {
        width: 59528,  // A4 가로 (약 210mm)
        height: 84188, // A4 세로 (약 297mm)
        margin_left: 8504,
        margin_right: 8504,
        margin_top: 5669,
        margin_bottom: 4252,
        margin_header: 4252,
        margin_footer: 4252,
        ..Default::default()
    };

    document.sections.push(Section {
        section_def: SectionDef {
            page_def,
            ..Default::default()
        },
        paragraphs: vec![
            Paragraph {
                text: "첫 번째 문단".to_string(),
                line_segs: vec![LineSeg {
                    line_height: 400,
                    baseline_distance: 320,
                    ..Default::default()
                }],
                ..Default::default()
            },
            Paragraph {
                text: "두 번째 문단".to_string(),
                line_segs: vec![LineSeg {
                    line_height: 400,
                    baseline_distance: 320,
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        raw_stream: None,
    });
    doc.set_document(document);

    assert_eq!(doc.page_count(), 1);
    let svg = doc.render_page_svg_native(0).unwrap();
    // 문자별 개별 렌더링이므로 개별 문자 존재 확인
    assert!(svg.contains(">첫</text>"));
    assert!(svg.contains(">문</text>"));
    assert!(svg.contains(">단</text>"));
}

#[test]
fn test_set_dpi() {
    let mut doc = HwpDocument::create_empty();
    doc.set_dpi(72.0);
    assert!((doc.get_dpi() - 72.0).abs() < 0.01);
}

#[test]
fn test_fallback_font() {
    let mut doc = HwpDocument::create_empty();
    assert_eq!(doc.get_fallback_font(), DEFAULT_FALLBACK_FONT);
    doc.set_fallback_font("/custom/font.ttf");
    assert_eq!(doc.get_fallback_font(), "/custom/font.ttf");
}

#[test]
fn test_viewer_creation() {
    let doc = HwpDocument::create_empty();
    let viewer = HwpViewer::new(doc);
    assert_eq!(viewer.page_count(), 1);
    assert_eq!(viewer.pending_task_count(), 0);
}

#[test]
fn test_viewer_viewport_update() {
    let doc = HwpDocument::create_empty();
    let mut viewer = HwpViewer::new(doc);
    viewer.update_viewport(0.0, 0.0, 800.0, 600.0);
    let visible = viewer.visible_pages();
    assert!(!visible.is_empty());
}

#[test]
fn test_export_hwp_empty() {
    let doc = HwpDocument::create_empty();
    let bytes = doc.export_hwp_native();
    assert!(bytes.is_ok());
    let bytes = bytes.unwrap();
    // CFB 시그니처 확인
    assert!(bytes.len() > 512);
    assert_eq!(&bytes[0..4], &[0xD0, 0xCF, 0x11, 0xE0]);
}

#[test]
fn test_hwp_error_display() {
    let err = HwpError::InvalidFile("테스트".to_string());
    assert!(err.to_string().contains("테스트"));
    let err = HwpError::PageOutOfRange(5);
    assert!(err.to_string().contains("5"));
}

/// 텍스트의 UTF-16 char_offsets를 생성한다.
fn make_char_offsets(text: &str) -> Vec<u32> {
    let mut offsets = Vec::new();
    let mut pos: u32 = 0;
    for c in text.chars() {
        offsets.push(pos);
        pos += if (c as u32) > 0xFFFF { 2 } else { 1 };
    }
    offsets
}

#[test]
fn test_merge_then_control_layout_has_col_span() {
    let mut doc = create_doc_with_table();
    // 병합 전: colSpan=1
    let layout_before = doc.get_page_control_layout_native(0).unwrap();
    assert!(
        !layout_before.contains("\"colSpan\":2"),
        "병합 전에는 colSpan:2가 없어야 합니다"
    );

    // 병합: 첫 행의 2개 셀
    doc.merge_table_cells_native(0, 0, 0, 0, 0, 0, 1).unwrap();

    // 병합 후: colSpan=2가 레이아웃에 반영되어야 함
    let layout_after = doc.get_page_control_layout_native(0).unwrap();
    assert!(
        layout_after.contains("\"colSpan\":2"),
        "병합 후 colSpan:2가 있어야 합니다. 레이아웃: {}",
        layout_after
    );
}

/// 한컴 오피스 참조 파일 분석: 병합된 표의 셀 구조 확인
#[test]
fn test_analyze_hancom_merged_file() {
    use crate::parser::record::Record;
    use std::path::Path;

    let orig_path = Path::new("samples/hwp_table_test.hwp");
    let hancom_path = Path::new("samples/hwp_table_test-m.hwp");
    if !orig_path.exists() || !hancom_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let hancom_data = std::fs::read(hancom_path).unwrap();

    // 원본 BodyText
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();

    // 한컴 병합 BodyText
    let hancom_doc = crate::parser::parse_hwp(&hancom_data).unwrap();
    let mut hancom_cfb = crate::parser::cfb_reader::CfbReader::open(&hancom_data).unwrap();
    let hancom_bt = hancom_cfb
        .read_body_text_section(0, hancom_doc.header.compressed, false)
        .unwrap();
    let hancom_recs = Record::read_all(&hancom_bt).unwrap();

    eprintln!(
        "원본: {} recs, 한컴 병합: {} recs",
        orig_recs.len(),
        hancom_recs.len()
    );

    // 표 범위 찾기
    let find_table = |recs: &[Record]| -> (usize, usize) {
        for (i, rec) in recs.iter().enumerate() {
            if rec.tag_id == crate::parser::tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
                let ctrl_id = u32::from_le_bytes(rec.data[0..4].try_into().unwrap());
                if ctrl_id == crate::parser::tags::CTRL_TABLE {
                    let mut end = recs.len();
                    for j in (i + 1)..recs.len() {
                        if recs[j].level <= rec.level {
                            end = j;
                            break;
                        }
                    }
                    return (i, end);
                }
            }
        }
        (0, 0)
    };

    let (ot_start, ot_end) = find_table(&orig_recs);
    let (ht_start, ht_end) = find_table(&hancom_recs);
    eprintln!(
        "원본 표: [{}..{}] ({} recs)",
        ot_start,
        ot_end,
        ot_end - ot_start
    );
    eprintln!(
        "한컴 표: [{}..{}] ({} recs)",
        ht_start,
        ht_end,
        ht_end - ht_start
    );

    // 한컴 표 레코드 전체 출력
    eprintln!("\n=== 한컴 병합 표 레코드 ===");
    for i in ht_start..ht_end {
        let r = &hancom_recs[i];
        let tag = crate::parser::tags::tag_name(r.tag_id);
        eprintln!(
            "  [{}] {} L{} {}B {:02X?}",
            i,
            tag,
            r.level,
            r.data.len(),
            &r.data[..r.data.len().min(50)]
        );
    }

    // TABLE 레코드 비교
    eprintln!("\n=== TABLE 레코드 비교 ===");
    for r in orig_recs[ot_start..ot_end].iter() {
        if r.tag_id == crate::parser::tags::HWPTAG_TABLE {
            eprintln!("  원본: {:02X?}", &r.data);
        }
    }
    for r in hancom_recs[ht_start..ht_end].iter() {
        if r.tag_id == crate::parser::tags::HWPTAG_TABLE {
            eprintln!("  한컴: {:02X?}", &r.data);
        }
    }

    // LIST_HEADER 비교 (셀 구조)
    eprintln!("\n=== 원본 LIST_HEADER (row=2 셀들) ===");
    let mut cell_idx = 0;
    for r in orig_recs[ot_start..ot_end].iter() {
        if r.tag_id == crate::parser::tags::HWPTAG_LIST_HEADER {
            let col = u16::from_le_bytes(r.data[8..10].try_into().unwrap());
            let row = u16::from_le_bytes(r.data[10..12].try_into().unwrap());
            if row == 2 {
                eprintln!(
                    "  cell[{}] col={} row={}: {:02X?}",
                    cell_idx, col, row, &r.data
                );
            }
            cell_idx += 1;
        }
    }

    eprintln!("\n=== 한컴 LIST_HEADER (row=2 셀들) ===");
    cell_idx = 0;
    for r in hancom_recs[ht_start..ht_end].iter() {
        if r.tag_id == crate::parser::tags::HWPTAG_LIST_HEADER {
            let col = u16::from_le_bytes(r.data[8..10].try_into().unwrap());
            let row = u16::from_le_bytes(r.data[10..12].try_into().unwrap());
            let col_span = u16::from_le_bytes(r.data[12..14].try_into().unwrap());
            let row_span = u16::from_le_bytes(r.data[14..16].try_into().unwrap());
            let width = u32::from_le_bytes(r.data[16..20].try_into().unwrap());
            let height = u32::from_le_bytes(r.data[20..24].try_into().unwrap());
            eprintln!(
                "  cell[{}] col={} row={} span={}x{} w={} h={}: {:02X?}",
                cell_idx, col, row, col_span, row_span, width, height, &r.data
            );
            cell_idx += 1;
        }
    }

    // 셀 개수 비교
    let orig_cells = orig_recs[ot_start..ot_end]
        .iter()
        .filter(|r| r.tag_id == crate::parser::tags::HWPTAG_LIST_HEADER)
        .count();
    let hancom_cells = hancom_recs[ht_start..ht_end]
        .iter()
        .filter(|r| r.tag_id == crate::parser::tags::HWPTAG_LIST_HEADER)
        .count();
    eprintln!("\n셀 개수: 원본={}, 한컴 병합={}", orig_cells, hancom_cells);
}

#[test]
fn test_distribution_raw_stream_preserved() {
    let path = "samples/20250130-hongbo-no.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }
    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();

    // raw_stream 확인
    let has_raw_before = doc.document().sections[0].raw_stream.is_some();
    eprintln!("raw_stream before convert: {}", has_raw_before);
    assert!(has_raw_before, "파싱 후 raw_stream 있어야 함");

    // 헤더 플래그 확인
    eprintln!("header.flags: 0x{:08X}", doc.document().header.flags);
    eprintln!(
        "header.distribution: {}",
        doc.document().header.distribution
    );

    // convert
    let result = doc.convert_to_editable_native().unwrap();
    eprintln!("convert result: {}", result);

    let has_raw_after = doc.document().sections[0].raw_stream.is_some();
    eprintln!("raw_stream after convert: {}", has_raw_after);
    assert!(has_raw_after, "convert 후에도 raw_stream 보존되어야 함");

    // export
    let bytes = doc.export_hwp_native().unwrap();
    eprintln!("export size: {} bytes", bytes.len());

    // 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&bytes).unwrap();
    assert_eq!(
        doc2.document().sections[0].paragraphs.len(),
        doc.document().sections[0].paragraphs.len()
    );
    eprintln!(
        "재파싱 문단 수 일치: {}",
        doc2.document().sections[0].paragraphs.len()
    );
}

/// 배포용 문서를 변환 후, raw_stream 없이 재직렬화하는 경로 테스트 (편집 시나리오)
#[test]
fn test_distribution_reserialization_without_raw_stream() {
    let path = "samples/20250130-hongbo-no.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }
    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();

    // 변환
    doc.convert_to_editable_native().unwrap();

    // 편집 시나리오 시뮬레이션: raw_stream 제거
    let orig_para_count = doc.document().sections[0].paragraphs.len();
    doc.document.sections[0].raw_stream = None;
    eprintln!("raw_stream 제거 후 재직렬화 테스트");

    // raw_stream 보존 경로 (기준)
    let data_with_raw = {
        let mut doc2 = HwpDocument::from_bytes(&data).unwrap();
        doc2.convert_to_editable_native().unwrap();
        doc2.export_hwp_native().unwrap()
    };

    // raw_stream 없는 경로 (편집 후)
    let data_without_raw = doc.export_hwp_native().unwrap();

    eprintln!("raw_stream 보존: {} bytes", data_with_raw.len());
    eprintln!("raw_stream 없음: {} bytes", data_without_raw.len());

    // 재직렬화된 파일 파싱 가능 여부
    let doc3 = HwpDocument::from_bytes(&data_without_raw).unwrap();
    let reserialized_para_count = doc3.document().sections[0].paragraphs.len();
    eprintln!(
        "원본 문단: {}, 재직렬화 문단: {}",
        orig_para_count, reserialized_para_count
    );

    // BodyText 레코드 수 비교
    use crate::parser::record::Record;
    let mut cfb_with = crate::parser::cfb_reader::CfbReader::open(&data_with_raw).unwrap();
    let bt_with = cfb_with.read_body_text_section(0, true, false).unwrap();
    let recs_with = Record::read_all(&bt_with).unwrap();

    let mut cfb_without = crate::parser::cfb_reader::CfbReader::open(&data_without_raw).unwrap();
    let bt_without = cfb_without.read_body_text_section(0, true, false).unwrap();
    let recs_without = Record::read_all(&bt_without).unwrap();

    eprintln!(
        "raw_stream 보존 레코드: {}, 재직렬화 레코드: {}",
        recs_with.len(),
        recs_without.len()
    );

    // 재직렬화 결과 파일을 디스크에 저장
    let out_dir = std::path::Path::new("output");
    if out_dir.exists() {
        std::fs::write(out_dir.join("hongbo_with_raw.hwp"), &data_with_raw).unwrap();
        std::fs::write(out_dir.join("hongbo_without_raw.hwp"), &data_without_raw).unwrap();
        eprintln!("저장: output/hongbo_with_raw.hwp, output/hongbo_without_raw.hwp");
    }

    // 레코드 유형별 차이 분석
    use std::collections::HashMap;
    let count_tags = |recs: &[Record]| -> HashMap<u16, usize> {
        let mut map = HashMap::new();
        for r in recs {
            *map.entry(r.tag_id).or_insert(0) += 1;
        }
        map
    };
    let tags_with = count_tags(&recs_with);
    let tags_without = count_tags(&recs_without);

    let mut all_tags: Vec<u16> = tags_with
        .keys()
        .chain(tags_without.keys())
        .copied()
        .collect();
    all_tags.sort();
    all_tags.dedup();
    for tag in &all_tags {
        let c1 = tags_with.get(tag).unwrap_or(&0);
        let c2 = tags_without.get(tag).unwrap_or(&0);
        if c1 != c2 {
            eprintln!(
                "  태그 차이: {} (0x{:04X}): raw={}, reserialized={}",
                crate::parser::tags::tag_name(*tag),
                tag,
                c1,
                c2
            );
        }
    }

    // CTRL_DATA 위치 분석
    for (idx, rec) in recs_with.iter().enumerate() {
        if rec.tag_id == crate::parser::tags::HWPTAG_CTRL_DATA {
            // 부모 CTRL_HEADER 찾기
            let mut parent_info = "?".to_string();
            for prev_idx in (0..idx).rev() {
                if recs_with[prev_idx].tag_id == crate::parser::tags::HWPTAG_CTRL_HEADER
                    && recs_with[prev_idx].level < rec.level
                {
                    let data = &recs_with[prev_idx].data;
                    if data.len() >= 4 {
                        let ctrl_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        parent_info = format!(
                            "{} (0x{:08X})",
                            crate::parser::tags::ctrl_name(ctrl_id),
                            ctrl_id
                        );
                    }
                    break;
                }
            }
            eprintln!(
                "  CTRL_DATA[{}]: level={}, size={}, parent={}",
                idx,
                rec.level,
                rec.data.len(),
                parent_info
            );
        }
    }

    // 인덱스 225~245 주변 레코드 트리 덤프 (level 6 구조 분석)
    eprintln!("\n--- 레코드 트리 (225~250) ---");
    for idx in 225..250.min(recs_with.len()) {
        let rec = &recs_with[idx];
        let indent = "  ".repeat(rec.level as usize);
        let mut extra = String::new();
        if rec.tag_id == crate::parser::tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
            let cid = u32::from_le_bytes(rec.data[0..4].try_into().unwrap());
            extra = format!(" ctrl={}", crate::parser::tags::ctrl_name(cid));
        }
        eprintln!(
            "  [{}] {}{}(lv={}, {}B){}",
            idx,
            indent,
            crate::parser::tags::tag_name(rec.tag_id),
            rec.level,
            rec.data.len(),
            extra
        );
    }

    // 문단 수가 같아야 함
    assert_eq!(
        reserialized_para_count, orig_para_count,
        "재직렬화 후 문단 수 불일치!"
    );
}


/// #1323 부수 해소: 본문 문단 시작 Backspace 병합 시 병합 대상 문단의 컨트롤이
/// 보존되어야 한다 (수정 전에는 merge_from이 controls를 드롭).
#[test]
fn test_merge_paragraph_preserves_controls() {
    use crate::model::control::Control;

    let mut doc = create_doc_with_floating_picture(true, 0, 0);
    // 문단 1에 텍스트 입력 후 문단 0(그림 문단)으로 병합
    doc.insert_text_native(0, 1, 0, "가나")
        .expect("텍스트 입력");
    doc.merge_paragraph_native(0, 1).expect("문단 병합");

    let para = &doc.document.sections[0].paragraphs[0];
    assert_eq!(para.text, "가나");
    assert_eq!(
        para.controls
            .iter()
            .filter(|c| matches!(c, Control::Picture(_)))
            .count(),
        1,
        "백스페이스 병합 시 그림 컨트롤이 보존되어야 한다 (#1323)"
    );
    assert_eq!(
        para.control_text_positions(),
        vec![0],
        "그림은 병합된 텍스트 앞 위치를 유지해야 한다"
    );
}

/// 정상 파일 vs 손상 파일: 바이너리 레코드 레벨 비교
#[test]
fn test_binary_record_comparison() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let files = [
        ("/app/pasts/20250130-hongbo-p2.hwp", "CORRECT"),
        ("/app/pasts/20250130-hongbo_saved-past-006.hwp", "OURS-006"),
    ];

    for (path, label) in &files {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("File not found: {}", path);
                continue;
            }
        };

        eprintln!("\n{}", "=".repeat(100));
        eprintln!("=== {} : {} ===", label, path);
        eprintln!("{}", "=".repeat(100));

        let mut cfb = CfbReader::open(&data).unwrap();
        let section_data = cfb.read_body_text_section(0, true, false).unwrap();
        let records = Record::read_all(&section_data).unwrap();
        eprintln!("Total records: {}", records.len());

        let ctrl_table_id = tags::CTRL_TABLE;

        for (ri, rec) in records.iter().enumerate() {
            if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
                let ctrl_id =
                    u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                if ctrl_id == ctrl_table_id {
                    eprintln!(
                        "\n--- TABLE CTRL_HEADER record #{} (level={}, size={}) ---",
                        ri,
                        rec.level,
                        rec.data.len()
                    );
                    for cs in (0..rec.data.len()).step_by(16) {
                        let ce = (cs + 16).min(rec.data.len());
                        let hex: Vec<String> = rec.data[cs..ce]
                            .iter()
                            .map(|b| format!("{:02X}", b))
                            .collect();
                        eprintln!("    [{:04X}] {}", cs, hex.join(" "));
                    }
                    if rec.data.len() >= 8 {
                        let table_attr = u32::from_le_bytes([
                            rec.data[4],
                            rec.data[5],
                            rec.data[6],
                            rec.data[7],
                        ]);
                        eprintln!("  table.attr = 0x{:08X}", table_attr);
                    }
                    let rcd = &rec.data[8..];
                    if rcd.len() >= 36 {
                        let coa_attr = u32::from_le_bytes([rcd[0], rcd[1], rcd[2], rcd[3]]);
                        let width = u32::from_le_bytes([rcd[12], rcd[13], rcd[14], rcd[15]]);
                        let height = u32::from_le_bytes([rcd[16], rcd[17], rcd[18], rcd[19]]);
                        let z_order = i32::from_le_bytes([rcd[20], rcd[21], rcd[22], rcd[23]]);
                        let margin_l = i16::from_le_bytes([rcd[24], rcd[25]]);
                        let margin_r = i16::from_le_bytes([rcd[26], rcd[27]]);
                        let margin_t = i16::from_le_bytes([rcd[28], rcd[29]]);
                        let margin_b = i16::from_le_bytes([rcd[30], rcd[31]]);
                        let instance_id = u32::from_le_bytes([rcd[32], rcd[33], rcd[34], rcd[35]]);
                        eprintln!(
                            "  CommonObjAttr: attr=0x{:08X} w={} h={} z={}",
                            coa_attr, width, height, z_order
                        );
                        eprintln!(
                            "    margins=L:{} R:{} T:{} B:{}",
                            margin_l, margin_r, margin_t, margin_b
                        );
                        eprintln!("    instance_id={} (0x{:08X})", instance_id, instance_id);
                        if rcd.len() > 36 {
                            let desc_len = u16::from_le_bytes([rcd[36], rcd[37]]);
                            eprintln!(
                                "    desc_len={}, remaining={} bytes",
                                desc_len,
                                rcd.len().saturating_sub(38)
                            );
                        }
                    }

                    let tbl_level = rec.level;
                    let mut nr = ri + 1;
                    let mut table_rec_shown = false;
                    let mut cell_count = 0;
                    while nr < records.len() && records[nr].level > tbl_level {
                        let sub = &records[nr];
                        if sub.tag_id == tags::HWPTAG_TABLE && !table_rec_shown {
                            eprintln!(
                                "\n  HWPTAG_TABLE record #{} (level={}, size={}):",
                                nr,
                                sub.level,
                                sub.data.len()
                            );
                            for cs in (0..sub.data.len()).step_by(16) {
                                let ce = (cs + 16).min(sub.data.len());
                                let hex: Vec<String> = sub.data[cs..ce]
                                    .iter()
                                    .map(|b| format!("{:02X}", b))
                                    .collect();
                                eprintln!("    [{:04X}] {}", cs, hex.join(" "));
                            }
                            if sub.data.len() >= 18 {
                                let tbl_attr = u32::from_le_bytes([
                                    sub.data[0],
                                    sub.data[1],
                                    sub.data[2],
                                    sub.data[3],
                                ]);
                                let row_cnt = u16::from_le_bytes([sub.data[4], sub.data[5]]);
                                let col_cnt = u16::from_le_bytes([sub.data[6], sub.data[7]]);
                                let pad_l = i16::from_le_bytes([sub.data[10], sub.data[11]]);
                                let pad_r = i16::from_le_bytes([sub.data[12], sub.data[13]]);
                                let pad_t = i16::from_le_bytes([sub.data[14], sub.data[15]]);
                                let pad_b = i16::from_le_bytes([sub.data[16], sub.data[17]]);
                                eprintln!(
                                    "    attr=0x{:08X} rows={} cols={}",
                                    tbl_attr, row_cnt, col_cnt
                                );
                                eprintln!(
                                    "    padding=L:{} R:{} T:{} B:{}",
                                    pad_l, pad_r, pad_t, pad_b
                                );
                                let mut off = 18usize;
                                for _ in 0..row_cnt {
                                    off += 2;
                                }
                                if off + 2 <= sub.data.len() {
                                    let bf_id =
                                        u16::from_le_bytes([sub.data[off], sub.data[off + 1]]);
                                    eprintln!("    border_fill_id={}", bf_id);
                                    off += 2;
                                }
                                if off < sub.data.len() {
                                    let extra: Vec<String> = sub.data[off..]
                                        .iter()
                                        .map(|b| format!("{:02X}", b))
                                        .collect();
                                    eprintln!("    extra={}", extra.join(" "));
                                }
                            }
                            table_rec_shown = true;
                        }
                        if sub.tag_id == tags::HWPTAG_LIST_HEADER && cell_count < 2 {
                            cell_count += 1;
                            eprintln!(
                                "\n  LIST_HEADER cell #{} record #{} (level={}, size={}):",
                                cell_count,
                                nr,
                                sub.level,
                                sub.data.len()
                            );
                            for cs in (0..sub.data.len()).step_by(16) {
                                let ce = (cs + 16).min(sub.data.len());
                                let hex: Vec<String> = sub.data[cs..ce]
                                    .iter()
                                    .map(|b| format!("{:02X}", b))
                                    .collect();
                                eprintln!("    [{:04X}] {}", cs, hex.join(" "));
                            }
                            if sub.data.len() >= 34 {
                                let n_p = u16::from_le_bytes([sub.data[0], sub.data[1]]);
                                let la = u32::from_le_bytes([
                                    sub.data[2],
                                    sub.data[3],
                                    sub.data[4],
                                    sub.data[5],
                                ]);
                                let wr = u16::from_le_bytes([sub.data[6], sub.data[7]]);
                                let col = u16::from_le_bytes([sub.data[8], sub.data[9]]);
                                let row = u16::from_le_bytes([sub.data[10], sub.data[11]]);
                                let w = u32::from_le_bytes([
                                    sub.data[16],
                                    sub.data[17],
                                    sub.data[18],
                                    sub.data[19],
                                ]);
                                let h = u32::from_le_bytes([
                                    sub.data[20],
                                    sub.data[21],
                                    sub.data[22],
                                    sub.data[23],
                                ]);
                                eprintln!(
                                    "    n_paras={} list_attr=0x{:08X} width_ref={}",
                                    n_p, la, wr
                                );
                                eprintln!("    col={} row={} w={} h={}", col, row, w, h);
                                if sub.data.len() > 34 {
                                    let extra: Vec<String> = sub.data[34..]
                                        .iter()
                                        .map(|b| format!("{:02X}", b))
                                        .collect();
                                    eprintln!(
                                        "    raw_list_extra ({} bytes) = {}",
                                        sub.data.len() - 34,
                                        extra.join(" ")
                                    );
                                }
                            }
                        } else if sub.tag_id == tags::HWPTAG_LIST_HEADER {
                            cell_count += 1;
                        }
                        if sub.tag_id == tags::HWPTAG_PARA_HEADER
                            && cell_count <= 2
                            && cell_count > 0
                        {
                            eprintln!(
                                "\n  Cell #{} PARA_HEADER record #{} (size={}):",
                                cell_count,
                                nr,
                                sub.data.len()
                            );
                            if sub.data.len() >= 22 {
                                let ccr = u32::from_le_bytes([
                                    sub.data[0],
                                    sub.data[1],
                                    sub.data[2],
                                    sub.data[3],
                                ]);
                                let cc = ccr & 0x7FFFFFFF;
                                let msb = (ccr & 0x80000000) != 0;
                                let cm = u32::from_le_bytes([
                                    sub.data[4],
                                    sub.data[5],
                                    sub.data[6],
                                    sub.data[7],
                                ]);
                                let ps = u16::from_le_bytes([sub.data[8], sub.data[9]]);
                                let inst = u32::from_le_bytes([
                                    sub.data[18],
                                    sub.data[19],
                                    sub.data[20],
                                    sub.data[21],
                                ]);
                                eprintln!(
                                    "    cc={} msb={} cm=0x{:08X} ps={} inst={}",
                                    cc, msb, cm, ps, inst
                                );
                            }
                            for cs in (0..sub.data.len()).step_by(16) {
                                let ce = (cs + 16).min(sub.data.len());
                                let hex: Vec<String> = sub.data[cs..ce]
                                    .iter()
                                    .map(|b| format!("{:02X}", b))
                                    .collect();
                                eprintln!("    [{:04X}] {}", cs, hex.join(" "));
                            }
                        }
                        nr += 1;
                    }
                    eprintln!("  Total cells: {}", cell_count);
                }
            }

            // 테이블 포함 문단의 PARA_HEADER (level 0)
            if rec.tag_id == tags::HWPTAG_PARA_HEADER && rec.level == 0 {
                let mut has_table = false;
                let mut nk = ri + 1;
                while nk < records.len() && records[nk].level > rec.level {
                    if records[nk].tag_id == tags::HWPTAG_CTRL_HEADER && records[nk].data.len() >= 4
                    {
                        let cid = u32::from_le_bytes([
                            records[nk].data[0],
                            records[nk].data[1],
                            records[nk].data[2],
                            records[nk].data[3],
                        ]);
                        if cid == ctrl_table_id {
                            has_table = true;
                            break;
                        }
                    }
                    nk += 1;
                }
                if has_table {
                    eprintln!(
                        "\n--- TABLE's PARA_HEADER record #{} (level={}, size={}) ---",
                        ri,
                        rec.level,
                        rec.data.len()
                    );
                    for cs in (0..rec.data.len()).step_by(16) {
                        let ce = (cs + 16).min(rec.data.len());
                        let hex: Vec<String> = rec.data[cs..ce]
                            .iter()
                            .map(|b| format!("{:02X}", b))
                            .collect();
                        eprintln!("    [{:04X}] {}", cs, hex.join(" "));
                    }
                    if rec.data.len() >= 22 {
                        let ccr = u32::from_le_bytes([
                            rec.data[0],
                            rec.data[1],
                            rec.data[2],
                            rec.data[3],
                        ]);
                        let cc = ccr & 0x7FFFFFFF;
                        let msb = (ccr & 0x80000000) != 0;
                        let cm = u32::from_le_bytes([
                            rec.data[4],
                            rec.data[5],
                            rec.data[6],
                            rec.data[7],
                        ]);
                        let ps = u16::from_le_bytes([rec.data[8], rec.data[9]]);
                        let inst = u32::from_le_bytes([
                            rec.data[18],
                            rec.data[19],
                            rec.data[20],
                            rec.data[21],
                        ]);
                        eprintln!(
                            "  cc={} msb={} cm=0x{:08X} ps_id={} inst={}",
                            cc, msb, cm, ps, inst
                        );
                    }
                }
            }
        }

        // 레코드 시퀀스 요약 (level 0,1)
        eprintln!("\n--- RECORD SEQUENCE (level 0-1) ---");
        for (ri, rec) in records.iter().enumerate() {
            if rec.level <= 1 {
                let extra = if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
                    let cid =
                        u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                    format!(" ctrl_id=0x{:08X}({})", cid, tags::ctrl_name(cid))
                } else {
                    String::new()
                };
                eprintln!(
                    "  #{:4}: L{} {} size={}{}",
                    ri,
                    rec.level,
                    rec.tag_name(),
                    rec.data.len(),
                    extra
                );
            }
        }
    }

    eprintln!("\n=== BINARY RECORD COMPARISON COMPLETE ===");
}

/// rp-006 BodyText 레코드 분석: dangling CharShape/ParaShape 참조 검출
#[test]
fn test_rp006_dangling_references() {
    use crate::parser::cfb_reader::{decompress_stream, CfbReader};
    use crate::parser::record::Record;
    use crate::parser::tags;

    let saved_path = "pasts/20250130-hongbo_saved-rp-006.hwp";
    if !std::path::Path::new(saved_path).exists() {
        eprintln!("SKIP: rp-006 파일 없음");
        return;
    }

    let saved_bytes = std::fs::read(saved_path).unwrap();
    let mut cfb = CfbReader::open(&saved_bytes).expect("CFB 열기 실패");

    // DocInfo: CharShape/ParaShape 총 수
    let doc_info_data = cfb.read_doc_info(true).expect("DocInfo 읽기 실패");
    let doc_recs = Record::read_all(&doc_info_data).unwrap();

    let cs_count = doc_recs
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_CHAR_SHAPE)
        .count();
    let ps_count = doc_recs
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_PARA_SHAPE)
        .count();
    let bf_count = doc_recs
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_BORDER_FILL)
        .count();
    eprintln!(
        "\n=== rp-006 DocInfo: CharShape={}, ParaShape={}, BorderFill={} ===",
        cs_count, ps_count, bf_count
    );

    // BodyText Section0
    let body_data = cfb
        .read_body_text_section(0, true, false)
        .expect("BodyText 읽기 실패");
    let body_recs = Record::read_all(&body_data).unwrap();
    eprintln!("BodyText 레코드 총 수: {}", body_recs.len());

    // 모든 PARA_HEADER에서 para_shape_id 추출
    let mut dangling_ps = Vec::new();
    let mut dangling_cs = Vec::new();
    let mut dangling_bf = Vec::new();
    let mut para_idx = 0;

    for (ri, rec) in body_recs.iter().enumerate() {
        if rec.tag_id == tags::HWPTAG_PARA_HEADER && rec.data.len() >= 10 {
            let ps_id = u16::from_le_bytes([rec.data[8], rec.data[9]]) as usize;
            if ps_id >= ps_count {
                dangling_ps.push((para_idx, ri, ps_id));
            }
            para_idx += 1;
        }
        // PARA_CHAR_SHAPE: 각 4바이트 쌍 (start_pos u32 + char_shape_id u32)
        if rec.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
            let mut pos = 0;
            while pos + 8 <= rec.data.len() {
                let cs_id = u32::from_le_bytes([
                    rec.data[pos + 4],
                    rec.data[pos + 5],
                    rec.data[pos + 6],
                    rec.data[pos + 7],
                ]) as usize;
                if cs_id >= cs_count {
                    dangling_cs.push((ri, pos / 8, cs_id));
                }
                pos += 8;
            }
        }
        // LIST_HEADER의 border_fill_id (셀) 및 TABLE의 border_fill_id
        if rec.tag_id == tags::HWPTAG_LIST_HEADER && rec.data.len() >= 34 {
            let bf_id = u16::from_le_bytes([rec.data[32], rec.data[33]]) as usize;
            if bf_id > 0 && bf_id > bf_count {
                dangling_bf.push((ri, "LIST_HEADER", bf_id));
            }
        }
    }

    eprintln!("\n--- Dangling ParaShape References ---");
    if dangling_ps.is_empty() {
        eprintln!("  None found (OK)");
    } else {
        for (pi, ri, ps_id) in &dangling_ps {
            eprintln!(
                "  para[{}] rec[{}]: para_shape_id={} >= max {}",
                pi, ri, ps_id, ps_count
            );
        }
    }

    eprintln!("\n--- Dangling CharShape References ---");
    if dangling_cs.is_empty() {
        eprintln!("  None found (OK)");
    } else {
        for (ri, entry, cs_id) in &dangling_cs {
            eprintln!(
                "  rec[{}] entry[{}]: char_shape_id={} >= max {}",
                ri, entry, cs_id, cs_count
            );
        }
    }

    eprintln!("\n--- Dangling BorderFill References ---");
    if dangling_bf.is_empty() {
        eprintln!("  None found (OK)");
    } else {
        for (ri, source, bf_id) in &dangling_bf {
            eprintln!(
                "  rec[{}] {}: border_fill_id={} >= max {}",
                ri, source, bf_id, bf_count
            );
        }
    }

    // 마지막 TABLE + 셀들의 레코드 덤프 (붙여넣기된 표 추정)
    eprintln!("\n--- Last 100 records (pasted table area) ---");
    let start = if body_recs.len() > 100 {
        body_recs.len() - 100
    } else {
        0
    };
    for (i, rec) in body_recs[start..].iter().enumerate() {
        let indent = "  ".repeat(rec.level as usize);
        let tag_name = tags::tag_name(rec.tag_id);
        let extra = if rec.tag_id == tags::HWPTAG_PARA_HEADER && rec.data.len() >= 12 {
            let cc = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
            let cm = u32::from_le_bytes([rec.data[4], rec.data[5], rec.data[6], rec.data[7]]);
            let ps_id = u16::from_le_bytes([rec.data[8], rec.data[9]]);
            let style_id = rec.data[10];
            let char_count = cc & 0x7FFFFFFF;
            let msb = cc >> 31;
            format!(
                " char_count={} msb={} ctrl_mask=0x{:08X} ps_id={} style_id={}",
                char_count, msb, cm, ps_id, style_id
            )
        } else if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
            let rev_bytes: Vec<u8> = rec.data[0..4].iter().rev().cloned().collect();
            let ctrl_str = String::from_utf8_lossy(&rev_bytes);
            format!(" ctrl_id=0x{:08X}('{}')", ctrl_id, ctrl_str)
        } else if rec.tag_id == tags::HWPTAG_TABLE && rec.data.len() >= 8 {
            let nrows = u16::from_le_bytes([rec.data[4], rec.data[5]]);
            let ncols = u16::from_le_bytes([rec.data[6], rec.data[7]]);
            format!(" rows={} cols={}", nrows, ncols)
        } else if rec.tag_id == tags::HWPTAG_LIST_HEADER && rec.data.len() >= 8 {
            let nparas = u16::from_le_bytes([rec.data[0], rec.data[1]]);
            format!(" n_paras={}", nparas)
        } else if rec.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
            let n_entries = rec.data.len() / 8;
            let mut ids: Vec<u32> = Vec::new();
            let mut pos = 0;
            while pos + 8 <= rec.data.len() {
                ids.push(u32::from_le_bytes([
                    rec.data[pos + 4],
                    rec.data[pos + 5],
                    rec.data[pos + 6],
                    rec.data[pos + 7],
                ]));
                pos += 8;
            }
            format!(" entries={} cs_ids={:?}", n_entries, ids)
        } else {
            String::new()
        };
        eprintln!(
            "  rec[{}] {}L{} {} ({}B){}",
            start + i,
            indent,
            rec.level,
            tag_name,
            rec.data.len(),
            extra
        );
    }

    // Summary assertion
    let total_dangling = dangling_cs.len() + dangling_ps.len();
    if total_dangling > 0 {
        eprintln!("\n*** FOUND {} DANGLING REFERENCES ***", total_dangling);
    }
}

/// template 파일 비교: step1 (원본 2x2표) vs step1-p (HWP 붙여넣기) vs step1_saved (우리 뷰어 붙여넣기)
#[test]
fn test_template_comparison() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let files = [
        (
            "step1_saved (뷰어 저장/손상)",
            "template/empty-step1_saved.hwp",
        ),
        (
            "step1_saved-a (HWP 다른이름저장/정상)",
            "template/empty-step1_saved-a.hwp",
        ),
    ];

    for (label, path) in &files {
        if !std::path::Path::new(path).exists() {
            eprintln!("SKIP: {} 파일 없음", path);
            continue;
        }

        let bytes = std::fs::read(path).unwrap();
        let mut cfb = CfbReader::open(&bytes).unwrap_or_else(|_| panic!("{} CFB 열기 실패", label));

        // DocInfo 분석
        let doc_info_data = cfb.read_doc_info(true).expect("DocInfo 읽기 실패");
        let doc_recs = Record::read_all(&doc_info_data).unwrap();

        let cs_count = doc_recs
            .iter()
            .filter(|r| r.tag_id == tags::HWPTAG_CHAR_SHAPE)
            .count();
        let ps_count = doc_recs
            .iter()
            .filter(|r| r.tag_id == tags::HWPTAG_PARA_SHAPE)
            .count();
        let bf_count = doc_recs
            .iter()
            .filter(|r| r.tag_id == tags::HWPTAG_BORDER_FILL)
            .count();
        let style_count = doc_recs
            .iter()
            .filter(|r| r.tag_id == tags::HWPTAG_STYLE)
            .count();

        eprintln!("\n{}", "=".repeat(80));
        eprintln!("  {} ({} bytes)", label, bytes.len());
        eprintln!(
            "  DocInfo: CS={} PS={} BF={} Style={}",
            cs_count, ps_count, bf_count, style_count
        );

        // BodyText Section0 분석
        let body_data = cfb
            .read_body_text_section(0, true, false)
            .expect("BodyText 읽기 실패");
        let body_recs = Record::read_all(&body_data).unwrap();

        eprintln!(
            "  BodyText: {} records, {} bytes",
            body_recs.len(),
            body_data.len()
        );

        // 전체 레코드 덤프
        eprintln!("\n  --- ALL RECORDS ---");
        for (i, rec) in body_recs.iter().enumerate() {
            let indent = "  ".repeat(rec.level as usize);
            let tag_name = tags::tag_name(rec.tag_id);
            let extra = if rec.tag_id == tags::HWPTAG_PARA_HEADER && rec.data.len() >= 12 {
                let cc = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                let cm = u32::from_le_bytes([rec.data[4], rec.data[5], rec.data[6], rec.data[7]]);
                let ps_id = u16::from_le_bytes([rec.data[8], rec.data[9]]);
                let style = rec.data[10];
                let char_count = cc & 0x7FFFFFFF;
                let msb = cc >> 31;
                format!(
                    " cc={} msb={} cm=0x{:08X} ps={} st={}",
                    char_count, msb, cm, ps_id, style
                )
            } else if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
                let ctrl_id =
                    u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                let rev: Vec<u8> = rec.data[0..4].iter().rev().cloned().collect();
                let ctrl_str = String::from_utf8_lossy(&rev);
                if rec.data.len() >= 8 {
                    let attr =
                        u32::from_le_bytes([rec.data[4], rec.data[5], rec.data[6], rec.data[7]]);
                    format!(" '{}' attr=0x{:08X}", ctrl_str, attr)
                } else {
                    format!(" '{}'", ctrl_str)
                }
            } else if rec.tag_id == tags::HWPTAG_TABLE && rec.data.len() >= 8 {
                let attr = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                let nrows = u16::from_le_bytes([rec.data[4], rec.data[5]]);
                let ncols = u16::from_le_bytes([rec.data[6], rec.data[7]]);
                format!(" attr=0x{:08X} {}x{}", attr, nrows, ncols)
            } else if rec.tag_id == tags::HWPTAG_LIST_HEADER && rec.data.len() >= 2 {
                let nparas = u16::from_le_bytes([rec.data[0], rec.data[1]]);
                format!(" nparas={}", nparas)
            } else if rec.tag_id == tags::HWPTAG_PARA_TEXT {
                // 첫 20바이트 hex 덤프
                let hex: String = rec
                    .data
                    .iter()
                    .take(20)
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(" [{}{}]", hex, if rec.data.len() > 20 { "..." } else { "" })
            } else if rec.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
                let mut ids = Vec::new();
                let mut pos = 0;
                while pos + 8 <= rec.data.len() {
                    let cs_id = u32::from_le_bytes([
                        rec.data[pos + 4],
                        rec.data[pos + 5],
                        rec.data[pos + 6],
                        rec.data[pos + 7],
                    ]);
                    ids.push(cs_id);
                    pos += 8;
                }
                format!(" cs_ids={:?}", ids)
            } else {
                String::new()
            };
            eprintln!(
                "  rec[{:3}] {}L{} {} ({}B){}",
                i,
                indent,
                rec.level,
                tag_name,
                rec.data.len(),
                extra
            );
        }

        // CTRL_HEADER 바이트 덤프 (tbl 컨트롤만)
        eprintln!("\n  --- TABLE CTRL_HEADER hex dump ---");
        for (i, rec) in body_recs.iter().enumerate() {
            if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
                let ctrl_id =
                    u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                if ctrl_id == 0x6C626174 {
                    // 'tbl '
                    let hex: String = rec
                        .data
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    eprintln!(
                        "  rec[{:3}] CTRL_HEADER(tbl) {}B: {}",
                        i,
                        rec.data.len(),
                        hex
                    );
                }
            }
            if rec.tag_id == tags::HWPTAG_TABLE {
                let hex: String = rec
                    .data
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                eprintln!("  rec[{:3}] TABLE {}B: {}", i, rec.data.len(), hex);
            }
            if rec.tag_id == tags::HWPTAG_LIST_HEADER {
                let hex: String = rec
                    .data
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                eprintln!("  rec[{:3}] LIST_HEADER {}B: {}", i, rec.data.len(), hex);
            }
        }

        eprintln!("\n{}", "=".repeat(80));
    }
}

/// 손상 HWP vs 정상 HWP 종합 비교 (DocInfo ID_MAPPINGS + BodyText Section 0 전체 레코드)
#[test]
fn test_complex_comparison() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let damaged_path = "template/20250130-hongbo_saved_err.hwp";
    let fixed_path = "template/111111.hwp";

    if !std::path::Path::new(damaged_path).exists() {
        eprintln!("SKIP: {} not found", damaged_path);
        return;
    }
    if !std::path::Path::new(fixed_path).exists() {
        eprintln!("SKIP: {} not found", fixed_path);
        return;
    }

    let damaged_bytes = std::fs::read(damaged_path).unwrap();
    let fixed_bytes = std::fs::read(fixed_path).unwrap();

    let mut damaged_cfb = CfbReader::open(&damaged_bytes).expect("damaged CFB open failed");
    let mut fixed_cfb = CfbReader::open(&fixed_bytes).expect("fixed CFB open failed");

    eprintln!("\n{}", "=".repeat(90));
    eprintln!("  COMPLEX COMPARISON: Damaged vs Fixed HWP");
    eprintln!(
        "  Damaged: {} ({} bytes)",
        damaged_path,
        damaged_bytes.len()
    );
    eprintln!("  Fixed:   {} ({} bytes)", fixed_path, fixed_bytes.len());
    eprintln!("{}", "=".repeat(90));

    // =====================================================================
    // Part 1: DocInfo - ID_MAPPINGS counts comparison
    // =====================================================================
    eprintln!("\n{}", "=".repeat(90));
    eprintln!("  PART 1: DocInfo ID_MAPPINGS Comparison");
    eprintln!("{}", "=".repeat(90));

    let damaged_di = damaged_cfb
        .read_doc_info(true)
        .expect("damaged DocInfo read failed");
    let fixed_di = fixed_cfb
        .read_doc_info(true)
        .expect("fixed DocInfo read failed");

    let damaged_di_recs = Record::read_all(&damaged_di).unwrap();
    let fixed_di_recs = Record::read_all(&fixed_di).unwrap();

    eprintln!(
        "  Damaged DocInfo: {} records, {} bytes",
        damaged_di_recs.len(),
        damaged_di.len()
    );
    eprintln!(
        "  Fixed   DocInfo: {} records, {} bytes",
        fixed_di_recs.len(),
        fixed_di.len()
    );

    // Count records by tag type
    let tag_types_of_interest: Vec<(u16, &str)> = vec![
        (tags::HWPTAG_BIN_DATA, "BinData"),
        (tags::HWPTAG_FACE_NAME, "FaceName"),
        (tags::HWPTAG_BORDER_FILL, "BorderFill"),
        (tags::HWPTAG_CHAR_SHAPE, "CharShape"),
        (tags::HWPTAG_TAB_DEF, "TabDef"),
        (tags::HWPTAG_NUMBERING, "Numbering"),
        (tags::HWPTAG_BULLET, "Bullet"),
        (tags::HWPTAG_PARA_SHAPE, "ParaShape"),
        (tags::HWPTAG_STYLE, "Style"),
    ];

    eprintln!(
        "\n  {:<20} {:>10} {:>10} {:>10}",
        "Record Type", "Damaged", "Fixed", "Diff"
    );
    eprintln!("  {}", "-".repeat(55));
    let mut docinfo_diff_count = 0;
    for (tag_id, name) in &tag_types_of_interest {
        let d_cnt = damaged_di_recs
            .iter()
            .filter(|r| r.tag_id == *tag_id)
            .count();
        let f_cnt = fixed_di_recs.iter().filter(|r| r.tag_id == *tag_id).count();
        let diff = f_cnt as i64 - d_cnt as i64;
        let marker = if diff != 0 { " <== DIFF" } else { "" };
        if diff != 0 {
            docinfo_diff_count += 1;
        }
        eprintln!(
            "  {:<20} {:>10} {:>10} {:>+10}{}",
            name, d_cnt, f_cnt, diff, marker
        );
    }

    // ID_MAPPINGS record comparison
    let id_mappings_field_names = [
        "BinData",
        "Font_Korean",
        "Font_English",
        "Font_Hanja",
        "Font_Japanese",
        "Font_Other",
        "Font_Symbol",
        "Font_User",
        "BorderFill",
        "CharShape",
        "TabDef",
        "Numbering",
        "Bullet",
        "ParaShape",
        "Style",
        "MemoShape",
        "Field16",
        "Field17",
        "Field18",
        "Field19",
    ];

    let damaged_idm = damaged_di_recs
        .iter()
        .find(|r| r.tag_id == tags::HWPTAG_ID_MAPPINGS);
    let fixed_idm = fixed_di_recs
        .iter()
        .find(|r| r.tag_id == tags::HWPTAG_ID_MAPPINGS);

    if let (Some(d_rec), Some(f_rec)) = (damaged_idm, fixed_idm) {
        eprintln!(
            "\n  ID_MAPPINGS record: damaged={}B, fixed={}B",
            d_rec.data.len(),
            f_rec.data.len()
        );
        let max_fields = (d_rec.data.len().max(f_rec.data.len())) / 4;
        let max_fields = max_fields.min(20);

        eprintln!(
            "  {:<5} {:<20} {:>10} {:>10} {:>10}",
            "Idx", "Field", "Damaged", "Fixed", "Diff"
        );
        eprintln!("  {}", "-".repeat(60));

        for i in 0..max_fields {
            let d_val = if i * 4 + 4 <= d_rec.data.len() {
                u32::from_le_bytes([
                    d_rec.data[i * 4],
                    d_rec.data[i * 4 + 1],
                    d_rec.data[i * 4 + 2],
                    d_rec.data[i * 4 + 3],
                ])
            } else {
                0
            };
            let f_val = if i * 4 + 4 <= f_rec.data.len() {
                u32::from_le_bytes([
                    f_rec.data[i * 4],
                    f_rec.data[i * 4 + 1],
                    f_rec.data[i * 4 + 2],
                    f_rec.data[i * 4 + 3],
                ])
            } else {
                0
            };
            let diff = f_val as i64 - d_val as i64;
            let name = if i < id_mappings_field_names.len() {
                id_mappings_field_names[i]
            } else {
                "???"
            };
            let marker = if diff != 0 { " <== DIFF" } else { "" };
            if diff != 0 {
                docinfo_diff_count += 1;
            }
            eprintln!(
                "  [{:>2}] {:<20} {:>10} {:>10} {:>+10}{}",
                i, name, d_val, f_val, diff, marker
            );
        }
    } else {
        eprintln!("  ERROR: ID_MAPPINGS record not found in one or both files!");
    }

    // =====================================================================
    // Part 2: BodyText Section 0 - every record comparison
    // =====================================================================
    eprintln!("\n{}", "=".repeat(90));
    eprintln!("  PART 2: BodyText Section 0 - Record-by-Record Comparison");
    eprintln!("{}", "=".repeat(90));

    let damaged_bt = damaged_cfb
        .read_body_text_section(0, true, false)
        .expect("damaged BodyText read failed");
    let fixed_bt = fixed_cfb
        .read_body_text_section(0, true, false)
        .expect("fixed BodyText read failed");

    let damaged_bt_recs = Record::read_all(&damaged_bt).unwrap();
    let fixed_bt_recs = Record::read_all(&fixed_bt).unwrap();

    eprintln!(
        "  Damaged BodyText: {} records, {} bytes",
        damaged_bt_recs.len(),
        damaged_bt.len()
    );
    eprintln!(
        "  Fixed   BodyText: {} records, {} bytes",
        fixed_bt_recs.len(),
        fixed_bt.len()
    );

    let max_recs = damaged_bt_recs.len().max(fixed_bt_recs.len());
    let mut body_diff_count = 0;

    eprintln!("\n  --- Record-by-record comparison ---");
    eprintln!(
        "  {:<6} {:<25} {:<6} {:<8} | {:<25} {:<6} {:<8} | Differences",
        "Idx", "Damaged Tag", "Lvl", "Size", "Fixed Tag", "Lvl", "Size"
    );
    eprintln!("  {}", "-".repeat(120));

    for i in 0..max_recs {
        let d_rec = damaged_bt_recs.get(i);
        let f_rec = fixed_bt_recs.get(i);

        match (d_rec, f_rec) {
            (Some(d), Some(f)) => {
                let d_tag_name = tags::tag_name(d.tag_id);
                let f_tag_name = tags::tag_name(f.tag_id);
                let mut diffs: Vec<String> = Vec::new();

                if d.tag_id != f.tag_id {
                    diffs.push(format!("tag: {}!={}", d.tag_id, f.tag_id));
                }
                if d.level != f.level {
                    diffs.push(format!("level: {}!={}", d.level, f.level));
                }
                if d.data.len() != f.data.len() {
                    diffs.push(format!("size: {}!={}", d.data.len(), f.data.len()));
                }
                if d.data != f.data {
                    diffs.push("bytes differ".to_string());
                }

                // PARA_HEADER detailed comparison
                if d.tag_id == tags::HWPTAG_PARA_HEADER && f.tag_id == tags::HWPTAG_PARA_HEADER {
                    if d.data.len() >= 11 && f.data.len() >= 11 {
                        let d_cc_raw =
                            u32::from_le_bytes([d.data[0], d.data[1], d.data[2], d.data[3]]);
                        let f_cc_raw =
                            u32::from_le_bytes([f.data[0], f.data[1], f.data[2], f.data[3]]);
                        let d_char_count = d_cc_raw & 0x7FFFFFFF;
                        let f_char_count = f_cc_raw & 0x7FFFFFFF;
                        let d_msb = d_cc_raw >> 31;
                        let f_msb = f_cc_raw >> 31;
                        let d_cm = u32::from_le_bytes([d.data[4], d.data[5], d.data[6], d.data[7]]);
                        let f_cm = u32::from_le_bytes([f.data[4], f.data[5], f.data[6], f.data[7]]);
                        let d_ps_id = u16::from_le_bytes([d.data[8], d.data[9]]);
                        let f_ps_id = u16::from_le_bytes([f.data[8], f.data[9]]);
                        let d_style = d.data[10];
                        let f_style = f.data[10];

                        if d_char_count != f_char_count {
                            diffs.push(format!("char_count: {}!={}", d_char_count, f_char_count));
                        }
                        if d_msb != f_msb {
                            diffs.push(format!("msb: {}!={}", d_msb, f_msb));
                        }
                        if d_cm != f_cm {
                            diffs.push(format!("ctrl_mask: 0x{:08X}!=0x{:08X}", d_cm, f_cm));
                        }
                        if d_ps_id != f_ps_id {
                            diffs.push(format!("para_shape_id: {}!={}", d_ps_id, f_ps_id));
                        }
                        if d_style != f_style {
                            diffs.push(format!("style_id: {}!={}", d_style, f_style));
                        }
                    }
                }

                let diff_str = if diffs.is_empty() {
                    "OK".to_string()
                } else {
                    body_diff_count += 1;
                    format!("DIFF: {}", diffs.join(", "))
                };

                // Always print if there is a difference; for matching records print a compact line
                if !diffs.is_empty() {
                    eprintln!(
                        "  [{:>4}] {:<25} L{:<4} {:>6}B | {:<25} L{:<4} {:>6}B | {}",
                        i,
                        d_tag_name,
                        d.level,
                        d.data.len(),
                        f_tag_name,
                        f.level,
                        f.data.len(),
                        diff_str
                    );
                } else {
                    eprintln!(
                        "  [{:>4}] {:<25} L{:<4} {:>6}B | {:<25} L{:<4} {:>6}B | OK",
                        i,
                        d_tag_name,
                        d.level,
                        d.data.len(),
                        f_tag_name,
                        f.level,
                        f.data.len()
                    );
                }
            }
            (Some(d), None) => {
                body_diff_count += 1;
                let d_tag_name = tags::tag_name(d.tag_id);
                eprintln!(
                    "  [{:>4}] {:<25} L{:<4} {:>6}B | {:<25}                    | ONLY IN DAMAGED",
                    i,
                    d_tag_name,
                    d.level,
                    d.data.len(),
                    "---"
                );
            }
            (None, Some(f)) => {
                body_diff_count += 1;
                let f_tag_name = tags::tag_name(f.tag_id);
                eprintln!(
                    "  [{:>4}] {:<25}                    | {:<25} L{:<4} {:>6}B | ONLY IN FIXED",
                    i,
                    "---",
                    f_tag_name,
                    f.level,
                    f.data.len()
                );
            }
            (None, None) => {}
        }
    }

    // =====================================================================
    // Part 3: TABLE/CTRL_HEADER raw bytes comparison
    // =====================================================================
    eprintln!("\n{}", "=".repeat(90));
    eprintln!("  PART 3: TABLE / CTRL_HEADER Raw Bytes Comparison");
    eprintln!("{}", "=".repeat(90));

    // Collect TABLE and CTRL_HEADER records from both files
    let interesting_tags = [tags::HWPTAG_TABLE, tags::HWPTAG_CTRL_HEADER];

    let damaged_interesting: Vec<(usize, &Record)> = damaged_bt_recs
        .iter()
        .enumerate()
        .filter(|(_, r)| interesting_tags.contains(&r.tag_id))
        .collect();
    let fixed_interesting: Vec<(usize, &Record)> = fixed_bt_recs
        .iter()
        .enumerate()
        .filter(|(_, r)| interesting_tags.contains(&r.tag_id))
        .collect();

    // Match records by index position in the record stream
    let max_interesting = damaged_interesting.len().max(fixed_interesting.len());

    for j in 0..max_interesting {
        let d_item = damaged_interesting.get(j);
        let f_item = fixed_interesting.get(j);

        match (d_item, f_item) {
            (Some(&(d_idx, d_rec)), Some(&(f_idx, f_rec))) => {
                let d_tag_name = tags::tag_name(d_rec.tag_id);
                let f_tag_name = tags::tag_name(f_rec.tag_id);

                // For CTRL_HEADER, show the ctrl type string
                let d_ctrl_type =
                    if d_rec.tag_id == tags::HWPTAG_CTRL_HEADER && d_rec.data.len() >= 4 {
                        let rev: Vec<u8> = d_rec.data[0..4].iter().rev().cloned().collect();
                        format!(" '{}'", String::from_utf8_lossy(&rev))
                    } else {
                        String::new()
                    };
                let f_ctrl_type =
                    if f_rec.tag_id == tags::HWPTAG_CTRL_HEADER && f_rec.data.len() >= 4 {
                        let rev: Vec<u8> = f_rec.data[0..4].iter().rev().cloned().collect();
                        format!(" '{}'", String::from_utf8_lossy(&rev))
                    } else {
                        String::new()
                    };

                let same = d_rec.data == f_rec.data;
                eprintln!(
                    "\n  Pair {}: damaged[{}] {}{} ({}B) vs fixed[{}] {}{} ({}B) => {}",
                    j,
                    d_idx,
                    d_tag_name,
                    d_ctrl_type,
                    d_rec.data.len(),
                    f_idx,
                    f_tag_name,
                    f_ctrl_type,
                    f_rec.data.len(),
                    if same { "IDENTICAL" } else { "DIFFERENT" }
                );

                if !same {
                    body_diff_count += 1;
                    // Show byte-level diff
                    let max_len = d_rec.data.len().max(f_rec.data.len());
                    let mut diff_positions: Vec<usize> = Vec::new();
                    for pos in 0..max_len {
                        let d_byte = d_rec.data.get(pos);
                        let f_byte = f_rec.data.get(pos);
                        if d_byte != f_byte {
                            diff_positions.push(pos);
                        }
                    }
                    eprintln!(
                        "    {} byte(s) differ at positions: {:?}",
                        diff_positions.len(),
                        if diff_positions.len() <= 30 {
                            &diff_positions[..]
                        } else {
                            &diff_positions[..30]
                        }
                    );

                    // Hex dump of first 80 bytes for both
                    let dump_len = 80.min(max_len);
                    let d_hex: String = d_rec
                        .data
                        .iter()
                        .take(dump_len)
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let f_hex: String = f_rec
                        .data
                        .iter()
                        .take(dump_len)
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    eprintln!(
                        "    Damaged (first {}B): {}{}",
                        dump_len,
                        d_hex,
                        if d_rec.data.len() > dump_len {
                            "..."
                        } else {
                            ""
                        }
                    );
                    eprintln!(
                        "    Fixed   (first {}B): {}{}",
                        dump_len,
                        f_hex,
                        if f_rec.data.len() > dump_len {
                            "..."
                        } else {
                            ""
                        }
                    );
                }
            }
            (Some(&(d_idx, d_rec)), None) => {
                let d_tag_name = tags::tag_name(d_rec.tag_id);
                eprintln!(
                    "\n  Pair {}: damaged[{}] {} ({}B) -- NO MATCH IN FIXED",
                    j,
                    d_idx,
                    d_tag_name,
                    d_rec.data.len()
                );
            }
            (None, Some(&(f_idx, f_rec))) => {
                let f_tag_name = tags::tag_name(f_rec.tag_id);
                eprintln!(
                    "\n  Pair {}: -- NO MATCH IN DAMAGED -- fixed[{}] {} ({}B)",
                    j,
                    f_idx,
                    f_tag_name,
                    f_rec.data.len()
                );
            }
            _ => {}
        }
    }

    // =====================================================================
    // Summary
    // =====================================================================
    eprintln!("\n{}", "=".repeat(90));
    eprintln!("  SUMMARY");
    eprintln!("  DocInfo differences:  {}", docinfo_diff_count);
    eprintln!("  BodyText differences: {}", body_diff_count);
    eprintln!(
        "  Total records: damaged={}, fixed={}",
        damaged_bt_recs.len(),
        fixed_bt_recs.len()
    );
    eprintln!("{}", "=".repeat(90));
}

/// 3개 HWP 파일 비교: empty-step2 원본, HWP 프로그램 붙여넣기, 뷰어 붙여넣기(손상)
///
/// DocInfo ID_MAPPINGS, BodyText 레코드 전체 덤프, 레코드별 차이 비교
#[test]
fn test_step2_comparison() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    // ============================================================
    // 파일 로드
    // ============================================================
    let files: Vec<(&str, &str)> = vec![
        ("template/empty-step2.hwp", "ORIGINAL"),
        ("template/empty-step2-p.hwp", "HWP_PASTE (VALID)"),
        (
            "template/empty-step2_saved_err.hwp",
            "VIEWER_PASTE (DAMAGED)",
        ),
    ];

    struct FileData {
        label: String,
        path: String,
        doc_info_records: Vec<Record>,
        body_records: Vec<Record>,
        body_raw_len: usize,
    }

    let mut all_files: Vec<FileData> = Vec::new();

    for (path, label) in &files {
        let bytes =
            std::fs::read(path).unwrap_or_else(|e| panic!("파일 읽기 실패: {} - {}", path, e));
        eprintln!(
            "\n=== Loading {} ({}) - {} bytes ===",
            label,
            path,
            bytes.len()
        );

        let mut cfb =
            CfbReader::open(&bytes).unwrap_or_else(|e| panic!("CFB 열기 실패: {} - {}", path, e));

        // DocInfo (compressed=true)
        let doc_info_data = cfb
            .read_doc_info(true)
            .unwrap_or_else(|e| panic!("DocInfo 읽기 실패: {} - {}", path, e));
        let doc_info_records = Record::read_all(&doc_info_data)
            .unwrap_or_else(|e| panic!("DocInfo 레코드 파싱 실패: {} - {}", path, e));

        // BodyText Section 0 (compressed=true, distribution=false)
        let body_data = cfb
            .read_body_text_section(0, true, false)
            .unwrap_or_else(|e| panic!("BodyText 읽기 실패: {} - {}", path, e));
        let body_raw_len = body_data.len();
        let body_records = Record::read_all(&body_data)
            .unwrap_or_else(|e| panic!("BodyText 레코드 파싱 실패: {} - {}", path, e));

        all_files.push(FileData {
            label: label.to_string(),
            path: path.to_string(),
            doc_info_records,
            body_records,
            body_raw_len,
        });
    }

    // ============================================================
    // Helper functions
    // ============================================================

    fn read_u32_le(data: &[u8], offset: usize) -> u32 {
        if offset + 4 <= data.len() {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        } else {
            0
        }
    }

    fn read_u16_le(data: &[u8], offset: usize) -> u16 {
        if offset + 2 <= data.len() {
            u16::from_le_bytes([data[offset], data[offset + 1]])
        } else {
            0
        }
    }

    fn ctrl_id_string(data: &[u8]) -> String {
        if data.len() >= 4 {
            // ctrl_id stored as LE u32, but represents big-endian character ordering
            let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            let be_bytes = id.to_be_bytes();
            let ascii: String = be_bytes
                .iter()
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            format!("\"{}\" (0x{:08X})", ascii, id)
        } else {
            format!("(data too short: {} bytes)", data.len())
        }
    }

    fn hex_preview(data: &[u8], max: usize) -> String {
        let show = std::cmp::min(data.len(), max);
        let hex: Vec<String> = data[..show].iter().map(|b| format!("{:02x}", b)).collect();
        let mut s = hex.join(" ");
        if data.len() > max {
            s.push_str(&format!(" ...({} more)", data.len() - max));
        }
        s
    }

    // ============================================================
    // 1. DocInfo ID_MAPPINGS summary for each file
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("  PART 1: DocInfo ID_MAPPINGS Summary");
    eprintln!("{}", "=".repeat(120));

    let id_map_field_names = [
        "bin_data_count",    // 0
        "font_han",          // 1
        "font_eng",          // 2
        "font_hanja",        // 3
        "font_jpn",          // 4
        "font_other",        // 5
        "font_symbol",       // 6
        "font_user",         // 7
        "border_fill_count", // 8
        "char_shape_count",  // 9
        "tab_def_count",     // 10
        "numbering_count",   // 11
        "bullet_count",      // 12
        "para_shape_count",  // 13
        "style_count",       // 14
        "memo_shape_count",  // 15
    ];

    for fd in &all_files {
        eprintln!("\n--- {} ({}) ---", fd.label, fd.path);
        eprintln!("  DocInfo records: {}", fd.doc_info_records.len());

        if let Some(id_rec) = fd
            .doc_info_records
            .iter()
            .find(|r| r.tag_id == tags::HWPTAG_ID_MAPPINGS)
        {
            eprintln!("  ID_MAPPINGS record size: {} bytes", id_rec.data.len());
            let num_values = id_rec.data.len() / 4;
            for i in 0..std::cmp::min(num_values, id_map_field_names.len()) {
                let val = read_u32_le(&id_rec.data, i * 4);
                eprintln!("    [{:>2}] {:<25} = {}", i, id_map_field_names[i], val);
            }

            // Highlight key counts
            let cs = if num_values > 9 {
                read_u32_le(&id_rec.data, 9 * 4)
            } else {
                0
            };
            let ps = if num_values > 13 {
                read_u32_le(&id_rec.data, 13 * 4)
            } else {
                0
            };
            let bf = if num_values > 8 {
                read_u32_le(&id_rec.data, 8 * 4)
            } else {
                0
            };
            eprintln!(
                "  >>> CharShape(CS)={}, ParaShape(PS)={}, BorderFill(BF)={}",
                cs, ps, bf
            );
        } else {
            eprintln!("  WARNING: No ID_MAPPINGS record found!");
        }
    }

    // ============================================================
    // 2. BodyText record full dump for each file
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("  PART 2: BodyText Section0 - Full Record Dump (ALL files)");
    eprintln!("{}", "=".repeat(120));

    for fd in &all_files {
        eprintln!("\n{}", "#".repeat(100));
        eprintln!("### {} ({}) ###", fd.label, fd.path);
        eprintln!(
            "### BodyText decompressed: {} bytes, records: {} ###",
            fd.body_raw_len,
            fd.body_records.len()
        );
        eprintln!("{}", "#".repeat(100));

        // Total bytes in records
        let total_data_bytes: usize = fd.body_records.iter().map(|r| r.data.len()).sum();
        eprintln!("  Total record data bytes: {}", total_data_bytes);

        eprintln!(
            "\n{:<5} {:<5} {:<25} {:>8}  Details",
            "Idx", "Lvl", "Tag", "Size"
        );
        eprintln!("{:-<120}", "");

        for (i, rec) in fd.body_records.iter().enumerate() {
            let indent = "  ".repeat(std::cmp::min(rec.level as usize, 8));
            let tag_str = format!("{}{}", indent, tags::tag_name(rec.tag_id));

            let mut details = String::new();

            // PARA_HEADER details
            if rec.tag_id == tags::HWPTAG_PARA_HEADER {
                if rec.data.len() >= 22 {
                    // PARA_HEADER layout:
                    // u32 nChars (bit31=MSB)
                    // u32 controlMask
                    // u16 paraShapeId
                    // u8  styleId (or u16)
                    // u8  breakType
                    // u16 charShapeCount (?)
                    // ...
                    let raw_char_count = read_u32_le(&rec.data, 0);
                    let msb = (raw_char_count >> 31) & 1;
                    let char_count = raw_char_count & 0x7FFFFFFF;
                    let control_mask = read_u32_le(&rec.data, 4);
                    let para_shape_id = read_u16_le(&rec.data, 8);
                    let style_id = rec.data[10];
                    details = format!(
                        "char_count={} msb={} control_mask=0x{:08X} para_shape_id={} style_id={}",
                        char_count, msb, control_mask, para_shape_id, style_id
                    );
                } else {
                    details = format!(
                        "(short data: {} bytes) hex={}",
                        rec.data.len(),
                        hex_preview(&rec.data, 32)
                    );
                }
            }
            // CTRL_HEADER details
            else if rec.tag_id == tags::HWPTAG_CTRL_HEADER {
                details = format!(
                    "ctrl={} hex={}",
                    ctrl_id_string(&rec.data),
                    hex_preview(&rec.data, 40)
                );
            }
            // TABLE details
            else if rec.tag_id == tags::HWPTAG_TABLE {
                if rec.data.len() >= 8 {
                    let attr = read_u32_le(&rec.data, 0);
                    let rows = read_u16_le(&rec.data, 4);
                    let cols = read_u16_le(&rec.data, 6);
                    details = format!(
                        "attr=0x{:08X} rows={} cols={} hex={}",
                        attr,
                        rows,
                        cols,
                        hex_preview(&rec.data, 40)
                    );
                } else {
                    details = format!("hex={}", hex_preview(&rec.data, 40));
                }
            }
            // LIST_HEADER details
            else if rec.tag_id == tags::HWPTAG_LIST_HEADER {
                if rec.data.len() >= 6 {
                    let para_count = read_u16_le(&rec.data, 0);
                    let attr = read_u32_le(&rec.data, 2);
                    details = format!(
                        "paraCount={} attr=0x{:08X} hex={}",
                        para_count,
                        attr,
                        hex_preview(&rec.data, 40)
                    );
                } else {
                    details = format!("hex={}", hex_preview(&rec.data, 40));
                }
            }
            // PARA_TEXT: show char codes
            else if rec.tag_id == tags::HWPTAG_PARA_TEXT {
                // UTF-16LE text
                let mut chars_preview = String::new();
                let max_chars = 30;
                let mut n = 0;
                let mut pos = 0;
                while pos + 1 < rec.data.len() && n < max_chars {
                    let code = u16::from_le_bytes([rec.data[pos], rec.data[pos + 1]]);
                    if code == 0x000D {
                        chars_preview.push_str("\\r");
                    } else if code == 0x000A {
                        chars_preview.push_str("\\n");
                    } else if code == 0x000B {
                        chars_preview.push_str("{CTRL}");
                    } else if code == 0x0002 {
                        chars_preview.push_str("{SECD}");
                    } else if code == 0x0003 {
                        chars_preview.push_str("{FLD_BEGIN}");
                    } else if code == 0x0004 {
                        chars_preview.push_str("{FLD_END}");
                    } else if code == 0x0008 {
                        chars_preview.push_str("{INLINE}");
                    } else if code < 0x0020 {
                        chars_preview.push_str(&format!("{{0x{:04X}}}", code));
                    } else if let Some(ch) = char::from_u32(code as u32) {
                        chars_preview.push(ch);
                    } else {
                        chars_preview.push_str(&format!("{{0x{:04X}}}", code));
                    }
                    pos += 2;
                    // Extended control chars take 16 bytes total (skip the inline data)
                    if code == 0x000B || code == 0x0002 || code == 0x0003 || code == 0x0008 {
                        pos += 14; // skip 14 more bytes (total 16 for extended char)
                    }
                    n += 1;
                }
                details = format!(
                    "text=\"{}\" hex={}",
                    chars_preview,
                    hex_preview(&rec.data, 32)
                );
            }
            // PARA_CHAR_SHAPE
            else if rec.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
                // pairs of (u32 pos, u32 charShapeId)
                let n_pairs = rec.data.len() / 8;
                let mut pairs_str = String::new();
                for p in 0..std::cmp::min(n_pairs, 8) {
                    let pos_val = read_u32_le(&rec.data, p * 8);
                    let cs_id = read_u32_le(&rec.data, p * 8 + 4);
                    if !pairs_str.is_empty() {
                        pairs_str.push_str(", ");
                    }
                    pairs_str.push_str(&format!("pos{}=>CS{}", pos_val, cs_id));
                }
                if n_pairs > 8 {
                    pairs_str.push_str(&format!(" ...({} more)", n_pairs - 8));
                }
                details = format!("[{}]", pairs_str);
            }
            // SHAPE_COMPONENT
            else if rec.tag_id == tags::HWPTAG_SHAPE_COMPONENT {
                details = format!("hex={}", hex_preview(&rec.data, 40));
            }
            // SHAPE_COMPONENT_PICTURE
            else if rec.tag_id == tags::HWPTAG_SHAPE_COMPONENT_PICTURE {
                details = format!("hex={}", hex_preview(&rec.data, 40));
            }
            // PAGE_DEF
            else if rec.tag_id == tags::HWPTAG_PAGE_DEF {
                if rec.data.len() >= 40 {
                    let w = read_u32_le(&rec.data, 0);
                    let h = read_u32_le(&rec.data, 4);
                    details = format!("width={} height={} (hwpunit)", w, h);
                }
            }
            // FOOTNOTE_SHAPE
            else if rec.tag_id == tags::HWPTAG_FOOTNOTE_SHAPE {
                details = format!("hex={}", hex_preview(&rec.data, 32));
            }
            // PAGE_BORDER_FILL
            else if rec.tag_id == tags::HWPTAG_PAGE_BORDER_FILL {
                details = format!("hex={}", hex_preview(&rec.data, 32));
            }
            // Default: show hex for smaller records
            else if rec.data.len() <= 64 {
                details = format!("hex={}", hex_preview(&rec.data, 48));
            } else {
                details = format!("hex={}", hex_preview(&rec.data, 32));
            }

            eprintln!(
                "{:<5} {:<5} {:<25} {:>8}  {}",
                i, rec.level, tag_str, rec.size, details
            );
        }
    }

    // ============================================================
    // 3. Side-by-side comparison: HWP_PASTE (VALID) vs VIEWER_PASTE (DAMAGED)
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("  PART 3: Side-by-Side Comparison - HWP_PASTE vs VIEWER_PASTE");
    eprintln!("{}", "=".repeat(120));

    let valid = &all_files[1]; // HWP_PASTE
    let damaged = &all_files[2]; // VIEWER_PASTE

    eprintln!(
        "\n  VALID (HWP_PASTE):   {} records, {} data bytes",
        valid.body_records.len(),
        valid.body_raw_len
    );
    eprintln!(
        "  DAMAGED (VIEWER_PASTE): {} records, {} data bytes",
        damaged.body_records.len(),
        damaged.body_raw_len
    );

    let max_recs = std::cmp::max(valid.body_records.len(), damaged.body_records.len());
    let mut total_diffs = 0;

    eprintln!(
        "\n{:<5} | {:<30} {:>4} {:>6} | {:<30} {:>4} {:>6} | Status",
        "#", "VALID Tag", "Lvl", "Size", "DAMAGED Tag", "Lvl", "Size"
    );
    eprintln!("{:-<130}", "");

    for i in 0..max_recs {
        let have_valid = i < valid.body_records.len();
        let have_damaged = i < damaged.body_records.len();

        let (v_tag_str, v_lvl, v_size) = if have_valid {
            let r = &valid.body_records[i];
            (format!("{}", tags::tag_name(r.tag_id)), r.level, r.size)
        } else {
            ("---".to_string(), 0u16, 0u32)
        };

        let (d_tag_str, d_lvl, d_size) = if have_damaged {
            let r = &damaged.body_records[i];
            (format!("{}", tags::tag_name(r.tag_id)), r.level, r.size)
        } else {
            ("---".to_string(), 0u16, 0u32)
        };

        let status = if !have_valid {
            total_diffs += 1;
            "EXTRA_IN_DAMAGED"
        } else if !have_damaged {
            total_diffs += 1;
            "MISSING_IN_DAMAGED"
        } else {
            let rv = &valid.body_records[i];
            let rd = &damaged.body_records[i];
            if rv.tag_id != rd.tag_id {
                total_diffs += 1;
                "TAG_DIFF"
            } else if rv.level != rd.level {
                total_diffs += 1;
                "LEVEL_DIFF"
            } else if rv.data != rd.data {
                total_diffs += 1;
                "DATA_DIFF"
            } else {
                "OK"
            }
        };

        // For diffs, always print. For OK, also print (full dump requested).
        let marker = if status != "OK" { ">>>" } else { "   " };
        eprintln!(
            "{} {:<5} | {:<30} {:>4} {:>6} | {:<30} {:>4} {:>6} | {}",
            marker, i, v_tag_str, v_lvl, v_size, d_tag_str, d_lvl, d_size, status
        );

        // For diffs, show detailed comparison
        if status != "OK" && have_valid && have_damaged {
            let rv = &valid.body_records[i];
            let rd = &damaged.body_records[i];

            // PARA_HEADER diff details
            if rv.tag_id == tags::HWPTAG_PARA_HEADER || rd.tag_id == tags::HWPTAG_PARA_HEADER {
                if rv.tag_id == tags::HWPTAG_PARA_HEADER && rd.tag_id == tags::HWPTAG_PARA_HEADER {
                    let v_char = read_u32_le(&rv.data, 0);
                    let d_char = read_u32_le(&rd.data, 0);
                    let v_mask = read_u32_le(&rv.data, 4);
                    let d_mask = read_u32_le(&rd.data, 4);
                    let v_ps = read_u16_le(&rv.data, 8);
                    let d_ps = read_u16_le(&rd.data, 8);
                    let v_st = if rv.data.len() > 10 { rv.data[10] } else { 0 };
                    let d_st = if rd.data.len() > 10 { rd.data[10] } else { 0 };
                    eprintln!("          PARA_HEADER diff: char_count {}={} vs {}={}, mask 0x{:08X} vs 0x{:08X}, ps {} vs {}, style {} vs {}",
                            if v_char != d_char { "DIFF" } else { "same" }, v_char & 0x7FFFFFFF,
                            if v_char != d_char { "DIFF" } else { "same" }, d_char & 0x7FFFFFFF,
                            v_mask, d_mask, v_ps, d_ps, v_st, d_st);
                }
            }

            // CTRL_HEADER diff details
            if rv.tag_id == tags::HWPTAG_CTRL_HEADER && rd.tag_id == tags::HWPTAG_CTRL_HEADER {
                eprintln!(
                    "          VALID ctrl={}  DAMAGED ctrl={}",
                    ctrl_id_string(&rv.data),
                    ctrl_id_string(&rd.data)
                );
            }

            // Show hex of both
            eprintln!("          VALID  hex: {}", hex_preview(&rv.data, 48));
            eprintln!("          DAMAGED hex: {}", hex_preview(&rd.data, 48));

            // Show first byte diff position
            let min_len = std::cmp::min(rv.data.len(), rd.data.len());
            if let Some(pos) = (0..min_len).find(|&j| rv.data[j] != rd.data[j]) {
                eprintln!(
                    "          First byte diff at offset {}: VALID=0x{:02x} DAMAGED=0x{:02x}",
                    pos, rv.data[pos], rd.data[pos]
                );
            }
            if rv.data.len() != rd.data.len() {
                eprintln!(
                    "          Size diff: VALID={} DAMAGED={}",
                    rv.data.len(),
                    rd.data.len()
                );
            }
        }
    }

    // ============================================================
    // 4. DocInfo comparison: VALID vs DAMAGED
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("  PART 4: DocInfo Comparison - HWP_PASTE vs VIEWER_PASTE");
    eprintln!("{}", "=".repeat(120));

    let v_doc = &all_files[1].doc_info_records;
    let d_doc = &all_files[2].doc_info_records;
    eprintln!("\n  VALID DocInfo records: {}", v_doc.len());
    eprintln!("  DAMAGED DocInfo records: {}", d_doc.len());

    let max_doc = std::cmp::max(v_doc.len(), d_doc.len());
    let mut doc_diffs = 0;
    for i in 0..max_doc {
        let have_v = i < v_doc.len();
        let have_d = i < d_doc.len();
        let status = if !have_v {
            "EXTRA_IN_DAMAGED"
        } else if !have_d {
            "MISSING_IN_DAMAGED"
        } else if v_doc[i].tag_id != d_doc[i].tag_id {
            "TAG_DIFF"
        } else if v_doc[i].level != d_doc[i].level {
            "LEVEL_DIFF"
        } else if v_doc[i].data != d_doc[i].data {
            "DATA_DIFF"
        } else {
            "OK"
        };

        if status != "OK" {
            doc_diffs += 1;
            let v_str = if have_v {
                format!(
                    "{} lvl={} sz={}",
                    tags::tag_name(v_doc[i].tag_id),
                    v_doc[i].level,
                    v_doc[i].size
                )
            } else {
                "---".to_string()
            };
            let d_str = if have_d {
                format!(
                    "{} lvl={} sz={}",
                    tags::tag_name(d_doc[i].tag_id),
                    d_doc[i].level,
                    d_doc[i].size
                )
            } else {
                "---".to_string()
            };
            eprintln!(
                "  [{}] {} | VALID: {} | DAMAGED: {} |",
                i, status, v_str, d_str
            );
        }
    }
    if doc_diffs == 0 {
        eprintln!("  DocInfo records are IDENTICAL between VALID and DAMAGED");
    } else {
        eprintln!("  Total DocInfo differences: {}", doc_diffs);
    }

    // ============================================================
    // 5. Summary
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("  SUMMARY");
    eprintln!("{}", "=".repeat(120));

    for fd in &all_files {
        let total_data: usize = fd.body_records.iter().map(|r| r.data.len()).sum();
        eprintln!(
            "  {:<25} DocInfo recs={:<5} BodyText recs={:<5} body_bytes={:<8} data_bytes={}",
            fd.label,
            fd.doc_info_records.len(),
            fd.body_records.len(),
            fd.body_raw_len,
            total_data
        );
    }

    eprintln!(
        "\n  BodyText record-by-record diffs (VALID vs DAMAGED): {}",
        total_diffs
    );
    eprintln!(
        "  DocInfo record-by-record diffs (VALID vs DAMAGED): {}",
        doc_diffs
    );

    eprintln!("\n=== test_step2_comparison complete ===");
}

#[test]
fn test_text_insert_detailed_diff() {
    // 텍스트 삽입 후 저장된 파일의 모든 레코드를 원본과 상세 비교
    let path = "template/empty.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let orig_data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    // 텍스트 삽입
    doc.insert_text_native(0, 0, 0, "가나다라마바사아").unwrap();
    let saved = doc.export_hwp_native().unwrap();

    // 레코드 파싱
    use crate::parser::record::Record;
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();

    let saved_doc = crate::parser::parse_hwp(&saved).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    eprintln!("\n=== 원본 레코드 ===");
    for (i, r) in orig_recs.iter().enumerate() {
        eprintln!(
            "[{:2}] tag={:3} (0x{:04x}) level={} size={}",
            i,
            r.tag_id,
            r.tag_id,
            r.level,
            r.data.len()
        );
    }
    eprintln!("\n=== 저장 레코드 ===");
    for (i, r) in saved_recs.iter().enumerate() {
        eprintln!(
            "[{:2}] tag={:3} (0x{:04x}) level={} size={}",
            i,
            r.tag_id,
            r.tag_id,
            r.level,
            r.data.len()
        );
    }

    // 레코드별 상세 비교
    let max = orig_recs.len().max(saved_recs.len());
    eprintln!("\n=== 바이트 비교 ===");
    for i in 0..max {
        let o = orig_recs.get(i);
        let s = saved_recs.get(i);
        match (o, s) {
            (Some(or), Some(sr)) => {
                if or.tag_id != sr.tag_id || or.level != sr.level || or.data != sr.data {
                    eprintln!(
                        "DIFF [{}]: tag={}/{} level={}/{} size={}/{}",
                        i,
                        or.tag_id,
                        sr.tag_id,
                        or.level,
                        sr.level,
                        or.data.len(),
                        sr.data.len()
                    );
                    // HWP 레코드 태그 이름 매핑
                    let tag_name = match or.tag_id {
                        66 => "PARA_HEADER",
                        67 => "PARA_TEXT",
                        68 => "PARA_CHAR_SHAPE",
                        69 => "PARA_LINE_SEG",
                        70 => "CTRL_HEADER",
                        71 => "LIST_HEADER",
                        _ => "UNKNOWN",
                    };
                    eprintln!("  Record type: {}", tag_name);

                    // 전체 데이터 헥스 덤프
                    let orig_hex: Vec<String> =
                        or.data.iter().map(|b| format!("{:02x}", b)).collect();
                    let save_hex: Vec<String> =
                        sr.data.iter().map(|b| format!("{:02x}", b)).collect();
                    eprintln!("  ORIG[{}]: {}", or.data.len(), orig_hex.join(" "));
                    eprintln!("  SAVE[{}]: {}", sr.data.len(), save_hex.join(" "));

                    // 바이트별 차이 표시
                    let min_len = or.data.len().min(sr.data.len());
                    for pos in 0..min_len {
                        if or.data[pos] != sr.data[pos] {
                            eprintln!(
                                "  Byte {}: 0x{:02x} → 0x{:02x}",
                                pos, or.data[pos], sr.data[pos]
                            );
                        }
                    }
                    if or.data.len() != sr.data.len() {
                        eprintln!(
                            "  Size diff: {} → {} (delta {})",
                            or.data.len(),
                            sr.data.len(),
                            sr.data.len() as i64 - or.data.len() as i64
                        );
                    }
                } else {
                    eprintln!(
                        "OK   [{}]: tag={} level={} size={}",
                        i,
                        or.tag_id,
                        or.level,
                        or.data.len()
                    );
                }
            }
            (Some(or), None) => eprintln!("MISSING [{}]: tag={}", i, or.tag_id),
            (None, Some(sr)) => eprintln!("EXTRA   [{}]: tag={}", i, sr.tag_id),
            _ => {}
        }
    }
}

#[test]
fn test_empty_hwp_editing_area() {
    // template/empty.hwp의 편집 영역, 캐럿 위치, LineSeg 값을 분석
    use crate::model::page::PageAreas;

    let path = "template/empty.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    eprintln!("\n{}", "=".repeat(70));
    eprintln!("  EMPTY HWP 편집 영역 분석");
    eprintln!("{}", "=".repeat(70));

    // 1. DocProperties 캐럿 정보
    let props = &doc.document.doc_properties;
    eprintln!("\n--- DocProperties 캐럿 정보 ---");
    eprintln!("  caret_list_id:  {}", props.caret_list_id);
    eprintln!("  caret_para_id:  {}", props.caret_para_id);
    eprintln!("  caret_char_pos: {}", props.caret_char_pos);

    // 2. PageDef (용지 설정)
    let section = &doc.document.sections[0];
    let page_def = &section.section_def.page_def;
    eprintln!("\n--- PageDef (용지 설정) ---");
    eprintln!(
        "  width:          {} HWPUNIT ({:.1}mm)",
        page_def.width,
        page_def.width as f64 / 283.46
    );
    eprintln!(
        "  height:         {} HWPUNIT ({:.1}mm)",
        page_def.height,
        page_def.height as f64 / 283.46
    );
    eprintln!(
        "  margin_left:    {} HWPUNIT ({:.1}mm)",
        page_def.margin_left,
        page_def.margin_left as f64 / 283.46
    );
    eprintln!(
        "  margin_right:   {} HWPUNIT ({:.1}mm)",
        page_def.margin_right,
        page_def.margin_right as f64 / 283.46
    );
    eprintln!(
        "  margin_top:     {} HWPUNIT ({:.1}mm)",
        page_def.margin_top,
        page_def.margin_top as f64 / 283.46
    );
    eprintln!(
        "  margin_bottom:  {} HWPUNIT ({:.1}mm)",
        page_def.margin_bottom,
        page_def.margin_bottom as f64 / 283.46
    );
    eprintln!(
        "  margin_header:  {} HWPUNIT ({:.1}mm)",
        page_def.margin_header,
        page_def.margin_header as f64 / 283.46
    );
    eprintln!(
        "  margin_footer:  {} HWPUNIT ({:.1}mm)",
        page_def.margin_footer,
        page_def.margin_footer as f64 / 283.46
    );
    eprintln!(
        "  margin_gutter:  {} HWPUNIT ({:.1}mm)",
        page_def.margin_gutter,
        page_def.margin_gutter as f64 / 283.46
    );
    eprintln!("  landscape:      {}", page_def.landscape);

    // 3. PageAreas (계산된 편집 영역)
    let areas = PageAreas::from_page_def(page_def);
    eprintln!("\n--- PageAreas (계산된 영역) ---");
    eprintln!(
        "  header_area:    left={} top={} right={} bottom={}",
        areas.header_area.left,
        areas.header_area.top,
        areas.header_area.right,
        areas.header_area.bottom
    );
    eprintln!(
        "  body_area:      left={} top={} right={} bottom={}",
        areas.body_area.left, areas.body_area.top, areas.body_area.right, areas.body_area.bottom
    );
    eprintln!(
        "  body_area size: width={} height={}",
        areas.body_area.right - areas.body_area.left,
        areas.body_area.bottom - areas.body_area.top
    );
    eprintln!(
        "  footer_area:    left={} top={} right={} bottom={}",
        areas.footer_area.left,
        areas.footer_area.top,
        areas.footer_area.right,
        areas.footer_area.bottom
    );

    // 4. 모든 문단의 LineSeg 정보
    eprintln!("\n--- 문단별 LineSeg 정보 ---");
    for (pi, para) in section.paragraphs.iter().enumerate() {
        eprintln!(
            "  문단[{}]: text='{}' char_count={} controls={}",
            pi,
            para.text,
            para.char_count,
            para.controls.len()
        );
        eprintln!(
            "    char_shapes: {:?}",
            para.char_shapes
                .iter()
                .map(|cs| (cs.start_pos, cs.char_shape_id))
                .collect::<Vec<_>>()
        );
        for (li, ls) in para.line_segs.iter().enumerate() {
            eprintln!("    LineSeg[{}]:", li);
            eprintln!("      text_start:         {}", ls.text_start);
            eprintln!(
                "      vertical_pos:       {} ({:.1}mm)",
                ls.vertical_pos,
                ls.vertical_pos as f64 / 283.46
            );
            eprintln!(
                "      line_height:        {} ({:.1}mm)",
                ls.line_height,
                ls.line_height as f64 / 283.46
            );
            eprintln!(
                "      text_height:        {} ({:.1}mm)",
                ls.text_height,
                ls.text_height as f64 / 283.46
            );
            eprintln!(
                "      baseline_distance:  {} ({:.1}mm)",
                ls.baseline_distance,
                ls.baseline_distance as f64 / 283.46
            );
            eprintln!(
                "      line_spacing:       {} ({:.1}mm)",
                ls.line_spacing,
                ls.line_spacing as f64 / 283.46
            );
            eprintln!(
                "      column_start:       {} ({:.1}mm)",
                ls.column_start,
                ls.column_start as f64 / 283.46
            );
            eprintln!(
                "      segment_width:      {} ({:.1}mm)",
                ls.segment_width,
                ls.segment_width as f64 / 283.46
            );
            eprintln!(
                "      tag:                0x{:08x} (first_of_page={} first_of_col={})",
                ls.tag,
                ls.is_first_line_of_page(),
                ls.is_first_line_of_column()
            );
        }
    }

    // 5. 편집 영역 첫 줄 캐럿 위치 분석
    if let Some(first_para) = section.paragraphs.first() {
        if let Some(first_ls) = first_para.line_segs.first() {
            eprintln!("\n--- 편집 영역 첫 줄 캐럿 위치 분석 ---");
            eprintln!("  body_area.top (계산값):     {}", areas.body_area.top);
            eprintln!("  LineSeg.vertical_pos (실제): {}", first_ls.vertical_pos);
            eprintln!(
                "  차이:                        {}",
                first_ls.vertical_pos - areas.body_area.top
            );
            eprintln!("  body_area.left (계산값):    {}", areas.body_area.left);
            eprintln!("  LineSeg.column_start (실제): {}", first_ls.column_start);
            eprintln!(
                "  차이:                        {}",
                first_ls.column_start - areas.body_area.left
            );
            let body_width = areas.body_area.right - areas.body_area.left;
            eprintln!("  body_area.width (계산값):   {}", body_width);
            eprintln!("  LineSeg.segment_width (실제): {}", first_ls.segment_width);
            eprintln!(
                "  차이:                        {}",
                first_ls.segment_width - body_width
            );
        }
    }

    // 6. ParaShape 정보 (첫 문단의 줄간격 등)
    if let Some(first_para) = section.paragraphs.first() {
        let ps_id = first_para.para_shape_id as usize;
        if ps_id < doc.document.doc_info.para_shapes.len() {
            let ps = &doc.document.doc_info.para_shapes[ps_id];
            eprintln!("\n--- ParaShape[{}] (첫 문단 문단모양) ---", ps_id);
            eprintln!("  line_spacing_type:  {:?}", ps.line_spacing_type);
            eprintln!("  line_spacing:       {}", ps.line_spacing);
            eprintln!("  line_spacing_v2:    {}", ps.line_spacing_v2);
            eprintln!("  margin_left:        {}", ps.margin_left);
            eprintln!("  margin_right:       {}", ps.margin_right);
        }
    }

    // 7. CharShape 정보 (첫 문단의 글자 크기)
    if let Some(first_para) = section.paragraphs.first() {
        if let Some(first_cs) = first_para.char_shapes.first() {
            let cs_id = first_cs.char_shape_id as usize;
            if cs_id < doc.document.doc_info.char_shapes.len() {
                let cs = &doc.document.doc_info.char_shapes[cs_id];
                eprintln!("\n--- CharShape[{}] (첫 문단 글자모양) ---", cs_id);
                eprintln!(
                    "  base_size:  {} ({:.1}pt)",
                    cs.base_size,
                    cs.base_size as f64 / 100.0
                );
            }
        }
    }

    eprintln!("\n{}", "=".repeat(70));
}

/// 진단: k-water-rfp.hwp 전체 문단의 char_count_msb 패턴 분석
#[test]
fn test_diag_msb_pattern_kwater() {
    use crate::model::control::Control;

    let path = "samples/k-water-rfp.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    eprintln!("\n{}", "=".repeat(70));
    eprintln!("  k-water-rfp.hwp MSB 패턴 분석");
    eprintln!("{}", "=".repeat(70));

    for (si, section) in doc.document.sections.iter().enumerate() {
        let para_count = section.paragraphs.len();
        eprintln!("\n  Section {} ({} paragraphs)", si, para_count);
        eprintln!(
            "  {:>4} | {:>5} | {:>3} | {:>5} | {:>3} | {:>8} | text_preview",
            "idx", "cc", "msb", "psid", "sid", "ctrl"
        );
        eprintln!("  {}", "-".repeat(65));

        for (pi, para) in section.paragraphs.iter().enumerate() {
            let is_last = pi == para_count - 1;
            let ctrl_info = if para.controls.is_empty() {
                String::new()
            } else {
                let ctrl_names: Vec<&str> = para
                    .controls
                    .iter()
                    .map(|c| match c {
                        Control::Table(_) => "TABLE",
                        Control::SectionDef(_) => "SECD",
                        Control::ColumnDef(_) => "COLD",
                        Control::Shape(_) => "SHAPE",
                        Control::Picture(_) => "PIC",
                        _ => "OTHER",
                    })
                    .collect();
                ctrl_names.join(",")
            };

            let text_preview: String = para.text.chars().take(30).collect();
            let msb_mark = if para.char_count_msb { "T" } else { "F" };
            let last_mark = if is_last { " <LAST>" } else { "" };

            eprintln!(
                "  {:>4} | {:>5} | {:>3} | {:>5} | {:>3} | {:>8} | {}{}",
                pi,
                para.char_count,
                msb_mark,
                para.para_shape_id,
                para.style_id,
                ctrl_info,
                text_preview,
                last_mark
            );

            // 컨트롤 내부 문단도 출력
            for ctrl in &para.controls {
                match ctrl {
                    Control::Table(tbl) => {
                        for (ci, cell) in tbl.cells.iter().enumerate() {
                            for (cpi, cp) in cell.paragraphs.iter().enumerate() {
                                let cp_msb = if cp.char_count_msb { "T" } else { "F" };
                                let cp_last = cpi == cell.paragraphs.len() - 1;
                                let cp_text: String = cp.text.chars().take(20).collect();
                                eprintln!("        cell[{}].p[{}]: cc={} msb={} psid={} sid={} text='{}'{}",
                                        ci, cpi, cp.char_count, cp_msb,
                                        cp.para_shape_id, cp.style_id, cp_text,
                                        if cp_last { " <CELL_LAST>" } else { "" });
                            }
                        }
                    }
                    Control::Shape(s) => {
                        // ShapeObject enum 에서 drawing.text_box 접근
                        let tb_opt = match s.as_ref() {
                            crate::model::shape::ShapeObject::Line(l) => {
                                l.drawing.text_box.as_ref()
                            }
                            crate::model::shape::ShapeObject::Rectangle(r) => {
                                r.drawing.text_box.as_ref()
                            }
                            crate::model::shape::ShapeObject::Ellipse(e) => {
                                e.drawing.text_box.as_ref()
                            }
                            crate::model::shape::ShapeObject::Arc(a) => a.drawing.text_box.as_ref(),
                            crate::model::shape::ShapeObject::Polygon(p) => {
                                p.drawing.text_box.as_ref()
                            }
                            crate::model::shape::ShapeObject::Curve(c) => {
                                c.drawing.text_box.as_ref()
                            }
                            _ => None,
                        };
                        if let Some(tb) = tb_opt {
                            for (tpi, tp) in tb.paragraphs.iter().enumerate() {
                                let tp_msb = if tp.char_count_msb { "T" } else { "F" };
                                let tp_last = tpi == tb.paragraphs.len() - 1;
                                let tp_text: String = tp.text.chars().take(20).collect();
                                eprintln!(
                                    "        textbox.p[{}]: cc={} msb={} psid={} text='{}'{}",
                                    tpi,
                                    tp.char_count,
                                    tp_msb,
                                    tp.para_shape_id,
                                    tp_text,
                                    if tp_last { " <TB_LAST>" } else { "" }
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // 통계 집계
    eprintln!("\n{}", "=".repeat(70));
    eprintln!("  MSB 패턴 통계");
    eprintln!("{}", "=".repeat(70));

    for (si, section) in doc.document.sections.iter().enumerate() {
        let para_count = section.paragraphs.len();
        let mut msb_true_count = 0;
        let mut msb_false_count = 0;
        let mut last_para_msb = false;
        let mut mid_para_msb_true = Vec::new(); // MSB=T인 중간 문단

        for (pi, para) in section.paragraphs.iter().enumerate() {
            if para.char_count_msb {
                msb_true_count += 1;
                if pi < para_count - 1 {
                    mid_para_msb_true.push(pi);
                }
            } else {
                msb_false_count += 1;
            }
            if pi == para_count - 1 {
                last_para_msb = para.char_count_msb;
            }
        }

        eprintln!(
            "  Section {}: total={} MSB_T={} MSB_F={} last_msb={}",
            si,
            para_count,
            msb_true_count,
            msb_false_count,
            if last_para_msb { "T" } else { "F" }
        );

        if !mid_para_msb_true.is_empty() {
            eprintln!("    ** 중간 문단에서 MSB=T: {:?}", mid_para_msb_true);
            for &pi in &mid_para_msb_true {
                let para = &section.paragraphs[pi];
                let ctrl_info: Vec<&str> = para
                    .controls
                    .iter()
                    .map(|c| match c {
                        Control::Table(_) => "TABLE",
                        Control::Shape(_) => "SHAPE",
                        Control::Picture(_) => "PIC",
                        Control::SectionDef(_) => "SECD",
                        _ => "OTHER",
                    })
                    .collect();
                eprintln!(
                    "    para[{}]: cc={} ctrl=[{}] psid={} sid={}",
                    pi,
                    para.char_count,
                    ctrl_info.join(","),
                    para.para_shape_id,
                    para.style_id
                );
            }
        }
    }
}

#[test]
fn test_textbox_render_tree_debug() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};
    use std::path::Path;

    let path = Path::new("samples/img-start-001.hwp");
    if !path.exists() {
        eprintln!("img-start-001.hwp 없음 — 건너뜀");
        return;
    }

    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();
    doc.convert_to_editable_native().unwrap();

    // 문서 구조 확인: Shape 컨트롤 찾기
    let mut shape_found = false;
    for (si, sec) in doc.document.sections.iter().enumerate() {
        for (pi, para) in sec.paragraphs.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                if let Control::Shape(shape) = ctrl {
                    let has_textbox = match shape.as_ref() {
                        crate::model::shape::ShapeObject::Rectangle(r) => {
                            r.drawing.text_box.is_some()
                        }
                        crate::model::shape::ShapeObject::Ellipse(e) => {
                            e.drawing.text_box.is_some()
                        }
                        crate::model::shape::ShapeObject::Polygon(p) => {
                            p.drawing.text_box.is_some()
                        }
                        crate::model::shape::ShapeObject::Curve(c) => c.drawing.text_box.is_some(),
                        _ => false,
                    };
                    if has_textbox {
                        let tb = get_textbox_from_shape(shape.as_ref()).unwrap();
                        let drawing = match shape.as_ref() {
                            crate::model::shape::ShapeObject::Rectangle(r) => Some(&r.drawing),
                            crate::model::shape::ShapeObject::Ellipse(e) => Some(&e.drawing),
                            crate::model::shape::ShapeObject::Polygon(p) => Some(&p.drawing),
                            crate::model::shape::ShapeObject::Curve(c) => Some(&c.drawing),
                            _ => None,
                        };
                        eprintln!(
                            "Shape 발견: sec={} para={} ctrl={} type={:?} textbox_paras={}",
                            si,
                            pi,
                            ci,
                            match shape.as_ref() {
                                crate::model::shape::ShapeObject::Rectangle(_) => "Rectangle",
                                crate::model::shape::ShapeObject::Ellipse(_) => "Ellipse",
                                crate::model::shape::ShapeObject::Polygon(_) => "Polygon",
                                crate::model::shape::ShapeObject::Curve(_) => "Curve",
                                _ => "Other",
                            },
                            tb.paragraphs.len(),
                        );
                        if let Some(d) = drawing {
                            eprintln!("  fill_type={:?}", d.fill.fill_type);
                            let sa = &d.shape_attr;
                            eprintln!(
                                "  shape_attr: orig_w={} orig_h={} cur_w={} cur_h={}",
                                sa.original_width,
                                sa.original_height,
                                sa.current_width,
                                sa.current_height
                            );
                            if let Some(ref tb) = d.text_box {
                                eprintln!(
                                    "  textbox margins: left={} right={} top={} bottom={} max_w={}",
                                    tb.margin_left,
                                    tb.margin_right,
                                    tb.margin_top,
                                    tb.margin_bottom,
                                    tb.max_width
                                );
                            }
                            if let Some(ref g) = d.fill.gradient {
                                eprintln!("  gradient: type={} angle={} cx={} cy={} blur={} colors={:?} positions={:?}",
                                        g.gradient_type, g.angle, g.center_x, g.center_y, g.blur,
                                        g.colors.iter().map(|c| format!("#{:06X}", c)).collect::<Vec<_>>(),
                                        g.positions,
                                    );
                            }
                        }
                        let common = match shape.as_ref() {
                            crate::model::shape::ShapeObject::Rectangle(r) => Some(&r.common),
                            crate::model::shape::ShapeObject::Ellipse(e) => Some(&e.common),
                            crate::model::shape::ShapeObject::Polygon(p) => Some(&p.common),
                            crate::model::shape::ShapeObject::Curve(c) => Some(&c.common),
                            _ => None,
                        };
                        if let Some(c) = common {
                            eprintln!("  common: width={} height={} treat_as_char={} horz_rel={:?} vert_rel={:?} h_off={} v_off={}",
                                    c.width, c.height, c.treat_as_char, c.horz_rel_to, c.vert_rel_to,
                                    c.horizontal_offset, c.vertical_offset);
                        }
                        for (tpi, tp) in tb.paragraphs.iter().enumerate() {
                            let text: String = tp.text.chars().take(30).collect();
                            eprintln!(
                                "  tb_para[{}]: text={:?} total_chars={}",
                                tpi,
                                text,
                                tp.text.chars().count()
                            );
                        }
                        shape_found = true;
                    }
                }
            }
        }
    }
    assert!(shape_found, "글상자가 있는 Shape 컨트롤을 찾지 못했습니다");

    // 모든 문단 내용 덤프
    eprintln!("\n=== 문단 목록 (섹션 0) ===");
    let sec = &doc.document.sections[0];
    for (pi, para) in sec.paragraphs.iter().enumerate() {
        let text: String = para.text.chars().take(60).collect();
        let ctrl_types: Vec<String> = para
            .controls
            .iter()
            .map(|c| match c {
                Control::Table(_) => "Table".to_string(),
                Control::Shape(s) => format!(
                    "Shape({:?})",
                    match s.as_ref() {
                        crate::model::shape::ShapeObject::Rectangle(_) => "Rect",
                        crate::model::shape::ShapeObject::Ellipse(_) => "Ellipse",
                        crate::model::shape::ShapeObject::Line(_) => "Line",
                        _ => "Other",
                    }
                ),
                Control::SectionDef(_) => "SectionDef".to_string(),
                Control::ColumnDef(_) => "ColumnDef".to_string(),
                _ => "Other".to_string(),
            })
            .collect();
        eprintln!(
            "  para[{}]: text_len={} line_segs={} char_shapes={} ctrls={:?} text={:?}",
            pi,
            para.text.chars().count(),
            para.line_segs.len(),
            para.char_shapes.len(),
            ctrl_types,
            text
        );
    }

    // 렌더 트리에서 TextRun의 cell context 확인
    let page_count = doc.page_count();
    eprintln!("\n페이지 수: {}", page_count);

    fn count_textruns(node: &RenderNode, body_runs: &mut Vec<String>, cell_runs: &mut Vec<String>) {
        if let RenderNodeType::TextRun(ref tr) = node.node_type {
            let (ppi, ci, cei, cpi) =
                tr.cell_context
                    .as_ref()
                    .map_or((None, None, None, None), |ctx| {
                        (
                            Some(ctx.parent_para_index),
                            Some(ctx.path[0].control_index),
                            Some(ctx.path[0].cell_index),
                            Some(ctx.path[0].cell_para_index),
                        )
                    });
            let info = format!(
                    "text={:?} sec={:?} para={:?} char_start={:?} ppi={:?} ci={:?} cei={:?} cpi={:?} bbox=({:.1},{:.1},{:.1},{:.1})",
                    tr.text.chars().take(15).collect::<String>(),
                    tr.section_index, tr.para_index, tr.char_start,
                    ppi, ci, cei, cpi,
                    node.bbox.x, node.bbox.y, node.bbox.width, node.bbox.height,
                );
            if tr.cell_context.is_some() {
                cell_runs.push(info);
            } else {
                body_runs.push(info);
            }
        }
        for child in &node.children {
            count_textruns(child, body_runs, cell_runs);
        }
    }

    for page in 0..page_count {
        let tree = doc.build_page_tree(page as u32).unwrap();
        let mut body_runs = Vec::new();
        let mut cell_runs = Vec::new();
        count_textruns(&tree.root, &mut body_runs, &mut cell_runs);
        eprintln!("\n--- 페이지 {} ---", page);
        eprintln!("본문 TextRun: {}개", body_runs.len());
        for r in &body_runs {
            eprintln!("  [body] {}", r);
        }
        eprintln!("셀/글상자 TextRun: {}개", cell_runs.len());
        for r in &cell_runs {
            eprintln!("  [cell] {}", r);
        }
    }
}

/// tb-err-003.hwp: 저장→로드→재저장 시 control_mask/has_para_text 보정 검증
#[test]
fn test_diag_tb_err_003() {
    use crate::model::control::Control;
    use crate::parser::body_text::parse_body_text_section;
    use crate::serializer::body_text::serialize_section;

    // 두 파일 모두 분석
    let files = vec!["saved/tb-err-003.hwp", "saved/tb-err-003-s.hwp"];
    for path in &files {
        if !std::path::Path::new(path).exists() {
            eprintln!("SKIP: {} 없음", path);
            continue;
        }

        let data = std::fs::read(path).unwrap();
        let doc = crate::parser::parse_hwp(&data).unwrap();

        eprintln!("\n{}", "=".repeat(80));
        eprintln!("=== {} 진단 ({} bytes) ===", path, data.len());
        eprintln!("섹션 수: {}", doc.sections.len());

        for (si, section) in doc.sections.iter().enumerate() {
            eprintln!(
                "\n--- Section {} (문단 {}개) ---",
                si,
                section.paragraphs.len()
            );
            for (pi, para) in section.paragraphs.iter().enumerate() {
                let ctrl_types: Vec<String> = para
                    .controls
                    .iter()
                    .map(|c| match c {
                        Control::Table(t) => format!("Table({}x{})", t.row_count, t.col_count),
                        _ => format!("{:?}", std::mem::discriminant(c)),
                    })
                    .collect();
                eprintln!("  문단[{}]: text={:?} char_count={} msb={} ctrl_mask=0x{:08X} controls=[{}] line_segs={} has_para_text={} raw_header_extra({})={:02x?}",
                    pi, &para.text.chars().take(30).collect::<String>(),
                    para.char_count, para.char_count_msb, para.control_mask,
                    ctrl_types.join(", "), para.line_segs.len(), para.has_para_text,
                    para.raw_header_extra.len(), &para.raw_header_extra);
                for (ci, ctrl) in para.controls.iter().enumerate() {
                    if let Control::Table(t) = ctrl {
                        eprintln!(
                            "\n  문단[{}] 컨트롤[{}]: 표 {}행×{}열 (셀 {}개)",
                            pi,
                            ci,
                            t.row_count,
                            t.col_count,
                            t.cells.len()
                        );
                        eprintln!("  row_sizes: {:?}", t.row_sizes);
                        eprintln!("  raw_table_record_attr: 0x{:08X}", t.raw_table_record_attr);
                        eprintln!(
                            "  raw_table_record_extra ({} bytes): {:02x?}",
                            t.raw_table_record_extra.len(),
                            &t.raw_table_record_extra
                        );

                        // 각 셀 상세
                        for (cell_idx, cell) in t.cells.iter().enumerate() {
                            eprintln!(
                                "  셀[{}]: col={} row={} cs={} rs={} w={} h={} bfid={} paras={}",
                                cell_idx,
                                cell.col,
                                cell.row,
                                cell.col_span,
                                cell.row_span,
                                cell.width,
                                cell.height,
                                cell.border_fill_id,
                                cell.paragraphs.len()
                            );
                            eprintln!(
                                "    raw_list_extra ({} bytes): {:02x?}",
                                cell.raw_list_extra.len(),
                                &cell.raw_list_extra
                            );
                            for (pp, para) in cell.paragraphs.iter().enumerate() {
                                eprintln!("    para[{}]: text={:?} char_count={} msb={} line_segs={} char_shapes={} has_para_text={}",
                                    pp, &para.text.chars().take(20).collect::<String>(),
                                    para.char_count, para.char_count_msb,
                                    para.line_segs.len(), para.char_shapes.len(), para.has_para_text);
                                eprintln!(
                                    "      raw_header_extra ({} bytes): {:02x?}",
                                    para.raw_header_extra.len(),
                                    &para.raw_header_extra
                                );
                            }
                        }

                        // row_sizes 검증: 각 행의 실제 셀 수와 row_sizes 비교
                        eprintln!("\n  --- row_sizes 검증 ---");
                        for r in 0..t.row_count {
                            let actual_count = t.cells.iter().filter(|c| c.row == r).count();
                            let expected = if (r as usize) < t.row_sizes.len() {
                                t.row_sizes[r as usize]
                            } else {
                                -1
                            };
                            let match_str = if actual_count as i16 == expected {
                                "OK"
                            } else {
                                "*** MISMATCH ***"
                            };
                            eprintln!(
                                "  행[{}]: row_sizes={} 실제셀수={} {}",
                                r, expected, actual_count, match_str
                            );
                        }

                        // col_count 검증: 셀들의 최대 col+col_span
                        let max_col_extent = t
                            .cells
                            .iter()
                            .map(|c| c.col + c.col_span)
                            .max()
                            .unwrap_or(0);
                        eprintln!(
                            "  col_count={} 최대열범위={} {}",
                            t.col_count,
                            max_col_extent,
                            if t.col_count == max_col_extent {
                                "OK"
                            } else {
                                "*** MISMATCH ***"
                            }
                        );

                        let max_row_extent = t
                            .cells
                            .iter()
                            .map(|c| c.row + c.row_span)
                            .max()
                            .unwrap_or(0);
                        eprintln!(
                            "  row_count={} 최대행범위={} {}",
                            t.row_count,
                            max_row_extent,
                            if t.row_count == max_row_extent {
                                "OK"
                            } else {
                                "*** MISMATCH ***"
                            }
                        );
                    }
                }
            }

            // 직렬화 → 재파싱 검증 (raw_stream이 있으면 원본 그대로이므로, 없는 것처럼 재직렬화)
            use crate::serializer::record_writer::write_records;
            let mut records = Vec::new();
            crate::serializer::body_text::serialize_paragraph_list(
                &section.paragraphs,
                0,
                &mut records,
            );
            let serialized = write_records(&records);
            match parse_body_text_section(&serialized) {
                Ok(reparsed) => {
                    eprintln!(
                        "\n  직렬화→재파싱: OK ({} → {} 문단)",
                        section.paragraphs.len(),
                        reparsed.paragraphs.len()
                    );
                    // 재파싱된 문단의 control_mask 검증
                    for (pi, para) in reparsed.paragraphs.iter().enumerate() {
                        let expected_mask: u32 = para.controls.iter().fold(0u32, |mask, ctrl| {
                            let bit = match ctrl {
                                Control::SectionDef(_) | Control::ColumnDef(_) => 2,
                                Control::Table(_) | Control::Shape(_) | Control::Picture(_) => 11,
                                _ => 0,
                            };
                            mask | (1u32 << bit)
                        });
                        let mask_ok = if para.controls.is_empty() {
                            para.control_mask == 0
                        } else {
                            para.control_mask == expected_mask
                        };
                        if !mask_ok {
                            eprintln!("  *** 재파싱 문단[{}] control_mask 불일치: 0x{:08X} (expected 0x{:08X}, controls={}) ***",
                                pi, para.control_mask, expected_mask, para.controls.len());
                        }
                        // has_para_text 검증: 빈 문단에 PARA_TEXT 없어야 함
                        if para.text.is_empty() && para.controls.is_empty() && para.has_para_text {
                            eprintln!("  *** 재파싱 문단[{}] has_para_text=true on empty para (char_count={}) ***",
                                pi, para.char_count);
                        }
                        for (ci, ctrl) in para.controls.iter().enumerate() {
                            if let Control::Table(t2) = ctrl {
                                eprintln!(
                                    "  재파싱 표[{},{}]: {}행×{}열 (셀 {}개) row_sizes={:?}",
                                    pi,
                                    ci,
                                    t2.row_count,
                                    t2.col_count,
                                    t2.cells.len(),
                                    t2.row_sizes
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("\n  *** 재파싱 실패: {} ***", e);
                }
            }
        }
    } // for files loop
}

/// 엔터키 후 저장 시 파일 손상 진단: blanK2020 원본 vs 손상 파일 비교
#[test]
fn test_blank2020_enter_corruption_diagnosis() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let files = [
        ("blanK2020 원본", "saved/blanK2020.hwp"),
        (
            "blanK2020 엔터후저장(손상)",
            "saved/blanK2020_enter_saved_currupt.hwp",
        ),
    ];

    for (label, path) in &files {
        if !std::path::Path::new(path).exists() {
            eprintln!("SKIP: {} 파일 없음", path);
            continue;
        }

        let bytes = std::fs::read(path).unwrap();
        let mut cfb = CfbReader::open(&bytes).unwrap_or_else(|_| panic!("{} CFB 열기 실패", label));

        eprintln!("\n{}", "=".repeat(80));
        eprintln!("  {} ({} bytes)", label, bytes.len());

        // DocInfo
        let doc_info_data = cfb.read_doc_info(true).expect("DocInfo 읽기 실패");
        let doc_recs = Record::read_all(&doc_info_data).unwrap();
        let cs_count = doc_recs
            .iter()
            .filter(|r| r.tag_id == tags::HWPTAG_CHAR_SHAPE)
            .count();
        let ps_count = doc_recs
            .iter()
            .filter(|r| r.tag_id == tags::HWPTAG_PARA_SHAPE)
            .count();
        eprintln!(
            "  DocInfo: CS={} PS={} records={} bytes={}",
            cs_count,
            ps_count,
            doc_recs.len(),
            doc_info_data.len()
        );

        // BodyText Section0
        let body_data = cfb
            .read_body_text_section(0, true, false)
            .expect("BodyText 읽기 실패");
        let body_recs = Record::read_all(&body_data).unwrap();
        eprintln!(
            "  BodyText: {} records, {} bytes",
            body_recs.len(),
            body_data.len()
        );

        for (i, rec) in body_recs.iter().enumerate() {
            let indent = "  ".repeat(rec.level as usize);
            let tag_name = tags::tag_name(rec.tag_id);

            let extra = if rec.tag_id == tags::HWPTAG_PARA_HEADER {
                let cc = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                let cm = u32::from_le_bytes([rec.data[4], rec.data[5], rec.data[6], rec.data[7]]);
                let ps_id = u16::from_le_bytes([rec.data[8], rec.data[9]]);
                let char_count = cc & 0x7FFFFFFF;
                let msb = cc >> 31;
                format!(
                    " cc={} msb={} cm=0x{:08X} ps={} data_len={} raw_extra={}",
                    char_count,
                    msb,
                    cm,
                    ps_id,
                    rec.data.len(),
                    rec.data
                        .iter()
                        .skip(12)
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            } else if rec.tag_id == tags::HWPTAG_PARA_LINE_SEG {
                let mut segs_info = String::new();
                let mut pos = 0;
                let mut seg_idx = 0;
                while pos + 36 <= rec.data.len() {
                    let lh = i32::from_le_bytes([
                        rec.data[pos + 8],
                        rec.data[pos + 9],
                        rec.data[pos + 10],
                        rec.data[pos + 11],
                    ]);
                    let th = i32::from_le_bytes([
                        rec.data[pos + 12],
                        rec.data[pos + 13],
                        rec.data[pos + 14],
                        rec.data[pos + 15],
                    ]);
                    let sw = i32::from_le_bytes([
                        rec.data[pos + 28],
                        rec.data[pos + 29],
                        rec.data[pos + 30],
                        rec.data[pos + 31],
                    ]);
                    let tag = u32::from_le_bytes([
                        rec.data[pos + 32],
                        rec.data[pos + 33],
                        rec.data[pos + 34],
                        rec.data[pos + 35],
                    ]);
                    segs_info += &format!(
                        " [seg{}: lh={} th={} sw={} tag=0x{:08X}]",
                        seg_idx, lh, th, sw, tag
                    );
                    seg_idx += 1;
                    pos += 36;
                }
                segs_info
            } else if rec.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
                let mut ids = Vec::new();
                let mut pos = 0;
                while pos + 8 <= rec.data.len() {
                    let cs_id = u32::from_le_bytes([
                        rec.data[pos + 4],
                        rec.data[pos + 5],
                        rec.data[pos + 6],
                        rec.data[pos + 7],
                    ]);
                    ids.push(cs_id);
                    pos += 8;
                }
                format!(" cs_ids={:?}", ids)
            } else if rec.tag_id == tags::HWPTAG_PARA_TEXT {
                let hex: String = rec
                    .data
                    .iter()
                    .take(20)
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(" [{}]", hex)
            } else {
                String::new()
            };

            eprintln!(
                "  rec[{:3}] {}L{} {} ({}B){}",
                i,
                indent,
                rec.level,
                tag_name,
                rec.data.len(),
                extra
            );
        }
    }

    // 추가: 우리 파서로 로드 → split → export → 다시 레코드 비교
    let blank_path = "saved/blanK2020.hwp";
    if std::path::Path::new(blank_path).exists() {
        eprintln!("\n{}", "=".repeat(80));
        eprintln!("  === split_at 라운드트립 테스트 ===");
        let bytes = std::fs::read(blank_path).unwrap();
        let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
        doc.convert_to_editable_native().unwrap();

        // 원본 문단 정보
        let para = &doc.document.sections[0].paragraphs[0];
        eprintln!(
            "  원본 para[0]: text='{}' cc={} raw_header_extra({} bytes): {:02x?}",
            para.text,
            para.char_count,
            para.raw_header_extra.len(),
            &para.raw_header_extra
        );
        eprintln!(
            "  원본 para[0] line_segs[0].tag = 0x{:08X}",
            para.line_segs.first().map(|ls| ls.tag).unwrap_or(0)
        );

        // 엔터 (split at 0)
        let result = doc.split_paragraph_native(0, 0, 0, None);
        eprintln!("  split result: {:?}", result);

        // 분할 후 문단 정보
        for (i, p) in doc.document.sections[0].paragraphs.iter().enumerate() {
            eprintln!("  split 후 para[{}]: text='{}' cc={} has_para_text={} raw_header_extra({} bytes): {:02x?}",
                    i, p.text, p.char_count, p.has_para_text, p.raw_header_extra.len(), &p.raw_header_extra);
            if let Some(ls) = p.line_segs.first() {
                eprintln!(
                    "    line_seg: lh={} th={} bd={} sw={} tag=0x{:08X}",
                    ls.line_height, ls.text_height, ls.baseline_distance, ls.segment_width, ls.tag
                );
            }
        }

        // export
        let exported = doc.export_hwp_native().unwrap();
        eprintln!("  exported: {} bytes", exported.len());

        // re-parse exported
        let mut cfb2 = CfbReader::open(&exported).expect("재파싱 CFB 열기 실패");
        let body2 = cfb2
            .read_body_text_section(0, true, false)
            .expect("재파싱 BodyText 실패");
        let recs2 = Record::read_all(&body2).unwrap();
        eprintln!(
            "  재파싱 BodyText: {} records, {} bytes",
            recs2.len(),
            body2.len()
        );

        for (i, rec) in recs2.iter().enumerate() {
            let tag_name = tags::tag_name(rec.tag_id);
            if rec.tag_id == tags::HWPTAG_PARA_HEADER {
                eprintln!(
                    "  re-rec[{:3}] L{} {} ({}B) raw_extra={}",
                    i,
                    rec.level,
                    tag_name,
                    rec.data.len(),
                    rec.data
                        .iter()
                        .skip(12)
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            } else if rec.tag_id == tags::HWPTAG_PARA_LINE_SEG {
                let mut pos = 0;
                while pos + 36 <= rec.data.len() {
                    let tag = u32::from_le_bytes([
                        rec.data[pos + 32],
                        rec.data[pos + 33],
                        rec.data[pos + 34],
                        rec.data[pos + 35],
                    ]);
                    eprintln!(
                        "  re-rec[{:3}] L{} {} ({}B) tag=0x{:08X}",
                        i,
                        rec.level,
                        tag_name,
                        rec.data.len(),
                        tag
                    );
                    pos += 36;
                }
            }
        }
    }
}

/// 빈 문단에서 반복 Enter + getCursorRect 동작 검증
#[test]
fn test_repeated_enter_on_empty_paragraph() {
    let bytes = std::fs::read("saved/blank2010.hwp").expect("blank2010.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    // 1. 텍스트 입력
    let result = doc.insert_text_native(0, 0, 0, "테스트").unwrap();
    println!("Insert: {}", result);

    // 2. 첫 번째 Enter (텍스트 끝에서)
    let result1 = doc.split_paragraph_native(0, 0, 3, None).unwrap();
    println!("Split 1 (para=0, offset=3): {}", result1);
    assert!(result1.contains("\"ok\":true"));
    assert_eq!(doc.document.sections[0].paragraphs.len(), 2);

    // getCursorRect para 1, offset 0
    let rect1 = doc.get_cursor_rect_native(0, 1, 0);
    println!("CursorRect(0,1,0): {:?}", rect1);
    assert!(
        rect1.is_ok(),
        "빈 문단(para=1) 커서 실패: {:?}",
        rect1.err()
    );

    // 3. 두 번째 Enter (빈 문단에서)
    let result2 = doc.split_paragraph_native(0, 1, 0, None).unwrap();
    println!("Split 2 (para=1, offset=0): {}", result2);
    assert!(result2.contains("\"ok\":true"));
    assert!(result2.contains("\"paraIdx\":2"));
    assert_eq!(doc.document.sections[0].paragraphs.len(), 3);

    let rect2 = doc.get_cursor_rect_native(0, 2, 0);
    println!("CursorRect(0,2,0): {:?}", rect2);
    assert!(
        rect2.is_ok(),
        "빈 문단(para=2) 커서 실패: {:?}",
        rect2.err()
    );

    // 4. 세 번째 Enter
    let result3 = doc.split_paragraph_native(0, 2, 0, None).unwrap();
    println!("Split 3 (para=2, offset=0): {}", result3);
    assert!(result3.contains("\"ok\":true"));

    let rect3 = doc.get_cursor_rect_native(0, 3, 0);
    println!("CursorRect(0,3,0): {:?}", rect3);
    assert!(
        rect3.is_ok(),
        "빈 문단(para=3) 커서 실패: {:?}",
        rect3.err()
    );

    // y좌표 순증 검증
    let parse_y = |json: &str| -> f64 {
        let y_start = json.find("\"y\":").unwrap() + 4;
        let y_end = json[y_start..]
            .find(|c: char| c == ',' || c == '}')
            .unwrap();
        json[y_start..y_start + y_end].parse::<f64>().unwrap()
    };
    let y1 = parse_y(&rect1.unwrap());
    let y2 = parse_y(&rect2.unwrap());
    let y3 = parse_y(&rect3.unwrap());
    println!("y좌표: y1={:.1}, y2={:.1}, y3={:.1}", y1, y2, y3);
    assert!(y2 > y1, "para2 y({:.1}) > para1 y({:.1})", y2, y1);
    assert!(y3 > y2, "para3 y({:.1}) > para2 y({:.1})", y3, y2);
}

/// 강제 줄바꿈(\n) 삽입 후 getCursorRect가 두 번째 줄 좌표를 반환하는지 검증
#[test]
fn test_cursor_rect_after_line_break() {
    let bytes = std::fs::read("saved/blank2010.hwp").expect("blank2010.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    // "가나다라마바" 입력
    doc.insert_text_native(0, 0, 0, "가나다라마바").unwrap();

    // offset 3에 \n 삽입 → "가나다\n라마바"
    doc.insert_text_native(0, 0, 3, "\n").unwrap();

    // offset 3 → \n 이전 (첫 줄)
    let rect_before = doc.get_cursor_rect_native(0, 0, 3);
    assert!(
        rect_before.is_ok(),
        "offset 3 커서 실패: {:?}",
        rect_before.err()
    );

    // offset 4 → \n 이후 (두 번째 줄)
    let rect_after = doc.get_cursor_rect_native(0, 0, 4);
    assert!(
        rect_after.is_ok(),
        "offset 4 커서 실패: {:?}",
        rect_after.err()
    );

    let parse_y = |json: &str| -> f64 {
        let y_start = json.find("\"y\":").unwrap() + 4;
        let y_end = json[y_start..]
            .find(|c: char| c == ',' || c == '}')
            .unwrap();
        json[y_start..y_start + y_end].parse::<f64>().unwrap()
    };
    let y_before = parse_y(&rect_before.unwrap());
    let y_after = parse_y(&rect_after.unwrap());
    assert!(
        y_after > y_before,
        "줄바꿈 후 커서 y({:.1})가 줄바꿈 전 y({:.1})보다 커야 함",
        y_after,
        y_before
    );
}

/// 텍스트 끝에 \n 삽입 후 빈 두 번째 줄에서 getCursorRect 검증
#[test]
fn test_cursor_rect_after_line_break_at_end() {
    let bytes = std::fs::read("saved/blank2010.hwp").expect("blank2010.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    // "가나다라" 입력 후 끝에 \n 삽입 → "가나다라\n"
    doc.insert_text_native(0, 0, 0, "가나다라").unwrap();
    doc.insert_text_native(0, 0, 4, "\n").unwrap();

    let para = &doc.document.sections[0].paragraphs[0];
    assert!(para.line_segs.len() >= 2, "line_segs가 2개 이상이어야 함");

    // composed lines 순서 검증: 첫 줄=텍스트, 둘째 줄=빈 줄
    let comp = &doc.composed[0][0];
    assert_eq!(comp.lines.len(), 2);
    assert!(
        comp.lines[0].has_line_break,
        "첫 줄에 line_break 플래그 있어야 함"
    );
    assert_eq!(comp.lines[1].runs.len(), 0, "둘째 줄은 빈 줄이어야 함");

    // offset 4 → \n 위치 (첫 줄 끝)
    let rect_at_newline = doc.get_cursor_rect_native(0, 0, 4);
    assert!(rect_at_newline.is_ok());

    // offset 5 → \n 직후, 빈 두 번째 줄
    let rect_after = doc.get_cursor_rect_native(0, 0, 5);
    assert!(
        rect_after.is_ok(),
        "빈 줄 offset 5 커서 실패: {:?}",
        rect_after.err()
    );

    let parse_y = |json: &str| -> f64 {
        let y_start = json.find("\"y\":").unwrap() + 4;
        let y_end = json[y_start..]
            .find(|c: char| c == ',' || c == '}')
            .unwrap();
        json[y_start..y_start + y_end].parse::<f64>().unwrap()
    };
    let y_newline = parse_y(&rect_at_newline.unwrap());
    let y_after = parse_y(&rect_after.unwrap());
    assert!(
        y_after > y_newline,
        "빈 줄 커서 y({:.1})가 첫 줄 y({:.1})보다 커야 함",
        y_after,
        y_newline
    );
}

// ── Event Sourcing + Batch Mode 테스트 ──

#[test]
fn test_superscript_in_new_document() {
    // 새 문서 생성 → 텍스트 입력 → 숫자 삽입 → 위첨자 적용 → 이후 글자 정상 확인
    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document_native().unwrap();

    // 1. "가나다라마바사" 입력 (실제로는 한 번에 삽입)
    let _ = doc.insert_text_native(0, 0, 0, "가나다라마바사");

    let para = &doc.document.sections[0].paragraphs[0];
    eprintln!(
        "Step1: text='{}' char_offsets={:?} char_shapes={:?}",
        para.text,
        para.char_offsets,
        para.char_shapes
            .iter()
            .map(|cs| (cs.start_pos, cs.char_shape_id))
            .collect::<Vec<_>>(),
    );

    // 2. 위치 2에 "123" 삽입 → "가나123다라마바사"
    let _ = doc.insert_text_native(0, 0, 2, "123");

    let para = &doc.document.sections[0].paragraphs[0];
    eprintln!(
        "Step2: text='{}' char_offsets={:?} char_shapes={:?}",
        para.text,
        para.char_offsets,
        para.char_shapes
            .iter()
            .map(|cs| (cs.start_pos, cs.char_shape_id))
            .collect::<Vec<_>>(),
    );

    // 3. "123" (chars 2-5)에 위첨자 적용
    let result = doc.apply_char_format_native(0, 0, 2, 5, r#"{"superscript":true}"#);
    assert!(result.is_ok(), "위첨자 적용 실패: {:?}", result.err());

    let para = &doc.document.sections[0].paragraphs[0];
    eprintln!(
        "Step3: text='{}' char_offsets={:?} char_shapes={:?}",
        para.text,
        para.char_offsets,
        para.char_shapes
            .iter()
            .map(|cs| (cs.start_pos, cs.char_shape_id))
            .collect::<Vec<_>>(),
    );

    // 검증: char_shapes가 3개여야 함 (원본, 위첨자, 원본)
    assert!(
        para.char_shapes.len() >= 3,
        "char_shapes should have at least 3 segments, got {}: {:?}",
        para.char_shapes.len(),
        para.char_shapes
            .iter()
            .map(|cs| (cs.start_pos, cs.char_shape_id))
            .collect::<Vec<_>>(),
    );

    // 위첨자가 적용된 CharShape와 원본 CharShape가 다른 ID인지 확인
    let original_id = para.char_shapes[0].char_shape_id;
    let superscript_id = para.char_shapes[1].char_shape_id;
    assert_ne!(
        original_id, superscript_id,
        "위첨자 CharShape ID는 원본과 달라야 함"
    );

    // 마지막 세그먼트는 원본 ID로 복원되어야 함
    let last_id = para.char_shapes.last().unwrap().char_shape_id;
    assert_eq!(last_id, original_id, "위첨자 이후 원본 ID로 복원되어야 함");

    // 위첨자 CharShape의 superscript 필드 확인
    let sup_cs = &doc.document.doc_info.char_shapes[superscript_id as usize];
    assert!(
        sup_cs.superscript,
        "위첨자 CharShape의 superscript가 true여야 함"
    );

    // 원본 CharShape의 superscript 필드 확인
    let orig_cs = &doc.document.doc_info.char_shapes[original_id as usize];
    assert!(
        !orig_cs.superscript,
        "원본 CharShape의 superscript가 false여야 함"
    );
}




#[test]
fn diag_raw_tail_dump() {
    for path in &[
        "samples/field-01.hwp",
        "samples/field-01-memo.hwp",
        "saved/field-01-h.hwp",
        "saved/field-10.hwp",
        "saved/field-10-2010.hwp",
    ] {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("[SKIP] {}", path);
                continue;
            }
        };
        let doc = HwpDocument::from_bytes(&data).expect("파싱");
        eprintln!("\n=== {} ===", path);
        for (si, sec) in doc.document.sections.iter().enumerate() {
            fn check_para(si: usize, loc: &str, para: &crate::model::paragraph::Paragraph) {
                for ctrl in &para.controls {
                    if let crate::model::control::Control::Field(f) = ctrl {
                        eprintln!("  [sec={} {}] field_type={:?} ctrl_id=0x{:08x} field_id=0x{:08x} memo_index={:02x?}",
                                si, loc, f.field_type, f.ctrl_id, f.field_id,
                                f.memo_index);
                    }
                }
            }
            for (pi, para) in sec.paragraphs.iter().enumerate() {
                check_para(si, &format!("para={}", pi), para);
                for ctrl in &para.controls {
                    match ctrl {
                        crate::model::control::Control::Table(t) => {
                            for (ci, cell) in t.cells.iter().enumerate() {
                                for (cpi, cp) in cell.paragraphs.iter().enumerate() {
                                    check_para(si, &format!("tbl cell={} para={}", ci, cpi), cp);
                                }
                            }
                        }
                        crate::model::control::Control::Shape(s) => {
                            if let Some(tb) = s.drawing().and_then(|d| d.text_box.as_ref()) {
                                for (tpi, tp) in tb.paragraphs.iter().enumerate() {
                                    check_para(si, &format!("shape para={}", tpi), tp);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[test]
fn diag_memo_controls() {
    for path in &["samples/field-01.hwp", "samples/field-01-memo.hwp"] {
        let data = std::fs::read(path).expect("read");
        let doc = HwpDocument::from_bytes(&data).expect("parse");
        eprintln!("\n=== {} ===", path);
        for (si, sec) in doc.document.sections.iter().enumerate() {
            for (pi, para) in sec.paragraphs.iter().enumerate() {
                if para.controls.is_empty() {
                    continue;
                }
                eprint!("  [sec={} para={}] controls:", si, pi);
                for ctrl in &para.controls {
                    let name = match ctrl {
                        crate::model::control::Control::SectionDef(_) => "SectionDef",
                        crate::model::control::Control::ColumnDef(_) => "ColumnDef",
                        crate::model::control::Control::Table(_) => "Table",
                        crate::model::control::Control::Shape(_) => "Shape",
                        crate::model::control::Control::Picture(_) => "Picture",
                        crate::model::control::Control::Header(_) => "Header",
                        crate::model::control::Control::Footer(_) => "Footer",
                        crate::model::control::Control::Footnote(_) => "Footnote",
                        crate::model::control::Control::Endnote(_) => "Endnote",
                        crate::model::control::Control::AutoNumber(_) => "AutoNumber",
                        crate::model::control::Control::NewNumber(_) => "NewNumber",
                        crate::model::control::Control::PageNumberPos(_) => "PageNumPos",
                        crate::model::control::Control::PageHide(_) => "PageHide",
                        crate::model::control::Control::Bookmark(_) => "Bookmark",
                        crate::model::control::Control::Hyperlink(_) => "Hyperlink",
                        crate::model::control::Control::Ruby(_) => "Ruby",
                        crate::model::control::Control::CharOverlap(_) => "CharOverlap",
                        crate::model::control::Control::HiddenComment(_) => "HiddenComment",
                        crate::model::control::Control::Field(f) => {
                            eprint!(" Field({:?},id=0x{:08x},props=0x{:08x},extra=0x{:02x},memo={},guide={:?},memo_text={:?})",
                                    f.field_type, f.field_id, f.properties, f.extra_properties, f.memo_index,
                                    f.guide_text(), f.memo_text());
                            continue;
                        }
                        crate::model::control::Control::Equation(_) => "Equation",
                        crate::model::control::Control::Form(_) => "Form",
                        crate::model::control::Control::Unknown(u) => {
                            eprint!(" Unknown(0x{:08x})", u.ctrl_id);
                            continue;
                        }
                    };
                    eprint!(" {}", name);
                }
                eprintln!();
                // field_ranges 정보
                let chars: Vec<char> = para.text.chars().collect();
                for (fri, fr) in para.field_ranges.iter().enumerate() {
                    let field_text: String =
                        if fr.start_char_idx < fr.end_char_idx && fr.end_char_idx <= chars.len() {
                            chars[fr.start_char_idx..fr.end_char_idx].iter().collect()
                        } else {
                            String::new()
                        };
                    eprintln!(
                        "    field_range[{}]: ctrl_idx={} start={} end={} text={:?}",
                        fri, fr.control_idx, fr.start_char_idx, fr.end_char_idx, field_text
                    );
                }
                eprintln!(
                    "    para.text({} chars): {:?}",
                    chars.len(),
                    &para.text[..para.text.len().min(80)]
                );
            }
        }
    }
}

/// 13페이지 엔터 후 페이지 전파 범위 분석
#[test]
fn test_page13_enter_propagation() {
    use crate::renderer::pagination::PageItem;

    let bytes = std::fs::read("samples/kps-ai.hwp").expect("kps-ai.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    let pages_before = doc.pagination[0].pages.len();

    // 분할 전: 각 페이지의 첫 번째/마지막 아이템의 para_index 기록
    let mut before_pages: Vec<(usize, usize, usize)> = Vec::new(); // (first_pi, last_pi, item_count)
    for page in &doc.pagination[0].pages {
        let items = &page.column_contents[0].items;
        let first = items.first().map(PageItem::para_index).unwrap_or(0);
        let last = items.last().map(PageItem::para_index).unwrap_or(0);
        before_pages.push((first, last, items.len()));
    }

    // page 13 (idx=12)의 pi=199 앞에서 엔터
    eprintln!("=== splitParagraph(0, 199, 0) ===");
    let result = doc.split_paragraph_native(0, 199, 0, None).unwrap();
    assert!(result.contains("\"ok\":true"));

    let pages_after = doc.pagination[0].pages.len();
    eprintln!("pages: {} → {}", pages_before, pages_after);

    // 분할 후: 각 페이지 비교
    let mut last_diff_page = 0;
    for (pidx, page) in doc.pagination[0].pages.iter().enumerate() {
        let items = &page.column_contents[0].items;
        let first = items.first().map(PageItem::para_index).unwrap_or(0);
        let last = items.last().map(PageItem::para_index).unwrap_or(0);

        let before = before_pages.get(pidx);
        let changed = before
            .map(|b| b.0 != first || b.1 != last || b.2 != items.len())
            .unwrap_or(true);

        if changed {
            last_diff_page = pidx;
            let before_str = before
                .map(|b| format!("pi={}-{} ({}items)", b.0, b.1, b.2))
                .unwrap_or_else(|| "(신규)".to_string());
            eprintln!(
                "  page {:2}: {} → pi={}-{} ({}items) ← CHANGED",
                pidx + 1,
                before_str,
                first,
                last,
                items.len()
            );
        }
    }
    eprintln!(
        "전파 범위: page 13 ~ page {} (총 {} 페이지 영향)",
        last_diff_page + 1,
        last_diff_page + 1 - 12
    );

    // 저장 후 재로드와 비교
    eprintln!("\n=== 저장 후 재로드 비교 ===");
    let exported = doc.export_hwp_native().unwrap();
    let mut doc2 = HwpDocument::from_bytes(&exported).unwrap();
    doc2.convert_to_editable_native().unwrap();
    doc2.paginate();

    let pages_reload = doc2.pagination[0].pages.len();
    eprintln!("재로드 pages: {}", pages_reload);

    let mut diff_count = 0;
    for pidx in 0..doc.pagination[0]
        .pages
        .len()
        .max(doc2.pagination[0].pages.len())
    {
        let items1 = doc.pagination[0]
            .pages
            .get(pidx)
            .map(|p| &p.column_contents[0].items);
        let items2 = doc2.pagination[0]
            .pages
            .get(pidx)
            .map(|p| &p.column_contents[0].items);

        let pi1_first = items1.and_then(|i| i.first()).map(PageItem::para_index);
        let pi2_first = items2.and_then(|i| i.first()).map(PageItem::para_index);
        let count1 = items1.map(|i| i.len()).unwrap_or(0);
        let count2 = items2.map(|i| i.len()).unwrap_or(0);

        if pi1_first != pi2_first || count1 != count2 {
            diff_count += 1;
            eprintln!(
                "  page {:2}: 편집={:?}({}items) vs 재로드={:?}({}items)",
                pidx + 1,
                pi1_first,
                count1,
                pi2_first,
                count2
            );
        }
    }
    if diff_count == 0 {
        eprintln!("  차이 없음 — 편집 결과와 재로드 결과 일치");
    } else {
        eprintln!("  {} 페이지에서 차이 발견", diff_count);
    }
}

/// 문단 분할 후 페이지 수가 과도하게 증가하지 않는지 검증
/// (measure_section_selective의 off-by-one 인덱싱 버그 회귀 방지)
#[test]
fn test_split_paragraph_page_count_stability() {
    let bytes = std::fs::read("samples/kps-ai.hwp").expect("kps-ai.hwp 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&bytes).unwrap();
    doc.convert_to_editable_native().unwrap();
    doc.paginate();

    let pages_before = doc.pagination.iter().map(|r| r.pages.len()).sum::<usize>();
    eprintln!("  pages_before = {}", pages_before);

    // pi=199 앞에서 엔터 (offset=0으로 분할)
    let result = doc.split_paragraph_native(0, 199, 0, None).unwrap();
    assert!(result.contains("\"ok\":true"), "split failed: {}", result);

    let pages_after = doc.pagination.iter().map(|r| r.pages.len()).sum::<usize>();
    eprintln!("  pages_after = {}", pages_after);

    // 한 줄 추가이므로 페이지 수 증가는 최대 2 이내여야 함
    let delta = pages_after as i64 - pages_before as i64;
    eprintln!("  delta = {}", delta);
    assert!(
        delta <= 2,
        "문단 분할 후 페이지 수가 {}에서 {}로 {}만큼 증가 (최대 2 예상)",
        pages_before,
        pages_after,
        delta
    );
}

#[test]
fn test_extract_thumbnail_with_preview() {
    // PrvImage가 있는 HWP 파일 테스트
    let data = std::fs::read("samples/biz_plan.hwp").expect("biz_plan.hwp 읽기 실패");
    let result = crate::parser::extract_thumbnail_only(&data);
    if let Some(ref r) = result {
        eprintln!(
            "  biz_plan.hwp 썸네일: format={}, size={}bytes, {}x{}",
            r.format,
            r.data.len(),
            r.width,
            r.height
        );
        eprintln!(
            "  매직 바이트: {:02x?}",
            &r.data[..std::cmp::min(16, r.data.len())]
        );
    } else {
        eprintln!("  biz_plan.hwp 썸네일: None");
    }
    // PrvImage 유무와 상관없이 패닉하지 않아야 함
}

#[test]
fn test_extract_thumbnail_without_preview() {
    // 잘못된 데이터에서는 None 반환
    let result = crate::parser::extract_thumbnail_only(&[0u8; 100]);
    assert!(result.is_none(), "잘못된 데이터에서는 None이어야 함");

    // 빈 바이트에서도 패닉하지 않아야 함
    let result = crate::parser::extract_thumbnail_only(&[]);
    assert!(result.is_none(), "빈 데이터에서는 None이어야 함");
    eprintln!("  잘못된/빈 데이터 썸네일: None (정상)");
}

// ---------- #177: getValidationWarnings / reflowLinesegs WASM API ----------

#[test]
fn test_get_validation_warnings_empty_document() {
    // 빈 문서는 경고 없음.
    let doc = HwpDocument::create_empty();
    let json = doc.get_validation_warnings();
    assert!(
        json.contains(r#""count":0"#),
        "empty doc must have count:0, got: {}",
        json
    );
    assert!(json.contains(r#""warnings":[]"#));
}

#[test]
fn test_get_validation_warnings_json_shape() {
    // JSON 구조 검증 — 빈 문서라도 최소 형태를 갖춰야 함.
    let doc = HwpDocument::create_empty();
    let json = doc.get_validation_warnings();
    // 필수 키: count, summary, warnings
    assert!(json.contains(r#""count":"#));
    assert!(json.contains(r#""summary":"#));
    assert!(json.contains(r#""warnings":"#));
}

#[test]
fn test_reflow_linesegs_empty_document_returns_zero() {
    // 빈 문서에선 reflow 대상 없음 → 0 반환.
    let mut doc = HwpDocument::create_empty();
    let count = doc.reflow_linesegs();
    assert_eq!(count, 0);
}

#[test]
fn test_create_blank_document_clears_previous_hwpx_validation_warnings() {
    let bytes = std::fs::read("samples/hwpx_sample2.hwpx").expect("HWPX 샘플 읽기");
    let mut doc = HwpDocument::new(&bytes).expect("HWPX 샘플 로드");

    let before: Value = serde_json::from_str(&doc.get_validation_warnings()).expect("경고 JSON");
    assert!(
        before["count"].as_u64().unwrap_or(0) > 0,
        "재현 샘플은 HWPX validation warning이 있어야 함: {before}"
    );
    assert_eq!(doc.get_source_format(), "hwpx");

    doc.create_blank_document_native()
        .expect("새 문서 생성 성공");

    let after: Value = serde_json::from_str(&doc.get_validation_warnings()).expect("경고 JSON");
    assert_eq!(
        after["count"].as_u64(),
        Some(0),
        "새 문서는 이전 HWPX warning을 물려받으면 안 됨: {after}"
    );
    assert_eq!(doc.get_source_format(), "hwp");
}

#[test]
fn test_reflow_linesegs_keeps_hwpx_sample2_page_count_for_textrun_warnings() {
    let bytes = std::fs::read("samples/hwpx_sample2.hwpx").expect("HWPX 샘플 읽기");
    let mut doc = HwpDocument::new(&bytes).expect("HWPX 샘플 로드");

    let before_page_count = doc.page_count();
    let before: Value = serde_json::from_str(&doc.get_validation_warnings()).expect("경고 JSON");
    assert_eq!(before_page_count, 29);
    assert_eq!(before["count"].as_u64(), Some(151));

    let reflowed = doc.reflow_linesegs();

    assert_eq!(
        reflowed, 0,
        "LinesegTextRunReflow 경고는 페이지 수를 바꿀 수 있어 자동 보정하지 않음"
    );
    assert_eq!(
        doc.page_count(),
        before_page_count,
        "권장 보정으로 HWPX 페이지 수가 바뀌면 안 됨"
    );
}

// ---------- #1413: insertPictureEx(options object) 동치 ----------



#[test]
fn local_body_replace_applies_ime_replacement_as_one_final_state() {
    let mut doc = HwpDocument::create_empty();
    doc.replace_body_text_local_native(0, 0, 0, 0, "ㅎ")
        .expect("initial composition");
    let raw = doc
        .replace_body_text_local_native(0, 0, 0, 1, "하")
        .expect("composition replacement");
    let result: Value = serde_json::from_str(&raw).expect("replace result json");

    assert_eq!(result["charOffset"].as_u64(), Some(1));
    assert_eq!(
        doc.get_text_range_native(0, 0, 0, 1)
            .expect("final composition"),
        "하"
    );
}

#[test]
fn local_body_replace_paginates_immediately_at_flow_boundary() {
    let mut doc = HwpDocument::create_empty();
    let mut boundary = None;

    for offset in 0..512 {
        let raw = doc
            .replace_body_text_local_native(0, 0, offset, 0, "가")
            .expect("sequential local insert");
        let result: Value = serde_json::from_str(&raw).expect("flow result json");
        if result["flowChanged"].as_bool() == Some(true) {
            boundary = Some(result);
            break;
        }
    }

    let result = boundary.expect("a body line-flow boundary within 512 characters");
    assert_eq!(result["documentPaginationPending"].as_bool(), Some(false));
    assert_eq!(result["flowChanged"].as_bool(), Some(true));
    assert_eq!(
        doc.page_count(),
        doc.pagination
            .iter()
            .map(|section| section.pages.len())
            .sum::<usize>() as u32
    );
}

// ─── Alt(local resize) 조절 셀이 Ctrl(칸 전체 delta) 조절을 따라오는지 ─────────
//
// Alt+방향키(localResize + render 힌트)로 조절한
// 행은 `local_resize_cell_widths` 에 절대값 override 를 갖는다. 이후 Ctrl+방향키
// (plain widthDelta)가 칸 전체를 조절하면 cell.width 는 움직이는데 override 는
// 그대로 남아 — Alt 만진 행만 옛 경계에 얼어붙고 나머지 칸이 움직인다("따로 논다").
// 계약: plain delta 가 override 를 가진 셀에 적용되면 override 도 같은 양만큼
// 이동해야 한다 (높이 동일).

#[test]
fn local_resize_override_follows_plain_width_delta() {
    let mut doc = HwpDocument::create_empty();
    let table_result = doc.create_table_native(0, 0, 0, 3, 3).expect("표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    // 대상 셀: row1 col1 (cellIdx 는 행 우선)
    let (target_idx, base_width) = {
        let table = issue_1481_table(&doc, table_para_idx);
        let (idx, cell) = table
            .cells
            .iter()
            .enumerate()
            .find(|(_, c)| c.row == 1 && c.col == 1)
            .expect("row1col1");
        (idx, cell.width)
    };

    // 1) Alt 상당: localResize + renderWidth override 등록
    let render_w = base_width + 900;
    doc.resize_table_cells_native(
        0,
        table_para_idx,
        0,
        &format!(
            r#"[{{"cellIdx":{target_idx},"widthDelta":900,"localResize":true,"renderWidth":{render_w}}}]"#
        ),
    )
    .expect("local resize");
    {
        let table = issue_1481_table(&doc, table_para_idx);
        let (_, w) = table
            .local_resize_cell_widths
            .iter()
            .find(|(idx, _)| *idx == target_idx)
            .expect("override 등록");
        assert_eq!(*w, render_w);
    }

    // 2) Ctrl 상당: 같은 칸(col1) 전체에 plain widthDelta +600
    let col_updates = {
        let table = issue_1481_table(&doc, table_para_idx);
        table
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.col == 1 && c.col_span == 1)
            .map(|(idx, _)| format!(r#"{{"cellIdx":{idx},"widthDelta":600}}"#))
            .collect::<Vec<_>>()
            .join(",")
    };
    doc.resize_table_cells_native(0, table_para_idx, 0, &format!("[{col_updates}]"))
        .expect("칸 전체 resize");

    let table = issue_1481_table(&doc, table_para_idx);
    let (_, w) = table
        .local_resize_cell_widths
        .iter()
        .find(|(idx, _)| *idx == target_idx)
        .expect("override 유지");
    assert_eq!(
        *w,
        render_w + 600,
        "plain widthDelta 가 override 를 가진 셀에 적용되면 override 도 같은 양만큼 \
         이동해야 한다 — 아니면 Alt 조절 행만 옛 경계에 얼어붙는다"
    );
}

#[test]
fn local_resize_override_follows_plain_height_delta() {
    // 폭 테스트(local_resize_override_follows_plain_width_delta)의 높이 대칭.
    let mut doc = HwpDocument::create_empty();
    let table_result = doc.create_table_native(0, 0, 0, 3, 3).expect("표 생성");
    let table_para_idx = issue_1481_json_usize(&table_result, "paraIdx");

    let (target_idx, base_height) = {
        let table = issue_1481_table(&doc, table_para_idx);
        let (idx, cell) = table
            .cells
            .iter()
            .enumerate()
            .find(|(_, c)| c.row == 1 && c.col == 1)
            .expect("row1col1");
        (idx, cell.height)
    };

    let render_h = base_height + 900;
    doc.resize_table_cells_native(
        0,
        table_para_idx,
        0,
        &format!(
            r#"[{{"cellIdx":{target_idx},"heightDelta":900,"localResize":true,"renderHeight":{render_h}}}]"#
        ),
    )
    .expect("local resize");

    let row_updates = {
        let table = issue_1481_table(&doc, table_para_idx);
        table
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.row == 1 && c.row_span == 1)
            .map(|(idx, _)| format!(r#"{{"cellIdx":{idx},"heightDelta":600}}"#))
            .collect::<Vec<_>>()
            .join(",")
    };
    doc.resize_table_cells_native(0, table_para_idx, 0, &format!("[{row_updates}]"))
        .expect("줄 전체 resize");

    let table = issue_1481_table(&doc, table_para_idx);
    let (_, h) = table
        .local_resize_cell_heights
        .iter()
        .find(|(idx, _)| *idx == target_idx)
        .expect("override 유지");
    assert_eq!(
        *h,
        render_h + 600,
        "높이 override 도 plain delta 를 따라와야 한다 (폭과 대칭)"
    );
}

// ─── 표 나누기 / 표 붙이기 (한컴 table(dividing).htm / table(attach).htm) ────
//
// 나누기: 커서 행부터 새 표로 분리. 첫 행에서는 불가. 뒤 표는 앞 표 속성 상속.
// 붙이기: 다음 표를 현재 표 뒤에 이어 붙임. 사이에 내용 문단이 있으면 거부.
//         칸 수가 달라도 붙는다 (한컴 명세).

/// 회귀 재현: 병합 이력이 있는 표에서 delete_row()를 두 번 연속 호출하면
/// (3-row-span 병합 → 2 → 1) 살아남은 행의 height가 하이라인(~5px)으로
/// 붕괴할 수 있다. merge_cells()가 자동맞춤(height==0) 행들을 여러 행에
/// 걸쳐 병합할 때 get_raw_row_heights()의 fallback-없는 합을 그대로 구워
/// 병합 셀 height가 0이 되고, delete_row()의 두 row_span 축소 루프가 이를
/// 비례 조정하지 않아 이 stale 0이 row_span==1에 도달할 때까지(3-row-span
/// 병합이면 delete_row 2회 후) 그대로 남기 때문이다.
#[test]
fn double_delete_row_after_vertical_merge_preserves_height() {
    let mut doc = HwpDocument::create_empty();
    // 단일 열: "다른 열이 max()로 가려주는" 탈출구를 제거해 버그를 노출시킨다.
    let created = doc.create_table_native(0, 0, 0, 3, 1).expect("표 생성");
    let para_idx = issue_1481_json_usize(&created, "paraIdx");

    // HWPX에서 가져온 "자동 맞춤" 표를 재현: 모든 셀 height를 0으로 강제
    // (create_table_native는 항상 0이 아닌 height를 채우므로 직접 주입해야 한다).
    match &mut doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            for cell in &mut table.cells {
                cell.height = 0;
            }
        }
        _ => panic!("target table"),
    }

    // 3행을 모두 하나의 셀로 세로 병합.
    doc.merge_table_cells_native(0, para_idx, 0, 0, 0, 2, 0)
        .expect("세로 병합");
    match &doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            assert_eq!(table.cells.len(), 1, "병합 후 셀은 1개");
            assert_eq!(table.cells[0].row_span, 3, "사전 조건: row_span==3");
        }
        _ => panic!("target table"),
    }

    // 행 삭제를 두 번 연속 수행 (row_span: 3 -> 2 -> 1).
    doc.delete_table_row_native(0, para_idx, 0, 0)
        .expect("첫 번째 행 삭제");
    doc.delete_table_row_native(0, para_idx, 0, 0)
        .expect("두 번째 행 삭제");

    match &doc.document.sections[0].paragraphs[para_idx].controls[0] {
        Control::Table(table) => {
            assert_eq!(table.row_count, 1, "행 1개만 남아야 한다");
            assert_eq!(table.cells.len(), 1);
            assert_eq!(table.cells[0].row_span, 1, "row_span이 1로 축소되어야 한다");
            assert!(
                table.cells[0].height >= 400,
                "살아남은 셀 height가 하이라인으로 붕괴하면 안 된다: {}",
                table.cells[0].height
            );
            assert!(
                table.get_row_heights()[0] >= 400,
                "get_row_heights()도 하이라인이면 안 된다: {:?}",
                table.get_row_heights()
            );
        }
        _ => panic!("target table"),
    }
}

