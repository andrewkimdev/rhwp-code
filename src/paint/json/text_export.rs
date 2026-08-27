//! text_export — table_layout.rs 에서 무변동 이동
use super::*;

pub(crate) fn write_text_export_metadata(buf: &mut String, root: &LayerNode, resources: &ResourceArena) {
    let externalized_visuals = externalized_text_visuals(root);
    let text_variant_features = collect_text_variant_features(root, resources);
    let has_variant_groups = text_variant_features.has_variant_groups();
    let has_glyph_runs = text_variant_features.has_glyph_runs;
    let has_glyph_outlines = text_variant_features.has_glyph_outlines;
    let has_glyph_outline_color_layers = text_variant_features.has_glyph_outline_color_layers;
    let has_glyph_outline_bitmap = text_variant_features.has_glyph_outline_bitmap;
    let has_glyph_outline_svg = text_variant_features.has_glyph_outline_svg;
    let has_glyph_outline_payload_resource_keys =
        text_variant_features.has_glyph_outline_payload_resource_keys;
    let has_glyph_outline_payload_resource_digest_keys =
        text_variant_features.has_glyph_outline_payload_resource_digest_keys;
    let has_display_text = text_variant_features.has_display_text;
    buf.push_str(",\"usedFeatures\":[\"text.paintStyle\",\"text.sourceTable\",\"text.sourceSpan\",\"text.v2.placement\",\"text.v2.clusters\",\"text.v2.diagnostics\",\"text.projectionKind\",\"text.legacyVisuals\",\"layer.optionMetadata\"");
    if has_display_text {
        buf.push_str(",\"text.displayText\"");
    }
    if has_glyph_runs || has_glyph_outlines {
        buf.push_str(",\"fontResources\"");
    }
    if has_glyph_runs {
        buf.push_str(",\"text.glyphRun\"");
    }
    if has_glyph_outlines {
        buf.push_str(",\"text.glyphOutline\",\"text.glyphOutline.strictSidecar\"");
    }
    if has_glyph_outline_color_layers {
        buf.push_str(",\"text.glyphOutline.colorLayers\"");
    }
    if has_glyph_outline_bitmap {
        buf.push_str(",\"text.glyphOutline.bitmapGlyph\"");
    }
    if has_glyph_outline_svg {
        buf.push_str(",\"text.glyphOutline.svgGlyph\"");
        buf.push_str(",\"text.glyphOutline.svgGlyph.vectorResourceId\"");
    }
    if has_glyph_outline_payload_resource_keys {
        buf.push_str(",\"text.glyphOutline.payloadResourceKey\"");
    }
    if has_glyph_outline_payload_resource_digest_keys {
        buf.push_str(",\"text.glyphOutline.payloadResourceDigestKey\"");
    }
    if has_variant_groups {
        buf.push_str(",\"text.variantGroups\"");
    }
    if externalized_visuals.contains(&"charOverlap") {
        buf.push_str(",\"text.charOverlapOp\",\"text.charOverlapOp.bounded\"");
    }
    if externalized_visuals.contains(&"controlMarks") {
        buf.push_str(",\"text.controlMarkOp\",\"text.controlMarkOp.positioned\",\"text.controlMarkOp.bounded\"");
    }
    if externalized_visuals.contains(&"tabLeaders") {
        buf.push_str(",\"text.tabLeaderOp\",\"text.tabLeaderOp.bounded\"");
    }
    if externalized_visuals.contains(&"decorations") {
        buf.push_str(",\"text.decorationOp\",\"text.decorationOp.bounded\"");
    }
    let mut optional_features = Vec::new();
    if has_glyph_runs || has_glyph_outlines {
        optional_features.push("fontResources");
    }
    if has_glyph_runs {
        optional_features.push("text.glyphRun");
    }
    if has_glyph_outlines {
        optional_features.push("text.glyphOutline");
        optional_features.push("text.glyphOutline.strictSidecar");
    }
    if has_glyph_outline_color_layers {
        optional_features.push("text.glyphOutline.colorLayers");
    }
    if has_glyph_outline_bitmap {
        optional_features.push("text.glyphOutline.bitmapGlyph");
    }
    if has_glyph_outline_svg {
        optional_features.push("text.glyphOutline.svgGlyph");
        optional_features.push("text.glyphOutline.svgGlyph.vectorResourceId");
    }
    if has_glyph_outline_payload_resource_keys {
        optional_features.push("text.glyphOutline.payloadResourceKey");
    }
    if has_glyph_outline_payload_resource_digest_keys {
        optional_features.push("text.glyphOutline.payloadResourceDigestKey");
    }
    buf.push_str("],\"optionalFeatures\":[");
    for (idx, feature) in optional_features.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        buf.push_str(&json_escape(feature));
    }
    buf.push_str("],\"knownFeatures\":[");
    for (idx, feature) in KNOWN_TEXT_FEATURES.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        buf.push_str(&json_escape(feature));
    }
    buf.push_str("],\"requiredFeatures\":[],\"text\":{\"defaultVariant\":\"textRun\",\"variants\":[\"textRun\"");
    if has_glyph_runs {
        buf.push_str(",\"glyphRun\"");
    }
    if has_glyph_outlines {
        buf.push_str(",\"glyphOutline\"");
    }
    buf.push_str("],\"variantSelection\":\"exclusiveVariantSet\",\"sourceTextPreserved\":true,\"clusterEncoding\":[\"utf8\",\"utf16\"],\"fallbackRequired\":true,\"placementAuthority\":\"compatibilityProjection\",\"externalizedVisuals\":[");
    for (idx, visual) in externalized_visuals.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        buf.push_str(&json_escape(visual));
    }
    buf.push_str("]}");
}


