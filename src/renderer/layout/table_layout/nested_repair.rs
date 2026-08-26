//! nested_repair — table_layout.rs 에서 무변동 이동
use super::*;

/// A clipped table cell still has to expose an immediately nested table's
/// *outer border*. A nested table can begin after the host cell's left padding
/// while retaining its stored width, which puts that right border just beyond
/// the host cell's logical content rectangle. A completed nested table can
/// likewise end one border-width below an ancestor wrapper clip. Clipping at
/// the logical rectangle then removes the entire border even though the table
/// layout emitted it (issue2007 p2-p4, p9).
///
/// Do not expand to every descendant: a RowBreak continuation deliberately
/// keeps future-page text below its physical cell clip. This is restricted to
/// direct nested `Table` outer vertical `Line`s. When the outer clip expands,
/// direct nested `TableCell` content remains bounded by the host's original
/// horizontal viewport so the border exception cannot reveal a text tail.
/// The vertical correction is separately bounded to a terminal border that
/// misses the clip by at most six pixels.
fn extend_clipped_cell_horizontal_clip_to_nested_table_borders(cell_node: &mut RenderNode) {
    let RenderNodeType::TableCell(cell_meta) = &cell_node.node_type else {
        return;
    };
    if !cell_meta.clip {
        return;
    }

    let host_clip_left = cell_node.bbox.x;
    let host_clip_right = cell_node.bbox.x + cell_node.bbox.width;
    let mut clip_left = host_clip_left;
    let mut clip_right = host_clip_right;

    for table_node in &mut cell_node.children {
        if !matches!(table_node.node_type, RenderNodeType::Table(_)) {
            continue;
        }
        let table_left = table_node.bbox.x;
        let table_right = table_node.bbox.x + table_node.bbox.width;
        let mut found_outer_vertical_border = false;

        for border_node in &table_node.children {
            let RenderNodeType::Line(line) = &border_node.node_type else {
                continue;
            };
            // Cell-content lines can be arbitrary.  Only a near-vertical
            // table edge that sits on the nested table's left/right boundary
            // is eligible to enlarge the clipping viewport.
            if (line.x1 - line.x2).abs() > 0.01 || (line.y1 - line.y2).abs() < 1.0 {
                continue;
            }
            let x = line.x1;
            let outer_edge_tolerance = (line.style.width + 1.0).max(2.0);
            if (x - table_left).abs() > outer_edge_tolerance
                && (x - table_right).abs() > outer_edge_tolerance
            {
                continue;
            }
            let half_stroke = line.style.width / 2.0;
            clip_left = clip_left.min(x - half_stroke);
            clip_right = clip_right.max(x + half_stroke);
            found_outer_vertical_border = true;
        }

        if !found_outer_vertical_border {
            // 일부 normal/partial 표는 현재 부모 subtree가 최종 edge `Line`을
            // 붙이기 전에도 직접 child Table bbox를 완성한다(42065 p2-p3). 이
            // bbox는 table의 물리 stored-width 경계이므로 작은 stroke 여유만
            // 포함해 가로 clip의 fallback으로 쓸 수 있다. 세로 bbox는 전혀
            // 확장하지 않아 다음 쪽 continuation tail은 계속 가려진다.
            const FALLBACK_BORDER_HALF_STROKE_PX: f64 = 1.0;
            clip_left = clip_left.min(table_left - FALLBACK_BORDER_HALF_STROKE_PX);
            clip_right = clip_right.max(table_right + FALLBACK_BORDER_HALF_STROKE_PX);
        }

        if table_left < host_clip_left - NESTED_FRAGMENT_EDGE_EPSILON_PX
            || table_right > host_clip_right + NESTED_FRAGMENT_EDGE_EPSILON_PX
        {
            // Keep the direct child table's frame visible through the expanded
            // host clip, but never let its cell content paint past the parent
            // grid edge. The table's `Line` children stay outside this clamp.
            for nested_cell in &mut table_node.children {
                let RenderNodeType::TableCell(nested_meta) = &nested_cell.node_type else {
                    continue;
                };
                if !nested_meta.clip {
                    continue;
                }
                let content_left = nested_cell.bbox.x.max(host_clip_left);
                let content_right = (nested_cell.bbox.x + nested_cell.bbox.width)
                    .min(host_clip_right)
                    .max(content_left);
                nested_cell.bbox.x = content_left;
                nested_cell.bbox.width = content_right - content_left;
            }
        }
    }

    cell_node.bbox.x = clip_left;
    cell_node.bbox.width = (clip_right - clip_left).max(0.0);
}


pub(super) const NESTED_COMPLETED_BORDER_CLIP_OVERFLOW_PX: f64 = 6.0;


