//! tests — json.rs 에서 무변동 이동
use super::*;
use crate::model::shape::TextWrap;
use crate::paint::{
    font_blob_resource_key, resource_digest_hex, BinaryResourceKind, BinaryResourceRef,
    BitmapGlyphFiltering, BitmapGlyphPayload, BitmapGlyphScalingPolicy, CacheHint, ClipKind,
    ColorGlyphFormat, ColorLayersPayload, ColorPaintGraphNode, ColorPaintGraphNodeKind,
    ColorPaintGraphPayload, ColorPaintSolidPathNode, FontBlobKey, FontBlobResource,
    FontColorGlyphRef, FontDigest, FontFaceKey, FontFaceResource, FontFallbackPolicyId,
    FontInstanceKey, FontPortability, FontResourceSource, GlyphCluster, GlyphOutlineFillRule,
    GlyphOutlinePayloadKind, GlyphOutlineStrokeCap, GlyphOutlineStrokeJoin,
    GlyphOutlineStrokeStyle, GlyphRange, GlyphRunDiagnostics, GlyphRunOrientation,
    GlyphRunReplayEligibility, GroupKind, ImageResourceId, LayerAffineTransform,
    LayerGlyphOutlinePaint, LayerGlyphOutlinePath, LayerGlyphRunPaint, LayerNode, LayerPoint,
    LayerVector, PageLayerTree, PaintTextStyle, PaintVariantMeta, ResolvedColor, ScriptTag,
    ShapeKey, ShapingEngineId, SvgGlyphPayload, SvgResourceId, TextDecorationKind,
    TextDirection, TextSourceId, TextSourceRange, TextSourceSpan, TextVariantKind,
    TextVariantQuality, WritingMode, RESOURCE_KEY_ALGORITHM,
};
use crate::renderer::composer::CharOverlapInfo;
use crate::renderer::equation::layout::{LayoutBox, LayoutKind};
use crate::renderer::render_tree::{
    EquationNode, FieldMarkerType, ImageNode, PageBackgroundImage, PageBackgroundNode,
    PathNode, PlaceholderNode, RawSvgNode, RenderLayerInfo, TextRunNode,
};
use serde_json::Value;

const FIXTURE_TTC: &[u8] = include_bytes!("../../../tests/fixtures/fonts/RHWPExactFaceSmoke.ttc");

#[test]
fn serializes_text_and_shape_ops_for_browser_replay() {
    let text = PaintOp::text_run(
        BoundingBox::new(10.0, 20.0, 80.0, 18.0),
        TextRunNode {
            text: "가A".to_string(),
            style: TextStyle {
                font_family: "Noto Sans KR".to_string(),
                font_size: 16.0,
                color: 0x00010203,
                bold: true,
                italic: true,
                underline: UnderlineType::Bottom,
                shade_color: 0x0000FFFF,
                emphasis_dot: 2,
                ..Default::default()
            },
            char_shape_id: None,
            para_shape_id: None,
            section_index: None,
            para_index: None,
            char_start: None,
            cell_context: None,
            is_para_end: true,
            is_line_break_end: true,
            rotation: 0.0,
            is_vertical: false,
            char_overlap: Some(CharOverlapInfo {
                border_type: 1,
                inner_char_size: 90,
            }),
            border_fill_id: 0,
            baseline: 13.0,
            field_marker: FieldMarkerType::FieldBegin,
            display_text: None,
        },
    );
    let rect = PaintOp::rectangle(
        BoundingBox::new(8.0, 18.0, 84.0, 22.0),
        crate::renderer::render_tree::RectangleNode::new(
            4.0,
            ShapeStyle {
                fill_color: Some(0x00F0F1F2),
                stroke_color: Some(0x00030405),
                stroke_width: 1.5,
                ..Default::default()
            },
            None,
        ),
    );

    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![text, rect],
        ),
    );

    let json = tree.to_json();
    let positions = compute_char_positions(
        "가A",
        &TextStyle {
            font_family: "Noto Sans KR".to_string(),
            font_size: 16.0,
            color: 0x00010203,
            bold: true,
            italic: true,
            underline: UnderlineType::Bottom,
            shade_color: 0x0000FFFF,
            emphasis_dot: 2,
            ..Default::default()
        },
    );
    let positions_json = format!(
        "\"positions\":[{:.3},{:.3},{:.3}]",
        positions[0], positions[1], positions[2]
    );

    assert!(json.contains("\"kind\":\"leaf\""));
    assert!(json.contains(&format!(
        "\"schemaVersion\":{}",
        LAYER_TREE_SCHEMA.schema_version
    )));
    assert!(json.contains(&format!(
        "\"schemaMinorVersion\":{}",
        LAYER_TREE_SCHEMA.schema_minor_version
    )));
    assert!(json.contains(&format!(
        "\"schema\":{{\"major\":{},\"minor\":{}}}",
        LAYER_TREE_SCHEMA.schema_version, LAYER_TREE_SCHEMA.schema_minor_version
    )));
    assert!(json.contains(&format!(
        "\"resourceTableVersion\":{}",
        LAYER_TREE_SCHEMA.resource_table_version
    )));
    assert!(json.contains(&format!(
        "\"resourceTableMinorVersion\":{}",
        LAYER_TREE_SCHEMA.resource_table_minor_version
    )));
    assert!(json.contains(&format!(
        "\"resourceTable\":{{\"major\":{},\"minor\":{}}}",
        LAYER_TREE_SCHEMA.resource_table_version,
        LAYER_TREE_SCHEMA.resource_table_minor_version
    )));
    assert!(json.contains("\"unit\":\"px\""));
    assert!(json.contains("\"coordinateSystem\":\"page-top-left-y-down\""));
    assert!(json.contains("\"profile\":\"screen\""));
    assert!(json.contains("\"buildOptions\":{"));
    assert!(json.contains("\"debugOptions\":{"));
    assert!(json.contains("\"outputOptions\":{"));
    assert!(json.contains("\"clipEnabled\":true"));
    assert!(json.contains("\"type\":\"textRun\""));
    assert!(json.contains("\"textSources\":[{\"id\":0,\"text\":\"가A\""));
    assert!(json.contains("\"source\":{\"id\":0"));
    assert!(json.contains("\"paintStyle\":{"));
    assert!(json.contains("\"placement\":{\"runToPage\":"));
    assert!(json.contains("\"clusterBasis\":\"legacyPosition\""));
    assert!(json.contains("\"clusters\":[{\"sourceRangeUtf8\""));
    assert!(json.contains("\"legacyVisuals\":{"));
    assert!(json.contains("\"layer.optionMetadata\""));
    assert!(json.contains(&positions_json));
    assert!(!json.contains("\"displayText\""));
    assert!(!json.contains("\"displayPositions\""));
    assert!(json.contains("\"isParaEnd\":true"));
    assert!(json.contains("\"isLineBreakEnd\":true"));
    assert!(json.contains("\"fieldMarker\":{\"kind\":\"fieldBegin\"}"));
    assert!(json.contains("\"charOverlap\":{\"borderType\":1,\"innerCharSize\":90}"));
    assert!(json.contains("\"usedFeatures\":[\"text.paintStyle\""));
    assert!(json.contains("\"text.v2.diagnostics\""));
    assert!(json.contains("\"knownFeatures\":[\"fontResources\""));
    assert!(json.contains("\"fontResources\":{\"blobs\":[],\"faces\":[]}"));
    assert!(json.contains("\"textV2\":{\"compatibilityProfile\":\"v1Compat\""));
    assert!(json.contains("\"downgradePath\":\"schemaV1FlattenedTextRunAndGlyphRun\""));
    assert!(json.contains("\"text\":{\"defaultVariant\":\"textRun\""));
    assert!(json.contains("\"fontFamily\":\"Noto Sans KR\""));
    assert!(json.contains("\"italic\":true"));
    assert!(json.contains("\"shadeColor\":\"#ffff00\""));
    assert!(json.contains("\"emphasisDot\":2"));
    assert!(json.contains("\"type\":\"rectangle\""));
    assert!(json.contains("\"cornerRadius\":4.000"));
}

