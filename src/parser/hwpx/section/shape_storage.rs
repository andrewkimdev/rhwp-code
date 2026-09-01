//! shape_storage — table_layout.rs 에서 무변동 이동
use super::*;

pub(crate) fn parse_picture(
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut img_attr = ImageAttr::default();
    let mut common = CommonObjAttr::default();
    common.hwp5_gen_shape_attr_bit26 = true;
    let mut shape_attr = ShapeComponentAttr::default();
    let mut crop = CropInfo::default();
    let mut padding = crate::model::Padding::default();
    let mut border_x = [0i32; 4];
    let mut border_y = [0i32; 4];
    let mut img_dim: (u32, u32) = (0, 0); // [#1389] hp:imgDim 원본 이미지 픽셀 크기
    let mut href: Option<String> = None;
    let mut picture_instance_id = 0;
    let mut effects = PictureEffects::default();
    let mut reverse = false;
    let mut lock = false;

    // <hp:pic> 요소 자체의 속성 파싱
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"id" => common.instance_id = parse_u32(&attr),
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
            b"instid" => picture_instance_id = parse_u32(&attr),
            b"href" => {
                let value = attr_str(&attr);
                if !value.is_empty() {
                    href = Some(value);
                }
            }
            b"groupLevel" => shape_attr.group_level = attr_str(&attr).parse().unwrap_or(0),
            // [#2861] 좌우 반전(한컴 Automation InsertPicture 의 reverse 옵션과 동일 개념).
            // 종전 미매칭으로 조용히 버려져 직렬화 시 항상 reverse="0" 하드코딩되던 유실.
            b"reverse" => reverse = attr_str(&attr) == "1",
            // [#2875] 개체 잠금(보호). 종전 미매칭으로 조용히 버려져 직렬화 시 항상
            // lock="0" 하드코딩되던 유실 — #2861(reverse), #2855(hp:tbl lock)과 동일 패턴.
            b"lock" => lock = attr_str(&attr) == "1",
            // dropcapstyle (개체를 감싼 문단의 드롭캡 표시 방식) 보존.
            // 미파싱 상태에서는 picture.rs 방출측이 항상 "None"으로 되돌려,
            // DoubleLine/TripleLine/Margin 드롭캡 문단에 있던 그림이 저장 시
            // 드롭캡 스타일을 잃는다.
            b"dropcapstyle" => {
                common.drop_cap_style = match attr_str(&attr).as_str() {
                    "DoubleLine" => crate::model::shape::DropCapStyle::DoubleLine,
                    "TripleLine" => crate::model::shape::DropCapStyle::TripleLine,
                    "Margin" => crate::model::shape::DropCapStyle::Margin,
                    _ => crate::model::shape::DropCapStyle::None,
                };
            }
            // [#2697 동형] numberingType (캡션 번호 범주) 보존 — 도형·표·그림 공통 속성.
            // 종전 미파싱으로 그림에 번호 범주를 NONE 등으로 변경한 HWPX에서 IR 기본값(None)으로
            // 떨어져 왕복 시 "PICTURE"로 강제복원되던 결함을 수정한다.
            b"numberingType" => {
                common.numbering_type = match attr_str(&attr).to_ascii_uppercase().as_str() {
                    "PICTURE" => crate::model::shape::ObjectNumberingType::Picture,
                    "TABLE" => crate::model::shape::ObjectNumberingType::Table,
                    "EQUATION" => crate::model::shape::ObjectNumberingType::Equation,
                    _ => crate::model::shape::ObjectNumberingType::None,
                };
            }
            _ => {}
        }
    }

    // 이미지 속성 읽기
    let mut has_pos = false; // <pos> 파싱 여부 — <offset>이 덮어쓰지 않도록 방지
    let mut caption: Option<crate::model::shape::Caption> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"imgRect" => {
                parse_picture_img_rect(reader, &mut border_x, &mut border_y)?;
            }
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"shapeComment" => {
                common.description = read_dutmal_text(reader, b"shapeComment")?;
            }
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"effects" => {
                effects = parse_picture_effects(reader)?;
            }
            // 그림 캡션 (#1403) — 미적재 시 roundtrip 에서 캡션 subList 소실
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"caption" => {
                caption = Some(parse_caption(ce, reader)?);
            }
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"sz" => {
                        // 최종 표시 크기 (최우선)
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => {
                                    let v = parse_u32(&attr);
                                    if v > 0 {
                                        common.width = v;
                                    }
                                }
                                b"height" => {
                                    let v = parse_u32(&attr);
                                    if v > 0 {
                                        common.height = v;
                                    }
                                }
                                // [#2712] 그림만 크기 기준·크기 보호 arm 이 없어 파싱 단계에서
                                // 유실됐다. 도형 파서(같은 파일 2901-2907)와 동형이며, 높이는
                                // 도형과 마찬가지로 allow_column_para=false 로 읽어 치역을
                                // {Paper, Page, Absolute} 로 제한한다.
                                b"widthRelTo" => {
                                    common.width_criterion =
                                        parse_size_criterion(&attr_str(&attr), true);
                                }
                                b"heightRelTo" => {
                                    common.height_criterion =
                                        parse_size_criterion(&attr_str(&attr), false);
                                }
                                b"protect" => common.size_protect = parse_bool(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"curSz" => {
                        // 현재 크기 → common + shape_attr.current_width/height
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => {
                                    let v = parse_u32(&attr);
                                    shape_attr.current_width = v;
                                    if v > 0 {
                                        common.width = v;
                                    }
                                }
                                b"height" => {
                                    let v = parse_u32(&attr);
                                    shape_attr.current_height = v;
                                    if v > 0 {
                                        common.height = v;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    // [#1389] 원본 이미지 픽셀 크기 — verbatim 적재
                    b"imgDim" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"dimwidth" => img_dim.0 = parse_u32(&attr),
                                b"dimheight" => img_dim.1 = parse_u32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"orgSz" => {
                        // 원본 크기 → shape_attr.original_width/height (렌더러 이미지 Fill 크기에 사용)
                        // curSz/sz가 없을 때 common.width/height 폴백으로도 사용
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => {
                                    let v = parse_u32(&attr);
                                    shape_attr.original_width = v;
                                    if common.width == 0 {
                                        common.width = v;
                                    }
                                }
                                b"height" => {
                                    let v = parse_u32(&attr);
                                    shape_attr.original_height = v;
                                    if common.height == 0 {
                                        common.height = v;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    b"pos" => {
                        has_pos = true;
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"treatAsChar" => {
                                    common.treat_as_char =
                                        attr_str(&attr) == "1" || attr_str(&attr) == "true";
                                }
                                // [#2784] affectLSpacing(줄 간격에 영향) — 그림/도형 pos 되읽기.
                                b"affectLSpacing" => common.affect_line_spacing = parse_bool(&attr),
                                b"flowWithText" => common.flow_with_text = parse_bool(&attr),
                                b"allowOverlap" => common.allow_overlap = parse_bool(&attr),
                                // holdAnchorAndSO(쪽나눔 방지). 방출측은 모든 개체에 내지만
                                // 종전엔 표 파서만 되읽어, 그림/도형/차트/OLE 는 prevent_page_break
                                // 이 0 으로 유실됐다(표 파서와 동형으로 보강).
                                b"holdAnchorAndSO" => {
                                    common.prevent_page_break =
                                        if parse_bool(&attr) { 1 } else { 0 };
                                }
                                b"vertRelTo" => {
                                    common.vert_rel_to = match attr_str(&attr).as_str() {
                                        "PAPER" => VertRelTo::Paper,
                                        "PAGE" => VertRelTo::Page,
                                        "PARA" => VertRelTo::Para,
                                        _ => VertRelTo::Para,
                                    };
                                }
                                b"horzRelTo" => {
                                    common.horz_rel_to = match attr_str(&attr).as_str() {
                                        "PAPER" => HorzRelTo::Paper,
                                        "PAGE" => HorzRelTo::Page,
                                        "COLUMN" => HorzRelTo::Column,
                                        "PARA" => HorzRelTo::Para,
                                        _ => HorzRelTo::Para,
                                    };
                                }
                                b"vertAlign" => {
                                    common.vert_align = match attr_str(&attr).as_str() {
                                        "TOP" => VertAlign::Top,
                                        "CENTER" => VertAlign::Center,
                                        "BOTTOM" => VertAlign::Bottom,
                                        "INSIDE" => VertAlign::Inside,
                                        "OUTSIDE" => VertAlign::Outside,
                                        _ => VertAlign::Top,
                                    };
                                }
                                b"horzAlign" => {
                                    common.horz_align = match attr_str(&attr).as_str() {
                                        "LEFT" => HorzAlign::Left,
                                        "CENTER" => HorzAlign::Center,
                                        "RIGHT" => HorzAlign::Right,
                                        "INSIDE" => HorzAlign::Inside,
                                        "OUTSIDE" => HorzAlign::Outside,
                                        _ => HorzAlign::Left,
                                    };
                                }
                                b"vertOffset" => {
                                    common.vertical_offset = parse_i32_wrapping(&attr) as u32
                                }
                                b"horzOffset" => {
                                    common.horizontal_offset = parse_i32_wrapping(&attr) as u32
                                }
                                _ => {}
                            }
                        }
                    }
                    b"outMargin" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => common.margin.left = parse_i16(&attr),
                                b"right" => common.margin.right = parse_i16(&attr),
                                b"top" => common.margin.top = parse_i16(&attr),
                                b"bottom" => common.margin.bottom = parse_i16(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"inMargin" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => padding.left = parse_i16(&attr),
                                b"right" => padding.right = parse_i16(&attr),
                                b"top" => padding.top = parse_i16(&attr),
                                b"bottom" => padding.bottom = parse_i16(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"imgClip" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => crop.left = parse_i32(&attr),
                                b"right" => crop.right = parse_i32(&attr),
                                b"top" => crop.top = parse_i32(&attr),
                                b"bottom" => crop.bottom = parse_i32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"img" | b"image" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"binaryItemIDRef" => {
                                    // "image1" → BinData ID 1
                                    let val = attr_str(&attr);
                                    let num: String =
                                        val.chars().filter(|c| c.is_ascii_digit()).collect();
                                    img_attr.bin_data_id = num.parse().unwrap_or(0);
                                }
                                b"bright" => img_attr.brightness = parse_i8(&attr),
                                b"contrast" => img_attr.contrast = parse_i8(&attr),
                                b"alpha" => {
                                    img_attr.transparency =
                                        parse_picture_transparency_attr(&attr_str(&attr));
                                }
                                b"effect" => {
                                    img_attr.effect = match attr_str(&attr).as_str() {
                                        "REAL_PIC" => ImageEffect::RealPic,
                                        "GRAY_SCALE" => ImageEffect::GrayScale,
                                        "BLACK_WHITE" => ImageEffect::BlackWhite,
                                        // 방출측 image_effect_str 은 Pattern8x8 을 이 문자열로
                                        // 낸다. 안 받으면 무늬(패턴) 효과가 왕복 시 RealPic 유실.
                                        "PATTERN_8_8" => ImageEffect::Pattern8x8,
                                        _ => ImageEffect::RealPic,
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                    b"offset" => {
                        // <offset>은 개체 내부의 shape-transform 오프셋이다.
                        // shape_attr.offset_x/offset_y에 항상 저장 (그룹 내부 좌표용).
                        // <pos>가 이미 파싱된 경우 페이지 레벨 좌표(vertOffset/horzOffset)는
                        // 덮어쓰지 않는다. <pos>가 없는 경우에만 폴백으로 적용한다.
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => {
                                    let v = parse_i32_wrapping(&attr);
                                    shape_attr.offset_x = v;
                                    if !has_pos {
                                        common.horizontal_offset = v as u32;
                                    }
                                }
                                b"y" => {
                                    let v = parse_i32_wrapping(&attr);
                                    shape_attr.offset_y = v;
                                    if !has_pos {
                                        common.vertical_offset = v as u32;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    b"renderingInfo" => {
                        // 그룹 내 자식의 아핀 변환 행렬 파싱
                        parse_rendering_info(reader, &mut shape_attr)?;
                    }
                    b"flip" => {
                        parse_shape_flip(ce, &mut shape_attr);
                    }
                    b"rotationInfo" => {
                        parse_shape_rotation_info(ce, &mut shape_attr);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == b"pic" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("pic: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    if common.instance_id == 0 && picture_instance_id != 0 {
        common.instance_id = picture_instance_id;
    }

    materialize_shape_hwp_storage_defaults(&mut common, &mut shape_attr, ShapeStorageKind::Picture);

    let mut pic = crate::model::image::Picture::default();
    pic.image_attr = img_attr;
    pic.common = common;
    pic.shape_attr = shape_attr;
    pic.href = href;
    pic.crop = crop;
    pic.padding = padding;
    pic.border_x = border_x;
    pic.border_y = border_y;
    pic.instance_id = picture_instance_id;
    pic.effects = effects;
    pic.caption = caption;
    pic.img_dim = img_dim;
    pic.reverse = reverse;
    pic.lock = lock;

    Ok(Control::Picture(Box::new(pic)))
}


pub(crate) fn parse_picture_effects(reader: &mut Reader<&[u8]>) -> Result<PictureEffects, HwpxError> {
    let mut effects = PictureEffects::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"shadow" => {
                effects.shadow = Some(parse_picture_shadow(e, reader)?);
            }
            Ok(Event::Empty(ref e)) if local_name(e.name().as_ref()) == b"shadow" => {
                effects.shadow = Some(parse_picture_shadow_attrs(e));
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"effects" => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("effects: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(effects)
}


pub(crate) fn parse_picture_shadow(
    e: &quick_xml::events::BytesStart<'_>,
    reader: &mut Reader<&[u8]>,
) -> Result<PictureShadow, HwpxError> {
    let mut shadow = parse_picture_shadow_attrs(e);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => match local_name(e.name().as_ref()) {
                b"skew" => shadow.skew = Some(parse_effect_point(e)),
                b"scale" => shadow.scale = Some(parse_effect_point(e)),
                b"effectsColor" => {
                    shadow.color = Some(parse_effect_color_attrs(e));
                }
                _ => {}
            },
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"effectsColor" => {
                shadow.color = Some(parse_effect_color(e, reader)?);
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"shadow" => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("shadow: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(shadow)
}


pub(crate) fn parse_picture_transparency_attr(raw: &str) -> u8 {
    let Ok(value) = raw.trim().parse::<f64>() else {
        return 0;
    };
    if !value.is_finite() {
        return 0;
    }
    if value <= 1.0 {
        (value * 100.0).round().clamp(0.0, 100.0) as u8
    } else {
        let alpha = value.clamp(0.0, 255.0).round() as u8;
        crate::model::image::alpha_byte_to_transparency_percent(alpha)
    }
}


pub(crate) fn parse_picture_shadow_attrs(e: &quick_xml::events::BytesStart<'_>) -> PictureShadow {
    let mut shadow = PictureShadow::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"style" => shadow.style = Some(attr_str(&attr)),
            b"alpha" => shadow.alpha = Some(attr_str(&attr)),
            b"radius" => shadow.radius = Some(attr_str(&attr)),
            b"direction" => shadow.direction = Some(attr_str(&attr)),
            b"distance" => shadow.distance = Some(attr_str(&attr)),
            b"alignStyle" => shadow.align_style = Some(attr_str(&attr)),
            b"rotationStyle" => shadow.rotation_style = Some(attr_str(&attr)),
            _ => {}
        }
    }
    shadow
}


#[derive(Clone, Copy)]
pub(crate) enum ShapeStorageKind {
    Picture,
    Group,
    Drawing,
    TextBoxDrawing,
}


#[derive(Default)]
pub(crate) struct ObjectElementIds {
    instid: u32,
    round_rate: u8,
    is_reverse_hv: bool,
}


/// HWPX 일부 샘플은 `<hp:curSz width="0" height="0">`를 기록하면서 실제 크기는
/// `<hp:orgSz>`와 `renderingInfo` scale로 표현한다. HWP 저장/재로드 경로에서는
/// current size 0이 effective size 0으로 해석되므로, 저장 가능한 IR에서는 current
/// size를 org size로 materialize한다.
pub(crate) fn materialize_shape_current_size_from_original(
    common: &mut CommonObjAttr,
    shape_attr: &mut ShapeComponentAttr,
) {
    if shape_attr.current_width == 0 && shape_attr.original_width > 0 {
        shape_attr.current_width = shape_attr.original_width;
        // [#2017] HWPX 재직렬화 시 원본 curSz=0 을 복원하기 위해 materialize 여부를 기록.
        shape_attr.current_width_was_zero = true;
        if common.width == 0 {
            common.width = shape_attr.original_width;
        }
    }
    if shape_attr.current_height == 0 && shape_attr.original_height > 0 {
        shape_attr.current_height = shape_attr.original_height;
        shape_attr.current_height_was_zero = true;
        if common.height == 0 {
            common.height = shape_attr.original_height;
        }
    }
}


/// HWP SHAPE_COMPONENT 저장 경로가 기대하는 storage 전용 필드를 materialize한다.
///
/// HWPX에는 같은 정보가 `flip`, `rotationInfo`, `imgRect` 같은 XML 자식 요소로
/// 분산되어 있다. 이 값을 SHAPE_COMPONENT 레코드 필드에 싣지 않으면 한컴은 그림/그룹
/// 개체 이후의 레코드 스트림을 정상적으로 이어 읽지 못하는 케이스가 있다.
pub(crate) fn materialize_shape_hwp_storage_defaults(
    common: &mut CommonObjAttr,
    shape_attr: &mut ShapeComponentAttr,
    kind: ShapeStorageKind,
) {
    materialize_shape_current_size_from_original(common, shape_attr);
    common.attr = pack_hwpx_common_obj_attr(common);

    if shape_attr.local_file_version == 0
        && (shape_attr.original_width > 0
            || shape_attr.original_height > 0
            || shape_attr.current_width > 0
            || shape_attr.current_height > 0
            || common.width > 0
            || common.height > 0)
    {
        shape_attr.local_file_version = 1;
    }

    if shape_attr.flip == 0 {
        let mut flip = match kind {
            // HWPX에는 HWP5 SHAPE_COMPONENT의 저장 전용 상위 비트가 없다. Hancom
            // 2020이 같은 HWPX를 HWP5로 저장한 값은 그림=0x2000_0000, 글상자
            // 도형=0x0100_0000이다. 그룹 자식에는 0x0003_0000도 함께 붙는다.
            // 0x2400_0000을 쓴 종전 값은 표지 묶음의 자식 좌표계를 다르게 해석하게
            // 하여 한컴 PDF에서 축척·위치를 틀리게 만들었다(#3930).
            ShapeStorageKind::Picture => 0x2000_0000,
            ShapeStorageKind::Group => 0x0009_0000,
            ShapeStorageKind::TextBoxDrawing => 0x0100_0000,
            ShapeStorageKind::Drawing => 0,
        };
        if shape_attr.group_level > 0
            && matches!(
                kind,
                ShapeStorageKind::Picture | ShapeStorageKind::TextBoxDrawing
            )
        {
            flip |= 0x0003_0000;
        }
        if shape_attr.horz_flip {
            flip |= 0x01;
        }
        if shape_attr.vert_flip {
            flip |= 0x02;
        }
        shape_attr.flip = flip;
    }

    if shape_attr.rotate_image {
        shape_attr.flip |= 0x0008_0000;
    }
}


/// `<hp:pic>`, `<hp:rect>`, `<hp:container>` 등 개체의 공통 속성을 요소 속성에서 파싱한다.
pub(crate) fn parse_object_element_attrs(
    e: &quick_xml::events::BytesStart,
    common: &mut CommonObjAttr,
    shape_attr: &mut ShapeComponentAttr,
) -> ObjectElementIds {
    common.hwp5_gen_shape_attr_bit26 = true;
    let mut ids = ObjectElementIds::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"id" => common.instance_id = parse_u32(&attr),
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
            b"instid" => ids.instid = parse_u32(&attr),
            b"groupLevel" => shape_attr.group_level = attr_str(&attr).parse().unwrap_or(0),
            b"ratio" => ids.round_rate = parse_u8(&attr).min(100),
            // [Task #1379] numberingType (캡션 번호 범주) 보존 — exam_kor 등 광범위 사용.
            b"numberingType" => {
                common.numbering_type = match attr_str(&attr).to_ascii_uppercase().as_str() {
                    "PICTURE" => crate::model::shape::ObjectNumberingType::Picture,
                    "TABLE" => crate::model::shape::ObjectNumberingType::Table,
                    "EQUATION" => crate::model::shape::ObjectNumberingType::Equation,
                    _ => crate::model::shape::ObjectNumberingType::None,
                };
            }
            // 선/연결선의 방향 뒤집기(isReverseHV). serializer 는 방출하나 파서가
            // 되읽지 않아 HWPX 원본 선의 방향 반전이 왕복 시 유실됐다.
            b"isReverseHV" => ids.is_reverse_hv = attr_str(&attr) == "1",
            // [#2840] 개체 잠금(lock) — 종전 미파싱으로 <hp:equation> 직렬화 시
            // 항상 "0"으로 되돌아가 원본의 잠금 상태가 유실됐다.
            b"lock" => common.locked = attr_str(&attr) == "1",
            _ => {}
        }
    }

    if common.instance_id == 0 && ids.instid != 0 {
        common.instance_id = ids.instid;
    }

    // HWP5 공통 개체 attr bit 28은 한컴 2020이 `numberingType="PICTURE"`인
    // 일반 도형/그림/묶음을 HWP로 저장할 때 함께 기록한다. 차트·OLE 경로는 이미
    // 같은 보정을 하지만, 공용 개체 경로에서 빠지면 HWPX -> HWP 저장본의 바탕쪽과
    // 본문 PICTURE 개체가 한컴 저장본과 다른 attr을 갖는다.
    if common.numbering_type == crate::model::shape::ObjectNumberingType::Picture {
        common.hwp5_gen_shape_attr_bit28 = true;
    }

    ids
}


/// 개체 자식 요소에서 공통 레이아웃 속성(pos, sz, curSz, orgSz, offset, outMargin)을 파싱한다.
pub(crate) fn parse_object_layout_child(
    local: &[u8],
    ce: &quick_xml::events::BytesStart,
    common: &mut CommonObjAttr,
    shape_attr: &mut ShapeComponentAttr,
    has_pos: &mut bool,
) {
    match local {
        b"sz" => {
            for attr in ce.attributes().flatten() {
                match attr.key.as_ref() {
                    b"width" => {
                        let v = parse_u32(&attr);
                        if v > 0 {
                            common.width = v;
                        }
                    }
                    b"height" => {
                        let v = parse_u32(&attr);
                        if v > 0 {
                            common.height = v;
                        }
                    }
                    b"widthRelTo" => {
                        common.width_criterion = parse_size_criterion(&attr_str(&attr), true);
                    }
                    b"heightRelTo" => {
                        common.height_criterion = parse_size_criterion(&attr_str(&attr), false);
                    }
                    b"protect" => common.size_protect = parse_bool(&attr),
                    _ => {}
                }
            }
        }
        b"curSz" => {
            for attr in ce.attributes().flatten() {
                match attr.key.as_ref() {
                    b"width" => {
                        let v = parse_u32(&attr);
                        shape_attr.current_width = v;
                        if v > 0 {
                            common.width = v;
                        }
                    }
                    b"height" => {
                        let v = parse_u32(&attr);
                        shape_attr.current_height = v;
                        if v > 0 {
                            common.height = v;
                        }
                    }
                    _ => {}
                }
            }
        }
        b"orgSz" => {
            for attr in ce.attributes().flatten() {
                match attr.key.as_ref() {
                    b"width" => {
                        let v = parse_u32(&attr);
                        shape_attr.original_width = v;
                        if common.width == 0 {
                            common.width = v;
                        }
                    }
                    b"height" => {
                        let v = parse_u32(&attr);
                        shape_attr.original_height = v;
                        if common.height == 0 {
                            common.height = v;
                        }
                    }
                    _ => {}
                }
            }
        }
        b"pos" => {
            *has_pos = true;
            for attr in ce.attributes().flatten() {
                match attr.key.as_ref() {
                    b"treatAsChar" => {
                        common.treat_as_char = attr_str(&attr) == "1" || attr_str(&attr) == "true";
                    }
                    // [#2784] affectLSpacing(줄 간격에 영향) — 공통 개체 pos 되읽기.
                    b"affectLSpacing" => common.affect_line_spacing = parse_bool(&attr),
                    b"flowWithText" => common.flow_with_text = parse_bool(&attr),
                    b"allowOverlap" => common.allow_overlap = parse_bool(&attr),
                    // holdAnchorAndSO(쪽나눔 방지). 방출측은 모든 개체에 내지만
                    // 종전엔 표 파서만 되읽어 개체 배치에선 prevent_page_break 이 유실됐다.
                    b"holdAnchorAndSO" => {
                        common.prevent_page_break = if parse_bool(&attr) { 1 } else { 0 };
                    }
                    b"vertRelTo" => {
                        common.vert_rel_to = match attr_str(&attr).as_str() {
                            "PAPER" => VertRelTo::Paper,
                            "PAGE" => VertRelTo::Page,
                            "PARA" => VertRelTo::Para,
                            _ => VertRelTo::Para,
                        };
                    }
                    b"horzRelTo" => {
                        common.horz_rel_to = match attr_str(&attr).as_str() {
                            "PAPER" => HorzRelTo::Paper,
                            "PAGE" => HorzRelTo::Page,
                            "COLUMN" => HorzRelTo::Column,
                            "PARA" => HorzRelTo::Para,
                            _ => HorzRelTo::Para,
                        };
                    }
                    b"vertAlign" => {
                        common.vert_align = match attr_str(&attr).as_str() {
                            "TOP" => VertAlign::Top,
                            "CENTER" => VertAlign::Center,
                            "BOTTOM" => VertAlign::Bottom,
                            "INSIDE" => VertAlign::Inside,
                            "OUTSIDE" => VertAlign::Outside,
                            _ => VertAlign::Top,
                        };
                    }
                    b"horzAlign" => {
                        common.horz_align = match attr_str(&attr).as_str() {
                            "LEFT" => HorzAlign::Left,
                            "CENTER" => HorzAlign::Center,
                            "RIGHT" => HorzAlign::Right,
                            "INSIDE" => HorzAlign::Inside,
                            "OUTSIDE" => HorzAlign::Outside,
                            _ => HorzAlign::Left,
                        };
                    }
                    b"vertOffset" => common.vertical_offset = parse_i32_wrapping(&attr) as u32,
                    b"horzOffset" => common.horizontal_offset = parse_i32_wrapping(&attr) as u32,
                    _ => {}
                }
            }
        }
        b"offset" => {
            for attr in ce.attributes().flatten() {
                match attr.key.as_ref() {
                    b"x" => {
                        let v = parse_i32_wrapping(&attr);
                        shape_attr.offset_x = v;
                        if !*has_pos {
                            common.horizontal_offset = v as u32;
                        }
                    }
                    b"y" => {
                        let v = parse_i32_wrapping(&attr);
                        shape_attr.offset_y = v;
                        if !*has_pos {
                            common.vertical_offset = v as u32;
                        }
                    }
                    _ => {}
                }
            }
        }
        b"outMargin" => {
            for attr in ce.attributes().flatten() {
                match attr.key.as_ref() {
                    b"left" => common.margin.left = parse_i16(&attr),
                    b"right" => common.margin.right = parse_i16(&attr),
                    b"top" => common.margin.top = parse_i16(&attr),
                    b"bottom" => common.margin.bottom = parse_i16(&attr),
                    _ => {}
                }
            }
        }
        b"flip" => parse_shape_flip(ce, shape_attr),
        b"rotationInfo" => parse_shape_rotation_info(ce, shape_attr),
        _ => {}
    }
}


pub(crate) fn parse_shape_flip(e: &quick_xml::events::BytesStart, shape_attr: &mut ShapeComponentAttr) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"horizontal" => shape_attr.horz_flip = parse_bool(&attr),
            b"vertical" => shape_attr.vert_flip = parse_bool(&attr),
            _ => {}
        }
    }

    if shape_attr.flip != 0 {
        if shape_attr.horz_flip {
            shape_attr.flip |= 0x01;
        } else {
            shape_attr.flip &= !0x01;
        }
        if shape_attr.vert_flip {
            shape_attr.flip |= 0x02;
        } else {
            shape_attr.flip &= !0x02;
        }
    }
}


pub(crate) fn parse_shape_rotation_info(
    e: &quick_xml::events::BytesStart,
    shape_attr: &mut ShapeComponentAttr,
) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"angle" => shape_attr.rotation_angle = parse_i16(&attr),
            b"centerX" => shape_attr.rotation_center.x = parse_i32(&attr),
            b"centerY" => shape_attr.rotation_center.y = parse_i32(&attr),
            b"rotateimage" => shape_attr.rotate_image = parse_bool(&attr),
            _ => {}
        }
    }
}


pub(crate) fn parse_picture_img_rect(
    reader: &mut Reader<&[u8]>,
    border_x: &mut [i32; 4],
    border_y: &mut [i32; 4],
) -> Result<(), HwpxError> {
    let mut pts = [(0i32, 0i32); 4];
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let index = match local_name(ce.name().as_ref()) {
                    b"pt0" => Some(0),
                    b"pt1" => Some(1),
                    b"pt2" => Some(2),
                    b"pt3" => Some(3),
                    _ => None,
                };
                if let Some(index) = index {
                    for attr in ce.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"x" => pts[index].0 = parse_i32(&attr),
                            b"y" => pts[index].1 = parse_i32(&attr),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref ee)) => {
                if local_name(ee.name().as_ref()) == b"imgRect" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("imgRect: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // HWP SHAPE_PICTURE 레코드는 HWPX 꼭짓점을 x/y 배열이 아니라 4개 스칼라씩
    // 앞뒤로 나누어 저장한다. 한컴 변환 정답지와 같은 순서로 materialize한다.
    *border_x = [pts[0].0, pts[0].1, pts[1].0, pts[1].1];
    *border_y = [pts[2].0, pts[2].1, pts[3].0, pts[3].1];

    Ok(())
}


/// OWPML Core `ArrowType` (`headStyle`/`tailStyle`) → hwplib `LineArrowShape` 코드.
/// `src/serializer/hwpx/shape.rs::arrow_style_str` 의 역매핑 — fill 여부는 별도
/// `headfill`/`tailfill` 속성(bit 30/31)이 담당하므로 여기서는 모양 코드만 뽑는다.
pub(crate) fn arrow_shape_code(value: &str) -> u32 {
    match value {
        "ARROW" => 1,
        "SPEAR" => 2,
        "CONCAVE_ARROW" => 3,
        "EMPTY_DIAMOND" | "FILLED_DIAMOND" => 4,
        "EMPTY_CIRCLE" | "FILLED_CIRCLE" => 5,
        "EMPTY_BOX" | "FILLED_BOX" => 6,
        _ => 0, // NORMAL 및 미인식 값
    }
}


/// `<hp:lineShape>` 요소에서 ShapeBorderLine을 파싱한다.
pub(crate) fn parse_line_shape_attr(e: &quick_xml::events::BytesStart) -> ShapeBorderLine {
    fn arrow_size(value: &str) -> Option<u32> {
        match value {
            "SMALL_SMALL" => Some(0),
            "SMALL_MEDIUM" => Some(1),
            "SMALL_BIG" | "SMALL_LARGE" => Some(2),
            "MEDIUM_SMALL" => Some(3),
            "MEDIUM_MEDIUM" => Some(4),
            "MEDIUM_BIG" | "MEDIUM_LARGE" => Some(5),
            "BIG_SMALL" | "LARGE_SMALL" => Some(6),
            "BIG_MEDIUM" | "LARGE_MEDIUM" => Some(7),
            "BIG_BIG" | "LARGE_LARGE" => Some(8),
            _ => None,
        }
    }

    let mut bl = ShapeBorderLine::default();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"color" => bl.color = parse_color(&attr),
            b"width" => bl.width = parse_i32(&attr),
            b"style" => {
                // 선 스타일 → attr 비트 플래그 (하위 바이트)
                let style_val: u32 = match attr_str(&attr).as_str() {
                    // 정본 코드 0=NONE(표 borderFill·HWP5 doc_info 와 동일). 종전 0x40 은
                    // bit 6 이 endCap(bit 6~9)에 겹쳐 써져 소실됐다(#1531).
                    "NONE" => 0,
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
                    _ => 1,
                };
                bl.attr = (bl.attr & !0xFF) | style_val;
            }
            b"endCap" => {
                let end_cap: u32 = match attr_str(&attr).as_str() {
                    "ROUND" => 0,
                    "FLAT" => 1,
                    "SQUARE" => 2,
                    _ => 0,
                };
                bl.attr = (bl.attr & !(0x0F << 6)) | ((end_cap & 0x0F) << 6);
            }
            b"headStyle" => {
                // 화살표 시작(head) 모양 → bit 10~15 (hwplib LineArrowShape, utils.rs::arrow_type_from_hwp 참조)
                let code = arrow_shape_code(&attr_str(&attr));
                bl.attr = (bl.attr & !(0x3F << 10)) | ((code & 0x3F) << 10);
            }
            b"tailStyle" => {
                // 화살표 끝(tail) 모양 → bit 16~21
                let code = arrow_shape_code(&attr_str(&attr));
                bl.attr = (bl.attr & !(0x3F << 16)) | ((code & 0x3F) << 16);
            }
            b"headfill" => {
                // head(시작) 채움 → bit 30 (utils.rs::drawing_to_line_style 의 start_fill 과 정합)
                if parse_bool(&attr) {
                    bl.attr |= 0x4000_0000;
                } else {
                    bl.attr &= !0x4000_0000;
                }
            }
            b"tailfill" => {
                // tail(끝) 채움 → bit 31 (utils.rs::drawing_to_line_style 의 end_fill 과 정합)
                if parse_bool(&attr) {
                    bl.attr |= 0x8000_0000;
                } else {
                    bl.attr &= !0x8000_0000;
                }
            }
            b"headSz" => {
                if let Some(size) = arrow_size(&attr_str(&attr)) {
                    bl.attr = (bl.attr & !(0x0F << 22)) | ((size & 0x0F) << 22);
                }
            }
            b"tailSz" => {
                if let Some(size) = arrow_size(&attr_str(&attr)) {
                    bl.attr = (bl.attr & !(0x0F << 26)) | ((size & 0x0F) << 26);
                }
            }
            b"outlineStyle" => {
                bl.outline_style = match attr_str(&attr).as_str() {
                    "NORMAL" => 0,
                    "OUTER" => 1,
                    "INNER" => 2,
                    _ => 0,
                };
            }
            _ => {}
        }
    }
    bl
}


/// shape 내부의 `<hp:fillBrush>` 자식 요소를 파싱하여 Fill을 반환한다.
pub(crate) fn parse_shape_fill_brush(reader: &mut Reader<&[u8]>) -> Result<Fill, HwpxError> {
    use crate::model::style::{FillType, GradientFill, ImageFill, ImageFillMode, SolidFill};
    let mut fill = Fill::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref ce)) | Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"winBrush" => {
                        fill.fill_type = FillType::Solid;
                        let mut solid = SolidFill {
                            pattern_type: -1,
                            ..SolidFill::default()
                        };
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"faceColor" => solid.background_color = parse_color(&attr),
                                b"hatchColor" => solid.pattern_color = parse_color(&attr),
                                b"hatchStyle" => {
                                    if let Some(pattern_type) = parse_hatch_style(&attr_str(&attr))
                                    {
                                        solid.pattern_type = pattern_type;
                                    }
                                }
                                b"alpha" => {
                                    let val = attr_str(&attr);
                                    if let Ok(f) = val.parse::<f64>() {
                                        fill.alpha = (f.clamp(0.0, 1.0) * 255.0) as u8;
                                    }
                                }
                                _ => {}
                            }
                        }
                        fill.solid = Some(solid);
                    }
                    b"gradation" => {
                        fill.fill_type = FillType::Gradient;
                        let mut grad = GradientFill::default();
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => {
                                    grad.gradient_type = parse_gradient_type(&attr_str(&attr))
                                }
                                b"angle" => grad.angle = parse_i16(&attr),
                                b"centerX" => grad.center_x = parse_i16(&attr),
                                b"centerY" => grad.center_y = parse_i16(&attr),
                                b"blur" | b"step" => grad.blur = parse_i16(&attr),
                                b"stepCenter" => grad.step_center = parse_u8(&attr),
                                b"alpha" => {
                                    let val = attr_str(&attr);
                                    if let Ok(f) = val.parse::<f64>() {
                                        fill.alpha = (f.clamp(0.0, 1.0) * 255.0) as u8;
                                    }
                                }
                                _ => {}
                            }
                        }
                        fill.gradient = Some(grad);
                    }
                    b"color" => {
                        // <hc:color value="#RRGGBB"/> -- shape gradation child.
                        // Header BorderFill already handles the same construct; shape-local
                        // fillBrush needs the same color stop materialization for rendering.
                        if let Some(ref mut grad) = fill.gradient {
                            for attr in ce.attributes().flatten() {
                                if attr.key.as_ref() == b"value" {
                                    grad.colors.push(parse_color(&attr));
                                }
                            }
                        }
                    }
                    b"imgBrush" => {
                        fill.fill_type = FillType::Image;
                        let mut img = ImageFill::default();
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                // [#2563] 헤더(borderFill) 파서와 동일한 12종 매핑.
                                // 종전엔 4종만 받아 TOTAL 등 8종이 TILE 로 붕괴했다.
                                b"mode" => {
                                    img.fill_mode = match attr_str(&attr).as_str() {
                                        "TILE" | "TILE_ALL" => ImageFillMode::TileAll,
                                        "TILE_HORZ_TOP" => ImageFillMode::TileHorzTop,
                                        "TILE_HORZ_BOTTOM" => ImageFillMode::TileHorzBottom,
                                        "TILE_VERT_LEFT" => ImageFillMode::TileVertLeft,
                                        "TILE_VERT_RIGHT" => ImageFillMode::TileVertRight,
                                        "CENTER" => ImageFillMode::Center,
                                        "CENTER_TOP" => ImageFillMode::CenterTop,
                                        "CENTER_BOTTOM" => ImageFillMode::CenterBottom,
                                        "FIT" | "FIT_TO_SIZE" | "STRETCH" => {
                                            ImageFillMode::FitToSize
                                        }
                                        "TOTAL" => ImageFillMode::Total,
                                        "TOP_LEFT_ALIGN" => ImageFillMode::LeftTop,
                                        _ => ImageFillMode::TileAll,
                                    };
                                }
                                _ => {}
                            }
                        }
                        fill.image = Some(img);
                    }
                    // [#2563] <hc:imgBrush> 의 <hc:img> 자식. 종전엔 이 arm 이 없어
                    // binaryItemIDRef/bright/contrast/effect 가 전부 버려졌고,
                    // bin_data_id 가 0 이라 직렬화가 <hc:img> 를 아예 못 내
                    // 이미지로 채운 도형이 왕복 후 빈 도형이 됐다.
                    // 헤더(borderFill) 파서 header.rs 의 b"img" arm 과 동형.
                    b"img" | b"image" => {
                        if let Some(ref mut img_fill) = fill.image {
                            for attr in ce.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"binaryItemIDRef" => {
                                        let val = attr_str(&attr);
                                        let num: String =
                                            val.chars().filter(|c| c.is_ascii_digit()).collect();
                                        img_fill.bin_data_id = num.parse().unwrap_or(0);
                                    }
                                    b"bright" => img_fill.brightness = parse_i8(&attr),
                                    b"contrast" => img_fill.contrast = parse_i8(&attr),
                                    b"effect" => {
                                        img_fill.effect = match attr_str(&attr).as_str() {
                                            "GRAY_SCALE" => 1,
                                            "BLACK_WHITE" => 2,
                                            _ => 0, // REAL_PIC
                                        };
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                if local_name(ee.name().as_ref()) == b"fillBrush" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("fillBrush: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(fill)
}


pub(crate) fn parse_shape_shadow_attr(e: &quick_xml::events::BytesStart) -> (u32, u32, i32, i32, u8) {
    let mut shadow_type = 0_u32;
    let mut shadow_color = 0_u32;
    let mut shadow_offset_x = 0_i32;
    let mut shadow_offset_y = 0_i32;
    let mut shadow_alpha = 0_u8;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"type" => {
                shadow_type = match attr_str(&attr).as_str() {
                    "NONE" => 0,
                    "LEFT_TOP" => 1,
                    "RIGHT_TOP" => 2,
                    "LEFT_BOTTOM" => 3,
                    "RIGHT_BOTTOM" => 4,
                    "CENTER" | "INSIDE" | "OUTSIDE" => 5,
                    _ => 0,
                };
            }
            b"color" => shadow_color = parse_color(&attr),
            b"offsetX" => shadow_offset_x = parse_i32(&attr),
            b"offsetY" => shadow_offset_y = parse_i32(&attr),
            b"alpha" => {
                let raw = attr_str(&attr);
                shadow_alpha = raw
                    .parse::<f64>()
                    .map(|value| {
                        if value <= 1.0 {
                            (value.clamp(0.0, 1.0) * 255.0) as u8
                        } else {
                            value.clamp(0.0, 255.0) as u8
                        }
                    })
                    .unwrap_or(0);
            }
            _ => {}
        }
    }

    (
        shadow_type,
        shadow_color,
        shadow_offset_x,
        shadow_offset_y,
        shadow_alpha,
    )
}


/// `<hp:rect>`, `<hp:ellipse>` 등 그리기 객체를 파싱하여 `Control::Shape`를 반환한다.
pub(crate) fn parse_shape_object(
    shape_type: &[u8],
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut common = CommonObjAttr::default();
    let mut shape_attr = ShapeComponentAttr::default();
    let mut border_line = ShapeBorderLine::default();
    let mut fill = Fill::default();
    let mut text_box: Option<TextBox> = None;
    let mut shadow_acc: Option<(u32, u32, i32, i32, u8)> = None;
    let mut has_pos = false;
    let mut x_coords = [0i32; 4];
    let mut y_coords = [0i32; 4];
    // [Task #1067] polygon / curve 의 가변 꼭짓점 `<hc:pt x=... y=.../>` 누적.
    // 기존 pt0/pt1/pt2/pt3 (rect 의 4 꼭짓점) 와 별개.
    let mut polygon_points: Vec<crate::model::Point> = Vec::new();
    // [#4676] curve 의 구간 종류(0: 직선, 1: 곡선) — `<hp:seg type>` 에서 채운다.
    let mut curve_segment_types: Vec<u8> = Vec::new();
    // [Task #1598] ellipse / arc 전용 지오메트리 (`<hc:center>`/`<hc:ax1>`/...).
    // 미적재 시 한글이 타원/호를 다르게 렌더 → 누적 레이아웃 변동 → 페이지 붕괴(#1589 잔여).
    let mut e_center = crate::model::Point::default();
    let mut e_axis1 = crate::model::Point::default();
    let mut e_axis2 = crate::model::Point::default();
    let mut e_start1 = crate::model::Point::default();
    let mut e_end1 = crate::model::Point::default();
    let mut e_start2 = crate::model::Point::default();
    let mut e_end2 = crate::model::Point::default();

    let object_ids = parse_object_element_attrs(e, &mut common, &mut shape_attr);
    let connect_line_type = parse_connect_line_type_attr(e);
    // [#4388] `<hp:arc>` 전용 `type` 속성 — 다른 태그의 동명 `type` 속성과
    // 섞이지 않도록 shape_type == b"arc" 로 한정한다.
    let arc_type = if shape_type == b"arc" {
        parse_arc_type_attr(e)
    } else {
        0
    };
    let mut connect_start_subject_id = 0_u32;
    let mut connect_start_subject_index = 0_u32;
    let mut connect_end_subject_id = 0_u32;
    let mut connect_end_subject_index = 0_u32;
    let mut connect_control_points = Vec::new();

    let tag_name = String::from_utf8_lossy(shape_type).to_string();
    let mut caption: Option<crate::model::shape::Caption> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"shapeComment" => {
                common.description = read_dutmal_text(reader, b"shapeComment")?;
            }
            // 도형 캡션 (#1403) — 미적재 시 roundtrip 에서 캡션 subList 소실
            Ok(Event::Start(ref ce)) if local_name(ce.name().as_ref()) == b"caption" => {
                caption = Some(parse_caption(ce, reader)?);
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
                    b"lineShape" => {
                        border_line = parse_line_shape_attr(ce);
                    }
                    b"drawText" => {
                        let mut tb = TextBox::default();
                        tb.max_width = common.width;
                        parse_draw_text(reader, &mut tb)?;
                        text_box = Some(tb);
                    }
                    b"pt0" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x_coords[0] = parse_i32(&attr),
                                b"y" => y_coords[0] = parse_i32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"pt1" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x_coords[1] = parse_i32(&attr),
                                b"y" => y_coords[1] = parse_i32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"pt2" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x_coords[2] = parse_i32(&attr),
                                b"y" => y_coords[2] = parse_i32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"pt3" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x_coords[3] = parse_i32(&attr),
                                b"y" => y_coords[3] = parse_i32(&attr),
                                _ => {}
                            }
                        }
                    }
                    // [Task #1067] polygon / curve 의 가변 꼭짓점 (<hc:pt x="..." y="..."/>).
                    // pt0/pt1/pt2/pt3 (rect 의 4 꼭짓점) 매칭 후 fall-through 로 본 분기 도달.
                    b"pt" => {
                        let mut px: i32 = 0;
                        let mut py: i32 = 0;
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => px = parse_i32(&attr),
                                b"y" => py = parse_i32(&attr),
                                _ => {}
                            }
                        }
                        polygon_points.push(crate::model::Point { x: px, y: py });
                    }
                    // [#1200] curve 의 가변 꼭짓점이 `<hp:seg x1 y1 x2 y2>` (점-대-점 chain)
                    // 으로 인코딩된 경우. `<hc:pt>` 미사용 curve 는 이 경로로 점을 채운다.
                    // seg 는 제어점이 아닌 sampled 꼭짓점이므로 폴리라인(LineTo)으로 재구성:
                    // 첫 seg 의 시작점 1회 + 각 seg 의 끝점.
                    b"seg" => {
                        let mut x1: i32 = 0;
                        let mut y1: i32 = 0;
                        let mut x2: i32 = 0;
                        let mut y2: i32 = 0;
                        // [#4676] 구간 종류(LINE/CURVE)를 보존한다 — 저장기가 seg 를 다시
                        // 쓸 때 이 값이 없으면 직선 구간이 곡선으로 바뀐다. 기본은 곡선.
                        let mut seg_kind: u8 = 1;
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x1" => x1 = parse_i32(&attr),
                                b"y1" => y1 = parse_i32(&attr),
                                b"x2" => x2 = parse_i32(&attr),
                                b"y2" => y2 = parse_i32(&attr),
                                b"type" => {
                                    if attr.value.as_ref() == b"LINE" {
                                        seg_kind = 0;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if polygon_points.is_empty() {
                            polygon_points.push(crate::model::Point { x: x1, y: y1 });
                        }
                        polygon_points.push(crate::model::Point { x: x2, y: y2 });
                        curve_segment_types.push(seg_kind);
                    }
                    b"startPt" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x_coords[0] = parse_i32(&attr),
                                b"y" => y_coords[0] = parse_i32(&attr),
                                b"subjectIDRef" => connect_start_subject_id = parse_u32(&attr),
                                b"subjectIdx" => connect_start_subject_index = parse_u32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"endPt" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x_coords[1] = parse_i32(&attr),
                                b"y" => y_coords[1] = parse_i32(&attr),
                                b"subjectIDRef" => connect_end_subject_id = parse_u32(&attr),
                                b"subjectIdx" => connect_end_subject_index = parse_u32(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"point" => {
                        let mut point = ConnectorControlPoint::default();
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => point.x = parse_i32(&attr),
                                b"y" => point.y = parse_i32(&attr),
                                b"type" => point.point_type = parse_u16(&attr),
                                _ => {}
                            }
                        }
                        connect_control_points.push(point);
                    }
                    // [Task #1598] ellipse / arc 전용 지오메트리. x/y 속성만 읽어 Point 채움.
                    b"center" => parse_xy(ce, &mut e_center),
                    b"ax1" => parse_xy(ce, &mut e_axis1),
                    b"ax2" => parse_xy(ce, &mut e_axis2),
                    b"start1" => parse_xy(ce, &mut e_start1),
                    b"end1" => parse_xy(ce, &mut e_end1),
                    b"start2" => parse_xy(ce, &mut e_start2),
                    b"end2" => parse_xy(ce, &mut e_end2),
                    b"renderingInfo" => {
                        parse_rendering_info(reader, &mut shape_attr)?;
                    }
                    b"fillBrush" => {
                        fill = parse_shape_fill_brush(reader)?;
                    }
                    b"shadow" => {
                        shadow_acc = Some(parse_shape_shadow_attr(ce));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                let eename = ee.name();
                if local_name(eename.as_ref()) == shape_type {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("{}: {}", tag_name, e))),
            _ => {}
        }
        buf.clear();
    }

    let storage_kind = if text_box.is_some() {
        ShapeStorageKind::TextBoxDrawing
    } else {
        ShapeStorageKind::Drawing
    };
    materialize_shape_hwp_storage_defaults(&mut common, &mut shape_attr, storage_kind);

    let (shadow_type, shadow_color, shadow_offset_x, shadow_offset_y, shadow_alpha) =
        shadow_acc.unwrap_or((0, 0, 0, 0, 0));

    let drawing = DrawingObjAttr {
        shape_attr,
        border_line,
        fill,
        shadow_type,
        shadow_color,
        shadow_offset_x,
        shadow_offset_y,
        shadow_alpha,
        inst_id: object_ids.instid,
        text_box,
        caption,
    };

    let shape = match shape_type {
        b"rect" => ShapeObject::Rectangle(RectangleShape {
            common,
            drawing,
            round_rate: object_ids.round_rate,
            x_coords,
            y_coords,
        }),
        b"ellipse" => ShapeObject::Ellipse(EllipseShape {
            common,
            drawing,
            // [Task #1598] 전용 지오메트리 적재 — 누락 시 한글 페이지 붕괴(#1589 잔여).
            center: e_center,
            axis1: e_axis1,
            axis2: e_axis2,
            start1: e_start1,
            end1: e_end1,
            start2: e_start2,
            end2: e_end2,
            ..Default::default()
        }),
        b"line" => ShapeObject::Line(LineShape {
            common,
            drawing,
            start: crate::model::Point {
                x: x_coords[0],
                y: y_coords[0],
            },
            end: crate::model::Point {
                x: x_coords[1],
                y: y_coords[1],
            },
            started_right_or_bottom: object_ids.is_reverse_hv,
            ..Default::default()
        }),
        b"connectLine" => ShapeObject::Line(LineShape {
            common,
            drawing,
            start: crate::model::Point {
                x: x_coords[0],
                y: y_coords[0],
            },
            end: crate::model::Point {
                x: x_coords[1],
                y: y_coords[1],
            },
            connector: Some(ConnectorData {
                link_type: connect_line_type,
                start_subject_id: connect_start_subject_id,
                start_subject_index: connect_start_subject_index,
                end_subject_id: connect_end_subject_id,
                end_subject_index: connect_end_subject_index,
                control_points: connect_control_points,
                raw_trailing: Vec::new(),
            }),
            started_right_or_bottom: object_ids.is_reverse_hv,
        }),
        b"arc" => ShapeObject::Arc(ArcShape {
            common,
            drawing,
            // [Task #1598] 호 전용 지오메트리(center/축).
            // [#4388] arc_type 은 `<hp:arc>` 자체의 `type` 속성(NORMAL/PIE/CHORD) —
            // 태그속성으로 읽는다(hancom-io/hwpx-owpml-model `ArcType.cpp` 확인).
            arc_type,
            center: e_center,
            axis1: e_axis1,
            axis2: e_axis2,
        }),
        b"polygon" => ShapeObject::Polygon(PolygonShape {
            common,
            drawing,
            // [Task #1067] HWPX `<hc:pt>` 점들을 PolygonShape::points 로 매핑.
            // 누락 시 polygon path 가 빈 상태로 렌더링되어 도형 미표시 (rhwp-studio + 한컴 둘 다).
            points: polygon_points,
            raw_trailing: Vec::new(),
        }),
        b"curve" => ShapeObject::Curve(CurveShape {
            common,
            drawing,
            // CurveShape 도 동일 패턴 — 누락 시 곡선 미표시.
            points: polygon_points,
            // [#4676] `<hp:seg type>` 에서 채운 구간 종류. 저장기가 seg 를 되돌릴 때 쓴다.
            segment_types: curve_segment_types,
        }),
        _ => ShapeObject::Rectangle(RectangleShape {
            common,
            drawing,
            round_rate: object_ids.round_rate,
            x_coords,
            y_coords,
        }),
    };

    Ok(Control::Shape(Box::new(shape)))
}


