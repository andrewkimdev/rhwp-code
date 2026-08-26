//! nested_split — table_layout.rs 에서 무변동 이동
use super::*;

pub(crate) type RowCut = Vec<usize>;


/// [Task #993] `advance_row_cut` 결과.
#[derive(Debug, Clone)]
pub(crate) struct RowCutResult {
    /// 셀별 소비 유닛 수 (전진 후).
    pub end_cut: RowCut,
    /// 어느 셀이든 vpos 리셋(hard break)에서 멈췄는가.
    pub hit_hard_break: bool,
    /// 모든 셀이 모든 유닛을 소비했는가.
    pub fully_consumed: bool,
    /// 이 프래그먼트의 콘텐츠 높이 (셀별 표시 높이의 최댓값, 패딩 제외).
    pub consumed_height: f64,
}


/// 재귀 1×1 block 앞의 source 문단 묶음 역할.
///
/// 부모 `RowCut`으로 투영한 뒤에는 자식 문단 인덱스가 사라지므로, 빈 구분 문단과
/// 바로 뒤 한 줄 제목을 높이나 텍스트 휴리스틱 없이 함께 넘기기 위해 보존한다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RecursiveBlockPreludeRole {
    #[default]
    None,
    EmptySeparator,
    ExplicitPageBreakSeparator,
    OneLineHeadingBeforeSingleCellTable,
}


/// mixed nested unit의 source-owner 판정에 필요한 최소 의미 정보.
///
/// `CellUnit` 전체를 helper에 노출하지 않아도 viewport reservation 규칙을 독립적으로
/// 회귀 고정할 수 있게 한다.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MixedNestedOwnerMarker {
    pub(crate) para_idx: usize,
    pub(crate) fragment: bool,
    pub(crate) trailing: bool,
    pub(crate) content_height: f64,
    pub(crate) height: f64,
}


/// 현재 cut 바로 뒤의 빈 trailing reservation이 mixed stream의 최종 source owner
/// 다음에 놓였을 때만 그 높이를 반환한다.
///
/// 뒤에 실제 source unit이 하나라도 남아 있으면 scalar child renderer는 명시적인
/// end-cut이 없으므로 viewport 확장이 미래 콘텐츠를 현재 쪽에 노출할 수 있다.
pub(crate) fn trailing_reservation_after_final_source_owner(
    para_idx: usize,
    successor: Option<MixedNestedOwnerMarker>,
    later_units: impl IntoIterator<Item = MixedNestedOwnerMarker>,
) -> f64 {
    let Some(successor) = successor.filter(|unit| {
        unit.para_idx == para_idx && unit.fragment && unit.trailing && unit.content_height <= 0.5
    }) else {
        return 0.0;
    };

    let has_later_source_owner = later_units.into_iter().any(|unit| {
        unit.para_idx == para_idx && unit.fragment && (!unit.trailing || unit.content_height > 0.5)
    });
    if has_later_source_owner {
        0.0
    } else {
        successor.height
    }
}


/// [#4069] 중첩 표의 셀 흐름을 바깥 셀 컷 원장으로 투영한 한 조각.
///
/// 기존 `(height, trailing, content_height)` 튜플은 내부 셀의 강제 쪽 경계를
/// 잃어버렸다. 특히 42065의 1×1 중첩 셀은 저장된 vpos가 쪽마다 0으로
/// 리셋되므로, 그 경계까지 함께 투영해야 첫 조각과 continuation이 같은 원장을 쓴다.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NestedFlowFragment {
    pub(crate) height: f64,
    pub(crate) hard_break_before: bool,
    pub(crate) stored_frame_break_before: bool,
    pub(crate) trailing: bool,
    pub(crate) content_height: f64,
    pub(crate) recursive: bool,
    pub(crate) starts_after_table: bool,
    /// Immediate child cell의 source paragraph. 1×1 host를 outer cell unit으로
    /// 평탄화할 때 table-control 경계(p20 등)를 잃지 않기 위한 provenance다.
    /// `None`은 기존 synthetic/row aggregate fragment이며 layout 값은 바꾸지 않는다.
    pub(crate) source_para_idx: Option<usize>,
    pub(crate) recursive_block_prelude_role: RecursiveBlockPreludeRole,
}


