//! geometry — table_layout.rs 에서 무변동 이동
use super::*;

pub(crate) fn should_render_table_caption(table: &crate::model::table::Table, depth: usize) -> bool {
    depth == 0
        || (depth == 1
            && table
                .caption
                .as_ref()
                .is_some_and(caption_has_topbottom_picture))
}


pub(crate) fn caption_flow_extra(caption: &Option<Caption>, caption_height: f64, caption_spacing: f64) -> f64 {
    let is_lr_caption = caption.as_ref().is_some_and(|cap| {
        matches!(
            cap.direction,
            CaptionDirection::Left | CaptionDirection::Right
        )
    });
    if is_lr_caption || caption_height <= 0.0 {
        0.0
    } else {
        caption_height + caption_spacing
    }
}


pub(crate) fn top_caption_flow_extra(
    caption: &Option<Caption>,
    caption_height: f64,
    caption_spacing: f64,
) -> f64 {
    if caption
        .as_ref()
        .is_some_and(|cap| matches!(cap.direction, CaptionDirection::Top))
    {
        caption_flow_extra(caption, caption_height, caption_spacing)
    } else {
        0.0
    }
}


pub(crate) fn build_col_row_y_from_cell_heights(
    table: &crate::model::table::Table,
    row_heights: &[f64],
    row_y: &[f64],
    col_count: usize,
    row_count: usize,
    cell_spacing: f64,
    dpi: f64,
) -> Vec<Vec<f64>> {
    let mut cell_height_grid = vec![vec![None::<f64>; row_count]; col_count];
    for (cell_idx, cell) in table.cells.iter().enumerate() {
        if cell.row_span == 1
            && cell.col_span == 1
            && cell.height < 0x8000_0000
            && (cell.col as usize) < col_count
            && (cell.row as usize) < row_count
        {
            let render_height = table
                .local_resize_cell_heights
                .iter()
                .find(|(idx, _)| *idx == cell_idx)
                .map(|(_, height)| *height)
                .unwrap_or(cell.height);
            cell_height_grid[cell.col as usize][cell.row as usize] =
                Some(hwpunit_to_px(render_height as i32, dpi));
        }
    }

    let fallback_h = hwpunit_to_px(400, dpi);
    let target_total = if table.common.height > 0 {
        hwpunit_to_px(table.common.height as i32, dpi)
            + cell_spacing * row_count.saturating_sub(1) as f64
    } else {
        row_y.last().copied().unwrap_or(0.0)
    };
    let mut col_row_y = vec![vec![0.0f64; row_count + 1]; col_count];
    for c in 0..col_count {
        let col_idx = c as u16;
        if !table.local_resize_cols.contains(&col_idx) {
            col_row_y[c].clone_from_slice(row_y);
            continue;
        }
        for r in 0..row_count {
            let h = cell_height_grid[c][r]
                .or_else(|| row_heights.get(r).copied())
                .unwrap_or(fallback_h);
            col_row_y[c][r + 1] =
                col_row_y[c][r] + h + if r + 1 < row_count { cell_spacing } else { 0.0 };
        }
        // 저장 파일의 cell.height는 표 전체 높이와 맞지 않는 보조값일 수 있다.
        // 열별 누적 높이가 표 외곽과 맞을 때만 독립 horizontal segment로 해석한다.
        if (col_row_y[c][row_count] - target_total).abs() > 0.5 && row_y.len() == row_count + 1 {
            col_row_y[c].clone_from_slice(row_y);
        }
    }
    col_row_y
}


pub(crate) fn has_independent_col_row_y(col_row_y: &[Vec<f64>], row_y: &[f64]) -> bool {
    col_row_y.iter().any(|cy| {
        cy.iter()
            .zip(row_y.iter())
            .any(|(a, b)| (a - b).abs() > 0.01)
    })
}


pub(crate) fn border_style_has_diagonal(bs: &ResolvedBorderStyle) -> bool {
    let slash_bits = (bs.diagonal_attr >> 2) & 0x07;
    let backslash_bits = (bs.diagonal_attr >> 5) & 0x07;
    (slash_bits != 0 || backslash_bits != 0 || bs.center_line != CenterLine::None)
        && bs.diagonal.diagonal_type != 0
}


