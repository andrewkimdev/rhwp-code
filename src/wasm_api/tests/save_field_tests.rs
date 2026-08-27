//! save_field_tests — tests/mod.rs 에서 무변동 이동
use super::*;

/// 사용자 저장 파일 vs 프로그래밍적 병합 파일 비교
#[test]
fn test_compare_user_saved_vs_programmatic() {
    use crate::parser::record::Record;
    use std::path::Path;

    let orig_path = Path::new("samples/hwp_table_test.hwp");
    let saved_path = Path::new("samples/hwp_table_test_saved.hwp");
    if !orig_path.exists() || !saved_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();

    // 프로그래밍적 병합 내보내기
    let mut merged_doc = HwpDocument::from_bytes(&orig_data).unwrap();
    merged_doc
        .merge_table_cells_native(0, 3, 0, 2, 0, 2, 1)
        .unwrap();
    let prog_data = merged_doc.export_hwp_native().unwrap();

    // 사용자 저장 파일 BodyText
    let saved_parsed = crate::parser::parse_hwp(&saved_data).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_parsed.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    // 프로그래밍적 병합 BodyText
    let prog_parsed = crate::parser::parse_hwp(&prog_data).unwrap();
    let mut prog_cfb = crate::parser::cfb_reader::CfbReader::open(&prog_data).unwrap();
    let prog_bt = prog_cfb
        .read_body_text_section(0, prog_parsed.header.compressed, false)
        .unwrap();
    let prog_recs = Record::read_all(&prog_bt).unwrap();

    eprintln!(
        "사용자 저장: {} recs, 프로그래밍: {} recs",
        saved_recs.len(),
        prog_recs.len()
    );
    eprintln!(
        "사용자 저장 파일: {}B, 프로그래밍 파일: {}B",
        saved_data.len(),
        prog_data.len()
    );

    // 모든 레코드 비교
    let max_recs = saved_recs.len().max(prog_recs.len());
    let mut diffs = 0;
    for i in 0..max_recs {
        if i >= saved_recs.len() {
            eprintln!(
                "!! [{}] 사용자에 없음, 프로그래밍: {} L{}",
                i,
                crate::parser::tags::tag_name(prog_recs[i].tag_id),
                prog_recs[i].level
            );
            diffs += 1;
            continue;
        }
        if i >= prog_recs.len() {
            eprintln!(
                "!! [{}] 프로그래밍에 없음, 사용자: {} L{}",
                i,
                crate::parser::tags::tag_name(saved_recs[i].tag_id),
                saved_recs[i].level
            );
            diffs += 1;
            continue;
        }
        let s = &saved_recs[i];
        let p = &prog_recs[i];
        if s.tag_id != p.tag_id || s.level != p.level || s.data != p.data {
            let stag = crate::parser::tags::tag_name(s.tag_id);
            let ptag = crate::parser::tags::tag_name(p.tag_id);
            eprintln!("!! [{}] 차이:", i);
            eprintln!(
                "  사용자: {} L{} {}B {:02X?}",
                stag,
                s.level,
                s.data.len(),
                &s.data[..s.data.len().min(50)]
            );
            eprintln!(
                "  프로그: {} L{} {}B {:02X?}",
                ptag,
                p.level,
                p.data.len(),
                &p.data[..p.data.len().min(50)]
            );
            diffs += 1;
        }
    }
    eprintln!("총 차이: {} 레코드", diffs);

    // DocInfo 비교
    let mut saved_cfb2 = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_di = saved_cfb2
        .read_doc_info(saved_parsed.header.compressed)
        .unwrap();
    let mut prog_cfb2 = crate::parser::cfb_reader::CfbReader::open(&prog_data).unwrap();
    let prog_di = prog_cfb2
        .read_doc_info(prog_parsed.header.compressed)
        .unwrap();
    if saved_di == prog_di {
        eprintln!("\nDocInfo: 동일 ({}B)", saved_di.len());
    } else {
        eprintln!(
            "\n!! DocInfo 차이: 사용자={}B, 프로그래밍={}B",
            saved_di.len(),
            prog_di.len()
        );
    }

    // BodyText raw bytes 비교
    if saved_bt == prog_bt {
        eprintln!("BodyText raw: 동일 ({}B)", saved_bt.len());
    } else {
        eprintln!(
            "!! BodyText raw 차이: 사용자={}B, 프로그래밍={}B",
            saved_bt.len(),
            prog_bt.len()
        );
        let mut byte_diffs = 0;
        for i in 0..saved_bt.len().min(prog_bt.len()) {
            if saved_bt[i] != prog_bt[i] {
                if byte_diffs < 10 {
                    eprintln!("  offset {}: {:02X} vs {:02X}", i, saved_bt[i], prog_bt[i]);
                }
                byte_diffs += 1;
            }
        }
        eprintln!(
            "  총 바이트 차이: {} (+ 길이 차이: {})",
            byte_diffs,
            (saved_bt.len() as i64 - prog_bt.len() as i64).abs()
        );
    }

    // 전체 CFB 파일 비교 (원본 라운드트립 vs 사용자 저장)
    let mut baseline_doc = HwpDocument::from_bytes(&orig_data).unwrap();
    let baseline_data = baseline_doc.export_hwp_native().unwrap();
    eprintln!(
        "\n원본 라운드트립: {}B, 사용자 저장: {}B",
        baseline_data.len(),
        saved_data.len()
    );

    // 프로그래밍적 병합 파일 디스크에 저장 (수동 확인용)
    let out_dir = Path::new("output");
    if out_dir.exists() {
        std::fs::write(out_dir.join("merge_test_programmatic.hwp"), &prog_data).unwrap();
        std::fs::write(out_dir.join("merge_test_baseline.hwp"), &baseline_data).unwrap();
        eprintln!("\n저장 완료:");
        eprintln!("  output/merge_test_baseline.hwp  (수정 없이 라운드트립)");
        eprintln!("  output/merge_test_programmatic.hwp  (프로그래밍적 병합)");
    }
}



/// 재직렬화 vs 원본: BodyText 레코드 상세 비교 (편집 시나리오)
#[test]
fn test_web_saved_vs_original_detailed() {
    use crate::parser::record::Record;
    use std::path::Path;

    let orig_path = Path::new("samples/20250130-hongbo.hwp");
    if !orig_path.exists() {
        eprintln!("SKIP: 파일 없음");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();

    // 현재 코드로 재직렬화 (raw_stream 제거 = 편집 시나리오)
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();
    doc.document.sections[0].raw_stream = None;
    let saved_data = doc.export_hwp_native().unwrap();
    eprintln!(
        "원본: {} bytes, 재직렬화: {} bytes",
        orig_data.len(),
        saved_data.len()
    );

    // CFB 스트림 목록 비교
    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();

    let orig_streams = orig_cfb.list_streams();
    let saved_streams = saved_cfb.list_streams();
    eprintln!("\n=== CFB 스트림 ===");
    eprintln!("원본: {:?}", orig_streams);
    eprintln!("저장: {:?}", saved_streams);

    // FileHeader 비교
    let orig_hdr = orig_cfb.read_file_header().unwrap();
    let saved_hdr = saved_cfb.read_file_header().unwrap();
    if orig_hdr != saved_hdr {
        eprintln!("\n=== FileHeader 차이 ===");
        for i in 0..orig_hdr.len().min(saved_hdr.len()) {
            if orig_hdr[i] != saved_hdr[i] {
                eprintln!("  offset {}: {:02X} → {:02X}", i, orig_hdr[i], saved_hdr[i]);
            }
        }
    } else {
        eprintln!("\nFileHeader: 동일");
    }

    // DocInfo 비교
    let orig_di = orig_cfb.read_doc_info(true).unwrap();
    let saved_di = saved_cfb.read_doc_info(true).unwrap();
    let orig_di_recs = Record::read_all(&orig_di).unwrap();
    let saved_di_recs = Record::read_all(&saved_di).unwrap();
    eprintln!("\n=== DocInfo ===");
    eprintln!(
        "원본: {} recs ({}B), 저장: {} recs ({}B)",
        orig_di_recs.len(),
        orig_di.len(),
        saved_di_recs.len(),
        saved_di.len()
    );

    // DocInfo 레코드별 비교
    let max_di = orig_di_recs.len().max(saved_di_recs.len());
    let mut di_diffs = 0;
    for i in 0..max_di {
        let o = orig_di_recs.get(i);
        let s = saved_di_recs.get(i);
        match (o, s) {
            (Some(or), Some(sr)) => {
                if or.tag_id != sr.tag_id || or.data != sr.data {
                    if di_diffs < 20 {
                        eprintln!(
                            "  DocInfo[{}] 차이: {} ({}B) vs {} ({}B)",
                            i,
                            crate::parser::tags::tag_name(or.tag_id),
                            or.data.len(),
                            crate::parser::tags::tag_name(sr.tag_id),
                            sr.data.len()
                        );
                    }
                    di_diffs += 1;
                }
            }
            (Some(or), None) => {
                eprintln!(
                    "  DocInfo[{}] 원본만: {} ({}B)",
                    i,
                    crate::parser::tags::tag_name(or.tag_id),
                    or.data.len()
                );
                di_diffs += 1;
            }
            (None, Some(sr)) => {
                eprintln!(
                    "  DocInfo[{}] 저장만: {} ({}B)",
                    i,
                    crate::parser::tags::tag_name(sr.tag_id),
                    sr.data.len()
                );
                di_diffs += 1;
            }
            _ => {}
        }
    }
    eprintln!("  DocInfo 차이 레코드 수: {}", di_diffs);

    // BodyText Section0 비교
    let orig_bt = orig_cfb.read_body_text_section(0, true, false).unwrap();
    let saved_bt = saved_cfb.read_body_text_section(0, true, false).unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();
    eprintln!("\n=== BodyText Section0 ===");
    eprintln!(
        "원본: {} recs ({}B), 저장: {} recs ({}B)",
        orig_recs.len(),
        orig_bt.len(),
        saved_recs.len(),
        saved_bt.len()
    );

    // 레코드별 비교
    let max_bt = orig_recs.len().max(saved_recs.len());
    let mut bt_diffs = 0;
    for i in 0..max_bt {
        let o = orig_recs.get(i);
        let s = saved_recs.get(i);
        match (o, s) {
            (Some(or), Some(sr)) => {
                if or.tag_id != sr.tag_id || or.level != sr.level || or.data != sr.data {
                    if bt_diffs < 30 {
                        let tag_same = or.tag_id == sr.tag_id;
                        let data_len_diff = or.data.len() as i64 - sr.data.len() as i64;
                        eprintln!(
                            "  BT[{}] 차이: {} L{} ({}B) vs {} L{} ({}B) tag_same={} data_diff={}",
                            i,
                            crate::parser::tags::tag_name(or.tag_id),
                            or.level,
                            or.data.len(),
                            crate::parser::tags::tag_name(sr.tag_id),
                            sr.level,
                            sr.data.len(),
                            tag_same,
                            data_len_diff
                        );
                        // 같은 태그인데 데이터만 다른 경우 바이트 비교
                        if tag_same && or.data.len() == sr.data.len() && or.data.len() <= 100 {
                            for j in 0..or.data.len() {
                                if or.data[j] != sr.data[j] {
                                    eprintln!(
                                        "    byte[{}]: {:02X} → {:02X}",
                                        j, or.data[j], sr.data[j]
                                    );
                                }
                            }
                        }
                    }
                    bt_diffs += 1;
                }
            }
            (Some(or), None) => {
                if bt_diffs < 30 {
                    eprintln!(
                        "  BT[{}] 원본만: {} L{} ({}B)",
                        i,
                        crate::parser::tags::tag_name(or.tag_id),
                        or.level,
                        or.data.len()
                    );
                }
                bt_diffs += 1;
            }
            (None, Some(sr)) => {
                if bt_diffs < 30 {
                    eprintln!(
                        "  BT[{}] 저장만: {} L{} ({}B)",
                        i,
                        crate::parser::tags::tag_name(sr.tag_id),
                        sr.level,
                        sr.data.len()
                    );
                }
                bt_diffs += 1;
            }
            _ => {}
        }
    }
    eprintln!("  BodyText 차이 레코드 수: {}", bt_diffs);

    // BinData 스트림 비교
    eprintln!("\n=== BinData 스트림 비교 ===");
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();
    eprintln!(
        "원본 bin_data_content: {} 항목",
        orig_doc.bin_data_content.len()
    );
    eprintln!(
        "저장 bin_data_content: {} 항목",
        saved_doc.bin_data_content.len()
    );
    for bc in &orig_doc.bin_data_content {
        let saved_bc = saved_doc.bin_data_content.iter().find(|c| c.id == bc.id);
        match saved_bc {
            Some(sbc) => {
                if bc.data.len() == sbc.data.len() && bc.data.load() == sbc.data.load() {
                    eprintln!(
                        "  ID {}: 동일 ({}B, ext={})",
                        bc.id,
                        bc.data.len(),
                        bc.extension
                    );
                } else {
                    eprintln!(
                        "  ID {}: 크기 차이! 원본={}B, 저장={}B",
                        bc.id,
                        bc.data.len(),
                        sbc.data.len()
                    );
                }
            }
            None => {
                eprintln!("  ID {}: 저장본에 없음!", bc.id);
            }
        }
    }
}

// =====================================================================
// 클립보드 테스트
// =====================================================================



/// 우리 편집기 저장 파일의 직렬화→재파싱 라운드트립 검증
#[test]
fn test_roundtrip_saved_file() {
    use crate::model::control::Control;
    use crate::parser::body_text::parse_body_text_section;
    use crate::serializer::body_text::serialize_section;

    let path = "/app/pasts/20250130-hongbo_saved-past-005.hwp";
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("File not found: {}", path);
            return;
        }
    };
    let doc = HwpDocument::from_bytes(&data).unwrap();

    for (si, section) in doc.document.sections.iter().enumerate() {
        eprintln!("\n=== Section {} ===", si);
        eprintln!("  Total paragraphs: {}", section.paragraphs.len());

        // 각 문단의 기본 정보 출력
        for (pi, para) in section.paragraphs.iter().enumerate() {
            let ctrl_types: Vec<String> = para
                .controls
                .iter()
                .map(|c| match c {
                    Control::Table(t) => format!("Table({}x{})", t.row_count, t.col_count),
                    Control::Picture(_) => "Picture".to_string(),
                    Control::Shape(_) => "Shape".to_string(),
                    Control::SectionDef(_) => "SectionDef".to_string(),
                    Control::ColumnDef(_) => "ColumnDef".to_string(),
                    _ => "Other".to_string(),
                })
                .collect();
            if !para.controls.is_empty() || para.text.is_empty() {
                eprintln!("  para[{}]: text={:?} chars={} ctrl_mask=0x{:08X} controls={:?} char_count={} msb={}",
                        pi, &para.text.chars().take(40).collect::<String>(),
                        para.text.len(), para.control_mask, ctrl_types,
                        para.char_count, para.char_count_msb);
            }
        }

        // 직렬화 → 재파싱
        let serialized = serialize_section(section);
        eprintln!("\n  Serialized section {} = {} bytes", si, serialized.len());

        match parse_body_text_section(&serialized) {
            Ok(reparsed) => {
                eprintln!("  Re-parsed: {} paragraphs", reparsed.paragraphs.len());

                if reparsed.paragraphs.len() != section.paragraphs.len() {
                    eprintln!(
                        "  *** MISMATCH: original {} vs reparsed {} paragraphs ***",
                        section.paragraphs.len(),
                        reparsed.paragraphs.len()
                    );
                }

                // 각 문단 비교
                for pi in 0..section.paragraphs.len().min(reparsed.paragraphs.len()) {
                    let orig = &section.paragraphs[pi];
                    let repr = &reparsed.paragraphs[pi];

                    let mut diffs = Vec::new();
                    if orig.char_count != repr.char_count {
                        diffs.push(format!(
                            "char_count: {}→{}",
                            orig.char_count, repr.char_count
                        ));
                    }
                    if orig.control_mask != repr.control_mask {
                        diffs.push(format!(
                            "control_mask: 0x{:08X}→0x{:08X}",
                            orig.control_mask, repr.control_mask
                        ));
                    }
                    if orig.controls.len() != repr.controls.len() {
                        diffs.push(format!(
                            "controls.len: {}→{}",
                            orig.controls.len(),
                            repr.controls.len()
                        ));
                    }
                    if orig.text != repr.text {
                        diffs.push(format!("text differs"));
                    }

                    if !diffs.is_empty() {
                        eprintln!("  *** para[{}] DIFFS: {} ***", pi, diffs.join(", "));
                    }
                }
            }
            Err(e) => {
                eprintln!("  *** RE-PARSE FAILED: {} ***", e);
            }
        }
    }

    // DocInfo 라운드트립 검증
    eprintln!("\n=== DocInfo Check ===");
    eprintln!(
        "  raw_stream present: {}",
        doc.document.doc_info.raw_stream.is_some()
    );
    eprintln!(
        "  char_shapes count: {}",
        doc.document.doc_info.char_shapes.len()
    );
    eprintln!(
        "  para_shapes count: {}",
        doc.document.doc_info.para_shapes.len()
    );
    eprintln!(
        "  border_fills count: {}",
        doc.document.doc_info.border_fills.len()
    );

    // 모든 셀 문단의 para_shape_id/char_shape_id 범위 검증
    let max_ps = doc.document.doc_info.para_shapes.len();
    let max_cs = doc.document.doc_info.char_shapes.len();
    let max_bf = doc.document.doc_info.border_fills.len();
    for (si, section) in doc.document.sections.iter().enumerate() {
        for (pi, para) in section.paragraphs.iter().enumerate() {
            if para.para_shape_id as usize >= max_ps {
                eprintln!(
                    "  *** INVALID para[{}] para_shape_id={} >= max {} ***",
                    pi, para.para_shape_id, max_ps
                );
            }
            for cs in &para.char_shapes {
                if cs.char_shape_id as usize >= max_cs {
                    eprintln!(
                        "  *** INVALID para[{}] char_shape_id={} >= max {} ***",
                        pi, cs.char_shape_id, max_cs
                    );
                }
            }
            // 셀 문단도 검사
            for ctrl in &para.controls {
                if let Control::Table(tbl) = ctrl {
                    for (ci, cell) in tbl.cells.iter().enumerate() {
                        if cell.border_fill_id as usize > max_bf {
                            eprintln!("  *** INVALID table para[{}] cell[{}] border_fill_id={} > max {} ***",
                                    pi, ci, cell.border_fill_id, max_bf);
                        }
                        for (cpi, cp) in cell.paragraphs.iter().enumerate() {
                            if cp.para_shape_id as usize >= max_ps {
                                eprintln!("  *** INVALID table para[{}] cell[{}] cp[{}] para_shape_id={} >= max {} ***",
                                        pi, ci, cpi, cp.para_shape_id, max_ps);
                            }
                            for cs in &cp.char_shapes {
                                if cs.char_shape_id as usize >= max_cs {
                                    eprintln!("  *** INVALID table para[{}] cell[{}] cp[{}] char_shape_id={} >= max {} ***",
                                            pi, ci, cpi, cs.char_shape_id, max_cs);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // DocInfo 직렬화→재파싱 라운드트립
    let serialized_di = crate::serializer::doc_info::serialize_doc_info(
        &doc.document.doc_info,
        &doc.document.doc_properties,
    );
    eprintln!("  Serialized DocInfo = {} bytes", serialized_di.len());
    match crate::parser::doc_info::parse_doc_info(&serialized_di) {
        Ok((reparsed_di, _)) => {
            eprintln!(
                "  Re-parsed DocInfo: char_shapes={} para_shapes={} border_fills={}",
                reparsed_di.char_shapes.len(),
                reparsed_di.para_shapes.len(),
                reparsed_di.border_fills.len()
            );
            if reparsed_di.char_shapes.len() != doc.document.doc_info.char_shapes.len() {
                eprintln!(
                    "  *** CHAR_SHAPES MISMATCH: {} vs {} ***",
                    doc.document.doc_info.char_shapes.len(),
                    reparsed_di.char_shapes.len()
                );
            }
            if reparsed_di.para_shapes.len() != doc.document.doc_info.para_shapes.len() {
                eprintln!(
                    "  *** PARA_SHAPES MISMATCH: {} vs {} ***",
                    doc.document.doc_info.para_shapes.len(),
                    reparsed_di.para_shapes.len()
                );
            }
            if reparsed_di.border_fills.len() != doc.document.doc_info.border_fills.len() {
                eprintln!(
                    "  *** BORDER_FILLS MISMATCH: {} vs {} ***",
                    doc.document.doc_info.border_fills.len(),
                    reparsed_di.border_fills.len()
                );
            }
        }
        Err(e) => {
            eprintln!("  *** DocInfo RE-PARSE FAILED: {} ***", e);
        }
    }
}



/// p2 (표 1개 붙여넣기) vs p3 (표 2개 붙여넣기) DocInfo 비교
#[test]
fn test_docinfo_comparison_p2_p3() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let files = [
        ("/app/pasts/20250130-hongbo-p2.hwp", "P2 (1 table pasted)"),
        ("/app/pasts/20250130-hongbo-p3.hwp", "P3 (2 tables pasted)"),
        ("/app/pasts/20250130-hongbo_saved-past-006.hwp", "OURS-006"),
    ];

    // 각 파일의 DocInfo 레코드를 비교
    let mut all_info: Vec<(String, Vec<(u16, u16, u32, Vec<u8>)>)> = Vec::new();

    for (path, label) in &files {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("File not found: {}", path);
                continue;
            }
        };

        eprintln!("\n{}", "=".repeat(100));
        eprintln!("=== {} : {} ===", label, path);

        let doc = HwpDocument::from_bytes(&data).unwrap();
        let di = &doc.document.doc_info;

        eprintln!("  char_shapes:   {}", di.char_shapes.len());
        eprintln!("  para_shapes:   {}", di.para_shapes.len());
        eprintln!("  border_fills:  {}", di.border_fills.len());
        eprintln!("  bin_data_list: {}", di.bin_data_list.len());
        eprintln!("  styles:        {}", di.styles.len());
        eprintln!("  tab_defs:      {}", di.tab_defs.len());
        eprintln!("  numberings:    {}", di.numberings.len());
        eprintln!("  font_faces:    {} groups", di.font_faces.len());
        for (fi, ff) in di.font_faces.iter().enumerate() {
            if !ff.is_empty() {
                eprintln!("    font_faces[{}]: {} fonts", fi, ff.len());
            }
        }

        // ID_MAPPINGS: DocInfo 레코드 레벨에서 직접 비교
        let mut cfb = CfbReader::open(&data).unwrap();
        let di_data = cfb.read_doc_info(true).unwrap();
        let records = Record::read_all(&di_data).unwrap();

        eprintln!("\n  DocInfo records: {}", records.len());

        // ID_MAPPINGS 레코드 찾기 (HWPTAG_ID_MAPPINGS = HWPTAG_BEGIN + 2 = 18)
        let id_mappings_tag = 16 + 2; // HWPTAG_BEGIN(16) + 2
        for rec in &records {
            if rec.tag_id == id_mappings_tag {
                eprintln!("\n  ID_MAPPINGS record (size={}):", rec.data.len());
                let count = rec.data.len() / 4;
                let labels = [
                    "BinData",
                    "KorFont",
                    "EnFont",
                    "CnFont",
                    "JpFont",
                    "OtherFont",
                    "SymFont",
                    "UsrFont",
                    "BorderFill",
                    "CharShape",
                    "TabDef",
                    "Numbering",
                    "Bullet",
                    "ParaShape",
                    "Style",
                    "MemoShape",
                    "TrackChange",
                    "TrackChangeUser",
                ];
                for i in 0..count.min(18) {
                    let off = i * 4;
                    if off + 4 <= rec.data.len() {
                        let val = u32::from_le_bytes([
                            rec.data[off],
                            rec.data[off + 1],
                            rec.data[off + 2],
                            rec.data[off + 3],
                        ]);
                        let name = if i < labels.len() { labels[i] } else { "???" };
                        eprintln!("    [{:2}] {:16} = {}", i, name, val);
                    }
                }
            }
        }

        // DocInfo 레코드 시퀀스 요약
        let mut rec_summary: std::collections::HashMap<u16, (usize, usize)> =
            std::collections::HashMap::new();
        for rec in &records {
            let entry = rec_summary.entry(rec.tag_id).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += rec.data.len();
        }
        let mut sorted: Vec<_> = rec_summary.iter().collect();
        sorted.sort_by_key(|(tid, _)| **tid);
        eprintln!("\n  DocInfo record types:");
        for (tid, (cnt, total_size)) in &sorted {
            eprintln!(
                "    tag={:3} ({:20}) count={:4} total_bytes={}",
                tid,
                tags::tag_name(**tid),
                cnt,
                total_size
            );
        }

        // 레코드 리스트 저장
        let rec_list: Vec<_> = records
            .iter()
            .map(|r| (r.tag_id, r.level, r.size, r.data.clone()))
            .collect();
        all_info.push((label.to_string(), rec_list));
    }

    // P2 vs P3 DocInfo 레코드 차이 출력
    if all_info.len() >= 2 {
        let (lbl_a, recs_a) = &all_info[0];
        let (lbl_b, recs_b) = &all_info[1];
        eprintln!("\n{}", "=".repeat(100));
        eprintln!("=== DIFF: {} vs {} ===", lbl_a, lbl_b);
        eprintln!(
            "  {} has {} records, {} has {} records",
            lbl_a,
            recs_a.len(),
            lbl_b,
            recs_b.len()
        );

        let max_len = recs_a.len().max(recs_b.len());
        for i in 0..max_len {
            let a = recs_a.get(i);
            let b = recs_b.get(i);
            match (a, b) {
                (Some(a), Some(b)) => {
                    if a.0 != b.0 || a.2 != b.2 || a.3 != b.3 {
                        eprintln!(
                            "  DIFF rec #{}: {} tag={}/size={} vs {} tag={}/size={}",
                            i,
                            tags::tag_name(a.0),
                            a.0,
                            a.3.len(),
                            tags::tag_name(b.0),
                            b.0,
                            b.3.len()
                        );
                        if a.0 == b.0 && a.3.len() == b.3.len() && a.3.len() <= 256 {
                            // 동일 크기면 바이트 단위 차이 출력
                            for j in 0..a.3.len() {
                                if a.3[j] != b.3[j] {
                                    eprintln!("    byte[{}]: {:02X} vs {:02X}", j, a.3[j], b.3[j]);
                                }
                            }
                        }
                        if a.0 != b.0 || a.3.len() != b.3.len() {
                            // 완전히 다른 레코드면 hex dump
                            if a.3.len() <= 64 {
                                let hex_a: Vec<String> =
                                    a.3.iter().map(|b| format!("{:02X}", b)).collect();
                                eprintln!("    A: {}", hex_a.join(" "));
                            }
                            if b.3.len() <= 64 {
                                let hex_b: Vec<String> =
                                    b.3.iter().map(|b| format!("{:02X}", b)).collect();
                                eprintln!("    B: {}", hex_b.join(" "));
                            }
                        }
                    }
                }
                (Some(a), None) => {
                    eprintln!(
                        "  ONLY-IN-{}: rec #{} tag={} size={}",
                        lbl_a,
                        i,
                        tags::tag_name(a.0),
                        a.3.len()
                    );
                }
                (None, Some(b)) => {
                    eprintln!(
                        "  ONLY-IN-{}: rec #{} tag={} size={}",
                        lbl_b,
                        i,
                        tags::tag_name(b.0),
                        b.3.len()
                    );
                }
                _ => {}
            }
        }
    }

    eprintln!("\n=== DOCINFO COMPARISON COMPLETE ===");
}



/// DocInfo 라운드트립 테스트: raw_stream 제거 후 직렬화→재파싱 시 데이터 보존 검증
#[test]
fn test_docinfo_roundtrip_charshape_preservation() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    // 먼저 모든 관련 파일의 char_shapes 수 출력
    let check_files = [
        "/app/pasts/20250130-hongbo_saved-past.hwp",
        "/app/pasts/20250130-hongbo_saved-past-002.hwp",
        "/app/pasts/20250130-hongbo_saved-past-003.hwp",
        "/app/pasts/20250130-hongbo_saved-past-004.hwp",
        "/app/pasts/20250130-hongbo_saved-past-005.hwp",
        "/app/pasts/20250130-hongbo-p2.hwp",
        "/app/pasts/20250130-hongbo-p3.hwp",
    ];
    eprintln!("\n=== ALL FILES: char_shapes count ===");
    for cf in &check_files {
        if let Ok(d) = std::fs::read(cf) {
            if let Ok(cdoc) = HwpDocument::from_bytes(&d) {
                eprintln!(
                    "  {} → char_shapes={} para_shapes={} border_fills={} styles={}",
                    cf.split('/').next_back().unwrap_or(cf),
                    cdoc.document.doc_info.char_shapes.len(),
                    cdoc.document.doc_info.para_shapes.len(),
                    cdoc.document.doc_info.border_fills.len(),
                    cdoc.document.doc_info.styles.len()
                );
            }
        }
    }

    let path = "/app/pasts/20250130-hongbo-p2.hwp";
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("File not found: {}", path);
            return;
        }
    };

    let mut doc = HwpDocument::from_bytes(&data).unwrap();

    let orig_cs = doc.document.doc_info.char_shapes.len();
    let orig_ps = doc.document.doc_info.para_shapes.len();
    let orig_bf = doc.document.doc_info.border_fills.len();
    let orig_st = doc.document.doc_info.styles.len();

    eprintln!("=== P2 DocInfo 라운드트립 테스트 ===");
    eprintln!(
        "  Original: char_shapes={} para_shapes={} border_fills={} styles={}",
        orig_cs, orig_ps, orig_bf, orig_st
    );
    eprintln!(
        "  raw_stream present: {}",
        doc.document.doc_info.raw_stream.is_some()
    );

    // 1) raw_stream이 있는 경우 → 원본 그대로 반환
    let serialized_raw = crate::serializer::doc_info::serialize_doc_info(
        &doc.document.doc_info,
        &doc.document.doc_properties,
    );
    let raw_records = Record::read_all(&serialized_raw).unwrap();
    let raw_cs_count = raw_records
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_CHAR_SHAPE)
        .count();
    eprintln!(
        "  With raw_stream: serialized={} bytes, CHAR_SHAPE records={}",
        serialized_raw.len(),
        raw_cs_count
    );

    // 2) raw_stream 제거 후 재직렬화
    doc.document.doc_info.raw_stream = None;
    let serialized_no_raw = crate::serializer::doc_info::serialize_doc_info(
        &doc.document.doc_info,
        &doc.document.doc_properties,
    );
    let no_raw_records = Record::read_all(&serialized_no_raw).unwrap();
    let no_raw_cs_count = no_raw_records
        .iter()
        .filter(|r| r.tag_id == tags::HWPTAG_CHAR_SHAPE)
        .count();
    eprintln!(
        "  Without raw_stream: serialized={} bytes, CHAR_SHAPE records={}",
        serialized_no_raw.len(),
        no_raw_cs_count
    );

    // 3) 재파싱
    match crate::parser::doc_info::parse_doc_info(&serialized_no_raw) {
        Ok((reparsed_di, reparsed_dp)) => {
            eprintln!(
                "  Re-parsed: char_shapes={} para_shapes={} border_fills={} styles={}",
                reparsed_di.char_shapes.len(),
                reparsed_di.para_shapes.len(),
                reparsed_di.border_fills.len(),
                reparsed_di.styles.len()
            );

            // 원본과 비교
            if reparsed_di.char_shapes.len() != orig_cs {
                eprintln!(
                    "  *** CHAR_SHAPES LOSS: {} → {} (lost {}) ***",
                    orig_cs,
                    reparsed_di.char_shapes.len(),
                    orig_cs as i64 - reparsed_di.char_shapes.len() as i64
                );
            }
            if reparsed_di.para_shapes.len() != orig_ps {
                eprintln!(
                    "  *** PARA_SHAPES DIFF: {} → {} ***",
                    orig_ps,
                    reparsed_di.para_shapes.len()
                );
            }
            if reparsed_di.border_fills.len() != orig_bf {
                eprintln!(
                    "  *** BORDER_FILLS DIFF: {} → {} ***",
                    orig_bf,
                    reparsed_di.border_fills.len()
                );
            }
            if reparsed_di.styles.len() != orig_st {
                eprintln!(
                    "  *** STYLES DIFF: {} → {} ***",
                    orig_st,
                    reparsed_di.styles.len()
                );
            }

            assert_eq!(
                reparsed_di.char_shapes.len(),
                orig_cs,
                "char_shapes 라운드트립 불일치!"
            );
        }
        Err(e) => {
            eprintln!("  *** RE-PARSE FAILED: {} ***", e);
            panic!("DocInfo re-parse failed");
        }
    }

    // 4) 레코드 수준 비교: raw_stream vs no_raw_stream
    eprintln!("\n  Record type comparison:");
    let mut raw_by_tag: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
    let mut noraw_by_tag: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
    for r in &raw_records {
        *raw_by_tag.entry(r.tag_id).or_default() += 1;
    }
    for r in &no_raw_records {
        *noraw_by_tag.entry(r.tag_id).or_default() += 1;
    }

    let mut all_tags: Vec<u16> = raw_by_tag
        .keys()
        .chain(noraw_by_tag.keys())
        .cloned()
        .collect();
    all_tags.sort();
    all_tags.dedup();
    for tag in &all_tags {
        let raw_cnt = raw_by_tag.get(tag).unwrap_or(&0);
        let noraw_cnt = noraw_by_tag.get(tag).unwrap_or(&0);
        if raw_cnt != noraw_cnt {
            eprintln!(
                "    tag={} ({}): raw={} vs rebuilt={}",
                tag,
                tags::tag_name(*tag),
                raw_cnt,
                noraw_cnt
            );
        }
    }

    // 5) ID_MAPPINGS 상세 덤프
    eprintln!("\n  === ID_MAPPINGS detail (original) ===");
    let labels = [
        "BinData",
        "KorFont",
        "EnFont",
        "CnFont",
        "JpFont",
        "OtherFont",
        "SymFont",
        "UsrFont",
        "BorderFill",
        "CharShape",
        "TabDef",
        "Numbering",
        "Bullet",
        "ParaShape",
        "Style",
        "MemoShape",
        "TrackChange",
        "TrackChangeUser",
    ];
    for r in &raw_records {
        if r.tag_id == tags::HWPTAG_ID_MAPPINGS {
            eprintln!(
                "    raw ID_MAPPINGS size={} ({} u32s)",
                r.data.len(),
                r.data.len() / 4
            );
            for i in 0..(r.data.len() / 4).min(18) {
                let off = i * 4;
                let val = u32::from_le_bytes([
                    r.data[off],
                    r.data[off + 1],
                    r.data[off + 2],
                    r.data[off + 3],
                ]);
                let name = if i < labels.len() { labels[i] } else { "???" };
                eprintln!("      [{:2}] {:16} = {}", i, name, val);
            }
        }
    }
    eprintln!("  === ID_MAPPINGS detail (rebuilt) ===");
    for r in &no_raw_records {
        if r.tag_id == tags::HWPTAG_ID_MAPPINGS {
            eprintln!(
                "    rebuilt ID_MAPPINGS size={} ({} u32s)",
                r.data.len(),
                r.data.len() / 4
            );
            for i in 0..(r.data.len() / 4).min(18) {
                let off = i * 4;
                let val = u32::from_le_bytes([
                    r.data[off],
                    r.data[off + 1],
                    r.data[off + 2],
                    r.data[off + 3],
                ]);
                let name = if i < labels.len() { labels[i] } else { "???" };
                eprintln!("      [{:2}] {:16} = {}", i, name, val);
            }
        }
    }

    // 6) 원본 DocInfo 레코드별 크기 확인 (CHAR_SHAPE)
    eprintln!("\n  Original CHAR_SHAPE record sizes:");
    let mut cs_sizes: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for r in &raw_records {
        if r.tag_id == tags::HWPTAG_CHAR_SHAPE {
            *cs_sizes.entry(r.data.len()).or_default() += 1;
        }
    }
    for (sz, cnt) in &cs_sizes {
        eprintln!("    size={}: {} records", sz, cnt);
    }

    eprintln!("\n  Rebuilt CHAR_SHAPE record sizes:");
    let mut cs_sizes2: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for r in &no_raw_records {
        if r.tag_id == tags::HWPTAG_CHAR_SHAPE {
            *cs_sizes2.entry(r.data.len()).or_default() += 1;
        }
    }
    for (sz, cnt) in &cs_sizes2 {
        eprintln!("    size={}: {} records", sz, cnt);
    }

    // 7) PARA_SHAPE, BORDER_FILL, STYLE 레코드 크기 비교
    for check_tag in &[
        tags::HWPTAG_PARA_SHAPE,
        tags::HWPTAG_BORDER_FILL,
        tags::HWPTAG_STYLE,
        tags::HWPTAG_TAB_DEF,
    ] {
        let tag_name = tags::tag_name(*check_tag);
        let mut raw_sizes: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut rebuilt_sizes: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for r in &raw_records {
            if r.tag_id == *check_tag {
                *raw_sizes.entry(r.data.len()).or_default() += 1;
            }
        }
        for r in &no_raw_records {
            if r.tag_id == *check_tag {
                *rebuilt_sizes.entry(r.data.len()).or_default() += 1;
            }
        }
        if raw_sizes != rebuilt_sizes {
            eprintln!("\n  {} SIZE MISMATCH:", tag_name);
            eprintln!("    Original: {:?}", raw_sizes);
            eprintln!("    Rebuilt:  {:?}", rebuilt_sizes);
        }
    }

    // 8) 전체 레코드 수 비교
    eprintln!(
        "\n  Total records: original={} vs rebuilt={}",
        raw_records.len(),
        no_raw_records.len()
    );

    eprintln!("\n=== ROUNDTRIP TEST COMPLETE ===");
}



