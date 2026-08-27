//! row_cut_tests — table_layout.rs 에서 무변동 이동
use super::{
    stored_layout_relocated_empty_rowbreak_picture_resets_offset,
    trailing_reservation_after_final_source_owner, CellUnit, LayoutEngine,
    MixedNestedOwnerMarker, RecursiveBlockPreludeRole,
};
use crate::model::control::Control;
use crate::model::image::Picture;
use crate::model::paragraph::{LineSeg, Paragraph};
use crate::model::shape::{CommonObjAttr, TextWrap, VertRelTo};
use crate::model::table::{Cell, Table};
use crate::renderer::style_resolver::ResolvedStyleSet;

/// line_height=1200 HU (=16 px @96dpi), line_spacing=0 인 N줄 텍스트 문단.
/// vpos 는 vpos_start 부터 1200 HU 간격. `.text` 가 비어 있어 [Task #1488]
/// 가시성 게이트 기준으로 **비가시(빈)** 문단으로 취급된다.
fn text_para(n_lines: usize, vpos_start: i32) -> Paragraph {
    Paragraph {
        text: "x".repeat(n_lines.max(1)),
        char_count: n_lines.max(1) as u32,
        line_segs: (0..n_lines)
            .map(|i| LineSeg {
                vertical_pos: vpos_start + i as i32 * 1200,
                line_height: 1200,
                line_spacing: 0,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// `text_para` 와 동일한 line_seg 구조에 가시 텍스트를 더한 문단. [Task #1488]
/// 가시성 게이트가 가시 문단으로 인식하므로 vpos 리셋이 하드 브레이크로 보존된다.
/// line_seg 가 있으면 compose 가 line_seg 수만큼 줄을 만들므로 유닛 수는 보존된다.
fn visible_text_para(n_lines: usize, vpos_start: i32) -> Paragraph {
    Paragraph {
        text: "가나다".to_string(),
        ..text_para(n_lines, vpos_start)
    }
}

/// [Task #1488] 비가시(빈 텍스트) 오버레이 스페이서 문단 — line_seg 만 갖고 가시
/// 텍스트는 없다. `text_para` 가 (#stabilize-rowbreak 이후) 가시 "x" 를 갖게 되어,
/// 빈-오버레이 게이트 검증용으로 빈 텍스트 문단을 별도 헬퍼로 분리한다.
fn empty_overlay_para(n_lines: usize, vpos_start: i32) -> Paragraph {
    Paragraph {
        text: String::new(),
        char_count: 0,
        ..text_para(n_lines, vpos_start)
    }
}

fn cell(row: u16, col: u16, paragraphs: Vec<Paragraph>) -> Cell {
    Cell {
        row,
        col,
        row_span: 1,
        col_span: 1,
        width: 10000,
        paragraphs,
        ..Default::default()
    }
}

fn table(cells: Vec<Cell>) -> Table {
    let row_count = cells.iter().map(|c| c.row + 1).max().unwrap_or(1);
    let col_count = cells.iter().map(|c| c.col + 1).max().unwrap_or(1);
    Table {
        row_count,
        col_count,
        cells,
        ..Default::default()
    }
}

fn rowbreak_table(cells: Vec<Cell>) -> Table {
    Table {
        page_break: crate::model::table::TablePageBreak::RowBreak,
        ..table(cells)
    }
}

fn mixed_owner_marker(
    para_idx: usize,
    trailing: bool,
    content_height: f64,
    height: f64,
) -> MixedNestedOwnerMarker {
    MixedNestedOwnerMarker {
        para_idx,
        fragment: true,
        trailing,
        content_height,
        height,
    }
}

fn recursive_block_unit(height: f64, role: RecursiveBlockPreludeRole) -> CellUnit {
    CellUnit {
        height,
        hard_break_before: false,
        stored_frame_break_before: false,
        vpos_gap_before: false,
        para_idx: 0,
        vis_start: 0,
        vis_end: 1,
        nested_row: None,
        nested_table_fragment: None,
        mixed_nested_fragment: true,
        mixed_nested_trailing: false,
        mixed_nested_content_height: height,
        mixed_nested_recursive: true,
        mixed_nested_starts_after_table: false,
        mixed_nested_source_para_idx: None,
        recursive_block_prelude_role: role,
        top_and_bottom_flow: false,
        empty_spacer: false,
        non_inline_control_range: None,
    }
}

#[test]
fn recursive_block_prelude_rewinds_already_fit_prefix_before_overflow() {
    let table = rowbreak_table(vec![]);
    let mut prior = recursive_block_unit(20.0, RecursiveBlockPreludeRole::None);
    prior.mixed_nested_fragment = false;
    prior.mixed_nested_recursive = false;
    let separator =
        recursive_block_unit(8.0, RecursiveBlockPreludeRole::ExplicitPageBreakSeparator);
    let heading = recursive_block_unit(
        12.0,
        RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable,
    );
    let prefix = recursive_block_unit(55.0, RecursiveBlockPreludeRole::None);
    let pending = recursive_block_unit(10.0, RecursiveBlockPreludeRole::None);
    let units = vec![prior, separator, heading, prefix, pending];

    // prior + separator + heading + 첫 recursive 조각은 95px로 fit했지만,
    // 다음 10px 조각은 100px 예산을 넘는다. separator부터 fit한 prefix까지
    // 함께 다음 fragment로 되감아야 제목만 이전 쪽에 고립되지 않는다.
    let mut j = 4;
    let mut h = 95.0;
    assert!(
        LayoutEngine::rewind_rowbreak_orphan_heading_before_recursive_block(
            &table, &units, 0, 100.0, &mut j, &mut h,
        )
    );
    assert_eq!(j, 1);
    assert!((h - 20.0).abs() < 0.001);

    // 명시적 쪽 나누기가 없는 일반 prelude는 이미 recursive prefix가 들어간
    // 뒤까지 되감지 않는다. 저장 프레임 경계에 맞춘 정상 continuation을
    // 앞당기면 뒤쪽 모든 physical page owner가 한 쪽씩 밀린다.
    let units = vec![
        recursive_block_unit(20.0, RecursiveBlockPreludeRole::None),
        recursive_block_unit(8.0, RecursiveBlockPreludeRole::EmptySeparator),
        recursive_block_unit(
            12.0,
            RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable,
        ),
        recursive_block_unit(55.0, RecursiveBlockPreludeRole::None),
        recursive_block_unit(10.0, RecursiveBlockPreludeRole::None),
    ];
    let mut j = 4;
    let mut h = 95.0;
    assert!(
        !LayoutEngine::rewind_rowbreak_orphan_heading_before_recursive_block(
            &table, &units, 0, 100.0, &mut j, &mut h,
        )
    );
    assert_eq!(j, 4);
    assert!((h - 95.0).abs() < 0.001);

    // 새 viewport가 separator부터 시작했다면 prefix가 일부 fit했더라도
    // separator까지 되감아 무진행 cut을 만들면 안 된다.
    let units = vec![
        recursive_block_unit(8.0, RecursiveBlockPreludeRole::ExplicitPageBreakSeparator),
        recursive_block_unit(
            12.0,
            RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable,
        ),
        recursive_block_unit(55.0, RecursiveBlockPreludeRole::None),
        recursive_block_unit(40.0, RecursiveBlockPreludeRole::None),
    ];
    let mut j = 3;
    let mut h = 75.0;
    assert!(
        !LayoutEngine::rewind_rowbreak_orphan_heading_before_recursive_block(
            &table, &units, 0, 100.0, &mut j, &mut h,
        )
    );
    assert_eq!(j, 3);
    assert!((h - 75.0).abs() < 0.001);

    // prefix가 없는 기존 direct-next 형상도 같은 계약을 유지한다.
    let units = vec![
        recursive_block_unit(20.0, RecursiveBlockPreludeRole::None),
        recursive_block_unit(8.0, RecursiveBlockPreludeRole::EmptySeparator),
        recursive_block_unit(
            12.0,
            RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable,
        ),
        recursive_block_unit(70.0, RecursiveBlockPreludeRole::None),
    ];
    let mut j = 3;
    let mut h = 40.0;
    assert!(
        LayoutEngine::rewind_rowbreak_orphan_heading_before_recursive_block(
            &table, &units, 0, 100.0, &mut j, &mut h,
        )
    );
    assert_eq!(j, 1);
    assert!((h - 20.0).abs() < 0.001);

    // 직전 recursive block 뒤에서 다음 prelude의 separator만 fit하고
    // pending 제목이 예산을 넘는 경우에도 separator를 다음 조각으로
    // 넘겨야 제목+재귀 block이 새 viewport에서 함께 시작한다.
    let units = vec![
        recursive_block_unit(20.0, RecursiveBlockPreludeRole::None),
        recursive_block_unit(8.0, RecursiveBlockPreludeRole::EmptySeparator),
        recursive_block_unit(
            12.0,
            RecursiveBlockPreludeRole::OneLineHeadingBeforeSingleCellTable,
        ),
        recursive_block_unit(70.0, RecursiveBlockPreludeRole::None),
    ];
    let mut j = 2;
    let mut h = 28.0;
    assert!(
        LayoutEngine::rewind_rowbreak_orphan_heading_before_recursive_block(
            &table, &units, 0, 30.0, &mut j, &mut h,
        )
    );
    assert_eq!(j, 1);
    assert!((h - 20.0).abs() < 0.001);
}

#[test]
fn trailing_reservation_does_not_extend_before_later_source_owner() {
    let empty_reservation = mixed_owner_marker(7, true, 0.0, 3.75);
    let later_source_owner = mixed_owner_marker(7, false, 18.0, 18.0);
    let later_contentful_trailing_owner = mixed_owner_marker(7, true, 12.0, 12.0);

    assert_eq!(
        trailing_reservation_after_final_source_owner(
            7,
            Some(empty_reservation),
            [later_source_owner],
        ),
        0.0,
        "an empty reservation before a later source owner must not enlarge the scalar viewport"
    );
    assert_eq!(
        trailing_reservation_after_final_source_owner(
            7,
            Some(empty_reservation),
            [later_contentful_trailing_owner],
        ),
        0.0,
        "a contentful trailing unit is still a later source owner"
    );
    assert_eq!(
        trailing_reservation_after_final_source_owner(
            7,
            Some(empty_reservation),
            std::iter::empty(),
        ),
        3.75,
        "the final contentless reservation must preserve the last painted line"
    );
}

fn non_inline_picture_para(vpos_start: i32) -> Paragraph {
    let common = CommonObjAttr {
        width: 10_000,
        height: 8_000,
        treat_as_char: false,
        text_wrap: TextWrap::TopAndBottom,
        vert_rel_to: VertRelTo::Para,
        vertical_offset: 1_000,
        flow_with_text: true,
        ..Default::default()
    };
    Paragraph {
        text: "그림".to_string(),
        char_count: 2,
        line_segs: vec![LineSeg {
            vertical_pos: vpos_start,
            line_height: 1200,
            line_spacing: 0,
            ..Default::default()
        }],
        controls: vec![Control::Picture(Box::new(Picture {
            common,
            ..Default::default()
        }))],
        ..Default::default()
    }
}

fn empty_anchor_non_inline_picture_para(vpos_start: i32) -> Paragraph {
    let mut para = non_inline_picture_para(vpos_start);
    para.text.clear();
    para.char_count = 0;
    para
}

#[test]
fn stored_layout_relocated_empty_rowbreak_picture_uses_outer_host_vpos() {
    let mut para = empty_anchor_non_inline_picture_para(0);
    let Control::Picture(picture) = &mut para.controls[0] else {
        panic!("그림 컨트롤 아님");
    };
    picture.common.vertical_offset = (-52_790i32) as u32;

    let cell = cell(0, 0, vec![para.clone()]);
    let mut host = rowbreak_table(vec![cell.clone()]);
    host.common = CommonObjAttr {
        treat_as_char: false,
        text_wrap: TextWrap::TopAndBottom,
        vert_rel_to: VertRelTo::Para,
        vertical_offset: 560,
        ..Default::default()
    };
    let Control::Picture(picture) = &para.controls[0] else {
        panic!("그림 컨트롤 아님");
    };

    assert!(
        stored_layout_relocated_empty_rowbreak_picture_resets_offset(
            true,
            true,
            Some(52_230),
            &host,
            &cell,
            &para,
            picture,
        )
    );
    assert!(
        !stored_layout_relocated_empty_rowbreak_picture_resets_offset(
            true,
            true,
            Some(52_220),
            &host,
            &cell,
            &para,
            picture,
        )
    );
    assert!(
        !stored_layout_relocated_empty_rowbreak_picture_resets_offset(
            false,
            true,
            Some(52_230),
            &host,
            &cell,
            &para,
            picture,
        )
    );
}

#[test]
fn native_hwp5_same_page_stale_empty_rowbreak_picture_resets_offset() {
    let mut para = empty_anchor_non_inline_picture_para(0);
    let Control::Picture(picture) = &mut para.controls[0] else {
        panic!("그림 컨트롤 아님");
    };
    picture.common.vertical_offset = (-50_000i32) as u32;

    let cell = cell(0, 0, vec![para.clone()]);
    let mut host = rowbreak_table(vec![cell.clone()]);
    host.common = CommonObjAttr {
        treat_as_char: false,
        text_wrap: TextWrap::TopAndBottom,
        vert_rel_to: VertRelTo::Para,
        vertical_offset: 0,
        ..Default::default()
    };
    let Control::Picture(picture) = &para.controls[0] else {
        panic!("그림 컨트롤 아님");
    };

    assert!(
        stored_layout_relocated_empty_rowbreak_picture_resets_offset(
            true,
            true,
            Some(12_000),
            &host,
            &cell,
            &para,
            picture,
        ),
        "native HWP5의 page-scale stale picture offset은 current cell top으로 reset해야 한다"
    );
    assert!(
        !stored_layout_relocated_empty_rowbreak_picture_resets_offset(
            true,
            false,
            Some(12_000),
            &host,
            &cell,
            &para,
            picture,
        ),
        "HWPX stored-layout에 native HWP5 stale-offset 규칙이 번지면 안 된다"
    );

    let Control::Picture(picture) = &mut para.controls[0] else {
        panic!("그림 컨트롤 아님");
    };
    picture.common.vertical_offset = (-39_999i32) as u32;
    let Control::Picture(picture) = &para.controls[0] else {
        panic!("그림 컨트롤 아님");
    };
    assert!(
        !stored_layout_relocated_empty_rowbreak_picture_resets_offset(
            true,
            true,
            Some(12_000),
            &host,
            &cell,
            &para,
            picture,
        ),
        "page-scale 기준보다 작은 일반 음수 offset은 보정하면 안 된다"
    );
}

#[test]
fn test_topandbottom_flow_height_includes_margins() {
    // TopAndBottom + Para + flow_with_text 그림은 실제 렌더 y가
    // vertical_offset + margin.top부터 시작하므로, 예약 높이도
    // vertical_offset + margin.top + height + margin.bottom이어야 한다.
    let eng = LayoutEngine::new(96.0);
    let mut para = non_inline_picture_para(0);
    let Control::Picture(pic) = &mut para.controls[0] else {
        panic!("그림 컨트롤 아님");
    };
    pic.common.vertical_offset = 720;
    pic.common.height = 7200;
    pic.common.margin.top = 720;
    pic.common.margin.bottom = 1440;

    let h = eng.paragraph_cell_non_inline_controls_flow_height(&para.controls);
    assert!(
        (h - 134.4).abs() < 0.01,
        "TopAndBottom flow height에 margin이 포함되어야 함: {h}"
    );
}

#[test]
fn test_advance_row_cut_basic_split() {
    // 1행 1셀, 6줄(각 16px). avail=50 → 3줄(48px) 소비, 4번째(64px)는 초과.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![cell(0, 0, vec![text_para(6, 0)])]);
    let r = eng.advance_row_cut(&t, 0, &[], 50.0, &styles);
    assert_eq!(r.end_cut, vec![3]);
    assert!(!r.fully_consumed);
    assert!(!r.hit_hard_break);
    assert!((r.consumed_height - 48.0).abs() < 0.5);
}

#[test]
fn test_advance_row_cut_fully_consumed() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![cell(0, 0, vec![text_para(6, 0)])]);
    let r = eng.advance_row_cut(&t, 0, &[], 500.0, &styles);
    assert_eq!(r.end_cut, vec![6]);
    assert!(r.fully_consumed);
}

#[test]
fn test_advance_row_cut_force_progress() {
    // avail 이 한 줄(16px)보다 작아도 시작 유닛 1개는 강제 소비 — 무한 루프 방지.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![cell(0, 0, vec![text_para(6, 0)])]);
    let r = eng.advance_row_cut(&t, 0, &[], 5.0, &styles);
    assert_eq!(r.end_cut, vec![1]);
    assert!(!r.fully_consumed);
}

#[test]
fn test_advance_row_cut_rowbreak_grace_denied_in_continuous_visible_run() {
    // [Task #1718 v2] over-fill grace 는 오버플로 꼬리줄과 첫 spacer 사이가
    // "끊김 없는 가시 텍스트 줄의 연속(run)" 이면 거부한다 — 거대 RowBreak 셀 본문
    // 한복판(spacer 는 저 멀리)에서 grace 가 걸려 페이지당 +1~5줄 과충전 →
    // under-pagination(승강기 별표27: 40 vs 한글 48) 을 막는다.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![
            visible_text_para(6, 0),     // 가시 6유닛 (vpos 0,1200,..6000)
            empty_overlay_para(1, 7200), // spacer 는 가시 run 뒤에 위치
        ],
    )]);
    // avail=52px: 3줄(48px) 소비, 4번째(64px)는 +12px 초과(<120 tolerance).
    // 첫 spacer 전까지 units[4..6]=[가시,가시] 연속 run → grace 거부 → end_cut=[3].
    let r = eng.advance_row_cut(&t, 0, &[], 52.0, &styles);
    assert_eq!(
        r.end_cut,
        vec![3],
        "연속 가시 run 한복판에서는 over-fill grace 미적용"
    );
    assert!(
        r.consumed_height <= 52.5,
        "본문 초과 채움 금지: {}",
        r.consumed_height
    );
}

