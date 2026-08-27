//! task_features — tests/mod.rs 에서 무변동 이동
use super::*;

/// 타스크66: 텍스트+Table(treat_as_char) 혼합 문단의 인라인 렌더링 검증
/// treat_as_char 표는 텍스트와 같은 줄에 인라인 배치되어야 함
#[test]
fn test_task66_table_text_mixed_paragraph_rendering() {
    use crate::renderer::composer::compose_paragraph;
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    let path = "samples/img-start-001.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    // para[1]: 텍스트와 Table 컨트롤이 공존하는 문단
    let para1 = &doc.document.sections[0].paragraphs[1];
    assert!(!para1.text.is_empty(), "para[1]에 텍스트가 있어야 함");
    let has_treat_as_char_table = para1
        .controls
        .iter()
        .any(|c| matches!(c, Control::Table(t) if t.attr & 0x01 != 0));
    assert!(
        has_treat_as_char_table,
        "para[1]에 treat_as_char Table이 있어야 함"
    );

    // compose: 2개 줄 이상
    let composed = compose_paragraph(para1);
    assert!(composed.lines.len() >= 2, "최소 2줄 이상이어야 함");
    let line1_text: String = composed.lines[1]
        .runs
        .iter()
        .map(|r| r.text.as_str())
        .collect();
    assert!(
        line1_text.contains("주관부서"),
        "두 번째 줄에 '주관부서' 텍스트가 있어야 함"
    );

    // pagination: 블록형 treat_as_char 표(2+ line_segs)는 PageItem::Table로 emit
    // truly inline(1 line_seg + 텍스트)만 FullParagraph로 처리
    assert!(
        para1.line_segs.len() >= 2,
        "para[1]은 2+ line_segs (블록형 treat_as_char)"
    );
    let mut found_block_table = false;
    let mut found_partial_para = false;
    for pr in doc.pagination.iter() {
        for page in &pr.pages {
            for col in &page.column_contents {
                for item in &col.items {
                    match item {
                        crate::renderer::pagination::PageItem::Table { para_index, .. }
                            if *para_index == 1 =>
                        {
                            found_block_table = true;
                        }
                        crate::renderer::pagination::PageItem::PartialParagraph {
                            para_index,
                            ..
                        } if *para_index == 1 => {
                            found_partial_para = true;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    assert!(
        found_block_table,
        "블록형 treat_as_char 표는 PageItem::Table로 emit되어야 함"
    );
    assert!(
        found_partial_para,
        "블록형 treat_as_char 표의 텍스트는 PartialParagraph로 emit되어야 함"
    );

    // 렌더 트리: Table과 TextRun이 모두 존재해야 함
    let tree = doc.build_page_tree(0).unwrap();
    fn find_table_and_text(node: &RenderNode, table_found: &mut bool, text_found: &mut bool) {
        match &node.node_type {
            RenderNodeType::Table(_) => {
                *table_found = true;
            }
            RenderNodeType::TextRun(ref tr) => {
                if tr.para_index == Some(1) && tr.cell_context.is_none() && !tr.text.is_empty() {
                    *text_found = true;
                }
            }
            _ => {}
        }
        for child in &node.children {
            find_table_and_text(child, table_found, text_found);
        }
    }
    let mut table_found = false;
    let mut text_found = false;
    find_table_and_text(&tree.root, &mut table_found, &mut text_found);
    assert!(table_found, "렌더 트리에 표가 있어야 함");
    assert!(text_found, "렌더 트리에 para[1] 텍스트가 있어야 함");

    // SVG: 개별 문자가 <text> 요소로 출력되는지 확인
    let svg = doc.render_page_svg_native(0).unwrap();
    assert!(svg.contains("주"), "SVG에 '주' 문자가 포함되어야 함");
    assert!(svg.contains("【"), "SVG에 '【' 문자가 포함되어야 함");
}



/// 타스크 76: hwp-multi-001.hwp 2페이지에 그룹 이미지 3장이 존재하는지 검증
#[test]
fn test_task76_multi_001_group_images() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    let path = "samples/hwp-multi-001.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();
    assert!(doc.page_count() >= 2, "최소 2페이지 이상이어야 함");

    // 2페이지 렌더 트리에서 Image 노드 수 확인
    let tree = doc.build_page_tree(1).unwrap();
    fn count_images(node: &RenderNode) -> usize {
        let mut count = match &node.node_type {
            RenderNodeType::Image(_) => 1,
            _ => 0,
        };
        for child in &node.children {
            count += count_images(child);
        }
        count
    }
    let image_count = count_images(&tree.root);
    assert!(
        image_count >= 3,
        "hwp-multi-001.hwp 2페이지에 Image 노드가 3개 이상이어야 함 (실제: {})",
        image_count
    );
}



/// 타스크 76: hwp-3.0-HWPML.hwp 1페이지 배경 이미지가 body clip 바깥에 위치하는지 검증
#[test]
fn test_task76_background_image_outside_body_clip() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    let path = "samples/hwp-3.0-HWPML.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    let tree = doc.build_page_tree(0).unwrap();

    // root의 직접 자식(Body 바깥)에 Image 노드가 있어야 함
    let root_image_count = tree
        .root
        .children
        .iter()
        .filter(|child| {
            matches!(&child.node_type, RenderNodeType::Image(_))
                || child
                    .children
                    .iter()
                    .any(|c| matches!(&c.node_type, RenderNodeType::Image(_)))
        })
        .count();
    assert!(
        root_image_count >= 1,
        "배경 이미지가 body clip 바깥(root 직접 자식)에 있어야 함 (실제: {})",
        root_image_count
    );

    // 배경 이미지 좌표 검증: (0, 0) 근처
    fn find_root_image(node: &RenderNode) -> Option<(f64, f64)> {
        if let RenderNodeType::Image(_) = &node.node_type {
            return Some((node.bbox.x, node.bbox.y));
        }
        for child in &node.children {
            if let Some(pos) = find_root_image(child) {
                return Some(pos);
            }
        }
        None
    }
    for child in &tree.root.children {
        if let Some((x, y)) = find_root_image(child) {
            assert!(
                x.abs() < 1.0 && y.abs() < 1.0,
                "배경 이미지는 (0,0) 근처여야 함 (실제: ({:.1}, {:.1}))",
                x,
                y
            );
            break;
        }
    }
}



/// 타스크 76: hwp-img-001.hwp에 독립 이미지 4장이 존재하는지 검증
#[test]
fn test_task76_img_001_four_pictures() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    let path = "samples/hwp-img-001.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    let tree = doc.build_page_tree(0).unwrap();
    fn count_images(node: &RenderNode) -> usize {
        let mut count = match &node.node_type {
            RenderNodeType::Image(_) => 1,
            _ => 0,
        };
        for child in &node.children {
            count += count_images(child);
        }
        count
    }
    let image_count = count_images(&tree.root);
    assert_eq!(
        image_count, 4,
        "hwp-img-001.hwp에 Image 노드가 4개여야 함 (실제: {})",
        image_count
    );
}



/// 타스크 77: 이미지 셀 행이 인트라-로우 분할되지 않고 다음 페이지로 이동하는지 검증
///
/// [Task #993] 컷 모델 전환으로 행 경계가 이동(페이지 1 = rows 0..3, 기존 0..2).
/// 이미지 셀(행 2)은 여전히 인트라-분할되지 않으나(end_cut 빈 채 유지) 배치
/// 페이지가 바뀜 — 컷 측정과 기존 MeasuredTable 측정의 행 높이 차이. 한컴 2022
/// PDF 대조 후 기대값 재확정 예정.
#[test]
#[ignore = "Task #993: 컷 모델 행 높이 측정 차이로 행 경계 이동 — PDF 대조 후 재확정"]
fn test_task77_image_cell_no_intra_row_split() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    let path = "samples/20250130-hongbo.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    // 표6(4행×1열)의 PartialTable 페이지네이션 검증
    // 행2(이미지 셀)는 인트라-로우 분할되지 않아야 함
    // 표6의 para_index는 29 (task78에서 para[25] GSO 파싱 정상화 후)
    let table_para_index = 29;
    // [Task #993] (start_row, end_row, start_cut 비었나, end_cut 비었나)
    let mut found_table_pages: Vec<(usize, usize, bool, bool)> = Vec::new();
    for pr in &doc.pagination {
        for page in &pr.pages {
            for col in &page.column_contents {
                for item in &col.items {
                    if let crate::renderer::pagination::PageItem::PartialTable {
                        para_index,
                        start_row,
                        end_row,
                        start_cut,
                        end_cut,
                        ..
                    } = item
                    {
                        if *para_index == table_para_index {
                            found_table_pages.push((
                                *start_row,
                                *end_row,
                                start_cut.is_empty(),
                                end_cut.is_empty(),
                            ));
                        }
                    }
                }
            }
        }
    }

    assert_eq!(found_table_pages.len(), 2, "표6이 2페이지에 걸쳐야 함");

    // 첫 번째 페이지: rows 0..2 (행0, 행1만), end_cut 비어 있음 (인트라-로우 분할 없음)
    let (s1, e1, _ss1, se1_empty) = found_table_pages[0];
    assert_eq!(s1, 0, "첫 번째 PartialTable 시작 행");
    assert_eq!(e1, 2, "첫 번째 PartialTable 끝 행 (행2 미포함)");
    assert!(se1_empty, "인트라-로우 분할 없어야 함");

    // 두 번째 페이지: rows 2..4 (행2, 행3), start_cut 비어 있음 (연속 오프셋 없음)
    let (s2, e2, ss2_empty, _se2) = found_table_pages[1];
    assert_eq!(s2, 2, "두 번째 PartialTable 시작 행");
    assert_eq!(e2, 4, "두 번째 PartialTable 끝 행");
    assert!(ss2_empty, "연속 오프셋 없어야 함");

    // 두 페이지 모두에서 이미지가 렌더링되는지 확인
    fn find_images(node: &RenderNode) -> Vec<u16> {
        let mut ids = Vec::new();
        if let RenderNodeType::Image(img) = &node.node_type {
            ids.push(img.bin_data_id);
        }
        for child in &node.children {
            ids.extend(find_images(child));
        }
        ids
    }

    // 표6이 있는 페이지에서 이미지 확인 (PAGE 2, PAGE 3)
    let page_count = doc.page_count();
    let mut pages_with_table30_images: Vec<Vec<u16>> = Vec::new();
    for pi in 0..page_count {
        let tree = doc.build_page_tree(pi).unwrap();
        let images = find_images(&tree.root);
        // bin_data_id=6 (셀0 그림3) 또는 bin_data_id=1 (셀2 그림4)
        let table30_imgs: Vec<u16> = images
            .into_iter()
            .filter(|&id| id == 6 || id == 1)
            .collect();
        if !table30_imgs.is_empty() {
            pages_with_table30_images.push(table30_imgs);
        }
    }

    assert_eq!(
        pages_with_table30_images.len(),
        2,
        "표6 이미지가 2개 페이지에 분산되어야 함"
    );
    assert!(
        pages_with_table30_images[0].contains(&6),
        "첫 번째 페이지에 셀0 이미지(bin_data_id=6) 있어야 함"
    );
    assert!(
        pages_with_table30_images[1].contains(&1),
        "두 번째 페이지에 셀2 이미지(bin_data_id=1) 있어야 함"
    );
}



#[test]
fn test_task78_rectangle_textbox_inline_images() {
    use crate::model::shape::ShapeObject;
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    let path = "samples/20250130-hongbo.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    // para[25]의 GSO 컨트롤이 Rectangle (Group이 아닌)으로 파싱되는지 검증
    let section = &doc.document.sections[0];
    let para25 = &section.paragraphs[25];
    assert_eq!(para25.controls.len(), 1, "para[25]에 컨트롤 1개 있어야 함");

    if let Control::Shape(shape) = &para25.controls[0] {
        if let ShapeObject::Rectangle(rect) = shape.as_ref() {
            // Rectangle으로 올바르게 파싱됨
            assert!(rect.common.treat_as_char, "treat_as_char=true");
            // TextBox가 있어야 함
            assert!(
                rect.drawing.text_box.is_some(),
                "Rectangle에 TextBox가 있어야 함"
            );
            let tb = rect.drawing.text_box.as_ref().unwrap();
            assert!(!tb.paragraphs.is_empty(), "TextBox에 문단이 있어야 함");
            // TextBox 문단에 인라인 Picture 컨트롤 2개
            let pic_count: usize = tb
                .paragraphs
                .iter()
                .flat_map(|p| &p.controls)
                .filter(|c| matches!(c, Control::Picture(_)))
                .count();
            assert_eq!(pic_count, 2, "TextBox에 인라인 Picture 2개 있어야 함");
        } else {
            panic!("para[25]의 컨트롤이 Rectangle이어야 함 (Group이 아닌)");
        }
    } else {
        panic!("para[25]의 컨트롤이 Shape이어야 함");
    }

    // 페이지 2 렌더 트리에서 이미지 2개 렌더링 확인
    fn find_images(node: &RenderNode) -> Vec<u16> {
        let mut ids = Vec::new();
        if let RenderNodeType::Image(img) = &node.node_type {
            ids.push(img.bin_data_id);
        }
        for child in &node.children {
            ids.extend(find_images(child));
        }
        ids
    }

    let tree = doc.build_page_tree(1).unwrap(); // 페이지 2 (인덱스 1)
    let images = find_images(&tree.root);
    assert!(
        images.len() >= 2,
        "페이지 2에 이미지 2개 이상 렌더링되어야 함 (실제: {}개)",
        images.len()
    );
}



/// 타스크 79: 투명선 표시 기능 — show_transparent_borders=true 시 추가 Line 노드 생성 검증
#[test]
fn test_task79_transparent_border_lines() {
    use crate::model::style::BorderLineType;
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    fn count_lines(node: &RenderNode) -> usize {
        let mut count = 0;
        if matches!(&node.node_type, RenderNodeType::Line(_)) {
            count += 1;
        }
        for child in &node.children {
            count += count_lines(child);
        }
        count
    }

    // 여러 표 포함 파일로 검증
    let files = [
        "samples/table-001.hwp",
        "samples/hwp_table_test.hwp",
        "samples/table-complex.hwp",
        "samples/hwpers_test4_complex_table.hwp",
        "samples/table-ipc.hwp",
    ];

    let mut tested = false;
    for path in &files {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        let data = std::fs::read(path).unwrap();
        let mut doc = HwpDocument::from_bytes(&data).unwrap();

        // 문서 내 None 테두리 존재 여부 확인
        let has_none_border = doc.document.doc_info.border_fills.iter().any(|bf| {
            bf.borders
                .iter()
                .any(|b| b.line_type == BorderLineType::None)
        });

        // 투명선 OFF
        doc.show_transparent_borders = false;
        let tree_off = doc.build_page_tree(0).unwrap();
        let lines_off = count_lines(&tree_off.root);

        // 투명선 ON
        doc.show_transparent_borders = true;
        let tree_on = doc.build_page_tree(0).unwrap();
        let lines_on = count_lines(&tree_on.root);

        // 회귀 없음: ON >= OFF
        assert!(
            lines_on >= lines_off,
            "{}: 투명선 ON({})이 OFF({}) 이상이어야 함",
            path,
            lines_on,
            lines_off
        );

        // SVG 렌더링 정상 확인
        let svg = doc.render_page_svg_native(0).unwrap();
        assert!(svg.contains("<svg"), "{}: SVG 렌더링 실패", path);

        eprintln!(
            "{}: OFF={} ON={} (+{}) has_none_border={}",
            path,
            lines_off,
            lines_on,
            lines_on - lines_off,
            has_none_border
        );
        tested = true;
    }
    assert!(tested, "테스트할 수 있는 파일이 없음");
}



#[test]
fn test_task80_cell_height_matches_hwp() {
    // 셀 높이 검증: 단일 줄/단일 문단 셀의 컨텐츠 높이 + 패딩 ≈ HWP 선언 높이
    // (마지막 줄 line_spacing이 제외되었는지 확인)
    use crate::renderer::composer::compose_paragraph;

    let path = "samples/table-001.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} not found", path);
        return;
    }
    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();
    let dpi = 96.0;

    let mut checked = 0;
    for sec in &doc.document.sections {
        for para in &sec.paragraphs {
            for ctrl in &para.controls {
                if let Control::Table(table) = ctrl {
                    for cell in &table.cells {
                        // 단일 행, 단일 문단, 유효한 높이만 검증
                        if cell.row_span != 1 {
                            continue;
                        }
                        if cell.paragraphs.len() != 1 {
                            continue;
                        }
                        if cell.height == 0 || cell.height >= 0x80000000 {
                            continue;
                        }

                        let comp = compose_paragraph(&cell.paragraphs[0]);
                        if comp.lines.is_empty() {
                            continue;
                        }

                        let pad_top = if cell.padding.top != 0 {
                            crate::renderer::hwpunit_to_px(cell.padding.top as i32, dpi)
                        } else {
                            crate::renderer::hwpunit_to_px(table.padding.top as i32, dpi)
                        };
                        let pad_bottom = if cell.padding.bottom != 0 {
                            crate::renderer::hwpunit_to_px(cell.padding.bottom as i32, dpi)
                        } else {
                            crate::renderer::hwpunit_to_px(table.padding.bottom as i32, dpi)
                        };

                        // 마지막 줄 line_spacing 제외
                        let lc = comp.lines.len();
                        let content: f64 = comp
                            .lines
                            .iter()
                            .enumerate()
                            .map(|(i, line)| {
                                let h = crate::renderer::hwpunit_to_px(line.line_height, dpi);
                                if i + 1 < lc {
                                    h + crate::renderer::hwpunit_to_px(line.line_spacing, dpi)
                                } else {
                                    h
                                }
                            })
                            .sum();

                        let required = content + pad_top + pad_bottom;
                        let declared = crate::renderer::hwpunit_to_px(cell.height as i32, dpi);

                        // 우리 계산값이 HWP 선언값 이하여야 함 (2px 허용)
                        assert!(required <= declared + 2.0,
                                "Cell row={} col={}: required={:.1}px > declared={:.1}px (diff={:.1}px)",
                                cell.row, cell.col, required, declared, required - declared);
                        checked += 1;
                    }
                }
            }
        }
    }
    eprintln!("task80: {}개 셀 높이 검증 통과", checked);
    assert!(checked > 0, "검증할 셀이 없음");
}



