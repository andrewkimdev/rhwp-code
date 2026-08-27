//! section*.xml 파싱 — HWPX 섹션 본문을 Section 모델로 변환
//!
//! 섹션 XML의 문단(<hp:p>), 텍스트 런(<hp:run>), 표(<hp:tbl>),
//! 이미지(<hp:pic>) 등을 기존 Document 모델로 변환한다.

use quick_xml::events::{BytesRef, Event};
use quick_xml::Reader;

use crate::model::control::{
    AutoNumber, AutoNumberType, Bookmark, CharOverlap, Control, Equation, Field, FieldType,
    FormObject, FormType, HiddenComment, NewNumber, PageHide, PageNumberPos, Parameter,
    ParameterList, Ruby, EQUATION_LINE_MODE_BIT,
};
use crate::model::document::{Section, SectionDef};
use crate::model::footnote::{Endnote, Footnote};
use crate::model::header_footer::{Footer, Header, HeaderFooterApply, MasterPage};
use crate::model::image::{
    CropInfo, EffectColor, EffectPoint, EffectRgb, ImageAttr, ImageEffect, PictureEffects,
    PictureShadow,
};
use crate::model::page::{
    BindingMethod, ColumnDef, ColumnDirection, ColumnType, PageBorderBasis, PageBorderFill,
    PageBorderUiBasis, PageDef,
};
use crate::model::paragraph::{CharShapeRef, FieldRange, LineSeg, OrphanFieldEnd, Paragraph};
use crate::model::shape::{
    ArcShape, CommonObjAttr, ConnectorControlPoint, ConnectorData, CurveShape, DrawingObjAttr,
    EllipseShape, GroupShape, HorzAlign, HorzRelTo, LineShape, LinkLineType, PolygonShape,
    RectangleShape, ShapeComponentAttr, ShapeObject, SizeCriterion, TextBox, TextWrap, VertAlign,
    VertRelTo,
};
use crate::model::style::{Fill, ShapeBorderLine};
use crate::model::table::{Cell, Table, TablePageBreak, VerticalAlign};
use crate::model::HwpUnit16;
use crate::parser::tags;

use super::utils::{
    attr_str, local_name, parse_bool, parse_color, parse_gradient_type, parse_hatch_style,
    parse_i16, parse_i32, parse_i32_wrapping, parse_i8, parse_u16, parse_u32, parse_u8,
    skip_element,
};
use super::HwpxError;
mod param_frame;
pub(crate) use param_frame::*;
mod shape_storage;
pub(crate) use shape_storage::*;
mod table_parsing;
pub(crate) use table_parsing::*;
mod paragraph_parsing;
pub(crate) use paragraph_parsing::*;



/// section*.xml을 파싱하여 Section 모델로 변환한다.
pub fn parse_hwpx_section(xml: &str) -> Result<Section, HwpxError> {
    let mut section = Section::default();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let ename = e.name();
                let local = local_name(ename.as_ref());
                match local {
                    b"p" => {
                        // 최상위 문단
                        let (para, sec_def_opt) = parse_paragraph(e, &mut reader)?;
                        if let Some(sec_def) = sec_def_opt {
                            section.section_def = sec_def;
                        }
                        section.paragraphs.push(para);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("section: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(section)
}

/// section XML의 `<hp:masterPage idRef="...">` 참조를 문서 순서대로 수집한다.
pub fn collect_hwpx_section_master_page_refs(xml: &str) -> Result<Vec<String>, HwpxError> {
    let mut reader = Reader::from_str(xml);
    let mut refs = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                if local_name(e.name().as_ref()) == b"masterPage" {
                    push_master_page_id_ref(e, &mut refs);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(HwpxError::XmlError(format!(
                    "section masterPage refs: {}",
                    e
                )))
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(refs)
}

fn push_master_page_id_ref(e: &quick_xml::events::BytesStart, refs: &mut Vec<String>) {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"idRef" {
            let id_ref = attr_str(&attr);
            if !id_ref.is_empty() {
                refs.push(id_ref);
            }
        }
    }
}

/// masterpage*.xml을 파싱하여 기존 HWP 바탕쪽 모델로 변환한다.
pub fn parse_hwpx_master_page(xml: &str) -> Result<MasterPage, HwpxError> {
    let mut master_page = MasterPage::default();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut root_sub_list_seen = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let ename = e.name();
                let local = local_name(ename.as_ref());
                match local {
                    b"masterPage" => parse_master_page_start(e, &mut master_page),
                    b"subList" if !root_sub_list_seen => {
                        parse_master_page_sub_list(e, &mut master_page);
                        root_sub_list_seen = true;
                    }
                    b"p" => {
                        let (para, _) = parse_paragraph(e, &mut reader)?;
                        master_page.paragraphs.push(para);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let ename = e.name();
                let local = local_name(ename.as_ref());
                match local {
                    b"masterPage" => parse_master_page_start(e, &mut master_page),
                    b"subList" if !root_sub_list_seen => {
                        parse_master_page_sub_list(e, &mut master_page);
                        root_sub_list_seen = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("masterpage: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    if master_page.text_width > 0 || master_page.text_height > 0 {
        master_page.raw_list_header = build_hwpx_master_page_list_header(&master_page);
    }

    Ok(master_page)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HwpxMasterPageType {
    Both,
    Even,
    Odd,
    LastPage,
    OptionalPage,
}

fn parse_hwpx_master_page_type(value: &str) -> HwpxMasterPageType {
    let normalized: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    match normalized.as_str() {
        "EVEN" => HwpxMasterPageType::Even,
        "ODD" => HwpxMasterPageType::Odd,
        "LASTPAGE" => HwpxMasterPageType::LastPage,
        "OPTIONALPAGE" => HwpxMasterPageType::OptionalPage,
        _ => HwpxMasterPageType::Both,
    }
}

fn parse_master_page_start(e: &quick_xml::events::BytesStart, master_page: &mut MasterPage) {
    let mut is_last_page = false;
    let mut is_optional_page = false;
    let mut page_duplicate: Option<bool> = None;
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"type" => {
                let value = attr_str(&attr);
                match parse_hwpx_master_page_type(&value) {
                    HwpxMasterPageType::Even => master_page.apply_to = HeaderFooterApply::Even,
                    HwpxMasterPageType::Odd => master_page.apply_to = HeaderFooterApply::Odd,
                    HwpxMasterPageType::LastPage => {
                        is_last_page = true;
                        master_page.apply_to = HeaderFooterApply::Both;
                        master_page.is_extension = true;
                    }
                    HwpxMasterPageType::OptionalPage => {
                        is_optional_page = true;
                        master_page.apply_to = HeaderFooterApply::Both;
                        master_page.is_extension = true;
                    }
                    HwpxMasterPageType::Both => master_page.apply_to = HeaderFooterApply::Both,
                }
            }
            b"pageDuplicate" => {
                let duplicate = attr_str(&attr) != "0";
                page_duplicate = Some(duplicate);
                master_page.overlap = duplicate;
            }
            b"pageNumber" => master_page.hwpx_page_number = Some(parse_u16(&attr)),
            // 표지(첫 쪽) 전용 바탕쪽. serializer 는 방출하나 종전엔 미독 →
            // pageFront="1" 바탕쪽이 왕복 시 "0" 으로 적용 범위가 바뀌었다.
            b"pageFront" => master_page.page_front = attr_str(&attr) != "0",
            _ => {}
        }
    }
    // 한컴 HWPX -> HWP5 저장본은 LAST_PAGE 바탕쪽을 확장 바탕쪽으로 저장하면서
    // pageDuplicate="0"인 경우에도 overlap bit를 함께 세운다.
    if is_last_page {
        master_page.replace_base = page_duplicate == Some(false);
        master_page.overlap = true;
    }
    if is_optional_page {
        master_page.overlap = true;
    }
    master_page.ext_flags = u16::from(master_page.overlap)
        | if master_page.is_extension { 0x02 } else { 0 }
        | if is_optional_page { 0x04 } else { 0 };
}

fn parse_master_page_sub_list(e: &quick_xml::events::BytesStart, master_page: &mut MasterPage) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"textWidth" => master_page.text_width = parse_u32(&attr),
            b"textHeight" => master_page.text_height = parse_u32(&attr),
            b"hasTextRef" => master_page.text_ref = parse_u8(&attr),
            b"hasNumRef" => master_page.num_ref = parse_u8(&attr),
            // 세로쓰기 바탕쪽(hp:subList@textDirection). serializer 는 항상
            // HORIZONTAL 로 고정 출력하지만 종전엔 파서가 미독 →
            // textDirection="VERTICAL" 바탕쪽이 왕복 시 가로쓰기로 바뀌었다.
            b"textDirection" => {
                master_page.text_direction = if attr_str(&attr) == "VERTICAL" { 1 } else { 0 };
            }
            _ => {}
        }
    }
}

fn build_hwpx_master_page_list_header(master_page: &MasterPage) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(34);
    bytes.extend_from_slice(&(master_page.paragraphs.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&master_page.text_width.to_le_bytes());
    bytes.extend_from_slice(&master_page.text_height.to_le_bytes());
    bytes.push(master_page.text_ref);
    bytes.push(master_page.num_ref);
    bytes.extend_from_slice(&master_page.ext_flags.to_le_bytes());
    bytes.extend_from_slice(&[0u8; 14]);
    bytes
}

// ─── SectionDef / PageDef ───

fn parse_section_def_start(e: &quick_xml::events::BytesStart, sec_def: &mut SectionDef) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"textDirection" => {
                let val = attr_str(&attr);
                sec_def.text_direction = if val == "VERTICAL" { 1 } else { 0 };
            }
            b"tabStop" => {
                sec_def.default_tab_spacing = parse_u32(&attr);
            }
            b"masterPageCnt" => {
                let count = parse_u32(&attr).min(3);
                sec_def.flags = (sec_def.flags & !(0x03 << 30)) | (count << 30);
            }
            // [Task #1058] 한컴 HWP5 spec 표 129 정합:
            //   - spaceColumns → column_spacing (HWPUNIT16, default 1134 for 다단)
            //   - outlineShapeIDRef → outline_numbering_id (UINT16, 1=기본 번호 문단 모양)
            b"spaceColumns" => {
                let v = parse_u32(&attr);
                sec_def.column_spacing = v as i16;
            }
            b"outlineShapeIDRef" => {
                sec_def.outline_numbering_id = parse_u16(&attr);
            }
            // [#2779] memoShapeIDRef → memo_shape_id (UINT16, header.xml `hh:memoPr@id` 참조).
            // 종전엔 수집하지 않아 저장 시 템플릿 상수 "0" 으로 리셋됐다(실측 14 secPr/9 파일).
            b"memoShapeIDRef" => {
                sec_def.memo_shape_id = parse_u16(&attr);
            }
            _ => {}
        }
    }
}

fn parse_page_pr(e: &quick_xml::events::BytesStart, page: &mut PageDef) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"width" => page.width = parse_u32(&attr),
            b"height" => page.height = parse_u32(&attr),
            // [#1166] HWPX 용지 방향. OWPML landscape 값:
            //   WIDELY  = 세로(Portrait)  → landscape=false
            //   NARROWLY= 가로(Landscape) → landscape=true
            // (hwplib ForSecPr: Portrait→WIDELY, Landscape→NARROWLY 매핑 권위.)
            // width/height 는 HWP 바이너리와 동일하게 짧은변=width/긴변=height 로
            // 저장되고, landscape=true 일 때 렌더러가 swap 한다(page.rs). 종전엔
            // landscape 를 무시해 가로 용지 HWPX 가 항상 세로로 렌더되는 결함.
            b"landscape" => {
                page.landscape = attr_str(&attr).eq_ignore_ascii_case("NARROWLY");
            }
            b"gutterType" => {
                let value = attr_str(&attr);
                let binding_code = match value.as_str() {
                    "LEFT_RIGHT" => 1,
                    "TOP_BOTTOM" => 2,
                    _ => 0,
                };
                page.attr = (page.attr & !(0x03 << 1)) | (binding_code << 1);
                page.binding = match binding_code {
                    1 => BindingMethod::DuplexSided,
                    2 => BindingMethod::TopFlip,
                    _ => BindingMethod::SingleSided,
                };
            }
            _ => {}
        }
    }
}

fn parse_grid(e: &quick_xml::events::BytesStart, sec_def: &mut SectionDef) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"lineGrid" => sec_def.line_grid = parse_i32(&attr) as i16,
            b"charGrid" => sec_def.char_grid = parse_i32(&attr) as i16,
            _ => {}
        }
    }
}

fn parse_page_margin(e: &quick_xml::events::BytesStart, page: &mut PageDef) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"left" => page.margin_left = parse_u32(&attr),
            b"right" => page.margin_right = parse_u32(&attr),
            b"top" => page.margin_top = parse_u32(&attr),
            b"bottom" => page.margin_bottom = parse_u32(&attr),
            b"header" => page.margin_header = parse_u32(&attr),
            b"footer" => page.margin_footer = parse_u32(&attr),
            b"gutter" => page.margin_gutter = parse_u32(&attr),
            _ => {}
        }
    }
}

// ─── Paragraph ───