#[test]
fn test_advance_row_cut_rowbreak_grace_kept_for_true_tail_before_spacers() {
    // [Task #1718] 오버플로 가시라인 바로 뒤가 spacer 면(진짜 꼬리줄) grace 유지 —
    // caption/꼬리줄 보존(byeolpyo1/4 over-pagination 방지 케이스 무회귀).
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![
            visible_text_para(4, 0),
            empty_overlay_para(1, 4800), // 바로 뒤 spacer → 진짜 꼬리줄
            empty_overlay_para(1, 6000),
        ],
    )]);
    let r = eng.advance_row_cut(&t, 0, &[], 52.0, &styles);
    assert!(
        r.end_cut[0] >= 4,
        "진짜 tail-before-spacer 는 grace 로 수용: {:?}",
        r.end_cut
    );
}

#[test]
fn test_advance_row_cut_rowbreak_grace_denied_before_spacer_then_visible_text() {
    // 빈 줄 spacer 뒤에 다시 일반 가시 본문이 이어지면 구조적 꼬리줄이 아니라
    // 문단 사이 여백이므로 페이지 예산을 넘겨 끌어올리지 않는다.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![
            visible_text_para(4, 0),
            empty_overlay_para(1, 4800),
            visible_text_para(2, 6000),
        ],
    )]);
    let r = eng.advance_row_cut(&t, 0, &[], 52.0, &styles);
    assert_eq!(
        r.end_cut,
        vec![3],
        "spacer 뒤 본문이 계속되면 tail-before-spacer grace 미적용"
    );
}