#[test]
fn serializes_display_text_for_pua_text_run() {
    let style = TextStyle {
        font_family: "Noto Sans KR".to_string(),
        font_size: 16.0,
        ..Default::default()
    };
    let text = "\u{F012B}(Signature)";
    let display_text = "(인)(Signature)";
    let source_positions = compute_char_positions(text, &style);
    let display_positions = compute_char_positions(display_text, &style);
    let text_run = PaintOp::text_run(
        BoundingBox::new(10.0, 20.0, 80.0, 18.0),
        TextRunNode {
            text: text.to_string(),
            style: style.clone(),
            char_shape_id: None,
            para_shape_id: None,
            section_index: None,
            para_index: None,
            char_start: None,
            cell_context: None,
            is_para_end: false,
            is_line_break_end: false,
            rotation: 0.0,
            is_vertical: false,
            char_overlap: None,
            border_fill_id: 0,
            baseline: 13.0,
            field_marker: FieldMarkerType::None,
            display_text: None,
        },
    );
    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![text_run],
        ),
    );

    let json = tree.to_json();
    let source_positions_json = format!(
        "\"positions\":[{}]",
        source_positions
            .iter()
            .map(|position| format!("{:.3}", position))
            .collect::<Vec<_>>()
            .join(",")
    );
    let display_positions_json = format!(
        "\"displayPositions\":[{}]",
        display_positions
            .iter()
            .map(|position| format!("{:.3}", position))
            .collect::<Vec<_>>()
            .join(",")
    );

    assert!(json.contains(&format!("\"text\":\"{}\"", text)));
    assert!(json.contains(&format!("\"displayText\":\"{}\"", display_text)));
    assert!(json.contains(&source_positions_json));
    assert!(json.contains(&display_positions_json));
    assert!(json.contains("\"text.displayText\""));
}

#[test]
fn serializes_empty_display_positions_for_hidden_pua_filler() {
    let text_run = PaintOp::text_run(
        BoundingBox::new(10.0, 20.0, 80.0, 18.0),
        TextRunNode {
            text: "\u{F081C}".to_string(),
            style: TextStyle {
                font_family: "Noto Sans KR".to_string(),
                font_size: 16.0,
                ..Default::default()
            },
            char_shape_id: None,
            para_shape_id: None,
            section_index: None,
            para_index: None,
            char_start: None,
            cell_context: None,
            is_para_end: false,
            is_line_break_end: false,
            rotation: 0.0,
            is_vertical: false,
            char_overlap: None,
            border_fill_id: 0,
            baseline: 13.0,
            field_marker: FieldMarkerType::None,
            display_text: None,
        },
    );
    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![text_run],
        ),
    );

    let json = tree.to_json();

    assert!(json.contains("\"displayText\":\"\""));
    assert!(json.contains("\"displayPositions\":[]"));
}

#[test]
fn serializes_external_text_visual_ops_as_additive_features() {
    let run = TextRunNode {
        text: "A\tB".to_string(),
        style: TextStyle {
            font_family: "Noto Sans".to_string(),
            font_size: 14.0,
            color: 0x00000000,
            underline: UnderlineType::Bottom,
            strikethrough: true,
            emphasis_dot: 1,
            tab_leaders: vec![TabLeaderInfo {
                start_x: 10.0,
                end_x: 40.0,
                fill_type: 3,
            }],
            ..Default::default()
        },
        char_shape_id: None,
        para_shape_id: None,
        section_index: Some(1),
        para_index: Some(2),
        char_start: Some(3),
        cell_context: None,
        is_para_end: true,
        is_line_break_end: false,
        rotation: 0.0,
        is_vertical: false,
        char_overlap: Some(CharOverlapInfo {
            border_type: 2,
            inner_char_size: 80,
        }),
        border_fill_id: 0,
        baseline: 11.0,
        field_marker: FieldMarkerType::FieldEnd,
        display_text: None,
    };
    let bbox = BoundingBox::new(10.0, 20.0, 40.0, 16.0);
    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![
                PaintOp::text_run(bbox, run.clone()),
                PaintOp::char_overlap(bbox, run.clone()),
                PaintOp::text_control_mark(bbox, run.clone()),
                PaintOp::tab_leader(bbox, run.clone()),
                PaintOp::text_decoration(bbox, run.clone(), TextDecorationKind::Underline),
                PaintOp::text_decoration(bbox, run.clone(), TextDecorationKind::EmphasisDot),
            ],
        ),
    );

    let json = tree.to_json();

    assert!(json.contains("\"type\":\"charOverlap\""));
    assert!(json.contains("\"type\":\"textControlMark\""));
    assert!(json.contains("\"type\":\"tabLeader\""));
    assert!(json.contains("\"type\":\"textDecoration\""));
    assert!(json.contains("\"kind\":\"underline\""));
    assert!(json.contains("\"kind\":\"emphasisDot\""));
    assert!(json.contains("\"textSources\":[{\"id\":0,\"text\":\"A\\tB\""));
    assert!(json.contains("\"stableSourceKey\":\"section:1/para:2/char:3\""));
    assert!(json.contains("\"marker\":\"fieldEnd\""));
    assert!(json.contains("\"text.charOverlapOp\""));
    assert!(json.contains("\"text.charOverlapOp.bounded\""));
    assert!(json.contains("\"text.controlMarkOp\""));
    assert!(json.contains("\"text.controlMarkOp.positioned\""));
    assert!(json.contains("\"text.tabLeaderOp\""));
    assert!(json.contains("\"text.tabLeaderOp.bounded\""));
    assert!(json.contains("\"text.decorationOp\""));
    assert!(json.contains("\"text.decorationOp.bounded\""));
    assert!(json.contains("\"externalizedVisuals\":[\"charOverlap\",\"controlMarks\",\"tabLeaders\",\"decorations\"]"));
    assert!(json.contains("\"legacyVisuals\":{\"charOverlap\":\"mirror\""));
    assert!(json.contains("\"controlMarks\":[{\"kind\":\"paragraphEnd\",\"text\":\"↵\",\"x\":40.000,\"y\":0.000,\"fontSize\":14.000}]"));
    assert!(json.contains("\"baseline\":11.000,\"rotation\":0.000,\"isVertical\":false,\"marks\":[{\"kind\":\"paragraphEnd\""));

    let value: Value = serde_json::from_str(&json).expect("valid layer JSON");
    let ops = value["root"]["ops"].as_array().expect("leaf ops");
    let display_text = expand_pua_display_text(&run.text);
    let positions = compute_char_positions(&display_text, &run.style);
    let expected_end = clamp_tab_leader_end_x(
        &display_text,
        &positions,
        &run.style.tab_leaders[0],
        run.style.font_size,
    );
    let expected_end = (expected_end * 1_000.0).round() / 1_000.0;
    assert_eq!(ops[0]["tabLeaders"][0]["endX"], 40.0);
    assert_eq!(ops[1]["positionsComplete"], true);
    assert_eq!(ops[3]["leaders"][0]["endX"], expected_end);
    assert_eq!(ops[3]["leadersComplete"], true);
    assert_eq!(ops[4]["decoration"]["positionsComplete"], true);
    assert_eq!(ops[5]["decoration"]["positionsComplete"], true);
}

