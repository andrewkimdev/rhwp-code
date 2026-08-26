//! paragraph_parsing — table_layout.rs 에서 무변동 이동
use super::*;

pub(crate) fn parse_paragraph(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<(Paragraph, Option<SectionDef>), HwpxError> {
    let mut para = Paragraph::default();
    let mut sec_def: Option<SectionDef> = None;

    // 문단 어트리뷰트
    // [Task #1058 후속] HWPX `<hp:p id>` → HWP PARA_HEADER instance_id (UINT32) 직접 매핑.
    // HWPX 의 id 값 ("0" 또는 "2147483648"=0x80000000) 이 한컴 정답지의 instance_id 패턴과
    // 정확 일치. 누락 시 한컴편집기가 각주 추가 시 본문 다단계 목록 부여 (Task #1058 본질).
    let mut hp_p_id: u32 = 0;
    let mut has_column_break_attr = false;
    let mut has_page_break_attr = false;
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"id" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    hp_p_id = s.parse::<u32>().unwrap_or(0);
                }
            }
            b"paraPrIDRef" => para.para_shape_id = parse_u16(&attr),
            b"styleIDRef" => para.style_id = parse_u8(&attr),
            b"columnBreak" => {
                if parse_u8(&attr) == 1 {
                    has_column_break_attr = true;
                    para.column_type = crate::model::paragraph::ColumnBreakType::Column;
                }
            }
            b"pageBreak" => {
                if parse_u8(&attr) == 1 {
                    has_page_break_attr = true;
                    para.column_type = crate::model::paragraph::ColumnBreakType::Page;
                }
            }
            _ => {}
        }
    }

    // 문단 내용 파싱
    let mut buf = Vec::new();
    let mut text_parts: Vec<String> = Vec::new();
    let mut current_char_shape_id: u32 = 0;
    let mut char_shape_changes: Vec<(u32, u32)> = Vec::new(); // (utf16_pos, char_shape_id)
                                                              // [Task #1556] fieldEnd 의 (beginIDRef, fieldid) 를 출현 순서대로 보관 — text_parts 의
                                                              // `\u{0004}` 와 1:1 대응. 고아 fieldEnd 복원에 사용.
    let mut field_end_attrs: Vec<(u32, u32)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"run" => {
                        // 런 시작: charPrIDRef 읽기
                        for attr in ce.attributes().flatten() {
                            if attr.key.as_ref() == b"charPrIDRef" {
                                current_char_shape_id = parse_u32(&attr);
                            }
                        }
                        // 현재 UTF-16 위치에서 글자모양 변경 기록
                        let utf16_pos = calc_utf16_len_from_parts(&text_parts);
                        char_shape_changes.push((utf16_pos, current_char_shape_id));
                    }
                    b"t" => {
                        // 텍스트 읽기 (탭 확장 데이터 포함)
                        let (text, tab_exts) = read_text_content_with_tabs(reader)?;
                        text_parts.push(text);
                        para.tab_extended.extend(tab_exts);
                    }
                    b"tbl" => {
                        // 표 파싱
                        let table = parse_table(ce, reader)?;
                        // 표 위치에 제어 문자(0x0002) 삽입
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(Control::Table(Box::new(table)));
                    }
                    b"pic" => {
                        // 이미지 파싱
                        let pic = parse_picture(ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(pic);
                    }
                    b"switch" => {
                        // <hp:switch> — OOXML 차트 또는 OLE fallback
                        // 구조: <hp:switch>
                        //         <hp:case hp:required-namespace="...ooxmlchart">
                        //           <hp:chart chartIDRef="Chart/chartN.xml" .../>
                        //         </hp:case>
                        //         <hp:default><hp:ole .../></hp:default>
                        //       </hp:switch>
                        if let Some(ctrl) = parse_switch_chart_or_ole(reader)? {
                            text_parts.push("\u{0002}".to_string());
                            para.controls.push(ctrl);
                        }
                    }
                    b"chart" => {
                        // <hp:chart> 직접 출현 (switch 없이) — 아직 보지 못한 변형. 안전 경로.
                        if let Some(ctrl) = parse_hp_chart_element(ce, reader)? {
                            text_parts.push("\u{0002}".to_string());
                            para.controls.push(ctrl);
                        }
                    }
                    b"ole" => {
                        // <hp:ole> 직접 출현 (switch 없이)
                        if let Some(ctrl) = parse_hp_ole_element(ce, reader)? {
                            text_parts.push("\u{0002}".to_string());
                            para.controls.push(ctrl);
                        }
                    }
                    b"secPr" => {
                        // 문단 내 섹션 정의 파싱
                        let mut sd = SectionDef::default();
                        parse_section_def_start(ce, &mut sd);
                        let col_def_opt = parse_sec_pr_children(reader, &mut sd)?;
                        sec_def = Some(sd.clone());
                        // [Task #901] SectionDef 도 HWP 바이너리에서 8 utf16 inline marker —
                        // line_seg.text_start (file 값) 가 HWP 인코딩 가정. HWPX parser
                        // 가 utf16_pos 동기화하지 않으면 paragraph 0 의 compose_lines 가
                        // 모든 chars 를 line 0 에 packing. \u{0002} 추가로 8 utf16 정합.
                        para.controls.push(Control::SectionDef(Box::new(sd)));
                        text_parts.push("\u{0002}".to_string());
                        // colPr이 있으면 ColumnDef 컨트롤 추가 (초기 단 정의) + 8 utf16.
                        if let Some(cd) = col_def_opt {
                            para.controls.push(Control::ColumnDef(cd));
                            text_parts.push("\u{0002}".to_string());
                        }
                    }
                    b"linesegarray" => {
                        // lineseg 배열 파싱
                        parse_lineseg_array(reader, &mut para)?;
                    }
                    b"rect" | b"ellipse" | b"line" | b"connectLine" | b"arc" | b"polygon"
                    | b"curve" => {
                        // 그리기 객체 파싱
                        let shape = parse_shape_object(local, ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(shape);
                    }
                    b"container" => {
                        // 묶음(그룹) 객체 파싱
                        let group = parse_container(ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(group);
                    }
                    b"ctrl" => {
                        parse_ctrl(
                            ce,
                            reader,
                            &mut para.controls,
                            &mut text_parts,
                            &mut field_end_attrs,
                        )?;
                    }
                    b"compose" => {
                        // 글자겹침 (CharOverlap)
                        let ctrl = parse_compose(ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    b"dutmal" => {
                        // 덧말 (Ruby)
                        let ctrl = parse_dutmal(ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    b"equation" => {
                        // 수식 — 개체(ShapeObject)로 처리
                        let ctrl = parse_equation(ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    b"btn" => {
                        let ctrl = parse_form_object(FormType::PushButton, ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    b"checkBtn" => {
                        let ctrl = parse_form_object(FormType::CheckBox, ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    b"radioBtn" => {
                        let ctrl = parse_form_object(FormType::RadioButton, ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    b"comboBox" => {
                        let ctrl = parse_form_object(FormType::ComboBox, ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    b"edit" => {
                        let ctrl = parse_form_object(FormType::Edit, ce, reader)?;
                        text_parts.push("\u{0002}".to_string());
                        para.controls.push(ctrl);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"run" => {
                        // self-closing 빈 run (예: <hp:run charPrIDRef="42"/>)
                        // 빈 paragraph 의 char_shape 가 누락되어 default(id=0) 로
                        // 처리되면 line height 계산이 어긋나 pagination 차이 발생.
                        for attr in ce.attributes().flatten() {
                            if attr.key.as_ref() == b"charPrIDRef" {
                                current_char_shape_id = parse_u32(&attr);
                            }
                        }
                        let utf16_pos = calc_utf16_len_from_parts(&text_parts);
                        char_shape_changes.push((utf16_pos, current_char_shape_id));
                    }
                    b"lineBreak" | b"softHyphen" => {
                        text_parts.push("\n".to_string());
                    }
                    b"columnBreak" => {
                        text_parts.push("\n".to_string());
                    }
                    b"tab" => {
                        text_parts.push("\t".to_string());
                        // "데이터 없음" 마커(width=0, #4403)는 tab_extended 에 싣지 않는다 —
                        // 렌더러가 TabDef 기준으로 다시 계산하도록 원본처럼 비워 둔다.
                        let ext = parse_tab_extension(ce);
                        if !is_tab_no_data_marker(&ext) {
                            para.tab_extended.push(ext);
                        }
                    }
                    b"lineseg" => {
                        // 단독 lineseg (linesegarray 밖에 나올 경우)
                        para.line_segs.push(parse_lineseg_element(ce));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"p" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("paragraph: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // FIELD_BEGIN/FIELD_END 쌍은 HWP PARA_TEXT에서 각각 8 code unit을 차지한다.
    // HWPX 파싱 결과를 HWP로 다시 저장할 때 FIELD_END를 복원하려면, visible text
    // 범위와 해당 Field 컨트롤 index를 field_ranges에 남겨야 한다.
    let mut field_ranges: Vec<FieldRange> = Vec::new();
    let mut orphan_field_ends: Vec<OrphanFieldEnd> = Vec::new();
    let mut field_stack: Vec<(usize, usize)> = Vec::new();
    let mut control_idx: usize = 0;
    let mut visible_char_idx: usize = 0;
    let mut field_end_idx: usize = 0;

    for part in &text_parts {
        match part.as_str() {
            "\u{0003}" => {
                if matches!(para.controls.get(control_idx), Some(Control::Field(_))) {
                    field_stack.push((visible_char_idx, control_idx));
                }
                control_idx += 1;
            }
            "\u{0004}" => {
                let (begin_id_ref, field_id) = field_end_attrs
                    .get(field_end_idx)
                    .copied()
                    .unwrap_or((0, 0));
                field_end_idx += 1;
                if let Some((start_char_idx, control_idx)) = field_stack.pop() {
                    field_ranges.push(FieldRange {
                        start_char_idx,
                        end_char_idx: visible_char_idx,
                        control_idx,
                        end_field_id: field_id,
                    });
                } else {
                    // [Task #1556] 짝 fieldBegin 이 다른 문단에 있는 다단락 필드의 종료 마커.
                    // 현 문단에 컨트롤·FieldRange 가 없으므로 위치+attrs 를 기록해 직렬화기가 복원.
                    orphan_field_ends.push(OrphanFieldEnd {
                        char_idx: visible_char_idx,
                        begin_id_ref,
                        field_id,
                    });
                }
            }
            "\u{0002}" => {
                control_idx += 1;
            }
            "\u{0012}" => {
                control_idx += 1;
                visible_char_idx += 1;
            }
            _ => {
                visible_char_idx += part.chars().count();
            }
        }
    }
    para.field_ranges = field_ranges;
    para.orphan_field_ends = orphan_field_ends;

    // 텍스트 조립: 제어 문자(\u{0002}, \u{0003}, \u{0004})는 HWP와 동일하게 텍스트에서 제외
    // HWP에서 컨트롤 위치는 char_offsets의 갭으로 표현되므로 원본 순서를 유지해 계산한다.
    let mut visual_text = String::new();
    let mut char_offsets: Vec<u32> = Vec::new();
    let mut utf16_pos: u32 = 0;

    for part in &text_parts {
        match part.as_str() {
            "\u{0002}" | "\u{0003}" | "\u{0004}" => {
                utf16_pos += 8;
            }
            "\u{0012}" => {
                // [Task #1050] AUTO_NUMBER (0x12) — HWP PARA_TEXT 정합:
                //   char_offsets.push(pos) + text.push(' ') (placeholder) + jump 8.
                char_offsets.push(utf16_pos);
                visual_text.push(' ');
                utf16_pos += 8;
            }
            _ => {
                for c in part.chars() {
                    char_offsets.push(utf16_pos);
                    visual_text.push(c);
                    let width = if c == '\t' {
                        8
                    } else if (c as u32) > 0xFFFF {
                        2
                    } else {
                        1
                    };
                    utf16_pos += width;
                }
            }
        }
    }

    para.text = visual_text;
    para.char_offsets = char_offsets;
    para.char_count = utf16_pos + 1; // +1 for 끝 마커
    para.has_para_text = !para.text.is_empty() || !para.controls.is_empty();

    // char_shapes는 원본 문단 순서(text_parts)를 기준으로 계산한 위치를 그대로 사용한다.
    // [Task #1058 후속] 같은 char_shape_id 연속 dedup — HWPX 의 여러 run 이 같은
    // charPrIDRef 일 때 HWP PARA_CHAR_SHAPE 는 첫 entry 1개만 유지하므로 정합.
    let mut deduped_cs: Vec<CharShapeRef> = Vec::new();
    for (pos, id) in char_shape_changes {
        if let Some(last) = deduped_cs.last() {
            if last.char_shape_id == id {
                continue;
            }
        }
        deduped_cs.push(CharShapeRef {
            start_pos: pos,
            char_shape_id: id,
        });
    }
    para.char_shapes = deduped_cs;

    // [Task #1058 후속] column_type/raw_break_type — HWP 정합 (스펙 표 59):
    //   bit 0 (0x01) = 구역 나누기, bit 1 (0x02) = 다단 나누기,
    //   bit 2 (0x04) = 쪽 나누기,  bit 3 (0x08) = 단 나누기
    // HWPX는 pageBreak/columnBreak attr 과 secPr/colPr 구조를 분리해 저장하므로, HWP5
    // PARA_HEADER 에서는 각 축을 bitwise 로 합성해야 한다. 후속 구역이라고 해서
    // 무조건 0x04(쪽 나누기)로 덮으면, pageBreak 없는 "구역+다단" 문단이 한컴에서
    // 다른 layout contract 로 해석된다.
    let has_section = sec_def.is_some();
    let has_column_def = para
        .controls
        .iter()
        .any(|c| matches!(c, Control::ColumnDef(_)));
    if para.raw_break_type == 0 {
        let mut break_type = 0u8;
        if has_section {
            break_type |= 0x01;
        }
        if has_column_def {
            break_type |= 0x02;
        }
        if has_page_break_attr {
            break_type |= 0x04;
        }
        if has_column_break_attr {
            break_type |= 0x08;
        }

        if break_type != 0 {
            para.raw_break_type = break_type;
            para.column_type = if break_type & 0x04 != 0 {
                crate::model::paragraph::ColumnBreakType::Page
            } else if break_type & 0x08 != 0 {
                crate::model::paragraph::ColumnBreakType::Column
            } else if break_type & 0x01 != 0 {
                crate::model::paragraph::ColumnBreakType::Section
            } else {
                crate::model::paragraph::ColumnBreakType::MultiColumn
            };
        }
    }

    // [#1380] 원본에 `<hp:linesegarray>` 가 없는 문단은 line_segs 를 빈 채로 유지한다.
    // 종전에는 zero-default LineSeg 1개를 합성 주입했으나, serializer 가 이 주입분을
    // `vertsize="0" ...` lineseg 로 방출하여 원본 무 → RT 유 비대칭을 만들었다.
    // 한컴은 lineseg 가 없으면 열 때 재계산하므로 빈 채 보존이 안전하다.

    // [#2070] 전부 0 높이(lh=0, th=0)인 linesegarray 는 부재로 정규화한다.
    // 생성계 문서(80168 등 규제영향분석서)는 lineseg 를 0 으로 채워 저장하는데,
    // 0 높이 lineseg 는 배치 권위가 없고(한글은 열 때 재계산) 실저장 취급 시
    // NO_LS 성장 경로가 죽어 셀/문단 높이가 선언값으로 붕괴한다 (#1380 대칭,
    // body_text.rs parse_para_line_seg 와 동일 규칙).
    if !para.line_segs.is_empty()
        && para
            .line_segs
            .iter()
            .all(|s| s.line_height == 0 && s.text_height == 0)
    {
        para.line_segs.clear();
    }

    // [Task #1058 후속] HWPX `<hp:p id>` → HWP PARA_HEADER instance_id 매핑.
    // raw_header_extra 구조 (serializer 정합 — body_text.rs:241):
    //   raw_header_extra[0..6] = numCharShapes(2) + numRangeTags(2) + numLineSegs(2)
    //                              ← serializer 가 건너뜀 (실제 데이터 기반 재계산)
    //   raw_header_extra[6..10] = instanceId (UINT32 LE) ← HWPX `id` 매핑
    // raw_header_extra 가 비어 있으면 serializer 가 instance_id=0 으로 작성.
    // 한컴편집기 호환을 위해 HWPX 의 id 값을 정확히 보존.
    let mut header_extra = Vec::with_capacity(10);
    header_extra.extend_from_slice(&[0u8; 6]); // numCharShapes/numRangeTags/numLineSegs 자리
    header_extra.extend_from_slice(&hp_p_id.to_le_bytes()); // instanceId
    para.raw_header_extra = header_extra;

    Ok((para, sec_def))
}


pub(crate) fn parse_col_line(e: &quick_xml::events::BytesStart, cd: &mut ColumnDef) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"type" => cd.separator_type = parse_hwpx_line_type(&attr_str(&attr)),
            b"width" => cd.separator_width = parse_hwpx_line_width(&attr_str(&attr)),
            b"color" => cd.separator_color = parse_color(&attr),
            _ => {}
        }
    }
}


pub(crate) fn parse_hwpx_line_type(value: &str) -> u8 {
    match value {
        "NONE" => 0,
        "SOLID" => 1,
        "DASH" => 2,
        "DOT" => 3,
        "DASH_DOT" => 4,
        "DASH_DOT_DOT" => 5,
        "LONG_DASH" => 6,
        "CIRCLE" => 7,
        _ => 1,
    }
}


pub(crate) fn parse_hwpx_line_width(value: &str) -> u8 {
    let mm: f64 = value
        .split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.12);

    if mm <= 0.10 {
        0
    } else if mm <= 0.12 {
        1
    } else if mm <= 0.15 {
        2
    } else if mm <= 0.20 {
        3
    } else if mm <= 0.25 {
        4
    } else if mm <= 0.30 {
        5
    } else if mm <= 0.40 {
        6
    } else if mm <= 0.50 {
        7
    } else if mm <= 0.60 {
        8
    } else if mm <= 0.70 {
        9
    } else if mm <= 1.00 {
        10
    } else if mm <= 1.50 {
        11
    } else if mm <= 2.00 {
        12
    } else if mm <= 3.00 {
        13
    } else if mm <= 4.00 {
        14
    } else {
        15
    }
}


/// <hp:linesegarray> 내부의 <hp:lineseg> 요소들을 파싱한다.
pub(crate) fn parse_lineseg_array(reader: &mut Reader<&[u8]>, para: &mut Paragraph) -> Result<(), HwpxError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                let ename = e.name();
                let local = local_name(ename.as_ref());
                if local == b"lineseg" {
                    para.line_segs.push(parse_lineseg_element(e));
                }
            }
            Ok(Event::End(ref e)) => {
                let ename = e.name();
                if local_name(ename.as_ref()) == b"linesegarray" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("linesegarray: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}


/// 단일 <hp:lineseg> 요소의 속성을 LineSeg로 변환한다.
pub(crate) fn parse_lineseg_element(e: &quick_xml::events::BytesStart) -> LineSeg {
    let mut seg = LineSeg::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"textpos" => seg.text_start = parse_u32(&attr),
            b"vertpos" => seg.vertical_pos = parse_i32(&attr),
            b"vertsize" => seg.line_height = parse_i32(&attr),
            b"textheight" => seg.text_height = parse_i32(&attr),
            b"baseline" => seg.baseline_distance = parse_i32(&attr),
            b"spacing" => seg.line_spacing = parse_i32(&attr),
            b"horzpos" => seg.column_start = parse_i32(&attr),
            b"horzsize" => seg.segment_width = parse_i32(&attr),
            b"flags" => seg.tag = parse_u32(&attr),
            _ => {}
        }
    }
    seg
}


/// 서브리스트(subList) 내의 문단들을 파싱한다.
/// header, footer, footnote, endnote, hiddenComment에서 공통 사용.
pub(crate) fn parse_sublist_paragraphs(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
) -> Result<Vec<Paragraph>, HwpxError> {
    let mut paragraphs = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                if local == b"p" {
                    let (para, _) = parse_paragraph(ce, reader)?;
                    paragraphs.push(para);
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == end_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(HwpxError::XmlError(format!(
                    "{}: {}",
                    String::from_utf8_lossy(end_tag),
                    e
                )))
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(paragraphs)
}


/// HWPX header/footer subList는 HWP5 LIST_HEADER의 layout 필드로 materialize해야 한다.
pub(crate) fn parse_sublist_paragraphs_with_layout(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
) -> Result<HwpxSubListLayout, HwpxError> {
    let mut layout = HwpxSubListLayout::default();
    let mut root_sub_list_seen = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"subList" if !root_sub_list_seen => {
                        parse_hwpx_sublist_layout_attrs(ce, &mut layout);
                        root_sub_list_seen = true;
                    }
                    b"p" => {
                        let (para, _) = parse_paragraph(ce, reader)?;
                        layout.paragraphs.push(para);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                if local == b"subList" && !root_sub_list_seen {
                    parse_hwpx_sublist_layout_attrs(ce, &mut layout);
                    root_sub_list_seen = true;
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == end_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(HwpxError::XmlError(format!(
                    "{}: {}",
                    String::from_utf8_lossy(end_tag),
                    e
                )))
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(layout)
}


pub(crate) fn parse_hwpx_sublist_layout_attrs(
    e: &quick_xml::events::BytesStart,
    layout: &mut HwpxSubListLayout,
) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"vertAlign" => {
                layout.list_attr |= match attr_str(&attr).as_str() {
                    "CENTER" => 1 << 21,
                    "BOTTOM" => 2 << 21,
                    _ => 0,
                };
            }
            b"textWidth" => layout.text_width = parse_u32(&attr),
            b"textHeight" => layout.text_height = parse_u32(&attr),
            b"hasTextRef" => layout.text_ref = parse_u8(&attr),
            b"hasNumRef" => layout.num_ref = parse_u8(&attr),
            _ => {}
        }
    }
}