#[test]
fn test_cell_cut_non_inline_controls_do_not_repeat_after_para_cut() {
    // 셀 안 non-inline 그림은 해당 문단의 유닛이 현재 컷에 들어올 때만 렌더
    // 후보다. 문단을 지난 뒤의 continuation 에서 되살리면 이전 쪽 그림이
    // 모든 페이지에 반복된다.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![non_inline_picture_para(0), visible_text_para(1, 1200)],
    )]);
    let cell_ref = &t.cells[0];
    let units = eng.cell_units(cell_ref, &t, &styles);
    let picture_unit = units
        .iter()
        .position(|unit| {
            unit.para_idx == 0
                && unit.vis_start == unit.vis_end
                && !unit.empty_spacer
                && unit.nested_row.is_none()
                && !unit.mixed_nested_fragment
        })
        .expect("그림 전용 유닛 존재");
    let after_picture_units = units
        .iter()
        .position(|unit| unit.para_idx == 1)
        .expect("두 번째 문단 유닛 존재");

    assert!(
        !eng.cell_cut_contains_non_inline_control_units(cell_ref, &t, &styles, 0, 1, 0),
        "그림 문단의 일반 텍스트 줄만 포함된 컷에서는 렌더하지 않음"
    );
    assert!(
        eng.cell_cut_contains_non_inline_control_units(
            cell_ref,
            &t,
            &styles,
            picture_unit,
            picture_unit + 1,
            0
        ),
        "그림 전용 유닛이 포함된 컷에서만 렌더 후보"
    );
    assert!(
        !eng.cell_cut_contains_non_inline_control_units(
            cell_ref,
            &t,
            &styles,
            after_picture_units,
            after_picture_units + 1,
            0
        ),
        "그림 문단을 지난 컷에서는 후속 페이지에 반복 렌더하지 않음"
    );
}

#[test]
fn test_advance_row_cut_non_inline_flow_unit_is_atomic() {
    // TopAndBottom non-inline 그림의 흐름 높이를 줄 높이 조각으로 쪼개면
    // 한 그림이 여러 continuation 컷에 반복 렌더된다. 객체 흐름 유닛은
    // 현재 쪽에 온전히 들어가지 않으면 다음 쪽에서 통째로 시작해야 한다.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![non_inline_picture_para(0), visible_text_para(1, 1200)],
    )]);

    let r = eng.advance_row_cut(&t, 0, &[], 40.0, &styles);
    assert_eq!(r.end_cut, vec![1], "그림 앞 텍스트 줄까지만 들어감");
    assert!(!r.fully_consumed);

    let r2 = eng.advance_row_cut(&t, 0, &r.end_cut, 1_000.0, &styles);
    assert!(
        r2.end_cut[0] > r.end_cut[0],
        "다음 컷에서 그림 흐름 유닛이 전진함"
    );
}

#[test]
fn test_advance_row_cut_non_inline_flow_unit_not_orphaned_before_spacer() {
    // RowBreak 거대 셀에서 TopAndBottom 그림 flow 유닛만 쪽 하단에 들어가고,
    // 바로 뒤 spacer 가 다음 쪽으로 밀리면 기준 렌더러보다 그림이 한 쪽 앞선다.
    // 그림 유닛+뒤 spacer 묶음이 함께 들어가지 못하면 그림 유닛부터 다음 조각으로 넘긴다.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![
            visible_text_para(1, 0),
            non_inline_picture_para(1200),
            empty_overlay_para(1, 2400),
            visible_text_para(1, 3600),
        ],
    )]);
    let units = eng.cell_units(&t.cells[0], &t, &styles);
    let picture_unit = units
        .iter()
        .position(|unit| {
            unit.vis_start == unit.vis_end
                && !unit.empty_spacer
                && unit.nested_row.is_none()
                && !unit.mixed_nested_fragment
        })
        .expect("그림 flow 유닛 존재");
    let spacer_unit = picture_unit + 1;
    assert!(units[spacer_unit].empty_spacer, "그림 뒤 spacer 존재");

    let before_picture: f64 = units[..picture_unit].iter().map(|unit| unit.height).sum();
    let picture_height = units[picture_unit].height;
    let spacer_height = units[spacer_unit].height;
    let avail = before_picture + picture_height + spacer_height * 0.5;

    let r = eng.advance_row_cut(&t, 0, &[], avail, &styles);
    assert_eq!(
        r.end_cut,
        vec![picture_unit],
        "그림만 들어가고 뒤 spacer 가 빠지는 컷은 만들지 않음"
    );

    let b = eng.advance_row_block_cut(&t, 0, 1, &[], avail, &styles);
    assert_eq!(
        b.end_cut, r.end_cut,
        "행블록 컷도 같은 orphan 방지 조건을 적용"
    );

    let r2 = eng.advance_row_cut(&t, 0, &r.end_cut, 1_000.0, &styles);
    assert!(
        r2.end_cut[0] > spacer_unit,
        "다음 조각에서는 그림과 spacer 를 함께 전진"
    );
}

#[test]
fn test_empty_anchor_topandbottom_flow_delayed_before_hard_break() {
    // 빈 anchor 문단의 TopAndBottom 그림은 저장 vpos hard break 직전까지 지연될 수 있다.
    // 이렇게 해야 그림은 다음 쪽 상단으로 넘기면서도 anchor 뒤 일반 텍스트는 이전 쪽에
    // 계속 채울 수 있다.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![
            visible_text_para(1, 0),
            empty_anchor_non_inline_picture_para(1200),
            empty_overlay_para(1, 2400),
            visible_text_para(2, 3600),
            visible_text_para(1, 1000),
        ],
    )]);
    let units = eng.cell_units(&t.cells[0], &t, &styles);
    let picture_unit = units
        .iter()
        .position(|unit| {
            unit.vis_start == unit.vis_end
                && !unit.empty_spacer
                && unit.nested_row.is_none()
                && !unit.mixed_nested_fragment
        })
        .expect("지연된 그림 flow 유닛 존재");
    let hard_break_unit = units
        .iter()
        .position(|unit| unit.hard_break_before && unit.vis_start < unit.vis_end)
        .expect("저장 vpos hard break 유닛 존재");

    assert_eq!(
        picture_unit + 1,
        hard_break_unit,
        "빈 anchor 그림 flow 유닛은 다음 가시 hard break 직전에 배치"
    );
    assert!(
        units[..picture_unit]
            .iter()
            .any(|unit| unit.para_idx == 3 && unit.vis_start < unit.vis_end),
        "그림 anchor 뒤 일반 텍스트는 그림보다 앞서 흐를 수 있어야 함"
    );
}

#[test]
fn test_advance_row_cut_vpos_reset_hard_break() {
    // 가시 텍스트 문단0(3줄 vpos 0..2400) + 가시 문단1(2줄 vpos 1000..) — 문단1
    // 시작 vpos 가 문단0 끝(3600)보다 작아 vpos 리셋 → 문단1 앞에서 강제 분할.
    // [Task #1488] 가시 문단 사이 리셋은 하드 브레이크로 보존(Task #993 의도).
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![cell(
        0,
        0,
        vec![visible_text_para(3, 0), visible_text_para(2, 1000)],
    )]);
    // avail 충분해도 리셋에서 정지.
    let r = eng.advance_row_cut(&t, 0, &[], 1000.0, &styles);
    assert_eq!(r.end_cut, vec![3]);
    assert!(r.hit_hard_break);
    assert!(!r.fully_consumed);
    // 다음 프래그먼트: 리셋 지점부터 재개 — 시작 유닛은 리셋이어도 소비.
    let r2 = eng.advance_row_cut(&t, 0, &r.end_cut, 1000.0, &styles);
    assert_eq!(r2.end_cut, vec![5]);
    assert!(r2.fully_consumed);
}