/// 007 파일 vs P2(정상) 파일의 DocInfo 레코드별 크기 비교
#[test]
fn test_docinfo_007_vs_correct() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let files = [
        ("/app/pasts/20250130-hongbo-p2.hwp", "CORRECT(P2)"),
        ("/app/pasts/20250130-hongbo_saved-past-007.hwp", "OURS-007"),
    ];

    for (path, label) in &files {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("File not found: {}", path);
                continue;
            }
        };

        eprintln!("\n{}", "=".repeat(80));
        eprintln!("=== {} : {} ===", label, path);

        let mut cfb = CfbReader::open(&data).unwrap();
        let di_data = cfb.read_doc_info(true).unwrap();
        let records = Record::read_all(&di_data).unwrap();

        eprintln!("  Total DocInfo records: {}", records.len());

        // 각 레코드 타입별 크기 상세 출력
        let tag_order = [
            tags::HWPTAG_DOCUMENT_PROPERTIES,
            tags::HWPTAG_ID_MAPPINGS,
            tags::HWPTAG_BIN_DATA,
            tags::HWPTAG_FACE_NAME,
            tags::HWPTAG_BORDER_FILL,
            tags::HWPTAG_CHAR_SHAPE,
            tags::HWPTAG_TAB_DEF,
            tags::HWPTAG_NUMBERING,
            tags::HWPTAG_PARA_SHAPE,
            tags::HWPTAG_STYLE,
        ];

        for tag in &tag_order {
            let matching: Vec<_> = records.iter().filter(|r| r.tag_id == *tag).collect();
            if matching.is_empty() {
                continue;
            }

            let mut sizes: std::collections::HashMap<usize, usize> =
                std::collections::HashMap::new();
            for r in &matching {
                *sizes.entry(r.data.len()).or_default() += 1;
            }
            let mut sorted_sizes: Vec<_> = sizes.iter().collect();
            sorted_sizes.sort_by_key(|(sz, _)| **sz);

            eprintln!(
                "  {} (tag={}): count={}, sizes={:?}",
                tags::tag_name(*tag),
                tag,
                matching.len(),
                sorted_sizes
                    .iter()
                    .map(|(s, c)| format!("{}b×{}", s, c))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        // 미지원 태그도 출력
        let known_tags: std::collections::HashSet<u16> = tag_order.iter().cloned().collect();
        let mut extra_tags: std::collections::HashMap<u16, usize> =
            std::collections::HashMap::new();
        for r in &records {
            if !known_tags.contains(&r.tag_id) {
                *extra_tags.entry(r.tag_id).or_default() += 1;
            }
        }
        if !extra_tags.is_empty() {
            let mut sorted_extra: Vec<_> = extra_tags.iter().collect();
            sorted_extra.sort_by_key(|(t, _)| **t);
            for (tag, cnt) in &sorted_extra {
                eprintln!(
                    "  [extra] {} (tag={}): count={}",
                    tags::tag_name(**tag),
                    tag,
                    cnt
                );
            }
        }

        // ID_MAPPINGS 상세
        let labels = [
            "BinData",
            "KorFont",
            "EnFont",
            "CnFont",
            "JpFont",
            "OtherFont",
            "SymFont",
            "UsrFont",
            "BorderFill",
            "CharShape",
            "TabDef",
            "Numbering",
            "Bullet",
            "ParaShape",
            "Style",
            "MemoShape",
        ];
        for r in &records {
            if r.tag_id == tags::HWPTAG_ID_MAPPINGS {
                eprintln!(
                    "  ID_MAPPINGS ({} bytes, {} u32s):",
                    r.data.len(),
                    r.data.len() / 4
                );
                for i in 0..(r.data.len() / 4).min(16) {
                    let off = i * 4;
                    let val = u32::from_le_bytes([
                        r.data[off],
                        r.data[off + 1],
                        r.data[off + 2],
                        r.data[off + 3],
                    ]);
                    eprintln!("    [{:2}] {:16} = {}", i, labels[i.min(15)], val);
                }
            }
        }

        // Style 레코드 상세 (para_shape_id, char_shape_id 확인)
        eprintln!("  Style records detail:");
        let mut style_idx = 0;
        for r in &records {
            if r.tag_id == tags::HWPTAG_STYLE {
                let hex: String = r
                    .data
                    .iter()
                    .take(32)
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                eprintln!("    style[{}] size={}: {}", style_idx, r.data.len(), hex);
                style_idx += 1;
            }
        }
    }

    eprintln!("\n=== 007 vs CORRECT COMPARISON COMPLETE ===");
}