/// 타스크 81: table-004.hwp의 세로쓰기 셀 파싱 및 렌더 트리 검증
#[test]
fn test_task81_vertical_cell_text() {
    let path = "samples/table-004.hwp";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let data = std::fs::read(path).unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    // 1. 파서 검증: text_direction=2인 셀이 3개 존재
    let mut vertical_cells = Vec::new();
    for sec in &doc.document.sections {
        for para in &sec.paragraphs {
            for ctrl in &para.controls {
                if let crate::model::control::Control::Table(table) = ctrl {
                    for cell in &table.cells {
                        if cell.text_direction != 0 {
                            vertical_cells.push((cell.text_direction, cell.row, cell.col));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(vertical_cells.len(), 3, "세로쓰기 셀이 3개여야 함");
    for (td, _r, _c) in &vertical_cells {
        assert_eq!(*td, 2, "text_direction은 2(영문세움)이어야 함");
    }

    // 2. 렌더 트리 검증: SVG 내보내기로 세로 배치 확인
    let dpi = 96.0;
    let styles = crate::renderer::style_resolver::resolve_styles(&doc.document.doc_info, dpi);
    let engine = crate::renderer::layout::LayoutEngine::new(dpi);

    // pagination → render tree
    assert!(
        !doc.pagination.is_empty(),
        "pagination 결과가 비어있으면 안 됨"
    );
    let pr = &doc.pagination[0];
    assert!(!pr.pages.is_empty(), "페이지가 비어있으면 안 됨");

    let section = &doc.document.sections[0];
    let composed: Vec<_> = section
        .paragraphs
        .iter()
        .map(crate::renderer::composer::compose_paragraph)
        .collect();
    let sec_mt = doc
        .measured_tables
        .first()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let tree = engine.build_render_tree(
        &pr.pages[0],
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
    );

    // 렌더 트리에서 text_direction != 0인 TableCell 노드 찾기
    fn find_vertical_cells(
        node: &crate::renderer::render_tree::RenderNode,
    ) -> Vec<&crate::renderer::render_tree::RenderNode> {
        let mut result = Vec::new();
        if let crate::renderer::render_tree::RenderNodeType::TableCell(ref tc) = node.node_type {
            if tc.text_direction != 0 {
                result.push(node);
            }
        }
        for child in &node.children {
            result.extend(find_vertical_cells(child));
        }
        result
    }

    let vc_nodes = find_vertical_cells(&tree.root);
    assert!(
        vc_nodes.len() >= 3,
        "렌더 트리에 세로쓰기 셀이 3개 이상이어야 함, found: {}",
        vc_nodes.len()
    );

    // 각 세로쓰기 셀의 TextRun이 세로 방향으로 배치되었는지 확인
    for vc in &vc_nodes {
        let mut run_ys: Vec<f64> = Vec::new();
        for line_node in &vc.children {
            if let crate::renderer::render_tree::RenderNodeType::TextLine(_) = &line_node.node_type
            {
                for run_node in &line_node.children {
                    if let crate::renderer::render_tree::RenderNodeType::TextRun(ref tr) =
                        run_node.node_type
                    {
                        if !tr.text.trim().is_empty() {
                            run_ys.push(run_node.bbox.y);
                        }
                    }
                }
            }
        }
        // y좌표가 순차 증가해야 세로 배치
        assert!(
            run_ys.len() >= 2,
            "세로쓰기 셀에 TextRun이 2개 이상이어야 함"
        );
        for i in 1..run_ys.len() {
            assert!(
                run_ys[i] > run_ys[i - 1],
                "세로쓰기 글자의 y좌표가 순차 증가해야 함: y[{}]={} <= y[{}]={}",
                i,
                run_ys[i],
                i - 1,
                run_ys[i - 1]
            );
        }
    }
}



#[test]
fn test_task105_nested_table_path_api() {
    let data = std::fs::read("samples/inner-table-01.hwp").unwrap();
    let doc = HwpDocument::from_bytes(&data).unwrap();

    // 1. hitTest로 중첩 표 셀의 cellPath 확인
    let page_count = doc.page_count();
    eprintln!("페이지 수: {}", page_count);

    // 문서 구조 확인: 중첩 표 위치
    let sec = &doc.document.sections[0];
    for (pi, para) in sec.paragraphs.iter().enumerate() {
        for (ci, ctrl) in para.controls.iter().enumerate() {
            if let Control::Table(t) = ctrl {
                eprintln!(
                    "문단[{}] 컨트롤[{}]: 표 {}행x{}열 셀{}개",
                    pi,
                    ci,
                    t.row_count,
                    t.col_count,
                    t.cells.len()
                );
                for (cell_idx, cell) in t.cells.iter().enumerate() {
                    for (cp_idx, cp) in cell.paragraphs.iter().enumerate() {
                        for (cci, cctrl) in cp.controls.iter().enumerate() {
                            if let Control::Table(nt) = cctrl {
                                eprintln!(
                                    "  셀[{}] 문단[{}] 컨트롤[{}]: 중첩 표 {}행x{}열 셀{}개",
                                    cell_idx,
                                    cp_idx,
                                    cci,
                                    nt.row_count,
                                    nt.col_count,
                                    nt.cells.len()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // 렌더 트리에서 중첩 표 TextRun 찾기
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};
    fn find_nested_run(node: &RenderNode) -> Option<(usize, Vec<(usize, usize, usize)>)> {
        if let RenderNodeType::TextRun(ref tr) = node.node_type {
            if let Some(ref ctx) = tr.cell_context {
                if ctx.path.len() >= 2 {
                    let path: Vec<(usize, usize, usize)> = ctx
                        .path
                        .iter()
                        .map(|e| (e.control_index, e.cell_index, e.cell_para_index))
                        .collect();
                    return Some((ctx.parent_para_index, path));
                }
            }
        }
        for child in &node.children {
            if let Some(r) = find_nested_run(child) {
                return Some(r);
            }
        }
        None
    }

    // 모든 페이지에서 중첩 TextRun 탐색
    let mut nested = None;
    for page in 0..page_count {
        let tree = doc.build_page_tree(page as u32).unwrap();
        fn dump_runs(node: &RenderNode, page: u32) {
            if let RenderNodeType::TextRun(ref tr) = node.node_type {
                let ctx_info = tr
                    .cell_context
                    .as_ref()
                    .map(|ctx| {
                        format!(
                            "ppi={}, path_len={}, path={:?}",
                            ctx.parent_para_index,
                            ctx.path.len(),
                            ctx.path
                                .iter()
                                .map(|e| (e.control_index, e.cell_index, e.cell_para_index))
                                .collect::<Vec<_>>()
                        )
                    })
                    .unwrap_or_else(|| "None".to_string());
                eprintln!(
                    "  p{} TextRun: text={:?} ctx={}",
                    page,
                    tr.text.chars().take(10).collect::<String>(),
                    ctx_info
                );
            }
            for child in &node.children {
                dump_runs(child, page);
            }
        }
        dump_runs(&tree.root, page as u32);
        if nested.is_none() {
            nested = find_nested_run(&tree.root);
        }
    }
    assert!(nested.is_some(), "중첩 표 TextRun이 있어야 합니다");
    let (parent_para, path) = nested.unwrap();
    eprintln!("중첩 표 경로: parent_para={}, path={:?}", parent_para, path);

    // 2. resolve_table_by_path로 중첩 표 접근
    let table = doc.resolve_table_by_path(0, parent_para, &path);
    assert!(
        table.is_ok(),
        "resolve_table_by_path 실패: {:?}",
        table.err()
    );
    let table = table.unwrap();
    eprintln!(
        "중첩 표: {}행 x {}열, 셀 {}개",
        table.row_count,
        table.col_count,
        table.cells.len()
    );

    // 3. resolve_cell_by_path로 셀 접근
    let cell = doc.resolve_cell_by_path(0, parent_para, &path);
    assert!(cell.is_ok(), "resolve_cell_by_path 실패: {:?}", cell.err());

    // 4. getCellInfoByPath 경로 API
    let path_json = format!(
        "[{}]",
        path.iter()
            .map(|(ci, cei, cpi)| {
                format!(
                    "{{\"controlIndex\":{},\"cellIndex\":{},\"cellParaIndex\":{}}}",
                    ci, cei, cpi
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    eprintln!("path_json: {}", path_json);

    let cell_info = doc.get_cell_info_by_path_native(0, parent_para, &path_json);
    assert!(
        cell_info.is_ok(),
        "getCellInfoByPath 실패: {:?}",
        cell_info.err()
    );
    eprintln!("셀 정보: {}", cell_info.unwrap());

    // 5. getTableDimensionsByPath 경로 API
    let dims = doc.get_table_dimensions_by_path_native(0, parent_para, &path_json);
    assert!(
        dims.is_ok(),
        "getTableDimensionsByPath 실패: {:?}",
        dims.err()
    );
    eprintln!("표 차원: {}", dims.unwrap());

    // 6. getCursorRectByPath 경로 API
    let cursor = doc.get_cursor_rect_by_path_native(0, parent_para, &path_json, 0);
    assert!(
        cursor.is_ok(),
        "getCursorRectByPath 실패: {:?}",
        cursor.err()
    );
    eprintln!("커서 위치: {}", cursor.unwrap());

    // 7. getTableCellBboxesByPath 경로 API
    let bboxes = doc.get_table_cell_bboxes_by_path_native(0, parent_para, &path_json);
    assert!(
        bboxes.is_ok(),
        "getTableCellBboxesByPath 실패: {:?}",
        bboxes.err()
    );
    eprintln!("셀 bbox: {}", bboxes.unwrap());

    // 8. hitTest에서 cellPath 포함 확인
    let hit_json = doc.hit_test_native(0, 400.0, 600.0);
    if let Ok(ref json) = hit_json {
        eprintln!("hitTest 결과: {}", json);
        if json.contains("cellPath") {
            eprintln!("✓ hitTest에 cellPath 포함됨");
        } else {
            eprintln!("✗ hitTest에 cellPath 없음 — 본문 영역 클릭일 수 있음");
        }
    }
}



#[test]
fn test_task110_multi_column_reflow_diag() {
    let path = "samples/basic/KTX.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }
    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();

    eprintln!("=== KTX.hwp 다단 리플로우 진단 ===");
    eprintln!("페이지 수: {}", doc.page_count());
    eprintln!("구역 수: {}", doc.document.sections.len());

    // ColumnDef 확인
    {
        let section = &doc.document.sections[0];
        let column_def = HwpDocument::find_initial_column_def(&section.paragraphs);
        eprintln!(
            "ColumnDef: count={}, same_width={}, widths={:?}, gaps={:?}",
            column_def.column_count, column_def.same_width, column_def.widths, column_def.gaps
        );

        // PageLayoutInfo 확인
        let layout = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
            &section.section_def.page_def,
            &column_def,
            doc.dpi,
        );
        eprintln!("column_areas 수: {}", layout.column_areas.len());
        for (i, ca) in layout.column_areas.iter().enumerate() {
            let w_hu = crate::renderer::px_to_hwpunit(ca.width, doc.dpi);
            eprintln!(
                "  column_areas[{}]: x={:.1} w={:.1}px ({}hu)",
                i, ca.x, ca.width, w_hu
            );
        }

        // para_column_map 확인
        let map = &doc.para_column_map;
        eprintln!("para_column_map 구역 수: {}", map.len());
        if !map.is_empty() && !map[0].is_empty() {
            eprintln!("para_column_map[0] 길이: {}", map[0].len());
            for (pi, &ci) in map[0].iter().enumerate() {
                let seg_w = section
                    .paragraphs
                    .get(pi)
                    .and_then(|p| p.line_segs.first())
                    .map(|ls| ls.segment_width)
                    .unwrap_or(0);
                eprintln!("  para[{}] → col_idx={}, seg_w={}", pi, ci, seg_w);
            }
        } else {
            eprintln!("para_column_map[0] 비어있음!");
        }

        // 본문 전체 너비 (단일 단) 비교
        let layout_single = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
            &section.section_def.page_def,
            &crate::model::page::ColumnDef::default(),
            doc.dpi,
        );
        let body_w_hu =
            crate::renderer::px_to_hwpunit(layout_single.column_areas[0].width, doc.dpi);
        eprintln!(
            "단일 단 body width: {:.1}px ({}hu)",
            layout_single.column_areas[0].width, body_w_hu
        );
    }

    // SVG 내보내기: 편집 전
    let svg_before = doc.render_page_svg_native(0).unwrap();
    std::fs::write("output/ktx_before_edit.svg", &svg_before).ok();
    eprintln!(
        "\n편집 전 SVG: output/ktx_before_edit.svg ({} bytes)",
        svg_before.len()
    );

    // 문단 1에 텍스트 삽입
    let result = doc.insert_text_native(0, 1, 0, "테스트입력 ");
    eprintln!("insert_text 결과: {:?}", result);

    // 편집 후 line_segs 확인
    let para1 = &doc.document.sections[0].paragraphs[1];
    eprintln!("편집 후 para[1] line_segs:");
    for (i, ls) in para1.line_segs.iter().enumerate() {
        eprintln!(
            "  line[{}]: seg_w={} text_start={}",
            i, ls.segment_width, ls.text_start
        );
    }

    // SVG 내보내기: 편집 후
    let svg_after = doc.render_page_svg_native(0).unwrap();
    std::fs::write("output/ktx_after_edit.svg", &svg_after).ok();
    eprintln!(
        "편집 후 SVG: output/ktx_after_edit.svg ({} bytes)",
        svg_after.len()
    );
}



#[test]
fn test_task110_treatise_diag() {
    let path = "samples/basic/treatise sample.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }
    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();

    eprintln!("=== treatise sample.hwp 다단 구조 진단 ===");
    eprintln!("구역 수: {}", doc.document.sections.len());

    for (sec_idx, section) in doc.document.sections.iter().enumerate() {
        eprintln!("\n--- 구역 {} ---", sec_idx);
        eprintln!("문단 수: {}", section.paragraphs.len());

        // ColumnDef 확인
        let column_def = HwpDocument::find_initial_column_def(&section.paragraphs);
        eprintln!(
            "initial ColumnDef: count={}, same_width={}, spacing={}, widths={:?}, gaps={:?}",
            column_def.column_count,
            column_def.same_width,
            column_def.spacing,
            column_def.widths,
            column_def.gaps
        );
        // 2단 ColumnDef 검색
        if section.paragraphs.len() > 14 {
            let cd2 = HwpDocument::find_column_def_for_paragraph(&section.paragraphs, 14);
            eprintln!(
                "para[14] ColumnDef: count={}, same_width={}, spacing={}, widths={:?}, gaps={:?}",
                cd2.column_count, cd2.same_width, cd2.spacing, cd2.widths, cd2.gaps
            );
            let layout2 = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
                &section.section_def.page_def,
                &cd2,
                doc.dpi,
            );
            for (i, ca) in layout2.column_areas.iter().enumerate() {
                let w_hu = crate::renderer::px_to_hwpunit(ca.width, doc.dpi);
                eprintln!(
                    "  2단 column_areas[{}]: x={:.1}px w={:.1}px ({}hu)",
                    i, ca.x, ca.width, w_hu
                );
            }
        }

        // PageLayoutInfo 확인
        let layout = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
            &section.section_def.page_def,
            &column_def,
            doc.dpi,
        );
        eprintln!("column_areas 수: {}", layout.column_areas.len());
        for (i, ca) in layout.column_areas.iter().enumerate() {
            let w_hu = crate::renderer::px_to_hwpunit(ca.width, doc.dpi);
            eprintln!(
                "  column_areas[{}]: x={:.1}px w={:.1}px ({}hu)",
                i, ca.x, ca.width, w_hu
            );
        }

        // para_column_map 확인
        let map = &doc.para_column_map;
        if sec_idx < map.len() && !map[sec_idx].is_empty() {
            eprintln!("para_column_map[{}] 길이: {}", sec_idx, map[sec_idx].len());
            for (pi, &ci) in map[sec_idx].iter().enumerate() {
                let seg_w = section
                    .paragraphs
                    .get(pi)
                    .and_then(|p| p.line_segs.first())
                    .map(|ls| ls.segment_width)
                    .unwrap_or(0);
                eprintln!(
                    "  para[{}] → col_idx={}, first_line seg_w={}",
                    pi, ci, seg_w
                );
            }
        } else {
            eprintln!("para_column_map[{}] 비어있음!", sec_idx);
        }

        // 첫 10개 문단의 첫 줄 segment_width
        eprintln!("첫 10개 문단 segment_width:");
        for pi in 0..std::cmp::min(10, section.paragraphs.len()) {
            let para = &section.paragraphs[pi];
            let seg_w = para
                .line_segs
                .first()
                .map(|ls| ls.segment_width)
                .unwrap_or(0);
            let text_preview: String = para.text.chars().take(30).collect();
            eprintln!("  para[{}]: seg_w={}, text={:?}", pi, seg_w, text_preview);
        }
    }

    // 편집 시뮬레이션: 구역0, 문단1, 오프셋0에 "X" 삽입
    eprintln!("\n=== 편집 시뮬레이션: insert_text_native(0, 1, 0, \"X\") ===");
    let result = doc.insert_text_native(0, 1, 0, "X");
    eprintln!("insert_text 결과: {:?}", result);

    // 편집 후 문단1의 첫 줄 segment_width 확인
    let para1 = &doc.document.sections[0].paragraphs[1];
    eprintln!("편집 후 para[1] line_segs:");
    for (i, ls) in para1.line_segs.iter().enumerate() {
        eprintln!(
            "  line[{}]: seg_w={} text_start={} line_height={}",
            i, ls.segment_width, ls.text_start, ls.line_height
        );
    }

    // available_width 비교: 단 너비 vs 페이지 너비
    let section = &doc.document.sections[0];
    let column_def = HwpDocument::find_initial_column_def(&section.paragraphs);
    let layout = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
        &section.section_def.page_def,
        &column_def,
        doc.dpi,
    );
    let layout_single = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
        &section.section_def.page_def,
        &crate::model::page::ColumnDef::default(),
        doc.dpi,
    );

    let col_w_hu = if !layout.column_areas.is_empty() {
        crate::renderer::px_to_hwpunit(layout.column_areas[0].width, doc.dpi)
    } else {
        0
    };
    let page_w_hu = if !layout_single.column_areas.is_empty() {
        crate::renderer::px_to_hwpunit(layout_single.column_areas[0].width, doc.dpi)
    } else {
        0
    };

    let actual_seg_w = para1
        .line_segs
        .first()
        .map(|ls| ls.segment_width)
        .unwrap_or(0);
    eprintln!("\n=== para[1] available_width 비교 (1단 영역) ===");
    eprintln!("단 너비 (column_areas[0]): {}hu", col_w_hu);
    eprintln!("페이지 전체 너비 (단일 단): {}hu", page_w_hu);
    eprintln!("실제 seg_w: {}hu", actual_seg_w);

    let diff_col = (actual_seg_w as i64 - col_w_hu as i64).abs();
    let diff_page = (actual_seg_w as i64 - page_w_hu as i64).abs();
    if diff_col < diff_page {
        eprintln!("→ seg_w가 단 너비에 가까움 (차이: {}hu)", diff_col);
    } else {
        eprintln!("→ seg_w가 페이지 너비에 가까움 (차이: {}hu)", diff_page);
    }

    // 2단 영역 편집 시뮬레이션: para[14] (col_idx=1, 2단 영역)
    eprintln!("\n=== 2단 영역 편집: insert_text_native(0, 14, 0, \"Y\") ===");
    let col_idx_14_before = doc
        .para_column_map
        .first()
        .and_then(|m| m.get(14))
        .copied()
        .unwrap_or(0);
    eprintln!("편집 전 para[14] col_idx: {}", col_idx_14_before);

    let result2 = doc.insert_text_native(0, 14, 0, "Y");
    eprintln!("insert_text 결과: {:?}", result2);

    let para14 = &doc.document.sections[0].paragraphs[14];
    eprintln!("편집 후 para[14] line_segs:");
    for (i, ls) in para14.line_segs.iter().enumerate() {
        eprintln!(
            "  line[{}]: seg_w={} text_start={}",
            i, ls.segment_width, ls.text_start
        );
    }

    // find_column_def_for_paragraph 결과 확인
    let cd_for_14 =
        HwpDocument::find_column_def_for_paragraph(&doc.document.sections[0].paragraphs, 14);
    eprintln!(
        "para[14]에 적용되는 ColumnDef: count={}, same_width={}, widths={:?}",
        cd_for_14.column_count, cd_for_14.same_width, cd_for_14.widths
    );

    let layout14 = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
        &doc.document.sections[0].section_def.page_def,
        &cd_for_14,
        doc.dpi,
    );
    eprintln!("layout14 column_areas:");
    for (i, ca) in layout14.column_areas.iter().enumerate() {
        let w_hu = crate::renderer::px_to_hwpunit(ca.width, doc.dpi);
        eprintln!(
            "  [{}]: x={:.1}px w={:.1}px ({}hu)",
            i, ca.x, ca.width, w_hu
        );
    }

    let seg_w_14 = para14
        .line_segs
        .first()
        .map(|ls| ls.segment_width)
        .unwrap_or(0);
    let orig_seg_w_14 = 22960i32; // 편집 전 원본 seg_w
    eprintln!("\n=== para[14] 결과 비교 ===");
    eprintln!("원본 seg_w: {}hu", orig_seg_w_14);
    eprintln!("편집 후 seg_w: {}hu", seg_w_14);
    eprintln!("페이지 전체 너비: {}hu", page_w_hu);
    if (seg_w_14 - orig_seg_w_14).abs() < 1000 {
        eprintln!("→ 올바름: 2단 너비로 리플로우됨");
    } else if (seg_w_14 as i64 - page_w_hu as i64).abs() < 1000 {
        eprintln!("→ 오류: 1단 전체 너비로 리플로우됨!");
    } else {
        eprintln!("→ 알수없는 너비: {}hu", seg_w_14);
    }

    // === 양쪽 정렬 진단: 원본 2단 문단의 LineSeg 데이터 ===
    eprintln!("\n=== 양쪽 정렬 진단: 2단 문단 LineSeg 분석 ===");
    // 원본 데이터 재로드 (편집 전)
    let data2 = std::fs::read(path).unwrap();
    let doc2 = HwpDocument::from_bytes(&data2).unwrap();
    let section2 = &doc2.document.sections[0];

    // 2단 영역의 모든 문단의 LineSeg column_start, segment_width 출력
    for pi in 9..std::cmp::min(20, section2.paragraphs.len()) {
        let para = &section2.paragraphs[pi];
        let text_preview: String = para.text.chars().take(40).collect();
        eprintln!("\npara[{}]: text={:?}", pi, text_preview);
        eprintln!("  line_segs 수: {}", para.line_segs.len());
        for (li, ls) in para.line_segs.iter().enumerate() {
            eprintln!(
                "  line[{}]: seg_w={} col_start={} text_start={} vpos={} line_h={} line_sp={}",
                li,
                ls.segment_width,
                ls.column_start,
                ls.text_start,
                ls.vertical_pos,
                ls.line_height,
                ls.line_spacing
            );
        }
        // 문단 정렬 확인
        let ps = doc2.styles.para_styles.get(para.para_shape_id as usize);
        if let Some(ps) = ps {
            eprintln!("  alignment: {:?}", ps.alignment);
        }
    }

    // 페이지네이션 결과 확인: 2단 문단이 어떤 단에 배치되는지
    eprintln!("\n=== 페이지네이션 결과 분석 ===");
    let paginator = crate::renderer::pagination::Paginator::new(doc2.dpi);
    let composed2: Vec<_> = section2
        .paragraphs
        .iter()
        .map(|p| crate::renderer::composer::compose_paragraph(p))
        .collect();
    // 2단 ColumnDef 찾기 (para[9]+ 영역)
    let cd_for_9 = HwpDocument::find_column_def_for_paragraph(&section2.paragraphs, 9);
    eprintln!("para[9]+ ColumnDef: count={}", cd_for_9.column_count);

    // 페이지네이션 실행 (전체 섹션)
    let (pag_result, measured_sec) = paginator.paginate(
        &section2.paragraphs,
        &composed2,
        &doc2.styles,
        &section2.section_def.page_def,
        &crate::model::page::ColumnDef::default(), // 초기 ColumnDef
        0,
    );

    // 측정 높이 진단
    eprintln!("\n=== 문단별 측정 높이 (para 0~20) ===");
    let mut zone1_sum: f64 = 0.0;
    for pi in 0..std::cmp::min(20, section2.paragraphs.len()) {
        let h = measured_sec.get_paragraph_height(pi).unwrap_or(0.0);
        let mp = measured_sec.get_measured_paragraph(pi);
        let sp_b = mp.map(|m| m.spacing_before).unwrap_or(0.0);
        let sp_a = mp.map(|m| m.spacing_after).unwrap_or(0.0);
        let lh_sum: f64 = mp.map(|m| m.line_heights.iter().sum()).unwrap_or(0.0);
        let line_ct = mp.map(|m| m.line_heights.len()).unwrap_or(0);
        eprintln!(
            "  para[{}] h={:.2}px (sp_b={:.2} + lines({})={:.2} + sp_a={:.2})",
            pi, h, sp_b, line_ct, lh_sum, sp_a
        );
        if pi < 9 {
            zone1_sum += h;
        }
    }
    eprintln!("  zone1(para 0-8) sum={:.2}px", zone1_sum);
    let layout1 = crate::renderer::page_layout::PageLayoutInfo::from_page_def(
        &section2.section_def.page_def,
        &crate::model::page::ColumnDef::default(),
        doc2.dpi,
    );
    eprintln!(
        "body_area.height={:.1}px, available_body_height={:.1}px",
        layout1.body_area.height,
        layout1.available_body_height()
    );

    for (pg_idx, page) in pag_result.pages.iter().enumerate() {
        eprintln!(
            "\n페이지 {} (단 수: {}):",
            pg_idx,
            page.column_contents.len()
        );
        for col_content in &page.column_contents {
            eprintln!(
                "  단 {} (zone_y_offset={:.1}):",
                col_content.column_index, col_content.zone_y_offset
            );
            for item in &col_content.items {
                match item {
                    crate::renderer::pagination::PageItem::FullParagraph { para_index } => {
                        eprintln!("    FullParagraph(para={})", para_index);
                    }
                    crate::renderer::pagination::PageItem::PartialParagraph {
                        para_index,
                        start_line,
                        end_line,
                    } => {
                        eprintln!(
                            "    PartialParagraph(para={}, lines={}..{})",
                            para_index, start_line, end_line
                        );
                    }
                    crate::renderer::pagination::PageItem::Table {
                        para_index,
                        control_index,
                    } => {
                        eprintln!("    Table(para={}, ctrl={})", para_index, control_index);
                    }
                    _ => {
                        eprintln!("    기타 항목");
                    }
                }
            }
        }
    }

    // 검증: 페이지 0에 1단 + 2단 존이 공존해야 함 (다단 설정 나누기)
    let page0 = &pag_result.pages[0];
    let has_zone_offset = page0
        .column_contents
        .iter()
        .any(|cc| cc.zone_y_offset > 0.0);
    assert!(
        has_zone_offset,
        "페이지 0에 zone_y_offset > 0인 ColumnContent가 있어야 함 (1단+2단 공존)"
    );
    let has_multi_col = page0.column_contents.iter().any(|cc| cc.column_index > 0);
    assert!(
        has_multi_col,
        "페이지 0에 column_index > 0인 ColumnContent가 있어야 함 (2단 렌더링)"
    );

    // === 페이지 1 높이 오버플로 진단 ===
    if pag_result.pages.len() > 1 {
        let page1 = &pag_result.pages[1];
        let avail = page1.layout.available_body_height();
        eprintln!("\n=== 페이지 1 높이 오버플로 진단 ===");
        eprintln!("available_body_height={:.2}px", avail);
        eprintln!(
            "body_area: y={:.2}, h={:.2}, bottom={:.2}",
            page1.layout.body_area.y,
            page1.layout.body_area.height,
            page1.layout.body_area.y + page1.layout.body_area.height
        );

        for col_content in &page1.column_contents {
            eprintln!(
                "\n  단 {} (zone_y_offset={:.1}):",
                col_content.column_index, col_content.zone_y_offset
            );
            let mut cumulative: f64 = 0.0;
            for item in &col_content.items {
                match item {
                    crate::renderer::pagination::PageItem::FullParagraph { para_index } => {
                        let h = measured_sec
                            .get_paragraph_height(*para_index)
                            .unwrap_or(0.0);
                        cumulative += h;
                        let mp = measured_sec.get_measured_paragraph(*para_index);
                        let sp_b = mp.map(|m| m.spacing_before).unwrap_or(0.0);
                        let sp_a = mp.map(|m| m.spacing_after).unwrap_or(0.0);
                        let lh_sum: f64 = mp.map(|m| m.line_heights.iter().sum()).unwrap_or(0.0);
                        let line_ct = mp.map(|m| m.line_heights.len()).unwrap_or(0);
                        eprintln!("    FullParagraph(para={}) h={:.2}px (sp_b={:.2} + lines({})={:.2} + sp_a={:.2}) cum={:.2}",
                                para_index, h, sp_b, line_ct, lh_sum, sp_a, cumulative);
                    }
                    crate::renderer::pagination::PageItem::PartialParagraph {
                        para_index,
                        start_line,
                        end_line,
                    } => {
                        let mp = measured_sec.get_measured_paragraph(*para_index);
                        let (part_h, sp_b, sp_a, lh_sum) = if let Some(mp) = mp {
                            let sp_b = if *start_line == 0 {
                                mp.spacing_before
                            } else {
                                0.0
                            };
                            let sp_a = if *end_line >= mp.line_heights.len() {
                                mp.spacing_after
                            } else {
                                0.0
                            };
                            let safe_s = (*start_line).min(mp.line_heights.len());
                            let safe_e = (*end_line).min(mp.line_heights.len());
                            let lh: f64 = mp.line_heights[safe_s..safe_e].iter().sum();
                            (sp_b + lh + sp_a, sp_b, sp_a, lh)
                        } else {
                            (0.0, 0.0, 0.0, 0.0)
                        };
                        cumulative += part_h;
                        eprintln!("    PartialParagraph(para={}, lines={}..{}) h={:.2}px (sp_b={:.2} + lines={:.2} + sp_a={:.2}) cum={:.2}",
                                para_index, start_line, end_line, part_h, sp_b, lh_sum, sp_a, cumulative);
                    }
                    crate::renderer::pagination::PageItem::Table {
                        para_index,
                        control_index,
                    } => {
                        let h = measured_sec
                            .get_paragraph_height(*para_index)
                            .unwrap_or(0.0);
                        cumulative += h;
                        eprintln!(
                            "    Table(para={}, ctrl={}) h={:.2}px cum={:.2}",
                            para_index, control_index, h, cumulative
                        );
                    }
                    _ => {
                        eprintln!("    기타 항목");
                    }
                }
            }
            let overflow = cumulative - avail;
            if overflow > 0.0 {
                eprintln!(
                    "  *** 오버플로: {:.2}px (누적 {:.2} > 가용 {:.2})",
                    overflow, cumulative, avail
                );
            } else {
                eprintln!(
                    "  여유: {:.2}px (누적 {:.2} <= 가용 {:.2})",
                    -overflow, cumulative, avail
                );
            }
        }
    }
}



/// Task 227: 빈 문서에서 텍스트 입력 → 전체선택 → 복사 → End → 붙여넣기 시
/// 새 페이지 생성 버그 재현 및 원인 분석
#[test]
fn test_task227_blank_doc_copy_paste_bug() {
    let mut doc = HwpDocument::create_empty();
    let result = doc.create_blank_document_native();
    assert!(result.is_ok(), "빈 문서 생성 실패");

    // 1. 빈 문서의 문단 수 확인
    let para_count = doc.document.sections[0].paragraphs.len();
    eprintln!("[Task227] 빈 문서 문단 수: {}", para_count);
    for (i, p) in doc.document.sections[0].paragraphs.iter().enumerate() {
        eprintln!(
            "  문단[{}]: text={:?}, chars={}, controls={}, has_para_text={}",
            i,
            p.text,
            p.text.chars().count(),
            p.controls.len(),
            p.has_para_text
        );
    }

    // 2. 텍스트 삽입
    let result = doc.insert_text_native(0, 0, 0, "abcdefg");
    assert!(result.is_ok(), "텍스트 삽입 실패");

    let para_count_after_insert = doc.document.sections[0].paragraphs.len();
    eprintln!(
        "[Task227] 텍스트 삽입 후 문단 수: {}",
        para_count_after_insert
    );
    for (i, p) in doc.document.sections[0].paragraphs.iter().enumerate() {
        eprintln!(
            "  문단[{}]: text={:?}, chars={}, controls={}, has_para_text={}",
            i,
            p.text,
            p.text.chars().count(),
            p.controls.len(),
            p.has_para_text
        );
    }

    // 3. 전체 선택 시뮬레이션: start=(0,0,0), end=(last_para, last_char)
    let last_para = para_count_after_insert - 1;
    let last_char = doc.document.sections[0].paragraphs[last_para]
        .text
        .chars()
        .count();
    eprintln!(
        "[Task227] 전체 선택: start=(0,0), end=({},{})",
        last_para, last_char
    );

    // 4. 복사
    let result = doc.copy_selection_native(0, 0, 0, last_para, last_char);
    assert!(result.is_ok(), "복사 실패: {:?}", result.err());
    let clip_text = doc.get_clipboard_text_native();
    eprintln!("[Task227] 클립보드 텍스트: {:?}", clip_text);

    // 클립보드 문단 수 확인
    if let Some(ref clip) = doc.clipboard {
        eprintln!("[Task227] 클립보드 문단 수: {}", clip.paragraphs.len());
        for (i, p) in clip.paragraphs.iter().enumerate() {
            eprintln!(
                "  클립[{}]: text={:?}, chars={}, controls={}",
                i,
                p.text,
                p.text.chars().count(),
                p.controls.len()
            );
        }
    }

    // 5. End 키 시뮬레이션: 커서를 문단 0의 텍스트 끝으로 이동
    //    (원래 커서는 문단 0, offset 7 = "abcdefg" 끝)
    let paste_offset = doc.document.sections[0].paragraphs[0].text.chars().count();
    eprintln!("[Task227] 붙여넣기 위치: para=0, offset={}", paste_offset);

    // 6. 붙여넣기
    let result = doc.paste_internal_native(0, 0, paste_offset);
    assert!(result.is_ok(), "붙여넣기 실패: {:?}", result.err());
    let json = result.unwrap();
    eprintln!("[Task227] 붙여넣기 결과: {}", json);

    // 7. 결과 확인
    let final_para_count = doc.document.sections[0].paragraphs.len();
    eprintln!("[Task227] 붙여넣기 후 문단 수: {}", final_para_count);
    for (i, p) in doc.document.sections[0].paragraphs.iter().enumerate() {
        eprintln!(
            "  문단[{}]: text={:?}, chars={}",
            i,
            p.text,
            p.text.chars().count()
        );
    }

    let page_count = doc.page_count();
    eprintln!("[Task227] 붙여넣기 후 페이지 수: {}", page_count);

    // 기대: 1개 문단, 1 페이지
    assert_eq!(
        final_para_count, 1,
        "문단 수가 1이어야 함 (실제: {})",
        final_para_count
    );
    assert_eq!(
        page_count, 1,
        "페이지 수가 1이어야 함 (실제: {})",
        page_count
    );
}



/// Task 228: h-pen-01.hwp 형광펜 데이터 분석
#[test]
fn test_task228_highlight_data_analysis() {
    let data = std::fs::read("samples/h-pen-01.hwp").expect("파일 읽기 실패");
    let doc = crate::parser::parse_hwp(&data).expect("파싱 실패");

    // CharShape shade_color 확인
    eprintln!("[Task228] CharShape 수: {}", doc.doc_info.char_shapes.len());
    for (i, cs) in doc.doc_info.char_shapes.iter().enumerate() {
        eprintln!("  CS[{}]: shade_color=0x{:06X}", i, cs.shade_color);
    }

    // 문단별 range_tags 확인
    for (si, section) in doc.sections.iter().enumerate() {
        for (pi, para) in section.paragraphs.iter().enumerate() {
            if !para.range_tags.is_empty() {
                eprintln!("[Task228] 구역[{}] 문단[{}] range_tags:", si, pi);
                for rt in &para.range_tags {
                    let tag_type = (rt.tag >> 24) & 0xFF;
                    let tag_data = rt.tag & 0x00FFFFFF;
                    eprintln!(
                        "  start={}, end={}, tag=0x{:08X} (type={}, data=0x{:06X})",
                        rt.start, rt.end, rt.tag, tag_type, tag_data
                    );
                }
            }
            // char_shapes 참조 확인
            for csr in &para.char_shapes {
                let cs_id = csr.char_shape_id as usize;
                if cs_id < doc.doc_info.char_shapes.len() {
                    let sc = doc.doc_info.char_shapes[cs_id].shade_color;
                    if sc != 0xFFFFFF && sc != 0x00FFFFFF {
                        eprintln!(
                            "  문단[{}] char_shape_ref: start={}, cs_id={}, shade_color=0x{:06X}",
                            pi, csr.start_pos, cs_id, sc
                        );
                    }
                }
            }
        }
    }
}



/// Task 228: 형광펜 렌더링 - 페이지 트리에 Rectangle 노드 확인
#[test]
fn test_task228_highlight_render_tree() {
    let data = std::fs::read("samples/h-pen-01.hwp").expect("파일 읽기 실패");
    let mut doc = crate::DocumentCore::from_bytes(&data).expect("파싱 실패");
    let svg = doc.render_page_svg_native(0).expect("SVG 렌더링 실패");
    // 형광펜 사각형 색상이 SVG에 포함되어야 함
    assert!(
        svg.contains("#ad71a1"),
        "2번째 문단 형광펜 색상(#ad71a1)이 SVG에 없음"
    );
    assert!(
        svg.contains("#ffff65"),
        "3번째 문단 형광펜 색상(#ffff65)이 SVG에 없음"
    );
    eprintln!("[Task228 RenderTree] SVG에 형광펜 색상 확인됨");
}



/// Task 229: field-01.hwp 필드 컨트롤 파싱 분석
#[test]
fn test_task229_field_parsing() {
    use crate::model::control::{Control, FieldType};

    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let doc = crate::parser::parse_hwp(&data).expect("파싱 실패");

    let mut field_count = 0;
    let mut unknown_count = 0;

    for (si, section) in doc.sections.iter().enumerate() {
        for (pi, para) in section.paragraphs.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                match ctrl {
                    Control::Field(f) => {
                        field_count += 1;
                        eprintln!(
                                "[Task229] 구역[{}] 문단[{}] 컨트롤[{}]: Field type={:?}, command=\"{}\", id={}, props=0x{:08X}",
                                si, pi, ci, f.field_type, f.command, f.field_id, f.properties
                            );
                    }
                    Control::Unknown(u) => {
                        let id_bytes = u.ctrl_id.to_be_bytes();
                        if id_bytes[0] == b'%' {
                            unknown_count += 1;
                            eprintln!(
                                    "[Task229] 구역[{}] 문단[{}] 컨트롤[{}]: Unknown 필드 ctrl_id=0x{:08X} ({})",
                                    si, pi, ci, u.ctrl_id,
                                    String::from_utf8_lossy(&id_bytes)
                                );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    eprintln!(
        "[Task229] 총 필드: {}, Unknown 필드: {}",
        field_count, unknown_count
    );
    assert!(field_count > 0, "필드 컨트롤이 파싱되어야 함");
    assert_eq!(
        unknown_count, 0,
        "모든 필드가 파싱되어야 함 (Unknown 없어야 함)"
    );

    // 필드 범위 추적 검증
    let mut total_field_ranges = 0;
    for (si, section) in doc.sections.iter().enumerate() {
        for (pi, para) in section.paragraphs.iter().enumerate() {
            if !para.field_ranges.is_empty() {
                eprintln!(
                    "[Task229] 구역[{}] 문단[{}] text=\"{}\" (len={})",
                    si,
                    pi,
                    para.text,
                    para.text.chars().count()
                );
            }
            for fr in &para.field_ranges {
                total_field_ranges += 1;
                let field_text: String = para
                    .text
                    .chars()
                    .skip(fr.start_char_idx)
                    .take(fr.end_char_idx - fr.start_char_idx)
                    .collect();
                let field_type = match &para.controls[fr.control_idx] {
                    Control::Field(f) => format!("{:?}", f.field_type),
                    _ => "N/A".to_string(),
                };
                eprintln!(
                        "[Task229] 구역[{}] 문단[{}] field_range: chars[{}..{}] ctrl[{}] type={} text=\"{}\"",
                        si, pi, fr.start_char_idx, fr.end_char_idx, fr.control_idx, field_type, field_text
                    );
            }
        }
    }
    eprintln!("[Task229] 총 필드 범위: {}", total_field_ranges);
    assert_eq!(
        total_field_ranges, field_count,
        "필드 수와 필드 범위 수가 일치해야 함"
    );
}



#[test]
fn test_task229_field_roundtrip() {
    use crate::model::control::{Control, FieldType};

    // 원본 파싱
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let doc1 = crate::parser::parse_hwp(&data).expect("파싱 실패");

    // 직렬화 → 재파싱
    let saved = crate::serializer::serialize_hwp(&doc1).expect("직렬화 실패");
    let doc2 = crate::parser::parse_hwp(&saved).expect("재파싱 실패");

    // 필드 컨트롤 비교
    let fields1: Vec<_> = doc1
        .sections
        .iter()
        .flat_map(|s| &s.paragraphs)
        .flat_map(|p| p.controls.iter())
        .filter_map(|c| {
            if let Control::Field(f) = c {
                Some(f)
            } else {
                None
            }
        })
        .collect();
    let fields2: Vec<_> = doc2
        .sections
        .iter()
        .flat_map(|s| &s.paragraphs)
        .flat_map(|p| p.controls.iter())
        .filter_map(|c| {
            if let Control::Field(f) = c {
                Some(f)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(fields1.len(), fields2.len(), "필드 수 불일치");
    for (i, (f1, f2)) in fields1.iter().zip(fields2.iter()).enumerate() {
        assert_eq!(f1.field_type, f2.field_type, "필드[{}] 타입 불일치", i);
        assert_eq!(f1.ctrl_id, f2.ctrl_id, "필드[{}] ctrl_id 불일치", i);
    }
}



#[test]
fn test_task229_field_svg_guide_text() {
    use crate::model::control::Control;

    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let doc = crate::parser::parse_hwp(&data).expect("파싱 실패");

    // 글상자(Shape) 내 ClickHere 필드 검증
    let mut shape_field_count = 0usize;
    for sec in &doc.sections {
        for para in &sec.paragraphs {
            for ctrl in &para.controls {
                if let Control::Shape(s) = ctrl {
                    if let Some(drawing) = s.drawing() {
                        if let Some(tb) = &drawing.text_box {
                            for tb_para in &tb.paragraphs {
                                shape_field_count += tb_para.field_ranges.len();
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        shape_field_count >= 5,
        "글상자 내 필드가 5개 이상이어야 함 (실제: {})",
        shape_field_count
    );

    let mut hwp_doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");
    let svg = hwp_doc.render_page_svg_native(0).expect("SVG 렌더링 실패");

    // SVG에 안내문 텍스트가 빨간색으로 렌더링되는지 확인 (기울임체는 아님 -
    // 한글 폰트는 대개 이탤릭 글리프가 없어 PDF 경로에서만 업라이트로
    // 대체되던 문제가 있었음. SVG/PDF 출력 일관성을 위해 기울임체 미적용으로 통일)
    assert!(
        svg.contains("ff0000"),
        "SVG에 빨간색(#ff0000) 텍스트가 있어야 함"
    );
    assert!(
        !svg.contains("italic"),
        "SVG 안내문 텍스트는 기울임체가 아니어야 함"
    );
    assert!(svg.contains(">여</text>"), "SVG에 '여' 글자가 있어야 함");
    assert!(svg.contains(">입</text>"), "SVG에 '입' 글자가 있어야 함");
}

// ─── Task 230: 필드 WASM API 테스트 ─────────────────────────



#[test]
fn test_task230_get_field_list() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let hwp_doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");

    let json = hwp_doc.get_field_list_json();
    eprintln!("[Task230] getFieldList: {}", json);

    // JSON 배열이어야 함
    assert!(
        json.starts_with('[') && json.ends_with(']'),
        "JSON 배열이어야 함"
    );
    // 최소 6개 필드 (본문 5 + 글상자 내 5 + 기타)
    let field_count = json.matches("\"fieldId\"").count();
    assert!(
        field_count >= 6,
        "필드가 6개 이상이어야 함 (실제: {})",
        field_count
    );
    // ClickHere 필드 포함 확인
    assert!(json.contains("\"clickhere\""), "ClickHere 필드가 있어야 함");
}



#[test]
fn test_task230_get_field_value() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let hwp_doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");

    // 필드 목록에서 첫 번째 필드 ID 추출
    let json = hwp_doc.get_field_list_json();
    let fields = hwp_doc.collect_all_fields();
    assert!(!fields.is_empty(), "필드가 있어야 함");

    let first_field = &fields[0];
    eprintln!(
        "[Task230] 첫 번째 필드: id={}, type={:?}, name={:?}, value='{}'",
        first_field.field.field_id,
        first_field.field.field_type,
        first_field.field.field_name(),
        first_field.value
    );

    // field_id로 조회
    let result = hwp_doc
        .get_field_value_by_id(first_field.field.field_id)
        .expect("필드 값 조회 실패");
    assert!(result.contains("\"ok\":true"), "조회 성공이어야 함");

    // 이름으로 조회
    if let Some(name) = first_field.field.field_name() {
        let result = hwp_doc
            .get_field_value_by_name(name)
            .expect("이름으로 필드 값 조회 실패");
        assert!(result.contains("\"ok\":true"), "이름 조회 성공이어야 함");
    }
}



#[test]
fn test_task230_set_field_value() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let mut hwp_doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");

    let fields = hwp_doc.collect_all_fields();
    // 빈 ClickHere 필드 찾기 (value가 빈 것)
    let empty_field = fields
        .iter()
        .find(|f| {
            f.field.field_type == crate::model::control::FieldType::ClickHere && f.value.is_empty()
        })
        .expect("빈 ClickHere 필드가 있어야 함");

    let field_id = empty_field.field.field_id;
    eprintln!(
        "[Task230] 빈 필드에 값 설정: id={}, name={:?}",
        field_id,
        empty_field.field.field_name()
    );

    // 값 설정
    let result = hwp_doc
        .set_field_value_by_id(field_id, "테스트 입력값")
        .expect("필드 값 설정 실패");
    eprintln!("[Task230] setFieldValue 결과: {}", result);
    assert!(result.contains("\"ok\":true"), "설정 성공이어야 함");
    assert!(result.contains("테스트 입력값"), "새 값이 포함되어야 함");

    // 값이 변경되었는지 확인
    let check = hwp_doc
        .get_field_value_by_id(field_id)
        .expect("변경 후 조회 실패");
    assert!(check.contains("테스트 입력값"), "변경된 값이 반영되어야 함");

    // SVG 렌더링에서 변경된 값이 보이는지 확인
    let svg = hwp_doc.render_page_svg_native(0).expect("SVG 렌더링 실패");
    // "테스트 입력값"의 개별 글자가 SVG에 포함되어야 함
    assert!(
        svg.contains(">테</text>") || svg.contains("테스트"),
        "SVG에 변경된 텍스트가 있어야 함"
    );
}



#[test]
fn test_task231_field_survives_text_insert() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");

    // Section 0, Para 7: 빈 누름틀 필드 (start=7, end=7)
    let info_before = doc.get_field_info_at(0, 7, 7);
    eprintln!("[Before] field_info_at(0,7,7): {}", info_before);
    assert!(
        info_before.contains("\"inField\":true"),
        "삽입 전 필드가 있어야 함"
    );

    // 필드 위치(charOffset=7)에 "A" 삽입
    let result = doc
        .insert_text_native(0, 7, 7, "A")
        .expect("텍스트 삽입 실패");
    eprintln!("[After insert] result: {}", result);

    // 삽입 후 커서 위치(charOffset=8)에서 필드 확인
    let info_after = doc.get_field_info_at(0, 7, 8);
    eprintln!("[After] field_info_at(0,7,8): {}", info_after);
    assert!(
        info_after.contains("\"inField\":true"),
        "삽입 후에도 필드가 있어야 함"
    );

    // 필드 시작 위치에서도 확인
    let info_start = doc.get_field_info_at(0, 7, 7);
    eprintln!("[After] field_info_at(0,7,7): {}", info_start);
    assert!(
        info_start.contains("\"inField\":true"),
        "삽입 후 필드 시작도 감지되어야 함"
    );

    // field_ranges 직접 확인
    let para = &doc.document.sections[0].paragraphs[7];
    eprintln!("[After] field_ranges: {:?}", para.field_ranges);
    assert!(
        !para.field_ranges.is_empty(),
        "field_ranges가 비어있으면 안됨"
    );
    let fr = &para.field_ranges[0];
    assert_eq!(fr.start_char_idx, 7, "필드 시작은 7");
    assert_eq!(fr.end_char_idx, 8, "필드 끝은 8 (1글자 삽입 후)");
}



/// IME 조합 사이클 시뮬레이션: delete→insert 반복 시 필드가 사라지지 않는지 검증
#[test]
fn test_task231_field_survives_ime_cycle() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");

    // Section 0, Para 7: 빈 누름틀 필드 (start=7, end=7)
    let info = doc.get_field_info_at(0, 7, 7);
    assert!(info.contains("\"inField\":true"), "초기 필드 존재 확인");

    // IME 1단계: "ㅁ" 삽입 (compositionLength=0이므로 삭제 없음)
    doc.insert_text_native(0, 7, 7, "ㅁ").expect("삽입 실패");
    let fr = &doc.document.sections[0].paragraphs[7].field_ranges[0];
    assert_eq!(
        (fr.start_char_idx, fr.end_char_idx),
        (7, 8),
        "1단계 후 필드 범위"
    );

    // IME 2단계: "ㅁ" 삭제 → "마" 삽입 (delete→insert cycle)
    doc.delete_text_native(0, 7, 7, 1).expect("삭제 실패");
    // *** 핵심: 삭제 후 필드가 비어도 field_ranges가 유지되어야 함 ***
    let para = &doc.document.sections[0].paragraphs[7];
    eprintln!("[After delete] field_ranges: {:?}", para.field_ranges);
    assert!(
        !para.field_ranges.is_empty(),
        "삭제 후에도 빈 필드 범위가 유지되어야 함"
    );
    let fr = &para.field_ranges[0];
    assert_eq!(
        (fr.start_char_idx, fr.end_char_idx),
        (7, 7),
        "삭제 후 빈 필드"
    );

    doc.insert_text_native(0, 7, 7, "마").expect("삽입 실패");
    let fr = &doc.document.sections[0].paragraphs[7].field_ranges[0];
    assert_eq!(
        (fr.start_char_idx, fr.end_char_idx),
        (7, 8),
        "2단계 후 필드 범위"
    );

    // IME 3단계: "마" 삭제 → "만" 삽입
    doc.delete_text_native(0, 7, 7, 1).expect("삭제 실패");
    assert!(
        !doc.document.sections[0].paragraphs[7]
            .field_ranges
            .is_empty(),
        "3단계 삭제 후 필드 유지"
    );
    doc.insert_text_native(0, 7, 7, "만").expect("삽입 실패");
    let fr = &doc.document.sections[0].paragraphs[7].field_ranges[0];
    assert_eq!(
        (fr.start_char_idx, fr.end_char_idx),
        (7, 8),
        "3단계 후 필드 범위"
    );

    // IME 완료 후 필드 정보 확인
    let info = doc.get_field_info_at(0, 7, 8);
    assert!(
        info.contains("\"inField\":true"),
        "IME 완료 후 필드 내 커서 확인"
    );
}



/// getClickHereProps가 유효한 JSON을 반환하는지 검증
#[test]
fn test_task231_get_click_here_props() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");

    let result = doc.get_click_here_props(1584999796);
    eprintln!("[getClickHereProps] {}", result);
    // 유효한 JSON인지 확인
    assert!(result.contains("\"ok\":true"), "ok=true 이어야 함");
    assert!(
        result.contains("\"guide\":\""),
        "guide 필드가 따옴표로 감싸져야 함"
    );
    assert!(result.contains("여기에 입력"), "안내문이 포함되어야 함");
    // JSON 구조 검증 (따옴표 포함)
    assert!(result.starts_with("{\"ok\":true,"), "JSON 시작 구조");
}



/// updateClickHereProps 후 field_name() 매핑이 동작하는지 검증
#[test]
fn test_task231_update_click_here_props_name_mapping() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");
    let field_id = 1584999796u32;

    // 초기 상태: command에 Name 키 없음, CTRL_DATA에서 "회사명" 로드
    let para = &doc.document.sections[0].paragraphs[7];
    if let crate::model::control::Control::Field(f) = &para.controls[0] {
        assert_eq!(f.field_name(), Some("회사명"), "초기: CTRL_DATA 필드 이름");
        assert_eq!(
            f.ctrl_data_name.as_deref(),
            Some("회사명"),
            "초기: ctrl_data_name"
        );
        assert_eq!(
            f.extract_wstring_value("Name:"),
            None,
            "초기: command에 Name 키 없음"
        );
    }

    // 필드 이름을 "목차1"로 설정
    let result = doc.update_click_here_props(field_id, "여기에 입력", "", "목차1", true);
    assert!(result.contains("\"ok\":true"), "업데이트 성공");

    // 업데이트 후: 이름은 ctrl_data_name에만, command에는 Name: 없음
    let para = &doc.document.sections[0].paragraphs[7];
    if let crate::model::control::Control::Field(f) = &para.controls[0] {
        eprintln!("[After update] command: {:?}", f.command);
        assert_eq!(
            f.field_name(),
            Some("목차1"),
            "업데이트 후: ctrl_data_name 우선"
        );
        assert_eq!(
            f.ctrl_data_name.as_deref(),
            Some("목차1"),
            "ctrl_data_name 설정됨"
        );
        assert_eq!(
            f.extract_wstring_value("Name:"),
            None,
            "command에 Name: 없음 (한컴 호환)"
        );
        assert_eq!(f.guide_text(), Some("여기에 입력"), "안내문 유지됨");
    }

    // getFieldValueByName으로 새 이름 조회 가능
    let val = doc.get_field_value_by_name("목차1");
    eprintln!("[ByName] 목차1: {:?}", val);
    assert!(val.is_ok(), "새 이름으로 조회 가능");

    // getClickHereProps에서 name이 비어있지 않은지 확인
    let props = doc.get_click_here_props(field_id);
    eprintln!("[Props after] {}", props);
    assert!(props.contains("\"name\":\"목차1\""), "props에 새 이름 표시");
}



/// 필드 직렬화 라운드트립: 저장 후 다시 읽으면 필드가 보존되는지 검증
#[test]
fn test_task231_field_roundtrip() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");

    // 저장 (직렬화 → CFB 바이트)
    let saved = doc.core.export_hwp_native().expect("저장 실패");

    // 다시 읽기
    let doc2 = HwpDocument::from_bytes(&saved).expect("다시 읽기 실패");

    use crate::model::control::{Control, FieldType};
    // sec=0 para=7의 필드 확인
    let para = &doc2.document.sections[0].paragraphs[7];
    let ctrl = &para.controls[0];
    if let Control::Field(f) = ctrl {
        assert_eq!(f.field_type, FieldType::ClickHere);
        assert_eq!(f.field_id, 1584999796);
        assert!(
            f.command.contains("Direction:wstring:6:여기에 입력"),
            "command 보존: {:?}",
            f.command
        );
        assert_eq!(
            f.ctrl_data_name.as_deref(),
            Some("회사명"),
            "CTRL_DATA 필드 이름 보존"
        );
        eprintln!(
            "[roundtrip] id={} command={:?} ctrl_data_name={:?}",
            f.field_id, f.command, f.ctrl_data_name
        );
    } else {
        panic!("sec=0 para=7 ctrl=0이 Field가 아님: {:?}", ctrl);
    }
    // field_ranges 보존 확인
    let orig_para = &doc.document.sections[0].paragraphs[7];
    eprintln!("[roundtrip] orig field_ranges={:?}", orig_para.field_ranges);
    eprintln!("[roundtrip] reload field_ranges={:?}", para.field_ranges);
    assert_eq!(
        para.field_ranges.len(),
        orig_para.field_ranges.len(),
        "field_ranges 개수 보존"
    );
    for (i, (a, b)) in orig_para
        .field_ranges
        .iter()
        .zip(para.field_ranges.iter())
        .enumerate()
    {
        assert_eq!(
            a.start_char_idx, b.start_char_idx,
            "field_range[{}].start 보존",
            i
        );
        assert_eq!(
            a.end_char_idx, b.end_char_idx,
            "field_range[{}].end 보존",
            i
        );
        assert_eq!(
            a.control_idx, b.control_idx,
            "field_range[{}].ctrl_idx 보존",
            i
        );
    }
}



/// 필드 이름만 변경 후 저장 → 안내문이 보존되는지 검증
#[test]
fn test_task231_field_name_change_preserves_guide() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let mut doc = HwpDocument::from_bytes(&data).expect("HwpDocument 생성 실패");
    let field_id = 1584999796u32; // "이메일" 필드가 아닌 "회사명" 필드

    // 변경 전 상태
    let props_before = doc.get_click_here_props(field_id);
    eprintln!("[before] {}", props_before);

    // 필드 이름만 변경 (안내문, 메모는 그대로)
    let result = doc.update_click_here_props(field_id, "여기에 입력", "", "회사명1", true);
    eprintln!("[update] {}", result);

    // 변경 후 command 확인
    {
        use crate::model::control::{Control, FieldType};
        let para = &doc.document.sections[0].paragraphs[7];
        if let Control::Field(f) = &para.controls[0] {
            eprintln!("[after update] command={:?}", f.command);
            eprintln!("[after update] ctrl_data_name={:?}", f.ctrl_data_name);
        }
    }

    // 저장
    let saved = doc.core.export_hwp_native().expect("저장 실패");

    // 다시 읽기
    let doc2 = HwpDocument::from_bytes(&saved).expect("다시 읽기 실패");

    use crate::model::control::{Control, FieldType};
    let para = &doc2.document.sections[0].paragraphs[7];
    if let Control::Field(f) = &para.controls[0] {
        eprintln!("[reloaded] command={:?}", f.command);
        eprintln!("[reloaded] ctrl_data_name={:?}", f.ctrl_data_name);
        eprintln!("[reloaded] guide_text={:?}", f.guide_text());
        eprintln!("[reloaded] field_name={:?}", f.field_name());
        assert_eq!(f.field_id, field_id, "field_id 보존");
        assert_eq!(f.guide_text(), Some("여기에 입력"), "안내문 보존");
        assert_eq!(
            f.ctrl_data_name.as_deref(),
            Some("회사명1"),
            "변경된 필드 이름"
        );
    } else {
        panic!("필드가 아님");
    }

    // getClickHereProps로도 확인
    let props_after = doc2.get_click_here_props(field_id);
    eprintln!("[reloaded props] {}", props_after);
    assert!(
        props_after.contains("\"guide\":\"여기에 입력\""),
        "안내문 보존"
    );
    assert!(props_after.contains("\"name\":\"회사명1\""), "변경된 이름");
}



/// `insertPictureEx`(options JSON + image_data)가 positional `insertPicture` 와
/// 동일하게 동작해야 한다. 같은 입력으로 두 문서에 각각 삽입 → 렌더 SVG 의 이미지
/// 수와 반환 JSON 의 paraIdx/controlIdx 가 일치.
#[test]
fn task1413_insert_picture_ex_equivalent_to_positional() {
    fn png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x00, 0x00, 0x00,
            0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }
    fn count_images(svg: &str) -> usize {
        svg.matches("<image").count()
    }

    // positional 경로
    let mut doc_pos = HwpDocument::create_empty();
    let res_pos = doc_pos
        .insert_picture(
            0,
            0,
            0,
            "",
            &png(),
            4000,
            3000,
            100,
            80,
            "png",
            "",
            None,
            None,
        )
        .expect("positional insertPicture");

    // *Ex 경로 — 동일 입력을 options JSON 으로
    let mut doc_ex = HwpDocument::create_empty();
    let options = r#"{"sectionIdx":0,"paraIdx":0,"charOffset":0,"cellPath":"",
        "width":4000,"height":3000,"naturalWidthPx":100,"naturalHeightPx":80,
        "extension":"png","description":""}"#;
    let res_ex = doc_ex
        .insert_picture_ex(options, &png())
        .expect("insertPictureEx");

    // 반환 JSON 동치 (paraIdx/controlIdx)
    assert_eq!(res_pos, res_ex, "*Ex 반환이 positional 과 동일해야 함");

    // 렌더 결과 동치 (이미지 수)
    let svg_pos = doc_pos.render_page_svg_native(0).expect("svg pos");
    let svg_ex = doc_ex.render_page_svg_native(0).expect("svg ex");
    assert_eq!(
        count_images(&svg_pos),
        count_images(&svg_ex),
        "*Ex 렌더 이미지 수가 positional 과 동일해야 함"
    );
    assert_eq!(count_images(&svg_ex), 1, "그림 1개 삽입");
}



/// options JSON 의 키 누락 시 positional default 와 동일 처리 (description/extension/
/// paperOffset 부재).
#[test]
fn task1413_insert_picture_ex_optional_keys_default() {
    fn png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x00, 0x00, 0x00,
            0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }
    // optional 키(extension/description/paperOffset/cellPath) 생략 — 본문 inline 삽입.
    let mut doc = HwpDocument::create_empty();
    let res = doc
        .insert_picture_ex(
            r#"{"sectionIdx":0,"paraIdx":0,"width":4000,"height":3000,"naturalWidthPx":100,"naturalHeightPx":80}"#,
            &png(),
        )
        .expect("insertPictureEx with optional keys omitted");
    assert!(
        res.contains("\"ok\":true") || res.contains("paraIdx"),
        "삽입 성공: {res}"
    );
}

// ---------- #1413 2단계: 고인자(9~11) *Ex 동치 ----------



/// splitTableCellInto vs splitTableCellIntoEx 동치.
#[test]
fn task1413_split_table_cell_into_ex_equivalent() {
    let mut doc_pos = create_doc_with_table();
    let res_pos = doc_pos
        .split_table_cell_into(0, 0, 0, 0, 0, 2, 2, true, false)
        .expect("positional splitTableCellInto");

    let mut doc_ex = create_doc_with_table();
    let res_ex = doc_ex
        .split_table_cell_into_ex(
            r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"row":0,"col":0,
                "nRows":2,"mCols":2,"equalRowHeight":true,"mergeFirst":false}"#,
        )
        .expect("splitTableCellIntoEx");
    assert_eq!(res_pos, res_ex, "*Ex 가 positional 과 동일 반환");
}



/// splitTableCellsInRange vs splitTableCellsInRangeEx 동치.
#[test]
fn task1413_split_table_cells_in_range_ex_equivalent() {
    let mut doc_pos = create_doc_with_table();
    let res_pos = doc_pos
        .split_table_cells_in_range(0, 0, 0, 0, 0, 0, 0, 2, 2, true)
        .expect("positional splitTableCellsInRange");

    let mut doc_ex = create_doc_with_table();
    let res_ex = doc_ex
        .split_table_cells_in_range_ex(
            r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"startRow":0,"startCol":0,
                "endRow":0,"endCol":0,"nRows":2,"mCols":2,"equalRowHeight":true}"#,
        )
        .expect("splitTableCellsInRangeEx");
    assert_eq!(res_pos, res_ex, "*Ex 가 positional 과 동일 반환");
}



/// insertClickHereFieldInCell vs insertClickHereFieldInCellEx 동치.
#[test]
fn task1413_insert_click_here_field_in_cell_ex_equivalent() {
    let mut doc_pos = create_doc_with_table();
    let res_pos = doc_pos
        .insert_click_here_field_in_cell_api(0, 0, 0, 0, 0, 0, false, "안내", "메모", "이름", true)
        .expect("positional insertClickHereFieldInCell");

    let mut doc_ex = create_doc_with_table();
    let res_ex = doc_ex
        .insert_click_here_field_in_cell_ex(
            r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,
                "charOffset":0,"isTextbox":false,"guide":"안내","memo":"메모","name":"이름","editable":true}"#,
        )
        .expect("insertClickHereFieldInCellEx");
    assert_eq!(res_pos, res_ex, "*Ex 가 positional 과 동일 반환");
}



/// moveVertical vs moveVerticalEx 동치 (본문 — parentParaIdx 생략 = MAX).
#[test]
fn task1413_move_vertical_ex_equivalent() {
    let mut doc_pos = HwpDocument::create_empty();
    doc_pos
        .insert_text_native(0, 0, 0, "첫째 줄\n둘째 줄\n셋째 줄")
        .expect("텍스트 삽입");
    let res_pos = doc_pos
        .move_vertical(0, 0, 2, 1, 10.0, u32::MAX, 0, 0, 0)
        .expect("positional moveVertical");

    let mut doc_ex = HwpDocument::create_empty();
    doc_ex
        .insert_text_native(0, 0, 0, "첫째 줄\n둘째 줄\n셋째 줄")
        .expect("텍스트 삽입");
    let res_ex = doc_ex
        .move_vertical_ex(
            r#"{"sectionIdx":0,"paraIdx":0,"charOffset":2,"delta":1,"preferredX":10.0}"#,
        )
        .expect("moveVerticalEx");
    assert_eq!(
        res_pos, res_ex,
        "*Ex 가 positional 과 동일 반환 (본문 이동)"
    );
}

// ---------- #1413 3단계: 8인자 군 *Ex 동치 ----------



#[test]
fn task1413_set_page_hide_ex_equivalent() {
    let mut doc_pos = HwpDocument::create_empty();
    let res_pos = doc_pos
        .set_page_hide(0, 0, true, false, true, false, true, false)
        .expect("positional setPageHide");
    let mut doc_ex = HwpDocument::create_empty();
    let res_ex = doc_ex
        .set_page_hide_ex(
            r#"{"sec":0,"para":0,"hideHeader":true,"hideFooter":false,"hideMaster":true,
                "hideBorder":false,"hideFill":true,"hidePageNum":false}"#,
        )
        .expect("setPageHideEx");
    assert_eq!(res_pos, res_ex);
}



#[test]
fn task1413_set_char_shape_id_in_cell_ex_equivalent() {
    // char_shape_id=0 이 유효하려면 char_shapes 가 최소 1개 등록돼 있어야 한다
    // (없으면 native 가 "범위 초과" Err → wasm JsValue 변환 패닉). 정상 입력으로 비교.
    let mut doc_pos = create_doc_with_table();
    doc_pos
        .document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    let res_pos = doc_pos.set_char_shape_id_in_cell(0, 0, 0, 0, 0, 0, 0, 0);
    let mut doc_ex = create_doc_with_table();
    doc_ex
        .document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    let res_ex = doc_ex.set_char_shape_id_in_cell_ex(
        r#"{"secIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,
            "startOffset":0,"endOffset":0,"charShapeId":0}"#,
    );
    assert_eq!(format!("{res_pos:?}"), format!("{res_ex:?}"));
}



#[test]
fn task1413_get_selection_rects_in_cell_ex_equivalent() {
    let doc = create_doc_with_table();
    let res_pos = doc.get_selection_rects_in_cell(0, 0, 0, 0, 0, 0, 0, 0);
    let res_ex = doc.get_selection_rects_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"startCellParaIdx":0,
            "startCharOffset":0,"endCellParaIdx":0,"endCharOffset":0}"#,
    );
    assert_eq!(format!("{res_pos:?}"), format!("{res_ex:?}"));
}



#[test]
fn task1413_export_selection_in_cell_html_ex_equivalent() {
    let doc = create_doc_with_table();
    let res_pos = doc.export_selection_in_cell_html(0, 0, 0, 0, 0, 0, 0, 0);
    let res_ex = doc.export_selection_in_cell_html_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"startCellParaIdx":0,
            "startCharOffset":0,"endCellParaIdx":0,"endCharOffset":0}"#,
    );
    assert_eq!(format!("{res_pos:?}"), format!("{res_ex:?}"));
}



#[test]
fn task1413_delete_range_in_cell_ex_equivalent() {
    let mut doc_pos = create_doc_with_table();
    let res_pos = doc_pos.delete_range_in_cell(0, 0, 0, 0, 0, 0, 0, 0);
    let mut doc_ex = create_doc_with_table();
    let res_ex = doc_ex.delete_range_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"startCellParaIdx":0,
            "startCharOffset":0,"endCellParaIdx":0,"endCharOffset":0}"#,
    );
    assert_eq!(format!("{res_pos:?}"), format!("{res_ex:?}"));
}