#[test]
fn decoration_uses_display_positions_and_script_metrics() {
    let run = TextRunNode {
        text: "\u{F012B}".to_string(),
        style: TextStyle {
            font_family: "Noto Sans".to_string(),
            font_size: 20.0,
            superscript: true,
            underline: UnderlineType::Bottom,
            tab_leaders: vec![TabLeaderInfo {
                start_x: 1.0,
                end_x: 20.0,
                fill_type: 1,
            }],
            ..Default::default()
        },
        char_shape_id: None,
        para_shape_id: None,
        section_index: None,
        para_index: None,
        char_start: None,
        cell_context: None,
        is_para_end: false,
        is_line_break_end: false,
        rotation: 0.0,
        is_vertical: false,
        char_overlap: None,
        border_fill_id: 0,
        baseline: 20.0,
        field_marker: FieldMarkerType::None,
        display_text: None,
    };
    let bbox = BoundingBox::new(0.0, 0.0, 80.0, 24.0);
    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            bbox,
            None,
            vec![
                PaintOp::text_run(bbox, run.clone()),
                PaintOp::tab_leader(bbox, run.clone()),
                PaintOp::text_decoration(bbox, run, TextDecorationKind::Underline),
            ],
        ),
    );

    let value: Value = serde_json::from_str(&tree.to_json()).expect("valid layer JSON");
    let ops = value["root"]["ops"].as_array().expect("leaf ops");
    assert_eq!(ops[1]["fontSize"], 14.0);
    assert_eq!(ops[1]["baseline"], 14.0);
    assert_eq!(ops[1]["leadersComplete"], true);
    let decoration = &ops[2]["decoration"];
    assert_eq!(decoration["fontSize"], 14.0);
    assert_eq!(decoration["baseline"], 14.0);
    assert_eq!(decoration["positions"], ops[0]["displayPositions"]);
    assert_eq!(decoration["positionsComplete"], true);
}

#[test]
fn serializes_positioned_space_tab_and_line_break_control_marks() {
    let bbox = BoundingBox::new(10.0, 20.0, 80.0, 18.0);
    let run = TextRunNode {
        text: "A \t".to_string(),
        style: TextStyle {
            font_family: "Noto Sans".to_string(),
            font_size: 16.0,
            ..Default::default()
        },
        char_shape_id: None,
        para_shape_id: None,
        section_index: None,
        para_index: None,
        char_start: None,
        cell_context: None,
        is_para_end: false,
        is_line_break_end: true,
        rotation: 0.0,
        is_vertical: false,
        char_overlap: None,
        border_fill_id: 0,
        baseline: 13.0,
        field_marker: FieldMarkerType::None,
        display_text: None,
    };
    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![
                PaintOp::text_run(bbox, run.clone()),
                PaintOp::text_control_mark(bbox, run),
            ],
        ),
    );

    let value: Value = serde_json::from_str(&tree.to_json()).expect("valid layer JSON");
    let ops = value["root"]["ops"].as_array().expect("leaf ops");
    let marks = ops[1]["marks"].as_array().expect("positioned marks");
    assert_eq!(
        marks
            .iter()
            .map(|mark| mark["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["space", "tab", "lineBreakEnd"]
    );
    assert_eq!(marks[0]["text"], "∨");
    assert_eq!(marks[1]["text"], "→");
    assert_eq!(marks[2]["text"], "↓");
    assert_eq!(ops[0]["controlMarks"], ops[1]["marks"]);
}

#[test]
fn bounds_positioned_control_mark_export_and_reports_truncation() {
    let run = TextRunNode {
        text: " ".repeat(crate::paint::MAX_POSITIONED_CONTROL_MARKS_PER_RUN + 1),
        style: TextStyle {
            font_family: "Noto Sans".to_string(),
            font_size: 12.0,
            tab_leaders: vec![TabLeaderInfo {
                start_x: 1.0,
                end_x: 20.0,
                fill_type: 1,
            }],
            ..Default::default()
        },
        char_shape_id: None,
        para_shape_id: None,
        section_index: None,
        para_index: None,
        char_start: None,
        cell_context: None,
        is_para_end: false,
        is_line_break_end: false,
        rotation: 0.0,
        is_vertical: false,
        char_overlap: Some(CharOverlapInfo {
            border_type: 1,
            inner_char_size: 100,
        }),
        border_fill_id: 0,
        baseline: 10.0,
        field_marker: FieldMarkerType::None,
        display_text: None,
    };
    let bbox = BoundingBox::new(0.0, 0.0, 100.0, 14.0);
    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            bbox,
            None,
            vec![
                PaintOp::text_run(bbox, run.clone()),
                PaintOp::text_control_mark(bbox, run.clone()),
                PaintOp::text_decoration(bbox, run.clone(), TextDecorationKind::Underline),
                PaintOp::char_overlap(bbox, run.clone()),
                PaintOp::tab_leader(bbox, run),
            ],
        ),
    );

    let value: Value = serde_json::from_str(&tree.to_json()).expect("valid layer JSON");
    let ops = value["root"]["ops"].as_array().expect("leaf ops");
    assert_eq!(
        ops[1]["marks"].as_array().expect("bounded marks").len(),
        crate::paint::MAX_POSITIONED_CONTROL_MARKS_PER_RUN
    );
    assert_eq!(ops[0]["controlMarksComplete"], false);
    assert_eq!(ops[1]["marksComplete"], false);
    assert_eq!(
        ops[2]["decoration"]["positions"]
            .as_array()
            .expect("bounded decoration positions")
            .len(),
        crate::paint::MAX_POSITIONED_CONTROL_MARKS_PER_RUN + 1
    );
    assert_eq!(ops[2]["decoration"]["positionsComplete"], false);
    assert_eq!(ops[3]["positionsComplete"], false);
    assert_eq!(
        ops[3]["positions"]
            .as_array()
            .expect("bounded overlap positions")
            .len(),
        crate::paint::MAX_POSITIONED_CONTROL_MARKS_PER_RUN + 1
    );
    assert_eq!(ops[4]["leadersComplete"], false);
    assert!(value["usedFeatures"]
        .as_array()
        .expect("used features")
        .iter()
        .any(|feature| feature == "text.controlMarkOp.bounded"));
    assert!(value["usedFeatures"]
        .as_array()
        .expect("used features")
        .iter()
        .any(|feature| feature == "text.decorationOp.bounded"));
    assert!(value["usedFeatures"]
        .as_array()
        .expect("used features")
        .iter()
        .any(|feature| feature == "text.charOverlapOp.bounded"));
    assert!(value["usedFeatures"]
        .as_array()
        .expect("used features")
        .iter()
        .any(|feature| feature == "text.tabLeaderOp.bounded"));
}