#[test]
fn test_block_cut_row_offsets_absorbs_sliver_before_stored_hard_break() {
    // [#1921] 예산 정지 지점 직후 48px 이내에 저장 hard-break(vpos 리셋)가 있으면
    // 그 지점까지 흡수한다. 흡수하지 않으면 다음 fragment 가 극소 잔여(여기서는
    // 16px 유닛 1개)만 담은 sliver 페이지가 된다 (59043 pi=160: 946px→22px 교대).
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    // 문단0: 3줄(vpos 0..2400) = 유닛 3개(각 16px). 문단1: vpos 1000 리셋
    // → 유닛 3 앞 hard break.
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![visible_text_para(3, 0), visible_text_para(2, 1000)],
    )]);
    // 예산 40px: 유닛 0..2(32px)까지 들어가고 유닛 2(16px)에서 예산 정지 —
    // 잔여(유닛 2, 16px) 직후가 hard break 이므로 48px 한도 내 흡수.
    let r = eng.advance_row_block_cut_with_row_offsets(&t, 0, 1, &[], 40.0, &[0.0], &styles);
    assert_eq!(
        r.end_cut,
        vec![3],
        "예산 정지 직후 hard-break 까지 흡수 (sliver 방지)"
    );
    assert!(r.hit_hard_break);
    assert!(!r.fully_consumed);
    assert!(
        r.consumed_height <= 40.0 + 48.0,
        "흡수 오버플로는 48px 한도 내: {}",
        r.consumed_height
    );
    // 다음 fragment: hard-break 유닛부터 잔여 전부 — sliver 없음.
    let r2 = eng.advance_row_block_cut_with_row_offsets(
        &t,
        0,
        1,
        &r.end_cut,
        1000.0,
        &[0.0],
        &styles,
    );
    assert!(r2.fully_consumed);
}

#[test]
fn test_block_cut_row_offsets_no_absorb_beyond_tolerance() {
    // [#1921] hard-break 까지 잔여가 48px 를 넘으면 흡수하지 않는다 — 정상 예산
    // 분할 유지 (86712 공식PDF 핀 계열의 비정상 경계 강제 방지).
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    // 문단0: 8줄(128px). 예산 40px → 유닛 2에서 정지. hard break 는 유닛 8 앞
    // → 잔여 6유닛(96px) > 48px 한도 → 흡수 없음.
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![visible_text_para(8, 0), visible_text_para(2, 1000)],
    )]);
    let r = eng.advance_row_block_cut_with_row_offsets(&t, 0, 1, &[], 40.0, &[0.0], &styles);
    assert_eq!(r.end_cut, vec![2], "한도 초과 시 예산 경계 유지");
    assert!(!r.hit_hard_break);
}

#[test]
fn test_advance_row_cut_hwpx_midpage_vpos_reset_is_absorbed() {
    // HWPX 저장 LINE_SEG vpos 리셋이어도 페이지 절반 이상이 남은 중간 리셋이면
    // 로컬 좌표 재시작으로 보고 같은 쪽에 이어 담는다.
    let eng = LayoutEngine::new(96.0);
    eng.set_layout_profile(crate::model::provenance::LayoutCompatibilityProfile::new(
        false, false, true, true, false, false,
    ));
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![visible_text_para(4, 0), visible_text_para(2, 0)],
    )]);
    let r = eng.advance_row_cut(&t, 0, &[], 200.0, &styles);
    assert_eq!(
        r.end_cut,
        vec![6],
        "중간 vpos 리셋은 페이지 경계로 보존하지 않음"
    );
    assert!(r.fully_consumed);
}

#[test]
fn test_advance_row_cut_hwpx_bottom_vpos_reset_is_preserved() {
    // 같은 HWPX 저장 리셋이라도 이미 페이지 하단 근처까지 채운 경우에는
    // 한컴 저장 쪽 경계로 보존한다.
    let eng = LayoutEngine::new(96.0);
    eng.set_layout_profile(crate::model::provenance::LayoutCompatibilityProfile::new(
        false, false, true, true, false, false,
    ));
    let styles = ResolvedStyleSet::default();
    let t = rowbreak_table(vec![cell(
        0,
        0,
        vec![visible_text_para(4, 0), visible_text_para(2, 0)],
    )]);
    let r = eng.advance_row_cut(&t, 0, &[], 80.0, &styles);
    assert_eq!(r.end_cut, vec![4], "하단 vpos 리셋은 저장 쪽 경계로 보존");
    assert!(!r.fully_consumed);
}

#[test]
fn test_multi_page_single_cell_nested_reset_is_authoritative_in_parent_projection() {
    // [#3820 Stage 50] 페이지 하단에서 시작한 1×1 자식 표의 첫 fragment는
    // body 절반보다 짧을 수 있다. 단일 3600HU→0 reset이더라도 표 전체가
    // 물리 body보다 크면 부모 RowCut에 저장 쪽 경계로 투영해야 한다.
    let eng = LayoutEngine::new(96.0);
    eng.current_body_area.set((0.0, 0.0, 600.0, 120.0));
    let styles = ResolvedStyleSet::default();
    let nested = rowbreak_table(vec![cell(
        0,
        0,
        vec![visible_text_para(3, 0), visible_text_para(6, 0)],
    )]);

    let child_units = eng.cell_units(&nested.cells[0], &nested, &styles);
    let reset = child_units
        .iter()
        .position(|unit| unit.hard_break_before)
        .expect("단일 저장 vpos reset");
    assert!(
        !child_units[reset].stored_frame_break_before,
        "로컬 48px reset 자체는 body 절반(60px) 기준 authoritative가 아님"
    );

    let fragments = eng.nested_table_mixed_fragment_heights(&nested, &styles);
    assert_eq!(fragments.len(), child_units.len());
    assert!(fragments[reset].hard_break_before);
    assert!(
        fragments[reset].stored_frame_break_before,
        "물리 multi-page 1×1의 유일 reset은 부모 투영에서 authoritative"
    );
    assert!(fragments.iter().all(|fragment| fragment.recursive));
}

#[test]
fn test_direct_hwpx_nested_resets_keep_legacy_parent_projection() {
    // [#3820 Stage 50/#3637] direct HWPX의 반복 vpos reset은 자식 셀의
    // 로컬 viewport 좌표다. reset 개수만으로 HWP5 canonical cursor를
    // 적용하면 마지막 RowBreak fragment와 후속 표가 새 쪽으로 밀린다.
    let eng = LayoutEngine::new(96.0);
    eng.set_layout_profile(crate::model::provenance::LayoutCompatibilityProfile::new(
        false, false, true, true, false, false,
    ));
    eng.current_body_area.set((0.0, 0.0, 600.0, 120.0));
    let styles = ResolvedStyleSet::default();
    let nested = rowbreak_table(vec![cell(
        0,
        0,
        vec![
            visible_text_para(3, 0),
            visible_text_para(3, 0),
            visible_text_para(3, 0),
        ],
    )]);

    let child_units = eng.cell_units(&nested.cells[0], &nested, &styles);
    assert_eq!(
        child_units
            .iter()
            .filter(|unit| unit.hard_break_before)
            .count(),
        2,
        "fixture는 direct HWPX 로컬 reset 두 개를 가져야 함"
    );

    let fragments = eng.nested_table_mixed_fragment_heights(&nested, &styles);
    assert!(
        fragments.iter().all(|fragment| !fragment.recursive),
        "direct HWPX reset은 HWP5 canonical child cursor로 승격하지 않음"
    );
}

#[test]
fn test_advance_row_cut_empty_overlay_reset_no_hard_break() {
    // [Task #1488] 비가시(빈 텍스트) 오버레이 스페이서 문단이 만든 vpos 리셋은
    // 하드 브레이크가 아니다 — 셀 본문 위에 겹친 빈 문단들이 리셋마다 여분 빈
    // 페이지를 양산하던 회귀(rowbreak-problem-pages.hwpx sec1 pi=28)를 방지한다.
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![cell(
        0,
        0,
        vec![empty_overlay_para(3, 0), empty_overlay_para(2, 1000)],
    )]);
    let r = eng.advance_row_cut(&t, 0, &[], 1000.0, &styles);
    assert!(
        !r.hit_hard_break,
        "빈 오버레이 문단 리셋은 강제 분할하지 않음"
    );
    assert_eq!(r.end_cut, vec![5]);
    assert!(r.fully_consumed);
}

