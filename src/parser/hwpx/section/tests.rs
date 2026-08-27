//! tests — section.rs 에서 무변동 이동
use super::*;

#[test]
fn test_parse_simple_section() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t>Hello World</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert_eq!(section.paragraphs.len(), 1);
    assert_eq!(section.paragraphs[0].text, "Hello World");
    assert_eq!(section.paragraphs[0].para_shape_id, 0);
}

// ---------- #2957: autoNumFormat 원 문자(CIRCLED_DIGIT) 인식 ----------

#[test]
fn task2957_autonum_format_circled_digit_parses_as_1() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0"><hp:ctrl><hp:autoNum num="1" numType="FOOTNOTE"><hp:autoNumFormat type="CIRCLED_DIGIT" userChar="" prefixChar="" suffixChar="" supscript="0"/></hp:autoNum></hp:ctrl><hp:t> </hp:t></hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    let an = section.paragraphs[0]
        .controls
        .iter()
        .find_map(|c| match c {
            Control::AutoNumber(an) => Some(an),
            _ => None,
        })
        .expect("autoNum 컨트롤이 파싱돼야 함");
    assert_eq!(
        an.format, 1,
        "type=\"CIRCLED_DIGIT\" 는 format=1(circled digit) 로 인식돼야 함(#2957)"
    );
}

// ---------- #1382: autoNum 폭 축 일관화 ----------

#[test]
fn task1382_calc_counts_autonum_as_8_units() {
    // \u{0012}(AUTO_NUMBER) 는 placeholder 포함 8유닛 — offsets 축과 동일.
    let parts = vec!["\u{0012}".to_string(), " ".to_string()];
    assert_eq!(calc_utf16_len_from_parts(&parts), 9);
}

#[test]
fn task1382_autonum_run_boundary_on_offsets_axis() {
    // 143E 각주 패턴: run1(ctrl autoNum + 공백) + run2(텍스트) →
    // run2 경계는 offsets 축 9 (autoNum 8 + 공백 1). 종전 1유닛 축에서는 2.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="10"><hp:ctrl><hp:autoNum num="1" numType="FOOTNOTE"/></hp:ctrl><hp:t> </hp:t></hp:run>
<hp:run charPrIDRef="11"><hp:t>본문</hp:t></hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    let p = &section.paragraphs[0];
    assert_eq!(p.text, "  본문", "placeholder 공백 + 실제 공백 + 텍스트");
    assert_eq!(p.char_offsets, vec![0, 8, 9, 10]);
    assert_eq!(
        p.char_shapes
            .iter()
            .map(|c| (c.start_pos, c.char_shape_id))
            .collect::<Vec<_>>(),
        vec![(0, 10), (9, 11)],
        "run2 경계는 offsets 축 9"
    );
}

#[test]
fn task1654_hide_first_empty_line_sets_hwp5_section_flag() {
    // HWPX visibility 값은 HWP 저장 경로가 읽는 SectionDef.flags bit 19와
    // 함께 동기화되어야 한다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr id="" textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" tabStopVal="4000" tabStopUnit="HWPUNIT" outlineShapeIDRef="1" memoShapeIDRef="0" textVerticalWidthHead="0" masterPageCnt="0">
    <hp:visibility hideFirstHeader="0" hideFirstFooter="0" hideFirstMasterPage="0" border="SHOW_ALL" fill="SHOW_ALL" hideFirstPageNum="0" hideFirstEmptyLine="1" showLineNumber="0"/>
  </hp:secPr>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert!(section.section_def.hide_empty_line);
    assert_ne!(section.section_def.flags & 0x0008_0000, 0);

    let Control::SectionDef(section_def) = &section.paragraphs[0].controls[0] else {
        panic!("첫 컨트롤은 SectionDef 여야 함");
    };
    assert!(section_def.hide_empty_line);
    assert_ne!(section_def.flags & 0x0008_0000, 0);
}

#[test]
fn equation_missing_attrs_fall_back_to_owpml_defaults() {
    // OWPML 스키마(ParaList, EquationType)의 속성 기본값:
    //   version  = "Equation Version 60"
    //   baseLine = 85
    //   font     = "HYhwpEQ"
    // 속성이 생략된 수식을 파싱하면 스펙 기본값으로 복원되어야 한다.
    // (직렬화기는 세 속성을 무조건 방출하므로, 파서가 0/"" 로 복원하면
    //  왕복 시 baseLine="0" font="" version="" 으로 값이 변형된다.)
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:equation id="1" zOrder="0" numberingType="EQUATION" textWrap="TOP_AND_BOTTOM" lock="0">
    <hp:script>1 over 2</hp:script>
    <hp:sz width="2000" widthRelTo="ABSOLUTE" height="1000" heightRelTo="ABSOLUTE"/>
    <hp:pos treatAsChar="1" affectLSpacing="0" flowWithText="1" allowOverlap="0" holdAnchorAndSO="0" vertRelTo="PARA" horzRelTo="PARA" vertAlign="TOP" horzAlign="LEFT" vertOffset="0" horzOffset="0"/>
  </hp:equation>
</hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    let eq = section.paragraphs[0]
        .controls
        .iter()
        .find_map(|c| match c {
            Control::Equation(e) => Some(e),
            _ => None,
        })
        .expect("수식 컨트롤");
    assert_eq!(eq.baseline, 85, "baseLine 생략 시 스펙 기본값 85");
    assert_eq!(eq.font_name, "HYhwpEQ", "font 생략 시 스펙 기본값 HYhwpEQ");
    assert_eq!(
        eq.version_info, "Equation Version 60",
        "version 생략 시 스펙 기본값"
    );
}

#[test]
fn task1380_no_linesegarray_keeps_line_segs_empty() {
    // 원본에 <hp:linesegarray> 가 없는 문단은 zero-default 를 주입하지 않고
    // line_segs 를 빈 채 유지한다 (#1380 — 원본 무 → RT 무 대칭의 전제).
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t>텍스트 있음</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert!(
        section.paragraphs[0].line_segs.is_empty(),
        "linesegarray 부재 문단에 zero-default 가 주입되면 안 됨: {:?}",
        section.paragraphs[0].line_segs
    );
}

#[test]
fn task1380_linesegarray_values_loaded_as_is() {
    // <hp:linesegarray> 가 있으면 9개 필드를 그대로 적재한다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t>한 줄</hp:t>
</hp:run>
<hp:linesegarray>
  <hp:lineseg textpos="0" vertpos="15360" vertsize="2197" textheight="2197" baseline="1867" spacing="1098" horzpos="0" horzsize="42520" flags="393216"/>
</hp:linesegarray>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let segs = &section.paragraphs[0].line_segs;
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].vertical_pos, 15360);
    assert_eq!(segs[0].line_height, 2197);
    assert_eq!(segs[0].tag, 393216);
}

#[test]
fn test_parse_text_preserves_xml_general_refs() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t>&lt; A &amp; B &gt; &quot;q&quot; &apos;s&apos; &#x25B3;</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert_eq!(section.paragraphs.len(), 1);
    assert_eq!(section.paragraphs[0].text, "< A & B > \"q\" 's' △");
}

#[test]
fn run_text_preserve_cdata() {
    // <hp:t> 본문 런 텍스트가 CDATA 로 저장된 경우, read_text_content_with_tabs 에
    // Event::CData arm 이 없어 `_ => {}` 로 버려지면서 문단 텍스트가 통째로 소실되던
    // 결함. #2916·#2951·#2974 와 같은 결함 클래스이나, 이 경로는 수식·덧말이 아닌
    // 일반 본문이라 영향 범위가 가장 넓다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t><![CDATA[a<b & c]]></hp:t>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert_eq!(section.paragraphs.len(), 1);
    assert_eq!(
        section.paragraphs[0].text, "a<b & c",
        "본문 런 텍스트의 CDATA 가 소실되면 안 됨"
    );
}

#[test]
fn form_edit_text_preserve_cdata() {
    // 양식 개체(<hp:edit>)의 <hp:text> 가 CDATA 로 저장된 경우 parse_form_object 의
    // arm 누락으로 form.text 가 비던 결함. 위와 같은 결함 클래스.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:edit id="1" name="edit1">
    <hp:text><![CDATA[a<b]]></hp:text>
  </hp:edit>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Form(form) = &section.paragraphs[0].controls[0] else {
        panic!("첫 컨트롤은 Form(양식 개체)이어야 함");
    };
    assert_eq!(
        form.text, "a<b",
        "양식 개체 텍스트의 CDATA 가 소실되면 안 됨"
    );
}

#[test]
fn test_parse_endnote_long_note_line_keeps_hwp5_low_word() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" outlineShapeIDRef="1" masterPageCnt="1">
    <hp:pagePr landscape="WIDELY" width="77102" height="111685" gutterType="LEFT_RIGHT">
      <hp:margin header="4960" footer="3401" gutter="0" left="5300" right="5300" top="6236" bottom="5952"/>
    </hp:pagePr>
    <hp:endNotePr>
      <hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar=")" supscript="0"/>
      <hp:noteLine length="14692344" type="SOLID" width="0.12 mm" color="#000000"/>
      <hp:noteSpacing betweenNotes="0" belowLine="567" aboveLine="850"/>
      <hp:numbering type="CONTINUOUS" newNum="1"/>
      <hp:placement place="END_OF_DOCUMENT" beneathText="0"/>
    </hp:endNotePr>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();

    assert_eq!(section.section_def.endnote_shape.separator_length, 14692344);
    assert_eq!(
        section
            .section_def
            .endnote_shape
            .separator_above_margin_hu(),
        850,
        "aboveLine은 공식 '구분선 위' 값"
    );
    assert_eq!(
        section
            .section_def
            .endnote_shape
            .separator_below_margin_hu(),
        567,
        "belowLine은 공식 '구분선 아래' 값"
    );
    assert_eq!(
        section.section_def.endnote_shape.separator_line_width, 1,
        "HWPX noteLine width도 공통 선 굵기 코드표를 사용해야 함"
    );
    assert_eq!(
        section.section_def.endnote_shape.placement,
        crate::model::footnote::FootnotePlacement::EachColumn
    );
}

#[test]
fn test_parse_endnote_placement_end_of_section() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" outlineShapeIDRef="1" masterPageCnt="0">
    <hp:endNotePr>
      <hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar=")" supscript="0"/>
      <hp:noteLine length="0" type="NONE" width="0.12 mm" color="#000000"/>
      <hp:noteSpacing betweenNotes="0" belowLine="567" aboveLine="850"/>
      <hp:numbering type="CONTINUOUS" newNum="1"/>
      <hp:placement place="END_OF_SECTION" beneathText="0"/>
    </hp:endNotePr>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();

    assert_eq!(
        section.section_def.endnote_shape.placement,
        crate::model::footnote::FootnotePlacement::BelowText
    );
    assert_eq!((section.section_def.endnote_shape.attr >> 8) & 0x03, 1);
    assert_eq!((section.section_def.endnote_shape.attr >> 10) & 0x03, 0);
}