/// 원본 HWP 파일과 저장된 HWP 파일의 DocInfo/BodyText 스트림 비교
#[test]
fn test_compare_orig_vs_saved() {
    use crate::parser::record::Record;
    use crate::parser::tags;
    use std::path::Path;

    let orig_path = Path::new("pasts/20250130-hongbo-p2.hwp");
    let saved_path = Path::new("pasts/20250130-hongbo_saved-rp-001.hwp");
    if !orig_path.exists() || !saved_path.exists() {
        eprintln!("파일 없음 — 건너뜀");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let saved_data = std::fs::read(saved_path).unwrap();

    eprintln!("=== 원본 vs 저장 파일 비교 ===");
    eprintln!("원본 파일 크기: {} bytes", orig_data.len());
    eprintln!("저장 파일 크기: {} bytes", saved_data.len());

    // 1. parse_hwp로 파싱
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();

    // 2. CfbReader로 raw 스트림 추출
    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();

    // DocInfo raw bytes 비교
    let orig_di = orig_cfb.read_doc_info(orig_doc.header.compressed).unwrap();
    let saved_di = saved_cfb
        .read_doc_info(saved_doc.header.compressed)
        .unwrap();

    eprintln!("\n--- DocInfo 스트림 비교 ---");
    eprintln!("원본 DocInfo: {} bytes", orig_di.len());
    eprintln!("저장 DocInfo: {} bytes", saved_di.len());
    if orig_di == saved_di {
        eprintln!("DocInfo: 동일");
    } else {
        let min_len = orig_di.len().min(saved_di.len());
        let mut first_diff = None;
        let mut diff_count = 0;
        for i in 0..min_len {
            if orig_di[i] != saved_di[i] {
                if first_diff.is_none() {
                    first_diff = Some(i);
                }
                diff_count += 1;
            }
        }
        eprintln!(
            "DocInfo: 차이 발견! 첫 차이 offset={}, 총 바이트 차이={}, 길이 차이={}",
            first_diff.unwrap_or(min_len),
            diff_count,
            (orig_di.len() as i64 - saved_di.len() as i64).abs()
        );
    }

    // BodyText/Section0 raw bytes 비교
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();

    eprintln!("\n--- BodyText/Section0 스트림 비교 ---");
    eprintln!("원본 BodyText: {} bytes", orig_bt.len());
    eprintln!("저장 BodyText: {} bytes", saved_bt.len());
    if orig_bt == saved_bt {
        eprintln!("BodyText: 동일");
    } else {
        let min_len = orig_bt.len().min(saved_bt.len());
        let mut first_diff = None;
        let mut diff_count = 0;
        for i in 0..min_len {
            if orig_bt[i] != saved_bt[i] {
                if first_diff.is_none() {
                    first_diff = Some(i);
                }
                if diff_count < 10 {
                    eprintln!(
                        "  offset {}: orig={:02X} saved={:02X}",
                        i, orig_bt[i], saved_bt[i]
                    );
                }
                diff_count += 1;
            }
        }
        eprintln!(
            "BodyText: 첫 차이 offset={}, 총 바이트 차이={}, 길이 차이={}",
            first_diff.unwrap_or(min_len),
            diff_count,
            (orig_bt.len() as i64 - saved_bt.len() as i64).abs()
        );
    }

    // 3. 문단 및 컨트롤 수 비교
    eprintln!("\n--- 문단/컨트롤 수 비교 ---");
    let orig_paras = &orig_doc.sections[0].paragraphs;
    let saved_paras = &saved_doc.sections[0].paragraphs;
    eprintln!("원본 문단 수: {}", orig_paras.len());
    eprintln!("저장 문단 수: {}", saved_paras.len());

    let orig_ctrl_count: usize = orig_paras.iter().map(|p| p.controls.len()).sum();
    let saved_ctrl_count: usize = saved_paras.iter().map(|p| p.controls.len()).sum();
    eprintln!("원본 컨트롤 수: {}", orig_ctrl_count);
    eprintln!("저장 컨트롤 수: {}", saved_ctrl_count);

    // 원본 컨트롤 목록
    eprintln!("\n--- 원본 파일 컨트롤 목록 ---");
    for (pi, para) in orig_paras.iter().enumerate() {
        for (ci, ctrl) in para.controls.iter().enumerate() {
            let ctrl_type = match ctrl {
                crate::model::control::Control::SectionDef(_) => "SectionDef",
                crate::model::control::Control::ColumnDef(_) => "ColumnDef",
                crate::model::control::Control::Table(t) => {
                    eprintln!(
                        "  para[{}] ctrl[{}]: Table (rows={}, cols={})",
                        pi, ci, t.row_count, t.col_count
                    );
                    continue;
                }
                crate::model::control::Control::Shape(_) => "Shape",
                crate::model::control::Control::Picture(_) => "Picture",
                crate::model::control::Control::Header(_) => "Header",
                crate::model::control::Control::Footer(_) => "Footer",
                crate::model::control::Control::Footnote(_) => "Footnote",
                crate::model::control::Control::Endnote(_) => "Endnote",
                crate::model::control::Control::AutoNumber(_) => "AutoNumber",
                crate::model::control::Control::NewNumber(_) => "NewNumber",
                crate::model::control::Control::PageNumberPos(_) => "PageNumberPos",
                crate::model::control::Control::Bookmark(_) => "Bookmark",
                crate::model::control::Control::Hyperlink(_) => "Hyperlink",
                crate::model::control::Control::Ruby(_) => "Ruby",
                crate::model::control::Control::CharOverlap(_) => "CharOverlap",
                crate::model::control::Control::PageHide(_) => "PageHide",
                crate::model::control::Control::HiddenComment(_) => "HiddenComment",
                crate::model::control::Control::Equation(_) => "Equation",
                crate::model::control::Control::Field(_) => "Field",
                crate::model::control::Control::Form(_) => "Form",
                crate::model::control::Control::Unknown(u) => {
                    eprintln!(
                        "  para[{}] ctrl[{}]: Unknown (ctrl_id=0x{:08X})",
                        pi, ci, u.ctrl_id
                    );
                    continue;
                }
            };
            eprintln!("  para[{}] ctrl[{}]: {}", pi, ci, ctrl_type);
        }
    }

    // 저장 파일 컨트롤 목록
    eprintln!("\n--- 저장 파일 컨트롤 목록 ---");
    for (pi, para) in saved_paras.iter().enumerate() {
        for (ci, ctrl) in para.controls.iter().enumerate() {
            let ctrl_type = match ctrl {
                crate::model::control::Control::SectionDef(_) => "SectionDef",
                crate::model::control::Control::ColumnDef(_) => "ColumnDef",
                crate::model::control::Control::Table(t) => {
                    eprintln!(
                        "  para[{}] ctrl[{}]: Table (rows={}, cols={})",
                        pi, ci, t.row_count, t.col_count
                    );
                    continue;
                }
                crate::model::control::Control::Shape(_) => "Shape",
                crate::model::control::Control::Picture(_) => "Picture",
                crate::model::control::Control::Header(_) => "Header",
                crate::model::control::Control::Footer(_) => "Footer",
                crate::model::control::Control::Footnote(_) => "Footnote",
                crate::model::control::Control::Endnote(_) => "Endnote",
                crate::model::control::Control::AutoNumber(_) => "AutoNumber",
                crate::model::control::Control::NewNumber(_) => "NewNumber",
                crate::model::control::Control::PageNumberPos(_) => "PageNumberPos",
                crate::model::control::Control::Bookmark(_) => "Bookmark",
                crate::model::control::Control::Hyperlink(_) => "Hyperlink",
                crate::model::control::Control::Ruby(_) => "Ruby",
                crate::model::control::Control::CharOverlap(_) => "CharOverlap",
                crate::model::control::Control::PageHide(_) => "PageHide",
                crate::model::control::Control::HiddenComment(_) => "HiddenComment",
                crate::model::control::Control::Equation(_) => "Equation",
                crate::model::control::Control::Field(_) => "Field",
                crate::model::control::Control::Form(_) => "Form",
                crate::model::control::Control::Unknown(u) => {
                    eprintln!(
                        "  para[{}] ctrl[{}]: Unknown (ctrl_id=0x{:08X})",
                        pi, ci, u.ctrl_id
                    );
                    continue;
                }
            };
            eprintln!("  para[{}] ctrl[{}]: {}", pi, ci, ctrl_type);
        }
    }

    // 4. 저장 파일 Section0의 마지막 20개 레코드 분석
    eprintln!("\n--- 저장 파일 Section0: 마지막 20개 레코드 ---");
    let saved_recs = Record::read_all(&saved_bt).unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();
    eprintln!("원본 레코드 수: {}", orig_recs.len());
    eprintln!("저장 레코드 수: {}", saved_recs.len());

    let start = if saved_recs.len() > 20 {
        saved_recs.len() - 20
    } else {
        0
    };
    for i in start..saved_recs.len() {
        let r = &saved_recs[i];
        let tag = tags::tag_name(r.tag_id);
        // CTRL_HEADER인 경우 ctrl_id 표시
        if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes(r.data[0..4].try_into().unwrap());
            let ctrl = tags::ctrl_name(ctrl_id);
            eprintln!(
                "  [{}] {} L{} {}B ctrl={} {:02X?}",
                i,
                tag,
                r.level,
                r.data.len(),
                ctrl,
                &r.data[..r.data.len().min(32)]
            );
        } else {
            eprintln!(
                "  [{}] {} L{} {}B {:02X?}",
                i,
                tag,
                r.level,
                r.data.len(),
                &r.data[..r.data.len().min(32)]
            );
        }
    }

    // 원본의 마지막 20개 레코드도 비교
    eprintln!("\n--- 원본 파일 Section0: 마지막 20개 레코드 ---");
    let start = if orig_recs.len() > 20 {
        orig_recs.len() - 20
    } else {
        0
    };
    for i in start..orig_recs.len() {
        let r = &orig_recs[i];
        let tag = tags::tag_name(r.tag_id);
        if r.tag_id == tags::HWPTAG_CTRL_HEADER && r.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes(r.data[0..4].try_into().unwrap());
            let ctrl = tags::ctrl_name(ctrl_id);
            eprintln!(
                "  [{}] {} L{} {}B ctrl={} {:02X?}",
                i,
                tag,
                r.level,
                r.data.len(),
                ctrl,
                &r.data[..r.data.len().min(32)]
            );
        } else {
            eprintln!(
                "  [{}] {} L{} {}B {:02X?}",
                i,
                tag,
                r.level,
                r.data.len(),
                &r.data[..r.data.len().min(32)]
            );
        }
    }

    // 레코드 전체 비교: 첫 차이 위치 찾기
    eprintln!("\n--- 레코드 비교 (전체) ---");
    let max_recs = orig_recs.len().max(saved_recs.len());
    let mut first_rec_diff = None;
    let mut total_diffs = 0;
    for i in 0..max_recs {
        if i >= orig_recs.len() {
            if first_rec_diff.is_none() {
                first_rec_diff = Some(i);
            }
            total_diffs += 1;
            if total_diffs <= 15 {
                eprintln!(
                    "  [{}] 원본에 없음, 저장: {} L{} {}B",
                    i,
                    tags::tag_name(saved_recs[i].tag_id),
                    saved_recs[i].level,
                    saved_recs[i].data.len()
                );
            }
            continue;
        }
        if i >= saved_recs.len() {
            if first_rec_diff.is_none() {
                first_rec_diff = Some(i);
            }
            total_diffs += 1;
            if total_diffs <= 15 {
                eprintln!(
                    "  [{}] 저장에 없음, 원본: {} L{} {}B",
                    i,
                    tags::tag_name(orig_recs[i].tag_id),
                    orig_recs[i].level,
                    orig_recs[i].data.len()
                );
            }
            continue;
        }
        let o = &orig_recs[i];
        let s = &saved_recs[i];
        if o.tag_id != s.tag_id || o.level != s.level || o.data != s.data {
            if first_rec_diff.is_none() {
                first_rec_diff = Some(i);
            }
            total_diffs += 1;
            if total_diffs <= 15 {
                let otag = tags::tag_name(o.tag_id);
                let stag = tags::tag_name(s.tag_id);
                eprintln!("  [{}] 차이:", i);
                eprintln!(
                    "    원본: {} L{} {}B {:02X?}",
                    otag,
                    o.level,
                    o.data.len(),
                    &o.data[..o.data.len().min(40)]
                );
                eprintln!(
                    "    저장: {} L{} {}B {:02X?}",
                    stag,
                    s.level,
                    s.data.len(),
                    &s.data[..s.data.len().min(40)]
                );
            }
        }
    }
    eprintln!(
        "첫 차이 레코드 인덱스: {:?}, 총 차이 레코드: {}",
        first_rec_diff, total_diffs
    );
    if total_diffs > 15 {
        eprintln!("  (15개 이후 생략, 총 {}개 차이)", total_diffs);
    }

    eprintln!("\n=== 원본 vs 저장 파일 비교 완료 ===");
}