pub(super) fn extend_clipped_cell_vertical_clip_to_nearby_nested_table_borders(cell_node: &mut RenderNode) {
    let RenderNodeType::TableCell(cell_meta) = &cell_node.node_type else {
        return;
    };
    if !cell_meta.clip {
        return;
    }

    fn scan_table_borders(
        node: &RenderNode,
        clip_top: f64,
        clip_bottom: f64,
        extended_top: &mut f64,
        extended_bottom: &mut f64,
    ) {
        if matches!(node.node_type, RenderNodeType::Table(_)) {
            let table_left = node.bbox.x;
            let table_right = table_left + node.bbox.width;
            let table_top = node.bbox.y;
            let table_bottom = table_top + node.bbox.height;
            for child in &node.children {
                let RenderNodeType::Line(line) = &child.node_type else {
                    continue;
                };
                if (line.y1 - line.y2).abs() > NESTED_FRAGMENT_EDGE_EPSILON_PX
                    || (line.x1.min(line.x2) - table_left).abs() > NESTED_FRAGMENT_EDGE_EPSILON_PX
                    || (line.x1.max(line.x2) - table_right).abs() > NESTED_FRAGMENT_EDGE_EPSILON_PX
                {
                    continue;
                }
                let is_outer_top = (line.y1 - table_top).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX;
                let is_outer_bottom =
                    (line.y1 - table_bottom).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX;
                if !is_outer_top && !is_outer_bottom {
                    continue;
                }
                let half_stroke = line.style.width.max(NESTED_FRAGMENT_EDGE_EPSILON_PX) / 2.0;
                let paint_top = line.y1 - half_stroke;
                let paint_bottom = line.y1 + half_stroke;
                if paint_top < clip_top
                    && clip_top - paint_top <= NESTED_COMPLETED_BORDER_CLIP_OVERFLOW_PX
                {
                    *extended_top =
                        (*extended_top).min(paint_top - NESTED_FRAGMENT_FRAME_INSET_EPSILON_PX);
                }
                if paint_bottom > clip_bottom
                    && paint_bottom - clip_bottom <= NESTED_COMPLETED_BORDER_CLIP_OVERFLOW_PX
                {
                    *extended_bottom = (*extended_bottom)
                        .max(paint_bottom + NESTED_FRAGMENT_FRAME_INSET_EPSILON_PX);
                }
            }
        }
        for child in &node.children {
            scan_table_borders(child, clip_top, clip_bottom, extended_top, extended_bottom);
        }
    }

    let clip_top = cell_node.bbox.y;
    let clip_bottom = clip_top + cell_node.bbox.height;
    let mut extended_top = clip_top;
    let mut extended_bottom = clip_bottom;
    for child in &cell_node.children {
        scan_table_borders(
            child,
            clip_top,
            clip_bottom,
            &mut extended_top,
            &mut extended_bottom,
        );
    }
    cell_node.bbox.y = extended_top;
    cell_node.bbox.height = (extended_bottom - extended_top).max(0.0);
}


/// A table's logical stored width can end just inside a direct cell whose
/// horizontal paint extent was widened for a nested table border.  Preserve
/// that direct-cell extent on the table node before its parent cell computes
/// the next clip.  Without the post-order propagation, p10-p16's 1x1
/// continuation chain stops at the first table: the grandchild right edge is
/// correct, but the outer Cell and Body clips still cut it away.
///
/// Only direct `TableCell` children participate.  This deliberately does not
/// union arbitrary descendants (which could include next-page continuation
/// tails); it forwards the same horizontal paint boundary that the immediate
/// table already owns.
fn extend_table_horizontal_bbox_to_direct_cell_paint(table_node: &mut RenderNode) {
    if !matches!(table_node.node_type, RenderNodeType::Table(_)) {
        return;
    }

    let mut left = table_node.bbox.x;
    let mut right = table_node.bbox.x + table_node.bbox.width;
    for child in &table_node.children {
        if matches!(child.node_type, RenderNodeType::TableCell(_)) {
            left = left.min(child.bbox.x);
            right = right.max(child.bbox.x + child.bbox.width);
        }
    }
    table_node.bbox.x = left;
    table_node.bbox.width = (right - left).max(0.0);
}


pub(super) const NESTED_FRAGMENT_EDGE_EPSILON_PX: f64 = 0.5;


pub(super) const NESTED_FRAGMENT_RESIDUAL_BORDER_PX: f64 = 6.0;


pub(super) const NESTED_FRAGMENT_FRAME_INSET_EPSILON_PX: f64 = 0.05;


pub(super) const NESTED_FRAGMENT_FRAME_TARGET_EPSILON_PX: f64 = 0.05;