/// secPr의 자식 요소들 (pagePr, margin, colPr 등) 파싱
/// 반환: 파싱된 ColumnDef (없으면 None)
fn parse_sec_pr_children(
    reader: &mut Reader<&[u8]>,
    sec_def: &mut SectionDef,
) -> Result<Option<ColumnDef>, HwpxError> {
    let mut buf = Vec::new();
    let mut col_def: Option<ColumnDef> = None;
    let mut page_border_fill_count = 0usize;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let ename = e.name();
                let local = local_name(ename.as_ref());
                match local {
                    b"pagePr" => parse_page_pr(e, &mut sec_def.page_def),
                    b"margin" => parse_page_margin(e, &mut sec_def.page_def),
                    b"grid" => parse_grid(e, sec_def),
                    b"colPr" => {
                        col_def = Some(parse_col_pr_with_children(e, reader)?);
                    }
                    b"startNum" => parse_start_num(e, sec_def),
                    b"visibility" => parse_visibility(e, sec_def),
                    b"pageBorderFill" => {
                        let (pbf, apply_type) = parse_page_border_fill(e, reader)?;
                        push_page_border_fill(
                            sec_def,
                            pbf,
                            &apply_type,
                            &mut page_border_fill_count,
                        );
                    }
                    // [Task #1050] footNotePr / endNotePr 의 자식 (autoNumFormat, noteLine 등)
                    // 파싱 — 한컴 정답 footnote 영역 렌더링을 위한 FootnoteShape contract.
                    b"footNotePr" => {
                        parse_note_pr_children(reader, &mut sec_def.footnote_shape, b"footNotePr")?;
                    }
                    b"endNotePr" => {
                        parse_note_pr_children(reader, &mut sec_def.endnote_shape, b"endNotePr")?;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let ename = e.name();
                let local = local_name(ename.as_ref());
                match local {
                    b"pagePr" => parse_page_pr(e, &mut sec_def.page_def),
                    b"margin" => parse_page_margin(e, &mut sec_def.page_def),
                    b"grid" => parse_grid(e, sec_def),
                    b"colPr" => {
                        col_def = Some(parse_col_pr(e));
                    }
                    b"startNum" => parse_start_num(e, sec_def),
                    b"visibility" => parse_visibility(e, sec_def),
                    b"pageBorderFill" => {
                        let (pbf, apply_type) = parse_page_border_fill_empty(e);
                        push_page_border_fill(
                            sec_def,
                            pbf,
                            &apply_type,
                            &mut page_border_fill_count,
                        );
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let ename = e.name();
                if local_name(ename.as_ref()) == b"secPr" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("secPr: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(col_def)
}

/// [Task #1050] `<hp:footNotePr>` / `<hp:endNotePr>` 의 자식 요소 파싱:
///   - `<hp:autoNumFormat type="DIGIT" suffixChar=")" prefixChar="" userChar="">` → FootnoteShape
///   - `<hp:noteLine length="-1" type="SOLID" width="0.12 mm" color="#000000">` → separator_*
///   - `<hp:noteSpacing betweenNotes="" belowLine="" aboveLine="">` → spacing
///   - `<hp:numbering type="CONTINUOUS" newNum="1">` → numbering
///   - `<hp:placement place="EACH_COLUMN" beneathText="0">` → placement
fn parse_note_pr_children(
    reader: &mut Reader<&[u8]>,
    shape: &mut crate::model::footnote::FootnoteShape,
    end_tag: &[u8],
) -> Result<(), HwpxError> {
    let is_end_note = end_tag == b"endNotePr";
    let mut saw_above_line = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let ename = e.name();
                let local = local_name(ename.as_ref());
                match local {
                    b"autoNumFormat" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        shape.number_format =
                                            crate::model::footnote::FootnoteShape::number_format_from_name(
                                                s,
                                                shape.number_format,
                                            );
                                    }
                                }
                                b"suffixChar" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Some(c) = s.chars().next() {
                                            shape.suffix_char = c;
                                        }
                                    }
                                }
                                b"prefixChar" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Some(c) = s.chars().next() {
                                            shape.prefix_char = c;
                                        }
                                    }
                                }
                                b"userChar" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Some(c) = s.chars().next() {
                                            shape.user_char = c;
                                        }
                                    }
                                }
                                b"supscript" => {
                                    shape.number_code_superscript = parse_bool_attr(&attr);
                                }
                                _ => {}
                            }
                        }
                    }
                    b"noteLine" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"length" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Ok(v) = s.parse::<i32>() {
                                            // 한컴 미주 기본값 "14692344"(전폭 sentinel)는 i16을
                                            // 넘으므로 절단하지 않고 그대로 보존한다. 렌더러가 col
                                            // 폭으로 clamp → 전폭. (i16 절단 시 12280 → 짧은 구분선)
                                            shape.separator_length = v;
                                        }
                                    }
                                }
                                b"type" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        shape.separator_line_type = match s {
                                            "SOLID" => 1,
                                            "DASH" => 2,
                                            "DOT" => 3,
                                            "DASH_DOT" => 4,
                                            "DASH_DOT_DOT" => 5,
                                            "LONG_DASH" => 6,
                                            "CIRCLE" => 7,
                                            "DOUBLE_SLIM" => 8,
                                            "SLIM_THICK" => 9,
                                            "THICK_SLIM" => 10,
                                            "SLIM_THICK_SLIM" => 11,
                                            "NONE" => 0,
                                            _ => 1, // default SOLID
                                        };
                                    }
                                }
                                b"width" => {
                                    // 미주/각주 구분선 굵기도 테두리 굵기 raw 코드와 같은 표를 쓴다.
                                    // 예: 0.12mm → 1, 0.7mm → 9.
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        shape.separator_line_width = parse_hwpx_line_width(s);
                                    }
                                }
                                b"color" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        // "#RRGGBB" → ColorRef (0xBBGGRR LE = HWP 표준)
                                        if let Some(hex) = s.strip_prefix('#') {
                                            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                                                let r = (rgb >> 16) & 0xFF;
                                                let g = (rgb >> 8) & 0xFF;
                                                let b = rgb & 0xFF;
                                                shape.separator_color = b << 16 | g << 8 | r;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    b"noteSpacing" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                // 공식 미주/각주 모양 의미:
                                // betweenNotes → 앞 번호 주석 내용과 다음 번호 주석 내용 사이
                                // belowLine → 구분선과 주석 내용 사이
                                // aboveLine → 본문과 구분선 사이
                                b"betweenNotes" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Ok(v) = s.parse::<u16>() {
                                            shape.raw_unknown = v;
                                        }
                                    }
                                }
                                b"belowLine" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Ok(v) = s.parse::<i16>() {
                                            shape.note_spacing = v;
                                        }
                                    }
                                }
                                b"aboveLine" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Ok(v) = s.parse::<i16>() {
                                            shape.separator_margin_top = v;
                                            saw_above_line = true;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        // 일부 오래된 HWPX에는 aboveLine 이 생략될 수 있으므로 기존 sentinel
                        // fallback 만 유지한다. aboveLine 이 있으면 공식 "구분선 위" 값으로 쓴다.
                        if !saw_above_line
                            && shape.separator_margin_top == 0
                            && shape.separator_line_type != 0
                        {
                            shape.separator_margin_top =
                                if is_end_note && shape.separator_length > 0 {
                                    224
                                } else {
                                    -1
                                };
                        }
                    }
                    b"numbering" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        let numbering = match s {
                                            "CONTINUOUS" | "continue" => {
                                                crate::model::footnote::FootnoteNumbering::Continue
                                            }
                                            "ON_SECTION" | "RESTART_SECTION" | "restartSection" => {
                                                crate::model::footnote::FootnoteNumbering::RestartSection
                                            }
                                            "ON_PAGE" | "RESTART_PAGE" | "restartPage" => {
                                                crate::model::footnote::FootnoteNumbering::RestartPage
                                            }
                                            _ => continue,
                                        };
                                        shape.numbering = numbering;
                                    }
                                }
                                b"newNum" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        if let Ok(v) = s.parse::<u16>() {
                                            shape.start_number = v;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    b"placement" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"place" => {
                                    if let Ok(s) = std::str::from_utf8(&attr.value) {
                                        // [#2779] OWPML 스키마(ParaList placement@place)의 정식
                                        // 토큰은 컨텍스트마다 다르지만 HWP5 attr bits 8-9 코드
                                        // 공간은 공유한다:
                                        //   각주 EACH_COLUMN(0)·MERGED_COLUMN(1)·RIGHT_MOST_COLUMN(2)
                                        //   미주 END_OF_DOCUMENT(0)·END_OF_SECTION(1)
                                        // 종전엔 MERGED_COLUMN/RIGHT_MOST_COLUMN 이 표에 없어
                                        // `_ => continue` 로 떨어져, 통단·오른쪽단 각주가 파싱
                                        // 단계에서 기본값(각 단마다)으로 소실됐다.
                                        // (BELOW_TEXT/RIGHT_COLUMN 은 스키마 밖 관용 표기 — 수용 유지.)
                                        let placement = match s {
                                            "END_OF_SECTION" | "MERGED_COLUMN" | "BELOW_TEXT"
                                            | "sectionEnd" | "belowText" => {
                                                crate::model::footnote::FootnotePlacement::BelowText
                                            }
                                            "RIGHT_MOST_COLUMN" | "RIGHT_COLUMN"
                                            | "rightColumn" => {
                                                crate::model::footnote::FootnotePlacement::RightColumn
                                            }
                                            "END_OF_DOCUMENT" | "EACH_COLUMN" | "documentEnd"
                                            | "eachColumn" => {
                                                crate::model::footnote::FootnotePlacement::EachColumn
                                            }
                                            _ => continue,
                                        };
                                        shape.placement = placement;
                                    }
                                }
                                b"beneathText" => {
                                    shape.print_inline_after_text = parse_bool_attr(&attr);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                if local_name(e.name().as_ref()) == end_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(HwpxError::XmlError(format!(
                    "{}: {}",
                    std::str::from_utf8(end_tag).unwrap_or("notePr"),
                    e
                )))
            }
            _ => {}
        }
        buf.clear();
    }
    shape.attr = shape.encode_attr();
    Ok(())
}

/// `type`(BOTH/EVEN/ODD) 속성 값을 기준으로 슬롯을 배정한다. XML 등장 순서가
/// BOTH → EVEN → ODD 를 보장하지 않으므로(#2885), 파싱된 `type` 값을 우선 사용하고
/// 인식하지 못하는/누락된 값에 한해서만 기존 등장 순서 기반 폴백을 적용한다.
fn push_page_border_fill(
    sec_def: &mut SectionDef,
    page_border_fill: PageBorderFill,
    apply_type: &str,
    count: &mut usize,
) {
    match apply_type.to_ascii_uppercase().as_str() {
        "BOTH" => sec_def.page_border_fill = page_border_fill,
        "EVEN" => {
            if sec_def.extra_page_border_fills.is_empty() {
                sec_def.extra_page_border_fills.push(page_border_fill);
            } else {
                sec_def.extra_page_border_fills[0] = page_border_fill;
            }
        }
        "ODD" => {
            while sec_def.extra_page_border_fills.is_empty() {
                sec_def
                    .extra_page_border_fills
                    .push(PageBorderFill::default());
            }
            if sec_def.extra_page_border_fills.len() < 2 {
                sec_def.extra_page_border_fills.push(page_border_fill);
            } else {
                sec_def.extra_page_border_fills[1] = page_border_fill;
            }
        }
        _ => {
            // type 값이 없거나 인식 불가 — 기존 등장 순서 기반 폴백(회귀 방지).
            if *count == 0 {
                sec_def.page_border_fill = page_border_fill;
            } else {
                sec_def.extra_page_border_fills.push(page_border_fill);
            }
        }
    }
    *count += 1;
}

fn parse_page_border_fill(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<(PageBorderFill, String), HwpxError> {
    let (mut page_border_fill, apply_type) = parse_page_border_fill_empty(e);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref child)) | Ok(Event::Empty(ref child)) => {
                if local_name(child.name().as_ref()) == b"offset" {
                    parse_page_border_fill_offset(child, &mut page_border_fill);
                }
            }
            Ok(Event::End(ref end)) => {
                if local_name(end.name().as_ref()) == b"pageBorderFill" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(HwpxError::XmlError(format!("pageBorderFill: {}", err)));
            }
            _ => {}
        }
        buf.clear();
    }
    Ok((page_border_fill, apply_type))
}

fn parse_page_border_fill_empty(e: &quick_xml::events::BytesStart) -> (PageBorderFill, String) {
    let mut page_border_fill = PageBorderFill::default();
    let mut text_border = String::new();
    let mut fill_area = String::new();
    let mut apply_type = String::new();
    let mut header_inside = false;
    let mut footer_inside = false;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"borderFillIDRef" => page_border_fill.border_fill_id = parse_u16(&attr),
            b"textBorder" => text_border = attr_str(&attr),
            b"fillArea" => fill_area = attr_str(&attr),
            b"type" => apply_type = attr_str(&attr),
            b"headerInside" => header_inside = parse_bool(&attr),
            b"footerInside" => footer_inside = parse_bool(&attr),
            _ => {}
        }
    }

    page_border_fill.attr = page_border_fill_attr(
        &text_border,
        &fill_area,
        &apply_type,
        header_inside,
        footer_inside,
    );
    page_border_fill.ui_basis = if text_border.eq_ignore_ascii_case("PAPER") {
        // Task #1129 Stage 28: textBorder=PAPER is shown as page basis in the
        // dialog and renders from the page/body area edge.
        page_border_fill.basis = PageBorderBasis::BodyBased;
        PageBorderUiBasis::Page
    } else {
        page_border_fill.basis = PageBorderBasis::PaperBased;
        PageBorderUiBasis::Paper
    };
    (page_border_fill, apply_type)
}

fn parse_page_border_fill_offset(
    e: &quick_xml::events::BytesStart,
    page_border_fill: &mut PageBorderFill,
) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"left" => page_border_fill.spacing_left = parse_i16(&attr),
            b"right" => page_border_fill.spacing_right = parse_i16(&attr),
            b"top" => page_border_fill.spacing_top = parse_i16(&attr),
            b"bottom" => page_border_fill.spacing_bottom = parse_i16(&attr),
            _ => {}
        }
    }
}

fn page_border_fill_attr(
    text_border: &str,
    fill_area: &str,
    apply_type: &str,
    header_inside: bool,
    footer_inside: bool,
) -> u32 {
    let mut attr = 0u32;

    if text_border.eq_ignore_ascii_case("PAPER") {
        attr |= 0x0000_0001;
    }
    if header_inside {
        attr |= 0x0000_0002;
    }
    if footer_inside {
        attr |= 0x0000_0004;
    }

    attr |= match fill_area {
        area if area.eq_ignore_ascii_case("PAGE") => 0x0000_0008,
        area if area.eq_ignore_ascii_case("BORDER") => 0x0000_0010,
        _ => 0,
    };

    attr
}

/// <hp:startNum> 요소 파싱
fn parse_start_num(e: &quick_xml::events::BytesStart, sec_def: &mut SectionDef) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"page" => sec_def.page_num = parse_u16(&attr),
            b"pic" => sec_def.picture_num = parse_u16(&attr),
            b"tbl" => sec_def.table_num = parse_u16(&attr),
            b"equation" => sec_def.equation_num = parse_u16(&attr),
            // 쪽 번호 시작 종류(0=이어서/1=홀수/2=짝수, flags bit20-21). 종전엔
            // 미독이라 HWPX 왕복 시 홀/짝 시작이 유실됐다(serializer 는 BOTH 고정).
            b"pageStartsOn" => {
                sec_def.page_num_type = match attr_str(&attr).as_str() {
                    "ODD" => 1,
                    "EVEN" => 2,
                    _ => 0,
                };
            }
            _ => {}
        }
    }
}