/// HWPX 양식 컨트롤 요소(`<hp:btn>`, `<hp:checkBtn>`, `<hp:radioBtn>`,
/// `<hp:comboBox>`, `<hp:edit>`)를 파싱하여 `Control::Form`으로 반환한다.
///
/// 요소는 `<hp:run>` 직접 자식으로 위치하며, `<hp:sz>` / `<hp:listItem>` /
/// `<hp:text>` / `<hp:formCharPr>` 등의 자식 요소를 포함한다.
pub(crate) fn parse_form_object(
    form_type: FormType,
    e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<Control, HwpxError> {
    let mut form = FormObject {
        form_type,
        enabled: true,
        ..Default::default()
    };

    // 요소 속성 파싱 (AbstractFormObjectType + AbstractButtonObjectType)
    // [Task #852 Stage 2.4] HWP5 직렬화에 필요한 ComboBox/Edit/Button 속성 보존
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"name" => form.name = attr_str(&attr),
            b"caption" => form.caption = attr_str(&attr),
            b"foreColor" => form.fore_color = parse_color(&attr),
            b"backColor" => form.back_color = parse_color(&attr),
            b"enabled" => form.enabled = parse_bool(&attr),
            // [Task #TBD] value 는 UNCHECKED/CHECKED/INDETERMINATE 3상태 열거형
            // (OWPML AbstractButtonObjectType). INDETERMINATE 를 UNCHECKED 로
            // 뭉개면 라운드트립 시 tri-state 체크박스의 중간 상태가 유실된다.
            b"value" => {
                form.value = match attr_str(&attr).as_str() {
                    "CHECKED" => 1,
                    "INDETERMINATE" => 2,
                    _ => 0,
                }
            }
            b"selectedValue" => form.text = attr_str(&attr), // comboBox 선택값
            // ComboBox 전용 속성 (HWP5 ComboBoxSet 직렬화에 필요)
            b"listBoxRows" => {
                form.properties
                    .insert("ListBoxRows".to_string(), attr_str(&attr));
            }
            b"listBoxWidth" => {
                form.properties
                    .insert("ListBoxWidth".to_string(), attr_str(&attr));
            }
            b"editEnable" => {
                form.properties
                    .insert("EditEnable".to_string(), attr_str(&attr));
            }
            // 공통 속성 (HWP5 CommonSet 직렬화에 필요)
            b"groupName" => {
                form.properties
                    .insert("GroupName".to_string(), attr_str(&attr));
            }
            b"tabStop" => {
                form.properties
                    .insert("TabStop".to_string(), attr_str(&attr));
            }
            b"editable" => {
                form.properties
                    .insert("Editable".to_string(), attr_str(&attr));
            }
            b"tabOrder" => {
                form.properties
                    .insert("TabOrder".to_string(), attr_str(&attr));
            }
            b"borderTypeIDRef" => {
                form.properties
                    .insert("BorderType".to_string(), attr_str(&attr));
            }
            b"drawFrame" => {
                form.properties
                    .insert("DrawFrame".to_string(), attr_str(&attr));
            }
            b"printable" => {
                form.properties
                    .insert("Printable".to_string(), attr_str(&attr));
            }
            b"command" => {
                form.properties
                    .insert("Command".to_string(), attr_str(&attr));
            }
            // 버튼류 전용 속성 (라운드트립 보존; writer 가 동일 키로 읽음)
            b"radioGroupName" => {
                form.properties
                    .insert("RadioGroupName".to_string(), attr_str(&attr));
            }
            b"triState" => {
                form.properties
                    .insert("TriState".to_string(), attr_str(&attr));
            }
            b"backStyle" => {
                form.properties
                    .insert("BackStyle".to_string(), attr_str(&attr));
            }
            // Edit 전용 속성 (라운드트립 보존)
            b"multiLine" => {
                form.properties
                    .insert("MultiLine".to_string(), attr_str(&attr));
            }
            b"passwordChar" => {
                form.properties
                    .insert("PasswordChar".to_string(), attr_str(&attr));
            }
            b"maxLength" => {
                form.properties
                    .insert("MaxLength".to_string(), attr_str(&attr));
            }
            b"scrollBars" => {
                form.properties
                    .insert("ScrollBars".to_string(), attr_str(&attr));
            }
            b"tabKeyBehavior" => {
                form.properties
                    .insert("TabKeyBehavior".to_string(), attr_str(&attr));
            }
            b"numOnly" => {
                form.properties
                    .insert("Number".to_string(), attr_str(&attr));
            }
            b"readOnly" => {
                form.properties
                    .insert("ReadOnly".to_string(), attr_str(&attr));
            }
            b"alignText" => {
                form.properties
                    .insert("AlignText".to_string(), attr_str(&attr));
            }
            _ => {}
        }
    }

    // 자식 요소 순회
    let end_tag = local_name(e.name().as_ref()).to_vec();
    let mut buf = Vec::new();
    // (value, displayText) 쌍으로 보존 — comboBox 항목
    let mut list_items: Vec<(String, String)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"text" => {
                        // <hp:text> 자식 (edit 컨트롤) — 텍스트 내용 읽기
                        let mut tbuf = Vec::new();
                        loop {
                            match reader.read_event_into(&mut tbuf) {
                                Ok(Event::Text(ref t)) => {
                                    if let Ok(s) = t.decode() {
                                        form.text.push_str(&s);
                                    }
                                }
                                // 양식 개체(edit 컨트롤) 텍스트의 CDATA 저장 형태.
                                // #2916 과 같은 결함 클래스 — 없으면 form.text 가 빈다.
                                Ok(Event::CData(ref cdata)) => {
                                    form.text.push_str(&String::from_utf8_lossy(cdata.as_ref()));
                                }
                                Ok(Event::GeneralRef(ref r)) => {
                                    form.text.push_str(&decode_xml_general_ref(r));
                                }
                                Ok(Event::End(_)) => break,
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                            tbuf.clear();
                        }
                    }
                    _ => {
                        skip_element(reader, local)?;
                    }
                }
            }
            Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"sz" => {
                        // <hp:sz width="..." widthRelTo="..." height="..." heightRelTo="..." protect="..."/>
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => form.width = parse_u32(&attr),
                                b"height" => form.height = parse_u32(&attr),
                                b"widthRelTo" => {
                                    form.properties
                                        .insert("SzWidthRelTo".to_string(), attr_str(&attr));
                                }
                                b"heightRelTo" => {
                                    form.properties
                                        .insert("SzHeightRelTo".to_string(), attr_str(&attr));
                                }
                                b"protect" => {
                                    form.properties
                                        .insert("SzProtect".to_string(), attr_str(&attr));
                                }
                                _ => {}
                            }
                        }
                    }
                    b"pos" => {
                        // <hp:pos .../> 앵커링 (표준 ShapePositionType 11속성) — 라운드트립 보존
                        for attr in ce.attributes().flatten() {
                            let key = match attr.key.as_ref() {
                                b"treatAsChar" => "PosTreatAsChar",
                                b"affectLSpacing" => "PosAffectLSpacing",
                                b"flowWithText" => "PosFlowWithText",
                                b"allowOverlap" => "PosAllowOverlap",
                                b"holdAnchorAndSO" => "PosHoldAnchorAndSO",
                                b"vertRelTo" => "PosVertRelTo",
                                b"horzRelTo" => "PosHorzRelTo",
                                b"vertAlign" => "PosVertAlign",
                                b"horzAlign" => "PosHorzAlign",
                                b"vertOffset" => "PosVertOffset",
                                b"horzOffset" => "PosHorzOffset",
                                _ => continue,
                            };
                            form.properties.insert(key.to_string(), attr_str(&attr));
                        }
                    }
                    b"outMargin" => {
                        // <hp:outMargin left=".." right=".." top=".." bottom=".."/> — 라운드트립 보존
                        for attr in ce.attributes().flatten() {
                            let key = match attr.key.as_ref() {
                                b"left" => "OutMarginLeft",
                                b"right" => "OutMarginRight",
                                b"top" => "OutMarginTop",
                                b"bottom" => "OutMarginBottom",
                                _ => continue,
                            };
                            form.properties.insert(key.to_string(), attr_str(&attr));
                        }
                    }
                    b"listItem" => {
                        // <hp:listItem displayText="..." value="..."/> (comboBox 항목)
                        let mut value = String::new();
                        let mut display = String::new();
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"value" => value = attr_str(&attr),
                                b"displayText" => display = attr_str(&attr),
                                _ => {}
                            }
                        }
                        list_items.push((value, display));
                    }
                    b"formCharPr" => {
                        // <hp:formCharPr charPrIDRef="0" followContext="0" autoSz="1" wordWrap="0"/>
                        // [Task #852 Stage 2.4] HWP5 CharShapeSet 직렬화에 필요한 속성 보존
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"charPrIDRef" => {
                                    form.properties
                                        .insert("CharShapeID".to_string(), attr_str(&attr));
                                }
                                b"followContext" => {
                                    form.properties
                                        .insert("FollowContext".to_string(), attr_str(&attr));
                                }
                                b"autoSz" => {
                                    form.properties
                                        .insert("AutoSize".to_string(), attr_str(&attr));
                                }
                                b"wordWrap" => {
                                    form.properties
                                        .insert("WordWrap".to_string(), attr_str(&attr));
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                if local_name(ee.name().as_ref()) == end_tag.as_slice() {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("form_object: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // comboBox 항목 목록(값 + 표시 텍스트)을 properties에 저장
    if !list_items.is_empty() {
        for (i, (value, display)) in list_items.iter().enumerate() {
            form.properties
                .insert(format!("listItem{}", i), value.clone());
            form.properties
                .insert(format!("listItemDisplay{}", i), display.clone());
        }
    }

    Ok(Control::Form(Box::new(form)))
}


pub(crate) fn apply_hwpx_ole_shape_component_contract(ole: &mut crate::model::shape::OleShape) {
    let extent_w = if ole.extent_x > 0 {
        ole.extent_x as u32
    } else {
        7200
    };
    let extent_h = if ole.extent_y > 0 {
        ole.extent_y as u32
    } else {
        7200
    };
    let shape_attr = &mut ole.drawing.shape_attr;
    shape_attr.ctrl_id = tags::SHAPE_OLE_ID;
    shape_attr.is_two_ctrl_id = true;
    if shape_attr.local_file_version == 0 {
        shape_attr.local_file_version = 1;
    }
    if shape_attr.original_width == 0 {
        shape_attr.original_width = extent_w;
    }
    if shape_attr.original_height == 0 {
        shape_attr.original_height = extent_h;
    }
    if shape_attr.current_width == 0 {
        shape_attr.current_width = shape_attr.original_width;
    }
    if shape_attr.current_height == 0 {
        shape_attr.current_height = shape_attr.original_height;
    }
}


/// `<hp:sz>`, `<hp:pos>`, `<hp:outMargin>` 등 공통 자식 요소를 공통 속성에 반영한다.
pub(crate) fn parse_common_shape_children(
    reader: &mut Reader<&[u8]>,
    common: &mut CommonObjAttr,
    end_tag: &[u8],
    // OLE 전용 `<hc:extent>`(원본 개체 크기) 수집용. 호출자(ole/chart)만 사용한다.
    // 종전엔 이 자식을 무시하고 호출자가 7200 을 하드코딩해 개체 크기가 유실됐다.
    extent_out: &mut Option<(i32, i32)>,
    // [#3546] `<hp:rotationInfo>` 수집용. 종전 미파싱으로 저장 시 기본값으로
    // 되쓰여 rotateimage="1" 등 원본 값이 뒤집혔다(#2726 sz 기준 유실과 동형).
    shape_attr_out: &mut ShapeComponentAttr,
    // [#4319] `<hp:caption>` 수집용. 종전엔 이 공용 자식 파서(차트·OLE 전용)에
    // caption arm 이 없어 캡션 subList 가 파싱 단계에서 완전히 유실됐다 —
    // 도형(parse_shape_object)·묶음(parse_container)·그림(parse_picture) 은 모두
    // 캡션을 읽지만 차트·OLE 만 빠져 있었다. HWP5 파서(parser/control/shape.rs:213,
    // 222)와 동형으로 drawing.caption 에 채운 뒤 호출자가 `.caption` 으로 정규화한다.
    caption_out: &mut Option<crate::model::shape::Caption>,
) -> Result<(), HwpxError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref ce)) | Ok(Event::Empty(ref ce)) => {
                let cname = ce.name();
                let local = local_name(cname.as_ref());
                match local {
                    b"extent" => {
                        let mut x = 0i32;
                        let mut y = 0i32;
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => x = parse_i32(&attr),
                                b"y" => y = parse_i32(&attr),
                                _ => {}
                            }
                        }
                        *extent_out = Some((x, y));
                    }
                    b"rotationInfo" => {
                        parse_shape_rotation_info(ce, shape_attr_out);
                    }
                    b"sz" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"width" => common.width = parse_u32(&attr),
                                b"height" => common.height = parse_u32(&attr),
                                // [#2726] 공용 자식 파서(차트·OLE)만 크기 기준 arm 이 없어
                                // 파싱 단계에서 유실됐다. 도형 공용 파서(같은 파일 2925/2928)·
                                // 표(1702/1706)·그림(#2712)과 동형이며, 높이는 동일하게
                                // allow_column_para=false 로 읽어 치역을 {Paper, Page,
                                // Absolute} 로 제한한다.
                                b"widthRelTo" => {
                                    common.width_criterion =
                                        parse_size_criterion(&attr_str(&attr), true);
                                }
                                b"heightRelTo" => {
                                    common.height_criterion =
                                        parse_size_criterion(&attr_str(&attr), false);
                                }
                                b"protect" => common.size_protect = parse_bool(&attr),
                                _ => {}
                            }
                        }
                    }
                    b"pos" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"vertRelTo" => {
                                    common.vert_rel_to = match attr_str(&attr).as_str() {
                                        "PAPER" => VertRelTo::Paper,
                                        "PAGE" => VertRelTo::Page,
                                        _ => VertRelTo::Para,
                                    };
                                }
                                b"horzRelTo" => {
                                    common.horz_rel_to = match attr_str(&attr).as_str() {
                                        "PAPER" => HorzRelTo::Paper,
                                        "PAGE" => HorzRelTo::Page,
                                        "COLUMN" => HorzRelTo::Column,
                                        _ => HorzRelTo::Para,
                                    };
                                }
                                b"vertAlign" => {
                                    common.vert_align = match attr_str(&attr).as_str() {
                                        "CENTER" => VertAlign::Center,
                                        "BOTTOM" => VertAlign::Bottom,
                                        "INSIDE" => VertAlign::Inside,
                                        "OUTSIDE" => VertAlign::Outside,
                                        _ => VertAlign::Top,
                                    };
                                }
                                b"horzAlign" => {
                                    common.horz_align = match attr_str(&attr).as_str() {
                                        "CENTER" => HorzAlign::Center,
                                        "RIGHT" => HorzAlign::Right,
                                        "INSIDE" => HorzAlign::Inside,
                                        "OUTSIDE" => HorzAlign::Outside,
                                        _ => HorzAlign::Left,
                                    };
                                }
                                // [버그 수정] chart/OLE 공용 <hp:pos> 파서만 유일하게 `parse_u32`
                                // 를 써서 음수 오프셋(왼쪽/위쪽 앵커 이탈)을 0 으로 뭉갰다 —
                                // 이미지·표 등 다른 개체 <hp:pos> 파서(위 parse_i32_wrapping 분기)
                                // 와 동형으로 맞춘다.
                                b"vertOffset" => {
                                    common.vertical_offset = parse_i32_wrapping(&attr) as u32
                                }
                                b"horzOffset" => {
                                    common.horizontal_offset = parse_i32_wrapping(&attr) as u32
                                }
                                b"treatAsChar" => common.treat_as_char = parse_bool(&attr),
                                // [#2784] affectLSpacing(줄 간격에 영향) — 공통 개체 pos 되읽기.
                                b"affectLSpacing" => common.affect_line_spacing = parse_bool(&attr),
                                b"flowWithText" => common.flow_with_text = parse_bool(&attr),
                                b"allowOverlap" => common.allow_overlap = parse_bool(&attr),
                                // holdAnchorAndSO(쪽나눔 방지). 방출측은 모든 개체에 내지만
                                // 종전엔 표 파서만 되읽어, 그림/도형/차트/OLE 는 prevent_page_break
                                // 이 0 으로 유실됐다(표 파서와 동형으로 보강).
                                b"holdAnchorAndSO" => {
                                    common.prevent_page_break =
                                        if parse_bool(&attr) { 1 } else { 0 };
                                }
                                _ => {}
                            }
                        }
                    }
                    b"outMargin" => {
                        for attr in ce.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"left" => common.margin.left = parse_i32(&attr) as i16,
                                b"right" => common.margin.right = parse_i32(&attr) as i16,
                                b"top" => common.margin.top = parse_i32(&attr) as i16,
                                b"bottom" => common.margin.bottom = parse_i32(&attr) as i16,
                                _ => {}
                            }
                        }
                    }
                    // 개체 설명문(대체 텍스트) — 방출측(write_shape_comment)은 OLE/차트에도
                    // <hp:shapeComment>를 쓰지만 이 공용 자식 파서에 arm 이 없어 되읽지
                    // 못하고 유실됐다(OLE 라운드트립 ir-diff 로 실측: HWP5→HWPX→재파싱 후
                    // shape comment 사라짐).
                    b"shapeComment" => {
                        common.description = read_dutmal_text(reader, b"shapeComment")?;
                    }
                    // [#4319] 캡션 — 미적재 시 라운드트립에서 캡션 subList 소실(다른
                    // 도형 변형과 동형, parse_shape_object/parse_container 참고).
                    b"caption" => {
                        *caption_out = Some(parse_caption(ce, reader)?);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref ee)) => {
                if local_name(ee.name().as_ref()) == end_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(HwpxError::XmlError(format!("shape_children: {}", e))),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}
