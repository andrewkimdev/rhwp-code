//! cell_units — table_layout.rs 에서 무변동 이동
use super::*;

impl LayoutEngine {
    /// [#4167] 문단의 cell-units 기여 지문 — 편집이 units 를 실제로 바꿨는지 판별용.
    ///
    /// units 산출이 읽는 문단 입력만 포함한다: line_segs 의 (vertical_pos, line_height,
    /// tag) 수열(개수 포함), controls 수, 공백/빈 문단 클래스. `text_start` 와
    /// `segment_width` 는 제자리 타이핑에도 매 키 변하지만 units 산출이 읽지 않으므로
    /// 제외한다(위 `cell_units_uncached` 전 구간 grep 근거). 인접 문단 결합(직전 끝
    /// seg·직후 첫 seg 참조)도 이 지문에 담긴 경계 seg 로 판별된다 — 지문 불변이면
    /// 이웃 기여도 불변이다.
    pub(crate) fn cell_paragraph_units_fingerprint(
        para: &crate::model::paragraph::Paragraph,
    ) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        para.line_segs.len().hash(&mut h);
        for seg in &para.line_segs {
            seg.vertical_pos.hash(&mut h);
            seg.line_height.hash(&mut h);
            // units 산출이 tag 에서 읽는 비트는 synthetic 판별(bit 31)뿐이다. 원본
            // 로드 tag 의 여타 비트(예: 0x100000)는 reflow 가 재방출하지 않아 첫
            // 편집에서 무의미하게 지문을 바꾸므로 마스킹한다.
            (seg.tag & crate::model::paragraph::LineSeg::TAG_IMPLEMENTATION_PROPERTY).hash(&mut h);
        }
        para.controls.len().hash(&mut h);
        para.text.is_empty().hash(&mut h);
        para.text.trim().is_empty().hash(&mut h);
        h.finish()
    }


    /// [Issue #2214/#2424] 텍스트 편집 뒤 edited cell의 memoized units를 국소 무효화한다.
    ///
    /// cached owner flag가 false인데 edited paragraph가 false→true가 된 경우에만
    /// owner의 직접 cell units를 모두 제거하고, local witness로 flag를 true로 갱신한다.
    /// 삭제로 true→false가 되면 다른 cell의 contribution 여부를 알 수 없으므로 owner의
    /// 직접 cell units와 flag를 제거해 다음 접근에서 한 번만 다시 계산한다.
    /// 이 direct-key 제거는 predicate 재스캔이 아니며 nested/unrelated table cache는 보존한다.
    pub(crate) fn invalidate_cell_units_after_text_edit(
        &self,
        edited_cell: &crate::model::table::Cell,
        owner_table: &crate::model::table::Table,
        local_before: bool,
        local_after: bool,
        unit_fingerprint_unchanged: bool,
    ) {
        let edited_cell_key = edited_cell as *const crate::model::table::Cell as usize;
        let owner_table_key = owner_table as *const crate::model::table::Table as usize;
        let cached_owner_flag = self
            .table_nested_text_flag_cache
            .borrow()
            .get(&owner_table_key)
            .copied();
        let local_became_true = !local_before && local_after;
        let local_became_false = local_before && !local_after;

        if local_became_false {
            let mut cell_cache = self.cell_units_cache.borrow_mut();
            for cell in &owner_table.cells {
                let key = cell as *const crate::model::table::Cell as usize;
                cell_cache.remove(&key);
            }
            drop(cell_cache);
            self.table_nested_text_flag_cache
                .borrow_mut()
                .remove(&owner_table_key);
            return;
        }

        if local_became_true && cached_owner_flag == Some(false) {
            let mut cell_cache = self.cell_units_cache.borrow_mut();
            for cell in &owner_table.cells {
                let key = cell as *const crate::model::table::Cell as usize;
                cell_cache.remove(&key);
            }
            drop(cell_cache);
            self.table_nested_text_flag_cache
                .borrow_mut()
                .insert(owner_table_key, true);
            return;
        }

        // [#4167] 편집 문단의 units 기여 지문이 불변이면(제자리 타이핑 — 줄 수·높이
        // 불변) 캐시된 units 벡터 전체가 그대로 유효하다 — 거대 셀(수천 문단)의
        // 전량 recompose(11ms/키)를 생략한다. 지문이 다르면 종전대로 셀 단위 제거.
        if unit_fingerprint_unchanged {
            return;
        }
        self.cell_units_cache.borrow_mut().remove(&edited_cell_key);
        if local_became_true && cached_owner_flag.is_none() {
            // cell_units entry가 있으면 owner flag도 먼저 warm된다는 현재 cache invariant에
            // 따라 owner-wide eviction은 불필요하다. local witness로 future scan도 피한다.
            self.table_nested_text_flag_cache
                .borrow_mut()
                .insert(owner_table_key, true);
        }
    }


    /// 빈 native HWP5 RowBreak parent의 마지막 1×1 block child가, parent의 선언
    /// 높이보다 content flow가 커서 페이지 tail에서 child unit을 나눠야 하는 구조인지 판별한다.
    ///
    /// 이 조건은 `cell_units()`가 단일행 child를 mixed fragment로 전개하는 경로와
    /// paginator의 split-eligibility가 반드시 공유해야 한다. 한 쪽만 열면 unit은
    /// 생성돼도 `MeasuredTable`이 row를 atomic으로 보고 `advance_row_cut()`까지
    /// 도달하지 않는다 (76076 p81→82).
    pub(crate) fn native_short_parent_child_fragment_eligible(
        &self,
        table: &crate::model::table::Table,
        cell: &crate::model::table::Cell,
        child: &crate::model::table::Table,
        child_flow_height: f64,
    ) -> bool {
        let parent_declared_height = hwpunit_to_px(table.common.height as i32, self.dpi);
        let eligible = self.profile.get().hwp5_stored_pagination_layout()
            && !table.common.treat_as_char
            && matches!(table.common.text_wrap, TextWrap::TopAndBottom)
            && matches!(table.common.vert_rel_to, VertRelTo::Para)
            && matches!(table.page_break, TablePageBreak::RowBreak)
            && table.row_count > 1
            && cell.row_span == 1
            && cell.row as usize + 1 == table.row_count as usize
            // HWP5 저장기는 block child 뒤에 vpos=0의 빈 reset 문단을 남길 수
            // 있다. 그것은 별 content host가 아니므로 허용하되, field/표/텍스트를
            // 가진 후속 문단이 있으면 일반 원자 경로를 유지한다.
            && cell.paragraphs.first().is_some_and(|host| {
                host.text.trim().is_empty()
                    && host
                        .controls
                        .iter()
                        .filter(|control| matches!(control, Control::Table(_)))
                        .count()
                        == 1
            })
            && cell.paragraphs.iter().skip(1).all(|paragraph| {
                paragraph.text.trim().is_empty()
                    && paragraph.controls.is_empty()
                    && paragraph.line_segs.len() <= 1
            })
            && child.col_count == 1
            && !child.common.treat_as_char
            // p831의 7문단 `산식 설명` child처럼 일반적인 큰 tail은 여기서
            // 분해하지 않는다. 이 경로는 page-tail에 한두 줄만 배치되는 short
            // child의 source owner를 보존하기 위한 것이다.
            && child.cells.len() == 1
            && child.cells[0].paragraphs.len() <= 3
            && parent_declared_height > 0.0
            && child_flow_height > parent_declared_height + 0.5;
        if std::env::var("RHWP_DIAG_SHORT_CHILD").is_ok()
            && child.row_count == 1
            && child.col_count == 1
            && child.cells.len() == 1
        {
            eprintln!(
                "DIAG_SHORT_CHILD eligible={} native={} parent=(rows={},h={:.1},wrap={:?},vert={:?},break={:?}) cell=(row={},span={},paras={}) child=(cols={},tac={},cells={},paras={},flow={:.1})",
                eligible,
                self.profile.get().hwp5_stored_pagination_layout(),
                table.row_count,
                parent_declared_height,
                table.common.text_wrap,
                table.common.vert_rel_to,
                table.page_break,
                cell.row,
                cell.row_span,
                cell.paragraphs.len(),
                child.col_count,
                child.common.treat_as_char,
                child.cells.len(),
                child.cells.first().map(|c| c.paragraphs.len()).unwrap_or(0),
                child_flow_height,
            );
        }
        eligible
    }


    /// Native HWP5 `RowBreak`의 마지막 행에서 이미 outer `CellUnit` cut으로
    /// 분할된 1×1 block child인지 판별한다.
    ///
    /// 짧은 child는 `native_short_parent_child_fragment_eligible`가 paginator의
    /// 분할 가능 여부까지 함께 결정한다. 이 helper는 그보다 좁은 후속 단계다:
    /// mixed split이 이미 child의 source height를 소비한 **terminal** tail에서
    /// 시작 cursor만 전달한다. 따라서 큰 child를 새로 fragment 단위로 승격하거나
    /// HWPCTRL/WASM API 계약을 바꾸지 않는다.
    pub(crate) fn native_terminal_rowbreak_child_source_cursor_eligible(
        &self,
        table: &crate::model::table::Table,
        cell: &crate::model::table::Cell,
        child: &crate::model::table::Table,
    ) -> bool {
        self.profile.get().hwp5_stored_pagination_layout()
            && !table.common.treat_as_char
            && matches!(table.common.text_wrap, TextWrap::TopAndBottom)
            && matches!(table.common.vert_rel_to, VertRelTo::Para)
            && matches!(table.page_break, TablePageBreak::RowBreak)
            && table.row_count > 1
            && cell.row_span == 1
            && cell.row as usize + 1 == table.row_count as usize
            && cell.paragraphs.first().is_some_and(|host| {
                host.text.trim().is_empty()
                    && host
                        .controls
                        .iter()
                        .filter(|control| matches!(control, Control::Table(_)))
                        .count()
                        == 1
            })
            && cell.paragraphs.iter().skip(1).all(|paragraph| {
                paragraph.text.trim().is_empty()
                    && paragraph.controls.is_empty()
                    && paragraph.line_segs.len() <= 1
            })
            && child.row_count == 1
            && child.col_count == 1
            && !child.common.treat_as_char
            && child.cells.len() == 1
    }


    /// `RowBreak` scan에서 short parent의 마지막 child row를 분할할 수 있는지
    /// 반환한다. 구조만 맞아도 child가 실제로 한 unit이면 분할할 것이 없으므로,
    /// non-spacer unit이 둘 이상인 것을 함께 확인한다.
    pub(crate) fn native_short_parent_child_row_is_fragmentable(
        &self,
        table: &crate::model::table::Table,
        row: usize,
        styles: &ResolvedStyleSet,
    ) -> bool {
        table
            .cells
            .iter()
            .filter(|cell| cell.row as usize == row && cell.row_span == 1)
            .any(|cell| {
                let Some(host) = cell.paragraphs.first() else {
                    return false;
                };
                let children: Vec<&crate::model::table::Table> = host
                    .controls
                    .iter()
                    .filter_map(|control| match control {
                        Control::Table(child) => Some(child.as_ref()),
                        _ => None,
                    })
                    .collect();
                let Some(child) = children.as_slice().first().copied() else {
                    return false;
                };
                let eligible = self.native_short_parent_child_fragment_eligible(
                    table,
                    cell,
                    child,
                    self.nested_table_mixed_fragment_heights(child, styles)
                        .iter()
                        .map(|fragment| fragment.height)
                        .sum::<f64>(),
                );
                if children.len() != 1 || !eligible {
                    return false;
                }
                let units = self.cell_units(cell, table, styles);
                units
                    .iter()
                    .filter(|unit| !unit.empty_spacer)
                    .take(2)
                    .count()
                    == 2
            })
    }


    /// [#4128 테스트 전용] cell_units 요약: (para_idx, vis_start, vis_end,
    /// empty_spacer, nested_row).
    #[cfg(test)]
    pub(crate) fn debug_cell_units(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
    ) -> Vec<(usize, usize, usize, bool, Option<usize>)> {
        self.cell_units(cell, table, styles)
            .iter()
            .map(|u| {
                (
                    u.para_idx,
                    u.vis_start,
                    u.vis_end,
                    u.empty_spacer,
                    u.nested_row,
                )
            })
            .collect()
    }


    pub(crate) fn cell_units_uncached(
        &self,
        cell: &crate::model::table::Cell,
        table: &crate::model::table::Table,
        styles: &ResolvedStyleSet,
    ) -> Vec<CellUnit> {
        let (pad_left, pad_right, pad_top, pad_bottom) = self.resolve_cell_padding(cell, table);
        let cell_w = if cell.width < 0x8000_0000 {
            hwpunit_to_px(cell.width as i32, self.dpi) * self.render_table_width_scale(table)
        } else {
            0.0
        };
        // [#2279 axis B 보류] 측정에 렌더의 오버플로 패딩 축소 폭을 적용하는 안은
        // 86712 산식 셀(측정 5줄 vs 렌더·한글 4줄)을 정합시키지만, 어드밴스가 사다리
        // 교정된 문서(80168 pi=1056 r7: 한글 PDF 8줄 실측)에서는 한글이 지키는 패딩을
        // 깨 7줄로 과소(157→156 회귀) — shrink 는 폰트 폭 오차의 문서별 보상재로,
        // 일반화 불가(#2279 코멘트). 측정 폭은 원 패딩 유지.
        let inner_width = (cell_w - pad_left - pad_right).max(0.0);
        let line_seg_is_synthetic = |seg: &crate::model::paragraph::LineSeg| {
            seg.tag & crate::model::paragraph::LineSeg::TAG_IMPLEMENTATION_PROPERTY != 0
        };
        let is_hwp5_stored_frame_rewind = |prev: &crate::model::paragraph::LineSeg,
                                           cur: &crate::model::paragraph::LineSeg|
         -> bool {
            let profile = self.profile.get();
            if (!profile.native_hwp5_layout() && !profile.hwp5_origin_hwpx())
                || line_seg_is_synthetic(prev)
                || line_seg_is_synthetic(cur)
            {
                return false;
            }
            let prev_end = prev.vertical_pos + prev.line_height;
            if cur.vertical_pos < 0 || prev_end <= 0 || cur.vertical_pos >= prev_end {
                return false;
            }
            // HWPX 저장 lineseg의 reset은 중첩 셀 로컬 좌표계일 수 있다
            // (#3637). HWP5 저장 계약 안에서도 작은 내부 표의 로컬 reset
            // (rowbreak-problem-pages.hwp: 9600→0HU)을 페이지 경계로 올리지
            // 않도록 직전 줄이 body 하단 절반에 도달한 경우만 인정한다.
            let body_height = self.current_body_area.get().3;
            let frame_floor = if body_height > 0.0 {
                body_height * 0.5
            } else {
                450.0
            };
            hwpunit_to_px(prev_end, self.dpi) >= frame_floor
        };
        let is_block_rowbreak_table = matches!(
            table.page_break,
            crate::model::table::TablePageBreak::RowBreak
        ) && !table.common.treat_as_char;
        let has_visible_text_with_nested_table =
            self.table_has_visible_text_with_nested_table(table);
        // [Task #700] vpos 동기화 가드와 동일 — 한컴 정상 인코딩(첫 문단 vpos=0) 한정.
        let cell_first_vpos = cell
            .paragraphs
            .first()
            .and_then(|p| p.line_segs.first().map(|s| s.vertical_pos))
            .unwrap_or(-1);
        let cell_has_local_vpos_origin = cell_first_vpos == 0
            || (is_block_rowbreak_table && (0..=500).contains(&cell_first_vpos));
        let preserve_linear_single_cell_vpos = is_block_rowbreak_table
            && table.row_count == 1
            && table.col_count == 1
            && (table.common.vertical_offset as i32) == 0
            && cell_first_vpos >= 0;
        let use_vpos_unit_positions = is_block_rowbreak_table
            && ((table.row_count > 1 && has_visible_text_with_nested_table)
                || preserve_linear_single_cell_vpos);
        let vpos_origin = if preserve_linear_single_cell_vpos {
            cell_first_vpos.max(0)
        } else {
            0
        };
        let normalized_vpos_px = |vertical_pos: i32| -> f64 {
            hwpunit_to_px((vertical_pos - vpos_origin).max(0), self.dpi)
        };
        let para_count = cell.paragraphs.len();
        let cell_has_visible_content = cell
            .paragraphs
            .iter()
            .any(|p| !p.text.trim().is_empty() || !p.controls.is_empty());
        // Native HWP5 RowBreak 표에는 문단 기준 Square/Tight/Through 개체 사이에,
        // 실제 줄간격이 아니라 개체 anchor를 저장한 연속 빈 문단 사다리가 남을 수 있다.
        // 이를 일반 em line으로 누적하면 row cut의 physical footprint가 Hancom보다 커져
        // 다음 개체 owner가 한 page 늦어진다 (59043 p11/p12). 단일 빈 줄은 저자가
        // 의도한 여백일 수 있으므로, 양쪽이 non-inline flow 문단인 2개 이상 run만 대상이다.
        // HWPX/CellBreak/TAC에는 stored-layout 의미가 달라 이 predicate를 적용하지 않는다.
        let native_hwp5_rowbreak_float_ladder =
            self.profile.get().native_hwp5_layout() && is_block_rowbreak_table;
        let plain_empty_paragraph: Vec<bool> = cell
            .paragraphs
            .iter()
            .map(|p| p.text.trim().is_empty() && p.controls.is_empty())
            .collect();
        let other_non_inline_flow_paragraph: Vec<bool> = cell
            .paragraphs
            .iter()
            .map(|p| {
                self.paragraph_cell_other_non_inline_control_heights(&p.controls)
                    .iter()
                    .any(|(_, height)| *height > 0.5)
            })
            .collect();
        let mut units: Vec<CellUnit> = Vec::new();
        let split_non_inline_extra =
            |extra_h: f64, top_and_bottom_h: f64, other_h: f64| -> (f64, f64) {
                if extra_h <= 0.5 {
                    return (0.0, 0.0);
                }
                if top_and_bottom_h <= 0.5 {
                    return (0.0, extra_h);
                }
                if other_h <= 0.5 {
                    return (extra_h, 0.0);
                }
                let total_h = top_and_bottom_h + other_h;
                let top_extra = extra_h * (top_and_bottom_h / total_h);
                (top_extra, extra_h - top_extra)
            };
        let append_fragment_units =
            |units: &mut Vec<CellUnit>, para_idx: usize, mut non_inline_h: f64| {
                const FILLER_UNIT_PX: f64 = 16.0;
                while non_inline_h > 0.5 {
                    let h = non_inline_h.min(FILLER_UNIT_PX);
                    units.push(CellUnit {
                        height: h,
                        hard_break_before: false,
                        stored_frame_break_before: false,
                        vpos_gap_before: false,
                        para_idx,
                        vis_start: 0,
                        vis_end: 0,
                        nested_row: None,
                        nested_table_fragment: None,
                        mixed_nested_fragment: false,
                        mixed_nested_trailing: false,
                        mixed_nested_content_height: 0.0,
                        mixed_nested_recursive: false,
                        mixed_nested_starts_after_table: false,
                        mixed_nested_source_para_idx: None,
                        recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                        top_and_bottom_flow: false,
                        empty_spacer: false,
                        non_inline_control_range: None,
                    });
                    non_inline_h -= h;
                }
            };
        let append_atomic_unit = |units: &mut Vec<CellUnit>, para_idx: usize, non_inline_h: f64| {
            if non_inline_h <= 0.5 {
                return;
            }
            units.push(CellUnit {
                height: non_inline_h,
                hard_break_before: false,
                stored_frame_break_before: false,
                vpos_gap_before: false,
                para_idx,
                vis_start: 0,
                vis_end: 0,
                nested_row: None,
                nested_table_fragment: None,
                mixed_nested_fragment: false,
                mixed_nested_trailing: false,
                mixed_nested_content_height: 0.0,
                mixed_nested_recursive: false,
                mixed_nested_starts_after_table: false,
                mixed_nested_source_para_idx: None,
                recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                top_and_bottom_flow: true,
                empty_spacer: false,
                non_inline_control_range: None,
            });
        };
        let append_non_inline_units = |units: &mut Vec<CellUnit>,
                                       para_idx: usize,
                                       extra_h: f64,
                                       top_and_bottom_h: f64,
                                       other_h: f64|
         -> std::ops::Range<usize> {
            let (top_extra_h, other_extra_h) =
                split_non_inline_extra(extra_h, top_and_bottom_h, other_h);
            // TopAndBottom flow 는 그림/도형이 통째로 다음 조각에 넘어가야 해서 atomic 으로
            // 유지한다. Square/Tight/Through flow 는 텍스트 박스 꼬리가 페이지를 걸쳐
            // 이어질 수 있으므로 기존 fragment 모델을 유지한다.
            let other_start = units.len();
            append_fragment_units(units, para_idx, other_extra_h);
            let other_end = units.len();
            append_atomic_unit(units, para_idx, top_extra_h);
            other_start..other_end
        };
        // 기존 16px generic fragment의 높이·개수·순서는 그대로 두고, 각 fragment가
        // 겹치는 Square/Tight/Through source control range만 복원한다. TopAndBottom
        // atomic unit은 이 metadata의 대상이 아니다.
        let tag_other_non_inline_control_units =
            |units: &mut [CellUnit], range: std::ops::Range<usize>, controls: &[(usize, f64)]| {
                if range.is_empty() || controls.is_empty() {
                    return;
                }
                let source_h: f64 = controls.iter().map(|(_, height)| *height).sum();
                let represented_h: f64 = units[range.clone()].iter().map(|unit| unit.height).sum();
                if source_h <= 0.5 || represented_h <= 0.5 {
                    return;
                }
                // TopAndBottom과 섞인 문단에서는 기존 비례 분할로 other flow가
                // 축소되어 있으므로, current unit 좌표를 source other-flow 좌표로
                // 환산한 뒤 겹치는 control 범위를 기록한다.
                let scale = represented_h / source_h;
                let mut rendered_offset = 0.0;
                for unit in &mut units[range] {
                    let source_start = rendered_offset / scale;
                    let source_end = (rendered_offset + unit.height) / scale;
                    let mut control_start = 0.0;
                    let mut first = None;
                    let mut last = None;
                    for (control_idx, control_h) in controls {
                        let control_end = control_start + control_h;
                        if control_end > source_start + 0.001 && control_start < source_end - 0.001
                        {
                            first.get_or_insert(*control_idx);
                            last = Some(*control_idx);
                        }
                        control_start = control_end;
                    }
                    unit.non_inline_control_range = first.zip(last);
                    rendered_offset += unit.height;
                }
            };
        for (pi, p) in cell.paragraphs.iter().enumerate() {
            let is_block_rowbreak = matches!(
                table.page_break,
                crate::model::table::TablePageBreak::RowBreak
            ) && !table.common.treat_as_char;
            let (para_top_and_bottom_h, summed_para_other_non_inline_h) =
                self.paragraph_cell_non_inline_control_flow_parts(&p.controls);
            let para_other_non_inline_h =
                if native_hwp5_rowbreak_float_ladder && p.text.trim().is_empty() {
                    self.paragraph_parallel_other_non_inline_flow_band_height(&p.controls)
                        .unwrap_or(summed_para_other_non_inline_h)
                } else {
                    summed_para_other_non_inline_h
                };
            let para_other_non_inline_controls =
                self.paragraph_cell_other_non_inline_control_heights(&p.controls);
            let para_non_inline_h = para_top_and_bottom_h + para_other_non_inline_h;
            let mut comp = compose_paragraph(p);
            crate::renderer::composer::recompose_for_cell_width(&mut comp, p, inner_width, styles);
            // [#2291] 부실 저장(ls==1 인데 실폭 초과) 문단 재분할 — 가로쓰기 셀 한정.
            if cell.text_direction == 0 {
                crate::renderer::composer::recompose_stored_single_line_if_overflowing(
                    &mut comp,
                    p,
                    inner_width,
                    styles,
                );
            }
            let para_style = styles.para_styles.get(p.para_shape_id as usize);
            let is_empty_spacer_para = p.text.trim().is_empty() && p.controls.is_empty();
            let preserve_forward_stored_empty_spacer = {
                let profile = self.profile.get();
                (profile.native_hwp5_layout() || profile.hwp5_origin_hwpx())
                    && is_empty_spacer_para
                    && matches!(p.line_segs.as_slice(), [seg] if !line_seg_is_synthetic(seg))
                    && match (p.line_segs.first(), cell.paragraphs.get(pi + 1)) {
                        (Some(seg), Some(next_para)) if next_para.controls.is_empty() => {
                            match next_para.line_segs.first() {
                                Some(next) if !line_seg_is_synthetic(next) => {
                                    let full_line_box = seg.line_height > 0
                                        && next.line_height > 0
                                        && i64::from(seg.line_height) * 4
                                            >= i64::from(next.line_height) * 3;
                                    full_line_box
                                        && next.vertical_pos >= seg.vertical_pos + seg.line_height
                                }
                                _ => false,
                            }
                        }
                        _ => false,
                    }
            };
            let preserve_vpos_empty_spacer = is_empty_spacer_para
                && ((preserve_linear_single_cell_vpos
                    && p.line_segs.len() == 1
                    && p.line_segs
                        .first()
                        .is_some_and(|seg| seg.vertical_pos >= cell_first_vpos))
                    // #2430 물리 16쪽의 1×1 비인라인 표는 vertical_offset이
                    // 1801HU라 선형 vpos 모드가 아니지만, p[0] 빈 Enter 뒤의
                    // p[1]이 0→1800HU로 전진하고 빈 줄 높이도 다음 본문 줄의
                    // 82%다. 이 저장 순방향 full-line box는 overlay가 아니므로
                    // 0높이로 접지 않는다. 반면 작은 장식 간격용 빈 문단은 기존
                    // collapse를 유지한다(hwpx_sample2.hwp: 500~600/1000HU).
                    // 다음 문단이 중첩 control을 host하면 그 vpos는 control 배치
                    // 좌표이므로 빈 Enter의 독립 줄박스 증거로 사용하지 않는다.
                    || preserve_forward_stored_empty_spacer);
            let legacy_single_cell_empty_spacer = is_block_rowbreak
                && table.row_count == 1
                && table.col_count == 1
                && is_empty_spacer_para
                && cell_has_visible_content
                && !preserve_vpos_empty_spacer;
            let collapse_native_float_ladder_spacer = if native_hwp5_rowbreak_float_ladder
                && is_empty_spacer_para
                && cell_has_visible_content
            {
                let run_start = (0..pi)
                    .rev()
                    .find(|&idx| !plain_empty_paragraph[idx])
                    .map_or(0, |idx| idx + 1);
                let run_end = ((pi + 1)..para_count)
                    .find(|&idx| !plain_empty_paragraph[idx])
                    .unwrap_or(para_count);
                run_end - run_start >= 2
                    && run_start > 0
                    && run_end < para_count
                    && other_non_inline_flow_paragraph[run_start - 1]
                    && other_non_inline_flow_paragraph[run_end]
            } else {
                false
            };
            let collapse_empty_rowbreak_spacer =
                legacy_single_cell_empty_spacer || collapse_native_float_ladder_spacer;
            let is_last_para = pi + 1 == para_count;
            // [Task #1488] 가시 텍스트 문단 여부 — 비가시(빈) 오버레이 스페이서 문단이 만든
            // vpos 리셋을 하드 브레이크(강제 페이지 분할)에서 제외하기 위한 게이트.
            // 가시 텍스트 문단 사이 리셋(Task #993 의도)은 그대로 하드 브레이크로 보존한다.
            let para_has_visible_text = p.text.chars().any(|c| c > '\u{001F}' && c != '\u{FFFC}');
            let para_uses_synthetic_line_segs =
                !p.line_segs.is_empty() && p.line_segs.iter().all(|seg| line_seg_is_synthetic(seg));
            let raw_spacing_before = para_style.map(|s| s.spacing_before).unwrap_or(0.0);
            let spacing_before = if pi > 0 {
                raw_spacing_before
            } else if self.profile.get().hwpx_stored_layout()
                && is_block_rowbreak
                && para_uses_synthetic_line_segs
            {
                // HWPX 에서 lineSegArray 가 누락된 표 셀 문단은 reflow 로 합성되지만,
                // ParaShape 의 spacing_before 는 여전히 문서 속성이다. 저장 HWP 는
                // 첫 줄 vpos 에 이 값을 반영하므로 row cut 측정도 같은 값을 사용한다.
                raw_spacing_before
            } else if raw_spacing_before > 0.0 {
                let first_vpos = p
                    .line_segs
                    .first()
                    .map(|ls| hwpunit_to_px(ls.vertical_pos, self.dpi))
                    .unwrap_or(0.0)
                    .max(0.0);
                raw_spacing_before.min(first_vpos)
            } else {
                0.0
            };
            let spacing_after = if !is_last_para {
                para_style.map(|s| s.spacing_after).unwrap_or(0.0)
            } else {
                0.0
            };
            // vpos 리셋 검출: 직전 문단 끝보다 현재 문단 시작 vpos 가 작으면 리셋.
            let reset_before = if pi > 0 && cell_has_local_vpos_origin {
                let prev = &cell.paragraphs[pi - 1];
                match (prev.line_segs.last(), p.line_segs.first()) {
                    (Some(prev_seg), Some(cur_seg))
                        if !line_seg_is_synthetic(prev_seg) && !line_seg_is_synthetic(cur_seg) =>
                    {
                        let prev_end = prev_seg.vertical_pos + prev_seg.line_height;
                        cur_seg.vertical_pos >= 0 && prev_end > 0 && cur_seg.vertical_pos < prev_end
                    }
                    _ => false,
                }
            } else {
                false
            };
            // #2430 p14의 비선형 부모 셀에는 한컴이 무시하는 빈 Enter가 있고,
            // 그 단일 lineseg도 다음 저장 좌표 0으로 rewind한다. 이를 프레임
            // 경계로 올리면 39쪽 정본이 38쪽으로 줄어든다. 실제 빈 문단은
            // #4069처럼 1×1 선형 부모의 저장 vpos를 보존하는 경우에만 증거로 쓴다.
            let stored_frame_tail_before_next_para = if cell_has_local_vpos_origin {
                match (p.line_segs.last(), cell.paragraphs.get(pi + 1)) {
                    (Some(prev), Some(next))
                        if !next.text.is_empty()
                            || !next.controls.is_empty()
                            || (preserve_linear_single_cell_vpos
                                && next.text.is_empty()
                                && next.controls.is_empty()
                                && next.line_segs.len() == 1
                                && next.line_segs.first().is_some_and(|seg| {
                                    seg.vertical_pos >= cell_first_vpos
                                        && !line_seg_is_synthetic(seg)
                                })) =>
                    {
                        next.line_segs
                            .first()
                            .is_some_and(|cur| is_hwp5_stored_frame_rewind(prev, cur))
                    }
                    _ => false,
                }
            } else {
                false
            };
            let stored_frame_break_before_para = if pi > 0 && cell_has_local_vpos_origin {
                let prev_para = &cell.paragraphs[pi - 1];
                match (prev_para.line_segs.last(), p.line_segs.first()) {
                    (Some(prev), Some(cur)) => is_hwp5_stored_frame_rewind(prev, cur),
                    _ => false,
                }
            } else {
                false
            };
            let prev_para_has_mixed_nested_table = if pi > 0 {
                let prev = &cell.paragraphs[pi - 1];
                !prev.text.trim().is_empty()
                    && prev.controls.iter().any(|c| matches!(c, Control::Table(_)))
            } else {
                false
            };
            let vpos_gap_threshold_hu = (12.0 / self.dpi * 7200.0).round() as i32;
            let vpos_gap_before_para = if use_vpos_unit_positions && pi > 0 && cell_first_vpos == 0
            {
                let prev = &cell.paragraphs[pi - 1];
                match (prev.line_segs.last(), p.line_segs.first()) {
                    (Some(prev_seg), Some(cur_seg))
                        if !line_seg_is_synthetic(prev_seg) && !line_seg_is_synthetic(cur_seg) =>
                    {
                        let prev_end =
                            prev_seg.vertical_pos + prev_seg.line_height + prev_seg.line_spacing;
                        cur_seg.vertical_pos >= 0
                            && prev_end > 0
                            && cur_seg.vertical_pos > prev_end + vpos_gap_threshold_hu
                    }
                    _ => false,
                }
            } else {
                false
            };
            let line_reset_before = |li: usize| -> bool {
                if li == 0 {
                    return reset_before;
                }
                if !cell_has_local_vpos_origin {
                    return false;
                }
                let Some(prev) = p.line_segs.get(li - 1) else {
                    return false;
                };
                let Some(cur) = p.line_segs.get(li) else {
                    return false;
                };
                if line_seg_is_synthetic(prev) || line_seg_is_synthetic(cur) {
                    return false;
                }
                let prev_end = prev.vertical_pos + prev.line_height;
                cur.vertical_pos >= 0 && prev_end > 0 && cur.vertical_pos < prev_end
            };
            let stored_frame_break_before = |li: usize| -> bool {
                if li == 0 {
                    return stored_frame_break_before_para;
                }
                if !line_reset_before(li) {
                    return false;
                }
                let Some(prev) = p.line_segs.get(li - 1) else {
                    return false;
                };
                let Some(cur) = p.line_segs.get(li) else {
                    return false;
                };
                // 42065 p10의 같은 문단 58620→0HU도 위의 HWP5 저장 프레임
                // 판정과 같은 계약을 사용한다.
                is_hwp5_stored_frame_rewind(prev, cur)
            };
            // [Task #993] 줄 높이는 렌더러(layout_composed_paragraph)와 동일하게
            // corrected_line_height 를 적용한다 — raw line_height 가 폰트보다
            // 작은 폴백 케이스에서 렌더러가 키운 높이를 컷 측정이 따라가지
            // 못하면 분할 표가 페이지를 넘는다(측정 공간 불일치).
            // [#2070 실험] 셀 마지막 줄 인덱스 - em 공식 게이트.
            let cell_last_line_idx = if is_last_para && !comp.lines.is_empty() {
                Some(comp.lines.len() - 1)
            } else {
                None
            };
            let corrected_h = |line: &ComposedLine, li: usize| -> f64 {
                let raw_lh = hwpunit_to_px(line.line_height, self.dpi);
                // [Task #1811] HWPX RowBreak 셀의 synthetic lineSeg 는 저장 근거가 아니라
                // reflow 산물이다. row cut 측정에서 다시 corrected_line_height 를 적용하면
                // HWP 기준보다 줄 유닛이 커져 p4→p5 split 이 한 유닛 빨라진다.
                if self.profile.get().hwpx_stored_layout()
                    && is_block_rowbreak
                    && para_uses_synthetic_line_segs
                {
                    return raw_lh;
                }
                // [#2112] 실제 저장 LINE_SEG 를 보유한 셀 문단은 저장 줄높이를 신뢰한다.
                // 한글은 압축 줄높이(lh < 글자크기)를 저장값대로 렌더하는데 corrected
                // 보정이 fs×줄간격% 로 대체해 행높이가 부풀었다(39607: 행별 +3.8~
                // +76.8px, 표 합계 +335px → 다쪽 표 쪽수 밀림). 보정은 lineseg 부재
                // 폴백(#674/#993 원 목적)에만 유지.
                if p.line_segs.iter().any(|ls| !line_seg_is_synthetic(ls)) {
                    return raw_lh;
                }
                match para_style {
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
                        // [#2169] NO_LS 순수 빈 문단(runs 없음 → max_fs=0)은 한글이
                        // 완전한 em 줄박스로 취급(80168 r4: 한글 = 10줄×em + 9gap 정확).
                        // 문단 char shape fs 로 폴백 — 컨트롤 앵커 문단은 제외(r6 중첩).
                        let max_fs = if max_fs <= 0.0
                            && crate::renderer::para_has_no_stored_line_segs(p)
                            && p.controls.is_empty()
                        {
                            p.char_shapes
                                .first()
                                .and_then(|cs| styles.char_styles.get(cs.char_shape_id as usize))
                                .map(|cs| cs.font_size)
                                .unwrap_or(0.0)
                        } else {
                            max_fs
                        };
                        // [Issue #1842] 부재 LINE_SEG 셀의 placeholder(400)→corrected
                        // max_fs*ls% 팽창을 em 으로 교정 — CellBreak 표.
                        // [#2150/#2169] 일반화: 한글 NO_LS fresh 공식 — 비마지막 줄
                        // fs×ls% 동치 + 셀 마지막 줄만 em (ls 사다리 + 80168 per-row 확정).
                        crate::renderer::corrected_line_height_for_variant_synthetic(
                            raw_lh,
                            max_fs,
                            ps.line_spacing_type,
                            ps.line_spacing,
                            crate::renderer::para_has_no_stored_line_segs(p)
                                && (!p.text.is_empty() || p.controls.is_empty())
                                && (matches!(table.page_break, TablePageBreak::CellBreak)
                                    // [#2070 실험] 셀 마지막 줄 = em (5축 전면).
                                    || cell_last_line_idx == Some(li)),
                        )
                    }
                    None => raw_lh,
                }
            };
            let has_table_in_para = p.controls.iter().any(|c| matches!(c, Control::Table(_)));
            let para_has_top_and_bottom_non_inline_control =
                p.controls.iter().any(|control| match control {
                    Control::Picture(pic) => matches!(pic.common.text_wrap, TextWrap::TopAndBottom),
                    Control::Shape(shape) => {
                        let common = shape.common();
                        matches!(common.text_wrap, TextWrap::TopAndBottom)
                    }
                    _ => false,
                });
            let line_count = comp.lines.len();
            let line_core_height: f64 = comp
                .lines
                .iter()
                .enumerate()
                .map(|(li, line)| corrected_h(line, li))
                .sum();
            let para_non_inline_extra_h = if p.text.trim().is_empty() && line_count > 0 {
                (para_non_inline_h - line_core_height).max(0.0)
            } else {
                para_non_inline_h
            };
            let para_top_and_bottom_flow_unit =
                para_has_top_and_bottom_non_inline_control && !para_has_visible_text;
            let previous_single_empty_unit_idx = if pi > 0
                && plain_empty_paragraph[pi - 1]
                && units
                    .last()
                    .is_some_and(|unit| unit.para_idx == pi - 1 && unit.empty_spacer)
                && (units.len() == 1 || units[units.len() - 2].para_idx != pi - 1)
            {
                Some(units.len() - 1)
            } else {
                None
            };
            let is_exact_recursive_prelude = line_count == 1
                && para_has_visible_text
                && !has_table_in_para
                && previous_single_empty_unit_idx.is_some()
                && cell
                    .paragraphs
                    .get(pi + 1)
                    .is_some_and(Self::paragraph_hosts_single_cell_nested_table);
            if is_exact_recursive_prelude {
                let separator_idx = previous_single_empty_unit_idx.expect("checked above");
                let separator_is_explicit_page_break = matches!(
                    cell.paragraphs[pi - 1].column_type,
                    crate::model::paragraph::ColumnBreakType::Page
                        | crate::model::paragraph::ColumnBreakType::Section
                );
                units[separator_idx].recursive_block_prelude_role =
                    if separator_is_explicit_page_break {
                        RecursiveBlockPreludeRole::ExplicitPageBreakSeparator
                    } else {
                        RecursiveBlockPreludeRole::EmptySeparator
                    };
            }
            let mut unit_cum = units.iter().map(|u| u.height).sum::<f64>();
            // [Task #1073] 텍스트 없는 문단(가시 텍스트 없음 — 합성 줄은 placeholder)에 단일
            // 중첩 표가 있고 그 표가 2행 이상이면 per-중첩행 유닛으로 분해 — advance_row_cut 가
            // 중첩 표 행 경계에서 페이지 분할할 수 있게 한다. whole-row 높이 합은
            // calc_nested_table_height 와 정확히 일치(드리프트 0):
            // Σ row_h + cs*(n-1) + om_top + om_bottom + spacing.
            // 2단계+ 중첩/텍스트 동거 문단은 아래 atom 폴백 유지(범위 외).
            if has_table_in_para && p.text.trim().is_empty() {
                let nested_tables: Vec<&crate::model::table::Table> = p
                    .controls
                    .iter()
                    .filter_map(|c| match c {
                        Control::Table(t) => Some(t.as_ref()),
                        _ => None,
                    })
                    .collect();
                if nested_tables.len() == 1
                    && nested_tables[0].row_count >= 2
                    && !matches!(
                        nested_tables[0].page_break,
                        crate::model::table::TablePageBreak::None
                    )
                {
                    let nt = nested_tables[0];
                    let ncol = nt.col_count as usize;
                    let nrow = nt.row_count as usize;
                    // 분할 컷은 저장된 표 높이보다 실제 콘텐츠 높이를 기준으로 잡아야
                    // page-larger 중첩 표가 한컴처럼 행 단위로 이어진다.
                    // [#2148/#2169] NO_LS 중첩 표(왕복 synthetic 포함)만 선언-fit
                    // (fit_row_heights_to_common_height, 성장 전용) — 저장 lineseg
                    // 문서는 #1073 콘텐츠 기준 유지 (자기-export HWPX 왕복 정합).
                    let nt_all_no_ls = nt
                        .cells
                        .iter()
                        .all(|c| c.paragraphs.iter().all(|p| p.line_segs.is_empty()));
                    let rhs = if nt_all_no_ls {
                        self.resolve_row_heights(nt, ncol, nrow, None, styles, true)
                    } else {
                        self.resolve_row_heights_for_content(nt, ncol, nrow, None, styles, true)
                    };
                    let ncs = hwpunit_to_px(nt.cell_spacing as i32, self.dpi);
                    let om_top = hwpunit_to_px(nt.outer_margin_top as i32, self.dpi);
                    let om_bot = hwpunit_to_px(nt.outer_margin_bottom as i32, self.dpi);
                    for (ri, rh) in rhs.iter().enumerate() {
                        // [#4069] CELL 분할 중첩 표는 큰 행을 단일 atom으로 바깥
                        // 원장에 올리지 않는다. 행에서 콘텐츠가 가장 높은 셀의 unit
                        // 경계를 공통 높이 축으로 삼고, 각 경계에서 모든 셀의 누적
                        // cursor를 기록한다. 따라서 첫 조각과 continuation 모두 같은
                        // 자식 RowCut을 렌더러에 전달할 수 있다.
                        if matches!(nt.page_break, TablePageBreak::RowBreak) {
                            let mut row_cells: Vec<&crate::model::table::Cell> = nt
                                .cells
                                .iter()
                                .filter(|cell| cell.row as usize == ri && cell.row_span == 1)
                                .collect();
                            row_cells.sort_by_key(|cell| cell.col);
                            let row_is_auto_height = !row_cells.is_empty()
                                && row_cells.iter().all(|cell| cell.height == 0);
                            let row_has_crossing_span = nt.cells.iter().any(|cell| {
                                let start = cell.row as usize;
                                let end = start + (cell.row_span as usize).max(1);
                                cell.row_span > 1 && start <= ri && ri < end
                            });
                            let row_units: Vec<std::sync::Arc<Vec<CellUnit>>> = row_cells
                                .iter()
                                .map(|cell| self.cell_units(cell, nt, styles))
                                .collect();
                            let driver = row_units
                                .iter()
                                .enumerate()
                                .filter(|(_, cell_units)| !cell_units.is_empty())
                                .max_by(|(_, a), (_, b)| {
                                    let ah: f64 = a.iter().map(|unit| unit.height).sum();
                                    let bh: f64 = b.iter().map(|unit| unit.height).sum();
                                    ah.total_cmp(&bh)
                                })
                                .map(|(index, _)| index);

                            if let Some(driver_index) = driver.filter(|driver_index| {
                                row_is_auto_height
                                    && !row_has_crossing_span
                                    && row_units[*driver_index].len() > 1
                            }) {
                                let driver_units = &row_units[driver_index];
                                let driver_total: f64 =
                                    driver_units.iter().map(|unit| unit.height).sum();
                                let row_extra = (*rh - driver_total).max(0.0);
                                let mut driver_before = 0.0;

                                for (fragment_index, driver_unit) in driver_units.iter().enumerate()
                                {
                                    let driver_after = driver_before + driver_unit.height;
                                    let cuts_at = |height: f64| -> RowCut {
                                        row_units
                                            .iter()
                                            .map(|cell_units| {
                                                let mut consumed = 0.0;
                                                let mut count = 0usize;
                                                while count < cell_units.len()
                                                    && consumed + cell_units[count].height
                                                        <= height + 0.1
                                                {
                                                    consumed += cell_units[count].height;
                                                    count += 1;
                                                }
                                                count
                                            })
                                            .collect()
                                    };
                                    let start_cut = cuts_at(driver_before);
                                    let end_cut = cuts_at(driver_after);
                                    let terminal = row_units
                                        .iter()
                                        .zip(end_cut.iter())
                                        .all(|(cell_units, end)| *end >= cell_units.len());
                                    let mut uh = driver_unit.height;
                                    if fragment_index == 0 {
                                        uh += row_extra * 0.5;
                                    }
                                    if fragment_index + 1 == driver_units.len() {
                                        uh += row_extra - row_extra * 0.5;
                                        if ri + 1 < nrow {
                                            uh += ncs;
                                        }
                                        if ri + 1 == nrow {
                                            uh += om_bot + spacing_after;
                                        }
                                    }
                                    if ri == 0 && fragment_index == 0 {
                                        uh += om_top + spacing_before;
                                    }

                                    let mut hard_break_before = driver_unit.hard_break_before
                                        || (reset_before && ri == 0 && fragment_index == 0);
                                    let mut stored_frame_break_before =
                                        driver_unit.stored_frame_break_before;
                                    let mut vpos_gap_before =
                                        vpos_gap_before_para && ri == 0 && fragment_index == 0;
                                    for ((cell_units, start), end) in
                                        row_units.iter().zip(start_cut.iter()).zip(end_cut.iter())
                                    {
                                        if end > start {
                                            if cell_units
                                                .get(*start)
                                                .is_some_and(|unit| unit.hard_break_before)
                                            {
                                                hard_break_before = true;
                                            }
                                            if cell_units
                                                .get(*start)
                                                .is_some_and(|unit| unit.stored_frame_break_before)
                                            {
                                                stored_frame_break_before = true;
                                            }
                                            if cell_units
                                                .get(*start)
                                                .is_some_and(|unit| unit.vpos_gap_before)
                                            {
                                                vpos_gap_before = true;
                                            }
                                        }
                                    }
                                    if use_vpos_unit_positions
                                        && ri == 0
                                        && fragment_index == 0
                                        && !hard_break_before
                                    {
                                        if let Some(seg) = p.line_segs.first() {
                                            let target_top = normalized_vpos_px(seg.vertical_pos);
                                            if target_top > unit_cum {
                                                uh += target_top - unit_cum;
                                                vpos_gap_before = true;
                                            }
                                        }
                                    }

                                    units.push(CellUnit {
                                        height: uh,
                                        hard_break_before,
                                        stored_frame_break_before,
                                        vpos_gap_before,
                                        para_idx: pi,
                                        vis_start: 0,
                                        vis_end: line_count.max(1),
                                        nested_row: Some(ri),
                                        nested_table_fragment: Some(NestedTableUnitCut {
                                            start_cut,
                                            end_cut,
                                            terminal,
                                        }),
                                        mixed_nested_fragment: false,
                                        mixed_nested_trailing: false,
                                        mixed_nested_content_height: 0.0,
                                        mixed_nested_recursive: false,
                                        mixed_nested_starts_after_table: false,
                                        mixed_nested_source_para_idx: None,
                                        recursive_block_prelude_role:
                                            RecursiveBlockPreludeRole::None,
                                        top_and_bottom_flow: false,
                                        empty_spacer: false,
                                        non_inline_control_range: None,
                                    });
                                    unit_cum += uh;
                                    driver_before = driver_after;
                                }
                                continue;
                            }
                        }

                        let mut uh = *rh;
                        let hard_break_before = reset_before && ri == 0;
                        let mut vpos_gap_before = vpos_gap_before_para && ri == 0;
                        if use_vpos_unit_positions && ri == 0 && !hard_break_before {
                            if let Some(seg) = p.line_segs.first() {
                                let target_top = normalized_vpos_px(seg.vertical_pos);
                                if target_top > unit_cum {
                                    uh += target_top - unit_cum;
                                    vpos_gap_before = true;
                                }
                            }
                        }
                        if ri + 1 < nrow {
                            uh += ncs;
                        }
                        if ri == 0 {
                            uh += om_top + spacing_before;
                        }
                        if ri + 1 == nrow {
                            uh += om_bot + spacing_after;
                        }
                        units.push(CellUnit {
                            height: uh,
                            hard_break_before,
                            stored_frame_break_before: false,
                            vpos_gap_before,
                            para_idx: pi,
                            vis_start: 0,
                            vis_end: line_count.max(1),
                            nested_row: Some(ri),
                            nested_table_fragment: None,
                            mixed_nested_fragment: false,
                            mixed_nested_trailing: false,
                            mixed_nested_content_height: 0.0,
                            mixed_nested_recursive: false,
                            mixed_nested_starts_after_table: false,
                            mixed_nested_source_para_idx: None,
                            recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                            top_and_bottom_flow: false,
                            empty_spacer: false,
                            non_inline_control_range: None,
                        });
                        unit_cum += uh;
                    }
                    let non_inline_range = append_non_inline_units(
                        &mut units,
                        pi,
                        para_non_inline_extra_h,
                        para_top_and_bottom_h,
                        para_other_non_inline_h,
                    );
                    tag_other_non_inline_control_units(
                        &mut units,
                        non_inline_range,
                        &para_other_non_inline_controls,
                    );
                    continue;
                } else if nested_tables.len() == 1 && nested_tables[0].row_count == 1 {
                    // [#2007] 1×1(단일 행) 중첩 표: per-중첩행 분해(row_count>=2)가 불가하나,
                    // 그 단일 셀 콘텐츠가 페이지보다 크면(42065 pi=7: 135문단 8164px) atomic 으로
                    // 두면 못 쪼개져 under-pagination. 텍스트+중첩표 문단에 쓰이는
                    // nested_table_mixed_fragment_heights(단일 행 셀 문단을 페이지 분할 가능한
                    // fragment 로 분해)를 빈-텍스트 문단에도 적용해 splittable 유닛으로 산출.
                    let nt = nested_tables[0];
                    let frags = self.nested_table_mixed_fragment_heights(nt, styles);
                    if std::env::var("RHWP_DIAG_NESTED_OWNER").is_ok()
                        && nt.cells.len() == 1
                        && nt.cells[0].paragraphs.iter().any(|paragraph| {
                            paragraph
                                .controls
                                .iter()
                                .any(|control| matches!(control, Control::Table(_)))
                        })
                    {
                        eprintln!(
                            "DIAG_NESTED_OWNER parent_pi={pi} child_paras={} fragments={}",
                            nt.cells[0].paragraphs.len(),
                            frags.len(),
                        );
                        for (fragment_index, fragment) in frags.iter().enumerate() {
                            let is_child_table = fragment
                                .source_para_idx
                                .and_then(|source_pi| nt.cells[0].paragraphs.get(source_pi))
                                .is_some_and(|paragraph| {
                                    paragraph
                                        .controls
                                        .iter()
                                        .any(|control| matches!(control, Control::Table(_)))
                                });
                            eprintln!(
                                "  unit={fragment_index} source={:?} table={} h={:.1} trailing={} after_table={}",
                                fragment.source_para_idx,
                                is_child_table,
                                fragment.height,
                                fragment.trailing,
                                fragment.starts_after_table,
                            );
                        }
                    }
                    // 게이트: 콘텐츠가 **명백히 여러 페이지가 필요**(≥ MULTI_PAGE_PX)할 때만
                    // fragment 분해한다. 임계를 넉넉히(≈2 페이지) 두는 이유:
                    // - 한 페이지에 맞는 1×1 중첩 표(서식): fragment 렌더 미세차로 회귀(form-002).
                    // - 1~2 페이지 경계선 표(76076 규제영향분석서의 여러 ~1000px 중첩셀): fragment
                    //   경계가 기존 배치와 ±1 어긋나 공식 PDF 쪽수(issue_1891) 회귀.
                    // 42065 pi=7(8164px, 8쪽분)·2781515 별표(수쪽분)처럼 ≫ 2페이지인 거대 셀만 대상.
                    let page_avail = self.current_body_area.get().3;
                    let multi_page_px = if page_avail > 0.0 {
                        page_avail * 1.0
                    } else {
                        900.0
                    };
                    let total_frag_h: f64 = frags.iter().map(|fragment| fragment.height).sum();
                    // 저장된 fragment 높이가 표의 물리 행 높이보다 작을 수 있다. 특히
                    // 59043 p35/p36의 1×1 child는 fragment 합이 body보다 3.6px 작지만
                    // 실제 행 기하는 1336px라 한 쪽에 들어가지 않는다. fragment 합만
                    // 보면 이 표를 atom으로 소비해 p36 source owner가 사라진다.
                    // page의 남은 공간이 아니라 문서 고유 물리 높이만 사용해
                    // `cell_units_cache`가 page context에 의존하지 않게 한다.
                    let physical_nested_h = self.calc_nested_table_height(nt, styles);
                    let exceeds_physical_page = physical_nested_h > multi_page_px + 0.5;
                    // child content flow가 parent 선언 높이보다 큰 native short-parent
                    // 구조에서만 1×1 child를 fragment unit으로 전개한다. `common.height`
                    // 자체는 stale viewport일 수 있으므로, 같은 mixed fragment 원장의
                    // 합으로 판단한다. paginator도 같은 helper로 이 row만 split 가능으로
                    // 올린다 (76076 p81→82).
                    let native_short_parent_child_fragment = self
                        .native_short_parent_child_fragment_eligible(
                            table,
                            cell,
                            nt,
                            total_frag_h.max(physical_nested_h),
                        );
                    // [#4069 Stage 3] 한 페이지 이하 1×1 자식 표라도 host line이
                    // 저장 프레임 하단까지 차지하고 다음 문단이 새 프레임으로
                    // rewind하면 현재 쪽의 남은 공간에서 시작해야 한다. 원자 처리하면
                    // 42065 p15의 `조달청` 다음 표가 통째로 p16으로 밀린다.
                    // HWP5 저장 경계로 확인된 경우만 열어 form-002/#1891의 일반
                    // 단일 페이지 중첩 표 배치는 유지한다.
                    if frags.len() > 1
                        && (total_frag_h > multi_page_px
                            || exceeds_physical_page
                            || native_short_parent_child_fragment
                            || stored_frame_tail_before_next_para)
                    {
                        let om_top = hwpunit_to_px(nt.outer_margin_top as i32, self.dpi);
                        let om_bot = hwpunit_to_px(nt.outer_margin_bottom as i32, self.dpi);
                        let n = frags.len();
                        for (fi, fragment) in frags.into_iter().enumerate() {
                            let mut uh = fragment.height;
                            let hard_break_before =
                                fragment.hard_break_before || (reset_before && fi == 0);
                            let mut vpos_gap_before = vpos_gap_before_para && fi == 0;
                            if use_vpos_unit_positions && fi == 0 && !hard_break_before {
                                if let Some(seg) = p.line_segs.first() {
                                    let target_top = normalized_vpos_px(seg.vertical_pos);
                                    if target_top > unit_cum {
                                        uh += target_top - unit_cum;
                                        vpos_gap_before = true;
                                    }
                                }
                            }
                            if fi == 0 {
                                uh += om_top + spacing_before;
                            }
                            if fi + 1 == n {
                                uh += om_bot + spacing_after;
                            }
                            units.push(CellUnit {
                                height: uh,
                                hard_break_before,
                                stored_frame_break_before: fragment.stored_frame_break_before,
                                vpos_gap_before,
                                para_idx: pi,
                                vis_start: line_count,
                                vis_end: line_count,
                                nested_row: None,
                                nested_table_fragment: None,
                                mixed_nested_fragment: true,
                                mixed_nested_trailing: fragment.trailing,
                                mixed_nested_content_height: fragment.content_height,
                                mixed_nested_recursive: fragment.recursive,
                                mixed_nested_starts_after_table: fragment.starts_after_table,
                                mixed_nested_source_para_idx: fragment.source_para_idx,
                                recursive_block_prelude_role: fragment.recursive_block_prelude_role,
                                top_and_bottom_flow: false,
                                empty_spacer: false,
                                non_inline_control_range: None,
                            });
                            unit_cum += uh;
                        }
                        let non_inline_range = append_non_inline_units(
                            &mut units,
                            pi,
                            para_non_inline_extra_h,
                            para_top_and_bottom_h,
                            para_other_non_inline_h,
                        );
                        tag_other_non_inline_control_units(
                            &mut units,
                            non_inline_range,
                            &para_other_non_inline_controls,
                        );
                        continue;
                    }
                }
            }
            if has_table_in_para && !p.text.trim().is_empty() && line_count > 0 {
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
                if nested_h > 0.0 {
                    for (li, line) in comp.lines.iter().enumerate() {
                        let h = corrected_h(line, li);
                        let ls = hwpunit_to_px(line.line_spacing, self.dpi);
                        let is_cell_last_line = is_last_para && li + 1 == line_count;
                        let is_block_rowbreak = matches!(
                            table.page_break,
                            crate::model::table::TablePageBreak::RowBreak
                        ) && !table.common.treat_as_char;
                        let include_trailing_ls = !is_cell_last_line || para_count > 1;
                        let include_trailing_ls =
                            include_trailing_ls && (!is_cell_last_line || !is_block_rowbreak);
                        let mut lh = if include_trailing_ls { h + ls } else { h };
                        if li == 0 {
                            lh += spacing_before;
                        }
                        if li == line_count - 1 {
                            lh += spacing_after;
                        }
                        let hard_break_before = line_reset_before(li);
                        let mut vpos_gap_before = if li == 0 {
                            vpos_gap_before_para
                        } else if use_vpos_unit_positions && cell_first_vpos == 0 {
                            match (p.line_segs.get(li - 1), p.line_segs.get(li)) {
                                (Some(prev), Some(cur))
                                    if !line_seg_is_synthetic(prev)
                                        && !line_seg_is_synthetic(cur) =>
                                {
                                    cur.vertical_pos
                                        > prev.vertical_pos
                                            + prev.line_height
                                            + prev.line_spacing
                                            + vpos_gap_threshold_hu
                                }
                                _ => false,
                            }
                        } else {
                            false
                        };
                        if use_vpos_unit_positions {
                            if let Some(seg) = p.line_segs.get(li) {
                                if !line_seg_is_synthetic(seg) {
                                    let target_top = normalized_vpos_px(seg.vertical_pos);
                                    if target_top > unit_cum {
                                        lh += target_top - unit_cum;
                                        vpos_gap_before = true;
                                    }
                                }
                            }
                        }
                        units.push(CellUnit {
                            height: lh,
                            hard_break_before,
                            stored_frame_break_before: stored_frame_break_before(li),
                            vpos_gap_before,
                            para_idx: pi,
                            vis_start: li,
                            vis_end: li + 1,
                            nested_row: None,
                            nested_table_fragment: None,
                            mixed_nested_fragment: false,
                            mixed_nested_trailing: false,
                            mixed_nested_content_height: 0.0,
                            mixed_nested_recursive: false,
                            mixed_nested_starts_after_table: false,
                            mixed_nested_source_para_idx: None,
                            recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                            top_and_bottom_flow: false,
                            empty_spacer: false,
                            non_inline_control_range: None,
                        });
                        unit_cum += lh;
                    }

                    let has_internal_line_reset = p
                        .line_segs
                        .windows(2)
                        .any(|pair| pair[1].vertical_pos < pair[0].vertical_pos);
                    let target_h = if has_internal_line_reset {
                        (nested_h + 4.0 - line_core_height).max(0.0)
                    } else {
                        nested_h + 4.0
                    };
                    if target_h > 0.5 {
                        let mut fragment_heights: Vec<NestedFlowFragment> = p
                            .controls
                            .iter()
                            .filter_map(|ctrl| {
                                if let Control::Table(t) = ctrl {
                                    Some(self.nested_table_mixed_fragment_heights(t, styles))
                                } else {
                                    None
                                }
                            })
                            .flatten()
                            .collect();
                        if fragment_heights.is_empty() {
                            const NESTED_FRAGMENT_UNIT_PX: f64 = 16.0;
                            let mut remaining = target_h;
                            while remaining > 0.5 {
                                let h = remaining.min(NESTED_FRAGMENT_UNIT_PX);
                                fragment_heights.push(NestedFlowFragment {
                                    height: h,
                                    hard_break_before: false,
                                    stored_frame_break_before: false,
                                    trailing: false,
                                    content_height: h,
                                    recursive: false,
                                    starts_after_table: false,
                                    source_para_idx: None,
                                    recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                                });
                                remaining -= h;
                            }
                        } else {
                            let current_h: f64 = fragment_heights
                                .iter()
                                .map(|fragment| fragment.height)
                                .sum();
                            // [Task #1809] top pad 차감(c7dbe8a2, 종전 HWPX 한정)을 소스
                            // 무관화 — 한글 편집기 대조에서 pad 적용 컷 위치가 정답
                            // (admrul_0556 p1 조각 하단: 한글 808.8 = pad 적용 808.7,
                            // 미적용 810.1). HWP5 재파스에도 동일 적용해야 정합.
                            let hwpx_rowbreak_top_pad =
                                if is_block_rowbreak && !has_internal_line_reset {
                                    p.controls
                                        .iter()
                                        .filter_map(|ctrl| {
                                            if let Control::Table(t) = ctrl {
                                                let top_pad = t
                                                    .cells
                                                    .iter()
                                                    .filter(|cell| cell.row == 0)
                                                    .map(|cell| {
                                                        let (_, _, pad_top, _) =
                                                            self.resolve_cell_padding(cell, t);
                                                        pad_top
                                                    })
                                                    .fold(0.0f64, f64::max);
                                                Some(top_pad)
                                            } else {
                                                None
                                            }
                                        })
                                        .sum::<f64>()
                                } else {
                                    0.0
                                };
                            let top_up = (target_h - current_h).max(0.0);
                            let target_h = target_h - hwpx_rowbreak_top_pad.min(top_up);
                            if target_h > current_h + 0.5 {
                                if let Some(first) = fragment_heights.first_mut() {
                                    first.height += target_h - current_h;
                                    first.content_height = first.content_height.max(first.height);
                                }
                            }
                        }
                        for fragment in fragment_heights {
                            units.push(CellUnit {
                                height: fragment.height,
                                hard_break_before: fragment.hard_break_before,
                                stored_frame_break_before: fragment.stored_frame_break_before,
                                vpos_gap_before: false,
                                para_idx: pi,
                                vis_start: line_count,
                                vis_end: line_count,
                                nested_row: None,
                                nested_table_fragment: None,
                                mixed_nested_fragment: true,
                                mixed_nested_trailing: fragment.trailing,
                                mixed_nested_content_height: fragment.content_height,
                                mixed_nested_recursive: fragment.recursive,
                                mixed_nested_starts_after_table: fragment.starts_after_table,
                                mixed_nested_source_para_idx: fragment.source_para_idx,
                                recursive_block_prelude_role: fragment.recursive_block_prelude_role,
                                top_and_bottom_flow: false,
                                empty_spacer: false,
                                non_inline_control_range: None,
                            });
                            unit_cum += fragment.height;
                        }
                    }
                    let non_inline_range = append_non_inline_units(
                        &mut units,
                        pi,
                        para_non_inline_extra_h,
                        para_top_and_bottom_h,
                        para_other_non_inline_h,
                    );
                    tag_other_non_inline_control_units(
                        &mut units,
                        non_inline_range,
                        &para_other_non_inline_controls,
                    );
                    continue;
                }
            }
            if line_count == 0 || has_table_in_para {
                // 중첩 표/빈 문단 — atomic 유닛 1개.
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
                let para_h = if collapse_empty_rowbreak_spacer {
                    0.0
                } else if line_count == 0 {
                    let h = if nested_h > 0.0 {
                        nested_h
                    } else if crate::renderer::para_has_no_stored_line_segs(p)
                        && p.controls.is_empty()
                    {
                        // [#2169] NO_LS 순수 빈 문단 = 완전한 em 줄박스 (한글 공식:
                        // 80168 r4 c2 = 10줄×em + 9gap 정확). 비마지막 문단은
                        // fs×ls%(gap 포함 동치), 셀 마지막 문단은 em.
                        let fs = p
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
                    let line_based_h: f64 = comp
                        .lines
                        .iter()
                        .enumerate()
                        .map(|(li, line)| {
                            let h = corrected_h(line, li);
                            let ls = hwpunit_to_px(line.line_spacing, self.dpi);
                            let is_cell_last_line = is_last_para && li + 1 == line_count;
                            // [Task #1022/#1086] trailing ls 규칙 — HeightMeasurer 와
                            // 정합. CellBreak/TAC 표는 기존 trailing geometry 를 보존하고,
                            // block RowBreak 표는 렌더 가시 높이처럼 셀 마지막 줄
                            // trailing 을 제외해 행 fit 을 맞춘다.
                            let is_block_rowbreak = matches!(
                                table.page_break,
                                crate::model::table::TablePageBreak::RowBreak
                            ) && !table.common.treat_as_char;
                            let include_trailing_ls = !is_cell_last_line || para_count > 1;
                            let include_trailing_ls =
                                include_trailing_ls && (!is_cell_last_line || !is_block_rowbreak);
                            let mut lh = if include_trailing_ls { h + ls } else { h };
                            if li == 0 {
                                lh += spacing_before;
                            }
                            if li == line_count - 1 {
                                lh += spacing_after;
                            }
                            lh
                        })
                        .sum();
                    let has_visible_text_with_nested = use_vpos_unit_positions
                        && comp
                            .lines
                            .iter()
                            .any(|line| line.runs.iter().any(|run| !run.text.trim().is_empty()));
                    if has_visible_text_with_nested && nested_h > 0.0 {
                        line_based_h + nested_h + 4.0
                    } else {
                        nested_h.max(line_based_h)
                    }
                };
                let hard_break_before = reset_before;
                let mut para_h = para_h;
                let mut vpos_gap_before = vpos_gap_before_para;
                if use_vpos_unit_positions {
                    if let Some(seg) = p.line_segs.first() {
                        if !line_seg_is_synthetic(seg) {
                            let target_top = normalized_vpos_px(seg.vertical_pos);
                            if target_top > unit_cum {
                                let delta = target_top - unit_cum;
                                let suppress_hwpx_mixed_nested_gap =
                                    self.profile.get().hwpx_stored_layout()
                                        && prev_para_has_mixed_nested_table
                                        && delta <= 24.0;
                                if !suppress_hwpx_mixed_nested_gap {
                                    para_h += delta;
                                    vpos_gap_before = true;
                                }
                            }
                        }
                    }
                }
                units.push(CellUnit {
                    height: para_h,
                    // [Task #1488] 비가시 빈 문단(중첩표 없음)의 오버레이 리셋은 페이지를
                    // 강제 분할하지 않는다 — 여분 빈 연속 페이지 방지. 중첩표가 있으면
                    // 가시 콘텐츠를 가지므로 리셋 보존.
                    hard_break_before: hard_break_before
                        && (has_table_in_para || para_has_visible_text),
                    stored_frame_break_before: false,
                    vpos_gap_before: vpos_gap_before && !collapse_empty_rowbreak_spacer,
                    para_idx: pi,
                    vis_start: 0,
                    vis_end: if collapse_empty_rowbreak_spacer {
                        0
                    } else {
                        line_count.max(1)
                    },
                    nested_row: None,
                    nested_table_fragment: None,
                    mixed_nested_fragment: false,
                    mixed_nested_trailing: false,
                    mixed_nested_content_height: 0.0,
                    mixed_nested_recursive: false,
                    mixed_nested_starts_after_table: false,
                    mixed_nested_source_para_idx: None,
                    recursive_block_prelude_role: RecursiveBlockPreludeRole::None,
                    top_and_bottom_flow: para_top_and_bottom_flow_unit,
                    empty_spacer: is_empty_spacer_para,
                    non_inline_control_range: None,
                });
                unit_cum += para_h;
            } else {
                // 일반 텍스트 문단 — 합성 줄마다 유닛 1개.
                for (li, line) in comp.lines.iter().enumerate() {
                    let h = corrected_h(line, li);
                    let ls = hwpunit_to_px(line.line_spacing, self.dpi);
                    let is_cell_last_line = is_last_para && li + 1 == line_count;
                    let include_trailing_ls = !is_cell_last_line || para_count > 1;
                    let include_trailing_ls =
                        include_trailing_ls && (!is_cell_last_line || !is_block_rowbreak);
                    let mut lh = if include_trailing_ls { h + ls } else { h };
                    if collapse_empty_rowbreak_spacer {
                        lh = 0.0;
                    } else {
                        if li == 0 {
                            lh += spacing_before;
                        }
                        if li == line_count - 1 {
                            lh += spacing_after;
                        }
                    }
                    let hard_break_before = line_reset_before(li);
                    let mut vpos_gap_before = if li == 0 {
                        vpos_gap_before_para
                    } else if use_vpos_unit_positions && cell_first_vpos == 0 {
                        match (p.line_segs.get(li - 1), p.line_segs.get(li)) {
                            (Some(prev), Some(cur))
                                if !line_seg_is_synthetic(prev) && !line_seg_is_synthetic(cur) =>
                            {
                                cur.vertical_pos
                                    > prev.vertical_pos
                                        + prev.line_height
                                        + prev.line_spacing
                                        + vpos_gap_threshold_hu
                            }
                            _ => false,
                        }
                    } else {
                        false
                    };
                    if use_vpos_unit_positions {
                        if let Some(seg) = p.line_segs.get(li) {
                            if !line_seg_is_synthetic(seg) {
                                let target_top = normalized_vpos_px(seg.vertical_pos);
                                if target_top > unit_cum {
                                    let delta = target_top - unit_cum;
                                    let suppress_hwpx_mixed_nested_gap =
                                        self.profile.get().hwpx_stored_layout()
                                            && li == 0
                                            && prev_para_has_mixed_nested_table
                                            && delta <= 24.0;
                                    if !suppress_hwpx_mixed_nested_gap {
                                        lh += delta;
                                        vpos_gap_before = true;
                                    }
                                }
                            }
                        }
                    }
                    units.push(CellUnit {
                        height: lh,
                        // [Task #1488] 비가시(빈 텍스트) 오버레이 스페이서 문단이 만든 vpos
                        // 리셋은 페이지를 강제 분할하지 않는다. 셀 안에서 본문 텍스트 위에
                        // 겹쳐 놓인 빈 문단(동일/역방향 vpos)들이 리셋마다 페이지를 1장씩
                        // 양산하던 여분 빈 연속 페이지 회귀를 제거한다. 가시 텍스트 문단 사이
                        // 리셋(Task #993 의도)은 그대로 하드 브레이크로 보존한다.
                        hard_break_before: hard_break_before && para_has_visible_text,
                        stored_frame_break_before: para_has_visible_text
                            && stored_frame_break_before(li),
                        vpos_gap_before: vpos_gap_before && !collapse_empty_rowbreak_spacer,
                        para_idx: pi,
                        vis_start: if collapse_empty_rowbreak_spacer {
                            0
                        } else {
                            li
                        },
                        vis_end: if collapse_empty_rowbreak_spacer {
                            0
                        } else {
                            li + 1
                        },
                        nested_row: None,
                        nested_table_fragment: None,
                        mixed_nested_fragment: false,
                        mixed_nested_trailing: false,
                        mixed_nested_content_height: 0.0,
                        mixed_nested_recursive: false,
                        mixed_nested_starts_after_table: false,
                        mixed_nested_source_para_idx: None,
                        recursive_block_prelude_role: if is_exact_recursive_prelude {
                            RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable
                        } else {
                            RecursiveBlockPreludeRole::None
                        },
                        top_and_bottom_flow: para_top_and_bottom_flow_unit,
                        empty_spacer: is_empty_spacer_para,
                        non_inline_control_range: None,
                    });
                    unit_cum += lh;
                }
            }
            let non_inline_range = append_non_inline_units(
                &mut units,
                pi,
                para_non_inline_extra_h,
                para_top_and_bottom_h,
                para_other_non_inline_h,
            );
            tag_other_non_inline_control_units(
                &mut units,
                non_inline_range,
                &para_other_non_inline_controls,
            );
        }

        let units =
            Self::delay_empty_anchor_topandbottom_flow_units_before_hard_break(units, cell, table);

        let _ = (pad_top, pad_bottom); // [Task #1022] cell.height 필러 제거 — row_cut_content_height 가 셀별 max(cell.height, content+pad) 로 행 단계에서 정합.
        units
    }

}