/// DocInfo CharShape 수 추적: 파싱 → convertToEditable → paste → export
#[test]
fn test_trace_charshape_loss() {
    use crate::parser::record::Record;
    use crate::parser::tags;

    let orig_path = "pasts/20250130-hongbo-p2.hwp";
    if !std::path::Path::new(orig_path).exists() {
        eprintln!("SKIP: 파일 없음");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    // Helper: count CHAR_SHAPE records in raw_stream
    fn count_cs_in_raw(raw: &Option<Vec<u8>>) -> usize {
        match raw {
            Some(data) => {
                let records = Record::read_all(data).unwrap_or_default();
                records
                    .iter()
                    .filter(|r| r.tag_id == tags::HWPTAG_CHAR_SHAPE)
                    .count()
            }
            None => 0,
        }
    }

    // Step 1: After parsing
    let model_cs_1 = doc.document().doc_info.char_shapes.len();
    let raw_cs_1 = count_cs_in_raw(&doc.document().doc_info.raw_stream);
    let is_dist = doc.document().header.distribution;
    eprintln!(
        "Step 1 (after parse): model={} raw={} distribution={}",
        model_cs_1, raw_cs_1, is_dist
    );

    // Step 2: After convert_to_editable
    let converted = doc.convert_to_editable_native().unwrap();
    let model_cs_2 = doc.document().doc_info.char_shapes.len();
    let raw_cs_2 = count_cs_in_raw(&doc.document().doc_info.raw_stream);
    eprintln!(
        "Step 2 (after convert): model={} raw={} result={}",
        model_cs_2, raw_cs_2, converted
    );

    // Step 3: Export without paste
    let saved_no_paste = doc.export_hwp_native().unwrap();
    let doc_np = crate::parser::parse_hwp(&saved_no_paste).unwrap();
    eprintln!(
        "Step 3 (export no paste): model_cs={}",
        doc_np.doc_info.char_shapes.len()
    );

    // Step 4: Paste simple table
    let last_para = doc.document.sections[0].paragraphs.len() - 1;
    let _ = doc.paste_html_native(
        0,
        last_para,
        0,
        r#"<table><tr><td>A</td><td>B</td></tr></table>"#,
    );
    let model_cs_4 = doc.document().doc_info.char_shapes.len();
    let raw_cs_4 = count_cs_in_raw(&doc.document().doc_info.raw_stream);
    eprintln!(
        "Step 4 (after paste): model={} raw={}",
        model_cs_4, raw_cs_4
    );

    // Step 5: Export after paste
    let saved_with_paste = doc.export_hwp_native().unwrap();
    let doc_wp = crate::parser::parse_hwp(&saved_with_paste).unwrap();
    eprintln!(
        "Step 5 (export with paste): model_cs={}",
        doc_wp.doc_info.char_shapes.len()
    );

    // Assertions
    assert_eq!(
        model_cs_1, raw_cs_1,
        "Model vs raw after parse should match"
    );
}



#[test]
fn test_simple_text_insert_and_save() {
    // template/empty.hwp 로드 → 텍스트 삽입 → 저장
    let path = "template/empty.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();

    eprintln!("=== 단순 텍스트 삽입 + 저장 테스트 ===");
    eprintln!(
        "원본: {} bytes, {}페이지, {}개 구역",
        data.len(),
        doc.page_count(),
        doc.document.sections.len()
    );

    // 첫 번째 구역, 첫 번째 문단에 텍스트 삽입
    let section = &doc.document.sections[0];
    eprintln!("문단 수: {}", section.paragraphs.len());
    for (i, p) in section.paragraphs.iter().enumerate() {
        eprintln!(
            "  문단[{}]: text='{}' controls={} line_segs={}",
            i,
            p.text,
            p.controls.len(),
            p.line_segs.len()
        );
    }

    // "가나다라마바사아" 삽입
    let result = doc.insert_text_native(0, 0, 0, "가나다라마바사아");
    assert!(result.is_ok(), "텍스트 삽입 실패: {:?}", result.err());
    eprintln!("텍스트 삽입 결과: {}", result.unwrap());

    // 삽입 후 상태 확인
    let section = &doc.document.sections[0];
    eprintln!("삽입 후 문단[0]: text='{}'", section.paragraphs[0].text);

    // HWP 내보내기
    let saved = doc.export_hwp_native();
    assert!(saved.is_ok(), "HWP 내보내기 실패: {:?}", saved.err());
    let saved_data = saved.unwrap();
    eprintln!("저장된 파일: {} bytes", saved_data.len());

    // output/ 폴더에 저장
    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/empty_with_text.hwp", &saved_data).unwrap();
    eprintln!("output/empty_with_text.hwp 저장 완료");

    // 저장된 파일 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&saved_data);
    assert!(doc2.is_ok(), "저장된 파일 재파싱 실패: {:?}", doc2.err());
    let doc2 = doc2.unwrap();
    eprintln!("재파싱 성공: {}페이지", doc2.page_count());

    let section2 = &doc2.document.sections[0];
    eprintln!("재파싱 문단[0]: text='{}'", section2.paragraphs[0].text);
    assert!(
        section2.paragraphs[0].text.contains("가나다라마바사아"),
        "저장된 파일에 삽입한 텍스트가 없음"
    );
}



#[test]
fn test_empty_save_analysis() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::header;
    use crate::parser::record::Record;
    use crate::parser::tags;
    use std::collections::BTreeMap;

    let orig_path = "template/empty.hwp";
    let saved_path = "output/empty_with_text.hwp";

    if !std::path::Path::new(orig_path).exists() {
        eprintln!("SKIP: {} 없음", orig_path);
        return;
    }
    if !std::path::Path::new(saved_path).exists() {
        // 먼저 저장 파일을 생성한다
        eprintln!("output/empty_with_text.hwp 없음 - 생성 시도...");
        let data = std::fs::read(orig_path).unwrap();
        let mut doc = HwpDocument::from_bytes(&data).unwrap();
        let _ = doc.insert_text_native(0, 0, 0, "가나다라마바사아");
        let saved = doc.export_hwp_native().unwrap();
        let _ = std::fs::create_dir_all("output");
        std::fs::write(saved_path, &saved).unwrap();
        eprintln!("output/empty_with_text.hwp 생성 완료");
    }

    let orig_data =
        std::fs::read(orig_path).unwrap_or_else(|e| panic!("원본 파일 읽기 실패: {}", e));
    let saved_data =
        std::fs::read(saved_path).unwrap_or_else(|e| panic!("저장 파일 읽기 실패: {}", e));

    println!("\n{}", "=".repeat(80));
    println!("  EMPTY HWP vs SAVED-WITH-TEXT HWP ANALYSIS");
    println!("  Original:  {} ({} bytes)", orig_path, orig_data.len());
    println!("  Saved:     {} ({} bytes)", saved_path, saved_data.len());
    println!("{}", "=".repeat(80));

    // ============================================================
    // 1. FILE SIZE COMPARISON
    // ============================================================
    println!("\n--- 1. FILE SIZE COMPARISON ---");
    println!("Original: {} bytes", orig_data.len());
    println!("Saved:    {} bytes", saved_data.len());
    println!(
        "Diff:     {} bytes",
        saved_data.len() as i64 - orig_data.len() as i64
    );

    // ============================================================
    // 2. CFB STREAM LIST AND SIZES
    // ============================================================
    println!("\n--- 2. CFB STREAM LIST AND SIZES ---");

    let orig_cfb = CfbReader::open(&orig_data).expect("원본 CFB 열기 실패");
    let saved_cfb = CfbReader::open(&saved_data).expect("저장 CFB 열기 실패");

    let orig_entries: BTreeMap<String, (u64, bool)> = orig_cfb
        .list_all_entries()
        .into_iter()
        .map(|(path, size, is_stream)| (path, (size, is_stream)))
        .collect();

    let saved_entries: BTreeMap<String, (u64, bool)> = saved_cfb
        .list_all_entries()
        .into_iter()
        .map(|(path, size, is_stream)| (path, (size, is_stream)))
        .collect();

    println!(
        "\n{:<40} {:>10} {:>10} {:>8}",
        "Path", "Orig Size", "Saved Size", "Type"
    );
    println!("{:-<72}", "");

    let all_paths: std::collections::BTreeSet<&String> =
        orig_entries.keys().chain(saved_entries.keys()).collect();

    for path in &all_paths {
        let orig_info = orig_entries.get(*path);
        let saved_info = saved_entries.get(*path);
        let orig_size = orig_info
            .map(|(s, _)| format!("{}", s))
            .unwrap_or_else(|| "---".to_string());
        let saved_size = saved_info
            .map(|(s, _)| format!("{}", s))
            .unwrap_or_else(|| "---".to_string());
        let type_str = orig_info
            .or(saved_info)
            .map(|(_, is)| if *is { "stream" } else { "storage" })
            .unwrap_or("?");
        let marker = if orig_info.is_none() {
            " [NEW]"
        } else if saved_info.is_none() {
            " [MISSING]"
        } else if orig_info.map(|(s, _)| s) != saved_info.map(|(s, _)| s) {
            " [CHANGED]"
        } else {
            ""
        };
        println!(
            "{:<40} {:>10} {:>10} {:>8}{}",
            path, orig_size, saved_size, type_str, marker
        );
    }

    // ============================================================
    // 3. FileHeader COMPARISON
    // ============================================================
    println!("\n--- 3. FileHeader COMPARISON ---");
    let mut orig_cfb2 = CfbReader::open(&orig_data).unwrap();
    let mut saved_cfb2 = CfbReader::open(&saved_data).unwrap();

    let orig_header_raw = orig_cfb2.read_file_header().unwrap();
    let saved_header_raw = saved_cfb2.read_file_header().unwrap();

    let orig_fh = header::parse_file_header(&orig_header_raw).unwrap();
    let saved_fh = header::parse_file_header(&saved_header_raw).unwrap();

    println!(
        "Original: version={}.{}.{}.{} flags=0x{:08X} compressed={} encrypted={} distribution={}",
        orig_fh.version.major,
        orig_fh.version.minor,
        orig_fh.version.build,
        orig_fh.version.revision,
        orig_fh.flags.raw,
        orig_fh.flags.compressed,
        orig_fh.flags.encrypted,
        orig_fh.flags.distribution
    );
    println!(
        "Saved:    version={}.{}.{}.{} flags=0x{:08X} compressed={} encrypted={} distribution={}",
        saved_fh.version.major,
        saved_fh.version.minor,
        saved_fh.version.build,
        saved_fh.version.revision,
        saved_fh.flags.raw,
        saved_fh.flags.compressed,
        saved_fh.flags.encrypted,
        saved_fh.flags.distribution
    );

    if orig_header_raw == saved_header_raw {
        println!("FileHeaders are IDENTICAL.");
    } else {
        println!("FileHeaders DIFFER:");
        for i in 0..std::cmp::max(orig_header_raw.len(), saved_header_raw.len()) {
            let ob = orig_header_raw.get(i).copied();
            let sb = saved_header_raw.get(i).copied();
            if ob != sb {
                println!(
                    "  offset {:#06x}: orig=0x{:02X} saved=0x{:02X}",
                    i,
                    ob.unwrap_or(0),
                    sb.unwrap_or(0)
                );
            }
        }
    }

    // ============================================================
    // 4. DocInfo STREAM COMPARISON (byte-level)
    // ============================================================
    println!("\n--- 4. DocInfo STREAM COMPARISON (byte-level) ---");
    let orig_compressed = orig_fh.flags.compressed;
    let saved_compressed = saved_fh.flags.compressed;

    let mut orig_cfb3 = CfbReader::open(&orig_data).unwrap();
    let mut saved_cfb3 = CfbReader::open(&saved_data).unwrap();

    let orig_docinfo = orig_cfb3.read_doc_info(orig_compressed).unwrap();
    let saved_docinfo = saved_cfb3.read_doc_info(saved_compressed).unwrap();

    println!(
        "Original DocInfo (decompressed): {} bytes",
        orig_docinfo.len()
    );
    println!(
        "Saved DocInfo (decompressed):    {} bytes",
        saved_docinfo.len()
    );

    if orig_docinfo == saved_docinfo {
        println!("DocInfo streams are IDENTICAL.");
    } else {
        println!("DocInfo streams DIFFER!");
        let min_len = std::cmp::min(orig_docinfo.len(), saved_docinfo.len());
        let mut diff_count = 0;
        let mut first_diff_pos = None;
        for i in 0..min_len {
            if orig_docinfo[i] != saved_docinfo[i] {
                if diff_count < 20 {
                    println!(
                        "  offset {:#06x}: orig=0x{:02X} saved=0x{:02X}",
                        i, orig_docinfo[i], saved_docinfo[i]
                    );
                }
                if first_diff_pos.is_none() {
                    first_diff_pos = Some(i);
                }
                diff_count += 1;
            }
        }
        if orig_docinfo.len() != saved_docinfo.len() {
            println!(
                "  Size difference: orig={} saved={} (diff={})",
                orig_docinfo.len(),
                saved_docinfo.len(),
                saved_docinfo.len() as i64 - orig_docinfo.len() as i64
            );
        }
        println!(
            "  Total differing bytes: {} (first at offset {:?})",
            diff_count, first_diff_pos
        );

        // Parse DocInfo records for comparison
        println!("\n  --- DocInfo Record-by-Record ---");
        let orig_di_records = Record::read_all(&orig_docinfo).unwrap_or_default();
        let saved_di_records = Record::read_all(&saved_docinfo).unwrap_or_default();
        println!("  Original DocInfo records: {}", orig_di_records.len());
        println!("  Saved DocInfo records:    {}", saved_di_records.len());

        let max_di = std::cmp::max(orig_di_records.len(), saved_di_records.len());
        for i in 0..max_di {
            let orig_r = orig_di_records.get(i);
            let saved_r = saved_di_records.get(i);
            let matches = match (orig_r, saved_r) {
                (Some(o), Some(s)) => {
                    o.tag_id == s.tag_id
                        && o.level == s.level
                        && o.size == s.size
                        && o.data == s.data
                }
                _ => false,
            };
            if !matches {
                let orig_str = orig_r
                    .map(|r| format!("{} lvl={} sz={}", r.tag_name(), r.level, r.size))
                    .unwrap_or_else(|| "---".to_string());
                let saved_str = saved_r
                    .map(|r| format!("{} lvl={} sz={}", r.tag_name(), r.level, r.size))
                    .unwrap_or_else(|| "---".to_string());
                println!("  [{}] ORIG: {:<40} SAVED: {}", i, orig_str, saved_str);
                // If tags match but data differs, show data diff
                if let (Some(o), Some(s)) = (orig_r, saved_r) {
                    if o.tag_id == s.tag_id && o.data != s.data {
                        let show = std::cmp::min(40, std::cmp::max(o.data.len(), s.data.len()));
                        print!("       orig data: ");
                        for b in &o.data[..std::cmp::min(show, o.data.len())] {
                            print!("{:02x} ", b);
                        }
                        println!();
                        print!("       saved data: ");
                        for b in &s.data[..std::cmp::min(show, s.data.len())] {
                            print!("{:02x} ", b);
                        }
                        println!();
                    }
                }
            }
        }
    }

    // ============================================================
    // 5. BodyText/Section0 RECORD-BY-RECORD COMPARISON
    // ============================================================
    println!("\n--- 5. BodyText/Section0 RECORD-BY-RECORD COMPARISON ---");
    let mut orig_cfb4 = CfbReader::open(&orig_data).unwrap();
    let mut saved_cfb4 = CfbReader::open(&saved_data).unwrap();

    let orig_section = orig_cfb4
        .read_body_text_section(0, orig_compressed, false)
        .unwrap();
    let saved_section = saved_cfb4
        .read_body_text_section(0, saved_compressed, false)
        .unwrap();

    println!(
        "Original Section0 (decompressed): {} bytes",
        orig_section.len()
    );
    println!(
        "Saved Section0 (decompressed):    {} bytes",
        saved_section.len()
    );

    let orig_records = Record::read_all(&orig_section).unwrap();
    let saved_records = Record::read_all(&saved_section).unwrap();

    println!("Original records: {}", orig_records.len());
    println!("Saved records:    {}", saved_records.len());

    // Helper functions
    fn hex_dump_n(data: &[u8], max: usize) -> String {
        let show = std::cmp::min(data.len(), max);
        let hex: Vec<String> = data[..show].iter().map(|b| format!("{:02x}", b)).collect();
        let mut result = hex.join(" ");
        if data.len() > max {
            result.push_str(&format!(" ...({} more)", data.len() - max));
        }
        result
    }

    fn decode_para_header(data: &[u8]) -> String {
        if data.len() < 8 {
            return format!("(too short: {} bytes)", data.len());
        }
        // PARA_HEADER structure:
        // 0-3: nCharCount (lower 31 bits = char count, bit 31 = char_count_msb)
        // 4-7: controlMask (u32)
        // 8-9: paraShapeId (u16)
        // 10: paraStyleId (u8)
        // 11: columnSplit (u8)
        // 12-13: charShapeCount (u16)
        // 14-15: rangeTagCount (u16)
        // 16-17: lineAlignCount (u16)
        // 18-19: instanceID (u16)
        let raw = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let char_count = raw & 0x7FFFFFFF;
        let char_count_msb = (raw >> 31) & 1;
        let ctrl_mask = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let mut result = format!(
            "char_count={} msb={} ctrl_mask=0x{:08X}",
            char_count, char_count_msb, ctrl_mask
        );
        if data.len() >= 10 {
            let para_shape_id = u16::from_le_bytes([data[8], data[9]]);
            result.push_str(&format!(" paraShapeId={}", para_shape_id));
        }
        if data.len() >= 11 {
            result.push_str(&format!(" paraStyleId={}", data[10]));
        }
        if data.len() >= 14 {
            let cs_count = u16::from_le_bytes([data[12], data[13]]);
            result.push_str(&format!(" charShapeCount={}", cs_count));
        }
        if data.len() >= 16 {
            let rt_count = u16::from_le_bytes([data[14], data[15]]);
            result.push_str(&format!(" rangeTagCount={}", rt_count));
        }
        if data.len() >= 18 {
            let la_count = u16::from_le_bytes([data[16], data[17]]);
            result.push_str(&format!(" lineAlignCount={}", la_count));
        }
        if data.len() >= 20 {
            let inst_id = u16::from_le_bytes([data[18], data[19]]);
            result.push_str(&format!(" instanceID={}", inst_id));
        }
        result
    }

    fn decode_para_text(data: &[u8]) -> String {
        // UTF-16LE text stream with control chars
        let mut result = String::new();
        let mut i = 0;
        let mut char_pos = 0;
        while i + 1 < data.len() {
            let ch = u16::from_le_bytes([data[i], data[i + 1]]);
            match ch {
                // Extended controls take 8 WCHARs (16 bytes)
                0x0001..=0x000F => {
                    let name = match ch {
                        0x0002 => "SEC/COL",
                        0x0003 => "FIELD_BEGIN",
                        0x0004 => "FIELD_END",
                        0x0008 => "INLINE",
                        0x000B => "EXT_CTRL",
                        0x000D => "PARA_BREAK",
                        0x000A => "LINE_BREAK",
                        _ => "CTRL",
                    };
                    result.push_str(&format!("[{}@{}]", name, char_pos));
                    // Extended controls (2,3,11,12,13,14,15) occupy 8 WCHARs
                    if ch >= 1 && ch <= 9 {
                        // These chars occupy 8 WCHARs (16 bytes)
                        i += 16;
                        char_pos += 8;
                    } else {
                        i += 2;
                        char_pos += 1;
                    }
                }
                _ => {
                    if let Some(c) = char::from_u32(ch as u32) {
                        result.push(c);
                    } else {
                        result.push_str(&format!("\\u{:04X}", ch));
                    }
                    i += 2;
                    char_pos += 1;
                }
            }
        }
        result
    }

    fn decode_para_char_shape(data: &[u8]) -> String {
        // Array of (position: u32, charShapeId: u32) pairs
        let pair_count = data.len() / 8;
        let mut result = format!("{} pairs: ", pair_count);
        for p in 0..pair_count {
            let off = p * 8;
            if off + 8 > data.len() {
                break;
            }
            let pos = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let id =
                u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            if p > 0 {
                result.push_str(", ");
            }
            result.push_str(&format!("(pos={}, id={})", pos, id));
        }
        result
    }

    fn decode_para_line_seg(data: &[u8]) -> String {
        // Each line segment is 36 bytes:
        // textStartPos(4) + lineVPos(4) + lineHPos(4) + lineHeight(4)
        // + textPartHeight(4) + distBaseline(4) + lineSpacing(4) + colStartPos(4) + segWidth(4)
        // Some versions use 32 bytes per segment
        let seg_size = if data.len().is_multiple_of(36) {
            36
        } else if data.len().is_multiple_of(32) {
            32
        } else {
            36
        };
        let seg_count = if seg_size > 0 {
            data.len() / seg_size
        } else {
            0
        };
        let mut result = format!("{} segments ({}B each): ", seg_count, seg_size);
        for s in 0..std::cmp::min(seg_count, 4) {
            let off = s * seg_size;
            if off + 16 > data.len() {
                break;
            }
            let text_start =
                u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let line_vpos =
                i32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            let line_hpos =
                i32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
            let line_height = u32::from_le_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]);
            if s > 0 {
                result.push_str(", ");
            }
            result.push_str(&format!(
                "[start={} v={} h={} h={}]",
                text_start, line_vpos, line_hpos, line_height
            ));
        }
        if seg_count > 4 {
            result.push_str(&format!(" ...({} more)", seg_count - 4));
        }
        result
    }

    fn decode_ctrl_header(data: &[u8]) -> String {
        if data.len() < 4 {
            return format!("(too short: {} bytes)", data.len());
        }
        let ctrl_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let be_bytes = ctrl_id.to_be_bytes();
        let ascii: String = be_bytes
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        format!(
            "ctrl_id=0x{:08X} \"{}\" ({})",
            ctrl_id,
            ascii,
            tags::ctrl_name(ctrl_id)
        )
    }

    // Print all records with decoded details
    println!(
        "\n  {:<4} {:<22} {:>3} {:>6}  |  {:<22} {:>3} {:>6}  | Status",
        "#", "Orig Tag", "Lvl", "Size", "Saved Tag", "Lvl", "Size"
    );
    println!("  {:-<110}", "");

    let max_recs = std::cmp::max(orig_records.len(), saved_records.len());
    for i in 0..max_recs {
        let orig_r = orig_records.get(i);
        let saved_r = saved_records.get(i);

        let orig_str = orig_r
            .map(|r| format!("{:<22} {:>3} {:>6}", r.tag_name(), r.level, r.size))
            .unwrap_or_else(|| format!("{:<22} {:>3} {:>6}", "---", "", ""));
        let saved_str = saved_r
            .map(|r| format!("{:<22} {:>3} {:>6}", r.tag_name(), r.level, r.size))
            .unwrap_or_else(|| format!("{:<22} {:>3} {:>6}", "---", "", ""));

        let status = match (orig_r, saved_r) {
            (Some(o), Some(s)) => {
                if o.tag_id == s.tag_id
                    && o.level == s.level
                    && o.size == s.size
                    && o.data == s.data
                {
                    "OK"
                } else if o.tag_id == s.tag_id && o.level == s.level && o.size == s.size {
                    "DATA_DIFF"
                } else if o.tag_id == s.tag_id && o.level == s.level {
                    "SIZE_DIFF"
                } else if o.tag_id == s.tag_id {
                    "LEVEL_DIFF"
                } else {
                    "TAG_DIFF"
                }
            }
            (Some(_), None) => "ORIG_ONLY",
            (None, Some(_)) => "SAVED_ONLY",
            (None, None) => "???",
        };

        // Always print, even OK, so we can see the full record layout
        println!("  {:<4} {}  |  {}  | {}", i, orig_str, saved_str, status);

        // Decode details for non-OK records
        if status != "OK" {
            // Show decoded info for both
            for (label, rec) in [("ORIG", orig_r), ("SAVED", saved_r)] {
                if let Some(r) = rec {
                    let detail = match r.tag_id {
                        t if t == tags::HWPTAG_PARA_HEADER => decode_para_header(&r.data),
                        t if t == tags::HWPTAG_PARA_TEXT => {
                            let text_decoded = decode_para_text(&r.data);
                            format!(
                                "hex[0..40]: {}  text: {}",
                                hex_dump_n(&r.data, 40),
                                text_decoded
                            )
                        }
                        t if t == tags::HWPTAG_PARA_CHAR_SHAPE => decode_para_char_shape(&r.data),
                        t if t == tags::HWPTAG_PARA_LINE_SEG => decode_para_line_seg(&r.data),
                        t if t == tags::HWPTAG_CTRL_HEADER => decode_ctrl_header(&r.data),
                        _ => format!("hex: {}", hex_dump_n(&r.data, 40)),
                    };
                    println!("       {}: {}", label, detail);
                }
            }
        }
    }

    // ============================================================
    // 6. Record SUMMARY & STATISTICS
    // ============================================================
    println!("\n--- 6. RECORD SUMMARY ---");
    println!("\nOriginal record types:");
    let mut orig_tag_counts: BTreeMap<u16, usize> = BTreeMap::new();
    for r in &orig_records {
        *orig_tag_counts.entry(r.tag_id).or_insert(0) += 1;
    }
    for (tag, count) in &orig_tag_counts {
        println!("  {:>3} ({:<22}): {}", tag, tags::tag_name(*tag), count);
    }

    println!("\nSaved record types:");
    let mut saved_tag_counts: BTreeMap<u16, usize> = BTreeMap::new();
    for r in &saved_records {
        *saved_tag_counts.entry(r.tag_id).or_insert(0) += 1;
    }
    for (tag, count) in &saved_tag_counts {
        println!("  {:>3} ({:<22}): {}", tag, tags::tag_name(*tag), count);
    }

    // ============================================================
    // 7. PARA_HEADER DETAIL for ALL paragraphs
    // ============================================================
    println!("\n--- 7. ALL PARA_HEADER DETAILS ---");
    println!("\n  Original paragraphs:");
    for (i, r) in orig_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_HEADER {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_para_header(&r.data)
            );
            println!("         hex: {}", hex_dump_n(&r.data, 40));
        }
    }
    println!("\n  Saved paragraphs:");
    for (i, r) in saved_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_HEADER {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_para_header(&r.data)
            );
            println!("         hex: {}", hex_dump_n(&r.data, 40));
        }
    }

    // ============================================================
    // 8. PARA_TEXT DETAIL (hex + decoded)
    // ============================================================
    println!("\n--- 8. ALL PARA_TEXT DETAILS ---");
    println!("\n  Original PARA_TEXT:");
    for (i, r) in orig_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_TEXT {
            println!("    [{}] lvl={} sz={}", i, r.level, r.size);
            println!("      hex: {}", hex_dump_n(&r.data, 60));
            println!("      decoded: {}", decode_para_text(&r.data));
        }
    }
    println!("\n  Saved PARA_TEXT:");
    for (i, r) in saved_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_TEXT {
            println!("    [{}] lvl={} sz={}", i, r.level, r.size);
            println!("      hex: {}", hex_dump_n(&r.data, 60));
            println!("      decoded: {}", decode_para_text(&r.data));
        }
    }

    // ============================================================
    // 9. PARA_CHAR_SHAPE DETAIL
    // ============================================================
    println!("\n--- 9. ALL PARA_CHAR_SHAPE DETAILS ---");
    println!("\n  Original PARA_CHAR_SHAPE:");
    for (i, r) in orig_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_para_char_shape(&r.data)
            );
            println!("      hex: {}", hex_dump_n(&r.data, 40));
        }
    }
    println!("\n  Saved PARA_CHAR_SHAPE:");
    for (i, r) in saved_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_para_char_shape(&r.data)
            );
            println!("      hex: {}", hex_dump_n(&r.data, 40));
        }
    }

    // ============================================================
    // 10. PARA_LINE_SEG DETAIL
    // ============================================================
    println!("\n--- 10. ALL PARA_LINE_SEG DETAILS ---");
    println!("\n  Original PARA_LINE_SEG:");
    for (i, r) in orig_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_LINE_SEG {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_para_line_seg(&r.data)
            );
            println!("      hex: {}", hex_dump_n(&r.data, 60));
        }
    }
    println!("\n  Saved PARA_LINE_SEG:");
    for (i, r) in saved_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_LINE_SEG {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_para_line_seg(&r.data)
            );
            println!("      hex: {}", hex_dump_n(&r.data, 60));
        }
    }

    // ============================================================
    // 11. CTRL_HEADER DETAIL
    // ============================================================
    println!("\n--- 11. ALL CTRL_HEADER DETAILS ---");
    println!("\n  Original CTRL_HEADER:");
    for (i, r) in orig_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_CTRL_HEADER {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_ctrl_header(&r.data)
            );
            println!("      hex: {}", hex_dump_n(&r.data, 60));
        }
    }
    println!("\n  Saved CTRL_HEADER:");
    for (i, r) in saved_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_CTRL_HEADER {
            println!(
                "    [{}] lvl={} sz={}: {}",
                i,
                r.level,
                r.size,
                decode_ctrl_header(&r.data)
            );
            println!("      hex: {}", hex_dump_n(&r.data, 60));
        }
    }

    // ============================================================
    // 12. BinData STREAMS
    // ============================================================
    println!("\n--- 12. BinData STREAMS ---");
    let orig_bins: Vec<_> = orig_entries
        .keys()
        .filter(|k| k.starts_with("/BinData/"))
        .collect();
    let saved_bins: Vec<_> = saved_entries
        .keys()
        .filter(|k| k.starts_with("/BinData/"))
        .collect();
    println!("Original BinData: {} streams", orig_bins.len());
    for p in &orig_bins {
        println!("  {} (size: {})", p, orig_entries[*p].0);
    }
    println!("Saved BinData: {} streams", saved_bins.len());
    for p in &saved_bins {
        println!("  {} (size: {})", p, saved_entries[*p].0);
    }

    // ============================================================
    // 13. FULL RAW HEX DUMP of Section0 for small files
    // ============================================================
    if saved_section.len() <= 2000 {
        println!(
            "\n--- 13. FULL RAW HEX DUMP (Saved Section0, {} bytes) ---",
            saved_section.len()
        );
        for chunk_start in (0..saved_section.len()).step_by(32) {
            let chunk_end = std::cmp::min(chunk_start + 32, saved_section.len());
            print!("  {:06x}: ", chunk_start);
            for i in chunk_start..chunk_end {
                print!("{:02x} ", saved_section[i]);
            }
            // ASCII view
            print!("  ");
            for i in chunk_start..chunk_end {
                let b = saved_section[i];
                if b >= 0x20 && b < 0x7f {
                    print!("{}", b as char);
                } else {
                    print!(".");
                }
            }
            println!();
        }
    }

    if orig_section.len() <= 2000 {
        println!(
            "\n--- 13b. FULL RAW HEX DUMP (Original Section0, {} bytes) ---",
            orig_section.len()
        );
        for chunk_start in (0..orig_section.len()).step_by(32) {
            let chunk_end = std::cmp::min(chunk_start + 32, orig_section.len());
            print!("  {:06x}: ", chunk_start);
            for i in chunk_start..chunk_end {
                print!("{:02x} ", orig_section[i]);
            }
            print!("  ");
            for i in chunk_start..chunk_end {
                let b = orig_section[i];
                if b >= 0x20 && b < 0x7f {
                    print!("{}", b as char);
                } else {
                    print!(".");
                }
            }
            println!();
        }
    }

    // ============================================================
    // 14. DIAGNOSIS SUMMARY
    // ============================================================
    println!("\n{}", "=".repeat(80));
    println!("  DIAGNOSIS SUMMARY");
    println!("{}", "=".repeat(80));

    let mut issues: Vec<String> = Vec::new();

    // Check record count differences
    if orig_records.len() != saved_records.len() {
        issues.push(format!(
            "Record count differs: orig={} saved={}",
            orig_records.len(),
            saved_records.len()
        ));
    }

    // Check for PARA_HEADER with invalid char_count_msb
    for (i, r) in saved_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_HEADER && r.data.len() >= 4 {
            let raw = u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
            let msb = (raw >> 31) & 1;
            let char_count = raw & 0x7FFFFFFF;
            if msb != 0 && msb != 1 {
                issues.push(format!("Record {}: PARA_HEADER invalid msb={}", i, msb));
            }
            // Check if char_count matches PARA_TEXT length
            if let Some(text_rec) = saved_records.get(i + 1) {
                if text_rec.tag_id == tags::HWPTAG_PARA_TEXT {
                    let text_wchars = text_rec.data.len() / 2;
                    // The char_count includes control character widths
                    // (extended ctrls = 8 WCHARs each)
                    if char_count == 0 && text_wchars > 0 {
                        issues.push(format!(
                            "Record {}: PARA_HEADER char_count=0 but PARA_TEXT has {} WCHARs",
                            i, text_wchars
                        ));
                    }
                }
            }
        }
    }

    // Check PARA_LINE_SEG size validity
    for (i, r) in saved_records.iter().enumerate() {
        if r.tag_id == tags::HWPTAG_PARA_LINE_SEG {
            if r.data.len() % 36 != 0 && r.data.len() % 32 != 0 {
                issues.push(format!(
                    "Record {}: PARA_LINE_SEG invalid size {} (not multiple of 32 or 36)",
                    i,
                    r.data.len()
                ));
            }
        }
    }

    // Check for data differences in matching records
    let min_count = std::cmp::min(orig_records.len(), saved_records.len());
    let mut diff_records: Vec<usize> = Vec::new();
    for i in 0..min_count {
        if orig_records[i].tag_id == saved_records[i].tag_id
            && orig_records[i].data != saved_records[i].data
        {
            diff_records.push(i);
        }
    }
    if !diff_records.is_empty() {
        issues.push(format!(
            "Records with data differences (same tag): {:?}",
            diff_records
        ));
    }

    // Check for missing or extra records
    if orig_records.len() > saved_records.len() {
        issues.push(format!(
            "Saved file is MISSING {} records from original",
            orig_records.len() - saved_records.len()
        ));
    } else if saved_records.len() > orig_records.len() {
        issues.push(format!(
            "Saved file has {} EXTRA records compared to original",
            saved_records.len() - orig_records.len()
        ));
    }

    // DocInfo differences
    if orig_docinfo != saved_docinfo {
        issues.push(format!(
            "DocInfo streams differ: orig={} bytes, saved={} bytes",
            orig_docinfo.len(),
            saved_docinfo.len()
        ));
    }

    if issues.is_empty() {
        println!("\n  No obvious issues found.");
    } else {
        println!("\n  Found {} potential issues:", issues.len());
        for (i, issue) in issues.iter().enumerate() {
            println!("  {}. {}", i + 1, issue);
        }
    }

    println!("\n  Analysis complete.");
}



#[test]
fn test_roundtrip_no_edit() {
    // 편집 없이 raw_stream 무효화 → 재직렬화 → 저장
    // 재직렬화 자체에 문제가 있는지 분리 확인
    let path = "template/empty.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let orig_data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    // raw_stream 무효화 (재직렬화 유도)
    doc.document.sections[0].raw_stream = None;

    let saved = doc.export_hwp_native().unwrap();
    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/empty_roundtrip.hwp", &saved).unwrap();
    eprintln!("output/empty_roundtrip.hwp 저장 ({} bytes)", saved.len());

    // 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&saved);
    assert!(doc2.is_ok(), "재파싱 실패: {:?}", doc2.err());

    // 레코드별 비교
    use crate::parser::record::Record;
    let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
    let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
    let orig_bt = orig_cfb
        .read_body_text_section(0, orig_doc.header.compressed, false)
        .unwrap();
    let orig_recs = Record::read_all(&orig_bt).unwrap();

    let saved_doc = crate::parser::parse_hwp(&saved).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_doc.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    eprintln!(
        "원본 레코드: {}, 재직렬화 레코드: {}",
        orig_recs.len(),
        saved_recs.len()
    );

    let max = orig_recs.len().max(saved_recs.len());
    for i in 0..max {
        let o = orig_recs.get(i);
        let s = saved_recs.get(i);
        match (o, s) {
            (Some(or), Some(sr)) => {
                if or.tag_id != sr.tag_id || or.level != sr.level || or.data != sr.data {
                    eprintln!(
                        "DIFF [{}]: tag={}/{} level={}/{} size={}/{}",
                        i,
                        or.tag_id,
                        sr.tag_id,
                        or.level,
                        sr.level,
                        or.data.len(),
                        sr.data.len()
                    );
                    if or.data != sr.data {
                        let show = or.data.len().min(sr.data.len()).min(36);
                        eprintln!("  ORIG: {:02x?}", &or.data[..show]);
                        eprintln!("  SAVE: {:02x?}", &sr.data[..show]);
                        // 첫 번째 다른 바이트 위치
                        for (pos, (a, b)) in or.data.iter().zip(sr.data.iter()).enumerate() {
                            if a != b {
                                eprintln!(
                                    "  First diff at byte {}: 0x{:02x} vs 0x{:02x}",
                                    pos, a, b
                                );
                                break;
                            }
                        }
                    }
                }
            }
            (Some(or), None) => eprintln!("MISSING in saved [{}]: tag={}", i, or.tag_id),
            (None, Some(sr)) => eprintln!("EXTRA in saved [{}]: tag={}", i, sr.tag_id),
            _ => {}
        }
    }
    eprintln!("비교 완료");
}