pub(super) fn push_fragment_border_line(
    tree: &mut LayoutFrame,
    table_node: &mut RenderNode,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    style: crate::renderer::LineStyle,
) {
    let width = style.width.max(NESTED_FRAGMENT_EDGE_EPSILON_PX);
    let bbox = BoundingBox::new(
        x1.min(x2),
        y1.min(y2),
        (x2 - x1).abs().max(width),
        (y2 - y1).abs().max(width),
    );
    table_node.children.push(RenderNode::new(
        tree.next_id(),
        RenderNodeType::Line(LineNode::new(x1, y1, x2, y2, style)),
        bbox,
    ));
}


pub(super) fn has_fragment_border_line(table_node: &RenderNode, x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    table_node.children.iter().any(|child| {
        matches!(
            &child.node_type,
            RenderNodeType::Line(line)
                if (line.x1 - x1).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.y1 - y1).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.x2 - x2).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.y2 - y2).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
        )
    })
}


/// Return the reconstructed frame coordinate whose full painted stroke stays
/// inside the active clip. `at_top` selects the clip's upper or lower edge.
fn fragment_horizontal_frame_y(
    clip_top: f64,
    clip_bottom: f64,
    style: &crate::renderer::LineStyle,
    at_top: bool,
) -> f64 {
    let inset = style.width.max(NESTED_FRAGMENT_EDGE_EPSILON_PX) / 2.0
        + NESTED_FRAGMENT_FRAME_INSET_EPSILON_PX;
    if at_top {
        clip_top + inset
    } else {
        clip_bottom - inset
    }
}


/// Only the borderless 1×1 RowBreak continuation contract needs a synthetic
/// bottom edge at a physical page clip.  A multi-cell nested table owns real
/// row boundaries on later fragments; turning a purely geometric
/// `ends_after_clip` result into a full-width rule exposes a premature terminal
/// border on the preceding page (#4159, issue2007 p2).
fn reconstructs_clipped_fragment_bottom(table_node: &RenderNode) -> bool {
    matches!(
        &table_node.node_type,
        RenderNodeType::Table(TableNode {
            row_count: 1,
            col_count: 1,
            ..
        })
    )
}


/// Make one full-width fragment edge paint-safe without producing a double
/// rule. Native table layout commonly leaves the source border exactly on the
/// clip edge.  The old broad `has_fragment_border_line` tolerance treated that
/// clipped source line as equivalent to the reconstructed one, so no usable
/// frame was emitted (issue2007 p11/p14).  Prefer moving that exact source
/// line inward; add a line only when the source did not retain one.
fn ensure_fragment_horizontal_frame_inside_clip(
    tree: &mut LayoutFrame,
    table_node: &mut RenderNode,
    table_left: f64,
    table_right: f64,
    clip_edge_y: f64,
    frame_y: f64,
    style: crate::renderer::LineStyle,
) {
    let has_target = table_node.children.iter().any(|child| {
        matches!(
            &child.node_type,
            RenderNodeType::Line(line)
                if child.visible
                    && (line.y1 - line.y2).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.x1.min(line.x2) - table_left).abs()
                        <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.x1.max(line.x2) - table_right).abs()
                        <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.y1 - frame_y).abs()
                        <= NESTED_FRAGMENT_FRAME_TARGET_EPSILON_PX
        )
    });
    if has_target {
        return;
    }

    if let Some(source_line) = table_node.children.iter_mut().find(|child| {
        matches!(
            &child.node_type,
            RenderNodeType::Line(line)
                if child.visible
                    && (line.y1 - line.y2).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.x1.min(line.x2) - table_left).abs()
                        <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.x1.max(line.x2) - table_right).abs()
                        <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && (line.y1 - clip_edge_y).abs()
                        <= NESTED_FRAGMENT_EDGE_EPSILON_PX
        )
    }) {
        let delta_y = frame_y - source_line.bbox.y;
        if let RenderNodeType::Line(line) = &mut source_line.node_type {
            line.y1 += delta_y;
            line.y2 += delta_y;
        }
        source_line.bbox.y += delta_y;
        return;
    }

    push_fragment_border_line(
        tree,
        table_node,
        table_left,
        frame_y,
        table_right,
        frame_y,
        style,
    );
}


pub(super) fn translate_render_subtree_y(node: &mut RenderNode, delta_y: f64) {
    node.bbox.y += delta_y;
    if let RenderNodeType::Line(line) = &mut node.node_type {
        line.y1 += delta_y;
        line.y2 += delta_y;
    }
    for child in &mut node.children {
        translate_render_subtree_y(child, delta_y);
    }
}


/// A `vpos=0` line with no text is the explicit empty spacer stored between
/// a completed nested table and its following source block.
fn is_empty_vpos_spacer_line(node: &RenderNode) -> bool {
    matches!(&node.node_type, RenderNodeType::TextLine(line) if line.vpos == Some(0))
        && node.children.iter().all(|child| {
            matches!(&child.node_type, RenderNodeType::TextRun(run) if run.text.trim().is_empty())
        })
}