/// [#2779] 각주 placement 의 OWPML 정식 토큰 MERGED_COLUMN(통단)·
/// RIGHT_MOST_COLUMN(가장 오른쪽 단)을 파서가 수용해야 한다. 종전엔 토큰 표에
/// 없어 `_ => continue` 로 떨어져, 통단/오른쪽단 각주가 파싱 단계에서 기본값
/// (각 단마다, 코드 0)으로 소실됐다.
#[test]
fn issue2779_footnote_placement_accepts_schema_column_tokens() {
    // (placement, attr bits 8-9 코드) 를 돌려준다.
    fn parse_place(place: &str) -> (crate::model::footnote::FootnotePlacement, u32) {
        let xml = format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" outlineShapeIDRef="1" masterPageCnt="0">
    <hp:footNotePr>
      <hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar=")" supscript="0"/>
      <hp:noteLine length="-1" type="SOLID" width="0.12 mm" color="#000000"/>
      <hp:noteSpacing betweenNotes="283" belowLine="567" aboveLine="850"/>
      <hp:numbering type="CONTINUOUS" newNum="1"/>
      <hp:placement place="{place}" beneathText="0"/>
    </hp:footNotePr>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"##
        );
        let section = parse_hwpx_section(&xml).unwrap();
        let shape = &section.section_def.footnote_shape;
        (shape.placement, (shape.attr >> 8) & 0x03)
    }

    use crate::model::footnote::FootnotePlacement;
    assert_eq!(
        parse_place("MERGED_COLUMN"),
        (FootnotePlacement::BelowText, 1),
        "MERGED_COLUMN(통단으로 배열) = attr bits 8-9 코드 1"
    );
    assert_eq!(
        parse_place("RIGHT_MOST_COLUMN"),
        (FootnotePlacement::RightColumn, 2),
        "RIGHT_MOST_COLUMN(가장 오른쪽 단에 배열) = attr bits 8-9 코드 2"
    );
    // 기본 토큰은 종전대로 코드 0.
    assert_eq!(
        parse_place("EACH_COLUMN"),
        (FootnotePlacement::EachColumn, 0),
        "EACH_COLUMN(각 단마다 따로 배열) = attr bits 8-9 코드 0"
    );
}

/// [#2779] secPr@memoShapeIDRef 가 SectionDef.memo_shape_id 로 수집돼야 한다.
/// 종전엔 파서가 속성을 읽지 않아 저장 시 템플릿 상수 "0" 으로 리셋됐다.
#[test]
fn issue2779_secpr_memo_shape_id_ref_parsed() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr id="" textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" tabStopVal="4000" tabStopUnit="HWPUNIT" outlineShapeIDRef="1" memoShapeIDRef="2" textVerticalWidthHead="0" masterPageCnt="0">
  </hp:secPr>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert_eq!(section.section_def.memo_shape_id, 2);
}

#[test]
fn test_parse_endnote_numbering_restart_section() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" outlineShapeIDRef="1" masterPageCnt="0">
    <hp:endNotePr>
      <hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar=")" supscript="0"/>
      <hp:noteLine length="0" type="NONE" width="0.12 mm" color="#000000"/>
      <hp:noteSpacing betweenNotes="0" belowLine="567" aboveLine="850"/>
      <hp:numbering type="ON_SECTION" newNum="5"/>
    </hp:endNotePr>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();

    assert_eq!(
        section.section_def.endnote_shape.numbering,
        crate::model::footnote::FootnoteNumbering::RestartSection
    );
    assert_eq!(section.section_def.endnote_shape.start_number, 5);
    assert_eq!((section.section_def.endnote_shape.attr >> 8) & 0x03, 0);
    assert_eq!((section.section_def.endnote_shape.attr >> 10) & 0x03, 1);
}

#[test]
fn test_parse_endnote_shape_attr_table134_flags() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" outlineShapeIDRef="1" masterPageCnt="0">
    <hp:endNotePr>
      <hp:autoNumFormat type="USER_CHAR" userChar="*" prefixChar="[" suffixChar="]" supscript="1"/>
      <hp:noteLine length="0" type="NONE" width="0.12 mm" color="#000000"/>
      <hp:noteSpacing betweenNotes="0" belowLine="567" aboveLine="850"/>
      <hp:numbering type="ON_PAGE" newNum="1"/>
      <hp:placement place="END_OF_SECTION" beneathText="1"/>
    </hp:endNotePr>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let shape = &section.section_def.endnote_shape;

    assert_eq!(
        shape.number_format,
        crate::model::footnote::NumberFormat::UserChar
    );
    assert_eq!(shape.user_char, '*');
    assert!(shape.number_code_superscript);
    assert!(shape.print_inline_after_text);
    assert_eq!((shape.attr & 0xff), 0x81);
    assert_eq!((shape.attr >> 8) & 0x03, 1);
    assert_eq!((shape.attr >> 10) & 0x03, 2);
    assert_ne!(shape.attr & (1 << 12), 0);
    assert_ne!(shape.attr & (1 << 13), 0);
}

/// [#1199] HWPX 미주/각주 ctrl 의 prefixChar(코드포인트 숫자) 가
/// before_decoration_letter 로 매핑되어야 한다. 누락 시 마커 접두문자('문')가 탈락.
#[test]
fn test_parse_note_prefix_char_maps_to_before_decoration_letter() {
    // prefixChar="47928"(0xBB38 '문'), suffixChar="65289"(0xFF09 '）')
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ctrl>
    <hp:endNote number="1" prefixChar="47928" suffixChar="65289" instId="100">
      <hp:subList>
        <hp:p paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>note body</hp:t></hp:run></hp:p>
      </hp:subList>
    </hp:endNote>
  </hp:ctrl>
  <hp:ctrl>
    <hp:footNote number="1" prefixChar="47928" suffixChar="65289" instId="200">
      <hp:subList>
        <hp:p paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>note body</hp:t></hp:run></hp:p>
      </hp:subList>
    </hp:footNote>
  </hp:ctrl>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let controls: Vec<&Control> = section
        .paragraphs
        .iter()
        .flat_map(|p| p.controls.iter())
        .collect();

    let endnote = controls
        .iter()
        .find_map(|c| match c {
            Control::Endnote(n) => Some(n),
            _ => None,
        })
        .expect("endnote ctrl");
    assert_eq!(
        endnote.before_decoration_letter, 47928,
        "endnote prefixChar"
    );
    assert_eq!(endnote.after_decoration_letter, 65289, "endnote suffixChar");

    let footnote = controls
        .iter()
        .find_map(|c| match c {
            Control::Footnote(n) => Some(n),
            _ => None,
        })
        .expect("footnote ctrl");
    assert_eq!(
        footnote.before_decoration_letter, 47928,
        "footnote prefixChar"
    );
    assert_eq!(
        footnote.after_decoration_letter, 65289,
        "footnote suffixChar"
    );
}

/// [#1199] prefixChar 속성이 없으면 before_decoration_letter 는 0(접두 없음) 유지 — 회귀 방지.
#[test]
fn test_parse_note_without_prefix_char_keeps_zero_before_letter() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ctrl>
    <hp:endNote number="1" suffixChar="41" instId="100">
      <hp:subList>
        <hp:p paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>x</hp:t></hp:run></hp:p>
      </hp:subList>
    </hp:endNote>
  </hp:ctrl>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let endnote = section
        .paragraphs
        .iter()
        .flat_map(|p| p.controls.iter())
        .find_map(|c| match c {
            Control::Endnote(n) => Some(n),
            _ => None,
        })
        .expect("endnote ctrl");
    assert_eq!(endnote.before_decoration_letter, 0);
    assert_eq!(endnote.after_decoration_letter, 41); // ')'
}

/// [#1200] curve 도형의 geometry 가 `<hp:seg x1 y1 x2 y2>` (점-대-점 chain)
/// 으로 인코딩된 경우 CurveShape.points 가 채워져야 한다. 누락 시 외곽선 미렌더.
#[test]
fn test_parse_curve_seg_populates_points() {
    // seg chain: (10,10)->(90,10)->(90,90)->(10,10) (폐곡선)
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:curve id="0" zOrder="0" numberingType="NONE" textWrap="TOP_AND_BOTTOM" textFlow="BOTH_SIDES" lock="0" href="" groupLevel="0" instid="1">
    <hp:offset x="0" y="0"/>
    <hp:orgSz width="100" height="100"/>
    <hp:curSz width="100" height="100"/>
    <hp:lineShape color="#000000" width="113" style="SOLID"/>
    <hp:seg type="LINE" x1="10" y1="10" x2="90" y2="10"/>
    <hp:seg type="LINE" x1="90" y1="10" x2="90" y2="90"/>
    <hp:seg type="LINE" x1="90" y1="90" x2="10" y2="10"/>
  </hp:curve>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let curve = section
        .paragraphs
        .iter()
        .flat_map(|p| p.controls.iter())
        .find_map(|c| match c {
            Control::Shape(s) => match s.as_ref() {
                crate::model::shape::ShapeObject::Curve(cv) => Some(cv),
                _ => None,
            },
            _ => None,
        })
        .expect("curve shape");

    // 첫 seg 시작점 + 각 seg 끝점 = 4점 chain
    let pts: Vec<(i32, i32)> = curve.points.iter().map(|p| (p.x, p.y)).collect();
    assert_eq!(pts, vec![(10, 10), (90, 10), (90, 90), (10, 10)]);
}

#[test]
fn test_parse_page_pr_gutter_type_materializes_hwp5_binding_attr() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL">
    <hp:pagePr landscape="WIDELY" width="77102" height="111685" gutterType="LEFT_RIGHT">
      <hp:margin header="4960" footer="3401" gutter="0" left="5300" right="5300" top="6236" bottom="5952"/>
    </hp:pagePr>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();

    assert_eq!(
        section.section_def.page_def.binding,
        BindingMethod::DuplexSided
    );
    assert_eq!(section.section_def.page_def.attr & (0x03 << 1), 0x02);
}

#[test]
fn test_parse_page_border_fill_basis_from_text_border() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL">
    <hp:pageBorderFill type="BOTH" borderFillIDRef="1" textBorder="CONTENT" fillArea="PAPER">
      <hp:offset left="1417" right="1417" top="1417" bottom="1417"/>
    </hp:pageBorderFill>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert_eq!(section.section_def.page_border_fill.attr & 0x01, 0);
    assert_eq!(
        section.section_def.page_border_fill.basis,
        PageBorderBasis::PaperBased
    );
    assert_eq!(
        section.section_def.page_border_fill.ui_basis,
        PageBorderUiBasis::Paper
    );

    let xml = xml.replace(r#"textBorder="CONTENT""#, r#"textBorder="PAPER""#);
    let section = parse_hwpx_section(&xml).unwrap();
    assert_eq!(section.section_def.page_border_fill.attr & 0x01, 0x01);
    assert_eq!(
        section.section_def.page_border_fill.basis,
        PageBorderBasis::BodyBased
    );
    assert_eq!(
        section.section_def.page_border_fill.ui_basis,
        PageBorderUiBasis::Page
    );
}

#[test]
fn test_parse_page_border_fill_slot_by_type_not_by_order() {
    // #2885: type(BOTH/EVEN/ODD) 이 등장 순서와 다르게 기록된 경우에도
    // borderFillIDRef 가 type 값에 맞는 슬롯으로 들어가야 한다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL">
    <hp:pageBorderFill type="EVEN" borderFillIDRef="7" textBorder="CONTENT" fillArea="PAPER">
      <hp:offset left="0" right="0" top="0" bottom="0"/>
    </hp:pageBorderFill>
    <hp:pageBorderFill type="BOTH" borderFillIDRef="9" textBorder="CONTENT" fillArea="PAPER">
      <hp:offset left="0" right="0" top="0" bottom="0"/>
    </hp:pageBorderFill>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    assert_eq!(section.section_def.page_border_fill.border_fill_id, 9);
}