/// <hp:visibility> 요소 파싱
fn parse_visibility(e: &quick_xml::events::BytesStart, sec_def: &mut SectionDef) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"hideFirstHeader" => {
                sec_def.hide_header = attr_str(&attr) == "1";
                if sec_def.hide_header {
                    sec_def.flags |= 0x0001;
                } else {
                    sec_def.flags &= !0x0001;
                }
            }
            b"hideFirstFooter" => {
                sec_def.hide_footer = attr_str(&attr) == "1";
                if sec_def.hide_footer {
                    sec_def.flags |= 0x0002;
                } else {
                    sec_def.flags &= !0x0002;
                }
            }
            b"hideFirstMasterPage" => {
                sec_def.hide_master_page = attr_str(&attr) == "1";
                if sec_def.hide_master_page {
                    sec_def.flags |= 0x0004;
                } else {
                    sec_def.flags &= !0x0004;
                }
            }
            b"border" => {
                sec_def.hide_border = attr_str(&attr) == "HIDE_ALL";
                if sec_def.hide_border {
                    sec_def.flags |= 0x0008;
                } else {
                    sec_def.flags &= !0x0008;
                }
            }
            b"fill" => {
                sec_def.hide_fill = attr_str(&attr) == "HIDE_ALL";
                if sec_def.hide_fill {
                    sec_def.flags |= 0x0010;
                } else {
                    sec_def.flags &= !0x0010;
                }
            }
            b"hideFirstEmptyLine" => {
                sec_def.hide_empty_line = attr_str(&attr) == "1";
                if sec_def.hide_empty_line {
                    sec_def.flags |= 0x0008_0000;
                } else {
                    sec_def.flags &= !0x0008_0000;
                }
            }
            _ => {}
        }
    }
}

/// <hp:colPr> 요소의 속성 파싱 → ColumnDef
fn parse_col_pr(e: &quick_xml::events::BytesStart) -> ColumnDef {
    let mut cd = ColumnDef::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"type" => {
                cd.column_type = match attr_str(&attr).as_str() {
                    "NEWSPAPER" => ColumnType::Normal,
                    "BalancedNewspaper" => ColumnType::Distribute,
                    "Parallel" => ColumnType::Parallel,
                    _ => ColumnType::Normal,
                };
            }
            b"layout" => {
                cd.direction = match attr_str(&attr).as_str() {
                    "RIGHT" => ColumnDirection::RightToLeft,
                    _ => ColumnDirection::LeftToRight,
                };
            }
            b"colCount" => cd.column_count = parse_u16(&attr),
            b"sameSz" => cd.same_width = parse_u8(&attr) != 0,
            b"sameGap" => cd.spacing = parse_i16(&attr),
            _ => {}
        }
    }
    cd
}

/// <hp:colPr> 요소의 속성과 자식 <hp:colLine>/<hp:colSz> 파싱 → ColumnDef
fn parse_col_pr_with_children(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<ColumnDef, HwpxError> {
    let mut cd = parse_col_pr(e);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                match local_name(cname.as_ref()) {
                    b"colLine" => parse_col_line(ce, &mut cd),
                    b"colSz" => parse_col_sz(ce, &mut cd),
                    _ => {}
                }
            }
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"colLine" => {
                        parse_col_line(ce, &mut cd);
                        skip_element(reader, b"colLine")?;
                    }
                    b"colSz" => {
                        parse_col_sz(ce, &mut cd);
                        skip_element(reader, b"colSz")?;
                    }
                    _ => {
                        let tag = local.to_vec();
                        skip_element(reader, &tag)?;
                    }
                }
            }
            Ok(Event::End(ref ee)) => {
                if local_name(ee.name().as_ref()) == b"colPr" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("colPr: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(cd)
}


/// <hp:colSz width="..." gap="..."/> 파싱 → ColumnDef.widths/gaps (#4387).
///
/// `sameSz="false"` 일 때 단 개수만큼(최대 255) 반복되는 요소로, 단별 절대
/// HWPUNIT 너비·뒤 간격을 담는다 (`mydocs/manual/OWPML SCHEMA/ParaList XML
/// schema.xml:1415` ColumnDefType). HWPX 는 절대값이므로 `proportional_widths`
/// 는 건드리지 않는다 — `ColumnDef::default()` 의 `false` 가 이미 정답이다
/// (HWP 5.0 바이너리 파서(body_text.rs)만 비례값이라 true 로 켠다).
///
/// [#4387 후속] 스키마상 `width` 는 `xs:positiveInteger`(상한 없음)인데
/// `ColumnDef.widths/gaps: Vec<HwpUnit16>` 은 i16(최대 32767 HWPUNIT ≈
/// 115.6mm)이다. A3 등 큰 용지나 비대칭 다단(예: 35000+13000)처럼 실측치가
/// i16 범위를 넘으면 공용 `parse_i16`(무경고 0-폴백)이 조용히 0으로 떨어뜨려
/// 단이 통째로 사라진다 — IR 폭 확장은 HWP5 바이너리 경로까지 파급이 커
/// 이번 범위를 넘으므로, 대신 saturating 클램프로 "조용한 소실/부호反전"을
/// "포화값으로 잘림 + 경고"로 좁힌다. 근본 해결(IR 타입 확장)은 별도 이슈로
/// 추적한다.
fn parse_col_sz(e: &quick_xml::events::BytesStart, cd: &mut ColumnDef) {
    let mut width: HwpUnit16 = 0;
    let mut gap: HwpUnit16 = 0;
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"width" => width = parse_hwpunit16_saturating(&attr, "colSz@width"),
            // gap 은 스키마상 xs:nonNegativeInteger — 음수 폴백 없이 0 이상만 허용.
            b"gap" => gap = parse_hwpunit16_saturating(&attr, "colSz@gap").max(0),
            _ => {}
        }
    }
    cd.widths.push(width);
    cd.gaps.push(gap);
}

/// XML 정수 속성을 `HwpUnit16`(i16)로 saturating 변환한다.
///
/// 공용 `parse_i16`(utils.rs)은 `str::parse::<i16>()` 오버플로 시 무경고
/// `unwrap_or(0)`이라 `positiveInteger` 등 무제한 스키마 값이 i16 범위를 넘으면
/// 조용히 0이 된다(#4387 후속 — colSz 처럼 HWPX 가 절대 HWPUNIT 을 그대로
/// 담는 자리에서 실측 재현됨). i64 로 먼저 파싱해 i16 범위로 clamp 하고,
/// 실제로 잘렸을 때만 stderr 경고를 남긴다(section.rs 의 다른 속성 파서들과
/// 달리 이 값은 손실 시 단이 통째로 사라지는 시각적 결함으로 이어져 무음
/// 폴백이 특히 위험하다).
fn parse_hwpunit16_saturating(
    attr: &quick_xml::events::attributes::Attribute,
    field: &str,
) -> HwpUnit16 {
    let raw = attr_str(attr);
    match raw.parse::<i64>() {
        Ok(v) => {
            let clamped = v.clamp(HwpUnit16::MIN as i64, HwpUnit16::MAX as i64) as HwpUnit16;
            if clamped as i64 != v {
                eprintln!(
                    "경고: {} 값 {} 이(가) HwpUnit16 범위를 초과해 {} 로 잘렸습니다",
                    field, v, clamped
                );
            }
            clamped
        }
        Err(_) => 0,
    }
}


/// <hp:t> 텍스트 컨텐츠를 읽는다.
/// 탭 확장 데이터도 함께 반환 (HWPX 인라인 탭의 leader/type/width)
fn read_text_content(reader: &mut Reader<&[u8]>) -> Result<String, HwpxError> {
    let (text, _) = read_text_content_with_tabs(reader)?;
    Ok(text)
}

fn decode_xml_general_ref(r: &BytesRef<'_>) -> String {
    if let Ok(Some(ch)) = r.resolve_char_ref() {
        return ch.to_string();
    }

    let name = r.decode().unwrap_or_default();
    match name.as_ref() {
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "amp" => "&".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        _ => format!("&{};", name),
    }
}

fn read_text_content_with_tabs(
    reader: &mut Reader<&[u8]>,
) -> Result<(String, Vec<[u16; 7]>), HwpxError> {
    let mut text = String::new();
    let mut tab_ext_buf: Vec<[u16; 7]> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref t)) => {
                text.push_str(&t.decode().unwrap_or_default());
            }
            // 본문 런 텍스트가 CDATA 로 저장된 경우. 이 분기가 없으면 `_ => {}` 로
            // 버려져 문단 텍스트가 통째로 소실된다(#2916·#2951·#2974 와 같은 결함
            // 클래스이나, 여기는 수식·덧말이 아닌 일반 <hp:t> 경로다).
            Ok(Event::CData(ref cdata)) => {
                text.push_str(&String::from_utf8_lossy(cdata.as_ref()));
            }
            Ok(Event::GeneralRef(ref r)) => {
                text.push_str(&decode_xml_general_ref(r));
            }
            Ok(Event::End(ref e)) => {
                let tn = e.name();
                if local_name(tn.as_ref()) == b"t" {
                    break;
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"lineBreak" | b"columnBreak" => text.push('\n'),
                    b"tab" => {
                        text.push('\t');
                        // "데이터 없음" 마커(width=0, #4403)는 tab_extended 에 싣지 않는다 —
                        // 렌더러가 TabDef 기준으로 다시 계산하도록 원본처럼 비워 둔다.
                        let ext = parse_tab_extension(ce);
                        if !is_tab_no_data_marker(&ext) {
                            tab_ext_buf.push(ext);
                        }
                    }
                    b"nbSpace" => text.push('\u{00A0}'),
                    b"fwSpace" => text.push('\u{2007}'),
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("text: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok((text, tab_ext_buf))
}

fn parse_tab_extension(e: &quick_xml::events::BytesStart) -> [u16; 7] {
    let mut ext = [0u16; 7];
    ext[6] = 0x0009;
    let mut leader = 0u16;
    let mut tab_type = 0u16;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"width" => ext[0] = parse_u16(&attr),
            b"leader" => leader = parse_u16(&attr) & 0x00ff,
            b"type" => tab_type = parse_u16(&attr) & 0x00ff,
            _ => {}
        }
    }
    ext[2] = (tab_type << 8) | leader;

    ext
}

/// `<hp:tab width="0" leader="0" type="1"/>` — 서식기(`serializer/hwpx/section.rs`
/// `TAB_NO_DATA_WIDTH_MARKER`)가 `tab_extended` 항목이 없던 "암묵적 기본 탭"을 내보낼 때
/// 쓰는 정확한 마커다(#4403). 실제 탭은 폭 0 이 나올 수 없으므로(시각적으로 아무 효과가 없어
/// 한컴도 만들지 않는다) 안전한 신호로 쓴다. `leader`/`type` 까지 우리 서식기의 고정 폴백값과
/// 정확히 일치할 때만 마커로 인정해, width=0 인 (극히 드문) 진짜 캡처 데이터를 오인해 버리지
/// 않도록 한다. 이 마커를 만나면 `tab_extended` 에 항목을 추가하지 않아, 렌더러가 문단의 실제
/// `TabDef`/커서 위치 기준 `find_next_tab_stop` 으로 탭 정지를 다시 계산하게 한다 — HWP5
/// 바이너리 파서의 동형 널 마커 스킵(`parser/body_text.rs` `is_null_ext`, #1892)과 같은 규약.
fn is_tab_no_data_marker(ext: &[u16; 7]) -> bool {
    ext[0] == 0 && ext[2] == 0x0100
}

// ─── Table ───


fn parse_size_criterion(value: &str, allow_column_para: bool) -> SizeCriterion {
    match value {
        "PAPER" => SizeCriterion::Paper,
        "PAGE" => SizeCriterion::Page,
        "COLUMN" if allow_column_para => SizeCriterion::Column,
        "PARA" if allow_column_para => SizeCriterion::Para,
        _ => SizeCriterion::Absolute,
    }
}

fn materialize_hwpx_table_attrs(table: &mut Table, table_record_flags: u32) {
    const HWPX_TABLE_NUMBERING_BIT: u32 = 0x0800_0000;

    // [#2697] "표 번호" 비트는 numberingType 이 실제로 TABLE 일 때만 세운다. 종전 무조건 OR
    // 은 numberingType="PICTURE" 표에서 IR 모순(numbering_type=Picture ↔ attr=TABLE)을 만든다.
    // 차트 파서(5800)가 PICTURE 를 별도 비트로 분기하는 것과 같은 취지.
    let mut attr = pack_hwpx_common_obj_attr(&table.common);
    if table.common.numbering_type == crate::model::shape::ObjectNumberingType::Table {
        attr |= HWPX_TABLE_NUMBERING_BIT;
    }
    table.common.attr = attr;
    // HWPX keeps semantic placement in hp:pos, while legacy layout code still reads
    // table.attr bit0 for some inline-table decisions. Only mirror the minimum
    // renderer compatibility bit here; the HWP5 storage attr is packed later by
    // the HWP adapter.
    table.attr = if table.common.treat_as_char && table.common.flow_with_text {
        0x01
    } else {
        0
    };
    let mut record_attr = match table.page_break {
        TablePageBreak::CellBreak => 0x01,
        TablePageBreak::RowBreak => 0x02,
        TablePageBreak::None => 0,
    };
    if table.repeat_header {
        record_attr |= 0x04;
    }
    if table_record_flags & 0x08 != 0 {
        record_attr |= 0x08;
    }
    if table.padding.left != 0
        || table.padding.right != 0
        || table.padding.top != 0
        || table.padding.bottom != 0
    {
        record_attr |= 0x0400_0000;
    }
    table.raw_table_record_attr = record_attr;
}

fn pack_hwpx_common_obj_attr(common: &CommonObjAttr) -> u32 {
    let mut attr = 0u32;
    if common.treat_as_char {
        attr |= 0x01;
    }
    if common.flow_with_text {
        attr |= 1 << 13;
    }
    if common.allow_overlap {
        attr |= 1 << 14;
    }
    if common.size_protect {
        attr |= 1 << 20;
    }
    if common.hwp5_gen_shape_attr_bit26 {
        attr |= 1 << 26;
    }
    if common.hwp5_gen_shape_attr_bit28 {
        attr |= 1 << 28;
    }

    attr |= (match common.vert_rel_to {
        VertRelTo::Paper => 0,
        VertRelTo::Page => 1,
        VertRelTo::Para => 2,
    }) << 3;
    attr |= (match common.vert_align {
        VertAlign::Top => 0,
        VertAlign::Center => 1,
        VertAlign::Bottom => 2,
        VertAlign::Inside => 3,
        VertAlign::Outside => 4,
    }) << 5;
    attr |= (match common.horz_rel_to {
        HorzRelTo::Paper => 0,
        HorzRelTo::Page => 1,
        HorzRelTo::Column => 2,
        HorzRelTo::Para => 3,
    }) << 8;
    attr |= (match common.horz_align {
        HorzAlign::Left => 0,
        HorzAlign::Center => 1,
        HorzAlign::Right => 2,
        HorzAlign::Inside => 3,
        HorzAlign::Outside => 4,
    }) << 10;
    attr |= (match common.width_criterion {
        SizeCriterion::Paper => 0,
        SizeCriterion::Page => 1,
        SizeCriterion::Column => 2,
        SizeCriterion::Para => 3,
        SizeCriterion::Absolute => 4,
    }) << 15;
    attr |= (match common.height_criterion {
        SizeCriterion::Paper => 0,
        SizeCriterion::Page => 1,
        _ => 2,
    }) << 18;
    attr |= (match common.text_wrap {
        TextWrap::Square | TextWrap::Tight | TextWrap::Through => 0,
        TextWrap::TopAndBottom => 1,
        TextWrap::BehindText => 2,
        TextWrap::InFrontOfText => 3,
    }) << 21;
    attr |= (match common.text_flow {
        crate::model::shape::TextFlow::BothSides => 0,
        crate::model::shape::TextFlow::LeftOnly => 1,
        crate::model::shape::TextFlow::RightOnly => 2,
        crate::model::shape::TextFlow::LargestOnly => 3,
    }) << 24;

    attr
}