pub(super) const NESTED_HEADING_WITH_TABLE_MAX_GAP_PX: f64 = 32.0;


pub(super) const NESTED_HEADING_WITH_TABLE_TOP_INSET_PX: f64 = 4.0;


pub(super) const CLIPPED_TEXT_INK_TOP_OVERFLOW_PX: f64 = 4.0;


pub(super) const CLIPPED_TEXT_INK_TOP_INSET_PX: f64 = 0.25;


pub(super) fn text_line_has_non_whitespace_text(node: &RenderNode) -> bool {
    matches!(node.node_type, RenderNodeType::TextLine(_))
        && node.children.iter().any(|child| {
            matches!(&child.node_type, RenderNodeType::TextRun(run) if !run.text.trim().is_empty())
        })
}


/// A complete source line belongs to the next RowBreak fragment when only a
/// sub-glyph sliver reaches the current clipped cell.  SVG/Canvas would still
/// paint that sliver because clipping works on ink geometry, producing a
/// duplicate heading at the preceding page bottom (issue2007 p16 -> p17).
///
/// Limit this to the same six-pixel residue window used for a future table
/// border: it never hides a usable line, and the successor fragment remains
/// responsible for the full line.
fn suppress_bottom_clipped_text_residue(node: &mut RenderNode, clip_bottom: f64) {
    for child in &mut node.children {
        if !child.visible || !text_line_has_non_whitespace_text(child) {
            continue;
        }
        let line_top = child.bbox.y;
        let line_bottom = line_top + child.bbox.height;
        let visible_sliver = clip_bottom - line_top;
        if line_top < clip_bottom - NESTED_FRAGMENT_EDGE_EPSILON_PX
            && line_bottom > clip_bottom + NESTED_FRAGMENT_EDGE_EPSILON_PX
            && visible_sliver < NESTED_FRAGMENT_RESIDUAL_BORDER_PX
        {
            child.visible = false;
        }
    }
}


/// A continued nested-cell line can retain its prior-page absolute y while a
/// one-pixel ink tail reaches this physical cell.  The outer page clip then
/// hides the full line even though the source owner is this fragment
/// (`rowbreak-problem-pages.hwpx` p8).  Rebase only that crossing line to the
/// cell top; older, wholly off-page sibling lines remain hidden.
///
/// This is deliberately independent of the cell's `clip` flag.  A nested
/// table can inherit its effective viewport from an ancestor RowBreak cell,
/// leaving the inner cell itself unclipped in the render tree.
fn rebase_nested_cell_top_residue_line(node: &mut RenderNode) {
    if !matches!(node.node_type, RenderNodeType::TableCell(_)) {
        return;
    }

    let cell_top = node.bbox.y;
    let has_wholly_off_top_predecessor = node.children.iter().any(|child| {
        child.visible
            && text_line_has_non_whitespace_text(child)
            && child.bbox.y + child.bbox.height < cell_top - NESTED_FRAGMENT_EDGE_EPSILON_PX
    });
    if !has_wholly_off_top_predecessor {
        return;
    }

    for child in &mut node.children {
        if !child.visible || !text_line_has_non_whitespace_text(child) {
            continue;
        }
        let line_top = child.bbox.y;
        let line_bottom = line_top + child.bbox.height;
        if line_top < cell_top
            && cell_top - line_top <= CLIPPED_TEXT_INK_TOP_OVERFLOW_PX * 4.0
            && line_bottom >= cell_top - NESTED_FRAGMENT_EDGE_EPSILON_PX
        {
            translate_render_subtree_y(child, cell_top + CLIPPED_TEXT_INK_TOP_INSET_PX - line_top);
        }
    }
}