#[test]
fn test_save_text_only() {
    // 단계 2: 빈 HWP에 텍스트만 삽입 → 저장 → 바이트 비교
    use crate::parser::record::Record;
    use crate::parser::tags;

    let path = "template/empty.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let orig_data = std::fs::read(path).unwrap();

    // 테스트 케이스: (파일명, 삽입 텍스트)
    let test_cases = vec![
        ("save_test_korean.hwp", "가나다라마바사아"),
        ("save_test_english.hwp", "Hello World"),
        ("save_test_mixed.hwp", "안녕 Hello 123 !@#"),
    ];

    for (filename, text) in &test_cases {
        eprintln!("\n{}", "=".repeat(60));
        eprintln!("  테스트: {} → '{}'", filename, text);
        eprintln!("{}", "=".repeat(60));

        let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

        // 텍스트 삽입 (첫 구역, 첫 문단, 캐럿 위치 0)
        let result = doc.insert_text_native(0, 0, 0, text);
        assert!(result.is_ok(), "텍스트 삽입 실패: {:?}", result.err());

        // 삽입 후 문단 상태 확인
        let para = &doc.document.sections[0].paragraphs[0];
        eprintln!(
            "  삽입 후: text='{}' char_count={}",
            para.text, para.char_count
        );
        eprintln!("  char_offsets: {:?}", &para.char_offsets);
        eprintln!(
            "  char_shapes: {:?}",
            para.char_shapes
                .iter()
                .map(|cs| (cs.start_pos, cs.char_shape_id))
                .collect::<Vec<_>>()
        );
        for (i, ls) in para.line_segs.iter().enumerate() {
            eprintln!("  LineSeg[{}]: text_start={} vpos={} lh={} th={} bd={} ls={} cs={} sw={} tag=0x{:08x}",
                    i, ls.text_start, ls.vertical_pos, ls.line_height, ls.text_height,
                    ls.baseline_distance, ls.line_spacing, ls.column_start, ls.segment_width, ls.tag);
        }

        // HWP 저장
        let saved = doc.export_hwp_native();
        assert!(saved.is_ok(), "HWP 저장 실패: {:?}", saved.err());
        let saved_data = saved.unwrap();

        // 파일 출력
        let _ = std::fs::create_dir_all("output");
        let out_path = format!("output/{}", filename);
        std::fs::write(&out_path, &saved_data).unwrap();
        eprintln!("  저장: {} ({} bytes)", out_path, saved_data.len());

        // 재파싱 검증
        let doc2 = HwpDocument::from_bytes(&saved_data);
        assert!(doc2.is_ok(), "재파싱 실패: {:?}", doc2.err());
        let doc2 = doc2.unwrap();
        let para2 = &doc2.document.sections[0].paragraphs[0];
        eprintln!(
            "  재파싱: text='{}' char_count={}",
            para2.text, para2.char_count
        );
        assert!(
            para2.text.contains(text),
            "재파싱 텍스트 불일치: expected '{}', got '{}'",
            text,
            para2.text
        );

        // 캐럿 위치 검증
        let caret = &doc2.document.doc_properties;
        eprintln!(
            "  캐럿: list_id={} para_id={} char_pos={}",
            caret.caret_list_id, caret.caret_para_id, caret.caret_char_pos
        );
        // 삽입 후 캐럿은 텍스트 마지막 글자 뒤여야 함
        let expected_caret_pos = 16u32
            + text
                .chars()
                .map(|c| if (c as u32) > 0xFFFF { 2u32 } else { 1u32 })
                .sum::<u32>();
        assert_eq!(
            caret.caret_char_pos, expected_caret_pos,
            "캐럿 위치 불일치: expected {} got {}",
            expected_caret_pos, caret.caret_char_pos
        );

        // BodyText 레코드 비교 (원본 vs 저장)
        let orig_doc = crate::parser::parse_hwp(&orig_data).unwrap();
        let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
        let orig_bt = orig_cfb
            .read_body_text_section(0, orig_doc.header.compressed, false)
            .unwrap();
        let orig_recs = Record::read_all(&orig_bt).unwrap();

        let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();
        let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
        let saved_bt = saved_cfb
            .read_body_text_section(0, saved_doc.header.compressed, false)
            .unwrap();
        let saved_recs = Record::read_all(&saved_bt).unwrap();

        eprintln!(
            "\n  --- 레코드 비교 (원본: {} / 저장: {}) ---",
            orig_recs.len(),
            saved_recs.len()
        );
        let tag_name = |id: u16| -> &str {
            match id {
                66 => "PARA_HEADER",
                67 => "PARA_TEXT",
                68 => "PARA_CHAR_SHAPE",
                69 => "PARA_LINE_SEG",
                70 => "CTRL_HEADER",
                71 => "LIST_HEADER",
                _ => "OTHER",
            }
        };

        let max = orig_recs.len().max(saved_recs.len());
        for i in 0..max {
            let o = orig_recs.get(i);
            let s = saved_recs.get(i);
            match (o, s) {
                (Some(or), Some(sr)) => {
                    let same = or.tag_id == sr.tag_id && or.level == sr.level && or.data == sr.data;
                    let status = if same { "OK  " } else { "DIFF" };
                    eprintln!(
                        "  [{}] {} tag={:3}({}) level={}/{} size={}/{}",
                        i,
                        status,
                        or.tag_id,
                        tag_name(or.tag_id),
                        or.level,
                        sr.level,
                        or.data.len(),
                        sr.data.len()
                    );
                    if !same {
                        let show = or.data.len().min(sr.data.len()).min(48);
                        let orig_hex: String = or.data[..show]
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let save_hex: String = sr.data[..show]
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        eprintln!("         ORIG: {}", orig_hex);
                        eprintln!("         SAVE: {}", save_hex);
                    }
                }
                (Some(or), None) => eprintln!(
                    "  [{}] MISSING tag={}({})",
                    i,
                    or.tag_id,
                    tag_name(or.tag_id)
                ),
                (None, Some(sr)) => eprintln!(
                    "  [{}] EXTRA   tag={}({})",
                    i,
                    sr.tag_id,
                    tag_name(sr.tag_id)
                ),
                _ => {}
            }
        }
    }
    eprintln!("\n=== 단계 2 텍스트 저장 검증 완료 ===");
}



/// 단계 4-2: 빈 HWP에 이미지 삽입 후 저장 검증
/// 참조: output/pic-01-as-text.hwp (HWP 프로그램으로 생성, 3tigers.jpg 글자처리 삽입)
#[test]
fn test_save_picture() {
    use crate::model::bin_data::{
        BinData, BinDataCompression, BinDataContent, BinDataStatus, BinDataType,
    };
    use crate::model::control::Control;
    use crate::model::paragraph::{CharShapeRef, LineSeg, Paragraph};
    use crate::parser::record::Record;
    use crate::parser::tags;

    eprintln!("\n=== 단계 4-2: 이미지 저장 검증 시작 ===");

    // 1. 참조 파일에서 Picture 구조 및 이미지 추출
    let ref_path = "output/pic-01-as-text.hwp";
    if !std::path::Path::new(ref_path).exists() {
        eprintln!("SKIP: {} 없음", ref_path);
        return;
    }
    let ref_data = std::fs::read(ref_path).unwrap();
    let ref_doc = HwpDocument::from_bytes(&ref_data).unwrap();

    // 참조 파일에서 Picture 컨트롤 추출
    let ref_pic = ref_doc.document.sections[0].paragraphs[0]
        .controls
        .iter()
        .find_map(|c| {
            if let Control::Picture(p) = c {
                Some(p)
            } else {
                None
            }
        })
        .expect("참조 파일에 Picture 컨트롤 없음");

    let ref_bindata = &ref_doc.document.doc_info.bin_data_list[0];
    let ref_bincontent = &ref_doc.document.bin_data_content[0];

    let pic_width = ref_pic.common.width;
    let pic_height = ref_pic.common.height;
    eprintln!(
        "  참조 Picture: {}×{} bin_data_id={} image={} bytes",
        pic_width,
        pic_height,
        ref_pic.image_attr.bin_data_id,
        ref_bincontent.data.len()
    );
    eprintln!(
        "  참조 캐럿: list_id={} para_id={} char_pos={}",
        ref_doc.document.doc_properties.caret_list_id,
        ref_doc.document.doc_properties.caret_para_id,
        ref_doc.document.doc_properties.caret_char_pos
    );

    // 2. empty.hwp 로드
    let empty_path = "template/empty.hwp";
    assert!(
        std::path::Path::new(empty_path).exists(),
        "template/empty.hwp 없음"
    );
    let empty_data = std::fs::read(empty_path).unwrap();
    let mut doc = HwpDocument::from_bytes(&empty_data).unwrap();

    // 3. DocInfo에 BinData 추가
    // 참조 파일: attr=0x0001 (Embedding), status=NotAccessed
    let bin_data_entry = BinData {
        attr: ref_bindata.attr,
        data_type: BinDataType::Embedding,
        compression: BinDataCompression::Default,
        status: BinDataStatus::NotAccessed, // 참조 파일과 동일
        storage_id: 1,
        extension: Some(ref_bincontent.extension.clone()),
        raw_data: None,
        ..Default::default()
    };
    doc.document.doc_info.bin_data_list.push(bin_data_entry);

    // BinDataContent 추가
    doc.document.bin_data_content.push(BinDataContent {
        id: 1,
        data: ref_bincontent.data.clone(),
        extension: ref_bincontent.extension.clone(),
    });

    // 4. Picture 컨트롤 구성 (참조 파일의 정확한 값 사용)
    let picture = crate::model::image::Picture {
        common: ref_pic.common.clone(),
        shape_attr: ref_pic.shape_attr.clone(),
        border_color: ref_pic.border_color,
        border_width: ref_pic.border_width,
        border_attr: ref_pic.border_attr.clone(),
        border_x: ref_pic.border_x,
        border_y: ref_pic.border_y,
        crop: ref_pic.crop.clone(),
        padding: ref_pic.padding.clone(),
        image_attr: ref_pic.image_attr.clone(),
        href: ref_pic.href.clone(),
        border_opacity: ref_pic.border_opacity,
        instance_id: ref_pic.instance_id,
        raw_picture_extra: ref_pic.raw_picture_extra.clone(),
        effects: ref_pic.effects.clone(),
        caption: None,
        img_dim: (0, 0),
        reverse: ref_pic.reverse,
        lock: false,
    };

    // 5. 문단 구성 (참조 파일: 단일 문단에 SectionDef + ColumnDef + Picture)
    let first_para = &doc.document.sections[0].paragraphs[0];
    let existing_controls: Vec<Control> = first_para.controls.clone();

    // 참조: char_count=25 (msb=true), control_mask=0x00000804
    // PARA_TEXT: secd(0~7) + cold(8~15) + gso(16~23) + CR(24) = 25 chars
    let mut new_controls = existing_controls;
    new_controls.push(Control::Picture(Box::new(picture)));

    // 참조 문단의 정확한 값 사용
    let ref_para = &ref_doc.document.sections[0].paragraphs[0];
    let pic_para = Paragraph {
        text: String::new(),
        char_count: 25,       // secd(8) + cold(8) + gso(8) + CR(1) = 25
        char_count_msb: true, // 참조: msb=true
        control_mask: 0x00000804,
        para_shape_id: first_para.para_shape_id, // empty.hwp 기본값 사용
        style_id: first_para.style_id,
        raw_break_type: ref_para.raw_break_type, // 참조: 0x03
        char_shapes: vec![CharShapeRef {
            start_pos: 0,
            char_shape_id: first_para
                .char_shapes
                .first()
                .map(|cs| cs.char_shape_id)
                .unwrap_or(0),
        }],
        line_segs: vec![LineSeg {
            text_start: 0,
            vertical_pos: 0,
            line_height: ref_para.line_segs[0].line_height, // 참조: 14775 (=이미지 높이)
            text_height: ref_para.line_segs[0].text_height,
            baseline_distance: ref_para.line_segs[0].baseline_distance,
            line_spacing: ref_para.line_segs[0].line_spacing,
            column_start: 0,
            segment_width: ref_para.line_segs[0].segment_width, // 참조: 42520
            tag: ref_para.line_segs[0].tag, // 참조: LineSeg::TAG_SINGLE_SEGMENT_LINE
        }],
        has_para_text: true,
        controls: new_controls,
        raw_header_extra: first_para.raw_header_extra.clone(),
        ..Default::default()
    };

    // 참조: 문단 1개만 (참조 파일에는 두 번째 문단이 없음)
    doc.document.sections[0].paragraphs = vec![pic_para];

    // 6. raw_stream 무효화 (재직렬화)
    doc.document.sections[0].raw_stream = None;
    doc.document.doc_info.raw_stream = None;
    doc.document.doc_properties.raw_data = None;

    // 캐럿 위치 (참조: list_id=0, para_id=0, char_pos=24)
    doc.document.doc_properties.caret_list_id = 0;
    doc.document.doc_properties.caret_para_id = 0;
    doc.document.doc_properties.caret_char_pos = 24;

    // 7. 저장
    let saved = doc.export_hwp_native();
    assert!(saved.is_ok(), "HWP 저장 실패: {:?}", saved.err());
    let saved_data = saved.unwrap();

    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/save_test_picture.hwp", &saved_data).unwrap();
    eprintln!(
        "  저장: output/save_test_picture.hwp ({} bytes)",
        saved_data.len()
    );

    // 8. 재파싱 검증
    let doc2 = HwpDocument::from_bytes(&saved_data);
    assert!(doc2.is_ok(), "재파싱 실패: {:?}", doc2.err());
    let doc2 = doc2.unwrap();

    // Picture 컨트롤 존재 검증
    let para2 = &doc2.document.sections[0].paragraphs[0];
    eprintln!(
        "  재파싱: char_count={} msb={} controls={}",
        para2.char_count,
        para2.char_count_msb,
        para2.controls.len()
    );
    let pic_found = para2
        .controls
        .iter()
        .any(|c| matches!(c, Control::Picture(_)));
    assert!(pic_found, "재파싱된 문서에 Picture 컨트롤이 없음");

    // Picture 속성 검증
    if let Some(Control::Picture(p)) = para2
        .controls
        .iter()
        .find(|c| matches!(c, Control::Picture(_)))
    {
        eprintln!(
            "  Picture: {}×{} bin_data_id={}",
            p.common.width, p.common.height, p.image_attr.bin_data_id
        );
        eprintln!("    border_x={:?} border_y={:?}", p.border_x, p.border_y);
        eprintln!(
            "    crop: L={} T={} R={} B={}",
            p.crop.left, p.crop.top, p.crop.right, p.crop.bottom
        );
        eprintln!(
            "    shape_attr: ctrl_id=0x{:08X} two={} orig={}×{} cur={}×{}",
            p.shape_attr.ctrl_id,
            p.shape_attr.is_two_ctrl_id,
            p.shape_attr.original_width,
            p.shape_attr.original_height,
            p.shape_attr.current_width,
            p.shape_attr.current_height
        );
        assert_eq!(p.image_attr.bin_data_id, 1);
        assert_eq!(p.common.width, pic_width);
        assert_eq!(p.common.height, pic_height);
    }

    // BinData 검증
    assert_eq!(
        doc2.document.doc_info.bin_data_list.len(),
        1,
        "BinData 없음"
    );
    assert_eq!(
        doc2.document.doc_info.bin_data_list[0].data_type,
        BinDataType::Embedding
    );
    assert_eq!(doc2.document.doc_info.bin_data_list[0].storage_id, 1);

    // BinDataContent 검증
    assert_eq!(
        doc2.document.bin_data_content.len(),
        1,
        "BinDataContent 없음"
    );
    assert_eq!(
        doc2.document.bin_data_content[0].data.len(),
        ref_bincontent.data.len(),
        "이미지 데이터 크기 불일치"
    );

    // 캐럿 위치 검증
    eprintln!(
        "  캐럿: list_id={} para_id={} char_pos={}",
        doc2.document.doc_properties.caret_list_id,
        doc2.document.doc_properties.caret_para_id,
        doc2.document.doc_properties.caret_char_pos
    );

    // 9. 저장 레코드 덤프 (참조 파일과 비교)
    let saved_parsed = crate::parser::parse_hwp(&saved_data).unwrap();
    let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved_data).unwrap();
    let saved_bt = saved_cfb
        .read_body_text_section(0, saved_parsed.header.compressed, false)
        .unwrap();
    let saved_recs = Record::read_all(&saved_bt).unwrap();

    // 참조 파일 레코드도 덤프
    let ref_parsed = crate::parser::parse_hwp(&ref_data).unwrap();
    let mut ref_cfb = crate::parser::cfb_reader::CfbReader::open(&ref_data).unwrap();
    let ref_bt = ref_cfb
        .read_body_text_section(0, ref_parsed.header.compressed, false)
        .unwrap();
    let ref_recs = Record::read_all(&ref_bt).unwrap();

    eprintln!(
        "\n  --- 레코드 비교 (참조={} 개, 저장={} 개) ---",
        ref_recs.len(),
        saved_recs.len()
    );
    let max_recs = ref_recs.len().max(saved_recs.len());
    for i in 0..max_recs {
        let ref_info = if i < ref_recs.len() {
            let r = &ref_recs[i];
            format!(
                "tag={:3}({:22}) lv={} sz={}",
                r.tag_id,
                tags::tag_name(r.tag_id),
                r.level,
                r.data.len()
            )
        } else {
            "---".to_string()
        };
        let saved_info = if i < saved_recs.len() {
            let r = &saved_recs[i];
            format!(
                "tag={:3}({:22}) lv={} sz={}",
                r.tag_id,
                tags::tag_name(r.tag_id),
                r.level,
                r.data.len()
            )
        } else {
            "---".to_string()
        };
        let match_mark = if i < ref_recs.len() && i < saved_recs.len() {
            let r = &ref_recs[i];
            let s = &saved_recs[i];
            if r.tag_id == s.tag_id && r.level == s.level && r.data.len() == s.data.len() {
                if r.data == s.data {
                    "=="
                } else {
                    "~="
                }
            } else {
                "!="
            }
        } else {
            "!="
        };
        eprintln!("  [{:2}] {} {} | {}", i, match_mark, ref_info, saved_info);
    }

    // 주요 레코드 바이트 비교
    for i in 0..ref_recs.len().min(saved_recs.len()) {
        let r = &ref_recs[i];
        let s = &saved_recs[i];
        if r.tag_id == s.tag_id && r.data != s.data {
            eprintln!("\n  [차이 상세] 레코드 {}: {}", i, tags::tag_name(r.tag_id));
            let max_show = r.data.len().max(s.data.len()).min(120);
            eprintln!("    참조: {:02x?}", &r.data[..r.data.len().min(max_show)]);
            eprintln!("    저장: {:02x?}", &s.data[..s.data.len().min(max_show)]);
            // 첫 번째 차이 위치
            for j in 0..r.data.len().min(s.data.len()) {
                if r.data[j] != s.data[j] {
                    eprintln!(
                        "    첫 차이: offset {} (참조=0x{:02x}, 저장=0x{:02x})",
                        j, r.data[j], s.data[j]
                    );
                    break;
                }
            }
        }
    }

    // CFB 스트림 목록 확인
    let streams = saved_cfb.list_streams();
    eprintln!("\n  --- CFB 스트림 목록 ---");
    for s in &streams {
        eprintln!("  {}", s);
    }
    let has_bindata = streams
        .iter()
        .any(|s| s.contains("BinData") || s.contains("BIN"));
    assert!(has_bindata, "BinData 스트림이 없음");

    eprintln!("\n=== 단계 4-2 이미지 저장 검증 완료 ===");
}



/// 단계 5: 기타 컨트롤 라운드트립 검증
/// 여러 샘플 파일에서 Header/Footer/Footnote/Endnote/Shape/Bookmark 라운드트립
#[test]
fn test_roundtrip_all_controls() {
    use crate::model::control::Control;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let samples = [
        "samples/k-water-rfp.hwp",
        "samples/20250130-hongbo.hwp",
        "samples/hwp-multi-001.hwp",
        "samples/hwp-multi-002.hwp",
        "samples/2010-01-06.hwp",
    ];

    eprintln!("\n{}", "=".repeat(70));
    eprintln!("  단계 5: 기타 컨트롤 라운드트립 검증");
    eprintln!("{}", "=".repeat(70));

    for sample_path in &samples {
        if !std::path::Path::new(sample_path).exists() {
            eprintln!("\n  SKIP: {} 없음", sample_path);
            continue;
        }

        let orig_data = std::fs::read(sample_path).unwrap();
        let doc = match HwpDocument::from_bytes(&orig_data) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("\n  SKIP: {} 파싱 실패: {}", sample_path, e);
                continue;
            }
        };

        // 컨트롤 종류 카운트
        let mut ctrl_counts = std::collections::HashMap::new();
        for sec in &doc.document.sections {
            fn count_controls(
                paras: &[crate::model::paragraph::Paragraph],
                counts: &mut std::collections::HashMap<String, usize>,
            ) {
                for para in paras {
                    for ctrl in &para.controls {
                        let name = match ctrl {
                            Control::SectionDef(_) => "SectionDef",
                            Control::ColumnDef(_) => "ColumnDef",
                            Control::Table(t) => {
                                // 표 안의 컨트롤도 카운트
                                for cell in &t.cells {
                                    count_controls(&cell.paragraphs, counts);
                                }
                                "Table"
                            }
                            Control::Picture(_) => "Picture",
                            Control::Shape(_) => "Shape",
                            Control::Header(h) => {
                                count_controls(&h.paragraphs, counts);
                                "Header"
                            }
                            Control::Footer(f) => {
                                count_controls(&f.paragraphs, counts);
                                "Footer"
                            }
                            Control::Footnote(f) => {
                                count_controls(&f.paragraphs, counts);
                                "Footnote"
                            }
                            Control::Endnote(e) => {
                                count_controls(&e.paragraphs, counts);
                                "Endnote"
                            }
                            Control::HiddenComment(_) => "HiddenComment",
                            Control::AutoNumber(_) => "AutoNumber",
                            Control::NewNumber(_) => "NewNumber",
                            Control::PageNumberPos(_) => "PageNumberPos",
                            Control::Bookmark(_) => "Bookmark",
                            _ => "Other",
                        };
                        *counts.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
            }
            count_controls(&sec.paragraphs, &mut ctrl_counts);
        }

        // 관심 대상 컨트롤만 필터링
        let target_ctrls = [
            "Header", "Footer", "Footnote", "Endnote", "Shape", "Bookmark", "Picture", "Table",
        ];
        let has_target = target_ctrls.iter().any(|t| ctrl_counts.contains_key(*t));

        eprintln!("\n  --- {} ---", sample_path);
        eprintln!(
            "  섹션: {} 문단: {}",
            doc.document.sections.len(),
            doc.document
                .sections
                .iter()
                .map(|s| s.paragraphs.len())
                .sum::<usize>()
        );
        eprintln!("  컨트롤: {:?}", ctrl_counts);

        if !has_target {
            eprintln!("  → 대상 컨트롤 없음, 건너뜀");
            continue;
        }

        // 라운드트립: 원본 → 재직렬화 → 저장 → 재파싱
        let mut doc_mut = doc;
        for sec in &mut doc_mut.document.sections {
            sec.raw_stream = None;
        }
        doc_mut.document.doc_info.raw_stream = None;
        doc_mut.document.doc_properties.raw_data = None;

        let saved = match doc_mut.export_hwp_native() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  저장 실패: {}", e);
                continue;
            }
        };

        // 재파싱
        let doc2 = match HwpDocument::from_bytes(&saved) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  재파싱 실패: {}", e);
                // 저장 파일 기록 (디버그용)
                let fname = format!(
                    "output/roundtrip_fail_{}.hwp",
                    std::path::Path::new(sample_path)
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                );
                let _ = std::fs::create_dir_all("output");
                std::fs::write(&fname, &saved).unwrap();
                eprintln!("  디버그 파일: {} ({} bytes)", fname, saved.len());
                continue;
            }
        };

        // 재파싱 후 컨트롤 카운트 비교
        let mut ctrl_counts2 = std::collections::HashMap::new();
        for sec in &doc2.document.sections {
            fn count_controls2(
                paras: &[crate::model::paragraph::Paragraph],
                counts: &mut std::collections::HashMap<String, usize>,
            ) {
                for para in paras {
                    for ctrl in &para.controls {
                        let name = match ctrl {
                            Control::SectionDef(_) => "SectionDef",
                            Control::ColumnDef(_) => "ColumnDef",
                            Control::Table(t) => {
                                for cell in &t.cells {
                                    count_controls2(&cell.paragraphs, counts);
                                }
                                "Table"
                            }
                            Control::Picture(_) => "Picture",
                            Control::Shape(_) => "Shape",
                            Control::Header(h) => {
                                count_controls2(&h.paragraphs, counts);
                                "Header"
                            }
                            Control::Footer(f) => {
                                count_controls2(&f.paragraphs, counts);
                                "Footer"
                            }
                            Control::Footnote(f) => {
                                count_controls2(&f.paragraphs, counts);
                                "Footnote"
                            }
                            Control::Endnote(e) => {
                                count_controls2(&e.paragraphs, counts);
                                "Endnote"
                            }
                            Control::HiddenComment(_) => "HiddenComment",
                            Control::AutoNumber(_) => "AutoNumber",
                            Control::NewNumber(_) => "NewNumber",
                            Control::PageNumberPos(_) => "PageNumberPos",
                            Control::Bookmark(_) => "Bookmark",
                            _ => "Other",
                        };
                        *counts.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
            }
            count_controls2(&sec.paragraphs, &mut ctrl_counts2);
        }

        // 대상 컨트롤별 보존 여부 확인
        let mut all_match = true;
        for target in &target_ctrls {
            let orig_count = ctrl_counts.get(*target).copied().unwrap_or(0);
            let saved_count = ctrl_counts2.get(*target).copied().unwrap_or(0);
            if orig_count > 0 || saved_count > 0 {
                let status = if orig_count == saved_count {
                    "✓"
                } else {
                    "✗"
                };
                eprintln!(
                    "  {:12} 원본={:2} 저장={:2} {}",
                    target, orig_count, saved_count, status
                );
                if orig_count != saved_count {
                    all_match = false;
                }
            }
        }

        // 레코드 수 비교 (섹션 0)
        let orig_parsed = crate::parser::parse_hwp(&orig_data).unwrap();
        let mut orig_cfb = crate::parser::cfb_reader::CfbReader::open(&orig_data).unwrap();
        let orig_bt = orig_cfb
            .read_body_text_section(0, orig_parsed.header.compressed, false)
            .unwrap();
        let orig_recs = Record::read_all(&orig_bt).unwrap();

        let saved_parsed = crate::parser::parse_hwp(&saved).unwrap();
        let mut saved_cfb = crate::parser::cfb_reader::CfbReader::open(&saved).unwrap();
        let saved_bt = saved_cfb
            .read_body_text_section(0, saved_parsed.header.compressed, false)
            .unwrap();
        let saved_recs = Record::read_all(&saved_bt).unwrap();

        eprintln!(
            "  레코드: 원본={} 저장={} {}",
            orig_recs.len(),
            saved_recs.len(),
            if orig_recs.len() == saved_recs.len() {
                "✓"
            } else {
                "✗"
            }
        );

        // 레코드 차이 요약 (다른 레코드만)
        let mut diff_count = 0;
        let max_recs = orig_recs.len().min(saved_recs.len());
        for i in 0..max_recs {
            let o = &orig_recs[i];
            let s = &saved_recs[i];
            if o.tag_id != s.tag_id || o.level != s.level || o.data != s.data {
                diff_count += 1;
                if diff_count <= 5 {
                    let match_type = if o.tag_id != s.tag_id || o.level != s.level {
                        "구조"
                    } else {
                        "데이터"
                    };
                    eprintln!(
                        "  DIFF[{:3}] {} {} lv{} sz{} vs {} lv{} sz{}",
                        i,
                        match_type,
                        tags::tag_name(o.tag_id),
                        o.level,
                        o.data.len(),
                        tags::tag_name(s.tag_id),
                        s.level,
                        s.data.len()
                    );
                }
            }
        }
        if diff_count > 5 {
            eprintln!("  ... 외 {} 개 차이", diff_count - 5);
        }
        eprintln!(
            "  일치: {}/{} 레코드 ({}%)",
            max_recs - diff_count,
            max_recs,
            if max_recs > 0 {
                (max_recs - diff_count) * 100 / max_recs
            } else {
                100
            }
        );

        if all_match {
            eprintln!("  → 라운드트립 성공 ✓");
        }
    }

    eprintln!("\n=== 단계 5 기타 컨트롤 라운드트립 검증 완료 ===");
}



