//! 표 레이아웃 (layout_table + 셀 높이/줄범위 계산)

use super::super::composer::{compose_paragraph, ComposedLine, ComposedParagraph};
use super::super::height_measurer::{
    fit_measured_table_nested_tail_to_declared_height, MeasuredTable,
};
use super::super::page_layout::LayoutRect;
use super::super::render_tree::*;
use super::super::style_resolver::{ResolvedBorderStyle, ResolvedStyleSet};
use crate::model::bin_data::BinDataContent;
use crate::model::control::Control;
use crate::model::paragraph::Paragraph;
use crate::model::style::{Alignment, BorderLine, CenterLine};
use crate::model::table::{TablePageBreak, VerticalAlign};
use crate::renderer::float_placement::signed_hwpunit;

const ROWBREAK_OBJECT_BOTTOM_BLEED_TOLERANCE_PX: f64 = 64.0;
/// [#3738 Stage 19] native HWP5가 빈 1×1 RowBreak picture table에 남기는 stale page
/// origin은 일반적인 local offset보다 한 페이지 단위로 크다. 이 값보다 작은 음수는
/// 일반 그림 위치일 수 있으므로 절대 보정하지 않는다.
const ROWBREAK_STALE_PAGE_SCALE_PICTURE_OFFSET_MIN_HU: i32 = -40_000;

/// [#2424 프로파일] 분할 표 컷 프리미티브 실측 카운터 — `RHWP_2424_PROFILE` 전용, 동작 불변.
/// 프로세스 누적이며 `RHWP_2424_STEP_PROFILE` 출력(typeset.rs)이 스냅샷을 읽는다.
pub(crate) static ISSUE2424_ADVANCE_ROW_CUT_CALLS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static ISSUE2424_ADVANCE_ROW_CUT_NANOS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static ISSUE2424_CELL_UNITS_HITS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static ISSUE2424_CELL_UNITS_MISSES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static ISSUE2424_CELL_UNITS_MISS_NANOS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// [#2424 프로파일] env 게이트 1회 판정. wasm 은 항상 false 라 `Instant::now` 가
/// 호출되지 않는다 (`paginate_pass` 의 게이트 패턴과 동일 규약).
pub(crate) fn issue2424_profile_enabled() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("RHWP_2424_PROFILE").is_ok_and(|value| !value.is_empty() && value != "0")
        })
    }
}

/// [Task #548] paragraph 의 line N 에 적용되는 effective margin_left.
/// paragraph_layout.rs 의 line_indent 산식과 동일 (단일 룰).
/// - positive indent: line 0 에만 +indent 적용 (첫줄 들여쓰기)
/// - negative indent (hanging): line N≥1 에 +|indent| 적용
/// - indent=0: 모든 line 에 margin_left 만 적용
pub(super) fn effective_margin_left_line(margin_left: f64, indent: f64, line_n: usize) -> f64 {
    let line_indent = if indent > 0.0 {
        if line_n == 0 {
            indent
        } else {
            0.0
        }
    } else if indent < 0.0 {
        if line_n == 0 {
            0.0
        } else {
            indent.abs()
        }
    } else {
        0.0
    };
    margin_left + line_indent
}

fn cell_para_line_anchor_y(
    base_y: f64,
    content_cell_y: f64,
    pad_top: f64,
    vertical_pos_hu: i32,
    dpi: f64,
    use_top_vpos_anchor: bool,
    upper_clip_line_reservation: f64,
) -> f64 {
    if use_top_vpos_anchor {
        // Top/vpos 앵커는 평상시 `base_y`의 vertical-align offset을 의도적으로
        // 무시한다. 단, RowBreak continuation의 page-clip 보정은 바로 이
        // absolute vpos 경로에도 적용해야 한다. 그렇지 않으면 text_y_start에
        // 예약값을 더해도 실제 문단은 종전 y에 남아 첫 줄이 clip 밖에서 사라진다.
        content_cell_y + pad_top + hwpunit_to_px(vertical_pos_hu, dpi) + upper_clip_line_reservation
    } else {
        base_y + hwpunit_to_px(vertical_pos_hu, dpi)
    }
}

fn has_initial_tac_shape_host(paragraphs: &[Paragraph]) -> bool {
    paragraphs.first().is_some_and(|para| {
        para.text.trim().is_empty()
            && para
                .controls
                .iter()
                .any(|ctrl| matches!(ctrl, Control::Shape(shape) if shape.common().treat_as_char))
    })
}

/// native HWP5와 original HWPX의 빈 RowBreak 그림 표가 fresh page로 이월된 뒤에도, 내부 picture가
/// 이월 전 outer host 좌표를 상쇄하지 않도록 하는 정확한 형상 판정이다.
///
/// `host_stored_vpos_hu`는 table의 소유 문단에서만 얻을 수 있으며 셀 paragraph의
/// vpos와 다르다. 이 값을 table-cell 경로까지 명시적으로 전달해, page-scale 음수
/// picture offset이 의도한 일반 음수 위치인지와 page boundary 상쇄인지 구분한다.
fn stored_layout_relocated_empty_rowbreak_picture_resets_offset(
    stored_layout: bool,
    native_hwp5_layout: bool,
    host_stored_vpos_hu: Option<i32>,
    table: &crate::model::table::Table,
    cell: &crate::model::table::Cell,
    para: &Paragraph,
    picture: &crate::model::image::Picture,
) -> bool {
    let Some(host_vpos) = host_stored_vpos_hu else {
        return false;
    };
    if !stored_layout
        || host_vpos <= 0
        || table.page_break != TablePageBreak::RowBreak
        || table.common.treat_as_char
        || !matches!(table.common.text_wrap, TextWrap::TopAndBottom)
        || !matches!(table.common.vert_rel_to, VertRelTo::Para)
        || table.row_count != 1
        || table.col_count != 1
        || table.cells.len() != 1
        || cell.row != 0
        || cell.col != 0
        || cell.row_span != 1
        || cell.col_span != 1
        || cell.paragraphs.len() != 1
        || !para.text.trim().is_empty()
        || para.controls.len() != 1
        || para.line_segs.len() != 1
        || para.line_segs[0].vertical_pos != 0
        || !matches!(picture.common.text_wrap, TextWrap::TopAndBottom)
        || picture.common.treat_as_char
        || !picture.common.flow_with_text
        || !matches!(picture.common.vert_rel_to, VertRelTo::Para)
    {
        return false;
    }

    let table_offset = signed_hwpunit(table.common.vertical_offset);
    let picture_offset = signed_hwpunit(picture.common.vertical_offset);
    let relocated_page_ladder = table_offset > 0
        && picture_offset < 0
        && (host_vpos as i64 + table_offset as i64 + picture_offset as i64).abs() <= 8;
    // HWP p25 (pi=357)는 table vOffset=0인데 picture에만 stale -50000 HU가 남았다.
    // 이는 다음 쪽 ladder가 아니라 같은 물리 쪽의 page-scale stale origin이다. HWPX
    // stored-layout에는 이 HWP5 직렬화 서명이 없으므로 native 경로에만 한정한다.
    let same_page_stale_hwp5_picture = native_hwp5_layout
        && table_offset == 0
        && picture_offset <= ROWBREAK_STALE_PAGE_SCALE_PICTURE_OFFSET_MIN_HU;
    relocated_page_ladder || same_page_stale_hwp5_picture
}

use super::super::composer::effective_text_for_metrics;
use super::super::{hwpunit_to_px, ShapeStyle};
use super::border_rendering::{
    build_row_col_x, collect_cell_borders, create_border_line_nodes, render_cell_diagonal,
    render_edge_borders, render_transparent_borders,
};
use super::text_measurement::{estimate_text_width, resolved_to_text_style};
use super::utils::find_bin_data_bytes;
use super::{CellContext, CellPathEntry, LayoutEngine};

// 표 수평 정렬: model::shape 타입 사용
use crate::model::shape::{
    Caption, CaptionDirection, CommonObjAttr, HorzAlign, HorzRelTo, TextWrap, VertRelTo,
};
mod nested_repair;
pub(crate) use nested_repair::*;
mod nested_split;
pub(crate) use nested_split::*;
mod geometry;
pub(crate) use geometry::*;
mod content_heights;
pub(crate) use content_heights::*;
mod cell_units;
pub(crate) use cell_units::*;
mod unit_row_cuts;
pub(crate) use unit_row_cuts::*;
mod cell_line_ranges;
pub(crate) use cell_line_ranges::*;
mod horizontal_cell;
pub(crate) use horizontal_cell::*;



fn caption_has_topbottom_picture(caption: &Caption) -> bool {
    caption.paragraphs.iter().any(|para| {
        para.controls.iter().any(|ctrl| {
            matches!(
                ctrl,
                Control::Picture(pic) if matches!(pic.common.text_wrap, TextWrap::TopAndBottom)
            )
        })
    })
}


/// A terminal table border may be just outside a *wrapper ancestor* clip even
/// though its direct host cell contains it. This is distinct from a real
/// continuation: allowing an arbitrary descendant's vertical extent would
/// reveal future-page text, but a completed outer horizontal border within a
/// few pixels is paint-only. Preserve exactly that stroke interval (42065 p9).


/// A table that leaks less than this distance into a clipped continuation cell
/// is the terminal border of the previous fragment, not content for this page.
/// Keeping it paints a stray horizontal line at the next page's top (42065
/// p10/p13), while the corresponding text is already correctly clipped away.
///
/// A native 1px border may be rasterized just below the logical clip by up to
/// roughly 5px at the renderer's layout scale.  Six pixels remains below the
/// smallest real continuation fragment in this fixture, so it only suppresses
/// that paint residue rather than current-page table content.

/// SVG and Canvas both clip a stroke by its painted area, rather than by the
/// centerline.  Keep a reconstructed frame's whole stroke a hair inside the
/// viewport: a centreline exactly on the clip boundary loses its anti-aliased
/// outer half, and can disappear entirely at a fractional device scale.


/// A non-empty direct text line immediately followed by a nested table is a
/// heading/table group, not independent bottom-of-page prose.  HWP5 can keep
/// the heading in the preceding RowBreak fragment while putting the table's
/// first usable row only in the next fragment.  That paints the heading twice:
/// once at the prior page bottom and once above the next cell clip
/// (issue2007 p7--p8).  Keep the group together at the actual viewport that
/// can paint the table.

/// A text bbox is the layout line box.  Canvas glyph ink can extend slightly
/// above it, so the first line of a clipped cell needs a small paint-safe
/// inset rather than a centreline exactly on the clip boundary.


/// [Task #993] 분할 표 행 컷 — 행에 속한 셀(col 오름차순)별 "소비한 콘텐츠 유닛 수".
/// 빈 Vec = 처음부터(아무것도 소비 안 함).


/// [Task #993] 한 셀의 콘텐츠 유닛 — 합성 줄 1개 또는 중첩 표 atom 1개.
pub(super) struct CellUnit {
    /// 유닛 높이 (px).
    height: f64,
    /// 이 유닛 앞에 vpos 리셋(셀 내부 페이지 분할)이 있는가.
    hard_break_before: bool,
    /// 같은 문단의 줄 사이에서 페이지 하단까지 진행한 저장 vpos가
    /// 상단으로 되돌아가며 프레임이 바뀌는 경계인가.
    /// 이 경계를 흡수하면 렌더러의 줄 좌표가 역행하므로 부모 컷에서도
    /// 반드시 보존한다. 문단 사이 reset의 orphan/sliver 완화 계약과 구분한다.
    stored_frame_break_before: bool,
    vpos_gap_before: bool,
    /// 이 유닛이 속한 문단 인덱스 (셀 내). [#4149] 커서 프로브 계획이 창 문단
    /// 범위를 계산할 때 형제 모듈(table_partial)에서 읽는다.
    pub(super) para_idx: usize,
    /// 이 유닛이 visible 일 때 기여하는 문단 내 줄 범위 `[vis_start, vis_end)`.
    /// 텍스트 줄 유닛 = `(li, li+1)`, 중첩/빈 atom = `(0, line_count.max(1))`.
    vis_start: usize,
    vis_end: usize,
    /// [Task #1073] 이 유닛이 중첩 표의 한 행을 표현하면 그 행 인덱스. 텍스트/일반 유닛은 None.
    /// 분할 행에서 컷 → `NestedTableSplit`(중첩행 범위) 매핑에 사용.
    nested_row: Option<usize>,
    /// [#4069] `CELL` 분할 중첩 표의 한 행을 셀별 cursor로 더 잘게 나눈 조각.
    /// 바깥 CellUnit 컷이 이 조각을 선택하면 렌더러도 같은 자식 컷을 사용한다.
    nested_table_fragment: Option<NestedTableUnitCut>,
    mixed_nested_fragment: bool,
    mixed_nested_trailing: bool,
    mixed_nested_content_height: f64,
    /// [#4069] 이 mixed fragment가 자식 1×1 표의 canonical CellUnit을 그대로
    /// 투영한 것인지 표시한다. true이면 렌더도 같은 자식 컷 범위를 재귀 사용한다.
    mixed_nested_recursive: bool,
    /// 이 fragment의 첫 실제 콘텐츠가 직전 중첩 표 다음 문단에서 시작하는가.
    /// 새 block은 이전 viewport의 예약 줄이 아니므로 continuation origin에서
    /// 건너뛰지 않는다(42065 p9).
    mixed_nested_starts_after_table: bool,
    /// `mixed_nested_fragment`를 만든 immediate child cell의 source paragraph.
    /// 상위 host의 `para_idx`와 분리해 중첩 표 control의 정확한 원본 경계를 보존한다.
    mixed_nested_source_para_idx: Option<usize>,
    /// 자식 source 문단의 재귀 block prelude 역할. 재귀 투영 단계에서도 보존한다.
    recursive_block_prelude_role: RecursiveBlockPreludeRole,
    top_and_bottom_flow: bool,
    empty_spacer: bool,
    /// Square/Tight/Through non-inline flow fragment가 걸친 원 문단 control index 범위
    /// (inclusive). 높이·unit 경계는 바꾸지 않고 partial renderer의 page owner 판단에만 쓴다.
    non_inline_control_range: Option<(usize, usize)>,
}


impl LayoutEngine {