/// `<hp:caption>` 파싱 — 표(#1387)·그림/도형/묶음(#1403) 공유.
fn parse_caption(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<crate::model::shape::Caption, HwpxError> {
    use crate::model::shape::{Caption, CaptionDirection};

    let mut caption = Caption::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"side" => {
                caption.direction = match attr_str(&attr).as_str() {
                    "LEFT" => CaptionDirection::Left,
                    "RIGHT" => CaptionDirection::Right,
                    "TOP" => CaptionDirection::Top,
                    "BOTTOM" => CaptionDirection::Bottom,
                    _ => CaptionDirection::Bottom,
                };
            }
            b"gap" => caption.spacing = parse_i16(&attr),
            b"width" => caption.width = parse_i32(&attr) as u32,
            b"lastWidth" => caption.max_width = parse_i32(&attr) as u32,
            b"fullSz" => caption.include_margin = attr_str(&attr) == "1",
            _ => {}
        }
    }

    // subList 내 문단 파싱
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"subList" => parse_caption_sub_list_attrs(ce, &mut caption),
                    b"p" => {
                        let (para, _) = parse_paragraph(ce, reader)?;
                        caption.paragraphs.push(para);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref ce)) if local_name(ce.name().as_ref()) == b"subList" => {
                parse_caption_sub_list_attrs(ce, &mut caption);
            }
            Ok(Event::End(ref end)) => {
                if local_name(end.name().as_ref()) == b"caption" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("caption: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(caption)
}


// ─── Picture ───


fn parse_effect_point(e: &quick_xml::events::BytesStart<'_>) -> EffectPoint {
    let mut point = EffectPoint::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"x" => point.x = Some(attr_str(&attr)),
            b"y" => point.y = Some(attr_str(&attr)),
            _ => {}
        }
    }
    point
}

fn parse_effect_color(
    e: &quick_xml::events::BytesStart<'_>,
    reader: &mut Reader<&[u8]>,
) -> Result<EffectColor, HwpxError> {
    let mut color = parse_effect_color_attrs(e);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) if local_name(e.name().as_ref()) == b"rgb" => {
                color.rgb = Some(parse_effect_rgb(e));
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"effectsColor" => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("effectsColor: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(color)
}

fn parse_effect_color_attrs(e: &quick_xml::events::BytesStart<'_>) -> EffectColor {
    let mut color = EffectColor::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"type" => color.color_type = Some(attr_str(&attr)),
            b"schemeIdx" => color.scheme_idx = Some(attr_str(&attr)),
            b"systemIdx" => color.system_idx = Some(attr_str(&attr)),
            b"presetIdx" => color.preset_idx = Some(attr_str(&attr)),
            _ => {}
        }
    }
    color
}

fn parse_effect_rgb(e: &quick_xml::events::BytesStart<'_>) -> EffectRgb {
    let mut rgb = EffectRgb::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"r" => rgb.r = Some(attr_str(&attr)),
            b"g" => rgb.g = Some(attr_str(&attr)),
            b"b" => rgb.b = Some(attr_str(&attr)),
            _ => {}
        }
    }
    rgb
}

// ─── 그리기 객체 공통 속성 파싱 ───


/// `<hp:renderingInfo>` 파싱.
///
/// HWP5 SHAPE_COMPONENT는 rendering block을 `cnt + transMatrix + cnt개의
/// (scaMatrix, rotMatrix)` 형태로 저장한다. HWPX source에도 같은 matrix sequence가
/// 있으므로, 합성된 affine 값과 함께 HWP5 writer가 그대로 사용할 raw_rendering도 보존한다.
///
/// HWPX 구조:
/// ```xml
/// <hp:renderingInfo>
///   <hp:transMatrix e1 e2 e3 e4 e5 e6/>   ← 이동
///   <hp:scaMatrix e1 e2 e3 e4 e5 e6/>     ← 스케일
///   <hp:rotMatrix e1 e2 e3 e4 e5 e6/>     ← 회전
///   ... (sca/rot 쌍이 추가될 수 있음)
/// </hp:renderingInfo>
/// ```
///
/// 행렬 [a, b, tx, c, d, ty] → (x',y') = (a*x+b*y+tx, c*x+d*y+ty)
/// 합성 순서: HWP 바이너리와 동일하게 trans × rot × sca
fn parse_rendering_info(
    reader: &mut Reader<&[u8]>,
    shape_attr: &mut ShapeComponentAttr,
) -> Result<(), HwpxError> {
    fn hwp5_matrix_value(raw: f64) -> f64 {
        if raw.fract() == 0.0 {
            raw
        } else {
            f64::from(raw as f32)
        }
    }

    // 행렬 값 파싱 헬퍼
    fn read_matrix(ce: &quick_xml::events::BytesStart) -> [f64; 6] {
        let mut m = [0.0f64; 6];
        for attr in ce.attributes().flatten() {
            let val: f64 = attr_str(&attr)
                .parse()
                .map(hwp5_matrix_value)
                .unwrap_or(0.0);
            match attr.key.as_ref() {
                b"e1" => m[0] = val,
                b"e2" => m[1] = val,
                b"e3" => m[2] = val,
                b"e4" => m[3] = val,
                b"e5" => m[4] = val,
                b"e6" => m[5] = val,
                _ => {}
            }
        }
        m
    }
    // 아핀 행렬 합성: result = A × B
    fn compose(a: &[f64; 6], b: &[f64; 6]) -> [f64; 6] {
        [
            a[0] * b[0] + a[1] * b[3],        // a
            a[0] * b[1] + a[1] * b[4],        // b
            a[0] * b[2] + a[1] * b[5] + a[2], // tx
            a[3] * b[0] + a[4] * b[3],        // c
            a[3] * b[1] + a[4] * b[4],        // d
            a[3] * b[2] + a[4] * b[5] + a[5], // ty
        ]
    }
    fn push_matrix_le(out: &mut Vec<u8>, matrix: &[f64; 6]) {
        for value in matrix {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    fn make_raw_rendering(trans: &[f64; 6], pairs: &[([f64; 6], [f64; 6])]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(2 + 48 + pairs.len() * 96);
        raw.extend_from_slice(&(pairs.len() as u16).to_le_bytes());
        push_matrix_le(&mut raw, trans);
        for (sca, rot) in pairs {
            push_matrix_le(&mut raw, sca);
            push_matrix_le(&mut raw, rot);
        }
        raw
    }

    let mut buf = Vec::new();
    let mut trans = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]; // identity
    let mut sca_rot_pairs: Vec<([f64; 6], [f64; 6])> = Vec::new();
    let mut pending_sca: Option<[f64; 6]> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"transMatrix" => trans = read_matrix(ce),
                    b"scaMatrix" => {
                        pending_sca = Some(read_matrix(ce));
                    }
                    b"rotMatrix" => {
                        let rot = read_matrix(ce);
                        let sca = pending_sca.take().unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
                        sca_rot_pairs.push((sca, rot));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                if local_name(ee.name().as_ref()) == b"renderingInfo" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("renderingInfo: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // sca만 있고 rot이 없는 경우 처리
    if let Some(sca) = pending_sca {
        sca_rot_pairs.push((sca, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
    }

    // HWP 바이너리와 동일한 합성: result = trans, 그 후 각 쌍마다 result = result × rot × sca
    let mut result = trans;
    for (sca, rot) in &sca_rot_pairs {
        result = compose(&result, rot);
        result = compose(&result, sca);
    }

    shape_attr.render_sx = result[0]; // a
    shape_attr.render_b = result[1]; // b (회전/전단)
    shape_attr.render_tx = result[2]; // tx
    shape_attr.render_c = result[3]; // c (회전/전단)
    shape_attr.render_sy = result[4]; // d
    shape_attr.render_ty = result[5]; // ty
    shape_attr.raw_rendering = make_raw_rendering(&trans, &sca_rot_pairs);

    Ok(())
}


fn parse_connect_line_type_attr(e: &quick_xml::events::BytesStart) -> LinkLineType {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"type" {
            return match attr_str(&attr).to_ascii_uppercase().as_str() {
                "STRAIGHT_ONEWAY" => LinkLineType::StraightOneWay,
                "STRAIGHT_BOTH" => LinkLineType::StraightBoth,
                "STROKE_NOARROW" => LinkLineType::StrokeNoArrow,
                "STROKE_ONEWAY" => LinkLineType::StrokeOneWay,
                "STROKE_BOTH" => LinkLineType::StrokeBoth,
                "ARC_NOARROW" => LinkLineType::ArcNoArrow,
                "ARC_ONEWAY" => LinkLineType::ArcOneWay,
                "ARC_BOTH" => LinkLineType::ArcBoth,
                _ => LinkLineType::StraightNoArrow,
            };
        }
    }

    LinkLineType::StraightNoArrow
}

/// [#4388] `<hp:arc>` 전용 `type` 속성 (OWPML `CArcType::WriteElement` —
/// hancom-io/hwpx-owpml-model `ArcType.cpp`) — `g_ArcTypeList`: NORMAL(0)/PIE(1)/CHORD(2).
/// `ArcShape.arc_type` (0: Arc, 1: CircularSector, 2: Bow) 와 1:1 대응. 같은 이름의
/// `type` 속성이 `<hp:connectLine>`(연결선 화살표 종류) 등 다른 도형 태그에도 쓰이므로
/// 반드시 `<hp:arc>` 요소 자체(shape_type == b"arc")에서만 호출한다.
fn parse_arc_type_attr(e: &quick_xml::events::BytesStart) -> u8 {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"type" {
            return match attr_str(&attr).to_ascii_uppercase().as_str() {
                "PIE" => 1,
                "CHORD" => 2,
                _ => 0,
            };
        }
    }
    0
}


/// [Task #1598] `<hc:center x="" y="">` 류 점 요소의 x/y 속성을 Point 로 읽는다.
fn parse_xy(e: &quick_xml::events::BytesStart, p: &mut crate::model::Point) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"x" => p.x = parse_i32(&attr),
            b"y" => p.y = parse_i32(&attr),
            _ => {}
        }
    }
}