#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TextVariantFeatureFlags {
    pub(crate) has_glyph_runs: bool,
    pub(crate) has_glyph_outlines: bool,
    pub(crate) has_glyph_outline_color_layers: bool,
    pub(crate) has_glyph_outline_bitmap: bool,
    pub(crate) has_glyph_outline_svg: bool,
    pub(crate) has_glyph_outline_payload_resource_keys: bool,
    pub(crate) has_glyph_outline_payload_resource_digest_keys: bool,
    pub(crate) has_display_text: bool,
}


pub(crate) fn collect_text_variant_features(
    root: &LayerNode,
    resources: &ResourceArena,
) -> TextVariantFeatureFlags {
    let mut features = TextVariantFeatureFlags::default();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match &node.kind {
            LayerNodeKind::Group { children, .. } => {
                for child in children {
                    stack.push(child);
                }
            }
            LayerNodeKind::ClipRect { child, .. } => stack.push(child),
            LayerNodeKind::Leaf { ops } => {
                for op in ops {
                    match op {
                        PaintOp::TextRun { run, .. } => {
                            features.has_display_text |= display_text_for_text_run(run).is_some()
                        }
                        PaintOp::GlyphRun { .. } => features.has_glyph_runs = true,
                        PaintOp::GlyphOutline { outline, .. } => {
                            features.has_glyph_outlines = true;
                            features.has_glyph_outline_color_layers |= matches!(
                                outline.payload_kind,
                                GlyphOutlinePayloadKind::ColorLayers
                            );
                            features.has_glyph_outline_bitmap |= matches!(
                                outline.payload_kind,
                                GlyphOutlinePayloadKind::BitmapGlyph
                            );
                            features.has_glyph_outline_svg |=
                                matches!(outline.payload_kind, GlyphOutlinePayloadKind::SvgGlyph);
                            features.has_glyph_outline_payload_resource_keys |=
                                outline.has_payload_resource_key();
                            features.has_glyph_outline_payload_resource_digest_keys |=
                                has_payload_resource_digest_key(outline, resources);
                        }
                        _ => {}
                    }
                }
            }
        }
        if features.has_glyph_runs
            && features.has_glyph_outlines
            && features.has_glyph_outline_color_layers
            && features.has_glyph_outline_bitmap
            && features.has_glyph_outline_svg
            && features.has_glyph_outline_payload_resource_keys
            && features.has_glyph_outline_payload_resource_digest_keys
            && features.has_display_text
        {
            return features;
        }
    }
    features
}


pub(crate) fn has_payload_resource_digest_key(
    outline: &crate::paint::LayerGlyphOutlinePaint,
    resources: &ResourceArena,
) -> bool {
    if !outline.has_payload_resource_key() {
        return false;
    }
    match outline.payload_kind {
        GlyphOutlinePayloadKind::BitmapGlyph => outline
            .bitmap_glyph
            .as_ref()
            .is_some_and(|payload| resources.image_bytes(payload.image_ref).is_some()),
        GlyphOutlinePayloadKind::SvgGlyph => outline
            .svg_glyph
            .as_ref()
            .is_some_and(|payload| resources.svg_fragment(payload.svg_ref).is_some()),
        GlyphOutlinePayloadKind::ColorLayers
        | GlyphOutlinePayloadKind::MonochromeFill
        | GlyphOutlinePayloadKind::MonochromeFillStroke => false,
    }
}


pub(crate) fn externalized_text_visuals(root: &LayerNode) -> Vec<&'static str> {
    let mut has_char_overlap = false;
    let mut has_control_marks = false;
    let mut has_tab_leaders = false;
    let mut has_decorations = false;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match &node.kind {
            LayerNodeKind::Group { children, .. } => {
                for child in children {
                    stack.push(child);
                }
            }
            LayerNodeKind::ClipRect { child, .. } => stack.push(child),
            LayerNodeKind::Leaf { ops } => {
                has_char_overlap |= ops
                    .iter()
                    .any(|op| matches!(op, PaintOp::CharOverlap { .. }));
                has_control_marks |= ops
                    .iter()
                    .any(|op| matches!(op, PaintOp::TextControlMark { .. }));
                has_tab_leaders |= ops.iter().any(|op| matches!(op, PaintOp::TabLeader { .. }));
                has_decorations |= ops
                    .iter()
                    .any(|op| matches!(op, PaintOp::TextDecoration { .. }));
            }
        }
    }
    let mut visuals = Vec::new();
    if has_char_overlap {
        visuals.push("charOverlap");
    }
    if has_control_marks {
        visuals.push("controlMarks");
    }
    if has_tab_leaders {
        visuals.push("tabLeaders");
    }
    if has_decorations {
        visuals.push("decorations");
    }
    visuals
}


#[derive(Default)]
pub(crate) struct TextSourceExportState {
    pub(crate) next_id: u32,
    pub(crate) last_source: Option<TextSourceSpan>,
}


#[derive(Debug, Clone, Default)]
pub(crate) struct LeafTextVisualOps {
    pub(crate) char_overlap: bool,
    pub(crate) control_marks: bool,
    pub(crate) tab_leaders: bool,
    pub(crate) decorations: bool,
    pub(crate) glyph_variant_groups: Vec<(u32, String)>,
}


pub(crate) fn stable_text_source_key(run: &TextRunNode) -> Option<String> {
    let section = run.section_index?;
    let para = run.para_index?;
    let char_start = run.char_start.unwrap_or(0);
    let mut key = format!("section:{section}/para:{para}/char:{char_start}");
    if let Some(cell) = &run.cell_context {
        let path = cell
            .path
            .iter()
            .map(|entry| {
                format!(
                    "{}:{}:{}:{}",
                    entry.control_index,
                    entry.cell_index,
                    entry.cell_para_index,
                    entry.text_direction
                )
            })
            .collect::<Vec<_>>()
            .join(".");
        key.push_str("/cell:");
        key.push_str(&cell.parent_para_index.to_string());
        key.push(':');
        key.push_str(&path);
    }
    Some(key)
}


