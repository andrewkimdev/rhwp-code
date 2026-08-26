//! content_heights — table_layout.rs 에서 무변동 이동
use super::*;

impl LayoutEngine {
    /// 셀 안 비-TAC 자리차지 개체가 표 흐름에 요구하는 세로 범위.
    ///
    /// 한컴의 `쪽 영역 안으로 제한`은 세로 기준이 문단일 때 개체를 쪽 영역 안에
    /// 남기도록 흐름 높이에 반영된다. 반대로 제한이 꺼진 문단 기준 floating
    /// 개체는 표 행 높이를 밀지 않는다.
    pub(crate) fn non_inline_control_flow_height(&self, common: &CommonObjAttr) -> f64 {
        if common.treat_as_char || !matches!(common.text_wrap, TextWrap::TopAndBottom) {
            return 0.0;
        }
        let object_height = hwpunit_to_px(common.height as i32, self.dpi)
            + hwpunit_to_px(common.margin.top as i32, self.dpi)
            + hwpunit_to_px(common.margin.bottom as i32, self.dpi);
        if matches!(common.vert_rel_to, VertRelTo::Para) {
            if common.flow_with_text {
                hwpunit_to_px((common.vertical_offset as i32).max(0), self.dpi) + object_height
            } else {
                0.0
            }
        } else {
            object_height
        }
    }


    pub(crate) fn cell_non_inline_control_flow_height(&self, common: &CommonObjAttr) -> f64 {
        let top_and_bottom_height = self.non_inline_control_flow_height(common);
        if top_and_bottom_height > 0.0 || common.treat_as_char {
            return top_and_bottom_height;
        }

        if !matches!(
            common.text_wrap,
            TextWrap::Square | TextWrap::Tight | TextWrap::Through
        ) {
            return 0.0;
        }

        hwpunit_to_px(common.height as i32, self.dpi)
            + hwpunit_to_px(common.margin.top as i32, self.dpi)
            + hwpunit_to_px(common.margin.bottom as i32, self.dpi)
    }


    pub(crate) fn paragraph_top_and_bottom_non_inline_flow_height(
        &self,
        controls: &[Control],
    ) -> f64 {
        controls
            .iter()
            .map(|ctrl| match ctrl {
                Control::Picture(pic) => self.non_inline_control_flow_height(&pic.common),
                Control::Shape(shape) => self.non_inline_control_flow_height(shape.common()),
                _ => 0.0,
            })
            .fold(0.0, f64::max)
    }


    pub(crate) fn paragraph_cell_non_inline_controls_flow_height(
        &self,
        controls: &[Control],
    ) -> f64 {
        let (top_and_bottom_h, other_h) =
            self.paragraph_cell_non_inline_control_flow_parts(controls);
        top_and_bottom_h + other_h
    }


    pub(crate) fn paragraph_cell_non_inline_control_flow_parts(&self, controls: &[Control]) -> (f64, f64) {
        let mut top_and_bottom_h = 0.0f64;
        let mut other_h = 0.0f64;
        for ctrl in controls {
            let Some(common) = (match ctrl {
                Control::Picture(pic) => Some(&pic.common),
                Control::Shape(shape) => Some(shape.common()),
                _ => None,
            }) else {
                continue;
            };
            if common.treat_as_char {
                continue;
            }
            if matches!(common.text_wrap, TextWrap::TopAndBottom) {
                top_and_bottom_h =
                    top_and_bottom_h.max(self.non_inline_control_flow_height(common));
            } else {
                other_h += self.cell_non_inline_control_flow_height(common);
            }
        }
        (top_and_bottom_h, other_h)
    }


    /// 텍스트 없는 legacy HWP5 host 문단에 수평으로 나란히 놓인 Square/Tight/Through
    /// 개체들의 vertical flow band. 일반 경로는 control 높이를 합산한다. 여기서는
    /// 모든 개체가 paragraph-relative nonnegative offset이고 interval이 공통으로
    /// 겹친다는 좁은 증거가 있을 때만, paragraph origin부터 가장 먼 bottom까지의
    /// physical band를 반환한다. 서로 다른 세로 band나 stale negative offset을 가진
    /// 개체는 `None`으로 돌려 기존 합산 계약을 그대로 보존한다.
    pub(crate) fn paragraph_parallel_other_non_inline_flow_band_height(
        &self,
        controls: &[Control],
    ) -> Option<f64> {
        if controls.len() < 2 {
            return None;
        }

        let mut latest_start = 0.0f64;
        let mut earliest_end = f64::INFINITY;
        let mut furthest_bottom = 0.0f64;
        for control in controls {
            let common = match control {
                Control::Picture(picture) => &picture.common,
                Control::Shape(shape) => shape.common(),
                _ => return None,
            };
            if common.treat_as_char
                || !matches!(
                    common.text_wrap,
                    TextWrap::Square | TextWrap::Tight | TextWrap::Through
                )
                || !matches!(common.vert_rel_to, VertRelTo::Para)
            {
                return None;
            }
            let offset_hu = signed_hwpunit(common.vertical_offset);
            if offset_hu < 0 {
                return None;
            }
            let start = hwpunit_to_px(offset_hu, self.dpi);
            let height = self.cell_non_inline_control_flow_height(common);
            if height <= 0.5 {
                return None;
            }
            let end = start + height;
            latest_start = latest_start.max(start);
            earliest_end = earliest_end.min(end);
            furthest_bottom = furthest_bottom.max(end);
        }

        (latest_start + 0.5 < earliest_end).then_some(furthest_bottom)
    }