#[test]
fn test_parse_section_grid_preserves_line_and_char_grid() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL">
    <hp:grid lineGrid="1200" charGrid="900" wonggojiFormat="0"/>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();

    assert_eq!(section.section_def.line_grid, 1200);
    assert_eq!(section.section_def.char_grid, 900);
}

#[test]
fn test_parse_section_col_pr_break_type_without_page_break() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL">
    <hp:colPr type="NEWSPAPER" layout="LEFT" colCount="2" sameSz="1" sameGap="1134"/>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];
    assert_eq!(para.raw_break_type, 0x03);
    assert_eq!(
        para.column_type,
        crate::model::paragraph::ColumnBreakType::Section
    );
}

#[test]
fn test_parse_section_col_pr_break_type_with_page_break() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0" pageBreak="1" columnBreak="0">
<hp:run charPrIDRef="0">
  <hp:secPr textDirection="HORIZONTAL">
    <hp:colPr type="NEWSPAPER" layout="LEFT" colCount="2" sameSz="1" sameGap="1134"/>
  </hp:secPr>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];
    assert_eq!(para.raw_break_type, 0x07);
    assert_eq!(
        para.column_type,
        crate::model::paragraph::ColumnBreakType::Page
    );
}

#[test]
fn test_parse_linebreak_preserves_offsets() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t>줄바꿈A<hp:lineBreak/>줄바꿈B</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];
    assert_eq!(para.text, "줄바꿈A\n줄바꿈B");
    assert_eq!(para.char_offsets, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_parse_hwpx_tab_extension_uses_hwp5_inline_format() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t>A<hp:tab width="17283" leader="3" type="2"/>(페이지 표기)</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];
    assert_eq!(para.text, "A\t(페이지 표기)");
    assert_eq!(para.tab_extended, vec![[17283, 0, 0x0203, 0, 0, 0, 9]]);
}

#[test]
fn test_parse_hwpx_tab_width_zero_marker_not_recorded_as_ext() {
    // #4403: 직렬화기가 "데이터 없음" 마커(width=0)로 내보낸 암묵적 기본 탭은
    // 재적재 시 tab_extended 항목을 만들면 안 된다 — 만들면 렌더러가 그 폭을
    // 실제 계산값으로 신뢰해(`total + width`) 문단의 진짜 TabDef(예: 우측 정렬)를
    // 무시하고 커서 위치와 무관한 고정 거리만 전진시킨다. width=0 은 실제 탭에서
    // 나올 수 없는 값(폭 0인 탭은 시각 효과가 없음)이라 안전한 마커다. 탭 문자(\t)
    // 자체는 그대로 보존해야 한다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:t>I.소설의 이해<hp:tab width="0" leader="0" type="1"/>3</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];
    assert_eq!(para.text, "I.소설의 이해\t3");
    assert!(
        para.tab_extended.is_empty(),
        "width=0 마커는 tab_extended 에 실리면 안 됨: {:?}",
        para.tab_extended
    );
}

#[test]
fn test_parse_control_keeps_interleaved_offsets() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0"><hp:t>A</hp:t></hp:run>
<hp:tbl rowCnt="1" colCnt="1" cellSpacing="0" borderFillIDRef="0">
  <hp:inMargin left="0" right="0" top="0" bottom="0"/>
  <hp:tr>
    <hp:tc name="0" header="0" hasMargin="0" editable="0" dirty="0" borderFillIDRef="0" textDirection="HORIZONTAL" vertAlign="TOP" colAddr="0" rowAddr="0" colSpan="1" rowSpan="1" width="1000" height="1000">
      <hp:cellAddr colAddr="0" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:cellSz width="1000" height="1000"/>
      <hp:cellMargin left="0" right="0" top="0" bottom="0"/>
      <hp:subList><hp:p paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>T</hp:t></hp:run></hp:p></hp:subList>
      <hp:lineBreak/>
    </hp:tc>
  </hp:tr>
</hp:tbl>
<hp:run charPrIDRef="0"><hp:t>B</hp:t></hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];
    assert_eq!(para.text, "AB");
    assert_eq!(para.char_offsets, vec![0, 9]);
    // [Task #1058] 같은 char_shape_id 연속 dedup — HWP PARA_CHAR_SHAPE 는 첫 entry 1개만 유지.
    // 두 run 모두 charPrIDRef="0" 이므로 dedup 후 char_shapes.len() = 1.
    assert_eq!(para.char_shapes[0].start_pos, 0);
    assert_eq!(para.char_shapes.len(), 1);
    assert_eq!(para.controls.len(), 1);
}

#[test]
fn test_parse_table_cell_has_margin() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:tbl rowCnt="1" colCnt="1" cellSpacing="0" borderFillIDRef="0">
  <hp:inMargin left="0" right="0" top="0" bottom="0"/>
  <hp:tr>
    <hp:tc name="" header="0" hasMargin="1" borderFillIDRef="0">
      <hp:subList><hp:p paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>T</hp:t></hp:run></hp:p></hp:subList>
      <hp:cellAddr colAddr="0" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:cellSz width="1000" height="1000"/>
      <hp:cellMargin left="141" right="141" top="113" bottom="113"/>
    </hp:tc>
  </hp:tr>
</hp:tbl>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let table = match &section.paragraphs[0].controls[0] {
        crate::model::control::Control::Table(table) => table,
        other => panic!("expected table, got {:?}", other),
    };
    assert!(table.cells[0].apply_inner_margin);
    assert_eq!(table.cells[0].padding.left, 141);
    assert_eq!(table.cells[0].padding.top, 113);
}

#[test]
fn test_parse_table_row_sizes_is_cell_count_not_height() {
    // [#row_sizes 계약] HWP 스펙 UINT16[NRows]("행별 셀 수")과 동일해야 한다.
    // model::table::Table::rebuild_row_sizes, parser::control(HWP5),
    // html_table_import 모두 이 필드를 "행별 셀 개수"로 채우므로 HWPX 파서만
    // 높이를 채우면 계약이 깨진다. 2행 2열에서 각 셀 높이를 다르게 주어
    // 카운트(2)와 높이(예: 500/3000)를 구분한다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:tbl rowCnt="2" colCnt="2" cellSpacing="0" borderFillIDRef="0">
  <hp:inMargin left="0" right="0" top="0" bottom="0"/>
  <hp:tr>
    <hp:tc borderFillIDRef="0"><hp:cellAddr colAddr="0" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="1000" height="500"/></hp:tc>
    <hp:tc borderFillIDRef="0"><hp:cellAddr colAddr="1" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="1000" height="500"/></hp:tc>
  </hp:tr>
  <hp:tr>
    <hp:tc borderFillIDRef="0"><hp:cellAddr colAddr="0" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="1000" height="3000"/></hp:tc>
    <hp:tc borderFillIDRef="0"><hp:cellAddr colAddr="1" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="1000" height="3000"/></hp:tc>
  </hp:tr>
</hp:tbl>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let table = match &section.paragraphs[0].controls[0] {
        crate::model::control::Control::Table(table) => table,
        other => panic!("expected table, got {:?}", other),
    };
    assert_eq!(table.row_sizes, vec![2, 2]);
}

#[test]
fn test_parse_table_page_break_table_vs_cell_mapping() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:tbl rowCnt="1" colCnt="1" pageBreak="TABLE" repeatHeader="1" cellSpacing="0" borderFillIDRef="0">
  <hp:tr><hp:tc borderFillIDRef="0"><hp:cellAddr colAddr="0" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="1000" height="1000"/></hp:tc></hp:tr>
</hp:tbl>
<hp:tbl rowCnt="1" colCnt="1" pageBreak="CELL" repeatHeader="1" cellSpacing="0" borderFillIDRef="0">
  <hp:tr><hp:tc borderFillIDRef="0"><hp:cellAddr colAddr="0" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="1000" height="1000"/></hp:tc></hp:tr>
</hp:tbl>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let tables: Vec<_> = section.paragraphs[0]
        .controls
        .iter()
        .filter_map(|control| match control {
            crate::model::control::Control::Table(table) => Some(table),
            _ => None,
        })
        .collect();

    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].page_break, TablePageBreak::CellBreak);
    assert_eq!(tables[1].page_break, TablePageBreak::RowBreak);
}

#[test]
fn test_parse_hwpx_table_materializes_hwp_common_attrs() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:tbl numberingType="TABLE" textWrap="TOP_AND_BOTTOM" pageBreak="CELL"
        repeatHeader="1" rowCnt="1" colCnt="1" cellSpacing="0" borderFillIDRef="0"
        noAdjust="1">
  <hp:sz width="30613" widthRelTo="ABSOLUTE" height="8580" heightRelTo="ABSOLUTE"/>
  <hp:pos treatAsChar="1" flowWithText="1" allowOverlap="0"
          vertRelTo="PARA" horzRelTo="COLUMN" vertAlign="TOP" horzAlign="LEFT"
          vertOffset="4294965296" horzOffset="0"/>
  <hp:outMargin left="141" right="141" top="141" bottom="141"/>
  <hp:inMargin left="0" right="0" top="283" bottom="283"/>
  <hp:tr>
    <hp:tc borderFillIDRef="0">
      <hp:cellAddr colAddr="0" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:cellSz width="30613" height="8580"/>
    </hp:tc>
  </hp:tr>
</hp:tbl>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let table = match &section.paragraphs[0].controls[0] {
        crate::model::control::Control::Table(table) => table,
        other => panic!("expected table, got {:?}", other),
    };

    assert!(table.common.treat_as_char);
    assert_eq!(table.common.text_wrap, TextWrap::TopAndBottom);
    assert_eq!(table.common.vertical_offset as i32, -2000);
    assert_eq!(table.common.attr, 0x082a_2211);
    assert_eq!(table.attr, 0x01);
    assert_eq!(table.raw_table_record_attr, 0x0400_000e);
}

#[test]
fn table_textwrap_tight_and_through_survive_roundtrip() {
    // 표 textWrap="TIGHT"/"THROUGH" 가 파서 arm 누락으로 SQUARE 로 유실되던 결함.
    // 방출측 text_wrap_str 은 이 두 값을 내므로 왕복 보존돼야 한다.
    for (s, expect) in [("TIGHT", TextWrap::Tight), ("THROUGH", TextWrap::Through)] {
        let xml = format!(
            r#"<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"><hp:p paraPrIDRef="0" styleIDRef="0"><hp:tbl numberingType="TABLE" textWrap="{s}" pageBreak="CELL" repeatHeader="0" rowCnt="1" colCnt="1" cellSpacing="0" borderFillIDRef="0" noAdjust="0"><hp:sz width="1000" widthRelTo="ABSOLUTE" height="1000" heightRelTo="ABSOLUTE"/><hp:pos treatAsChar="0" flowWithText="1" allowOverlap="0" vertRelTo="PARA" horzRelTo="COLUMN" vertAlign="TOP" horzAlign="LEFT" vertOffset="0" horzOffset="0"/><hp:outMargin left="0" right="0" top="0" bottom="0"/><hp:inMargin left="0" right="0" top="0" bottom="0"/><hp:tr><hp:tc borderFillIDRef="0"><hp:cellAddr colAddr="0" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="1000" height="1000"/></hp:tc></hp:tr></hp:tbl></hp:p></hs:sec>"#
        );
        let section = parse_hwpx_section(&xml).unwrap();
        let table = match &section.paragraphs[0].controls[0] {
            crate::model::control::Control::Table(t) => t,
            other => panic!("expected table, got {other:?}"),
        };
        assert_eq!(
            table.common.text_wrap, expect,
            "textWrap={s} 가 {expect:?} 로 파싱돼야 함(SQUARE 유실 방지)"
        );
    }
}