pub(crate) fn write_text_source_entries(buf: &mut String, table: &TextSourceTable) {
    buf.push('[');
    for (idx, entry) in table.entries.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        write_text_source_entry(buf, entry);
    }
    buf.push(']');
}


pub(crate) fn write_visual_resources(buf: &mut String, resources: &ResourceArena) {
    let _ = write!(
        buf,
        "{{\"tableId\":{},\"images\":[",
        LAYER_TREE_SCHEMA.resource_table_version
    );
    for (index, (_, bytes)) in resources.image_resources().enumerate() {
        if index > 0 {
            buf.push(',');
        }
        write_json_base64(buf, bytes);
    }
    buf.push_str("],\"imageKeys\":[");
    for index in 0..resources.image_count() {
        if index > 0 {
            buf.push(',');
        }
        let key = resources
            .image_resource_key(crate::paint::ImageResourceId(index))
            .unwrap_or("");
        buf.push_str(&json_escape(key));
    }
    buf.push_str("],\"svgFragments\":[");
    for (index, (_, fragment)) in resources.svg_resources().enumerate() {
        if index > 0 {
            buf.push(',');
        }
        buf.push_str(&json_escape(fragment));
    }
    buf.push_str("],\"svgKeys\":[");
    for index in 0..resources.svg_count() {
        if index > 0 {
            buf.push(',');
        }
        let key = resources
            .svg_resource_key(crate::paint::SvgResourceId(index))
            .unwrap_or("");
        buf.push_str(&json_escape(key));
    }
    buf.push_str("],\"fontBlobs\":[");
    for (index, (_, bytes)) in resources.font_blob_resources().enumerate() {
        if index > 0 {
            buf.push(',');
        }
        write_json_base64(buf, bytes);
    }
    buf.push_str("],\"fontBlobKeys\":[");
    for index in 0..resources.font_blob_count() {
        if index > 0 {
            buf.push(',');
        }
        let key = resources
            .font_blob_resource_key(crate::paint::FontBlobResourceId(index))
            .unwrap_or("");
        buf.push_str(&json_escape(key));
    }
    buf.push_str("]}");
}


pub(crate) fn write_text_source_entry(buf: &mut String, entry: &TextSourceEntry) {
    let _ = write!(
        buf,
        "{{\"id\":{},\"text\":{},\"utf8Range\":",
        entry.id.0,
        json_escape(&entry.text),
    );
    write_text_source_range(buf, entry.utf8_range);
    buf.push_str(",\"utf16Range\":");
    write_text_source_range(buf, entry.utf16_range);
    if let Some(stable_source_key) = &entry.stable_source_key {
        let _ = write!(
            buf,
            ",\"stableSourceKey\":{}",
            json_escape(stable_source_key)
        );
    }
    buf.push_str(",\"annotations\":");
    write_text_source_annotations(buf, &entry.annotations);
    buf.push('}');
}


pub(crate) fn write_text_source_span(buf: &mut String, span: &TextSourceSpan) {
    let _ = write!(buf, "{{\"id\":{},\"utf8Range\":", span.id.0);
    write_text_source_range(buf, span.utf8_range);
    buf.push_str(",\"utf16Range\":");
    write_text_source_range(buf, span.utf16_range);
    if let Some(stable_source_key) = &span.stable_source_key {
        let _ = write!(
            buf,
            ",\"stableSourceKey\":{}",
            json_escape(stable_source_key)
        );
    }
    buf.push('}');
}


pub(crate) fn write_text_source_range(buf: &mut String, range: TextSourceRange) {
    let _ = write!(buf, "{{\"start\":{},\"end\":{}}}", range.start, range.end);
}


pub(crate) fn write_text_source_annotations(buf: &mut String, annotations: &[TextSourceAnnotation]) {
    buf.push('[');
    for (idx, annotation) in annotations.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        match annotation {
            TextSourceAnnotation::FieldMarker {
                marker,
                range_utf8,
                range_utf16,
            } => {
                let _ = write!(
                    buf,
                    "{{\"kind\":\"fieldMarker\",\"marker\":{},\"rangeUtf8\":",
                    json_escape(field_marker_str(*marker))
                );
                write_text_source_range(buf, *range_utf8);
                buf.push_str(",\"rangeUtf16\":");
                write_text_source_range(buf, *range_utf16);
                if let FieldMarkerType::ShapeMarker(index) = marker {
                    let _ = write!(buf, ",\"shapeMarkerIndex\":{}", index);
                }
                buf.push('}');
            }
            TextSourceAnnotation::ParagraphEnd {
                offset_utf8,
                offset_utf16,
            } => {
                let _ = write!(
                    buf,
                    "{{\"kind\":\"paragraphEnd\",\"offsetUtf8\":{},\"offsetUtf16\":{}}}",
                    offset_utf8, offset_utf16
                );
            }
            TextSourceAnnotation::LineBreakEnd {
                offset_utf8,
                offset_utf16,
            } => {
                let _ = write!(
                    buf,
                    "{{\"kind\":\"lineBreakEnd\",\"offsetUtf8\":{},\"offsetUtf16\":{}}}",
                    offset_utf8, offset_utf16
                );
            }
        }
    }
    buf.push(']');
}