    #[allow(clippy::too_many_arguments)]
    pub(crate) fn layout_table(
        &self,
        tree: &mut LayoutFrame,
        col_node: &mut RenderNode,
        table: &crate::model::table::Table,
        section_index: usize,
        styles: &ResolvedStyleSet,
        outline_numbering_id: u16,
        col_area: &LayoutRect,
        y_start: f64,
        bin_data_content: &[BinDataContent],
        measured_table: Option<&MeasuredTable>,
        depth: usize,
        table_meta: Option<(usize, usize)>,
        host_alignment: Alignment,
        enclosing_cell_ctx: Option<CellContext>,
        host_margin_left: f64,
        host_margin_right: f64,
        inline_x_override: Option<f64>,
        nested_split: Option<&NestedTableSplit>,
        para_y: Option<f64>,
        outer_host_stored_vpos_hu: Option<i32>,
        allow_para_top_bleed: bool,
        clamp_header_negative_para_offset: bool,
        physical_outer_box_paint_inset: bool,
    ) -> f64 {
        if table.cells.is_empty() {
            if depth == 0 {
                return y_start;
            } else {
                return 0.0;
            }
        }
        // 1x1 래퍼 표 감지: 외곽 표를 무시하고 내부 표를 직접 렌더링.
        // (Task #688) 셀 paragraphs 가 2개 이상이면 첫 nested 표만 unwrap 시 나머지
        // paragraph 의 nested 표가 누락되므로 paragraphs.len() == 1 가드를 둔다.
        // controls.len() == 1 가드는 두지 않는다 — exam_social.hwp pi=15 (PR #681)
        // 처럼 정렬 마커 등 다른 control 이 동거하는 케이스에서 unwrap + 외곽선 분기를
        // 모두 보존해야 하므로 find_map 으로 첫 nested table 만 추출한다.
        if table.row_count == 1 && table.col_count == 1 && table.cells.len() == 1 {
            let cell = &table.cells[0];
            if cell.paragraphs.len() == 1 {
                let p = &cell.paragraphs[0];
                let has_visible_text = p
                    .text
                    .chars()
                    .any(|ch| !ch.is_whitespace() && ch != '\r' && ch != '\n');
                if !has_visible_text {
                    if let Some(nested) = p.controls.iter().find_map(|c| {
                        if let Control::Table(t) = c {
                            Some(t.as_ref())
                        } else {
                            None
                        }
                    }) {
                        // [Task #1658 v3] 외곽 1×1 래퍼가 페이지/용지 앵커 자리차지
                        // (절대배치) 표면, unwrap 이 외곽의 절대 y 를 소실시키고 내부 표를
                        // flow 커서(y_start)에 렌더하던 결함 교정 — 외곽 표 속성으로 절대
                        // y 를 계산해 내부 표 시작점으로 사용한다 (하단 고정 결재/서명 틀이
                        // 본문 상단에 그려지던 문제, #1653 RCA 패턴 B).
                        let y_start = if depth == 0
                            && !table.common.treat_as_char
                            && matches!(
                                table.common.text_wrap,
                                crate::model::shape::TextWrap::TopAndBottom
                            )
                            && matches!(
                                table.common.vert_rel_to,
                                crate::model::shape::VertRelTo::Page
                                    | crate::model::shape::VertRelTo::Paper
                            ) {
                            let outer_h = hwpunit_to_px(
                                crate::renderer::float_placement::signed_hwpunit(
                                    table.common.height,
                                )
                                .max(0),
                                self.dpi,
                            );
                            // [Issue #1858] valign=Bottom 하단앵커는 한컴이 **실측
                            // 내용 높이**로 박스 하단을 anchor 하단에 밀착시킨다.
                            // 선언높이(common.height)가 실측보다 크면(stale) 선언
                            // 기준 top 이 위로 떠서 결재/발신명의 코퍼스 전반이
                            // −30.5pt 상향(36389312 계열, 18건 중 13건 동일 상수).
                            // MeasuredTable(캡션 제외 행높이 합) 사용, 부재 시 선언 유지.
                            let effective_h = if matches!(
                                table.common.vert_align,
                                crate::model::shape::VertAlign::Bottom
                                    | crate::model::shape::VertAlign::Outside
                            ) {
                                measured_table
                                    .map(|mt| (mt.total_height - mt.caption_height).max(0.0))
                                    .filter(|h| *h > 0.0)
                                    .unwrap_or(outer_h)
                            } else {
                                outer_h
                            };
                            self.compute_table_y_position(
                                table,
                                effective_h,
                                y_start,
                                col_area,
                                depth,
                                0.0,
                                0.0,
                                para_y,
                                allow_para_top_bleed,
                            )
                        } else {
                            y_start
                        };
                        // [Task: nested-table-border] 자료 박스 외곽 테두리 추가:
                        // 외부 1x1 표가 wrapper 라도 padding + border_fill 에 테두리선이
                        // 정의된 경우 (자료 박스 외곽), 외곽 4개 라인을 별도 추가하여 시각 정합.
                        // 외곽 박스의 size 는 nested layout 의 실제 결과 (y_end - y_start) 와
                        // nested 표의 측정 width 를 사용하여 내부 표 영역과 정확히 정합.
                        // (exam_social.hwp pi=15 4번 자료 박스: 외부 1x1 padding=(850,850,850,850)
                        //  border_fill_id=6, 내부 6x3 대화체 셀.)
                        let outer_y = y_start;
                        let outer_border_meta = if depth == 0 {
                            let has_outer_padding = cell.padding.left != 0
                                || cell.padding.right != 0
                                || cell.padding.top != 0
                                || cell.padding.bottom != 0;
                            if has_outer_padding {
                                // border_fill_id 는 1-based(borderFillIDRef), border_styles 는
                                // 0-based Vec 이므로 -1 변환한다. (일반 셀/표/zone lookup 과 동일)
                                if let Some(bs) = styles
                                    .border_styles
                                    .get((cell.border_fill_id as usize).saturating_sub(1))
                                {
                                    let any_border = bs.borders.iter().any(|b| {
                                        b.line_type != crate::model::style::BorderLineType::None
                                    });
                                    if any_border {
                                        Some(bs.borders)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        // nested 표 위치/size 미리 결정 (nested layout 의 위치 결정 logic 동일)
                        let pw_now = self.current_paper_width.get();
                        let paper_w = if pw_now > 0.0 { Some(pw_now) } else { None };
                        let nested_w = hwpunit_to_px(nested.common.width as i32, self.dpi)
                            * self.render_table_width_scale(nested);
                        let outer_w_for_box = nested_w;
                        let outer_x_for_box = self.compute_table_x_position(
                            nested,
                            nested_w,
                            col_area,
                            depth,
                            host_alignment,
                            host_margin_left,
                            host_margin_right,
                            inline_x_override,
                            paper_w,
                        );

                        let y_end = self.layout_table(
                            tree,
                            col_node,
                            nested,
                            section_index,
                            styles,
                            outline_numbering_id,
                            col_area,
                            y_start,
                            bin_data_content,
                            None,
                            depth,
                            table_meta,
                            host_alignment,
                            enclosing_cell_ctx,
                            host_margin_left,
                            host_margin_right,
                            inline_x_override,
                            nested_split,
                            para_y,
                            None,
                            allow_para_top_bleed,
                            clamp_header_negative_para_offset,
                            false,
                        );

                        if let Some(bs_borders) = outer_border_meta {
                            let outer_h_actual = (y_end - outer_y).max(0.0);
                            if outer_h_actual > 0.0 {
                                use super::border_rendering::create_border_line_nodes;
                                // 좌
                                col_node.children.extend(create_border_line_nodes(
                                    tree,
                                    &bs_borders[0],
                                    outer_x_for_box,
                                    outer_y,
                                    outer_x_for_box,
                                    outer_y + outer_h_actual,
                                ));
                                // 우
                                col_node.children.extend(create_border_line_nodes(
                                    tree,
                                    &bs_borders[1],
                                    outer_x_for_box + outer_w_for_box,
                                    outer_y,
                                    outer_x_for_box + outer_w_for_box,
                                    outer_y + outer_h_actual,
                                ));
                                // 상
                                col_node.children.extend(create_border_line_nodes(
                                    tree,
                                    &bs_borders[2],
                                    outer_x_for_box,
                                    outer_y,
                                    outer_x_for_box + outer_w_for_box,
                                    outer_y,
                                ));
                                // 하
                                col_node.children.extend(create_border_line_nodes(
                                    tree,
                                    &bs_borders[3],
                                    outer_x_for_box,
                                    outer_y + outer_h_actual,
                                    outer_x_for_box + outer_w_for_box,
                                    outer_y + outer_h_actual,
                                ));
                            }
                        }
                        return y_end;
                    }
                }
            }
        }

        let col_count = table.col_count as usize;
        let row_count = table.row_count as usize;
        let cell_spacing = hwpunit_to_px(table.cell_spacing as i32, self.dpi);

        // ── 1. 열 폭 + 행 높이 계산 ──
        let mut col_widths = self.resolve_column_widths(table, col_count);
        let row_heights = self.resolve_row_heights(
            table,
            col_count,
            row_count,
            measured_table,
            styles,
            depth > 0 || table.common.treat_as_char,
        );
        if std::env::var("RHWP_DIAG_TAC").is_ok() {
            let decl: Vec<f64> = table
                .cells
                .iter()
                .filter(|c| c.row_span == 1)
                .map(|c| hwpunit_to_px(c.height as i32, self.dpi))
                .collect();
            eprintln!(
                "[DIAG_TAC layout_table] tac={} depth={} mt={} meta={:?} rows={} y_start={:.1} decl_common_h={:.1} row_heights={:?} decl_cell_h={:?}",
                table.common.treat_as_char,
                depth,
                measured_table.is_some(),
                table_meta,
                row_count,
                y_start,
                hwpunit_to_px(table.common.height as i32, self.dpi),
                row_heights.iter().map(|h| (h * 10.0).round() / 10.0).collect::<Vec<_>>(),
                decl.iter().map(|h| (h * 10.0).round() / 10.0).collect::<Vec<_>>(),
            );
        }

        // ── 2. 누적 위치 계산 ──
        let mut col_x = vec![0.0f64; col_count + 1];
        for i in 0..col_count {
            col_x[i + 1] =
                col_x[i] + col_widths[i] + if i + 1 < col_count { cell_spacing } else { 0.0 };
        }
        let mut row_y = vec![0.0f64; row_count + 1];
        for i in 0..row_count {
            row_y[i + 1] =
                row_y[i] + row_heights[i] + if i + 1 < row_count { cell_spacing } else { 0.0 };
        }

        // 부모 셀 조각이 전달한 viewport보다 큰 손자 표는, source-unit split이 따로
        // 없는 경우에도 이 조각에서 실제 보이는 행까지만 생성한다. 종전에는 이 경우
        // 전체 행을 RenderTree에 넣은 다음 조상 Cell clip으로만 숨겼다. clip은 SVG
        // 잉크는 가리지만 쪽 하단 밖 TextLine까지 없애지는 않아, 다음 쪽 소유인 줄이
        // 현재 쪽의 `LAYOUT_OVERFLOW_CELL`로 계상됐다(#3637 p28).
        //
        // 저장된 split은 source 소유를 표현하므로 항상 우선한다. 여기의 geometry
        // fallback은 split 부재 + 실제 중첩 표(depth>0)로 한정한다. 저장 파일은 이
        // 크기의 표에도 `treat_as_char` 비트를 남길 수 있으므로 그 비트로 제외하지
        // 않으며, 다음 조각은 부모 RowBreak viewport가 다시 호출해 소유한다.
        let inferred_viewport_split = if nested_split.is_none()
            && depth > 0
            && col_area.height > 0.0
            && row_y.last().copied().unwrap_or(0.0) > col_area.height + 0.5
        {
            Some(calc_nested_split_rows(
                &row_heights,
                cell_spacing,
                0.0,
                col_area.height,
            ))
        } else {
            None
        };
        let nested_split = nested_split.or(inferred_viewport_split.as_ref());

        // 중첩 표 부분 렌더링: row_y를 시프트하여 보이는 행만 표시
        let (row_y_shift, split_row_range, split_y_offset) = if let Some(split) = nested_split {
            let sr = split.start_row.min(row_count);
            let er = split.end_row.min(row_count);
            let shift = row_y[sr];
            // row_y를 시프트하여 start_row가 0에서 시작하도록 함
            for y in row_y.iter_mut() {
                *y -= shift;
            }
            // end_row 이후의 모든 row_y를 캡하여 spanning 셀이 보이는 영역을 초과하지 않도록 함
            let cap_y = if split.visible_height > 0.0 {
                split.visible_height.min(row_y[er])
            } else {
                row_y[er]
            };
            for i in er..=row_count {
                row_y[i] = cap_y;
            }
            // start_row 내부 오프셋: 이미 이전 페이지에 표시된 부분만큼 위로 올림
            (shift, Some((sr, er)), split.offset_within_start)
        } else {
            (0.0, None, 0.0)
        };
        // [#3658] 종료 조각 여부 — 셀 하단 초과 줄 드롭 예외 판정에 사용.
        let split_terminal = nested_split.is_some_and(|s| s.terminal);
        // PR #4122의 재귀 child cursor가 있으면 자식 RowCut이 continuation 소유권을
        // 직접 결정한다. 기존 1×1 위치/정렬 보정은 그 cursor가 없는 scalar fallback
        // continuation에서만 적용해 같은 흐름 오프셋을 두 번 소비하지 않는다.
        let scalar_single_row_fragment = nested_split.is_some_and(|split| {
            row_count == 1
                && col_count == 1
                && split.start_row == 0
                && split.end_row >= 1
                && split.recursive_cut.is_none()
        });
        let scalar_single_row_continuation_offset = nested_split.and_then(|split| {
            (scalar_single_row_fragment && split.offset_within_start > 0.5)
                .then_some(split.offset_within_start)
        });
        let scalar_single_row_fragment_content_offset = nested_split
            .filter(|_| scalar_single_row_fragment)
            .map(|split| split.content_offset);
        let scalar_force_source_start_cut = nested_split
            .is_some_and(|split| scalar_single_row_fragment && split.force_source_start_cut);
        let scalar_replay_terminal_boundary_unit = nested_split
            .is_some_and(|split| scalar_single_row_fragment && split.replay_terminal_boundary_unit);
        let scalar_single_row_continuation = scalar_single_row_continuation_offset.is_some();
        let mut row_col_x = build_row_col_x(
            table,
            &col_widths,
            col_count,
            row_count,
            cell_spacing,
            self.dpi,
            self.render_table_width_scale(table),
        );
        let independent_col_row_y = if split_row_range.is_none() && !table.common.treat_as_char {
            let col_row_y = build_col_row_y_from_cell_heights(
                table,
                &row_heights,
                &row_y,
                col_count,
                row_count,
                cell_spacing,
                self.dpi,
            );
            if has_independent_col_row_y(&col_row_y, &row_y) {
                Some(col_row_y)
            } else {
                None
            }
        } else {
            None
        };

        let mut table_width = row_col_x
            .iter()
            .map(|rx| rx.last().copied().unwrap_or(0.0))
            .fold(col_x.last().copied().unwrap_or(0.0), f64::max);
        // [#4042 버그 A] 셀 안 중첩 표(depth>0, 비-TAC)의 렌더 폭은 표 선언 폭(=부모 셀
        // full 폭)으로 결정되는데, 호출자(table_partial.rs:1309 continuation, table_layout.rs
        // 정상 비-TAC 셀 경로)는 이미 패딩을 뺀 col_area(inner_width)를 넘기고 원점도
        // compute_table_x_position 이 패딩 반영(inner_x)해 잡는다. 빠진 단계는 폭을 그
        // 안쪽 내용 상자(col_area.width)에 맞춰 clamp 하는 것뿐이라, 원점은 패딩만큼 우측
        // 이동했는데 폭은 full 이라 우측이 pad_left 만큼 셀 밖으로 넘쳐 클립됐다. col_area
        // 에 맞춰 균일 축소해 좌우 원점·폭을 정합시킨다. table_width < col_area.width 인
        // 정상/좁은 표(#3308 가운데 배치)와 TAC 표는 조건상 no-op. 지역변수 스케일링뿐이라
        // cell_units/projection 캐시를 재계산하지 않아 단일 패스 성능 불변.
        // row_col_x 를 함께 축소하지 않으면 셀 내용만 줄고 테두리 세로선이 full 로 남아
        // 우측 세로선이 어긋나므로 col_widths·col_x·row_col_x·table_width 를 동일 fit 로 축소.
        // fit 타깃은 col_area.width 가 아니라 원점 로직(compute_table_x_position depth>0
        // 분기, 2573-2575)이 실제로 쓰는 가용 폭 `area_w = col_area.width - om_left` 와
        // 정확히 일치시킨다. 표 원점이 col_area.x + om_left 로 밀리므로, 폭을 col_area.width
        // 로 맞추면 om_left(예: 조문대비표 ≈1.9px)만큼 우측이 여전히 초과한다. area_w 로
        // 맞추면 표 우측 = (col_area.x + om_left) + area_w = col_area.x + col_area.width 로
        // 셀 내용 우측에 정확히 flush 된다.
        let fit_om_left = hwpunit_to_px(table.outer_margin_left as i32, self.dpi);
        let fit_avail_w = (col_area.width - fit_om_left).max(0.0);
        // [#4042 버그 A] 다중열(col_count>1) 중첩 표만 대상. 1×1(단일 셀) 중첩 표는
        // 셀 자체가 표 폭이라 render_normalization 이 부모 셀에 맞춰 스트레치(#2195/#4058
        // 76076)하는 것이 정답 기하이며, 여기서 col_area.width 로 되축소하면 그 스트레치
        // 를 되돌려 nested fragment 기하가 어긋난다(issue_2308 회귀). 우측 클립 defect 는
        // 열 경계 합이 셀 내용 상자를 넘는 다중열 표의 증상이므로 col_count>1 로 한정한다
        // (케이스별 구조 가드).
        if depth > 0
            && table.col_count > 1
            && !table.common.treat_as_char
            && table_width > fit_avail_w + 0.5
        {
            let fit = fit_avail_w / table_width;
            for w in col_widths.iter_mut() {
                *w *= fit;
            }
            for x in col_x.iter_mut() {
                *x *= fit;
            }
            for rx in row_col_x.iter_mut() {
                for x in rx.iter_mut() {
                    *x *= fit;
                }
            }
            table_width *= fit;
        }
        let table_height = if let Some(col_row_y) = independent_col_row_y.as_ref() {
            col_row_y
                .iter()
                .filter_map(|cy| cy.last().copied())
                .fold(row_y.last().copied().unwrap_or(0.0), f64::max)
        } else if let Some((_, er)) = split_row_range {
            row_y[er].max(0.0)
        } else {
            row_y.last().copied().unwrap_or(0.0)
        };

        // ── 3. 위치 결정 ──
        let pw = self.current_paper_width.get();
        let paper_w = if pw > 0.0 { Some(pw) } else { None };
        let mut table_x = self.compute_table_x_position(
            table,
            table_width,
            col_area,
            depth,
            host_alignment,
            host_margin_left,
            host_margin_right,
            inline_x_override,
            paper_w,
        );

        let render_caption = should_render_table_caption(table, depth);
        let (caption_height, caption_spacing) = if render_caption {
            let ch = self.calculate_caption_height(&table.caption, styles);
            let cs = table
                .caption
                .as_ref()
                .map(|c| hwpunit_to_px(c.spacing as i32, self.dpi))
                .unwrap_or(0.0);
            (ch, cs)
        } else {
            (0.0, 0.0)
        };

        // Left 캡션: 표를 캡션 크기만큼 오른쪽으로 이동
        if render_caption {
            if let Some(ref cap) = table.caption {
                if matches!(cap.direction, crate::model::shape::CaptionDirection::Left) {
                    let cap_w = hwpunit_to_px(cap.width as i32, self.dpi);
                    table_x += cap_w + caption_spacing;
                }
            }
        }
        if physical_outer_box_paint_inset {
            table_x += hwpunit_to_px(table.outer_margin_left as i32, self.dpi);
        }

        let table_text_wrap = if depth == 0 {
            table.common.text_wrap
        } else {
            crate::model::shape::TextWrap::Square
        };
        let inline_top_caption_offset = if inline_x_override.is_some() && render_caption {
            top_caption_flow_extra(&table.caption, caption_height, caption_spacing)
        } else {
            0.0
        };

        // inline_x_override가 있으면 외부에서 inline 위치를 계산했으므로 x/y 기준은 유지한다.
        // 단, Top 캡션은 표 본문 위의 별도 영역이므로 표 본문 y 에 캡션 높이만큼 반영한다.
        let flow_table_y = if inline_x_override.is_some() {
            y_start + inline_top_caption_offset
        } else {
            let computed_y = self.compute_table_y_position(
                table,
                table_height,
                y_start,
                col_area,
                depth,
                caption_height,
                caption_spacing,
                para_y,
                allow_para_top_bleed,
            );
            if depth > 0 && render_caption {
                computed_y + top_caption_flow_extra(&table.caption, caption_height, caption_spacing)
            } else {
                computed_y
            }
        };
        let table_y = flow_table_y
            + if physical_outer_box_paint_inset {
                hwpunit_to_px(table.outer_margin_top as i32, self.dpi)
            } else {
                0.0
            };
        let inline_table_flow_y_shift = if inline_x_override.is_some() {
            para_y
                .map(|anchor_y| (flow_table_y - anchor_y).max(0.0))
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // ── 4. 표 노드 생성 ──
        let table_id = tree.next_id();
        let mut table_node = RenderNode::new(
            table_id,
            RenderNodeType::Table(TableNode {
                row_count: table.row_count,
                col_count: table.col_count,
                border_fill_id: table.border_fill_id,
                section_index: Some(section_index),
                para_index: table_meta.map(|(pi, _)| pi),
                control_index: table_meta.map(|(_, ci)| ci),
                // [#4334] 셀 안에 중첩된 표(nested table)의 문서 경로 — 최외곽 표는 None.
                cell_context: enclosing_cell_ctx.clone(),
            }),
            BoundingBox::new(table_x, table_y, table_width, table_height),
        );

        // ── 4-1. 표 배경 렌더링 (표 > 배경 > 색 > 면색) ──
        if table.border_fill_id > 0 {
            let tbl_idx = (table.border_fill_id as usize).saturating_sub(1);
            if let Some(tbl_bs) = styles.border_styles.get(tbl_idx) {
                self.render_cell_background(
                    tree,
                    &mut table_node,
                    Some(tbl_bs),
                    table_x,
                    table_y,
                    table_width,
                    table_height,
                    bin_data_content,
                );
            }
        }

        // ── 4-2. cellzone 배경 렌더링 (zone 전체 영역에 한 번) ──
        let mut cellzone_diagonal_nodes = Vec::new();
        let mut cellzone_diagonal_origin_covered = vec![vec![false; col_count]; row_count];
        for zone in &table.zones {
            if zone.border_fill_id == 0 {
                continue;
            }
            let zone_idx = (zone.border_fill_id as usize).saturating_sub(1);
            if let Some(zone_bs) = styles.border_styles.get(zone_idx) {
                // zone 영역의 좌표 계산
                let sc = zone.start_col as usize;
                let ec = (zone.end_col as usize + 1).min(col_count);
                let sr = zone.start_row as usize;
                let er = (zone.end_row as usize + 1).min(row_count);
                if sc < col_count && sr < row_count {
                    let zone_x = table_x
                        + row_col_x
                            .get(sr)
                            .and_then(|r| r.get(sc))
                            .copied()
                            .unwrap_or(0.0);
                    let zone_y = table_y + row_y.get(sr).copied().unwrap_or(0.0);
                    let zone_x_end = table_x
                        + row_col_x
                            .get(sr)
                            .and_then(|r| {
                                if ec < r.len() {
                                    Some(r[ec])
                                } else {
                                    r.last().map(|&last_x| {
                                        // 마지막 열 끝 = 마지막 열 시작 + 해당 셀 너비
                                        let last_col = r.len() - 1;
                                        table
                                            .cells
                                            .iter()
                                            .find(|c| {
                                                c.row as usize == sr && c.col as usize == last_col
                                            })
                                            .map(|c| {
                                                last_x + hwpunit_to_px(c.width as i32, self.dpi)
                                            })
                                            .unwrap_or(last_x)
                                    })
                                }
                            })
                            .unwrap_or(0.0);
                    let zone_y_end = table_y
                        + row_y.get(er).copied().unwrap_or_else(|| {
                            // 마지막 행 끝 = 마지막 행 시작 + 해당 행 높이
                            row_y.get(er - 1).copied().unwrap_or(0.0)
                                + table
                                    .row_sizes
                                    .get(er - 1)
                                    .map(|&h| hwpunit_to_px(h as i32, self.dpi))
                                    .unwrap_or(0.0)
                        });
                    let zone_w = (zone_x_end - zone_x).max(0.0);
                    let zone_h = (zone_y_end - zone_y).max(0.0);
                    // [Task #429] 단색/패턴/그라데이션 + 이미지 채우기 (zone 의 별도 image fill 처리는
                    // render_cell_background 가 통합 처리하므로 제거)
                    self.render_cell_background(
                        tree,
                        &mut table_node,
                        Some(zone_bs),
                        zone_x,
                        zone_y,
                        zone_w,
                        zone_h,
                        bin_data_content,
                    );
                    if border_style_has_diagonal(zone_bs)
                        && !cellzone_diagonal_fully_overridden_by_cells(
                            table,
                            styles,
                            sr,
                            er,
                            sc,
                            ec,
                            zone.border_fill_id,
                        )
                    {
                        mark_cellzone_diagonal_origin_coverage(
                            &mut cellzone_diagonal_origin_covered,
                            sr,
                            sc,
                        );
                        cellzone_diagonal_nodes.extend(render_cell_diagonal(
                            tree, zone_bs, zone_x, zone_y, zone_w, zone_h,
                        ));
                    }
                }
            }
        }

        // ── 5. 셀 레이아웃 ──
        let mut h_edges: Vec<Vec<Option<BorderLine>>> = vec![vec![None; col_count]; row_count + 1];
        let mut v_edges: Vec<Vec<Option<BorderLine>>> = vec![vec![None; row_count]; col_count + 1];

        self.layout_table_cells(
            tree,
            &mut table_node,
            table,
            section_index,
            styles,
            outline_numbering_id,
            col_area,
            bin_data_content,
            depth,
            table_meta,
            outer_host_stored_vpos_hu,
            enclosing_cell_ctx.clone(),
            &row_col_x,
            &row_y,
            independent_col_row_y.as_deref(),
            col_count,
            row_count,
            table_x,
            table_y,
            &mut h_edges,
            &mut v_edges,
            split_row_range,
            row_y_shift,
            split_y_offset,
            scalar_single_row_continuation,
            scalar_single_row_continuation_offset,
            scalar_single_row_fragment,
            scalar_single_row_fragment_content_offset,
            scalar_force_source_start_cut,
            scalar_replay_terminal_boundary_unit,
            split_terminal,
            clamp_header_negative_para_offset,
            inline_table_flow_y_shift,
            // HWP5에서는 표 안의 비글자 1×1 표가 `inMargin=(0,0,141,141)`를
            // 갖더라도 셀의 작은 좌우 저장 margin을 계속 적용하는 형상이 있다.
            // 일반 최상위 표의 #2195 pad 사다리는 유지하고, 실제 셀 내부 중첩에만
            // 문맥을 제한한다 (#2308 HWP 2024 PDF p34). MasterPage/footer의
            // 중첩 표는 #2195의 일반 aim=false 표 여백을 써야 하므로 제외한다
            // (exam_social p2 footer 번호 회귀).
            depth > 0
                && !table.common.treat_as_char
                // p34의 일반 non-TAC nested table은 saved 510HU margin을 유지해
                // 우측 테두리 침범을 막는다. 반면 RenderNormalizationOverlay가
                // 표시한 native short RowBreak child는 parent owner content box를
                // 그대로 써야 한컴 PDF의 p81 `… 등의 사고` wrap이 재현된다.
                && !self
                    .render_normalization_overlay()
                    .uses_owner_content_box(table)
                && !matches!(
                    col_node.node_type,
                    RenderNodeType::Header | RenderNodeType::Footer | RenderNodeType::MasterPage
                ),
            &cellzone_diagonal_origin_covered,
        );

        if !cellzone_diagonal_nodes.is_empty() {
            table_node.children.extend(cellzone_diagonal_nodes);
        }

        // ── 5-1. 표 전체 외곽 테두리 보충 ──
        // 셀 테두리만으로는 표 외곽이 비어있을 수 있음.
        // 셀이 해당 외곽 엣지를 커버하지 않는 곳에만 table.border_fill_id fallback 적용.
        // (셀이 존재하지만 의도적으로 테두리를 없앤 곳에는 적용하지 않음)
        if table.border_fill_id > 0 {
            let tbl_idx = (table.border_fill_id as usize).saturating_sub(1);
            if let Some(tbl_bs) = styles.border_styles.get(tbl_idx) {
                let borders = &tbl_bs.borders; // [left, right, top, bottom]

                // 셀이 커버하는 외곽 엣지 맵 구축
                let mut h_covered = vec![vec![false; col_count]; row_count + 1];
                let mut v_covered = vec![vec![false; row_count]; col_count + 1];
                for cell in &table.cells {
                    let c = cell.col as usize;
                    let r = cell.row as usize;
                    if c >= col_count || r >= row_count {
                        continue;
                    }
                    let ec = (c + cell.col_span as usize).min(col_count);
                    let er = (r + cell.row_span as usize).min(row_count);
                    // 상단
                    if r == 0 {
                        for cc in c..ec {
                            h_covered[0][cc] = true;
                        }
                    }
                    // 하단
                    if er == row_count {
                        for cc in c..ec {
                            h_covered[row_count][cc] = true;
                        }
                    }
                    // 좌측
                    if c == 0 {
                        for rr in r..er {
                            v_covered[0][rr] = true;
                        }
                    }
                    // 우측
                    if ec == col_count {
                        for rr in r..er {
                            v_covered[col_count][rr] = true;
                        }
                    }
                }

                // 셀이 커버하지 않는 외곽 엣지에만 fallback 적용
                for c in 0..col_count {
                    if h_edges[0][c].is_none() && !h_covered[0][c] {
                        let b = &borders[2];
                        if !matches!(b.line_type, crate::model::style::BorderLineType::None) {
                            h_edges[0][c] = Some(*b);
                        }
                    }
                    if h_edges[row_count][c].is_none() && !h_covered[row_count][c] {
                        let b = &borders[3];
                        if !matches!(b.line_type, crate::model::style::BorderLineType::None) {
                            h_edges[row_count][c] = Some(*b);
                        }
                    }
                }
                for r in 0..row_count {
                    if v_edges[0][r].is_none() && !v_covered[0][r] {
                        let b = &borders[0];
                        if !matches!(b.line_type, crate::model::style::BorderLineType::None) {
                            v_edges[0][r] = Some(*b);
                        }
                    }
                    if v_edges[col_count][r].is_none() && !v_covered[col_count][r] {
                        let b = &borders[1];
                        if !matches!(b.line_type, crate::model::style::BorderLineType::None) {
                            v_edges[col_count][r] = Some(*b);
                        }
                    }
                }
            }
        }

        // ── 6. 테두리 렌더링 ──
        if independent_col_row_y.is_none() {
            let body_top_clip = (depth == 0
                && self.is_body_flow_col_area(col_area)
                && (table_y - col_area.y).abs() <= 0.5)
                .then_some(col_area.y);
            table_node.children.extend(render_edge_borders(
                tree,
                &h_edges,
                &v_edges,
                &row_col_x,
                &row_y,
                table_x,
                table_y,
                body_top_clip,
            ));
            if self.show_transparent_borders.get() {
                table_node.children.extend(render_transparent_borders(
                    tree, &h_edges, &v_edges, &row_col_x, &row_y, table_x, table_y,
                ));
            }
        }

        // Cell children may complete normal table-edge rendering only after
        // the parent cell loop. Correct their horizontal clip at this point
        // without changing the vertical continuation viewport.
        extend_completed_nested_table_border_clips(
            tree,
            &mut table_node,
            self.profile.get().native_hwp5_layout() || self.profile.get().hwp5_origin_hwpx(),
            self.profile.get().hwpx_container(),
        );

        col_node.children.push(table_node);

        // ── 7. 캡션 렌더링 ──
        if render_caption {
            if let Some(ref caption) = table.caption {
                use crate::model::shape::{CaptionDirection, CaptionVertAlign};
                let (cap_x, cap_w, cap_y) = match caption.direction {
                    CaptionDirection::Top => (table_x, table_width, y_start),
                    CaptionDirection::Bottom => (
                        table_x,
                        table_width,
                        table_y + table_height + caption_spacing,
                    ),
                    CaptionDirection::Left | CaptionDirection::Right => {
                        let cw = hwpunit_to_px(caption.width as i32, self.dpi);
                        let cx = if caption.direction == CaptionDirection::Left {
                            table_x - cw - caption_spacing
                        } else {
                            table_x + table_width + caption_spacing
                        };
                        let cy = match caption.vert_align {
                            CaptionVertAlign::Top => table_y,
                            CaptionVertAlign::Center => {
                                table_y + (table_height - caption_height).max(0.0) / 2.0
                            }
                            CaptionVertAlign::Bottom => {
                                table_y + (table_height - caption_height).max(0.0)
                            }
                        };
                        (cx, cw, cy)
                    }
                };
                let cap_cell_ctx = table_meta
                    .map(|(pi, ci)| CellContext {
                        parent_para_index: pi,
                        path: vec![CellPathEntry {
                            control_index: ci,
                            cell_index: 65534, // 캡션 식별 센티널
                            cell_para_index: 0,
                            text_direction: 0,
                        }],
                    })
                    .or_else(|| {
                        enclosing_cell_ctx.as_ref().map(|ctx| {
                            let mut cc = ctx.clone();
                            if let Some(last) = cc.path.last_mut() {
                                last.cell_index = 65534;
                                last.cell_para_index = 0;
                            }
                            cc
                        })
                    });
                self.layout_caption(
                    tree,
                    col_node,
                    caption,
                    styles,
                    col_area,
                    cap_x,
                    cap_w,
                    cap_y,
                    &mut self.auto_counter.borrow_mut(),
                    bin_data_content,
                    cap_cell_ctx,
                );
            }
        }

        // ── 8. 반환값 ──
        if depth == 0 {
            // Left/Right 캡션은 표 높이에 영향 없음
            let is_lr_cap = table.caption.as_ref().map_or(false, |c| {
                use crate::model::shape::CaptionDirection;
                matches!(
                    c.direction,
                    CaptionDirection::Left | CaptionDirection::Right
                )
            });
            let caption_extra = if is_lr_cap {
                0.0
            } else {
                caption_height
                    + if caption_height > 0.0 {
                        caption_spacing
                    } else {
                        0.0
                    }
            };
            if matches!(
                table_text_wrap,
                crate::model::shape::TextWrap::BehindText
                    | crate::model::shape::TextWrap::InFrontOfText
            ) {
                // 글뒤로/글앞으로: y_offset 변경 없음
                y_start
            } else if matches!(table_text_wrap, crate::model::shape::TextWrap::TopAndBottom)
                && !table.common.treat_as_char
            {
                // 자리차지: 표 아래쪽까지 y_offset 진행 (절대 위치 기준)
                let table_bottom = table_y + table_height + caption_extra;
                table_bottom.max(y_start)
            } else {
                let total_height = table_height + caption_extra;
                y_start + total_height
            }
        } else {
            // 중첩 표: outer_margin 포함 높이 반환
            let om_top = hwpunit_to_px(table.outer_margin_top as i32, self.dpi);
            let om_bottom = hwpunit_to_px(table.outer_margin_bottom as i32, self.dpi);
            (table_height
                + caption_flow_extra(&table.caption, caption_height, caption_spacing)
                + om_top
                + om_bottom)
                .max(0.0)
        }
    }

    /// 열 폭 계산 (단일 셀 + 병합 셀 해결)
    pub(crate) fn resolve_column_widths(
        &self,
        table: &crate::model::table::Table,
        col_count: usize,
    ) -> Vec<f64> {
        let width_scale = self.render_table_width_scale(table);
        // 1단계: col_span==1인 셀에서 개별 열 폭 추출
        let base_grid_outlier_rows = table.base_grid_outlier_rows();
        let mut col_widths = vec![0.0f64; col_count];
        for cell in &table.cells {
            if table.local_resize_rows.contains(&cell.row)
                || base_grid_outlier_rows.contains(&cell.row)
            {
                continue;
            }
            if cell.col_span == 1 && (cell.col as usize) < col_count {
                let w = hwpunit_to_px(cell.width as i32, self.dpi) * width_scale;
                if w > col_widths[cell.col as usize] {
                    col_widths[cell.col as usize] = w;
                }
            }
        }

        // 2단계: 병합 셀에서 미지 열 폭을 반복적으로 해결
        {
            let mut constraints: Vec<(usize, usize, f64)> = Vec::new();
            for cell in &table.cells {
                if table.local_resize_rows.contains(&cell.row)
                    || base_grid_outlier_rows.contains(&cell.row)
                {
                    continue;
                }
                let c = cell.col as usize;
                let span = cell.col_span as usize;
                if span > 1 && c + span <= col_count {
                    let total_w = hwpunit_to_px(cell.width as i32, self.dpi) * width_scale;
                    if let Some(existing) = constraints.iter_mut().find(|x| x.0 == c && x.1 == span)
                    {
                        if total_w > existing.2 {
                            existing.2 = total_w;
                        }
                    } else {
                        constraints.push((c, span, total_w));
                    }
                }
            }

            // 고정점 반복: 서로 다른 행이 같은 열 구간을 다르게 묶어 선언해도
            // (각 행 자신은 유효한, hwp authoring 에서 흔한 패턴) 유일해가
            // 존재하면 그 값을 정확히 찾는다. 매 라운드마다 (1) "미지 열이
            // 정확히 하나"인 제약을 고정점까지 모두 채우고 — 한 제약을 풀면
            // 다른 제약의 미지 열이 하나로 줄어들 수 있으므로 반복한다 — (2)
            // 더 이상 그런 제약이 없으면, 아직 남은 제약 중 *미지 열이 가장
            // 적은* 것 하나만 균등분할로 확정한 뒤 다시 (1)로 돌아간다. span
            // 크기 순으로 고정된 순서를 쓰지 않는 이유: span 이 좁다고 미지
            // 열이 적은 것은 아니며(이미 다른 제약으로 일부가 풀렸을 수
            // 있음), 미지 열 개수가 실제로 "이미 확보된 정보량"을 반영한다.
            // 이렇게 하면 좁은 제약에서 확정된 값이 넓은 제약에 즉시
            // 전파되어, 종전처럼 나중에 처리되는 넓은 span 제약이 이미
            // 채워진 열들 때문에 무시되는 일이 없다.
            let exact_phase_bound = col_count + constraints.len() + 1;
            let round_bound = constraints.len() + 1;
            for _ in 0..round_bound {
                for _ in 0..exact_phase_bound {
                    let mut progress = false;
                    for &(c, span, total_w) in &constraints {
                        let known_sum: f64 = (c..c + span).map(|i| col_widths[i]).sum();
                        let unknown_cols: Vec<usize> =
                            (c..c + span).filter(|&i| col_widths[i] == 0.0).collect();
                        if unknown_cols.len() == 1 {
                            let remaining = (total_w - known_sum).max(0.0);
                            col_widths[unknown_cols[0]] = remaining;
                            progress = true;
                        }
                    }
                    if !progress {
                        break;
                    }
                }

                // 남은 제약 중 미지 열이 가장 적은 것 하나만 균등분할로 확정
                // (동률이면 constraints 순서상 먼저 나오는 것 — 결정적 결과).
                let next = constraints
                    .iter()
                    .filter_map(|&(c, span, total_w)| {
                        let known_sum: f64 = (c..c + span).map(|i| col_widths[i]).sum();
                        let unknown_cols: Vec<usize> =
                            (c..c + span).filter(|&i| col_widths[i] == 0.0).collect();
                        if unknown_cols.is_empty() {
                            None
                        } else {
                            Some((unknown_cols.len(), total_w, known_sum, unknown_cols))
                        }
                    })
                    .min_by_key(|&(unknown_count, ..)| unknown_count);

                match next {
                    Some((_, total_w, known_sum, unknown_cols)) => {
                        let remaining = (total_w - known_sum).max(0.0);
                        let per_col = remaining / unknown_cols.len() as f64;
                        for i in unknown_cols {
                            col_widths[i] = per_col;
                        }
                    }
                    None => break,
                }
            }

            // 병합 셀 제약이 이미 값이 있는 열들로만 구성되어도 총합이 더 클 수 있다
            // (예: 서로 다른 행이 같은 열 구간을 진짜로 상충되게 선언한 malformed
            // 입력). 위 고정점 반복이 정상 authoring 에서는 이미 정확한 값을
            // 찾아내므로 이 보정은 그런 malformed 케이스에 대한 안전망으로만
            // 작동한다 — 한컴은 이 경우 뒤쪽 열을 확장해 병합 셀 폭을 만족시킨다.
            for &(c, span, total_w) in &constraints {
                let known_sum: f64 = (c..c + span).map(|i| col_widths[i]).sum();
                let deficit = total_w - known_sum;
                if deficit > 0.5 {
                    let target_col = c + span - 1;
                    if target_col < col_widths.len() {
                        col_widths[target_col] += deficit;
                    }
                }
            }
        }

        // 3단계: 여전히 폭이 0인 열에 기본값 할당
        for c in 0..col_count {
            if col_widths[c] <= 0.0 {
                col_widths[c] = hwpunit_to_px(1800, self.dpi);
            }
        }
        let target_width = if table.common.width > 0 {
            hwpunit_to_px(table.common.width as i32, self.dpi) * width_scale
        } else {
            0.0
        };
        if target_width > 0.0 {
            let current: f64 = col_widths.iter().sum();
            let residual = target_width - current;
            if residual > 0.5 {
                if let Some(last) = col_widths.last_mut() {
                    *last += residual;
                }
            } else if residual < -0.5 {
                // [열 폭 오버슈트] 2단계는 이제 span 오름차순 단일 패스가 아니라
                // 고정점 반복으로 전체 연립방정식을 푼다(위 "고정점 반복" 주석
                // 참고) — 서로 다른 행이 같은 열 구간을 다르게 병합해 나누는
                // authoring(각 행 자신은 유효함, hwp 포맷에서 흔한 패턴)은 이제
                // 정상적으로 유일해를 찾아낸다. 이 지점까지 남는 오버슈트는 위
                // "malformed 입력" 주석과 같은 근본 원인 — 서로 다른 행이 같은
                // 열 구간을 진짜로 상충되게 선언해 연립방정식 자체에 유일해가
                // 없는 경우 — 뿐이다. 그런 malformed 케이스에서도 표가 선언 폭보다
                // 넓게 렌더되어서는 안 되므로, 초과분은 모든 열에 비례 축소해
                // 흡수한다 — 특정 열이 다른 열보다 부당하게 깎이지 않고, 이미
                // 해석된 열들 사이의 상대적 비율(각 열이 실제로 담아야 하는
                // 내용 폭 비율)도 그대로 유지된다.
                let scale = target_width / current;
                for w in col_widths.iter_mut() {
                    *w *= scale;
                }
            }
        }
        col_widths
    }

    /// 행 높이 계산 (MeasuredTable 우선, 없으면 셀/병합/컨텐츠 기반)
    pub(crate) fn resolve_row_heights(
        &self,
        table: &crate::model::table::Table,
        col_count: usize,
        row_count: usize,
        measured_table: Option<&MeasuredTable>,
        styles: &ResolvedStyleSet,
        relaxed_pad: bool,
    ) -> Vec<f64> {
        self.resolve_row_heights_with_common_fit(
            table,
            col_count,
            row_count,
            measured_table,
            styles,
            true,
            relaxed_pad,
        )
    }

    fn resolve_row_heights_for_content(
        &self,
        table: &crate::model::table::Table,
        col_count: usize,
        row_count: usize,
        measured_table: Option<&MeasuredTable>,
        styles: &ResolvedStyleSet,
        relaxed_pad: bool,
    ) -> Vec<f64> {
        self.resolve_row_heights_with_common_fit(
            table,
            col_count,
            row_count,
            measured_table,
            styles,
            false,
            relaxed_pad,
        )
    }

    /// [Task #2211] 셀의 전 문단이 저장 LINE_SEG 를 보유하는지 — 보유 셀은
    /// 한컴이 저장 시 셀 h 를 콘텐츠에 맞춰 확정했으므로 행 성장 판정에서
    /// 저장 지오메트리를 그대로 신뢰한다 (#2112 계보). 합성 seg(tag bit31)는
    /// 저장으로 치지 않는다 — height_measurer 와 동일 술어.
    fn cell_has_stored_line_segs(cell: &crate::model::table::Cell) -> bool {
        !cell.paragraphs.is_empty()
            && cell
                .paragraphs
                .iter()
                .all(|p| !crate::renderer::para_has_no_stored_line_segs(p))
    }

    /// [#3386] MeasuredTable 행높이를 행별 저장 선언(cellSz)으로 교정한다.
    /// 발동 조건(전부 충족 시에만):
    /// - 모든 셀이 row_span==1 이고 저장 LINE_SEG 를 보유(#2211 술어)
    /// - 모든 행에 유효 선언 높이 존재(cell.height < 0x8000_0000)
    /// - 선언 합 == 측정 합 (±1.5px; 총높이 보존 → 쪽수·후속 흐름 불변)
    /// - 행별 |선언-측정| <= max(12px, 선언의 15%) (실콘텐츠 성장 행 보호)
    fn trust_declared_row_heights(
        &self,
        table: &crate::model::table::Table,
        row_count: usize,
        rh: &mut [f64],
    ) {
        if row_count == 0 || rh.len() < row_count {
            return;
        }
        let mut decl = vec![f64::NAN; row_count];
        for cell in &table.cells {
            if cell.row_span != 1 {
                return;
            }
            if !Self::cell_has_stored_line_segs(cell) {
                return;
            }
            let r = cell.row as usize;
            if r >= row_count || cell.height >= 0x8000_0000 {
                return;
            }
            let h = hwpunit_to_px(cell.height as i32, self.dpi);
            if decl[r].is_nan() || h > decl[r] {
                decl[r] = h;
            }
        }
        if decl.iter().any(|d| d.is_nan()) {
            return;
        }
        let decl_sum: f64 = decl.iter().sum();
        let measured_sum: f64 = rh[..row_count].iter().sum();
        if (decl_sum - measured_sum).abs() > 1.5 {
            return;
        }
        for r in 0..row_count {
            if (decl[r] - rh[r]).abs() > (decl[r] * 0.15).max(12.0) {
                return;
            }
        }
        rh[..row_count].copy_from_slice(&decl);
    }

    fn resolve_row_heights_with_common_fit(
        &self,
        table: &crate::model::table::Table,
        col_count: usize,
        row_count: usize,
        measured_table: Option<&MeasuredTable>,
        styles: &ResolvedStyleSet,
        fit_common_height: bool,
        relaxed_pad: bool,
    ) -> Vec<f64> {
        if let Some(mt) = measured_table {
            // `TypesetEngine::format_table` uses this same narrow replacement for
            // native HWP5 empty RowBreak hosts.  Layout must consume the identical
            // row geometry: otherwise pagination reserves the declared tail height
            // but the SVG layout paints the old over-measured table and moves every
            // following table back down (76076 p81→82).  The helper verifies the
            // actual last-row 1×1 block-child shape and the bounded drift; this
            // outer gate confines it to the native TopAndBottom RowBreak contract.
            let native_rowbreak_nested_tail = self.profile.get().hwp5_stored_pagination_layout()
                && !table.common.treat_as_char
                && matches!(table.common.text_wrap, TextWrap::TopAndBottom)
                && matches!(table.common.vert_rel_to, VertRelTo::Para)
                && matches!(table.page_break, TablePageBreak::RowBreak)
                && table.row_count > 1
                && table.cells.iter().all(|cell| cell.row_span == 1);
            let tail_fitted = native_rowbreak_nested_tail
                .then(|| fit_measured_table_nested_tail_to_declared_height(mt, table, self.dpi))
                .flatten();
            let measured = tail_fitted.as_ref().unwrap_or(mt);
            let mut rh = measured.row_heights.clone();
            rh.resize(row_count, hwpunit_to_px(400, self.dpi));
            // [#3386] 행별 저장-선언 신뢰(렌더 전용): 전 셀이 rowspan 없이 저장
            // LINE_SEG 를 보유하고 행별 선언(cellSz)이 모두 존재하며, 선언 합이
            // 측정 합과 일치(±1.5px)하고 행별 편차가 max(12px,15%) 이내면 행
            // 경계는 선언을 따른다. 한글 PDF 실측(156678235 p5 pi=46): 한글
            // 행경계 == cellSz [22.4,41.9,69.4], rhwp 측정 재분배 [19.8,44.8,
            // 69.1] 은 폰트 메트릭 차로 행 경계만 드리프트 — 총높이 동일.
            self.trust_declared_row_heights(table, row_count, &mut rh);
            if fit_common_height {
                self.fit_row_heights_to_common_height(table, &mut rh);
            }
            return rh;
        }

        // 1단계: row_span==1인 셀에서 개별 행 높이 추출
        let mut row_heights = vec![0.0f64; row_count];
        for cell in &table.cells {
            if table.local_resize_cols.contains(&cell.col) {
                continue;
            }
            if cell.row_span == 1 && (cell.row as usize) < row_count {
                let r = cell.row as usize;
                if cell.height < 0x80000000 {
                    let h = hwpunit_to_px(cell.height as i32, self.dpi);
                    if h > row_heights[r] {
                        row_heights[r] = h;
                    }
                }
            }
        }

        // 1-b단계: 셀 내 실제 컨텐츠 높이 계산
        for cell in &table.cells {
            if table.local_resize_cols.contains(&cell.col) {
                continue;
            }
            if cell.row_span == 1 && (cell.row as usize) < row_count {
                let r = cell.row as usize;
                let (pad_left, pad_right, pad_top, pad_bottom) =
                    self.resolve_cell_padding(cell, table);

                // LINE_SEG의 line_height에 이미 셀 내 중첩 표 높이가 반영되어 있으므로
                // controls_height를 별도로 더하면 이중 계산됨
                // [Task #2211] 저장 LINE_SEG 보유 셀의 줄 흐름은 성장 판정에 pad 를
                // 더하지 않는다 — 한컴 저장 h 는 콘텐츠에 꽉 맞게 저장되며(빈 셀
                // lh=h), pad 가산 시 그런 행마다 +pad 상하합(주보 p1: 행당 +282HU)씩
                // 부풀어 하단이 절단된다. 개체 기반 지오메트리(Square bottom 등,
                // #1486 p19)와 LINE_SEG 부재(합성 줄) 셀은 pad 포함 유지.
                let required_height = if cell.text_direction != 0 {
                    // 세로쓰기: line_seg.segment_width가 열의 세로 길이
                    self.calc_vertical_cell_content_height(&cell.paragraphs) + pad_top + pad_bottom
                } else {
                    let cell_w_px = hwpunit_to_px(cell.width as i32, self.dpi)
                        * self.render_table_width_scale(table);
                    let inner_width = (cell_w_px - pad_left - pad_right).max(0.0);
                    let (line_based, object_based) = self.calc_cell_paragraphs_content_parts(
                        &cell.paragraphs,
                        styles,
                        inner_width,
                    );
                    // [#3386] 저장 cellSz 가 저장 줄 흐름보다 작은 모순 셀은 한글이
                    // 줄 흐름 + 상하 여백으로 재성장한다 (156678235 p5 내부 표 r0:
                    // cellSz 3.8px·lineseg 14.7px → 한글 PDF 실측 18.4px = 14.7+1.9×2).
                    // 선언이 줄 흐름을 수용하는 셀은 종전대로 pad 미가산 (#2211 유지).
                    let line_req = if relaxed_pad && Self::cell_has_stored_line_segs(cell) {
                        let decl_h = if cell.height < 0x8000_0000 {
                            hwpunit_to_px(cell.height as i32, self.dpi)
                        } else {
                            f64::MAX
                        };
                        if line_based > decl_h + 1.5 {
                            // 한글 실좌표는 원(cellMargin) 상하 여백 가산 — resolve
                            // 축소 pad(0.9×2)가 아니라 저장 1.9×2 로 18.4px 재현.
                            let raw_pad_v = hwpunit_to_px(cell.padding.top as i32, self.dpi)
                                + hwpunit_to_px(cell.padding.bottom as i32, self.dpi);
                            line_based + (pad_top + pad_bottom).max(raw_pad_v)
                        } else {
                            line_based
                        }
                    } else {
                        line_based + pad_top + pad_bottom
                    };
                    let object_req = if object_based > 0.0 {
                        object_based + pad_top + pad_bottom
                    } else {
                        0.0
                    };
                    line_req.max(object_req)
                };
                if required_height > row_heights[r] {
                    row_heights[r] = required_height;
                }
            }
        }

        // 2단계: 병합 셀에서 미지 행 높이를 반복적으로 해결
        {
            let mut constraints: Vec<(usize, usize, f64)> = Vec::new();
            for cell in &table.cells {
                if table.local_resize_cols.contains(&cell.col) {
                    continue;
                }
                let r = cell.row as usize;
                let span = cell.row_span as usize;
                if span > 1 && r + span <= row_count && cell.height < 0x80000000 {
                    let total_h = hwpunit_to_px(cell.height as i32, self.dpi);
                    if let Some(existing) = constraints.iter_mut().find(|x| x.0 == r && x.1 == span)
                    {
                        if total_h > existing.2 {
                            existing.2 = total_h;
                        }
                    } else {
                        constraints.push((r, span, total_h));
                    }
                }
            }
            constraints.sort_by_key(|&(_, span, _)| span);
            let max_iter = row_count + constraints.len();
            for _ in 0..max_iter {
                let mut progress = false;
                for &(r, span, total_h) in &constraints {
                    let known_sum: f64 = (r..r + span).map(|i| row_heights[i]).sum();
                    let unknown_rows: Vec<usize> =
                        (r..r + span).filter(|&i| row_heights[i] == 0.0).collect();
                    if unknown_rows.len() == 1 {
                        let remaining = (total_h - known_sum).max(0.0);
                        row_heights[unknown_rows[0]] = remaining;
                        progress = true;
                    }
                }
                if !progress {
                    break;
                }
            }
            for &(r, span, total_h) in &constraints {
                let known_sum: f64 = (r..r + span).map(|i| row_heights[i]).sum();
                let unknown_rows: Vec<usize> =
                    (r..r + span).filter(|&i| row_heights[i] == 0.0).collect();
                if !unknown_rows.is_empty() {
                    let remaining = (total_h - known_sum).max(0.0);
                    let per_row = remaining / unknown_rows.len() as f64;
                    for i in unknown_rows {
                        row_heights[i] = per_row;
                    }
                }
            }
            // [#2291/#2237] 병합 셀 **선언** 높이가 걸친 행합을 초과하면 잔여를
            // 마지막 걸침 행에 가산한다 — 한글 관례 실측(연결맵 244×10 r183:
            // c3 rs=4 선언 217.8px vs 행합 201.3px, 한글 행 괘선 실측 r183 =
            // 39.8+16.5 = 56.3px 정확 일치). 종전에는 모든 행이 rs=1 선언으로
            // 채워진(미지 행 없음) 표에서 이 잔여가 지면에서 소실되어, rowspan
            // 중첩 문서가 한글보다 쪽당 +15% 조밀해졌다(연결맵 −35쪽의 지배
            // 성분). 콘텐츠 기반 확장(2-b)과 별개의 선언 기반 규칙이다.
            for &(r, span, total_h) in &constraints {
                let known_sum: f64 = (r..r + span).map(|i| row_heights[i]).sum();
                if total_h > known_sum + 0.5 {
                    row_heights[r + span - 1] += total_h - known_sum;
                }
            }
        }

        // 2-b단계: 병합 셀 컨텐츠 높이 > 결합 행 높이이면 마지막 행 확장
        for cell in &table.cells {
            if table.local_resize_cols.contains(&cell.col) {
                continue;
            }
            let r = cell.row as usize;
            let span = cell.row_span as usize;
            if span > 1 && r + span <= row_count {
                let (pad_left, pad_right, pad_top, pad_bottom) =
                    self.resolve_cell_padding(cell, table);
                let cell_w_px = hwpunit_to_px(cell.width as i32, self.dpi)
                    * self.render_table_width_scale(table);
                let inner_width = (cell_w_px - pad_left - pad_right).max(0.0);
                // LINE_SEG의 line_height에 이미 셀 내 중첩 표 높이가 반영되어 있으므로
                // controls_height를 별도로 더하면 이중 계산됨
                // [Task #2211] 1-b 와 동일 — 저장 LINE_SEG 줄 흐름은 pad 미가산,
                // 개체 기반 지오메트리는 pad 가산 유지.
                let (line_based, object_based) =
                    self.calc_cell_paragraphs_content_parts(&cell.paragraphs, styles, inner_width);
                // [#3386] 1-b 와 동일 — 모순 선언(span 합) 초과 성장 시 여백 가산.
                let line_req = if relaxed_pad && Self::cell_has_stored_line_segs(cell) {
                    let decl_h = if cell.height < 0x8000_0000 {
                        hwpunit_to_px(cell.height as i32, self.dpi)
                    } else {
                        f64::MAX
                    };
                    if line_based > decl_h + 1.5 {
                        let raw_pad_v = hwpunit_to_px(cell.padding.top as i32, self.dpi)
                            + hwpunit_to_px(cell.padding.bottom as i32, self.dpi);
                        line_based + (pad_top + pad_bottom).max(raw_pad_v)
                    } else {
                        line_based
                    }
                } else {
                    line_based + pad_top + pad_bottom
                };
                let object_req = if object_based > 0.0 {
                    object_based + pad_top + pad_bottom
                } else {
                    0.0
                };
                let required_height = line_req.max(object_req);
                let combined: f64 = (r..r + span).map(|i| row_heights[i]).sum();
                if required_height > combined {
                    let deficit = required_height - combined;
                    row_heights[r + span - 1] += deficit;
                }
            }
        }

        // 3단계: 높이 0인 행에 기본값
        for r in 0..row_count {
            if row_heights[r] <= 0.0 {
                row_heights[r] = hwpunit_to_px(400, self.dpi);
            }
        }
        if fit_common_height {
            self.fit_row_heights_to_common_height(table, &mut row_heights);
        }
        row_heights
    }

    fn fit_row_heights_to_common_height(
        &self,
        table: &crate::model::table::Table,
        row_heights: &mut [f64],
    ) {
        if row_heights.is_empty() {
            return;
        }
        let target_height = if table.common.height > 0 {
            hwpunit_to_px(table.common.height as i32, self.dpi)
        } else {
            0.0
        };
        if target_height > 0.0 {
            let current: f64 = row_heights.iter().sum();
            let residual = target_height - current;
            if residual > 0.5 {
                if let Some(last) = row_heights.last_mut() {
                    *last += residual;
                }
            }
        }
    }


    /// 셀 패딩 계산
    pub(crate) fn resolve_cell_padding(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
    ) -> (f64, f64, f64, f64) {
        self.resolve_cell_padding_for_context(cell, table, false)
    }

    fn resolve_cell_padding_for_context(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        allow_saved_small_cell_margin: bool,
    ) -> (f64, f64, f64, f64) {
        // HWP 스펙: aim(apply_inner_margin)=true → cell.padding,
        //           aim=false → table.padding 우선.
        // 한컴은 aim=false일 때 cell.padding 원값을 파일에 보존하더라도 렌더에는 쓰지 않는다.
        // aim=true에서는 0mm도 사용자가 지정한 셀 고유 안 여백으로 존중한다.
        // [#2195 stage50] 표 기본 전축 0 = 미지정 → 셀 pad (Cell::table_padding_unspecified).
        let table_pad_unspec = !cell.apply_inner_margin
            && crate::model::table::Cell::table_padding_unspecified(&table.padding);
        let use_cell_left = (table_pad_unspec && cell.padding.left < 2500)
            || Self::should_use_cell_padding_axis_for_context(
                cell,
                cell.padding.left,
                table.padding.left,
                allow_saved_small_cell_margin,
            );
        let use_cell_right = (table_pad_unspec && cell.padding.right < 2500)
            || Self::should_use_cell_padding_axis_for_context(
                cell,
                cell.padding.right,
                table.padding.right,
                allow_saved_small_cell_margin,
            );
        let use_cell_top = (table_pad_unspec && cell.padding.top < 2500)
            || Self::should_use_cell_padding_axis_for_context(
                cell,
                cell.padding.top,
                table.padding.top,
                allow_saved_small_cell_margin,
            );
        let use_cell_bottom = (table_pad_unspec && cell.padding.bottom < 2500)
            || Self::should_use_cell_padding_axis_for_context(
                cell,
                cell.padding.bottom,
                table.padding.bottom,
                allow_saved_small_cell_margin,
            );

        let pad_left = if use_cell_left {
            hwpunit_to_px(cell.padding.left as i32, self.dpi)
        } else {
            hwpunit_to_px(table.padding.left as i32, self.dpi)
        };
        let pad_right = if use_cell_right {
            hwpunit_to_px(cell.padding.right as i32, self.dpi)
        } else {
            hwpunit_to_px(table.padding.right as i32, self.dpi)
        };
        let pad_top = if use_cell_top {
            hwpunit_to_px(cell.padding.top as i32, self.dpi)
        } else {
            hwpunit_to_px(table.padding.top as i32, self.dpi)
        };
        let pad_bottom = if use_cell_bottom {
            hwpunit_to_px(cell.padding.bottom as i32, self.dpi)
        } else {
            hwpunit_to_px(table.padding.bottom as i32, self.dpi)
        };
        // [Task #501] 한컴 방어 로직 모방 — cell.padding.top + bottom 합산이
        // cell.height 자체를 초과하면 (mel-001 p2 셀[21]: pad=1700 HU 두 축, h=1280 HU)
        // 한컴은 자체 가드로 cell 안에 콘텐츠가 들어가도록 처리. cell.height 의 절반까지
        // 비례 축소 (HWP 스펙 외 한컴 동작 모방).
        let (pad_top, pad_bottom) = if cell.height < 0x80000000 {
            let cell_h_px = hwpunit_to_px(cell.height as i32, self.dpi);
            let total_v_pad = pad_top + pad_bottom;
            if cell_h_px > 0.0 && total_v_pad >= cell_h_px {
                let max_v_pad = cell_h_px * 0.5;
                let scale = max_v_pad / total_v_pad;
                (pad_top * scale, pad_bottom * scale)
            } else {
                (pad_top, pad_bottom)
            }
        } else {
            (pad_top, pad_bottom)
        };
        (pad_left, pad_right, pad_top, pad_bottom)
    }

    fn should_use_cell_padding_axis_for_context(
        cell: &crate::model::table::Cell,
        cell_padding: i16,
        table_padding: i16,
        allow_saved_small_cell_margin: bool,
    ) -> bool {
        // [Task #1785] 규칙 본체는 Cell::use_cell_padding_axis 로 이동 — height_measurer
        // 와 단일 출처 공유 (규칙이 갈리면 예약 높이와 실제 렌더가 어긋난다).
        cell.use_cell_padding_axis(cell_padding, table_padding, allow_saved_small_cell_margin)
    }


    /// 표 수평 위치 결정
    pub(crate) fn compute_table_x_position(
        &self,
        table: &crate::model::table::Table,
        table_width: f64,
        col_area: &LayoutRect,
        depth: usize,
        host_alignment: Alignment,
        host_margin_left: f64,
        host_margin_right: f64,
        inline_x_override: Option<f64>,
        paper_width: Option<f64>,
    ) -> f64 {
        if let Some(ix) = inline_x_override {
            // inline_x_override: 外部(テキストフロー)で既に正しい位置が計算済み
            // TAC表のh_offsetはテキストフロー位置には不要 (非TAC表のみ加算)
            if table.common.treat_as_char {
                ix
            } else {
                let h_offset = hwpunit_to_px(table.common.horizontal_offset as i32, self.dpi);
                ix + h_offset
            }
        } else if depth == 0 && table.common.treat_as_char {
            // 글자처럼 취급(treat_as_char)
            // TAC 표의 위치는 텍스트 플로우에 의해 결정되므로 h_offset 미적용
            let ref_x = col_area.x + host_margin_left;
            let ref_w = col_area.width - host_margin_left - host_margin_right;
            match host_alignment {
                Alignment::Center | Alignment::Distribute => {
                    ref_x + (ref_w - table_width).max(0.0) / 2.0
                }
                Alignment::Right => ref_x + (ref_w - table_width).max(0.0),
                _ => ref_x,
            }
        } else if depth == 0 {
            // 표 자체 위치 속성
            let horz_rel_to = table.common.horz_rel_to;
            let horz_align = table.common.horz_align;
            let h_offset = hwpunit_to_px(table.common.horizontal_offset as i32, self.dpi);
            let (ref_x, ref_w) = match horz_rel_to {
                HorzRelTo::Paper => {
                    let paper_w = paper_width.unwrap_or({
                        // fallback: col_area 기반 추정 (paper_width 미전달 시)
                        if table_width > col_area.width {
                            col_area.x * 2.0 + table_width
                        } else {
                            col_area.x * 2.0 + col_area.width
                        }
                    });
                    (0.0, paper_w)
                }
                HorzRelTo::Page => {
                    // Task #347: 본문 영역(body_area) 기준. 미설정 시 col_area 폴백.
                    let body = self.current_body_area.get();
                    if body.2 > 0.0 {
                        (body.0, body.2)
                    } else {
                        (col_area.x, col_area.width)
                    }
                }
                HorzRelTo::Para => (
                    col_area.x + host_margin_left,
                    col_area.width - host_margin_left,
                ),
                _ => (col_area.x, col_area.width),
            };
            match horz_align {
                HorzAlign::Left | HorzAlign::Inside => ref_x + h_offset,
                HorzAlign::Center => ref_x + (ref_w - table_width).max(0.0) / 2.0 + h_offset,
                // Task #347: picture_footnote.rs:185와 동일하게 - h_offset (오른쪽 끝에서 안쪽으로 오프셋).
                HorzAlign::Right | HorzAlign::Outside => {
                    ref_x + (ref_w - table_width).max(0.0) - h_offset
                }
            }
        } else {
            // 중첩 표: outer_margin_left 적용 + host_alignment에 따라 셀 내에서 정렬
            let om_left = hwpunit_to_px(table.outer_margin_left as i32, self.dpi);
            let area_x = col_area.x + om_left;
            let area_w = (col_area.width - om_left).max(0.0);
            // [#3308/#3820] 비-TAC 중첩 표는 저장 폭을 유지하고, 부모 셀보다 좁으면
            // 저장 h_offset(편집기 대화상자 표시값)과 무관하게 셀 안 가운데에 배치한다.
            // 76076 p34의 near-fit 1×1 표도 이 계약을 따른다. 부모 폭으로의 확장은
            // PDF 줄바꿈·조각 높이를 바꾸므로 적용하지 않는다.
            if !table.common.treat_as_char
                && table_width
                    < area_w * crate::renderer::render_normalization::NESTED_STRETCH_MIN_RATIO
            {
                return area_x + (area_w - table_width).max(0.0) / 2.0;
            }
            match host_alignment {
                Alignment::Center | Alignment::Distribute => {
                    area_x + (area_w - table_width).max(0.0) / 2.0
                }
                Alignment::Right => area_x + (area_w - table_width).max(0.0),
                _ => area_x,
            }
        }
    }

    /// 표 세로 위치 결정 (text_wrap + v_offset + 캡션)
    fn compute_table_y_position(
        &self,
        table: &crate::model::table::Table,
        table_height: f64,
        y_start: f64,
        col_area: &LayoutRect,
        depth: usize,
        caption_height: f64,
        caption_spacing: f64,
        para_y: Option<f64>,
        allow_para_top_bleed: bool,
    ) -> f64 {
        let table_treat_as_char = table.common.treat_as_char;
        let table_text_wrap = if depth == 0 {
            table.common.text_wrap
        } else {
            crate::model::shape::TextWrap::Square
        };

        if depth == 0
            && !table_treat_as_char
            && matches!(
                table_text_wrap,
                crate::model::shape::TextWrap::TopAndBottom
                    | crate::model::shape::TextWrap::BehindText
                    | crate::model::shape::TextWrap::InFrontOfText
            )
        {
            // 자리차지(1) / 글뒤로(2) / 글앞으로(3): v_offset 기반 절대 위치

            let v_offset = hwpunit_to_px(table.common.vertical_offset as i32, self.dpi);
            // 문단 기준일 때 para_y 사용 (같은 문단의 여러 표가 동일 기준점 공유)
            let anchor_y = para_y.unwrap_or(y_start);
            // bit 13: VertRelTo가 'para'일 때 본문 영역으로 제한

            let page_h_approx = col_area.y * 2.0 + col_area.height;
            let vert_rel_to = table.common.vert_rel_to;
            // Task #297: Page는 본문 영역(body area) 기준, Paper는 용지 전체 기준
            // (HWP 스펙: Page=쪽 본문, Paper=용지 전체). 바탕쪽 문맥에서는
            // col_area = paper_area이므로 두 경로 결과가 동일하여 회귀 없음.
            let (ref_y, ref_h) = match vert_rel_to {
                crate::model::shape::VertRelTo::Page => {
                    // Task #347: 본문 영역(body_area) 기준. 미설정 시 col_area 폴백.
                    let body = self.current_body_area.get();
                    if body.3 > 0.0 {
                        (body.1, body.3)
                    } else {
                        (col_area.y, col_area.height)
                    }
                }
                crate::model::shape::VertRelTo::Para => {
                    (anchor_y, col_area.height - (anchor_y - col_area.y).max(0.0))
                }
                crate::model::shape::VertRelTo::Paper => (0.0, page_h_approx),
            };
            // Top 캡션: 표 위치를 캡션 높이만큼 아래로 이동
            let caption_top_offset = if let Some(ref cap) = table.caption {
                use crate::model::shape::CaptionDirection;
                if matches!(cap.direction, CaptionDirection::Top) {
                    caption_height
                        + if caption_height > 0.0 {
                            caption_spacing
                        } else {
                            0.0
                        }
                } else {
                    0.0
                }
            } else {
                0.0
            };
            let vert_align = table.common.vert_align;
            // [Task #898] Paper-relative 표는 v_offset 이 외곽 박스 (outer_margin 포함) 기준이므로
            // 가시 표 상단 = v_offset + outer_margin_top. 한컴 PDF (exam_math.hwp 바탕쪽 쪽번호 박스) 정합.
            let om_top_px = if matches!(vert_rel_to, crate::model::shape::VertRelTo::Paper) {
                hwpunit_to_px(table.outer_margin_top as i32, self.dpi)
            } else {
                0.0
            };
            let om_bottom_px = if matches!(vert_rel_to, crate::model::shape::VertRelTo::Paper) {
                hwpunit_to_px(table.outer_margin_bottom as i32, self.dpi)
            } else {
                0.0
            };
            let raw_y = match vert_align {
                crate::model::shape::VertAlign::Top | crate::model::shape::VertAlign::Inside => {
                    ref_y + v_offset + caption_top_offset + om_top_px
                }
                crate::model::shape::VertAlign::Center => {
                    ref_y + (ref_h - table_height) / 2.0 + v_offset + caption_top_offset
                }
                crate::model::shape::VertAlign::Bottom
                | crate::model::shape::VertAlign::Outside => {
                    ref_y + ref_h - table_height - v_offset + caption_top_offset - om_bottom_px
                }
            };
            // Para 기준 + bit 13: 본문 영역으로 제한
            // 앞선 표/텍스트가 차지한 영역(y_start) 아래로 밀어내고, 본문 영역 내로 클램핑
            // Task #347: TopAndBottom 만 y_start 이하로 밀어냄. 글뒤로(BehindText) /
            // 글앞으로(InFrontOfText) 표는 절대 위치 오버레이이므로 push-down 미적용.
            if matches!(vert_rel_to, crate::model::shape::VertRelTo::Para) {
                let body_top = col_area.y;
                let body_bottom = col_area.y + col_area.height - table_height;
                let declared_height = hwpunit_to_px(table.common.height as i32, self.dpi).max(0.0);
                let allow_rowbreak_object_bottom_bleed =
                    matches!(table.page_break, TablePageBreak::RowBreak)
                        && !table.common.treat_as_char
                        && table.row_count == 1
                        && table.col_count == 1
                        && table.cells.len() == 1
                        && signed_hwpunit(table.common.vertical_offset) <= 0
                        && declared_height > 0.0
                        && table_height
                            > declared_height + ROWBREAK_OBJECT_BOTTOM_BLEED_TOLERANCE_PX;
                let pushed =
                    if matches!(table_text_wrap, crate::model::shape::TextWrap::TopAndBottom) {
                        raw_y.max(y_start)
                    } else {
                        raw_y
                    };
                let min_y = if allow_para_top_bleed && v_offset < 0.0 {
                    body_top + v_offset
                } else {
                    body_top
                };
                // [#4514] 문단 기준 다행 RowBreak overlay(글앞/글뒤) 표는 상향 클램프를
                // 걸지 않는다. 앵커가 쪽 하단 부근이면 body_bottom 클램프가 표를 수백
                // px 위로 끌어올려 선행 표 위에 겹쳐 그렸다(8쪽: 880→491.4, 555.5px
                // 겹침 — 판독 불가). 한컴은 이 표를 쪽 경계에서 행 분할한다. 분할
                // 페인트 전 단계로, 앵커 위치를 보존하고 하단 bleed 는 쪽에서 잘리게
                // 둔다(겹침 해소가 우선). 1×1 장식 래퍼는 종전 클램프 유지.
                let overlay_multirow_rowbreak = matches!(
                    table_text_wrap,
                    crate::model::shape::TextWrap::InFrontOfText
                        | crate::model::shape::TextWrap::BehindText
                ) && table.row_count > 1
                    && matches!(table.page_break, TablePageBreak::RowBreak);
                if allow_rowbreak_object_bottom_bleed || overlay_multirow_rowbreak {
                    pushed.max(min_y)
                } else {
                    pushed.clamp(min_y, body_bottom.max(min_y))
                }
            } else {
                raw_y
            }
        } else if depth == 0 {
            let v_offset = if table_treat_as_char {
                hwpunit_to_px(table.common.vertical_offset as i32, self.dpi)
            } else {
                0.0
            };
            if let Some(ref caption) = table.caption {
                use crate::model::shape::CaptionDirection;
                if matches!(caption.direction, CaptionDirection::Top) {
                    y_start + caption_height + caption_spacing + v_offset
                } else {
                    y_start + v_offset
                }
            } else {
                y_start + v_offset
            }
        } else {
            // 중첩 표: outer_margin_top 적용
            let om_top = hwpunit_to_px(table.outer_margin_top as i32, self.dpi);
            y_start + om_top
        }
    }


    /// 각 셀 레이아웃 (배경, 패딩, 텍스트, 컨트롤, 테두리)
    #[allow(clippy::too_many_arguments)]
    fn layout_table_cells(
        &self,
        tree: &mut LayoutFrame,
        table_node: &mut RenderNode,
        table: &crate::model::table::Table,
        section_index: usize,
        styles: &ResolvedStyleSet,
        outline_numbering_id: u16,
        col_area: &LayoutRect,
        bin_data_content: &[BinDataContent],
        depth: usize,
        table_meta: Option<(usize, usize)>,
        outer_host_stored_vpos_hu: Option<i32>,
        enclosing_cell_ctx: Option<CellContext>,
        row_col_x: &[Vec<f64>],
        row_y: &[f64],
        independent_col_row_y: Option<&[Vec<f64>]>,
        col_count: usize,
        row_count: usize,
        table_x: f64,
        table_y: f64,
        h_edges: &mut Vec<Vec<Option<BorderLine>>>,
        v_edges: &mut Vec<Vec<Option<BorderLine>>>,
        row_filter: Option<(usize, usize)>,
        row_y_shift: f64,
        split_y_offset: f64,
        scalar_single_row_continuation: bool,
        single_row_continuation_offset: Option<f64>,
        single_row_fragment: bool,
        single_row_fragment_content_offset: Option<f64>,
        force_source_start_cut: bool,
        replay_terminal_boundary_unit: bool,
        split_terminal: bool,
        clamp_header_negative_para_offset: bool,
        inline_table_flow_y_shift: f64,
        nested_non_tac_cell_margin_compat: bool,
        cellzone_diagonal_origin_covered: &[Vec<bool>],
    ) {
        let mut independent_border_nodes: Vec<RenderNode> = Vec::new();
        for (cell_idx, cell) in table.cells.iter().enumerate() {
            let c = cell.col as usize;
            let r = cell.row as usize;
            if c >= col_count || r >= row_count {
                continue;
            }

            // 행 범위 필터: 보이는 행에 겹치지 않는 셀은 스킵
            let cell_end_row = (r + cell.row_span as usize).min(row_count);
            if let Some((sr, er)) = row_filter {
                if cell_end_row <= sr || r >= er {
                    continue;
                }
            }

            let cell_x = table_x + row_col_x[r][c];
            let cell_col_y = independent_col_row_y.and_then(|col_y| col_y.get(c));
            // row_y는 이미 시프트된 상태이므로 음수일 수 있음 (start_row 이전 행).
            // 독립 셀 높이가 있는 표는 해당 열의 누적 y를 사용한다.
            let raw_cell_y = table_y
                + cell_col_y
                    .and_then(|cy| cy.get(r).copied())
                    .unwrap_or(row_y[r]);
            let cell_y = if row_filter.is_some() {
                raw_cell_y.max(table_y)
            } else {
                raw_cell_y
            };
            let end_col = (c + cell.col_span as usize).min(col_count);
            let end_row = (r + cell.row_span as usize).min(row_count);
            let cell_w = row_col_x[r][end_col] - row_col_x[r][c];
            let raw_cell_h = cell_col_y
                .and_then(|cy| {
                    let start = cy.get(r).copied()?;
                    let end = cy.get(end_row).copied()?;
                    Some(end - start)
                })
                .unwrap_or_else(|| row_y[end_row] - row_y[r]);
            let cell_h = if row_filter.is_some() {
                // 클램프된 y에 맞게 높이도 조정
                (raw_cell_h - (cell_y - raw_cell_y)).max(0.0)
            } else {
                raw_cell_h
            };
            let content_cell_y = if row_filter.is_some() {
                cell_y - split_y_offset
            } else {
                cell_y
            };

            let cell_id = tree.next_id();
            let mut cell_node = RenderNode::new(
                cell_id,
                RenderNodeType::TableCell(TableCellNode {
                    col: cell.col,
                    row: cell.row,
                    col_span: cell.col_span,
                    row_span: cell.row_span,
                    border_fill_id: cell.border_fill_id,
                    text_direction: cell.text_direction,
                    clip: true,
                    model_cell_index: Some(cell_idx as u32),
                }),
                BoundingBox::new(cell_x, cell_y, cell_w, cell_h),
            );

            // 셀 BorderFill 조회
            let border_style = if cell.border_fill_id > 0 {
                let idx = (cell.border_fill_id as usize).saturating_sub(1);
                styles.border_styles.get(idx)
            } else {
                None
            };

            // (a) 셀 배경
            self.render_cell_background(
                tree,
                &mut cell_node,
                border_style,
                cell_x,
                cell_y,
                cell_w,
                cell_h,
                bin_data_content,
            );

            // 셀 패딩 (cell.padding이 0이면 table.padding fallback)
            let (pad_left, pad_right, pad_top, pad_bottom) = self.resolve_cell_padding_for_context(
                cell,
                table,
                nested_non_tac_cell_margin_compat,
            );

            let mut composed_paras: Vec<_> = cell
                .paragraphs
                .iter()
                .map(|p| compose_paragraph(p))
                .collect();

            // [Task #1073] 중첩 표 분할 연속 페이지(row_filter sr>0)에서 분할 시작 행보다
            // 먼저 시작한 rowspan 셀(r < sr)은 라벨이 이전 페이지에 이미 렌더됨 → 연속
            // 페이지에선 공란(영역/배경만, 텍스트 미렌더). 외부 표 advance_row_block_cut 의
            // rs>1 라벨 공란 정합. row_filter 는 중첩 표 분할 전용(외부 표는 별도 경로).
            if let Some((sr, _)) = row_filter {
                if sr > 0 && r < sr {
                    composed_paras.clear();
                }
            }

            let inner_x = cell_x + pad_left;
            let inner_width = (cell_w - pad_left - pad_right).max(0.0);
            let inner_height = (cell_h - pad_top - pad_bottom).max(0.0);

            // [Task #671] line_segs 비어 있는 셀 paragraph 의 단일 ComposedLine 압축
            // 결과를 셀 가용 너비 (inner_width) 에 맞춰 다중 ComposedLine 으로 재분할.
            // 한컴이 PARA_LINE_SEG 를 인코딩하지 않은 케이스 (samples/계획서.hwp) 의
            // 줄겹침 시각 결함 정정. 정상 line_segs 인코딩된 paragraph 는 무영향.
            for (cpi, para) in cell.paragraphs.iter().enumerate() {
                if let Some(comp) = composed_paras.get_mut(cpi) {
                    crate::renderer::composer::recompose_for_cell_width(
                        comp,
                        para,
                        inner_width,
                        styles,
                    );
                    // [cell-cold-load-overflow-recompose] #2291의 "부실 저장(ls==1인데
                    // 실폭 초과) 문단 재분할" 안전장치가 cell_units_uncached(측정)와
                    // layout_partial_table_cells(분할 연속 페이지)에는 이미 있었지만,
                    // 이 함수(단일 페이지·비분할 표의 최초 렌더 경로)에는 빠져 있었다 —
                    // 그래서 authoring 시점엔 짧았던 값이 값 교체로 훨씬 길어져도(예:
                    // 템플릿 필드 채움) 여기서는 절대 재래핑되지 않고 그대로 잘려
                    // 렌더됐다. 다른 두 경로와 동일한 호출(×1.8 임계, 메모 공유)로
                    // 통일한다 — ×1.8보다 낮은 전용 임계를 이 호출부에만 쓰는 실험은
                    // `tests/overflow_cell_baseline.rs`(샘플 전수 래칫)로 기각됐다:
                    // `issue2559/1341000_research_report_footnotes.hwp`에서 87줄이
                    // 새로 쪽 밖으로 밀려났다(#3236 계열, 실측 확인). ×1.8은 "임의로
                    // 보수적"이 아니라 이 저장소의 실제 문서 코퍼스로 검증된 값이다.
                    if cell.text_direction == 0 {
                        crate::renderer::composer::recompose_stored_single_line_if_overflowing(
                            comp,
                            para,
                            inner_width,
                            styles,
                        );
                    }
                }
            }

            // AutoNumber(Page) 치환: 셀 내 쪽번호 필드를 현재 페이지 번호로 변환
            let current_pn = self.current_page_number.get();
            if current_pn > 0 {
                for (cpi, para) in cell.paragraphs.iter().enumerate() {
                    if para.controls.iter().any(|c| {
                        matches!(c, Control::AutoNumber(an)
                            if an.number_type == crate::model::control::AutoNumberType::Page)
                    }) {
                        if let Some(comp) = composed_paras.get_mut(cpi) {
                            self.substitute_page_auto_numbers_in_composed(para, comp, current_pn);
                        }
                    }
                }
            }

            // AutoNumber(TotalPage) 치환: 셀 내 총쪽수 필드를 문서 전체 쪽수로 변환.
            // exam_eng.hwp 같은 시험지의 꼬리말 쪽번호 상자는 현재쪽/총쪽수 두 atno를
            // 같은 셀 안에 서로 다른 문단으로 둔다 — Page만 치환하면 총쪽수 자리에도
            // 현재 쪽번호가 그려진다 (Task: 꼬리말 총쪽수 필드 미치환 버그).
            let total_pages = self.total_pages.get();
            if total_pages > 0 {
                for (cpi, para) in cell.paragraphs.iter().enumerate() {
                    if para.controls.iter().any(|c| {
                        matches!(c, Control::AutoNumber(an)
                            if an.number_type == crate::model::control::AutoNumberType::TotalPage)
                    }) {
                        if let Some(comp) = composed_paras.get_mut(cpi) {
                            self.substitute_total_page_auto_numbers_in_composed(
                                para,
                                comp,
                                total_pages,
                            );
                        }
                    }
                }
            }

            // 인라인 이미지/도형 최대 높이
            let mut max_inline_height: f64 = 0.0;

            // 수직 정렬용 콘텐츠 높이
            // (A) composed 기반: LINE_SEG line_height 합산 + 비인라인 도형/그림
            let total_content_height: f64 = {
                let mut text_height: f64 = self.calc_composed_paras_content_height(
                    &composed_paras,
                    &cell.paragraphs,
                    styles,
                );
                for para in &cell.paragraphs {
                    text_height +=
                        self.paragraph_top_and_bottom_non_inline_flow_height(&para.controls);
                    for ctrl in &para.controls {
                        match ctrl {
                            Control::Picture(pic) => {
                                let pic_h = hwpunit_to_px(pic.common.height as i32, self.dpi);
                                if pic.common.treat_as_char {
                                    if pic_h > max_inline_height {
                                        max_inline_height = pic_h;
                                    }
                                }
                            }
                            Control::Shape(shape) => {
                                let shape_h = hwpunit_to_px(shape.common().height as i32, self.dpi);
                                if shape.common().treat_as_char {
                                    if shape_h > max_inline_height {
                                        max_inline_height = shape_h;
                                    }
                                }
                            }
                            Control::Equation(eq) => {
                                let eq_h = hwpunit_to_px(eq.common.height as i32, self.dpi);
                                if eq.common.treat_as_char {
                                    if eq_h > max_inline_height {
                                        max_inline_height = eq_h;
                                    }
                                } else {
                                    text_height += eq_h;
                                }
                            }
                            // [Task #1658] 중첩 표 높이를 composed(text_height)에 가산하지 않는다.
                            // 가산하면 stored vpos(last_seg_end, nested 포함) 및 아래 nested_bottom
                            // 과 double-count 되어 total_content_height 가 ~2× 과대 → Center/Bottom
                            // offset≈0 → 상단정렬(valign over-count, kkyu8925 제보). 중첩 표 기여는
                            // final max 의 vpos_height(B)·nested_bottom 이 담당하며, composed 의
                            // line_height 가 중첩을 반영하는 케이스는 composed 가, 미반영(과소)
                            // 케이스는 nested_bottom 이 max 로 보정한다(#44 under-count 가드 보존).
                            Control::Table(_) => {}
                            _ => {}
                        }
                    }
                }
                let composed_height = text_height.max(max_inline_height);

                // (B) vpos 기반: 마지막 문단의 vpos_end + 중첩 표 보정
                // LINE_SEG lh에 중첩 표 높이가 미반영된 경우를 보정
                let vpos_height = if cell.paragraphs.len() > 1 {
                    let last_para = cell.paragraphs.last().unwrap();
                    if let Some(seg) = last_para.line_segs.last() {
                        let mut last_end = seg.vertical_pos + seg.line_height;
                        // 마지막 문단에 중첩 표가 있고 lh가 표 높이보다 작으면 보정
                        for ctrl in &last_para.controls {
                            if let Control::Table(t) = ctrl {
                                let table_h = t.common.height as i32;
                                if table_h > seg.line_height {
                                    last_end += table_h - seg.line_height;
                                }
                            }
                        }
                        hwpunit_to_px(last_end, self.dpi)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                let nested_bottom =
                    self.calc_nested_controls_bottom_height(&cell.paragraphs, styles);
                let wrap_object_bottom =
                    self.calc_cell_wrap_objects_bottom_height(&cell.paragraphs);
                composed_height
                    .max(vpos_height)
                    .max(nested_bottom)
                    .max(wrap_object_bottom)
            };

            // 수직 정렬 (분할 표에서는 Top 강제 — 보이는 영역이 전체 셀보다 작음)
            // 중첩 표 행 범위 부분 렌더에서, **셀이 실제로 잘릴 때만** Top 을 강제한다.
            //
            // `row_filter` 는 행 단위로 자르므로 필터 안에 온전히 들어가는 셀은 잘리지
            // 않는다. 그런 셀까지 Top 으로 덮으면 세로로 긴 병합 라벨이 정중앙이 아니라
            // 맨 위에 붙는다 (한컴 pdf/kps-ai-2022.pdf p65 실측 = 정중앙. rhwp p66 은
            // 상단). 실측: kps-ai 의 Center 지정 셀 57건 중 55건이 안 잘리는데도 Top
            // 강제를 받았다.
            //
            // 잘리는 두 경우는 이 조건과 무관하게 결과가 Top 으로 수렴한다 —
            // 상단 잘림(r < sr)은 라벨이 앞 페이지에 이미 렌더돼 문단을 비우고(아래 #1073
            // 처리), 하단 잘림은 콘텐츠가 가시 높이를 넘어 정렬 오프셋이 0 으로 클램프된다.
            // 그래도 종전 동작을 그 두 경우에 한해 그대로 남긴다.
            let cell_clipped_by_row_filter = row_filter.is_some_and(|(sr, er)| {
                let cell_end_row = (r + cell.row_span as usize).min(row_count);
                r < sr || cell_end_row > er
            });
            // 이 표 자신은 `nested_split`을 받지 않아도, 현재 표가 부모 1×1
            // RowBreak continuation의 남은 viewport 안에서 호출될 수 있다. 이때 큰
            // 하위 셀은 부모 Cell clip에 의해 위나 아래가 잘린다. 원래 Center/Bottom
            // 정렬을 유지하면 잘린 반대쪽의 보이지 않는 공간을 기준으로 본문이 다시
            // 밀린다. 특히 위쪽만 잘린 p11에서는 앞 쪽에 이미 그린 문단군을 다시
            // 현재 페이지로 끌어내려 중복했다(42065). `col_area`는 호출자가 넘긴
            // 물리 viewport이므로, 그와 교차하면서 한쪽이라도 벗어난 중첩 셀만
            // Top으로 수렴시킨다. 다만 p11처럼 호출자가 전한 `col_area`가 직전
            // 조각까지 포함할 수 있으므로, 실제 페이지 viewport에서도 같은 판정을
            // 한다. 일반 완전 셀 및 최상위 표(depth=0)는 영향이 없다.
            let parent_view_top = col_area.y;
            let parent_view_bottom = col_area.y + col_area.height;
            let cell_intersects_parent_viewport =
                cell_y < parent_view_bottom - 0.5 && cell_y + cell_h > parent_view_top + 0.5;
            let cell_clipped_by_parent_viewport = depth > 0
                && !table.common.treat_as_char
                && col_area.height > 0.5
                && cell_intersects_parent_viewport
                && (cell_y < parent_view_top - 0.5 || cell_y + cell_h > parent_view_bottom + 0.5);
            // nested continuation은 부모 `col_area`가 이전 페이지의 logical
            // viewport를 포함한 채 호출될 수 있다. 렌더 트리의 page bbox는 실제
            // SVG/Canvas clip이므로, 그 밖으로 나간 셀은 그 logical viewport 안에
            // 있더라도 Center/Bottom 기준으로 배치하면 안 된다.
            let page_bbox = tree.page_bbox();
            let page_view_top = page_bbox.y;
            let page_view_bottom = page_bbox.y + page_bbox.height;
            let cell_intersects_page_viewport =
                cell_y < page_view_bottom - 0.5 && cell_y + cell_h > page_view_top + 0.5;
            let cell_clipped_by_page_viewport = depth > 0
                && !table.common.treat_as_char
                && cell_intersects_page_viewport
                && (cell_y < page_view_top - 0.5 || cell_y + cell_h > page_view_bottom + 0.5);
            // 위쪽 continuation에서는 source의 첫 가시 줄이 page clip 직전까지
            // 내려와 있다. Top을 셀의 논리 원점에 그대로 맞추면 그 줄의 잉크가
            // clip 바로 위에서 잘리고 다음 줄부터 나타난다(42065 p11: PDF의
            // "행하여야 하며 …"가 사라지고 "제50조의4"부터 시작). 첫 *유효*
            // 저장 줄의 물리 line-height만큼 예약해 그 줄을 clip 안으로 되돌린다.
            // HWP는 표 안의 빈 anchor line을 line_height=0으로 먼저 저장할 수 있어
            // 단순 first()를 쓰면 이 보정이 무효가 된다. 아래쪽 잘림이나 정상 완전
            // 셀에는 적용하지 않는다.
            let upper_page_clip_line_reservation =
                if cell_clipped_by_page_viewport && cell_y < page_view_top - 0.5 {
                    cell.paragraphs
                        .iter()
                        .flat_map(|para| para.line_segs.iter())
                        .find(|seg| seg.line_height > 0)
                        .map(|seg| hwpunit_to_px(seg.line_height, self.dpi))
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
            let effective_valign = if cell_clipped_by_row_filter
                || scalar_single_row_continuation
                || cell_clipped_by_parent_viewport
                || cell_clipped_by_page_viewport
            {
                VerticalAlign::Top
            } else {
                cell.vertical_align
            };
            // Task #347: HWP는 LineSeg.vertical_pos에 첫 줄의 절대 위치(셀 내부 컨텐츠 상단부터)
            // 를 기록한다. 다만 이 값을 모든 vertical_align에 곧바로 적용하면 Center/Bottom
            // 지정 셀도 Top처럼 배치된다. vpos 앵커링은 Top 셀의 세부 줄 위치 보정으로만
            // 사용하고, Center/Bottom은 전체 콘텐츠 높이 기반의 기존 정렬 계산을 유지한다.
            // 단, line_segs가 비어있는 Top 케이스는 기존 폴백 유지.
            // [Task #362] 셀 안에 nested table 이 있는 경우 vpos 적용 제외.
            // nested table 케이스에서 LineSeg.vpos 가 셀 콘텐츠 시작 오프셋 의미가 아니라
            // 셀 안의 누적 위치로 사용되어, vpos 를 추가하면 콘텐츠가 표 높이를 초과하여 클립 발생.
            // (kps-ai p56 case: 외부 셀 vpos=2000HU 가 추가되어 19.5px 클립.)
            let has_nested_table = cell
                .paragraphs
                .iter()
                .any(|p| p.controls.iter().any(|c| matches!(c, Control::Table(_))));
            // HWPX block-TAC 셀의 nested table은 예외다. 이 형상은 모든 문단의
            // 연속된 LineSeg.vpos가 셀 기준 좌표를 보존하며, 이를 무시하면
            // Center 정렬의 순차 flow가 누적되어 실제로 fit하는 하위 표가 다음
            // 쪽으로 clip된다 (#3820 production HWPX p144). 아래의 stored-flow
            // 신뢰 조건(연속 anchor, extent, 비-flow 객체)을 통과한 경우에만 허용해
            // Task #362의 일반 nested-table 누적-vpos 차단은 유지한다.
            let hwpx_noninline_tac_nested_stored_flow = self.profile.get().hwpx_stored_layout()
                && table.common.treat_as_char
                && !table.common.flow_with_text
                && matches!(table.page_break, TablePageBreak::None)
                && has_nested_table;
            // [cell-cold-load-overflow-recompose 후속] 어떤 문단이든 부실 저장 1-lineseg
            // (overflow-recompose 대상, composer::recompose_stored_single_line_if_overflowing
            // 참고)이었고 실제로 재래핑돼 composed 줄 수가 1보다 커졌다면, 그 문단의 저장
            // LINE_SEG(vertical_pos/line_height)는 authoring 시점 "1줄" 가정의 잔재라 더 이상
            // 신뢰할 수 없다 — composed 는 여러 줄인데 저장 seg 는 하나뿐이라 vpos/line_height
            // 둘 다 실제 배치와 무관해진다. 이 값을 그대로 쓰면(첫 줄 top 앵커든, 아래
            // stored_flow_extent 기반 정렬 오프셋이든) 텍스트 시작 y 가 실제보다 아래로
            // 밀리는데, 셀 clip rect 는 (정상적으로 커진) 행높이 그대로 셀 진짜 상단에
            // 고정돼 있어 마지막 줄이 clip 밖으로 밀려나 소실된다(scslic.hwpx 제품명 필드
            // 3줄 중 3번째 미출력 — 2026-08-25 실측, Center 정렬 셀에서
            // trust_stored_cell_flow 경로로 재현). 재래핑되지 않은 문단(정상 저장 다중 줄,
            // 또는 재래핑 임계 미달로 그대로인 1줄)은 종전대로 저장 지오메트리를 신뢰한다.
            let para_overflow_recomposed = |idx: usize, p: &Paragraph| {
                p.line_segs.len() == 1 && composed_paras.get(idx).is_some_and(|c| c.lines.len() > 1)
            };
            let first_para_overflow_recomposed = cell
                .paragraphs
                .first()
                .is_some_and(|p| para_overflow_recomposed(0, p));
            let any_para_overflow_recomposed = cell
                .paragraphs
                .iter()
                .enumerate()
                .any(|(idx, p)| para_overflow_recomposed(idx, p));
            let first_line_vpos = if first_para_overflow_recomposed {
                None
            } else {
                cell.paragraphs
                    .first()
                    .and_then(|p| p.line_segs.first())
                    .map(|ls| hwpunit_to_px(ls.vertical_pos, self.dpi))
            };
            // [Task #2211] 저장 LINE_SEG 흐름 extent(각 seg 의 vpos+lh 최댓값)가
            // 자체 스택 합(total_content_height)보다 작으면 — 예: 악보 셀처럼
            // 빈 앵커 줄이 TopAndBottom 그림 높이에 흡수된 문서 — 한컴 저장
            // 지오메트리를 신뢰한다: 정렬 기준 콘텐츠 높이를 저장 extent 로
            // 바꾸고, 문단 배치도 저장 vpos 스냅을 강제한다 (한컴 실측:
            // 가사 top = 셀 top + pad + 센터 오프셋(저장 extent 기준) + vpos).
            let (stored_flow_extent, stored_flow_line_sum) = if (!has_nested_table
                || hwpx_noninline_tac_nested_stored_flow)
                && !any_para_overflow_recomposed
                && !cell.paragraphs.is_empty()
                && cell.paragraphs.iter().all(|p| !p.line_segs.is_empty())
            {
                cell.paragraphs
                    .iter()
                    .flat_map(|p| p.line_segs.iter())
                    .filter(|s| s.vertical_pos >= 0 && s.line_height > 0)
                    .map(|s| {
                        (
                            hwpunit_to_px(s.vertical_pos + s.line_height, self.dpi),
                            hwpunit_to_px(s.line_height, self.dpi),
                        )
                    })
                    .fold((0.0f64, 0.0f64), |(ext, sum), (e, h)| (ext.max(e), sum + h))
            } else {
                (0.0, 0.0)
            };
            // Square/중첩 표 등 비-flow 개체의 시각 bottom 은 저장 LINE_SEG 흐름에
            // 포함되지 않으므로(#1486 p19 Square 그림), 그런 개체가 저장 extent 를
            // 넘는 셀은 저장 흐름 신뢰 대상이 아니다 — TopAndBottom flow 개체만
            // 저장 vpos 에 흡수된다(악보 셀).
            let non_flow_object_extent = self
                .calc_nested_controls_bottom_height(&cell.paragraphs, styles)
                .max(self.calc_cell_wrap_objects_bottom_height(&cell.paragraphs));
            // [#2148 #2279] 저장 vpos 흐름이 물리적으로 줄들을 담지 못하는 퇴화
            // 형상(다문단 전부 vpos=0 등, 36399374 pi=79 병합 셀: extent 35px vs
            // 줄높이 합 260px)은 신뢰 대상이 아니다 — 전 문단이 셀 상단 한 y 에
            // 겹쳐 그려진다(한글은 fresh 재적층). 음수 line_spacing 누적 보정용
            // 정상 vpos 스냅(조직도형·악보 셀)은 extent ≈ 줄높이 합이므로 0.5
            // 비율 가드에 걸리지 않는다.
            // 위 비율 가드는 문단이 2개인 셀에서 경계에 정확히 걸려 통과한다
            // (전 문단 vpos=0, lh 동일 → extent = 줄높이합/2). 그래서 문단 단위
            // 앵커 유무를 직접 본다: 둘째 이후 문단의 first seg vpos == 0 은
            // "앵커 없음" 센티널이므로, 그런 문단이 있으면 저장 흐름은 문단 위치를
            // 구분해 담고 있지 않다. 이때 extent 를 콘텐츠 높이로 받아들이면 세로
            // 정렬 오프셋과 담을 줄 수가 1줄분으로 굳어 뒤 문단이 셀 밖으로 밀려
            // 잘리거나 아예 렌더되지 않는다. 배치 쪽 first_seg_vpos_is_anchor 와
            // 같은 규약을 측정에도 적용한다.
            let stored_flow_has_para_anchors = cell
                .paragraphs
                .iter()
                .enumerate()
                .all(|(idx, para)| crate::renderer::first_seg_vpos_is_anchor(para, idx));
            let stored_flow_shape_is_trusted = (depth > 0 || table.common.treat_as_char)
                && stored_flow_extent > 0.0
                && non_flow_object_extent <= stored_flow_extent + 0.5
                && stored_flow_extent + 0.5 >= 0.5 * stored_flow_line_sum
                && stored_flow_has_para_anchors;
            // 일반 셀은 저장 extent가 자체 측정값보다 실제로 압축된 경우에만
            // anchor를 신뢰한다. 다만 위의 좁은 HWPX block-TAC nested-table 형상은
            // extent와 자체 측정값이 같아도 문단별 vpos가 하위 표의 실제 위치를
            // 담고 있다. 이 경우에는 total height는 변하지 않지만 순차 배치만
            // 저장 anchor로 복원해야 한다 (#3820 p144).
            let trust_stored_cell_flow = stored_flow_shape_is_trusted
                && (stored_flow_extent + 0.5 < total_content_height
                    || (hwpx_noninline_tac_nested_stored_flow
                        && (stored_flow_extent - total_content_height).abs() <= 0.5));
            let total_content_height = if trust_stored_cell_flow {
                stored_flow_extent
            } else {
                total_content_height
            };
            let use_top_vpos_anchor = matches!(effective_valign, VerticalAlign::Top);
            let text_y_start = if use_top_vpos_anchor
                && !has_nested_table
                && first_line_vpos.filter(|&v| v > 0.0).is_some()
            {
                // vpos는 셀 컨텐츠 상단(=cell_y+pad_top)으로부터의 첫 줄 top y 오프셋
                content_cell_y + pad_top + first_line_vpos.unwrap()
            } else {
                match effective_valign {
                    VerticalAlign::Top => content_cell_y + pad_top,
                    VerticalAlign::Center => {
                        let mechanical_offset =
                            (inner_height - total_content_height).max(0.0) / 2.0;
                        content_cell_y + pad_top + mechanical_offset
                    }
                    VerticalAlign::Bottom => {
                        content_cell_y + pad_top + (inner_height - total_content_height).max(0.0)
                    }
                }
            };
            let text_y_start = text_y_start + upper_page_clip_line_reservation;
            // 세로쓰기 셀
            if cell.text_direction != 0 {
                let vert_inner_area = LayoutRect {
                    x: inner_x,
                    y: content_cell_y + pad_top,
                    width: inner_width,
                    height: inner_height,
                };
                self.layout_vertical_cell_text(
                    tree,
                    &mut cell_node,
                    &composed_paras,
                    &cell.paragraphs,
                    styles,
                    &vert_inner_area,
                    cell.vertical_align,
                    cell.text_direction,
                    section_index,
                    table_meta,
                    cell_idx,
                    enclosing_cell_ctx.clone(),
                );
            } else {
                self.layout_horizontal_cell_paragraphs(
                    tree,
                    table_node,
                    &mut cell_node,
                    cell,
                    &composed_paras,
                    table,
                    styles,
                    bin_data_content,
                    table_meta,
                    &enclosing_cell_ctx,
                    row_filter,
                    row_y,
                    effective_valign,
                    HorizontalCellVars {
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
                        upper_clip_line_reservation: upper_page_clip_line_reservation,
                        trust_stored_cell_flow,
                        has_nested_table,
                        section_index,
                        outline_numbering_id,
                        depth,
                        clamp_header_negative_para_offset,
                        outer_host_stored_vpos_hu,
                        inline_table_flow_y_shift,
                        single_row_continuation: scalar_single_row_continuation,
                        single_row_continuation_offset,
                        single_row_fragment,
                        single_row_fragment_content_offset,
                        force_source_start_cut,
                        replay_terminal_boundary_unit,
                        split_terminal,
                    },
                );
            } // else (가로쓰기)

            // 셀 내 각주 참조 번호 윗첨자
            for para in &cell.paragraphs {
                self.add_footnote_superscripts(tree, &mut cell_node, para, styles);
            }

            // (b) 셀 테두리를 수집한다. 열별 높이가 다른 표는 row_y 격자로
            // 테두리를 그릴 수 없으므로 셀 bbox 기준 라인을 별도로 생성한다.
            if let Some(bs) = border_style {
                if independent_col_row_y.is_some() {
                    independent_border_nodes.extend(render_cell_box_borders(
                        tree, bs, cell_x, cell_y, cell_w, cell_h,
                    ));
                } else {
                    collect_cell_borders(
                        h_edges,
                        v_edges,
                        c,
                        r,
                        cell.col_span as usize,
                        cell.row_span as usize,
                        &bs.borders,
                    );
                }
            }

            table_node.children.push(cell_node);

            // (c) 셀 대각선 렌더링 (셀 콘텐츠 위에 그림)
            let suppress_cell_diagonal = cell_span_has_cellzone_diagonal(
                cellzone_diagonal_origin_covered,
                r,
                c,
                cell.row_span as usize,
                cell.col_span as usize,
                row_count,
                col_count,
            );
            if let Some(bs) = border_style {
                if !suppress_cell_diagonal || border_style_has_center_line_only(bs) {
                    table_node.children.extend(render_cell_diagonal(
                        tree, bs, cell_x, cell_y, cell_w, cell_h,
                    ));
                }
            }
        }
        if !independent_border_nodes.is_empty() {
            table_node.children.extend(independent_border_nodes);
        }
    }

    pub(crate) fn calc_cell_controls_height(
        &self,
        cell: &crate::model::table::Cell,
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let measurer = super::super::height_measurer::HeightMeasurer::new(self.dpi)
            .with_hwp3_variant(self.profile.get().hwp3_layout())
            .with_native_hwp5(self.profile.get().native_hwp5_layout())
            .with_render_normalization(self.render_normalization_overlay());
        measurer.cell_controls_height(&cell.paragraphs, styles, 0, 0.0)
    }

    /// 중첩 표의 총 높이를 계산한다 (행 높이 합 + cell_spacing).
    /// MeasuredCell.line_heights에서 중첩 표가 추가 줄로 포함될 때의 높이와 일관되게 계산.
    pub(crate) fn calc_nested_table_height(
        &self,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
    ) -> f64 {
        let col_count = table.col_count as usize;
        let row_count = table.row_count as usize;
        let row_heights = self.resolve_row_heights(table, col_count, row_count, None, styles, true);
        let cell_spacing = hwpunit_to_px(table.cell_spacing as i32, self.dpi);
        let om_top = hwpunit_to_px(table.outer_margin_top as i32, self.dpi);
        let om_bottom = hwpunit_to_px(table.outer_margin_bottom as i32, self.dpi);
        row_heights.iter().sum::<f64>()
            + cell_spacing * (row_count.saturating_sub(1) as f64)
            + om_top
            + om_bottom
    }

    /// 셀 내 중첩 표가 실제로 차지하는 하단 위치를 계산한다.
    ///
    /// 일부 HWP/HWPX는 중첩 표 문단의 LINE_SEG.line_height에 내부 표의 실제
    /// 높이를 반영하지 않는다. 렌더링/측정은 해당 문단의 vertical_pos에 중첩 표
    /// 측정 높이를 더한 값을 셀 콘텐츠 끝점 후보로 사용한다.
    pub(crate) fn calc_nested_controls_bottom_height(
        &self,
        paragraphs: &[Paragraph],
        styles: &ResolvedStyleSet,
    ) -> f64 {
        // [#4533] `para_top + nested_h` 는 "중첩 표가 앵커 문단 아래로 흐른다"는
        // 가정이다. 앵커 줄이 셀 하단에 있고 표가 셀 상단에 절대배치되는 서식
        // 문서(기장군 20420347: para_top 740.9 + 601.8 = 1342.6 vs 선언 794.1)
        // 에서는 이 가정이 셀을 548px 부풀려 후속 문단을 쪽 밖으로 민다.
        // 호스트 **뒤에** 저장 사다리가 이어지면(뒤 문단 저장 vpos 존재) 그
        // 사다리가 흐름-표 높이까지 이미 증명하므로 사다리 끝점으로 캡한다.
        // 호스트가 마지막 문단이면 기존 휴리스틱 유지(lh 미반영 문서의 원 목적).
        let native_hwp5 = self.profile.get().native_hwp5_layout();
        let ladder_end: f64 = if native_hwp5
            && paragraphs
                .iter()
                .all(|p| !crate::renderer::para_has_no_stored_line_segs(p))
        {
            paragraphs
                .iter()
                .flat_map(|p| p.line_segs.iter())
                .map(|s| hwpunit_to_px(s.vertical_pos + s.line_height, self.dpi))
                .fold(0.0f64, f64::max)
        } else {
            0.0
        };
        paragraphs
            .iter()
            .enumerate()
            .map(|(pidx, p)| {
                let nested_h: f64 = p
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
                if nested_h <= 0.0 {
                    0.0
                } else {
                    let para_top = p
                        .line_segs
                        .first()
                        .map(|s| hwpunit_to_px(s.vertical_pos, self.dpi))
                        .unwrap_or(0.0);
                    let candidate = para_top + nested_h;
                    // 절대배치의 직접 증거는 "표의 공간이 호스트 줄 **위**에 이미
                    // 예약됨"이다 — 직전 저장 줄 끝→호스트 vpos 갭이 표 높이만큼
                    // 벌어진다(기장군 612 vs 표 598 · 수면 작성례 563 vs 536.5 —
                    // 호스트가 셀 마지막 문단인 변형도 같은 식으로 갈린다). 흐름형
                    // (lh 미흡수 포함)은 직전 갭이 평범한 줄간격이라 배제되고,
                    // 호스트가 셀 첫 문단이면(49308 조각 셀) 직전 줄이 없어 배제된다.
                    let prev_end = paragraphs
                        .iter()
                        .take(pidx)
                        .flat_map(|prev| prev.line_segs.iter())
                        .map(|s| hwpunit_to_px(s.vertical_pos + s.line_height, self.dpi))
                        .filter(|&e| e <= para_top + 0.5)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let gap_before = para_top - prev_end;
                    // 쪽을 넘는 거대 중첩 표는 셀 사다리가 조각-국소라 표를
                    // 기술하지 못한다(49308: nested_h 2664 vs ladder_end 122 —
                    // 캡하면 쪽수 70->69 로 한글 71쪽에서 멀어짐). 표가 사다리
                    // 안에 들어갈 때만 절대배치 캡을 허용한다.
                    let anchored_not_flowing = ladder_end > 0.0
                        && nested_h <= ladder_end
                        && prev_end.is_finite()
                        && gap_before >= nested_h * 0.85;
                    if anchored_not_flowing {
                        candidate.min(ladder_end)
                    } else {
                        candidate
                    }
                }
            })
            .fold(0.0f64, f64::max)
    }

    /// 셀의 content_offset 이후 실제 남은 콘텐츠 높이를 계산한다.
    /// MeasuredCell과 동일한 높이 로직을 사용한다 (pagination 엔진이 MeasuredCell 기준으로
    /// content_offset을 산출하므로 동일 기준이어야 함).
    pub(crate) fn calc_cell_remaining_content_height(
        &self,
        cell: &crate::model::table::Cell,
        styles: &ResolvedStyleSet,
        content_offset: f64,
    ) -> f64 {
        // MeasuredCell과 동일한 높이 계산:
        // 각 줄 h+ls, 단 셀의 마지막 줄(마지막 문단의 마지막 줄)은 ls 제외
        let mut total = 0.0;
        let cell_para_count = cell.paragraphs.len();
        for (pidx, p) in cell.paragraphs.iter().enumerate() {
            let comp = compose_paragraph(p);
            let para_style = styles.para_styles.get(p.para_shape_id as usize);
            let is_last_para = pidx + 1 == cell_para_count;
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
            if comp.lines.is_empty() {
                // 중첩 표 컨트롤 문단: 실제 중첩 표 높이로 계산
                let nested_h: f64 = p
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
                let h = if nested_h > 0.0 {
                    nested_h
                } else {
                    hwpunit_to_px(400, self.dpi)
                };
                total += spacing_before + h + spacing_after;
            } else {
                // 중첩 표가 있는 문단: LINE_SEG 높이와 실제 중첩 표 높이 중 큰 값 사용
                let has_table_in_para = p.controls.iter().any(|c| matches!(c, Control::Table(_)));
                let line_count = comp.lines.len();
                let line_based_h: f64 = comp
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(li, line)| {
                        let h = hwpunit_to_px(line.line_height, self.dpi);
                        let is_cell_last_line = is_last_para && li + 1 == line_count;
                        let ls = if !is_cell_last_line {
                            hwpunit_to_px(line.line_spacing, self.dpi)
                        } else {
                            0.0
                        };
                        spacing_before * (if li == 0 { 1.0 } else { 0.0 })
                            + h
                            + ls
                            + spacing_after * (if li + 1 == line_count { 1.0 } else { 0.0 })
                    })
                    .sum();
                if has_table_in_para {
                    let nested_h: f64 = p
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
                    total += nested_h.max(line_based_h);
                } else {
                    total += line_based_h;
                }
            }
        }
        (total - content_offset).max(0.0)
    }


    /// [Issue #2214] 표 단위 nested-text flag에 대한 문단 로컬 기여 여부.
    /// 편집 경로와 table-wide 계산이 같은 predicate를 사용하도록 단일화한다.
    pub(crate) fn paragraph_contributes_to_table_nested_text_flag(paragraph: &Paragraph) -> bool {
        !paragraph.text.trim().is_empty()
            && paragraph
                .controls
                .iter()
                .any(|control| matches!(control, Control::Table(_)))
    }

    /// 문단이 정확히 하나의 1×1 자식 표를 host하는가.
    ///
    /// 저장 프레임 끝에서 이 형태의 표를 fragment로 푼 뒤 다음 문단 reset을
    /// 엄격히 보존할 때만 사용한다. 일반 문단 사이 reset의 orphan 완화와 분리한다.
    fn paragraph_hosts_single_cell_nested_table(paragraph: &Paragraph) -> bool {
        let mut tables = paragraph
            .controls
            .iter()
            .filter_map(|control| match control {
                Control::Table(table) => Some(table.as_ref()),
                _ => None,
            });
        matches!(
            (tables.next(), tables.next()),
            (Some(table), None) if table.row_count == 1 && table.col_count == 1
        )
    }

    /// [Issue #2063] 표에 "가시 텍스트 + 중첩 표"를 가진 셀이 하나라도 있는지 직접 계산한다.
    /// predicate table scan과 test counter는 이 helper에만 둔다.
    fn compute_table_nested_text_flag(&self, table: &crate::model::table::Table) -> bool {
        #[cfg(test)]
        self.table_nested_text_flag_scan_count
            .set(self.table_nested_text_flag_scan_count.get() + 1);
        table.cells.iter().any(|cell| {
            cell.paragraphs
                .iter()
                .any(Self::paragraph_contributes_to_table_nested_text_flag)
        })
    }

    /// [Issue #2063] 표에 "가시 텍스트 + 중첩 표"를 가진 셀이 하나라도 있는지(표 단위 불변량).
    /// `cell_units_uncached` 안에서 셀마다 계산되면 O(셀²)(52,694² ≈ 28억)로 폭증하므로
    /// 표 포인터를 키로 1회만 계산해 캐시한다(`cell_units_cache` 와 동일 조판 경계에서 clear).
    fn table_has_visible_text_with_nested_table(&self, table: &crate::model::table::Table) -> bool {
        let key = table as *const crate::model::table::Table as usize;
        if let Some(&cached) = self.table_nested_text_flag_cache.borrow().get(&key) {
            return cached;
        }
        let flag = self.compute_table_nested_text_flag(table);
        self.table_nested_text_flag_cache
            .borrow_mut()
            .insert(key, flag);
        flag
    }


    /// [Task #1949] `cell_units_uncached` 의 메모이즈 래퍼. 거대 셀이 RowBreak 로
    /// 여러 페이지에 걸칠 때 각 페이지 컷 판정이 같은 셀 units 를 재계산하는 O(pages×cell)
    /// 폭증을 제거한다. 셀 포인터를 키로 표 단위 캐시(문서 재조판 경계에서 clear).
    pub(super) fn cell_units(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
    ) -> std::sync::Arc<Vec<CellUnit>> {
        let key = cell as *const crate::model::table::Cell as usize;
        if let Some(cached) = self.cell_units_cache.borrow().get(&key) {
            if issue2424_profile_enabled() {
                ISSUE2424_CELL_UNITS_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            return std::sync::Arc::clone(cached);
        }
        let issue2424_started = issue2424_profile_enabled().then(std::time::Instant::now);
        let units = std::sync::Arc::new(self.cell_units_uncached(cell, table, styles));
        if let Some(started) = issue2424_started {
            use std::sync::atomic::Ordering::Relaxed;
            ISSUE2424_CELL_UNITS_MISSES.fetch_add(1, Relaxed);
            ISSUE2424_CELL_UNITS_MISS_NANOS.fetch_add(started.elapsed().as_nanos() as u64, Relaxed);
        }
        self.cell_units_cache
            .borrow_mut()
            .insert(key, std::sync::Arc::clone(&units));
        units
    }

    /// [#4128] `(cell_para_idx, target_line)` 이 속한 `cell_units` 서수.
    /// 텍스트 줄 유닛 `(li, li+1)` / atom 유닛 `(0, line_count.max(1))` 의
    /// `vis_start..vis_end` 계약을 그대로 조회한다. 콘텐츠(비 spacer) 유닛을
    /// 우선하되, 빈 문단처럼 spacer 유닛만 있는 문단은 spacer 서수로 폴백한다.
    /// 없으면 None.
    pub(super) fn cell_unit_ordinal_for(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
        cell_para_idx: usize,
        target_line: usize,
    ) -> Option<usize> {
        let units = self.cell_units(cell, table, styles);
        let hit = |u: &CellUnit| {
            u.para_idx == cell_para_idx
                && u.vis_start <= target_line
                && target_line < u.vis_end.max(u.vis_start + 1)
        };
        units
            .iter()
            .position(|u| !u.empty_spacer && hit(u))
            .or_else(|| units.iter().position(|u| hit(u)))
    }


}

#[cfg(test)]

mod row_cut_tests;