/// Preserve source ownership at a clipped cell's title/table seam and keep a
/// first visible glyph out of the ancestor Canvas/SVG clip.
///
/// This operates only on direct source siblings.  It never grows the clip:
/// prior-page text remains hidden and a future-page tail cannot be exposed.
fn repair_clipped_cell_text_table_seam(node: &mut RenderNode, suppress_bottom_text_residue: bool) {
    let is_clipped_cell = matches!(
        &node.node_type,
        RenderNodeType::TableCell(TableCellNode { clip: true, .. })
    );
    if !is_clipped_cell {
        return;
    }

    let clip_top = node.bbox.y;
    let clip_bottom = clip_top + node.bbox.height;

    for table_index in 0..node.children.len() {
        if !node.children[table_index].visible
            || !matches!(
                node.children[table_index].node_type,
                RenderNodeType::Table(_)
            )
        {
            continue;
        }
        let table_top = node.children[table_index].bbox.y;
        let title_index = (0..table_index).rev().find(|&index| {
            node.children[index].visible && text_line_has_non_whitespace_text(&node.children[index])
        });
        let Some(title_index) = title_index else {
            continue;
        };
        // A real intervening text paragraph owns its own page boundary.  Only
        // a title followed by empty host lines and the next table is movable.
        if node.children[title_index + 1..table_index]
            .iter()
            .any(text_line_has_non_whitespace_text)
        {
            continue;
        }
        let title_top = node.children[title_index].bbox.y;
        let title_bottom = title_top + node.children[title_index].bbox.height;
        if table_top + NESTED_FRAGMENT_EDGE_EPSILON_PX < title_bottom
            || table_top - title_bottom > NESTED_HEADING_WITH_TABLE_MAX_GAP_PX
        {
            continue;
        }

        if table_top >= clip_bottom - NESTED_FRAGMENT_EDGE_EPSILON_PX
            && title_top >= clip_top - NESTED_FRAGMENT_EDGE_EPSILON_PX
            && title_bottom <= clip_bottom + NESTED_FRAGMENT_EDGE_EPSILON_PX
        {
            // The table has no paintable content in this fragment.  Its title
            // belongs to the next fragment with the table rather than to this
            // page's last line.
            for child in &mut node.children[title_index..=table_index] {
                child.visible = false;
            }
            continue;
        }

        if title_top < clip_top
            && clip_top - title_top <= NESTED_HEADING_WITH_TABLE_MAX_GAP_PX
            && table_top >= clip_top - NESTED_FRAGMENT_EDGE_EPSILON_PX
        {
            // Move the complete source group, retaining its title-to-table
            // spacing.  Moving only the text would detach it from the table;
            // expanding the cell clip would replay preceding-page content.
            let target_top = clip_top + NESTED_HEADING_WITH_TABLE_TOP_INSET_PX;
            let delta_y = target_top - title_top;
            for child in &mut node.children[title_index..=table_index] {
                translate_render_subtree_y(child, delta_y);
            }
        }
    }

    for child in &mut node.children {
        if !child.visible || !text_line_has_non_whitespace_text(child) {
            continue;
        }
        let line_top = child.bbox.y;
        if line_top < clip_top && clip_top - line_top <= CLIPPED_TEXT_INK_TOP_OVERFLOW_PX {
            translate_render_subtree_y(child, clip_top + CLIPPED_TEXT_INK_TOP_INSET_PX - line_top);
        }
    }
    if suppress_bottom_text_residue {
        suppress_bottom_clipped_text_residue(node, clip_bottom);
    }
}


/// Suppress a next-fragment table whose first border only grazes the current
/// clipped cell.  A Canvas clip can still anti-alias a fraction of that border
/// even when the table's logical top is just below the clip, leaving a stray
/// horizontal rule at the preceding page's bottom (issue2007 p8).
///
/// This is deliberately limited to a table beginning no more than one border
/// residue window below the physical clip.  A separate page render owns that
/// table with a fresh viewport; no current-page text or usable table area is
/// discarded here.
fn suppress_future_nested_table_border_residue(node: &mut RenderNode, clip_bottom: f64) {
    for child in &mut node.children {
        if !child.visible {
            continue;
        }
        if matches!(child.node_type, RenderNodeType::Table(_)) {
            let table_top = child.bbox.y;
            if table_top >= clip_bottom - NESTED_FRAGMENT_EDGE_EPSILON_PX
                && table_top - clip_bottom <= NESTED_FRAGMENT_RESIDUAL_BORDER_PX
            {
                child.visible = false;
                continue;
            }
        }
        suppress_future_nested_table_border_residue(child, clip_bottom);
    }
}