/// `<hp:drawText>` 내부의 `<hp:subList>` → `<hp:p>` 문단을 파싱한다.
fn parse_draw_text(reader: &mut Reader<&[u8]>, text_box: &mut TextBox) -> Result<(), HwpxError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"subList" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"vertAlign" => {
                                    let align_code = match attr_str(&attr).as_str() {
                                        "CENTER" => 1_u32,
                                        "BOTTOM" => 2_u32,
                                        _ => 0_u32,
                                    };
                                    text_box.vertical_align = match align_code {
                                        1 => VerticalAlign::Center,
                                        2 => VerticalAlign::Bottom,
                                        _ => VerticalAlign::Top,
                                    };
                                    text_box.list_attr =
                                        (text_box.list_attr & !(0b11 << 5)) | (align_code << 5);
                                }
                                // [Task #1028] HWPX 글상자 세로쓰기 (textDirection)
                                // 파싱. HWP5 LIST_HEADER 의 list_attr bit 0~2
                                // (text_direction) 영역에 set — renderer 의
                                // shape_layout.rs:1652 `(list_attr & 0x07)` 분기
                                // 가 세로쓰기 (`layout_vertical_textbox_text_with_paras`)
                                // 활성화. "VERTICAL"/"VERTICALALL" 모두 code 1.
                                b"textDirection" => {
                                    let dir = attr_str(&attr);
                                    let direction_code: u32 = match dir.as_str() {
                                        "VERTICAL" | "VERTICALALL" => 1,
                                        _ => 0,
                                    };
                                    text_box.list_attr =
                                        (text_box.list_attr & !0b111) | direction_code;
                                    // [Task #1379] VERTICAL/VERTICALALL 구분 보존
                                    // — serializer 역방출용 (list_attr 만으로는 구분 불가).
                                    text_box.vertical_all = dir == "VERTICALALL";
                                }
                                _ => {}
                            }
                        }
                    }
                    b"p" => {
                        // subList 내 p를 독립 파싱
                        let (para, _) = parse_paragraph(ce, reader)?;
                        text_box.paragraphs.push(para);
                    }
                    b"textMargin" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => text_box.margin_left = parse_i16(&attr),
                                b"right" => text_box.margin_right = parse_i16(&attr),
                                b"top" => text_box.margin_top = parse_i16(&attr),
                                b"bottom" => text_box.margin_bottom = parse_i16(&attr),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"drawText" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("drawText: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

// ─── 그리기 객체 파싱 (rect, ellipse, line, arc, polygon, curve) ───


// ─── 묶음(그룹) 객체 파싱 ───

/// `<hp:container>` 요소를 파싱하여 `Control::Shape(GroupShape)`를 반환한다.
fn parse_container(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut common = CommonObjAttr::default();
    let mut shape_attr = ShapeComponentAttr::default();
    let mut has_pos = false;
    let mut children = Vec::new();

    parse_object_element_attrs(e, &mut common, &mut shape_attr);

    let mut caption: Option<crate::model::shape::Caption> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            // 묶음 개체 캡션 (#1403) — 미적재 시 roundtrip 에서 캡션 subList 소실
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"caption" => {
                caption = Some(parse_caption(ce, reader)?);
            }
            // 묶음 개체 설명 (#1392) — 미적재 시 roundtrip 에서 소실
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"shapeComment" => {
                common.description = read_dutmal_text(reader, b"shapeComment")?;
            }
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"sz" | b"curSz" | b"orgSz" | b"pos" | b"offset" | b"outMargin" | b"flip"
                    | b"rotationInfo" => {
                        parse_object_layout_child(
                            local,
                            ce,
                            &mut common,
                            &mut shape_attr,
                            &mut has_pos,
                        );
                    }
                    b"pic" => {
                        // 자식 그림 객체
                        let child = parse_picture(ce, reader)?;
                        if let Control::Picture(pic) = child {
                            children.push(ShapeObject::Picture(pic));
                        }
                    }
                    b"rect" | b"ellipse" | b"line" | b"connectLine" | b"arc" | b"polygon"
                    | b"curve" => {
                        // 자식 그리기 객체
                        let child = parse_shape_object(local, ce, reader)?;
                        if let Control::Shape(shape) = child {
                            children.push(*shape);
                        }
                    }
                    b"container" => {
                        // 중첩 그룹
                        let child = parse_container(ce, reader)?;
                        if let Control::Shape(shape) = child {
                            children.push(*shape);
                        }
                    }
                    b"renderingInfo" => {
                        parse_rendering_info(reader, &mut shape_attr)?;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"container" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("container: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    materialize_shape_hwp_storage_defaults(&mut common, &mut shape_attr, ShapeStorageKind::Group);

    let group = GroupShape {
        common,
        shape_attr,
        children,
        caption,
    };

    Ok(Control::Shape(Box::new(ShapeObject::Group(group))))
}

// ─── <hp:ctrl> 파싱 ───

/// `<hp:ctrl>` 내부 자식 요소를 파싱하여 해당 컨트롤을 추가한다.
/// ForChars.java 매핑 기준: header, footer, footNote, endNote, autoNum, newNum,
/// pageHiding, pageNum, bookmark, hiddenComment, fieldBegin, fieldEnd, colPr
fn parse_ctrl(
    _e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
    controls: &mut Vec<Control>,
    text_parts: &mut Vec<String>,
    field_end_attrs: &mut Vec<(u32, u32)>,
) -> Result<(), HwpxError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"colPr" => {
                        let cd = parse_col_pr_with_children(ce, reader)?;
                        controls.push(Control::ColumnDef(cd));
                        // [Task #901] ColumnDef 도 8 utf16 inline marker (HWP 정합).
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"header" => {
                        let ctrl = parse_ctrl_header(ce, reader)?;
                        controls.push(ctrl);
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"footer" => {
                        let ctrl = parse_ctrl_footer(ce, reader)?;
                        controls.push(ctrl);
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"footNote" => {
                        let ctrl = parse_ctrl_footnote(ce, reader)?;
                        controls.push(ctrl);
                        // [Task #1050] HWP 정합 — extended ctrl: 8 code unit (16 byte) 차지만
                        // text/char_offsets 에는 placeholder 미 push.
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"endNote" => {
                        let ctrl = parse_ctrl_endnote(ce, reader)?;
                        controls.push(ctrl);
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"autoNum" => {
                        let ctrl = parse_ctrl_autonum(ce, reader)?;
                        controls.push(ctrl);
                        // [Task #1050] AUTO_NUMBER (0x12) 는 HWP PARA_TEXT 에서:
                        //   char_offsets.push(pos) + text.push(' ') + pos += 8 (16 byte)
                        // 본 컨트롤은 placeholder space 1 char 점하고 jump 8 처리.
                        // \u{0012} 표시자 사용 — 후속 visual_text 조립 단계에서 처리.
                        text_parts.push("\u{0012}".to_string());
                    }
                    b"hiddenComment" => {
                        let ctrl = parse_ctrl_hidden_comment(reader)?;
                        controls.push(ctrl);
                    }
                    b"fieldBegin" => {
                        let ctrl = parse_ctrl_field_begin(ce, reader)?;
                        controls.push(ctrl);
                        // FIELD_BEGIN 제어 문자 추가 (Task #11)
                        text_parts.push("\u{0003}".to_string());
                    }
                    b"fieldEnd" => {
                        // [Task #1556] beginIDRef/fieldid 포착 (고아 fieldEnd 복원용).
                        field_end_attrs.push(parse_field_end_attrs(ce));
                        skip_element(reader, b"fieldEnd")?;
                        // FIELD_END 제어 문자 추가 (Task #11)
                        text_parts.push("\u{0004}".to_string());
                    }
                    b"pageHiding" => {
                        let ph = parse_page_hiding_attrs(ce);
                        controls.push(Control::PageHide(ph));
                        text_parts.push("\u{0002}".to_string());
                        skip_element(reader, b"pageHiding")?;
                    }
                    b"pageNum" => {
                        let pn = parse_page_num_attrs(ce);
                        controls.push(Control::PageNumberPos(pn));
                        text_parts.push("\u{0002}".to_string());
                        skip_element(reader, b"pageNum")?;
                    }
                    b"bookmark" => {
                        let bm = parse_bookmark_attrs(ce);
                        controls.push(Control::Bookmark(bm));
                        skip_element(reader, b"bookmark")?;
                    }
                    b"newNum" => {
                        let nn = parse_new_num_attrs(ce);
                        controls.push(Control::NewNumber(nn));
                        // HWPX newNum is an inline page-control marker in HWP5
                        // PARA_TEXT. It occupies 8 UTF-16 code units like
                        // pageHiding, but it must not synthesize a visible
                        // placeholder space; that behavior is only for autoNum.
                        text_parts.push("\u{0002}".to_string());
                        skip_element(reader, b"newNum")?;
                    }
                    _ => {
                        let tag = local.to_vec();
                        skip_element(reader, &tag)?;
                    }
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"colPr" => {
                        let cd = parse_col_pr(ce);
                        controls.push(Control::ColumnDef(cd));
                        // [Task #901] ColumnDef 도 8 utf16 inline marker (HWP 정합).
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"pageHiding" => {
                        let ph = parse_page_hiding_attrs(ce);
                        controls.push(Control::PageHide(ph));
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"pageNum" => {
                        let pn = parse_page_num_attrs(ce);
                        controls.push(Control::PageNumberPos(pn));
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"bookmark" => {
                        let bm = parse_bookmark_attrs(ce);
                        controls.push(Control::Bookmark(bm));
                    }
                    b"newNum" => {
                        let nn = parse_new_num_attrs(ce);
                        controls.push(Control::NewNumber(nn));
                        // See the Start branch above. Without this marker the
                        // following pageHiding/header controls drift behind the
                        // visible text when saved back to HWP5.
                        text_parts.push("\u{0002}".to_string());
                    }
                    b"autoNum" => {
                        let an = parse_autonum_attrs(ce);
                        controls.push(Control::AutoNumber(an));
                        // [Task #1050] AUTO_NUMBER inline (Empty 분기): placeholder space.
                        text_parts.push("\u{0012}".to_string());
                    }
                    b"fieldBegin" => {
                        let f = parse_field_begin_attrs(ce);
                        controls.push(Control::Field(f));
                        text_parts.push("\u{0003}".to_string());
                    }
                    b"fieldEnd" => {
                        // [Task #1556] 자기닫힘 fieldEnd — beginIDRef/fieldid 포착.
                        field_end_attrs.push(parse_field_end_attrs(ce));
                        text_parts.push("\u{0004}".to_string());
                    }
                    b"hiddenComment" => {}
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"ctrl" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("ctrl: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

// ─── ctrl 자식 요소 속성 파싱 헬퍼 ───

fn parse_bool_attr(attr: &quick_xml::events::attributes::Attribute) -> bool {
    let s = attr_str(attr);
    s == "1" || s == "true"
}

/// `<hp:fieldEnd beginIDRef=".." fieldid="..">` 속성 → (begin_id_ref, field_id) (Task #1556).
fn parse_field_end_attrs(e: &quick_xml::events::BytesStart) -> (u32, u32) {
    let mut begin_id_ref = 0u32;
    let mut field_id = 0u32;
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"beginIDRef" => begin_id_ref = parse_u32(&attr),
            b"fieldid" => field_id = parse_u32(&attr),
            _ => {}
        }
    }
    (begin_id_ref, field_id)
}

fn parse_page_hiding_attrs(e: &quick_xml::events::BytesStart) -> PageHide {
    let mut ph = PageHide::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"hideHeader" => ph.hide_header = parse_bool_attr(&attr),
            b"hideFooter" => ph.hide_footer = parse_bool_attr(&attr),
            b"hideMasterPage" => ph.hide_master_page = parse_bool_attr(&attr),
            b"hideBorder" => ph.hide_border = parse_bool_attr(&attr),
            b"hideFill" => ph.hide_fill = parse_bool_attr(&attr),
            b"hidePageNum" => ph.hide_page_num = parse_bool_attr(&attr),
            _ => {}
        }
    }
    ph
}

fn parse_page_num_attrs(e: &quick_xml::events::BytesStart) -> PageNumberPos {
    let mut pn = PageNumberPos::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"pos" => {
                pn.position = match attr_str(&attr).as_str() {
                    "NONE" => 0,
                    "TOP_LEFT" => 1,
                    "TOP_CENTER" => 2,
                    "TOP_RIGHT" => 3,
                    "BOTTOM_LEFT" => 4,
                    "BOTTOM_CENTER" => 5,
                    "BOTTOM_RIGHT" => 6,
                    "OUTSIDE_TOP" => 7,
                    "OUTSIDE_BOTTOM" => 8,
                    "INSIDE_TOP" => 9,
                    "INSIDE_BOTTOM" => 10,
                    _ => 5, // 기본: 가운데 아래
                };
            }
            b"formatType" => {
                pn.format = match attr_str(&attr).as_str() {
                    "DIGIT" => 0,
                    // [#XXXX] 스펙 표기는 "CIRCLED_DIGIT"(NumberType1). 과거 오탈자
                    // "CIRCLE_DIGIT"로 저장된 한컴 실물 파일과의 호환을 위해 둘 다 인식한다.
                    "CIRCLED_DIGIT" | "CIRCLE_DIGIT" => 1,
                    "ROMAN_CAPITAL" => 2,
                    "ROMAN_SMALL" => 3,
                    "LATIN_CAPITAL" => 4,
                    "LATIN_SMALL" => 5,
                    "HANGUL" => 6,
                    "HANJA" => 7,
                    _ => 0,
                };
            }
            b"sideChar" => {
                let s = attr_str(&attr);
                pn.dash_char = s.chars().next().unwrap_or('-');
            }
            _ => {}
        }
    }
    pn
}

fn parse_bookmark_attrs(e: &quick_xml::events::BytesStart) -> Bookmark {
    let mut bm = Bookmark::default();
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"name" {
            bm.name = attr_str(&attr);
        }
    }
    bm
}

fn parse_new_num_attrs(e: &quick_xml::events::BytesStart) -> NewNumber {
    let mut nn = NewNumber::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"num" => nn.number = parse_u16(&attr),
            b"numType" => nn.number_type = parse_num_type(&attr_str(&attr)),
            _ => {}
        }
    }
    nn
}

fn parse_autonum_attrs(e: &quick_xml::events::BytesStart) -> AutoNumber {
    let mut an = AutoNumber::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"num" => {
                an.number = parse_u16(&attr);
                an.assigned_number = an.number;
            }
            b"numType" => an.number_type = parse_num_type(&attr_str(&attr)),
            _ => {}
        }
    }
    an
}

fn parse_field_begin_attrs(e: &quick_xml::events::BytesStart) -> Field {
    let mut f = Field::default();
    let mut field_name: Option<String> = None;
    let mut id_attr: Option<u32> = None;
    let mut fieldid_attr: Option<u32> = None;
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"type" => f.field_type = parse_field_type(&attr_str(&attr)),
            b"name" => field_name = Some(attr_str(&attr)),
            // [Task #852 Stage 2.5] HWP5 직렬화에 필요한 필드 메타
            b"id" => {
                if let Ok(v) = attr_str(&attr).parse::<u32>() {
                    id_attr = Some(v);
                }
            }
            b"fieldid" => {
                if let Ok(v) = attr_str(&attr).parse::<u32>() {
                    // fieldid (instance ID) — 정답지의 CTRL_HEADER 끝에 저장
                    fieldid_attr = Some(v);
                }
            }
            b"editable" => {
                // properties bit 0 = editable in form
                if attr_str(&attr) == "1" {
                    f.properties |= 1;
                }
            }
            b"dirty" => {
                // properties bit 15 = 수정됨 표식 — 버리면 clear_initial_field_texts 의
                // 보존 게이트(#3380)가 HWPX 축에서 항상 열려 텍스트가 유실된다 (#3545)
                if parse_bool_attr(&attr) {
                    f.properties |= 1 << 15;
                }
            }
            _ => {}
        }
    }
    // field_id 는 필드별 고유 식별자여야 한다(모델 계약 "문서 내 고유 ID").
    // OWPML `id` 가 필드마다 고유하고, `<hp:fieldEnd beginIDRef>` 가 이 `id` 를
    // 참조하며, 직렬화도 `id="{field_id}"` 로 쓴다. 반면 `fieldid` 는 같은 종류 필드
    // (예: FORMULA 다수)에서 공유될 수 있어, 이를 우선하면 모든 필드가 동일 ID 로
    // 반환된다(#1512). Memo/비-Memo 모두 고유 `id` 우선으로 통일한다.
    f.field_id = id_attr.or(fieldid_attr).unwrap_or(0);
    // [#task-m100] `fieldid` 는 위 field_id 계산에 폴백으로만 쓰였고, `id` 가 존재하는
    // 실물 필드(예: id=1878228493, fieldid=627272811 — 서로 다름)에선 원본 fieldid 값이
    // 그대로 버려져 직렬화기가 이 속성을 영구히 방출하지 못했다. instance_id 로 별도 보존.
    f.instance_id = fieldid_attr;
    // [Task #852 Stage 2.5] field_type → ctrl_id 매핑.
    // 정답지 (samples/form-01.hwp) reverse engineering: ClickHere CTRL_HEADER 의 ctrl_id 가
    // "%clk" (FIELD_CLICKHERE). HWPX parser 가 이전엔 ctrl_id 미설정 → serializer 가
    // 0x00000000 작성 → 한컴이 무효 컨트롤로 인식 (JS 핸들러 reference 끊김).
    f.ctrl_id = match f.field_type {
        FieldType::Date => tags::FIELD_DATE,
        FieldType::DocDate => tags::FIELD_DOCDATE,
        FieldType::Path => tags::FIELD_PATH,
        FieldType::Bookmark => tags::FIELD_BOOKMARK,
        FieldType::MailMerge => tags::FIELD_MAILMERGE,
        FieldType::CrossRef => tags::FIELD_CROSSREF,
        FieldType::Formula => tags::FIELD_FORMULA,
        FieldType::ClickHere => tags::FIELD_CLICKHERE,
        FieldType::Summary => tags::FIELD_SUMMARY,
        FieldType::UserInfo => tags::FIELD_USERINFO,
        FieldType::Hyperlink => tags::FIELD_HYPERLINK,
        FieldType::Memo => tags::FIELD_MEMO,
        FieldType::PrivateInfoSecurity => tags::FIELD_PRIVATE_INFO,
        FieldType::TableOfContents => tags::FIELD_TOC,
        FieldType::Unknown => 0,
    };
    // ClickHere 의 extra_properties 정답지 관찰값: 0x09
    if matches!(f.field_type, FieldType::ClickHere) {
        f.extra_properties = 0x09;
    }
    // command 가 비어있으면 fieldBegin 의 name 사용 (CTRL_DATA name 으로도 활용)
    if f.command.is_empty() {
        if let Some(name) = field_name.as_ref() {
            f.ctrl_data_name = Some(name.clone());
        }
    } else if let Some(name) = field_name.as_ref() {
        f.ctrl_data_name = Some(name.clone());
    }
    f
}