pub(crate) fn write_text_style(buf: &mut String, style: &TextStyle) {
    buf.push('{');
    let _ = write!(
        buf,
        "\"fontFamily\":{},\"fontSize\":{:.3},\"color\":{},\"bold\":{},\"italic\":{},\"ratio\":{:.6},\"underline\":{},\"underlineShape\":{},\"strikethrough\":{},\"strikeShape\":{},\"outlineType\":{},\"shadowType\":{},\"shadowColor\":{},\"shadowOffsetX\":{:.3},\"shadowOffsetY\":{:.3},\"emboss\":{},\"engrave\":{},\"superscript\":{},\"subscript\":{},\"underlineColor\":{},\"strikeColor\":{},\"shadeColor\":{},\"emphasisDot\":{}",
        json_escape(&style.font_family),
        style.font_size,
        json_escape(&color_ref_to_css(style.color)),
        style.bold,
        style.italic,
        style.ratio,
        json_escape(underline_type_str(style.underline)),
        style.underline_shape,
        style.strikethrough,
        style.strike_shape,
        style.outline_type,
        style.shadow_type,
        json_escape(&color_ref_to_css(style.shadow_color)),
        style.shadow_offset_x,
        style.shadow_offset_y,
        style.emboss,
        style.engrave,
        style.superscript,
        style.subscript,
        json_escape(&color_ref_to_css(style.underline_color)),
        json_escape(&color_ref_to_css(style.strike_color)),
        json_escape(&color_ref_to_css(style.shade_color)),
        style.emphasis_dot,
    );
    buf.push('}');
}


pub(crate) fn write_paint_text_style(buf: &mut String, style: &PaintTextStyle) {
    buf.push('{');
    let _ = write!(
        buf,
        "\"fontFamily\":{},\"fontSize\":{:.3},\"color\":{},\"bold\":{},\"italic\":{},\"ratio\":{:.6},\"underline\":{},\"underlineShape\":{},\"strikethrough\":{},\"strikeShape\":{},\"outlineType\":{},\"shadowType\":{},\"shadowColor\":{},\"shadowOffsetX\":{:.3},\"shadowOffsetY\":{:.3},\"emboss\":{},\"engrave\":{},\"superscript\":{},\"subscript\":{},\"underlineColor\":{},\"strikeColor\":{},\"shadeColor\":{},\"emphasisDot\":{}",
        json_escape(&style.font_family),
        style.font_size,
        json_escape(&color_ref_to_css(style.color)),
        style.bold,
        style.italic,
        style.ratio,
        json_escape(underline_type_str(style.underline)),
        style.underline_shape,
        style.strikethrough,
        style.strike_shape,
        style.outline_type,
        style.shadow_type,
        json_escape(&color_ref_to_css(style.shadow_color)),
        style.shadow_offset_x,
        style.shadow_offset_y,
        style.emboss,
        style.engrave,
        style.superscript,
        style.subscript,
        json_escape(&color_ref_to_css(style.underline_color)),
        json_escape(&color_ref_to_css(style.strike_color)),
        json_escape(&color_ref_to_css(style.shade_color)),
        style.emphasis_dot,
    );
    buf.push('}');
}


pub(crate) fn write_text_positions(buf: &mut String, run: &TextRunNode) {
    write_text_positions_for_text(buf, &run.text, &run.style);
}


pub(crate) fn write_text_positions_for_text(buf: &mut String, text: &str, style: &TextStyle) {
    let positions = compute_char_positions(text, style);
    write_position_values(buf, &positions);
}


pub(crate) fn bounded_text_prefix(text: &str) -> (String, bool) {
    let mut chars = text.chars();
    let prefix = chars
        .by_ref()
        .take(crate::paint::MAX_POSITIONED_CONTROL_MARKS_PER_RUN)
        .collect();
    (prefix, chars.next().is_none())
}


pub(crate) fn bounded_display_text_for_run(run: &TextRunNode) -> (String, bool) {
    let (source_prefix, source_complete) = bounded_text_prefix(run.display_or_text());
    let display_text = expand_pua_display_text(&source_prefix);
    let (display_prefix, display_complete) = bounded_text_prefix(&display_text);
    (display_prefix, source_complete && display_complete)
}


pub(crate) fn write_bounded_text_positions(buf: &mut String, text: &str, style: &TextStyle) -> bool {
    let (prefix, complete) = bounded_text_prefix(text);
    let positions = compute_char_positions(&prefix, style);
    write_position_values(buf, &positions);
    complete
}


pub(crate) fn display_text_for_text_run(run: &TextRunNode) -> Option<String> {
    let display_text = expand_pua_display_text(run.display_or_text());
    (display_text != run.text.as_str()).then_some(display_text)
}


pub(crate) fn effective_text_font_size_and_baseline(run: &TextRunNode) -> (f64, f64) {
    let base_font_size = if run.style.font_size > 0.0 {
        run.style.font_size
    } else {
        12.0
    };
    run.style.script_draw_metrics(base_font_size, run.baseline)
}