/// Reconstruct one table's physical fragment frame inside an ancestor
/// `TableCell` clip.  A 1×1 RowBreak wrapper often has no border of its own:
/// the paintable frame belongs to a deeper table in its cell.  Consequently
/// this helper intentionally accepts the ancestor clip rather than requiring
/// the table itself to own it (42065 p10-p14).
fn reconstruct_nested_table_fragment_frame(
    tree: &mut LayoutFrame,
    table_node: &mut RenderNode,
    clip_top: f64,
    clip_bottom: f64,
) {
    let table_top = table_node.bbox.y;
    let table_bottom = table_top + table_node.bbox.height;
    if table_bottom <= clip_top + NESTED_FRAGMENT_EDGE_EPSILON_PX
        || table_top >= clip_bottom - NESTED_FRAGMENT_EDGE_EPSILON_PX
    {
        return;
    }
    let starts_before_clip = table_top < clip_top - NESTED_FRAGMENT_EDGE_EPSILON_PX;
    let ends_after_clip = table_bottom > clip_bottom + NESTED_FRAGMENT_EDGE_EPSILON_PX;
    if !starts_before_clip && !ends_after_clip {
        return;
    }

    let fragment_top = table_top.max(clip_top);
    let fragment_bottom = table_bottom.min(clip_bottom);
    let fragment_height = fragment_bottom - fragment_top;
    if fragment_height <= NESTED_FRAGMENT_EDGE_EPSILON_PX {
        return;
    }
    // Preserve the source-flow seam rule from the direct wrapper repair. A
    // sub-line tail belongs to the preceding page, even when its visible
    // border is owned by a deeper descendant table; rebuilding it here would
    // turn that tail into a false top frame on p10/p13.
    if starts_before_clip && fragment_height < NESTED_FRAGMENT_RESIDUAL_BORDER_PX {
        return;
    }

    let table_left = table_node.bbox.x;
    let table_right = table_left + table_node.bbox.width;
    let mut horizontal_style = None;
    let mut left_style = None;
    let mut right_style = None;
    for child in &table_node.children {
        let RenderNodeType::Line(line) = &child.node_type else {
            continue;
        };
        let horizontal = (line.y2 - line.y1).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX;
        let vertical = (line.x2 - line.x1).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX;
        if horizontal
            && (line.x1.min(line.x2) - table_left).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
            && (line.x1.max(line.x2) - table_right).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
            && horizontal_style.is_none()
        {
            horizontal_style = Some(line.style.clone());
        }
        if vertical && (line.x1 - table_left).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX {
            left_style.get_or_insert_with(|| line.style.clone());
        }
        if vertical && (line.x1 - table_right).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX {
            right_style.get_or_insert_with(|| line.style.clone());
        }
    }

    // Keep the horizontal centreline inside the clip by half a stroke.  A
    // centreline exactly at the SVG/Canvas clip edge otherwise loses its
    // anti-aliased outer half, and at some device scales the entire rule.
    let frame_top = horizontal_style.as_ref().map_or(fragment_top, |style| {
        if starts_before_clip {
            fragment_horizontal_frame_y(clip_top, clip_bottom, style, true)
        } else {
            fragment_top
        }
    });
    let frame_bottom = horizontal_style.as_ref().map_or(fragment_bottom, |style| {
        if ends_after_clip {
            fragment_horizontal_frame_y(clip_top, clip_bottom, style, false)
        } else {
            fragment_bottom
        }
    });

    if let Some(style) = horizontal_style.as_ref() {
        if starts_before_clip {
            ensure_fragment_horizontal_frame_inside_clip(
                tree,
                table_node,
                table_left,
                table_right,
                clip_top,
                frame_top,
                style.clone(),
            );
        }
        if ends_after_clip && reconstructs_clipped_fragment_bottom(table_node) {
            ensure_fragment_horizontal_frame_inside_clip(
                tree,
                table_node,
                table_left,
                table_right,
                clip_bottom,
                frame_bottom,
                style.clone(),
            );
        }
    }
    if let Some(style) = left_style {
        if !has_fragment_border_line(table_node, table_left, frame_top, table_left, frame_bottom) {
            push_fragment_border_line(
                tree,
                table_node,
                table_left,
                frame_top,
                table_left,
                frame_bottom,
                style,
            );
        }
    }
    if let Some(style) = right_style {
        if !has_fragment_border_line(
            table_node,
            table_right,
            frame_top,
            table_right,
            frame_bottom,
        ) {
            push_fragment_border_line(
                tree,
                table_node,
                table_right,
                frame_top,
                table_right,
                frame_bottom,
                style,
            );
        }
    }
}


/// Visit only descendants: the direct child is handled by the original seam
/// repair first, while this pass reaches the real bordered table below an
/// unbordered RowBreak wrapper.
fn reconstruct_nested_table_descendant_fragment_frames(
    tree: &mut LayoutFrame,
    node: &mut RenderNode,
    clip_top: f64,
    clip_bottom: f64,
) {
    for child in &mut node.children {
        if matches!(child.node_type, RenderNodeType::Table(_)) {
            reconstruct_nested_table_fragment_frame(tree, child, clip_top, clip_bottom);
        }
        reconstruct_nested_table_descendant_fragment_frames(tree, child, clip_top, clip_bottom);
    }
}


