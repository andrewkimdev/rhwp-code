//! unit_row_cuts — table_layout.rs 에서 무변동 이동
use super::*;

impl LayoutEngine {
    /// 셀 배경 렌더링 (fill_color + pattern + gradient)
    pub(crate) fn render_cell_background(
        &self,
        tree: &mut LayoutFrame,
        cell_node: &mut RenderNode,
        border_style: Option<&crate::renderer::style_resolver::ResolvedBorderStyle>,
        cell_x: f64,
        cell_y: f64,
        cell_w: f64,
        cell_h: f64,
        bin_data_content: &[BinDataContent],
    ) {
        let fill_color = border_style.and_then(|bs| bs.fill_color);
        let pattern = border_style.and_then(|bs| bs.pattern);
        let gradient = border_style.and_then(|bs| bs.gradient.clone());
        if fill_color.is_some() || gradient.is_some() || pattern.is_some() {
            let rect_id = tree.next_id();
            let rect_node = RenderNode::new(
                rect_id,
                RenderNodeType::Rectangle(RectangleNode::new(
                    0.0,
                    ShapeStyle {
                        fill_color,
                        pattern,
                        stroke_color: None,
                        stroke_width: 0.0,
                        ..Default::default()
                    },
                    gradient,
                )),
                BoundingBox::new(cell_x, cell_y, cell_w, cell_h),
            );
            cell_node.children.push(rect_node);
        }
        // [Task #429] image fill 처리 — zone 처리와 동일 패턴
        if let Some(img_fill) = border_style.and_then(|bs| bs.image_fill.as_ref()) {
            if let Some(img_bytes) = find_bin_data_bytes(bin_data_content, img_fill.bin_data_id) {
                let img_id = tree.next_id();
                let img_node = RenderNode::new(
                    img_id,
                    RenderNodeType::Image(ImageNode {
                        fill_mode: Some(img_fill.fill_mode),
                        brightness: img_fill.brightness,
                        contrast: img_fill.contrast,
                        effect: img_fill.effect,
                        ..ImageNode::new(img_fill.bin_data_id, Some(img_bytes))
                    }),
                    BoundingBox::new(cell_x, cell_y, cell_w, cell_h),
                );
                cell_node.children.push(img_node);
            }
        }
    }


    pub(crate) fn delay_empty_anchor_topandbottom_flow_units_before_hard_break(
        units: Vec<CellUnit>,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
    ) -> Vec<CellUnit> {
        if !matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) || table.common.treat_as_char
        {
            return units;
        }
        let mut has_future_visible_hard_break = vec![false; units.len()];
        let mut seen_visible_hard_break = false;
        for idx in (0..units.len()).rev() {
            has_future_visible_hard_break[idx] = seen_visible_hard_break;
            let unit = &units[idx];
            if unit.hard_break_before && unit.vis_start < unit.vis_end {
                seen_visible_hard_break = true;
            }
        }