pub(crate) fn write_text_control_marks(buf: &mut String, bbox: BoundingBox, run: &TextRunNode) -> bool {
    let (bounded_text, mut complete) = bounded_text_prefix(&run.text);
    let positions = compute_char_positions(&bounded_text, &run.style);
    let font_size = if run.style.font_size > 0.0 {
        run.style.font_size
    } else {
        12.0
    };
    let mark_font_size = (font_size * 0.5).max(1.0);
    let has_end_mark = run.is_para_end || run.is_line_break_end;
    let inline_limit =
        crate::paint::MAX_POSITIONED_CONTROL_MARKS_PER_RUN.saturating_sub(has_end_mark as usize);
    let mut inline_count = 0usize;
    let mut wrote = false;
    buf.push('[');

    if run.field_marker == FieldMarkerType::None {
        for (index, ch) in bounded_text.chars().enumerate() {
            let (kind, glyph, x, size) = match ch {
                ' ' => {
                    let current_x = positions
                        .get(index)
                        .copied()
                        .unwrap_or_else(|| positions.last().copied().unwrap_or(0.0));
                    let next_x = positions.get(index + 1).copied().unwrap_or(bbox.width);
                    (
                        "space",
                        "\u{2228}",
                        (current_x + next_x) / 2.0 - mark_font_size * 0.25,
                        mark_font_size,
                    )
                }
                '\t' => (
                    "tab",
                    "\u{2192}",
                    positions
                        .get(index)
                        .copied()
                        .unwrap_or_else(|| positions.last().copied().unwrap_or(0.0)),
                    mark_font_size,
                ),
                _ => continue,
            };
            if inline_count >= inline_limit {
                complete = false;
                continue;
            }
            if wrote {
                buf.push(',');
            }
            let _ = write!(
                buf,
                "{{\"kind\":{},\"text\":{},\"x\":{:.3},\"y\":0.000,\"fontSize\":{:.3}}}",
                json_escape(kind),
                json_escape(glyph),
                x,
                size,
            );
            wrote = true;
            inline_count += 1;
        }
    }

    if run.is_para_end || run.is_line_break_end {
        if wrote {
            buf.push(',');
        }
        let (kind, glyph) = if run.is_line_break_end {
            ("lineBreakEnd", "\u{2193}")
        } else {
            ("paragraphEnd", "\u{21B5}")
        };
        let x = if run.text.is_empty() { 0.0 } else { bbox.width };
        let _ = write!(
            buf,
            "{{\"kind\":{},\"text\":{},\"x\":{:.3},\"y\":0.000,\"fontSize\":{:.3}}}",
            json_escape(kind),
            json_escape(glyph),
            x,
            font_size,
        );
    }
    buf.push(']');
    complete
}


pub(crate) fn write_text_legacy_visuals(
    buf: &mut String,
    run: &TextRunNode,
    leaf_visuals: &LeafTextVisualOps,
) {
    let has_decorations = run.style.underline != UnderlineType::None
        || run.style.strikethrough
        || run.style.emphasis_dot > 0;
    if run.char_overlap.is_none()
        && !leaf_visuals.control_marks
        && run.style.tab_leaders.is_empty()
        && !has_decorations
    {
        return;
    }

    buf.push_str(",\"legacyVisuals\":{");
    let mut wrote = false;
    if run.char_overlap.is_some() {
        let state = if leaf_visuals.char_overlap {
            "mirror"
        } else {
            "canonical"
        };
        let _ = write!(buf, "\"charOverlap\":{}", json_escape(state));
        wrote = true;
    }
    if leaf_visuals.control_marks {
        if wrote {
            buf.push(',');
        }
        buf.push_str("\"controlMarks\":\"mirror\"");
        wrote = true;
    }
    if !run.style.tab_leaders.is_empty() {
        if wrote {
            buf.push(',');
        }
        let state = if leaf_visuals.tab_leaders {
            "mirror"
        } else {
            "canonical"
        };
        let _ = write!(buf, "\"tabLeaders\":{}", json_escape(state));
        wrote = true;
    }
    if has_decorations {
        if wrote {
            buf.push(',');
        }
        let state = if leaf_visuals.decorations {
            "mirror"
        } else {
            "canonical"
        };
        let _ = write!(buf, "\"decorations\":{}", json_escape(state));
    }
    buf.push('}');
}


pub(crate) fn write_text_run_placement(buf: &mut String, bbox: BoundingBox, run: &TextRunNode) {
    let radians = run.rotation.to_radians();
    let (sin, cos) = radians.sin_cos();
    let local_origin_x = -bbox.width / 2.0;
    let local_origin_y = -bbox.height / 2.0 + run.baseline;
    let center_x = bbox.x + bbox.width / 2.0;
    let center_y = bbox.y + bbox.height / 2.0;
    let _ = write!(
        buf,
        "{{\"runToPage\":{{\"a\":{:.6},\"b\":{:.6},\"c\":{:.6},\"d\":{:.6},\"e\":{:.6},\"f\":{:.6}}},\"baselineY\":0.000000}}",
        cos,
        sin,
        -sin,
        cos,
        center_x + cos * local_origin_x - sin * local_origin_y,
        center_y + sin * local_origin_x + cos * local_origin_y,
    );
}


pub(crate) fn write_text_run_placement_value(buf: &mut String, placement: crate::paint::TextRunPlacement) {
    buf.push_str("{\"runToPage\":");
    write_affine_transform(buf, placement.run_to_page);
    let _ = write!(buf, ",\"baselineY\":{:.6}}}", placement.baseline_y);
}


pub(crate) fn write_text_clusters(buf: &mut String, run: &TextRunNode) {
    let positions = compute_char_positions(&run.text, &run.style);
    let mut utf16_start = 0_u32;
    let chars = run
        .text
        .char_indices()
        .map(|(offset, ch)| (offset as u32, ch))
        .collect::<Vec<_>>();

    buf.push('[');
    for (idx, (utf8_start, ch)) in chars.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        let utf8_end = chars
            .get(idx + 1)
            .map_or(run.text.len() as u32, |(next, _)| *next);
        let utf16_end = utf16_start + ch.len_utf16() as u32;
        let origin_x = positions.get(idx).copied().unwrap_or_default();
        let projection = text_projection_kind_str(run);
        buf.push_str("{\"sourceRangeUtf8\":");
        write_text_source_range(buf, TextSourceRange::new(*utf8_start, utf8_end));
        buf.push_str(",\"textRangeUtf8\":");
        write_text_source_range(buf, TextSourceRange::new(*utf8_start, utf8_end));
        buf.push_str(",\"textRangeUtf16\":");
        write_text_source_range(buf, TextSourceRange::new(utf16_start, utf16_end));
        let _ = write!(
            buf,
            ",\"projection\":{},\"origin\":{{\"x\":{:.6},\"y\":0.000000}}",
            json_escape(projection),
            origin_x
        );
        if let Some(next_x) = positions.get(idx + 1) {
            let _ = write!(
                buf,
                ",\"advance\":{{\"dx\":{:.6},\"dy\":0.000000}}",
                next_x - origin_x
            );
        }
        if run.char_overlap.is_some() {
            buf.push_str(",\"flags\":[\"specialVisual\",\"notShapingCandidate\"]");
        }
        buf.push('}');
        utf16_start = utf16_end;
    }
    buf.push(']');
}


