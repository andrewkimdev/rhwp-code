//! table_parsing — table_layout.rs 에서 무변동 이동
use super::*;

pub(crate) fn parse_table(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Table, HwpxError> {
    let mut table = Table::default();
    let mut table_record_flags = 0u32;
    // [#2697] numberingType 부재 시 표의 자연 기본값은 TABLE (종전 방출 리터럴과 동일).
    table.common.numbering_type = crate::model::shape::ObjectNumberingType::Table;

    // 표 기본 속성
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"id" | b"instid" => table.common.instance_id = parse_u32(&attr),
            b"zOrder" => table.common.z_order = parse_i32(&attr),
            b"rowCnt" => table.row_count = parse_u16(&attr),
            b"colCnt" => table.col_count = parse_u16(&attr),
            b"cellSpacing" => table.cell_spacing = parse_i16(&attr),
            b"borderFillIDRef" => table.border_fill_id = parse_u16(&attr),
            b"noAdjust" => {
                if attr_str(&attr) == "1" {
                    table_record_flags |= 0x08;
                }
            }
            b"pageBreak" => {
                let val = attr_str(&attr);
                table.page_break = match val.as_str() {
                    // HWPX pageBreak="CELL" is serialized by Hancom as HWP5
                    // row-break (TABLE attr bit 1). HWPX pageBreak="TABLE"
                    // is serialized as HWP5 cell/table break (bit 0).
                    "TABLE" | "TABLE_BREAK" => TablePageBreak::CellBreak,
                    "CELL" | "CELL_BREAK" => TablePageBreak::RowBreak,
                    "ROW" | "ROW_BREAK" => TablePageBreak::RowBreak,
                    _ => TablePageBreak::None,
                };
            }
            b"repeatHeader" => {
                table.repeat_header = attr_str(&attr) == "1";
            }
            b"textWrap" => {
                table.common.text_wrap = match attr_str(&attr).as_str() {
                    // 표 textWrap 파서만 TIGHT/THROUGH arm 이 빠져 있어, 방출측
                    // (text_wrap_str)이 내는 이 두 값이 SQUARE 로 유실됐다. 도형/그림/차트
                    // 파서(같은 파일 2228/2795/5681)는 이미 처리하므로 표만 맞춘다.
                    "TIGHT" => crate::model::shape::TextWrap::Tight,
                    "THROUGH" => crate::model::shape::TextWrap::Through,
                    "TOP_AND_BOTTOM" => crate::model::shape::TextWrap::TopAndBottom,
                    "BEHIND_TEXT" => crate::model::shape::TextWrap::BehindText,
                    "IN_FRONT_OF_TEXT" => crate::model::shape::TextWrap::InFrontOfText,
                    _ => crate::model::shape::TextWrap::Square,
                };
            }
            b"textFlow" => {
                table.common.text_flow = match attr_str(&attr).as_str() {
                    "LEFT_ONLY" => crate::model::shape::TextFlow::LeftOnly,
                    "RIGHT_ONLY" => crate::model::shape::TextFlow::RightOnly,
                    "LARGEST_ONLY" => crate::model::shape::TextFlow::LargestOnly,
                    _ => crate::model::shape::TextFlow::BothSides,
                };
            }
            // [#2697] 표만 numberingType arm 이 없어 캡션 번호 범주가 파싱 단계에서
            // 유실됐다. 도형 파서(같은 파일 2855)와 동형. 방출측은 종전 "TABLE" 하드코딩.
            b"numberingType" => {
                table.common.numbering_type = match attr_str(&attr).to_ascii_uppercase().as_str() {
                    "PICTURE" => crate::model::shape::ObjectNumberingType::Picture,
                    "TABLE" => crate::model::shape::ObjectNumberingType::Table,
                    "EQUATION" => crate::model::shape::ObjectNumberingType::Equation,
                    _ => crate::model::shape::ObjectNumberingType::None,
                };
            }
            // [#2855] 표만 lock arm 이 없어 개체 잠금이 파싱 단계에서 유실됐다. 도형/그림
            // 계열이 공유하는 parse_object_element_attrs(같은 파일 2905행, #2840)와 동형.
            b"lock" => table.common.locked = attr_str(&attr) == "1",
            _ => {}
        }
    }

    // 표 내용 파싱 (행/셀)
    let mut buf = Vec::new();
    let mut current_row: u16 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"tr" => {
                        // 새 행
                    }
                    b"tc" => {
                        // 셀 파싱
                        let cell = parse_table_cell(ce, reader, current_row)?;
                        table.cells.push(cell);
                    }
                    b"caption" => {
                        let caption = parse_caption(ce, reader)?;
                        table.caption = Some(caption);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"sz" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => {
                                    table.common.width = parse_u32(&attr);
                                }
                                b"height" => {
                                    table.common.height = parse_u32(&attr);
                                }
                                b"widthRelTo" => {
                                    table.common.width_criterion =
                                        parse_size_criterion(&attr_str(&attr), true);
                                }
                                b"heightRelTo" => {
                                    table.common.height_criterion =
                                        parse_size_criterion(&attr_str(&attr), false);
                                }
                                // [#2697] 표만 protect arm 이 없어 "표 크기 보호"가 파싱
                                // 단계에서 유실됐다. 도형(2907)·사각형(5967)·양식(5590)
                                // 파서는 모두 같은 hp:sz@protect 를 읽는다.
                                b"protect" => table.common.size_protect = parse_bool(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"pos" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"treatAsChar" => {
                                    table.common.treat_as_char =
                                        attr_str(&attr) == "1" || attr_str(&attr) == "true";
                                }
                                // [#2784] affectLSpacing(줄 간격에 영향) — 표 pos 되읽기.
                                b"affectLSpacing" => {
                                    table.common.affect_line_spacing = parse_bool(&attr)
                                }
                                b"flowWithText" => table.common.flow_with_text = parse_bool(&attr),
                                b"allowOverlap" => table.common.allow_overlap = parse_bool(&attr),
                                b"holdAnchorAndSO" => {
                                    table.common.prevent_page_break =
                                        if parse_bool(&attr) { 1 } else { 0 };
                                }
                                b"vertRelTo" => {
                                    table.common.vert_rel_to = match attr_str(&attr).as_str() {
                                        "PAPER" => crate::model::shape::VertRelTo::Paper,
                                        "PAGE" => crate::model::shape::VertRelTo::Page,
                                        _ => crate::model::shape::VertRelTo::Para,
                                    };
                                }
                                b"horzRelTo" => {
                                    table.common.horz_rel_to = match attr_str(&attr).as_str() {
                                        "PAPER" => crate::model::shape::HorzRelTo::Paper,
                                        "PAGE" => crate::model::shape::HorzRelTo::Page,
                                        "COLUMN" => crate::model::shape::HorzRelTo::Column,
                                        _ => crate::model::shape::HorzRelTo::Para,
                                    };
                                }
                                b"vertAlign" => {
                                    table.common.vert_align = match attr_str(&attr).as_str() {
                                        "TOP" => crate::model::shape::VertAlign::Top,
                                        "CENTER" => crate::model::shape::VertAlign::Center,
                                        "BOTTOM" => crate::model::shape::VertAlign::Bottom,
                                        "INSIDE" => crate::model::shape::VertAlign::Inside,
                                        "OUTSIDE" => crate::model::shape::VertAlign::Outside,
                                        _ => crate::model::shape::VertAlign::Top,
                                    };
                                }
                                b"horzAlign" => {
                                    table.common.horz_align = match attr_str(&attr).as_str() {
                                        "LEFT" => crate::model::shape::HorzAlign::Left,
                                        "CENTER" => crate::model::shape::HorzAlign::Center,
                                        "RIGHT" => crate::model::shape::HorzAlign::Right,
                                        "INSIDE" => crate::model::shape::HorzAlign::Inside,
                                        "OUTSIDE" => crate::model::shape::HorzAlign::Outside,
                                        _ => crate::model::shape::HorzAlign::Left,
                                    };
                                }
                                b"vertOffset" => {
                                    table.common.vertical_offset = parse_i32_wrapping(&attr) as u32;
                                }
                                b"horzOffset" => {
                                    table.common.horizontal_offset =
                                        parse_i32_wrapping(&attr) as u32;
                                }
                                _ => {}
                            }
                        }
                    }
                    b"outMargin" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => table.outer_margin_left = parse_i16(&attr),
                                b"right" => table.outer_margin_right = parse_i16(&attr),
                                b"top" => table.outer_margin_top = parse_i16(&attr),
                                b"bottom" => table.outer_margin_bottom = parse_i16(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"inMargin" => {
                        // 표 안쪽 여백 → table.padding
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => table.padding.left = parse_i16(&attr),
                                b"right" => table.padding.right = parse_i16(&attr),
                                b"top" => table.padding.top = parse_i16(&attr),
                                b"bottom" => table.padding.bottom = parse_i16(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"cellzone" => {
                        let mut zone = crate::model::table::TableZone::default();
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"startColAddr" => zone.start_col = parse_u16(&attr),
                                b"startRowAddr" => zone.start_row = parse_u16(&attr),
                                b"endColAddr" => zone.end_col = parse_u16(&attr),
                                b"endRowAddr" => zone.end_row = parse_u16(&attr),
                                b"borderFillIDRef" => zone.border_fill_id = parse_u16(&attr),
                                _ => {}
                            }
                        }
                        table.zones.push(zone);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                let local = local_name(eename.as_ref());
                match local {
                    b"tr" => current_row += 1,
                    b"tbl" => break,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("table: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // [Task #1772] outMargin → common.margin 동기화 (IR 계약).
    // 레이아웃의 쪽 고정 자리차지 표 예약 하단(calc_shape_bottom_y)은 common.margin 을
    // 참조하고, HWPX→HWP 어댑터(materialize_table_outer_margin)도 직렬화 시 동일하게
    // 동기화한다. 파서가 outer_margin_* 만 채우면 HWPX 직파스 문서에서만 표 바깥 여백이
    // 무시되어 본문이 저장 lineseg(한컴 위치)보다 위로 붙는다 (11.36px 군집).
    table.common.margin.left = table.outer_margin_left;
    table.common.margin.right = table.outer_margin_right;
    table.common.margin.top = table.outer_margin_top;
    table.common.margin.bottom = table.outer_margin_bottom;

    // row_sizes 설정 (행별 셀 수, HWP 스펙 UINT16[NRows] 계약과 동일 — 높이가 아니다).
    // model::table::Table::rebuild_row_sizes, parser::control(HWP5), html_table_import,
    // document_core::commands::object_ops::table 이 모두 이 필드를 "행별 셀 개수"로 채운다.
    table.row_sizes = (0..table.row_count)
        .map(|r| table.cells.iter().filter(|c| c.row == r).count() as i16)
        .collect();

    materialize_hwpx_table_attrs(&mut table, table_record_flags);
    table.rebuild_grid();
    Ok(table)
}


pub(crate) fn parse_caption_sub_list_attrs(
    e: &quick_xml::events::BytesStart,
    caption: &mut crate::model::shape::Caption,
) {
    use crate::model::shape::CaptionVertAlign;

    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"vertAlign" {
            caption.vert_align = match attr_str(&attr).as_str() {
                "CENTER" => CaptionVertAlign::Center,
                "BOTTOM" => CaptionVertAlign::Bottom,
                // 누락·미지·미래 lexical 값은 모델 기본값을 쓴다. 다른 HWPX subList
                // enum 파서가 알 수 없는 값을 기본값으로 관용 처리하는 정책과 같다.
                _ => CaptionVertAlign::Top,
            };
        }
    }
}


pub(crate) fn parse_table_cell(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
    current_row: u16,
) -> Result<Cell, HwpxError> {
    let mut cell = Cell::default();
    cell.row = current_row;
    cell.col_span = 1;
    cell.row_span = 1;

    // <hp:tc> 요소 자체의 속성 파싱
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"borderFillIDRef" => cell.border_fill_id = parse_u16(&attr),
            b"header" => cell.set_header(parse_bool(&attr)),
            b"hasMargin" => cell.set_apply_inner_margin(parse_bool(&attr)),
            b"protect" => cell.set_cell_protect(parse_bool(&attr)),
            b"editable" => cell.set_editable_in_form(parse_bool(&attr)),
            b"dirty" => cell.dirty_flag = parse_bool(&attr),
            // 셀 필드 이름 (누름틀 셀 필드, #493). 직렬화기는 무명 셀도 name=""로
            // 항상 방출하므로 빈 값은 None — HWP5 파서(parse_cell_field_name)와
            // 동일 의미. 누락 시 HWPX 로드에서 getFieldList가 셀 필드를 반환하지 못하고
            // HWPX 라운드트립에서 셀 필드 이름이 유실된다.
            b"name" => {
                let v = attr_str(&attr);
                cell.field_name = if v.is_empty() { None } else { Some(v) };
            }
            _ => {}
        }
    }

    // 셀 자식 요소 파싱
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"cellAddr" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"colAddr" => {
                                    cell.col = parse_u16(&attr);
                                }
                                b"rowAddr" => cell.row = parse_u16(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"cellSpan" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"colSpan" => cell.col_span = parse_u16(&attr).max(1),
                                b"rowSpan" => cell.row_span = parse_u16(&attr).max(1),
                                _ => {}
                            }
                        }
                    }
                    b"cellSz" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => cell.width = parse_u32(&attr),
                                b"height" => cell.height = parse_u32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"cellMargin" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => cell.padding.left = parse_i16(&attr),
                                b"right" => cell.padding.right = parse_i16(&attr),
                                b"top" => cell.padding.top = parse_i16(&attr),
                                b"bottom" => cell.padding.bottom = parse_i16(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"tcPr" => {
                        // 셀 속성 (legacy format)
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"borderFillIDRef" => cell.border_fill_id = parse_u16(&attr),
                                b"textDirection" => {
                                    let val = attr_str(&attr);
                                    cell.text_direction = if val == "VERTICAL" { 1 } else { 0 };
                                }
                                b"vAlign" => {
                                    cell.vertical_align = match attr_str(&attr).as_str() {
                                        "CENTER" => VerticalAlign::Center,
                                        "BOTTOM" => VerticalAlign::Bottom,
                                        _ => VerticalAlign::Top,
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    b"subList" => {
                        // subList: vertAlign + textDirection 속성 파싱
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"vertAlign" => {
                                    cell.vertical_align = match attr_str(&attr).as_str() {
                                        "CENTER" => VerticalAlign::Center,
                                        "BOTTOM" => VerticalAlign::Bottom,
                                        _ => VerticalAlign::Top,
                                    };
                                }
                                // 세로쓰기 셀(textDirection). serializer 는 셀 <hp:subList>
                                // 에 방출하지만 종전엔 vertAlign 만 읽어 세로쓰기가 왕복 시
                                // 유실됐다(cellPr 경로는 serializer 가 방출하지 않음).
                                b"textDirection" => {
                                    cell.text_direction =
                                        if attr_str(&attr) == "VERTICAL" { 1 } else { 0 };
                                }
                                _ => {}
                            }
                        }
                    }
                    b"cellPr" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"borderFillIDRef" => cell.border_fill_id = parse_u16(&attr),
                                b"textDirection" => {
                                    let val = attr_str(&attr);
                                    cell.text_direction = if val == "VERTICAL" { 1 } else { 0 };
                                }
                                b"vAlign" => {
                                    cell.vertical_align = match attr_str(&attr).as_str() {
                                        "CENTER" => VerticalAlign::Center,
                                        "BOTTOM" => VerticalAlign::Bottom,
                                        _ => VerticalAlign::Top,
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    b"p" => {
                        // 셀 내 문단 (secDef는 무시)
                        let (para, _) = parse_paragraph(ce, reader)?;
                        cell.paragraphs.push(para);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"cellAddr" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"colAddr" => {
                                    cell.col = parse_u16(&attr);
                                }
                                b"rowAddr" => cell.row = parse_u16(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"cellSpan" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"colSpan" => cell.col_span = parse_u16(&attr).max(1),
                                b"rowSpan" => cell.row_span = parse_u16(&attr).max(1),
                                _ => {}
                            }
                        }
                    }
                    b"cellSz" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => cell.width = parse_u32(&attr),
                                b"height" => cell.height = parse_u32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"cellMargin" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => cell.padding.left = parse_i16(&attr),
                                b"right" => cell.padding.right = parse_i16(&attr),
                                b"top" => cell.padding.top = parse_i16(&attr),
                                b"bottom" => cell.padding.bottom = parse_i16(&attr),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"tc" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("tc: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // 셀에 문단이 없으면 빈 문단 추가
    if cell.paragraphs.is_empty() {
        cell.paragraphs.push(Paragraph::new_empty());
    }

    Ok(cell)
}