#[derive(Debug, Clone)]
pub(crate) struct NestedTableUnitCut {
    pub(crate) start_cut: RowCut,
    pub(crate) end_cut: RowCut,
    pub(crate) terminal: bool,
}


#[derive(Debug, Clone)]
pub(crate) struct NestedTableCut {
    pub start_row: usize,
    pub end_row: usize,
    pub start_cut: RowCut,
    pub end_cut: RowCut,
    pub is_block_split: bool,
}


/// 중첩 표 부분 렌더링을 위한 행 범위 정보
pub(crate) struct NestedTableSplit {
    pub start_row: usize,
    pub end_row: usize,
    /// 실제 표시할 높이 (마지막 행이 부분적으로 보일 때 전체 행 높이 대신 사용)
    pub visible_height: f64,
    /// 다음 셀 내용의 흐름 위치를 전진시킬 높이. 일반 split 에서는 visible_height 와 같고,
    /// mixed nested tail 에서는 표시 bbox 보다 큰 원래 flow slice 를 유지할 수 있다.
    pub flow_height: f64,
    /// start_row 내부 오프셋: 이미 이전 페이지에 렌더링된 start_row 상단 부분의 높이
    pub offset_within_start: f64,
    /// 셀 유닛 스트림에서 이 조각이 시작하는 원본 소비 위치. mixed nested tail은
    /// 물리 y 보정 때문에 `offset_within_start`에 첫 가시 유닛을 더할 수 있으므로,
    /// 다음 깊이의 동일한 유닛 컷을 재구성할 때는 이 원본 값을 사용한다.
    pub content_offset: f64,
    /// 부모 native short-parent-child fragment가 이미 소비한 source unit을 terminal
    /// child viewport에서도 다시 그리지 않도록 한다. 일반 terminal tail은 종전처럼
    /// source cut을 끈다; native short-parent-child와 p33→34류 terminal rowbreak
    /// source cursor를 함께 OR한 값이므로 두 형상 모두 unit 컷 자체는 켠다.
    pub force_source_start_cut: bool,
    /// native short-parent-child(76076 p81→82)에서만 true. `force_source_start_cut`이
    /// p33→34류 terminal rowbreak source cursor만으로 true인 경우에는 false를 유지해
    /// 이미 소유가 끝난 마지막 unit을 다음 조각에서 중복 페인트하지 않는다.
    pub replay_terminal_boundary_unit: bool,
    /// [#3658] 이 조각이 해당 셀 콘텐츠의 **마지막** 조각인가 (컷이 마지막 유닛까지
    /// 포함 — end_cut 종료). true 면 이어받을 continuation 이 없으므로 셀 하단 초과
    /// 줄 드롭(다음 쪽 소속 줄 제외용)을 적용하지 않는다 — 꼬리 문단 유실 방지.
    pub terminal: bool,
    /// [#4069] 중첩 표의 자식 행·CellUnit 범위. Some이면 scalar clip 대신
    /// `layout_partial_table`에 동일 컷을 넘겨 측정과 렌더의 fragment 권위를 통일한다.
    pub recursive_cut: Option<NestedTableCut>,
}