fn optional_glyph_run_variant_tree() -> PageLayerTree {
    let source = TextSourceSpan {
        id: TextSourceId(0),
        utf8_range: TextSourceRange::new(0, 1),
        utf16_range: TextSourceRange::new(0, 1),
        stable_source_key: None,
    };
    let shape_key = ShapeKey {
        font_instance: FontInstanceKey {
            face_key: FontFaceKey("face-0".to_string()),
            size_px: 12.0,
            variations: Vec::new(),
            synthetic_bold: false,
            synthetic_italic: false,
        },
        direction: TextDirection::Ltr,
        writing_mode: WritingMode::HorizontalTb,
        script: Some(ScriptTag("DFLT".to_string())),
        language: None,
        features: Vec::new(),
        shaping_engine: ShapingEngineId("test".to_string()),
        fallback_policy: FontFallbackPolicyId("none".to_string()),
    };
    let text_run = PaintOp::text_run(
        BoundingBox::new(0.0, 0.0, 20.0, 20.0),
        TextRunNode {
            text: "A".to_string(),
            style: TextStyle {
                font_family: "Test".to_string(),
                font_size: 12.0,
                shade_color: 0x00FF_FFFF,
                ..Default::default()
            },
            char_shape_id: None,
            para_shape_id: None,
            section_index: None,
            para_index: None,
            char_start: None,
            cell_context: None,
            is_para_end: false,
            is_line_break_end: false,
            rotation: 0.0,
            is_vertical: false,
            char_overlap: None,
            border_fill_id: 0,
            baseline: 12.0,
            field_marker: FieldMarkerType::None,
            display_text: None,
        },
    );
    let glyph_run = PaintOp::GlyphRun {
        bbox: BoundingBox::new(0.0, 0.0, 20.0, 20.0),
        run: Box::new(LayerGlyphRunPaint {
            source,
            variant: PaintVariantMeta {
                equivalence_group: "text-0".to_string(),
                variant_id: "glyphRun".to_string(),
                variant_kind: TextVariantKind::GlyphRun,
                part_index: 0,
                part_count: 1,
                is_default_fallback: false,
                requires: vec!["fontResources".to_string(), "text.glyphRun".to_string()],
                quality: Some(TextVariantQuality::Exact),
                anchor_op_id: None,
                local_paint_order: None,
            },
            paint_style: PaintTextStyle::from(&TextStyle {
                font_family: "Test".to_string(),
                font_size: 12.0,
                shade_color: 0x00FF_FFFF,
                ..Default::default()
            }),
            shape_key,
            placement: crate::paint::TextRunPlacement {
                run_to_page: LayerAffineTransform {
                    a: 1.0,
                    b: 0.0,
                    c: 0.0,
                    d: 1.0,
                    e: 0.0,
                    f: 12.0,
                },
                baseline_y: 0.0,
            },
            glyph_ids: vec![42],
            positions: vec![LayerPoint { x: 0.0, y: 0.0 }],
            advances: Some(vec![LayerVector { dx: 12.0, dy: 0.0 }]),
            clusters: vec![GlyphCluster {
                source_range_utf8: TextSourceRange::new(0, 1),
                source_range_utf16: Some(TextSourceRange::new(0, 1)),
                text_range_utf8: Some(TextSourceRange::new(0, 1)),
                glyph_range: GlyphRange::new(0, 1),
                flags: Vec::new(),
            }],
            direction: TextDirection::Ltr,
            bidi_level: Some(0),
            writing_mode: WritingMode::HorizontalTb,
            orientation: GlyphRunOrientation::Horizontal,
            glyph_transforms: None,
            diagnostics: GlyphRunDiagnostics {
                quality: TextVariantQuality::Exact,
                replay_eligibility: GlyphRunReplayEligibility::Portable,
                strict_visual_eligible: true,
                max_origin_delta_px: 0.0,
                max_advance_delta_px: 0.0,
                max_residual_after_adjustment_px: 0.0,
                cluster_mismatch_count: 0,
                missing_glyph_count: 0,
                used_fallback_font_count: 0,
                reason: None,
            },
        }),
    };

    PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![text_run, glyph_run],
        ),
    )
}

fn add_portable_font_resources(resources: &mut ResourceArena) {
    let font_bytes = FIXTURE_TTC;
    resources.intern_font_blob_bytes(font_bytes);
    let blob_key = FontBlobKey("blob-0".to_string());
    let face_key = FontFaceKey("face-0".to_string());
    let digest_value = resource_digest_hex(font_bytes);
    let digest = FontDigest {
        algorithm: RESOURCE_KEY_ALGORITHM.to_string(),
        value: digest_value.clone(),
    };
    let data_ref = BinaryResourceRef {
        kind: BinaryResourceKind::FontBlob,
        id: font_blob_resource_key(font_bytes.len(), &digest_value),
    };
    resources.font_resources_mut().blobs.push(FontBlobResource {
        id: blob_key.clone(),
        digest: Some(digest.clone()),
        source: FontResourceSource::Embedded,
        data_ref: Some(data_ref.clone()),
        portability: FontPortability::PortableBlob { digest, data_ref },
    });
    resources.font_resources_mut().faces.push(FontFaceResource {
        id: face_key,
        blob_key,
        face_index: 0,
        postscript_name: None,
        family_names: Vec::new(),
        style_names: Vec::new(),
        weight_class: None,
        width_class: None,
        italic: None,
    });
}

#[test]
fn serializes_optional_glyph_run_variant_with_text_run_fallback() {
    let tree = optional_glyph_run_variant_tree();
    let json = tree.to_json();

    assert!(json.contains("\"type\":\"glyphRun\""));
    assert!(json.contains("\"fontResources\":{\"blobs\":[],\"faces\":[]}"));
    assert!(json.contains("\"optionalFeatures\":[\"fontResources\",\"text.glyphRun\"]"));
    assert!(json.contains("\"variants\":[\"textRun\",\"glyphRun\"]"));
    assert!(
        json.contains("\"variant\":{\"equivalenceGroup\":\"text-0\",\"variantId\":\"textRun\"")
    );
    assert!(json.contains("\"variantId\":\"glyphRun\""));
    assert!(json.contains("\"glyphIds\":[42]"));
    assert!(json.contains("\"replayEligibility\":\"portable\""));
    assert!(json.contains("\"strictVisualEligible\":true"));
    assert!(json.contains("\"slotDiagnostics\":[{\"paintOrderSlotId\":\"text-0\""));
    assert!(json.contains("\"strictVariantAvailable\":false"));
    assert!(json.contains("\"fallbackReason\":\"fontFaceMissing\""));
}