#[test]
fn picture_pattern_8_8_effect_is_preserved() {
    // 방출측이 내는 PATTERN_8_8 효과가 기본값 RealPic 으로 되돌아가지 않아야 한다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:pic id="1" zOrder="0" textWrap="SQUARE" textFlow="BOTH_SIDES">
    <hp:img binaryItemIDRef="image1" effect="PATTERN_8_8"/>
  </hp:pic>
</hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    let Control::Picture(picture) = &section.paragraphs[0].controls[0] else {
        panic!("첫 컨트롤은 그림이어야 함");
    };
    assert_eq!(
        picture.image_attr.effect,
        crate::model::image::ImageEffect::Pattern8x8,
        "PATTERN_8_8 그림 효과가 RealPic 으로 유실되면 안 됨"
    );
}

#[test]
fn dutmal_maintext_subtext_preserve_cdata() {
    // hp:dutmal(덧말)의 mainText/subText가 CDATA로 인코딩된 경우
    // (예: 비교연산자 `<`/`>` 포함) 파서 arm 누락으로 소실되던 결함.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:dutmal posType="TOP" align="CENTER" szRatio="50" option="0" styleIDRef="0">
    <hp:mainText><![CDATA[a<b]]></hp:mainText>
    <hp:subText><![CDATA[c>d]]></hp:subText>
  </hp:dutmal>
</hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    let Control::Ruby(ruby) = &section.paragraphs[0].controls[0] else {
        panic!("첫 컨트롤은 Ruby(덧말)여야 함");
    };
    assert_eq!(ruby.main_text, "a<b", "mainText CDATA 가 소실되면 안 됨");
    assert_eq!(ruby.ruby_text, "c>d", "subText CDATA 가 소실되면 안 됨");
}

#[test]
fn test_parse_hwpx_masterpage_line_materializes_shape_common_attr() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<masterPage xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core"
        id="masterpage0" type="BOTH" pageNumber="0">
  <hp:subList textWidth="66502" textHeight="91136">
<hp:p paraPrIDRef="0" styleIDRef="0">
  <hp:run charPrIDRef="0">
    <hp:line id="1" zOrder="0" textWrap="BEHIND_TEXT" instid="2">
      <hp:offset x="0" y="0"/>
      <hp:orgSz width="100" height="100"/>
      <hp:curSz width="1" height="92409"/>
      <hp:rotationInfo angle="0" centerX="0" centerY="46204" rotateimage="1"/>
      <hp:lineShape color="#000000" width="113" style="SOLID"
                    endCap="FLAT" headfill="1" tailfill="1"
                    headSz="MEDIUM_MEDIUM" tailSz="MEDIUM_MEDIUM"
                    outlineStyle="NORMAL"/>
      <hp:sz width="1" widthRelTo="ABSOLUTE" height="92409" heightRelTo="ABSOLUTE"/>
      <hp:pos treatAsChar="0" flowWithText="0" allowOverlap="1"
              vertRelTo="PAPER" horzRelTo="PARA" vertAlign="TOP" horzAlign="CENTER"
              vertOffset="9912" horzOffset="0"/>
      <hc:startPt x="0" y="0"/>
      <hc:endPt x="100" y="100"/>
    </hp:line>
  </hp:run>
</hp:p>
  </hp:subList>
</masterPage>"##;

    let master_page = parse_hwpx_master_page(xml).unwrap();
    assert_eq!(master_page.hwpx_page_number, Some(0));
    let line = match &master_page.paragraphs[0].controls[0] {
        crate::model::control::Control::Shape(shape) => match shape.as_ref() {
            ShapeObject::Line(line) => line,
            other => panic!("expected line shape, got {:?}", other),
        },
        other => panic!("expected shape control, got {:?}", other),
    };

    assert_eq!(line.common.attr, 0x044a_4700);
    assert_eq!(line.common.text_wrap, TextWrap::BehindText);
    assert_eq!(line.common.width_criterion, SizeCriterion::Absolute);
    assert_eq!(line.common.height_criterion, SizeCriterion::Absolute);
    assert_eq!(line.drawing.border_line.color, 0x000000);
    assert_eq!(line.drawing.border_line.width, 113);
    assert_eq!(line.drawing.border_line.attr, 0xd100_0041);
    assert_eq!(line.drawing.border_line.outline_style, 0);
    assert_eq!(line.start.x, 0);
    assert_eq!(line.start.y, 0);
    assert_eq!(line.end.x, 100);
    assert_eq!(line.end.y, 100);
}

/// #17958715: `hp:lineShape@headStyle`/`tailStyle` 는 지금까지 파서에서 아예
/// 읽히지 않아, 실제 문서에 `headStyle="SPEAR"` 가 있어도 화살표가 항상
/// `ArrowStyle::None` 으로 렌더링됐다(별지 제30호양식의 8.6cm/5.4cm 치수선
/// 화살촉 미표시 버그). bit 10~15(head)/16~21(tail) 에 반영돼야 한다.
#[test]
fn bugfind_17958715_line_shape_head_tail_style_parsed() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:line id="1" zOrder="0" textWrap="IN_FRONT_OF_TEXT" instid="1">
    <hp:offset x="0" y="0"/>
    <hp:orgSz width="100" height="100"/>
    <hp:curSz width="0" height="12334"/>
    <hp:lineShape color="#000000" width="33" style="SOLID" endCap="FLAT"
                  headStyle="SPEAR" tailStyle="NORMAL"
                  headfill="1" tailfill="0"
                  headSz="SMALL_SMALL" tailSz="MEDIUM_MEDIUM" outlineStyle="NORMAL"/>
    <hc:startPt x="0" y="0"/>
    <hc:endPt x="100" y="100"/>
  </hp:line>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Line(line) = shape.as_ref() else {
        panic!("expected line shape");
    };

    let attr = line.drawing.border_line.attr;
    // head(SPEAR=2) → bit 10~15
    assert_eq!(
        (attr >> 10) & 0x3F,
        2,
        "headStyle=SPEAR 가 bit 10~15 에 반영돼야 함"
    );
    // tail(NORMAL=0) → bit 16~21
    assert_eq!(
        (attr >> 16) & 0x3F,
        0,
        "tailStyle=NORMAL 은 bit 16~21 이 0 이어야 함"
    );
    // headfill=1 → bit 30, tailfill=0 → bit 31 은 꺼져 있어야 함
    assert_ne!(attr & 0x4000_0000, 0, "headfill=1 은 bit 30 이어야 함");
    assert_eq!(
        attr & 0x8000_0000,
        0,
        "tailfill=0 은 bit 31 이 꺼져 있어야 함"
    );
}

#[test]
fn test_parse_field_begin_end_materializes_field_range() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ctrl>
    <hp:fieldBegin type="MEMO" id="2135782115" fieldid="623209829"/>
  </hp:ctrl>
  <hp:t>ABC</hp:t>
  <hp:ctrl>
    <hp:fieldEnd beginIDRef="2135782115" fieldid="623209829"/>
  </hp:ctrl>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];

    assert_eq!(para.text, "ABC");
    assert_eq!(para.char_offsets, vec![8, 9, 10]);
    assert_eq!(para.char_count, 20);
    assert_eq!(para.controls.len(), 1);
    assert_eq!(para.field_ranges.len(), 1);

    let range = &para.field_ranges[0];
    assert_eq!(range.start_char_idx, 0);
    assert_eq!(range.end_char_idx, 3);
    assert_eq!(range.control_idx, 0);
}

#[test]
fn test_rendering_info_materializes_hwp5_raw_rendering_count() {
    let xml = r#"<hp:renderingInfo xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
      <hc:transMatrix e1="1" e2="0" e3="10" e4="0" e5="1" e6="20"/>
      <hc:scaMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/>
      <hc:rotMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/>
      <hc:scaMatrix e1="2" e2="0" e3="0" e4="0" e5="3" e6="0"/>
      <hc:rotMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/>
    </hp:renderingInfo>"#;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut shape_attr = ShapeComponentAttr::default();

    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"renderingInfo" => {
                parse_rendering_info(&mut reader, &mut shape_attr).unwrap();
                break;
            }
            Event::Eof => panic!("renderingInfo not found"),
            _ => {}
        }
        buf.clear();
    }

    fn read_f64(raw: &[u8], offset: usize) -> f64 {
        f64::from_le_bytes(raw[offset..offset + 8].try_into().unwrap())
    }

    assert_eq!(shape_attr.raw_rendering.len(), 2 + 48 + 2 * 96);
    assert_eq!(
        u16::from_le_bytes([shape_attr.raw_rendering[0], shape_attr.raw_rendering[1],]),
        2
    );
    assert_eq!(read_f64(&shape_attr.raw_rendering, 2 + 16), 10.0);
    assert_eq!(read_f64(&shape_attr.raw_rendering, 2 + 40), 20.0);
    assert_eq!(read_f64(&shape_attr.raw_rendering, 2 + 48 + 96), 2.0);
    assert_eq!(read_f64(&shape_attr.raw_rendering, 2 + 48 + 96 + 32), 3.0);
}

#[test]
fn hwpx_storage_flip_defaults_follow_hancom_group_contract() {
    let mut top_level_picture = ShapeComponentAttr {
        rotate_image: true,
        ..Default::default()
    };
    materialize_shape_hwp_storage_defaults(
        &mut CommonObjAttr::default(),
        &mut top_level_picture,
        ShapeStorageKind::Picture,
    );
    assert_eq!(top_level_picture.flip, 0x2008_0000);

    let mut grouped_picture = ShapeComponentAttr {
        group_level: 1,
        rotate_image: true,
        ..Default::default()
    };
    materialize_shape_hwp_storage_defaults(
        &mut CommonObjAttr::default(),
        &mut grouped_picture,
        ShapeStorageKind::Picture,
    );
    assert_eq!(grouped_picture.flip, 0x200b_0000);

    let mut grouped_text_box = ShapeComponentAttr {
        group_level: 1,
        rotate_image: true,
        ..Default::default()
    };
    materialize_shape_hwp_storage_defaults(
        &mut CommonObjAttr::default(),
        &mut grouped_text_box,
        ShapeStorageKind::TextBoxDrawing,
    );
    assert_eq!(grouped_text_box.flip, 0x010b_0000);
}