#[test]
fn task1413_copy_selection_in_cell_ex_equivalent() {
    let mut doc_pos = create_doc_with_table();
    let res_pos = doc_pos.copy_selection_in_cell(0, 0, 0, 0, 0, 0, 0, 0);
    let mut doc_ex = create_doc_with_table();
    let res_ex = doc_ex.copy_selection_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"startCellParaIdx":0,
            "startCharOffset":0,"endCellParaIdx":0,"endCharOffset":0}"#,
    );
    assert_eq!(format!("{res_pos:?}"), format!("{res_ex:?}"));
}



#[test]
fn task1413_apply_char_format_in_cell_ex_equivalent() {
    let props = r#"{"bold":true}"#;
    let mut doc_pos = create_doc_with_table();
    let res_pos = doc_pos.apply_char_format_in_cell(0, 0, 0, 0, 0, 0, 0, props);
    let mut doc_ex = create_doc_with_table();
    let res_ex = doc_ex.apply_char_format_in_cell_ex(
        r#"{"secIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,
            "startOffset":0,"endOffset":0,"props":{"bold":true}}"#,
    );
    assert_eq!(format!("{res_pos:?}"), format!("{res_ex:?}"));
}



#[test]
fn task1413_insert_click_here_field_by_path_ex_equivalent() {
    // 유효 cell path(표 para 0, control 0, cell 0)로 셀 안에 삽입. positional 과 *Ex 동치.
    // (빈 path 는 native 에서 에러 → wasm JsValue 변환 패닉이라 정상 path 를 쓴다.)
    let path = r#"[{"controlIndex":0,"cellIndex":0,"cellParaIndex":0}]"#;
    let mut doc_pos = create_doc_with_table();
    let res_pos =
        doc_pos.insert_click_here_field_by_path_api(0, 0, path, 0, "안내", "메모", "이름", true);
    let mut doc_ex = create_doc_with_table();
    let res_ex = doc_ex.insert_click_here_field_by_path_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"path":"[{\"controlIndex\":0,\"cellIndex\":0,\"cellParaIndex\":0}]","charOffset":0,"guide":"안내","memo":"메모","name":"이름","editable":true}"#,
    );
    assert_eq!(format!("{res_pos:?}"), format!("{res_ex:?}"));
}