/// numType 문자열 → AutoNumberType 변환
fn parse_num_type(s: &str) -> AutoNumberType {
    match s {
        "PAGE" => AutoNumberType::Page,
        "FOOTNOTE" => AutoNumberType::Footnote,
        "ENDNOTE" => AutoNumberType::Endnote,
        "FIGURE" | "PICTURE" => AutoNumberType::Picture,
        "TABLE" => AutoNumberType::Table,
        "EQUATION" => AutoNumberType::Equation,
        "TOTAL_PAGE" => AutoNumberType::TotalPage,
        _ => AutoNumberType::Page,
    }
}

/// FieldType 문자열 → FieldType 변환
fn parse_field_type(s: &str) -> FieldType {
    match s {
        "DATE" => FieldType::Date,
        "DOC_DATE" | "DOCDATE" => FieldType::DocDate,
        "PATH" => FieldType::Path,
        "BOOKMARK" => FieldType::Bookmark,
        "MAILMERGE" => FieldType::MailMerge,
        "CROSSREF" => FieldType::CrossRef,
        "FORMULA" => FieldType::Formula,
        "CLICK_HERE" | "CLICKHERE" => FieldType::ClickHere,
        "SUMMARY" | "SUMMERY" => FieldType::Summary,
        "USER_INFO" | "USERINFO" => FieldType::UserInfo,
        "HYPERLINK" => FieldType::Hyperlink,
        "MEMO" => FieldType::Memo,
        "PRIVATE_INFO" | "PRIVATEINFO" => FieldType::PrivateInfoSecurity,
        // 직렬화기(serializer/hwpx/field.rs)는 TableOfContents 를 "TOC" 로 방출하므로
        // 파서도 이를 받아야 hwpx 왕복에서 차례 필드 타입이 Unknown 으로 유실되지 않는다.
        "TABLE_OF_CONTENTS" | "TABLEOFCONTENTS" | "TOC" => FieldType::TableOfContents,
        _ => FieldType::Unknown,
    }
}

/// applyPageType 문자열 → HeaderFooterApply 변환
fn parse_apply_page_type(s: &str) -> HeaderFooterApply {
    match s {
        "EVEN" => HeaderFooterApply::Even,
        "ODD" => HeaderFooterApply::Odd,
        _ => HeaderFooterApply::Both,
    }
}

// ─── ctrl 자식 요소별 파싱 함수 ───

/// `<hp:ctrl>` → `<header applyPageType="..." id="...">` → subList → paragraphs
fn parse_ctrl_header(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut header = Header::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"applyPageType" => {
                header.apply_to = parse_apply_page_type(&attr_str(&attr));
            }
            b"id" => {
                header
                    .raw_ctrl_extra
                    .extend_from_slice(&parse_u32(&attr).to_le_bytes());
            }
            _ => {}
        }
    }
    let sublist = parse_sublist_paragraphs_with_layout(reader, b"header")?;
    header.paragraphs = sublist.paragraphs;
    header.list_attr = sublist.list_attr;
    header.text_width = sublist.text_width;
    header.text_height = sublist.text_height;
    header.text_ref = sublist.text_ref;
    header.num_ref = sublist.num_ref;
    Ok(Control::Header(Box::new(header)))
}

/// `<hp:ctrl>` → `<footer applyPageType="..." id="...">` → subList → paragraphs
fn parse_ctrl_footer(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut footer = Footer::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"applyPageType" => {
                footer.apply_to = parse_apply_page_type(&attr_str(&attr));
            }
            b"id" => {
                footer
                    .raw_ctrl_extra
                    .extend_from_slice(&parse_u32(&attr).to_le_bytes());
            }
            _ => {}
        }
    }
    let sublist = parse_sublist_paragraphs_with_layout(reader, b"footer")?;
    footer.paragraphs = sublist.paragraphs;
    footer.list_attr = sublist.list_attr;
    footer.text_width = sublist.text_width;
    footer.text_height = sublist.text_height;
    footer.text_ref = sublist.text_ref;
    footer.num_ref = sublist.num_ref;
    Ok(Control::Footer(Box::new(footer)))
}

/// `<hp:ctrl>` → `<footNote number="..." suffixChar="..." instId="...">` → subList → paragraphs
fn parse_ctrl_footnote(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut note = Footnote::default();
    // [Task #1050] HWP5 CTRL_FOOTNOTE 한컴 default 매핑:
    // suffixChar → after_decoration_letter (default 0x29 ')')
    // instId → instance_id (UInt4)
    note.after_decoration_letter = 0x0029; // default ')'
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"number" => note.number = parse_u16(&attr),
            // [#1199] prefixChar(코드포인트 숫자) → before_decoration_letter
            // 누락 시 0 유지(접두 없음). 예: "47928" = 0xBB38 '문'
            b"prefixChar" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u16>()
                {
                    note.before_decoration_letter = v;
                }
            }
            b"suffixChar" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u16>()
                {
                    note.after_decoration_letter = v;
                }
            }
            // [#2716] flag = HWP5 CTRL_FOOTNOTE numberShape(UInt4). 한컴 HWP5/HWPX 쌍
            // (3-09월_교육_통합_2023) 각주/미주 46개 전수 대조에서 바이트 단위로 일치했다.
            // 값이 0 이면 한컴이 속성 자체를 생략하므로 default 0 유지.
            b"flag" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u32>()
                {
                    note.number_shape = v;
                }
            }
            b"instId" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u32>()
                {
                    note.instance_id = v;
                }
            }
            _ => {}
        }
    }
    note.paragraphs = parse_sublist_paragraphs(reader, b"footNote")?;
    for paragraph in &mut note.paragraphs {
        normalize_hwpx_note_line_vpos(paragraph);
    }
    Ok(Control::Footnote(Box::new(note)))
}

/// `<hp:ctrl>` → `<endNote number="..." suffixChar="..." instId="...">` → subList → paragraphs
fn parse_ctrl_endnote(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut note = Endnote::default();
    // [Task #1050] Footnote 와 동일 매핑
    note.after_decoration_letter = 0x0029;
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"number" => note.number = parse_u16(&attr),
            // [#1199] prefixChar(코드포인트 숫자) → before_decoration_letter
            // 누락 시 0 유지(접두 없음). 예: "47928" = 0xBB38 '문'
            b"prefixChar" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u16>()
                {
                    note.before_decoration_letter = v;
                }
            }
            b"suffixChar" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u16>()
                {
                    note.after_decoration_letter = v;
                }
            }
            // [#2716] flag = HWP5 CTRL_ENDNOTE numberShape(UInt4). footNote 와 동일 계약.
            b"flag" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u32>()
                {
                    note.number_shape = v;
                }
            }
            b"instId" => {
                if let Ok(v) = std::str::from_utf8(&attr.value)
                    .unwrap_or("")
                    .parse::<u32>()
                {
                    note.instance_id = v;
                }
            }
            _ => {}
        }
    }
    note.paragraphs = parse_sublist_paragraphs(reader, b"endNote")?;
    for paragraph in &mut note.paragraphs {
        normalize_hwpx_note_line_vpos(paragraph);
    }
    Ok(Control::Endnote(Box::new(note)))
}

fn normalize_hwpx_note_line_vpos(paragraph: &mut Paragraph) {
    if paragraph.line_segs.len() <= 1 {
        return;
    }

    let mut expected_vpos = None;
    for line_seg in &mut paragraph.line_segs {
        if let Some(expected) = expected_vpos {
            if line_seg.vertical_pos == 0 && expected > 0 {
                // HWPX 미주/각주 내부에는 실제 단/쪽 리셋이 아닌 후속 줄
                // vpos=0이 저장되는 사례가 있다. 본문 의미는 유지하고,
                // note 내부 연속줄만 이전 줄 advance 기준으로 복원한다.
                line_seg.vertical_pos = expected;
            }
        }

        expected_vpos = Some(
            line_seg
                .vertical_pos
                .saturating_add(line_seg.line_height)
                .saturating_add(line_seg.line_spacing),
        );
    }
}