pub(crate) fn write_glyph_clusters(buf: &mut String, clusters: &[GlyphCluster]) {
    buf.push('[');
    for (idx, cluster) in clusters.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        buf.push('{');
        buf.push_str("\"sourceRangeUtf8\":");
        write_text_source_range(buf, cluster.source_range_utf8);
        if let Some(range) = cluster.source_range_utf16 {
            buf.push_str(",\"sourceRangeUtf16\":");
            write_text_source_range(buf, range);
        }
        if let Some(range) = cluster.text_range_utf8 {
            buf.push_str(",\"textRangeUtf8\":");
            write_text_source_range(buf, range);
        }
        let _ = write!(
            buf,
            ",\"glyphRange\":{{\"start\":{},\"end\":{}}}",
            cluster.glyph_range.start, cluster.glyph_range.end
        );
        if !cluster.flags.is_empty() {
            buf.push_str(",\"flags\":[");
            for (flag_idx, flag) in cluster.flags.iter().enumerate() {
                if flag_idx > 0 {
                    buf.push(',');
                }
                let _ = write!(buf, "{}", json_escape(flag.as_str()));
            }
            buf.push(']');
        }
        buf.push('}');
    }
    buf.push(']');
}


pub(crate) fn write_glyph_outline_paths(buf: &mut String, paths: &[LayerGlyphOutlinePath]) {
    buf.push('[');
    for (idx, path) in paths.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        let _ = write!(buf, "{{\"glyphId\":{},\"sourceRangeUtf8\":", path.glyph_id);
        write_text_source_range(buf, path.source_range_utf8);
        let _ = write!(
            buf,
            ",\"glyphRange\":{{\"start\":{},\"end\":{}}},\"fillRule\":{}",
            path.glyph_range.start,
            path.glyph_range.end,
            json_escape(path.fill_rule.as_str())
        );
        buf.push_str(",\"commands\":");
        write_path_commands(buf, &path.commands);
        buf.push('}');
    }
    buf.push(']');
}


pub(crate) fn write_glyph_outline_stroke(buf: &mut String, stroke: &GlyphOutlineStrokeStyle) {
    let _ = write!(
        buf,
        "{{\"color\":{},\"width\":{:.6},\"join\":{},\"cap\":{},\"miterLimit\":{:.6},\"paintOrder\":{},\"strictSubset\":{}}}",
        json_escape(&color_ref_to_css(stroke.color)),
        stroke.width,
        json_escape(stroke.join.as_str()),
        json_escape(stroke.cap.as_str()),
        stroke.miter_limit,
        json_escape(stroke.paint_order.as_str()),
        stroke.is_strict_subset()
    );
}


pub(crate) fn write_bitmap_glyph_payload(buf: &mut String, payload: &BitmapGlyphPayload) {
    let _ = write!(
        buf,
        "{{\"imageRef\":{},\"sourceRangeUtf8\":",
        payload.image_ref.0
    );
    write_text_source_range(buf, payload.source_range_utf8);
    let _ = write!(
        buf,
        ",\"glyphRange\":{{\"start\":{},\"end\":{}}},\"placement\":",
        payload.glyph_range.start, payload.glyph_range.end
    );
    write_bbox(buf, payload.placement);
    let _ = write!(
        buf,
        ",\"alphaPremultiplied\":{},\"scalingPolicy\":{},\"filtering\":{},\"strictVisualContract\":{}",
        payload.alpha_premultiplied,
        json_escape(payload.scaling_policy.as_str()),
        json_escape(payload.filtering.as_str()),
        payload.has_strict_visual_contract()
    );
    if let Some(transform) = payload.transform_to_run {
        buf.push_str(",\"transformToRun\":");
        write_affine_transform(buf, transform);
    }
    buf.push('}');
}


pub(crate) fn write_svg_glyph_payload(buf: &mut String, payload: &SvgGlyphPayload) {
    let _ = write!(
        buf,
        "{{\"svgRef\":{},\"vectorResourceId\":{},\"sourceRangeUtf8\":",
        payload.svg_ref.0, payload.svg_ref.0
    );
    write_text_source_range(buf, payload.source_range_utf8);
    let _ = write!(
        buf,
        ",\"glyphRange\":{{\"start\":{},\"end\":{}}},\"viewBox\":",
        payload.glyph_range.start, payload.glyph_range.end
    );
    write_bbox(buf, payload.view_box);
    if let Some(size) = payload.intrinsic_size {
        let _ = write!(
            buf,
            ",\"intrinsicSize\":{{\"width\":{:.6},\"height\":{:.6}}}",
            size.dx, size.dy
        );
    }
    let _ = write!(
        buf,
        ",\"staticSanitized\":{},\"scriptAllowed\":{},\"animationAllowed\":{},\"externalResourcesAllowed\":{},\"interactivityAllowed\":{},\"staticSanitizedContract\":{}",
        payload.static_sanitized,
        payload.script_allowed,
        payload.animation_allowed,
        payload.external_resources_allowed,
        payload.interactivity_allowed,
        payload.has_static_sanitized_contract()
    );
    if let Some(transform) = payload.transform_to_run {
        buf.push_str(",\"transformToRun\":");
        write_affine_transform(buf, transform);
    }
    buf.push('}');
}