#[test]
fn serializes_strict_glyph_run_variant_when_font_resources_are_proven() {
    let mut tree = optional_glyph_run_variant_tree();
    add_portable_font_resources(&mut tree.resources);
    let json = tree.to_json();

    assert!(json.contains("\"type\":\"glyphRun\""));
    assert!(json.contains("\"fontResources\":{\"blobs\":["));
    assert!(json.contains("\"portability\":\"portableBlob\""));
    assert!(json.contains("\"faces\":["));
    assert!(json.contains("\"fontBlobs\":[\"dHRjZg"));
    assert!(json.contains("\"fontBlobKeys\":[\"font:blake3:1752:"));
    assert!(json.contains("\"optionalFeatures\":[\"fontResources\",\"text.glyphRun\"]"));
    assert!(json.contains("\"variants\":[\"textRun\",\"glyphRun\"]"));
    assert!(json.contains("\"strictVariantAvailable\":true"));
}

#[test]
fn serializes_glyph_outline_variant_with_strict_sidecar_contract() {
    let source = TextSourceSpan {
        id: TextSourceId(0),
        utf8_range: TextSourceRange::new(0, 1),
        utf16_range: TextSourceRange::new(0, 1),
        stable_source_key: None,
    };
    let text_run = PaintOp::text_run(
        BoundingBox::new(0.0, 0.0, 20.0, 20.0),
        TextRunNode {
            text: "A".to_string(),
            style: TextStyle {
                font_family: "Test".to_string(),
                font_size: 12.0,
                shade_color: 0x00FF_FFFF,
                ..Default::default()
            },
            char_shape_id: None,
            para_shape_id: None,
            section_index: None,
            para_index: None,
            char_start: None,
            cell_context: None,
            is_para_end: false,
            is_line_break_end: false,
            rotation: 0.0,
            is_vertical: false,
            char_overlap: None,
            border_fill_id: 0,
            baseline: 12.0,
            field_marker: FieldMarkerType::None,
            display_text: None,
        },
    );
    let outline = PaintOp::GlyphOutline {
        bbox: BoundingBox::new(0.0, 0.0, 20.0, 20.0),
        outline: Box::new(LayerGlyphOutlinePaint {
            source,
            variant: PaintVariantMeta {
                equivalence_group: "text-0".to_string(),
                variant_id: "glyphOutline".to_string(),
                variant_kind: TextVariantKind::GlyphOutline,
                part_index: 0,
                part_count: 1,
                is_default_fallback: false,
                requires: vec![
                    "text.glyphOutline".to_string(),
                    "text.glyphOutline.strictSidecar".to_string(),
                ],
                quality: Some(TextVariantQuality::Exact),
                anchor_op_id: Some("text-0".to_string()),
                local_paint_order: Some(0),
            },
            payload_kind: GlyphOutlinePayloadKind::MonochromeFillStroke,
            color_layers: None,
            bitmap_glyph: None,
            svg_glyph: None,
            paint_style: PaintTextStyle::from(&TextStyle {
                font_family: "Test".to_string(),
                font_size: 12.0,
                shade_color: 0x00FF_FFFF,
                ..Default::default()
            }),
            placement: crate::paint::TextRunPlacement {
                run_to_page: LayerAffineTransform {
                    a: 1.0,
                    b: 0.0,
                    c: 0.0,
                    d: 1.0,
                    e: 0.0,
                    f: 12.0,
                },
                baseline_y: 0.0,
            },
            paths: vec![LayerGlyphOutlinePath {
                glyph_id: 42,
                source_range_utf8: TextSourceRange::new(0, 1),
                glyph_range: GlyphRange::new(0, 1),
                commands: vec![
                    PathCommand::MoveTo(0.0, 0.0),
                    PathCommand::LineTo(10.0, 0.0),
                    PathCommand::ClosePath,
                ],
                fill_rule: GlyphOutlineFillRule::NonZero,
            }],
            stroke: Some(GlyphOutlineStrokeStyle {
                color: 0x00000000,
                width: 1.0,
                join: GlyphOutlineStrokeJoin::Miter,
                cap: GlyphOutlineStrokeCap::Butt,
                miter_limit: 2.0,
                paint_order: crate::paint::GlyphOutlinePaintOrder::FillThenStroke,
            }),
            diagnostics: GlyphRunDiagnostics {
                quality: TextVariantQuality::Exact,
                replay_eligibility: GlyphRunReplayEligibility::Portable,
                strict_visual_eligible: true,
                max_origin_delta_px: 0.0,
                max_advance_delta_px: 0.0,
                max_residual_after_adjustment_px: 0.0,
                cluster_mismatch_count: 0,
                missing_glyph_count: 0,
                used_fallback_font_count: 0,
                reason: None,
            },
        }),
    };

    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![text_run, outline],
        ),
    );
    let json = tree.to_json();

    assert!(json.contains("\"type\":\"glyphOutline\""));
    assert!(json.contains("\"payloadKind\":\"monochromeFillStroke\""));
    assert!(json.contains("\"anchorOpId\":\"text-0\""));
    assert!(json.contains("\"paths\":[{\"glyphId\":42"));
    assert!(json.contains("\"fillRule\":\"nonzero\""));
    assert!(json.contains("\"stroke\":{\"color\":\"#000000\""));
    assert!(json.contains("\"strictSubset\":true"));
    assert!(json.contains("\"text.glyphOutline\""));
    assert!(json.contains("\"text.glyphOutline.strictSidecar\""));
    assert!(json.contains("\"variants\":[\"textRun\",\"glyphOutline\"]"));
    assert!(json.contains("\"variantKind\":\"glyphOutline\""));
}