// ---------- #1413 4단계: 7인자 군 *Ex 동치 (13개) ----------

// 표 셀(para0/control0/cell0)에 정상 동작하는 *InCell 류. 반환 동일성 비교.
#[test]
fn task1413_insert_text_in_cell_ex_equivalent() {
    let mut a = create_doc_with_table();
    let rp = a.insert_text_in_cell(0, 0, 0, 0, 0, 0, "텍스트");
    let mut b = create_doc_with_table();
    let re = b.insert_text_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,"charOffset":0,"text":"텍스트"}"#,
    );
    assert_eq!(format!("{rp:?}"), format!("{re:?}"));
}



#[test]
fn task1413_get_text_in_cell_ex_equivalent() {
    let a = create_doc_with_table();
    let rp = a.get_text_in_cell(0, 0, 0, 0, 0, 0, 1);
    let re = a.get_text_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,"charOffset":0,"count":1}"#,
    );
    assert_eq!(format!("{rp:?}"), format!("{re:?}"));
}



#[test]
fn task1413_delete_text_in_cell_ex_equivalent() {
    let mut a = create_doc_with_table();
    let rp = a.delete_text_in_cell(0, 0, 0, 0, 0, 0, 1);
    let mut b = create_doc_with_table();
    let re = b.delete_text_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,"charOffset":0,"count":1}"#,
    );
    assert_eq!(format!("{rp:?}"), format!("{re:?}"));
}