/// 중첩 표에서 pixel offset/space를 행 범위로 변환한다.
/// 공간이 부족한 마지막 행은 제외하여 다음 페이지에서 렌더링되도록 한다.
pub(crate) fn calc_nested_split_rows(
    row_heights: &[f64],
    cell_spacing: f64,
    offset: f64,
    space: f64,
) -> NestedTableSplit {
    let row_count = row_heights.len();
    if row_count == 0 {
        return NestedTableSplit {
            start_row: 0,
            end_row: 0,
            visible_height: 0.0,
            flow_height: 0.0,
            offset_within_start: 0.0,
            content_offset: 0.0,
            force_source_start_cut: false,
            replay_terminal_boundary_unit: false,
            terminal: false,
            recursive_cut: None,
        };
    }

    // row_y 누적 배열 (layout_table과 동일 방식)
    let mut row_y = vec![0.0f64; row_count + 1];
    for i in 0..row_count {
        row_y[i + 1] =
            row_y[i] + row_heights[i] + if i + 1 < row_count { cell_spacing } else { 0.0 };
    }

    // offset에 해당하는 시작 행 찾기
    let mut start_row = 0;
    if offset > 0.0 {
        start_row = row_count;
        for r in 0..row_count {
            if row_y[r] + row_heights[r] > offset {
                start_row = r;
                break;
            }
        }
    }

    // space에 해당하는 끝 행 찾기
    let visible_end = offset + space;
    let mut end_row = row_count;
    if space > 0.0 && space < f64::MAX {
        for r in 0..row_count {
            if row_y[r] + row_heights[r] >= visible_end {
                end_row = r + 1;
                break;
            }
        }
    }

    // 마지막 행이 거의 들어가지 않으면 제외하여 다음 페이지에서 온전하게 렌더링
    if end_row > start_row {
        let last_r = end_row - 1;
        let last_row_top = row_y[last_r];
        let available_for_last = visible_end - last_row_top;
        let last_h = row_heights[last_r];
        let min_threshold = (last_h * 0.5).min(10.0);
        if available_for_last < last_h && available_for_last < min_threshold {
            end_row -= 1;
        }
    }

    // visible_height: 포함된 행의 실제 높이 (start_row 전체 포함)
    let range_height = if end_row > start_row {
        row_y[end_row] - row_y[start_row]
    } else {
        0.0
    };
    // 연속 페이지(offset>0): start_row를 처음부터 완전히 렌더링하므로
    // offset_within_start=0, visible_height=range_height (포함된 행 전체 높이)
    // 첫 페이지(offset==0): 가용 공간으로 캡
    let visible_height = if offset > 0.0 {
        range_height
    } else {
        space.min(range_height)
    };

    NestedTableSplit {
        start_row,
        end_row,
        visible_height,
        flow_height: visible_height,
        offset_within_start: 0.0,
        content_offset: offset.max(0.0),
        force_source_start_cut: false,
        replay_terminal_boundary_unit: false,
        terminal: false,
        recursive_cut: None,
    }
}


/// [#2089] 가로쓰기 셀 본문 배치의 셀-스코프 스칼라 묶음.
#[derive(Clone, Copy)]
pub(crate) struct HorizontalCellVars {
    pub(crate) cell_idx: usize,
    pub(crate) r: usize,
    pub(crate) cell_y: f64,
    pub(crate) cell_h: f64,
    pub(crate) content_cell_y: f64,
    pub(crate) pad_top: f64,
    pub(crate) inner_x: f64,
    pub(crate) inner_width: f64,
    pub(crate) inner_height: f64,
    pub(crate) text_y_start: f64,
    pub(crate) use_top_vpos_anchor: bool,
    /// Physical page-clip continuation이 저장 vpos 앵커도 함께 내려야 하는 높이.
    pub(crate) upper_clip_line_reservation: f64,
    /// [Task #2211] 저장 LINE_SEG 흐름이 자체 스택 합보다 압축된 셀 —
    /// 문단 배치를 저장 vpos 스냅으로 강제한다 (valign 무관).
    pub(crate) trust_stored_cell_flow: bool,
    pub(crate) has_nested_table: bool,
    pub(crate) section_index: usize,
    pub(crate) outline_numbering_id: u16,
    pub(crate) depth: usize,
    pub(crate) clamp_header_negative_para_offset: bool,
    /// root-body table owner의 첫 저장 LINE_SEG vpos. nested/header/footer 호출은 None.
    pub(crate) outer_host_stored_vpos_hu: Option<i32>,
    pub(crate) inline_table_flow_y_shift: f64,
    /// 이 1×1 표가 앞 페이지에서 일부 흐름을 이미 소비한 continuation인가.
    /// 단순 row filter의 첫 조각과 구분해, 미래 중첩 표의 위치 상한을 풀 수 있는
    /// 유일한 문맥으로 쓴다 (#2007/#3637).
    pub(crate) single_row_continuation: bool,
    /// 부모 RowBreak 조각이 이 1×1 표에 명시적으로 전달한 물리 viewport인가.
    /// 첫 조각(offset=0)도 포함한다. 표 셀의 유닛 컷을 재구성할 때 continuation과
    /// 구분하지 않고 같은 소유 범위를 적용해야 한다.
    pub(crate) single_row_fragment: bool,
    /// 현재 1행 continuation이 앞 조각에서 이미 소비한 높이. 중첩 표도 같은
    /// 물리 viewport를 이어 그리려면 이 누적 오프셋을 그대로 받아야 한다.
    pub(crate) single_row_continuation_offset: Option<f64>,
    /// 같은 조각의 원본 unit 소비 위치. 물리 y 보정과 분리해 다음 중첩 표의
    /// unit 경계를 계산한다.
    pub(crate) single_row_fragment_content_offset: Option<f64>,
    /// native short-parent child의 terminal continuation도 이미 소비한 source
    /// prefix를 건너뛰게 하는 명시 신호. 일반 terminal tail에는 false다.
    pub(crate) force_source_start_cut: bool,
    /// true면 마지막 source unit을 다음 조각에서 재생한다 (76076 p81→82 형상만 해당).
    pub(crate) replay_terminal_boundary_unit: bool,
    /// [#3658] 분할 렌더(row_filter)가 이 셀 콘텐츠의 마지막 조각인가.
    /// true 면 셀 하단 초과 줄 드롭(다음 쪽 소속 줄 제외)을 적용하지 않는다 —
    /// 이어받을 continuation 이 없어 드롭된 꼬리 줄은 영구 유실되기 때문.
    pub(crate) split_terminal: bool,
}