/// `<hp:ctrl>` → `<autoNum num="..." numType="...">` + `<autoNumFormat .../>` 자식
fn parse_ctrl_autonum(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut an = parse_autonum_attrs(e);
    // autoNumFormat 등 자식 요소 파싱
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                if local == b"autoNumFormat" {
                    for attr in ce.attributes().flatten() {
                        match attr.key.as_ref() {
                            // autoNumFormat type 은 문자열 enum (DIGIT/CIRCLE_DIGIT/…).
                            // 과거 parse_u8 은 문자열을 0으로만 떨궈 DIGIT 외 형식을 잃었다.
                            // pageNum formatType 과 동일한 문자열→코드 매핑을 사용한다.
                            b"type" => {
                                an.format = match attr_str(&attr).as_str() {
                                    "DIGIT" => 0,
                                    // [#2957] 실제 한컴 스펙 표기는 "CIRCLED_DIGIT" (pageNum
                                    // formatType 의 "CIRCLE_DIGIT" 와 다름). 구값도 겸용 인식.
                                    "CIRCLE_DIGIT" | "CIRCLED_DIGIT" => 1,
                                    "ROMAN_CAPITAL" => 2,
                                    "ROMAN_SMALL" => 3,
                                    "LATIN_CAPITAL" => 4,
                                    "LATIN_SMALL" => 5,
                                    "HANGUL" => 6,
                                    "HANJA" => 7,
                                    _ => 0,
                                };
                            }
                            b"userChar" => {
                                let s = attr_str(&attr);
                                an.user_symbol = s.chars().next().unwrap_or('\0');
                            }
                            b"prefixChar" => {
                                let s = attr_str(&attr);
                                an.prefix_char = s.chars().next().unwrap_or('\0');
                            }
                            b"suffixChar" => {
                                let s = attr_str(&attr);
                                an.suffix_char = s.chars().next().unwrap_or('\0');
                            }
                            b"supscript" => an.superscript = parse_bool_attr(&attr),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"autoNum" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("autoNum: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(Control::AutoNumber(an))
}

/// `<hp:ctrl>` → `<hiddenComment>` → subList → paragraphs
fn parse_ctrl_hidden_comment(reader: &mut Reader<&[u8]>) -> Result<Control, HwpxError> {
    let mut hc = HiddenComment::default();
    hc.paragraphs = parse_sublist_paragraphs(reader, b"hiddenComment")?;
    Ok(Control::HiddenComment(Box::new(hc)))
}

/// `<hp:ctrl>` → `<fieldBegin type="..." name="..." ...>` + `<parameters>` 자식
fn parse_ctrl_field_begin(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut f = parse_field_begin_attrs(e);
    // parameters 자식에서 Command 값 추출
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                if local == b"parameters" {
                    parse_field_parameters(ce, reader, &mut f)?;
                } else if local == b"subList" && f.field_type == FieldType::Memo {
                    for attr in ce.attributes().flatten() {
                        if attr.key.as_ref() == b"textDirection" {
                            let dir = attr_str(&attr);
                            if dir != "HORIZONTAL" {
                                f.memo_text_direction = Some(dir);
                            }
                        }
                    }
                    f.memo_paragraphs = parse_sublist_paragraphs(reader, b"subList")?;
                } else {
                    let tag = local.to_vec();
                    skip_element(reader, &tag)?;
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"fieldBegin" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("fieldBegin: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(Control::Field(f))
}

/// `<parameters>` 내부에서 Command 문자열 파라미터를 추출한다.
/// XML 텍스트/속성값 이스케이프 (#1391 parameters verbatim 재조립용).
fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // XML 1.0 허용 문자: #x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF]
            // 그 외(제어문자 등)는 제거 — 재조립된 문자열이 그대로 저장돼 불법 XML 이 되지 않도록 (#3382 계열)
            '\u{09}' | '\u{0A}' | '\u{0D}' => out.push(c),
            '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}' => {
                out.push(c)
            }
            _ => {} // XML 무효 문자 제거
        }
    }
    out
}

/// 파라미터 요소(local name)를 여는 프레임으로 변환한다. 5종 외에는 `None`
/// (스키마 밖 요소 — 원문 보존에는 영향 없이 트리에서만 건너뛴다).
fn open_param_frame<'a>(
    local: &[u8],
    attrs: impl Iterator<Item = quick_xml::events::attributes::Attribute<'a>>,
) -> Option<ParamFrame> {
    let mut name: Option<String> = None;
    let mut preserve_space = false;
    for attr in attrs {
        match attr.key.as_ref() {
            b"name" => name = Some(attr_str(&attr)),
            b"xml:space" if attr_str(&attr) == "preserve" => preserve_space = true,
            _ => {}
        }
    }
    match local {
        b"booleanParam" => Some(ParamFrame::Boolean {
            name,
            text: String::new(),
        }),
        b"integerParam" => Some(ParamFrame::Integer {
            name,
            text: String::new(),
        }),
        b"floatParam" => Some(ParamFrame::Float {
            name,
            text: String::new(),
        }),
        b"stringParam" => Some(ParamFrame::String {
            name,
            text: String::new(),
            preserve_space,
        }),
        b"listParam" => Some(ParamFrame::List {
            name,
            items: Vec::new(),
        }),
        _ => None,
    }
}

fn parse_field_parameters(
    start: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
    field: &mut Field,
) -> Result<(), HwpxError> {
    let mut buf = Vec::new();
    let mut in_command = false;
    let mut in_memo_number = false;

    // [#1391] parameters 요소 원문 verbatim 재조립 — 순수 HWPX 왕복(포맷을 안 벗어남)
    // 은 이 문자열을 그대로 재사용해 바이트 정확성을 보장한다(diff_documents 계약).
    // parameters 자식은 stringParam/integerParam(name 속성 + 텍스트)만으로
    // 단순하므로 이벤트 재방출이 안전하다.
    let mut raw = String::from("<hp:parameters");
    for attr in start.attributes().flatten() {
        raw.push(' ');
        raw.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        raw.push_str("=\"");
        raw.push_str(&escape_xml_text(&attr_str(&attr)));
        raw.push('"');
    }
    raw.push('>');

    // [#4396] 병행해서 트리도 만든다 — HWP5 왕복(포맷을 벗어남)에서 `raw_parameters_xml`
    // 이 무효화된 뒤에도 Prop/Direction/Path/Category 등이 Command 하나로 축소되지
    // 않도록. 루트(parameters) 프레임을 스택 바닥에 미리 얹어둔다.
    let root_name = start
        .attributes()
        .flatten()
        .find(|a| a.key.as_ref() == b"name")
        .map(|a| attr_str(&a));
    let mut stack: Vec<ParamFrame> = vec![ParamFrame::List {
        name: root_name,
        items: Vec::new(),
    }];

    // 현재 열린 파라미터 요소 태그(닫을 때 사용).
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                let tag = String::from_utf8_lossy(cname.as_ref()).to_string();
                raw.push('<');
                raw.push_str(&tag);
                for attr in ce.attributes().flatten() {
                    raw.push(' ');
                    raw.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
                    raw.push_str("=\"");
                    raw.push_str(&escape_xml_text(&attr_str(&attr)));
                    raw.push('"');
                }
                raw.push('>');
                if local == b"stringParam" {
                    for attr in ce.attributes().flatten() {
                        if attr.key.as_ref() == b"name" && attr_str(&attr) == "Command" {
                            in_command = true;
                            field.command.clear();
                        }
                    }
                } else if local == b"integerParam" {
                    for attr in ce.attributes().flatten() {
                        if attr.key.as_ref() == b"name" && attr_str(&attr) == "Number" {
                            in_memo_number = true;
                        }
                    }
                }
                if let Some(frame) = open_param_frame(local, ce.attributes().flatten()) {
                    stack.push(frame);
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                raw.push('<');
                raw.push_str(&String::from_utf8_lossy(cname.as_ref()));
                for attr in ce.attributes().flatten() {
                    raw.push(' ');
                    raw.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
                    raw.push_str("=\"");
                    raw.push_str(&escape_xml_text(&attr_str(&attr)));
                    raw.push('"');
                }
                raw.push_str("/>");
                if local == b"stringParam" {
                    for attr in ce.attributes().flatten() {
                        if attr.key.as_ref() == b"name" && attr_str(&attr) == "Command" {
                            field.command.clear();
                        }
                    }
                }
                // 자기닫힘(빈 값) — 여닫 없이 즉시 부모에 붙인다.
                if let Some(frame) = open_param_frame(local, ce.attributes().flatten()) {
                    if let Some(ParamFrame::List { items, .. }) = stack.last_mut() {
                        items.push(frame.finish());
                    }
                }
            }
            Ok(Event::Text(ref t)) => {
                let decoded = t.decode().unwrap_or_default();
                raw.push_str(&escape_xml_text(&decoded));
                if in_command {
                    field.command.push_str(&decoded);
                } else if in_memo_number {
                    if let Ok(value) = decoded.trim().parse::<u32>() {
                        field.memo_index = value;
                    }
                }
                if let Some(frame) = stack.last_mut() {
                    frame.push_text(&decoded);
                }
            }
            Ok(Event::GeneralRef(ref r)) => {
                let decoded = decode_xml_general_ref(r);
                raw.push_str(&escape_xml_text(&decoded));
                if in_command {
                    field.command.push_str(&decoded);
                }
                if let Some(frame) = stack.last_mut() {
                    frame.push_text(&decoded);
                }
            }
            // [CDATA] stringParam(Command)이 CDATA로 인코딩된 경우(예: 하이퍼링크 URL의
            // 쿼리스트링 `&`, 수식 필드의 비교연산자 `<`/`>`) 처리하지 않으면 필드 명령
            // 문자열이 소실된다. #2916/#2927의 hp:script CDATA 누락과 동일한 패턴.
            Ok(Event::CData(ref cdata)) => {
                let decoded = String::from_utf8_lossy(cdata.as_ref()).into_owned();
                raw.push_str(&escape_xml_text(&decoded));
                if in_command {
                    field.command.push_str(&decoded);
                }
                if let Some(frame) = stack.last_mut() {
                    frame.push_text(&decoded);
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                let local = local_name(eename.as_ref());
                if local == b"parameters" {
                    raw.push_str("</hp:parameters>");
                    // 루트 프레임을 팝해 최종 트리로 확정한다.
                    if let Some(ParamFrame::List { name, items }) = stack.pop() {
                        field.parameters = ParameterList { name, items };
                    }
                    break;
                }
                // 임의 깊이 중첩(listParam 안의 stringParam 등)에서도 균형 잡힌 XML 을
                // 재조립하도록, 단일 open_param 추적 대신 End 이벤트 자신의 정규화 이름으로 닫는다.
                // 종전엔 open_param 이 마지막 Start 로 덮여, 바깥 태그의 닫는 태그가 누락됐다.
                let qn = String::from_utf8_lossy(eename.as_ref());
                raw.push_str("</");
                raw.push_str(&qn);
                raw.push('>');
                if local == b"stringParam" {
                    in_command = false;
                } else if local == b"integerParam" {
                    in_memo_number = false;
                }
                // 스키마 5종 중 하나를 닫는 End 라면 스택에서 팝해 부모 List 에 붙인다.
                // (스키마 밖 요소는 애초에 push 되지 않았으므로 이 조건이 걸리지 않는다.)
                if matches!(
                    local,
                    b"booleanParam"
                        | b"integerParam"
                        | b"floatParam"
                        | b"stringParam"
                        | b"listParam"
                ) {
                    if let Some(frame) = stack.pop() {
                        let param = frame.finish();
                        if let Some(ParamFrame::List { items, .. }) = stack.last_mut() {
                            items.push(param);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("parameters: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    field.raw_parameters_xml = Some(raw);
    Ok(())
}


// ─── 문단 레벨 컨트롤 파싱 (compose, dutmal, equation) ───

/// `<hp:compose>` 요소 (글자겹침/CharOverlap)를 파싱한다.
fn parse_compose(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut co = CharOverlap::default();
    // 요소 속성 파싱
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"circleType" => {
                co.border_type = match attr_str(&attr).as_str() {
                    "CHAR" => 0,
                    "SHAPE_CIRCLE" => 1,
                    "SHAPE_REVERSAL_CIRCLE" => 2,
                    "SHAPE_RECTANGLE" => 3,
                    "SHAPE_REVERSAL_RECTANGLE" => 4,
                    "SHAPE_TRIANGLE" => 5,
                    "SHAPE_REVERSAL_TIRANGLE" => 6,
                    _ => 0,
                };
            }
            b"charSz" => co.inner_char_size = parse_i8(&attr),
            b"composeType" => {
                co.expansion = match attr_str(&attr).as_str() {
                    "OVERLAP" => 1,
                    _ => 0, // SPREAD
                };
            }
            // 한컴 HWPX는 `composeText="장"`처럼 속성에 글자를 넣기도 한다.
            // 자식 element form(<composeText>장</composeText>)이 뒤에 나오면 그쪽이 덮어쓴다.
            b"composeText" => co.chars = attr_str(&attr).chars().collect(),
            _ => {}
        }
    }
    // 자식 요소 파싱 (composeText, charPr)
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                if local == b"composeText" {
                    let text = read_compose_text(reader)?;
                    co.chars = text.chars().collect();
                } else {
                    let tag = local.to_vec();
                    skip_element(reader, &tag)?;
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                if local == b"charPr" {
                    for attr in ce.attributes().flatten() {
                        if attr.key.as_ref() == b"prIDRef" {
                            co.char_shape_ids.push(parse_u32(&attr));
                        }
                    }
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"compose" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("compose: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(Control::CharOverlap(co))
}

/// `<composeText>` 내부 텍스트를 읽는다.
fn read_compose_text(reader: &mut Reader<&[u8]>) -> Result<String, HwpxError> {
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref t)) => {
                text.push_str(&t.decode().unwrap_or_default());
            }
            Ok(Event::GeneralRef(ref r)) => {
                text.push_str(&decode_xml_general_ref(r));
            }
            // [CDATA] composeText(글자겹치기) 본문이 CDATA로 인코딩된 경우 처리하지
            // 않으면 겹침 텍스트가 소실된다. #2916/#2935/#2951의 CDATA 누락과 동일한 패턴.
            Ok(Event::CData(ref cdata)) => {
                text.push_str(&String::from_utf8_lossy(cdata.as_ref()));
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"composeText" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("composeText: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

/// `<hp:dutmal>` 요소 (덧말/Ruby)를 파싱한다.
fn parse_dutmal(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut ruby = Ruby::default();
    // 요소 속성 (#1587 — posType/align 분리 보존 + szRatio/option/styleIDRef)
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"posType" => {
                ruby.pos_type = match attr_str(&attr).as_str() {
                    "TOP" => 0,
                    "BOTTOM" => 1,
                    _ => 0,
                };
            }
            b"align" => {
                ruby.align = match attr_str(&attr).as_str() {
                    "LEFT" => 0,
                    "RIGHT" => 1,
                    "CENTER" => 2,
                    _ => 0,
                };
            }
            b"szRatio" => {
                ruby.sz_ratio = attr_str(&attr).parse().unwrap_or(0);
            }
            b"option" => {
                ruby.option = attr_str(&attr).parse().unwrap_or(0);
            }
            b"styleIDRef" => {
                ruby.style_id_ref = attr_str(&attr).parse().unwrap_or(0);
            }
            _ => {}
        }
    }
    // 자식 요소 파싱 (mainText 기준 텍스트 + subText 덧말)
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                if local == b"subText" {
                    ruby.ruby_text = read_dutmal_text(reader, b"subText")?;
                } else if local == b"mainText" {
                    // [#1587] mainText(기준 텍스트)는 para.text 에 포함되지 않으므로
                    // 모델에 보존한다(종전 skip → 손실 → 직렬화 시 복원 불가였음).
                    ruby.main_text = read_dutmal_text(reader, b"mainText")?;
                } else {
                    let tag = local.to_vec();
                    skip_element(reader, &tag)?;
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"dutmal" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("dutmal: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(Control::Ruby(ruby))
}

/// dutmal 내부 텍스트 요소(mainText, subText)의 텍스트를 읽는다.
fn read_dutmal_text(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> Result<String, HwpxError> {
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref t)) => {
                text.push_str(&t.decode().unwrap_or_default());
            }
            Ok(Event::GeneralRef(ref r)) => {
                text.push_str(&decode_xml_general_ref(r));
            }
            // [CDATA] dutmal(덧말)의 mainText/subText가 CDATA로 인코딩된 경우 처리하지
            // 않으면 덧말 텍스트가 소실된다. #2916/#2935의 hp:script/stringParam CDATA
            // 누락과 동일한 패턴.
            Ok(Event::CData(ref cdata)) => {
                text.push_str(&String::from_utf8_lossy(cdata.as_ref()));
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
    Ok(text)
}

/// `<hp:equation>` 요소 (수식)를 파싱한다.
/// 수식 속성(version, baseLine, textColor, baseUnit, lineMode, font)과
/// `<hp:script>` 하위 요소에서 수식 스크립트를 추출하여 `Control::Equation`을 생성한다.
fn parse_equation(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut common = CommonObjAttr::default();
    let mut shape_attr = ShapeComponentAttr::default();
    let mut has_pos = false;

    // 수식 전용 속성 — 초기값은 OWPML(ParaList 스키마 EquationType) 속성 기본값.
    // 속성이 생략된 파일에서 zero-계열 값으로 복원하면 직렬화기가 세 속성을
    // 무조건 방출하므로 라운드트립 시 version=""/baseLine="0"/font="" 으로 변형된다.
    let mut version_info = String::from("Equation Version 60");
    let mut baseline: i16 = 85;
    let mut color: u32 = 0;
    let mut font_size: u32 = 1000;
    let mut font_name = String::from("HYhwpEQ");
    // [#2727] lineMode(수식이 차지하는 범위) → EQEDIT attribute bit0.
    // OWPML 기본값은 CHAR 이므로 속성이 없으면 0(글자 단위)으로 둔다.
    // `attr`/`eqedit` 두 필드가 동일한 값을 보관하므로 함께 채운다.
    let mut eq_attr: u32 = 0;
    let mut eqedit: u32 = 0;

    // 공통 개체 속성 + 수식 속성 파싱
    parse_object_element_attrs(e, &mut common, &mut shape_attr);
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"version" => version_info = attr_str(&attr),
            b"baseLine" => baseline = attr_str(&attr).parse().unwrap_or(85),
            b"textColor" => color = parse_color(&attr),
            b"baseUnit" => font_size = parse_u32(&attr),
            // [#2727] LINE 이면 bit0 set. 종전엔 미파싱으로 왕복 시 CHAR 로 고정됐다.
            // `attr`/`eqedit` 두 필드가 동일한 값을 보관하므로 함께 채운다.
            b"lineMode" => {
                if attr_str(&attr).eq_ignore_ascii_case("LINE") {
                    eq_attr |= EQUATION_LINE_MODE_BIT;
                } else {
                    eq_attr &= !EQUATION_LINE_MODE_BIT;
                }
                eqedit = eq_attr;
            }
            b"font" => font_name = attr_str(&attr),
            _ => {}
        }
    }

    let mut script = String::new();
    let mut in_script = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"sz" | b"curSz" | b"orgSz" | b"pos" | b"offset" | b"outMargin" => {
                        parse_object_layout_child(
                            local,
                            ce,
                            &mut common,
                            &mut shape_attr,
                            &mut has_pos,
                        );
                    }
                    b"script" => {
                        in_script = true;
                    }
                    // 수식 설명 (#1392) — 미적재 시 roundtrip 에서 소실
                    b"shapeComment" => {
                        common.description = read_dutmal_text(reader, b"shapeComment")?;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref txt)) => {
                if in_script {
                    if let Ok(s) = txt.decode() {
                        script.push_str(&s);
                    }
                }
            }
            Ok(Event::CData(ref cdata)) => {
                // #2916: 수식 스크립트가 CDATA 로 저장된 경우(비교 연산자 등 XML
                // 예약 문자를 다량 포함해 엔티티 이스케이프 대신 CDATA 로 감싸는
                // 케이스), 이 분기가 없으면 script 가 통째로 빈 문자열이 된다.
                if in_script {
                    script.push_str(&String::from_utf8_lossy(cdata.as_ref()));
                }
            }
            Ok(Event::GeneralRef(ref r)) => {
                if in_script {
                    if let Ok(Some(ch)) = r.resolve_char_ref() {
                        script.push(ch);
                    } else if let Ok(name) = r.decode() {
                        match name.as_ref() {
                            "lt" => script.push('<'),
                            "gt" => script.push('>'),
                            "amp" => script.push('&'),
                            "quot" => script.push('"'),
                            "apos" => script.push('\''),
                            _ => {
                                script.push('&');
                                script.push_str(&name);
                                script.push(';');
                            }
                        }
                    }
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                let local = local_name(eename.as_ref());
                if local == b"script" {
                    in_script = false;
                } else if local == b"equation" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("equation: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    let equation = Equation {
        common,
        // [#2727] HWPX lineMode → EQEDIT attribute bit0
        attr: eq_attr,
        script,
        font_size,
        color,
        baseline,
        unknown: 0,
        eqedit,
        font_name,
        version_info,
        raw_ctrl_data: Vec::new(),
    };
    Ok(Control::Equation(Box::new(equation)))
}

// ─── 유틸리티 (section 전용) ───

/// 텍스트 파트들의 UTF-16 길이 합산
/// 탭 문자는 HWP 바이너리와 동일하게 8 code unit으로 계산
fn calc_utf16_len_from_parts(parts: &[String]) -> u32 {
    parts
        .iter()
        .map(|s| match s.as_str() {
            // [#1382] \u{0012}(AUTO_NUMBER) 포함 — placeholder 공백을 포함해 8유닛
            // (offsets 조립 루프와 동일 축). 종전 `_` 분기(1유닛)로 빠져 char_shapes
            // 경계가 offsets 축과 어긋났다 (143E 각주 run 경계 2 → 정답 9).
            "\u{0002}" | "\u{0003}" | "\u{0004}" | "\u{0012}" => 8,
            _ => s
                .chars()
                .map(|c| {
                    if c == '\t' {
                        8u32
                    } else if (c as u32) > 0xFFFF {
                        2
                    } else {
                        1
                    }
                })
                .sum(),
        })
        .sum()
}

// ─── 양식 컨트롤 파싱 ───


// ---------------- HWPX switch / chart / ole 핸들러 ----------------

/// `<hp:switch>`를 열고 내부에서 OOXML 차트(hp:chart)를 우선적으로,
/// 없으면 OLE fallback(hp:ole)을 파싱하여 Control로 반환
fn parse_switch_chart_or_ole(reader: &mut Reader<&[u8]>) -> Result<Option<Control>, HwpxError> {
    let mut chart_ctrl: Option<Control> = None;
    let mut ole_ctrl: Option<Control> = None;
    let mut buf = Vec::new();
    let mut in_case = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"case" => {
                        in_case = true;
                    }
                    b"default" => {
                        in_case = false;
                    }
                    b"chart" => {
                        if chart_ctrl.is_none() {
                            chart_ctrl = parse_hp_chart_element(ce, reader)?;
                        } else {
                            skip_element(reader, b"chart")?;
                        }
                    }
                    b"ole" => {
                        if ole_ctrl.is_none() {
                            ole_ctrl = parse_hp_ole_element(ce, reader)?;
                        } else {
                            skip_element(reader, b"ole")?;
                        }
                    }
                    _ => {}
                }
                let _ = in_case;
            }
            Ok(Event::End(ref ee)) => {
                if local_name(ee.name().as_ref()) == b"switch" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("switch: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    // [#3546] 차트가 있으면 <hp:default> 의 fallback OLE 를 버리지 않고 차트에
    // 매달아 보존한다 — 저장 시 원형 <hp:switch>/<hp:case>/<hp:default> 구조
    // 재방출의 재료다(종전에는 fallback 이 소실되어 hp:ole 단독으로 되쓰였다).
    match (chart_ctrl, ole_ctrl) {
        (Some(mut chart), Some(ole)) => {
            if let (Control::Shape(chart_shape), Control::Shape(ole_shape)) = (&mut chart, ole) {
                if let (ShapeObject::Ole(chart_ole), ShapeObject::Ole(fallback)) =
                    (chart_shape.as_mut(), *ole_shape)
                {
                    chart_ole.chart_switch_fallback = Some(fallback);
                }
            }
            Ok(Some(chart))
        }
        (chart, ole) => Ok(chart.or(ole)),
    }
}

/// `<hp:chart chartIDRef="Chart/chartN.xml" zOrder="..." textWrap="..." ...>` 내부를 OLE 모델로 변환
fn parse_hp_chart_element(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Option<Control>, HwpxError> {
    use crate::model::shape::OleShape;

    let mut common = CommonObjAttr::default();
    common.hwp5_gen_shape_attr_bit26 = true;
    let mut chart_num: u16 = 0;
    let mut chart_id_ref: Option<String> = None;
    let mut id_attr: u32 = 0;
    let mut numbering_type_picture = false;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            // [#2882] common.numbering_type(ObjectNumberingType) 도 함께 채운다.
            // 직렬화기(numbering_type_str, serializer/hwpx/shape.rs)가 참조하는
            // 필드는 이것뿐이라, bool 지역 변수만으로는 저장 시 항상 NONE 으로
            // 되쓰인다(공용 도형 파서 section.rs:2892 와 동일 패턴으로 맞춤).
            b"numberingType" => {
                numbering_type_picture = attr_str(&attr).eq_ignore_ascii_case("PICTURE");
                common.numbering_type = match attr_str(&attr).to_ascii_uppercase().as_str() {
                    "PICTURE" => crate::model::shape::ObjectNumberingType::Picture,
                    "TABLE" => crate::model::shape::ObjectNumberingType::Table,
                    "EQUATION" => crate::model::shape::ObjectNumberingType::Equation,
                    _ => crate::model::shape::ObjectNumberingType::None,
                };
            }
            b"zOrder" => common.z_order = parse_i32(&attr),
            b"textWrap" => {
                common.text_wrap = match attr_str(&attr).as_str() {
                    "SQUARE" => TextWrap::Square,
                    "TIGHT" => TextWrap::Tight,
                    "THROUGH" => TextWrap::Through,
                    "TOP_AND_BOTTOM" => TextWrap::TopAndBottom,
                    "BEHIND_TEXT" => TextWrap::BehindText,
                    "IN_FRONT_OF_TEXT" => TextWrap::InFrontOfText,
                    _ => TextWrap::Square,
                };
            }
            b"textFlow" => {
                common.text_flow = match attr_str(&attr).as_str() {
                    "LEFT_ONLY" => crate::model::shape::TextFlow::LeftOnly,
                    "RIGHT_ONLY" => crate::model::shape::TextFlow::RightOnly,
                    "LARGEST_ONLY" => crate::model::shape::TextFlow::LargestOnly,
                    _ => crate::model::shape::TextFlow::BothSides,
                };
            }
            b"chartIDRef" => {
                // "Chart/chart1.xml" → 1
                let s = attr_str(&attr);
                let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                chart_num = digits.parse().unwrap_or(0);
                // [#3546] 원문을 보존한다 — 저장 시 hp:chart 원형 재방출의 표식.
                chart_id_ref = Some(s);
            }
            // [#3546] 실물 hp:chart 는 instid 없이 id 만 기록한다 — 미파싱이면
            // 재방출 id 가 항상 "0" 으로 되쓰인다. instid 가 있으면 그쪽이 우선
            // (아래 arm 이 뒤에서 덮는 것이 아니라 후처리에서 판정).
            b"id" => id_attr = parse_u32(&attr),
            b"instid" => common.instance_id = parse_u32(&attr),
            // [#2931] 개체 잠금(lock) — 종전 미파싱으로 직렬화 시 항상 "0"으로
            // 되돌아가 차트 개체의 잠금 상태가 유실됐다.
            b"lock" => common.locked = attr_str(&attr) == "1",
            _ => {}
        }
    }
    if common.instance_id == 0 {
        common.instance_id = id_attr;
    }

    let mut extent: Option<(i32, i32)> = None;
    let mut shape_attr = ShapeComponentAttr::default();
    let mut caption: Option<crate::model::shape::Caption> = None;
    parse_common_shape_children(
        reader,
        &mut common,
        b"chart",
        &mut extent,
        &mut shape_attr,
        &mut caption,
    )?;
    if numbering_type_picture {
        common.hwp5_gen_shape_attr_bit28 = true;
    }
    common.attr = pack_hwpx_common_obj_attr(&common);

    if chart_num == 0 {
        return Ok(None);
    }

    let mut ole = OleShape::default();
    ole.common = common;
    ole.drawing.shape_attr = shape_attr;
    ole.bin_data_id = 60000u32 + chart_num as u32;
    ole.chart_id_ref = chart_id_ref;
    // <hc:extent> 가 있으면 원본 개체 크기를 보존한다(없으면 종전 기본값 7200).
    let (ext_x, ext_y) = extent.unwrap_or((7200, 7200));
    ole.extent_x = if ext_x > 0 { ext_x } else { 7200 };
    ole.extent_y = if ext_y > 0 { ext_y } else { 7200 };
    apply_hwpx_ole_shape_component_contract(&mut ole);
    // [#4319] HWP5 파서(parser/control/shape.rs:213)와 동형 정규화 — drawing.caption
    // 에 남기지 않고 OleShape 자신의 caption 필드로 옮긴다. 게이트(shape_caption,
    // serializer/hwpx/roundtrip.rs)는 `x.caption` 만 보므로 정규화하지 않으면
    // drawing.caption 잔류가 라운드트립 비교에서 보이지 않는다.
    ole.drawing.caption = caption;
    ole.caption = ole.drawing.caption.take();
    Ok(Some(Control::Shape(Box::new(ShapeObject::Ole(Box::new(
        ole,
    ))))))
}

/// `<hp:ole binaryItemIDRef="oleN" ...>` 내부를 OLE 모델로 변환 (fallback용)
fn parse_hp_ole_element(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Option<Control>, HwpxError> {
    use crate::model::shape::OleShape;

    use crate::model::shape::OleDrawingAspect;

    let mut common = CommonObjAttr::default();
    common.hwp5_gen_shape_attr_bit26 = true;
    let mut bin_id: u32 = 0;
    let mut numbering_type_picture = false;
    let mut draw_aspect = OleDrawingAspect::default();

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            // [#2882] common.numbering_type(ObjectNumberingType) 도 함께 채운다.
            // 직렬화기(numbering_type_str, serializer/hwpx/shape.rs)가 참조하는
            // 필드는 이것뿐이라, bool 지역 변수만으로는 저장 시 항상 NONE 으로
            // 되쓰인다(공용 도형 파서 section.rs:2892 와 동일 패턴으로 맞춤).
            b"numberingType" => {
                numbering_type_picture = attr_str(&attr).eq_ignore_ascii_case("PICTURE");
                common.numbering_type = match attr_str(&attr).to_ascii_uppercase().as_str() {
                    "PICTURE" => crate::model::shape::ObjectNumberingType::Picture,
                    "TABLE" => crate::model::shape::ObjectNumberingType::Table,
                    "EQUATION" => crate::model::shape::ObjectNumberingType::Equation,
                    _ => crate::model::shape::ObjectNumberingType::None,
                };
            }
            // 표시 방식(아이콘/썸네일/인쇄용/내용). serializer 는 방출하나 종전엔
            // 파서가 읽지 않아 ICON 등이 왕복 시 CONTENT 로 바뀌었다.
            b"drawAspect" => {
                draw_aspect = match attr_str(&attr).as_str() {
                    "ICON" => OleDrawingAspect::Icon,
                    "THUMBNAIL" => OleDrawingAspect::Thumbnail,
                    "DOCPRINT" => OleDrawingAspect::DocPrint,
                    _ => OleDrawingAspect::Content,
                };
            }
            b"zOrder" => common.z_order = parse_i32(&attr),
            b"textWrap" => {
                common.text_wrap = match attr_str(&attr).as_str() {
                    "SQUARE" => TextWrap::Square,
                    "TIGHT" => TextWrap::Tight,
                    "THROUGH" => TextWrap::Through,
                    "TOP_AND_BOTTOM" => TextWrap::TopAndBottom,
                    "BEHIND_TEXT" => TextWrap::BehindText,
                    "IN_FRONT_OF_TEXT" => TextWrap::InFrontOfText,
                    _ => TextWrap::Square,
                };
            }
            b"textFlow" => {
                common.text_flow = match attr_str(&attr).as_str() {
                    "LEFT_ONLY" => crate::model::shape::TextFlow::LeftOnly,
                    "RIGHT_ONLY" => crate::model::shape::TextFlow::RightOnly,
                    "LARGEST_ONLY" => crate::model::shape::TextFlow::LargestOnly,
                    _ => crate::model::shape::TextFlow::BothSides,
                };
            }
            b"binaryItemIDRef" => {
                let s = attr_str(&attr);
                let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                bin_id = digits.parse().unwrap_or(0);
            }
            b"instid" => common.instance_id = parse_u32(&attr),
            // [#2931] 개체 잠금(lock) — 종전 미파싱으로 직렬화 시 항상 "0"으로
            // 되돌아가 OLE 개체의 잠금 상태가 유실됐다.
            b"lock" => common.locked = attr_str(&attr) == "1",
            _ => {}
        }
    }

    let mut extent: Option<(i32, i32)> = None;
    let mut shape_attr = ShapeComponentAttr::default();
    let mut caption: Option<crate::model::shape::Caption> = None;
    parse_common_shape_children(
        reader,
        &mut common,
        b"ole",
        &mut extent,
        &mut shape_attr,
        &mut caption,
    )?;
    if numbering_type_picture {
        common.hwp5_gen_shape_attr_bit28 = true;
    }
    common.attr = pack_hwpx_common_obj_attr(&common);

    let mut ole = OleShape::default();
    ole.common = common;
    ole.drawing.shape_attr = shape_attr;
    ole.bin_data_id = bin_id;
    ole.drawing_aspect = draw_aspect;
    // <hc:extent> 가 있으면 원본 개체 크기를 보존한다(없으면 종전 기본값 7200).
    let (ext_x, ext_y) = extent.unwrap_or((7200, 7200));
    ole.extent_x = if ext_x > 0 { ext_x } else { 7200 };
    ole.extent_y = if ext_y > 0 { ext_y } else { 7200 };
    apply_hwpx_ole_shape_component_contract(&mut ole);
    // [#4319] HWP5 파서(parser/control/shape.rs:222)와 동형 정규화 — 차트와 동일한
    // 이유로 drawing.caption 이 아니라 ole.caption 에 남겨야 게이트가 검출한다.
    ole.drawing.caption = caption;
    ole.caption = ole.drawing.caption.take();
    Ok(Some(Control::Shape(Box::new(ShapeObject::Ole(Box::new(
        ole,
    ))))))
}


#[cfg(test)]
mod tests;