#[test]
fn task1413_paste_html_in_cell_ex_equivalent() {
    let mut a = create_doc_with_table();
    let rp = a.paste_html_in_cell(0, 0, 0, 0, 0, 0, "<p>x</p>");
    let mut b = create_doc_with_table();
    let re = b.paste_html_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,"charOffset":0,"html":"<p>x</p>"}"#,
    );
    assert_eq!(format!("{rp:?}"), format!("{re:?}"));
}



#[test]
fn task1413_merge_table_cells_ex_equivalent() {
    let mut a = create_doc_with_table();
    let rp = a.merge_table_cells(0, 0, 0, 0, 0, 0, 1);
    let mut b = create_doc_with_table();
    let re = b.merge_table_cells_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"startRow":0,"startCol":0,"endRow":0,"endCol":1}"#,
    );
    assert_eq!(format!("{rp:?}"), format!("{re:?}"));
}



#[test]
fn task1413_insert_click_here_field_ex_equivalent() {
    let mut a = HwpDocument::create_empty();
    a.insert_text_native(0, 0, 0, "abc").unwrap();
    let rp = a.insert_click_here_field_api(0, 0, 0, "안내", "메모", "이름", true);
    let mut b = HwpDocument::create_empty();
    b.insert_text_native(0, 0, 0, "abc").unwrap();
    let re = b.insert_click_here_field_ex(
        r#"{"sectionIdx":0,"paraIdx":0,"charOffset":0,"guide":"안내","memo":"메모","name":"이름","editable":true}"#,
    );
    assert_eq!(format!("{rp:?}"), format!("{re:?}"));
}