/// 엔터 2회 후 저장 시 파일 손상 재현 진단 테스트
#[test]
fn test_diag_double_enter_save() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    let path = "samples/20250130-hongbo.hwp";
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: {} 없음", path);
        return;
    }

    let data = std::fs::read(path).unwrap();
    let mut doc = HwpDocument::from_bytes(&data).unwrap();

    eprintln!("=== 엔터 2회 후 저장 파일 손상 진단 ===");
    let section = &doc.document.sections[0];
    eprintln!("원본 문단 수: {}", section.paragraphs.len());

    // 텍스트가 있고 컨트롤이 없는 문단 찾기
    let mut target_para = 0;
    for (i, p) in section.paragraphs.iter().enumerate() {
        eprintln!(
            "  문단[{}]: text_len={} cc={} ctrl={} has_pt={}",
            i,
            p.text.chars().count(),
            p.char_count,
            p.controls.len(),
            p.has_para_text
        );
        if p.text.chars().count() >= 10 && p.controls.is_empty() && target_para == 0 {
            target_para = i;
        }
    }
    assert!(target_para > 0, "텍스트가 있는 문단을 찾을 수 없음");
    let para = &section.paragraphs[target_para];
    let text_len = para.text.chars().count();
    eprintln!(
        "\n대상 문단[{}]: text_len={} cc={} controls={} has_para_text={}",
        target_para,
        text_len,
        para.char_count,
        para.controls.len(),
        para.has_para_text
    );
    eprintln!(
        "  text(앞40)='{}'",
        para.text.chars().take(40).collect::<String>()
    );

    let split_offset = 4; // 4번째 글자 뒤에서 분할 (사용자 시나리오)

    // === 엔터 1회 ===
    let result1 = doc.split_paragraph_native(0, target_para, split_offset, None);
    assert!(result1.is_ok(), "1차 분할 실패: {:?}", result1.err());
    eprintln!("\n--- 1차 분할 (offset={}) ---", split_offset);

    let section = &doc.document.sections[0];
    for i in target_para..=(target_para + 1).min(section.paragraphs.len() - 1) {
        let p = &section.paragraphs[i];
        eprintln!(
            "  문단[{}]: cc={} text_len={} controls={} has_para_text={} line_segs={}",
            i,
            p.char_count,
            p.text.chars().count(),
            p.controls.len(),
            p.has_para_text,
            p.line_segs.len()
        );
    }

    // 1회 분할 후 저장 테스트
    let saved1 = doc.export_hwp_native();
    assert!(saved1.is_ok(), "1차 저장 실패");
    let saved1_data = saved1.unwrap();
    let parse1 = HwpDocument::from_bytes(&saved1_data);
    eprintln!(
        "1회 분할 후 저장+재파싱: {}",
        if parse1.is_ok() { "성공" } else { "실패" }
    );

    // === 엔터 2회 (새 문단의 시작에서 다시 분할) ===
    let new_para_idx = target_para + 1;
    let result2 = doc.split_paragraph_native(0, new_para_idx, 0, None);
    assert!(result2.is_ok(), "2차 분할 실패: {:?}", result2.err());
    eprintln!("\n--- 2차 분할 (문단[{}], offset=0) ---", new_para_idx);

    let section = &doc.document.sections[0];
    eprintln!("문단 수: {}", section.paragraphs.len());
    for i in target_para..=(target_para + 2).min(section.paragraphs.len() - 1) {
        let p = &section.paragraphs[i];
        eprintln!(
            "  문단[{}]: cc={} text_len={} controls={} has_para_text={} raw_extra_len={}",
            i,
            p.char_count,
            p.text.chars().count(),
            p.controls.len(),
            p.has_para_text,
            p.raw_header_extra.len()
        );
    }

    // 2회 분할 후 저장 테스트
    let saved2 = doc.export_hwp_native();
    assert!(saved2.is_ok(), "2차 저장 실패");
    let saved2_data = saved2.unwrap();

    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/diag_double_enter.hwp", &saved2_data).unwrap();
    eprintln!(
        "\noutput/diag_double_enter.hwp 저장 ({} bytes)",
        saved2_data.len()
    );

    // 재파싱 테스트
    let parse2 = HwpDocument::from_bytes(&saved2_data);
    eprintln!(
        "2회 분할 후 저장+재파싱: {}",
        if parse2.is_ok() { "성공" } else { "실패" }
    );

    // 직렬화된 Section0 레코드 분석 - 분할 영역 주변만 상세 출력
    eprintln!("\n=== Section0 직렬화 레코드 분석 (level 0만, 분할 영역) ===");
    let section_bytes = crate::serializer::body_text::serialize_section(&doc.document.sections[0]);
    let recs = Record::read_all(&section_bytes).unwrap();
    let mut top_para_idx = 0;
    for (ri, rec) in recs.iter().enumerate() {
        if rec.tag_id == tags::HWPTAG_PARA_HEADER && rec.level == 0 {
            let cc_raw = u32::from_le_bytes(rec.data[0..4].try_into().unwrap());
            let cc = cc_raw & 0x7FFFFFFF;
            let msb = cc_raw & 0x80000000 != 0;
            let ctrl_mask = u32::from_le_bytes(rec.data[4..8].try_into().unwrap());
            // 분할 영역 (target_para-1 ~ target_para+4) 표시
            if top_para_idx >= target_para.saturating_sub(1) && top_para_idx <= target_para + 4 {
                eprintln!(
                    "rec[{}] PARA_HEADER(L0): model_para={} cc={} msb={} ctrl=0x{:08X}",
                    ri, top_para_idx, cc, msb, ctrl_mask
                );
            }
            top_para_idx += 1;
        } else if rec.tag_id == tags::HWPTAG_PARA_TEXT && rec.level == 1 {
            // 바로 앞의 PARA_HEADER가 분할 영역이면 표시
            if top_para_idx > target_para.saturating_sub(1) && top_para_idx <= target_para + 5 {
                let code_units = rec.data.len() / 2;
                eprintln!(
                    "rec[{}]   PARA_TEXT(L1): {} code_units ({} bytes)",
                    ri,
                    code_units,
                    rec.data.len()
                );
            }
        } else if rec.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE && rec.level == 1 {
            if top_para_idx > target_para.saturating_sub(1) && top_para_idx <= target_para + 5 {
                let entries = rec.data.len() / 8;
                eprintln!("rec[{}]   PARA_CHAR_SHAPE(L1): {} entries", ri, entries);
            }
        } else if rec.tag_id == tags::HWPTAG_PARA_LINE_SEG && rec.level == 1 {
            if top_para_idx > target_para.saturating_sub(1) && top_para_idx <= target_para + 5 {
                let entries = rec.data.len() / 36;
                eprintln!("rec[{}]   PARA_LINE_SEG(L1): {} entries", ri, entries);
            }
        }
    }
    eprintln!("총 top-level 문단: {}", top_para_idx);

    if parse2.is_err() {
        panic!("2회 분할 후 저장된 파일 재파싱 실패!");
    }
}



#[test]
fn test_single_command_emits_event() {
    let mut doc = create_editable_doc();
    assert!(doc.event_log.is_empty());

    let result = doc.insert_text_native(0, 0, 0, "Hello");
    assert!(result.is_ok(), "insert_text_native failed: {:?}", result);
    assert_eq!(doc.event_log.len(), 1);

    let json = doc.event_log[0].to_json();
    assert!(json.contains("\"type\":\"TextInserted\""));
    assert!(json.contains("\"section\":0"));
    assert!(json.contains("\"para\":0"));
    assert!(json.contains("\"offset\":0"));
    assert!(json.contains("\"len\":5"));
}



#[test]
fn test_batch_mode_events_collected() {
    let mut doc = create_editable_doc();

    let r = doc.begin_batch_native();
    assert!(r.is_ok());
    assert!(doc.batch_mode);
    assert!(doc.event_log.is_empty());

    // Batch 중 여러 편집
    let r1 = doc.insert_text_native(0, 0, 0, "Hello");
    assert!(r1.is_ok(), "1st insert failed: {:?}", r1);
    let r2 = doc.insert_text_native(0, 0, 5, " World");
    assert!(r2.is_ok(), "2nd insert failed: {:?}", r2);

    assert_eq!(doc.event_log.len(), 2);
    assert!(doc.event_log[0]
        .to_json()
        .contains("\"type\":\"TextInserted\""));
    assert!(doc.event_log[1]
        .to_json()
        .contains("\"type\":\"TextInserted\""));
}



#[test]
fn test_end_batch_returns_events_and_clears() {
    let mut doc = create_editable_doc();

    let _ = doc.begin_batch_native();
    let r = doc.insert_text_native(0, 0, 0, "Test");
    assert!(r.is_ok(), "insert failed: {:?}", r);
    assert_eq!(doc.event_log.len(), 1);

    let result = doc.end_batch_native();
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"events\":["));
    assert!(json.contains("\"type\":\"TextInserted\""));

    // end_batch 후 event_log 비워짐 + batch_mode 해제
    assert!(!doc.batch_mode);
    assert!(doc.event_log.is_empty());
}



#[test]
fn test_batch_multiple_edit_types() {
    let mut doc = create_editable_doc();

    let _ = doc.begin_batch_native();
    let r1 = doc.insert_text_native(0, 0, 0, "Hello World");
    assert!(r1.is_ok(), "insert failed: {:?}", r1);
    let r2 = doc.delete_text_native(0, 0, 5, 6);
    assert!(r2.is_ok(), "delete failed: {:?}", r2);

    assert_eq!(doc.event_log.len(), 2);
    assert!(doc.event_log[0]
        .to_json()
        .contains("\"type\":\"TextInserted\""));
    assert!(doc.event_log[1]
        .to_json()
        .contains("\"type\":\"TextDeleted\""));

    let result = doc.end_batch_native();
    assert!(result.is_ok());
    // 종료 후 paginate 실행되므로 페이지 수 유효
    assert!(doc.page_count() >= 1);
}



#[test]
fn test_serialize_event_log_format() {
    let mut doc = create_editable_doc();
    let r = doc.insert_text_native(0, 0, 0, "A");
    assert!(r.is_ok(), "insert failed: {:?}", r);

    let json = doc.serialize_event_log();
    assert!(json.starts_with("{\"ok\":true,\"events\":["));
    assert!(json.ends_with("]}"));
    assert!(json.contains("\"type\":\"TextInserted\""));
}



/// [진단용] HWP 파일의 모든 ClickHere 필드 command + CTRL_DATA 덤프
#[test]
fn diag_dump_all_clickhere_commands() {
    // field-01.hwp와 saved/field-02.hwp 비교
    for path in &["samples/field-01.hwp", "saved/field-02.hwp"] {
        eprintln!("\n=== {} ===", path);
        let Ok(data) = std::fs::read(path) else {
            eprintln!("  (파일 없음)");
            continue;
        };
        let Ok(doc) = HwpDocument::from_bytes(&data) else {
            eprintln!("  (파싱 실패)");
            continue;
        };
        dump_clickhere_fields(&doc);
    }
}


fn dump_clickhere_fields(doc: &HwpDocument) {
    use crate::model::control::{Control, FieldType};
    for (si, sec) in doc.document.sections.iter().enumerate() {
        for (pi, para) in sec.paragraphs.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                if let Control::Field(f) = ctrl {
                    if f.field_type == FieldType::ClickHere {
                        eprintln!(
                            "[sec={} para={} ctrl={}] id={} ctrl_data_name={:?}",
                            si, pi, ci, f.field_id, f.ctrl_data_name
                        );
                        eprintln!("  command={:?}", f.command);
                        // CTRL_DATA 확인
                        if let Some(Some(cd)) = para.ctrl_data_records.get(ci) {
                            eprintln!(
                                "  CTRL_DATA({} bytes): {:02x?}",
                                cd.len(),
                                &cd[..cd.len().min(24)]
                            );
                        } else {
                            eprintln!("  CTRL_DATA: None");
                        }
                    }
                }
            }
        }
    }
}



/// [진단] 직렬화된 PARA_TEXT에서 FIELD_BEGIN/END 순서 확인
#[test]
fn diag_para_text_field_markers() {
    let data = std::fs::read("samples/field-01.hwp").expect("파일 읽기 실패");
    let doc = HwpDocument::from_bytes(&data).expect("파싱");
    let sec = &doc.document.sections[0];

    for (pi, para) in sec.paragraphs.iter().enumerate() {
        let has_field = para
            .controls
            .iter()
            .any(|c| matches!(c, crate::model::control::Control::Field(_)));
        if !has_field {
            continue;
        }

        let serialized = crate::serializer::body_text::test_serialize_para_text(para);

        eprintln!("\n[para={}] text={:?}", pi, para.text);
        eprintln!("  field_ranges={:?}", para.field_ranges);
        eprintln!("  char_offsets={:?}", para.char_offsets);

        // 직렬화된 바이트에서 컨트롤 문자 위치 추출
        let code_units: Vec<u16> = serialized
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();

        eprintln!("  serialized code_units({}):", code_units.len());
        let mut pos = 0;
        while pos < code_units.len() {
            let cu = code_units[pos];
            if cu == 0x0003 {
                let ctrl_id = if pos + 4 < code_units.len() {
                    (code_units[pos + 1] as u32) | ((code_units[pos + 2] as u32) << 16)
                } else {
                    0
                };
                eprintln!("    [{}] FIELD_BEGIN ctrl_id=0x{:08x}", pos, ctrl_id);
                pos += 8;
            } else if cu == 0x0004 {
                let ctrl_id = if pos + 4 < code_units.len() {
                    (code_units[pos + 1] as u32) | ((code_units[pos + 2] as u32) << 16)
                } else {
                    0
                };
                eprintln!("    [{}] FIELD_END ctrl_id=0x{:08x}", pos, ctrl_id);
                pos += 8;
            } else if cu == 0x000D {
                eprintln!("    [{}] PARA_END", pos);
                pos += 1;
            } else if cu == 0x000A {
                eprintln!("    [{}] NEWLINE", pos);
                pos += 1;
            } else if cu == 0x0009 {
                eprintln!("    [{}] TAB", pos);
                pos += 8;
            } else if cu < 0x0020 {
                eprintln!("    [{}] CTRL 0x{:04x}", pos, cu);
                if cu >= 0x0008 {
                    pos += 8;
                } else {
                    pos += 1;
                }
            } else {
                // 일반 문자: 연속 출력
                let start = pos;
                while pos < code_units.len() && code_units[pos] >= 0x0020 {
                    pos += 1;
                }
                let text = String::from_utf16_lossy(&code_units[start..pos]);
                eprintln!("    [{}..{}] TEXT {:?}", start, pos, text);
            }
        }
    }
}



/// [진단] field-06.hwp (우리가 저장) vs field-01.hwp (원본) vs field-01-h.hwp (한컴 저장 참조)
/// 누름틀 필드의 CTRL_HEADER / CTRL_DATA 비교
#[test]
fn diag_field06_vs_reference() {
    use crate::model::control::{Control, FieldType};
    use crate::parser::record::Record;
    use crate::parser::tags;

    let files: &[(&str, &str)] = &[
        ("samples/field-01.hwp", "ORIGINAL"),
        ("saved/field-01-h.hwp", "HANCOM_REF"),
        ("saved/field-06.hwp", "OUR_SAVED"),
    ];

    for (path, label) in files {
        eprintln!("\n{}", "=".repeat(60));
        eprintln!("=== {} ({}) ===", label, path);
        eprintln!("{}", "=".repeat(60));

        let Ok(data) = std::fs::read(path) else {
            eprintln!("  (파일 없음 - 건너뜀)");
            continue;
        };
        let Ok(doc) = HwpDocument::from_bytes(&data) else {
            eprintln!("  (파싱 실패)");
            continue;
        };

        // 1) 파싱된 모델에서 ClickHere 필드 정보 출력
        for (si, sec) in doc.document.sections.iter().enumerate() {
            for (pi, para) in sec.paragraphs.iter().enumerate() {
                for (ci, ctrl) in para.controls.iter().enumerate() {
                    if let Control::Field(f) = ctrl {
                        if f.field_type != FieldType::ClickHere {
                            continue;
                        }
                        eprintln!("\n  [sec={} para={} ctrl={}]", si, pi, ci);
                        eprintln!("    field_type: {:?}", f.field_type);
                        eprintln!(
                            "    ctrl_id: 0x{:08x} ({})",
                            f.ctrl_id,
                            String::from_utf8_lossy(&f.ctrl_id.to_le_bytes())
                        );
                        eprintln!("    field_id: {} (0x{:08x})", f.field_id, f.field_id);
                        eprintln!("    properties: 0x{:08x} ({})", f.properties, f.properties);
                        eprintln!(
                            "    extra_properties: 0x{:02x} ({})",
                            f.extra_properties, f.extra_properties
                        );
                        eprintln!("    command({} chars): {:?}", f.command.len(), f.command);
                        eprintln!("    ctrl_data_name: {:?}", f.ctrl_data_name);
                        eprintln!("    guide_text: {:?}", f.guide_text());
                        eprintln!("    field_name: {:?}", f.field_name());
                        eprintln!("    memo_text: {:?}", f.memo_text());

                        // command를 UTF-16LE 바이트로 덤프
                        let cmd_utf16: Vec<u16> = f.command.encode_utf16().collect();
                        eprintln!("    command UTF-16 len: {}", cmd_utf16.len());

                        // CTRL_DATA 원본 바이트 덤프
                        if let Some(Some(cd)) = para.ctrl_data_records.get(ci) {
                            eprintln!("    CTRL_DATA({} bytes): {:02x?}", cd.len(), cd);
                            // 필드 이름 파싱 상세
                            if cd.len() >= 12 {
                                let name_len = u16::from_le_bytes([cd[10], cd[11]]) as usize;
                                eprintln!("    CTRL_DATA header(0..10): {:02x?}", &cd[..10]);
                                eprintln!("    CTRL_DATA name_len: {}", name_len);
                                if name_len > 0 && cd.len() >= 12 + name_len * 2 {
                                    let wchars: Vec<u16> = cd[12..12 + name_len * 2]
                                        .chunks_exact(2)
                                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                        .collect();
                                    let name = String::from_utf16_lossy(&wchars);
                                    eprintln!("    CTRL_DATA name: {:?}", name);
                                }
                                // 이름 이후 남은 바이트
                                let after_name = 12 + name_len * 2;
                                if cd.len() > after_name {
                                    eprintln!(
                                        "    CTRL_DATA after_name({} bytes): {:02x?}",
                                        cd.len() - after_name,
                                        &cd[after_name..]
                                    );
                                }
                            }
                        } else {
                            eprintln!("    CTRL_DATA: None");
                        }

                        // 직렬화 결과: 우리가 쓰는 CTRL_HEADER 데이터 생성
                        let ser_cmd_utf16: Vec<u16> = f.command.encode_utf16().collect();
                        let ser_cmd_len = ser_cmd_utf16.len();
                        let mut ser_data = Vec::new();
                        ser_data.extend_from_slice(&f.ctrl_id.to_le_bytes());
                        ser_data.extend_from_slice(&f.properties.to_le_bytes());
                        ser_data.push(f.extra_properties);
                        ser_data.extend_from_slice(&(ser_cmd_len as u16).to_le_bytes());
                        for ch in &ser_cmd_utf16 {
                            ser_data.extend_from_slice(&ch.to_le_bytes());
                        }
                        ser_data.extend_from_slice(&f.field_id.to_le_bytes());
                        eprintln!(
                            "    SERIALIZED CTRL_HEADER({} bytes): {:02x?}",
                            ser_data.len(),
                            &ser_data[..ser_data.len().min(80)]
                        );
                        if ser_data.len() > 80 {
                            eprintln!("    ... (truncated, {} more bytes)", ser_data.len() - 80);
                        }
                    }
                }
            }
        }

        // 2) 원본 바이너리에서 직접 레코드 읽어 CTRL_HEADER 비교
        eprintln!("\n  --- Raw records from BodyText/Section0 ---");
        let mut cfb = match crate::parser::cfb_reader::CfbReader::open(&data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  CFB open error: {:?}", e);
                continue;
            }
        };

        let section_data = match cfb.read_body_text_section(0, true, false) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  Section read error: {:?}", e);
                continue;
            }
        };

        let records = match Record::read_all(&section_data) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Record parse error: {:?}", e);
                continue;
            }
        };

        // CTRL_HEADER 중 필드인 것만 찾기
        for (ri, rec) in records.iter().enumerate() {
            if rec.tag_id != tags::HWPTAG_CTRL_HEADER || rec.data.len() < 4 {
                continue;
            }
            let ctrl_id = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
            if !tags::is_field_ctrl_id(ctrl_id) {
                continue;
            }
            let ctrl_id_bytes = ctrl_id.to_le_bytes();
            let ctrl_id_str = String::from_utf8_lossy(&ctrl_id_bytes);
            eprintln!(
                "\n  [raw rec={}] CTRL_HEADER ctrl_id=0x{:08x}({}) level={} size={}",
                ri, ctrl_id, ctrl_id_str, rec.level, rec.size
            );
            eprintln!(
                "    raw data({} bytes): {:02x?}",
                rec.data.len(),
                &rec.data[..rec.data.len().min(120)]
            );
            if rec.data.len() > 120 {
                eprintln!("    ... (truncated, {} more bytes)", rec.data.len() - 120);
            }

            // 바로 다음 레코드가 CTRL_DATA인지 확인
            if ri + 1 < records.len() && records[ri + 1].tag_id == tags::HWPTAG_CTRL_DATA {
                let cd = &records[ri + 1];
                eprintln!(
                    "    CTRL_DATA[rec={}]({} bytes): {:02x?}",
                    ri + 1,
                    cd.data.len(),
                    &cd.data[..cd.data.len().min(80)]
                );
            }
        }
    }

    eprintln!("\n\n=== COMPARISON SUMMARY ===");
    eprintln!("Check above for differences in:");
    eprintln!("  - CTRL_HEADER raw data sizes and content");
    eprintln!("  - CTRL_DATA presence and content");
    eprintln!("  - command string differences");
    eprintln!("  - field_id / properties / extra_properties differences");
}