    /// Square/Tight/Through cell-flow의 control별 높이. 기존 aggregate 높이 계산과 같은
    /// contract를 유지하되, 16px fragment unit이 어떤 source control에 해당하는지 복원한다.
    pub(crate) fn paragraph_cell_other_non_inline_control_heights(
        &self,
        controls: &[Control],
    ) -> Vec<(usize, f64)> {
        controls
            .iter()
            .enumerate()
            .filter_map(|(control_idx, control)| {
                let common = match control {
                    Control::Picture(picture) => &picture.common,
                    Control::Shape(shape) => shape.common(),
                    _ => return None,
                };
                if common.treat_as_char || matches!(common.text_wrap, TextWrap::TopAndBottom) {
                    return None;
                }
                let height = self.cell_non_inline_control_flow_height(common);
                (height > 0.5).then_some((control_idx, height))
            })
            .collect()
    }


    pub(crate) fn cell_has_top_and_bottom_non_inline_flow(&self, cell: &crate::model::table::Cell) -> bool {
        cell.paragraphs
            .iter()
            .any(|para| self.paragraph_top_and_bottom_non_inline_flow_height(&para.controls) > 0.5)
    }


    pub(crate) fn calc_non_inline_controls_flow_height(&self, paragraphs: &[Paragraph]) -> f64 {
        paragraphs
            .iter()
            .map(|p| self.paragraph_top_and_bottom_non_inline_flow_height(&p.controls))
            .sum()
    }


    pub(crate) fn cell_wrap_object_visual_bottom(&self, common: &CommonObjAttr) -> f64 {
        if common.treat_as_char {
            return 0.0;
        }
        if !matches!(
            common.text_wrap,
            TextWrap::Square | TextWrap::Tight | TextWrap::Through
        ) {
            return 0.0;
        }

        let object_height = hwpunit_to_px(common.height as i32, self.dpi);
        let top_offset = if matches!(common.vert_rel_to, VertRelTo::Para) {
            hwpunit_to_px((common.vertical_offset as i32).max(0), self.dpi)
        } else {
            0.0
        };
        top_offset + object_height
    }


    pub(crate) fn calc_cell_wrap_objects_bottom_height(&self, paragraphs: &[Paragraph]) -> f64 {
        // [Task #2226] TopAndBottom flow 개체 보유 문단의 para_top 은 사다리 기반
        // 문단 시작 — height_measurer::cell_wrap_objects_bottom_height 와 동일 정정.
        let mut prev_extent = 0.0f64;
        paragraphs
            .iter()
            .map(|p| {
                let first_vpos = p
                    .line_segs
                    .first()
                    .map(|s| hwpunit_to_px(s.vertical_pos, self.dpi))
                    .unwrap_or(0.0);
                // 개체가 문단 시작~줄 상단 구간을 채우는 배치(줄이 개체 아래로
                // 밀림)면 first_vpos 는 문단 시작이 아니다 — 기하 판정으로 전환.
                let probe_object_bottom = p
                    .controls
                    .iter()
                    .map(|ctrl| match ctrl {
                        Control::Picture(pic) => self.cell_wrap_object_visual_bottom(&pic.common),
                        Control::Shape(shape) => {
                            self.cell_wrap_object_visual_bottom(shape.common())
                        }
                        _ => 0.0,
                    })
                    .fold(0.0f64, f64::max);
                let objects_above_line = probe_object_bottom > 0.0
                    && prev_extent + probe_object_bottom <= first_vpos + 0.5;
                let para_top = if objects_above_line {
                    prev_extent
                } else {
                    first_vpos
                };
                let para_extent = p
                    .line_segs
                    .iter()
                    .map(|s| hwpunit_to_px(s.vertical_pos + s.line_height.max(0), self.dpi))
                    .fold(prev_extent, f64::max);
                prev_extent = para_extent;
                let object_bottom = p
                    .controls
                    .iter()
                    .map(|ctrl| match ctrl {
                        Control::Picture(pic) => self.cell_wrap_object_visual_bottom(&pic.common),
                        Control::Shape(shape) => {
                            self.cell_wrap_object_visual_bottom(shape.common())
                        }
                        _ => 0.0,
                    })
                    .fold(0.0f64, f64::max);
                if object_bottom > 0.0 {
                    para_top + object_bottom
                } else {
                    0.0
                }
            })
            .fold(0.0f64, f64::max)
    }