#[test]
fn serializes_advanced_glyph_outline_payload_gate_metadata() {
    let outline = PaintOp::GlyphOutline {
        bbox: BoundingBox::new(0.0, 0.0, 20.0, 20.0),
        outline: Box::new(LayerGlyphOutlinePaint {
            source: TextSourceSpan {
                id: TextSourceId(0),
                utf8_range: TextSourceRange::new(0, 1),
                utf16_range: TextSourceRange::new(0, 1),
                stable_source_key: None,
            },
            variant: PaintVariantMeta {
                equivalence_group: "text-0".to_string(),
                variant_id: "glyphOutlineColor".to_string(),
                variant_kind: TextVariantKind::GlyphOutline,
                part_index: 0,
                part_count: 1,
                is_default_fallback: false,
                requires: vec![
                    "text.glyphOutline".to_string(),
                    "text.glyphOutline.colorLayers".to_string(),
                    "text.glyphOutline.colorLayers.colrV1".to_string(),
                ],
                quality: Some(TextVariantQuality::Exact),
                anchor_op_id: Some("text-0".to_string()),
                local_paint_order: Some(0),
            },
            payload_kind: GlyphOutlinePayloadKind::ColorLayers,
            color_layers: Some(ColorLayersPayload {
                color_format: ColorGlyphFormat::ColrV1,
                source_font_ref: Some(FontColorGlyphRef {
                    face_key: Some("fixture:resource:face".to_string()),
                    glyph_id: Some(42),
                    palette_index: Some(0),
                    color_format: Some(ColorGlyphFormat::ColrV1),
                }),
                palette_ref: None,
                layers: Vec::new(),
                paint_graph: Some(ColorPaintGraphPayload {
                    root_node_id: 0,
                    nodes: vec![ColorPaintGraphNode {
                        node_id: 0,
                        kind: ColorPaintGraphNodeKind::SolidPath,
                        solid_path: Some(ColorPaintSolidPathNode {
                            commands: vec![
                                PathCommand::MoveTo(0.0, 0.0),
                                PathCommand::LineTo(10.0, 0.0),
                                PathCommand::ClosePath,
                            ],
                            fill: ResolvedColor {
                                color_space: Some("sRGB".to_string()),
                                rgba: [0.0, 0.0, 0.0, 1.0],
                            },
                            fill_rule: GlyphOutlineFillRule::NonZero,
                            source_glyph_id: Some(42),
                            palette_index: Some(0),
                        }),
                        linear_gradient_path: None,
                        radial_gradient_path: None,
                        sweep_gradient_path: None,
                        transform: None,
                        source_range_utf8: Some(TextSourceRange::new(0, 1)),
                        glyph_range: Some(GlyphRange::new(0, 1)),
                        source_font_ref: Some(FontColorGlyphRef {
                            face_key: Some("fixture:resource:face".to_string()),
                            glyph_id: Some(42),
                            palette_index: Some(0),
                            color_format: Some(ColorGlyphFormat::ColrV1),
                        }),
                    }],
                }),
                source_range_utf8: Some(TextSourceRange::new(0, 1)),
                glyph_range: Some(GlyphRange::new(0, 1)),
            }),
            bitmap_glyph: None,
            svg_glyph: None,
            paint_style: PaintTextStyle::from(&TextStyle {
                font_family: "Test".to_string(),
                font_size: 12.0,
                shade_color: 0x00FF_FFFF,
                ..Default::default()
            }),
            placement: crate::paint::TextRunPlacement {
                run_to_page: LayerAffineTransform {
                    a: 1.0,
                    b: 0.0,
                    c: 0.0,
                    d: 1.0,
                    e: 0.0,
                    f: 12.0,
                },
                baseline_y: 0.0,
            },
            paths: Vec::new(),
            stroke: None,
            diagnostics: GlyphRunDiagnostics {
                quality: TextVariantQuality::Exact,
                replay_eligibility: GlyphRunReplayEligibility::Portable,
                strict_visual_eligible: true,
                max_origin_delta_px: 0.0,
                max_advance_delta_px: 0.0,
                max_residual_after_adjustment_px: 0.0,
                cluster_mismatch_count: 0,
                missing_glyph_count: 0,
                used_fallback_font_count: 0,
                reason: None,
            },
        }),
    };

    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(BoundingBox::new(0.0, 0.0, 120.0, 80.0), None, vec![outline]),
    );
    let json = tree.to_json();

    assert!(json.contains("\"payloadKind\":\"colorLayers\""));
    assert!(json.contains("\"payloadResourceKey\":\"glyphPayload:colorLayers"));
    assert!(json.contains("\"colorLayers\":{\"colorFormat\":\"colrV1\""));
    assert!(json.contains("\"kind\":\"solidPath\""));
    assert!(json.contains("\"colrv1Stage1GraphContract\":true"));
    assert!(json.contains("\"text.glyphOutline.colorLayers\""));
    assert!(json.contains("\"text.glyphOutline.colorLayers.colrV1\""));
    assert!(json.contains("\"text.glyphOutline.payloadResourceKey\""));
    assert_eq!(
        json.matches("\"text.glyphOutline.payloadResourceDigestKey\"")
            .count(),
        1,
        "color-layer metadata containing ':resource:' must not advertise a resource digest payload feature"
    );
}

