//! horizontal_cell — table_layout.rs 에서 무변동 이동
use super::*;

impl LayoutEngine {
    /// [Task #2089] 가로쓰기 셀 본문 배치 — 셀 문단/TAC/수식/중첩표 방출.
    /// 원본 무변경 통이동 (탈출은 전부 내부 루프 소속).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn layout_horizontal_cell_paragraphs(
        &self,
        tree: &mut LayoutFrame,
        table_node: &mut RenderNode,
        cell_node: &mut RenderNode,
        cell: &crate::model::table::Cell,
        composed_paras: &[ComposedParagraph],
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        bin_data_content: &[BinDataContent],
        table_meta: Option<(usize, usize)>,
        enclosing_cell_ctx: &Option<CellContext>,
        row_filter: Option<(usize, usize)>,
        row_y: &[f64],
        effective_valign: VerticalAlign,
        v: HorizontalCellVars,
    ) {
        let HorizontalCellVars {
            cell_idx,
            r,
            cell_y,
            cell_h,
            content_cell_y,
            pad_top,
            inner_x,
            inner_width,
            inner_height,
            text_y_start,
            use_top_vpos_anchor,
            upper_clip_line_reservation,
            trust_stored_cell_flow,
            has_nested_table,
            section_index,
            outline_numbering_id,
            depth,
            clamp_header_negative_para_offset,
            outer_host_stored_vpos_hu,
            inline_table_flow_y_shift,
            single_row_continuation,
            single_row_continuation_offset,
            single_row_fragment,
            single_row_fragment_content_offset,
            force_source_start_cut,
            replay_terminal_boundary_unit,
            split_terminal,
        } = v;
        let inner_area = LayoutRect {
            x: inner_x,
            y: text_y_start,
            width: inner_width,
            height: inner_height,
        };
        // 1×1 RowBreak 표는 행을 다시 자를 수 없으므로, 부모가 넘긴 픽셀
        // viewport를 이 셀 자신의 유닛 경계로 되돌린다. 여기서 얻은 범위는
        // `layout_partial_table`의 start_cut/end_cut와 같은 의미다. 단순히 현재
        // 셀 하단에서 줄을 버리면 중첩 표/비인라인 컨트롤은 이후 쪽 소유를 잃고
        // SVG clip에만 가려진다 (#3637 HWP 2020 p25–p30).
        // 마지막 조각은 다음 페이지로 넘길 소유자가 없다. 이때 viewport cut을
        // 적용하면 한컴이 같은 쪽에 보존하는 꼬리 문단/중첩 표를 영구 유실한다.
        // 기존 line-fit 경로도 `split_terminal`에서 같은 예외를 두었다 (#3658).
        // Stage 42 diagnostic: make the same source-unit viewport available to
        // native HWP5 RowBreak fragments as to stored HWPX fragments.  The
        // selected overflow fixtures and issue2007 ownership tests determine
        // the final, narrower eligibility predicate.
        let fragment_cut_units = if single_row_fragment
            && row_filter.is_some()
            && (!split_terminal || force_source_start_cut)
        {
            let offset = single_row_fragment_content_offset
                .unwrap_or_else(|| single_row_continuation_offset.unwrap_or(0.0))
                .max(0.0);
            // 앞 조각의 콘텐츠는 `content_cell_y`가 이미 음수 방향으로 옮겨 놓고
            // 물리 Cell clip이 위쪽을 제거한다. 여기서 start까지 다시 버리면 현
            // 페이지 상단에 이어져야 할 줄이 사라진다. 따라서 현재 페이지 **하단**
            // 까지만 정확히 자르고, 앞부분은 같은 논리 원점에서 배치시킨다.
            let start = if force_source_start_cut {
                let units = self.cell_units_fitting_height(cell, table, styles, offset);
                if replay_terminal_boundary_unit {
                    // Native HWP5 short-parent child fragments can end the preceding
                    // viewport inside the final source unit.  That unit is physically
                    // clipped on the preceding page, so treating it as fully consumed
                    // loses its first visible line on the terminal continuation
                    // (76076 p81 -> p82).  Keep the outer fragment geometry intact and
                    // replay exactly that boundary unit on the next page.
                    units.saturating_sub(1)
                } else {
                    // terminal_rowbreak_source_cursor-only (76076 p33 -> p34) already
                    // owns every unit through `units`; replaying one back here would
                    // repaint its last source line again.
                    units
                }
            } else {
                0
            };
            let end = self
                .cell_units_fitting_height(cell, table, styles, offset + cell_h.max(0.0))
                .max(start);
            Some((start, end))
        } else {
            None
        };
        let fragment_line_ranges = fragment_cut_units
            .map(|(start, end)| self.cell_line_ranges_from_cut(cell, table, styles, start, end));
        // 셀 내 문단 + 컨트롤 통합 레이아웃
        let mut para_y = text_y_start;
        let mut has_preceding_text = false;
        for (cp_idx, (composed, para)) in composed_paras
            .iter()
            .zip(cell.paragraphs.iter())
            .enumerate()
        {
            let (start_line, end_line) = fragment_line_ranges
                .as_ref()
                .and_then(|ranges| ranges.get(cp_idx).copied())
                .unwrap_or((0, composed.lines.len()));
            let mixed_nested_split = fragment_cut_units.and_then(|(start, end)| {
                self.mixed_nested_split_from_cut(cell, table, styles, start, end, cp_idx)
            });
            let visible_non_inline_controls = fragment_cut_units.is_some_and(|(start, end)| {
                self.cell_cut_contains_non_inline_control_units(
                    cell, table, styles, start, end, cp_idx,
                )
            });
            // 빈 host 문단은 블록 중첩 표만 담을 수 있다. 그러므로 이 조각이
            // 실제 unit cut을 가진 경우에만 빈 범위를 건너뛴다. 일반 표까지
            // 건너뛰면 `근거설명`처럼 text_len=0인 host의 Table control 자체가
            // 방출되지 않는다 (76076 regulatory analysis p34).
            if fragment_cut_units.is_some()
                && start_line >= end_line
                && mixed_nested_split.is_none()
                && !visible_non_inline_controls
            {
                continue;
            }
            let cell_context = if let Some(ref ctx) = enclosing_cell_ctx {
                let mut new_ctx = ctx.clone();
                if let Some(last) = new_ctx.path.last_mut() {
                    last.cell_index = cell_idx;
                    last.cell_para_index = cp_idx;
                    last.text_direction = cell.text_direction;
                }
                Some(new_ctx)
            } else {
                table_meta.map(|(pi, ci)| CellContext {
                    parent_para_index: pi,
                    path: vec![CellPathEntry {
                        control_index: ci,
                        cell_index: cell_idx,
                        cell_para_index: cp_idx,
                        text_direction: cell.text_direction,
                    }],
                })
            };

            let has_table_ctrl = para.controls.iter().any(|c| matches!(c, Control::Table(_)));
            // [Task #573] inline TAC 표(treat_as_char=true) 와 block 표(treat_as_char=false)
            // 를 분리. 인라인 TAC 표가 있는 셀 paragraph 의 surrounding text (예: "ㄷ. ",
            // "이다.") 가 layout_composed_paragraph 호출 미진입으로 미렌더되던 결함 정정.
            // block 표는 별도 layout_table 호출로 배치되므로 텍스트 흐름 외부 — 기존
            // ELSE 분기 로직 유지. inline TAC 표는 layout_composed_paragraph 의 run_tacs
            // 에서 텍스트와 함께 배치되어야 함.
            let has_block_table_ctrl = para
                .controls
                .iter()
                .any(|c| matches!(c, Control::Table(t) if !t.common.treat_as_char));

            // HWP/HWPX가 셀 내부 문단의 LINE_SEG.vpos를 제공하는 경우에는
            // 누적 y 대신 그 절대 위치를 우선한다. 조직도형 표처럼 셀 하나에
            // 여러 짧은 문단이 있고 paraPr spacing/lineSpacing이 함께 지정된
            // 문서는 한컴이 각 문단 top을 vpos로 고정해 둔다. 누적 y만 쓰면
            // spacing_before가 중복되거나 음수 line_spacing이 누적되어 줄 위치가
            // 점점 어긋난다.
            //
            // 단, vpos == 0 은 "앵커 없음"의 센티널이기도 하다. 셀 안 문단이 전부
            // vpos == 0 으로 저장된 문서(중첩 표 안쪽 셀에서 흔하다)에서 이를 절대
            // 위치로 받아들이면 모든 문단이 셀 상단이라는 같은 y 로 리셋되어 서로
            // 겹쳐 그려진다. 첫 문단의 vpos == 0 은 "셀 상단"이라는 유효한 값이므로
            // 그대로 두고, 두 번째 이후 문단은 양수 vpos 가 저장돼 있을 때만 앵커로
            // 쓴다. (같은 파일의 text_y_start 계산도 `v > 0.0` 을 앵커 조건으로 쓴다)
            // 셀 안의 문단 또는 문단 내부 줄이 중간에 `vpos=0`으로 다시 시작한 뒤,
            // 그 다음 양수 vpos를 cell top 기준 절대 좌표로 해석하면 앞 문단 위로
            // 되감겨 겹친다. 이 reset은 RowBreak continuation에만 한정되지 않는다.
            // 예컨대 42065 p2의 일반 9×2 표 우측 셀과 p10--p16의 손자 1×1 셀은
            // 모두 같은 저장 형식이다. reset 뒤에는 저장 anchor 대신 누적 flow를
            // 쓴다. 첫 문단 첫 줄의 0은 정상적인 cell-top anchor이므로 제외한다.
            let local_vpos_restart_seen = cell
                .paragraphs
                .iter()
                .take(cp_idx.saturating_add(1))
                .enumerate()
                .any(|(prior_para_idx, prior)| {
                    prior.line_segs.iter().enumerate().any(|(line_idx, seg)| {
                        seg.vertical_pos == 0 && (prior_para_idx > 0 || line_idx > 0)
                    })
                });
            let has_stored_para_anchor =
                !local_vpos_restart_seen && crate::renderer::first_seg_vpos_is_anchor(para, cp_idx);
            let use_saved_cell_para_vpos = use_top_vpos_anchor
                || trust_stored_cell_flow
                || has_initial_tac_shape_host(&cell.paragraphs);
            if use_saved_cell_para_vpos
                && (!has_nested_table || trust_stored_cell_flow)
                && has_stored_para_anchor
            {
                if let Some(first_seg) = para.line_segs.first() {
                    if first_seg.vertical_pos >= 0 {
                        let spacing_before = styles
                            .para_styles
                            .get(para.para_shape_id as usize)
                            .map(|s| s.spacing_before)
                            .unwrap_or(0.0);
                        let anchored_y = cell_para_line_anchor_y(
                            text_y_start,
                            content_cell_y,
                            pad_top,
                            first_seg.vertical_pos,
                            self.dpi,
                            use_top_vpos_anchor,
                            upper_clip_line_reservation,
                        );
                        // layout_composed_paragraph()가 spacing_before를 더하므로
                        // 호출 전에 그 값을 빼서 최종 line top이 vpos와 일치하게 한다.
                        para_y = anchored_y - spacing_before;
                    }
                }
            }

            let para_y_before_compose = para_y;

            // 줄별 TAC 컨트롤 너비 합산: 각 TAC가 속한 줄을 판별하여 줄별 최대 너비 계산
            let tac_line_widths: Vec<f64> = {
                // 줄별 너비 합산 벡터
                let mut line_widths = vec![0.0f64; composed.lines.len().max(1)];
                for ctrl in &para.controls {
                    let (is_tac, w) = match ctrl {
                        Control::Picture(pic) if pic.common.treat_as_char => {
                            (true, hwpunit_to_px(pic.common.width as i32, self.dpi))
                        }
                        Control::Shape(shape) if shape.common().treat_as_char => {
                            (true, hwpunit_to_px(shape.common().width as i32, self.dpi))
                        }
                        Control::Equation(eq) => {
                            (true, hwpunit_to_px(eq.common.width as i32, self.dpi))
                        }
                        Control::Table(t) if t.common.treat_as_char => {
                            // [Issue #3396] 한글은 TAC 표의 문자 폭에 outMargin
                            // 좌/우를 포함한다 (정렬·전진 폭 공히).
                            (
                                true,
                                hwpunit_to_px(
                                    t.common.width as i32
                                        + t.outer_margin_left as i32
                                        + t.outer_margin_right as i32,
                                    self.dpi,
                                ),
                            )
                        }
                        _ => (false, 0.0),
                    };
                    if !is_tac {
                        continue;
                    }
                    // 줄이 1개이면 무조건 0번 줄
                    if composed.lines.len() <= 1 {
                        line_widths[0] += w;
                    } else {
                        // 아직 줄 분배 전이므로 순서대로 채워넣기:
                        // 현재 줄 너비 + 이 컨트롤 너비 > 셀 너비이면 다음 줄로
                        let mut placed = false;
                        for lw in line_widths.iter_mut() {
                            if *lw == 0.0 || *lw + w <= inner_width + 0.5 {
                                *lw += w;
                                placed = true;
                                break;
                            }
                        }
                        if !placed {
                            if let Some(last) = line_widths.last_mut() {
                                *last += w;
                            }
                        }
                    }
                }
                line_widths
            };
            let total_inline_width: f64 = tac_line_widths.iter().cloned().fold(0.0f64, f64::max);

            if !has_block_table_ctrl {
                let is_last_para = cp_idx + 1 == composed_paras.len();
                let numbered_comp = if start_line == 0 && end_line > start_line {
                    self.apply_paragraph_numbering(
                        Some(composed),
                        para,
                        styles,
                        outline_numbering_id,
                    )
                } else {
                    None
                };
                let composed_for_layout = numbered_comp.as_ref().unwrap_or(composed);
                para_y = self.layout_composed_paragraph(
                    tree,
                    cell_node,
                    composed_for_layout,
                    styles,
                    &inner_area,
                    para_y,
                    start_line,
                    end_line,
                    section_index,
                    cp_idx,
                    cell_context.clone(),
                    !use_top_vpos_anchor,
                    is_last_para,
                    0.0,
                    None,
                    Some(para),
                    Some(bin_data_content),
                    None, // 셀 컨텍스트 — wrap zone 무관
                );

                let has_visible_text = composed
                    .lines
                    .iter()
                    .any(|line| line.runs.iter().any(|run| !run.text.trim().is_empty()));
                if has_visible_text {
                    has_preceding_text = true;
                }
            } else {
                // has_table_ctrl: 표가 포함된 문단
                // LINE_SEG vpos가 문단 위치를 정확히 지정하므로,
                // 추가 spacing 없이 para_y를 그대로 사용.
                // (leading spacing은 LINE_SEG vpos에 이미 반영되어 있음)
            }

            let para_alignment = styles
                .para_styles
                .get(para.para_shape_id as usize)
                .map(|s| s.alignment)
                .unwrap_or(Alignment::Left);
            // [Task #548] paragraph margin_left + first-line indent 를 inline shape
            // 위치에 반영. paragraph_layout 텍스트 경로와 동일한 effective_margin_left
            // 산식을 적용해 텍스트와 shape 위치 일관성 보장.
            let para_margin_left_px = styles
                .para_styles
                .get(para.para_shape_id as usize)
                .map(|s| s.margin_left)
                .unwrap_or(0.0);
            let para_indent_px = styles
                .para_styles
                .get(para.para_shape_id as usize)
                .map(|s| s.indent)
                .unwrap_or(0.0);

            let mut prev_tac_text_pos: usize = 0;
            // LINE_SEG 기반 줄별 TAC 이미지 배치를 위한 상태
            // 빈 문단(runs 없음)에서 TAC 컨트롤을 LINE_SEG에 순서대로 매핑
            let all_runs_empty = composed.lines.iter().all(|l| l.runs.is_empty());
            let mut tac_seq_index: usize = 0; // TAC 컨트롤 순번 (빈 문단용)
            let mut current_tac_line: usize = 0;
            let mut inline_x = {
                let line_w = tac_line_widths
                    .first()
                    .copied()
                    .unwrap_or(total_inline_width);
                let line_margin =
                    effective_margin_left_line(para_margin_left_px, para_indent_px, 0);
                match para_alignment {
                    Alignment::Center | Alignment::Distribute => {
                        inner_area.x + (inner_area.width - line_w).max(0.0) / 2.0
                    }
                    Alignment::Right => inner_area.x + (inner_area.width - line_w).max(0.0),
                    _ => inner_area.x + line_margin,
                }
            };
            let mut tac_img_y = para_y_before_compose;
            let mut rendered_top_and_bottom_non_inline = false;

            for (ctrl_idx, ctrl) in para.controls.iter().enumerate() {
                match ctrl {
                    Control::Picture(pic) => {
                        let visible_non_inline_control =
                            fragment_cut_units.map_or(true, |(su, eu)| {
                                self.cell_cut_starts_non_inline_control(
                                    cell, table, styles, su, eu, cp_idx, ctrl_idx,
                                )
                            });
                        let fragment_owned_square_flow = self.profile.get().native_hwp5_layout()
                            && fragment_cut_units.is_some()
                            && visible_non_inline_control
                            && pic.common.flow_with_text
                            && matches!(pic.common.text_wrap, TextWrap::Square);
                        if !pic.common.treat_as_char
                            && fragment_cut_units.is_some()
                            && !visible_non_inline_control
                        {
                            continue;
                        }
                        if pic.common.treat_as_char {
                            let pic_w = hwpunit_to_px(pic.common.width as i32, self.dpi);
                            // [Task #928] paragraph_layout 이 inline picture 를 emit 한
                            // 경우 set_inline_shape_position 을 호출하므로 (paragraph_layout.rs
                            // 라인 2019-2022), 본 가드는 inline_shape_position 등록 여부로
                            // 판정한다. 기존 tac_controls + line_chars 기반 가드는 boundary
                            // 케이스 (abs_pos == line_chars) 를 빠뜨려 exam_kor 5p ㉢
                            // 그림 중복 emit 회귀가 있었다.
                            let will_render_inline = tree
                                .get_inline_shape_position(
                                    section_index,
                                    cp_idx,
                                    ctrl_idx,
                                    cell_context.as_ref(),
                                )
                                .is_some();
                            if !will_render_inline {
                                // LINE_SEG 기반 줄 판별
                                let target_line = if all_runs_empty && para.line_segs.len() > 1 {
                                    // 빈 문단: TAC 순번으로 LINE_SEG에 1:1 매핑
                                    let li = tac_seq_index.min(para.line_segs.len() - 1);
                                    tac_seq_index += 1;
                                    li
                                } else {
                                    // 텍스트 있는 문단: char position으로 줄 판별
                                    composed
                                        .tac_controls
                                        .iter()
                                        .find(|&&(_, _, ci)| ci == ctrl_idx)
                                        .map(|&(abs_pos, _, _)| {
                                            composed
                                                .lines
                                                .iter()
                                                .enumerate()
                                                .rev()
                                                .find(|(_, line)| abs_pos >= line.char_start)
                                                .map(|(li, _)| li)
                                                .unwrap_or(0)
                                        })
                                        .unwrap_or(0)
                                };

                                if target_line > current_tac_line {
                                    // 줄이 바뀜: inline_x 리셋, y를 LINE_SEG vpos 기준으로 이동
                                    current_tac_line = target_line;
                                    let line_w =
                                        tac_line_widths.get(target_line).copied().unwrap_or(0.0);
                                    // [Task #548] target_line 의 effective_margin_left 적용
                                    let line_margin = effective_margin_left_line(
                                        para_margin_left_px,
                                        para_indent_px,
                                        target_line,
                                    );
                                    inline_x = match para_alignment {
                                        Alignment::Center | Alignment::Distribute => {
                                            inner_area.x
                                                + (inner_area.width - line_w).max(0.0) / 2.0
                                        }
                                        Alignment::Right => {
                                            inner_area.x + (inner_area.width - line_w).max(0.0)
                                        }
                                        _ => inner_area.x + line_margin,
                                    };
                                    if let Some(seg) = para.line_segs.get(target_line) {
                                        // [Task #520 / #624 복원] LineSeg.vertical_pos 는 셀 origin 기준 절대값.
                                        // para_y_before_compose 에 이미 ls[0].vpos 가 누적되어 있어
                                        // 상대 오프셋(seg.vpos - ls[0].vpos)만 더해야 이중 합산을 피한다.
                                        let first_vpos = para
                                            .line_segs
                                            .first()
                                            .map(|f| f.vertical_pos)
                                            .unwrap_or(0);
                                        tac_img_y = para_y_before_compose
                                            + hwpunit_to_px(
                                                seg.vertical_pos - first_vpos,
                                                self.dpi,
                                            );
                                    }
                                }

                                let pic_h = hwpunit_to_px(pic.common.height as i32, self.dpi);
                                // [Task #477] 셀 폭 초과 시 비율 유지 클램프
                                let clamped_w = pic_w.min(inner_area.width);
                                let clamped_h = if pic_w > 0.0 {
                                    pic_h * (clamped_w / pic_w)
                                } else {
                                    pic_h
                                };
                                let pic_area = LayoutRect {
                                    x: inline_x,
                                    y: tac_img_y,
                                    width: clamped_w,
                                    height: clamped_h,
                                };
                                // [Task #1151 v4] 셀 안 inline picture (tac=true):
                                // outer paragraph idx + inner picture ctrl idx +
                                // cell_ctx 전달 → ImageNode cell_index + cursor_rect
                                // hit-test 정합.
                                self.layout_picture(
                                    tree,
                                    cell_node,
                                    pic,
                                    &pic_area,
                                    bin_data_content,
                                    Alignment::Left,
                                    Some(section_index),
                                    cell_context.as_ref().map(|c| c.parent_para_index),
                                    Some(ctrl_idx),
                                    cell_context.as_ref(),
                                );
                                inline_x += clamped_w;
                                continue;
                            }
                            inline_x += pic_w;
                        } else {
                            // 비-인라인(자리차지/글뒤로/글앞으로) 이미지:
                            // 본문배치 속성(가로/세로 기준, 정렬, 오프셋) 적용
                            let pic_w = hwpunit_to_px(pic.common.width as i32, self.dpi);
                            let pic_h = hwpunit_to_px(pic.common.height as i32, self.dpi);
                            // vert_rel_to=Para 인 셀 내부 비인라인 이미지의 앵커 기준점.
                            // `para_y` 는 `layout_composed_paragraph` 가 advance 시킨 뒤의
                            // 값이라 한 줄 아래를 가리킨다 — 그대로 쓰면 그림이 줄 높이만큼
                            // 내려가 셀 경계에 잘린다.
                            //
                            // 이 자리는 wrap 종류를 하나씩 열거하며 고쳐 왔다 —
                            // [Task #577] TopAndBottom(exam_science 2번 보기 ⑤ 등 5개가
                            // line_height 약 15.32px 만큼 밀려 잘림), [Task #2207] 글뒤로·
                            // 글앞으로(오버레이는 텍스트 플로우를 밀지 않아 같은 원리).
                            // [#4059] 그 열거에 Square·Tight·Through 가 빠져 있었다 — 관세청
                            // 보도자료 1쪽 "한국판뉴딜" 로고가 줄 높이(17.3px)만큼 밀려 잘렸다.
                            //
                            // 다만 **두 무리는 기준점이 다르다.** wrap 무관하게 #577 공식
                            // (`content_cell_y + pad_top + seg.vpos`)으로 통일해 보았더니
                            // `pic-in-table-with-toggle` 이 한글 대비 +8.6px 에서 −43.8px 로
                            // 더 어긋났다. 그 셀은 valign=Center 라 문단이 셀 상단이 아니라
                            // 가운데에 놓이는데, 저 공식은 셀 콘텐츠 상단을 가리키기 때문이다.
                            // Square 계열은 **실제 문단 top**(`para_y_before_compose`)이 맞다.
                            //
                            // 한글 PDF 오라클 실측 (그림 top, px):
                            //   문서                        한글     종전      정정 후
                            //   관세청 한국판뉴딜           191.9   208.3    191.0
                            //   pic-in-table-with-toggle    249.5   258.1    244.8
                            //   hwpx_sample2 p19            970.2   978.4    965.1
                            // 잔여 약 5px 는 별개 축이다 — toggle 은 x 도 같은 크기로 어긋난다
                            // (한글 170.1 vs 166.4). 앵커 원점(셀 padding 해석) 쪽으로 보인다.
                            //
                            // 이 분기는 이미 `treat_as_char == false` 안이므로 wrap 조건 없이
                            // vert_rel_to 만 본다.
                            let non_inline_para = matches!(
                                pic.common.vert_rel_to,
                                crate::model::shape::VertRelTo::Para
                            );
                            // #2071 셀 valign 강제 + 앵커 분기 판정용. 그쪽은 한글 2024
                            // 오라클로 TopAndBottom 한정 검증된 **별개 계약**이라 위 앵커
                            // 정정과 함께 넓히지 않는다.
                            let top_and_bottom_para = non_inline_para
                                && matches!(
                                    pic.common.text_wrap,
                                    crate::model::shape::TextWrap::TopAndBottom
                                );
                            let overlay_para = non_inline_para
                                && matches!(
                                    pic.common.text_wrap,
                                    crate::model::shape::TextWrap::BehindText
                                        | crate::model::shape::TextWrap::InFrontOfText
                                );
                            // [Task #2226] 텍스트 없는 문단에서 seg.vpos > 0 이면 그
                            // 줄은 flow 그림에 밀려난 위치다 — 그림 오프셋의 원점은
                            // 문단 시작이므로 앵커에 vpos 를 더하면 그림이 셀 아래로
                            // 이탈한다 (주보 p2 로고 표 붓글씨 셀: line vpos 51.3px).
                            let displaced_empty_line_para = para.text.trim().is_empty()
                                && para
                                    .line_segs
                                    .first()
                                    .is_some_and(|seg| seg.vertical_pos > 0);
                            let anchor_y = if displaced_empty_line_para {
                                // Square 포함 모든 비인라인 그림 — 원점은 문단 시작.
                                content_cell_y + pad_top
                            } else if non_inline_para && !top_and_bottom_para && !overlay_para {
                                // Square·Tight·Through — 흐름을 미는 wrap. 기준점은 셀
                                // 콘텐츠 상단이 아니라 **실제 문단 top** 이다(valign 반영).
                                para_y_before_compose
                            } else if non_inline_para {
                                para.line_segs
                                    .first()
                                    .filter(|seg| seg.vertical_pos >= 0)
                                    .map(|seg| {
                                        content_cell_y
                                            + pad_top
                                            + hwpunit_to_px(seg.vertical_pos, self.dpi)
                                    })
                                    .unwrap_or(para_y_before_compose)
                            } else {
                                para_y
                            };
                            let unrestricted_take_place_cell_float = !pic.common.flow_with_text
                                && matches!(pic.common.text_wrap, TextWrap::TopAndBottom)
                                && matches!(pic.common.vert_rel_to, VertRelTo::Para);
                            let reset_relocated_stored_picture_offset =
                                stored_layout_relocated_empty_rowbreak_picture_resets_offset(
                                    self.profile.get().native_hwp5_layout()
                                        || self.profile.get().hwpx_stored_layout(),
                                    self.profile.get().native_hwp5_layout(),
                                    outer_host_stored_vpos_hu,
                                    table,
                                    cell,
                                    para,
                                    pic,
                                );
                            let detached_from_inline_table_flow = inline_table_flow_y_shift > 0.0
                                && unrestricted_take_place_cell_float;
                            let picture_anchor_y = if detached_from_inline_table_flow {
                                anchor_y - inline_table_flow_y_shift - row_y[r].max(0.0)
                            } else if unrestricted_take_place_cell_float {
                                // 한컴의 셀 내부 자리차지 그림은 제한이 꺼지면
                                // offset 지점에 그림 하단이 걸리도록 위로 빠진다.
                                // compute_object_position 이 아래에서 vOffset 을
                                // 다시 더하므로 여기서는 미리 vOffset+높이를 뺀다.
                                anchor_y
                                    - pic_h
                                    - hwpunit_to_px(pic.common.vertical_offset as i32, self.dpi)
                            } else {
                                anchor_y
                            };
                            let cell_area = LayoutRect {
                                y: picture_anchor_y,
                                height: (inner_area.height - (picture_anchor_y - inner_area.y))
                                    .max(0.0),
                                ..inner_area
                            };
                            let (pic_x, pic_y) = self.compute_object_position(
                                &pic.common,
                                pic_w,
                                pic_h,
                                &cell_area,
                                &inner_area,
                                &inner_area,
                                &inner_area,
                                picture_anchor_y,
                                para_alignment,
                            );
                            // [Issue #2071] 셀 앵커 floating 그림(restrict-ON,
                            // TopAndBottom+Para)은 한컴이 **셀 vertical_align 으로만**
                            // 배치하고 그림 자체 pos vert_align 은 무시한다. 위
                            // compute_object_position 은 그림 pos vert_align 을 따르므로
                            // pic≠Top 이거나 셀 valign≠Top 이면 어긋난다.
                            // 한글 2024 편집기 오라클(ta-pic pos/cell vertAlign 변형 실측):
                            //   셀=Center × pic=Top/Center/Bottom → 모두 362.5(셀 중앙)
                            //   셀=Top × pic=Center → 153.8(셀 상단)  [pic 무시 확인]
                            // 콘텐츠 box·그림 높이 기준으로 셀 valign 위치를 강제:
                            //   TOP    = content_top + vOffset
                            //   CENTER = content_top + (content_h − pic_h + vOffset)/2
                            //   BOTTOM = content_bottom − pic_h − vOffset
                            let pic_y = if fragment_owned_square_flow {
                                // partial-table와 같은 source-owner 계약: 현재 cut이
                                // 소유한 Square flow picture는 page-local flow anchor를
                                // 쓴다. 이전 source ladder의 negative vOffset을 다시
                                // 적용하면 같은 paragraph의 후속 control이 fragment 위로
                                // 빠진다.
                                picture_anchor_y
                            } else if reset_relocated_stored_picture_offset {
                                // 이 형상은 cell의 Center 값이 현 물리 페이지의 정렬 계약이
                                // 아니라 stale 음수 offset과 짝을 이룬 이전 페이지 ladder다.
                                // page-local content top이 한컴 PDF의 그림 상단이다.
                                content_cell_y + pad_top
                            } else if top_and_bottom_para
                                && pic.common.flow_with_text
                                && !unrestricted_take_place_cell_float
                                && !detached_from_inline_table_flow
                            {
                                let v_off = hwpunit_to_px(
                                    signed_hwpunit(pic.common.vertical_offset),
                                    self.dpi,
                                );
                                // [#3738 Stage 8] Bottom caption은 그림과 하나의
                                // 시각 블록이다. 이를 빼고 Center/Bottom을 계산하면
                                // 그림 본체만 셀 중앙에 놓이고, caption이 셀 밖으로
                                // 넘쳐 뒤 본문과 겹친다. Top caption은 그림 위쪽
                                // 좌표 계약이 달라 이 보정 대상이 아니다.
                                let bottom_caption_h =
                                    pic.caption.as_ref().map_or(0.0, |caption| {
                                        if matches!(
                                            caption.direction,
                                            crate::model::shape::CaptionDirection::Bottom
                                        ) {
                                            self.calculate_caption_height(&pic.caption, styles)
                                                + hwpunit_to_px(caption.spacing as i32, self.dpi)
                                        } else {
                                            0.0
                                        }
                                    });
                                let aligned_visual_h = pic_h + bottom_caption_h;
                                let content_top = content_cell_y + pad_top;
                                match effective_valign {
                                    VerticalAlign::Top => content_top + v_off,
                                    VerticalAlign::Center => {
                                        content_top
                                            + (inner_height - aligned_visual_h + v_off) / 2.0
                                    }
                                    VerticalAlign::Bottom => {
                                        content_top + inner_height - aligned_visual_h - v_off
                                    }
                                }
                            } else {
                                pic_y
                            };
                            let pic_area = LayoutRect {
                                x: pic_x,
                                y: pic_y,
                                width: pic_w,
                                height: pic_h,
                            };
                            let mut pic_for_layout = pic.clone();
                            pic_for_layout.common.horizontal_offset = 0;
                            pic_for_layout.common.vertical_offset = 0;
                            pic_for_layout.common.horz_align = crate::model::shape::HorzAlign::Left;
                            pic_for_layout.common.vert_align = crate::model::shape::VertAlign::Top;
                            // [Task #1151 v4] 셀 안 non-inline picture (tac=false 자리차지 등):
                            // outer paragraph idx + inner picture ctrl idx +
                            // cell_ctx 전달.
                            let picture_parent = if detached_from_inline_table_flow
                                || unrestricted_take_place_cell_float
                            {
                                &mut *table_node
                            } else {
                                &mut *cell_node
                            };
                            self.layout_picture(
                                tree,
                                picture_parent,
                                &pic_for_layout,
                                &pic_area,
                                bin_data_content,
                                Alignment::Left,
                                Some(section_index),
                                cell_context.as_ref().map(|c| c.parent_para_index),
                                Some(ctrl_idx),
                                cell_context.as_ref(),
                            );
                            // 셀 안 부동 그림도 본문/각주 그림과 마찬가지로 자체 caption을
                            // 방출해야 한다. 이 경로가 빠져 있으면 HWP5 LIST_HEADER의
                            // 그림 caption은 파싱돼도 화면에는 사라지고 후속 flow의 기준도
                            // 달라진다. 현재는 image frame 아래에 놓이는 Bottom caption만
                            // 이 경로의 cell-local placement와 같은 좌표계로 렌더한다.
                            if let Some(caption) = pic.caption.as_ref().filter(|caption| {
                                matches!(caption.direction, CaptionDirection::Bottom)
                                    && !caption.paragraphs.is_empty()
                            }) {
                                let caption_spacing =
                                    hwpunit_to_px(caption.spacing as i32, self.dpi);
                                self.layout_caption(
                                    tree,
                                    picture_parent,
                                    caption,
                                    styles,
                                    &inner_area,
                                    pic_x,
                                    pic_w,
                                    pic_y + pic_h + caption_spacing,
                                    &mut self.auto_counter.borrow_mut(),
                                    bin_data_content,
                                    cell_context.clone(),
                                );
                            }
                            if matches!(pic.common.text_wrap, TextWrap::TopAndBottom) {
                                rendered_top_and_bottom_non_inline = true;
                            } else if fragment_owned_square_flow {
                                para_y += self.cell_non_inline_control_flow_height(&pic.common);
                            } else {
                                para_y += self.non_inline_control_flow_height(&pic.common);
                            }
                        }
                        has_preceding_text = true;
                    }
                    Control::Shape(shape) => {
                        // Shape/TextBox는 control entry 뒤의 physical fragment에서도
                        // 잔여 내부 문단을 계속 렌더한다. entry-only 판정은 원자적으로
                        // 소유하는 Picture에만 적용한다.
                        if !shape.common().treat_as_char
                            && fragment_cut_units.is_some()
                            && !visible_non_inline_controls
                        {
                            continue;
                        }
                        if shape.common().treat_as_char {
                            let shape_w = hwpunit_to_px(shape.common().width as i32, self.dpi);
                            // [Task #928] paragraph_layout 의 run_tacs 처리 (라인 2026-2034)
                            // 가 inline Shape 위치를 set_inline_shape_position 으로 등록
                            // 하므로, 본 가드는 등록 여부로 판정한다. Picture 분기와 동일
                            // 패턴이며 boundary 케이스에 안전.
                            let will_render_inline = tree
                                .get_inline_shape_position(
                                    section_index,
                                    cp_idx,
                                    ctrl_idx,
                                    cell_context.as_ref(),
                                )
                                .is_some();
                            // [Task #500] Picture 분기와 정합: target_line 산출 + 줄 변경 시
                            // inline_x/tac_img_y 리셋. multi-line paragraph 에서 사각형이
                            // ls[1]+ 에 있을 때 paragraph 첫 줄 좌표가 잘못 사용되던 결함 정정.
                            let target_line = if all_runs_empty && para.line_segs.len() > 1 {
                                let li = tac_seq_index.min(para.line_segs.len() - 1);
                                tac_seq_index += 1;
                                li
                            } else {
                                composed
                                    .tac_controls
                                    .iter()
                                    .find(|&&(_, _, ci)| ci == ctrl_idx)
                                    .map(|&(abs_pos, _, _)| {
                                        composed
                                            .lines
                                            .iter()
                                            .enumerate()
                                            .rev()
                                            .find(|(_, line)| abs_pos >= line.char_start)
                                            .map(|(li, _)| li)
                                            .unwrap_or(0)
                                    })
                                    .unwrap_or(0)
                            };
                            if target_line > current_tac_line {
                                current_tac_line = target_line;
                                let line_w =
                                    tac_line_widths.get(target_line).copied().unwrap_or(0.0);
                                // [Task #548] target_line 의 effective_margin_left 적용
                                let line_margin = effective_margin_left_line(
                                    para_margin_left_px,
                                    para_indent_px,
                                    target_line,
                                );
                                inline_x = match para_alignment {
                                    Alignment::Center | Alignment::Distribute => {
                                        inner_area.x + (inner_area.width - line_w).max(0.0) / 2.0
                                    }
                                    Alignment::Right => {
                                        inner_area.x + (inner_area.width - line_w).max(0.0)
                                    }
                                    _ => inner_area.x + line_margin,
                                };
                                if let Some(seg) = para.line_segs.get(target_line) {
                                    // [Task #520] LineSeg.vertical_pos 는 셀 origin 기준 절대값.
                                    // para_y_before_compose 에 이미 ls[0].vpos 가 누적되어 있어
                                    // 상대 오프셋만 더해야 한다 (Picture 분기와 동일).
                                    let first_vpos =
                                        para.line_segs.first().map(|f| f.vertical_pos).unwrap_or(0);
                                    tac_img_y = para_y_before_compose
                                        + hwpunit_to_px(seg.vertical_pos - first_vpos, self.dpi);
                                }
                            }
                            if !will_render_inline {
                                // Shape 앞의 텍스트 너비 계산: tac_controls에서 이 Shape의 text_pos와
                                // 이전 Shape의 text_pos 차이에 해당하는 텍스트 너비를 inline_x에 반영
                                if let Some(&(tac_pos, _, _)) = composed
                                    .tac_controls
                                    .iter()
                                    .find(|&&(_, _, ci)| ci == ctrl_idx)
                                {
                                    // [Task #495] 가드: 사각형이 paragraph 첫 줄(ls[0]) 범위 안에 있을 때만
                                    // text_before 추출/발행. multi-line paragraph 에서 사각형이 ls[1]+ 에
                                    // 있는 경우 composed.lines.first() 만 보던 기존 코드는 첫 줄 전체
                                    // 텍스트를 잘못 추출해 paragraph_layout 결과와 중복 발행했음.
                                    let in_first_line = composed
                                        .lines
                                        .first()
                                        .map(|line| {
                                            let line_chars: usize = line
                                                .runs
                                                .iter()
                                                .map(|r| r.text.chars().count())
                                                .sum();
                                            tac_pos >= line.char_start
                                                && tac_pos < line.char_start + line_chars
                                        })
                                        .unwrap_or(false);
                                    // 이 Shape 앞에 아직 inline_x에 반영되지 않은 텍스트가 있는지 계산
                                    let text_before: String = if in_first_line {
                                        composed
                                            .lines
                                            .first()
                                            .map(|line| {
                                                let mut chars_so_far = 0usize;
                                                let mut result = String::new();
                                                for run in &line.runs {
                                                    for ch in run.text.chars() {
                                                        if chars_so_far >= prev_tac_text_pos
                                                            && chars_so_far < tac_pos
                                                        {
                                                            result.push(ch);
                                                        }
                                                        chars_so_far += 1;
                                                    }
                                                }
                                                result
                                            })
                                            .unwrap_or_default()
                                    } else {
                                        String::new()
                                    };
                                    if !text_before.is_empty() {
                                        let char_style_id = composed
                                            .lines
                                            .first()
                                            .and_then(|l| l.runs.first())
                                            .map(|r| r.char_style_id)
                                            .unwrap_or(0);
                                        let lang_index = composed
                                            .lines
                                            .first()
                                            .and_then(|l| l.runs.first())
                                            .map(|r| r.lang_index)
                                            .unwrap_or(0);
                                        let ts = resolved_to_text_style(
                                            styles,
                                            char_style_id,
                                            lang_index,
                                        );
                                        // [Task #555] PUA 옛한글 char 은 자모 시퀀스로 변환 후 폭 측정.
                                        let text_before_metrics: String = {
                                            use super::super::super::pua_oldhangul::map_pua_old_hangul;
                                            text_before
                                                .chars()
                                                .flat_map(|ch| {
                                                    if let Some(jamos) = map_pua_old_hangul(ch) {
                                                        jamos.iter().copied().collect::<Vec<_>>()
                                                    } else {
                                                        vec![ch]
                                                    }
                                                })
                                                .collect()
                                        };
                                        let text_w = estimate_text_width(&text_before_metrics, &ts);
                                        let text_font_size = ts.font_size;
                                        // 텍스트 렌더링: Shape 사이에 배치
                                        // 텍스트 y를 Shape 하단 baseline에 맞춤
                                        // (Shape 높이 - 폰트 줄 높이)만큼 아래로 이동
                                        let text_baseline = text_font_size * 0.85;
                                        let font_line_h = text_font_size * 1.2;
                                        // 인접 Shape의 높이를 사용하여 텍스트 y를 baseline 정렬
                                        let adjacent_shape_h = para
                                            .controls
                                            .iter()
                                            .find_map(|c| {
                                                if let Control::Shape(s) = c {
                                                    if s.common().treat_as_char {
                                                        Some(hwpunit_to_px(
                                                            s.common().height as i32,
                                                            self.dpi,
                                                        ))
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                }
                                            })
                                            .unwrap_or(0.0);
                                        let text_y = para_y_before_compose
                                            + (adjacent_shape_h - font_line_h).max(0.0);
                                        let text_node_id = tree.next_id();
                                        let text_node = RenderNode::new(
                                            text_node_id,
                                            RenderNodeType::TextRun(TextRunNode {
                                                text: text_before,
                                                style: ts,
                                                char_shape_id: Some(char_style_id),
                                                para_shape_id: Some(composed.para_style_id),
                                                section_index: Some(section_index),
                                                para_index: None,
                                                char_start: None,
                                                cell_context: None,
                                                is_para_end: false,
                                                is_line_break_end: false,
                                                rotation: 0.0,
                                                is_vertical: false,
                                                char_overlap: None,
                                                border_fill_id: 0,
                                                baseline: text_baseline,
                                                field_marker: FieldMarkerType::None,
                                                display_text: None,
                                            }),
                                            BoundingBox::new(inline_x, text_y, text_w, font_line_h),
                                        );
                                        cell_node.children.push(text_node);
                                        inline_x += text_w;
                                    }
                                    prev_tac_text_pos = tac_pos;
                                }
                            }
                            // [Task #520 / #624 복원] target_line 기반 tac_img_y 사용 (Picture 분기와 동일).
                            // para_y_before_compose 사용 시 multi-line paragraph 의 ls[1]+ inline TAC Shape 가
                            // 항상 line 0 좌표에 떨어져 본문 텍스트와 겹친다 (exam_science p2 7번 글상자 ㉠).
                            // [Task #928] will_render_inline=true 인 경우 paragraph_layout 이
                            // 등록한 inline_shape_position 좌표를 사용해 도형 위치를
                            // run_tacs split 에서 reserve 한 gap 과 정확히 정합시킨다.
                            let (shape_x, shape_y) = if will_render_inline {
                                tree.get_inline_shape_position(
                                    section_index,
                                    cp_idx,
                                    ctrl_idx,
                                    cell_context.as_ref(),
                                )
                                .unwrap_or((inline_x, tac_img_y))
                            } else {
                                (inline_x, tac_img_y)
                            };
                            let shape_area = LayoutRect {
                                x: shape_x,
                                y: shape_y,
                                width: shape_w,
                                height: inner_area.height,
                            };
                            // [Task #1138] 셀 컨텍스트 (section, outer_para, outer_table_ctrl, cell, cell_para, inner_ctrl)
                            let table_cell_ctx = table_meta.map(|(opi, otci)| {
                                (section_index, opi, otci, cell_idx, cp_idx, ctrl_idx)
                            });
                            self.layout_cell_shape(
                                tree,
                                cell_node,
                                shape,
                                &shape_area,
                                shape_y,
                                Alignment::Left,
                                styles,
                                bin_data_content,
                                clamp_header_negative_para_offset,
                                table_cell_ctx,
                            );
                            inline_x += shape_w;
                        } else {
                            let shape_anchor_y = if matches!(
                                shape.common().vert_rel_to,
                                crate::model::shape::VertRelTo::Para
                            ) {
                                para_y_before_compose
                            } else {
                                para_y
                            };
                            // [Task #1138] 셀 컨텍스트
                            let table_cell_ctx = table_meta.map(|(opi, otci)| {
                                (section_index, opi, otci, cell_idx, cp_idx, ctrl_idx)
                            });
                            self.layout_cell_shape(
                                tree,
                                cell_node,
                                shape,
                                &inner_area,
                                shape_anchor_y,
                                para_alignment,
                                styles,
                                bin_data_content,
                                clamp_header_negative_para_offset,
                                table_cell_ctx,
                            );
                            if matches!(shape.common().text_wrap, TextWrap::TopAndBottom) {
                                rendered_top_and_bottom_non_inline = true;
                            }
                        }
                    }
                    Control::Equation(eq) => {
                        // 수식 컨트롤: 글자처럼 인라인 배치
                        let eq_w = hwpunit_to_px(eq.common.width as i32, self.dpi);

                        // 수식이 텍스트 run 사이에 인라인으로 배치되는 경우
                        // layout_composed_paragraph에서 이미 렌더링됨 → 건너뛰기
                        let has_text_in_para =
                            para.text.chars().any(|c| c > '\u{001F}' && c != '\u{FFFC}');
                        // 빈 runs 셀 + TAC 수식: paragraph_layout(Task #287 경로)이 이미
                        // 렌더 후 set_inline_shape_position 호출. 중복 emit 방지(Issue #301).
                        let already_rendered_inline = tree
                            .get_inline_shape_position(
                                section_index,
                                cp_idx,
                                ctrl_idx,
                                cell_context.as_ref(),
                            )
                            .is_some();
                        if has_text_in_para || already_rendered_inline {
                            // paragraph_layout 경로에서 이미 렌더됨
                            inline_x += eq_w;
                        } else {
                            // 수식만 있는 문단: 여기서 직접 렌더링
                            let eq_h = hwpunit_to_px(eq.common.height as i32, self.dpi);
                            let eq_x = {
                                let x = inline_x;
                                inline_x += eq_w;
                                x
                            };
                            let eq_y = para_y_before_compose;

                            let tokens = super::super::super::equation::tokenizer::tokenize(&eq.script);
                            let ast = super::super::super::equation::parser::EqParser::new(tokens).parse();
                            let font_size_px = hwpunit_to_px(eq.font_size as i32, self.dpi);
                            let layout_box =
                                super::super::super::equation::layout::EqLayout::new(font_size_px)
                                    .layout(&ast);
                            let color_str =
                                super::super::super::equation::svg_render::eq_color_to_svg(eq.color);
                            let svg_content =
                                super::super::super::equation::svg_render::render_equation_svg(
                                    &layout_box,
                                    &color_str,
                                    font_size_px,
                                );

                            let eq_node = RenderNode::new(
                                tree.next_id(),
                                RenderNodeType::Equation(EquationNode {
                                    svg_content,
                                    layout_box,
                                    color_str,
                                    color: eq.color,
                                    script: eq.script.clone(),
                                    font_size: font_size_px,
                                    section_index: Some(section_index),
                                    para_index: table_meta.map(|(pi, _)| pi),
                                    control_index: Some(ctrl_idx),
                                    cell_index: Some(cell_idx),
                                    cell_para_index: Some(cp_idx),
                                    note_ref: None,
                                }),
                                BoundingBox::new(eq_x, eq_y, eq_w, eq_h),
                            );
                            cell_node.children.push(eq_node);
                        }
                    }
                    Control::Table(nested_table) => {
                        let is_tac_table = nested_table.common.treat_as_char;
                        // HWPX의 같은 빈 host 문단 안에 있는 `글 뒤로` 1×1 표는
                        // 문단 흐름을 차지하지 않는 overlay control이다. 특히 자동날인
                        // 안내처럼 세 control이 같은 `vpos`에 있고 horzOffset만 다른
                        // 경우, 일반 nested-table 경로처럼 table_h만큼 para_y를 전진하면
                        // PDF의 가로 3개 상자가 세로로 쌓인다 (#3820 p144).
                        //
                        // HWP5의 legacy non-TAC 정렬이나 HWPX TopAndBottom 표까지
                        // horizontal offset을 강제하면 기존 셀 레이아웃을 바꾼다. stored
                        // HWPX의 paragraph-relative BehindText + Column anchor에만
                        // 한정해 parent cell x를 explicit anchor로 넘긴다. 기존
                        // compute_table_x_position은 이 override에 non-TAC horzOffset을
                        // 더하므로 offset의 부호/단위 규칙은 한 곳에 유지된다.
                        let hwpx_nested_behind_text_overlay =
                            self.profile.get().hwpx_stored_layout()
                                && !is_tac_table
                                && matches!(nested_table.common.text_wrap, TextWrap::BehindText)
                                && nested_table.common.flow_with_text
                                && matches!(nested_table.common.vert_rel_to, VertRelTo::Para)
                                && matches!(nested_table.common.horz_rel_to, HorzRelTo::Column);
                        let nested_y = if has_preceding_text {
                            para_y
                        } else {
                            inner_area.y
                        };
                        // [#3637] 중첩 표는 부모 셀 안에서 시작해야 한다. 앞 텍스트가 셀
                        // 밖으로 밀린 `para_y` 를 그대로 쓰면 컨테이너가 통째로 셀 아래에
                        // 놓여 쪽 밖으로 나간다(80550 29쪽: 셀 310~889 인데 중첩2가
                        // 889~1193). PR #3666 이 문단에 건 상한과 같은 계열의 한 단계 깊은
                        // 경로다.
                        // 누적 오프셋을 가진 1×1 RowBreak continuation은 현재 clip보다
                        // 뒤에 있는 다음 내부 표까지 하단으로 clamp하면 안 된다. 원래
                        // 다음 페이지에 속할 표들이 모두 같은 셀 하단으로 재배치되어
                        // 겹치기 때문이다(issue2007 42065 p7–p12). 이때만 원래 y를
                        // 유지해 부모 Cell clip이 미래 표를 제외하고, 앞쪽 표는 음수 y로
                        // 이어서 그릴 수 있다.
                        //
                        // 단순 `row_filter + 1×1`은 continuation의 첫 조각(offset=0)도
                        // 포함한다. 그 형상까지 상한을 풀면 #3637처럼 실제 셀 밖으로
                        // 새는 중첩 표가 다시 허용된다. 따라서 페이지 간에 이미 소비된
                        // 행 높이가 있는 실제 continuation으로 문맥을 좁힌다.
                        // native HWP5 RowBreak 1×1 wrapper는 offset=0인 첫 조각도
                        // 바깥 Cell을 continuation viewport로 사용한다. 이 조각의
                        // 하단으로 미래 descendant를 clamp하면 42065 p17 제목이
                        // p16에 미리 들어온다. HWPX는 실제로 셀 밖으로 빠진 중첩 표만
                        // 막기 위해 아래의 좁은 누적-offset guard를 계속 적용한다.
                        let hwp5_rowbreak_fragment = (self.profile.get().native_hwp5_layout()
                            || self.profile.get().hwp5_origin_hwpx())
                            && row_filter.is_some()
                            && table.row_count == 1
                            && table.col_count == 1;
                        let nested_y = if single_row_continuation || hwp5_rowbreak_fragment {
                            nested_y
                        } else {
                            nested_y.min(inner_area.y + inner_area.height)
                        };
                        let nested_ctx = cell_context.as_ref().map(|ctx| {
                            let mut new_ctx = ctx.clone();
                            new_ctx.path.push(CellPathEntry {
                                control_index: ctrl_idx,
                                cell_index: 0,
                                cell_para_index: 0,
                                text_direction: 0,
                            });
                            new_ctx
                        });
                        // [#4334] 아래 재귀 `layout_table` 호출 두 곳이 `table_meta: None`
                        // 을 넘겨 TableNode.para_index/control_index 가 항상 비었다 —
                        // 방금 확장한 `nested_ctx` 에서 이 중첩 표 자신의 좌표를 읽는다.
                        let derived_table_meta =
                            nested_ctx.as_ref().and_then(CellContext::nested_table_meta);
                        if is_tac_table {
                            // TAC 표: inline_x를 사용하여 수평 배치
                            // [Task #573] layout_composed_paragraph 의 run_tacs 가
                            // 인라인 TAC 표를 이미 렌더하고 set_inline_shape_position
                            // 등록했다면 중복 emit 방지 (Equation 의 L1800 가드와 동일 패턴).
                            let already_rendered_inline = tree
                                .get_inline_shape_position(
                                    section_index,
                                    cp_idx,
                                    ctrl_idx,
                                    cell_context.as_ref(),
                                )
                                .is_some();
                            let tac_w = hwpunit_to_px(nested_table.common.width as i32, self.dpi);
                            // [Issue #3396] 한글 TAC 표 문자 규칙: 괘선은
                            // pen + outMargin.left, 전진 폭은 outMargin 좌/우 포함.
                            let tac_om_l =
                                hwpunit_to_px(nested_table.outer_margin_left as i32, self.dpi);
                            let tac_om_r =
                                hwpunit_to_px(nested_table.outer_margin_right as i32, self.dpi);
                            if already_rendered_inline {
                                inline_x += tac_om_l + tac_w + tac_om_r;
                            } else {
                                // [Task #1195] 표 앞에 텍스트(공백 등)가 선행하면, 한컴은
                                // 그 textRun 너비 다음에 표를 놓되 잔여 너비가 부족하면
                                // 다음 줄(line feed)에 조판한다. 즉 표는 문단 첫 줄이 아니라
                                // 표가 속한 line_seg(표 앞 빈 줄 다음)에 위치한다.
                                // 이미지 TAC 분기(L2231)와 동일하게 para_y_before_compose 에
                                // (표 line_seg.vpos − 첫 line_seg.vpos) 상대 오프셋을 더한다.
                                // (para_y_before_compose 에 이미 ls[0].vpos 가 누적되어 있음.)
                                let table_anchor_y = if has_preceding_text
                                    && para.line_segs.len() > 1
                                {
                                    let first_vpos =
                                        para.line_segs.first().map(|f| f.vertical_pos).unwrap_or(0);
                                    let tbl_vpos = para
                                        .line_segs
                                        .last()
                                        .map(|s| s.vertical_pos)
                                        .unwrap_or(first_vpos);
                                    para_y_before_compose
                                        + hwpunit_to_px(tbl_vpos - first_vpos, self.dpi)
                                } else {
                                    para_y_before_compose
                                };
                                // [#3386] 표 전용 줄(저장 lh == h + om_top + om_bottom)
                                // 은 표 상단 = 줄 상단 + om_top 이 한글 실좌표다
                                // (156678235 p5: 저장 vpos+om_top == 한글 PDF 상단
                                // 536.7px, 종전 anchor 는 om_top 소실로 3.8px 상향).
                                let host_seg_lh = if has_preceding_text && para.line_segs.len() > 1
                                {
                                    para.line_segs.last().map(|s| s.line_height).unwrap_or(0)
                                } else {
                                    para.line_segs.first().map(|s| s.line_height).unwrap_or(0)
                                };
                                let om_top_hu = i64::from(nested_table.outer_margin_top);
                                let om_bottom_hu = i64::from(nested_table.outer_margin_bottom);
                                let table_anchor_y = if nested_table.common.height < 0x8000_0000
                                    && om_top_hu + om_bottom_hu > 0
                                    && i64::from(host_seg_lh)
                                        >= i64::from(nested_table.common.height)
                                            + om_top_hu
                                            + om_bottom_hu
                                            - 10
                                {
                                    table_anchor_y
                                        + hwpunit_to_px(
                                            nested_table.outer_margin_top as i32,
                                            self.dpi,
                                        )
                                } else {
                                    table_anchor_y
                                };
                                let ctrl_area = LayoutRect {
                                    x: inline_x + tac_om_l,
                                    y: table_anchor_y,
                                    width: tac_w,
                                    height: (inner_area.height - (table_anchor_y - inner_area.y))
                                        .max(0.0),
                                };
                                let table_h = self.layout_table(
                                    tree,
                                    cell_node,
                                    nested_table,
                                    section_index,
                                    styles,
                                    outline_numbering_id,
                                    &ctrl_area,
                                    table_anchor_y,
                                    bin_data_content,
                                    None,
                                    depth + 1,
                                    derived_table_meta,
                                    para_alignment,
                                    nested_ctx,
                                    0.0,
                                    0.0,
                                    Some(inline_x + tac_om_l),
                                    None,
                                    None,
                                    None,
                                    false,
                                    clamp_header_negative_para_offset,
                                    false,
                                );
                                inline_x += tac_om_l + tac_w + tac_om_r;
                                // para_y는 TAC 표 높이만큼 갱신 (같은 문단 내 다음 표도 같은 y)
                                let new_bottom = para_y_before_compose + table_h;
                                if new_bottom > para_y {
                                    para_y = new_bottom;
                                }
                            }
                        } else {
                            // 비-TAC 표: 기존 수직 배치
                            // 앞 텍스트 너비만큼 x 오프셋 적용
                            let tac_text_offset = if nested_table.attr & 0x01 != 0 {
                                let mut text_w = 0.0;
                                for line in &composed.lines {
                                    for run in &line.runs {
                                        if !run.text.is_empty() {
                                            let ts = resolved_to_text_style(
                                                styles,
                                                run.char_style_id,
                                                run.lang_index,
                                            );
                                            // [Task #555] PUA 옛한글 변환 후 자모 시퀀스 폭.
                                            text_w += estimate_text_width(
                                                effective_text_for_metrics(run),
                                                &ts,
                                            );
                                        }
                                    }
                                }
                                text_w
                            } else {
                                0.0
                            };
                            // TAC 표 앞 텍스트 렌더링 (문단부호 등 표시용)
                            if tac_text_offset > 0.0 {
                                let line_h = composed
                                    .lines
                                    .first()
                                    .map(|l| hwpunit_to_px(l.line_height, self.dpi))
                                    .unwrap_or(12.0);
                                let baseline = line_h * 0.85;
                                let line_id = tree.next_id();
                                let mut line_node = RenderNode::new(
                                    line_id,
                                    RenderNodeType::TextLine(TextLineNode::new(line_h, baseline)),
                                    BoundingBox::new(
                                        inner_area.x,
                                        nested_y,
                                        tac_text_offset,
                                        line_h,
                                    ),
                                );
                                let mut run_x = inner_area.x;
                                for line in &composed.lines {
                                    for run in &line.runs {
                                        if run.text.is_empty() {
                                            continue;
                                        }
                                        let ts = resolved_to_text_style(
                                            styles,
                                            run.char_style_id,
                                            run.lang_index,
                                        );
                                        // [Task #555] PUA 옛한글 변환 후 자모 시퀀스 폭.
                                        let run_w = estimate_text_width(
                                            effective_text_for_metrics(run),
                                            &ts,
                                        );
                                        let run_id = tree.next_id();
                                        let run_node = RenderNode::new(
                                            run_id,
                                            RenderNodeType::TextRun(TextRunNode {
                                                text: run.text.clone(),
                                                style: ts,
                                                char_shape_id: Some(run.char_style_id),
                                                para_shape_id: Some(para.para_shape_id),
                                                section_index: Some(section_index),
                                                para_index: None,
                                                char_start: None,
                                                cell_context: cell_context.clone(),
                                                is_para_end: false,
                                                is_line_break_end: false,
                                                rotation: 0.0,
                                                is_vertical: false,
                                                char_overlap: None,
                                                border_fill_id: 0,
                                                baseline,
                                                field_marker: FieldMarkerType::None,
                                                display_text: None,
                                            }),
                                            BoundingBox::new(run_x, nested_y, run_w, line_h),
                                        );
                                        line_node.children.push(run_node);
                                        run_x += run_w;
                                    }
                                }
                                cell_node.children.push(line_node);
                            }
                            let ctrl_area = LayoutRect {
                                x: inner_area.x + tac_text_offset,
                                y: nested_y,
                                width: (inner_area.width - tac_text_offset).max(0.0),
                                height: (inner_area.height - (nested_y - inner_area.y)).max(0.0),
                            };
                            // 이 셀 조각의 unit cut이 만든 중첩 표 slice를 다음 깊이에도
                            // 그대로 넘긴다. 픽셀 높이만으로 다시 행을 추정하면 첫 조각과
                            // continuation이 같은 행을 각각 재렌더해 쪽 소유가 깨진다.
                            // Source-unit viewport ownership is normally encoded in HWPX
                            // wrapper layouts. Native HWP5 RowBreak wrappers normally carry
                            // the equivalent physical cell clip and cumulative vpos;
                            // forwarding a general mixed split there advances the child early
                            // (42065 p16 -> p17).  The narrow short-parent child contract is
                            // different: its child was expanded to CellUnits specifically for
                            // this parent fragment, so omitting the cursor paints the first
                            // source line again on the continuation (76076 p81 -> p82).
                            let native_short_parent_child_split = self
                                .profile
                                .get()
                                .hwp5_stored_pagination_layout()
                                && self.native_short_parent_child_fragment_eligible(
                                    table,
                                    cell,
                                    nested_table,
                                    self.nested_table_mixed_fragment_heights(nested_table, styles)
                                        .iter()
                                        .map(|fragment| fragment.height)
                                        .sum(),
                                );
                            let nested_split = (self.profile.get().hwpx_stored_layout()
                                || native_short_parent_child_split)
                                .then_some(mixed_nested_split.as_ref())
                                .flatten();
                            let table_h = self.layout_table(
                                tree,
                                cell_node,
                                nested_table,
                                section_index,
                                styles,
                                outline_numbering_id,
                                &ctrl_area,
                                nested_y,
                                bin_data_content,
                                None,
                                depth + 1,
                                derived_table_meta,
                                para_alignment,
                                nested_ctx,
                                0.0,
                                0.0,
                                hwpx_nested_behind_text_overlay.then_some(inner_area.x),
                                nested_split,
                                None,
                                None,
                                false,
                                clamp_header_negative_para_offset,
                                false,
                            );
                            if !hwpx_nested_behind_text_overlay {
                                para_y = nested_y
                                    + nested_split
                                        .map(|split| split.flow_height)
                                        .unwrap_or(table_h);
                            }
                        }
                        has_preceding_text = true;
                    }
                    _ => {}
                }
            }
            if rendered_top_and_bottom_non_inline {
                para_y += self.paragraph_top_and_bottom_non_inline_flow_height(&para.controls);
            }

            // 마지막 인라인 Shape 이후의 남은 텍스트 렌더링 (예: "일")
            if prev_tac_text_pos > 0 {
                let total_text_chars = composed
                    .lines
                    .first()
                    .map(|line| {
                        line.runs
                            .iter()
                            .map(|r| r.text.chars().count())
                            .sum::<usize>()
                    })
                    .unwrap_or(0);
                if prev_tac_text_pos < total_text_chars {
                    let remaining_text: String = composed
                        .lines
                        .first()
                        .map(|line| {
                            let mut chars_so_far = 0usize;
                            let mut result = String::new();
                            for run in &line.runs {
                                for ch in run.text.chars() {
                                    if chars_so_far >= prev_tac_text_pos {
                                        result.push(ch);
                                    }
                                    chars_so_far += 1;
                                }
                            }
                            result
                        })
                        .unwrap_or_default();
                    let remaining_trimmed = remaining_text.trim_end();
                    if !remaining_trimmed.is_empty() {
                        let char_style_id = composed
                            .lines
                            .first()
                            .and_then(|l| l.runs.last())
                            .map(|r| r.char_style_id)
                            .unwrap_or(0);
                        let lang_index = composed
                            .lines
                            .first()
                            .and_then(|l| l.runs.last())
                            .map(|r| r.lang_index)
                            .unwrap_or(0);
                        let ts = resolved_to_text_style(styles, char_style_id, lang_index);
                        // [Task #555] PUA 옛한글 char 은 자모 시퀀스로 변환 후 폭 측정.
                        let remaining_metrics: String = {
                            use super::super::super::pua_oldhangul::map_pua_old_hangul;
                            remaining_trimmed
                                .chars()
                                .flat_map(|ch| {
                                    if let Some(jamos) = map_pua_old_hangul(ch) {
                                        jamos.iter().copied().collect::<Vec<_>>()
                                    } else {
                                        vec![ch]
                                    }
                                })
                                .collect()
                        };
                        let text_w = estimate_text_width(&remaining_metrics, &ts);
                        let text_baseline = ts.font_size * 0.85;
                        let text_h = ts.font_size * 1.2;
                        // 마지막 Shape 높이 기준으로 텍스트 y 계산
                        let last_shape_h = para
                            .controls
                            .iter()
                            .rev()
                            .find_map(|c| {
                                if let Control::Shape(s) = c {
                                    if s.common().treat_as_char {
                                        Some(hwpunit_to_px(s.common().height as i32, self.dpi))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0.0);
                        let text_y = para_y_before_compose + (last_shape_h - text_h).max(0.0);
                        let text_node_id = tree.next_id();
                        let text_node = RenderNode::new(
                            text_node_id,
                            RenderNodeType::TextRun(TextRunNode {
                                text: remaining_trimmed.to_string(),
                                style: ts,
                                char_shape_id: Some(char_style_id),
                                para_shape_id: Some(composed.para_style_id),
                                section_index: Some(section_index),
                                para_index: None,
                                char_start: None,
                                cell_context: None,
                                is_para_end: false,
                                is_line_break_end: false,
                                rotation: 0.0,
                                is_vertical: false,
                                char_overlap: None,
                                border_fill_id: 0,
                                baseline: text_baseline,
                                field_marker: FieldMarkerType::None,
                                display_text: None,
                            }),
                            BoundingBox::new(inline_x, text_y, text_w, text_h),
                        );
                        cell_node.children.push(text_node);
                    }
                }
            }

            if has_table_ctrl {
                // LINE_SEG vpos 기반으로 para_y 보정.
                // LINE_SEG.line_height에는 중첩 표 높이가 미포함될 수 있으므로
                // layout_table 반환값과 vpos 기반 중 적절한 값을 선택한다.
                let is_last_para = cp_idx + 1 == composed_paras.len();
                // 다음 문단의 vpos가 있으면 그것을 기준으로 para_y 보정
                if !is_last_para {
                    if let Some(next_para) = cell.paragraphs.get(cp_idx + 1) {
                        if let Some(next_seg) = next_para.line_segs.first() {
                            let next_vpos_y =
                                text_y_start + hwpunit_to_px(next_seg.vertical_pos, self.dpi);
                            // layout_table 기반 para_y와 다음 문단 vpos 중
                            // 더 큰 값 사용 (표가 LINE_SEG보다 클 수 있으므로)
                            para_y = para_y.max(next_vpos_y);
                        }
                    }
                }
                // 음수 line_spacing 처리 (중첩 구조에서 para_y 되돌리기)
                if !(is_last_para && enclosing_cell_ctx.is_some()) {
                    if let Some(last_line) = composed.lines.last() {
                        let ls = hwpunit_to_px(last_line.line_spacing, self.dpi);
                        if ls < -0.01 {
                            para_y += ls;
                        }
                    }
                }
            }
        }
    }

}
