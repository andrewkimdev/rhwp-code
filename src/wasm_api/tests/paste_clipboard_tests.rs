//! paste_clipboard_tests — tests/mod.rs 에서 무변동 이동
use super::*;

#[test]
fn test_clipboard_copy_paste_single_paragraph() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    let mut para = Paragraph::default();
    para.text = "Hello World 안녕하세요".to_string();
    para.char_count = para.text.chars().count() as u32 + 1;
    para.char_offsets = para
        .text
        .chars()
        .enumerate()
        .map(|(i, _)| i as u32)
        .collect();
    para.char_shapes = vec![crate::model::paragraph::CharShapeRef {
        start_pos: 0,
        char_shape_id: 0,
    }];
    para.line_segs = vec![crate::model::paragraph::LineSeg {
        text_start: 0,
        line_height: 400,
        text_height: 400,
        baseline_distance: 320,
        ..Default::default()
    }];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // "World" 복사 (offset 6~11)
    let result = doc.copy_selection_native(0, 0, 6, 0, 11);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("World"));

    // 내부 클립보드 확인
    assert!(doc.has_internal_clipboard_native());
    assert_eq!(doc.get_clipboard_text_native(), "World");

    // 문단 끝에 붙여넣기
    let text_len = doc.document.sections[0].paragraphs[0].text.chars().count();
    let result = doc.paste_internal_native(0, 0, text_len);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));

    // 텍스트 확인
    let text = &doc.document.sections[0].paragraphs[0].text;
    assert!(text.contains("Hello World 안녕하세요World"));
}



#[test]
fn test_clipboard_copy_paste_multi_paragraph() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();

    let make_para = |text: &str| {
        let mut p = Paragraph::default();
        p.text = text.to_string();
        p.char_count = text.chars().count() as u32 + 1;
        p.char_offsets = text.chars().enumerate().map(|(i, _)| i as u32).collect();
        p.char_shapes = vec![crate::model::paragraph::CharShapeRef {
            start_pos: 0,
            char_shape_id: 0,
        }];
        p.line_segs = vec![crate::model::paragraph::LineSeg {
            text_start: 0,
            line_height: 400,
            text_height: 400,
            baseline_distance: 320,
            ..Default::default()
        }];
        p.has_para_text = true;
        p
    };

    document.sections.push(Section {
        paragraphs: vec![
            make_para("첫 번째 문단"),
            make_para("두 번째 문단"),
            make_para("세 번째 문단"),
        ],
        ..Default::default()
    });
    doc.set_document(document);

    // 첫 번째 문단 3번째 글자부터 두 번째 문단 3번째 글자까지 복사
    let result = doc.copy_selection_native(0, 0, 3, 1, 3);
    assert!(result.is_ok());

    // 클립보드에 2개 문단이 있어야 함
    assert!(doc.has_internal_clipboard_native());
    let clip = doc.clipboard.as_ref().unwrap();
    assert_eq!(clip.paragraphs.len(), 2);

    // 세 번째 문단 끝에 붙여넣기
    let text_len = doc.document.sections[0].paragraphs[2].text.chars().count();
    let result = doc.paste_internal_native(0, 2, text_len);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));

    // 문단 수 증가 확인 (3 → 4: 분할 + 삽입)
    assert_eq!(doc.document.sections[0].paragraphs.len(), 4);
}



#[test]
fn test_clipboard_copy_control() {
    let mut doc = create_doc_with_table();

    // 표 컨트롤 복사
    let result = doc.copy_control_native(0, 0, &[], 0);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("[표]"));

    // 클립보드 확인
    assert!(doc.has_internal_clipboard_native());
    let clip = doc.clipboard.as_ref().unwrap();
    assert_eq!(clip.paragraphs.len(), 1);
    assert_eq!(clip.paragraphs[0].controls.len(), 1);
    assert!(matches!(&clip.paragraphs[0].controls[0], Control::Table(_)));
}



#[test]
fn test_paste_cascade_floating_picture() {
    let mut doc = create_doc_with_floating_picture(false, 1000, 1000);
    doc.copy_control_native(0, 0, &[], 0).expect("copy");
    doc.paste_control_native(0, 1, 0).expect("paste1");
    doc.paste_control_native(0, 1, 0).expect("paste2");

    // 원본 1000, 붙여넣기 1000+567, 1000+2*567 (PASTE_CASCADE_STEP_HU=567)
    let offs = collect_picture_voffsets(&doc);
    assert_eq!(
        offs,
        vec![1000, 1567, 2134],
        "cascade 오프셋 누적 불일치: {offs:?}"
    );
}



#[test]
fn test_paste_inline_picture_no_cascade() {
    // tac=true(글자처럼 취급)는 텍스트 흐름이 위치를 정하므로 cascade 미적용(오프셋 불변).
    let mut doc = create_doc_with_floating_picture(true, 1000, 1000);
    doc.copy_control_native(0, 0, &[], 0).expect("copy");
    doc.paste_control_native(0, 1, 0).expect("paste1");
    doc.paste_control_native(0, 1, 0).expect("paste2");

    let offs = collect_picture_voffsets(&doc);
    assert_eq!(
        offs,
        vec![1000, 1000, 1000],
        "inline 그림에 cascade 적용됨: {offs:?}"
    );
}