#[test]
fn serializes_bitmap_and_svg_glyph_payload_resource_keys() {
    let source = TextSourceSpan {
        id: TextSourceId(0),
        utf8_range: TextSourceRange::new(0, 1),
        utf16_range: TextSourceRange::new(0, 1),
        stable_source_key: None,
    };
    let diagnostics = GlyphRunDiagnostics {
        quality: TextVariantQuality::Exact,
        replay_eligibility: GlyphRunReplayEligibility::Portable,
        strict_visual_eligible: true,
        max_origin_delta_px: 0.0,
        max_advance_delta_px: 0.0,
        max_residual_after_adjustment_px: 0.0,
        cluster_mismatch_count: 0,
        missing_glyph_count: 0,
        used_fallback_font_count: 0,
        reason: None,
    };
    let placement = crate::paint::TextRunPlacement {
        run_to_page: LayerAffineTransform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 12.0,
        },
        baseline_y: 0.0,
    };
    let text_style = PaintTextStyle::from(&TextStyle {
        font_family: "Test".to_string(),
        font_size: 12.0,
        shade_color: 0x00FF_FFFF,
        ..Default::default()
    });
    let incomplete_bitmap_outline = LayerGlyphOutlinePaint {
        source: source.clone(),
        variant: PaintVariantMeta {
            equivalence_group: "text-invalid".to_string(),
            variant_id: "bitmapGlyphInvalid".to_string(),
            variant_kind: TextVariantKind::GlyphOutline,
            part_index: 0,
            part_count: 1,
            is_default_fallback: false,
            requires: vec!["text.glyphOutline.bitmapGlyph".to_string()],
            quality: Some(TextVariantQuality::Exact),
            anchor_op_id: Some("text-invalid".to_string()),
            local_paint_order: Some(0),
        },
        payload_kind: GlyphOutlinePayloadKind::BitmapGlyph,
        color_layers: None,
        bitmap_glyph: Some(BitmapGlyphPayload {
            image_ref: ImageResourceId(7),
            source_range_utf8: TextSourceRange::new(0, 1),
            glyph_range: GlyphRange::new(0, 1),
            placement: BoundingBox::new(0.0, 0.0, 10.0, 10.0),
            alpha_premultiplied: true,
            scaling_policy: BitmapGlyphScalingPolicy::BackendDefault,
            filtering: BitmapGlyphFiltering::Linear,
            transform_to_run: None,
        }),
        svg_glyph: None,
        paint_style: text_style.clone(),
        placement,
        paths: Vec::new(),
        stroke: None,
        diagnostics: diagnostics.clone(),
    };
    assert!(!incomplete_bitmap_outline.has_payload_resource_key());
    assert!(incomplete_bitmap_outline.payload_resource_key().is_none());
    let mut resources = ResourceArena::default();
    let invalid_image_id = resources.intern_image_bytes(&[1, 2, 3, 4]);
    let mut invalid_bitmap_with_resource = incomplete_bitmap_outline.clone();
    invalid_bitmap_with_resource
        .bitmap_glyph
        .as_mut()
        .unwrap()
        .image_ref = invalid_image_id;
    assert!(!has_payload_resource_digest_key(
        &invalid_bitmap_with_resource,
        &resources
    ));
    let bitmap_outline = PaintOp::GlyphOutline {
        bbox: BoundingBox::new(0.0, 0.0, 20.0, 20.0),
        outline: Box::new(LayerGlyphOutlinePaint {
            source: source.clone(),
            variant: PaintVariantMeta {
                equivalence_group: "text-0".to_string(),
                variant_id: "bitmapGlyph".to_string(),
                variant_kind: TextVariantKind::GlyphOutline,
                part_index: 0,
                part_count: 1,
                is_default_fallback: false,
                requires: vec!["text.glyphOutline.bitmapGlyph".to_string()],
                quality: Some(TextVariantQuality::Exact),
                anchor_op_id: Some("text-0".to_string()),
                local_paint_order: Some(0),
            },
            payload_kind: GlyphOutlinePayloadKind::BitmapGlyph,
            color_layers: None,
            bitmap_glyph: Some(BitmapGlyphPayload {
                image_ref: ImageResourceId(0),
                source_range_utf8: TextSourceRange::new(0, 1),
                glyph_range: GlyphRange::new(0, 1),
                placement: BoundingBox::new(0.1234, 0.5678, 10.9876, 10.5432),
                alpha_premultiplied: true,
                scaling_policy: BitmapGlyphScalingPolicy::SourceExact,
                filtering: BitmapGlyphFiltering::Linear,
                transform_to_run: None,
            }),
            svg_glyph: None,
            paint_style: text_style.clone(),
            placement,
            paths: Vec::new(),
            stroke: None,
            diagnostics: diagnostics.clone(),
        }),
    };
    let svg_outline = PaintOp::GlyphOutline {
        bbox: BoundingBox::new(20.0, 0.0, 20.0, 20.0),
        outline: Box::new(LayerGlyphOutlinePaint {
            source,
            variant: PaintVariantMeta {
                equivalence_group: "text-1".to_string(),
                variant_id: "svgGlyph".to_string(),
                variant_kind: TextVariantKind::GlyphOutline,
                part_index: 0,
                part_count: 1,
                is_default_fallback: false,
                requires: vec!["text.glyphOutline.svgGlyph".to_string()],
                quality: Some(TextVariantQuality::Exact),
                anchor_op_id: Some("text-1".to_string()),
                local_paint_order: Some(0),
            },
            payload_kind: GlyphOutlinePayloadKind::SvgGlyph,
            color_layers: None,
            bitmap_glyph: None,
            svg_glyph: Some(SvgGlyphPayload {
                svg_ref: SvgResourceId(0),
                source_range_utf8: TextSourceRange::new(0, 1),
                glyph_range: GlyphRange::new(0, 1),
                view_box: BoundingBox::new(0.1234, 0.5678, 10.9876, 10.5432),
                intrinsic_size: Some(LayerVector { dx: 10.0, dy: 10.0 }),
                static_sanitized: true,
                script_allowed: false,
                animation_allowed: false,
                external_resources_allowed: false,
                interactivity_allowed: false,
                transform_to_run: None,
            }),
            paint_style: text_style,
            placement,
            paths: Vec::new(),
            stroke: None,
            diagnostics,
        }),
    };
    let mut tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![bitmap_outline, svg_outline],
        ),
    );
    let image_bytes = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let svg_fragment = "<path d=\"M0 0H10V10Z\"/>";
    let image_id = tree.resources.intern_image_bytes(&image_bytes);
    let svg_id = tree.resources.intern_svg_fragment(svg_fragment);
    assert_eq!(image_id, ImageResourceId(0));
    assert_eq!(svg_id, SvgResourceId(0));
    let image_resource_key = tree
        .resources
        .image_resource_key(image_id)
        .unwrap()
        .to_string();
    let svg_resource_key = tree.resources.svg_resource_key(svg_id).unwrap().to_string();

    let json = tree.to_json();

    // 상수에서 끌어온다 — 숫자를 박아 두면 schema minor 를 올릴 때마다 무관한 테스트가 깨진다.
    assert!(json.contains(&format!(
        "\"schemaMinorVersion\":{}",
        LAYER_TREE_SCHEMA.schema_minor_version
    )));
    assert!(json.contains("\"payloadResourceKey\":\"glyphPayload:bitmapGlyph:imageRef:0"));
    assert!(json.contains("placement:0.123,0.568,10.988,10.543"));
    assert!(json.contains(&format!(":resource:{image_resource_key}\"")));
    assert!(json.contains("\"payloadResourceKey\":\"glyphPayload:svgGlyph:svgRef:0"));
    assert!(json.contains("viewBox:0.123,0.568,10.988,10.543"));
    assert!(json.contains(&format!(":resource:{svg_resource_key}\"")));
    assert!(json.contains("\"vectorResourceId\":0"));
    assert!(json.contains("\"strictVisualContract\":true"));
    assert!(json.contains("\"staticSanitizedContract\":true"));
    assert!(json.contains("\"text.glyphOutline.payloadResourceKey\""));
    assert!(json.contains("\"text.glyphOutline.payloadResourceDigestKey\""));
    assert!(json.contains("\"text.glyphOutline.svgGlyph.vectorResourceId\""));
    assert!(json.contains("\"usedFeatures\":[\"text.paintStyle\""));
    assert!(json.contains("\"fontResources\""));
    assert!(json.contains("\"optionalFeatures\":[\"fontResources\""));
    assert!(json.contains("\"resources\":{\"tableId\":1,\"images\":[\"iVBORw0KGgo=\"]"));
    assert!(json.contains(&format!("\"imageKeys\":[\"{image_resource_key}\"]")));
    assert!(json.contains(&format!("\"svgKeys\":[\"{svg_resource_key}\"]")));
    assert!(json.contains("\"svgFragments\":[\"<path d=\\\"M0 0H10V10Z\\\"/>\"]"));
}

