//! fixtures — tests/mod.rs 에서 무변동 이동
use super::*;

/// 표 셀이 포함된 테스트 문서를 생성한다.
pub(crate) fn create_doc_with_table() -> HwpDocument {
    use crate::model::control::Control;
    use crate::model::document::SectionDef;
    use crate::model::page::PageDef;
    use crate::model::table::{Cell, Table};
    use crate::model::Padding;

    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();

    let page_def = PageDef {
        width: 59528,
        height: 84188,
        margin_left: 8504,
        margin_right: 8504,
        margin_top: 5669,
        margin_bottom: 4252,
        margin_header: 4252,
        margin_footer: 4252,
        ..Default::default()
    };

    let mut table = Table {
        row_count: 2,
        col_count: 2,
        padding: Padding {
            left: 100,
            right: 100,
            top: 100,
            bottom: 100,
        },
        cells: vec![
            Cell {
                col: 0,
                row: 0,
                col_span: 1,
                row_span: 1,
                width: 21000,
                height: 3000,
                paragraphs: vec![Paragraph {
                    text: "셀A".to_string(),
                    char_count: 2,
                    char_offsets: make_char_offsets("셀A"),
                    line_segs: vec![LineSeg {
                        line_height: 400,
                        baseline_distance: 320,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
            Cell {
                col: 1,
                row: 0,
                col_span: 1,
                row_span: 1,
                width: 21000,
                height: 3000,
                paragraphs: vec![Paragraph {
                    text: "셀B".to_string(),
                    char_count: 2,
                    char_offsets: make_char_offsets("셀B"),
                    line_segs: vec![LineSeg {
                        line_height: 400,
                        baseline_distance: 320,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
            Cell {
                col: 0,
                row: 1,
                col_span: 1,
                row_span: 1,
                width: 21000,
                height: 3000,
                paragraphs: vec![Paragraph {
                    text: "셀C".to_string(),
                    char_count: 2,
                    char_offsets: make_char_offsets("셀C"),
                    line_segs: vec![LineSeg {
                        line_height: 400,
                        baseline_distance: 320,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
            Cell {
                col: 1,
                row: 1,
                col_span: 1,
                row_span: 1,
                width: 21000,
                height: 3000,
                paragraphs: vec![Paragraph {
                    text: "셀D".to_string(),
                    char_count: 2,
                    char_offsets: make_char_offsets("셀D"),
                    line_segs: vec![LineSeg {
                        line_height: 400,
                        baseline_distance: 320,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    table.rebuild_grid();

    let parent_para = Paragraph {
        text: String::new(),
        controls: vec![Control::Table(Box::new(table))],
        line_segs: vec![LineSeg {
            line_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }],
        ..Default::default()
    };

    document.sections.push(Section {
        section_def: SectionDef {
            page_def,
            ..Default::default()
        },
        paragraphs: vec![parent_para],
        raw_stream: None,
    });
    doc.set_document(document);
    doc
}

/// #2424 page-count commit 검증용: 한 쪽에 거의 차는 1열 RowBreak 표.
/// 마지막 cell의 줄 수만 늘리면 표 continuation이 한 쪽 더 필요해진다.
pub(crate) fn create_doc_with_page_count_boundary_table() -> HwpDocument {
    use crate::model::control::Control;
    use crate::model::document::SectionDef;
    use crate::model::page::PageDef;
    use crate::model::table::{Cell, Table, TablePageBreak};
    use crate::model::Padding;

    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    let page_def = PageDef {
        width: 59528,
        height: 84188,
        margin_left: 8504,
        margin_right: 8504,
        margin_top: 5669,
        margin_bottom: 4252,
        margin_header: 4252,
        margin_footer: 4252,
        ..Default::default()
    };
    let row_count = 13u16;
    let mut cells = Vec::with_capacity(row_count as usize);
    for row in 0..row_count {
        let text = if row + 1 == row_count {
            "가"
        } else {
            "고정"
        };
        cells.push(Cell {
            row,
            col: 0,
            row_span: 1,
            col_span: 1,
            width: if row + 1 == row_count { 2_200 } else { 42_000 },
            height: if row + 1 == row_count { 600 } else { 5_250 },
            paragraphs: vec![Paragraph {
                text: text.to_string(),
                char_count: text.chars().count() as u32,
                char_offsets: make_char_offsets(text),
                line_segs: vec![LineSeg {
                    line_height: 400,
                    baseline_distance: 320,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        });
    }
    let mut table = Table {
        row_count,
        col_count: 1,
        page_break: TablePageBreak::RowBreak,
        padding: Padding {
            left: 100,
            right: 100,
            top: 100,
            bottom: 100,
        },
        cells,
        ..Default::default()
    };
    table.rebuild_grid();
    let parent_para = Paragraph {
        controls: vec![Control::Table(Box::new(table))],
        line_segs: vec![LineSeg {
            line_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }],
        ..Default::default()
    };
    document.sections.push(Section {
        section_def: SectionDef {
            page_def,
            ..Default::default()
        },
        paragraphs: vec![parent_para],
        raw_stream: None,
    });
    doc.set_document(document);
    doc
}




/// 편집 가능한 빈 문서 생성 헬퍼 (blank 템플릿 기반)
pub(crate) fn create_editable_doc() -> HwpDocument {
    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document_native().unwrap();
    doc
}




/// [Task #1161] 떠 있는 그림(tac=false)을 반복 붙여넣으면 cascade 오프셋이 누적된다.
pub(crate) fn create_doc_with_floating_picture(tac: bool, voff: u32, hoff: u32) -> HwpDocument {
    use crate::model::control::Control;
    use crate::model::document::{Section, SectionDef};
    use crate::model::image::Picture;
    use crate::model::page::PageDef;
    use crate::model::shape::CommonObjAttr;

    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    let page_def = PageDef {
        width: 59528,
        height: 84188,
        margin_left: 8504,
        margin_right: 8504,
        margin_top: 5669,
        margin_bottom: 4252,
        margin_header: 4252,
        margin_footer: 4252,
        ..Default::default()
    };
    let pic = Control::Picture(Box::new(Picture {
        common: CommonObjAttr {
            treat_as_char: tac,
            vertical_offset: voff,
            horizontal_offset: hoff,
            width: 5000,
            height: 5000,
            ..Default::default()
        },
        ..Default::default()
    }));
    let pic_para = Paragraph {
        controls: vec![pic],
        line_segs: vec![LineSeg {
            line_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }],
        ..Default::default()
    };
    document.sections.push(Section {
        section_def: SectionDef {
            page_def,
            ..Default::default()
        },
        paragraphs: vec![pic_para, Paragraph::default()],
        raw_stream: None,
    });
    doc.set_document(document);
    doc
}

pub(crate) fn collect_picture_voffsets(doc: &HwpDocument) -> Vec<u32> {
    use crate::model::control::Control;
    let mut offs = Vec::new();
    for sec in &doc.document.sections {
        for p in &sec.paragraphs {
            for c in &p.controls {
                if let Control::Picture(pic) = c {
                    offs.push(pic.common.vertical_offset);
                }
            }
        }
    }
    offs.sort_unstable();
    offs
}



/// 단계 4-1: HWP 프로그램으로 생성한 이미지 참조 파일 분석
/// output/pic-01-as-text.hwp: 빈 문서 → 이미지 1개 → 글자처리로 삽입

pub(crate) fn issue_1481_json_usize(json: &str, key: &str) -> usize {
    let parsed: Value = serde_json::from_str(json).expect("JSON 파싱");
    parsed[key].as_u64().expect("usize 값") as usize
}

pub(crate) fn issue_1481_table<'a>(doc: &'a HwpDocument, para_idx: usize) -> &'a crate::model::table::Table {
    use crate::model::control::Control;

    doc.document.sections[0].paragraphs[para_idx]
        .controls
        .iter()
        .find_map(|control| match control {
            Control::Table(table) => Some(table.as_ref()),
            _ => None,
        })
        .expect("표 컨트롤")
}

pub(crate) fn issue_1481_first_page_render_tree(
    doc: &HwpDocument,
) -> crate::renderer::render_tree::PageRenderTree {
    let dpi = 96.0;
    let styles = crate::renderer::style_resolver::resolve_styles(&doc.document.doc_info, dpi);
    let engine = crate::renderer::layout::LayoutEngine::new(dpi);
    let section = &doc.document.sections[0];
    let composed: Vec<_> = section
        .paragraphs
        .iter()
        .map(crate::renderer::composer::compose_paragraph)
        .collect();
    let sec_mt = doc
        .measured_tables
        .first()
        .map(|tables| tables.as_slice())
        .unwrap_or(&[]);
    let page = &doc.pagination[0].pages[0];

    engine.build_render_tree(
        page,
        &section.paragraphs,
        &section.paragraphs,
        &section.paragraphs,
        &composed,
        &styles,
        &section.section_def.footnote_shape,
        &doc.document.bin_data_content,
        None,
        sec_mt,
        Some(&section.section_def.page_border_fill),
        section.section_def.outline_numbering_id,
        &[],
    )
}