pub(crate) fn border_style_has_center_line_only(bs: &ResolvedBorderStyle) -> bool {
    let slash_bits = (bs.diagonal_attr >> 2) & 0x07;
    let backslash_bits = (bs.diagonal_attr >> 5) & 0x07;
    bs.diagonal.diagonal_type != 0
        && bs.center_line != CenterLine::None
        && slash_bits == 0
        && backslash_bits == 0
}


/// cellzone 대각선은 영역 전체에 한 번 그리고, 원본 중복 BF가 붙는 시작 셀만 숨긴다.
pub(crate) fn mark_cellzone_diagonal_origin_coverage(
    covered: &mut [Vec<bool>],
    start_row: usize,
    start_col: usize,
) {
    if let Some(row) = covered.get_mut(start_row) {
        if let Some(cell) = row.get_mut(start_col) {
            *cell = true;
        }
    }
}


pub(crate) fn cell_span_has_cellzone_diagonal(
    covered: &[Vec<bool>],
    row: usize,
    col: usize,
    row_span: usize,
    col_span: usize,
    row_count: usize,
    col_count: usize,
) -> bool {
    let end_row = (row + row_span).min(row_count);
    let end_col = (col + col_span).min(col_count);
    (row..end_row).any(|rr| {
        (col..end_col).any(|cc| {
            covered
                .get(rr)
                .and_then(|cells| cells.get(cc))
                .copied()
                .unwrap_or(false)
        })
    })
}


pub(crate) fn border_style_has_center_line(bs: &ResolvedBorderStyle) -> bool {
    bs.center_line != CenterLine::None && bs.diagonal.diagonal_type != 0
}


pub(crate) fn table_grid_cell_has_own_diagonal(
    table: &crate::model::table::Table,
    styles: &ResolvedStyleSet,
    row: usize,
    col: usize,
    zone_border_fill_id: u16,
) -> bool {
    table.cells.iter().any(|cell| {
        let start_row = cell.row as usize;
        let end_row = start_row + cell.row_span as usize;
        let start_col = cell.col as usize;
        let end_col = start_col + cell.col_span as usize;
        if row < start_row
            || row >= end_row
            || col < start_col
            || col >= end_col
            || cell.border_fill_id == 0
            || cell.border_fill_id == zone_border_fill_id
        {
            return false;
        }
        styles
            .border_styles
            .get((cell.border_fill_id as usize).saturating_sub(1))
            .is_some_and(border_style_has_diagonal)
    })
}


pub(crate) fn cellzone_diagonal_fully_overridden_by_cells(
    table: &crate::model::table::Table,
    styles: &ResolvedStyleSet,
    start_row: usize,
    end_row: usize,
    start_col: usize,
    end_col: usize,
    zone_border_fill_id: u16,
) -> bool {
    start_row < end_row
        && start_col < end_col
        && (start_row..end_row).all(|row| {
            (start_col..end_col).all(|col| {
                table_grid_cell_has_own_diagonal(table, styles, row, col, zone_border_fill_id)
            })
        })
}

pub(crate) fn render_cell_box_borders(
    tree: &mut LayoutFrame,
    bs: &ResolvedBorderStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Vec<RenderNode> {
    let mut nodes = Vec::new();
    nodes.extend(create_border_line_nodes(
        tree,
        &bs.borders[2],
        x,
        y,
        x + w,
        y,
    ));
    nodes.extend(create_border_line_nodes(
        tree,
        &bs.borders[3],
        x,
        y + h,
        x + w,
        y + h,
    ));
    nodes.extend(create_border_line_nodes(
        tree,
        &bs.borders[0],
        x,
        y,
        x,
        y + h,
    ));
    nodes.extend(create_border_line_nodes(
        tree,
        &bs.borders[1],
        x + w,
        y,
        x + w,
        y + h,
    ));
    nodes
}