#[test]
fn test_rendering_info_quantizes_fractional_matrix_values_like_hwp5() {
    let xml = r#"<hp:renderingInfo xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
      <hc:transMatrix e1="1" e2="0" e3="-310" e4="0" e5="1" e6="0"/>
      <hc:scaMatrix e1="0.723629" e2="0" e3="310" e4="0" e5="0.723636" e6="0"/>
      <hc:rotMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/>
    </hp:renderingInfo>"#;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut shape_attr = ShapeComponentAttr::default();

    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"renderingInfo" => {
                parse_rendering_info(&mut reader, &mut shape_attr).unwrap();
                break;
            }
            Event::Eof => panic!("renderingInfo not found"),
            _ => {}
        }
        buf.clear();
    }

    fn read_f64(raw: &[u8], offset: usize) -> f64 {
        f64::from_le_bytes(raw[offset..offset + 8].try_into().unwrap())
    }

    let scale_start = 2 + 48;
    assert_eq!(
        read_f64(&shape_attr.raw_rendering, scale_start),
        f64::from(0.723629f32)
    );
    assert_eq!(
        read_f64(&shape_attr.raw_rendering, scale_start + 32),
        f64::from(0.723636f32)
    );
    assert_eq!(read_f64(&shape_attr.raw_rendering, scale_start + 16), 310.0);
}

#[test]
fn test_parse_memo_field_parameters_preserves_number_as_memo_index() {
    let xml = r#"<hp:parameters xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
  <hp:stringParam name="Command">MEMO/65535/2/1650281184/31247371/user/\;;</hp:stringParam>
  <hp:integerParam name="Number">2</hp:integerParam>
</hp:parameters>"#;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut field = Field {
        field_type: FieldType::Memo,
        ..Default::default()
    };

    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"parameters" => {
                let start = e.to_owned();
                parse_field_parameters(&start, &mut reader, &mut field).unwrap();
                break;
            }
            Event::Eof => panic!("parameters not found"),
            _ => {}
        }
        buf.clear();
    }

    assert_eq!(field.command, "MEMO/65535/2/1650281184/31247371/user/\\;;");
    assert_eq!(field.memo_index, 2);
}

#[test]
fn parse_field_parameters_reassembles_nested_params_balanced() {
    // 중첩 파라미터(listParam 안의 stringParam). 종전엔 open_param 이 마지막 Start 로
    // 덮여 바깥 </hp:listParam> 닫는 태그가 누락돼 raw_parameters_xml 이 불균형이었다.
    let xml = r#"<hp:parameters xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" cnt="1" name=""><hp:listParam cnt="1" name="L"><hp:stringParam name="A">x</hp:stringParam></hp:listParam></hp:parameters>"#;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut field = Field::default();

    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"parameters" => {
                let start = e.to_owned();
                parse_field_parameters(&start, &mut reader, &mut field).unwrap();
                break;
            }
            Event::Eof => panic!("parameters not found"),
            _ => {}
        }
        buf.clear();
    }

    let raw = field.raw_parameters_xml.expect("raw_parameters_xml");
    assert!(raw.contains("</hp:stringParam>"), "inner close: {raw}");
    assert!(
        raw.contains("</hp:listParam>"),
        "바깥 </hp:listParam> 누락(중첩 불균형): {raw}"
    );
    assert!(raw.ends_with("</hp:parameters>"), "params close: {raw}");
}

#[test]
fn test_parse_field_parameters_preserves_cdata_command() {
    let xml = r#"<hp:parameters xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
  <hp:stringParam name="Command"><![CDATA[HYPERLINK "https://example.com/?a=1&b=2"]]></hp:stringParam>
</hp:parameters>"#;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut field = Field::default();

    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"parameters" => {
                let start = e.to_owned();
                parse_field_parameters(&start, &mut reader, &mut field).unwrap();
                break;
            }
            Event::Eof => panic!("parameters not found"),
            _ => {}
        }
        buf.clear();
    }

    assert_eq!(field.command, "HYPERLINK \"https://example.com/?a=1&b=2\"");
}

#[test]
fn test_parse_memo_field_begin_uses_id_as_hwp5_field_id() {
    let xml = r#"<hp:fieldBegin xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" type="MEMO" id="2135782115" fieldid="623209829" />"#;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Empty(ref e) | Event::Start(ref e)
                if local_name(e.name().as_ref()) == b"fieldBegin" =>
            {
                let field = parse_field_begin_attrs(e);
                assert_eq!(field.field_type, FieldType::Memo);
                assert_eq!(field.field_id, 2_135_782_115);
                assert_eq!(field.ctrl_id, tags::FIELD_MEMO);
                break;
            }
            Event::Eof => panic!("fieldBegin not found"),
            _ => {}
        }
        buf.clear();
    }
}

// ---------- #1556: 다단락 필드의 고아 fieldEnd ----------

#[test]
fn task1556_orphan_field_end_recorded_in_end_paragraph() {
    // fieldBegin 은 문단 0, fieldEnd 는 문단 1 (다단락 누름틀 필드).
    // 문단 1 은 컨트롤·field_range 없이 8유닛 슬롯만 갖는다 → orphan_field_ends 로 기록.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0"><hp:ctrl><hp:fieldBegin id="1878228493" type="CLICK_HERE" name="본문" fieldid="627272811"/></hp:ctrl><hp:t>본문시작</hp:t></hp:run>
  </hp:p>
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="3"><hp:t>끝.</hp:t><hp:ctrl><hp:fieldEnd beginIDRef="1878228493" fieldid="627272811"/></hp:ctrl></hp:run>
<hp:run charPrIDRef="30"><hp:t/></hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    // 문단 0: fieldBegin 보존 (Control::Field), 고아 없음.
    let p0 = &section.paragraphs[0];
    assert!(
        matches!(p0.controls.first(), Some(Control::Field(_))),
        "문단 0 은 fieldBegin 컨트롤 보존"
    );
    assert!(p0.orphan_field_ends.is_empty(), "문단 0 고아 없음");

    // 문단 1: 텍스트 "끝." (2자) + 고아 fieldEnd 8유닛.
    let p1 = &section.paragraphs[1];
    assert_eq!(p1.text, "끝.");
    assert_eq!(p1.orphan_field_ends.len(), 1, "고아 fieldEnd 1개 기록");
    let ofe = &p1.orphan_field_ends[0];
    assert_eq!(ofe.char_idx, 2, "텍스트 끝(인덱스 2) 위치");
    assert_eq!(ofe.begin_id_ref, 1_878_228_493);
    assert_eq!(ofe.field_id, 627_272_811);
    // char_count = 텍스트 2 + fieldEnd 8 + 끝마커 1 = 11.
    assert_eq!(
        p1.char_count, 11,
        "고아 fieldEnd 8유닛이 char_count 에 반영"
    );
    // 두 번째 char_shape(run charPrIDRef=30)는 offsets 축 10 (텍스트 2 + 8).
    assert_eq!(
        p1.char_shapes
            .iter()
            .map(|c| (c.start_pos, c.char_shape_id))
            .collect::<Vec<_>>(),
        vec![(0, 3), (10, 30)],
    );
}

#[test]
fn task1556_same_paragraph_field_uses_range_not_orphan() {
    // 동일 문단 내 begin+end 는 종전대로 field_ranges 로만 처리 (고아 0) — 회귀 가드.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0"><hp:ctrl><hp:fieldBegin id="100" type="HYPERLINK" name="" fieldid="100"/></hp:ctrl><hp:t>링크</hp:t><hp:ctrl><hp:fieldEnd beginIDRef="100" fieldid="100"/></hp:ctrl></hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    let p = &section.paragraphs[0];
    assert_eq!(p.field_ranges.len(), 1, "동일 문단 필드는 field_range");
    assert!(p.orphan_field_ends.is_empty(), "고아 기록 없음");
}

/// #1512: 비-Memo 필드도 고유 OWPML `id` 를 field_id 로 써야 한다. 같은 종류 필드가
/// 공유하는 `fieldid` 를 우선하면 모든 필드가 동일 ID 로 반환된다(누름틀 구분 불가).
#[test]
fn task1512_non_memo_field_uses_unique_id() {
    fn parse_one(xml: &str) -> Field {
        let mut reader = Reader::from_str(xml);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Empty(ref e) | Event::Start(ref e)
                    if local_name(e.name().as_ref()) == b"fieldBegin" =>
                {
                    return parse_field_begin_attrs(e);
                }
                Event::Eof => panic!("fieldBegin not found"),
                _ => {}
            }
        }
    }
    // 공유 fieldid(627469685) + 서로 다른 고유 id → field_id 는 고유 id 여야 한다.
    let ns = "http://www.hancom.co.kr/hwpml/2011/paragraph";
    let a = parse_one(&format!(
        r#"<hp:fieldBegin xmlns:hp="{ns}" type="FORMULA" id="1685705574" fieldid="627469685"/>"#
    ));
    let b = parse_one(&format!(
        r#"<hp:fieldBegin xmlns:hp="{ns}" type="FORMULA" id="1685705575" fieldid="627469685"/>"#
    ));
    assert_eq!(a.field_id, 1_685_705_574);
    assert_eq!(b.field_id, 1_685_705_575);
    assert_ne!(
        a.field_id, b.field_id,
        "공유 fieldid 가 아닌 고유 id 로 구분돼야 함"
    );
}

#[test]
fn test_collect_hwpx_section_master_page_refs() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:masterPage idRef="masterpage0"/>
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0"><hp:t>body</hp:t></hp:run>
  </hp:p>
  <masterPage idRef="masterpage1"/>
</hs:sec>"#;

    let refs = collect_hwpx_section_master_page_refs(xml).unwrap();
    assert_eq!(refs, vec!["masterpage0", "masterpage1"]);
}

#[test]
fn test_collect_hwpx_section_master_page_refs_ignores_root_masterpage_without_id_ref() {
    let xml = r#"<masterPage xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        id="masterpage0" type="EVEN">
  <hp:subList textWidth="1000" textHeight="2000"/>
</masterPage>"#;

    let refs = collect_hwpx_section_master_page_refs(xml).unwrap();
    assert!(refs.is_empty());
}

#[test]
fn test_parse_hwpx_master_page_type_accepts_official_and_sample_spellings() {
    assert_eq!(
        parse_hwpx_master_page_type("BOTH"),
        HwpxMasterPageType::Both
    );
    assert_eq!(
        parse_hwpx_master_page_type("Both"),
        HwpxMasterPageType::Both
    );
    assert_eq!(
        parse_hwpx_master_page_type("both"),
        HwpxMasterPageType::Both
    );
    assert_eq!(
        parse_hwpx_master_page_type("EVEN"),
        HwpxMasterPageType::Even
    );
    assert_eq!(
        parse_hwpx_master_page_type("Even"),
        HwpxMasterPageType::Even
    );
    assert_eq!(
        parse_hwpx_master_page_type("even"),
        HwpxMasterPageType::Even
    );
    assert_eq!(parse_hwpx_master_page_type("ODD"), HwpxMasterPageType::Odd);
    assert_eq!(parse_hwpx_master_page_type("Odd"), HwpxMasterPageType::Odd);
    assert_eq!(parse_hwpx_master_page_type("odd"), HwpxMasterPageType::Odd);
    assert_eq!(
        parse_hwpx_master_page_type("LAST_PAGE"),
        HwpxMasterPageType::LastPage
    );
    assert_eq!(
        parse_hwpx_master_page_type("LastPage"),
        HwpxMasterPageType::LastPage
    );
    assert_eq!(
        parse_hwpx_master_page_type("lastPage"),
        HwpxMasterPageType::LastPage
    );
    assert_eq!(
        parse_hwpx_master_page_type("OPTIONAL_PAGE"),
        HwpxMasterPageType::OptionalPage
    );
    assert_eq!(
        parse_hwpx_master_page_type("OptionalPage"),
        HwpxMasterPageType::OptionalPage
    );
    assert_eq!(
        parse_hwpx_master_page_type("optionalPage"),
        HwpxMasterPageType::OptionalPage
    );
}