/// Repair the frame and source-flow seam of a true nested-table continuation.
///
/// A direct nested table keeps its document-global coordinates even when its
/// owning 1×1 RowBreak cell is a clipped page fragment. SVG/Canvas therefore
/// naturally retains old table geometry, but loses the new fragment's frame:
/// the source top or bottom can lie beyond the physical clip rectangle. A
/// few-pixel terminal remnant is the inverse case and must be suppressed.
///
/// The source also retains the completed table's following empty `vpos=0`
/// spacer.  If only a sub-line table tail reaches the new viewport, that
/// consumed spacer otherwise starts a second time in the new cell and moves
/// the next real source block down by one line advance (42065 p10/p13).
/// Normalize that exact two-line seam after layout; it changes neither the
/// pagination cut nor a non-empty text line's ownership.
///
/// This runs after native table edges are emitted. It never changes a table
/// with current-page content, and the small source-spacer translation is
/// limited to the direct siblings following a suppressed residual tail.
fn repair_clipped_nested_table_fragment_frame(
    tree: &mut LayoutFrame,
    node: &mut RenderNode,
    suppress_bottom_text_residue: bool,
    repair_unclipped_hwpx_top_residue: bool,
) {
    if repair_unclipped_hwpx_top_residue {
        rebase_nested_cell_top_residue_line(node);
    }
    let is_clipped_cell = matches!(
        &node.node_type,
        RenderNodeType::TableCell(TableCellNode { clip: true, .. })
    );
    if !is_clipped_cell {
        return;
    }

    let clip_top = node.bbox.y;
    let clip_bottom = node.bbox.y + node.bbox.height;
    repair_clipped_cell_text_table_seam(node, suppress_bottom_text_residue);
    suppress_future_nested_table_border_residue(node, clip_bottom);
    for table_index in 0..node.children.len() {
        if !node.children[table_index].visible
            || !matches!(
                node.children[table_index].node_type,
                RenderNodeType::Table(_)
            )
        {
            continue;
        }

        let table_node = &mut node.children[table_index];

        let table_top = table_node.bbox.y;
        let table_bottom = table_top + table_node.bbox.height;
        if table_bottom <= clip_top + NESTED_FRAGMENT_EDGE_EPSILON_PX
            || table_top >= clip_bottom - NESTED_FRAGMENT_EDGE_EPSILON_PX
        {
            continue;
        }
        let starts_before_clip = table_top < clip_top - NESTED_FRAGMENT_EDGE_EPSILON_PX;
        let ends_after_clip = table_bottom > clip_bottom + NESTED_FRAGMENT_EDGE_EPSILON_PX;
        if !starts_before_clip && !ends_after_clip {
            continue;
        }
        let fragment_top = table_top.max(clip_top);
        let fragment_bottom = table_bottom.min(clip_bottom);
        let fragment_height = fragment_bottom - fragment_top;
        if fragment_height <= NESTED_FRAGMENT_EDGE_EPSILON_PX {
            continue;
        }

        if starts_before_clip && fragment_height < NESTED_FRAGMENT_RESIDUAL_BORDER_PX {
            // Only a sub-line tail reaches this page. Hiding its source edge
            // prevents the previous table's bottom border from appearing as a
            // false top border without affecting any current-page content.
            table_node.visible = false;

            // The tail's following empty `vpos=0` source line was consumed on
            // the preceding fragment.  The renderer still lays it out at the
            // new cell's top, so its line box and following advance are
            // incorrectly paid twice. Drop that empty spacer and shift its
            // following source siblings by precisely those two stored
            // advances. This is
            // deliberately tighter than a global continuation offset: p9 has
            // a real spacer at its new block and must retain it, while p10/p13
            // have an actual table tail in this viewport.
            let following_lines: Vec<(f64, f64)> = node
                .children
                .iter()
                .skip(table_index + 1)
                .filter(|child| child.visible && child.bbox.y >= clip_top)
                .filter(|child| matches!(child.node_type, RenderNodeType::TextLine(_)))
                .map(|child| (child.bbox.y, child.bbox.height))
                .collect();
            if let (Some((spacer_y, _)), Some((next_line_y, _))) =
                (following_lines.first(), following_lines.get(1))
            {
                let spacer_index = node
                    .children
                    .iter()
                    .enumerate()
                    .skip(table_index + 1)
                    .find(|(_, child)| {
                        child.visible
                            && (child.bbox.y - *spacer_y).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                    })
                    .map(|(index, _)| index);
                let line_advance = next_line_y - spacer_y;
                if spacer_index
                    .is_some_and(|index| is_empty_vpos_spacer_line(&node.children[index]))
                    && spacer_y - clip_top <= 24.0
                    && line_advance > NESTED_FRAGMENT_EDGE_EPSILON_PX
                    && line_advance <= 16.0
                {
                    let source_seam_height = line_advance * 2.0;
                    let spacer_index = spacer_index.expect("checked empty spacer index");
                    node.children[spacer_index].visible = false;
                    for child in node.children.iter_mut().skip(spacer_index + 1) {
                        translate_render_subtree_y(child, -source_seam_height);
                    }
                }
            }
            continue;
        }

        let table_left = table_node.bbox.x;
        let table_right = table_left + table_node.bbox.width;
        let mut horizontal_style = None;
        let mut left_style = None;
        let mut right_style = None;
        for child in &table_node.children {
            let RenderNodeType::Line(line) = &child.node_type else {
                continue;
            };
            let horizontal = (line.y2 - line.y1).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX;
            let vertical = (line.x2 - line.x1).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX;
            if horizontal
                && (line.x1.min(line.x2) - table_left).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                && (line.x1.max(line.x2) - table_right).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX
                && horizontal_style.is_none()
            {
                horizontal_style = Some(line.style.clone());
            }
            if vertical && (line.x1 - table_left).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX {
                left_style.get_or_insert_with(|| line.style.clone());
            }
            if vertical && (line.x1 - table_right).abs() <= NESTED_FRAGMENT_EDGE_EPSILON_PX {
                right_style.get_or_insert_with(|| line.style.clone());
            }
        }

        // Place reconstructed horizontal centerlines half a stroke inside the
        // clip. SVG clipPath and Canvas clip both otherwise remove half (and
        // at some device scales all) of a line placed exactly on the boundary.
        // The sub-pixel inset retains the physical edge while keeping paint
        // independent of browser anti-aliasing rules.
        let frame_top = horizontal_style.as_ref().map_or(fragment_top, |style| {
            if starts_before_clip {
                fragment_horizontal_frame_y(clip_top, clip_bottom, style, true)
            } else {
                fragment_top
            }
        });
        let frame_bottom = horizontal_style.as_ref().map_or(fragment_bottom, |style| {
            if ends_after_clip {
                fragment_horizontal_frame_y(clip_top, clip_bottom, style, false)
            } else {
                fragment_bottom
            }
        });

        // The original vertical sides already intersect the cell clip in most
        // renderers. Emit them again at fragment boundaries nevertheless: an
        // SVG/Canvas clip exactly on the edge otherwise yields an open frame.
        if let Some(style) = horizontal_style.as_ref() {
            if starts_before_clip {
                ensure_fragment_horizontal_frame_inside_clip(
                    tree,
                    table_node,
                    table_left,
                    table_right,
                    clip_top,
                    frame_top,
                    style.clone(),
                );
            }
            if ends_after_clip && reconstructs_clipped_fragment_bottom(table_node) {
                ensure_fragment_horizontal_frame_inside_clip(
                    tree,
                    table_node,
                    table_left,
                    table_right,
                    clip_bottom,
                    frame_bottom,
                    style.clone(),
                );
            }
        }
        if let Some(style) = left_style {
            if !has_fragment_border_line(
                table_node,
                table_left,
                frame_top,
                table_left,
                frame_bottom,
            ) {
                push_fragment_border_line(
                    tree,
                    table_node,
                    table_left,
                    frame_top,
                    table_left,
                    frame_bottom,
                    style,
                );
            }
        }
        if let Some(style) = right_style {
            if !has_fragment_border_line(
                table_node,
                table_right,
                frame_top,
                table_right,
                frame_bottom,
            ) {
                push_fragment_border_line(
                    tree,
                    table_node,
                    table_right,
                    frame_top,
                    table_right,
                    frame_bottom,
                    style,
                );
            }
        }
    }

    // The immediate 1×1 RowBreak table can be an unbordered structural
    // wrapper.  Its descendant table owns the visible rule, so apply the same
    // physical fragment-frame contract below that wrapper as well.
    reconstruct_nested_table_descendant_fragment_frames(tree, node, clip_top, clip_bottom);
}