/// #1323: 그림 캡션 안 붙여넣기(Control::Picture 분기)도 컨트롤을 보존한다.
#[test]
fn test_paste_picture_into_picture_caption() {
    use crate::model::control::Control;
    use crate::model::shape::Caption;

    let mut doc = create_doc_with_floating_picture(true, 0, 0);
    doc.copy_control_native(0, 0, &[], 0).expect("그림 복사");

    // 본문 그림에 캡션 부여
    match &mut doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Picture(p) => {
            p.caption = Some(Caption {
                paragraphs: vec![Paragraph::default()],
                ..Default::default()
            });
        }
        other => panic!("그림이 아님: {other:?}"),
    }

    doc.paste_internal_in_cell_native(0, 0, 0, 0, 0, 0)
        .expect("캡션에 그림 붙여넣기");

    let caption = match &doc.document.sections[0].paragraphs[0].controls[0] {
        Control::Picture(p) => p.caption.as_ref().expect("캡션 존재"),
        other => panic!("그림이 아님: {other:?}"),
    };
    let pic_count: usize = caption
        .paragraphs
        .iter()
        .map(|p| {
            p.controls
                .iter()
                .filter(|c| matches!(c, Control::Picture(_)))
                .count()
        })
        .sum();
    assert_eq!(
        pic_count, 1,
        "캡션 안에 붙여넣은 그림 컨트롤이 보존되어야 한다 (#1323)"
    );
}



#[test]
fn test_clipboard_clear() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    let mut para = Paragraph::default();
    para.text = "테스트".to_string();
    para.char_count = 4;
    para.char_offsets = vec![0, 1, 2];
    para.char_shapes = vec![crate::model::paragraph::CharShapeRef {
        start_pos: 0,
        char_shape_id: 0,
    }];
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // 복사
    doc.copy_selection_native(0, 0, 0, 0, 3).unwrap();
    assert!(doc.has_internal_clipboard_native());

    // 초기화
    doc.clear_clipboard_native();
    assert!(!doc.has_internal_clipboard_native());
    assert_eq!(doc.get_clipboard_text_native(), "");
}



#[test]
fn test_clipboard_paste_empty() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    let mut para = Paragraph::default();
    para.text = "테스트".to_string();
    para.char_count = 4;
    para.char_offsets = vec![0, 1, 2];
    para.char_shapes = vec![crate::model::paragraph::CharShapeRef {
        start_pos: 0,
        char_shape_id: 0,
    }];
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // 클립보드 비어있는 상태에서 붙여넣기
    let result = doc.paste_internal_native(0, 0, 0);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":false"));
}



#[test]
fn test_export_selection_html_basic() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();

    // CharShape 추가 (bold)
    let mut cs = crate::model::style::CharShape::default();
    cs.base_size = 1200; // 12pt
    cs.bold = true;
    document.doc_info.char_shapes.push(cs);

    // ParaShape 추가 (center align)
    let mut ps = crate::model::style::ParaShape::default();
    ps.alignment = crate::model::style::Alignment::Center;
    document.doc_info.para_shapes.push(ps);

    let mut para = Paragraph::default();
    para.text = "Hello World".to_string();
    para.char_count = 12;
    para.char_offsets = (0..11).collect();
    para.char_shapes = vec![crate::model::paragraph::CharShapeRef {
        start_pos: 0,
        char_shape_id: 0,
    }];
    para.para_shape_id = 0;
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;

    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // HTML 내보내기
    let result = doc.export_selection_html_native(0, 0, 0, 0, 11);
    assert!(result.is_ok());
    let html = result.unwrap();

    // 기본 구조 확인
    assert!(html.contains("<!--StartFragment-->"));
    assert!(html.contains("<!--EndFragment-->"));
    assert!(html.contains("Hello World"));
    assert!(html.contains("<p "));
    assert!(html.contains("<span "));
    assert!(html.contains("text-align:center"));
}



#[test]
fn test_export_selection_html_partial() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();

    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());

    let mut para = Paragraph::default();
    para.text = "ABCDE".to_string();
    para.char_count = 6;
    para.char_offsets = (0..5).collect();
    para.char_shapes = vec![crate::model::paragraph::CharShapeRef {
        start_pos: 0,
        char_shape_id: 0,
    }];
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;

    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // 부분 선택 (B, C, D)
    let result = doc.export_selection_html_native(0, 0, 1, 0, 4);
    assert!(result.is_ok());
    let html = result.unwrap();

    assert!(html.contains("BCD"));
    // "ABCDE" 전체 문자열이 포함되지 않아야 함
    assert!(!html.contains("ABCDE"));
    // 정확히 BCD만 span 안에 있는지 확인
    assert!(html.contains(">BCD<"));
}



#[test]
fn test_paste_html_plain_text() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    let mut para = Paragraph::default();
    para.text = "가나다".to_string();
    para.char_count = para.text.encode_utf16().count() as u32;
    para.char_offsets = para
        .text
        .chars()
        .scan(0u32, |acc, c| {
            let off = *acc;
            *acc += c.len_utf16() as u32;
            Some(off)
        })
        .collect();
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // 플레인 텍스트 HTML 붙여넣기
    let html = "<html><body><!--StartFragment--><p>안녕하세요</p><!--EndFragment--></body></html>";
    let result = doc.paste_html_native(0, 0, 3, html);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));

    // 삽입 후 텍스트 확인
    let text = &doc.document.sections[0].paragraphs[0].text;
    assert!(text.contains("안녕하세요"));
    assert!(text.contains("가나다"));
}