#[test]
fn test_parse_master_page_mixed_case_type_attrs() {
    fn parse_type(type_value: &str) -> MasterPage {
        let xml = format!(
            r#"<masterPage xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        type="{type_value}" pageNumber="4" pageDuplicate="0">
  <hp:subList textWidth="1000" textHeight="2000" hasTextRef="0" hasNumRef="0"/>
</masterPage>"#
        );
        parse_hwpx_master_page(&xml).unwrap()
    }

    let both = parse_type("Both");
    assert_eq!(both.apply_to, HeaderFooterApply::Both);
    assert!(!both.is_extension);

    let even = parse_type("Even");
    assert_eq!(even.apply_to, HeaderFooterApply::Even);
    assert!(!even.is_extension);

    let odd = parse_type("odd");
    assert_eq!(odd.apply_to, HeaderFooterApply::Odd);
    assert!(!odd.is_extension);

    let last_page = parse_type("LastPage");
    assert_eq!(last_page.apply_to, HeaderFooterApply::Both);
    assert!(last_page.is_extension);
    assert!(last_page.overlap);
    assert!(last_page.replace_base);
    assert_eq!(last_page.ext_flags, 0x0003);

    let optional_page = parse_type("optionalPage");
    assert_eq!(optional_page.apply_to, HeaderFooterApply::Both);
    assert!(optional_page.is_extension);
    assert!(optional_page.overlap);
    assert!(!optional_page.replace_base);
    assert_eq!(optional_page.ext_flags, 0x0007);
}

#[test]
fn test_parse_master_page_last_page_extension() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<masterPage xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        type="LAST_PAGE" pageDuplicate="0">
  <hp:subList textWidth="1000" textHeight="2000" hasTextRef="1" hasNumRef="0">
<hp:p id="0" paraPrIDRef="0" styleIDRef="0">
  <hp:run charPrIDRef="0">
    <hp:t>last page</hp:t>
  </hp:run>
</hp:p>
  </hp:subList>
</masterPage>"#;

    let master_page = parse_hwpx_master_page(xml).unwrap();
    assert_eq!(master_page.apply_to, HeaderFooterApply::Both);
    assert!(master_page.is_extension);
    assert!(master_page.overlap);
    assert!(master_page.replace_base);
    assert_eq!(master_page.ext_flags, 0x0003);
    assert_eq!(master_page.text_width, 1000);
    assert_eq!(master_page.text_height, 2000);
    assert_eq!(master_page.text_ref, 1);
    assert_eq!(master_page.paragraphs.len(), 1);
    assert_eq!(master_page.paragraphs[0].text, "last page");
    assert_eq!(master_page.raw_list_header.len(), 34);
}

#[test]
fn test_parse_master_page_optional_page_extension() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<masterPage xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        type="OPTIONAL_PAGE" pageNumber="4" pageDuplicate="0">
  <hp:subList textWidth="1000" textHeight="2000" hasTextRef="0" hasNumRef="0">
<hp:p id="0" paraPrIDRef="0" styleIDRef="0">
  <hp:run charPrIDRef="0">
    <hp:t>optional page</hp:t>
  </hp:run>
</hp:p>
  </hp:subList>
</masterPage>"#;

    let master_page = parse_hwpx_master_page(xml).unwrap();
    assert_eq!(master_page.apply_to, HeaderFooterApply::Both);
    assert!(master_page.is_extension);
    assert!(master_page.overlap);
    assert!(!master_page.replace_base);
    assert_eq!(master_page.ext_flags, 0x0007);
    assert_eq!(master_page.hwpx_page_number, Some(4));
    assert_eq!(master_page.raw_list_header.len(), 34);
}

#[test]
fn test_parse_hwpx_connect_line_materializes_connector() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:connectLine id="1522096658" zOrder="513" textWrap="IN_FRONT_OF_TEXT" textFlow="BOTH_SIDES" instid="448354835" type="STRAIGHT_ONEWAY">
    <hp:offset x="0" y="0"/>
    <hp:orgSz width="1257" height="1"/>
    <hp:curSz width="1257" height="0"/>
    <hp:pos treatAsChar="0" flowWithText="0" allowOverlap="1" vertRelTo="PAPER" horzRelTo="PAPER" vertOffset="25812" horzOffset="45538"/>
    <hp:lineShape color="#000000" width="141" style="SOLID" headStyle="NORMAL" tailStyle="ARROW" headfill="1" tailfill="1" headSz="MEDIUM_MEDIUM" tailSz="MEDIUM_MEDIUM"/>
    <hp:startPt x="0" y="0" subjectIDRef="11" subjectIdx="2"/>
    <hp:endPt x="1257" y="0" subjectIDRef="22" subjectIdx="3"/>
    <hp:controlPoints>
      <hp:point x="0" y="0" type="3"/>
      <hp:point x="100" y="0" type="26"/>
    </hp:controlPoints>
  </hp:connectLine>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Line(line) = shape.as_ref() else {
        panic!("expected line shape");
    };

    assert_eq!(line.common.instance_id, 1522096658);
    assert_eq!(line.common.horizontal_offset, 45538);
    assert_eq!(line.common.vertical_offset, 25812);
    assert_eq!(line.start.x, 0);
    assert_eq!(line.end.x, 1257);

    let connector = line.connector.as_ref().expect("connector data");
    assert_eq!(connector.link_type, LinkLineType::StraightOneWay);
    assert_eq!(connector.start_subject_id, 11);
    assert_eq!(connector.start_subject_index, 2);
    assert_eq!(connector.end_subject_id, 22);
    assert_eq!(connector.end_subject_index, 3);
    assert_eq!(connector.control_points.len(), 2);
    assert_eq!(connector.control_points[1].x, 100);
    assert_eq!(connector.control_points[1].point_type, 26);
}

#[test]
fn bugfind_shape_offset_negative_x_y_not_dropped_to_zero() {
    // hp:offset(개체 내부 shape-transform 오프셋) x/y 는 음수일 수 있는데,
    // 종전엔 parse_u32 로 읽어 "-500" 같은 문자열이 파싱 실패로 0 이 됐다
    // (hp:pos 의 vertOffset/horzOffset 형제 필드는 이미 parse_i32_wrapping 사용).
    // hp:pos 가 없어 offset 이 common.horizontal_offset/vertical_offset 에도
    // 그대로 폴백되는 경로를 함께 확인한다.
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:connectLine id="1" zOrder="0" textWrap="SQUARE" textFlow="BOTH_SIDES" instid="1" type="STRAIGHT_ONEWAY">
    <hp:offset x="-500" y="-800"/>
    <hp:orgSz width="100" height="1"/>
    <hp:curSz width="100" height="0"/>
    <hp:lineShape color="#000000" width="141" style="SOLID" headStyle="NORMAL" tailStyle="NORMAL" headfill="1" tailfill="1" headSz="MEDIUM_MEDIUM" tailSz="MEDIUM_MEDIUM"/>
    <hp:startPt x="0" y="0" subjectIDRef="1" subjectIdx="0"/>
    <hp:endPt x="100" y="0" subjectIDRef="2" subjectIdx="0"/>
  </hp:connectLine>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Line(line) = shape.as_ref() else {
        panic!("expected line shape");
    };

    assert_eq!(
        line.drawing.shape_attr.offset_x, -500,
        "offset x=-500 이 0으로 뭉개지면 안 됨"
    );
    assert_eq!(
        line.drawing.shape_attr.offset_y, -800,
        "offset y=-800 이 0으로 뭉개지면 안 됨"
    );
    assert_eq!(
        line.common.horizontal_offset as i32, -500,
        "hp:pos 가 없으면 offset 이 common.horizontal_offset 으로 폴백돼야 함"
    );
    assert_eq!(
        line.common.vertical_offset as i32, -800,
        "hp:pos 가 없으면 offset 이 common.vertical_offset 으로 폴백돼야 함"
    );
}

#[test]
fn test_parse_rect_ratio_as_round_rate() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:rect id="1" zOrder="0" ratio="50" numberingType="PICTURE">
    <hp:sz width="100" height="50"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
  </hp:rect>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Rectangle(rect) = shape.as_ref() else {
        panic!("expected rectangle shape");
    };
    assert_eq!(rect.round_rate, 50);
    assert!(
        rect.common.hwp5_gen_shape_attr_bit28,
        "numberingType=PICTURE는 한컴 HWP5 공통 개체 bit 28로 저장돼야 한다"
    );
}

#[test]
fn test_parse_rect_preserves_size_protect() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:rect id="1" zOrder="0" textWrap="SQUARE" textFlow="RIGHT_ONLY">
    <hp:drawText>
      <hp:subList vertAlign="CENTER">
        <hp:p paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>기</hp:t></hp:run></hp:p>
      </hp:subList>
    </hp:drawText>
    <hp:sz width="2600" height="2600" protect="1"/>
    <hp:pos treatAsChar="0" flowWithText="1" allowOverlap="1" holdAnchorAndSO="1" vertRelTo="PARA" horzRelTo="PARA"/>
  </hp:rect>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Rectangle(rect) = shape.as_ref() else {
        panic!("expected rectangle shape");
    };
    assert!(rect.common.size_protect);
    assert!(rect.common.flow_with_text);
    assert!(rect.common.allow_overlap);
    // holdAnchorAndSO="1" → prevent_page_break 이 비표 개체에서도 되읽혀야 한다.
    assert_eq!(rect.common.prevent_page_break, 1);
    assert_eq!(
        rect.common.text_flow,
        crate::model::shape::TextFlow::RightOnly
    );
}

/// [#2726] 공용 자식 파서(`parse_common_shape_children`)는 차트·OLE 를 담당하는데
/// `widthRelTo`/`heightRelTo` arm 이 없어 크기 기준이 **파싱 단계에서** 유실됐다.
/// 표(1702/1706)·도형(2925/2928)·그림(#2712) 파서와 동형으로 보강한다.
#[test]
fn issue2726_parse_chart_preserves_size_criteria() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:chart chartIDRef="Chart/chart1.xml" id="1" zOrder="0" textWrap="SQUARE">
    <hp:sz width="4000" height="3000" widthRelTo="COLUMN" heightRelTo="PAGE" protect="1"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
  </hp:chart>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected OLE(chart) shape");
    };
    assert_eq!(
        ole.common.width_criterion,
        SizeCriterion::Column,
        "widthRelTo=\"COLUMN\" 이 IR 에 적재되어야 한다"
    );
    assert_eq!(
        ole.common.height_criterion,
        SizeCriterion::Page,
        "heightRelTo=\"PAGE\" 가 IR 에 적재되어야 한다"
    );
    assert!(ole.common.size_protect, "protect=\"1\" 은 종전에도 읽혔다");
}