/// Run the narrow horizontal clip correction only after every nested table in
/// the current subtree has emitted its border edges. Calling the single-cell
/// helper during the parent cell loop is too early for normal edge rendering:
/// p2-p3's 4×2/9×2 tables append their `Line`s after that loop and therefore
/// retained the undersized wrapper clip. The traversal stays post-order and
/// only delegates to the direct-child-table helper above, so it cannot widen a
/// continuation's vertical viewport or reveal a future-page text tail.
pub(crate) fn extend_completed_nested_table_border_clips(
    tree: &mut LayoutFrame,
    node: &mut RenderNode,
    suppress_bottom_text_residue: bool,
    repair_unclipped_hwpx_top_residue: bool,
) {
    for child in &mut node.children {
        extend_completed_nested_table_border_clips(
            tree,
            child,
            suppress_bottom_text_residue,
            repair_unclipped_hwpx_top_residue,
        );
    }
    extend_table_horizontal_bbox_to_direct_cell_paint(node);
    extend_clipped_cell_horizontal_clip_to_nested_table_borders(node);
    extend_clipped_cell_vertical_clip_to_nearby_nested_table_borders(node);
    repair_clipped_nested_table_fragment_frame(
        tree,
        node,
        suppress_bottom_text_residue,
        repair_unclipped_hwpx_top_residue,
    );
}