/// 그림 바이트는 base64 로 나가므로 이스케이프 대상이 없다 — 그 전제로 이스케이프
/// 스캔을 없앴으니, 제어문자·따옴표·역슬래시를 포함한 바이트도 JSON 파서를 통과해
/// 원본으로 되돌아오는지 왕복으로 고정한다 (Task #3315).
#[test]
fn image_bytes_survive_json_round_trip_for_every_byte_value() {
    use base64::Engine;

    let image_bytes: Vec<u8> = (0..=255u8).collect();
    let background = PageBackgroundNode {
        background_color: None,
        border_color: None,
        border_width: 0.0,
        gradient: None,
        image: Some(PageBackgroundImage {
            data: image_bytes.clone(),
            fill_mode: ImageFillMode::FitToSize,
            brightness: 0,
            contrast: 0,
            effect: ImageEffect::RealPic,
        }),
    };

    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![
                PaintOp::page_background(BoundingBox::new(0.0, 0.0, 120.0, 80.0), background),
                PaintOp::image(
                    BoundingBox::new(3.0, 4.0, 30.0, 20.0),
                    ImageNode::new(7, Some(image_bytes.clone())),
                    None,
                ),
            ],
        ),
    );

    let value: Value = serde_json::from_str(&tree.to_json()).expect("valid layer JSON");
    let ops = value["root"]["ops"].as_array().expect("ops");

    let decode = |op: &Value, pointer: &str| {
        let encoded = op.pointer(pointer).and_then(Value::as_str).expect(pointer);
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("base64 왕복")
    };

    assert_eq!(decode(&ops[0], "/image/base64"), image_bytes);
    assert_eq!(decode(&ops[1], "/base64"), image_bytes);
}

#[test]
fn known_text_features_are_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for feature in KNOWN_TEXT_FEATURES {
        assert!(seen.insert(*feature), "duplicate known feature: {feature}");
    }
}

#[test]
fn serializes_backend_replay_payload_fields() {
    let mut path = PathNode::new(
        vec![
            PathCommand::MoveTo(0.0, 0.0),
            PathCommand::LineTo(10.0, 10.0),
        ],
        ShapeStyle::default(),
        None,
    );
    path.connector_endpoints = Some((1.0, 2.0, 3.0, 4.0));
    path.line_style = Some(LineStyle {
        color: 0x0056_3412,
        width: 2.0,
        dash: StrokeDash::DashDot,
        ..Default::default()
    });

    let mut image = ImageNode::new(7, Some(vec![1, 2, 3]));
    image.effect = ImageEffect::BlackWhite;
    image.brightness = -50;
    image.contrast = 70;
    image.crop = Some((0, 0, 144000, 81000));
    image.original_size_hu = Some((144000, 81000));

    let tree = PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![
                PaintOp::path(BoundingBox::new(1.0, 2.0, 30.0, 20.0), path),
                PaintOp::image(BoundingBox::new(3.0, 4.0, 30.0, 20.0), image, None),
                PaintOp::equation(
                    BoundingBox::new(5.0, 6.0, 30.0, 20.0),
                    EquationNode {
                        svg_content: "<text>x</text>".to_string(),
                        layout_box: LayoutBox {
                            x: 0.0,
                            y: 0.0,
                            width: 8.0,
                            height: 12.0,
                            baseline: 10.0,
                            kind: LayoutKind::Text("x".to_string()),
                        },
                        color_str: "#000000".to_string(),
                        color: 0x00000000,
                        font_size: 12.0,
                        script: String::new(),
                        section_index: None,
                        para_index: None,
                        control_index: None,
                        cell_index: None,
                        cell_para_index: None,
                        note_ref: None,
                    },
                ),
                PaintOp::placeholder(
                    BoundingBox::new(7.0, 8.0, 30.0, 20.0),
                    PlaceholderNode::new(0x00F0F0F0, 0x00000000, "OLE".to_string()),
                ),
                PaintOp::raw_svg(
                    BoundingBox::new(9.0, 10.0, 30.0, 20.0),
                    RawSvgNode::new("<g><path d=\"M0 0L1 1\"/></g>".to_string()),
                ),
            ],
        ),
    );

    let json = tree.to_json();
    let value: Value = serde_json::from_str(&json).expect("valid layer JSON");
    let path_op = &value["root"]["ops"][0];

    assert!(json.contains("\"connectorEndpoints\":{\"x1\":1.000"));
    assert!(json.contains("\"lineStyle\":"));
    assert_eq!(path_op["lineStyle"]["color"], "#123456");
    assert_eq!(path_op["lineStyle"]["width"], 2.0);
    assert_eq!(path_op["lineStyle"]["dash"], "dashDot");
    assert!(json.contains("\"effect\":\"blackWhite\""));
    assert!(json.contains("\"brightness\":-50"));
    assert!(json.contains("\"contrast\":70"));
    assert!(json.contains("\"originalSizeHu\":[144000,81000]"));
    assert!(json.contains("\"svgContent\":\"<text>x</text>\""));
    assert!(json.contains("\"layoutBox\":{\"x\":0.000000"));
    assert!(json.contains("\"kind\":{\"type\":\"text\",\"text\":\"x\"}"));
    assert!(json.contains("\"type\":\"placeholder\""));
    assert!(json.contains("\"label\":\"OLE\""));
    assert!(json.contains("\"type\":\"rawSvg\""));
    assert!(json.contains("\"svg\":\"<g><path d=\\\"M0 0L1 1\\\"/></g>\""));
}

#[test]
fn serializes_layer_node_metadata() {
    let leaf = LayerNode::leaf(BoundingBox::new(0.0, 0.0, 10.0, 10.0), None, Vec::new());
    let clip = LayerNode::clip_rect(
        BoundingBox::new(0.0, 0.0, 10.0, 10.0),
        None,
        BoundingBox::new(1.0, 1.0, 8.0, 8.0),
        leaf,
        ClipKind::Body,
    );
    let root = LayerNode::group(
        BoundingBox::new(0.0, 0.0, 10.0, 10.0),
        None,
        vec![clip],
        CacheHint::StaticSubtree,
        GroupKind::Column(2),
    )
    .with_layer(Some(RenderLayerInfo::new(
        Some(TextWrap::BehindText),
        7,
        42,
    )));

    let json = PageLayerTree::new(10.0, 10.0, root).to_json();

    assert!(json.contains("\"groupKind\":{\"kind\":\"column\",\"index\":2}"));
    assert!(json.contains("\"cacheHint\":\"staticSubtree\""));
    assert!(json.contains("\"clipKind\":\"body\""));
    assert!(json
        .contains("\"layer\":{\"textWrap\":\"behindText\",\"zOrder\":7,\"stableIndex\":42}"));
}

#[test]
fn serializes_textbox_clip_kind() {
    let leaf = LayerNode::leaf(BoundingBox::new(0.0, 0.0, 10.0, 10.0), None, Vec::new());
    let root = LayerNode::clip_rect(
        BoundingBox::new(0.0, 0.0, 10.0, 10.0),
        None,
        BoundingBox::new(1.0, 1.0, 8.0, 8.0),
        leaf,
        ClipKind::TextBox,
    );

    let json = PageLayerTree::new(10.0, 10.0, root).to_json();

    assert!(json.contains("\"clipKind\":\"textBox\""));
}

#[test]
fn serializes_layer_option_metadata() {
    let root = LayerNode::leaf(BoundingBox::new(0.0, 0.0, 10.0, 10.0), None, Vec::new());
    let json = PageLayerTree::new(10.0, 10.0, root)
        .with_output_options(crate::paint::LayerOutputOptions {
            show_paragraph_marks: true,
            show_control_codes: true,
            show_transparent_borders: true,
            clip_enabled: false,
            debug_overlay: true,
        })
        .to_json();

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