/// [#2726] 높이 기준은 `allow_column_para=false` 로 읽어 치역이
/// `{Paper, Page, Absolute}` 3값이어야 한다. `COLUMN`/`PARA` 가 들어와도 `Absolute`
/// 로 접혀야 직렬화기 `height_criterion_str` 와 정확한 역 관계가 유지된다.
#[test]
fn issue2726_parse_chart_height_folds_column_and_para_to_absolute() {
    for raw in ["COLUMN", "PARA"] {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:chart chartIDRef="Chart/chart1.xml" id="1" zOrder="0" textWrap="SQUARE">
    <hp:sz width="4000" height="3000" widthRelTo="{raw}" heightRelTo="{raw}" protect="0"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
  </hp:chart>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#
        );

        let section = parse_hwpx_section(&xml).unwrap();
        let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
            panic!("expected shape control");
        };
        let ShapeObject::Ole(ole) = shape.as_ref() else {
            panic!("expected OLE(chart) shape");
        };
        // 너비는 5값 전부 허용 → 원문 그대로.
        let expected_width = if raw == "COLUMN" {
            SizeCriterion::Column
        } else {
            SizeCriterion::Para
        };
        assert_eq!(
            ole.common.width_criterion, expected_width,
            "너비는 {raw} 를 그대로 보존해야 한다"
        );
        // 높이는 3값으로 접힘.
        assert_eq!(
            ole.common.height_criterion,
            SizeCriterion::Absolute,
            "높이 {raw} 는 Absolute 로 접혀야 한다"
        );
    }
}

#[test]
fn test_parse_line_preserves_is_reverse_hv() {
    // <hp:line isReverseHV="1"> → LineShape.started_right_or_bottom.
    // 종전엔 파서가 isReverseHV 를 읽지 않아 방향 반전이 유실됐다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:line id="1" zOrder="0" textWrap="SQUARE" textFlow="BOTH_SIDES" isReverseHV="1">
    <hp:sz width="1000" height="0" protect="0"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
    <hp:pt0 x="0" y="0"/>
    <hp:pt1 x="1000" y="0"/>
  </hp:line>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Line(line) = shape.as_ref() else {
        panic!("expected line shape");
    };
    assert!(
        line.started_right_or_bottom,
        "isReverseHV=\"1\" 이 started_right_or_bottom 로 되읽혀야 함"
    );
}

// ---------- #2882: hp:ole/hp:chart numberingType 라운드트립 ----------

#[test]
fn issue2882_ole_numbering_type_picture_is_parsed_into_common_field() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ole id="1" zOrder="0" numberingType="PICTURE" binaryItemIDRef="ole1">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
  </hp:ole>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected ole shape");
    };
    assert_eq!(
        ole.common.numbering_type,
        crate::model::shape::ObjectNumberingType::Picture,
        "numberingType=\"PICTURE\" 가 common.numbering_type 에 매핑돼야 함(직렬화기가 참조하는 필드)"
    );
}

#[test]
fn issue2882_chart_numbering_type_table_is_parsed_into_common_field() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:chart id="1" zOrder="0" numberingType="TABLE" chartIDRef="Chart/chart1.xml">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
  </hp:chart>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected ole shape (chart is modeled as OleShape)");
    };
    assert_eq!(
        ole.common.numbering_type,
        crate::model::shape::ObjectNumberingType::Table,
        "numberingType=\"TABLE\" 가 common.numbering_type 에 매핑돼야 함(직렬화기가 참조하는 필드)"
    );
}

#[test]
fn test_parse_ole_preserves_extent_and_draw_aspect() {
    // <hc:extent> 원본 개체 크기와 drawAspect(표시 방식)가 IR 로 되읽혀야 한다.
    // 종전엔 extent 를 7200 으로 하드코딩하고 drawAspect 를 읽지 않아,
    // 모든 OLE 이 7200x7200 / CONTENT 로 왕복에서 뭉개졌다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ole id="1" zOrder="0" drawAspect="ICON" binaryItemIDRef="ole1">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
    <hc:extent x="12345" y="6789"/>
  </hp:ole>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected ole shape");
    };
    assert_eq!(ole.extent_x, 12345, "hc:extent x 가 보존돼야 함");
    assert_eq!(ole.extent_y, 6789, "hc:extent y 가 보존돼야 함");
    assert_eq!(
        ole.drawing_aspect,
        crate::model::shape::OleDrawingAspect::Icon,
        "drawAspect=ICON 이 보존돼야 함"
    );
}

#[test]
fn bugfind_ole_negative_pos_offset_is_not_zeroed() {
    // [버그] `parse_common_shape_children` (chart/OLE 공용 `<hp:pos>` 파서)는
    // vertOffset/horzOffset 을 `parse_u32` 로 읽는다 — `str::parse::<u32>` 는
    // 부호 문자를 거부해 실패 시 `unwrap_or(0)` 로 조용히 0 이 된다. 반면 이미지/
    // 표 등 다른 개체의 `<hp:pos>` 파서(section.rs:3150-3151, parse_object_layout_child)
    // 는 `parse_i32_wrapping` 을 써서 음수 오프셋(왼쪽/위쪽으로 벗어난 앵커 상대
    // 배치)을 올바르게 보존한다. 우리 자신의 직렬화기(serializer/hwpx/shape.rs)가
    // signed 오프셋을 그대로 십진수로 방출하므로(예: "-100"), 그런 OLE/차트가
    // 저장 후 재로드되면 위치가 0 으로 뭉개진다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ole id="1" zOrder="0" binaryItemIDRef="ole1">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA" vertOffset="-200" horzOffset="-100"/>
  </hp:ole>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected ole shape");
    };
    assert_eq!(
        ole.common.horizontal_offset as i32, -100,
        "hp:pos horzOffset=\"-100\" 이 0 으로 뭉개지면 안 됨"
    );
    assert_eq!(
        ole.common.vertical_offset as i32, -200,
        "hp:pos vertOffset=\"-200\" 이 0 으로 뭉개지면 안 됨"
    );
}

#[test]
fn bugfind_ole_unsigned_wrapped_pos_offset_is_preserved() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ole id="1" zOrder="0" binaryItemIDRef="ole1">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"
            vertOffset="4294965296" horzOffset="4294964867"/>
    <hc:extent x="1" y="1"/>
  </hp:ole>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected ole shape");
    };
    assert_eq!(ole.common.vertical_offset as i32, -2000);
    assert_eq!(ole.common.horizontal_offset as i32, -2429);
}

#[test]
fn bugfind_ole_shape_comment_is_parsed_into_common_description() {
    // 실측: samples/bitmap.hwp 를 export-hwpx --verify 로 왕복하면
    // OLE 개체(그림판 개체)의 "OLE 개체입니다.\r\n개체 형식은 Paintbrush
    // Picture입니다." 설명문(hp:shapeComment)이 IR 차이 1건으로 검출됐다.
    // 방출측(write_shape_comment)은 <hp:shapeComment>를 정상적으로 쓰지만
    // OLE/차트 공용 자식 파서(parse_common_shape_children)에 shapeComment
    // arm 이 없어 되읽지 못하고 유실되었다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ole id="1" zOrder="0" binaryItemIDRef="ole1">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
    <hp:shapeComment>OLE 개체입니다.&#13;&#10;개체 형식은 Paintbrush Picture입니다.</hp:shapeComment>
  </hp:ole>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected ole shape");
    };
    assert_eq!(
        ole.common.description, "OLE 개체입니다.\r\n개체 형식은 Paintbrush Picture입니다.",
        "hp:shapeComment 가 ole.common.description 으로 되읽혀야 함"
    );
}

#[test]
fn chart_shape_comment_is_parsed_into_common_description() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:chart id="1" zOrder="0" chartIDRef="Chart/chart1.xml">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
    <hp:shapeComment>분기별 매출 차트</hp:shapeComment>
  </hp:chart>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).expect("parse chart with shapeComment");
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(chart) = shape.as_ref() else {
        panic!("expected chart modeled as OLE shape");
    };
    assert_eq!(chart.common.description, "분기별 매출 차트");
}

#[test]
fn test_shape_img_brush_preserves_image_ref_and_mode() {
    // [#2563] 도형 <hc:imgBrush> 의 <hc:img> 자식과 12종 mode 매핑.
    // 종전엔 mode 4종만 받아 TOTAL 이 TILE 로 붕괴했고, <hc:img> arm 이 없어
    // binaryItemIDRef/bright/contrast/effect 가 전부 버려졌다. bin_data_id 가
    // 0 이면 직렬화가 <hc:img> 를 못 내므로 이미지 도형이 빈 도형이 된다.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
    xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:rect id="1" zOrder="0" textWrap="SQUARE">
    <hp:sz width="2600" height="2600"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
    <hc:fillBrush>
      <hc:imgBrush mode="TOTAL">
        <hc:img binaryItemIDRef="image3" bright="10" contrast="-5" effect="GRAY_SCALE"/>
      </hc:imgBrush>
    </hc:fillBrush>
  </hp:rect>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Rectangle(rect) = shape.as_ref() else {
        panic!("expected rectangle shape");
    };
    let img = rect
        .drawing
        .fill
        .image
        .as_ref()
        .expect("imgBrush 는 ImageFill 을 남겨야 함");

    assert_eq!(img.bin_data_id, 3, "binaryItemIDRef 가 보존돼야 함");
    assert_eq!(img.brightness, 10, "bright 가 보존돼야 함");
    assert_eq!(img.contrast, -5, "contrast 가 보존돼야 함");
    assert_eq!(img.effect, 1, "effect=GRAY_SCALE 가 보존돼야 함");
    assert_eq!(
        img.fill_mode,
        crate::model::style::ImageFillMode::Total,
        "mode=TOTAL 이 TILE 로 붕괴하면 안 됨"
    );
}

#[test]
fn test_task1124_col_pr_parses_col_line() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ctrl>
    <hp:colPr type="NEWSPAPER" layout="LEFT" colCount="2" sameSz="1" sameGap="850">
      <hp:colLine type="SOLID" width="0.12 mm" color="#000000"/>
    </hp:colPr>
  </hp:ctrl>
  <hp:t>A</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"##;

    let section = parse_hwpx_section(xml).unwrap();
    let para = &section.paragraphs[0];
    assert_eq!(para.text, "A");
    assert_eq!(para.controls.len(), 1);
    let Control::ColumnDef(cd) = &para.controls[0] else {
        panic!("expected ColumnDef control");
    };
    assert_eq!(cd.column_count, 2);
    assert!(cd.same_width);
    assert_eq!(cd.spacing, 850);
    assert_eq!(cd.separator_type, 1);
    assert_eq!(cd.separator_width, 1);
    assert_eq!(cd.separator_color, 0x00000000);
}

#[test]
fn issue4387_col_sz_parses_individual_widths_and_gaps() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ctrl>
    <hp:colPr type="NEWSPAPER" layout="LEFT" colCount="2" sameSz="0" sameGap="0">
      <hp:colSz width="4000" gap="500"/>
      <hp:colSz width="6000" gap="0"/>
    </hp:colPr>
  </hp:ctrl>
  <hp:t>A</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"##;
    let section = parse_hwpx_section(xml).unwrap();
    let Control::ColumnDef(cd) = &section.paragraphs[0].controls[0] else {
        panic!("expected ColumnDef control");
    };
    assert!(!cd.same_width);
    assert_eq!(
        cd.widths,
        vec![4000, 6000],
        "단별 너비가 파싱돼야 함(#4387)"
    );
    assert_eq!(cd.gaps, vec![500, 0], "단별 간격이 파싱돼야 함(#4387)");
    assert!(!cd.proportional_widths, "HWPX colSz 는 절대 HWPUNIT");
}