        let mut reordered = Vec::with_capacity(units.len());
        let mut pending = Vec::new();
        for (idx, unit) in units.into_iter().enumerate() {
            if has_future_visible_hard_break[idx]
                && Self::is_delayable_empty_anchor_topandbottom_flow_unit(cell, &unit)
            {
                pending.push(unit);
                continue;
            }
            if unit.hard_break_before && unit.vis_start < unit.vis_end && !pending.is_empty() {
                reordered.append(&mut pending);
            }
            reordered.push(unit);
        }
        reordered.append(&mut pending);
        reordered
    }


    pub(crate) fn is_delayable_empty_anchor_topandbottom_flow_unit(
        cell: &crate::model::table::Cell,
        unit: &CellUnit,
    ) -> bool {
        if !Self::is_non_inline_control_flow_unit(unit) {
            return false;
        }
        let Some(para) = cell.paragraphs.get(unit.para_idx) else {
            return false;
        };
        para.text.trim().is_empty()
            && para.controls.iter().any(|control| match control {
                Control::Picture(pic) => {
                    !pic.common.treat_as_char
                        && pic.common.flow_with_text
                        && matches!(pic.common.text_wrap, TextWrap::TopAndBottom)
                        && matches!(pic.common.vert_rel_to, VertRelTo::Para)
                }
                Control::Shape(shape) => {
                    let common = shape.common();
                    !common.treat_as_char
                        && common.flow_with_text
                        && matches!(common.text_wrap, TextWrap::TopAndBottom)
                        && matches!(common.vert_rel_to, VertRelTo::Para)
                }
                _ => false,
            })
    }


    /// [#2097] 셀 문단 cp_idx 의 첫 유닛 앞까지의 누적 콘텐츠 높이(셀-로컬).
    /// 각주 앵커 문단이 컷 조각에 포함되는 경계(인서트-인지 컷 예산 상한) 산정용.
    /// 해당 문단 유닛이 없으면 None.
    pub(crate) fn cell_para_unit_offset(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        cp_idx: usize,
    ) -> Option<f64> {
        let units = self.cell_units(cell, table, styles);
        let mut h = 0.0f64;
        for u in units.iter() {
            if u.para_idx >= cp_idx {
                return Some(h);
            }
            h += u.height;
        }
        None
    }


    pub(crate) fn cell_units_content_height(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
    ) -> f64 {
        self.cell_units(cell, table, styles)
            .iter()
            .map(|unit| unit.height)
            .sum()
    }


    /// [Task #1718] RowBreak 셀에서 용량을 살짝 넘긴 "가시 꼬리줄"에 over-fill grace 를
    /// 줄지 판정한다.
    ///
    /// 원래 grace 조건은 `units[j+1..].any(spacer)` — 뒤 어딘가에 빈 문단 spacer 가
    /// 하나라도 있으면 grace 였다. 이 때문에 654문단 거대 셀(spacer 가 문서 전체에
    /// 흩어져 있음)에서는 연속 본문 한복판에서도 항상 grace 가 걸려 페이지당 +1~5줄
    /// over-fill → under-pagination(승강기 별표27: rhwp 40 vs 한글 48).
    ///
    /// 반대로 `all(spacer)` 로 좁히면 caption 줄 + 개체(그림상자) 앞의 spacer 처럼
    /// 뒤에 가시/개체 유닛이 남아 있는 진짜 구조적 꼬리줄까지 무너뜨린다
    /// (rowbreak-problem-pages 13쪽 회귀).
    ///
    /// 정답 판별: 오버플로 꼬리줄 다음 "첫 spacer 전까지"의 유닛과, spacer 뒤에
    /// 본문이 계속되는지를 함께 본다.
    /// - spacer 가 없다 → 순수 본문 꼬리 → grace 거부.
    /// - 그 사이가 전부 가시 텍스트 줄의 끊김 없는 연속(run) → 본문 한복판 → grace 거부.
    /// - spacer 가 바로 뒤여도 spacer run 뒤에 다시 가시 본문이 이어지면 문단 사이
    ///   빈 줄일 뿐이므로 grace 거부.
    /// - spacer 뒤가 문서/셀 끝이거나, 첫 spacer 전후에 비가시 유닛(개체/중첩/오브젝트
    ///   높이 등)이 끼어 있으면 → 구조적 꼬리줄 → grace 유지.
    pub(crate) fn grace_visible_tail_before_spacer(units: &[CellUnit], j: usize) -> bool {
        let Some(first_spacer) = units[j + 1..].iter().position(|u| u.empty_spacer) else {
            return false;
        };
        if first_spacer > 0 {
            // spacer 전에 비가시 유닛이 끼면 구조적 꼬리로 본다.
            return !units[j + 1..j + 1 + first_spacer]
                .iter()
                .all(|u| u.vis_start < u.vis_end);
        }

        // 오버플로 줄 바로 뒤가 spacer 인 경우에도, spacer run 뒤에 다시 일반 가시 본문이
        // 이어지면 문단 사이 빈 줄이므로 페이지 예산을 넘겨 끌어올리지 않는다.
        let after_spacers = units[j + 1..]
            .iter()
            .position(|u| !u.empty_spacer)
            .map(|idx| j + 1 + idx);
        match after_spacers {
            None => true,
            Some(idx) => {
                let next = &units[idx];
                !(next.vis_start < next.vis_end && !next.mixed_nested_fragment)
            }
        }
    }


    /// [#1921] 예산 정지 유닛 `j` 부터 다음 저장 hard-break 유닛까지의 잔여 높이가
    /// 소량(오버플로 한도 48px)이면 `(흡수 후 높이, hard-break 유닛 인덱스)` 를 반환한다.
    ///
    /// 저장 hard-break 는 한글이 실제로 페이지를 넘긴 지점이므로, 그 직전의 극소 잔여
    /// 유닛은 한글 기준으로 현재 페이지에 담겨 있었다. 흡수하지 않으면 다음 fragment 가
    /// 그 잔여만 담은 sliver 페이지(59043 pi=160: 22px/쪽)가 되어 과분할된다.
    pub(crate) fn absorb_tail_before_stored_hard_break(
        units: &[CellUnit],
        j: usize,
        h: f64,
        avail_height: f64,
    ) -> Option<(f64, usize)> {
        const SLIVER_ABSORB_OVERFLOW_TOLERANCE_PX: f64 = 48.0;
        let mut extra = 0.0f64;
        for (k, u) in units.iter().enumerate().skip(j) {
            if k > j && u.hard_break_before {
                return Some((h + extra, k));
            }
            extra += u.height;
            if h + extra > avail_height + SLIVER_ABSORB_OVERFLOW_TOLERANCE_PX {
                return None;
            }
        }
        None
    }


    pub(crate) fn is_non_inline_control_flow_unit(unit: &CellUnit) -> bool {
        unit.vis_start == unit.vis_end
            && !unit.empty_spacer
            && unit.nested_row.is_none()
            && !unit.mixed_nested_fragment
            && !unit.mixed_nested_trailing
            && unit.mixed_nested_content_height <= 0.0
    }


    /// `unit_idx`가 tagged Square/Tight/Through control의 source entry라면, 그
    /// control이 차지하는 마지막 generic unit의 exclusive index를 돌려준다. 같은
    /// 16px fragment가 경계에서 두 control range에 걸칠 수 있으므로, entry가 되는
    /// control 모두의 마지막 unit 중 가장 뒤를 사용한다.
    pub(crate) fn entering_non_inline_control_range_end(units: &[CellUnit], unit_idx: usize) -> Option<usize> {
        let unit = units.get(unit_idx)?;
        let (first_control, last_control) = unit.non_inline_control_range?;
        let mut range_end = None;
        for control_idx in first_control..=last_control {
            let control_start = units.iter().position(|candidate| {
                candidate.para_idx == unit.para_idx
                    && candidate
                        .non_inline_control_range
                        .is_some_and(|(first, last)| first <= control_idx && control_idx <= last)
            });
            if control_start != Some(unit_idx) {
                continue;
            }
            let control_end = units
                .iter()
                .rposition(|candidate| {
                    candidate.para_idx == unit.para_idx
                        && candidate
                            .non_inline_control_range
                            .is_some_and(|(first, last)| {
                                first <= control_idx && control_idx <= last
                            })
                })
                .map(|idx| idx + 1)?;
            range_end = Some(range_end.unwrap_or(0).max(control_end));
        }
        range_end
    }


    pub(crate) fn would_orphan_non_inline_flow_before_spacer(
        units: &[CellUnit],
        j: usize,
        consumed_height: f64,
        avail_height: f64,
    ) -> bool {
        let Some(next) = units.get(j + 1) else {
            return false;
        };
        Self::is_non_inline_control_flow_unit(&units[j])
            && next.empty_spacer
            && !next.hard_break_before
            && consumed_height + units[j].height <= avail_height
            && consumed_height + units[j].height + next.height > avail_height
    }


    pub(crate) fn rewind_rowbreak_fragment_tail_before_topandbottom_flow(
        table: &crate::model::table::Table,
        units: &[CellUnit],
        start: usize,
        avail_height: f64,
        j: &mut usize,
        h: &mut f64,
    ) -> bool {
        if !matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) || table.common.treat_as_char
            || *j >= units.len()
            || *j <= start + 1
            || !units[*j].top_and_bottom_flow
        {
            return false;
        }

        let Some(prev_idx) = units[start..*j]
            .iter()
            .rposition(|unit| !unit.empty_spacer)
            .map(|idx| start + idx)
        else {
            return false;
        };
        if prev_idx + 1 < *j
            && !units[prev_idx + 1..*j]
                .iter()
                .all(|unit| unit.empty_spacer && !unit.hard_break_before)
        {
            return false;
        }

        let prev = &units[prev_idx];
        if prev.top_and_bottom_flow || !Self::is_non_inline_control_flow_unit(prev) {
            return false;
        }
        let fragment_run = prev.height <= 16.5
            || (prev_idx > start
                && units[prev_idx - 1].para_idx == prev.para_idx
                && Self::is_non_inline_control_flow_unit(&units[prev_idx - 1])
                && !units[prev_idx - 1].top_and_bottom_flow);
        if !fragment_run {
            return false;
        }

        let rewind_h: f64 = units[prev_idx..*j].iter().map(|unit| unit.height).sum();
        let rewound_h = *h - rewind_h;
        const MAX_REWIND_BLANK_PX: f64 = 96.0;
        let max_rewind_blank = MAX_REWIND_BLANK_PX.max(units[*j].height * 0.4);
        if avail_height - rewound_h > max_rewind_blank {
            return false;
        }
        *h = rewound_h;
        *j = prev_idx;
        true
    }


    /// Mixed 1×1 nested-cell projection에서 완결 중첩 표와 뒤 tail을 fresh page로
    /// 함께 이월한다.
    ///
    /// 상위 `CellUnit`에는 깊은 자식 문단 index가 남지 않는다. 대신 자식 표 뒤의
    /// 첫 실제 unit만 `mixed_nested_starts_after_table` ownership marker를 보존한다.
    /// 표 자체가 현재 쪽에 들어가더라도 출처·설명·뒤 표를 더는 담지 못하면, 표를
    /// 현재 쪽 하단에 소비해 다음 쪽에서 순서가 뒤집힌다.
    pub(crate) fn rewind_rowbreak_mixed_nested_table_tail_for_fresh_page(
        table: &crate::model::table::Table,
        units: &[CellUnit],
        start: usize,
        fresh_page_height: f64,
        j: &mut usize,
        h: &mut f64,
    ) -> bool {
        if !matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) || table.common.treat_as_char
            // 이 rewind는 outer RowBreak 표가 아니라, 한 셀짜리 mixed nested
            // projection 자체의 table atom/tail 경계만 다룬다. 다열 본문 표에도
            // marker가 우연히 전파될 수 있는데(21217935: 17×4), 거기서 한 cell의
            // tail을 이월하면 COM 기준 8쪽을 9쪽으로 늘린다.
            || table.row_count != 1
            || table.col_count != 1
            || *j <= start + 1
            || *j >= units.len()
            || !fresh_page_height.is_finite()
            || fresh_page_height <= 0.0
        {
            return false;
        }

        let Some(after_table) = units[start..*j]
            .iter()
            .rposition(|unit| unit.mixed_nested_fragment && unit.mixed_nested_starts_after_table)
            .map(|index| start + index)
        else {
            return false;
        };
        let Some(after_source_para) = units[after_table].mixed_nested_source_para_idx else {
            return false;
        };
        let Some(last_table_atom) = (start..after_table).rev().find(|&index| {
            let unit = &units[index];
            unit.mixed_nested_fragment
                && !unit.mixed_nested_trailing
                && unit.mixed_nested_content_height > 0.5
                && unit
                    .mixed_nested_source_para_idx
                    .is_some_and(|source_para| source_para != after_source_para)
        }) else {
            return false;
        };
        let Some(table_source_para) = units[last_table_atom].mixed_nested_source_para_idx else {
            return false;
        };
        let mut table_atom = last_table_atom;
        while table_atom > start
            && units[table_atom - 1].mixed_nested_fragment
            && units[table_atom - 1].mixed_nested_source_para_idx == Some(table_source_para)
        {
            table_atom -= 1;
        }
        let Some(end_before_table) = table_atom.checked_sub(1).filter(|end| *end > start) else {
            return false;
        };
        let tail = &units[table_atom..];
        let tail_height: f64 = tail.iter().map(|unit| unit.height).sum();
        if tail.iter().any(|unit| unit.hard_break_before) || tail_height > fresh_page_height + 0.5 {
            return false;
        }
        *h = units[start..=end_before_table]
            .iter()
            .map(|unit| unit.height)
            .sum();
        *j = end_before_table;
        true
    }


    pub(crate) fn should_absorb_midpage_saved_vpos_reset(
        &self,
        table: &crate::model::table::Table,
        unit: &CellUnit,
        consumed_height: f64,
        avail_height: f64,
        allow_midpage_reset_absorb: bool,
    ) -> bool {
        // RowBreak 셀에는 한컴 저장 LINE_SEG vertical_pos 리셋이 남아 있다.
        // 대부분은 쪽 경계 근처의 저장 페이지 경계로 보존해야 하지만, 현재 조각이
        // 페이지 절반도 채우지 못한 중간 리셋은 같은 쪽 안의 로컬 좌표 재시작으로
        // 보는 편이 기준 PDF와 맞다. 파일명/쪽번호가 아니라 저장 위치와 현재 예산에
        // 근거해 구분한다.
        allow_midpage_reset_absorb
            && matches!(
                table.page_break,
                crate::model::table::TablePageBreak::RowBreak
            )
            && !unit.empty_spacer
            && unit.vis_start < unit.vis_end
            && avail_height.is_finite()
            && avail_height > 0.0
            && (avail_height - consumed_height) > avail_height * 0.5
    }


    /// [Task #993] 분할 표 행 컷을 전진시킨다 — 분할 표 페이지네이션의 단일 권위 함수.
    ///
    /// `start_cut`(이전 페이지까지 셀별 소비 유닛 수)에서 시작해, 각 셀을 공통
    /// 높이 예산 `avail_height` 안에서 동시 전진시킨다. 어느 유닛도 `avail_height`
    /// 안에 안 들어가면 진행 보장을 위해 셀당 유닛 1개는 강제 소비한다. vpos
    /// 리셋(hard break)을 만나면 그 셀은 거기서 정지한다.
    ///
    /// 페이지네이터(분할 판정)와 렌더러(가시 범위)가 모두 이 함수를 호출하므로
    /// 두 경로의 컷이 정의상 일치한다.
    pub(crate) fn advance_row_cut(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        start_cut: &[usize],
        avail_height: f64,
        styles: &ResolvedStyleSet,
    ) -> RowCutResult {
        let issue2424_started = issue2424_profile_enabled().then(std::time::Instant::now);
        let result = self.advance_row_cut_inner(table, row, start_cut, avail_height, styles);
        if let Some(started) = issue2424_started {
            use std::sync::atomic::Ordering::Relaxed;
            ISSUE2424_ADVANCE_ROW_CUT_CALLS.fetch_add(1, Relaxed);
            ISSUE2424_ADVANCE_ROW_CUT_NANOS.fetch_add(started.elapsed().as_nanos() as u64, Relaxed);
        }
        result
    }


    pub(crate) fn advance_row_cut_inner(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        start_cut: &[usize],
        avail_height: f64,
        styles: &ResolvedStyleSet,
    ) -> RowCutResult {
        let mut row_cells: Vec<&crate::model::table::Cell> = table
            .cells
            .iter()
            .filter(|c| c.row as usize == row && c.row_span == 1)
            .collect();
        row_cells.sort_by_key(|c| c.col);

        let mut end_cut: RowCut = Vec::with_capacity(row_cells.len());
        let mut hit_hard_break = false;
        let mut fully_consumed = true;
        let mut consumed_height = 0.0f64;
        const HARD_BREAK_REMAINING_TOLERANCE_PX: f64 = 32.0;
        const ROWBREAK_VISIBLE_TAIL_OVERFLOW_TOLERANCE_PX: f64 = 120.0;
        let row_has_top_and_bottom_flow = row_cells
            .iter()
            .any(|cell| self.cell_has_top_and_bottom_non_inline_flow(cell));
        // [#3820 Stage 11] native HWP5의 1×1 TopAndBottom RowBreak float는 셀 문단
        // 경계의 `양수 vpos -> 0`을 실제 물리 쪽 전환으로 저장하는 경우가 있다.
        // 일반 1~2열 RowBreak 표에 적용하는 relaxed rule은 이 reset을 "공간이 남은
        // 로컬 재시작"으로 흡수하지만, p172의 `<OPTN>` 뒤 `간 특수 검사`처럼 다음
        // fragment의 내용을 기존 FootnoteArea 위에 과적재한다. 표 내부의 같은 문단
        // 줄 reset과 달리 **문단 경계** reset이고, native HWP5·empty-host float가
        // 갖는 1×1 저장 형상일 때만 relaxed rule에서 제외한다.
        let native_hwp5_single_cell_topbottom_cross_para_reset = self
            .profile
            .get()
            .native_hwp5_layout()
            && !table.common.treat_as_char
            && matches!(
                table.page_break,
                crate::model::table::TablePageBreak::RowBreak
            )
            && matches!(
                table.common.text_wrap,
                crate::model::shape::TextWrap::TopAndBottom
            )
            && table.row_count == 1
            && table.col_count == 1
            && row_cells.len() == 1
            && row_cells[0].paragraphs.windows(2).any(|pair| {
                match (
                    pair[0].line_segs.iter().rev().find(|seg| {
                        seg.tag & crate::model::paragraph::LineSeg::TAG_IMPLEMENTATION_PROPERTY == 0
                    }),
                    pair[1].line_segs.iter().find(|seg| {
                        seg.tag & crate::model::paragraph::LineSeg::TAG_IMPLEMENTATION_PROPERTY == 0
                    }),
                ) {
                    (Some(previous), Some(current)) => {
                        previous.vertical_pos > 0 && current.vertical_pos <= 0
                    }
                    _ => false,
                }
            });
        let relaxed_hard_break = matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) && (table.col_count <= 2 || table.row_count > 5)
            && !row_has_top_and_bottom_flow
            && !native_hwp5_single_cell_topbottom_cross_para_reset;
        let allow_midpage_reset_absorb =
            self.profile.get().hwpx_stored_layout() || row_has_top_and_bottom_flow;
        let rewind_internal_hard_break_orphan = Self::row_has_prior_rowspan_cover(table, row);
        let native_hwp5_atomic_non_inline_entry = self.profile.get().native_hwp5_layout()
            && matches!(
                table.page_break,
                crate::model::table::TablePageBreak::RowBreak
            )
            && !table.common.treat_as_char;
        for (i, cell) in row_cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let start = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            let mut j = start;
            let mut h = 0.0f64;
            while j < units.len() {
                let u = &units[j];
                // 시작 유닛(j==start)은 항상 소비 — 진행 보장.
                if start > 0
                    && u.empty_spacer
                    && !u.hard_break_before
                    && units[start..=j].iter().all(|unit| unit.empty_spacer)
                {
                    j += 1;
                    continue;
                }
                if start > 0
                    && u.empty_spacer
                    && !u.hard_break_before
                    && units[j..]
                        .iter()
                        .all(|unit| unit.empty_spacer && !unit.hard_break_before)
                {
                    j = units.len();
                    break;
                }
                // CellUnit fragment는 Square/Tight/Through control의 source range를
                // 나눌 수 있지만 renderer는 entry fragment에서 picture 전체를 한 번
                // emit한다. native HWP5 RowBreak에서 entry만 넣으면 cell clip이 picture
                // 를 자르고 continuation은 owner가 없어지는 반쪽 control이 된다. 이미
                // content를 소비한 page에서는 range 전체가 fit할 때만 시작한다. page보다
                // 큰 control의 fresh fragment(start/h==0)는 기존 progress 경로를 보존한다.
                if native_hwp5_atomic_non_inline_entry && h > 0.5 {
                    if let Some(control_end) =
                        Self::entering_non_inline_control_range_end(&units, j)
                    {
                        let control_height: f64 =
                            units[j..control_end].iter().map(|unit| unit.height).sum();
                        if h + control_height > avail_height + 0.5 {
                            break;
                        }
                    }
                }
                // [Task #1658] 미세 fragment 낭비 페이지 방지: 거대 셀이 페이지를 가로질러 분할될
                // 때 셀 내용 vpos reset(hard_break_before)이 촘촘하면, 잔여공간이 충분한데도 reset 마다
                // 페이지를 끊어 2줄 이하만 담은 낭비 페이지가 양산된다(법령 별표 거대 셀:
                // 별표1 5→4쪽, 산업통상부 별표4 33→27쪽). 흡수 임계: continuation(start>0, 셀 중간
                // 조각)은 ≤3 유닛, fresh(start==0)는 ≤2 유닛. continuation 의 reset 은 셀 내부
                // page-wrap 인데 rhwp 가 한글 break 보다 1~3줄 일찍 capacity-break 하여 reset 직전
                // 1~3줄 orphan 을 만든다(한글 COM 대조: 한글 break @line 5/40/75 vs rhwp 3·6/74·76).
                // fresh 의 ≤2 는 #1488(가시 문단 사이 reset 3유닛 후 보존)을 깨지 않도록 유지한다.
                let waste_thresh = if start > 0 { 3 } else { 2 };
                let tiny_fragment_waste = j <= start + waste_thresh
                    && !u.empty_spacer
                    && h + u.height <= avail_height
                    && avail_height - h > HARD_BREAK_REMAINING_TOLERANCE_PX;
                // [#4069 Stage 2/3] 재귀 투영된 자식 표의 문단 내부 저장 프레임
                // 경계(p10), 그리고 1×1 자식 표 host 직후의 저장 프레임 경계(p15)는
                // relaxed/absorb 규칙으로 넘지 않는다. 그 밖의 문단 사이 hard break의
                // orphan/sliver 완화와 직접 표 셀의 기존 적응은 유지한다.
                let follows_single_cell_nested_host = u
                    .para_idx
                    .checked_sub(1)
                    .and_then(|para_idx| cell.paragraphs.get(para_idx))
                    .is_some_and(Self::paragraph_hosts_single_cell_nested_table);
                let strict_saved_frame_break = u.stored_frame_break_before
                    && (u.mixed_nested_recursive || follows_single_cell_nested_host);
                if j > start
                    && u.hard_break_before
                    && (strict_saved_frame_break
                        || ((rewind_internal_hard_break_orphan
                            || !relaxed_hard_break
                            || (!u.empty_spacer
                                && (h + u.height > avail_height
                                    || avail_height - h <= HARD_BREAK_REMAINING_TOLERANCE_PX)))
                            && !units[start..j].iter().all(|unit| unit.empty_spacer)
                            && !tiny_fragment_waste))
                {
                    if !strict_saved_frame_break
                        && self.should_absorb_midpage_saved_vpos_reset(
                            table,
                            u,
                            h,
                            avail_height,
                            allow_midpage_reset_absorb,
                        )
                    {
                        h += u.height;
                        j += 1;
                        continue;
                    }
                    if rewind_internal_hard_break_orphan {
                        Self::rewind_rowbreak_orphan_before_hard_break(
                            table,
                            &units,
                            start,
                            avail_height,
                            rewind_internal_hard_break_orphan,
                            &mut j,
                            &mut h,
                        );
                    }
                    hit_hard_break = true;
                    break;
                }
                if j > start && h + u.height > avail_height {
                    let visible_tail_before_spacer = relaxed_hard_break
                        && !u.empty_spacer
                        && u.vis_start < u.vis_end
                        && h + u.height
                            <= avail_height + ROWBREAK_VISIBLE_TAIL_OVERFLOW_TOLERANCE_PX
                        && Self::grace_visible_tail_before_spacer(&units, j);
                    if visible_tail_before_spacer {
                        h += u.height;
                        j += 1;
                        continue;
                    }
                    if Self::rewind_rowbreak_mixed_nested_table_tail_for_fresh_page(
                        table,
                        &units,
                        start,
                        self.current_body_area.get().3.max(avail_height),
                        &mut j,
                        &mut h,
                    ) {
                        break;
                    }
                    // [#1921] sliver 흡수는 with_row_offsets 경로에만 적용한다. 이 walk 는
                    // relaxed_hard_break(hard-break 조건부 무시) 의미론이라 다음 break 로의
                    // 흡수가 비정상 경계를 강제한다(86712 공식PDF 65→66 회귀 실증).
                    break;
                }
                if j > start
                    && Self::would_orphan_non_inline_flow_before_spacer(&units, j, h, avail_height)
                {
                    // TopAndBottom 개체만 쪽 하단에 남기고 뒤 spacer 를 다음 쪽으로 보내면
                    // 기준 렌더러와 달리 그림이 한 쪽 앞당겨진다. 개체+spacer 묶음이 함께
                    // 들어가지 못할 때는 개체 유닛부터 다음 조각에서 시작하게 한다.
                    break;
                }
                h += u.height;
                j += 1;
            }
            if j < units.len()
                && Self::rewind_rowbreak_orphan_heading_before_recursive_block(
                    table,
                    &units,
                    start,
                    avail_height,
                    &mut j,
                    &mut h,
                )
            {
                // 짧은 제목만 현재 쪽에 남고 뒤의 page-scale 재귀 block이 다음 쪽으로
                // 넘어가는 경우, 제목도 같은 source block의 첫 unit으로 넘긴다.
            }
            if j < units.len()
                && Self::rewind_rowbreak_fragment_tail_before_topandbottom_flow(
                    table,
                    &units,
                    start,
                    avail_height,
                    &mut j,
                    &mut h,
                )
            {
                // 뒤 TopAndBottom 개체 앞의 텍스트박스 꼬리 fragment 를 다음 조각에
                // 남겨 continuation 에서 선행 설명 박스가 사라지지 않게 한다.
            }
            if j < units.len()
                && units[j..].iter().any(|unit| unit.hard_break_before)
                && Self::rewind_rowbreak_tail_before_pending_hard_break(
                    table,
                    &units,
                    start,
                    avail_height,
                    &mut j,
                    &mut h,
                )
            {
                hit_hard_break = true;
            }
            if j < units.len() {
                fully_consumed = false;
            }
            if h > consumed_height {
                consumed_height = h;
            }
            end_cut.push(j);
        }
        RowCutResult {
            end_cut,
            hit_hard_break,
            fully_consumed,
            consumed_height,
        }
    }


    /// [Task #1025] 행블록 컷 — rowspan(rs>1) 셀로 묶인 연속 행 블록 `[b_start, b_end)`
    /// 의 셀을 `(row, col)` 안정 순서로 순회하며 CellUnit(줄/중첩 atom) 단위로 진행한다.
    /// `advance_row_cut` 의 블록 일반화: 블록을 걸친 rs>1 셀 + 블록 내 각 행의 셀을 모두
    /// 포함한다. rs>1 라벨 셀은 첫 조각(start_cut 비었을 때)에서 전량 소비되고, 연속
    /// 조각에선 시작 인덱스가 이미 끝이라 0 유닛 진행 → 렌더 공란(한컴 정답).
    /// 거대 `row_span==1` 셀은 줄 단위로 페이지 경계까지 채우고 잔여를 다음 조각으로 넘긴다.
    ///
    /// 셀 순서·인덱스는 `row_block_content_height` / 렌더러와 공유하는 단일 정의다.
    /// 단일 비-rowspan 행(`b_end==b_start+1`, 블록 내 rs>1 셀 없음)에서는
    /// `advance_row_cut` 과 동일 결과를 낸다(회귀 0).
    pub(crate) fn advance_row_block_cut(
        &self,
        table: &crate::model::table::Table,
        b_start: usize,
        b_end: usize,
        start_cut: &[usize],
        avail_height: f64,
        styles: &ResolvedStyleSet,
    ) -> RowCutResult {
        let mut cells = Self::row_block_cells(table, b_start, b_end);
        // 안정 순서: (row, col) 오름차순.
        cells.sort_by_key(|c| (c.row, c.col));

        let mut end_cut: RowCut = Vec::with_capacity(cells.len());
        let mut hit_hard_break = false;
        let mut fully_consumed = true;
        let mut consumed_height = 0.0f64;
        const HARD_BREAK_REMAINING_TOLERANCE_PX: f64 = 32.0;
        const ROWBREAK_VISIBLE_TAIL_OVERFLOW_TOLERANCE_PX: f64 = 120.0;
        let block_has_top_and_bottom_flow = cells
            .iter()
            .any(|cell| self.cell_has_top_and_bottom_non_inline_flow(cell));
        let relaxed_hard_break = matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) && (table.col_count <= 2 || table.row_count > 5)
            && !block_has_top_and_bottom_flow;
        let allow_midpage_reset_absorb =
            self.profile.get().hwpx_stored_layout() || block_has_top_and_bottom_flow;
        for (i, cell) in cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let start = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            let mut j = start;
            let mut h = 0.0f64;
            while j < units.len() {
                let u = &units[j];
                // 시작 유닛(j==start)은 항상 소비 — 진행 보장.
                if start > 0
                    && u.empty_spacer
                    && !u.hard_break_before
                    && units[start..=j].iter().all(|unit| unit.empty_spacer)
                {
                    j += 1;
                    continue;
                }
                if start > 0
                    && u.empty_spacer
                    && !u.hard_break_before
                    && units[j..]
                        .iter()
                        .all(|unit| unit.empty_spacer && !unit.hard_break_before)
                {
                    j = units.len();
                    break;
                }
                let follows_single_cell_nested_host = u
                    .para_idx
                    .checked_sub(1)
                    .and_then(|para_idx| cell.paragraphs.get(para_idx))
                    .is_some_and(Self::paragraph_hosts_single_cell_nested_table);
                let strict_saved_frame_break = u.stored_frame_break_before
                    && (u.mixed_nested_recursive || follows_single_cell_nested_host);
                if j > start
                    && u.hard_break_before
                    && (strict_saved_frame_break
                        || ((!relaxed_hard_break
                            || (!u.empty_spacer
                                && (h + u.height > avail_height
                                    || avail_height - h <= HARD_BREAK_REMAINING_TOLERANCE_PX)))
                            && !units[start..j].iter().all(|unit| unit.empty_spacer)))
                {
                    if !strict_saved_frame_break
                        && self.should_absorb_midpage_saved_vpos_reset(
                            table,
                            u,
                            h,
                            avail_height,
                            allow_midpage_reset_absorb,
                        )
                    {
                        h += u.height;
                        j += 1;
                        continue;
                    }
                    Self::rewind_rowbreak_orphan_before_hard_break(
                        table,
                        &units,
                        start,
                        avail_height,
                        false,
                        &mut j,
                        &mut h,
                    );
                    hit_hard_break = true;
                    break;
                }
                if j > start && h + u.height > avail_height {
                    let visible_tail_before_spacer = relaxed_hard_break
                        && !u.empty_spacer
                        && u.vis_start < u.vis_end
                        && h + u.height
                            <= avail_height + ROWBREAK_VISIBLE_TAIL_OVERFLOW_TOLERANCE_PX
                        && Self::grace_visible_tail_before_spacer(&units, j);
                    if visible_tail_before_spacer {
                        h += u.height;
                        j += 1;
                        continue;
                    }
                    if Self::rewind_rowbreak_mixed_nested_table_tail_for_fresh_page(
                        table,
                        &units,
                        start,
                        self.current_body_area.get().3.max(avail_height),
                        &mut j,
                        &mut h,
                    ) {
                        break;
                    }
                    // [#1921] sliver 흡수는 with_row_offsets 경로에만 적용한다. 이 walk 는
                    // relaxed_hard_break(hard-break 조건부 무시) 의미론이라 다음 break 로의
                    // 흡수가 비정상 경계를 강제한다(86712 공식PDF 65→66 회귀 실증).
                    break;
                }
                if j > start
                    && Self::would_orphan_non_inline_flow_before_spacer(&units, j, h, avail_height)
                {
                    // `advance_row_cut` 과 같은 CellUnit 구조 판정이다. 행블록 컷에서도
                    // TopAndBottom 개체 유닛이 뒤 spacer 와 분리되어 고립되지 않게 한다.
                    break;
                }
                h += u.height;
                j += 1;
            }
            if j < units.len()
                && Self::rewind_rowbreak_fragment_tail_before_topandbottom_flow(
                    table,
                    &units,
                    start,
                    avail_height,
                    &mut j,
                    &mut h,
                )
            {
                // `advance_row_cut` 과 같은 후처리다.
            }
            if j < units.len()
                && units[j..].iter().any(|unit| unit.hard_break_before)
                && Self::rewind_rowbreak_tail_before_pending_hard_break(
                    table,
                    &units,
                    start,
                    avail_height,
                    &mut j,
                    &mut h,
                )
            {
                hit_hard_break = true;
            }
            if j < units.len() {
                fully_consumed = false;
            }
            if h > consumed_height {
                consumed_height = h;
            }
            // [#2097 진단] 셀별 walk 결과 — 동작 불변.
            if std::env::var("RHWP_DIAG_BLKCUT").is_ok() {
                let stop = if j >= units.len() {
                    "end"
                } else if units[j].hard_break_before {
                    "hard"
                } else {
                    "budget"
                };
                eprintln!(
                    "DIAG_BLKCUT cell[{}] r={} c={} units={} start={} j={} h={:.1} stop={} next_h={:.1}",
                    i,
                    cell.row,
                    cell.col,
                    units.len(),
                    start,
                    j,
                    h,
                    stop,
                    units.get(j).map(|u| u.height).unwrap_or(0.0)
                );
            }
            end_cut.push(j);
        }
        RowCutResult {
            end_cut,
            hit_hard_break,
            fully_consumed,
            consumed_height,
        }
    }


    /// RowBreak rowspan 블록에서 셀의 행 시작 y를 반영해 컷을 전진시킨다.
    ///
    /// 일반 `advance_row_block_cut`은 블록 안의 모든 셀에 같은 예산을 주기 때문에,
    /// 위쪽 큰 셀이 페이지 경계에서 잘릴 때 아래 행의 짧은 셀까지 먼저 소비할 수 있다.
    /// 이 함수는 행별 top offset을 빼고 남은 예산으로 셀을 전진시켜 같은 블록 안의
    /// 아래 행 내용이 한컴처럼 다음 조각에 남도록 한다.
    pub(crate) fn advance_row_block_cut_with_row_offsets(
        &self,
        table: &crate::model::table::Table,
        b_start: usize,
        b_end: usize,
        start_cut: &[usize],
        avail_height: f64,
        row_offsets: &[f64],
        styles: &ResolvedStyleSet,
    ) -> RowCutResult {
        let mut cells = Self::row_block_cells(table, b_start, b_end);
        cells.sort_by_key(|c| (c.row, c.col));

        let mut end_cut: RowCut = Vec::with_capacity(cells.len());
        let mut hit_hard_break = false;
        let mut fully_consumed = true;
        let mut consumed_height = 0.0f64;
        // [#2291] plain 블록 walk(advance_row_block_cut)와 동일한 relaxed_hard_break
        // 의미론 — 한글 2022 는 재개방 시 저장 vpos-reset(원저작 쪽나눔 흔적)을
        // 무시하고 fresh 재배치로 쪽을 만충한다(연결맵 p26 = 81줄 실측, #2291).
        // 종전 이 walk 는 hard-break 에서 무조건 정지해 예산 잔여(≤52px)를 버리고
        // 조각 경계가 한글과 어긋났다.
        const HARD_BREAK_REMAINING_TOLERANCE_PX: f64 = 32.0;
        let block_has_top_and_bottom_flow = cells
            .iter()
            .any(|cell| self.cell_has_top_and_bottom_non_inline_flow(cell));
        let relaxed_hard_break = matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) && (table.col_count <= 2 || table.row_count > 5)
            && !block_has_top_and_bottom_flow;
        for (i, cell) in cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let start = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            let cell_row = cell.row as usize;
            let row_offset = cell_row
                .checked_sub(b_start)
                .and_then(|idx| row_offsets.get(idx))
                .copied()
                .unwrap_or(0.0);
            let cell_budget = (avail_height - row_offset).max(0.0);
            let allow_force_progress = row_offset <= 0.5;
            let mut j = start;
            let mut h = 0.0f64;
            // [#2287/PR #2290 P1] 연속 조각(start>0)이 시작 직후(start+1) 저장
            // hard-break 를 만나면, start 유닛은 직전 조각의 orphan-rewind 가
            // 이월시킨 고아다 — 여기서 hard 를 쪽 경계로 존중하면 고아 혼자
            // 한 쪽(교육부 47×9 p26: 유닛 1개 17.3px sliver)이 되어 rewind 의
            // 의도(고아 방지)와 정반대가 된다. 극소 소비(h ≤ 한 줄급) 한정으로
            // 그 hard 는 이미 소비된 경계로 보고 통과한다.
            const REWIND_ORPHAN_CONT_PX: f64 = 48.0;
            // [#2291] relaxed 통과는 거대 셀(원저작 쪽나눔 흔적이 촘촘한 다문단
            // 셀)에 한정한다 — 소형 셀의 저장 hard-break 는 실제 행/조각 경계라
            // 통과시키면 조각이 비대해져 밴드 컷이 어긋난다 (21217935 8→9쪽
            // 회귀 실측). 연결맵 결함 셀은 유닛 37~123개.
            const GIANT_CELL_RELAXED_MIN_UNITS: usize = 24;
            let cell_relaxed = relaxed_hard_break && units.len() >= GIANT_CELL_RELAXED_MIN_UNITS;
            while j < units.len() {
                let u = &units[j];
                if j > start
                    && u.hard_break_before
                    && (!cell_relaxed
                        || (!u.empty_spacer
                            && (h + u.height > cell_budget
                                || cell_budget - h <= HARD_BREAK_REMAINING_TOLERANCE_PX)))
                {
                    if start > 0 && j == start + 1 && h <= REWIND_ORPHAN_CONT_PX {
                        h += u.height;
                        j += 1;
                        continue;
                    }
                    Self::rewind_rowbreak_orphan_before_hard_break(
                        table,
                        &units,
                        start,
                        cell_budget,
                        true,
                        &mut j,
                        &mut h,
                    );
                    hit_hard_break = true;
                    break;
                }
                if j > start && h + u.height > cell_budget {
                    if Self::rewind_rowbreak_mixed_nested_table_tail_for_fresh_page(
                        table,
                        &units,
                        start,
                        self.current_body_area.get().3.max(cell_budget),
                        &mut j,
                        &mut h,
                    ) {
                        break;
                    }
                    // [#1921] sliver 흡수 — advance_row_block_cut 의 예산 정지와 동일.
                    // 직후 tolerance 안의 저장 hard-break(한글 실제 페이지 경계)까지
                    // 흡수해, 다음 fragment 가 극소 잔여 sliver 페이지가 되는 것을 막는다.
                    if let Some((absorbed_h, absorbed_j)) =
                        Self::absorb_tail_before_stored_hard_break(&units, j, h, cell_budget)
                    {
                        h = absorbed_h;
                        j = absorbed_j;
                        hit_hard_break = true;
                        break;
                    }
                    break;
                }
                if j == start && !allow_force_progress && h + u.height > cell_budget {
                    break;
                }
                h += u.height;
                j += 1;
            }
            if j < units.len() {
                fully_consumed = false;
            }
            if h > 0.0 {
                consumed_height = consumed_height.max(row_offset + h);
            }
            // [#2097 진단] 오프셋 walk 셀별 결과 — 동작 불변.
            if std::env::var("RHWP_DIAG_BLKCUT").is_ok() {
                let stop = if j >= units.len() {
                    "end"
                } else if units[j].hard_break_before {
                    "hard"
                } else {
                    "budget"
                };
                eprintln!(
                    "DIAG_BLKCUT(ofs) cell[{}] r={} c={} units={} start={} j={} h={:.1} row_off={:.1} cell_budget={:.1} stop={} next_h={:.1}",
                    i,
                    cell.row,
                    cell.col,
                    units.len(),
                    start,
                    j,
                    h,
                    row_offset,
                    cell_budget,
                    stop,
                    units.get(j).map(|u| u.height).unwrap_or(0.0)
                );
            }
            end_cut.push(j);
        }
        RowCutResult {
            end_cut,
            hit_hard_break,
            fully_consumed,
            consumed_height,
        }
    }


    pub(crate) fn rewind_rowbreak_orphan_before_hard_break(
        table: &crate::model::table::Table,
        units: &[CellUnit],
        start: usize,
        avail_height: f64,
        force_rewind: bool,
        j: &mut usize,
        h: &mut f64,
    ) {
        if !matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) || *j <= start + 1
        {
            return;
        }

        let hard_break_unit = &units[*j];
        let prev = &units[*j - 1];
        if prev.para_idx == hard_break_unit.para_idx {
            *h -= prev.height;
            *j -= 1;
            return;
        }

        if table.common.treat_as_char {
            return;
        }

        if let Some(rewind_to) = units[start..*j]
            .iter()
            .rposition(|unit| unit.vpos_gap_before)
            .map(|idx| start + idx)
        {
            if rewind_to > start {
                let rewind_h: f64 = units[rewind_to..*j].iter().map(|unit| unit.height).sum();
                let rewound_h = *h - rewind_h;
                const MAX_REWIND_BLANK_PX: f64 = 80.0;
                if !force_rewind && avail_height - rewound_h > MAX_REWIND_BLANK_PX {
                    return;
                }
                *h -= rewind_h;
                *j = rewind_to;
            }
        }
    }


    /// 재귀 1×1 표를 부모 `RowCut` 원장으로 투영할 때, source에서 한 묶음으로 표시한
    /// 빈 separator와 한 줄 제목만 현재 쪽에 들어가고 바로 다음 block이 넘치면 한컴은
    /// prelude 전체를 그 block과 함께 다음 쪽에 둔다. 제목 뒤 재귀 block의 작은
    /// 선행 fragment가 이미 들어간 경우에도, 그 연속 prefix까지 같은 묶음으로 되감는다.
    pub(crate) fn rewind_rowbreak_orphan_heading_before_recursive_block(
        table: &crate::model::table::Table,
        units: &[CellUnit],
        start: usize,
        avail_height: f64,
        j: &mut usize,
        h: &mut f64,
    ) -> bool {
        if !matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) || table.common.treat_as_char
            || *j >= units.len()
            || *j <= start + 1
            || !avail_height.is_finite()
            || avail_height <= 0.0
        {
            return false;
        }

        let next = &units[*j];
        if !next.mixed_nested_fragment
            || !next.mixed_nested_recursive
            || next.mixed_nested_trailing
            || next.hard_break_before
            || *h + next.height <= avail_height + 0.5
        {
            return false;
        }

        // 직전 block은 모두 들어갔지만 다음 prelude의 빈 separator만 현재 조각에
        // 들어가고 제목이 예산을 넘는 형상도 같은 orphan이다. separator를 그대로
        // 소비하면 다음 조각은 제목부터 시작해 아래의 separator+heading 탐색을
        // 다시 수행할 수 없고, 제목과 작은 recursive prefix만 가진 sliver 쪽이
        // 생긴다. role은 source에서 정확히 `빈 문단 + 한 줄 제목 + 1×1 표`일 때만
        // 부여되므로 pending 제목 앞의 separator 하나만 되감는다.
        if next.recursive_block_prelude_role
            == RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable
        {
            let separator_idx = *j - 1;
            let separator = &units[separator_idx];
            if !separator.hard_break_before
                && separator.mixed_nested_fragment
                && separator.mixed_nested_recursive
                && matches!(
                    separator.recursive_block_prelude_role,
                    RecursiveBlockPreludeRole::EmptySeparator
                        | RecursiveBlockPreludeRole::ExplicitPageBreakSeparator
                )
            {
                *h = (*h - separator.height).max(0.0);
                *j = separator_idx;
                return true;
            }
        }

        // `j-1`이 바로 제목인 기존 형상은 loop를 한 번도 돌지 않는다. 제목 뒤
        // recursive block의 작은 조각이 먼저 fit한 형상에서는 role=None인 연속
        // nontrailing prefix만 거슬러 올라가 가장 가까운 prelude 제목을 찾는다.
        let mut block_prefix_start = *j;
        while block_prefix_start > start {
            let unit = &units[block_prefix_start - 1];
            if unit.mixed_nested_fragment
                && unit.mixed_nested_recursive
                && !unit.mixed_nested_trailing
                && !unit.hard_break_before
                && !unit.stored_frame_break_before
                && !unit.vpos_gap_before
                && unit.recursive_block_prelude_role == RecursiveBlockPreludeRole::None
            {
                block_prefix_start -= 1;
            } else {
                break;
            }
        }
        if block_prefix_start <= start + 1 {
            return false;
        }

        let heading_idx = block_prefix_start - 1;
        let separator_idx = heading_idx - 1;
        // 현재 fragment가 separator부터 시작했다면 되감기는 `j == start`가 되어
        // pagination progress를 잃는다. 새 viewport는 separator를 예약값으로
        // 소비한 뒤 반드시 제목/자식 block 쪽으로 전진시킨다.
        if separator_idx <= start {
            return false;
        }
        let heading = &units[heading_idx];
        let separator = &units[separator_idx];
        let already_fit_recursive_prefix = block_prefix_start < *j;
        if !heading.mixed_nested_fragment
            || !heading.mixed_nested_recursive
            || heading.recursive_block_prelude_role
                != RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable
            || separator.hard_break_before
            || !separator.mixed_nested_fragment
            || !separator.mixed_nested_recursive
            || !matches!(
                separator.recursive_block_prelude_role,
                RecursiveBlockPreludeRole::EmptySeparator
                    | RecursiveBlockPreludeRole::ExplicitPageBreakSeparator
            )
            // 일반 prelude는 제목 바로 뒤 block이 아직 시작되지 않았을 때만 기존
            // direct-next orphan 보정을 적용한다. 이미 recursive prefix가 들어간
            // source block까지 되감는 것은 명시적 Page/Section separator에 한정해,
            // 저장 프레임 경계에서 끝나야 할 정상 continuation을 앞당기지 않는다.
            || (already_fit_recursive_prefix
                && separator.recursive_block_prelude_role
                    != RecursiveBlockPreludeRole::ExplicitPageBreakSeparator)
        {
            return false;
        }

        let rewind_height: f64 = units[separator_idx..*j]
            .iter()
            .map(|unit| unit.height)
            .sum();
        *h = (*h - rewind_height).max(0.0);
        *j = separator_idx;
        true
    }


    pub(crate) fn rewind_rowbreak_tail_before_pending_hard_break(
        table: &crate::model::table::Table,
        units: &[CellUnit],
        start: usize,
        avail_height: f64,
        j: &mut usize,
        h: &mut f64,
    ) -> bool {
        if !matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) || table.common.treat_as_char
            || *j <= start + 1
            || units[start..*j].iter().all(|unit| unit.empty_spacer)
        {
            return false;
        }

        let Some(rewind_to) = units[start..*j]
            .iter()
            .rposition(|unit| unit.vpos_gap_before)
            .map(|idx| start + idx)
        else {
            return false;
        };
        if units.get(*j).is_some_and(|unit| unit.hard_break_before) || rewind_to <= start {
            return false;
        }

        let rewind_h: f64 = units[rewind_to..*j].iter().map(|unit| unit.height).sum();
        let rewound_h = *h - rewind_h;
        const MAX_REWIND_BLANK_PX: f64 = 80.0;
        if avail_height - rewound_h > MAX_REWIND_BLANK_PX {
            return false;
        }
        *h -= rewind_h;
        *j = rewind_to;
        true
    }


    pub(crate) fn row_has_prior_rowspan_cover(table: &crate::model::table::Table, row: usize) -> bool {
        table.cells.iter().any(|cell| {
            let start = cell.row as usize;
            let end = start + (cell.row_span as usize).max(1);
            cell.row_span > 1 && start < row && row < end
        })
    }


    /// RowBreak 표의 rowspan 블록 중 셀 내부 HWP page reset 이 처음 나타나는 셀의
    /// 시작 행을 찾는다. 단순 rowspan 라벨 표는 기존 행 경계 분할을 유지한다.
    pub(crate) fn row_block_first_internal_hard_break_row(
        &self,
        table: &crate::model::table::Table,
        b_start: usize,
        b_end: usize,
        styles: &ResolvedStyleSet,
    ) -> Option<usize> {
        Self::row_block_cells(table, b_start, b_end)
            .iter()
            .filter_map(|cell| {
                let has_hard_break = self
                    .cell_units(cell, table, styles)
                    .iter()
                    .enumerate()
                    .any(|(i, unit)| i > 0 && unit.hard_break_before);
                has_hard_break.then_some(cell.row as usize)
            })
            .min()
    }


    /// RowBreak 표의 rowspan 블록 중 셀 내부 HWP page reset 이 있는 블록만
    /// 블록 컷 대상으로 삼기 위한 가드.
    pub(crate) fn row_block_has_internal_hard_break(
        &self,
        table: &crate::model::table::Table,
        b_start: usize,
        b_end: usize,
        styles: &ResolvedStyleSet,
    ) -> bool {
        self.row_block_first_internal_hard_break_row(table, b_start, b_end, styles)
            .is_some()
    }


    /// [Task #1025] 행블록 `[b_start, b_end)` 와 교차하는 셀(rs>1 포함)을 모은다.
    /// `advance_row_block_cut` / `row_block_content_height` / 렌더러 공유 — 순서는
    /// 호출부에서 `(row, col)` 로 정렬한다.
    pub(crate) fn row_block_cells<'a>(
        table: &'a crate::model::table::Table,
        b_start: usize,
        b_end: usize,
    ) -> Vec<&'a crate::model::table::Cell> {
        table
            .cells
            .iter()
            .filter(|c| {
                let cr = c.row as usize;
                let ce = cr + (c.row_span as usize).max(1);
                cr < b_end && ce > b_start
            })
            .collect()
    }


    /// [Task #1025] 행블록 컷 범위 `[start_cut, end_cut)` 의 블록 표시 높이(패딩 포함).
    /// 블록 셀별 `content_in_cut + pad`, 블록 max. `advance_row_block_cut` 과 동일한
    /// `(row, col)` 셀 순서를 사용한다.
    pub(crate) fn row_block_content_height(
        &self,
        table: &crate::model::table::Table,
        b_start: usize,
        b_end: usize,
        start_cut: &[usize],
        end_cut: &[usize],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let mut cells = Self::row_block_cells(table, b_start, b_end);
        cells.sort_by_key(|c| (c.row, c.col));
        let mut max_h = 0.0f64;
        for (i, cell) in cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let su = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            let eu = end_cut
                .get(i)
                .copied()
                .unwrap_or(units.len())
                .clamp(su, units.len());
            let content: f64 = units[su..eu].iter().map(|u| u.height).sum();
            let (_, _, pad_top, pad_bottom) = self.resolve_cell_padding(cell, table);
            let h = content + pad_top + pad_bottom;
            // [#2287 진단] start_cut 적용 잔여 평가 분해 — 동작 불변.
            if std::env::var("RHWP_DIAG_BLKH").is_ok() && !start_cut.is_empty() {
                eprintln!(
                    "DIAG_BLKH cell[{}] r={} c={} units={} su={} eu={} content={:.1} h={:.1}",
                    i,
                    cell.row,
                    cell.col,
                    units.len(),
                    su,
                    eu,
                    content,
                    h
                );
            }
            if h > max_h {
                max_h = h;
            }
        }
        max_h
    }


    /// [#2287] start_cut 이후 블록 잔여 콘텐츠 높이 — `advance_row_block_cut` 의
    /// spacer 소비 의미론(컷 재개 지점의 선두/후미 empty-spacer run 은 무높이
    /// 소비)을 미러한 잔여 평가. `row_block_content_height` 는 spacer 꼬리를
    /// 전량 합산해 잔여를 과대평가한다 (59043 규제영향분석서 41→44쪽 회귀 실측).
    /// [#2287/PR #2290 P1] 셀의 컷 범위(su..eu) 유닛 가시 높이 + 상하 패딩.
    /// 블록-합 보정(table_partial)에서 rowspan 셀 bbox 를 컷과 정합시키는 데 쓴다.
    pub(crate) fn cell_cut_visible_height(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
    ) -> f64 {
        let units = self.cell_units(cell, table, styles);
        let su = start_unit.min(units.len());
        let eu = end_unit.clamp(su, units.len());
        let content: f64 = units[su..eu].iter().map(|u| u.height).sum();
        if content <= 0.0 {
            return 0.0;
        }
        let (_, _, pad_top, pad_bottom) = self.resolve_cell_padding(cell, table);
        content + pad_top + pad_bottom
    }


    pub(crate) fn row_block_cut_remaining_height(
        &self,
        table: &crate::model::table::Table,
        b_start: usize,
        b_end: usize,
        start_cut: &[usize],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let mut cells = Self::row_block_cells(table, b_start, b_end);
        cells.sort_by_key(|c| (c.row, c.col));
        let mut max_h = 0.0f64;
        for (i, cell) in cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let su = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            if su >= units.len() {
                continue;
            }
            let (mut lo, mut hi) = (su, units.len());
            if su > 0 {
                while lo < hi && units[lo].empty_spacer && !units[lo].hard_break_before {
                    lo += 1;
                }
                while hi > lo && units[hi - 1].empty_spacer && !units[hi - 1].hard_break_before {
                    hi -= 1;
                }
            }
            let content: f64 = units[lo..hi].iter().map(|u| u.height).sum();
            if content <= 0.0 {
                continue;
            }
            let (_, _, pad_top, pad_bottom) = self.resolve_cell_padding(cell, table);
            let h = content + pad_top + pad_bottom;
            if h > max_h {
                max_h = h;
            }
        }
        max_h
    }


    /// 블록 컷 벡터를 특정 행의 per-row 컷으로 변환해 해당 행 표시 높이를 계산한다.
    pub(crate) fn row_block_cut_row_content_height(
        &self,
        table: &crate::model::table::Table,
        b_start: usize,
        b_end: usize,
        row: usize,
        start_cut: &[usize],
        end_cut: &[usize],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let mut block_cells = Self::row_block_cells(table, b_start, b_end);
        block_cells.sort_by_key(|c| (c.row, c.col));

        let mut row_cells: Vec<&crate::model::table::Cell> = table
            .cells
            .iter()
            .filter(|c| c.row as usize == row && c.row_span == 1)
            .collect();
        row_cells.sort_by_key(|c| c.col);

        if row_cells.is_empty() {
            return 0.0;
        }

        let mut per_start = Vec::with_capacity(row_cells.len());
        let mut per_end = Vec::with_capacity(row_cells.len());
        let mut has_visible_range = false;
        let mut has_row_cut = false;
        for cell in row_cells {
            let block_idx = block_cells
                .iter()
                .position(|c| c.row == cell.row && c.col == cell.col);
            let units = self.cell_units(cell, table, styles);
            let su = block_idx
                .and_then(|idx| start_cut.get(idx).copied())
                .unwrap_or(0)
                .min(units.len());
            let eu = block_idx
                .and_then(|idx| end_cut.get(idx).copied())
                .unwrap_or(units.len())
                .clamp(su, units.len());
            if eu > su {
                has_visible_range = true;
            }
            if su > 0 || eu < units.len() {
                has_row_cut = true;
            }
            per_start.push(su);
            per_end.push(eu);
        }

        if !has_visible_range {
            return 0.0;
        }

        if has_row_cut {
            self.row_cut_content_height(table, row, &per_start, &per_end, styles)
        } else {
            self.row_cut_content_height(table, row, &[], &[], styles)
        }
    }


    /// [Task #1748] 셀 유닛 누적높이가 `budget`(패딩 제외 콘텐츠 예산) 안에
    /// 들어가는 선두 유닛 수를 반환한다. 컷 행에 걸친(straddling) rowspan 셀의
    /// 높이 기반 가시 유닛 컷 산출용 — 컷 페이지의 eu 와 연속 페이지의 su 가
    /// 같은 예산 식으로 계산되어 경계 줄 인덱스가 산술적으로 일치한다.
    pub(crate) fn cell_units_fitting_height(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        budget: f64,
    ) -> usize {
        const EPS: f64 = 0.1;
        let units = self.cell_units(cell, table, styles);
        let mut n = 0usize;
        let mut h = 0.0f64;
        while n < units.len() && h + units[n].height <= budget + EPS {
            h += units[n].height;
            n += 1;
        }
        n
    }


    /// HWP5 저장 pagination 계약의 `RowBreak` 표에서 정확히 두 행을 덮는 병합 셀의
    /// 두 저장 문단이 각각 한 줄 유닛이면, 행 경계의 문단 owner를 그대로 유지할 수
    /// 있는지 판정한다.
    ///
    /// 일반 rowspan 분할은 물리 높이로 잘라야 한다. 다만 이 좁은 형상은 저장된 두
    /// 문단이 두 물리 행에 정확히 대응한다. 첫 문단의 trailing line/문단 간격까지
    /// 첫 행 예산에 포함하면, ink는 들어가는데 unit만 다음 fragment로 밀려 두 문단이
    /// 재방출된다(76076 p18→p19 `11.영향평가` / `여부`).
    pub(crate) fn native_two_row_rowspan_paragraph_owner_boundary(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
    ) -> bool {
        if !self.profile.get().hwp5_stored_pagination_layout()
            || table.common.treat_as_char
            || !matches!(table.page_break, TablePageBreak::RowBreak)
            || cell.row_span != 2
            || cell.paragraphs.len() != 2
            || cell
                .paragraphs
                .iter()
                .any(|paragraph| paragraph.text.trim().is_empty() || !paragraph.controls.is_empty())
        {
            return false;
        }

        let units = self.cell_units(cell, table, styles);
        units.len() == 2
            && units.iter().enumerate().all(|(para_idx, unit)| {
                unit.para_idx == para_idx
                    && unit.vis_start == 0
                    && unit.vis_end == 1
                    && !unit.empty_spacer
                    && unit.nested_row.is_none()
                    && unit.nested_table_fragment.is_none()
                    && !unit.mixed_nested_fragment
                    && unit.non_inline_control_range.is_none()
            })
    }


    /// [Task #993] 한 셀의 유닛 범위 `[start_unit, end_unit)`를 문단별 줄 범위로
    /// 변환한다. `layout_partial_table`이 `RowCut`으로 가시 범위를 렌더할 때
    /// 사용 — 결과는 종전 `compute_cell_line_ranges`와 같은
    /// `Vec<(start_line, end_line)>` 형식(문단마다 1개, 미가시 문단은 `(0,0)`).
    pub(crate) fn cell_line_ranges_from_cut(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
    ) -> Vec<(usize, usize)> {
        let units = self.cell_units(cell, table, styles);
        let mut ranges = vec![(0usize, 0usize); cell.paragraphs.len()];
        let mut seen = vec![false; cell.paragraphs.len()];
        let lo = start_unit.min(units.len());
        let hi = end_unit.min(units.len());
        for u in units.iter().take(hi).skip(lo) {
            if u.para_idx >= ranges.len() {
                continue;
            }
            if !seen[u.para_idx] {
                ranges[u.para_idx] = (u.vis_start, u.vis_end);
                seen[u.para_idx] = true;
            } else {
                let r = &mut ranges[u.para_idx];
                r.0 = r.0.min(u.vis_start);
                r.1 = r.1.max(u.vis_end);
            }
        }
        ranges
    }


    pub(crate) fn cell_cut_contains_non_inline_control_units(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
        para_idx: usize,
    ) -> bool {
        let units = self.cell_units(cell, table, styles);
        let lo = start_unit.min(units.len());
        let hi = end_unit.min(units.len()).max(lo);
        let has_non_inline_control = cell.paragraphs.get(para_idx).is_some_and(|para| {
            para.controls.iter().any(|control| match control {
                Control::Picture(picture) => !picture.common.treat_as_char,
                Control::Shape(shape) => !shape.common().treat_as_char,
                _ => false,
            })
        });
        if !has_non_inline_control {
            return false;
        }

        // 현재 컷 안에 non-inline 개체가 차지하는 명시 유닛이 실제로 포함될 때만
        // 셀 안 non-inline 개체를 그린다. 같은 문단의 텍스트 줄만 continuation 에
        // 남아 있는 경우까지 허용하면 이전 쪽 그림이 모든 페이지에 반복된다.
        units.iter().take(hi).skip(lo).any(|unit| {
            unit.para_idx == para_idx
                && unit.vis_start == unit.vis_end
                && !unit.empty_spacer
                && unit.nested_row.is_none()
                && !unit.mixed_nested_fragment
        })
    }


    /// `cell_cut_contains_non_inline_control_units`의 control-identity 버전.
    ///
    /// Square/Tight/Through flow fragment는 control range의 **첫** unit을 포함한 cut만
    /// picture/shape를 emit한다. 같은 control의 뒷 unit은 다음 physical fragment에서
    /// 다시 image를 paint하지 않는다. legacy/TopAndBottom unit처럼 range가 없는 경우에는
    /// 기존 paragraph-level 판정을 유지해 저장 형식별 기존 contract를 바꾸지 않는다.
    pub(crate) fn cell_cut_starts_non_inline_control(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
        para_idx: usize,
        control_idx: usize,
    ) -> bool {
        let Some(para) = cell.paragraphs.get(para_idx) else {
            return false;
        };
        let has_non_inline_control =
            para.controls
                .get(control_idx)
                .is_some_and(|control| match control {
                    Control::Picture(picture) => !picture.common.treat_as_char,
                    Control::Shape(shape) => !shape.common().treat_as_char,
                    _ => false,
                });
        if !has_non_inline_control {
            return false;
        }

        let units = self.cell_units(cell, table, styles);
        let lo = start_unit.min(units.len());
        let hi = end_unit.min(units.len()).max(lo);
        let control_start = units.iter().position(|unit| {
            unit.para_idx == para_idx
                && unit
                    .non_inline_control_range
                    .is_some_and(|(first, last)| first <= control_idx && control_idx <= last)
        });
        if let Some(start) = control_start {
            return lo <= start && start < hi;
        }

        let mut saw_legacy_candidate = false;
        for unit in units.iter().take(hi).skip(lo) {
            let candidate = unit.para_idx == para_idx
                && unit.vis_start == unit.vis_end
                && !unit.empty_spacer
                && unit.nested_row.is_none()
                && !unit.mixed_nested_fragment;
            if !candidate {
                continue;
            }
            if unit.non_inline_control_range.is_none() {
                saw_legacy_candidate = true;
            }
        }
        saw_legacy_candidate
    }


    pub(crate) fn mixed_nested_split_from_cut(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
        para_idx: usize,
    ) -> Option<NestedTableSplit> {
        let units = self.cell_units(cell, table, styles);
        let lo = start_unit.min(units.len());
        let hi = end_unit.min(units.len()).max(lo);
        let mut total = 0.0;
        let mut offset = 0.0;
        let mut visible_units: Vec<(f64, bool, bool, f64)> = Vec::new();
        let mut recursive_total = 0usize;
        let mut recursive_start = 0usize;
        let mut has_non_recursive_fragment = false;
        for (idx, unit) in units.iter().enumerate() {
            if unit.para_idx != para_idx || !unit.mixed_nested_fragment {
                continue;
            }
            if unit.mixed_nested_recursive {
                recursive_total += 1;
                if idx < lo {
                    recursive_start += 1;
                }
            } else {
                has_non_recursive_fragment = true;
            }
            total += unit.height;
            if idx < lo {
                offset += unit.height;
            }
            if idx >= lo && idx < hi {
                visible_units.push((
                    unit.height,
                    unit.mixed_nested_trailing,
                    unit.mixed_nested_recursive,
                    unit.mixed_nested_content_height,
                ));
            }
        }
        // `terminal` is scoped to this mixed nested stream, not the host
        // cell's entire unit list.  A completed inner table can be followed
        // by another paragraph/table in the same outer cell; treating it as
        // non-terminal drops its final reservation and shortens the current
        // frame (42065 p13→p15).
        let terminal = !units
            .iter()
            .skip(hi)
            .any(|unit| unit.para_idx == para_idx && unit.mixed_nested_fragment);
        let successor_trailing_reservation = trailing_reservation_after_final_source_owner(
            para_idx,
            units.get(hi).map(MixedNestedOwnerMarker::from),
            units
                .iter()
                .skip(hi.saturating_add(1))
                .map(MixedNestedOwnerMarker::from),
        );
        // A non-terminal fragment must not paint the synthetic trailing unit:
        // its successor owns that source window.  The terminal fragment is
        // different — that trailing unit can contain the final ordinary
        // paragraphs after the nested table, so discarding it clips the
        // document's last content (42065 p17's section 4).
        if offset > 0.5 && !terminal {
            while visible_units
                .last()
                .is_some_and(|(_, trailing, _, _)| *trailing)
            {
                visible_units.pop();
            }
        }
        let flow_visible: f64 = visible_units.iter().map(|(h, _, _, _)| *h).sum();
        let recursive_visible = visible_units
            .iter()
            .filter(|(_, _, recursive, _)| *recursive)
            .count();
        let recursive_cut = if recursive_total > 0 && !has_non_recursive_fragment {
            // The terminal page can begin with a synthetic trailing unit that
            // only reserved physical flow on the preceding viewport.  It has
            // no visible child content and must not become the recursive
            // child's first source owner (42065 p17: 32px blank before the
            // heading).  Keep the unit in outer flow accounting and advance
            // only the authoritative child cursor past it.
            let leading_trailing = if terminal && offset > 0.5 {
                visible_units
                    .iter()
                    .take_while(|(_, trailing, recursive, content_height)| {
                        *trailing && *recursive && *content_height <= 0.5
                    })
                    .count()
            } else {
                0
            };
            let recursive_start = recursive_start + leading_trailing;
            let recursive_end =
                recursive_start + recursive_visible.saturating_sub(leading_trailing);
            Some(NestedTableCut {
                start_row: 0,
                end_row: 1,
                start_cut: if recursive_start == 0 {
                    Vec::new()
                } else {
                    vec![recursive_start]
                },
                end_cut: if recursive_end >= recursive_total {
                    Vec::new()
                } else {
                    vec![recursive_end]
                },
                is_block_split: false,
            })
        } else {
            None
        };
        // Continuation pages still need the whole visible slice clipped in, even
        // when the same host cell has following paragraphs in the current cut.
        // Shrinking the clip to the first non-trailing unit keeps the flow
        // advance but clips the nested table content above the cell on page 8.
        let visible: f64 = flow_visible;
        let first_visible_content_height = visible_units
            .iter()
            .find_map(|(height, trailing, _, _)| (!*trailing).then_some(*height))
            .unwrap_or(0.0);
        let first_visible_paint_height = visible_units
            .iter()
            .find_map(|(_, trailing, _, content_height)| (!*trailing).then_some(*content_height))
            .unwrap_or(0.0);
        let last_visible_content_height = visible_units
            .iter()
            .rev()
            .find_map(|(height, trailing, _, _)| (!*trailing).then_some(*height))
            .unwrap_or(0.0);
        let first_visible_starts_after_table = units
            .iter()
            .skip(lo)
            .take(hi.saturating_sub(lo))
            .find(|unit| {
                unit.para_idx == para_idx
                    && unit.mixed_nested_fragment
                    && !unit.mixed_nested_trailing
            })
            .is_some_and(|unit| unit.mixed_nested_starts_after_table);
        // 1×1 host 안의 1×1 표 continuation은 이전 조각의 첫 unit을 물리
        // reservation으로 이미 전진시킨다. 다음 조각의 content origin까지 원래
        // `offset`만 쓰면 그 unit이 다시 페이지 상단에 그려져 이후 제목/표가 한 줄씩
        // 아래로 drift한다. 이 중복은 종료 조각에도 남는다. terminal tail의 물리
        // clip 높이는 아래 `terminal_single_cell_tail` 분기가 별도로 보존하므로,
        // 여기서는 종료 여부와 관계없이 content origin을 같은 기준으로 전진시킨다
        // (42065 p12–p17).
        let single_cell_nested_continuation = table.row_count == 1
                && table.col_count == 1
                && cell.paragraphs.get(para_idx).is_some_and(|paragraph| {
                    paragraph.controls.iter().any(|control| {
                        matches!(control, Control::Table(nested) if nested.row_count == 1 && nested.col_count == 1)
                    })
                });
        // PR #4122가 만든 재귀 child cursor가 있으면 그 cursor가 소유권의
        // 권위다. scalar offset 보정은 재귀 투영이 없는 기존 fallback에만 쓴다.
        let compensate_first_visible = recursive_cut.is_none()
            && offset > 0.5
            && single_cell_nested_continuation
            && !terminal
            && !first_visible_starts_after_table;
        let offset_within_start = if recursive_cut.is_some() {
            (offset - first_visible_content_height).max(0.0)
        } else if compensate_first_visible {
            offset + first_visible_content_height
        } else if self.profile.get().hwpx_stored_layout()
            && terminal
            && offset > 0.5
            && !first_visible_starts_after_table
            && cell.paragraphs.get(para_idx).is_some_and(|paragraph| {
                paragraph.controls.iter().any(|control| {
                    matches!(control, Control::Table(nested) if nested.row_count == 1 && nested.col_count == 1)
                })
            })
        {
            // The HWPX outer RowBreak cell's last source unit is already
            // painted by the preceding fragment. Its terminal 1×1 child uses
            // a fresh viewport, so the raw source offset would paint that same
            // unit at the new page top. Advance by precisely one visible child
            // line; this preserves the terminal tail while starting from the
            // next physical source owner (#3637 HWP 2020 p26→p27).
            offset + first_visible_content_height
        } else {
            offset
        };
        let is_offset_continuation = offset_within_start > 0.5;
        let has_later_host_source_owner = units
            .iter()
            .skip(hi)
            .any(|unit| Self::cell_unit_has_visible_content(cell, unit));
        let terminal_table_before_host_successor = recursive_cut.is_none()
            && terminal
            && is_offset_continuation
            && self.profile.get().native_hwp5_layout()
            && single_cell_nested_continuation
            && has_later_host_source_owner
            && cell
                .paragraphs
                .get(para_idx)
                .is_some_and(|paragraph| paragraph.text.trim().is_empty());
        let terminal_host_line_spacing = if terminal_table_before_host_successor {
            cell.paragraphs
                .get(para_idx)
                .and_then(|paragraph| paragraph.line_segs.first())
                .map(|segment| hwpunit_to_px(segment.line_spacing, self.dpi))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let terminal_continuation_inset = if terminal_table_before_host_successor {
            let nested_top_padding = cell
                .paragraphs
                .get(para_idx)
                .and_then(|paragraph| {
                    paragraph.controls.iter().find_map(|control| match control {
                        Control::Table(table) => Some(table),
                        _ => None,
                    })
                })
                .and_then(|table| {
                    table
                        .cells
                        .first()
                        .map(|nested_cell| self.resolve_cell_padding(nested_cell, table).2)
                })
                .unwrap_or(0.0);
            (first_visible_content_height - first_visible_paint_height).max(0.0)
                + nested_top_padding
        } else {
            0.0
        };
        let terminal_single_cell_tail = recursive_cut.is_none()
            && terminal
            && is_offset_continuation
            && single_cell_nested_continuation
            && !has_later_host_source_owner;
        let visible_height = if terminal_table_before_host_successor {
            // 이 mixed stream은 끝났지만 같은 host cell에는 다음 source 문단이 있다.
            // 자식 표의 실제 마지막 unit까지만 frame을 닫고, host 문단의 후행
            // line-spacing은 아래 flow에만 더한다. terminal tail 보정까지 frame에
            // 넣으면 다음 separator 앞에 빈 표 영역이 생긴다(issue2007 p14).
            flow_visible + terminal_continuation_inset
        } else if recursive_cut.is_some() && is_offset_continuation && !terminal {
            // 재귀 child cursor는 이전 viewport가 예약한 첫 가시 unit을 source offset에서
            // 되감아 정확한 시작 owner를 복원한다. paint viewport도 같은 unit만큼
            // 늘려야 end_cut 안의 마지막 줄이 셀 clip 밖으로 잘리지 않는다. child cut이
            // source 끝을 제한하므로 다음 owner를 다시 그리지는 않는다(42065 p14/p15).
            flow_visible + first_visible_content_height
        } else if recursive_cut.is_none()
            && !terminal
            && offset <= 0.5
            && single_cell_nested_continuation
            && successor_trailing_reservation > 0.5
        {
            // 현재 cut이 자식 1×1 표의 모든 실제 source unit을 포함하고, 바로 다음
            // unit이 content 없는 trailing reservation이며 그 뒤에 실제 source owner가
            // 없을 때 그 reservation은 다음 쪽의 text owner가 아니다. scalar child
            // renderer는 물리 cell 높이로 같은 cut을 다시 계산하므로, 이 작은 예약
            // 높이를 clip에 보존하지 않으면 셀 padding 때문에 마지막 실제 줄 하나가
            // fitting budget 밖으로 밀린다(42065 p15).
            // flow 높이는 아래에서 `flow_visible`을 유지해 다음 sibling 위치는 바꾸지 않는다.
            flow_visible + successor_trailing_reservation
        } else if terminal_single_cell_tail {
            // The terminal 1×1 fragment has no successor to reserve space
            // for. Its final ordinary paragraphs are still laid out one
            // first-unit below the fragment origin, however, so both the
            // nested cell clip and its host RowBreak cell must retain that
            // physical tail. Otherwise the source remains in export-text but
            // SVG/Canvas clips it (42065 p17 section 4).
            flow_visible + first_visible_content_height * 2.0 + 4.0
        } else if self.profile.get().native_hwp5_layout() && compensate_first_visible && !terminal {
            // `compensate_first_visible` advances the child content origin by
            // one unit because the preceding viewport already reserved it.
            // Native HWP5 must shorten the child paint viewport by the same
            // unit; otherwise its end advances one line past the RowCut and
            // both adjacent pages paint that line (42065 p10/p11).  Keep the
            // parent flow height unchanged so pagination/sibling placement
            // continues to use the authoritative RowCut geometry.
            (flow_visible - first_visible_content_height).max(0.0)
        } else if self.profile.get().native_hwp5_layout()
            && is_offset_continuation
            && first_visible_starts_after_table
            && !terminal
        {
            // A new physical block can begin inside a continuation cut.  Its
            // first unit is not a preceding-page reservation, but the cut's
            // final unit is the successor viewport reservation.  Do not let
            // that final line extend the nested paint window past the RowCut
            // owner boundary (42065 p10/p11).
            (flow_visible + first_visible_content_height - 4.0 - last_visible_content_height)
                .max(visible)
        } else if is_offset_continuation && !compensate_first_visible {
            // Mixed text+nested-table units include a small layout allowance
            // (`nested_h + 4.0`) so pagination has enough flow room. That
            // allowance must not expand the visible nested border, otherwise
            // the continuation box encloses the following host paragraph.
            (flow_visible + first_visible_content_height - 4.0).max(visible)
        } else {
            visible
        };
        if total <= 0.5 || visible <= 0.5 {
            return None;
        }
        let remaining = (total - offset).max(0.0);
        let flow_height = if terminal_table_before_host_successor {
            flow_visible + terminal_host_line_spacing
        } else if recursive_cut.is_some() {
            flow_visible
        } else if terminal_single_cell_tail {
            visible_height
        } else if is_offset_continuation && !compensate_first_visible {
            flow_visible + first_visible_content_height
        } else {
            flow_visible.min(remaining)
        };
        // 행 범위는 픽셀 오프셋에서 유도한다. 종전에는 `0..1` 로 고정해, 2행 이상
        // 중첩 표가 텍스트와 문단을 공유하면 연속 페이지가 **행 0 만** 다시 그리고
        // 뒤 행의 내용이 어느 페이지에도 나오지 않았다 (75544 pi=527: 2행 표,
        // off 672 + vis 747 = 전체 1419 로 높이 회계는 완전한데 end_row=1 이라
        // 행 1 의 25문단이 통째로 탈락). #1073 이 per-중첩행 컷 경로에서 고친
        // "row0 재렌더" 와 같은 결함이 혼재 문단 경로에 남아 있던 것이다.
        //
        // 높이 필드는 유닛 회계에서 온 값을 그대로 쓴다 — 조각 경계는 이미 컷이
        // 정했고, 행 변환은 "그 조각이 어느 행들을 담는가" 만 정한다.
        let nested = cell.paragraphs.get(para_idx).and_then(|p| {
            p.controls.iter().find_map(|ctrl| match ctrl {
                Control::Table(t) => Some(&**t),
                _ => None,
            })
        });
        // `terminal` 자체는 마지막 tail의 source cut을 끄는 기존 안전장치다.
        // 그러나 마지막 RowBreak 행의 1×1 block child는 앞 fragment가 child 첫
        // source unit을 이미 소비했어도 terminal이 될 수 있다. 이 경우 물리 clip만
        // 쓰면 p33의 마지막 줄을 p34 top에 다시 paint한다(76076 p33→p34).
        // `native_short_parent_child_fragment_eligible`는 짧은 child를 paginator
        // unit으로 승격하는 별도 계약이고, 여기서는 이미 mixed source cut으로
        // 도달한 terminal child의 시작 cursor만 보존한다.
        let native_short_terminal_child = nested.is_some_and(|child| {
            self.native_short_parent_child_fragment_eligible(
                table,
                cell,
                child,
                self.nested_table_mixed_fragment_heights(child, styles)
                    .iter()
                    .map(|fragment| fragment.height)
                    .sum(),
            )
        });
        let terminal_rowbreak_source_cursor = nested.is_some_and(|child| {
            self.native_terminal_rowbreak_child_source_cursor_eligible(table, cell, child)
        });
        let force_source_start_cut = offset > 0.5
            && terminal
            && (native_short_terminal_child || terminal_rowbreak_source_cursor);
        let (start_row, end_row, mut row_offset_within_start, visible_height) = match nested {
            Some(_) if recursive_cut.is_some() => (0, 1, 0.0, visible_height),
            Some(nt) if nt.row_count > 1 => {
                let ncol = nt.col_count as usize;
                let nrow = nt.row_count as usize;
                let row_heights = self.resolve_row_heights(nt, ncol, nrow, None, styles, true);
                let cs = hwpunit_to_px(nt.cell_spacing as i32, self.dpi);
                let rows = calc_nested_split_rows(&row_heights, cs, offset, visible);
                // [#3658] 종료 조각: start_row 상단 중 이전 쪽에 이미 보인 밴드만큼
                // 내부 오프셋을 부여한다. 종전(0.0 고정)에는 종료 조각이 start_row 를
                // 처음부터 재적층해 행 그리드보다 커지고, 초과한 꼬리 문단이 셀 하단
                // 드롭에 걸려 어느 쪽에도 렌더되지 않았다 (75544 pi=527: 재적층 766px
                // vs 행높이 747px → 마지막 2문단 유실). 이미 보인 밴드를 건너뛰면
                // 잔여 콘텐츠가 행 그리드 안에 들어가고 중복 렌더도 없다.
                let shown_band = if terminal && rows.start_row > 0 && rows.end_row >= nrow {
                    let mut prefix = 0.0f64;
                    for (r, rh) in row_heights.iter().enumerate().take(rows.start_row) {
                        prefix += rh;
                        if r + 1 < nrow {
                            prefix += cs;
                        }
                    }
                    let band = offset - prefix;
                    if band > 0.5 {
                        band
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                let vis_h = if terminal && shown_band > 0.0 {
                    // 종료 조각의 표시 상자는 포함 행 전체(그리드) 높이를 유지한다 —
                    // 유닛 회계 기반 상자가 행 그리드보다 작으면 셀 클립이 꼬리를 자른다.
                    visible_height.max(rows.visible_height)
                } else {
                    visible_height
                };
                (rows.start_row, rows.end_row, shown_band, vis_h)
            }
            // 1행 표는 종전 규약 유지 — 행 경계가 없어 오프셋만으로 이어진다.
            _ => (0, 1, offset_within_start, visible_height),
        };
        if terminal_table_before_host_successor {
            // continuation 첫 line box의 leading과 셀 top padding을 source offset에서
            // 되돌려 현재 fragment 안에 다시 보인다. paint만 아래로 옮기며 위에서
            // 계산한 flow_height에는 더하지 않아 다음 host 문단 위치는 유지한다.
            row_offset_within_start =
                (row_offset_within_start - terminal_continuation_inset).max(0.0);
        }
        Some(NestedTableSplit {
            start_row,
            end_row,
            visible_height,
            flow_height,
            // Keep one visible content unit reserved in bbox/flow so the
            // border wraps only that tail line and the following paragraph in
            // the host cell starts below it. This reservation is physical
            // space only; `offset_within_start` above remains the full
            // consumed content origin.
            // 긴 terminal child는 source cursor가 p33까지의 단위를 이미 버린다.
            // 같은 offset으로 물리 원점까지 올리면 p34의 새 첫 줄도 clip 위로
            // 이중 소비된다. short-parent 계약은 기존 물리 inset을 유지한다.
            offset_within_start: if terminal_rowbreak_source_cursor && !native_short_terminal_child
            {
                0.0
            } else {
                row_offset_within_start
            },
            content_offset: offset,
            force_source_start_cut,
            // p33→34류(terminal_rowbreak_source_cursor만 true)는 이미 소유가 끝난 마지막
            // unit을 재생하면 안 되므로, native short-parent 형상에서만 켠다.
            replay_terminal_boundary_unit: native_short_terminal_child,
            terminal,
            recursive_cut,
        })
    }


    /// [#4069] 바깥 셀 컷에 선택된 `CELL` 분할 중첩 표 조각을 자식 표의
    /// `(row, RowCut)` 범위로 되돌린다. 측정 원장에 기록한 시작/끝 cursor를
    /// 그대로 사용하므로 페이지마다 전체 행을 다시 그리는 scalar clip이 없다.
    pub(crate) fn nested_table_split_from_cut_units(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
        para_idx: usize,
    ) -> Option<NestedTableSplit> {
        let units = self.cell_units(cell, table, styles);
        let lo = start_unit.min(units.len());
        let hi = end_unit.min(units.len()).max(lo);
        let mut first_unit: Option<&CellUnit> = None;
        let mut last_unit: Option<&CellUnit> = None;
        let mut has_fragment = false;
        let mut visible_height = 0.0;
        for unit in units.iter().take(hi).skip(lo) {
            if unit.para_idx != para_idx || unit.nested_row.is_none() {
                continue;
            }
            first_unit.get_or_insert(unit);
            last_unit = Some(unit);
            has_fragment |= unit.nested_table_fragment.is_some();
            visible_height += unit.height;
        }
        if !has_fragment {
            return None;
        }
        let (first_unit, last_unit) = (first_unit?, last_unit?);
        let first_row = first_unit.nested_row?;
        let last_row = last_unit.nested_row?;
        let start_cut = first_unit
            .nested_table_fragment
            .as_ref()
            .map(|fragment| {
                if fragment.start_cut.iter().all(|cut| *cut == 0) {
                    Vec::new()
                } else {
                    fragment.start_cut.clone()
                }
            })
            .unwrap_or_default();
        let (end_cut, terminal) = last_unit
            .nested_table_fragment
            .as_ref()
            .map(|fragment| {
                if fragment.terminal {
                    (Vec::new(), true)
                } else {
                    (fragment.end_cut.clone(), false)
                }
            })
            .unwrap_or_else(|| (Vec::new(), true));
        Some(NestedTableSplit {
            start_row: first_row,
            end_row: last_row + 1,
            visible_height,
            flow_height: visible_height,
            offset_within_start: 0.0,
            content_offset: 0.0,
            force_source_start_cut: false,
            replay_terminal_boundary_unit: false,
            terminal,
            recursive_cut: Some(NestedTableCut {
                start_row: first_row,
                end_row: last_row + 1,
                start_cut,
                end_cut,
                is_block_split: false,
            }),
        })
    }


    /// 컷 유닛 범위를 **중첩 표 행 범위**로 옮긴다 (per-중첩행 유닛 경로).
    ///
    /// per-중첩행 분해가 붙은 문단은 유닛이 `nested_row` 를 들고 있으므로, 컷에 들어온
    /// 유닛들의 행 번호에서 곧바로 범위를 얻는다.
    ///
    /// 종전에는 호출부가 "컷 유닛 인덱스 == 중첩행 번호" 라고 가정해 셀이 **문단 1개**
    /// 일 때만 이 경로를 썼다. 문단이 여럿인 셀에서는 유닛에 텍스트 줄이 섞여 인덱스가
    /// 행 번호가 아니게 되고, 그러면 렌더가 `available_h` 휴리스틱으로 폴백해 연속
    /// 페이지가 행 0 부터 다시 그린다(뒤 행 유실). 유닛이 이미 행 번호를 들고 있으니
    /// 인덱스 가정을 버리고 그 필드를 읽는다.
    pub(crate) fn nested_row_range_from_cut_units(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
        para_idx: usize,
    ) -> Option<(usize, usize)> {
        let units = self.cell_units(cell, table, styles);
        let lo = start_unit.min(units.len());
        let hi = end_unit.min(units.len()).max(lo);
        let mut first: Option<usize> = None;
        let mut last: Option<usize> = None;
        for unit in units.iter().take(hi).skip(lo) {
            if unit.para_idx != para_idx {
                continue;
            }
            let Some(row) = unit.nested_row else {
                continue;
            };
            first = Some(first.map_or(row, |f: usize| f.min(row)));
            last = Some(last.map_or(row, |l: usize| l.max(row)));
        }
        match (first, last) {
            (Some(f), Some(l)) => Some((f, l + 1)),
            _ => None,
        }
    }


    /// RowBreak 분할의 컷 범위에 실제 보이는 내용이 남아 있는지 확인한다.
    ///
    /// 마지막 continuation 에 빈 문단/패딩만 남은 조각은 한컴 PDF에서 별도 페이지를
    /// 만들지 않는 경우가 있어, 페이지네이터가 terminal sliver 를 걸러낼 때 사용한다.
    pub(crate) fn row_cut_range_has_visible_content(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        start_cut: &[usize],
        end_cut: &[usize],
        styles: &ResolvedStyleSet,
    ) -> bool {
        let mut row_cells: Vec<&crate::model::table::Cell> = table
            .cells
            .iter()
            .filter(|c| c.row as usize == row && c.row_span == 1)
            .collect();
        row_cells.sort_by_key(|c| c.col);

        for (i, cell) in row_cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let su = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            let eu = end_cut
                .get(i)
                .copied()
                .unwrap_or(units.len())
                .clamp(su, units.len());
            if units[su..eu]
                .iter()
                .any(|unit| Self::cell_unit_has_visible_content(cell, unit))
            {
                return true;
            }
        }

        false
    }


    pub(crate) fn cell_unit_has_visible_content(cell: &crate::model::table::Cell, unit: &CellUnit) -> bool {
        if unit.nested_row.is_some() {
            return true;
        }

        let Some(para) = cell.paragraphs.get(unit.para_idx) else {
            return false;
        };
        !para.text.trim().is_empty() || !para.controls.is_empty()
    }


    /// [Task #1809] 종전 is_hwpx_source 조기 0 반환 제거 — 컷 이월 조각의 flow
    /// extra 는 소스 무관 기하다. 한글 편집기 대조(admrul_0072 서명 셀: 텍스트→
    /// 하단 경계 한글 25.5pt = extra 적용 25.9pt, 미적용 13.9pt)로 적용이 정답.
    ///
    /// [#4129] per-para O(P×U) 재스캔을 units 1-pass run-walk 로 재작성 (O(U)).
    /// mixed 유닛은 `cell_units_uncached` 의 단일 문단 루프(ascending `pi`)에서만
    /// 생성되므로 `para_idx` 가 유닛 순서상 단조 비감소 — 문단별 mixed run 이
    /// 연속 구간이다 (단조성은 아래 debug_assert 가 지킨다). 종전 구현과의 비트
    /// 동일성은 corpus 355개 전수 RHWP_2424_SHADOW A/B 대조로 검증했고, 게이트와
    /// reference 구현은 검증 완료 후 같은 PR 체인의 후속 레이어에서 제거했다.
    pub(crate) fn mixed_nested_flow_extra_from_cut(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        start_unit: usize,
        end_unit: usize,
    ) -> f64 {
        let units = self.cell_units(cell, table, styles);
        let lo = start_unit.min(units.len());
        let hi = end_unit.min(units.len()).max(lo);
        let mut extra = 0.0;
        // [#4129 회귀 가드] 실제 유닛 방문 횟수 집계 — run-walk 는 호출당 ≤2×U.
        // per-para 재스캔(O(P×U))류가 되살아나면 통합 테스트의 스캔 총량 상한이
        // 폭발한다. 반환 직전 한 번에 프로세스 카운터로 누적한다.
        let mut issue4129_visited: u64 = 0;

        let mut u = 0;
        while u < units.len() {
            // 종전 per-para 루프는 0..paragraphs.len() 이라 범위 밖 para_idx
            // 유닛은 방문 자체가 없었다 — 동일하게 무시한다.
            if !units[u].mixed_nested_fragment || units[u].para_idx >= cell.paragraphs.len() {
                issue4129_visited += 1;
                u += 1;
                continue;
            }
            let para_idx = units[u].para_idx;
            let mut offset = 0.0;
            let mut total = 0.0;
            let mut visible_units: Vec<(f64, bool)> = Vec::new();
            let mut has_recursive_fragment = false;
            let mut has_non_recursive_fragment = false;
            let mut idx = u;
            while idx < units.len() {
                issue4129_visited += 1;
                let unit = &units[idx];
                if unit.mixed_nested_fragment {
                    if unit.para_idx != para_idx {
                        break;
                    }
                    if unit.mixed_nested_recursive {
                        has_recursive_fragment = true;
                    } else {
                        has_non_recursive_fragment = true;
                    }
                    total += unit.height;
                    if idx < lo {
                        offset += unit.height;
                    }
                    if idx >= lo && idx < hi {
                        visible_units.push((unit.height, unit.mixed_nested_trailing));
                    }
                }
                idx += 1;
            }
            debug_assert!(
                idx >= units.len() || units[idx].para_idx > para_idx,
                "cell_units mixed para_idx 단조성 위반: {} 뒤에 {}",
                para_idx,
                units[idx].para_idx,
            );
            u = idx;

            if total <= 0.5 || offset <= 0.5 {
                continue;
            }
            while visible_units.last().is_some_and(|(_, trailing)| *trailing) {
                visible_units.pop();
            }
            let flow_visible: f64 = visible_units.iter().map(|(height, _)| *height).sum();
            if flow_visible <= 0.5 {
                continue;
            }
            let first_visible_content_height = visible_units
                .iter()
                .find_map(|(height, trailing)| (!*trailing).then_some(*height))
                .unwrap_or(0.0);
            let offset_within_start = (offset - first_visible_content_height).max(0.0);
            let terminal = end_unit >= units.len();
            let authoritative_recursive_run = self.profile.get().native_hwp5_layout()
                && has_recursive_fragment
                && !has_non_recursive_fragment;
            let single_cell_nested_continuation = table.row_count == 1
                && table.col_count == 1
                && cell.paragraphs.get(para_idx).is_some_and(|paragraph| {
                    paragraph.controls.iter().any(|control| {
                        matches!(control, Control::Table(nested) if nested.row_count == 1 && nested.col_count == 1)
                    })
                });
            let native_short_parent_child = cell
                .paragraphs
                .get(para_idx)
                .and_then(|paragraph| {
                    paragraph.controls.iter().find_map(|control| match control {
                        Control::Table(child) => Some(child.as_ref()),
                        _ => None,
                    })
                })
                .is_some_and(|child| {
                    self.native_short_parent_child_fragment_eligible(
                        table,
                        cell,
                        child,
                        self.nested_table_mixed_fragment_heights(child, styles)
                            .iter()
                            .map(|fragment| fragment.height)
                            .sum(),
                    )
                });
            // 재귀 투영 run은 `mixed_nested_split_from_cut`의 child RowCut이
            // source cursor와 viewport를 이미 함께 소유한다. 여기에 scalar
            // continuation 보정을 다시 더하면 부모 행만 첫 가시 유닛만큼 커져
            // 뒤 sibling을 다음 쪽으로 민다(59043 p36의 27.7px 중복). legacy
            // mixed fallback의 42065 p17 terminal 보정과 HWPX 저장 viewport
            // 보정은 유지한다.
            if offset_within_start > 0.5 && !authoritative_recursive_run {
                if terminal && native_short_parent_child {
                    // The native short-parent continuation replays its boundary
                    // source unit in the current fragment.  The generic parent
                    // extra would reserve that whole unit a second time and leave
                    // an empty row tail (76076 p82); retain only the mixed-flow
                    // clip guard.
                    extra += 4.0;
                } else if terminal && single_cell_nested_continuation {
                    // Keep the parent RowBreak cell in lockstep with the
                    // terminal nested-cell viewport.  Reserving only one
                    // unit leaves the parent clip above the nested tail, so
                    // the last ordinary paragraphs exist in the tree but
                    // disappear in SVG/Canvas (42065 p17 section 4).
                    extra += first_visible_content_height * 2.0 + 4.0;
                } else if !single_cell_nested_continuation {
                    extra += first_visible_content_height;
                }
            }
        }

        crate::diagnostics::perf_counters::MIXED_NESTED_UNITS_SCANNED
            .fetch_add(issue4129_visited, std::sync::atomic::Ordering::Relaxed);
        extra
    }


    /// [Task #993 / #1022] 분할 행에서 컷 범위 `[start_cut, end_cut)` 사이의
    /// **행 총 높이**(패딩 포함)를 반환한다. HeightMeasurer 와 정합 — 셀별로
    /// `max(cell.height, content + pad_cell)` 를 산출해 행 max.
    ///
    /// - 분할 아닌 행(start_cut/end_cut 모두 빈 Vec): `max(cell.height,
    ///   content+pad_cell)` per cell, row max.
    /// - 분할 행(컷 범위 일부): `content_in_range + pad_cell` per cell, row max.
    ///   분할 시 cell.height 강제는 적용하지 않는다(콘텐츠가 부분이므로).
    ///
    /// 셀 인덱스는 `advance_row_cut` 과 동일하게 `row_span==1` 셀을 col
    /// 오름차순 정렬한 순서다.
    pub(crate) fn row_cut_content_height(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        start_cut: &[usize],
        end_cut: &[usize],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let mut row_cells: Vec<&crate::model::table::Cell> = table
            .cells
            .iter()
            .filter(|c| c.row as usize == row && c.row_span == 1)
            .collect();
        row_cells.sort_by_key(|c| c.col);
        let is_whole_row = start_cut.is_empty() && end_cut.is_empty();
        let mut max_h = 0.0f64;
        for (i, cell) in row_cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let su = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            let eu = end_cut
                .get(i)
                .copied()
                .unwrap_or(units.len())
                .clamp(su, units.len());
            let mixed_nested_extra = if is_whole_row {
                0.0
            } else {
                self.mixed_nested_flow_extra_from_cut(cell, table, styles, su, eu)
            };
            let content: f64 =
                units[su..eu].iter().map(|u| u.height).sum::<f64>() + mixed_nested_extra;
            let (_, _, pad_top, pad_bottom) = self.resolve_cell_padding(cell, table);
            let has_visible_cut = units[su..eu]
                .iter()
                .any(|unit| Self::cell_unit_has_visible_content(cell, unit));
            let pad_cell = if is_whole_row || has_visible_cut {
                pad_top + pad_bottom
            } else {
                0.0
            };
            let cell_h_px = if cell.height < 0x8000_0000 {
                hwpunit_to_px(cell.height as i32, self.dpi)
            } else {
                0.0
            };
            // [#2146] 저장 LINE_SEG 이 전혀 없고 모든 문단이 1줄(폭 여유 포함)인
            // 라벨 셀(사선 헤더 등)은 재합성 초과가 순수 줄높이 인플레이션 —
            // 선언 셀높이 신뢰. (21761835 r0: 선언 3928HU=52.4px = 한글 실측,
            // 재합성 79.3px) 판정 기준은 composer::no_ls_short_label_cell 주석 참조.
            let no_ls_label_cell = cell_h_px > 0.0 && {
                let (pad_left, pad_right, _, _) = self.resolve_cell_padding(cell, table);
                let cell_w_px = if cell.width < 0x8000_0000 {
                    hwpunit_to_px(cell.width as i32, self.dpi)
                        * self.render_table_width_scale(table)
                } else {
                    0.0
                };
                crate::renderer::composer::no_ls_short_label_cell(
                    cell,
                    table,
                    (cell_w_px - pad_left - pad_right).max(0.0),
                    cell_h_px - pad_top - pad_bottom,
                    styles,
                )
            };
            let h = if is_whole_row {
                if no_ls_label_cell {
                    cell_h_px
                } else {
                    // HeightMeasurer required_height + row 단계 1 cell.height max 정합.
                    (content + pad_cell).max(cell_h_px)
                }
            } else {
                // 분할 행 — cell.height 강제 없음.
                content + pad_cell
            };
            if h > max_h {
                max_h = h;
            }
        }
        max_h
    }


    /// 새 physical page에 온전히 들어갈 1×1 중첩 표 래퍼의 선행 문단 묶음을
    /// 현재 쪽 끝에 고립시키지 않아야 하는지 판정한다.
    ///
    /// Native HWP5 `RowBreak` 표에는 비어 있는 outer host → 1×1 child table →
    /// child 본문/내부 표라는 저장 형상이 있다. outer 행의 잔여 공간에 child 본문의
    /// 앞 몇 줄만 소비하면, scalar child renderer는 그 줄을 현재 쪽에 칠하지만 한컴은
    /// 그 다음 내부 표 앞의 묶음을 새 쪽에서 함께 시작한다. 86712의 r27이 그 사례다.
    ///
    /// 이 규칙은 일반 문단/중첩 표에 적용하지 않는다. 다음 내부 표 source paragraph
    /// 직전의 mixed-unit 경계를 provenance로 식별하고, 그 전 묶음이 새 본문의 대부분을
    /// 채울 때만 true다. 단순히 새 쪽에 들어간다는 이유로 중간 길이의 도입부까지
    /// 이월하면 59043 p35처럼 PDF가 허용한 분할을 망가뜨린다. 따라서 실제로 한
    /// 페이지보다 긴 child나 이미 continuation인 행, 새 본문의 일부만 쓰는 prefix는
    /// 종전 CellUnit 분할을 유지한다.
    pub(crate) fn should_defer_fresh_rowbreak_wrapper_prefix(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        start_cut: &[usize],
        end_cut: &[usize],
        fresh_body_height: f64,
        styles: &ResolvedStyleSet,
    ) -> bool {
        if !start_cut.is_empty()
            || !self.profile.get().native_hwp5_layout()
            || table.common.treat_as_char
            || !matches!(table.page_break, TablePageBreak::RowBreak)
        {
            return false;
        }

        let mut row_cells: Vec<&crate::model::table::Cell> = table
            .cells
            .iter()
            .filter(|cell| cell.row as usize == row && cell.row_span == 1)
            .collect();
        row_cells.sort_by_key(|cell| cell.col);

        for (wrapper_index, wrapper_cell) in row_cells.iter().enumerate() {
            let Some(host) = wrapper_cell
                .paragraphs
                .iter()
                .find(|paragraph| Self::paragraph_hosts_single_cell_nested_table(paragraph))
            else {
                continue;
            };
            let Some(child) = host.controls.iter().find_map(|control| match control {
                Control::Table(table) if table.row_count == 1 && table.col_count == 1 => {
                    Some(table.as_ref())
                }
                _ => None,
            }) else {
                continue;
            };
            let Some(child_cell) = child.cells.first() else {
                continue;
            };
            let Some(first_inner_table_para) = child_cell.paragraphs.iter().position(|paragraph| {
                paragraph
                    .controls
                    .iter()
                    .any(|control| matches!(control, Control::Table(_)))
            }) else {
                if std::env::var("RHWP_DIAG_SCAN").is_ok() {
                    eprintln!(
                        "DIAG_SCAN DEFER_WRAPPER_PREFIX? r={} c={} child_paras={} inner_table_para=none",
                        row,
                        wrapper_cell.col,
                        child_cell.paragraphs.len(),
                    );
                }
                continue;
            };

            let wrapper_units = self.cell_units(wrapper_cell, table, styles);
            let Some(inner_table_start) = wrapper_units.iter().position(|unit| {
                unit.mixed_nested_fragment
                    && !unit.mixed_nested_trailing
                    && unit.mixed_nested_source_para_idx == Some(first_inner_table_para)
            }) else {
                if std::env::var("RHWP_DIAG_SCAN").is_ok() {
                    eprintln!(
                        "DIAG_SCAN DEFER_WRAPPER_PREFIX? r={} c={} child_inner_para={} units={} mixed_sources={:?}",
                        row,
                        wrapper_cell.col,
                        first_inner_table_para,
                        wrapper_units.len(),
                        wrapper_units
                            .iter()
                            .filter(|unit| unit.mixed_nested_fragment && !unit.mixed_nested_trailing)
                            .filter_map(|unit| unit.mixed_nested_source_para_idx)
                            .collect::<Vec<_>>(),
                    );
                }
                continue;
            };
            if inner_table_start == 0 {
                continue;
            }

            let partial_end = end_cut
                .get(wrapper_index)
                .copied()
                .unwrap_or(wrapper_units.len())
                .min(wrapper_units.len());
            if std::env::var("RHWP_DIAG_SCAN").is_ok() {
                eprintln!(
                    "DIAG_SCAN DEFER_WRAPPER_PREFIX? r={} c={} inner_start={} partial_end={} start_cut={:?} end_cut={:?}",
                    row,
                    wrapper_cell.col,
                    inner_table_start,
                    partial_end,
                    start_cut,
                    end_cut,
                );
            }
            if partial_end == 0 || partial_end >= inner_table_start {
                continue;
            }

            // 형제 라벨 셀도 선행 묶음에 포함돼야 한다. 현재 조각에서 아직 남은
            // 형제 content가 있으면 이월이 행 구조를 바꾸므로 적용하지 않는다.
            if row_cells.iter().enumerate().any(|(index, cell)| {
                if index == wrapper_index {
                    return false;
                }
                let units = self.cell_units(cell, table, styles);
                end_cut.get(index).copied().unwrap_or(units.len()) < units.len()
            }) {
                continue;
            }

            let mut prefix_end = Vec::with_capacity(row_cells.len());
            for (index, cell) in row_cells.iter().enumerate() {
                if index == wrapper_index {
                    prefix_end.push(inner_table_start);
                } else {
                    prefix_end.push(self.cell_units(cell, table, styles).len());
                }
            }
            let prefix_height = self.row_cut_content_height(table, row, &[], &prefix_end, styles);
            // 이 helper는 scan 중 호출돼 `LayoutEngine::current_body_area`가 아직
            // 갱신되지 않은 경로가 있다. 현재 `TypesetState`가 보유한 fresh 본문
            // 높이를 호출자가 넘겨야 실제 다음 페이지 수용성을 판정할 수 있다.
            let body_height = fresh_body_height;
            if std::env::var("RHWP_DIAG_SCAN").is_ok() {
                eprintln!(
                    "DIAG_SCAN DEFER_WRAPPER_PREFIX_FIT r={} prefix={:.1} body={:.1} prefix_end={:?}",
                    row, prefix_height, body_height, prefix_end,
                );
            }
            // 86712 r27의 926.2 / 971.3px처럼 사실상 한 page를 이루는 prefix만
            // atomic start로 다룬다. 59043 r5의 569.1 / 971.3px 도입부는 PDF에서
            // 앞 page에 남아야 하므로 이 임계값 아래에서는 기존 분할을 보존한다.
            const NEAR_FULL_FRESH_BODY_RATIO: f64 = 0.80;
            if body_height > 0.0
                && prefix_height <= body_height + 0.5
                && prefix_height >= body_height * NEAR_FULL_FRESH_BODY_RATIO
            {
                return true;
            }
        }

        false
    }


    /// Fresh 1×1 wrapper fragment가 첫 내부 표를 paint하지 않는 마지막 RowCut을
    /// 반환한다.
    ///
    /// mixed projection에서 `end_cut`은 scalar child renderer에 inclusive owner로
    /// 전달된다. 따라서 첫 inner-table atom(예: unit 59) 바로 전 spacer(unit 58)를
    /// end로 넘겨도 표가 현재 fragment에 그려질 수 있다. 표 source와 그 선행 spacer
    /// 둘 다 다음 page로 넘기는 `table_atom - 2`가 안전한 경계다. 이 값은 source
    /// paragraph provenance로만 구하며 native HWP5 non-TAC RowBreak wrapper에만 적용한다.
    pub(crate) fn fresh_rowbreak_wrapper_safe_prefix_end_cut(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        start_cut: &[usize],
        end_cut: &[usize],
        styles: &ResolvedStyleSet,
    ) -> Option<RowCut> {
        if !start_cut.is_empty()
            || !self.profile.get().native_hwp5_layout()
            || table.common.treat_as_char
            || !matches!(table.page_break, TablePageBreak::RowBreak)
        {
            return None;
        }
        let mut row_cells: Vec<&crate::model::table::Cell> = table
            .cells
            .iter()
            .filter(|cell| cell.row as usize == row && cell.row_span == 1)
            .collect();
        row_cells.sort_by_key(|cell| cell.col);

        for (wrapper_index, wrapper_cell) in row_cells.iter().enumerate() {
            let Some(child) = wrapper_cell.paragraphs.iter().find_map(|paragraph| {
                if !Self::paragraph_hosts_single_cell_nested_table(paragraph) {
                    return None;
                }
                paragraph.controls.iter().find_map(|control| match control {
                    Control::Table(table) if table.row_count == 1 && table.col_count == 1 => {
                        Some(table.as_ref())
                    }
                    _ => None,
                })
            }) else {
                continue;
            };
            let Some(first_inner_table_para) = child.cells.first().and_then(|cell| {
                cell.paragraphs.iter().position(|paragraph| {
                    paragraph
                        .controls
                        .iter()
                        .any(|control| matches!(control, Control::Table(_)))
                })
            }) else {
                continue;
            };
            let units = self.cell_units(wrapper_cell, table, styles);
            let Some(first_table_atom) = units.iter().position(|unit| {
                unit.mixed_nested_fragment
                    && !unit.mixed_nested_trailing
                    && unit.mixed_nested_source_para_idx == Some(first_inner_table_para)
            }) else {
                continue;
            };
            let current_end = end_cut
                .get(wrapper_index)
                .copied()
                .unwrap_or(units.len())
                .min(units.len());
            if current_end != first_table_atom || first_table_atom < 2 {
                continue;
            }
            if row_cells.iter().enumerate().any(|(index, cell)| {
                index != wrapper_index
                    && end_cut
                        .get(index)
                        .copied()
                        .unwrap_or_else(|| self.cell_units(cell, table, styles).len())
                        < self.cell_units(cell, table, styles).len()
            }) {
                continue;
            }

            let mut safe_end_cut = end_cut.to_vec();
            safe_end_cut[wrapper_index] = first_table_atom - 2;
            return Some(safe_end_cut);
        }
        None
    }


    /// RowBreak 분할 예산에서 실제 남은 가시 내용이 있는 셀의 패딩만 예약한다.
    ///
    /// Q&A 표처럼 왼쪽 gutter 빈 셀에 큰 padding 이 들어간 행은 그 padding 때문에
    /// 오른쪽 답변 셀의 첫 줄까지 다음 쪽으로 밀릴 수 있다. 분할 행에서는 보이는
    /// cut 이 남은 셀의 padding 만 행 예산에 반영해 렌더러의 split 높이와 맞춘다.
    pub(crate) fn row_remaining_visible_padding_height(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        start_cut: &[usize],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let mut row_cells: Vec<&crate::model::table::Cell> = table
            .cells
            .iter()
            .filter(|c| c.row as usize == row && c.row_span == 1)
            .collect();
        row_cells.sort_by_key(|c| c.col);

        let mut max_padding = 0.0f64;
        for (i, cell) in row_cells.iter().enumerate() {
            let units = self.cell_units(cell, table, styles);
            let su = start_cut.get(i).copied().unwrap_or(0).min(units.len());
            if !units[su..]
                .iter()
                .any(|unit| Self::cell_unit_has_visible_content(cell, unit))
            {
                continue;
            }
            let (_, _, pad_top, pad_bottom) = self.resolve_cell_padding(cell, table);
            max_padding = max_padding.max(pad_top + pad_bottom);
        }
        max_padding
    }


    /// 줄 범위(line_ranges)에 해당하는 셀 콘텐츠의 실제 렌더링 높이를 계산한다.
    /// compute_cell_line_ranges()의 결과를 받아서, 렌더링될 줄들의 높이를 합산한다.
    /// MeasuredCell 규칙: 첫 문단 spacing_before 없음, 마지막 문단 spacing_after 없음,
    /// 셀 마지막 줄 line_spacing 제외.
    pub(crate) fn calc_visible_content_height_from_ranges(
        &self,
        composed_paras: &[ComposedParagraph],
        paragraphs: &[crate::model::paragraph::Paragraph],
        line_ranges: &[(usize, usize)],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        self.calc_visible_content_height_from_ranges_with_offset(
            composed_paras,
            paragraphs,
            line_ranges,
            styles,
            0.0,
        )
    }


    /// calc_visible_content_height_from_ranges 의 확장판 — split_start 의 content_offset 을 받아서
    /// 한 페이지보다 큰 nested table 의 잔여 높이를 정확히 계산한다.
    /// [Task #362] split_start 시 nested table 잔여 높이 누락으로 row 높이가 잘못 계산되는 결함 정정.
    pub(crate) fn calc_visible_content_height_from_ranges_with_offset(
        &self,
        composed_paras: &[ComposedParagraph],
        paragraphs: &[crate::model::paragraph::Paragraph],
        line_ranges: &[(usize, usize)],
        styles: &ResolvedStyleSet,
        content_offset: f64,
    ) -> f64 {
        let para_count = paragraphs.len();
        let mut total = 0.0;
        let mut cum_pos = 0.0f64; // 누적 콘텐츠 위치 (compute_cell_line_ranges 와 동일)
        let first_visible_pi = line_ranges.iter().position(|&(s, e)| s < e);
        let _last_visible_pi = line_ranges.iter().rposition(|&(s, e)| s < e);
        for (pi, ((comp, para), &(start, end))) in composed_paras
            .iter()
            .zip(paragraphs.iter())
            .zip(line_ranges.iter())
            .enumerate()
        {
            let para_style = styles.para_styles.get(para.para_shape_id as usize);
            let is_last_para = pi + 1 == para_count;
            let line_count = comp.lines.len();
            let spacing_before = if pi > 0 {
                para_style.map(|s| s.spacing_before).unwrap_or(0.0)
            } else {
                0.0
            };
            let spacing_after = if !is_last_para {
                para_style.map(|s| s.spacing_after).unwrap_or(0.0)
            } else {
                0.0
            };
            let has_table_in_para = para.controls.iter().any(|c| matches!(c, Control::Table(_)));

            // [Task #362] nested table paragraph 의 실제 콘텐츠 높이
            // (compute_cell_line_ranges 와 동일한 시멘틱)
            let para_h = if line_count == 0 || has_table_in_para {
                let nested_h: f64 = para
                    .controls
                    .iter()
                    .map(|ctrl| {
                        if let Control::Table(t) = ctrl {
                            self.calc_nested_table_height(t, styles)
                        } else {
                            0.0
                        }
                    })
                    .sum();
                if line_count == 0 {
                    let h = if nested_h > 0.0 {
                        nested_h
                    } else {
                        hwpunit_to_px(400, self.dpi)
                    };
                    spacing_before + h + spacing_after
                } else {
                    let line_based_h: f64 = comp
                        .lines
                        .iter()
                        .enumerate()
                        .map(|(li, line)| {
                            let h = hwpunit_to_px(line.line_height, self.dpi);
                            let ls = hwpunit_to_px(line.line_spacing, self.dpi);
                            let is_cell_last_line = is_last_para && li + 1 == line_count;
                            let mut lh = if !is_cell_last_line { h + ls } else { h };
                            if li == 0 {
                                lh += spacing_before;
                            }
                            if li == line_count - 1 {
                                lh += spacing_after;
                            }
                            lh
                        })
                        .sum();
                    nested_h.max(line_based_h)
                }
            } else {
                0.0 // 일반 line 단위 처리는 아래 분기에서
            };

            // nested table paragraph 처리
            if (line_count == 0 || has_table_in_para) && start < end {
                // [Task #362] 한 페이지보다 큰 nested table 분할: 시작 위치가 offset 이전이면
                // 잔여 = para_end_pos - max(content_offset, para_start_pos)
                let para_start_pos = cum_pos;
                let para_end_pos = cum_pos + para_h;
                if content_offset > 0.0
                    && para_start_pos < content_offset
                    && para_end_pos > content_offset
                {
                    // 분할 케이스: offset 이후의 잔여만 누적
                    total += para_end_pos - content_offset;
                } else if content_offset > 0.0 && para_end_pos <= content_offset {
                    // 이전 페이지에서 다 표시됨
                } else {
                    // 전체 표시
                    total += para_h;
                }
                cum_pos = para_end_pos;
                continue;
            }

            if start >= end {
                // 보이지 않는 일반 paragraph: cum_pos 만 진행
                if has_table_in_para || line_count == 0 {
                    cum_pos += para_h;
                } else {
                    let line_based_h: f64 = comp
                        .lines
                        .iter()
                        .enumerate()
                        .map(|(li, line)| {
                            let h = hwpunit_to_px(line.line_height, self.dpi);
                            let ls = hwpunit_to_px(line.line_spacing, self.dpi);
                            let is_cell_last_line = is_last_para && li + 1 == line_count;
                            let mut lh = if !is_cell_last_line { h + ls } else { h };
                            if li == 0 {
                                lh += spacing_before;
                            }
                            if li == line_count - 1 {
                                lh += spacing_after;
                            }
                            lh
                        })
                        .sum();
                    cum_pos += line_based_h;
                }
                continue;
            }

            let is_visible_first = Some(pi) == first_visible_pi;
            // spacing_before: 렌더링되는 첫 문단에서는 적용하지 않음
            if start == 0 && !is_visible_first {
                total += spacing_before;
            }
            for li in start..end {
                if li < line_count {
                    let line = &comp.lines[li];
                    let h = hwpunit_to_px(line.line_height, self.dpi);
                    let is_cell_last_line = is_last_para && li + 1 == line_count;
                    if !is_cell_last_line {
                        total += h + hwpunit_to_px(line.line_spacing, self.dpi);
                    } else {
                        total += h;
                    }
                }
            }
            // spacing_after: 마지막 문단에서는 적용하지 않음
            if end == comp.lines.len() && end > start && !is_last_para {
                total += spacing_after;
            }
            // cum_pos 갱신 (전체 paragraph 가 차지하는 위치)
            let line_based_h: f64 = comp
                .lines
                .iter()
                .enumerate()
                .map(|(li, line)| {
                    let h = hwpunit_to_px(line.line_height, self.dpi);
                    let ls = hwpunit_to_px(line.line_spacing, self.dpi);
                    let is_cell_last_line = is_last_para && li + 1 == line_count;
                    let mut lh = if !is_cell_last_line { h + ls } else { h };
                    if li == 0 {
                        lh += spacing_before;
                    }
                    if li == line_count - 1 {
                        lh += spacing_after;
                    }
                    lh
                })
                .sum();
            cum_pos += line_based_h;
        }
        total
    }

}