    /// 셀 문단들의 콘텐츠 높이 합산 (spacing + line_height + line_spacing)
    pub(crate) fn calc_cell_paragraphs_content_height(
        &self,
        paragraphs: &[Paragraph],
        styles: &ResolvedStyleSet,
        cell_inner_width_px: f64,
    ) -> f64 {
        let (line_based, object_based) =
            self.calc_cell_paragraphs_content_parts(paragraphs, styles, cell_inner_width_px);
        line_based.max(object_based)
    }


    /// [Task #2211] 셀 콘텐츠 높이를 (줄 기반, 개체 기반)으로 분리 반환.
    /// 행 성장 판정에서 저장 LINE_SEG 줄 흐름은 pad 미가산, 개체(중첩 표·
    /// TopAndBottom flow·Square bottom) 지오메트리는 pad 가산이 한컴 정합 —
    /// 두 축의 pad 취급이 다르다 (#1486 p19 Square 그림 캘리브레이션).
    pub(crate) fn calc_cell_paragraphs_content_parts(
        &self,
        paragraphs: &[Paragraph],
        styles: &ResolvedStyleSet,
        cell_inner_width_px: f64,
    ) -> (f64, f64) {
        let cell_para_count = paragraphs.len();
        let line_based_height: f64 = paragraphs
            .iter()
            .enumerate()
            .map(|(pidx, p)| {
                let mut comp = compose_paragraph(p);
                // [Task #671] line_segs 비어 있는 셀 paragraph 의 단일 ComposedLine
                // 압축 결과를 셀 가용 너비에 맞춰 다중 ComposedLine 으로 재분할.
                // 측정/렌더링 일관성 보장 (table_layout.rs:1226 의 렌더링 경로와 동일).
                crate::renderer::composer::recompose_for_cell_width(
                    &mut comp,
                    p,
                    cell_inner_width_px,
                    styles,
                );
                self.calc_para_lines_height(
                    &comp.lines,
                    p,
                    self.profile.get().hwp3_layout()
                        && p.line_segs.is_empty()
                        && !p.text.is_empty(),
                    !p.line_segs.is_empty(),
                    pidx,
                    cell_para_count,
                    styles.para_styles.get(p.para_shape_id as usize),
                    styles,
                )
            })
            .sum();
        let object_based = self
            .calc_nested_controls_bottom_height(paragraphs, styles)
            .max(self.calc_non_inline_controls_flow_height(paragraphs))
            .max(self.calc_cell_wrap_objects_bottom_height(paragraphs));
        (line_based_height, object_based)
    }


    /// pre-composed 문단들의 콘텐츠 높이 합산 (compose 생략)
    pub(crate) fn calc_composed_paras_content_height(
        &self,
        composed_paras: &[ComposedParagraph],
        paragraphs: &[Paragraph],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let cell_para_count = paragraphs.len();
        composed_paras
            .iter()
            .zip(paragraphs.iter())
            .enumerate()
            .map(|(pidx, (comp, para))| {
                self.calc_para_lines_height(
                    &comp.lines,
                    para,
                    self.profile.get().hwp3_layout()
                        && para.line_segs.is_empty()
                        && !para.text.is_empty(),
                    !para.line_segs.is_empty(),
                    pidx,
                    cell_para_count,
                    styles.para_styles.get(para.para_shape_id as usize),
                    styles,
                )
            })
            .sum()
    }


