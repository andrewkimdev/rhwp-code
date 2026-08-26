//! picture_tests — tests/mod.rs 에서 무변동 이동
use super::*;



#[test]
fn test_analyze_reference_picture() {
    use crate::model::control::Control;
    use crate::parser::cfb_reader::LenientCfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let path = "output/pic-01-as-text.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  이미지 참조 파일 분석: {}", path);
    eprintln!("  파일 크기: {} bytes", data.len());
    eprintln!("{}", "=".repeat(60));

    // 표준 파서 시도, 실패 시 LenientCfbReader
    let doc = match HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  표준 파서 실패 ({}), LenientCfbReader로 분석합니다.", e);
            let lcfb = LenientCfbReader::open(&data).unwrap();

            eprintln!("\n  [LenientCFB 엔트리 목록]");
            for (name, start, size, otype) in lcfb.list_entries() {
                let tname = match otype {
                    1 => "storage",
                    2 => "stream",
                    5 => "root",
                    _ => "?",
                };
                eprintln!(
                    "  {:30} start={:5} size={:8} type={}",
                    name, start, size, tname
                );
            }

            // FileHeader
            let fh = lcfb.read_stream("FileHeader").unwrap();
            let compressed = fh.len() >= 37 && (fh[36] & 0x01) != 0;
            eprintln!(
                "\n  FileHeader: {} bytes, compressed={}",
                fh.len(),
                compressed
            );

            // DocInfo
            let di_data = lcfb.read_doc_info(compressed).unwrap();
            let di_recs = Record::read_all(&di_data).unwrap();
            eprintln!(
                "  DocInfo: {} bytes → {} 레코드",
                di_data.len(),
                di_recs.len()
            );

            // DocProperties (캐럿 위치)
            if let Some(dp_rec) = di_recs.first() {
                if dp_rec.tag_id == tags::HWPTAG_DOCUMENT_PROPERTIES && dp_rec.data.len() >= 26 {
                    let d = &dp_rec.data;
                    let caret_list_id = u32::from_le_bytes([d[14], d[15], d[16], d[17]]);
                    let caret_para_id = u32::from_le_bytes([d[18], d[19], d[20], d[21]]);
                    let caret_char_pos = u32::from_le_bytes([d[22], d[23], d[24], d[25]]);
                    eprintln!("\n  [캐럿 위치 (raw)]");
                    eprintln!("  caret_list_id:  {}", caret_list_id);
                    eprintln!("  caret_para_id:  {}", caret_para_id);
                    eprintln!("  caret_char_pos: {}", caret_char_pos);
                }
            }

            // ID_MAPPINGS
            if di_recs.len() > 1 && di_recs[1].tag_id == tags::HWPTAG_ID_MAPPINGS {
                let d = &di_recs[1].data;
                if d.len() >= 72 {
                    eprintln!("\n  [ID_MAPPINGS]");
                    let labels = [
                        "bin_data",
                        "font_kr",
                        "font_en",
                        "font_cn",
                        "font_jp",
                        "font_etc",
                        "font_sym",
                        "font_usr",
                        "border_fill",
                        "char_shape",
                        "tab_def",
                        "numbering",
                        "bullet",
                        "para_shape",
                        "style",
                        "memo_shape",
                        "trackchange",
                        "trackchange_author",
                    ];
                    for (i, label) in labels.iter().enumerate() {
                        let off = i * 4;
                        let val = u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]]);
                        if val > 0 {
                            eprintln!("  {:20}: {}", label, val);
                        }
                    }
                }
            }

            // BIN_DATA 레코드 덤프
            eprintln!("\n  [DocInfo BIN_DATA 레코드]");
            for (i, r) in di_recs.iter().enumerate() {
                if r.tag_id == tags::HWPTAG_BIN_DATA {
                    eprintln!(
                        "  [{:2}] BIN_DATA size={} data: {:02x?}",
                        i,
                        r.data.len(),
                        &r.data[..r.data.len().min(60)]
                    );
                }
            }

            // BodyText
            let bt_data = lcfb.read_body_text_section(0, compressed).unwrap();
            let bt_recs = Record::read_all(&bt_data).unwrap();

            eprintln!("\n  [BodyText 레코드 덤프] ({} 개)", bt_recs.len());
            for (i, r) in bt_recs.iter().enumerate() {
                let tname = tags::tag_name(r.tag_id);
                let mut extra = String::new();
                if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
                    let cid = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                    extra = format!(" ctrl='{}'", tags::ctrl_name(cid));
                }
                eprintln!(
                    "  [{:2}] tag={:3}({:25}) level={} size={}{}",
                    i,
                    r.tag_id,
                    tname,
                    r.level,
                    r.data.len(),
                    extra
                );
                // 주요 레코드 데이터 덤프
                if matches!(
                    r.tag_id,
                    66 | 67 | 68 | 69 | 71 | // PARA_HEADER, TEXT, CHAR_SHAPE, LINE_SEG, CTRL_HEADER
                        76 | 79 // SHAPE_COMPONENT, SHAPE_COMPONENT_PICTURE
                ) {
                    let show = r.data.len().min(100);
                    eprintln!("        data[..{}]: {:02x?}", show, &r.data[..show]);
                    if r.data.len() > 100 {
                        eprintln!("        total: {} bytes", r.data.len());
                    }
                }
            }

            // BinData 스트림 확인
            eprintln!("\n  [BinData 스트림]");
            for (name, _start, size, otype) in lcfb.list_entries() {
                if *otype == 2 && name.contains("BIN") {
                    eprintln!("  {} size={}", name, size);
                    if let Ok(stream) = lcfb.read_stream(&name) {
                        let sig_show = stream.len().min(16);
                        eprintln!("    sig[..{}]: {:02x?}", sig_show, &stream[..sig_show]);
                    }
                }
            }

            // empty.hwp 비교
            let empty_path = "template/empty.hwp";
            if std::path::Path::new(empty_path).exists() {
                let empty_data = std::fs::read(empty_path).unwrap();
                let empty_parsed = crate::parser::parse_hwp(&empty_data).unwrap();
                let mut empty_cfb =
                    crate::parser::cfb_reader::CfbReader::open(&empty_data).unwrap();
                let empty_bt = empty_cfb
                    .read_body_text_section(0, empty_parsed.header.compressed, false)
                    .unwrap();
                let empty_recs = Record::read_all(&empty_bt).unwrap();
                eprintln!(
                    "\n  [비교] empty.hwp={} 개, pic-01.hwp={} 개 → 차이={} 개",
                    empty_recs.len(),
                    bt_recs.len(),
                    bt_recs.len() as i32 - empty_recs.len() as i32
                );
            }

            eprintln!("\n=== 참조 파일 분석 완료 (LenientCfbReader) ===");
            return;
        }
    };

    // === 표준 파서 성공 경로 ===

    // 1. 캐럿 위치
    let dp = &doc.document.doc_properties;
    eprintln!("\n  [캐럿 위치]");
    eprintln!("  caret_list_id:  {}", dp.caret_list_id);
    eprintln!("  caret_para_id:  {}", dp.caret_para_id);
    eprintln!("  caret_char_pos: {}", dp.caret_char_pos);

    // 2. BinData 목록
    eprintln!(
        "\n  [BinData 목록] ({} 개)",
        doc.document.doc_info.bin_data_list.len()
    );
    for (i, bd) in doc.document.doc_info.bin_data_list.iter().enumerate() {
        eprintln!(
            "  [{}] attr=0x{:04X} type={:?} storage_id={} ext={:?} compression={:?} status={:?}",
            i, bd.attr, bd.data_type, bd.storage_id, bd.extension, bd.compression, bd.status
        );
        if let Some(ref raw) = bd.raw_data {
            let show = raw.len().min(60);
            eprintln!(
                "       raw_data({} bytes): {:02x?}",
                raw.len(),
                &raw[..show]
            );
        }
    }

    // 3. BinDataContent 목록
    eprintln!(
        "\n  [BinDataContent 목록] ({} 개)",
        doc.document.bin_data_content.len()
    );
    for (i, bc) in doc.document.bin_data_content.iter().enumerate() {
        eprintln!(
            "  [{}] id={} ext='{}' data_size={} bytes",
            i,
            bc.id,
            bc.extension,
            bc.data.len()
        );
        if bc.data.len() >= 8 {
            let sig = &bc.data.load()[..8];
            let format = if sig[0..2] == [0xFF, 0xD8] {
                "JPEG"
            } else if sig[0..4] == [0x89, 0x50, 0x4E, 0x47] {
                "PNG"
            } else if sig[0..2] == [0x42, 0x4D] {
                "BMP"
            } else if sig[0..4] == [0x47, 0x49, 0x46, 0x38] {
                "GIF"
            } else {
                "Unknown"
            };
            eprintln!("       시그니처: {:02x?} → {}", &sig[..4], format);
        }
    }

    // 4. 섹션/문단 구조 상세
    for (si, sec) in doc.document.sections.iter().enumerate() {
        eprintln!("\n  [섹션 {}] 문단 수: {}", si, sec.paragraphs.len());
        for (pi, para) in sec.paragraphs.iter().enumerate() {
            eprintln!(
                "  문단[{}]: text='{}' char_count={} msb={} controls={} char_offsets={:?}",
                pi,
                para.text,
                para.char_count,
                para.char_count_msb,
                para.controls.len(),
                para.char_offsets
            );
            eprintln!(
                "    control_mask=0x{:08X} para_shape_id={} style_id={}",
                para.control_mask, para.para_shape_id, para.style_id
            );
            eprintln!("    char_shapes: {:?}", para.char_shapes);
            eprintln!("    line_segs: {:?}", para.line_segs);
            eprintln!(
                "    raw_header_extra({} bytes): {:02x?}",
                para.raw_header_extra.len(),
                &para.raw_header_extra[..para.raw_header_extra.len().min(30)]
            );

            for (ci, ctrl) in para.controls.iter().enumerate() {
                match ctrl {
                    Control::SectionDef(_) => eprintln!("    ctrl[{}]: SectionDef", ci),
                    Control::ColumnDef(_) => eprintln!("    ctrl[{}]: ColumnDef", ci),
                    Control::Picture(pic) => {
                        eprintln!("    ctrl[{}]: Picture", ci);
                        eprintln!("      CommonObjAttr:");
                        eprintln!("        ctrl_id: 0x{:08X}", pic.common.ctrl_id);
                        eprintln!("        attr: 0x{:08X}", pic.common.attr);
                        eprintln!("        vertical_offset: {}", pic.common.vertical_offset);
                        eprintln!(
                            "        horizontal_offset: {}",
                            pic.common.horizontal_offset
                        );
                        eprintln!("        width: {}", pic.common.width);
                        eprintln!("        height: {}", pic.common.height);
                        eprintln!("        z_order: {}", pic.common.z_order);
                        eprintln!(
                            "        margin: L={} R={} T={} B={}",
                            pic.common.margin.left,
                            pic.common.margin.right,
                            pic.common.margin.top,
                            pic.common.margin.bottom
                        );
                        eprintln!("        instance_id: 0x{:08X}", pic.common.instance_id);
                        eprintln!("        description: '{}'", pic.common.description);
                        eprintln!(
                            "        raw_extra({} bytes): {:02x?}",
                            pic.common.raw_extra.len(),
                            &pic.common.raw_extra[..pic.common.raw_extra.len().min(40)]
                        );
                        eprintln!("      ShapeComponentAttr:");
                        eprintln!("        ctrl_id: 0x{:08X}", pic.shape_attr.ctrl_id);
                        eprintln!("        is_two_ctrl_id: {}", pic.shape_attr.is_two_ctrl_id);
                        eprintln!(
                            "        offset: ({}, {})",
                            pic.shape_attr.offset_x, pic.shape_attr.offset_y
                        );
                        eprintln!("        group_level: {}", pic.shape_attr.group_level);
                        eprintln!(
                            "        local_file_version: {}",
                            pic.shape_attr.local_file_version
                        );
                        eprintln!(
                            "        original: {}×{}",
                            pic.shape_attr.original_width, pic.shape_attr.original_height
                        );
                        eprintln!(
                            "        current: {}×{}",
                            pic.shape_attr.current_width, pic.shape_attr.current_height
                        );
                        eprintln!("        flip: 0x{:08X}", pic.shape_attr.flip);
                        eprintln!("        rotation_angle: {}", pic.shape_attr.rotation_angle);
                        eprintln!(
                            "        raw_rendering({} bytes): {:02x?}",
                            pic.shape_attr.raw_rendering.len(),
                            &pic.shape_attr.raw_rendering
                                [..pic.shape_attr.raw_rendering.len().min(80)]
                        );
                        eprintln!("      PictureData:");
                        eprintln!("        border_color: 0x{:08X}", pic.border_color);
                        eprintln!("        border_width: {}", pic.border_width);
                        eprintln!("        border_x: {:?}", pic.border_x);
                        eprintln!("        border_y: {:?}", pic.border_y);
                        eprintln!(
                            "        crop: L={} T={} R={} B={}",
                            pic.crop.left, pic.crop.top, pic.crop.right, pic.crop.bottom
                        );
                        eprintln!(
                            "        padding: L={} R={} T={} B={}",
                            pic.padding.left,
                            pic.padding.right,
                            pic.padding.top,
                            pic.padding.bottom
                        );
                        eprintln!("        image_attr: brightness={} contrast={} effect={:?} bin_data_id={}",
                                pic.image_attr.brightness, pic.image_attr.contrast, pic.image_attr.effect, pic.image_attr.bin_data_id);
                        eprintln!("        border_opacity: {}", pic.border_opacity);
                        eprintln!("        instance_id: {}", pic.instance_id);
                        eprintln!(
                            "        raw_picture_extra({} bytes): {:02x?}",
                            pic.raw_picture_extra.len(),
                            &pic.raw_picture_extra[..pic.raw_picture_extra.len().min(40)]
                        );
                    }
                    _ => eprintln!("    ctrl[{}]: {:?}", ci, std::mem::discriminant(ctrl)),
                }
            }
        }
    }

    // 5. BodyText 레코드 덤프
    let parsed_doc = crate::parser::parse_hwp(&data).unwrap();
    let mut cfb = crate::parser::cfb_reader::CfbReader::open(&data).unwrap();
    let bt_data = cfb
        .read_body_text_section(0, parsed_doc.header.compressed, false)
        .unwrap();
    let bt_recs = Record::read_all(&bt_data).unwrap();

    eprintln!("\n  [BodyText 레코드 덤프] ({} 개)", bt_recs.len());
    for (i, r) in bt_recs.iter().enumerate() {
        let tname = tags::tag_name(r.tag_id);
        let mut extra = String::new();
        if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let cid = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            extra = format!(" ctrl='{}'", tags::ctrl_name(cid));
        }
        eprintln!(
            "  [{:2}] tag={:3}({:25}) level={} size={}{}",
            i,
            r.tag_id,
            tname,
            r.level,
            r.data.len(),
            extra
        );
        // 주요 레코드 데이터 상세 덤프
        if matches!(
            r.tag_id,
            66 | 67 | 68 | 69 | 71 | // PARA_HEADER, TEXT, CHAR_SHAPE, LINE_SEG, CTRL_HEADER
                76 | 85 // SHAPE_COMPONENT, SHAPE_COMPONENT_PICTURE (tag 85)
        ) {
            let show = r.data.len().min(120);
            eprintln!("        data[..{}]: {:02x?}", show, &r.data[..show]);
            if r.data.len() > 120 {
                eprintln!("        total: {} bytes", r.data.len());
            }
        }
    }

    // 6. empty.hwp 비교
    let empty_path = "template/empty.hwp";
    if std::path::Path::new(empty_path).exists() {
        let empty_data = std::fs::read(empty_path).unwrap();
        let empty_parsed = crate::parser::parse_hwp(&empty_data).unwrap();
        let mut empty_cfb = crate::parser::cfb_reader::CfbReader::open(&empty_data).unwrap();
        let empty_bt = empty_cfb
            .read_body_text_section(0, empty_parsed.header.compressed, false)
            .unwrap();
        let empty_recs = Record::read_all(&empty_bt).unwrap();
        eprintln!(
            "\n  [비교] empty.hwp={} 개, pic-01.hwp={} 개 → 차이={} 개",
            empty_recs.len(),
            bt_recs.len(),
            bt_recs.len() as i32 - empty_recs.len() as i32
        );
    }

    // 7. DocInfo 상세
    eprintln!("\n  [DocInfo]");
    eprintln!(
        "  bin_data_count: {}",
        doc.document.doc_info.bin_data_list.len()
    );
    eprintln!(
        "  border_fill_count: {}",
        doc.document.doc_info.border_fills.len()
    );
    eprintln!(
        "  char_shape_count: {}",
        doc.document.doc_info.char_shapes.len()
    );
    eprintln!(
        "  para_shape_count: {}",
        doc.document.doc_info.para_shapes.len()
    );

    // 8. CFB 스트림 목록
    let streams = cfb.list_streams();
    eprintln!("\n  [CFB 스트림 목록]");
    for s in &streams {
        eprintln!("  {}", s);
    }

    eprintln!("\n=== 이미지 참조 파일 분석 완료 ===");
}