pub(crate) fn write_font_color_glyph_ref(buf: &mut String, value: &FontColorGlyphRef) {
    buf.push('{');
    let mut wrote = false;
    if let Some(face_key) = &value.face_key {
        let _ = write!(buf, "\"faceKey\":{}", json_escape(face_key));
        wrote = true;
    }
    if let Some(glyph_id) = value.glyph_id {
        if wrote {
            buf.push(',');
        }
        let _ = write!(buf, "\"glyphId\":{}", glyph_id);
        wrote = true;
    }
    if let Some(palette_index) = value.palette_index {
        if wrote {
            buf.push(',');
        }
        let _ = write!(buf, "\"paletteIndex\":{}", palette_index);
        wrote = true;
    }
    if let Some(color_format) = value.color_format {
        if wrote {
            buf.push(',');
        }
        let _ = write!(
            buf,
            "\"colorFormat\":{}",
            json_escape(color_format.as_str())
        );
    }
    buf.push('}');
}


pub(crate) fn write_glyph_transforms(buf: &mut String, transforms: &[GlyphTransform]) {
    buf.push('[');
    for (idx, transform) in transforms.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        let _ = write!(
            buf,
            "{{\"xx\":{:.6},\"xy\":{:.6},\"yx\":{:.6},\"yy\":{:.6},\"tx\":{:.6},\"ty\":{:.6}}}",
            transform.xx, transform.xy, transform.yx, transform.yy, transform.tx, transform.ty
        );
    }
    buf.push(']');
}


pub(crate) fn write_glyph_run_diagnostics(buf: &mut String, diagnostics: &GlyphRunDiagnostics) {
    let _ = write!(
        buf,
        "{{\"quality\":{},\"replayEligibility\":{},\"strictVisualEligible\":{},\"maxOriginDeltaPx\":{:.6},\"maxAdvanceDeltaPx\":{:.6},\"maxResidualAfterAdjustmentPx\":{:.6},\"clusterMismatchCount\":{},\"missingGlyphCount\":{},\"usedFallbackFontCount\":{}",
        json_escape(diagnostics.quality.as_str()),
        json_escape(diagnostics.replay_eligibility.as_str()),
        diagnostics.strict_visual_eligible,
        diagnostics.max_origin_delta_px,
        diagnostics.max_advance_delta_px,
        diagnostics.max_residual_after_adjustment_px,
        diagnostics.cluster_mismatch_count,
        diagnostics.missing_glyph_count,
        diagnostics.used_fallback_font_count,
    );
    if let Some(reason) = &diagnostics.reason {
        let _ = write!(buf, ",\"reason\":{}", json_escape(reason));
    }
    buf.push('}');
}


pub(crate) fn write_text_decoration(buf: &mut String, kind: TextDecorationKind, run: &TextRunNode) {
    let (color, shape, underline, emphasis_dot) = match kind {
        TextDecorationKind::Underline => (
            if run.style.underline_color != 0 {
                run.style.underline_color
            } else {
                run.style.color
            },
            run.style.underline_shape,
            run.style.underline,
            0,
        ),
        TextDecorationKind::Strikethrough => (
            if run.style.strike_color != 0 {
                run.style.strike_color
            } else {
                run.style.color
            },
            run.style.strike_shape,
            UnderlineType::None,
            0,
        ),
        TextDecorationKind::EmphasisDot => (
            run.style.color,
            0,
            UnderlineType::None,
            run.style.emphasis_dot,
        ),
    };
    let (font_size, baseline) = effective_text_font_size_and_baseline(run);
    let (bounded_text, complete) = bounded_display_text_for_run(run);
    let positions = compute_char_positions(&bounded_text, &run.style);
    let _ = write!(
        buf,
        "{{\"kind\":{},\"baseline\":{:.3},\"rotation\":{:.3},\"isVertical\":{},\"fontSize\":{:.3},\"ratio\":{:.6},\"color\":{},\"shape\":{},\"underline\":{},\"emphasisDot\":{},\"positions\":[",
        json_escape(kind.as_str()),
        baseline,
        run.rotation,
        run.is_vertical,
        font_size,
        run.style.ratio,
        json_escape(&color_ref_to_css(color)),
        shape,
        json_escape(underline_type_str(underline)),
        emphasis_dot,
    );
    for (idx, position) in positions.iter().enumerate() {
        if idx > 0 {
            buf.push(',');
        }
        let _ = write!(buf, "{:.3}", position);
    }
    let _ = write!(buf, "],\"positionsComplete\":{complete}}}");
}


pub(crate) fn write_equation_layout_box(buf: &mut String, layout: &LayoutBox) {
    let _ = write!(
        buf,
        "{{\"x\":{:.6},\"y\":{:.6},\"width\":{:.6},\"height\":{:.6},\"baseline\":{:.6},\"kind\":",
        layout.x, layout.y, layout.width, layout.height, layout.baseline,
    );
    write_equation_layout_kind(buf, &layout.kind);
    buf.push('}');
}