#[test]
fn diag_field07_vs_field03h() {
    use crate::model::control::{Control, FieldType};
    use crate::parser::record::Record;
    use crate::parser::tags;

    let files: &[(&str, &str)] = &[
        ("samples/field-01.hwp", "ORIGINAL"),
        ("saved/field-03-h.hwp", "HANCOM_SAVED"),
        ("saved/field-07.hwp", "OUR_SAVED"),
    ];

    // ============================================================
    // PHASE 1: High-level model comparison
    // ============================================================
    for (path, label) in files {
        eprintln!("\n{}", "=".repeat(70));
        eprintln!("=== PHASE 1: MODEL — {} ({}) ===", label, path);
        eprintln!("{}", "=".repeat(70));

        let Ok(data) = std::fs::read(path) else {
            eprintln!("  (파일 없음 - 건너뜀)");
            continue;
        };
        let Ok(doc) = HwpDocument::from_bytes(&data) else {
            eprintln!("  (파싱 실패)");
            continue;
        };

        // Section-level summary
        for (si, sec) in doc.document.sections.iter().enumerate() {
            eprintln!("\n  Section {}: {} paragraphs", si, sec.paragraphs.len());
            eprintln!(
                "    section_def page_def: {}x{}",
                sec.section_def.page_def.width, sec.section_def.page_def.height
            );

            for (pi, para) in sec.paragraphs.iter().enumerate() {
                // Find paragraphs with ClickHere fields (E-Mail)
                let has_email_field = para.controls.iter().any(|c| {
                    if let Control::Field(f) = c {
                        f.field_type == FieldType::ClickHere
                            && (f.command.contains("메일")
                                || f.command.contains("mail")
                                || f.command.contains("Mail")
                                || f.field_name().map_or(false, |n| {
                                    n.contains("메일") || n.contains("mail") || n.contains("Mail")
                                })
                                || f.ctrl_data_name.as_ref().map_or(false, |n| {
                                    n.contains("메일") || n.contains("mail") || n.contains("Mail")
                                }))
                    } else {
                        false
                    }
                });

                if !has_email_field {
                    // Brief summary for non-email paragraphs
                    eprintln!(
                        "    para[{}]: char_count={} control_mask=0x{:08x} text={:?} controls={}",
                        pi,
                        para.char_count,
                        para.control_mask,
                        if para.text.len() > 40 {
                            format!("{}...", &para.text[..40])
                        } else {
                            para.text.clone()
                        },
                        para.controls.len()
                    );
                    continue;
                }

                // DETAILED dump for E-Mail paragraph
                eprintln!("\n  *** E-MAIL PARAGRAPH [sec={} para={}] ***", si, pi);
                eprintln!(
                    "    char_count: {} (0x{:08x})",
                    para.char_count, para.char_count
                );
                eprintln!("    char_count_msb: {}", para.char_count_msb);
                eprintln!("    control_mask: 0x{:08x}", para.control_mask);
                eprintln!("    para_shape_id: {}", para.para_shape_id);
                eprintln!("    style_id: {}", para.style_id);
                eprintln!("    column_type: {:?}", para.column_type);
                eprintln!("    raw_break_type: 0x{:02x}", para.raw_break_type);
                eprintln!(
                    "    raw_header_extra({} bytes): {:02x?}",
                    para.raw_header_extra.len(),
                    para.raw_header_extra
                );
                eprintln!("    has_para_text: {}", para.has_para_text);
                eprintln!(
                    "    text({} chars): {:?}",
                    para.text.chars().count(),
                    para.text
                );
                eprintln!(
                    "    char_offsets({} entries): {:?}",
                    para.char_offsets.len(),
                    para.char_offsets
                );

                // PARA_CHAR_SHAPE
                eprintln!("    char_shapes({} entries):", para.char_shapes.len());
                for (i, cs) in para.char_shapes.iter().enumerate() {
                    eprintln!(
                        "      [{}] pos={} shape_id={}",
                        i, cs.start_pos, cs.char_shape_id
                    );
                }

                // PARA_LINE_SEG
                eprintln!("    line_segs({} entries):", para.line_segs.len());
                for (i, ls) in para.line_segs.iter().enumerate() {
                    eprintln!("      [{}] start={} vpos={} height={} text_h={} baseline={} spacing={} col_start={} seg_w={} tag=0x{:08x}",
                            i, ls.text_start, ls.vertical_pos, ls.line_height, ls.text_height,
                            ls.baseline_distance, ls.line_spacing, ls.column_start, ls.segment_width, ls.tag);
                }

                // PARA_RANGE_TAG
                eprintln!("    range_tags({} entries):", para.range_tags.len());
                for (i, rt) in para.range_tags.iter().enumerate() {
                    eprintln!(
                        "      [{}] start={} end={} tag=0x{:08x}",
                        i, rt.start, rt.end, rt.tag
                    );
                }

                // field_ranges
                eprintln!("    field_ranges({} entries):", para.field_ranges.len());
                for (i, fr) in para.field_ranges.iter().enumerate() {
                    eprintln!(
                        "      [{}] start_char={} end_char={} ctrl_idx={}",
                        i, fr.start_char_idx, fr.end_char_idx, fr.control_idx
                    );
                }

                // Controls + CTRL_DATA
                eprintln!("    controls({} entries):", para.controls.len());
                for (ci, ctrl) in para.controls.iter().enumerate() {
                    if let Control::Field(f) = ctrl {
                        eprintln!("      [{}] FIELD: type={:?} ctrl_id=0x{:08x}({}) field_id={} props=0x{:08x} extra=0x{:02x}",
                                ci, f.field_type, f.ctrl_id,
                                String::from_utf8_lossy(&f.ctrl_id.to_le_bytes()),
                                f.field_id, f.properties, f.extra_properties);
                        eprintln!(
                            "           command({} chars): {:?}",
                            f.command.len(),
                            f.command
                        );
                        eprintln!("           ctrl_data_name: {:?}", f.ctrl_data_name);
                        eprintln!("           field_name(): {:?}", f.field_name());
                        eprintln!("           guide_text(): {:?}", f.guide_text());
                    } else {
                        eprintln!("      [{}] {:?}", ci, std::mem::discriminant(ctrl));
                    }

                    // CTRL_DATA
                    if let Some(Some(cd)) = para.ctrl_data_records.get(ci) {
                        eprintln!("      CTRL_DATA[{}]({} bytes): {:02x?}", ci, cd.len(), cd);
                    } else {
                        eprintln!("      CTRL_DATA[{}]: None", ci);
                    }
                }

                // Serialize PARA_TEXT and dump
                let text_data = crate::serializer::body_text::test_serialize_para_text(para);
                eprintln!(
                    "    SERIALIZED PARA_TEXT({} bytes = {} code units):",
                    text_data.len(),
                    text_data.len() / 2
                );
                // Hex dump in lines of 32 bytes
                for chunk_start in (0..text_data.len()).step_by(32) {
                    let chunk_end = (chunk_start + 32).min(text_data.len());
                    let chunk = &text_data[chunk_start..chunk_end];
                    eprint!("      {:04x}: ", chunk_start);
                    for b in chunk {
                        eprint!("{:02x} ", b);
                    }
                    // Also show as u16 code units
                    eprint!(" | ");
                    for pair in chunk.chunks_exact(2) {
                        let cu = u16::from_le_bytes([pair[0], pair[1]]);
                        if cu >= 0x20 && cu < 0x7F {
                            eprint!("{} ", cu as u8 as char);
                        } else if cu >= 0xAC00 && cu <= 0xD7A3 {
                            eprint!("K "); // Korean
                        } else {
                            eprint!("{:04x} ", cu);
                        }
                    }
                    eprintln!();
                }
            }
        }
    }

    // ============================================================
    // PHASE 2: Raw binary record comparison
    // ============================================================
    for (path, label) in files {
        eprintln!("\n{}", "=".repeat(70));
        eprintln!("=== PHASE 2: RAW RECORDS — {} ({}) ===", label, path);
        eprintln!("{}", "=".repeat(70));

        let Ok(data) = std::fs::read(path) else {
            eprintln!("  (파일 없음 - 건너뜀)");
            continue;
        };

        let mut cfb = match crate::parser::cfb_reader::CfbReader::open(&data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  CFB open error: {:?}", e);
                continue;
            }
        };

        let section_data = match cfb.read_body_text_section(0, true, false) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  Section read error: {:?}", e);
                continue;
            }
        };

        let records = match Record::read_all(&section_data) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Record parse error: {:?}", e);
                continue;
            }
        };

        eprintln!("  Total records: {}", records.len());

        // Find the E-Mail paragraph by scanning for field CTRL_HEADER with email-related command
        // First pass: identify all PARA_HEADER positions
        let mut para_starts: Vec<usize> = Vec::new();
        for (ri, rec) in records.iter().enumerate() {
            if rec.tag_id == tags::HWPTAG_PARA_HEADER {
                para_starts.push(ri);
            }
        }
        eprintln!(
            "  Paragraph count (from PARA_HEADER records): {}",
            para_starts.len()
        );

        // For each paragraph, check if it has a field CTRL_HEADER with email command
        for (pi, &para_start) in para_starts.iter().enumerate() {
            let para_end = if pi + 1 < para_starts.len() {
                para_starts[pi + 1]
            } else {
                records.len()
            };
            let para_records = &records[para_start..para_end];

            // Check for email-related field
            let has_email = para_records.iter().any(|rec| {
                if rec.tag_id != tags::HWPTAG_CTRL_HEADER || rec.data.len() < 11 {
                    return false;
                }
                let ctrl_id =
                    u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                if !tags::is_field_ctrl_id(ctrl_id) {
                    return false;
                }
                // Check command string for email
                if rec.data.len() >= 11 {
                    let cmd_len = u16::from_le_bytes([rec.data[9], rec.data[10]]) as usize;
                    if cmd_len > 0 && rec.data.len() >= 11 + cmd_len * 2 {
                        let wchars: Vec<u16> = rec.data[11..11 + cmd_len * 2]
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let cmd = String::from_utf16_lossy(&wchars);
                        return cmd.contains("메일")
                            || cmd.contains("mail")
                            || cmd.contains("Mail");
                    }
                }
                false
            });

            if !has_email {
                continue;
            }

            eprintln!(
                "\n  *** RAW E-MAIL PARAGRAPH [para_idx={}] (records {}..{}) ***",
                pi, para_start, para_end
            );

            for (offset, rec) in para_records.iter().enumerate() {
                let ri = para_start + offset;
                let tag_name = tags::tag_name(rec.tag_id);
                eprintln!(
                    "\n    [rec={}] {} (tag=0x{:04x}) level={} size={}",
                    ri,
                    tag_name,
                    rec.tag_id,
                    rec.level,
                    rec.data.len()
                );

                match rec.tag_id {
                    tags::HWPTAG_PARA_HEADER => {
                        eprintln!("      raw({} bytes): {:02x?}", rec.data.len(), rec.data);
                        if rec.data.len() >= 12 {
                            let char_count_raw = u32::from_le_bytes([
                                rec.data[0],
                                rec.data[1],
                                rec.data[2],
                                rec.data[3],
                            ]);
                            let control_mask = u32::from_le_bytes([
                                rec.data[4],
                                rec.data[5],
                                rec.data[6],
                                rec.data[7],
                            ]);
                            let para_shape_id = u16::from_le_bytes([rec.data[8], rec.data[9]]);
                            let style_id = rec.data[10];
                            let break_type = rec.data[11];
                            eprintln!(
                                "      char_count_raw=0x{:08x} (count={}, msb={})",
                                char_count_raw,
                                char_count_raw & 0x7FFFFFFF,
                                char_count_raw >> 31
                            );
                            eprintln!("      control_mask=0x{:08x}", control_mask);
                            eprintln!(
                                "      para_shape_id={} style_id={} break_type=0x{:02x}",
                                para_shape_id, style_id, break_type
                            );
                        }
                        if rec.data.len() >= 18 {
                            let num_cs = u16::from_le_bytes([rec.data[12], rec.data[13]]);
                            let num_rt = u16::from_le_bytes([rec.data[14], rec.data[15]]);
                            let num_ls = u16::from_le_bytes([rec.data[16], rec.data[17]]);
                            eprintln!(
                                "      numCharShapes={} numRangeTags={} numLineSegs={}",
                                num_cs, num_rt, num_ls
                            );
                        }
                        if rec.data.len() >= 22 {
                            let instance_id = u32::from_le_bytes([
                                rec.data[18],
                                rec.data[19],
                                rec.data[20],
                                rec.data[21],
                            ]);
                            eprintln!("      instanceId={}", instance_id);
                        }
                        if rec.data.len() > 22 {
                            eprintln!(
                                "      extra bytes after instanceId: {:02x?}",
                                &rec.data[22..]
                            );
                        }
                    }
                    tags::HWPTAG_PARA_TEXT => {
                        eprintln!(
                            "      raw({} bytes = {} code units):",
                            rec.data.len(),
                            rec.data.len() / 2
                        );
                        for chunk_start in (0..rec.data.len()).step_by(32) {
                            let chunk_end = (chunk_start + 32).min(rec.data.len());
                            let chunk = &rec.data[chunk_start..chunk_end];
                            eprint!("        {:04x}: ", chunk_start);
                            for b in chunk {
                                eprint!("{:02x} ", b);
                            }
                            eprint!(" | ");
                            for pair in chunk.chunks_exact(2) {
                                let cu = u16::from_le_bytes([pair[0], pair[1]]);
                                if cu >= 0x20 && cu < 0x7F {
                                    eprint!("{} ", cu as u8 as char);
                                } else if cu >= 0xAC00 && cu <= 0xD7A3 {
                                    eprint!("K ");
                                } else {
                                    eprint!("{:04x} ", cu);
                                }
                            }
                            eprintln!();
                        }
                    }
                    tags::HWPTAG_PARA_CHAR_SHAPE => {
                        eprintln!(
                            "      raw({} bytes, {} entries):",
                            rec.data.len(),
                            rec.data.len() / 8
                        );
                        for i in (0..rec.data.len()).step_by(8) {
                            if i + 8 <= rec.data.len() {
                                let pos = u32::from_le_bytes([
                                    rec.data[i],
                                    rec.data[i + 1],
                                    rec.data[i + 2],
                                    rec.data[i + 3],
                                ]);
                                let id = u32::from_le_bytes([
                                    rec.data[i + 4],
                                    rec.data[i + 5],
                                    rec.data[i + 6],
                                    rec.data[i + 7],
                                ]);
                                eprintln!("        pos={} shape_id={}", pos, id);
                            }
                        }
                    }
                    tags::HWPTAG_PARA_LINE_SEG => {
                        eprintln!(
                            "      raw({} bytes, {} entries):",
                            rec.data.len(),
                            rec.data.len() / 36
                        );
                        for i in (0..rec.data.len()).step_by(36) {
                            if i + 36 <= rec.data.len() {
                                let d = &rec.data[i..i + 36];
                                let text_start = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
                                let vpos = i32::from_le_bytes([d[4], d[5], d[6], d[7]]);
                                let height = i32::from_le_bytes([d[8], d[9], d[10], d[11]]);
                                let text_h = i32::from_le_bytes([d[12], d[13], d[14], d[15]]);
                                let baseline = i32::from_le_bytes([d[16], d[17], d[18], d[19]]);
                                let spacing = i32::from_le_bytes([d[20], d[21], d[22], d[23]]);
                                let col_start = i32::from_le_bytes([d[24], d[25], d[26], d[27]]);
                                let seg_w = i32::from_le_bytes([d[28], d[29], d[30], d[31]]);
                                let tag_val = u32::from_le_bytes([d[32], d[33], d[34], d[35]]);
                                eprintln!("        start={} vpos={} h={} th={} bl={} sp={} cs={} sw={} tag=0x{:08x}",
                                        text_start, vpos, height, text_h, baseline, spacing, col_start, seg_w, tag_val);
                            }
                        }
                    }
                    tags::HWPTAG_PARA_RANGE_TAG => {
                        eprintln!(
                            "      raw({} bytes, {} entries):",
                            rec.data.len(),
                            rec.data.len() / 12
                        );
                        for i in (0..rec.data.len()).step_by(12) {
                            if i + 12 <= rec.data.len() {
                                let start = u32::from_le_bytes([
                                    rec.data[i],
                                    rec.data[i + 1],
                                    rec.data[i + 2],
                                    rec.data[i + 3],
                                ]);
                                let end = u32::from_le_bytes([
                                    rec.data[i + 4],
                                    rec.data[i + 5],
                                    rec.data[i + 6],
                                    rec.data[i + 7],
                                ]);
                                let tag_val = u32::from_le_bytes([
                                    rec.data[i + 8],
                                    rec.data[i + 9],
                                    rec.data[i + 10],
                                    rec.data[i + 11],
                                ]);
                                eprintln!(
                                    "        start={} end={} tag=0x{:08x}",
                                    start, end, tag_val
                                );
                            }
                        }
                    }
                    tags::HWPTAG_CTRL_HEADER => {
                        if rec.data.len() >= 4 {
                            let ctrl_id = u32::from_le_bytes([
                                rec.data[0],
                                rec.data[1],
                                rec.data[2],
                                rec.data[3],
                            ]);
                            let ctrl_id_bytes = ctrl_id.to_le_bytes();
                            let ctrl_id_str = String::from_utf8_lossy(&ctrl_id_bytes);
                            eprintln!("      ctrl_id=0x{:08x}({})", ctrl_id, ctrl_id_str);

                            if tags::is_field_ctrl_id(ctrl_id) {
                                eprintln!("      *** FIELD CTRL_HEADER DETAIL ***");
                                eprintln!(
                                    "      full raw({} bytes): {:02x?}",
                                    rec.data.len(),
                                    rec.data
                                );
                                if rec.data.len() >= 11 {
                                    let props = u32::from_le_bytes([
                                        rec.data[4],
                                        rec.data[5],
                                        rec.data[6],
                                        rec.data[7],
                                    ]);
                                    let extra = rec.data[8];
                                    let cmd_len =
                                        u16::from_le_bytes([rec.data[9], rec.data[10]]) as usize;
                                    eprintln!("      properties=0x{:08x} extra_properties=0x{:02x} command_len={}", props, extra, cmd_len);
                                    if cmd_len > 0 && rec.data.len() >= 11 + cmd_len * 2 {
                                        let wchars: Vec<u16> = rec.data[11..11 + cmd_len * 2]
                                            .chunks_exact(2)
                                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                            .collect();
                                        let cmd = String::from_utf16_lossy(&wchars);
                                        eprintln!("      command: {:?}", cmd);
                                    }
                                    let field_id_offset = 11 + cmd_len * 2;
                                    if rec.data.len() >= field_id_offset + 4 {
                                        let field_id = u32::from_le_bytes([
                                            rec.data[field_id_offset],
                                            rec.data[field_id_offset + 1],
                                            rec.data[field_id_offset + 2],
                                            rec.data[field_id_offset + 3],
                                        ]);
                                        eprintln!(
                                            "      field_id={} (0x{:08x})",
                                            field_id, field_id
                                        );
                                    }
                                    // Any bytes after field_id?
                                    let expected_end = field_id_offset + 4;
                                    if rec.data.len() > expected_end {
                                        eprintln!("      *** EXTRA BYTES AFTER field_id ({} bytes): {:02x?}",
                                                rec.data.len() - expected_end, &rec.data[expected_end..]);
                                    }
                                }
                            } else {
                                eprintln!(
                                    "      raw({} bytes): {:02x?}",
                                    rec.data.len(),
                                    &rec.data[..rec.data.len().min(80)]
                                );
                            }
                        }
                    }
                    tags::HWPTAG_CTRL_DATA => {
                        eprintln!("      raw({} bytes): {:02x?}", rec.data.len(), rec.data);
                        // Parse CTRL_DATA for field: typically has name
                        if rec.data.len() >= 12 {
                            eprintln!("      header(0..10): {:02x?}", &rec.data[..10]);
                            let name_len =
                                u16::from_le_bytes([rec.data[10], rec.data[11]]) as usize;
                            eprintln!("      name_len={}", name_len);
                            if name_len > 0 && rec.data.len() >= 12 + name_len * 2 {
                                let wchars: Vec<u16> = rec.data[12..12 + name_len * 2]
                                    .chunks_exact(2)
                                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                    .collect();
                                let name = String::from_utf16_lossy(&wchars);
                                eprintln!("      name: {:?}", name);
                            }
                        }
                    }
                    _ => {
                        eprintln!(
                            "      raw({} bytes): {:02x?}",
                            rec.data.len(),
                            &rec.data[..rec.data.len().min(40)]
                        );
                    }
                }
            }
        }
    }

    // ============================================================
    // PHASE 3: TAB extended data comparison
    // ============================================================
    eprintln!("\n{}", "=".repeat(70));
    eprintln!("=== PHASE 3: TAB EXTENDED DATA COMPARISON ===");
    eprintln!("{}", "=".repeat(70));

    for (path, label) in files {
        eprintln!("\n  --- {} ({}) ---", label, path);

        let Ok(data) = std::fs::read(path) else {
            eprintln!("    (파일 없음 - 건너뜀)");
            continue;
        };

        let mut cfb = match crate::parser::cfb_reader::CfbReader::open(&data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("    CFB open error: {:?}", e);
                continue;
            }
        };

        let section_data = match cfb.read_body_text_section(0, true, false) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("    Section read error: {:?}", e);
                continue;
            }
        };

        let records = match Record::read_all(&section_data) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("    Record parse error: {:?}", e);
                continue;
            }
        };

        // Find all PARA_TEXT records and check TAB extended data
        let mut tab_count = 0;
        let mut tab_zeroed = 0;
        let mut tab_nonzero = 0;
        for rec in &records {
            if rec.tag_id != tags::HWPTAG_PARA_TEXT {
                continue;
            }
            // Scan for TAB characters (0x0009)
            let code_units: Vec<u16> = rec
                .data
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            for (i, &cu) in code_units.iter().enumerate() {
                if cu == 0x0009 && i + 7 < code_units.len() {
                    tab_count += 1;
                    let ext: Vec<u16> = code_units[i + 1..i + 8].to_vec();
                    let all_zero = ext.iter().all(|&x| x == 0);
                    if all_zero {
                        tab_zeroed += 1;
                    } else {
                        tab_nonzero += 1;
                        eprintln!("    TAB at cu_offset={}: extended data = {:04x?}", i, ext);
                    }
                }
            }
        }
        eprintln!(
            "    TABs total={} zeroed={} nonzero={}",
            tab_count, tab_zeroed, tab_nonzero
        );
    }

    // ============================================================
    // PHASE 4: Full section record count comparison
    // ============================================================
    eprintln!("\n{}", "=".repeat(70));
    eprintln!("=== PHASE 4: SECTION RECORD SUMMARY ===");
    eprintln!("{}", "=".repeat(70));

    for (path, label) in files {
        eprintln!("\n  --- {} ({}) ---", label, path);

        let Ok(data) = std::fs::read(path) else {
            eprintln!("    (파일 없음 - 건너뜀)");
            continue;
        };

        let mut cfb = match crate::parser::cfb_reader::CfbReader::open(&data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("    CFB open error: {:?}", e);
                continue;
            }
        };

        let section_data = match cfb.read_body_text_section(0, true, false) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("    Section read error: {:?}", e);
                continue;
            }
        };

        let records = match Record::read_all(&section_data) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("    Record parse error: {:?}", e);
                continue;
            }
        };

        // Count by tag type
        use std::collections::BTreeMap;
        let mut tag_counts: BTreeMap<u16, usize> = BTreeMap::new();
        for rec in &records {
            *tag_counts.entry(rec.tag_id).or_insert(0) += 1;
        }
        for (tag_id, count) in &tag_counts {
            eprintln!(
                "    {} (0x{:04x}): {} records",
                tags::tag_name(*tag_id),
                tag_id,
                count
            );
        }
        eprintln!("    Total: {} records", records.len());
    }

    // ============================================================
    // PHASE 5: Byte-level diff of serialized vs original PARA_TEXT for E-Mail paragraph
    // ============================================================
    eprintln!("\n{}", "=".repeat(70));
    eprintln!("=== PHASE 5: SERIALIZED vs ORIGINAL PARA_TEXT DIFF ===");
    eprintln!("{}", "=".repeat(70));

    // Compare our serialized output for saved/field-07.hwp against its raw PARA_TEXT
    if let Ok(data) = std::fs::read("saved/field-07.hwp") {
        if let Ok(doc) = HwpDocument::from_bytes(&data) {
            // Find the E-Mail paragraph
            for sec in &doc.document.sections {
                for para in &sec.paragraphs {
                    let has_email = para.controls.iter().any(|c| {
                        if let Control::Field(f) = c {
                            f.field_type == FieldType::ClickHere
                                && (f.command.contains("메일")
                                    || f.command.contains("mail")
                                    || f.command.contains("Mail")
                                    || f.ctrl_data_name.as_ref().map_or(false, |n| {
                                        n.contains("메일")
                                            || n.contains("mail")
                                            || n.contains("Mail")
                                    }))
                        } else {
                            false
                        }
                    });
                    if !has_email {
                        continue;
                    }

                    // Re-serialize
                    let serialized = crate::serializer::body_text::test_serialize_para_text(para);
                    eprintln!("  Serialized PARA_TEXT: {} bytes", serialized.len());

                    // Get original raw
                    let mut cfb = crate::parser::cfb_reader::CfbReader::open(&data).unwrap();
                    let section_data = cfb.read_body_text_section(0, true, false).unwrap();
                    let records = Record::read_all(&section_data).unwrap();

                    // Find the matching PARA_TEXT
                    let mut para_starts: Vec<usize> = Vec::new();
                    for (ri, rec) in records.iter().enumerate() {
                        if rec.tag_id == tags::HWPTAG_PARA_HEADER {
                            para_starts.push(ri);
                        }
                    }
                    for (pi, &ps) in para_starts.iter().enumerate() {
                        let pe = if pi + 1 < para_starts.len() {
                            para_starts[pi + 1]
                        } else {
                            records.len()
                        };
                        let has_field = records[ps..pe].iter().any(|r| {
                            if r.tag_id != tags::HWPTAG_CTRL_HEADER || r.data.len() < 11 {
                                return false;
                            }
                            let cid =
                                u32::from_le_bytes([r.data[0], r.data[1], r.data[2], r.data[3]]);
                            if !tags::is_field_ctrl_id(cid) {
                                return false;
                            }
                            let cl = u16::from_le_bytes([r.data[9], r.data[10]]) as usize;
                            if cl > 0 && r.data.len() >= 11 + cl * 2 {
                                let w: Vec<u16> = r.data[11..11 + cl * 2]
                                    .chunks_exact(2)
                                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                    .collect();
                                let cmd = String::from_utf16_lossy(&w);
                                cmd.contains("메일") || cmd.contains("mail") || cmd.contains("Mail")
                            } else {
                                false
                            }
                        });
                        if !has_field {
                            continue;
                        }

                        // Find PARA_TEXT in this paragraph
                        for rec in &records[ps..pe] {
                            if rec.tag_id == tags::HWPTAG_PARA_TEXT {
                                let original = &rec.data;
                                eprintln!("  Original PARA_TEXT: {} bytes", original.len());

                                if serialized.len() != original.len() {
                                    eprintln!(
                                        "  *** SIZE MISMATCH: serialized={} vs original={} ***",
                                        serialized.len(),
                                        original.len()
                                    );
                                }

                                // Find byte-level differences
                                let min_len = serialized.len().min(original.len());
                                let mut diff_count = 0;
                                for i in 0..min_len {
                                    if serialized[i] != original[i] {
                                        if diff_count < 30 {
                                            eprintln!("    DIFF at byte {}: serialized=0x{:02x} original=0x{:02x}",
                                                    i, serialized[i], original[i]);
                                        }
                                        diff_count += 1;
                                    }
                                }
                                if diff_count > 30 {
                                    eprintln!("    ... and {} more differences", diff_count - 30);
                                }
                                eprintln!("  Total byte differences: {}", diff_count);
                                break;
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    eprintln!("\n=== DIAGNOSTIC COMPLETE ===");
}



/// [진단] field-10.hwp (우리 저장) vs field-10-2010.hwp (한컴 2010 저장) 비교
/// 한컴에서 page 2 ClickHere 필드의 안내문이 빈 문자열로 표시되는 원인 분석
#[test]
fn diag_field10_comparison() {
    use crate::model::control::{Control, FieldType};

    let files: &[(&str, &str)] = &[
        ("saved/field-10.hwp", "OUR_SAVED"),
        ("saved/field-10-2010.hwp", "HANCOM_2010"),
    ];

    for (path, label) in files {
        eprintln!("\n{}", "=".repeat(70));
        eprintln!("=== {} ({}) ===", label, path);
        eprintln!("{}", "=".repeat(70));

        let Ok(data) = std::fs::read(path) else {
            eprintln!("  (파일 없음 - 건너뜀)");
            continue;
        };
        let Ok(doc) = HwpDocument::from_bytes(&data) else {
            eprintln!("  (파싱 실패)");
            continue;
        };

        eprintln!("  Sections: {}", doc.document.sections.len());

        for (si, sec) in doc.document.sections.iter().enumerate() {
            eprintln!(
                "\n  --- Section {} ({} paragraphs) ---",
                si,
                sec.paragraphs.len()
            );

            // 1) 섹션 최상위 문단의 ClickHere 필드
            for (pi, para) in sec.paragraphs.iter().enumerate() {
                diag_field10_print_clickhere_in_para(&format!("sec={} para={}", si, pi), para);

                // 2) 표 셀 내부 문단
                for (ci, ctrl) in para.controls.iter().enumerate() {
                    if let Control::Table(t) = ctrl {
                        for (cell_i, cell) in t.cells.iter().enumerate() {
                            for (cp, cpara) in cell.paragraphs.iter().enumerate() {
                                diag_field10_print_clickhere_in_para(
                                    &format!(
                                        "sec={} para={} table_ctrl={} cell={} cell_para={}",
                                        si, pi, ci, cell_i, cp
                                    ),
                                    cpara,
                                );
                                // 표 셀 안의 표/글상자도 확인 (중첩)
                                for (cci, cctrl) in cpara.controls.iter().enumerate() {
                                    diag_field10_check_nested(
                                            &format!("sec={} para={} table_ctrl={} cell={} cell_para={} nested_ctrl={}",
                                                si, pi, ci, cell_i, cp, cci),
                                            cctrl,
                                        );
                                }
                            }
                        }
                    }
                    // 3) 글상자(Shape) 내부 문단
                    if let Control::Shape(s) = ctrl {
                        if let Some(drawing) = s.drawing() {
                            if let Some(tb) = &drawing.text_box {
                                for (tp, tpara) in tb.paragraphs.iter().enumerate() {
                                    diag_field10_print_clickhere_in_para(
                                        &format!(
                                            "sec={} para={} shape_ctrl={} textbox_para={}",
                                            si, pi, ci, tp
                                        ),
                                        tpara,
                                    );
                                    // 글상자 안의 표/글상자도 확인
                                    for (tci, tctrl) in tpara.controls.iter().enumerate() {
                                        diag_field10_check_nested(
                                                &format!("sec={} para={} shape_ctrl={} textbox_para={} nested_ctrl={}",
                                                    si, pi, ci, tp, tci),
                                                tctrl,
                                            );
                                    }
                                }
                            }
                        }
                    }
                    // 4) Picture 내부 (caption 등은 별도, Picture는 보통 텍스트 없음)
                }
            }
        }

        // Raw record 분석: CTRL_HEADER에서 필드 레코드 추출
        eprintln!("\n  --- Raw CTRL_HEADER records (all sections) ---");
        let mut cfb = match crate::parser::cfb_reader::CfbReader::open(&data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  CFB open error: {:?}", e);
                continue;
            }
        };
        for sec_idx in 0..doc.document.sections.len() {
            let Ok(section_data) = cfb.read_body_text_section(sec_idx as u32, true, false) else {
                eprintln!("  Section {} read error", sec_idx);
                continue;
            };
            let Ok(records) = crate::parser::record::Record::read_all(&section_data) else {
                eprintln!("  Section {} record parse error", sec_idx);
                continue;
            };

            let mut field_count = 0;
            for (ri, rec) in records.iter().enumerate() {
                if rec.tag_id != crate::parser::tags::HWPTAG_CTRL_HEADER || rec.data.len() < 4 {
                    continue;
                }
                let ctrl_id =
                    u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                if !crate::parser::tags::is_field_ctrl_id(ctrl_id) {
                    continue;
                }
                let ctrl_id_bytes = ctrl_id.to_le_bytes();
                let ctrl_id_str = String::from_utf8_lossy(&ctrl_id_bytes);
                // ClickHere 필드인지 확인: ctrl_id == '%clk' = 0x25636c6b
                let is_clickhere = ctrl_id == crate::parser::tags::FIELD_CLICKHERE;

                if is_clickhere {
                    field_count += 1;
                    eprintln!("\n  [raw sec={} rec={}] ClickHere CTRL_HEADER ctrl_id=0x{:08x}({}) size={}",
                            sec_idx, ri, ctrl_id, ctrl_id_str, rec.data.len());
                    // 처음 120바이트 출력
                    let dump_len = rec.data.len().min(200);
                    eprintln!(
                        "    raw({} bytes): {:02x?}",
                        rec.data.len(),
                        &rec.data[..dump_len]
                    );
                    if rec.data.len() > dump_len {
                        eprintln!("    ... ({} more bytes)", rec.data.len() - dump_len);
                    }

                    // command 문자열 추출 시도: offset 9에 command_len(u16), 이후 UTF-16LE
                    if rec.data.len() >= 11 {
                        let cmd_len = u16::from_le_bytes([rec.data[9], rec.data[10]]) as usize;
                        let cmd_byte_start = 11;
                        let cmd_byte_end = cmd_byte_start + cmd_len * 2;
                        if rec.data.len() >= cmd_byte_end {
                            let wchars: Vec<u16> = rec.data[cmd_byte_start..cmd_byte_end]
                                .chunks_exact(2)
                                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                .collect();
                            let cmd = String::from_utf16_lossy(&wchars);
                            eprintln!("    command({} chars): {:?}", cmd.len(), cmd);

                            // set:N 값 추출
                            if let Some(set_start) = cmd.find("set:") {
                                let rest = &cmd[set_start + 4..];
                                if let Some(colon) = rest.find(':') {
                                    let n_str = &rest[..colon];
                                    eprintln!("    set:N value: {:?}", n_str);
                                }
                            }
                        }
                    }

                    // CTRL_DATA 확인
                    if ri + 1 < records.len()
                        && records[ri + 1].tag_id == crate::parser::tags::HWPTAG_CTRL_DATA
                    {
                        let cd = &records[ri + 1];
                        eprintln!(
                            "    CTRL_DATA[rec={}]({} bytes): {:02x?}",
                            ri + 1,
                            cd.data.len(),
                            &cd.data[..cd.data.len().min(80)]
                        );
                    }
                }
            }
            if field_count == 0 {
                eprintln!("  [sec={}] No ClickHere fields in raw records", sec_idx);
            }
        }
    }

    eprintln!("\n\n{}", "=".repeat(70));
    eprintln!("=== COMPARISON SUMMARY ===");
    eprintln!("{}", "=".repeat(70));
    eprintln!("Compare the command strings, set:N values, guide_text, memo_text");
    eprintln!("between OUR_SAVED and HANCOM_2010 for page 2 fields.");
    eprintln!("Look for: trailing spaces, empty guide text, different set:N counts");
}


fn diag_field10_print_clickhere_in_para(location: &str, para: &crate::model::paragraph::Paragraph) {
    use crate::model::control::{Control, FieldType};

    for (ci, ctrl) in para.controls.iter().enumerate() {
        if let Control::Field(f) = ctrl {
            if f.field_type != FieldType::ClickHere {
                continue;
            }
            eprintln!("\n  [{}  ctrl={}] ClickHere", location, ci);
            eprintln!("    field_id: {} (0x{:08x})", f.field_id, f.field_id);
            eprintln!(
                "    ctrl_id: 0x{:08x} ({})",
                f.ctrl_id,
                String::from_utf8_lossy(&f.ctrl_id.to_le_bytes())
            );
            eprintln!("    properties: 0x{:08x} ({})", f.properties, f.properties);
            eprintln!(
                "    extra_properties: 0x{:02x} ({})",
                f.extra_properties, f.extra_properties
            );
            eprintln!("    command_len(bytes): {}", f.command.len());
            eprintln!("    command_len(chars): {}", f.command.chars().count());
            eprintln!("    command: {:?}", f.command);

            // command의 각 바이트를 escape하여 trailing space 등 확인
            let escaped: String = f
                .command
                .chars()
                .map(|c| {
                    if c == ' ' {
                        "·".to_string()
                    } else if c == '\t' {
                        "\\t".to_string()
                    } else if c == '\n' {
                        "\\n".to_string()
                    } else if c == '\r' {
                        "\\r".to_string()
                    } else {
                        c.to_string()
                    }
                })
                .collect();
            eprintln!("    command(escaped): {}", escaped);

            // set:N 값 추출
            if let Some(set_start) = f.command.find("set:") {
                let rest = &f.command[set_start + 4..];
                if let Some(colon) = rest.find(':') {
                    let n_str = &rest[..colon];
                    eprintln!(
                        "    set:N value: {:?} (parsed: {:?})",
                        n_str,
                        n_str.parse::<usize>().ok()
                    );
                }
            }

            // guide/memo 추출 결과
            eprintln!("    guide_text(): {:?}", f.guide_text());
            eprintln!("    memo_text(): {:?}", f.memo_text());
            eprintln!("    field_name(): {:?}", f.field_name());
            eprintln!("    ctrl_data_name: {:?}", f.ctrl_data_name);

            // extract_wstring_value 상세 (Direction/HelpState/Name)
            for key in &["Direction:", "HelpState:", "Name:"] {
                let val = f.extract_wstring_value(key);
                eprintln!("    extract_wstring_value({:?}): {:?}", key, val);
            }

            // CTRL_DATA 원본 바이트
            if let Some(Some(cd)) = para.ctrl_data_records.get(ci) {
                eprintln!(
                    "    CTRL_DATA({} bytes): {:02x?}",
                    cd.len(),
                    &cd[..cd.len().min(80)]
                );
                if cd.len() >= 12 {
                    let name_len = u16::from_le_bytes([cd[10], cd[11]]) as usize;
                    eprintln!("    CTRL_DATA name_len: {}", name_len);
                    if name_len > 0 && cd.len() >= 12 + name_len * 2 {
                        let wchars: Vec<u16> = cd[12..12 + name_len * 2]
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let name = String::from_utf16_lossy(&wchars);
                        eprintln!("    CTRL_DATA name: {:?}", name);
                    }
                }
            } else {
                eprintln!("    CTRL_DATA: None");
            }

            // UTF-16 command 길이 (직렬화 시 사용)
            let cmd_utf16: Vec<u16> = f.command.encode_utf16().collect();
            eprintln!("    command UTF-16 len: {}", cmd_utf16.len());
        }
    }
}


fn diag_field10_check_nested(location: &str, ctrl: &crate::model::control::Control) {
    use crate::model::control::Control;
    match ctrl {
        Control::Table(t) => {
            for (cell_i, cell) in t.cells.iter().enumerate() {
                for (cp, cpara) in cell.paragraphs.iter().enumerate() {
                    diag_field10_print_clickhere_in_para(
                        &format!("{} nested_table cell={} para={}", location, cell_i, cp),
                        cpara,
                    );
                }
            }
        }
        Control::Shape(s) => {
            if let Some(drawing) = s.drawing() {
                if let Some(tb) = &drawing.text_box {
                    for (tp, tpara) in tb.paragraphs.iter().enumerate() {
                        diag_field10_print_clickhere_in_para(
                            &format!("{} nested_shape textbox_para={}", location, tp),
                            tpara,
                        );
                    }
                }
            }
        }
        _ => {}
    }
}



#[test]
fn test_unknown_equation_values_survive_edit_undo_redo_and_save_state() {
    let unknown = br#"<HWPML Version="2.91"><HEAD/><BODY><SECTION><P><TEXT><EQUATION><SCRIPT>x</SCRIPT><FUTURE Mode="matrix&amp;inline">secret &lt; value</FUTURE></EQUATION></TEXT></P></SECTION></BODY><TAIL/></HWPML>"#;
    let mut doc = HwpDocument::new(unknown).expect("unknown equation semantics remain readable");
    let before_snapshot = doc.save_snapshot_native();

    doc.set_equation_properties_native(0, 0, 0, None, None, r#"{"script":"x^2 + 2"}"#)
        .expect("equation edit should apply");
    let after_snapshot = doc.save_snapshot_native();

    let assert_values = |doc: &HwpDocument| {
        let state: Value =
            serde_json::from_str(&doc.get_hml_save_state()).expect("save state JSON");
        let blockers = state["blockers"].as_array().expect("blocker array");
        assert!(blockers.iter().any(|blocker| {
            blocker["xmlPath"] == "/HWPML/BODY/SECTION/P/TEXT/EQUATION/FUTURE/@Mode"
                && blocker["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("Mode=matrix&inline"))
                && blocker["preserved"] == false
        }));
        assert!(blockers.iter().any(|blocker| {
            blocker["xmlPath"] == "/HWPML/BODY/SECTION/P/TEXT/EQUATION/FUTURE/#text"
                && blocker["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("#text=secret < value"))
                && blocker["preserved"] == false
        }));
    };

    assert_values(&doc);
    doc.restore_snapshot_native(before_snapshot)
        .expect("undo snapshot should restore");
    assert_values(&doc);
    doc.restore_snapshot_native(after_snapshot)
        .expect("redo snapshot should restore");
    assert_values(&doc);
}



#[test]
fn update_style_dirties_docinfo_for_hwp5_save() {
    use crate::model::style::Style;
    let mut doc = HwpDocument::create_empty();
    if doc.document.doc_info.styles.is_empty() {
        doc.document.doc_info.styles.push(Style::default());
    }
    doc.document.doc_info.styles.push(Style {
        local_name: "OLD".to_string(),
        ..Default::default()
    });
    let sid = (doc.document.doc_info.styles.len() - 1) as u32;
    // parsed 문서처럼 DocInfo 원본 스트림을 채운다(clean 상태): 무효화가 없으면 저장이 원본 반환.
    doc.document.doc_info.raw_stream = Some(vec![0xAB; 64]);
    doc.document.doc_info.raw_stream_dirty = false;

    assert!(doc.update_style(sid, r#"{"name":"NEW"}"#));

    assert!(
        doc.document.doc_info.raw_stream_dirty,
        "update_style 후 raw_stream_dirty=true 여야 이름 변경이 .hwp 저장에 반영된다"
    );
    let bytes = crate::serializer::doc_info::serialize_doc_info(
        &doc.document.doc_info,
        &doc.document.doc_properties,
    );
    assert_ne!(
        bytes,
        vec![0xAB; 64],
        "serialize_doc_info 가 여전히 원본 스트림을 반환"
    );
}



#[test]
fn delete_style_invalidates_docinfo_and_sections() {
    use crate::model::style::Style;
    let mut doc = HwpDocument::create_empty();
    if doc.document.doc_info.styles.is_empty() {
        doc.document.doc_info.styles.push(Style::default());
    }
    doc.document.doc_info.styles.push(Style::default());
    let sid = (doc.document.doc_info.styles.len() - 1) as u32;
    doc.document.sections[0].paragraphs[0].style_id = sid as u8;
    doc.document.doc_info.raw_stream = Some(vec![0xAB; 64]);
    doc.document.doc_info.raw_stream_dirty = false;
    doc.document.sections[0].raw_stream = Some(vec![0xCD; 64]);

    assert!(doc.delete_style(sid));

    assert!(
        doc.document.doc_info.raw_stream_dirty,
        "delete_style 후 DocInfo raw_stream_dirty=true 여야 한다"
    );
    assert!(
        doc.document.sections[0].raw_stream.is_none(),
        "문단 style_id 재배정이 반영되도록 섹션 raw_stream 이 무효화돼야 한다"
    );
}



/// [#2557] 스타일 이름에 역슬래시/개행/탭이 있어도 방출 JSON 이 파싱 가능해야 한다.
///
/// 종전엔 큰따옴표만 이스케이프해 깨진 JSON 이 나왔고, TS 측은 가드 없이
/// JSON.parse 하므로(wasm-bridge.ts:1957, :2025) 예외가 났다. getStyleAt 은 커서
/// 이동마다 호출되어 해당 문서에서 키 입력마다 편집기가 멈춘다.
#[test]
fn style_json_survives_backslash_and_control_chars() {
    let mut doc = HwpDocument::create_empty();
    {
        let styles = &mut doc.core.document.doc_info.styles;
        if styles.is_empty() {
            styles.push(crate::model::style::Style::default());
        }
        styles[0].local_name = "a\\b\nc\td\"e".to_string();
        styles[0].english_name = "x\\y\nz".to_string();
    }

    let list = doc.get_style_list();
    let parsed: Value = serde_json::from_str(&list)
        .expect("스타일 이름에 역슬래시/개행이 있어도 유효한 JSON 이어야 함");
    assert_eq!(
        parsed[0]["name"].as_str().unwrap(),
        "a\\b\nc\td\"e",
        "이스케이프 왕복 후 원래 이름이 복원돼야 함"
    );

    let at = doc.get_style_at(0, 0);
    serde_json::from_str::<Value>(&at)
        .expect("getStyleAt 도 유효한 JSON 이어야 함(커서 이동마다 호출됨)");
}