#[test]
fn test_hy001_textbox_inline_pictures_render_for_hwp_and_hwpx() {
    use crate::model::shape::ShapeObject;
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    fn collect_image_positions(node: &RenderNode, out: &mut Vec<(u16, f64)>) {
        if let RenderNodeType::Image(img) = &node.node_type {
            out.push((img.bin_data_id, node.bbox.x));
        }
        for child in &node.children {
            collect_image_positions(child, out);
        }
    }

    fn assert_textbox_picture_roundtrip(path: &str) {
        if !std::path::Path::new(path).exists() {
            eprintln!("SKIP: {} 없음", path);
            return;
        }

        let data = std::fs::read(path).unwrap();
        let doc = HwpDocument::from_bytes(&data).unwrap();
        let para = &doc.document.sections[0].paragraphs[27];
        let shape = para
            .controls
            .iter()
            .find_map(|ctrl| match ctrl {
                Control::Shape(shape) => Some(shape.as_ref()),
                _ => None,
            })
            .expect("hy-001 paragraph 27 should contain a shape control");

        let text_box = match shape {
            ShapeObject::Rectangle(rect) => rect.drawing.text_box.as_ref(),
            _ => None,
        }
        .expect("hy-001 shape should contain a text box");

        let textbox_picture_ids: Vec<u16> = text_box
            .paragraphs
            .iter()
            .flat_map(|p| &p.controls)
            .filter_map(|ctrl| match ctrl {
                Control::Picture(pic) => Some(pic.image_attr.bin_data_id),
                _ => None,
            })
            .collect();
        assert_eq!(
            textbox_picture_ids,
            vec![2, 3],
            "{}: text box should keep picture controls for BinData 2 and 3",
            path
        );

        let tree = doc.build_page_tree(1).unwrap();
        let mut rendered_images = Vec::new();
        collect_image_positions(&tree.root, &mut rendered_images);
        let rendered_image_ids: Vec<u16> = rendered_images.iter().map(|(id, _)| *id).collect();
        assert!(
            rendered_image_ids.contains(&2) && rendered_image_ids.contains(&3),
            "{}: page 2 render tree should contain text box images BinData 2 and 3 (actual: {:?})",
            path,
            rendered_image_ids
        );

        let image2_x = rendered_images
            .iter()
            .find_map(|(id, x)| (*id == 2).then_some(*x))
            .expect("BinData 2 should be rendered");
        let image3_x = rendered_images
            .iter()
            .find_map(|(id, x)| (*id == 3).then_some(*x))
            .expect("BinData 3 should be rendered");
        let gap = image3_x - image2_x;
        assert!(
                (525.0..=550.0).contains(&gap),
                "{}: text box TAC pictures should preserve Hancom-width space advance between controls (x2={:.1}, x3={:.1}, gap={:.1})",
                path,
                image2_x,
                image3_x,
                gap
            );
    }

    assert_textbox_picture_roundtrip("samples/hwpx/hancom-hwp/hy-001.hwp");
    assert_textbox_picture_roundtrip("samples/hwpx/hy-001.hwpx");
}