#[test]
fn test_advance_row_cut_rowbreak_rewinds_internal_hard_break_orphan() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    // [Task #1488] 가시 텍스트 문단으로 구성 — 가시 문단 사이 리셋은 하드 브레이크
    // 보존(Task #993 의도)이라 rewind-orphan 로직이 그대로 검증된다.
    let internal_reset = Paragraph {
        text: "가나다".to_string(),
        line_segs: vec![
            LineSeg {
                vertical_pos: 0,
                line_height: 1200,
                line_spacing: 0,
                ..Default::default()
            },
            LineSeg {
                vertical_pos: 0,
                line_height: 1200,
                line_spacing: 0,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let t = rowbreak_table(vec![
        rscell(0, 0, 2, vec![visible_text_para(1, 0)]),
        cell(
            1,
            1,
            vec![
                visible_text_para(1, 0),
                visible_text_para(1, 1200),
                internal_reset,
            ],
        ),
    ]);

    let r = eng.advance_row_cut(&t, 1, &[], 1000.0, &styles);

    assert_eq!(r.end_cut, vec![2]);
    assert!(r.hit_hard_break);
    assert!(!r.fully_consumed);
}

#[test]
fn test_advance_row_cut_multi_cell() {
    // 1행 2셀: 셀0=3줄, 셀1=6줄. avail 충분 → 각 셀 전부 소비,
    // consumed_height = 두 셀 표시 높이의 최댓값(셀1, 96px).
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![
        cell(0, 0, vec![text_para(3, 0)]),
        cell(0, 1, vec![text_para(6, 0)]),
    ]);
    let r = eng.advance_row_cut(&t, 0, &[], 500.0, &styles);
    assert_eq!(r.end_cut, vec![3, 6]);
    assert!(r.fully_consumed);
    assert!((r.consumed_height - 96.0).abs() < 0.5);
}

fn rscell(row: u16, col: u16, row_span: u16, paragraphs: Vec<Paragraph>) -> Cell {
    Cell {
        row,
        col,
        row_span,
        col_span: 1,
        width: 10000,
        paragraphs,
        ..Default::default()
    }
}

/// [Task #1025] 단일 비-rowspan 행에서 advance_row_block_cut == advance_row_cut (회귀 0).
#[test]
fn test_block_cut_single_row_parity() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![
        cell(0, 0, vec![text_para(3, 0)]),
        cell(0, 1, vec![text_para(6, 0)]),
    ]);
    for avail in [50.0, 96.0, 500.0, 5.0] {
        let a = eng.advance_row_cut(&t, 0, &[], avail, &styles);
        let b = eng.advance_row_block_cut(&t, 0, 1, &[], avail, &styles);
        assert_eq!(a.end_cut, b.end_cut, "avail={avail}");
        assert_eq!(a.fully_consumed, b.fully_consumed, "avail={avail}");
        assert_eq!(a.hit_hard_break, b.hit_hard_break, "avail={avail}");
        assert!(
            (a.consumed_height - b.consumed_height).abs() < 0.5,
            "avail={avail}"
        );
    }
}

/// [Task #1025] rowspan 블록(rows 0-1)에서 거대 row_span==1 셀이 줄 단위로 분할.
/// cell[label] r=0 rs=2(2줄), cell[a] r=0(2줄), cell[big] r=1(10줄).
/// avail=80px(=5줄): 첫 조각은 라벨2 + a2 + big5 까지, big 잔여 5줄은 다음 조각.
#[test]
fn test_block_cut_rowspan_giant_split() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let t = table(vec![
        rscell(0, 0, 2, vec![text_para(2, 0)]), // 라벨 (rows 0-1 걸침)
        cell(0, 1, vec![text_para(2, 0)]),      // row 0 일반 셀
        cell(1, 1, vec![text_para(10, 0)]),     // row 1 거대 셀 (10줄=160px)
    ]);
    // 셀 순서 (row,col): [ (0,0)라벨, (0,1)a, (1,1)big ]
    let first = eng.advance_row_block_cut(&t, 0, 2, &[], 80.0, &styles);
    // 라벨 2줄 전량, a 2줄 전량, big 5줄(80px) 까지.
    assert_eq!(first.end_cut, vec![2, 2, 5], "first: {:?}", first.end_cut);
    assert!(!first.fully_consumed);
    // 연속 조각: 라벨/a 는 이미 전량(공란), big 잔여 5줄.
    let cont = eng.advance_row_block_cut(&t, 0, 2, &first.end_cut, 500.0, &styles);
    assert_eq!(cont.end_cut, vec![2, 2, 10], "cont: {:?}", cont.end_cut);
    assert!(cont.fully_consumed);
}