pub(crate) fn write_equation_layout_kind(buf: &mut String, kind: &LayoutKind) {
    match kind {
        LayoutKind::Row(children) => {
            buf.push_str("{\"type\":\"row\",\"children\":[");
            for (index, child) in children.iter().enumerate() {
                if index > 0 {
                    buf.push(',');
                }
                write_equation_layout_box(buf, child);
            }
            buf.push_str("]}");
        }
        LayoutKind::Text(text) => write_equation_text_kind(buf, "text", text),
        LayoutKind::Number(text) => write_equation_text_kind(buf, "number", text),
        LayoutKind::Symbol(text) => write_equation_text_kind(buf, "symbol", text),
        LayoutKind::MathSymbol(text) => write_equation_text_kind(buf, "mathSymbol", text),
        LayoutKind::Function(name) => {
            let _ = write!(
                buf,
                "{{\"type\":\"function\",\"name\":{}}}",
                json_escape(name)
            );
        }
        LayoutKind::Fraction { numer, denom } => {
            buf.push_str("{\"type\":\"fraction\",\"numer\":");
            write_equation_layout_box(buf, numer);
            buf.push_str(",\"denom\":");
            write_equation_layout_box(buf, denom);
            buf.push('}');
        }
        LayoutKind::Atop { top, bottom } => {
            buf.push_str("{\"type\":\"atop\",\"top\":");
            write_equation_layout_box(buf, top);
            buf.push_str(",\"bottom\":");
            write_equation_layout_box(buf, bottom);
            buf.push('}');
        }
        LayoutKind::Sqrt { index, body } => {
            buf.push_str("{\"type\":\"sqrt\"");
            if let Some(index) = index {
                buf.push_str(",\"index\":");
                write_equation_layout_box(buf, index);
            }
            buf.push_str(",\"body\":");
            write_equation_layout_box(buf, body);
            buf.push('}');
        }
        LayoutKind::Superscript { base, sup } => {
            write_equation_binary_kind(buf, "superscript", "base", base, "sup", sup);
        }
        LayoutKind::Subscript { base, sub } => {
            write_equation_binary_kind(buf, "subscript", "base", base, "sub", sub);
        }
        LayoutKind::SubSup { base, sub, sup } => {
            buf.push_str("{\"type\":\"subSup\",\"base\":");
            write_equation_layout_box(buf, base);
            buf.push_str(",\"sub\":");
            write_equation_layout_box(buf, sub);
            buf.push_str(",\"sup\":");
            write_equation_layout_box(buf, sup);
            buf.push('}');
        }
        LayoutKind::BigOp { symbol, sub, sup } => {
            let _ = write!(
                buf,
                "{{\"type\":\"bigOp\",\"symbol\":{}",
                json_escape(symbol)
            );
            write_optional_equation_box(buf, "sub", sub.as_deref());
            write_optional_equation_box(buf, "sup", sup.as_deref());
            buf.push('}');
        }
        LayoutKind::Limit { is_upper, sub } => {
            let _ = write!(buf, "{{\"type\":\"limit\",\"isUpper\":{}", is_upper);
            write_optional_equation_box(buf, "sub", sub.as_deref());
            buf.push('}');
        }
        LayoutKind::Matrix { cells, style } => {
            let _ = write!(
                buf,
                "{{\"type\":\"matrix\",\"style\":{},\"cells\":[",
                json_escape(equation_matrix_style(*style))
            );
            for (row_index, row) in cells.iter().enumerate() {
                if row_index > 0 {
                    buf.push(',');
                }
                buf.push('[');
                for (cell_index, cell) in row.iter().enumerate() {
                    if cell_index > 0 {
                        buf.push(',');
                    }
                    write_equation_layout_box(buf, cell);
                }
                buf.push(']');
            }
            buf.push_str("]}");
        }
        LayoutKind::Rel { arrow, over, under } => {
            buf.push_str("{\"type\":\"rel\",\"arrow\":");
            write_equation_layout_box(buf, arrow);
            buf.push_str(",\"over\":");
            write_equation_layout_box(buf, over);
            write_optional_equation_box(buf, "under", under.as_deref());
            buf.push('}');
        }
        LayoutKind::EqAlign { rows } => {
            buf.push_str("{\"type\":\"eqAlign\",\"rows\":[");
            for (index, (left, right)) in rows.iter().enumerate() {
                if index > 0 {
                    buf.push(',');
                }
                buf.push_str("{\"left\":");
                write_equation_layout_box(buf, left);
                buf.push_str(",\"right\":");
                write_equation_layout_box(buf, right);
                buf.push('}');
            }
            buf.push_str("]}");
        }
        LayoutKind::Paren { left, right, body } => {
            let _ = write!(
                buf,
                "{{\"type\":\"paren\",\"left\":{},\"right\":{},\"body\":",
                json_escape(left),
                json_escape(right),
            );
            write_equation_layout_box(buf, body);
            buf.push('}');
        }
        LayoutKind::Decoration { kind, body } => {
            let _ = write!(
                buf,
                "{{\"type\":\"decoration\",\"decoration\":{},\"body\":",
                json_escape(equation_decoration(*kind))
            );
            write_equation_layout_box(buf, body);
            buf.push('}');
        }
        LayoutKind::FontStyle { style, body } => {
            let _ = write!(
                buf,
                "{{\"type\":\"fontStyle\",\"fontStyle\":{},\"body\":",
                json_escape(equation_font_style(*style))
            );
            write_equation_layout_box(buf, body);
            buf.push('}');
        }
        LayoutKind::Space(width) => {
            let _ = write!(buf, "{{\"type\":\"space\",\"width\":{width:.6}}}");
        }
        LayoutKind::Newline => buf.push_str("{\"type\":\"newline\"}"),
        LayoutKind::Empty => buf.push_str("{\"type\":\"empty\"}"),
    }
}


pub(crate) fn write_equation_text_kind(buf: &mut String, kind: &str, text: &str) {
    let _ = write!(
        buf,
        "{{\"type\":{},\"text\":{}}}",
        json_escape(kind),
        json_escape(text)
    );
}


pub(crate) fn write_equation_binary_kind(
    buf: &mut String,
    kind: &str,
    left_name: &str,
    left: &LayoutBox,
    right_name: &str,
    right: &LayoutBox,
) {
    let _ = write!(
        buf,
        "{{\"type\":{},{}:",
        json_escape(kind),
        json_escape(left_name)
    );
    write_equation_layout_box(buf, left);
    let _ = write!(buf, ",{}:", json_escape(right_name));
    write_equation_layout_box(buf, right);
    buf.push('}');
}


pub(crate) fn write_optional_equation_box(buf: &mut String, name: &str, layout: Option<&LayoutBox>) {
    if let Some(layout) = layout {
        let _ = write!(buf, ",{}:", json_escape(name));
        write_equation_layout_box(buf, layout);
    }
}