#[test]
fn test_hy002_textbox_non_tac_picture_keeps_declared_size() {
    use crate::renderer::render_tree::{RenderNode, RenderNodeType};

    fn collect_images(node: &RenderNode, out: &mut Vec<(u16, f64, f64)>) {
        if let RenderNodeType::Image(img) = &node.node_type {
            out.push((img.bin_data_id, node.bbox.width, node.bbox.height));
        }
        for child in &node.children {
            collect_images(child, out);
        }
    }

    fn assert_textbox_picture_size(path: &str) {
        if !std::path::Path::new(path).exists() {
            eprintln!("SKIP: {} 없음", path);
            return;
        }

        let data = std::fs::read(path).unwrap();
        let doc = HwpDocument::from_bytes(&data).unwrap();
        let tree = doc.build_page_tree(1).unwrap();

        let mut images = Vec::new();
        collect_images(&tree.root, &mut images);
        let image2 = images
            .iter()
            .find(|(id, width, height)| *id == 2 && *width > 600.0 && *height > 50.0);

        assert!(
            image2.is_some(),
            "{}: text box non-TAC image should keep declared display size near 642x58px (actual: {:?})",
            path,
            images
        );
    }

    assert_textbox_picture_size("samples/hwpx/hancom-hwp/hy-002.hwp");
    assert_textbox_picture_size("samples/hwpx/hy-002.hwpx");
}