/// [Issue #2214 Stage 3] 실제 deferred insert 호출부가 edited cell만 제거하는지
/// 고정한다. #2214 fixture의 owner table-wide nested-text flag는 입력 전후 불변이므로
/// flag와 same-table sibling identity를 함께 보존해야 한다.
#[test]
fn issue2214_deferred_insert_uses_scoped_cache_eviction() {
    use crate::document_core::DocumentCore;

    fn owner_table(core: &DocumentCore) -> &Table {
        match &core.document.sections[0].paragraphs[0].controls[2] {
            Control::Table(table) => table.as_ref(),
            other => panic!("#2214 owner control is not a table: {other:?}"),
        }
    }

    fn uncached_table_flag(table: &Table) -> bool {
        table.cells.iter().any(|cell| {
            cell.paragraphs.iter().any(|para| {
                !para.text.trim().is_empty()
                    && para
                        .controls
                        .iter()
                        .any(|control| matches!(control, Control::Table(_)))
            })
        })
    }

    let mut failures = Vec::new();
    for (format_label, relative) in [
        ("hwp", "samples/issue1949_giant_cell_nested_tables_perf.hwp"),
        (
            "hwpx",
            "samples/issue1949_giant_cell_nested_tables_perf.hwpx",
        ),
    ] {
        for (phase, preinsert_count) in [("stable", 0), ("flow-boundary", 43)] {
            let label = format!("{format_label}-{phase}");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
            let bytes = std::fs::read(path).expect("read #2214 fixture");
            let mut core = DocumentCore::from_bytes(&bytes).expect("load #2214 fixture");
            assert_eq!(core.page_count(), 115, "{label}: initial page count");
            for inserted in 0..preinsert_count {
                core.insert_text_in_cell_native_deferred_pagination(
                    0,
                    0,
                    2,
                    2,
                    5,
                    130 + inserted,
                    "1",
                )
                .expect("prepare flow boundary");
            }

            let (
                table_key,
                target_key,
                sibling_key,
                target_before,
                sibling_before,
                target_shape_before,
                owner_flag_before,
                target_units_fp_before,
            ) = {
                let table = owner_table(&core);
                let target = &table.cells[2];
                let sibling = &table.cells[1];
                let target_before = core.layout_engine.cell_units(target, table, &core.styles);
                let sibling_before =
                    core.layout_engine.cell_units(sibling, table, &core.styles);
                let target_para = &target.paragraphs[5];
                (
                    table as *const Table as usize,
                    target as *const Cell as usize,
                    sibling as *const Cell as usize,
                    target_before,
                    sibling_before,
                    (
                        !target_para.text.trim().is_empty(),
                        target_para
                            .controls
                            .iter()
                            .any(|control| matches!(control, Control::Table(_))),
                    ),
                    uncached_table_flag(table),
                    LayoutEngine::cell_paragraph_units_fingerprint(target_para),
                )
            };
            assert!(
                core.layout_engine
                    .table_nested_text_flag_cache
                    .borrow()
                    .contains_key(&table_key),
                "{label}: owner flag must be warmed by cell units"
            );
            core.layout_engine.table_nested_text_flag_scan_count.set(0);

            core.insert_text_in_cell_native_deferred_pagination(
                0,
                0,
                2,
                2,
                5,
                130 + preinsert_count,
                "1",
            )
            .expect("deferred one-char insert");
            assert_eq!(core.page_count(), 115, "{label}: deferred page count");

            let table = owner_table(&core);
            let target = &table.cells[2];
            let sibling = &table.cells[1];
            assert_eq!(
                table as *const Table as usize, table_key,
                "{label}: owner table pointer stability"
            );
            assert_eq!(
                target as *const Cell as usize, target_key,
                "{label}: target cell pointer stability"
            );
            assert_eq!(
                sibling as *const Cell as usize, sibling_key,
                "{label}: sibling cell pointer stability"
            );
            let target_para = &target.paragraphs[5];
            let target_shape_after = (
                !target_para.text.trim().is_empty(),
                target_para
                    .controls
                    .iter()
                    .any(|control| matches!(control, Control::Table(_))),
            );
            let owner_flag_after_uncached = uncached_table_flag(table);
            assert_eq!(
                target_shape_after, target_shape_before,
                "{label}: target visible-text/nested-table shape must be invariant"
            );
            assert_eq!(
                owner_flag_after_uncached, owner_flag_before,
                "{label}: owner table-wide flag must be invariant"
            );

            let membership = {
                let cell_cache = core.layout_engine.cell_units_cache.borrow();
                let flag_cache = core.layout_engine.table_nested_text_flag_cache.borrow();
                (
                    cell_cache.contains_key(&target_key),
                    cell_cache.contains_key(&sibling_key),
                    flag_cache.contains_key(&table_key),
                )
            };
            let target_after = core.layout_engine.cell_units(target, table, &core.styles);
            let sibling_after = core.layout_engine.cell_units(sibling, table, &core.styles);
            let owner_flag_after = core
                .layout_engine
                .table_has_visible_text_with_nested_table(table);
            let table_scan_count = core.layout_engine.table_nested_text_flag_scan_count.get();
            let target_recomputed = !std::sync::Arc::ptr_eq(&target_before, &target_after);
            let sibling_reused = std::sync::Arc::ptr_eq(&sibling_before, &sibling_after);
            // [#4167 갱신 이력] 기대값을 "무조건 evict"에서 "units 지문 변화 시에만
            // evict"로 변경. 제자리 1자 삽입은 units 지문(줄 수·높이·vpos·synthetic·
            // 공백 클래스) 불변이라 캐시 항등 보존이 정답이 됐다 — 재계산해도 동일
            // 벡터가 나옴은 issue4167_units_fingerprint_doc_contract 가 고정한다.
            // 실측상 두 phase 의 최종 삽입 모두 지문 불변(재래핑 없음)이며, 지문이
            // 변하는 편집의 evict 계약은 issue4167_fingerprint_unchanged_edit_
            // retains_cell_units 의 false 분기가 고정한다.
            let target_units_fp_after =
                LayoutEngine::cell_paragraph_units_fingerprint(&target.paragraphs[5]);
            let fp_stable = target_units_fp_after == target_units_fp_before;
            let desired = if fp_stable {
                membership == (true, true, true)
                    && !target_recomputed
                    && sibling_reused
                    && owner_flag_after == owner_flag_before
                    && table_scan_count == 0
            } else {
                membership == (false, true, true)
                    && target_recomputed
                    && sibling_reused
                    && owner_flag_after == owner_flag_before
                    && table_scan_count == 0
            };
            eprintln!(
                "#2214 {label}: membership={membership:?} target_recomputed={target_recomputed} sibling_reused={sibling_reused} owner_flag={owner_flag_before}->{owner_flag_after} table_scans={table_scan_count}"
            );
            if !desired {
                failures.push(format!(
                    "{label}: membership={membership:?} target_recomputed={target_recomputed} sibling_reused={sibling_reused} owner_flag_stable={} table_scans={table_scan_count}",
                    owner_flag_after == owner_flag_before,
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "deferred insert must use scoped cache eviction:\n{}",
        failures.join("\n")
    );
}

/// [Issue #2214 Stage 3] 실제 deferred insert가 빈 nested-table host를 non-empty로
/// 바꿔 owner flag가 false→true가 되는 경우, owner table의 모든 cell units를 evict하고
/// flag를 true로 갱신하되 nested table 자체의 cache는 보존해야 한다.
#[test]
fn issue2214_deferred_insert_flag_change_evicts_owner_cells() {
    use crate::document_core::DocumentCore;

    fn owner_table(core: &DocumentCore) -> &Table {
        match &core.document.sections[0].paragraphs[0].controls[2] {
            Control::Table(table) => table.as_ref(),
            other => panic!("#2214 owner control is not a table: {other:?}"),
        }
    }

    fn uncached_table_flag(table: &Table) -> bool {
        table.cells.iter().any(|cell| {
            cell.paragraphs.iter().any(|para| {
                !para.text.trim().is_empty()
                    && para
                        .controls
                        .iter()
                        .any(|control| matches!(control, Control::Table(_)))
            })
        })
    }

    let mut failures = Vec::new();
    for (label, relative) in [
        ("hwp", "samples/issue1949_giant_cell_nested_tables_perf.hwp"),
        (
            "hwpx",
            "samples/issue1949_giant_cell_nested_tables_perf.hwpx",
        ),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let bytes = std::fs::read(path).expect("read #2214 fixture");
        let mut core = DocumentCore::from_bytes(&bytes).expect("load #2214 fixture");
        let (host_cell, host_para, nested_control) = owner_table(&core)
            .cells
            .iter()
            .enumerate()
            .find_map(|(cell_index, cell)| {
                cell.paragraphs
                    .iter()
                    .enumerate()
                    .find_map(|(para_index, para)| {
                        if !para.text.trim().is_empty() {
                            return None;
                        }
                        para.controls
                            .iter()
                            .enumerate()
                            .find_map(|(control_index, control)| match control {
                                Control::Table(table) if !table.cells.is_empty() => {
                                    Some((cell_index, para_index, control_index))
                                }
                                _ => None,
                            })
                    })
            })
            .expect("#2214 fixture must contain an empty nested-table host");

        let (
            owner_table_key,
            owner_cell_keys,
            owner_before,
            nested_table_key,
            nested_cell_key,
            nested_before,
        ) = {
            let table = owner_table(&core);
            assert!(
                !uncached_table_flag(table),
                "{label}: owner flag must start false"
            );
            let nested =
                match &table.cells[host_cell].paragraphs[host_para].controls[nested_control] {
                    Control::Table(table) => table.as_ref(),
                    other => panic!("nested control changed: {other:?}"),
                };
            let owner_before = table
                .cells
                .iter()
                .map(|cell| core.layout_engine.cell_units(cell, table, &core.styles))
                .collect::<Vec<_>>();
            let nested_before =
                core.layout_engine
                    .cell_units(&nested.cells[0], nested, &core.styles);
            (
                table as *const Table as usize,
                table
                    .cells
                    .iter()
                    .map(|cell| cell as *const Cell as usize)
                    .collect::<Vec<_>>(),
                owner_before,
                nested as *const Table as usize,
                &nested.cells[0] as *const Cell as usize,
                nested_before,
            )
        };
        assert_eq!(
            core.layout_engine
                .table_nested_text_flag_cache
                .borrow()
                .get(&owner_table_key)
                .copied(),
            Some(false),
            "{label}: cached owner flag before edit"
        );
        core.layout_engine.table_nested_text_flag_scan_count.set(0);

        core.insert_text_in_cell_native_deferred_pagination(
            0, 0, 2, host_cell, host_para, 0, "x",
        )
        .expect("deferred nested-host insert");
        assert_eq!(core.page_count(), 115, "{label}: deferred page count");

        let table = owner_table(&core);
        assert_eq!(
            table as *const Table as usize, owner_table_key,
            "{label}: owner table pointer stability"
        );
        assert!(
            uncached_table_flag(table),
            "{label}: nested-host insert must flip the uncached owner flag"
        );
        assert!(
            !table.cells[host_cell].paragraphs[host_para]
                .text
                .trim()
                .is_empty(),
            "{label}: nested host text"
        );
        let nested =
            match &table.cells[host_cell].paragraphs[host_para].controls[nested_control] {
                Control::Table(table) => table.as_ref(),
                other => panic!("nested control changed: {other:?}"),
            };
        assert_eq!(
            nested as *const Table as usize, nested_table_key,
            "{label}: nested table pointer stability"
        );
        assert_eq!(
            &nested.cells[0] as *const Cell as usize, nested_cell_key,
            "{label}: nested cell pointer stability"
        );
        assert_eq!(
            table
                .cells
                .iter()
                .map(|cell| cell as *const Cell as usize)
                .collect::<Vec<_>>(),
            owner_cell_keys,
            "{label}: owner cell pointer stability"
        );

        let membership = {
            let cell_cache = core.layout_engine.cell_units_cache.borrow();
            let flag_cache = core.layout_engine.table_nested_text_flag_cache.borrow();
            (
                owner_cell_keys
                    .iter()
                    .any(|key| cell_cache.contains_key(key)),
                cell_cache.contains_key(&nested_cell_key),
                flag_cache.get(&owner_table_key).copied(),
                flag_cache.contains_key(&nested_table_key),
            )
        };
        let owner_after = table
            .cells
            .iter()
            .map(|cell| core.layout_engine.cell_units(cell, table, &core.styles))
            .collect::<Vec<_>>();
        let nested_after =
            core.layout_engine
                .cell_units(&nested.cells[0], nested, &core.styles);
        let table_scan_count = core.layout_engine.table_nested_text_flag_scan_count.get();
        let owner_recomputed = owner_before
            .iter()
            .zip(&owner_after)
            .all(|(before, after)| !std::sync::Arc::ptr_eq(before, after));
        let nested_reused = std::sync::Arc::ptr_eq(&nested_before, &nested_after);
        let desired = membership == (false, true, Some(true), true)
            && owner_recomputed
            && nested_reused
            && table_scan_count == 0;
        eprintln!(
            "#2214 {label}-flag-change: membership={membership:?} owner_recomputed={owner_recomputed} nested_reused={nested_reused} table_scans={table_scan_count}"
        );
        if !desired {
            failures.push(format!(
                "{label}: membership={membership:?} owner_recomputed={owner_recomputed} nested_reused={nested_reused} table_scans={table_scan_count}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "deferred flag change must use owner-wide scoped eviction:\n{}",
        failures.join("\n")
    );
}

/// [Issue #2214 Stage 3] owner table-wide flag가 불변이면 edited cell만 evict하고
/// cached owner flag와 sibling/unrelated cache를 보존한다.
#[test]
fn issue2214_scoped_eviction_retains_unrelated_cache() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let edited_table = table(vec![
        cell(0, 0, vec![text_para(2, 0)]),
        cell(0, 1, vec![text_para(4, 0)]),
    ]);
    let unrelated_table = table(vec![cell(0, 0, vec![text_para(3, 0)])]);

    let edited_before = eng.cell_units(&edited_table.cells[0], &edited_table, &styles);
    let sibling_before = eng.cell_units(&edited_table.cells[1], &edited_table, &styles);
    let unrelated_before = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    let _ = eng.table_has_visible_text_with_nested_table(&edited_table);
    let _ = eng.table_has_visible_text_with_nested_table(&unrelated_table);

    assert_eq!(
        eng.cell_units_cache.borrow().len(),
        3,
        "three warmed cell entries"
    );
    assert_eq!(
        eng.table_nested_text_flag_cache.borrow().len(),
        2,
        "two warmed table-flag entries"
    );

    let edited_cell_key = &edited_table.cells[0] as *const crate::model::table::Cell as usize;
    let sibling_cell_key = &edited_table.cells[1] as *const crate::model::table::Cell as usize;
    let unrelated_cell_key =
        &unrelated_table.cells[0] as *const crate::model::table::Cell as usize;
    let owner_table_key = &edited_table as *const crate::model::table::Table as usize;
    let unrelated_table_key = &unrelated_table as *const crate::model::table::Table as usize;
    eng.invalidate_cell_units_after_text_edit(
        &edited_table.cells[0],
        &edited_table,
        false,
        false,
        false,
    );

    let cell_cache = eng.cell_units_cache.borrow();
    let flag_cache = eng.table_nested_text_flag_cache.borrow();
    let membership = (
        cell_cache.contains_key(&edited_cell_key),
        cell_cache.contains_key(&sibling_cell_key),
        cell_cache.contains_key(&unrelated_cell_key),
        flag_cache.contains_key(&owner_table_key),
        flag_cache.contains_key(&unrelated_table_key),
    );
    drop(cell_cache);
    drop(flag_cache);
    assert_eq!(
        membership,
        (false, true, true, true, true),
        "desired scoped membership: edited cell evicted; owner flag, sibling and unrelated caches retained"
    );

    let edited_after = eng.cell_units(&edited_table.cells[0], &edited_table, &styles);
    let sibling_after = eng.cell_units(&edited_table.cells[1], &edited_table, &styles);
    let unrelated_after = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    assert!(
        !std::sync::Arc::ptr_eq(&edited_before, &edited_after),
        "edited cell units must be recomputed"
    );
    assert!(
        std::sync::Arc::ptr_eq(&sibling_before, &sibling_after),
        "same-table sibling units must be reused"
    );
    assert!(
        std::sync::Arc::ptr_eq(&unrelated_before, &unrelated_after),
        "unrelated-table units must be reused"
    );
}

/// [Issue #2214 Stage 3] cold false→true는 기존 owner cell cache가 없으므로
/// owner-wide key 순회 없이 local witness로 flag=true를 기록한다.
#[test]
fn issue2214_cold_local_change_records_true_without_table_scan() {
    let eng = LayoutEngine::new(96.0);
    let nested_table = table(vec![cell(0, 0, vec![visible_text_para(1, 0)])]);
    let mut nested_host = text_para(1, 0);
    nested_host.text.clear();
    nested_host.char_count = 0;
    nested_host
        .controls
        .push(Control::Table(Box::new(nested_table)));
    let mut owner_table = rowbreak_table(vec![
        cell(0, 0, vec![nested_host]),
        cell(0, 1, vec![visible_text_para(2, 0)]),
    ]);
    let owner_table_key = &owner_table as *const Table as usize;

    assert!(eng.cell_units_cache.borrow().is_empty());
    assert!(eng.table_nested_text_flag_cache.borrow().is_empty());
    eng.table_nested_text_flag_scan_count.set(0);

    owner_table.cells[0].paragraphs[0].insert_text_at(0, "x");
    eng.invalidate_cell_units_after_text_edit(
        &owner_table.cells[0],
        &owner_table,
        false,
        true,
        false,
    );

    assert!(eng.cell_units_cache.borrow().is_empty());
    assert_eq!(
        eng.table_nested_text_flag_cache
            .borrow()
            .get(&owner_table_key)
            .copied(),
        Some(true)
    );
    assert!(eng.table_has_visible_text_with_nested_table(&owner_table));
    assert_eq!(eng.table_nested_text_flag_scan_count.get(), 0);
}

/// [Issue #2214 Stage 3] 다른 host가 이미 owner flag=true를 만든 상태에서 두 번째
/// empty nested host가 non-empty가 되어도 table-wide 값은 불변이다. 이 branch는 edited
/// cell만 evict하고 owner flag·다른 owner cells·unrelated cache를 보존해야 한다.
#[test]
fn issue2214_cached_true_local_change_evicts_edited_cell_only() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();

    let mut visible_host = visible_text_para(1, 0);
    visible_host
        .controls
        .push(Control::Table(Box::new(table(vec![cell(
            0,
            0,
            vec![visible_text_para(1, 0)],
        )]))));
    let mut empty_host = text_para(1, 0);
    empty_host.text.clear();
    empty_host.char_count = 0;
    empty_host
        .controls
        .push(Control::Table(Box::new(table(vec![cell(
            0,
            0,
            vec![visible_text_para(1, 0)],
        )]))));
    let mut edited_table = rowbreak_table(vec![
        cell(0, 0, vec![visible_host]),
        cell(0, 1, vec![empty_host]),
        cell(1, 0, vec![visible_text_para(2, 0)]),
        cell(1, 1, vec![visible_text_para(2, 0)]),
    ]);
    let unrelated_table = table(vec![cell(0, 0, vec![text_para(3, 0)])]);

    let owner_before = edited_table
        .cells
        .iter()
        .map(|cell| eng.cell_units(cell, &edited_table, &styles))
        .collect::<Vec<_>>();
    let unrelated_before = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    assert!(
        eng.table_has_visible_text_with_nested_table(&edited_table),
        "first visible nested host must set owner flag=true"
    );
    let _ = eng.table_has_visible_text_with_nested_table(&unrelated_table);
    let owner_cell_keys = edited_table
        .cells
        .iter()
        .map(|cell| cell as *const crate::model::table::Cell as usize)
        .collect::<Vec<_>>();
    let unrelated_cell_key =
        &unrelated_table.cells[0] as *const crate::model::table::Cell as usize;
    let owner_table_key = &edited_table as *const crate::model::table::Table as usize;
    let unrelated_table_key = &unrelated_table as *const crate::model::table::Table as usize;
    eng.table_nested_text_flag_scan_count.set(0);

    edited_table.cells[1].paragraphs[0].insert_text_at(0, "x");
    eng.invalidate_cell_units_after_text_edit(
        &edited_table.cells[1],
        &edited_table,
        false,
        true,
        false,
    );

    let membership = {
        let cell_cache = eng.cell_units_cache.borrow();
        let flag_cache = eng.table_nested_text_flag_cache.borrow();
        (
            cell_cache.contains_key(&owner_cell_keys[1]),
            owner_cell_keys
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != 1)
                .all(|(_, key)| cell_cache.contains_key(key)),
            cell_cache.contains_key(&unrelated_cell_key),
            flag_cache.get(&owner_table_key).copied(),
            flag_cache.contains_key(&unrelated_table_key),
        )
    };
    let owner_after = edited_table
        .cells
        .iter()
        .map(|cell| eng.cell_units(cell, &edited_table, &styles))
        .collect::<Vec<_>>();
    let unrelated_after = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    let edited_recomputed = !std::sync::Arc::ptr_eq(&owner_before[1], &owner_after[1]);
    let siblings_reused = owner_before
        .iter()
        .zip(&owner_after)
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .all(|(_, (before, after))| std::sync::Arc::ptr_eq(before, after));
    let unrelated_reused = std::sync::Arc::ptr_eq(&unrelated_before, &unrelated_after);
    let table_scan_count = eng.table_nested_text_flag_scan_count.get();
    assert!(
        membership == (false, true, true, Some(true), true)
            && edited_recomputed
            && siblings_reused
            && unrelated_reused
            && table_scan_count == 0,
        "cached-true local change scope: membership={membership:?} edited_recomputed={edited_recomputed} siblings_reused={siblings_reused} unrelated_reused={unrelated_reused} table_scans={table_scan_count}"
    );
}

/// [Issue #2214 Stage 3] owner table-wide nested-text flag가 바뀌면 같은 표의 모든
/// cell units가 stale할 수 있다. 이때 owner-table-wide eviction은 허용하되 unrelated
/// table cache는 보존해야 한다.
#[test]
fn issue2214_table_flag_change_evicts_owner_cells_only() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let nested_table = table(vec![cell(0, 0, vec![visible_text_para(1, 0)])]);
    let mut nested_host = text_para(1, 0);
    nested_host.text.clear();
    nested_host.char_count = 0;
    nested_host
        .controls
        .push(Control::Table(Box::new(nested_table)));
    let mut edited_table = rowbreak_table(vec![
        cell(0, 0, vec![nested_host]),
        cell(0, 1, vec![visible_text_para(2, 0)]),
        cell(1, 0, vec![visible_text_para(2, 0)]),
        cell(1, 1, vec![visible_text_para(2, 0)]),
    ]);
    let unrelated_table = table(vec![cell(0, 0, vec![text_para(3, 0)])]);

    let owner_before = edited_table
        .cells
        .iter()
        .map(|cell| eng.cell_units(cell, &edited_table, &styles))
        .collect::<Vec<_>>();
    let unrelated_before = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    assert!(
        !eng.table_has_visible_text_with_nested_table(&edited_table),
        "empty nested host must start with a false owner flag"
    );
    let _ = eng.table_has_visible_text_with_nested_table(&unrelated_table);
    eng.table_nested_text_flag_scan_count.set(0);

    edited_table.cells[0].paragraphs[0].insert_text_at(0, "x");
    assert!(
        edited_table.cells.iter().any(|cell| {
            cell.paragraphs.iter().any(|para| {
                !para.text.trim().is_empty()
                    && para
                        .controls
                        .iter()
                        .any(|control| matches!(control, Control::Table(_)))
            })
        }),
        "edit must flip the uncached owner flag to true"
    );

    let owner_cell_keys = edited_table
        .cells
        .iter()
        .map(|cell| cell as *const crate::model::table::Cell as usize)
        .collect::<Vec<_>>();
    let unrelated_cell_key =
        &unrelated_table.cells[0] as *const crate::model::table::Cell as usize;
    let owner_table_key = &edited_table as *const crate::model::table::Table as usize;
    let unrelated_table_key = &unrelated_table as *const crate::model::table::Table as usize;
    eng.invalidate_cell_units_after_text_edit(
        &edited_table.cells[0],
        &edited_table,
        false,
        true,
        false,
    );

    let membership = {
        let cell_cache = eng.cell_units_cache.borrow();
        let flag_cache = eng.table_nested_text_flag_cache.borrow();
        (
            owner_cell_keys
                .iter()
                .any(|key| cell_cache.contains_key(key)),
            cell_cache.contains_key(&unrelated_cell_key),
            flag_cache.get(&owner_table_key).copied(),
            flag_cache.contains_key(&unrelated_table_key),
        )
    };
    assert_eq!(
        membership,
        (false, true, Some(true), true),
        "flag change must evict all owner cells, update owner flag, and retain unrelated caches"
    );

    let owner_after = edited_table
        .cells
        .iter()
        .map(|cell| eng.cell_units(cell, &edited_table, &styles))
        .collect::<Vec<_>>();
    let unrelated_after = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    let table_scan_count = eng.table_nested_text_flag_scan_count.get();
    assert!(
        owner_before
            .iter()
            .zip(&owner_after)
            .all(|(before, after)| !std::sync::Arc::ptr_eq(before, after)),
        "all owner-table cell units must be recomputed after owner flag change"
    );
    assert!(
        std::sync::Arc::ptr_eq(&unrelated_before, &unrelated_after),
        "unrelated-table units must be reused"
    );
    assert!(
        eng.table_has_visible_text_with_nested_table(&edited_table),
        "owner flag must recompute to true"
    );
    assert_eq!(
        table_scan_count, 0,
        "flag update and cache rewarm must not rescan the owner table"
    );
}

/// [Issue #2424] 삭제로 visible nested host가 비게 되면 true owner flag를
/// 보수적으로 버리고 owner cell units만 다시 계산한다. unrelated cache는 유지한다.
#[test]
fn issue2424_delete_local_contribution_recomputes_owner_flag_only() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let nested_table = table(vec![cell(0, 0, vec![visible_text_para(1, 0)])]);
    let mut nested_host = visible_text_para(1, 0);
    nested_host
        .controls
        .push(Control::Table(Box::new(nested_table)));
    let mut owner_table = rowbreak_table(vec![
        cell(0, 0, vec![nested_host]),
        cell(0, 1, vec![visible_text_para(2, 0)]),
    ]);
    let unrelated_table = table(vec![cell(0, 0, vec![visible_text_para(3, 0)])]);

    let owner_before = owner_table
        .cells
        .iter()
        .map(|cell| eng.cell_units(cell, &owner_table, &styles))
        .collect::<Vec<_>>();
    let unrelated_before = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    assert!(eng.table_has_visible_text_with_nested_table(&owner_table));
    let owner_table_key = &owner_table as *const Table as usize;
    let unrelated_table_key = &unrelated_table as *const Table as usize;
    eng.table_nested_text_flag_scan_count.set(0);

    owner_table.cells[0].paragraphs[0].text.clear();
    owner_table.cells[0].paragraphs[0].char_count = 0;
    eng.invalidate_cell_units_after_text_edit(
        &owner_table.cells[0],
        &owner_table,
        true,
        false,
        false,
    );

    assert!(
        owner_table.cells.iter().all(|cell| {
            let key = cell as *const crate::model::table::Cell as usize;
            !eng.cell_units_cache.borrow().contains_key(&key)
        }),
        "all owner cell units must be evicted"
    );
    assert!(
        !eng.table_nested_text_flag_cache
            .borrow()
            .contains_key(&owner_table_key),
        "owner flag must be recomputed after a true→false local change"
    );
    assert!(
        eng.table_nested_text_flag_cache
            .borrow()
            .contains_key(&unrelated_table_key),
        "unrelated flag must be retained"
    );

    let owner_after = owner_table
        .cells
        .iter()
        .map(|cell| eng.cell_units(cell, &owner_table, &styles))
        .collect::<Vec<_>>();
    let unrelated_after = eng.cell_units(&unrelated_table.cells[0], &unrelated_table, &styles);
    assert!(owner_before
        .iter()
        .zip(&owner_after)
        .all(|(before, after)| !std::sync::Arc::ptr_eq(before, after)));
    assert!(std::sync::Arc::ptr_eq(&unrelated_before, &unrelated_after));
    assert!(!eng.table_has_visible_text_with_nested_table(&owner_table));
    assert_eq!(
        eng.table_nested_text_flag_scan_count.get(),
        1,
        "owner table must be rescanned once after deletion"
    );
}

/// [#4167] 지문 불변 편집(제자리 타이핑)은 memoized cell units 를 보존한다.
#[test]
fn issue4167_fingerprint_unchanged_edit_retains_cell_units() {
    let eng = LayoutEngine::new(96.0);
    let styles = ResolvedStyleSet::default();
    let owner_table = table(vec![cell(0, 0, vec![text_para(3, 0)])]);
    let _ = eng.cell_units(&owner_table.cells[0], &owner_table, &styles);
    let key = &owner_table.cells[0] as *const crate::model::table::Cell as usize;
    assert!(eng.cell_units_cache.borrow().contains_key(&key), "warmed");

    eng.invalidate_cell_units_after_text_edit(
        &owner_table.cells[0],
        &owner_table,
        true,
        true,
        true,
    );
    assert!(
        eng.cell_units_cache.borrow().contains_key(&key),
        "지문 불변이면 entry 를 보존해야 한다 — 제거되면 거대 셀 타이핑마다 전량 recompose (#4167)"
    );

    eng.invalidate_cell_units_after_text_edit(
        &owner_table.cells[0],
        &owner_table,
        true,
        true,
        false,
    );
    assert!(
        !eng.cell_units_cache.borrow().contains_key(&key),
        "지문 변경이면 종전대로 제거해야 한다"
    );
}

/// [#4167] units 지문은 units 산출이 읽는 입력에만 반응한다.
#[test]
fn issue4167_units_fingerprint_sensitivity() {
    let base = text_para(3, 0);
    let fp = LayoutEngine::cell_paragraph_units_fingerprint;

    // 불변이어야 하는 변이: text_start·segment_width 시프트, synthetic 외 tag 비트
    let mut typed = base.clone();
    for seg in &mut typed.line_segs {
        seg.text_start += 1;
        seg.segment_width += 120;
        seg.tag |= 0x0010_0000; // 원본 로드 tag 잔여 비트 — units 미소비
    }
    assert_eq!(
        fp(&base),
        fp(&typed),
        "제자리 타이핑 급 변이는 지문 불변이어야 한다"
    );

    // 변해야 하는 변이: 줄 수, 줄 높이, synthetic 비트, 공백 클래스
    let mut grown = base.clone();
    grown.line_segs.push(grown.line_segs[0].clone());
    assert_ne!(fp(&base), fp(&grown), "줄 수 변화는 지문이 변해야 한다");

    let mut taller = base.clone();
    taller.line_segs[1].line_height += 240;
    assert_ne!(fp(&base), fp(&taller), "줄 높이 변화는 지문이 변해야 한다");

    let mut synthetic = base.clone();
    synthetic.line_segs[1].tag |= crate::model::paragraph::LineSeg::TAG_IMPLEMENTATION_PROPERTY;
    assert_ne!(
        fp(&base),
        fp(&synthetic),
        "synthetic 비트는 지문이 변해야 한다"
    );

    let mut spacer = base.clone();
    spacer.text = "   ".to_string();
    assert_ne!(
        fp(&base),
        fp(&spacer),
        "공백 스페이서 클래스 전이는 지문이 변해야 한다"
    );
}