impl From<&CellUnit> for MixedNestedOwnerMarker {
    fn from(unit: &CellUnit) -> Self {
        Self {
            para_idx: unit.para_idx,
            fragment: unit.mixed_nested_fragment,
            trailing: unit.mixed_nested_trailing,
            content_height: unit.mixed_nested_content_height,
            height: unit.height,
        }
    }
}

impl LayoutEngine {
    /// [Task #993] 한 셀의 콘텐츠를 "유닛" 시퀀스로 평탄화한다.
    ///
    /// 유닛 1개 = 합성 줄 1개 또는 중첩 표 atom 1개(중첩 표 문단 = 유닛 1개,
    /// 분할 불가). 유닛 높이는 `compute_cell_line_ranges`/`calc_visible_content_*`
    /// 의 줄 높이 계산과 동일 규칙(줄 h+ls, 셀 마지막 줄 ls 제외, 문단 첫·마지막
    /// 줄에 spacing_before/after). `hard_break_before` = 이 유닛 앞에 HWP vpos
    /// 리셋(셀 내부 페이지 분할, `[Task #697]`)이 있는가.
    pub(super) fn nested_table_mixed_fragment_heights(
        &self,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
    ) -> Vec<NestedFlowFragment> {
        if table.row_count != 1 {
            return Vec::new();
        }

        // [#4069] 단일 셀 중첩 표는 별도 높이 추정식을 다시 만들지 않고 그 셀의
        // canonical CellUnit 원장을 재사용한다. 저장 페이지 프레임 경계를 가진
        // 장문 흐름에 한정해 깊이와 무관하게 재귀하며, placeholder line_height와
        // nested_h를 동시에 더하던 기존 이중 회계를 제거한다. 단일 경계 문서는
        // #2279의 검증된 legacy 측정 원장을 유지한다.
        let mut row_cells = table
            .cells
            .iter()
            .filter(|cell| cell.row == 0 && cell.row_span == 1);
        if let (Some(cell), None) = (row_cells.next(), row_cells.next()) {
            let units = self.cell_units(cell, table, styles);
            let stored_page_frame_boundaries =
                units.iter().filter(|unit| unit.hard_break_before).count();
            let has_authoritative_frame_boundary =
                units.iter().any(|unit| unit.stored_frame_break_before);
            // [#3820 Stage 50] 페이지 하단에서 시작한 1×1 자식 표는 첫 저장
            // fragment가 짧아 문단 사이 vpos reset 직전 좌표가 body 절반에 못
            // 미칠 수 있다(59043 p35: 7540HU→0). 하지만 표 자체의 물리 높이가
            // 한 페이지를 넘고 저장 reset이 정확히 하나라면 이 경계는 로컬
            // 재시작이 아니라 다음 쪽 source cursor다. legacy mixed fallback은
            // 모든 hard break를 지우므로 이 경우에만 canonical CellUnit을 투영한다.
            // 한 페이지 이하 단일 reset과 다중 reset의 기존 판정은 유지한다.
            let body_height = self.current_body_area.get().3;
            let page_height = if body_height > 0.0 {
                body_height
            } else {
                900.0
            };
            let nested_table_height = self.calc_nested_table_height(table, styles);
            let preserve_single_multi_page_boundary = stored_page_frame_boundaries == 1
                // HWP5-origin HWPX는 변환 과정에서 단일 로컬 reset을 남길 수
                // 있으므로 #3637의 31쪽 계약처럼 기존 HWPX 원장을 유지한다.
                // 이 예외는 원본 HWP5 바이너리의 저장 좌표 계약에만 적용한다.
                && self.profile.get().native_hwp5_layout()
                && nested_table_height > page_height + 0.5;
            // direct HWPX도 물리 한 쪽을 넘는 1×1 표에 저장 reset이 하나 있으면
            // canonical CellUnit의 높이/세분성이 필요하다(#3637: 1740.6px,
            // body 971.3px). 다만 reset은 HWP5 source cursor로 승격하지 않고 아래
            // scalar projection에서 제거한다. reset이 전혀 없는 issue1891의 깊은
            // wrapper까지 이 조건에 포함하면 마지막 쪽 overflow가 34→56으로 는다.
            let direct_hwpx_single_multi_page_projection = stored_page_frame_boundaries == 1
                && self.profile.get().hwpx_stored_layout()
                && !self.profile.get().hwp5_origin_hwpx()
                && nested_table_height > page_height + 0.5;
            // canonical CellUnit의 hard-break 원장은 HWP5 저장 좌표 계약이다.
            // direct HWPX의 셀 lineSeg reset은 중첩 셀 로컬 viewport 재시작일 수
            // 있으므로, reset 수가 둘 이상이어도 이 경로로 승격하지 않는다.
            // 그렇지 않으면 #3637 pi=197의 마지막 RowBreak 조각이 한 조각 늘어
            // 후속 표 전체가 불필요한 32쪽으로 밀린다. HWP5-origin HWPX는 원본
            // HWP5의 pagination marker를 보존하므로 canonical 경로를 유지한다.
            let canonical_stored_frame_profile =
                self.profile.get().native_hwp5_layout() || self.profile.get().hwp5_origin_hwpx();
            if let Ok(pattern) = std::env::var("RHWP_DIAG_MIXFRAG") {
                if cell
                    .paragraphs
                    .iter()
                    .any(|paragraph| paragraph.text.contains(&pattern))
                {
                    let nested_controls = cell
                        .paragraphs
                        .iter()
                        .flat_map(|paragraph| paragraph.controls.iter())
                        .filter(|control| matches!(control, Control::Table(_)))
                        .count();
                    eprintln!(
                        "DIAG_MIXFRAG_PROFILE paras={} units={} resets={} authoritative={} nested_ctrls={} table_h={:.1} body_h={:.1}",
                        cell.paragraphs.len(),
                        units.len(),
                        stored_page_frame_boundaries,
                        has_authoritative_frame_boundary,
                        nested_controls,
                        nested_table_height,
                        page_height,
                    );
                    for (unit_idx, unit) in units.iter().enumerate() {
                        eprintln!(
                            "  unit[{unit_idx}] h={:.2} para={} lines={}..{} mixed={} trailing={} content_h={:.2} hard={} stored={} spacer={}",
                            unit.height,
                            unit.para_idx,
                            unit.vis_start,
                            unit.vis_end,
                            unit.mixed_nested_fragment,
                            unit.mixed_nested_trailing,
                            unit.mixed_nested_content_height,
                            unit.hard_break_before,
                            unit.stored_frame_break_before,
                            unit.empty_spacer,
                        );
                    }
                }
            }
            // [#4069 Stage 2/Task #3820 Stage 48] 저장 프레임 경계는 문단 내부인지
            // 문단 사이인지와 무관하게 하나라도 부모 원장에 보존한다. 작은 로컬 reset은
            // `is_hwp5_stored_frame_rewind`의 body-half 조건에서 이미 제외된다.
            // 42065 p10은 같은 문단 58620→0, p14는 item7→item8의 문단간
            // 32932→0 경계이며 둘 다 한컴 정본의 실제 쪽 경계다.
            if canonical_stored_frame_profile
                && (stored_page_frame_boundaries >= 2
                    || has_authoritative_frame_boundary
                    || preserve_single_multi_page_boundary)
            {
                return units
                    .iter()
                    .map(|unit| {
                        let visible = Self::cell_unit_has_visible_content(cell, unit);
                        NestedFlowFragment {
                            height: unit.height,
                            hard_break_before: unit.hard_break_before,
                            // 물리적으로 여러 쪽을 차지하는 1×1 자식 표의 유일한
                            // 저장 reset은 부모 viewport에서 실제 쪽 경계다. 자식의
                            // 일반 CellUnit 의미는 바꾸지 않고, 부모로 투영되는
                            // fragment에만 authoritative 표식을 부여해 RowBreak의
                            // 중간-reset 완화가 첫 source 줄을 앞쪽에 흡수하지 않게 한다.
                            stored_frame_break_before: unit.stored_frame_break_before
                                || (preserve_single_multi_page_boundary && unit.hard_break_before),
                            trailing: unit.mixed_nested_trailing || !visible,
                            content_height: if unit.mixed_nested_content_height > 0.0 {
                                unit.mixed_nested_content_height
                            } else if visible {
                                unit.height
                            } else {
                                0.0
                            },
                            recursive: true,
                            starts_after_table: unit.mixed_nested_starts_after_table,
                            source_para_idx: Some(unit.para_idx),
                            recursive_block_prelude_role: unit.recursive_block_prelude_role,
                        }
                    })
                    .collect();
            }
            if self.profile.get().hwpx_stored_layout()
                && !self.profile.get().hwp5_origin_hwpx()
                && (stored_page_frame_boundaries >= 2
                    || has_authoritative_frame_boundary
                    || direct_hwpx_single_multi_page_projection)
            {
                // PR #4122 이전 direct-HWPX fallback은 빈 host의 자식 표를
                // 재귀적으로 평탄화하고, 같은 문단의 placeholder line과 표 높이를
                // 한 번만 회계했다. 단순 legacy 재측정은 그 세 동작을 잃어
                // #3637이 30쪽으로 과소 조판되거나 p26 source owner를 잃는다.
                // 현재 canonical CellUnit은 그 재귀 원장을 이미 보유하므로, 높이와
                // 가시 단위만 재사용하되 HWP5 전용 hard/stored cursor 의미를 제거해
                // 검증된 HWPX scalar viewport 계약으로 투영한다. reset이 없는 일반
                // 중첩 표까지 이 경로로 바꾸면 issue1891의 깊은 표가 마지막 쪽에서
                // 22줄 더 밀리므로, canonical 승격 대상이었던 반복/authoritative
                // reset 표에만 이 변환을 적용한다. reset 0개이거나 물리 한 쪽에
                // 못 미치는 단일 reset 표는 legacy fallback을 유지한다.
                return units
                    .iter()
                    .map(|unit| {
                        let visible = Self::cell_unit_has_visible_content(cell, unit);
                        NestedFlowFragment {
                            height: unit.height,
                            hard_break_before: false,
                            stored_frame_break_before: false,
                            trailing: unit.mixed_nested_trailing || !visible,
                            content_height: if unit.mixed_nested_content_height > 0.0 {
                                unit.mixed_nested_content_height
                            } else if visible {
                                unit.height
                            } else {
                                0.0
                            },
                            recursive: false,
                            starts_after_table: unit.mixed_nested_starts_after_table,
                            source_para_idx: Some(unit.para_idx),
                            recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                        }
                    })
                    .collect();
            }
        }

        let mut row_units: Vec<(f64, bool, f64, bool, Option<usize>)> = Vec::new();
        for cell in table.cells.iter().filter(|cell| cell.row == 0) {
            // 이 helper는 부모 셀의 Control::Table, 즉 중첩 표의 조각 유닛을
            // 산출한다. 비글자 중첩 표는 실제 배치에서 보존된 작은 cellMargin을
            // 사용하므로, 여기에서도 같은 폭으로 조판해야 한다. 기본 padding
            // 규칙(aim=false → table inMargin)으로 재조판하면 더 넓은 폭에서 줄이
            // 하나 덜 생겨, 유닛 컷은 마지막 줄을 포함했다고 판단해도 실제 SVG
            // glyph가 다음 페이지 clip에 걸린다 (76076 p33→34).
            let preserve_saved_small_margin = !table.common.treat_as_char
                && !self
                    .render_normalization_overlay()
                    .uses_owner_content_box(table);
            let (pad_left, pad_right, pad_top, pad_bottom) =
                self.resolve_cell_padding_for_context(cell, table, preserve_saved_small_margin);
            let cell_w = if cell.width < 0x8000_0000 {
                hwpunit_to_px(cell.width as i32, self.dpi) * self.render_table_width_scale(table)
            } else {
                0.0
            };
            let inner_width = (cell_w - pad_left - pad_right).max(0.0);
            let mut cell_units = Vec::new();
            let mut after_completed_multiline_table = false;
            for (pi, para) in cell.paragraphs.iter().enumerate() {
                let para_is_empty_spacer = para.text.trim().is_empty() && para.controls.is_empty();
                let starts_after_completed_multiline_table =
                    after_completed_multiline_table && !para_is_empty_spacer;
                let mut comp = compose_paragraph(para);
                crate::renderer::composer::recompose_for_cell_width(
                    &mut comp,
                    para,
                    inner_width,
                    styles,
                );
                // [#2279 axis A] 종전에는 comp.lines 빈 문단을 통째 skip 해 (a) 2단계
                // 중첩 표(빈 문단 소속)와 (b) 빈 문단 줄박스가 유닛에서 누락됐다 —
                // 86712 pi=172 r27 근거설명(25문단 + 3×12 + 5×4 내부표) 프래그먼트 합
                // 933px vs mt·한글 ~1402px 의 -448 주성분. 중첩 표는
                // calc_nested_table_height(행합+cs+outer margin, 측정 단일 출처),
                // 빈 문단은 #2169 em 줄박스 규칙으로 유닛화한다.
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
                let empty_line_box = if comp.lines.is_empty()
                    && nested_h <= 0.0
                    && para.line_segs.is_empty()
                    && para.controls.is_empty()
                    && para.text.trim().is_empty()
                {
                    let fs = para
                        .char_shapes
                        .first()
                        .and_then(|cs| styles.char_styles.get(cs.char_shape_id as usize))
                        .map(|cs| cs.font_size)
                        .unwrap_or(0.0);
                    if fs > 0.0 {
                        fs
                    } else {
                        hwpunit_to_px(400, self.dpi)
                    }
                } else {
                    0.0
                };
                if comp.lines.is_empty() && nested_h <= 0.5 && empty_line_box <= 0.5 {
                    continue;
                }

                let para_style = styles.para_styles.get(para.para_shape_id as usize);
                if pi == 0 && pad_top > 0.5 {
                    cell_units.push((pad_top, false, 0.0, false, None));
                }
                if pi > 0 {
                    let spacing_before = para_style.map(|s| s.spacing_before).unwrap_or(0.0);
                    if spacing_before > 0.5 {
                        cell_units.push((spacing_before, false, 0.0, false, None));
                    }
                }
                for (li, line) in comp.lines.iter().enumerate() {
                    let raw_lh = hwpunit_to_px(line.line_height, self.dpi);
                    let corrected_h = match para_style {
                        Some(ps) => {
                            let max_fs = line
                                .runs
                                .iter()
                                .map(|r| {
                                    let ts = super::super::text_measurement::resolved_to_text_style(
                                        styles,
                                        r.char_style_id,
                                        r.lang_index,
                                    );
                                    if ts.font_size > 0.0 {
                                        ts.font_size
                                    } else {
                                        12.0
                                    }
                                })
                                .fold(0.0f64, f64::max);
                            crate::renderer::corrected_line_height_for_variant_synthetic(
                                raw_lh,
                                max_fs,
                                ps.line_spacing_type,
                                ps.line_spacing,
                                self.profile.get().hwp3_layout()
                                    && para.line_segs.is_empty()
                                    && !para.text.is_empty(),
                            )
                        }
                        None => raw_lh,
                    };
                    // [#2279 axis A] 문단 말미 줄간격은 셀의 마지막 문단에서만 탈락 —
                    // mt(calc_para_lines_height / #2211 include_trailing_ls)와 정합.
                    // 종전 per-문단 탈락은 25문단 셀에서 -83px 과소(86712 r27).
                    let is_cell_last_para = pi + 1 == cell.paragraphs.len();
                    let line_spacing = if li + 1 == comp.lines.len() && is_cell_last_para {
                        0.0
                    } else {
                        hwpunit_to_px(line.line_spacing, self.dpi)
                    };
                    cell_units.push((
                        corrected_h + line_spacing,
                        false,
                        corrected_h,
                        starts_after_completed_multiline_table && li == 0,
                        Some(pi),
                    ));
                }
                if nested_h > 0.5 {
                    cell_units.push((
                        nested_h,
                        false,
                        nested_h,
                        starts_after_completed_multiline_table,
                        Some(pi),
                    ));
                }
                if empty_line_box > 0.5 {
                    cell_units.push((
                        empty_line_box,
                        false,
                        empty_line_box,
                        starts_after_completed_multiline_table,
                        Some(pi),
                    ));
                }
                if pi + 1 < cell.paragraphs.len() {
                    let spacing_after = para_style.map(|s| s.spacing_after).unwrap_or(0.0);
                    if spacing_after > 0.5 {
                        cell_units.push((spacing_after, true, 0.0, false, None));
                    }
                }
                let completed_multiline_table = para
                    .controls
                    .iter()
                    .any(|control| matches!(control, Control::Table(table) if table.row_count > 1));
                if completed_multiline_table {
                    after_completed_multiline_table = true;
                } else if !para_is_empty_spacer {
                    after_completed_multiline_table = false;
                }
            }
            if pad_bottom > 0.5 {
                cell_units.push((pad_bottom, true, 0.0, false, None));
            }
            // [#2279 진단] 1×1 중첩 셀 프래그먼트 분해 — 동작 불변.
            if let Ok(pat) = std::env::var("RHWP_DIAG_MIXFRAG") {
                if cell.paragraphs.iter().any(|p| p.text.contains(&pat)) {
                    let total: f64 = cell_units.iter().map(|(h, _, _, _, _)| *h).sum();
                    eprintln!(
                        "DIAG_MIXFRAG cell paras={} units={} total={:.1} inner_w={:.2}",
                        cell.paragraphs.len(),
                        cell_units.len(),
                        total,
                        inner_width,
                    );
                    for (pi, para) in cell.paragraphs.iter().enumerate() {
                        let mut comp = compose_paragraph(para);
                        crate::renderer::composer::recompose_for_cell_width(
                            &mut comp,
                            para,
                            inner_width,
                            styles,
                        );
                        let nctl = para.controls.len();
                        eprintln!(
                            "  p[{pi}] lines={} text_len={} ctrls={} ls_stored={} text={:?}",
                            comp.lines.len(),
                            para.text.chars().count(),
                            nctl,
                            para.line_segs.len(),
                            para.text.chars().take(16).collect::<String>(),
                        );
                    }
                }
            }
            if cell_units.len() > row_units.len() {
                row_units.resize(cell_units.len(), (0.0, true, 0.0, false, None));
            }
            for (idx, (h, trailing, content_h, starts_after_table, source_para_idx)) in
                cell_units.into_iter().enumerate()
            {
                if h > row_units[idx].0 {
                    row_units[idx] = (h, trailing, content_h, starts_after_table, source_para_idx);
                } else if (h - row_units[idx].0).abs() <= 0.5 {
                    row_units[idx].1 = row_units[idx].1 && trailing;
                    row_units[idx].2 = row_units[idx].2.max(content_h);
                    row_units[idx].3 = row_units[idx].3 || starts_after_table;
                    if row_units[idx].4 != source_para_idx {
                        row_units[idx].4 = None;
                    }
                }
            }
        }
        row_units
            .into_iter()
            .map(
                |(height, trailing, content_height, starts_after_table, source_para_idx)| {
                    NestedFlowFragment {
                        height,
                        hard_break_before: false,
                        stored_frame_break_before: false,
                        trailing,
                        content_height,
                        recursive: false,
                        starts_after_table,
                        source_para_idx,
                        recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                    }
                },
            )
            .collect()
    }

}