#[test]
fn test_paste_html_styled_text() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    let mut para = Paragraph::default();
    para.text = "테스트".to_string();
    para.char_count = para.text.encode_utf16().count() as u32;
    para.char_offsets = para
        .text
        .chars()
        .scan(0u32, |acc, c| {
            let off = *acc;
            *acc += c.len_utf16() as u32;
            Some(off)
        })
        .collect();
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // 볼드+색상 스타일 HTML
    let html = r#"<html><body><!--StartFragment-->
            <p style="text-align:center;">
                <span style="font-weight:bold;color:#ff0000;">볼드 빨강</span>
            </p>
        <!--EndFragment--></body></html>"#;

    let result = doc.paste_html_native(0, 0, 0, html);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));

    // CharShape가 추가되었는지 확인 (bold + red color)
    let char_shapes_count = doc.document.doc_info.char_shapes.len();
    assert!(char_shapes_count > 1, "새 CharShape가 생성되어야 함");

    // 볼드 속성 확인
    let bold_shape = doc.document.doc_info.char_shapes.iter().find(|cs| cs.bold);
    assert!(bold_shape.is_some(), "볼드 CharShape가 존재해야 함");
}



#[test]
fn test_paste_html_multi_paragraph() {
    let mut doc = HwpDocument::create_empty();
    let mut document = Document::default();
    document
        .doc_info
        .char_shapes
        .push(crate::model::style::CharShape::default());
    document
        .doc_info
        .para_shapes
        .push(crate::model::style::ParaShape::default());
    let mut para = Paragraph::default();
    para.text = "원본".to_string();
    para.char_count = para.text.encode_utf16().count() as u32;
    para.char_offsets = para
        .text
        .chars()
        .scan(0u32, |acc, c| {
            let off = *acc;
            *acc += c.len_utf16() as u32;
            Some(off)
        })
        .collect();
    para.line_segs = vec![crate::model::paragraph::LineSeg::default()];
    para.has_para_text = true;
    document.sections.push(Section {
        paragraphs: vec![para],
        ..Default::default()
    });
    doc.set_document(document);

    // 다중 문단 HTML
    let html = r#"<html><body><!--StartFragment-->
            <p>첫째 문단</p>
            <p>둘째 문단</p>
            <p>셋째 문단</p>
        <!--EndFragment--></body></html>"#;

    let result = doc.paste_html_native(0, 0, 2, html);
    assert!(result.is_ok());
    let json = result.unwrap();
    assert!(json.contains("\"ok\":true"));

    // 문단 수 확인 (원본 1 + 삽입 3 = 최소 3)
    let para_count = doc.document.sections[0].paragraphs.len();
    assert!(
        para_count >= 3,
        "최소 3개 문단이어야 함, 실제: {}",
        para_count
    );
}