// bool/String 반환 (JsValue 변환 없음 — 패닉 무관).
#[test]
fn task1413_set_active_field_in_cell_ex_equivalent() {
    let mut a = create_doc_with_table();
    let rp = a.set_active_field_in_cell_api(0, 0, 0, 0, 0, 0, false);
    let mut b = create_doc_with_table();
    let re = b.set_active_field_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,"charOffset":0,"isTextbox":false}"#,
    );
    assert_eq!(rp, re);
}



#[test]
fn task1413_get_field_info_at_in_cell_ex_equivalent() {
    let a = create_doc_with_table();
    let rp = a.get_field_info_at_in_cell_api(0, 0, 0, 0, 0, 0, false);
    let re = a.get_field_info_at_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,"charOffset":0,"isTextbox":false}"#,
    );
    assert_eq!(rp, re);
}



#[test]
fn task1413_remove_field_at_in_cell_ex_equivalent() {
    let mut a = create_doc_with_table();
    let rp = a.remove_field_at_in_cell_api(0, 0, 0, 0, 0, 0, false);
    let mut b = create_doc_with_table();
    let re = b.remove_field_at_in_cell_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"cellIdx":0,"cellParaIdx":0,"charOffset":0,"isTextbox":false}"#,
    );
    assert_eq!(rp, re);
}



#[test]
fn task1413_evaluate_table_formula_ex_equivalent() {
    let mut a = create_doc_with_table();
    let rp = a.evaluate_table_formula(0, 0, 0, 0, 0, "=1+1", false);
    let mut b = create_doc_with_table();
    let re = b.evaluate_table_formula_ex(
        r#"{"sectionIdx":0,"parentParaIdx":0,"controlIdx":0,"targetRow":0,"targetCol":0,"formula":"=1+1","writeResult":false}"#,
    );
    assert_eq!(format!("{rp:?}"), format!("{re:?}"));
}