    /// 단일 문단의 줄 높이 합산 (공통 로직)
    ///
    /// [Task #674] line_height 측정에 corrected_line_height 보정 적용.
    /// line_segs 부재 paragraph 의 fallback line_height (400 HU = 5.33 px) 가
    /// max_fs 보다 작은 경우 ParaShape 의 line_spacing_type + line_spacing 으로
    /// 보정. height_measurer.rs:570-587 와 동일 로직 — 측정/layout 일관성 보장.
    /// [#2112] `trust_stored_lh`: 실제 저장 LINE_SEG 를 보유한 문단은 저장 줄높이를
    /// 그대로 신뢰한다. 한글은 압축 줄높이(lh < 글자크기)를 저장값대로 렌더하는데,
    /// #674 보정(fs×줄간격% 대체)이 저장 줄에도 적용되어 셀 행높이가 부풀었다
    /// (39607: 행별 +3.8~+76.8px, 표 합계 +335.5px → 다쪽 표 쪽수 밀림).
    /// 보정은 line_segs 부재 폴백(400HU 합성 줄, #671/#674 원 목적)에만 유지.
    pub(crate) fn calc_para_lines_height(
        &self,
        lines: &[crate::renderer::composer::ComposedLine],
        para: &Paragraph,
        hwp3_variant_synthetic: bool,
        trust_stored_lh: bool,
        pidx: usize,
        total_para_count: usize,
        para_style: Option<&crate::renderer::style_resolver::ResolvedParaStyle>,
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let is_last_para = pidx + 1 == total_para_count;
        let spacing_before = if pidx > 0 {
            para_style.map(|s| s.spacing_before).unwrap_or(0.0)
        } else {
            0.0
        };
        let spacing_after = if !is_last_para {
            para_style.map(|s| s.spacing_after).unwrap_or(0.0)
        } else {
            0.0
        };
        if lines.is_empty() {
            // [#2169] NO_LS 순수 빈 문단 = em 줄박스 (한글 공식).
            let h = if crate::renderer::para_has_no_stored_line_segs(para)
                && para.controls.is_empty()
            {
                let fs = para
                    .char_shapes
                    .first()
                    .and_then(|cs| styles.char_styles.get(cs.char_shape_id as usize))
                    .map(|cs| cs.font_size)
                    .unwrap_or(0.0);
                if fs <= 0.0 {
                    hwpunit_to_px(400, self.dpi)
                } else if is_last_para {
                    fs
                } else {
                    match para_style {
                        Some(ps) => crate::renderer::corrected_line_height(
                            hwpunit_to_px(400, self.dpi),
                            fs,
                            ps.line_spacing_type,
                            ps.line_spacing,
                        ),
                        None => fs,
                    }
                }
            } else {
                hwpunit_to_px(400, self.dpi)
            };
            spacing_before + h + spacing_after
        } else {
            let cell_ls_val = para_style.map(|s| s.line_spacing).unwrap_or(160.0);
            let cell_ls_type = para_style
                .map(|s| s.line_spacing_type)
                .unwrap_or(crate::model::style::LineSpacingType::Percent);
            let line_count = lines.len();
            let lines_total: f64 = lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let raw_lh = hwpunit_to_px(line.line_height, self.dpi);
                    let max_fs = line
                        .runs
                        .iter()
                        .map(|r| {
                            styles
                                .char_styles
                                .get(r.char_style_id as usize)
                                .map(|cs| cs.font_size)
                                .unwrap_or(0.0)
                        })
                        .fold(0.0f64, f64::max);
                    // [#2169] NO_LS 순수 빈 문단 — 문단 char shape fs 폴백 (em 줄박스).
                    let max_fs = if max_fs <= 0.0
                        && crate::renderer::para_has_no_stored_line_segs(para)
                        && para.controls.is_empty()
                    {
                        para.char_shapes
                            .first()
                            .and_then(|cs| styles.char_styles.get(cs.char_shape_id as usize))
                            .map(|cs| cs.font_size)
                            .unwrap_or(0.0)
                    } else {
                        max_fs
                    };
                    let is_cell_last_line = is_last_para && i + 1 == line_count;
                    let h = if trust_stored_lh {
                        raw_lh
                    } else {
                        // [#2150/#2148] 셀 마지막 줄 em 공식 — #2195 축 정합 세트로
                        // 정식화됨 (종전 "[정식화 보류]" 주석은 stage3 실험기 잔재).
                        // [#2070] NO_LS 단일 문단·단일 줄 셀 = em — 한글은 1줄 셀에서
                        // 줄간격(Percent/Fixed)을 완전 무시 (fixed_ladder 실측).
                        crate::renderer::corrected_line_height_for_variant_synthetic(
                            raw_lh,
                            max_fs,
                            cell_ls_type,
                            cell_ls_val,
                            hwp3_variant_synthetic || is_cell_last_line,
                        )
                    };
                    if !is_cell_last_line {
                        h + hwpunit_to_px(line.line_spacing, self.dpi)
                    } else {
                        h
                    }
                })
                .sum();
            spacing_before + lines_total + spacing_after
        }
    }


    /// 세로쓰기 셀의 콘텐츠 높이 계산
    /// 세로쓰기에서 line_seg.segment_width = 열의 세로 길이 (HWPUNIT)
    /// 셀 높이 = 최대 segment_width
    pub(crate) fn calc_vertical_cell_content_height(&self, paragraphs: &[Paragraph]) -> f64 {
        let mut max_seg_height: f64 = 0.0;
        for para in paragraphs {
            for ls in &para.line_segs {
                let h = hwpunit_to_px(ls.segment_width, self.dpi);
                if h > max_seg_height {
                    max_seg_height = h;
                }
            }
        }
        if max_seg_height <= 0.0 {
            // fallback: 기본 높이
            hwpunit_to_px(400, self.dpi)
        } else {
            max_seg_height
        }
    }

}
