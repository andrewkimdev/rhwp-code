//! images — table_layout.rs 에서 무변동 이동
use super::*;

impl SvgRenderer {
    /// PageBackground/BorderFill 이미지를 fill_mode에 따라 렌더링한다.
    pub(crate) fn render_page_background_image(&mut self, img: &PageBackgroundImage, bbox: &BoundingBox) {
        // PageBackground RealPic 워터마크 프리셋은 한컴의 색상 있는 배경 워터마크에 맞춰
        // 색감 보정을 PNG 픽셀에 bake한 뒤 반투명으로 합성한다.
        let preserve_color_watermark = img.is_real_picture_watermark_tone_preset();
        // 쪽 배경의 밝기·대비는 그 자체로 워터마크 표식이 아니다. HWP3/HWP5의
        // 일반 배경 그림도 색조 값을 쓸 수 있으며, 이를 legacy opacity로 합성하면
        // 기준 PDF보다 지나치게 희게 사라진다. 검증된 RealPic 워터마크 프리셋만
        // 별도 반투명 합성을 적용한다.
        let detected_mime = detect_image_mime_type(&img.data);
        // BMP/PCX → PNG 재인코딩 (브라우저 호환성과 PCX white transparency 정합)
        let (render_bytes, render_mime): (std::borrow::Cow<[u8]>, &str) =
            if preserve_color_watermark {
                match real_picture_watermark_bytes_to_hancom_tone_png_bytes(&img.data) {
                    Some(png) => (std::borrow::Cow::Owned(png), "image/png"),
                    None => (std::borrow::Cow::Borrowed(&img.data[..]), detected_mime),
                }
            } else if detected_mime == "image/bmp" {
                match bmp_bytes_to_png_bytes(&img.data) {
                    Some(png) => (std::borrow::Cow::Owned(png), "image/png"),
                    None => (std::borrow::Cow::Borrowed(&img.data[..]), detected_mime),
                }
            } else if detected_mime == "image/x-pcx" {
                match pcx_bytes_to_png_bytes(&img.data) {
                    Some(png) => (std::borrow::Cow::Owned(png), "image/png"),
                    None => (std::borrow::Cow::Borrowed(&img.data[..]), detected_mime),
                }
            } else if detected_mime == "image/tiff" {
                match tiff_bytes_to_png_bytes(&img.data) {
                    Some(png) => (std::borrow::Cow::Owned(png), "image/png"),
                    None => (std::borrow::Cow::Borrowed(&img.data[..]), detected_mime),
                }
            } else if detected_mime == "application/postscript" {
                match crate::renderer::image_resolver::dos_eps_preview_bytes(&img.data) {
                    Some((mime, bytes)) => (std::borrow::Cow::Owned(bytes), mime),
                    None => (std::borrow::Cow::Borrowed(&img.data[..]), detected_mime),
                }
            } else {
                (std::borrow::Cow::Borrowed(&img.data[..]), detected_mime)
            };
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&*render_bytes);
        let data_uri = format!("data:{};base64,{}", render_mime, base64_data);

        let effect_filter_id = if preserve_color_watermark {
            None
        } else {
            self.ensure_image_effect_filter(img.effect)
        };
        if let Some(ref fid) = effect_filter_id {
            self.output
                .push_str(&format!("<g filter=\"url(#{})\">\n", fid));
        }
        let bc_filter_id = if preserve_color_watermark {
            None
        } else {
            let (brightness, contrast) = img.display_brightness_contrast();
            self.ensure_brightness_contrast_filter(brightness, contrast)
        };
        if let Some(ref fid) = bc_filter_id {
            self.output
                .push_str(&format!("<g filter=\"url(#{})\">\n", fid));
        }
        // 일반 RealPic 쪽 배경의 색조 조정은 불투명으로 유지한다. 기존 비-RealPic
        // 워터마크(GrayScale/Pattern 등)는 legacy opacity 계약을 계속 따른다.
        let needs_watermark_opacity = preserve_color_watermark
            || (!matches!(img.effect, crate::model::image::ImageEffect::RealPic)
                && img.is_watermark());
        if needs_watermark_opacity {
            let opacity = if preserve_color_watermark {
                REAL_PICTURE_WATERMARK_PAGE_OPACITY
            } else {
                LEGACY_IMAGE_WATERMARK_OPACITY
            };
            self.output
                .push_str(&format!("<g opacity=\"{}\">\n", opacity));
        }

