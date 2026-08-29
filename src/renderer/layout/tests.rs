use super::super::page_layout::PageLayoutInfo;
use super::super::pagination::{ColumnContent, PageContent, PageItem};
use super::text_measurement::estimate_text_width;
use super::utils::{expand_numbering_format, numbering_format_to_number_format};
use super::*;
use crate::model::image::Picture;
use crate::model::page::{ColumnDef, PageDef};
use crate::model::paragraph::{CharShapeRef, LineSeg, Paragraph};
use crate::model::shape::{
    Caption, CaptionDirection, CommonObjAttr, RectangleShape, TextWrap, VertRelTo,
};
use crate::model::style::{Numbering, NumberingHead};
use crate::model::table::{Cell, Table, TablePageBreak};
use crate::renderer::composer::compose_paragraph;
use crate::renderer::style_resolver::ResolvedStyleSet;
use crate::renderer::{TabStop, TextStyle};

fn a4_page_def() -> PageDef {
    PageDef {
        width: 59528,
        height: 84188,
        margin_left: 8504,
        margin_right: 8504,
        margin_top: 5669,
        margin_bottom: 4252,
        margin_header: 4252,
        margin_footer: 4252,
        margin_gutter: 0,
        ..Default::default()
    }
}

#[test]
fn test_build_empty_page() {
    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());
    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: Vec::new(),
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };
    let styles = ResolvedStyleSet::default();
    let tree = engine.build_render_tree(
        &page_content,
        &[],
        &[],
        &[],
        &[],
        &styles,
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    );
    // 페이지 노드 + 배경 + 머리말 + 본문 + 각주 + 꼬리말
    assert!(tree.root.children.len() >= 4);
}

#[test]
fn physical_outer_box_paint_inset_layout_gate_requires_single_active_zone_column_and_height_match()
{
    use super::physical_outer_box_paint_inset_layout_gate;

    assert!(physical_outer_box_paint_inset_layout_gate(
        1,
        Some(317.2),
        317.2,
    ));
    assert!(physical_outer_box_paint_inset_layout_gate(
        1,
        Some(317.7),
        317.2,
    ));
    assert!(!physical_outer_box_paint_inset_layout_gate(
        2,
        Some(317.2),
        317.2,
    ));
    assert!(!physical_outer_box_paint_inset_layout_gate(1, None, 317.2,));
    assert!(!physical_outer_box_paint_inset_layout_gate(
        1,
        Some(317.71),
        317.2,
    ));
}

#[test]
fn note_separator_length_resolves_schema_sentinels_and_absolute_hwpunit() {
    use super::{footnote_separator_length_px, note_separator_length_px};

    let width = 600.0;
    let dpi = 96.0;
    assert!((note_separator_length_px(-1, width, dpi) - 188.976_377_95).abs() < 1e-6);
    assert!((note_separator_length_px(-2, width, dpi) - 75.590_551_18).abs() < 1e-6);
    assert!((note_separator_length_px(-3, width, dpi) - 200.0).abs() < 1e-9);
    assert!((note_separator_length_px(-4, width, dpi) - width).abs() < 1e-9);
    assert!((note_separator_length_px(7_200, width, dpi) - 96.0).abs() < 1e-9);
    assert!((note_separator_length_px(72_000, width, dpi) - width).abs() < 1e-9);
    assert_eq!(note_separator_length_px(0, width, dpi), 0.0);

    // FootnoteArea가 실제 단 폭/시작점을 갖기 전에는 상대 sentinel을 넓히지 않는다.
    assert!((footnote_separator_length_px(-3, width, dpi) - 200.0).abs() < 1e-9);
    assert!((footnote_separator_length_px(-4, width, dpi) - 200.0).abs() < 1e-9);
}

#[test]
fn endnote_separator_caller_uses_schema_sentinel_widths() {
    fn rendered_width(separator_length: i32) -> f64 {
        let engine = LayoutEngine::with_default_dpi();
        let mut tree = PageRenderTree::new(0, 800.0, 1_100.0);
        let area = LayoutRect {
            x: 100.0,
            y: 100.0,
            width: 600.0,
            height: 900.0,
        };
        let mut column = RenderNode::new(
            tree.next_id(),
            RenderNodeType::Column(0),
            BoundingBox::new(area.x, area.y, area.width, area.height),
        );
        engine.layout_endnote_separator_item(
            &mut tree,
            &mut column,
            &area,
            area.y,
            separator_length,
            0,
            0,
            1,
            1,
            0,
        );
        let line = column.children.first().expect("endnote separator line");
        match &line.node_type {
            RenderNodeType::Line(line) => (line.x2 - line.x1).abs(),
            other => panic!("expected separator Line, got {other:?}"),
        }
    }

    assert!((rendered_width(-2) - 96.0 * 2.0 / 2.54).abs() < 1e-6);
    assert!((rendered_width(-4) - 600.0).abs() < 1e-9);
}

#[test]
fn footnote_separator_caller_uses_absolute_hwpunit_width() {
    let engine = LayoutEngine::with_default_dpi();
    let mut tree = PageRenderTree::new(0, 800.0, 1_100.0);
    let area = LayoutRect {
        x: 100.0,
        y: 900.0,
        width: 600.0,
        height: 100.0,
    };
    let mut footnote_area = RenderNode::new(
        tree.next_id(),
        RenderNodeType::FootnoteArea,
        BoundingBox::new(area.x, area.y, area.width, area.height),
    );
    let paragraphs = vec![Paragraph {
        controls: vec![Control::Footnote(Box::new(
            crate::model::footnote::Footnote {
                number: 1,
                ..Default::default()
            },
        ))],
        ..Default::default()
    }];
    let footnotes = vec![FootnoteRef {
        number: 1,
        source: FootnoteSource::Body {
            para_index: 0,
            control_index: 0,
        },
        fragment: None,
    }];
    let shape = FootnoteShape {
        separator_length: 7_200,
        separator_line_type: 1,
        separator_line_width: 1,
        ..Default::default()
    };

    engine.layout_footnote_area(
        &mut tree,
        &mut footnote_area,
        &footnotes,
        &paragraphs,
        &ResolvedStyleSet::default(),
        &area,
        &shape,
    );

    let line = footnote_area
        .children
        .iter()
        .find(|child| matches!(child.node_type, RenderNodeType::Line(_)))
        .expect("footnote separator line");
    assert!((line.bbox.width - 96.0).abs() < 1e-9);
}

/// Task #3216: AutoNumber(Page)와 명시 쪽번호 필드가 같은 문단에 있어도 각각은
/// 모델 한 글자를 유지하고 표시값만 확장해야 한다.
#[test]
fn issue3216_page_auto_number_does_not_expand_manual_page_field_model_text() {
    use crate::model::control::{AutoNumber, AutoNumberType};

    let para = Paragraph {
        // 앞 U+0015는 Studio에서 삽입한 명시 쪽번호 필드, 뒤 U+0015는 HWPX
        // AutoNumber(Page) placeholder다. char_offsets의 8-unit gap이 컨트롤 위치를
        // 뒤 placeholder로 고정한다.
        text: "\u{0015}\u{0015}".to_string(),
        char_offsets: vec![0, 9],
        char_count: 10,
        controls: vec![Control::AutoNumber(AutoNumber {
            number_type: AutoNumberType::Page,
            ..Default::default()
        })],
        ..Default::default()
    };
    let engine = LayoutEngine::with_default_dpi();
    let mut composed = compose_paragraph(&para);

    engine.substitute_hf_field_markers(&mut composed, 12);
    engine.substitute_page_auto_numbers_in_composed(&para, &mut composed, 12);

    let runs: Vec<(String, Option<String>)> = composed
        .lines
        .iter()
        .flat_map(|line| line.runs.iter())
        .map(|run| (run.text.clone(), run.display_text.clone()))
        .collect();
    assert_eq!(
        runs,
        vec![
            ("\u{0015}".to_string(), Some("12".to_string())),
            ("\u{0015}".to_string(), Some("12".to_string())),
        ],
        "명시 필드와 AutoNumber 모두 raw text는 모델 marker 한 글자여야 한다"
    );
}

fn issue2817_textless_picture_host(vert_rel_to: VertRelTo, text_wrap: TextWrap) -> Paragraph {
    let mut picture = crate::model::image::Picture::default();
    picture.common.treat_as_char = false;
    picture.common.vert_rel_to = vert_rel_to;
    picture.common.text_wrap = text_wrap;
    Paragraph {
        controls: vec![Control::Picture(Box::new(picture))],
        ..Default::default()
    }
}

#[test]
fn issue2817_paper_anchor_infront_picture_host_reserves_line_advance() {
    let para = issue2817_textless_picture_host(VertRelTo::Paper, TextWrap::InFrontOfText);
    assert!(textless_infront_para_host_requires_line_advance(&para));
}

#[test]
fn issue2817_paper_anchor_behind_picture_host_keeps_no_line_advance() {
    let para = issue2817_textless_picture_host(VertRelTo::Paper, TextWrap::BehindText);
    assert!(!textless_infront_para_host_requires_line_advance(&para));
}

#[test]
fn issue2439_fragment_margin_evidence_is_narrow_and_structural() {
    let anchor = Paragraph {
        line_segs: vec![LineSeg {
            line_height: 1200,
            line_spacing: 240,
            ..Default::default()
        }],
        controls: vec![Control::Table(Box::new(Table {
            page_break: TablePageBreak::RowBreak,
            common: CommonObjAttr {
                treat_as_char: false,
                text_wrap: TextWrap::TopAndBottom,
                vert_rel_to: VertRelTo::Para,
                vertical_offset: 399,
                ..Default::default()
            },
            ..Default::default()
        }))],
        ..Default::default()
    };
    // [#2808] 접힌 ladder 증거(next.vpos = host.vpos + host 줄 advance)가 있어야
    // native 재현 형상으로 인정된다 — typeset::tests::issue2439_native_empty_host_
    // rowbreak_evidence_is_narrow 의 스탬프와 동일.
    let signature = Paragraph {
        text: "signature".to_string(),
        line_segs: vec![LineSeg {
            line_height: 1000,
            vertical_pos: 1440,
            ..Default::default()
        }],
        ..Default::default()
    };
    let paragraphs = vec![anchor.clone(), signature];

    assert!(repeats_native_empty_host_rowbreak_fragment_margin(
        true,
        &paragraphs,
        0,
        0,
    ));
    assert!(!repeats_native_empty_host_rowbreak_fragment_margin(
        false,
        &paragraphs,
        0,
        0,
    ));

    let no_plain_tail = vec![anchor, Paragraph::new_empty()];
    assert!(!repeats_native_empty_host_rowbreak_fragment_margin(
        true,
        &no_plain_tail,
        0,
        0,
    ));
}

#[test]
fn issue2439_full_table_top_matches_first_partial_fragment_top() {
    let para_y = 100.0;
    let vertical_offset = 5.32;
    let outer_top = 3.77;

    let full_table_top = empty_host_float_raw_top(para_y, vertical_offset, outer_top);
    let first_partial_fragment_top = para_y + outer_top + vertical_offset;
    assert!((full_table_top - first_partial_fragment_top).abs() < 1e-9);

    // The generic empty-host float contract remains unchanged when the strict structural
    // evidence is absent, and negative offsets remain clamped at the host paragraph top.
    assert_eq!(empty_host_float_raw_top(para_y, -8.0, 0.0), para_y);
}