/// [#4387 후속] `colSz@width` 는 스키마상 `xs:positiveInteger`(상한 없음)인데
/// `ColumnDef.widths` 는 `Vec<i16>`(최대 32767). A3 등 큰 용지·비대칭 다단에서
/// 나올 수 있는 40000(≈141mm) 처럼 i16 범위를 넘는 값을 공용 `parse_i16` 로
/// 파싱하면 `str::parse::<i16>()` 오버플로 에러를 `unwrap_or(0)` 이 삼켜
/// widths=[0, 13000] 처럼 무경고 0-폴백됐다(단이 통째로 사라짐 — 수정 전
/// 코드로 직접 재현·확인). saturating 클램프로 i16::MAX 로 잘리는지
/// 확인한다 — 0 이 되면 안 된다.
#[test]
fn issue4387_col_sz_width_overflow_saturates_not_zeroes() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ctrl>
    <hp:colPr type="NEWSPAPER" layout="LEFT" colCount="2" sameSz="0" sameGap="0">
      <hp:colSz width="40000" gap="500"/>
      <hp:colSz width="13000" gap="-7"/>
    </hp:colPr>
  </hp:ctrl>
  <hp:t>A</hp:t>
</hp:run>
  </hp:p>
</hs:sec>"##;
    let section = parse_hwpx_section(xml).unwrap();
    let Control::ColumnDef(cd) = &section.paragraphs[0].controls[0] else {
        panic!("expected ColumnDef control");
    };
    assert_eq!(
        cd.widths[0],
        i16::MAX,
        "i16 범위를 넘는 width 는 0 이 아니라 i16::MAX 로 saturate 해야 함"
    );
    assert_eq!(cd.widths[1], 13000, "범위 내 값은 그대로 보존돼야 함");
    assert_eq!(
        cd.gaps[1], 0,
        "음수 gap(스키마상 nonNegativeInteger 위반)은 0 으로 클램프돼야 함"
    );
}

#[test]
fn test_task1124_col_line_type_and_width_mapping() {
    assert_eq!(parse_hwpx_line_type("NONE"), 0);
    assert_eq!(parse_hwpx_line_type("SOLID"), 1);
    assert_eq!(parse_hwpx_line_type("DASH"), 2);
    assert_eq!(parse_hwpx_line_type("DOT"), 3);
    assert_eq!(parse_hwpx_line_type("DASH_DOT"), 4);
    assert_eq!(parse_hwpx_line_type("DASH_DOT_DOT"), 5);
    assert_eq!(parse_hwpx_line_type("LONG_DASH"), 6);
    assert_eq!(parse_hwpx_line_type("CIRCLE"), 7);

    assert_eq!(parse_hwpx_line_width("0.1 mm"), 0);
    assert_eq!(parse_hwpx_line_width("0.12 mm"), 1);
    assert_eq!(parse_hwpx_line_width("0.4 mm"), 6);
    assert_eq!(parse_hwpx_line_width("0.7 mm"), 9);
    assert_eq!(parse_hwpx_line_width("5.0 mm"), 15);
}

#[test]
fn test_parse_empty_section() {
    let xml = r#"<?xml version="1.0"?><hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"/>"#;
    let section = parse_hwpx_section(xml).unwrap();
    assert!(section.paragraphs.is_empty());
}

/// #2916: `<hp:equation>`의 `<hp:script>` 본문이 CDATA 섹션으로 인코딩된 경우
/// (실제 한글 저장 결과에서 관찰되는 형태 — 수식 스크립트에 `<`, `>` 등이
/// 다수 포함되어 개별 엔티티 이스케이프 대신 CDATA 로 감싸짐), 파서가
/// Event::CData 를 처리하지 않으면 script 가 빈 문자열로 소실된다.
#[test]
fn task_m100_2916_equation_script_cdata_not_lost() {
    let xml = r##"<hp:equation xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
        id="1" version="Equation Version 60" baseLine="0" textColor="#000000" baseUnit="1000" font="HYhwpEQ"><hp:script><![CDATA[a < b > c]]></hp:script></hp:equation>"##;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let ctrl = loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"equation" => {
                break parse_equation(e, &mut reader).unwrap();
            }
            Event::Eof => panic!("equation not found"),
            _ => {}
        }
        buf.clear();
    };
    let Control::Equation(eq) = ctrl else {
        panic!("expected Equation control");
    };
    assert_eq!(
        eq.script, "a < b > c",
        "CDATA 로 감싸진 수식 스크립트가 소실되면 안 된다"
    );
}

#[test]
fn parse_field_type_accepts_toc() {
    // 직렬화기(hwpx/field.rs)가 방출하는 "TOC" 가 TableOfContents 로 파싱돼야
    // hwpx 왕복에서 차례 필드 타입이 Unknown 으로 유실되지 않는다.
    assert_eq!(parse_field_type("TOC"), FieldType::TableOfContents);
    assert_eq!(
        parse_field_type("TABLE_OF_CONTENTS"),
        FieldType::TableOfContents
    );
}

#[test]
fn compose_text_preserve_cdata() {
    // hp:compose(글자겹치기)의 composeText가 CDATA로 인코딩된 경우
    // (예: 비교연산자 `<`/`>` 포함) read_compose_text의 arm 누락으로 소실되던 결함(#2974).
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:compose circleType="CHAR" charSz="100" composeType="OVERLAP">
    <composeText><![CDATA[a<b]]></composeText>
  </hp:compose>
</hp:run>
  </hp:p>
</hs:sec>"#;
    let section = parse_hwpx_section(xml).unwrap();
    let Control::CharOverlap(co) = &section.paragraphs[0].controls[0] else {
        panic!("첫 컨트롤은 CharOverlap(글자겹치기)이어야 함");
    };
    assert_eq!(
        co.chars,
        vec!['a', '<', 'b'],
        "composeText CDATA 가 소실되면 안 됨"
    );
}

#[test]
fn task2931_chart_lock_attr_roundtrips_into_common() {
    // <hp:chart lock="1" .../> → common.locked 이 true 로 되읽혀야 한다.
    // 종전엔 parse_hp_chart_element 가 lock 속성을 매치하지 않아 항상 기본값(false)
    // 으로 남고, 직렬화 시에도 render_common_shape_xml 이 "0"을 하드코딩했다(#2931).
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:chart id="1" zOrder="0" numberingType="NONE" textWrap="SQUARE" textFlow="BOTH_SIDES" lock="1" chartIDRef="Chart/chart1.xml" instid="1"></hp:chart>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected chart (modeled as OLE) shape");
    };
    assert!(
        ole.common.locked,
        "lock=\"1\" 이 common.locked 에 보존돼야 한다"
    );
}

// ---------- #4319: 차트·OLE 캡션 파싱 ----------

/// [#4319] `<hp:chart>` 내부 `<hp:caption>` — 종전엔 공용 자식 파서
/// (`parse_common_shape_children`, 차트·OLE 전용)에 caption arm 이 없어
/// 캡션 subList 가 파싱 단계에서 완전히 유실됐다(표/도형/묶음/그림은 모두
/// 캡션을 읽지만 차트·OLE 만 빠져 있었다). 캡션 구조는 실 코퍼스 hp:pic
/// 캡션 실측(outMargin 뒤·shapeComment 앞, side/fullSz/width/gap/lastWidth
/// 속성 + subList/p/run/t)과 OWPML AbstractShapeObjectType 스키마
/// (sz→pos→outMargin→caption→shapeComment 순서, hp:chart/hp:ole 모두 이
/// 타입을 상속)를 그대로 따른다.
#[test]
fn issue4319_chart_caption_parses_into_caption_field() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:chart id="1" zOrder="0" numberingType="NONE" textWrap="SQUARE" textFlow="BOTH_SIDES" chartIDRef="Chart/chart1.xml" instid="1">
    <hp:sz width="4000" height="3000" widthRelTo="ABSOLUTE" heightRelTo="ABSOLUTE" protect="0"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
    <hp:outMargin left="0" right="0" top="0" bottom="0"/>
    <hp:caption side="BOTTOM" fullSz="0" width="4000" gap="850" lastWidth="4000">
      <hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="TOP" linkListIDRef="0" linkListNextIDRef="0" textWidth="0" textHeight="0" hasTextRef="0" hasNumRef="0">
        <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
          <hp:run charPrIDRef="0"><hp:t>그림 1. 매출 추이</hp:t></hp:run>
        </hp:p>
      </hp:subList>
    </hp:caption>
  </hp:chart>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected chart (modeled as OLE) shape");
    };
    let caption = ole
        .caption
        .as_ref()
        .expect("<hp:caption> 이 ole.caption 에 적재돼야 한다 (#4319)");
    assert_eq!(caption.paragraphs.len(), 1);
    assert_eq!(caption.paragraphs[0].text, "그림 1. 매출 추이");
    assert!(
        ole.drawing.caption.is_none(),
        "HWP5 파서와 동형 정규화 — drawing.caption 은 비어 있어야 한다 \
         (shape_caption 게이트는 x.caption 만 본다)"
    );
}

/// [#4319] `<hp:ole>` 내부 `<hp:caption>` — chart 와 동일한 결함, 동일한 수정.
#[test]
fn issue4319_ole_caption_parses_into_caption_field() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"
    xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section">
  <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
<hp:run charPrIDRef="0">
  <hp:ole id="1" zOrder="0" numberingType="NONE" textWrap="SQUARE" textFlow="BOTH_SIDES" binaryItemIDRef="ole1" instid="1">
    <hp:sz width="4000" height="3000" widthRelTo="ABSOLUTE" heightRelTo="ABSOLUTE" protect="0"/>
    <hp:pos treatAsChar="0" vertRelTo="PARA" horzRelTo="PARA"/>
    <hp:outMargin left="0" right="0" top="0" bottom="0"/>
    <hp:caption side="BOTTOM" fullSz="0" width="4000" gap="850" lastWidth="4000">
      <hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="TOP" linkListIDRef="0" linkListNextIDRef="0" textWidth="0" textHeight="0" hasTextRef="0" hasNumRef="0">
        <hp:p id="0" paraPrIDRef="0" styleIDRef="0">
          <hp:run charPrIDRef="0"><hp:t>수식 1. 표준편차 계산</hp:t></hp:run>
        </hp:p>
      </hp:subList>
    </hp:caption>
  </hp:ole>
  <hp:t/>
</hp:run>
  </hp:p>
</hs:sec>"#;

    let section = parse_hwpx_section(xml).unwrap();
    let Control::Shape(shape) = &section.paragraphs[0].controls[0] else {
        panic!("expected shape control");
    };
    let ShapeObject::Ole(ole) = shape.as_ref() else {
        panic!("expected OLE shape");
    };
    let caption = ole
        .caption
        .as_ref()
        .expect("<hp:caption> 이 ole.caption 에 적재돼야 한다 (#4319)");
    assert_eq!(caption.paragraphs.len(), 1);
    assert_eq!(caption.paragraphs[0].text, "수식 1. 표준편차 계산");
    assert!(
        ole.drawing.caption.is_none(),
        "HWP5 파서와 동형 정규화 — drawing.caption 은 비어 있어야 한다"
    );
}