        match img.fill_mode {
            // Total(HWPX "TOTAL")은 바이너리 채우기 유형 5(크기에 맞추어)의 HWPX
            // 표기로, FitToSize 와 같은 의미다 — 영역 전체로 늘려 채운다.
            ImageFillMode::FitToSize | ImageFillMode::Total | ImageFillMode::None => {
                self.output.push_str(&format!(
                    "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>\n",
                    bbox.x, bbox.y, bbox.width, bbox.height, data_uri,
                ));
            }
            ImageFillMode::TileAll => {
                self.render_tiled_image(&render_bytes, &data_uri, bbox, true, true, None);
            }
            ImageFillMode::TileHorzTop | ImageFillMode::TileHorzBottom => {
                self.render_tiled_image(&render_bytes, &data_uri, bbox, true, false, None);
            }
            ImageFillMode::TileVertLeft | ImageFillMode::TileVertRight => {
                self.render_tiled_image(&render_bytes, &data_uri, bbox, false, true, None);
            }
            _ => {
                self.render_positioned_image(&render_bytes, &data_uri, bbox, img.fill_mode, None);
            }
        }

        if needs_watermark_opacity {
            self.output.push_str("</g>\n");
        }
        if bc_filter_id.is_some() {
            self.output.push_str("</g>\n");
        }
        if effect_filter_id.is_some() {
            self.output.push_str("</g>\n");
        }
    }


    /// 이미지 노드를 fill_mode에 따라 렌더링한다.
    pub(crate) fn render_image_node(&mut self, img: &ImageNode, bbox: &super::super::render_tree::BoundingBox) {
        // [Task #741] 빈 binary 데이터 (외부 file path 그림 등) 도 placeholder 처리.
        // 한컴 한글 2024 viewer 정합 — 외부 file 못 찾는 경우 점선 사각형 + 깨진 image 아이콘.
        let data = match img.data {
            Some(ref d) if !d.is_empty() => d,
            _ => {
                // 이미지 데이터 부재 (None 또는 빈 vec) — placeholder 표시
                self.output.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#f0f0f0\" stroke=\"#999999\" stroke-dasharray=\"4\"/>\n",
                    bbox.x, bbox.y, bbox.width, bbox.height,
                ));
                // 외부 file path 그림: file path 표시 (가독성)
                if let Some(ref path) = img.external_path {
                    let cx = bbox.x + bbox.width / 2.0;
                    let cy = bbox.y + bbox.height / 2.0;
                    let escaped = path
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;");
                    self.output.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"#666666\" font-size=\"10\">[외부: {}]</text>\n",
                        cx, cy, escaped,
                    ));
                }
                return;
            }
        };

        // RealPic 워터마크 프리셋은 한컴의 색상 있는 배경 워터마크에 맞춰
        // 색감을 살린 뒤 반투명으로 합성한다. 표/셀 배경 fill은 쪽 배경보다
        // 더 투명하게 합성되는 샘플이 있어 opacity만 별도 프로파일을 사용한다.
        let preserve_color_watermark = img.is_real_picture_watermark_tone_preset();
        // [Issue #1156] 워터마크 판정 = 밝기·대비가 둘 다 0 이 아님 (effect 무관).
        let is_watermark_image = img.is_watermark();
        let mime_type = detect_image_mime_type(data);

        // WMF → SVG 변환 (브라우저는 WMF를 렌더링할 수 없으므로 SVG로 변환)
        // BMP → PNG 변환 (브라우저는 SVG <image> 내부의 data:image/bmp 미지원)
        // PCX → PNG 변환 (브라우저는 PCX 포맷을 native 렌더링하지 못함, Task #514)
        let (render_data, render_mime, baked_watermark): (std::borrow::Cow<[u8]>, &str, bool) =
            if preserve_color_watermark {
                match real_picture_watermark_fill_bytes_to_hancom_tone_png_bytes(data) {
                    Some(png_bytes) => (std::borrow::Cow::Owned(png_bytes), "image/png", true),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else if mime_type == "image/x-wmf" {
                match convert_wmf_to_svg(data) {
                    Some(svg_bytes) => (std::borrow::Cow::Owned(svg_bytes), "image/svg+xml", false),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else if mime_type == "image/x-emf" {
                match crate::emf::convert_to_standalone_svg(data) {
                    Some(svg_bytes) => (std::borrow::Cow::Owned(svg_bytes), "image/svg+xml", false),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else if mime_type == "image/bmp" {
                match bmp_bytes_to_png_bytes(data) {
                    Some(png_bytes) => (std::borrow::Cow::Owned(png_bytes), "image/png", false),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else if mime_type == "image/x-pcx" {
                match pcx_bytes_to_png_bytes(data) {
                    Some(png_bytes) => (std::borrow::Cow::Owned(png_bytes), "image/png", false),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else if mime_type == "image/tiff" {
                match tiff_bytes_to_png_bytes(data) {
                    Some(png_bytes) => (std::borrow::Cow::Owned(png_bytes), "image/png", false),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else if mime_type == "application/postscript" {
                match crate::renderer::image_resolver::dos_eps_preview_bytes(data) {
                    Some((mime, bytes)) => (std::borrow::Cow::Owned(bytes), mime, false),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else if is_watermark_image && mime_type == "image/jpeg" {
                match watermark_jpeg_bytes_to_hancom_baked_png_bytes(data) {
                    Some(png_bytes) => (std::borrow::Cow::Owned(png_bytes), "image/png", true),
                    None => (std::borrow::Cow::Borrowed(data), mime_type, false),
                }
            } else {
                (std::borrow::Cow::Borrowed(data), mime_type, false)
            };

        // 그림 효과(그레이스케일/흑백) → SVG 필터 래핑
        let effect_filter_id = if baked_watermark || preserve_color_watermark {
            None
        } else {
            self.ensure_image_effect_filter(img.effect)
        };
        if let Some(ref fid) = effect_filter_id {
            self.output
                .push_str(&format!("<g filter=\"url(#{})\">\n", fid));
        }
        let object_opacity = img.opacity.clamp(0.0, 1.0);
        if object_opacity < 1.0 {
            self.output
                .push_str(&format!("<g opacity=\"{:.3}\">\n", object_opacity));
        }
        // 밝기/대비 → SVG 필터 래핑
        // [Issue #677] 한컴 워터마크 효과 (effect != RealPic 이고 brightness/contrast 가
        // 비-zero) 는 저장값을 그대로 brightness/contrast 필터로 적용한다. JPEG 워터마크는
        // #976의 baked PNG 선보정이 성공하면 런타임 필터를 생략하고, RealPic 색상
        // 워터마크는 #975의 baked PNG 톤 보정으로 처리한다.
        let bc_filter_id = if baked_watermark || preserve_color_watermark {
            None
        } else {
            self.ensure_brightness_contrast_filter(img.brightness, img.contrast)
        };
        if let Some(ref fid) = bc_filter_id {
            self.output
                .push_str(&format!("<g filter=\"url(#{})\">\n", fid));
        }
        // 워터마크 반투명 영역. JPEG baked 워터마크는 이미 한컴 톤으로 픽셀화되어
        // 있으므로 추가 opacity를 적용하지 않는다.
        let needs_watermark_opacity =
            preserve_color_watermark || (is_watermark_image && !baked_watermark);
        if needs_watermark_opacity {
            let opacity = if preserve_color_watermark {
                REAL_PICTURE_WATERMARK_FILL_OPACITY
            } else {
                LEGACY_IMAGE_WATERMARK_OPACITY
            };
            self.output
                .push_str(&format!("<g opacity=\"{}\">\n", opacity));
        }

        let base64_data = base64::engine::general_purpose::STANDARD.encode(&*render_data);
        let data_uri = format!("data:{};base64,{}", render_mime, base64_data);

        let fill_mode = img.fill_mode.unwrap_or(ImageFillMode::FitToSize);

        match fill_mode {
            ImageFillMode::FitToSize | ImageFillMode::Total => {
                // 그림 자르기: crop이 있으면 원본 이미지의 일부만 표시
                if let Some((cl, ct, cr, cb)) = img.crop {
                    if let Some((img_w, img_h)) = parse_image_dimensions(&render_data) {
                        let img_w = img_w as f64;
                        let img_h = img_h as f64;
                        let (src_x, src_y, src_w, src_h) = compute_image_crop_src(
                            (cl, ct, cr, cb),
                            img.original_size_hu,
                            img_w,
                            img_h,
                        );
                        // 전체 이미지 대비 잘림이 있는지 확인
                        let is_cropped = src_x > 0.5
                            || src_y > 0.5
                            || (src_w - img_w).abs() > 1.0
                            || (src_h - img_h).abs() > 1.0;
                        if is_cropped {
                            // SVG: 중첩 svg + viewBox로 crop 영역만 표시
                            self.output.push_str(&format!(
                                "<svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"none\">\
                                <image width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/></svg>\n",
                                bbox.x, bbox.y, bbox.width, bbox.height,
                                src_x, src_y, src_w, src_h,
                                img_w, img_h, data_uri,
                            ));
                        } else {
                            self.output.push_str(&format!(
                                "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>\n",
                                bbox.x, bbox.y, bbox.width, bbox.height, data_uri,
                            ));
                        }
                    } else {
                        // 이미지 크기 파싱 실패 → crop 무시
                        self.output.push_str(&format!(
                            "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>\n",
                            bbox.x, bbox.y, bbox.width, bbox.height, data_uri,
                        ));
                    }
                } else {
                    // crop 없음: 기존 동작
                    self.output.push_str(&format!(
                        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>\n",
                        bbox.x, bbox.y, bbox.width, bbox.height, data_uri,
                    ));
                }
            }
            ImageFillMode::TileAll => {
                // 바둑판식으로-모두: 원래 크기로 전체 타일링
                self.render_tiled_image(
                    &render_data,
                    &data_uri,
                    bbox,
                    true,
                    true,
                    img.original_size,
                );
            }
            ImageFillMode::TileHorzTop | ImageFillMode::TileHorzBottom => {
                // 바둑판식으로-가로: 가로 방향만 타일링 (위 또는 아래 기준)
                self.render_tiled_image(
                    &render_data,
                    &data_uri,
                    bbox,
                    true,
                    false,
                    img.original_size,
                );
            }
            ImageFillMode::TileVertLeft | ImageFillMode::TileVertRight => {
                // 바둑판식으로-세로: 세로 방향만 타일링 (왼쪽 또는 오른쪽 기준)
                self.render_tiled_image(
                    &render_data,
                    &data_uri,
                    bbox,
                    false,
                    true,
                    img.original_size,
                );
            }
            _ => {
                // 배치 모드: 원래 크기대로 지정 위치에 배치
                self.render_positioned_image(
                    &render_data,
                    &data_uri,
                    bbox,
                    fill_mode,
                    img.original_size,
                );
            }
        }

        if needs_watermark_opacity {
            self.output.push_str("</g>\n");
        }
        if bc_filter_id.is_some() {
            self.output.push_str("</g>\n");
        }
        if object_opacity < 1.0 {
            self.output.push_str("</g>\n");
        }
        if effect_filter_id.is_some() {
            self.output.push_str("</g>\n");
        }
    }


    /// 그림 효과(ImageEffect)에 해당하는 SVG 필터를 defs에 보장하고 ID를 반환한다.
    /// RealPic(기본)은 필터가 필요 없으므로 None 반환.
    pub(crate) fn ensure_image_effect_filter(
        &mut self,
        effect: crate::model::image::ImageEffect,
    ) -> Option<String> {
        use crate::model::image::ImageEffect;
        let (id, def) = match effect {
            ImageEffect::RealPic => return None,
            ImageEffect::GrayScale => (
                "rhwp-img-grayscale",
                "<filter id=\"rhwp-img-grayscale\"><feColorMatrix type=\"matrix\" values=\"\
                    0.299 0.587 0.114 0 0 \
                    0.299 0.587 0.114 0 0 \
                    0.299 0.587 0.114 0 0 \
                    0 0 0 1 0\"/></filter>\n",
            ),
            ImageEffect::BlackWhite => (
                "rhwp-img-blackwhite",
                "<filter id=\"rhwp-img-blackwhite\">\
                    <feColorMatrix type=\"matrix\" values=\"\
                        0.299 0.587 0.114 0 0 \
                        0.299 0.587 0.114 0 0 \
                        0.299 0.587 0.114 0 0 \
                        0 0 0 1 0\"/>\
                    <feComponentTransfer>\
                        <feFuncR type=\"discrete\" tableValues=\"0 1\"/>\
                        <feFuncG type=\"discrete\" tableValues=\"0 1\"/>\
                        <feFuncB type=\"discrete\" tableValues=\"0 1\"/>\
                    </feComponentTransfer>\
                </filter>\n",
            ),
            // Pattern8x8은 SVG 필터로 표현하기 어려워 그레이스케일로 폴백
            ImageEffect::Pattern8x8 => (
                "rhwp-img-grayscale",
                "<filter id=\"rhwp-img-grayscale\"><feColorMatrix type=\"matrix\" values=\"\
                    0.299 0.587 0.114 0 0 \
                    0.299 0.587 0.114 0 0 \
                    0.299 0.587 0.114 0 0 \
                    0 0 0 1 0\"/></filter>\n",
            ),
        };
        if self.defs_ids.insert(id.to_string()) {
            self.defs.push(def.to_string());
        }
        Some(id.to_string())
    }


    /// 밝기/대비 조정용 SVG 필터를 defs에 보장하고 ID를 반환한다.
    /// 둘 다 0이면 필터 불필요 → None 반환.
    /// HWP 스펙은 brightness/contrast 를 -100..=100 으로 정의하므로 손상된 입력에 대비해 clamp 한다.
    pub(crate) fn ensure_brightness_contrast_filter(
        &mut self,
        brightness: i8,
        contrast: i8,
    ) -> Option<String> {
        let brightness = brightness.clamp(-100, 100);
        let contrast = contrast.clamp(-100, 100);
        if brightness == 0 && contrast == 0 {
            return None;
        }

        let id = format!("rhwp-img-bc-b{}c{}", brightness, contrast);

        // 밝기: intercept 오프셋으로 구현 (slope=1, intercept=brightness/100)
        // 대비: slope 조정으로 구현 (slope=(100+contrast)/100, intercept=0.5-0.5*slope)
        // 둘을 합성: slope=contrast_slope, intercept=contrast_intercept + brightness_offset
        let b = brightness as f64 / 100.0;
        let slope = (100.0 + contrast as f64) / 100.0;
        let intercept = (0.5 - 0.5 * slope) + b;

        let def = format!(
            "<filter id=\"{id}\">\
                <feComponentTransfer>\
                    <feFuncR type=\"linear\" slope=\"{slope:.4}\" intercept=\"{intercept:.4}\"/>\
                    <feFuncG type=\"linear\" slope=\"{slope:.4}\" intercept=\"{intercept:.4}\"/>\
                    <feFuncB type=\"linear\" slope=\"{slope:.4}\" intercept=\"{intercept:.4}\"/>\
                </feComponentTransfer>\
            </filter>\n"
        );
        if self.defs_ids.insert(id.clone()) {
            self.defs.push(def);
        }
        Some(id)
    }


    /// 이미지를 원래 크기로 지정 위치에 배치 (배치 모드)
    pub(crate) fn render_positioned_image(
        &mut self,
        data: &[u8],
        data_uri: &str,
        bbox: &super::super::render_tree::BoundingBox,
        fill_mode: ImageFillMode,
        original_size: Option<(f64, f64)>,
    ) {
        // 원본 크기: HWP shape_attr 기반(우선) 또는 이미지 픽셀 크기(폴백)
        let (img_width, img_height) = if let Some((ow, oh)) = original_size {
            (ow, oh)
        } else {
            match parse_image_dimensions(data) {
                Some((w, h)) => (w as f64, h as f64),
                None => {
                    // 크기 파싱 실패 시 meet으로 폴백
                    let par = match fill_mode {
                        ImageFillMode::Center => "xMidYMid meet",
                        ImageFillMode::CenterTop => "xMidYMin meet",
                        ImageFillMode::CenterBottom => "xMidYMax meet",
                        ImageFillMode::LeftCenter => "xMinYMid meet",
                        ImageFillMode::LeftTop => "xMinYMin meet",
                        ImageFillMode::LeftBottom => "xMinYMax meet",
                        ImageFillMode::RightCenter => "xMaxYMid meet",
                        ImageFillMode::RightTop => "xMaxYMin meet",
                        ImageFillMode::RightBottom => "xMaxYMax meet",
                        _ => "xMidYMid meet",
                    };
                    self.output.push_str(&format!(
                        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{}\" href=\"{}\"/>\n",
                        bbox.x, bbox.y, bbox.width, bbox.height, par, data_uri,
                    ));
                    return;
                }
            }
        };

        // 배치 위치 계산
        let (ix, iy) = match fill_mode {
            ImageFillMode::LeftTop => (bbox.x, bbox.y),
            ImageFillMode::CenterTop => (bbox.x + (bbox.width - img_width) / 2.0, bbox.y),
            ImageFillMode::RightTop => (bbox.x + bbox.width - img_width, bbox.y),
            ImageFillMode::LeftCenter => (bbox.x, bbox.y + (bbox.height - img_height) / 2.0),
            ImageFillMode::Center => (
                bbox.x + (bbox.width - img_width) / 2.0,
                bbox.y + (bbox.height - img_height) / 2.0,
            ),
            ImageFillMode::RightCenter => (
                bbox.x + bbox.width - img_width,
                bbox.y + (bbox.height - img_height) / 2.0,
            ),
            ImageFillMode::LeftBottom => (bbox.x, bbox.y + bbox.height - img_height),
            ImageFillMode::CenterBottom => (
                bbox.x + (bbox.width - img_width) / 2.0,
                bbox.y + bbox.height - img_height,
            ),
            ImageFillMode::RightBottom => (
                bbox.x + bbox.width - img_width,
                bbox.y + bbox.height - img_height,
            ),
            _ => (bbox.x, bbox.y),
        };

        // 도형 영역으로 클리핑
        let clip_id = format!("fill-clip-{}", self.next_clip_id());
        self.defs.push(format!(
            "<clipPath id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>\n",
            clip_id, bbox.x, bbox.y, bbox.width, bbox.height,
        ));
        self.output.push_str(&format!(
            "<g clip-path=\"url(#{})\"><image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/></g>\n",
            clip_id, ix, iy, img_width, img_height, data_uri,
        ));
    }


    /// 이미지를 타일링 모드로 렌더링
    pub(crate) fn render_tiled_image(
        &mut self,
        data: &[u8],
        data_uri: &str,
        bbox: &super::super::render_tree::BoundingBox,
        tile_h: bool,
        tile_v: bool,
        original_size: Option<(f64, f64)>,
    ) {
        // 원본 크기: HWP shape_attr 기반(우선) 또는 이미지 픽셀 크기(폴백)
        let (img_width, img_height) = if let Some((ow, oh)) = original_size {
            (ow, oh)
        } else {
            match parse_image_dimensions(data) {
                Some((w, h)) => (w as f64, h as f64),
                None => {
                    // 크기 파싱 실패 시 전체 채우기로 폴백
                    self.output.push_str(&format!(
                        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>\n",
                        bbox.x, bbox.y, bbox.width, bbox.height, data_uri,
                    ));
                    return;
                }
            }
        };

        let pat_id = format!("tile-pat-{}", self.next_clip_id());
        let pat_w = if tile_h { img_width } else { bbox.width };
        let pat_h = if tile_v { img_height } else { bbox.height };

        self.defs.push(format!(
            "<pattern id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" patternUnits=\"userSpaceOnUse\">\
             <image width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>\
             </pattern>\n",
            pat_id, bbox.x, bbox.y, pat_w, pat_h,
            img_width, img_height, data_uri,
        ));
        self.output.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"url(#{})\"/>\n",
            bbox.x, bbox.y, bbox.width, bbox.height, pat_id,
        ));
    }


    /// 고유 클립/패턴 ID 생성
    pub(crate) fn next_clip_id(&mut self) -> u32 {
        self.clip_counter += 1;
        self.clip_counter
    }


    /// 양식 개체 SVG 렌더링
    pub(crate) fn render_form_object(&mut self, form: &FormObjectNode, bbox: &BoundingBox) {
        let x = bbox.x;
        let y = bbox.y;
        let w = bbox.width;
        let h = bbox.height;

        match form.form_type {
            FormType::PushButton => {
                // 3D 버튼 (웹 환경 비활성 — 회색 스타일)
                self.output.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#d0d0d0\" stroke=\"#a0a0a0\" stroke-width=\"0.5\"/>\n",
                    x, y, w, h));
                // 캡션 텍스트 (회색, 중앙)
                if !form.caption.is_empty() {
                    let caption = display_form_caption(&form.caption);
                    let font_size = (h * 0.55).min(12.0).max(7.0);
                    self.output.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" font-size=\"{:.1}\" fill=\"#808080\" text-anchor=\"middle\" dominant-baseline=\"central\" font-family=\"'맑은 고딕',sans-serif\">{}</text>\n",
                        x + w / 2.0, y + h / 2.0, font_size, escape_xml(caption.as_ref())));
                }
            }
            FormType::CheckBox => {
                // 체크박스: □/☑ + 캡션
                let box_size = (h * 0.7).min(13.0);
                let box_y = y + (h - box_size) / 2.0;
                let box_x = x + 2.0;
                self.output.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\" stroke=\"#606060\" stroke-width=\"0.8\"/>\n",
                    box_x, box_y, box_size, box_size));
                if form.value != 0 {
                    // 체크 마크 (✓)
                    let cx = box_x + box_size * 0.2;
                    let cy = box_y + box_size * 0.55;
                    let mx = box_x + box_size * 0.45;
                    let my = box_y + box_size * 0.8;
                    let ex = box_x + box_size * 0.85;
                    let ey = box_y + box_size * 0.2;
                    self.output.push_str(&format!(
                        "<polyline points=\"{},{} {},{} {},{}\" fill=\"none\" stroke=\"#000000\" stroke-width=\"1.5\"/>\n",
                        cx, cy, mx, my, ex, ey));
                }
                // 캡션
                if !form.caption.is_empty() {
                    let caption = display_form_caption(&form.caption);
                    let text_x = box_x + box_size + 3.0;
                    let font_size = (h * 0.55).min(12.0).max(7.0);
                    self.output.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" font-size=\"{:.1}\" fill=\"{}\" dominant-baseline=\"central\" font-family=\"'맑은 고딕',sans-serif\">{}</text>\n",
                        text_x, y + h / 2.0, font_size, form.fore_color, escape_xml(caption.as_ref())));
                }
            }
            FormType::RadioButton => {
                // 라디오: ○/◉ + 캡션
                let r = (h * 0.3).min(6.5);
                let cx = x + 2.0 + r;
                let cy = y + h / 2.0;
                self.output.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"white\" stroke=\"#606060\" stroke-width=\"0.8\"/>\n",
                    cx, cy, r));
                if form.value != 0 {
                    self.output.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#000000\"/>\n",
                        cx,
                        cy,
                        r * 0.5
                    ));
                }
                // 캡션
                if !form.caption.is_empty() {
                    let caption = display_form_caption(&form.caption);
                    let text_x = cx + r + 3.0;
                    let font_size = (h * 0.55).min(12.0).max(7.0);
                    self.output.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" font-size=\"{:.1}\" fill=\"{}\" dominant-baseline=\"central\" font-family=\"'맑은 고딕',sans-serif\">{}</text>\n",
                        text_x, y + h / 2.0, font_size, form.fore_color, escape_xml(caption.as_ref())));
                }
            }
            FormType::ComboBox => {
                // 콤보박스: 입력 영역 + 드롭다운 버튼(▼)
                let btn_w = (h * 0.8).min(16.0);
                self.output.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\" stroke=\"#a0a0a0\" stroke-width=\"0.8\"/>\n",
                    x, y, w, h));
                // 드롭다운 버튼
                self.output.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#e0e0e0\" stroke=\"#a0a0a0\" stroke-width=\"0.5\"/>\n",
                    x + w - btn_w, y, btn_w, h));
                // ▼ 화살표
                let arrow_cx = x + w - btn_w / 2.0;
                let arrow_cy = y + h / 2.0;
                let arrow_size = (h * 0.2).min(4.0);
                self.output.push_str(&format!(
                    "<polygon points=\"{},{} {},{} {},{}\" fill=\"#404040\"/>\n",
                    arrow_cx - arrow_size,
                    arrow_cy - arrow_size * 0.5,
                    arrow_cx + arrow_size,
                    arrow_cy - arrow_size * 0.5,
                    arrow_cx,
                    arrow_cy + arrow_size * 0.5
                ));
                // 텍스트
                if !form.text.is_empty() {
                    let font_size = (h * 0.55).min(12.0).max(7.0);
                    self.output.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" font-size=\"{:.1}\" fill=\"{}\" dominant-baseline=\"central\" font-family=\"'맑은 고딕',sans-serif\">{}</text>\n",
                        x + 3.0, y + h / 2.0, font_size, form.fore_color, escape_xml(&form.text)));
                }
            }
            FormType::Edit => {
                // 입력 상자: 테두리 사각형 + 내부 텍스트
                self.output.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\" stroke=\"#a0a0a0\" stroke-width=\"0.8\"/>\n",
                    x, y, w, h));
                if !form.text.is_empty() {
                    let font_size = (h * 0.55).min(12.0).max(7.0);
                    self.output.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" font-size=\"{:.1}\" fill=\"{}\" dominant-baseline=\"central\" font-family=\"'맑은 고딕',sans-serif\">{}</text>\n",
                        x + 3.0, y + h / 2.0, font_size, form.fore_color, escape_xml(&form.text)));
                }
            }
        }
    }

}