#[test]
fn native_multiline_visible_float_uses_host_end_only_for_short_offset() {
    let mut table = Table {
        common: CommonObjAttr {
            treat_as_char: false,
            text_wrap: TextWrap::TopAndBottom,
            vert_rel_to: VertRelTo::Para,
            vertical_offset: 4_000,
            flow_with_text: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let para = Paragraph {
        text: "표 앞 본문 첫째 줄\n둘째 줄\n셋째 줄".to_string(),
        line_segs: vec![
            LineSeg {
                vertical_pos: 0,
                line_height: 1_000,
                ..Default::default()
            },
            LineSeg {
                vertical_pos: 2_000,
                line_height: 1_000,
                ..Default::default()
            },
            LineSeg {
                vertical_pos: 4_000,
                line_height: 1_000,
                ..Default::default()
            },
        ],
        controls: vec![Control::Table(Box::new(table.clone()))],
        ..Default::default()
    };

    assert!(native_multiline_visible_float_table_top(true, &para, &table, 100.0, 96.0).is_some());

    // p9 같은 3줄 host에서 offset(5,317 HU)이 마지막 줄의 bottom(5,000 HU)
    // 이후를 이미 가리키면, host 높이를 다시 가산하지 않아야 한다.
    table.common.vertical_offset = 5_317;
    assert!(native_multiline_visible_float_table_top(true, &para, &table, 100.0, 96.0).is_none());
}

#[test]
fn stored_layout_relocated_picture_caption_uses_next_saved_flow_anchor() {
    let picture = Picture {
        common: CommonObjAttr {
            height: 24_791,
            treat_as_char: false,
            text_wrap: TextWrap::TopAndBottom,
            vert_rel_to: VertRelTo::Para,
            vertical_offset: (-52_790_i32) as u32,
            flow_with_text: true,
            ..Default::default()
        },
        caption: Some(Caption {
            direction: CaptionDirection::Bottom,
            paragraphs: vec![Paragraph::default()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let table = Table {
        row_count: 1,
        col_count: 1,
        page_break: TablePageBreak::RowBreak,
        common: CommonObjAttr {
            height: 18_257,
            treat_as_char: false,
            text_wrap: TextWrap::TopAndBottom,
            vert_rel_to: VertRelTo::Para,
            vertical_offset: 560,
            ..Default::default()
        },
        cells: vec![Cell {
            row: 0,
            col: 0,
            row_span: 1,
            col_span: 1,
            height: 36_782,
            paragraphs: vec![Paragraph {
                line_segs: vec![LineSeg {
                    vertical_pos: 0,
                    ..Default::default()
                }],
                controls: vec![Control::Picture(Box::new(picture))],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let host = Paragraph {
        line_segs: vec![LineSeg {
            vertical_pos: 52_230,
            ..Default::default()
        }],
        controls: vec![Control::Table(Box::new(table.clone()))],
        ..Default::default()
    };
    let next = Paragraph {
        line_segs: vec![LineSeg {
            vertical_pos: 30_689,
            ..Default::default()
        }],
        ..Default::default()
    };
    let col = LayoutRect {
        x: 0.0,
        y: 79.2,
        width: 600.0,
        height: 900.0,
    };

    let flow_top = stored_layout_relocated_empty_rowbreak_picture_next_flow_top(
        true,
        &host,
        &table,
        Some(&next),
        &col,
        96.0,
    )
    .expect("이월 그림의 다음 저장 anchor");
    assert!(
        (flow_top - 488.386_666_7).abs() < 0.001,
        "flow_top={flow_top}"
    );
    assert!(
        stored_layout_relocated_empty_rowbreak_picture_next_flow_top(
            false,
            &host,
            &table,
            Some(&next),
            &col,
            96.0,
        )
        .is_none()
    );
}

#[test]
fn oversized_row_width_outlier_does_not_expand_base_columns() {
    let mut cells = Vec::new();
    for row in 0..3 {
        for col in 0..2 {
            cells.push(Cell {
                row,
                col,
                row_span: 1,
                col_span: 1,
                width: if row == 0 && col == 0 { 150 } else { 100 },
                ..Default::default()
            });
        }
    }
    let table = Table {
        row_count: 3,
        col_count: 2,
        cells,
        common: CommonObjAttr {
            width: 200,
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = LayoutEngine::with_default_dpi();
    let widths = engine.resolve_column_widths(&table, 2);
    let expected = hwpunit_to_px(100, DEFAULT_DPI);

    assert!((widths[0] - expected).abs() < 0.01, "widths={widths:?}");
    assert!((widths[1] - expected).abs() < 0.01, "widths={widths:?}");
}

/// [열 폭 오버슈트] 서로 다른 행이 같은 9열 구간을 서로 다르게 병합해 나누는
/// authoring(각 행 자신은 유효함, hwp 포맷에서 흔한 패턴) — 예: 한 행은
/// `[cols 0-3][cols 4-6][cols 7-8]`로, 다른 행은
/// `[col 0][cols 1-2][cols 3-4][cols 5-8]`로 같은 9열을 나눠 병합. 2단계의 병합
/// 셀 제약 해석이 span 오름차순으로 하나씩만 풀다 보니(전체 연립방정식을 풀지
/// 않음), 좁은 span 제약이 먼저 열들을 다 채워 넓은 span 제약이 무시되고
/// `col_widths` 합이 표 선언 폭(48000)을 크게 초과했었다(실측 스캐너: 56400,
/// ×1.175 — hwpx-template-engine 레포의 `scslic2.hwpx` `#HEADER` 표에서 그대로
/// 재현된 실제 버그). 이제는 초과분을 모든 열에 비례 축소해 흡수해 합이 다시
/// 선언 폭과 일치해야 한다.
#[test]
fn overlapping_row_colspan_partitions_do_not_overshoot_declared_table_width() {
    let mut cells = Vec::new();
    // row 0: [cols 0-3]=7500 | [cols 4-6]=33000 | [cols 7-8]=7500
    cells.push(Cell {
        row: 0,
        col: 0,
        row_span: 1,
        col_span: 4,
        width: 7500,
        ..Default::default()
    });
    cells.push(Cell {
        row: 0,
        col: 4,
        row_span: 1,
        col_span: 3,
        width: 33000,
        ..Default::default()
    });
    cells.push(Cell {
        row: 0,
        col: 7,
        row_span: 1,
        col_span: 2,
        width: 7500,
        ..Default::default()
    });
    // row 1: [col 0]=2000 | [cols 1-2]=3800 | [cols 3-4]=20200 | [cols 5-8]=22000
    cells.push(Cell {
        row: 1,
        col: 0,
        row_span: 1,
        col_span: 1,
        width: 2000,
        ..Default::default()
    });
    cells.push(Cell {
        row: 1,
        col: 1,
        row_span: 1,
        col_span: 2,
        width: 3800,
        ..Default::default()
    });
    cells.push(Cell {
        row: 1,
        col: 3,
        row_span: 1,
        col_span: 2,
        width: 20200,
        ..Default::default()
    });
    cells.push(Cell {
        row: 1,
        col: 5,
        row_span: 1,
        col_span: 4,
        width: 22000,
        ..Default::default()
    });
    let table = Table {
        row_count: 2,
        col_count: 9,
        cells,
        common: CommonObjAttr {
            width: 48000,
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = LayoutEngine::with_default_dpi();
    let widths = engine.resolve_column_widths(&table, 9);
    let expected_total = hwpunit_to_px(48000, DEFAULT_DPI);
    let actual_total: f64 = widths.iter().sum();

    assert!(
        (actual_total - expected_total).abs() < 0.5,
        "widths={widths:?} actual_total={actual_total} expected_total={expected_total}"
    );

    // 총합만이 아니라 개별 열 폭도 유일해와 정확히 일치해야 한다 — 이 제약
    // 집합은 실제로 모호하지 않다(col0=2000, col1+col2=3800 이므로
    // col3=7500-2000-3800=1700, col4=20200-1700=18500 이 유일해로 도출됨).
    // col1/col2, col5/col6, col7/col8 은 개별 분할이 정말 미결정이므로 합만
    // 검증한다.
    let px = |hwpunit: i32| hwpunit_to_px(hwpunit, DEFAULT_DPI);
    let close = |a: f64, b: f64| (a - b).abs() < 0.5;
    assert!(
        close(widths[0], px(2000)),
        "col0 widths={widths:?} expected={}",
        px(2000)
    );
    assert!(
        close(widths[1] + widths[2], px(3800)),
        "col1+col2 widths={widths:?} expected={}",
        px(3800)
    );
    assert!(
        close(widths[3], px(1700)),
        "col3 widths={widths:?} expected={}",
        px(1700)
    );
    assert!(
        close(widths[4], px(18500)),
        "col4 widths={widths:?} expected={}",
        px(18500)
    );
    assert!(
        close(widths[5] + widths[6], px(14500)),
        "col5+col6 widths={widths:?} expected={}",
        px(14500)
    );
    assert!(
        close(widths[7] + widths[8], px(7500)),
        "col7+col8 widths={widths:?} expected={}",
        px(7500)
    );
}

#[test]
fn conflicting_row_colspan_partitions_do_not_panic() {
    // 두 행이 같은 열 구간을 진짜로 상충되게 선언하는 malformed 입력(단순
    // 분할 차이가 아니라 실제 불일치): row0 은 [cols 0-1]=1000, row1 은
    // 같은 [cols 0-1]=4000 이라고 선언한다. 고정점 반복이 패닉하거나
    // 무한루프에 빠지지 않고, 유한하며 표 선언 폭과 합이 맞는 결과를
    // 내야 한다(개별 열 값의 "정답"은 malformed 입력이므로 존재하지 않음).
    let mut cells = Vec::new();
    cells.push(Cell {
        row: 0,
        col: 0,
        row_span: 1,
        col_span: 2,
        width: 1000,
        ..Default::default()
    });
    cells.push(Cell {
        row: 0,
        col: 2,
        row_span: 1,
        col_span: 1,
        width: 2000,
        ..Default::default()
    });
    cells.push(Cell {
        row: 1,
        col: 0,
        row_span: 1,
        col_span: 2,
        width: 4000,
        ..Default::default()
    });
    cells.push(Cell {
        row: 1,
        col: 2,
        row_span: 1,
        col_span: 1,
        width: 2000,
        ..Default::default()
    });
    let table = Table {
        row_count: 2,
        col_count: 3,
        cells,
        common: CommonObjAttr {
            width: 3000,
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = LayoutEngine::with_default_dpi();
    let widths = engine.resolve_column_widths(&table, 3);

    assert_eq!(widths.len(), 3);
    assert!(widths.iter().all(|w| w.is_finite() && *w >= 0.0));
    let expected_total = hwpunit_to_px(3000, DEFAULT_DPI);
    let actual_total: f64 = widths.iter().sum();
    assert!(
        (actual_total - expected_total).abs() < 0.5,
        "widths={widths:?} actual_total={actual_total} expected_total={expected_total}"
    );
}

#[test]
fn compact_endnote_tail_log_tolerance_allows_line_box_bleed_only() {
    let col_bottom = 1092.3;

    assert!(is_tolerated_endnote_column_bottom_bleed(
        true,
        col_bottom + 43.3,
        col_bottom
    ));
    assert!(!is_tolerated_endnote_column_bottom_bleed(
        true,
        col_bottom + 49.0,
        col_bottom
    ));
    assert!(is_tolerated_endnote_column_bottom_bleed_with_limit(
        true,
        col_bottom + 64.0,
        col_bottom,
        ENDNOTE_EQUATION_TAIL_LINE_BOX_OVERFLOW_LOG_TOLERANCE_PX,
    ));
    assert!(!is_tolerated_endnote_column_bottom_bleed_with_limit(
        true,
        col_bottom + 69.0,
        col_bottom,
        ENDNOTE_EQUATION_TAIL_LINE_BOX_OVERFLOW_LOG_TOLERANCE_PX,
    ));
    assert!(!is_tolerated_endnote_column_bottom_bleed(
        false,
        col_bottom + 1.0,
        col_bottom
    ));
}

#[test]
fn test_build_page_with_paragraph() {
    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());

    let paragraphs = vec![Paragraph {
        text: "안녕하세요".to_string(),
        line_segs: vec![LineSeg {
            line_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }],
        ..Default::default()
    }];

    let composed: Vec<_> = paragraphs.iter().map(|p| compose_paragraph(p)).collect();
    let styles = ResolvedStyleSet::default();

    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: vec![ColumnContent {
            column_index: 0,
            start_height: 0.0,
            endnote_flow: false,
            items: vec![PageItem::FullParagraph { para_index: 0 }],
            zone_layout: None,
            zone_y_offset: 0.0,
            wrap_around_paras: Vec::new(),
            used_height: 0.0,
            wrap_anchors: std::collections::HashMap::new(),
        }],
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };

    let tree = engine.build_render_tree(
        &page_content,
        &paragraphs,
        &paragraphs,
        &paragraphs,
        &composed,
        &styles,
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    );

    // Body 노드 찾기
    let body = tree
        .root
        .children
        .iter()
        .find(|n| matches!(n.node_type, RenderNodeType::Body { .. }));
    assert!(body.is_some());
    let body = body.unwrap();
    // Column 노드가 있어야 함
    assert!(!body.children.is_empty());
}

/// [Issue #1945] PartialParagraph 의 start_line 이 조판 라인 수를 넘어도
/// 패닉하지 않아야 한다 (실문서 크래시 — paragraph_layout.rs 슬라이스 범위 밖).
/// 수정 전에는 `composed.lines[start_line..end]` 직접 인덱싱이
/// "range start index N out of range" 로 패닉했다.
#[test]
fn partial_paragraph_start_line_beyond_lines_does_not_panic() {
    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());

    // 조판 라인 1개짜리 문단.
    let paragraphs = vec![Paragraph {
        text: "한 줄".to_string(),
        line_segs: vec![LineSeg {
            line_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }],
        ..Default::default()
    }];
    let composed: Vec<_> = paragraphs.iter().map(|p| compose_paragraph(p)).collect();
    let styles = ResolvedStyleSet::default();

    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: vec![ColumnContent {
            column_index: 0,
            start_height: 0.0,
            endnote_flow: false,
            // start_line(5) > 조판 라인 수(1) — 이월 오버슛 재현.
            items: vec![PageItem::PartialParagraph {
                para_index: 0,
                start_line: 5,
                end_line: 6,
            }],
            zone_layout: None,
            zone_y_offset: 0.0,
            wrap_around_paras: Vec::new(),
            used_height: 0.0,
            wrap_anchors: std::collections::HashMap::new(),
        }],
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };

    // 패닉 없이 반환하면 성공 (범위 밖 조각은 빈 렌더).
    let _tree = engine.build_render_tree(
        &page_content,
        &paragraphs,
        &paragraphs,
        &paragraphs,
        &composed,
        &styles,
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    );
}

#[test]
fn test_layout_with_composed_styles() {
    use crate::renderer::style_resolver::ResolvedCharStyle;

    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());

    let paragraphs = vec![Paragraph {
        text: "AAABBB".to_string(),
        char_offsets: vec![0, 1, 2, 3, 4, 5],
        char_count: 7,
        char_shapes: vec![
            CharShapeRef {
                start_pos: 0,
                char_shape_id: 0,
            },
            CharShapeRef {
                start_pos: 3,
                char_shape_id: 1,
            },
        ],
        line_segs: vec![LineSeg {
            line_height: 800,
            baseline_distance: 640,
            ..Default::default()
        }],
        ..Default::default()
    }];

    let composed: Vec<_> = paragraphs.iter().map(|p| compose_paragraph(p)).collect();

    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        char_styles: vec![
            ResolvedCharStyle {
                font_family: "함초롬돋움".to_string(),
                font_size: 16.0,
                bold: true,
                ..Default::default()
            },
            ResolvedCharStyle {
                font_family: "함초롬바탕".to_string(),
                font_size: 12.0,
                italic: true,
                text_color: 0x00FF0000,
                ..Default::default()
            },
        ],
        para_styles: Vec::new(),
        border_styles: Vec::new(),
        numberings: Vec::new(),
        bullets: Vec::new(),
    };

    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: vec![ColumnContent {
            column_index: 0,
            start_height: 0.0,
            endnote_flow: false,
            items: vec![PageItem::FullParagraph { para_index: 0 }],
            zone_layout: None,
            zone_y_offset: 0.0,
            wrap_around_paras: Vec::new(),
            used_height: 0.0,
            wrap_anchors: std::collections::HashMap::new(),
        }],
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };

    let tree = engine.build_render_tree(
        &page_content,
        &paragraphs,
        &paragraphs,
        &paragraphs,
        &composed,
        &styles,
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    );

    // Body > Column > TextLine 찾기
    let body = tree
        .root
        .children
        .iter()
        .find(|n| matches!(n.node_type, RenderNodeType::Body { .. }))
        .unwrap();
    let col = &body.children[0];
    let line = &col.children[0];

    // TextLine 내에 2개의 TextRun이 있어야 함
    assert_eq!(line.children.len(), 2);

    // 첫 번째 TextRun: "AAA", bold, 함초롬돋움
    match &line.children[0].node_type {
        RenderNodeType::TextRun(run) => {
            assert_eq!(run.text, "AAA");
            assert_eq!(run.style.font_family, "함초롬돋움");
            assert!(run.style.bold);
            assert!(!run.style.italic);
            assert!((run.style.font_size - 16.0).abs() < 0.01);
        }
        _ => panic!("Expected TextRun"),
    }

    // 두 번째 TextRun: "BBB", italic, 함초롬바탕
    match &line.children[1].node_type {
        RenderNodeType::TextRun(run) => {
            assert_eq!(run.text, "BBB");
            assert_eq!(run.style.font_family, "함초롬바탕");
            assert!(!run.style.bold);
            assert!(run.style.italic);
            assert_eq!(run.style.color, 0x00FF0000);
        }
        _ => panic!("Expected TextRun"),
    }
}

#[test]
fn test_layout_multi_run_x_position() {
    use crate::renderer::style_resolver::ResolvedCharStyle;

    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());

    let paragraphs = vec![Paragraph {
        text: "AB가나".to_string(),
        char_offsets: vec![0, 1, 2, 3],
        char_count: 5,
        char_shapes: vec![
            CharShapeRef {
                start_pos: 0,
                char_shape_id: 0,
            },
            CharShapeRef {
                start_pos: 2,
                char_shape_id: 1,
            },
        ],
        line_segs: vec![LineSeg {
            line_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }],
        ..Default::default()
    }];

    let composed: Vec<_> = paragraphs.iter().map(|p| compose_paragraph(p)).collect();
    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        char_styles: vec![
            ResolvedCharStyle {
                font_size: 16.0,
                ..Default::default()
            },
            ResolvedCharStyle {
                font_size: 16.0,
                ..Default::default()
            },
        ],
        para_styles: Vec::new(),
        border_styles: Vec::new(),
        numberings: Vec::new(),
        bullets: Vec::new(),
    };

    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: vec![ColumnContent {
            column_index: 0,
            start_height: 0.0,
            endnote_flow: false,
            items: vec![PageItem::FullParagraph { para_index: 0 }],
            zone_layout: None,
            zone_y_offset: 0.0,
            wrap_around_paras: Vec::new(),
            used_height: 0.0,
            wrap_anchors: std::collections::HashMap::new(),
        }],
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };

    let tree = engine.build_render_tree(
        &page_content,
        &paragraphs,
        &paragraphs,
        &paragraphs,
        &composed,
        &styles,
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    );

    let body = tree
        .root
        .children
        .iter()
        .find(|n| matches!(n.node_type, RenderNodeType::Body { .. }))
        .unwrap();
    let col = &body.children[0];
    let line = &col.children[0];

    assert_eq!(line.children.len(), 2);

    // 두 번째 TextRun의 x 좌표가 첫 번째 TextRun 끝 이후여야 함
    let run1_x = line.children[0].bbox.x;
    let run1_w = line.children[0].bbox.width;
    let run2_x = line.children[1].bbox.x;
    assert!((run2_x - (run1_x + run1_w)).abs() < 0.01);
}

#[test]
fn test_resolved_to_text_style() {
    use crate::model::style::UnderlineType;
    use crate::renderer::style_resolver::ResolvedCharStyle;

    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        char_styles: vec![ResolvedCharStyle {
            font_family: "나눔고딕".to_string(),
            font_size: 14.0,
            bold: true,
            italic: false,
            text_color: 0x000000FF,
            underline: UnderlineType::Bottom,
            letter_spacing: 1.5,
            ..Default::default()
        }],
        para_styles: Vec::new(),
        border_styles: Vec::new(),
        numberings: Vec::new(),
        bullets: Vec::new(),
    };

    let ts = resolved_to_text_style(&styles, 0, 0);
    assert_eq!(ts.font_family, "나눔고딕");
    assert!((ts.font_size - 14.0).abs() < 0.01);
    assert!(ts.bold);
    assert!(!ts.italic);
    assert!(matches!(ts.underline, UnderlineType::Bottom));
    assert_eq!(ts.color, 0x000000FF);
    assert!((ts.letter_spacing - 1.5).abs() < 0.01);
    assert!((ts.ratio - 1.0).abs() < 0.01); // 기본 장평 100%
}

#[test]
fn test_resolved_to_text_style_with_ratio() {
    use crate::renderer::style_resolver::ResolvedCharStyle;

    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        char_styles: vec![ResolvedCharStyle {
            font_family: "함초롬돋움".to_string(),
            font_size: 16.0,
            ratio: 0.8,
            ..Default::default()
        }],
        para_styles: Vec::new(),
        border_styles: Vec::new(),
        numberings: Vec::new(),
        bullets: Vec::new(),
    };

    let ts = resolved_to_text_style(&styles, 0, 0);
    assert!((ts.ratio - 0.8).abs() < 0.01);
}

#[test]
fn test_resolved_to_text_style_missing_id() {
    let styles = ResolvedStyleSet::default();
    let ts = resolved_to_text_style(&styles, 999, 0);
    assert!(ts.font_family.is_empty());
    assert!((ts.font_size - 0.0).abs() < 0.01);
    assert!((ts.ratio - 1.0).abs() < 0.01); // 기본값 1.0
}

#[test]
fn test_estimate_text_width() {
    let style = TextStyle {
        font_size: 16.0,
        ..Default::default()
    };

    // Latin characters: 0.5 * font_size each
    let w = estimate_text_width("AB", &style);
    assert!((w - 16.0).abs() < 0.01); // 2 * 8.0

    // CJK characters: 1.0 * font_size each
    let w = estimate_text_width("가나", &style);
    assert!((w - 32.0).abs() < 0.01); // 2 * 16.0

    // Mixed
    let w = estimate_text_width("A가", &style);
    assert!((w - 24.0).abs() < 0.01); // 8.0 + 16.0
}

#[test]
fn test_estimate_text_width_with_ratio() {
    // 장평 80%: 기본 폭의 80%
    let style = TextStyle {
        font_size: 16.0,
        ratio: 0.8,
        ..Default::default()
    };
    let w = estimate_text_width("가나", &style);
    // base: 2 * 16.0 = 32.0, * 0.8 = 25.6 → round = 26.0
    assert!((w - 26.0).abs() < 0.01);

    // 장평 150%
    let style = TextStyle {
        font_size: 16.0,
        ratio: 1.5,
        ..Default::default()
    };
    let w = estimate_text_width("AB", &style);
    // base: 2 * 8.0 = 16.0, * 1.5 = 24.0
    assert!((w - 24.0).abs() < 0.01);

    // 장평 100%: 기존과 동일
    let style = TextStyle {
        font_size: 16.0,
        ratio: 1.0,
        ..Default::default()
    };
    let w = estimate_text_width("가나", &style);
    assert!((w - 32.0).abs() < 0.01);
}

#[test]
fn test_compute_char_positions_extra_word_spacing() {
    // extra_word_spacing은 공백 문자에만 추가 간격 적용
    let style = TextStyle {
        font_size: 16.0,
        extra_word_spacing: 10.0,
        ..Default::default()
    };
    let positions = compute_char_positions("A B", &style);
    // A: 8.0, ' ': 8.0 + 10.0 = 18.0, B: 8.0
    assert_eq!(positions.len(), 4); // 3문자 + 1
    assert!((positions[0] - 0.0).abs() < 0.01);
    assert!((positions[1] - 8.0).abs() < 0.01); // A
    assert!((positions[2] - 26.0).abs() < 0.01); // A + space(8+10)
    assert!((positions[3] - 34.0).abs() < 0.01); // A + space + B
}

#[test]
fn test_compute_char_positions_extra_char_spacing() {
    // extra_char_spacing은 모든 문자에 추가 간격 적용
    let style = TextStyle {
        font_size: 16.0,
        extra_char_spacing: 5.0,
        ..Default::default()
    };
    let positions = compute_char_positions("AB", &style);
    // A: 8.0 + 5.0 = 13.0, B: 8.0 + 5.0 = 13.0
    assert_eq!(positions.len(), 3);
    assert!((positions[0] - 0.0).abs() < 0.01);
    assert!((positions[1] - 13.0).abs() < 0.01);
    assert!((positions[2] - 26.0).abs() < 0.01);
}

#[test]
fn test_estimate_text_width_with_extra_spacing() {
    // extra_word_spacing + extra_char_spacing 동시 적용
    let style = TextStyle {
        font_size: 16.0,
        extra_word_spacing: 10.0,
        extra_char_spacing: 2.0,
        ..Default::default()
    };
    // "A B": A(8+2) + space(8+2+10) + B(8+2) = 10 + 20 + 10 = 40
    let w = estimate_text_width("A B", &style);
    assert!((w - 40.0).abs() < 0.01);
}

#[test]
fn test_extra_spacing_zero_default() {
    // 기본값(0.0)에서는 기존 동작과 동일
    let style = TextStyle {
        font_size: 16.0,
        ..Default::default()
    };
    let w_no_extra = estimate_text_width("가나다", &style);
    let positions_no_extra = compute_char_positions("가나다", &style);

    let style_explicit = TextStyle {
        font_size: 16.0,
        extra_word_spacing: 0.0,
        extra_char_spacing: 0.0,
        ..Default::default()
    };
    let w_explicit = estimate_text_width("가나다", &style_explicit);
    let positions_explicit = compute_char_positions("가나다", &style_explicit);

    assert!((w_no_extra - w_explicit).abs() < 0.01);
    for (a, b) in positions_no_extra.iter().zip(positions_explicit.iter()) {
        assert!((a - b).abs() < 0.01);
    }
}

#[test]
fn test_extra_word_spacing_no_effect_on_non_space() {
    // 공백 없는 텍스트에서 extra_word_spacing은 영향 없음
    let style_base = TextStyle {
        font_size: 16.0,
        ..Default::default()
    };
    let style_extra = TextStyle {
        font_size: 16.0,
        extra_word_spacing: 100.0,
        ..Default::default()
    };
    let w_base = estimate_text_width("가나다", &style_base);
    let w_extra = estimate_text_width("가나다", &style_extra);
    assert!((w_base - w_extra).abs() < 0.01);
}

#[test]
fn test_tab_not_affected_by_extra_spacing() {
    // 탭 문자는 extra_char_spacing/extra_word_spacing에 영향받지 않음
    let style = TextStyle {
        font_size: 16.0,
        extra_char_spacing: 100.0,
        extra_word_spacing: 100.0,
        ..Default::default()
    };
    let positions = compute_char_positions("\t", &style);
    assert_eq!(positions.len(), 2);
    // 탭은 tab_w로 스냅 (font_size * 4 = 64)
    assert!((positions[1] - 64.0).abs() < 0.01);
}

#[test]
fn test_layout_table_basic() {
    use crate::model::control::Control;
    use crate::model::table::{Cell, Table};
    use crate::renderer::style_resolver::ResolvedBorderStyle;

    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());

    // 2x2 표가 있는 문단 (각 셀에 border_fill_id=1 설정)
    let table = Table {
        row_count: 2,
        col_count: 2,
        row_sizes: vec![2, 2], // 행별 셀 수
        cells: vec![
            Cell {
                col: 0,
                row: 0,
                col_span: 1,
                row_span: 1,
                width: 3000,
                height: 1200,
                border_fill_id: 1,
                paragraphs: vec![Paragraph {
                    text: "A".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            Cell {
                col: 1,
                row: 0,
                col_span: 1,
                row_span: 1,
                width: 3000,
                height: 1200,
                border_fill_id: 1,
                paragraphs: vec![Paragraph {
                    text: "B".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            Cell {
                col: 0,
                row: 1,
                col_span: 1,
                row_span: 1,
                width: 3000,
                height: 1200,
                border_fill_id: 1,
                paragraphs: vec![Paragraph {
                    text: "C".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            Cell {
                col: 1,
                row: 1,
                col_span: 1,
                row_span: 1,
                width: 3000,
                height: 1200,
                border_fill_id: 1,
                paragraphs: vec![Paragraph {
                    text: "D".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let paragraphs = vec![Paragraph {
        text: String::new(),
        controls: vec![Control::Table(Box::new(table))],
        line_segs: vec![LineSeg {
            line_height: 400,
            ..Default::default()
        }],
        ..Default::default()
    }];

    let composed: Vec<_> = paragraphs.iter().map(|p| compose_paragraph(p)).collect();
    // border_fill_id=1은 styles.border_styles[0]을 참조 (1-indexed)
    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        border_styles: vec![ResolvedBorderStyle::default()],
        ..Default::default()
    };

    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: vec![ColumnContent {
            column_index: 0,
            start_height: 0.0,
            endnote_flow: false,
            items: vec![
                PageItem::FullParagraph { para_index: 0 },
                PageItem::Table {
                    para_index: 0,
                    control_index: 0,
                },
            ],
            zone_layout: None,
            zone_y_offset: 0.0,
            wrap_around_paras: Vec::new(),
            used_height: 0.0,
            wrap_anchors: std::collections::HashMap::new(),
        }],
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };

    let tree = engine.build_render_tree(
        &page_content,
        &paragraphs,
        &paragraphs,
        &paragraphs,
        &composed,
        &styles,
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    );

    // Body > Column 내에 Table 노드가 있어야 함
    let body = tree
        .root
        .children
        .iter()
        .find(|n| matches!(n.node_type, RenderNodeType::Body { .. }))
        .unwrap();
    let col = &body.children[0];

    let table_node = col
        .children
        .iter()
        .find(|n| matches!(n.node_type, RenderNodeType::Table(_)))
        .expect("Table node should exist");

    // 4개 셀 + 엣지 기반 테두리 Line 노드들
    let cell_count = table_node
        .children
        .iter()
        .filter(|c| matches!(c.node_type, RenderNodeType::TableCell(_)))
        .count();
    assert_eq!(cell_count, 4);

    // 엣지 기반 테두리: 표 노드의 직접 자식으로 Line 노드가 있어야 함
    // 2x2 표: 수평 3줄 + 수직 3줄 = 6개 이상의 Line 노드
    // (기본 Solid 테두리이므로 이중선/삼중선이 아니면 각 엣지당 1개)
    let table_line_count = table_node
        .children
        .iter()
        .filter(|c| matches!(c.node_type, RenderNodeType::Line(_)))
        .count();
    assert!(
        table_line_count >= 6,
        "표에 6개 이상의 엣지 테두리가 있어야 함 (실제: {})",
        table_line_count
    );
}

#[test]
fn test_layout_table_cell_positions() {
    use crate::model::control::Control;
    use crate::model::table::{Cell, Table};

    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());

    let table = Table {
        row_count: 2,
        col_count: 2,
        row_sizes: vec![2, 2], // 행별 셀 수
        cells: vec![
            Cell {
                col: 0,
                row: 0,
                col_span: 1,
                row_span: 1,
                width: 3600,
                height: 720,
                ..Default::default()
            },
            Cell {
                col: 1,
                row: 0,
                col_span: 1,
                row_span: 1,
                width: 3600,
                height: 720,
                ..Default::default()
            },
            Cell {
                col: 0,
                row: 1,
                col_span: 1,
                row_span: 1,
                width: 3600,
                height: 720,
                ..Default::default()
            },
            Cell {
                col: 1,
                row: 1,
                col_span: 1,
                row_span: 1,
                width: 3600,
                height: 720,
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let paragraphs = vec![Paragraph {
        text: String::new(),
        controls: vec![Control::Table(Box::new(table))],
        line_segs: vec![LineSeg {
            line_height: 400,
            ..Default::default()
        }],
        ..Default::default()
    }];

    let composed: Vec<_> = paragraphs.iter().map(|p| compose_paragraph(p)).collect();
    let styles = ResolvedStyleSet::default();

    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: vec![ColumnContent {
            column_index: 0,
            start_height: 0.0,
            endnote_flow: false,
            items: vec![
                PageItem::FullParagraph { para_index: 0 },
                PageItem::Table {
                    para_index: 0,
                    control_index: 0,
                },
            ],
            zone_layout: None,
            zone_y_offset: 0.0,
            wrap_around_paras: Vec::new(),
            used_height: 0.0,
            wrap_anchors: std::collections::HashMap::new(),
        }],
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };

    let tree = engine.build_render_tree(
        &page_content,
        &paragraphs,
        &paragraphs,
        &paragraphs,
        &composed,
        &styles,
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    );

    let body = tree
        .root
        .children
        .iter()
        .find(|n| matches!(n.node_type, RenderNodeType::Body { .. }))
        .unwrap();
    let col = &body.children[0];
    let table_node = col
        .children
        .iter()
        .find(|n| matches!(n.node_type, RenderNodeType::Table(_)))
        .unwrap();

    // 셀 (1,0)의 x좌표는 셀 (0,0)의 x + width 이후
    let cell_00 = &table_node.children[0];
    let cell_10 = &table_node.children[1];
    let cell_01 = &table_node.children[2];

    // 3600 HWPUNIT @ 96dpi = 48.0 px
    let cell_width = 3600.0 * 96.0 / 7200.0;
    assert!((cell_10.bbox.x - cell_00.bbox.x - cell_width).abs() < 0.1);

    // 셀 (0,1)의 y좌표는 셀 (0,0)의 y + row_height 이후
    let row_height = 720.0 * 96.0 / 7200.0;
    assert!((cell_01.bbox.y - cell_00.bbox.y - row_height).abs() < 0.1);
}

#[test]
fn test_layout_rect_to_bbox() {
    let rect = LayoutRect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 200.0,
    };
    let bbox = layout_rect_to_bbox(&rect);
    assert!((bbox.x - 10.0).abs() < 0.01);
    assert!((bbox.width - 100.0).abs() < 0.01);
}

#[test]
fn test_numbering_state_advance() {
    let mut state = NumberingState::default();

    // 첫 번째 수준 0 → counter[0] = 1
    let c = state.advance(0, 0, None);
    assert_eq!(c[0], 1);

    // 수준 1 → counter[1] = 1
    let c = state.advance(0, 1, None);
    assert_eq!(c[0], 1);
    assert_eq!(c[1], 1);

    // 수준 1 반복 → counter[1] = 2
    let c = state.advance(0, 1, None);
    assert_eq!(c[1], 2);

    // 수준 0으로 복귀 → counter[0] = 2, counter[1] 리셋
    let c = state.advance(0, 0, None);
    assert_eq!(c[0], 2);
    assert_eq!(c[1], 0);

    // 다른 numbering_id → 히스토리 없으면 리셋
    let c = state.advance(1, 0, None);
    assert_eq!(c[0], 1);
}

#[test]
fn test_expand_numbering_format_digit() {
    let numbering = Numbering {
        raw_data: None,
        heads: [NumberingHead {
            number_format: 0,
            ..Default::default()
        }; 7],
        level_formats: [
            "^1.".to_string(),
            "^2.".to_string(),
            "^3)".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ],
        start_number: 0,
        level_start_numbers: [1, 1, 1, 1, 1, 1, 1],
        raw_para_heads: None,
    };
    let counters = [3, 2, 1, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^1.",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        0,
    );
    assert_eq!(result, "3.");

    let result = expand_numbering_format(
        "^2.",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "2.");

    let result = expand_numbering_format(
        "(^3)",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        2,
    );
    assert_eq!(result, "(1)");
}

#[test]
fn test_expand_numbering_format_hangul() {
    let mut heads = [NumberingHead::default(); 7];
    heads[1].number_format = 8; // HangulGaNaDa
    let numbering = Numbering {
        raw_data: None,
        heads,
        level_formats: [
            String::new(),
            "^2.".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ],
        start_number: 0,
        level_start_numbers: [1, 1, 1, 1, 1, 1, 1],
        raw_para_heads: None,
    };
    let counters = [1, 3, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^2.",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "다.");
}

#[test]
fn test_expand_numbering_format_level_path() {
    // ^n/^N: 레벨 경로 자동코드 (#2145). 재현 문서는 전 수준 "^N".
    let numbering = Numbering {
        raw_data: None,
        heads: [NumberingHead {
            number_format: 0,
            ..Default::default()
        }; 7],
        level_formats: [
            "^N".to_string(),
            "^N".to_string(),
            "^N".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ],
        start_number: 0,
        level_start_numbers: [1, 1, 1, 1, 1, 1, 1],
        raw_para_heads: None,
    };

    // level 0: "1.", 카운터 전진 후 "2."
    let counters = [1, 0, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^N",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        0,
    );
    assert_eq!(result, "1.");
    let counters = [2, 0, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^N",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        0,
    );
    assert_eq!(result, "2.");

    // level 1: "1.1." → "1.4."
    let counters = [1, 1, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^N",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "1.1.");
    let counters = [1, 4, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^N",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "1.4.");

    // ^n: 후행 마침표 없음
    let counters = [2, 3, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^n",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "2.3");

    // 접두·접미 문자 보존
    let result = expand_numbering_format(
        "[^n]",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "[2.3]");

    // 상위 수준 카운터 0이면 시작번호로 폴백
    let counters = [0, 2, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^N",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "1.2.");
}

#[test]
fn test_expand_numbering_format_level_path_mixed_format() {
    // 수준별 number_format 혼합: L1=Digit, L2=HangulGaNaDa → "1.가."
    let mut heads = [NumberingHead::default(); 7];
    heads[1].number_format = 8; // HangulGaNaDa
    let numbering = Numbering {
        raw_data: None,
        heads,
        level_formats: [
            "^N".to_string(),
            "^N".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ],
        start_number: 0,
        level_start_numbers: [1, 1, 1, 1, 1, 1, 1],
        raw_para_heads: None,
    };
    let counters = [1, 1, 0, 0, 0, 0, 0];
    let result = expand_numbering_format(
        "^N",
        &counters,
        &numbering,
        &numbering.level_start_numbers,
        1,
    );
    assert_eq!(result, "1.가.");
}

#[test]
fn test_numbering_format_to_number_format() {
    assert!(matches!(
        numbering_format_to_number_format(0),
        NumFmt::Digit
    ));
    assert!(matches!(
        numbering_format_to_number_format(1),
        NumFmt::CircledDigit
    ));
    assert!(matches!(
        numbering_format_to_number_format(2),
        NumFmt::RomanUpper
    ));
    assert!(matches!(
        numbering_format_to_number_format(8),
        NumFmt::HangulGaNaDa
    ));
    assert!(matches!(
        numbering_format_to_number_format(255),
        NumFmt::Digit
    ));
}

// =====================================================================
// NumberingState 카운터 재계산 테스트
// =====================================================================

#[test]
fn test_numbering_state_level_change_recalculation() {
    // 시나리오: 가, 나, 다 → 나를 한 단계 내리면 → 가, 1), 나
    let mut state = NumberingState::default();

    // 같은 numbering_id=1로 3개 문단 모두 level 0
    let c1 = state.advance(1, 0, None); // "가"
    assert_eq!(c1[0], 1);

    let c2 = state.advance(1, 0, None); // "나"
    assert_eq!(c2[0], 2);

    let c3 = state.advance(1, 0, None); // "다"
    assert_eq!(c3[0], 3);

    // 이제 나를 level 1로 변경 후 처음부터 재계산
    state.reset();

    let c1 = state.advance(1, 0, None); // "가" (level 0, counter[0]=1)
    assert_eq!(c1[0], 1);

    let c2 = state.advance(1, 1, None); // level 1, counter[1]=1 → "1)"
    assert_eq!(c2[0], 1); // level 0 카운터 유지
    assert_eq!(c2[1], 1); // level 1 카운터 = 1

    let c3 = state.advance(1, 0, None); // 다 → "나" (level 0, counter[0]=2)
    assert_eq!(c3[0], 2); // level 0 = 2, 즉 "나"
    assert_eq!(c3[1], 0); // 하위 수준 리셋
}

#[test]
fn test_numbering_state_promote_recalculation() {
    // 시나리오: 한 단계 올리기
    // 1), 2), 3) → 2)를 한 단계 올리면 → 1), 가, 1)
    let mut state = NumberingState::default();

    // 모두 level 1
    let c1 = state.advance(1, 1, None);
    assert_eq!(c1[1], 1); // 1)

    let c2 = state.advance(1, 1, None);
    assert_eq!(c2[1], 2); // 2)

    let c3 = state.advance(1, 1, None);
    assert_eq!(c3[1], 3); // 3)

    // 2)를 level 0으로 올린 후 재계산
    state.reset();

    let c1 = state.advance(1, 1, None);
    assert_eq!(c1[1], 1); // 1)

    let c2 = state.advance(1, 0, None); // 한 단계 올림 → level 0
    assert_eq!(c2[0], 1); // "가"
    assert_eq!(c2[1], 0); // 하위 수준 리셋

    let c3 = state.advance(1, 1, None);
    assert_eq!(c3[0], 1); // level 0 유지
    assert_eq!(c3[1], 1); // level 1 = 1 → "1)" (리셋되었으므로)
}

#[test]
fn test_numbering_state_different_numbering_id_resets() {
    use crate::model::paragraph::NumberingRestart;
    // para-head-num-2.hwp 패턴 재현:
    // id=3: 가(1), 나(2) → id=2: 가(1, 리셋) → id=3: 다(3, 복원) → id=4: 1(1) → id=4: 2(2)
    let mut state = NumberingState::default();

    // id=3: 가, 나
    let c1 = state.advance(3, 1, None);
    assert_eq!(c1[1], 1); // "가"
    let c2 = state.advance(3, 1, None);
    assert_eq!(c2[1], 2); // "나"

    // id=2: 새 번호 시작 (히스토리 없음 → 리셋)
    let c3 = state.advance(2, 1, None);
    assert_eq!(c3[1], 1); // "가" (리셋)

    // id=3: 이전 번호 이어 (히스토리 복원 → 2에서 이어서 3)
    let c4 = state.advance(3, 1, None);
    assert_eq!(c4[1], 3); // "다"

    // id=4: 새 번호 시작 (히스토리 없음 → 리셋)
    let c5 = state.advance(4, 1, None);
    assert_eq!(c5[1], 1); // "1" (format이 다르지만 counter=1)

    // id=4: 앞 번호 이어
    let c6 = state.advance(4, 1, None);
    assert_eq!(c6[1], 2); // "2"
}

#[test]
fn test_geometric_shapes_treated_as_fullwidth() {
    // Task #146: Geometric Shapes (U+25A0-U+25FF) 는 HWP 문서의 섹션 머리
    // 기호 (□ 1. / ■ 가. / ○ ㅇ 등) 로 널리 쓰이므로 전각(font_size) 폭
    // 으로 측정되어야 한다.
    let style = TextStyle {
        font_size: 20.0,
        ..Default::default()
    };
    for c in ['□', '■', '▲', '▼', '◆', '○', '●', '◇'] {
        let text = c.to_string();
        let positions = compute_char_positions(&text, &style);
        assert!(
            (positions[1] - 20.0).abs() < 0.01,
            "'{}' (U+{:04X}) expected full-width advance 20.0, got {}",
            c,
            c as u32,
            positions[1]
        );
    }
}

#[test]
fn test_square_bullet_with_space_preserves_layout() {
    // Task #146 회귀 방지: "□ 가" 제목 패턴에서 □ 가 반각으로 측정되면
    // 후속 글자 x 좌표가 em 단위만큼 좌측으로 붕괴한다.
    // 자간 -8% 는 text-align.hwp 제목 CharShape 와 동일.
    let style = TextStyle {
        font_size: 20.0,
        letter_spacing: -1.6, // -8% of 20
        ..Default::default()
    };
    let positions = compute_char_positions("□ 가", &style);
    assert_eq!(positions.len(), 4);
    // [#2279] 자간은 글자폭 비례 (통제 사다리 실측): 전각은 fs-비례와 동일,
    // 반각(공백)은 절반만 압축된다.
    // □: 전각(20) + 자간(20×-8%) = advance 18.4
    assert!(
        (positions[1] - 18.4).abs() < 0.01,
        "positions[1] expected 18.4, got {}",
        positions[1]
    );
    // 공백: 반각(10) + 자간(10×-8% = -0.8) = advance 9.2 (min_clamp 5.0 미작동)
    assert!(
        (positions[2] - 27.6).abs() < 0.01,
        "positions[2] expected 27.6, got {}",
        positions[2]
    );
    // 가: 전각(20) + 자간(-1.6) = advance 18.4
    assert!(
        (positions[3] - 46.0).abs() < 0.01,
        "positions[3] expected 46.0, got {}",
        positions[3]
    );
}

#[test]
fn test_tac_leading_width_block_table_full_line() {
    // Task #146 v3: block 취급 TAC 표(너비 ≥ 90% seg_width)에서
    // composed.tac_controls 가 비어있을 때, 선행 텍스트는 line 0 전체로
    // 간주해 모든 run 폭을 합산해야 한다. text-align.hwp 문단 0.2 시나리오.
    use super::super::composer::{ComposedLine, ComposedParagraph, ComposedTextRun};
    use crate::renderer::style_resolver::{ResolvedCharStyle, ResolvedStyleSet};

    let line = ComposedLine {
        runs: vec![ComposedTextRun {
            text: "    ".to_string(),
            char_style_id: 0,
            lang_index: 0,
            ..Default::default()
        }],
        line_height: 400,
        baseline_distance: 320,
        segment_width: 48188,
        column_start: 0,
        line_spacing: 0,
        has_line_break: false,
        char_start: 0,
    };
    let composed = ComposedParagraph {
        lines: vec![line],
        para_style_id: 0,
        inline_controls: Vec::new(),
        numbering_text: None,
        tac_controls: Vec::new(), // block 취급이라 비어있음
        footnote_positions: Vec::new(),
        tab_extended: Vec::new(),
    };
    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        char_styles: vec![ResolvedCharStyle {
            font_size: 20.0,
            letter_spacing: -1.6,
            ..Default::default()
        }],
        ..Default::default()
    };
    let width = super::compute_tac_leading_width(&composed, 0, &styles);
    // [#2279] 자간 글자폭 비례: 4 spaces × (10 base + 10×-8% = 9.2) = 36.8
    // (min_clamp 5.0 미작동)
    assert!((width - 36.8).abs() < 0.5, "expected ~36.8, got {}", width);
}

#[test]
fn test_is_heavy_display_face_matches_known_heavy_faces() {
    // Task #146 v4: HY헤드라인M 등 heavy display face 는 CharShape.bold=false
    // 여도 본래 heavy 이므로 SVG 에서 font-weight="bold" 강제 대상이어야 한다.
    //
    // Task #574: HY견명조 는 한컴 일반 두께 명조 — heavy 가 아님. 제거.
    // HY견명조B 는 명시 Bold variant — 보존.
    use crate::renderer::style_resolver::is_heavy_display_face;
    for face in [
        "HY헤드라인M",
        "HYHeadLine M",
        "HYHeadLine Medium",
        "HY견고딕",
        "HY견명조B",
        "HY그래픽",
        "HY그래픽M",
    ] {
        assert!(is_heavy_display_face(face), "{} should be heavy", face);
    }
    // 일반 face 는 false (HY견명조 는 Task #574 에서 heavy 제거)
    for face in [
        "Malgun Gothic",
        "맑은 고딕",
        "함초롬바탕",
        "함초롬돋움",
        "바탕",
        "돋움",
        "HY신명조",
        "HY중고딕",
        "HY견명조",
    ] {
        assert!(!is_heavy_display_face(face), "{} should NOT be heavy", face);
    }
}

#[test]
fn test_is_heavy_display_face_with_family_chain() {
    // font-family 체인에서 primary face(첫 항목) 기준 판정.
    use crate::renderer::style_resolver::is_heavy_display_face;
    assert!(is_heavy_display_face(
        "HY헤드라인M,'Malgun Gothic',sans-serif"
    ));
    assert!(is_heavy_display_face("HY견고딕, 돋움"));
    // 따옴표 포함
    assert!(is_heavy_display_face("'HY헤드라인M',Malgun Gothic"));
    assert!(is_heavy_display_face("\"HY그래픽\",바탕"));
    // primary 가 heavy 가 아니면 false (HY헤드라인M 이 두번째여도 false)
    assert!(!is_heavy_display_face("Malgun Gothic,HY헤드라인M"));
}

#[test]
fn test_tac_leading_width_inline_table_partial() {
    // inline 취급 TAC 표: tac_controls 에 위치 기록. 해당 위치까지만 합산.
    use super::super::composer::{ComposedLine, ComposedParagraph, ComposedTextRun};
    use crate::renderer::style_resolver::{ResolvedCharStyle, ResolvedStyleSet};

    let line = ComposedLine {
        runs: vec![ComposedTextRun {
            text: "ab가나".to_string(),
            char_style_id: 0,
            lang_index: 0,
            ..Default::default()
        }],
        line_height: 400,
        baseline_distance: 320,
        segment_width: 48188,
        column_start: 0,
        line_spacing: 0,
        has_line_break: false,
        char_start: 0,
    };
    let composed = ComposedParagraph {
        lines: vec![line],
        para_style_id: 0,
        inline_controls: Vec::new(),
        numbering_text: None,
        tac_controls: vec![(2, 1000, 0)], // pos=2 (ab 뒤), control_index=0
        footnote_positions: Vec::new(),
        tab_extended: Vec::new(),
    };
    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        char_styles: vec![ResolvedCharStyle {
            font_size: 20.0,
            ..Default::default()
        }],
        ..Default::default()
    };
    let width = super::compute_tac_leading_width(&composed, 0, &styles);
    // "ab" 2 chars, 반각 × font_size/2 = 20*0.5*2 = 20
    assert!((width - 20.0).abs() < 0.5, "expected ~20.0, got {}", width);
}

// ────────────────────────────────────────────────────────────
// Task #290: resolve_last_tab_pending — cross-run 탭 감지 헬퍼
// ────────────────────────────────────────────────────────────

/// ext[2] 생성 편의: high=tab_type_enum+1, low=fill_type
fn mk_ext(width_hu: u16, tab_kind_hi: u8, fill_lo: u8) -> [u16; 7] {
    let tab_type = ((tab_kind_hi as u16) << 8) | (fill_lo as u16);
    [width_hu, 0, tab_type, 0, 0, 0, 9]
}

fn mk_text_style() -> TextStyle {
    TextStyle {
        font_size: 12.0,
        font_family: String::new(),
        line_x_offset: 0.0,
        ..Default::default()
    }
}

#[test]
fn task290_inline_left_returns_none() {
    // inline 이 LEFT (ext[2] high=1) 이면 pending 없음 — 본 수정의 핵심
    let ext = vec![mk_ext(100, 1, 0)]; // LEFT, fill=none
    let ts = mk_text_style();
    let tab_stops = vec![TabStop {
        position: 22.0,
        tab_type: 0,
        fill_type: 0,
    }];
    let result = super::paragraph_layout::resolve_last_tab_pending(
        "abc\t", 0, &ext, &ts, &tab_stops, 48.0, true, 420.0,
    );
    assert_eq!(result, None, "LEFT inline 은 pending 없음");
}

#[test]
fn task290_inline_right_uses_tabdef() {
    // inline 이 RIGHT (ext[2] high=2) 면 TabDef find_next_tab_stop 경로로 폴스루
    let ext = vec![mk_ext(200, 2, 3)]; // RIGHT, fill=dot
    let ts = mk_text_style();
    let tab_stops = vec![TabStop {
        position: 300.0,
        tab_type: 1,
        fill_type: 3,
    }];
    let result = super::paragraph_layout::resolve_last_tab_pending(
        "abc\t", 0, &ext, &ts, &tab_stops, 48.0, false, 420.0,
    );
    assert_eq!(
        result,
        Some((300.0, 1, 3)),
        "RIGHT inline → TabDef 기반 위치, fill=dot"
    );
}

#[test]
fn task290_inline_center_uses_tabdef() {
    // inline 이 CENTER (ext[2] high=3) 면 TabDef 기반 위치
    let ext = vec![mk_ext(150, 3, 0)]; // CENTER
    let ts = mk_text_style();
    let tab_stops = vec![TabStop {
        position: 200.0,
        tab_type: 2,
        fill_type: 0,
    }];
    let result = super::paragraph_layout::resolve_last_tab_pending(
        "abc\t", 0, &ext, &ts, &tab_stops, 48.0, false, 420.0,
    );
    assert_eq!(
        result,
        Some((200.0, 2, 0)),
        "CENTER inline → TabDef 기반 위치, fill 없음"
    );
}

#[test]
fn task290_no_inline_fallback_to_tabdef() {
    // inline_tabs 가 비었으면 TabDef 폴백 — 기존 동작 유지
    let ext: Vec<[u16; 7]> = vec![];
    let ts = mk_text_style();
    let tab_stops = vec![TabStop {
        position: 250.0,
        tab_type: 1,
        fill_type: 0,
    }];
    let result = super::paragraph_layout::resolve_last_tab_pending(
        "abc\t", 0, &ext, &ts, &tab_stops, 48.0, false, 420.0,
    );
    assert_eq!(
        result,
        Some((250.0, 1, 0)),
        "inline 없음 → TabDef RIGHT stop 사용, fill 없음"
    );
}

#[test]
fn task290_no_inline_auto_tab_right_fallthrough() {
    // inline 없음 + TabDef stop 소진 + auto_tab_right=true → 우측 끝 RIGHT (기존 동작 유지)
    let ext: Vec<[u16; 7]> = vec![];
    let ts = mk_text_style();
    let tab_stops = vec![TabStop {
        position: 10.0,
        tab_type: 0,
        fill_type: 0,
    }]; // 이미 지나친 stop
    let result = super::paragraph_layout::resolve_last_tab_pending(
        "abcdef\t", 0, &ext, &ts, &tab_stops, 48.0, true, 420.0,
    );
    assert!(result.is_some(), "auto_tab_right 폴스루 → Some");
    let (tp, tt, _ft) = result.unwrap();
    assert_eq!(tt, 1, "auto_tab_right 은 RIGHT(1)");
    assert!(
        (tp - 420.0).abs() < 0.1,
        "tab_pos 는 available_width 에 고정"
    );
}

// [Task #296] inline_tab_type 헬퍼 단위 테스트
// HWP tab_extended 의 ext[2] 포맷: high byte = 탭 종류 enum+1, low byte = fill_type

#[test]
fn task296_inline_tab_type_left() {
    // ext[2] = 0x0100 (256) → high=1 = LEFT (exam_math #18 실측 케이스)
    let ext = [132u16, 0, 0x0100, 0, 0, 0, 9];
    assert_eq!(super::text_measurement::inline_tab_type(&ext), 1);
}

#[test]
fn task296_inline_tab_type_right() {
    // ext[2] = 0x0203 (515) → high=2 = RIGHT, low=3 = fill=dot
    //         (hwp-3.0-HWPML 저작권\t1 실측 케이스, PR #292 트러블슈팅 기록)
    let ext = [200u16, 0, 0x0203, 0, 0, 0, 9];
    assert_eq!(super::text_measurement::inline_tab_type(&ext), 2);
}

#[test]
fn task296_inline_tab_type_center() {
    // ext[2] = 0x0300 → high=3 = CENTER
    let ext = [150u16, 0, 0x0300, 0, 0, 0, 9];
    assert_eq!(super::text_measurement::inline_tab_type(&ext), 3);
}

#[test]
fn task296_inline_tab_type_decimal() {
    // ext[2] = 0x0400 → high=4 = DECIMAL
    let ext = [100u16, 0, 0x0400, 0, 0, 0, 9];
    assert_eq!(super::text_measurement::inline_tab_type(&ext), 4);
}

/// [#4334 stableIndex 서수화] `paper_node_sort_key`가 더 이상 `node.id`(next_id 카운터)
/// 를 참조하지 않음을 고정한다. **폐기된 이전 pin**(2026-08-09 이전 커밋)은 정확히
/// 반대를 검증했다 — layer 없는(inline) 노드는 `(plane=2, z=0, stable=node.id)` 폴백,
/// layer 있는(task1197) 노드는 `object_stable_index(para,ctrl)` 패킹 — 두 갈래가
/// 서로 다른 수 공간(para_index=1 layered 노드가 벌써 65536, 카운터인 inline 은
/// 보통 수십~수백)이라 하나의 수로 비교할 수 없었다. 이제 둘 다
/// `doc_path_for_node`(render_tree.rs, #4334)가 유도하는 같은 `DocPath` 좌표계를
/// 쓴다 — `RenderLayerInfo.stable_index` 는 더 이상 세 번째 정렬키가 아니다.
#[test]
fn issue_4334_paper_node_sort_key_no_longer_depends_on_node_id() {
    use crate::renderer::render_tree::ImageNode;

    fn image_at(id: u32, para: usize, control: usize) -> RenderNode {
        let mut image = ImageNode::new(0, None);
        image.section_index = Some(0);
        image.para_index = Some(para);
        image.control_index = Some(control);
        RenderNode::new(
            id,
            RenderNodeType::Image(image),
            BoundingBox::new(0.0, 0.0, 1.0, 1.0),
        )
    }

    // 같은 문서 위치(para=3, control=1), 다른 node.id(5 vs 999) → 같은 정렬키.
    // 카운터 기반이었다면 달랐을 것 — #4334 목표(node.id 로부터 독립)의 직접 증거.
    assert_eq!(
        LayoutEngine::paper_node_sort_key(&image_at(5, 3, 1)),
        LayoutEngine::paper_node_sort_key(&image_at(999, 3, 1)),
        "node.id 가 달라도 문서 위치(para,control)가 같으면 정렬키가 같아야 한다"
    );

    // node.id 는 "이르지만"(1) 문서 위치는 더 늦은(para=5) 노드가, node.id 는
    // "늦지만"(9000) 문서 위치는 더 이른(para=1) 노드보다 위(뒤)여야 한다 — 카운터
    // 순서였다면 반대로 나왔을 상황.
    let later_para_low_id = image_at(1, 5, 0);
    let earlier_para_high_id = image_at(9000, 1, 0);
    assert!(
        LayoutEngine::paper_node_sort_key(&later_para_low_id)
            > LayoutEngine::paper_node_sort_key(&earlier_para_high_id),
        "정렬은 node.id 가 아니라 문서 위치(para)를 따라야 한다"
    );

    // layer=Some(레이어 있음, task1197 케이스)과 layer=None(인라인)이 이제 같은
    // 좌표계를 공유한다 — `RenderLayerInfo.stable_index`(예전엔 65536 같은 패킹값)를
    // 더 이상 세 번째 원소로 읽지 않으므로, 그 값을 아무리 크게 채워도 무시된다.
    let mut layered_para1 = image_at(50, 1, 0);
    layered_para1.set_layer(RenderLayerInfo::new(None, 0, 999_999));
    let inline_para300 = image_at(1, 300, 0);
    assert!(
        LayoutEngine::paper_node_sort_key(&inline_para300)
            > LayoutEngine::paper_node_sort_key(&layered_para1),
        "para_index=300 인라인이 para_index=1 layered 보다 위여야 한다 — 예전엔 \
         layered 의 패킹된 stable_index 가 자릿수만으로 항상 이겼다(#4334 결함)"
    );

    // 문서 위치를 유도할 수 없는 노드(#4334 stage3 실측 잔여 — 대부분 구조 노드나
    // 아직 doc_path_for_node 가 다루지 않는 타입) → 빈 경로로 결정적으로 폴백한다.
    // node.id 를 전혀 참조하지 않으므로 서로 다른 id 라도 완전히 동일한 정렬키다.
    fn positionless(id: u32) -> RenderNode {
        RenderNode::new(
            id,
            RenderNodeType::Column(0),
            BoundingBox::new(0.0, 0.0, 1.0, 1.0),
        )
    }
    assert_eq!(
        LayoutEngine::paper_node_sort_key(&positionless(5)).2,
        Vec::<u32>::new(),
        "문서 위치를 못 만드는 노드는 빈 DocPath 로 폴백한다(node.id 아님)"
    );
    assert_eq!(
        LayoutEngine::paper_node_sort_key(&positionless(5)),
        LayoutEngine::paper_node_sort_key(&positionless(999)),
        "빈 경로 폴백은 node.id 값과 무관하게 항상 동일해야 한다"
    );
}

/// [#4334 갱신] 세 번째 정렬키가 `RenderLayerInfo.stable_index`(패킹된 u32) 에서
/// `doc_path_for_node`(문서 위치 — 이 테스트에서는 `GroupNode` 의 section/para/control)
/// 로 바뀌었다. 원래 이 테스트는 `RenderNodeType::Column` 을 자리표시자로 썼는데,
/// Column 은 out-of-flow 개체로 실제 존재하지 않고(`paper_images` 에 절대 들어가지
/// 않는 순수 레이아웃 구조 노드) `doc_path_for_node` 도 다루지 않으므로 이제 항상
/// 빈 경로를 반환해 정렬이 깨진다 — 실제로 `paper_images` 에 들어가는 타입(Group)
/// 으로 바꾸고, 원래 stable_index 값(0,2,3,0,1)과 같은 상대 순서가 나오도록
/// control_index 를 그대로 재사용한다. 정렬 결과(BehindText → flow → InFrontOfText,
/// 각 안에서 z_order/문서위치 오름차순)는 바뀌지 않는다 — 정렬 알고리즘이 아니라
/// 세 번째 키의 유도 방식만 바뀌었다는 근거.
#[test]
fn task1197_paper_nodes_sort_by_plane_z_order_and_stable_index() {
    use crate::renderer::render_tree::GroupNode;

    fn node(id: u32, text_wrap: TextWrap, z_order: i32, control_index: usize) -> RenderNode {
        RenderNode::new(
            id,
            RenderNodeType::Group(GroupNode {
                section_index: Some(0),
                para_index: Some(0),
                control_index: Some(control_index),
            }),
            BoundingBox::new(0.0, 0.0, 1.0, 1.0),
        )
        .with_layer(RenderLayerInfo::new(
            Some(text_wrap),
            z_order,
            control_index as u32,
        ))
    }

    let mut nodes = vec![
        node(1, TextWrap::InFrontOfText, 0, 0),
        node(2, TextWrap::BehindText, 11, 2),
        node(3, TextWrap::BehindText, 1, 3),
        node(4, TextWrap::TopAndBottom, 0, 0),
        node(5, TextWrap::BehindText, 11, 1),
    ];

    LayoutEngine::sort_paper_render_nodes(&mut nodes);

    let order: Vec<u32> = nodes.iter().map(|node| node.id).collect();
    assert_eq!(
        order,
        vec![3, 5, 2, 4, 1],
        "BehindText는 z-order/stable 순서로 먼저, flow, InFrontOfText 순으로 정렬"
    );
}

#[test]
fn master_page_controls_sort_by_render_layer_z_order() {
    fn rect_control(z_order: i32, horizontal_offset: u32) -> Control {
        Control::Shape(Box::new(ShapeObject::Rectangle(RectangleShape {
            common: CommonObjAttr {
                width: 10_000,
                height: 10_000,
                horizontal_offset,
                z_order,
                text_wrap: TextWrap::InFrontOfText,
                horz_rel_to: HorzRelTo::Paper,
                vert_rel_to: VertRelTo::Paper,
                ..Default::default()
            },
            ..Default::default()
        })))
    }

    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());
    let mut tree = PageRenderTree::new(0, layout.page_width, layout.page_height);
    let master_page = MasterPage {
        paragraphs: vec![Paragraph {
            controls: vec![
                rect_control(20, 0),
                rect_control(10, 20_000),
                rect_control(20, 40_000),
            ],
            ..Default::default()
        }],
        text_width: 10_000,
        text_height: 10_000,
        ..Default::default()
    };

    engine.build_master_page_into(
        &mut tree,
        Some(&master_page),
        &layout,
        &[],
        &ResolvedStyleSet::default(),
        &[],
        0,
        1,
    );

    let master_node = tree
        .root
        .children
        .iter()
        .find(|node| matches!(node.node_type, RenderNodeType::MasterPage))
        .expect("master page node should be rendered");
    let z_order: Vec<i32> = master_node
        .children
        .iter()
        .filter_map(|node| match node.node_type {
            RenderNodeType::Rectangle(_) => node.layer.map(|layer| layer.z_order),
            _ => None,
        })
        .collect();

    assert_eq!(
        z_order,
        vec![10, 20, 20],
        "master-page children should replay Hancom object order, not raw control order"
    );
}

fn first_master_child_layer<F>(tree: &PageRenderTree, predicate: F) -> RenderLayerInfo
where
    F: Fn(&RenderNodeType) -> bool + Copy,
{
    fn find<F>(node: &RenderNode, predicate: F) -> Option<RenderLayerInfo>
    where
        F: Fn(&RenderNodeType) -> bool + Copy,
    {
        if predicate(&node.node_type) {
            return node.layer;
        }
        node.children
            .iter()
            .find_map(|child| find(child, predicate))
    }

    let master = tree
        .root
        .children
        .iter()
        .find(|node| matches!(node.node_type, RenderNodeType::MasterPage))
        .expect("master page node should be rendered");
    find(master, predicate).expect("matching master-page child should carry a layer")
}

fn master_rect_control(width: u32, height: u32) -> Control {
    Control::Shape(Box::new(ShapeObject::Rectangle(RectangleShape {
        common: CommonObjAttr {
            width,
            height,
            text_wrap: TextWrap::InFrontOfText,
            horz_rel_to: HorzRelTo::Paper,
            vert_rel_to: VertRelTo::Paper,
            horz_align: HorzAlign::Left,
            vert_align: VertAlign::Top,
            ..Default::default()
        },
        ..Default::default()
    })))
}

#[test]
fn master_page_paper_sized_background_replays_behind_body_text() {
    let page = a4_page_def();
    let tree = render_tree_with_master_page_control(master_rect_control(page.width, page.height));
    let layer = first_master_child_layer(&tree, |node_type| {
        matches!(node_type, RenderNodeType::Rectangle(_))
    });

    assert_eq!(layer.text_wrap, Some(TextWrap::BehindText));
}

#[test]
fn master_page_smaller_front_control_stays_in_front_of_body_text() {
    let page = a4_page_def();
    let tree =
        render_tree_with_master_page_control(master_rect_control(page.width / 2, page.height / 2));
    let layer = first_master_child_layer(&tree, |node_type| {
        matches!(node_type, RenderNodeType::Rectangle(_))
    });

    assert_eq!(layer.text_wrap, Some(TextWrap::InFrontOfText));
}

fn first_master_child_bbox<F>(tree: &PageRenderTree, predicate: F) -> BoundingBox
where
    F: Fn(&RenderNodeType) -> bool + Copy,
{
    fn find<F>(node: &RenderNode, predicate: F) -> Option<BoundingBox>
    where
        F: Fn(&RenderNodeType) -> bool + Copy,
    {
        if predicate(&node.node_type) {
            return Some(node.bbox);
        }
        node.children
            .iter()
            .find_map(|child| find(child, predicate))
    }

    let master = tree
        .root
        .children
        .iter()
        .find(|node| matches!(node.node_type, RenderNodeType::MasterPage))
        .expect("master page node should be rendered");
    find(master, predicate).expect("matching master-page child should be rendered")
}

fn render_tree_with_master_page_control(control: Control) -> PageRenderTree {
    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());
    let mut tree = PageRenderTree::new(0, layout.page_width, layout.page_height);
    let master_page = MasterPage {
        paragraphs: vec![Paragraph {
            controls: vec![control],
            ..Default::default()
        }],
        text_width: 10_000,
        text_height: 10_000,
        ..Default::default()
    };

    engine.build_master_page_into(
        &mut tree,
        Some(&master_page),
        &layout,
        &[],
        &ResolvedStyleSet::default(),
        &[],
        0,
        1,
    );
    tree
}

#[test]
fn master_page_paper_relative_shape_uses_page_origin() {
    let tree = render_tree_with_master_page_control(Control::Shape(Box::new(
        ShapeObject::Rectangle(RectangleShape {
            common: CommonObjAttr {
                width: 7_500,
                height: 3_000,
                horizontal_offset: 1_500,
                vertical_offset: 2_250,
                horz_rel_to: HorzRelTo::Paper,
                vert_rel_to: VertRelTo::Paper,
                text_wrap: TextWrap::InFrontOfText,
                ..Default::default()
            },
            ..Default::default()
        }),
    )));

    let bbox = first_master_child_bbox(&tree, |node_type| {
        matches!(node_type, RenderNodeType::Rectangle(_))
    });
    assert!((bbox.x - hwpunit_to_px(1_500, DEFAULT_DPI)).abs() < 0.01);
    assert!((bbox.y - hwpunit_to_px(2_250, DEFAULT_DPI)).abs() < 0.01);
}

#[test]
fn master_page_paper_relative_picture_uses_page_origin() {
    let tree = render_tree_with_master_page_control(Control::Picture(Box::new(
        crate::model::image::Picture {
            common: CommonObjAttr {
                width: 7_500,
                height: 3_000,
                horizontal_offset: 1_500,
                vertical_offset: 2_250,
                horz_rel_to: HorzRelTo::Paper,
                vert_rel_to: VertRelTo::Paper,
                text_wrap: TextWrap::InFrontOfText,
                ..Default::default()
            },
            ..Default::default()
        },
    )));

    let bbox = first_master_child_bbox(&tree, |node_type| {
        // [Task #2225] 데이터 없는 픽스처 그림은 MissingPicture placeholder 로
        // 방출된다 — 위치 검증 프로브이므로 두 형태 모두 수용 (bbox 동일).
        matches!(
            node_type,
            RenderNodeType::Image(_) | RenderNodeType::Placeholder(_)
        )
    });
    assert!((bbox.x - hwpunit_to_px(1_500, DEFAULT_DPI)).abs() < 0.01);
    assert!((bbox.y - hwpunit_to_px(2_250, DEFAULT_DPI)).abs() < 0.01);
}

fn first_header_child_bbox<F>(tree: &PageRenderTree, predicate: F) -> BoundingBox
where
    F: Fn(&RenderNodeType) -> bool + Copy,
{
    fn find<F>(node: &RenderNode, predicate: F) -> Option<BoundingBox>
    where
        F: Fn(&RenderNodeType) -> bool + Copy,
    {
        if predicate(&node.node_type) {
            return Some(node.bbox);
        }
        node.children
            .iter()
            .find_map(|child| find(child, predicate))
    }

    let header = tree
        .root
        .children
        .iter()
        .find(|node| matches!(node.node_type, RenderNodeType::Header))
        .expect("header node should be rendered");
    find(header, predicate).expect("matching header child should be rendered")
}

fn render_tree_with_header_control(control: Control) -> PageRenderTree {
    use crate::model::header_footer::Header;
    use crate::renderer::pagination::HeaderFooterRef;

    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());
    let paragraphs = vec![Paragraph {
        controls: vec![Control::Header(Box::new(Header {
            paragraphs: vec![Paragraph {
                controls: vec![control],
                ..Default::default()
            }],
            ..Default::default()
        }))],
        ..Default::default()
    }];
    let page_content = PageContent {
        page_index: 0,
        page_number: 1,
        section_index: 0,
        layout,
        column_contents: Vec::new(),
        active_header: Some(HeaderFooterRef {
            para_index: 0,
            control_index: 0,
            source_section_index: 0,
            table_path: Vec::new(),
        }),
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };
    engine.build_render_tree(
        &page_content,
        &paragraphs,
        &paragraphs,
        &paragraphs,
        &[],
        &ResolvedStyleSet::default(),
        &FootnoteShape::default(),
        &[],
        None,
        &[],
        None,
        0,
        &[],
    )
}

#[test]
fn header_paper_relative_shape_uses_page_origin() {
    let tree = render_tree_with_header_control(Control::Shape(Box::new(ShapeObject::Rectangle(
        RectangleShape {
            common: CommonObjAttr {
                width: 7_500,
                height: 3_000,
                horizontal_offset: 1_500,
                vertical_offset: 2_250,
                horz_rel_to: HorzRelTo::Paper,
                vert_rel_to: VertRelTo::Paper,
                text_wrap: TextWrap::InFrontOfText,
                ..Default::default()
            },
            ..Default::default()
        },
    ))));

    let bbox = first_header_child_bbox(&tree, |node_type| {
        matches!(node_type, RenderNodeType::Rectangle(_))
    });
    assert!((bbox.x - hwpunit_to_px(1_500, DEFAULT_DPI)).abs() < 0.01);
    assert!((bbox.y - hwpunit_to_px(2_250, DEFAULT_DPI)).abs() < 0.01);
}

#[test]
fn header_paper_relative_picture_uses_page_origin() {
    let tree =
        render_tree_with_header_control(Control::Picture(Box::new(crate::model::image::Picture {
            common: CommonObjAttr {
                width: 7_500,
                height: 3_000,
                horizontal_offset: 1_500,
                vertical_offset: 2_250,
                horz_rel_to: HorzRelTo::Paper,
                vert_rel_to: VertRelTo::Paper,
                text_wrap: TextWrap::InFrontOfText,
                ..Default::default()
            },
            ..Default::default()
        })));

    let bbox = first_header_child_bbox(&tree, |node_type| {
        // [Task #2225] 데이터 없는 픽스처 그림은 MissingPicture placeholder 로
        // 방출된다 — 위치 검증 프로브이므로 두 형태 모두 수용 (bbox 동일).
        matches!(
            node_type,
            RenderNodeType::Image(_) | RenderNodeType::Placeholder(_)
        )
    });
    assert!((bbox.x - hwpunit_to_px(1_500, DEFAULT_DPI)).abs() < 0.01);
    assert!((bbox.y - hwpunit_to_px(2_250, DEFAULT_DPI)).abs() < 0.01);
}

// [Task #2102] 쪽 배경 이미지 채우기는 구역 첫 쪽에만 적용된다.
// 색 채우기는 첫 쪽 여부와 무관하게 유지된다.

/// 이미지 채우기 + 색 채우기를 가진 쪽 테두리/배경으로 렌더 트리를 만든 뒤
/// 루트 자식에서 PageBackground 노드를 찾아 (background_color, image 유무) 를 반환.
fn page_bg_color_and_image_present(is_section_first: bool) -> (bool, bool) {
    use crate::model::bin_data::BinDataContent;
    use crate::model::image::ImageEffect;
    use crate::model::page::PageBorderFill;
    use crate::model::style::ImageFillMode;
    use crate::renderer::style_resolver::{ResolvedBorderStyle, ResolvedImageFill};

    let engine = LayoutEngine::with_default_dpi();
    let layout = PageLayoutInfo::from_page_def_default(&a4_page_def(), &ColumnDef::default());
    let page_content = PageContent {
        page_index: 0,
        page_number: 0,
        section_index: 0,
        layout,
        column_contents: Vec::new(),
        active_header: None,
        active_footer: None,
        page_number_pos: None,
        page_hide: None,
        footnotes: Vec::new(),
        active_master_page: None,
        extra_master_pages: Vec::new(),
    };

    let styles = ResolvedStyleSet {
        hwp3_variant: false,
        border_styles: vec![ResolvedBorderStyle {
            fill_color: Some(0x00F0F0F0),
            image_fill: Some(ResolvedImageFill {
                bin_data_id: 1,
                fill_mode: ImageFillMode::FitToSize,
                brightness: 0,
                contrast: 0,
                effect: ImageEffect::RealPic,
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let bin_data = vec![BinDataContent {
        id: 1,
        data: vec![0xFF, 0xD8, 0xFF, 0xE0].into(), // JPEG magic (내용 무관, 존재만 확인)
        extension: "jpg".to_string(),
    }];
    let page_border_fill = PageBorderFill {
        border_fill_id: 1,
        ..Default::default()
    };

    engine.set_current_page_is_section_first(is_section_first);
    let tree = engine.build_render_tree(
        &page_content,
        &[],
        &[],
        &[],
        &[],
        &styles,
        &FootnoteShape::default(),
        &bin_data,
        None,
        &[],
        Some(&page_border_fill),
        0,
        &[],
    );

    let bg = tree.root.children.iter().find_map(|c| match &c.node_type {
        RenderNodeType::PageBackground(bg) => Some(bg),
        _ => None,
    });
    let bg = bg.expect("PageBackground 노드가 있어야 함");
    (bg.background_color.is_some(), bg.image.is_some())
}

#[test]
fn page_bg_image_only_on_section_first_page() {
    // 구역 첫 쪽: 이미지 채우기 적용
    let (color_first, image_first) = page_bg_color_and_image_present(true);
    assert!(image_first, "구역 첫 쪽에는 배경 이미지가 있어야 한다");
    assert!(color_first, "색 채우기는 유지되어야 한다");

    // 구역 첫 쪽 아님: 이미지 채우기 억제, 색 채우기는 유지
    let (color_rest, image_rest) = page_bg_color_and_image_present(false);
    assert!(!image_rest, "구역 첫 쪽이 아니면 배경 이미지가 없어야 한다");
    assert!(color_rest, "이미지가 억제돼도 색 채우기는 유지되어야 한다");
}

/// [Task #2835] TAC picture/shape 배치 경로의 좌측 margin 이 paragraph_layout.rs
/// (Task #544 v2, 커밋 a30dca73) 의 "margin_left 단일 가산" 규칙과 일치해야 한다.
///
/// 버그(수정 전): `has_visible_stroke && border_spacing[0]==[1]==0` 인 문단에서
/// `inner_pad_left = para_margin_left` 를 추가로 더해 TAC 그림이 같은 문단의 본문
/// 텍스트보다 `para_margin_left` 만큼 더 오른쪽으로 밀렸다 (exam_kor.hwp pi=46 등
/// 실측 inner_pad_left=11.33px). 본 테스트는 `border_fill_id`/`border_spacing` 유무와
/// 무관하게 `tac_picture_effective_margin_left` 가 `para_margin_left`(+indent) 만
/// 반환해야 함을 검증한다.
#[test]
fn tac_picture_effective_margin_left_matches_paragraph_layout_single_margin_rule() {
    use super::tac_picture_effective_margin_left;

    // 테두리(has_visible_stroke) + border_spacing=0 케이스 (버그 트리거 조건)여도
    // margin_left 를 한 번만 반영해야 한다. 버그 있던 구현이라면 11.33 + 11.33 = 22.66.
    let para_margin_left = 11.33;
    assert!(
        (tac_picture_effective_margin_left(para_margin_left, 0.0) - para_margin_left).abs() < 1e-9,
        "border_spacing=0/유테두리 문단에서도 margin_left 를 한 번만 더해야 함 \
         (이중 가산 버그: 22.66 이 아니라 11.33 이어야 함)"
    );

    // indent>0 (첫 줄 hanging indent) 이면 margin_left + indent 만 더해야 한다.
    let para_indent = 13.23;
    assert!(
        (tac_picture_effective_margin_left(para_margin_left, para_indent)
            - (para_margin_left + para_indent))
            .abs()
            < 1e-9,
        "indent>0 이면 margin_left + indent 만 반영해야 함 (inner_pad 이중 가산 없이)"
    );
}

/// [#4515] 최상위 표 y 겹침 검출 — 검출기 단위 동작.
///
/// `LAYOUT_OVERFLOW` 는 본문 하단 초과만 잡아 하단 clamp 겹침(#4514)에 침묵했다.
/// 검출기는 y 시작 정렬 후 인접 쌍의 `위 표 하단 - 아래 표 상단 > 임계` 를 겹침으로
/// 판정한다. 임계 2px 이하는 테두리 접합 오차다 (sample1 20쪽 1.7px 실측).
#[test]
fn detect_table_overlaps_flags_only_above_threshold() {
    // 겹침 없음 (접합 오차 1.7px 포함) → 0건
    let spans = vec![(10, 75.6, 312.7), (20, 311.0, 577.1)];
    assert!(detect_table_overlaps(spans, TABLE_OVERLAP_THRESHOLD_PX).is_empty());

    // #4514 8쪽 실측 좌표: 102(182.3~676.3) / 118(202.5~710.8) / 119(430.1~1046.9)
    // / 139(491.4~1046.9) → 인접 3쌍 전부 겹침. 입력 순서는 뒤섞여도 정렬로 복원된다.
    let spans = vec![
        (139, 491.4, 555.5 + 491.4),
        (119, 430.1, 616.8 + 430.1),
        (102, 182.3, 494.0 + 182.3),
        (118, 202.5, 508.3 + 202.5),
    ];
    let found = detect_table_overlaps(spans, TABLE_OVERLAP_THRESHOLD_PX);
    assert_eq!(found.len(), 3, "인접 3쌍 모두 겹침으로 검출돼야 한다");
    let pairs: Vec<(usize, usize)> = found.iter().map(|f| (f.0, f.1)).collect();
    assert_eq!(pairs, vec![(102, 118), (118, 119), (119, 139)]);
    let max_overlap = found.iter().map(|f| f.6).fold(0.0f64, f64::max);
    assert!(
        (max_overlap - 555.5).abs() < 0.1,
        "최대 겹침은 119~139 쌍의 555.5px 이어야 한다 (실측: {max_overlap})"
    );

    // 정확히 임계값(2.0px)은 접합 오차로 보고 무시한다
    let spans = vec![(1, 0.0, 100.0), (2, 98.0, 200.0)];
    assert!(detect_table_overlaps(spans, TABLE_OVERLAP_THRESHOLD_PX).is_empty());
}

/// [#4515] 최상위 표 수집 도메인 — Page 직계(overlay z-layer)와 Body→Column 직계
/// (흐름 표)는 포함하고, 셀 안 중첩 표와 비가시 노드는 제외한다.
#[test]
fn collect_top_level_table_spans_domain() {
    fn table_node(id: u32, pi: usize, y: f64, h: f64) -> RenderNode {
        RenderNode::new(
            id,
            RenderNodeType::Table(crate::renderer::render_tree::TableNode {
                row_count: 4,
                col_count: 5,
                border_fill_id: 0,
                section_index: Some(0),
                para_index: Some(pi),
                control_index: Some(0),
                cell_context: None,
            }),
            BoundingBox::new(75.6, y, 642.5, h),
        )
    }

    let mut root = RenderNode::new(
        0,
        RenderNodeType::Page(crate::renderer::render_tree::PageNode {
            page_index: 7,
            width: 793.7,
            height: 1122.5,
            section_index: 0,
        }),
        BoundingBox::new(0.0, 0.0, 793.7, 1122.5),
    );

    // Body → Column → 흐름 표 (+ 그 셀 안의 중첩 표는 제외 대상)
    let mut flow_table = table_node(4, 118, 202.5, 508.3);
    let mut cell = RenderNode::new(
        5,
        RenderNodeType::TableCell(crate::renderer::render_tree::TableCellNode {
            col: 0,
            row: 0,
            col_span: 1,
            row_span: 1,
            border_fill_id: 0,
            text_direction: 0,
            clip: false,
            model_cell_index: None,
        }),
        BoundingBox::new(75.6, 202.5, 100.0, 100.0),
    );
    cell.children.push(table_node(6, 999, 210.0, 50.0)); // 중첩 표 — 수집 금지
    flow_table.children.push(cell);
    let mut column = RenderNode::new(
        3,
        RenderNodeType::Column(0),
        BoundingBox::new(75.6, 75.6, 642.5, 971.3),
    );
    column.children.push(flow_table);
    let mut body = RenderNode::new(
        2,
        RenderNodeType::Body { clip_rect: None },
        BoundingBox::new(75.6, 75.6, 642.5, 971.3),
    );
    body.children.push(column);
    root.children.push(body);

    // Page 직계 overlay 표 2개 (하나는 비가시 — 제외)
    root.children.push(table_node(7, 102, 182.3, 494.0));
    let mut hidden = table_node(8, 555, 300.0, 100.0);
    hidden.visible = false;
    root.children.push(hidden);

    let mut spans = collect_top_level_table_spans(&root);
    spans.sort_by_key(|s| s.0);
    assert_eq!(
        spans.iter().map(|s| s.0).collect::<Vec<_>>(),
        vec![102, 118],
        "Page 직계 overlay 표와 Column 직계 흐름 표만 수집한다 \
         (중첩 표 999·비가시 표 555 는 제외)"
    );
}

// ── [#4610 · #4599 ④] 공백-전용 TAC 캐리어 문단 페인트 변위 게이트 ──

/// 야간방호일지 36374873 p1 pi4 형상: 공백 텍스트 + TAC 표 1개 + 저장 세그 2개가
/// 문단 안에서 개체 밴드만큼(>100px) 벌어진 캐리어.
fn whitespace_tac_carrier_para() -> Paragraph {
    Paragraph {
        text: " \u{FFFC} ".repeat(4),
        controls: vec![Control::Table(Box::new(Table {
            common: CommonObjAttr {
                treat_as_char: true,
                width: 19_000,
                height: 1_800,
                ..Default::default()
            },
            ..Default::default()
        }))],
        line_segs: vec![
            LineSeg {
                vertical_pos: 13_575,
                line_height: 2_414,
                text_start: 0,
                ..Default::default()
            },
            LineSeg {
                vertical_pos: 67_877,
                line_height: 1_000,
                text_start: 6,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

/// 실제 배치 경로에서는 compose 결과가 존재하고, 대상 표가 inline TAC 로 등록돼야 한다.
fn whitespace_tac_carrier_composed(para: &Paragraph) -> ComposedParagraph {
    let mut composed = compose_paragraph(para);
    composed.tac_controls = vec![(0, 19_000, 0)];
    composed
}

#[test]
fn whitespace_tac_carrier_paint_y_rewinds_to_stored_vpos() {
    let para = whitespace_tac_carrier_para();
    let composed = whitespace_tac_carrier_composed(&para);
    // 흐름 커서가 선행 자리차지 표 하단(1064px)까지 밀린 상태 — 저장 위치로 되돌린다.
    let got =
        whitespace_tac_carrier_stored_paint_y(true, &para, Some(&composed), 75.6, 1064.0, 96.0);
    let expected = 75.6 + 13_575.0 / 75.0;
    assert!((got.unwrap() - expected).abs() < 0.1, "got {got:?}");
}

#[test]
fn whitespace_tac_carrier_paint_y_requires_hwpx_stored_profile() {
    let para = whitespace_tac_carrier_para();
    let composed = whitespace_tac_carrier_composed(&para);
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(false, &para, Some(&composed), 75.6, 1064.0, 96.0),
        None
    );
}

#[test]
fn whitespace_tac_carrier_paint_y_rejects_substantive_text_host() {
    let mut para = whitespace_tac_carrier_para();
    para.text = "본문 텍스트".into();
    let composed = whitespace_tac_carrier_composed(&para);
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(true, &para, Some(&composed), 75.6, 1064.0, 96.0),
        None
    );
}

#[test]
fn whitespace_tac_carrier_paint_y_rejects_float_host_para() {
    // 자리차지(비-TAC) 표를 함께 앵커한 host 문단은 기존 float 계약 소관 — 제외.
    let mut para = whitespace_tac_carrier_para();
    para.controls.push(Control::Table(Box::new(Table {
        common: CommonObjAttr {
            treat_as_char: false,
            text_wrap: TextWrap::TopAndBottom,
            ..Default::default()
        },
        ..Default::default()
    })));
    let composed = whitespace_tac_carrier_composed(&para);
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(true, &para, Some(&composed), 75.6, 1064.0, 96.0),
        None
    );
}

#[test]
fn whitespace_tac_carrier_paint_y_rejects_small_intra_gap() {
    // 낡은 세대 사다리(문단 간격 누락류)는 문단-내 거대 간격을 만들지 않는다 —
    // 세그 간 간격이 100px(7500HU) 미만이면 무동작.
    let mut para = whitespace_tac_carrier_para();
    para.line_segs[1].vertical_pos = 13_575 + 2_414 + 7_000;
    let composed = whitespace_tac_carrier_composed(&para);
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(true, &para, Some(&composed), 75.6, 1064.0, 96.0),
        None
    );
}

#[test]
fn whitespace_tac_carrier_paint_y_rejects_forward_displacement() {
    // 방향-한정: 흐름이 저장 위치보다 아래로 충분히 밀렸을 때만 되돌린다.
    let para = whitespace_tac_carrier_para();
    let composed = whitespace_tac_carrier_composed(&para);
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(true, &para, Some(&composed), 75.6, 200.0, 96.0),
        None
    );
}

#[test]
fn whitespace_tac_carrier_paint_y_rejects_synthetic_segs() {
    let mut para = whitespace_tac_carrier_para();
    para.line_segs[0].tag |= crate::model::paragraph::LineSeg::TAG_IMPLEMENTATION_PROPERTY;
    let composed = whitespace_tac_carrier_composed(&para);
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(true, &para, Some(&composed), 75.6, 1064.0, 96.0),
        None
    );
}

#[test]
fn whitespace_tac_carrier_paint_y_rejects_missing_or_block_tac() {
    let para = whitespace_tac_carrier_para();
    let block_composed = compose_paragraph(&para);
    assert!(
        block_composed.tac_controls.is_empty(),
        "이 fixture의 표는 강제 inline 등록 없이 block 후보여야 한다"
    );
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(true, &para, None, 75.6, 1064.0, 96.0),
        None
    );
    assert_eq!(
        whitespace_tac_carrier_stored_paint_y(
            true,
            &para,
            Some(&block_composed),
            75.6,
            1064.0,
            96.0,
        ),
        None
    );
}

/// [표 간 세로선 이음매 회귀 — scslic2] 실제 관세 서식
/// `samples/hwpx/scslic2-header-repeatbody-seam.hwpx`(전기용품 세관장확인물품
/// 확인서)는 본문에 최상위 표 두 개가 위아래로 붙어 있다: `#HEADER`(17행×9열)와
/// 바로 아래의 `#REPEAT-BODY:신청내용`(13행×6열). 원본 XML 기준으로
/// `#HEADER` 의 맨 왼쪽 "신청인" 열(col0, 2000 HWPUNIT)과 그 오른쪽 "수입자/
/// 수입화주" 열(col1-2 병합, 3800 HWPUNIT)의 합은 `#REPEAT-BODY` 맨 왼쪽
/// "신청내용" 열(col0-1 병합, 5800 HWPUNIT)과 **정확히 같아야** 하며
/// (2000+3800=5800), 그래야 두 표 사이로 세로 테두리가 끊김 없이 이어진다
/// (한/글 실측 렌더와 동일).
///
/// 회귀의 정체: 두 표는 서로를 전혀 모른 채 각각 독립적으로 열 폭이 풀리므로,
/// 한쪽만 틀려도 단일 표 단위 검증으로는 잡히지 않고 **이웃 표와 비교할 때만**
/// 눈에 보인다. 과거 span 오름차순 해석은 `#HEADER` 의 col3/col4 제약을
/// 잘못 풀어 합이 선언 폭 48000 을 넘겼고(실측 56400), 그 초과분을 모든 열에
/// 일률 비례 축소(48000/56400)해 흡수하면서 이미 올바르던 col0+col1+col2 까지
/// 5800 → 약 4936 HWPUNIT 으로 함께 줄여버렸다(약 11.5px 이음매 어긋남).
/// `#REPEAT-BODY` 는 초과가 없어 5800 을 그대로 유지하므로 어긋남이 드러났다.
///
/// 합성 표 리터럴 테스트(`overlapping_row_colspan_partitions_do_not_overshoot_declared_table_width`)가
/// `#HEADER` 의 제약 패턴만 따로 고정한다면, 이 테스트는 실제 파일을 파싱해
/// **두 표 사이의 관계**를 고정한다.
#[test]
fn scslic2_header_and_repeat_body_share_left_column_seam_x() {
    use crate::model::document::Document;

    /// 표 식별용 마커 텍스트: 이 파이프라인의 `#HEADER` / `#REPEAT-BODY:...`
    /// 규약대로 표의 (row 0, col 0) 셀 문단 텍스트에 마커가 들어 있다.
    /// 하드코딩 인덱스 대신 이 텍스트로 찾아 표 순서가 바뀌어도 견디게 한다.
    /// `Paragraph.text` 는 HWP 인라인 제어문자를 그대로 지니므로 완전일치가
    /// 아니라 `contains` 로 본다(`#HEADER` 는 `#FOOTER` 와 겹치지 않는다).
    fn table_marker_text(table: &Table) -> String {
        table
            .cells
            .iter()
            .filter(|cell| cell.row == 0 && cell.col == 0)
            .flat_map(|cell| cell.paragraphs.iter())
            .map(|para| para.text.as_str())
            .collect::<String>()
    }

    fn find_top_level_table<'a>(doc: &'a Document, marker: &str) -> Option<&'a Table> {
        doc.sections
            .iter()
            .flat_map(|section| section.paragraphs.iter())
            .flat_map(|para| para.controls.iter())
            .find_map(|control| match control {
                Control::Table(table) if table_marker_text(table).contains(marker) => {
                    Some(table.as_ref())
                }
                _ => None,
            })
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("samples/hwpx/scslic2-header-repeatbody-seam.hwpx");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|err| panic!("픽스처 읽기 실패 {}: {err}", path.display()));
    let doc = crate::parser::hwpx::parse_hwpx(&bytes).expect("scslic2 HWPX 파싱 실패");

    let header = find_top_level_table(&doc, "#HEADER").expect("전제: `#HEADER` 표를 찾지 못함");
    let repeat_body = find_top_level_table(&doc, "#REPEAT-BODY:신청내용")
        .expect("전제: `#REPEAT-BODY:신청내용` 표를 찾지 못함");

    assert_eq!(header.col_count, 9, "전제: `#HEADER` 는 9열 표다");
    assert_eq!(
        repeat_body.col_count, 6,
        "전제: `#REPEAT-BODY:신청내용` 은 6열 표다"
    );

    let engine = LayoutEngine::with_default_dpi();
    let header_widths = engine.resolve_column_widths(header, header.col_count as usize);
    let body_widths = engine.resolve_column_widths(repeat_body, repeat_body.col_count as usize);

    // `#HEADER`: col0("신청인") + col1+col2("수입자"/"수입화주")
    let header_seam: f64 = header_widths[0] + header_widths[1] + header_widths[2];
    // `#REPEAT-BODY`: col0+col1("신청내용")
    let body_seam: f64 = body_widths[0] + body_widths[1];
    let expected_seam = hwpunit_to_px(5800, DEFAULT_DPI);

    // (1) 상대 검증 — 두 표의 이음매 x 좌표가 어긋나지 않는다.
    assert!(
        (header_seam - body_seam).abs() < 0.5,
        "표 간 세로선 이음매 어긋남: header(col0+col1+col2)={header_seam} body(col0+col1)={body_seam} \
         header_widths={header_widths:?} body_widths={body_widths:?}"
    );

    // (2) 절대 검증 — 둘 다 원본 XML 의 5800 HWPUNIT 과 일치한다. 두 표가
    //     같은 값으로 함께 틀리는 경우(예: 공통 스케일 오류)도 잡는다.
    assert!(
        (header_seam - expected_seam).abs() < 0.5,
        "`#HEADER` 이음매 폭이 5800 HWPUNIT 과 다름: actual={header_seam} expected={expected_seam} \
         header_widths={header_widths:?}"
    );
    assert!(
        (body_seam - expected_seam).abs() < 0.5,
        "`#REPEAT-BODY` 이음매 폭이 5800 HWPUNIT 과 다름: actual={body_seam} expected={expected_seam} \
         body_widths={body_widths:?}"
    );

    // (3) 근본 원인 가드 — 이음매 어긋남을 만든 것은 선언 폭 초과 뒤의 일률
    //     비례 축소였다. 두 표 모두 합이 선언 폭 48000 HWPUNIT 과 맞아야 한다.
    let declared_total = hwpunit_to_px(48000, DEFAULT_DPI);
    let header_total: f64 = header_widths.iter().sum();
    let body_total: f64 = body_widths.iter().sum();
    assert!(
        (header_total - declared_total).abs() < 0.5,
        "`#HEADER` 총 열 폭이 선언 폭과 다름: actual={header_total} expected={declared_total} \
         header_widths={header_widths:?}"
    );
    assert!(
        (body_total - declared_total).abs() < 0.5,
        "`#REPEAT-BODY` 총 열 폭이 선언 폭과 다름: actual={body_total} expected={declared_total} \
         body_widths={body_widths:?}"
    );
}