/// 재직렬화 격리 테스트: paste 없이 raw_stream 제거만으로 레코드 수 비교
#[test]
fn test_roundtrip_isolation_no_paste() {
    use crate::parser::record::Record;
    use crate::parser::tags;

    let orig_path = "pasts/20250130-hongbo-p2.hwp";
    if !std::path::Path::new(orig_path).exists() {
        eprintln!("SKIP: 파일 없음");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    // Step 1: Re-serialize WITHOUT paste (just clear raw_stream)
    doc.document.sections[0].raw_stream = None;
    let saved_data = doc.export_hwp_native().unwrap();
    eprintln!(
        "원본: {} bytes, 재직렬화(no paste): {} bytes",
        orig_data.len(),
        saved_data.len()
    );

    // Step 2: Re-parse the saved file
    let doc2 = HwpDocument::from_bytes(&saved_data);
    match &doc2 {
        Ok(d) => eprintln!(
            "재파싱 성공: {} sections, {} paragraphs",
            d.document().sections.len(),
            d.document().sections[0].paragraphs.len()
        ),
        Err(e) => eprintln!("재파싱 실패: {:?}", e),
    }
    assert!(doc2.is_ok(), "재직렬화 파일 파싱 실패");

    // Step 3: Compare record counts
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

    eprintln!("\n=== Record count comparison (no paste) ===");
    eprintln!("Original records: {}", orig_recs.len());
    eprintln!("Saved records: {}", saved_recs.len());

    let count_tag = |recs: &[Record], tag: u16| recs.iter().filter(|r| r.tag_id == tag).count();

    let tags_to_check: [(u16, &str); 7] = [
        (tags::HWPTAG_PARA_HEADER, "PARA_HEADER"),
        (tags::HWPTAG_PARA_TEXT, "PARA_TEXT"),
        (tags::HWPTAG_PARA_CHAR_SHAPE, "PARA_CHAR_SHAPE"),
        (tags::HWPTAG_PARA_LINE_SEG, "PARA_LINE_SEG"),
        (tags::HWPTAG_CTRL_HEADER, "CTRL_HEADER"),
        (tags::HWPTAG_LIST_HEADER, "LIST_HEADER"),
        (tags::HWPTAG_TABLE, "TABLE"),
    ];

    let mut any_diff = false;
    for (tag, name) in &tags_to_check {
        let orig_cnt = count_tag(&orig_recs, *tag);
        let saved_cnt = count_tag(&saved_recs, *tag);
        let diff = saved_cnt as i64 - orig_cnt as i64;
        if diff != 0 {
            eprintln!(
                "  {}: {} → {} ({}{}) ← DIFF",
                name,
                orig_cnt,
                saved_cnt,
                if diff > 0 { "+" } else { "" },
                diff
            );
            any_diff = true;
        } else {
            eprintln!("  {}: {} (동일)", name, orig_cnt);
        }
    }

    if !any_diff {
        eprintln!("\n모든 레코드 타입 동일 ✓");
    }

    // Step 4: Check that PARA_HEADER char_count matches PARA_TEXT existence
    eprintln!("\n=== PARA_HEADER/PARA_TEXT consistency check ===");
    let mut inconsistencies = 0;
    let mut i = 0;
    while i < saved_recs.len() {
        if saved_recs[i].tag_id == tags::HWPTAG_PARA_HEADER {
            let ph_data = &saved_recs[i].data;
            let ph_level = saved_recs[i].level;
            let nchars = if ph_data.len() >= 4 {
                u32::from_le_bytes([ph_data[0], ph_data[1], ph_data[2], ph_data[3]]) & 0x7FFFFFFF
            } else {
                0
            };
            // Check next record
            let has_text = i + 1 < saved_recs.len()
                && saved_recs[i + 1].tag_id == tags::HWPTAG_PARA_TEXT
                && saved_recs[i + 1].level == ph_level + 1;
            if nchars > 1 && !has_text {
                eprintln!(
                    "  rec[{}] PARA_HEADER nchars={} but NO PARA_TEXT follows!",
                    i, nchars
                );
                inconsistencies += 1;
            }
            if nchars <= 1 && has_text {
                let pt_size = saved_recs[i + 1].data.len();
                eprintln!("  rec[{}] PARA_HEADER nchars={} but HAS PARA_TEXT ({}B) — might be OK (terminator only)", i, nchars, pt_size);
            }
        }
        i += 1;
    }
    eprintln!("  Total inconsistencies: {}", inconsistencies);
}



/// CharShape 보존 검증: 붙여넣기 후 내보내기 시 원본 CharShape가 모두 보존되는지 확인
#[test]
fn test_charshape_preservation_after_paste() {
    use crate::parser::record::Record;
    use crate::parser::tags;

    // raw_stream에서 특정 tag의 레코드 수 세기
    fn count_tag_in_raw(raw: &[u8], target_tag: u16) -> usize {
        Record::read_all(raw)
            .unwrap_or_default()
            .iter()
            .filter(|r| r.tag_id == target_tag)
            .count()
    }

    let orig_path = "pasts/20250130-hongbo-p2.hwp";
    if !std::path::Path::new(orig_path).exists() {
        eprintln!("SKIP: 파일 없음");
        return;
    }

    let orig_data = std::fs::read(orig_path).unwrap();
    let mut doc = HwpDocument::from_bytes(&orig_data).unwrap();

    // 원본 CharShape 개수 확인
    let orig_cs_count = doc.document.doc_info.char_shapes.len();
    let orig_ps_count = doc.document.doc_info.para_shapes.len();
    eprintln!(
        "원본 CharShape: {}, ParaShape: {}",
        orig_cs_count, orig_ps_count
    );

    // raw_stream에서 CharShape 레코드 개수 확인
    let orig_raw_cs = doc
        .document
        .doc_info
        .raw_stream
        .as_ref()
        .map(|raw| count_tag_in_raw(raw, tags::HWPTAG_CHAR_SHAPE))
        .unwrap_or(0);
    eprintln!("원본 raw_stream CharShape 레코드: {}", orig_raw_cs);
    assert_eq!(
        orig_cs_count, orig_raw_cs,
        "모델과 raw_stream의 CharShape 개수 불일치"
    );

    // HTML 테이블 붙여넣기
    let table_html = r#"<table><tr><td style="font-weight:bold">Bold A</td><td style="color:red">Red B</td></tr><tr><td>Cell C</td><td style="font-style:italic">Italic D</td></tr></table>"#;
    let last_para = doc.document.sections[0].paragraphs.len() - 1;
    doc.paste_html_native(0, last_para, 0, table_html).unwrap();

    // 붙여넣기 후 CharShape 개수 확인
    let post_cs_count = doc.document.doc_info.char_shapes.len();
    let post_ps_count = doc.document.doc_info.para_shapes.len();
    eprintln!(
        "붙여넣기 후 CharShape: {}, ParaShape: {}",
        post_cs_count, post_ps_count
    );
    assert!(
        post_cs_count >= orig_cs_count,
        "CharShape 개수 감소! {} → {}",
        orig_cs_count,
        post_cs_count
    );

    // raw_stream CharShape 레코드 확인
    let post_raw_cs = doc
        .document
        .doc_info
        .raw_stream
        .as_ref()
        .map(|raw| count_tag_in_raw(raw, tags::HWPTAG_CHAR_SHAPE))
        .unwrap_or(0);
    eprintln!("붙여넣기 후 raw_stream CharShape 레코드: {}", post_raw_cs);
    assert!(
        post_raw_cs >= orig_raw_cs,
        "raw_stream CharShape 감소! {} → {}",
        orig_raw_cs,
        post_raw_cs
    );
    assert_eq!(
        post_cs_count, post_raw_cs,
        "붙여넣기 후 모델({})과 raw_stream({})의 CharShape 불일치",
        post_cs_count, post_raw_cs
    );

    // 내보내기 후 재파싱하여 CharShape 확인
    let saved_data = doc.export_hwp_native().unwrap();
    let saved_doc = crate::parser::parse_hwp(&saved_data).unwrap();
    let saved_cs_count = saved_doc.doc_info.char_shapes.len();
    let saved_ps_count = saved_doc.doc_info.para_shapes.len();
    eprintln!(
        "재파싱 CharShape: {}, ParaShape: {}",
        saved_cs_count, saved_ps_count
    );
    assert!(
        saved_cs_count >= orig_cs_count,
        "저장 후 CharShape 감소! 원본 {} → 저장 {}",
        orig_cs_count,
        saved_cs_count
    );

    // 모든 PARA_CHAR_SHAPE가 유효한 CharShape ID를 참조하는지 확인
    let mut max_cs_id: u32 = 0;
    for section in &saved_doc.sections {
        for para in &section.paragraphs {
            for cs_ref in &para.char_shapes {
                if cs_ref.char_shape_id > max_cs_id {
                    max_cs_id = cs_ref.char_shape_id;
                }
            }
            for ctrl in &para.controls {
                if let Control::Table(tbl) = ctrl {
                    for cell in &tbl.cells {
                        for cp in &cell.paragraphs {
                            for cs_ref in &cp.char_shapes {
                                if cs_ref.char_shape_id > max_cs_id {
                                    max_cs_id = cs_ref.char_shape_id;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "최대 CharShape ID 참조: {}, 사용 가능 범위: 0..{}",
        max_cs_id, saved_cs_count
    );
    assert!(
        (max_cs_id as usize) < saved_cs_count,
        "CharShape ID {} 참조 but 가용 개수 {}! (dangling reference)",
        max_cs_id,
        saved_cs_count
    );

    eprintln!("=== CharShape 보존 검증 통과 ===");
}



#[test]
fn test_step2_paste_area() {
    use crate::parser::cfb_reader::CfbReader;
    use crate::parser::record::Record;
    use crate::parser::tags;

    // ============================================================
    // Helper functions
    // ============================================================
    fn read_u32_le(data: &[u8], offset: usize) -> u32 {
        if offset + 4 <= data.len() {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        } else {
            0
        }
    }
    fn read_u16_le(data: &[u8], offset: usize) -> u16 {
        if offset + 2 <= data.len() {
            u16::from_le_bytes([data[offset], data[offset + 1]])
        } else {
            0
        }
    }
    fn hex_dump(data: &[u8], max: usize) -> String {
        let show = std::cmp::min(data.len(), max);
        let hex: Vec<String> = data[..show].iter().map(|b| format!("{:02x}", b)).collect();
        let mut s = hex.join(" ");
        if data.len() > max {
            s.push_str(&format!(" ...({} more)", data.len() - max));
        }
        s
    }
    fn hex_full(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
    fn utf16le_decode(data: &[u8]) -> String {
        let mut result = String::new();
        let mut pos = 0;
        while pos + 1 < data.len() {
            let code = u16::from_le_bytes([data[pos], data[pos + 1]]);
            if code == 0x000D {
                result.push_str("\\r");
            } else if code == 0x000A {
                result.push_str("\\n");
            } else if code == 0x000B {
                result.push_str("{CTRL}");
                pos += 14;
            } else if code == 0x0002 {
                result.push_str("{SECD}");
                pos += 14;
            } else if code == 0x0003 {
                result.push_str("{FLD_BEGIN}");
                pos += 14;
            } else if code == 0x0004 {
                result.push_str("{FLD_END}");
                pos += 14;
            } else if code == 0x0008 {
                result.push_str("{INLINE}");
                pos += 14;
            } else if code < 0x0020 {
                result.push_str(&format!("{{0x{:04X}}}", code));
            } else if let Some(ch) = char::from_u32(code as u32) {
                result.push(ch);
            } else {
                result.push_str(&format!("{{0x{:04X}}}", code));
            }
            pos += 2;
        }
        result
    }

    // ============================================================
    // Print detailed record info
    // ============================================================
    fn print_record_detail(label: &str, idx: usize, rec: &Record) {
        let tag_name = tags::tag_name(rec.tag_id);
        eprintln!(
            "  [{:>3}] lvl={} tag={:<20} size={:<6} tag_id={}",
            idx, rec.level, tag_name, rec.size, rec.tag_id
        );

        if rec.tag_id == tags::HWPTAG_PARA_HEADER {
            if rec.data.len() >= 22 {
                let raw_nchars = read_u32_le(&rec.data, 0);
                let msb = (raw_nchars >> 31) & 1;
                let char_count = raw_nchars & 0x7FFFFFFF;
                let control_mask = read_u32_le(&rec.data, 4);
                let para_shape_id = read_u16_le(&rec.data, 8);
                let style_id = rec.data[10];
                let break_type = rec.data[11];
                let num_char_shapes = read_u16_le(&rec.data, 12);
                let num_range_tags = read_u16_le(&rec.data, 14);
                let num_line_segs = read_u16_le(&rec.data, 16);
                let para_inst_id = read_u32_le(&rec.data, 18);
                eprintln!(
                    "         PARA_HEADER: char_count={} msb={} control_mask=0x{:08X}",
                    char_count, msb, control_mask
                );
                eprintln!(
                    "           para_shape_id={} style_id={} break_type={}",
                    para_shape_id, style_id, break_type
                );
                eprintln!(
                    "           numCharShapes={} numRangeTags={} numLineSegs={} paraInstId={}",
                    num_char_shapes, num_range_tags, num_line_segs, para_inst_id
                );
                eprintln!("           full hex: {}", hex_full(&rec.data));
            } else {
                eprintln!(
                    "         PARA_HEADER (short {}b): {}",
                    rec.data.len(),
                    hex_full(&rec.data)
                );
            }
        } else if rec.tag_id == tags::HWPTAG_PARA_TEXT {
            let show_bytes = std::cmp::min(rec.data.len(), 100);
            eprintln!(
                "         PARA_TEXT hex(first {}): {}",
                show_bytes,
                hex_dump(&rec.data, 100)
            );
            eprintln!(
                "         PARA_TEXT decoded: \"{}\"",
                utf16le_decode(&rec.data)
            );
        } else if rec.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE {
            let n_pairs = rec.data.len() / 8;
            for p in 0..n_pairs {
                let pos_val = read_u32_le(&rec.data, p * 8);
                let cs_id = read_u32_le(&rec.data, p * 8 + 4);
                eprintln!(
                    "         PARA_CHAR_SHAPE[{}]: pos={} => CS_id={}",
                    p, pos_val, cs_id
                );
            }
            eprintln!("         full hex: {}", hex_full(&rec.data));
        } else if rec.tag_id == tags::HWPTAG_PARA_LINE_SEG {
            eprintln!("         PARA_LINE_SEG hex: {}", hex_full(&rec.data));
            // Decode line seg entries (each 36 bytes)
            let entry_size = 36;
            let n_entries = rec.data.len() / entry_size;
            for e in 0..n_entries {
                let off = e * entry_size;
                let text_start = read_u32_le(&rec.data, off);
                let y_pos = read_u32_le(&rec.data, off + 4) as i32;
                let height = read_u32_le(&rec.data, off + 8);
                let text_height = read_u32_le(&rec.data, off + 12);
                let baseline = read_u32_le(&rec.data, off + 16);
                let spacing = read_u32_le(&rec.data, off + 20);
                let x_pos = read_u32_le(&rec.data, off + 24) as i32;
                let seg_width = read_u32_le(&rec.data, off + 28);
                let tag_flags = read_u32_le(&rec.data, off + 32);
                eprintln!("           seg[{}]: textStart={} yPos={} h={} textH={} baseline={} spacing={} xPos={} segW={} flags=0x{:08X}",
                        e, text_start, y_pos, height, text_height, baseline, spacing, x_pos, seg_width, tag_flags);
            }
        } else if rec.tag_id == tags::HWPTAG_CTRL_HEADER {
            if rec.data.len() >= 4 {
                let ctrl_id =
                    u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
                let be = ctrl_id.to_be_bytes();
                let ascii: String = be
                    .iter()
                    .map(|&b| {
                        if b.is_ascii_graphic() || b == b' ' {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                eprintln!(
                    "         CTRL_HEADER: ctrl=\"{}\" (0x{:08X})",
                    ascii, ctrl_id
                );
            }
            eprintln!("         full hex: {}", hex_dump(&rec.data, 80));
        } else if rec.tag_id == tags::HWPTAG_TABLE {
            if rec.data.len() >= 8 {
                let attr = read_u32_le(&rec.data, 0);
                let rows = read_u16_le(&rec.data, 4);
                let cols = read_u16_le(&rec.data, 6);
                eprintln!(
                    "         TABLE: attr=0x{:08X} rows={} cols={}",
                    attr, rows, cols
                );
                if rec.data.len() >= 10 {
                    let cell_spacing = read_u16_le(&rec.data, 8);
                    eprintln!("           cellSpacing={}", cell_spacing);
                }
                // border fill id per row: after cell_spacing(2) + padding(2*rows for row sizes)
                let row_sizes_start = 10;
                for r in 0..rows as usize {
                    if row_sizes_start + (r + 1) * 2 <= rec.data.len() {
                        let rs = read_u16_le(&rec.data, row_sizes_start + r * 2);
                        eprintln!("           rowSize[{}]={}", r, rs);
                    }
                }
            }
            eprintln!("         full hex(40): {}", hex_dump(&rec.data, 40));
            eprintln!("         full hex(all): {}", hex_full(&rec.data));
        } else if rec.tag_id == tags::HWPTAG_LIST_HEADER {
            if rec.data.len() >= 6 {
                let para_count = read_u16_le(&rec.data, 0);
                let attr = read_u32_le(&rec.data, 2);
                eprintln!(
                    "         LIST_HEADER: paraCount={} attr=0x{:08X}",
                    para_count, attr
                );
                if rec.data.len() >= 47 {
                    // Cell-specific fields (for table cells)
                    let col_addr = read_u16_le(&rec.data, 8);
                    let row_addr = read_u16_le(&rec.data, 10);
                    let col_span = read_u16_le(&rec.data, 12);
                    let row_span = read_u16_le(&rec.data, 14);
                    let cell_w = read_u32_le(&rec.data, 16);
                    let cell_h = read_u32_le(&rec.data, 20);
                    let padding_l = read_u16_le(&rec.data, 24);
                    let padding_r = read_u16_le(&rec.data, 26);
                    let padding_t = read_u16_le(&rec.data, 28);
                    let padding_b = read_u16_le(&rec.data, 30);
                    let border_fill_id = read_u16_le(&rec.data, 32);
                    let cell_w2 = read_u32_le(&rec.data, 34);
                    eprintln!(
                        "           col={} row={} colSpan={} rowSpan={}",
                        col_addr, row_addr, col_span, row_span
                    );
                    eprintln!(
                        "           cellW={} cellH={} pad=({},{},{},{}) borderFillId={} cellW2={}",
                        cell_w,
                        cell_h,
                        padding_l,
                        padding_r,
                        padding_t,
                        padding_b,
                        border_fill_id,
                        cell_w2
                    );
                }
            }
            eprintln!("         full hex: {}", hex_full(&rec.data));
        } else {
            eprintln!("         hex: {}", hex_dump(&rec.data, 60));
        }
    }

    // ============================================================
    // Load files
    // ============================================================
    let files: Vec<(&str, &str)> = vec![
        ("template/empty-step2-p.hwp", "VALID"),
        ("template/empty-step2_saved_err.hwp", "DAMAGED"),
    ];

    struct FileData {
        label: String,
        body_records: Vec<Record>,
        body_raw_len: usize,
    }

    let mut all_files: Vec<FileData> = Vec::new();

    for (path, label) in &files {
        let bytes =
            std::fs::read(path).unwrap_or_else(|e| panic!("File read failed: {} - {}", path, e));
        let mut cfb =
            CfbReader::open(&bytes).unwrap_or_else(|e| panic!("CFB open failed: {} - {}", path, e));
        let body_data = cfb
            .read_body_text_section(0, true, false)
            .unwrap_or_else(|e| panic!("BodyText read failed: {} - {}", path, e));
        let body_raw_len = body_data.len();
        let body_records = Record::read_all(&body_data)
            .unwrap_or_else(|e| panic!("Record parse failed: {} - {}", path, e));
        let rec_count = body_records.len();
        all_files.push(FileData {
            label: label.to_string(),
            body_records,
            body_raw_len,
        });
        eprintln!(
            "[{}] {} loaded: {} bytes decompressed, {} records",
            label, path, body_raw_len, rec_count
        );
    }

    let valid_recs = &all_files[0].body_records;
    let damaged_recs = &all_files[1].body_records;

    // ============================================================
    // Find the pasted table area: PARA_HEADER with control_mask containing 0x800
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!("  Finding pasted table PARA_HEADER (control_mask has bit 0x800 = table control)");
    eprintln!("{}", "=".repeat(120));

    let mut valid_table_start: Option<usize> = None;
    let mut damaged_table_start: Option<usize> = None;

    for (i, rec) in valid_recs.iter().enumerate() {
        if rec.tag_id == tags::HWPTAG_PARA_HEADER && rec.data.len() >= 8 {
            let mask = read_u32_le(&rec.data, 4);
            if mask & 0x800 != 0 {
                eprintln!(
                    "  VALID: pasted table PARA_HEADER found at record {} (control_mask=0x{:08X})",
                    i, mask
                );
                if valid_table_start.is_none() {
                    valid_table_start = Some(i);
                }
            }
        }
    }
    for (i, rec) in damaged_recs.iter().enumerate() {
        if rec.tag_id == tags::HWPTAG_PARA_HEADER && rec.data.len() >= 8 {
            let mask = read_u32_le(&rec.data, 4);
            if mask & 0x800 != 0 {
                eprintln!("  DAMAGED: pasted table PARA_HEADER found at record {} (control_mask=0x{:08X})", i, mask);
                if damaged_table_start.is_none() {
                    damaged_table_start = Some(i);
                }
            }
        }
    }

    // Also find CTRL_HEADER with "tbl " to identify the second table
    eprintln!("\n  Scanning for ALL CTRL_HEADER 'tbl ' records:");
    for (i, rec) in valid_recs.iter().enumerate() {
        if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
            if ctrl_id == 0x7462_6C20 {
                // " lbt" = "tbl " in big endian display
                eprintln!("    VALID: tbl CTRL_HEADER at record {}", i);
            }
        }
    }
    for (i, rec) in damaged_recs.iter().enumerate() {
        if rec.tag_id == tags::HWPTAG_CTRL_HEADER && rec.data.len() >= 4 {
            let ctrl_id = u32::from_le_bytes([rec.data[0], rec.data[1], rec.data[2], rec.data[3]]);
            if ctrl_id == 0x7462_6C20 {
                eprintln!("    DAMAGED: tbl CTRL_HEADER at record {}", i);
            }
        }
    }

    // ============================================================
    // Dump records around the pasted table area in both files
    // ============================================================
    // Use the second table start if found, otherwise start from first table para header
    let v_start = valid_table_start.unwrap_or(30);
    let d_start = damaged_table_start.unwrap_or(30);

    // Print a generous range: from 4 records before the table para to end or +200 records
    let v_range_start = if v_start >= 4 { v_start - 4 } else { 0 };
    let d_range_start = if d_start >= 4 { d_start - 4 } else { 0 };
    let v_range_end = std::cmp::min(valid_recs.len(), v_start + 200);
    let d_range_end = std::cmp::min(damaged_recs.len(), d_start + 200);

    eprintln!("\n{}", "=".repeat(120));
    eprintln!(
        "  VALID FILE: Records {} - {} (around pasted table)",
        v_range_start,
        v_range_end - 1
    );
    eprintln!("{}", "=".repeat(120));

    for i in v_range_start..v_range_end {
        print_record_detail("VALID", i, &valid_recs[i]);
    }

    eprintln!("\n{}", "=".repeat(120));
    eprintln!(
        "  DAMAGED FILE: Records {} - {} (around pasted table)",
        d_range_start,
        d_range_end - 1
    );
    eprintln!("{}", "=".repeat(120));

    for i in d_range_start..d_range_end {
        print_record_detail("DAMAGED", i, &damaged_recs[i]);
    }

    // ============================================================
    // Side-by-side comparison of matching region
    // ============================================================
    eprintln!("\n{}", "=".repeat(120));
    eprintln!(
        "  SIDE-BY-SIDE: VALID[{}..] vs DAMAGED[{}..] - first 60 records",
        v_start, d_start
    );
    eprintln!("{}", "=".repeat(120));

    let compare_count = 60;
    for offset in 0..compare_count {
        let vi = v_start + offset;
        let di = d_start + offset;
        if vi >= valid_recs.len() && di >= damaged_recs.len() {
            break;
        }

        let have_v = vi < valid_recs.len();
        let have_d = di < damaged_recs.len();

        let (v_tag, v_lvl, v_sz) = if have_v {
            (
                tags::tag_name(valid_recs[vi].tag_id).to_string(),
                valid_recs[vi].level,
                valid_recs[vi].size,
            )
        } else {
            ("---".to_string(), 0u16, 0u32)
        };

        let (d_tag, d_lvl, d_sz) = if have_d {
            (
                tags::tag_name(damaged_recs[di].tag_id).to_string(),
                damaged_recs[di].level,
                damaged_recs[di].size,
            )
        } else {
            ("---".to_string(), 0u16, 0u32)
        };

        let status = if !have_v || !have_d {
            "MISSING"
        } else if valid_recs[vi].tag_id != damaged_recs[di].tag_id {
            "TAG_DIFF"
        } else if valid_recs[vi].level != damaged_recs[di].level {
            "LVL_DIFF"
        } else if valid_recs[vi].data != damaged_recs[di].data {
            "DATA_DIFF"
        } else {
            "OK"
        };

        let marker = if status != "OK" { ">>>" } else { "   " };
        eprintln!(
            "{} off={:<3} V[{:>3}] {:<20} lvl={} sz={:<5} | D[{:>3}] {:<20} lvl={} sz={:<5} | {}",
            marker, offset, vi, v_tag, v_lvl, v_sz, di, d_tag, d_lvl, d_sz, status
        );

        if status != "OK" && have_v && have_d {
            // Show critical details for differing records
            let vr = &valid_recs[vi];
            let dr = &damaged_recs[di];

            if vr.tag_id == tags::HWPTAG_PARA_HEADER && dr.tag_id == tags::HWPTAG_PARA_HEADER {
                let v_cc = read_u32_le(&vr.data, 0);
                let d_cc = read_u32_le(&dr.data, 0);
                let v_mask = read_u32_le(&vr.data, 4);
                let d_mask = read_u32_le(&dr.data, 4);
                let v_ps = read_u16_le(&vr.data, 8);
                let d_ps = read_u16_le(&dr.data, 8);
                eprintln!(
                    "       V: cc={} mask=0x{:08X} ps={}  D: cc={} mask=0x{:08X} ps={}",
                    v_cc & 0x7FFFFFFF,
                    v_mask,
                    v_ps,
                    d_cc & 0x7FFFFFFF,
                    d_mask,
                    d_ps
                );
            }

            if vr.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE
                && dr.tag_id == tags::HWPTAG_PARA_CHAR_SHAPE
            {
                let v_pairs = vr.data.len() / 8;
                let d_pairs = dr.data.len() / 8;
                eprintln!("       V pairs: {} D pairs: {}", v_pairs, d_pairs);
                for p in 0..std::cmp::max(v_pairs, d_pairs) {
                    let v_str = if p < v_pairs {
                        format!(
                            "pos{}=>CS{}",
                            read_u32_le(&vr.data, p * 8),
                            read_u32_le(&vr.data, p * 8 + 4)
                        )
                    } else {
                        "---".to_string()
                    };
                    let d_str = if p < d_pairs {
                        format!(
                            "pos{}=>CS{}",
                            read_u32_le(&dr.data, p * 8),
                            read_u32_le(&dr.data, p * 8 + 4)
                        )
                    } else {
                        "---".to_string()
                    };
                    eprintln!("         [{}] V: {}  D: {}", p, v_str, d_str);
                }
            }

            if vr.tag_id == tags::HWPTAG_LIST_HEADER && dr.tag_id == tags::HWPTAG_LIST_HEADER {
                let v_pc = read_u16_le(&vr.data, 0);
                let d_pc = read_u16_le(&dr.data, 0);
                let v_bf = if vr.data.len() >= 34 {
                    read_u16_le(&vr.data, 32)
                } else {
                    0
                };
                let d_bf = if dr.data.len() >= 34 {
                    read_u16_le(&dr.data, 32)
                } else {
                    0
                };
                eprintln!(
                    "       V: paraCount={} borderFillId={}  D: paraCount={} borderFillId={}",
                    v_pc, v_bf, d_pc, d_bf
                );
                if vr.data.len() >= 14 && dr.data.len() >= 14 {
                    let v_col = read_u16_le(&vr.data, 8);
                    let v_row = read_u16_le(&vr.data, 10);
                    let d_col = read_u16_le(&dr.data, 8);
                    let d_row = read_u16_le(&dr.data, 10);
                    eprintln!(
                        "       V: col={} row={}  D: col={} row={}",
                        v_col, v_row, d_col, d_row
                    );
                }
            }

            // Show hex diff
            eprintln!("       V hex: {}", hex_dump(&vr.data, 60));
            eprintln!("       D hex: {}", hex_dump(&dr.data, 60));

            // Find first diff byte
            let min_len = std::cmp::min(vr.data.len(), dr.data.len());
            if let Some(pos) = (0..min_len).find(|&j| vr.data[j] != dr.data[j]) {
                eprintln!(
                    "       First diff at byte {}: V=0x{:02x} D=0x{:02x}",
                    pos, vr.data[pos], dr.data[pos]
                );
            }
        }
    }

    eprintln!("\n=== test_step2_paste_area complete ===");
}
