//! drawing — table_layout.rs 에서 무변동 이동
use super::*;

impl SvgRenderer {
    /// 그라데이션 색상 stop 목록 생성
    pub(crate) fn build_gradient_stops(grad: &GradientFillInfo) -> String {
        let mut stops = String::new();
        for (i, &color) in grad.colors.iter().enumerate() {
            let offset = if i < grad.positions.len() {
                grad.positions[i] * 100.0
            } else {
                let n = grad.colors.len();
                if n <= 1 {
                    0.0
                } else {
                    i as f64 / (n - 1) as f64 * 100.0
                }
            };
            stops.push_str(&format!(
                "<stop offset=\"{:.1}%\" stop-color=\"{}\"/>\n",
                offset,
                color_to_svg(color),
            ));
        }
        stops
    }


    /// 그라데이션을 포함한 사각형 그리기 (렌더 트리 전용)
    pub(crate) fn draw_rect_with_gradient(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        corner_radius: f64,
        style: &ShapeStyle,
        gradient: Option<&GradientFillInfo>,
    ) {
        let mut attrs = format!("x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"", x, y, w, h);

        if corner_radius > 0.0 {
            attrs.push_str(&format!(
                " rx=\"{}\" ry=\"{}\"",
                corner_radius, corner_radius
            ));
        }

        attrs.push_str(&self.build_fill_attr(style, gradient));

        if let Some(stroke) = style.stroke_color {
            attrs.push_str(&format!(
                " stroke=\"{}\" stroke-width=\"{}\"",
                color_to_svg(stroke),
                style.stroke_width
            ));
            match style.stroke_dash {
                StrokeDash::Dash => attrs.push_str(" stroke-dasharray=\"6 3\""),
                StrokeDash::Dot => attrs.push_str(" stroke-dasharray=\"2 2\""),
                StrokeDash::DashDot => attrs.push_str(" stroke-dasharray=\"6 3 2 3\""),
                StrokeDash::DashDotDot => attrs.push_str(" stroke-dasharray=\"6 3 2 3 2 3\""),
                _ => {}
            }
        }

        if style.opacity < 1.0 {
            attrs.push_str(&format!(" opacity=\"{:.3}\"", style.opacity));
        }

        self.output.push_str(&format!("<rect {}/>\n", attrs));
    }


    /// 그라데이션을 포함한 타원 그리기 (렌더 트리 전용)
    pub(crate) fn draw_ellipse_with_gradient(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        style: &ShapeStyle,
        gradient: Option<&GradientFillInfo>,
    ) {
        let mut attrs = format!("cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"", cx, cy, rx, ry);

        attrs.push_str(&self.build_fill_attr(style, gradient));

        if let Some(stroke) = style.stroke_color {
            attrs.push_str(&format!(
                " stroke=\"{}\" stroke-width=\"{}\"",
                color_to_svg(stroke),
                style.stroke_width
            ));
        }

        if style.opacity < 1.0 {
            attrs.push_str(&format!(" opacity=\"{:.3}\"", style.opacity));
        }

        self.output.push_str(&format!("<ellipse {}/>\n", attrs));
    }


    /// 그라데이션을 포함한 패스 그리기 (렌더 트리 전용)
    pub(crate) fn draw_path_with_gradient(
        &mut self,
        commands: &[PathCommand],
        style: &ShapeStyle,
        gradient: Option<&GradientFillInfo>,
    ) {
        let mut d = String::new();
        for cmd in commands {
            match cmd {
                PathCommand::MoveTo(x, y) => d.push_str(&format!("M{} {} ", x, y)),
                PathCommand::LineTo(x, y) => d.push_str(&format!("L{} {} ", x, y)),
                PathCommand::CurveTo(x1, y1, x2, y2, x, y) => {
                    d.push_str(&format!("C{} {} {} {} {} {} ", x1, y1, x2, y2, x, y))
                }
                PathCommand::ArcTo(rx, ry, x_rot, large_arc, sweep, x, y) => {
                    d.push_str(&format!(
                        "A{} {} {} {} {} {} {} ",
                        rx,
                        ry,
                        x_rot,
                        if *large_arc { 1 } else { 0 },
                        if *sweep { 1 } else { 0 },
                        x,
                        y
                    ));
                }
                PathCommand::ClosePath => d.push_str("Z "),
            }
        }

        let mut attrs = format!("d=\"{}\"", d.trim());

        attrs.push_str(&self.build_fill_attr(style, gradient));

        if let Some(stroke) = style.stroke_color {
            attrs.push_str(&format!(
                " stroke=\"{}\" stroke-width=\"{}\"",
                color_to_svg(stroke),
                style.stroke_width
            ));
            match style.stroke_dash {
                StrokeDash::Dash => attrs.push_str(" stroke-dasharray=\"6 3\""),
                StrokeDash::Dot => attrs.push_str(" stroke-dasharray=\"2 2\""),
                StrokeDash::DashDot => attrs.push_str(" stroke-dasharray=\"6 3 2 3\""),
                StrokeDash::DashDotDot => attrs.push_str(" stroke-dasharray=\"6 3 2 3 2 3\""),
                _ => {}
            }
        }

        self.output.push_str(&format!("<path {}/>\n", attrs));
    }


    /// HWP 각도(도) → SVG linearGradient 좌표 (x1%, y1%, x2%, y2%) 변환
    pub(crate) fn angle_to_svg_coords(angle: i16) -> (f64, f64, f64, f64) {
        let a = ((angle % 360 + 360) % 360) as f64;
        match a as i32 {
            0 => (0.0, 0.0, 0.0, 100.0),
            45 => (0.0, 0.0, 100.0, 100.0),
            90 => (0.0, 0.0, 100.0, 0.0),
            135 => (0.0, 100.0, 100.0, 0.0),
            180 => (0.0, 100.0, 0.0, 0.0),
            225 => (100.0, 100.0, 0.0, 0.0),
            270 => (100.0, 0.0, 0.0, 0.0),
            315 => (100.0, 0.0, 0.0, 100.0),
            _ => {
                let rad = a.to_radians();
                let sin = rad.sin();
                let cos = rad.cos();
                let x1 = 50.0 - sin * 50.0;
                let y1 = 50.0 - cos * 50.0;
                let x2 = 50.0 + sin * 50.0;
                let y2 = 50.0 + cos * 50.0;
                (x1, y1, x2, y2)
            }
        }
    }


    /// 이중선/삼중선 렌더링: 원래 선에 수직 방향으로 평행선들을 그림
    pub(crate) fn draw_multi_line(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        total_width: f64,
        color: &str,
        line_type: &super::super::LineRenderType,
    ) {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }

        // 수직 방향 단위벡터 (선의 법선)
        let nx = -dy / len;
        let ny = dx / len;

        // (width_ratio, offset_ratio) — offset은 선 중심으로부터의 거리 비율
        let lines: Vec<(f64, f64)> = match line_type {
            super::super::LineRenderType::Double => {
                // 같은 굵기 이중선: 각 선 30%, 간격 40%
                vec![(0.30, -0.35), (0.30, 0.35)]
            }
            super::super::LineRenderType::ThickThinDouble => {
                // 굵은선(위)-얇은선(아래): 굵은선 40%, 얇은선 20%, 간격 40%
                vec![(0.4, -0.30), (0.2, 0.40)]
            }
            super::super::LineRenderType::ThinThickDouble => {
                // 얇은선(위)-굵은선(아래): 얇은선 20%, 굵은선 40%, 간격 40%
                vec![(0.2, -0.40), (0.4, 0.30)]
            }
            super::super::LineRenderType::ThinThickThinTriple => {
                // 얇은-굵은-얇은 삼중선: 15%, 30%, 15%, 간격 20%×2
                vec![(0.15, -0.425), (0.30, 0.0), (0.15, 0.425)]
            }
            _ => return,
        };

        for (width_ratio, offset_ratio) in &lines {
            let w = total_width * width_ratio;
            let off = total_width * offset_ratio;
            let ox = nx * off;
            let oy = ny * off;
            self.output.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                x1 + ox, y1 + oy, x2 + ox, y2 + oy, color, w,
            ));
        }
    }


    /// 글자겹침(CharOverlap) 렌더링
    ///
    /// 각 문자를 테두리 도형(원/사각형) 안에 중앙 배치하여 렌더링한다.
    /// border_type: 0=없음, 1=원, 2=반전원, 3=사각형, 4=반전사각형
    /// 반전: 도형 채움(검정) + 흰 글자, 일반: 도형 테두리(검정) + 검정 글자
    ///
    /// 다자리 PUA 숫자 (2~3자리): 모든 문자를 하나의 원/사각형 안에 합쳐서 렌더링.
    /// border_type=0이고 PUA 겹침 숫자이면 원형(circle)으로 자동 렌더링.
    /// 한컴 방식: 장평 조절로 좁은 숫자를 하나의 도형 안에 배치.
    pub(crate) fn draw_char_overlap(
        &mut self,
        text: &str,
        style: &TextStyle,
        overlap: &CharOverlapInfo,
        bbox_x: f64,
        bbox_y: f64,
        bbox_w: f64,
        bbox_h: f64,
    ) {
        let font_size = if style.font_size > 0.0 {
            style.font_size
        } else {
            12.0
        };
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return;
        }

        // PUA 다자리 숫자 디코딩 시도
        if let Some(number_str) = decode_pua_overlap_number(&chars) {
            self.draw_char_overlap_combined(
                style,
                overlap,
                &number_str,
                bbox_x,
                bbox_y,
                bbox_w,
                bbox_h,
            );
            return;
        }

        // 일반 CharOverlap 처리. 디코딩되지 않는 다중 PUA 조합도 한 컨트롤 안에서
        // 같은 중심에 겹쳐 그린다. table-vpos-01의 10/11/12 마커는
        // U+F02BA + U+F02C3/C4/C5 조합으로 저장되며, 나란히 그리면 숫자가
        // 사각형 밖으로 밀린다.
        let box_size = font_size;
        let boxed_pua = boxed_pua_char_overlap_semantics(&chars, overlap.border_type);
        let effective_border = boxed_pua
            .map(|(_, border_type)| border_type)
            .unwrap_or(overlap.border_type);

        let is_reversed = effective_border == 2 || effective_border == 4;
        let is_circle = effective_border == 1 || effective_border == 2;
        let is_rect = effective_border == 3 || effective_border == 4;

        // charSz 는 "테두리 내부" 글자 비율이므로 테두리를 안 그리면 적용하지 않는다 (#4085).
        let size_ratio = char_overlap_size_ratio(effective_border, overlap.inner_char_size);
        let inner_font_size = font_size * size_ratio;

        // 한컴은 동그라미 테두리도 글자색과 동일 색상으로 그림 (raw PDF 0 0 1 RG/rg).
        // reversed(반전)는 기존대로 검정 채움 + 흰 글자.
        let glyph_color = color_to_svg(style.color);
        let fill_color = if is_reversed { "#000000" } else { "none" };
        let stroke_color: &str = if is_reversed { "#000000" } else { &glyph_color };
        let text_color: &str = if is_reversed { "#FFFFFF" } else { &glyph_color };

        let font_family_str = if style.font_family.is_empty() {
            "sans-serif".to_string()
        } else {
            // [#3314] 요청 face → base family → generic 체인.
            super::super::render_font_family_chain(&style.font_family)
        };
        let mut font_attrs = format!(
            "font-family=\"{}\" font-size=\"{:.2}\"",
            escape_xml(&font_family_str),
            inner_font_size
        );
        if style.is_visually_bold() {
            font_attrs.push_str(" font-weight=\"bold\"");
        } else if style.is_medium_weight() {
            font_attrs.push_str(" font-weight=\"500\"");
        }
        if style.italic {
            font_attrs.push_str(" font-style=\"italic\"");
        }

        if chars.len() > 1 {
            let cx = bbox_x + bbox_w / 2.0;
            let cy = bbox_y + bbox_h / 2.0;

            if is_circle {
                let ry = box_size / 2.0;
                let rx = ry * 0.85;
                self.output.push_str(&format!(
                    "<ellipse cx=\"{:.2}\" cy=\"{:.2}\" rx=\"{:.2}\" ry=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"0.8\"/>\n",
                    cx, cy, rx, ry, fill_color, stroke_color,
                ));
            } else if is_rect {
                let rx = cx - box_size / 2.0;
                let ry = cy - box_size / 2.0;
                self.output.push_str(&format!(
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"0.8\"/>\n",
                    rx, ry, box_size, box_size, fill_color, stroke_color,
                ));
            }

            for ch in chars.iter() {
                let display_str = {
                    let cp = *ch as u32;
                    if (0x2460..=0x2473).contains(&cp) {
                        format!("{}", cp - 0x2460 + 1)
                    } else if let Some(s) = pua_to_display_text(*ch) {
                        s
                    } else {
                        ch.to_string()
                    }
                };
                self.output.push_str(&format!(
                    "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"{}\" {} text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>\n",
                    cx, cy, text_color, font_attrs, escape_xml(&display_str),
                ));
            }
            return;
        }

        for (i, ch) in chars.iter().enumerate() {
            let display_str = if let Some((number, _)) = boxed_pua {
                number.to_string()
            } else {
                let cp = *ch as u32;
                if (0x2460..=0x2473).contains(&cp) {
                    format!("{}", cp - 0x2460 + 1)
                } else if let Some(s) = pua_to_display_text(*ch) {
                    s
                } else {
                    ch.to_string()
                }
            };

            let cx = bbox_x + i as f64 * box_size + box_size / 2.0;
            let cy = bbox_y + bbox_h / 2.0;

            if is_circle {
                // 한컴 글자겹침은 세로로 긴 타원 (h/w ≈ 1.18). 한글 글리프 비율과 정합.
                let ry = box_size / 2.0;
                let rx = ry * 0.85;
                self.output.push_str(&format!(
                    "<ellipse cx=\"{:.2}\" cy=\"{:.2}\" rx=\"{:.2}\" ry=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"0.8\"/>\n",
                    cx, cy, rx, ry, fill_color, stroke_color,
                ));
            } else if is_rect {
                let rx = cx - box_size / 2.0;
                let ry = cy - box_size / 2.0;
                self.output.push_str(&format!(
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"0.8\"/>\n",
                    rx, ry, box_size, box_size, fill_color, stroke_color,
                ));
            }

            self.output.push_str(&format!(
                "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"{}\" {} text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>\n",
                cx, cy, text_color, font_attrs, escape_xml(&display_str),
            ));
        }
    }


    /// PUA 다자리 숫자를 하나의 도형 안에 합쳐서 렌더링
    ///
    /// border_type=0이면 원형으로 자동 렌더링 (PUA 겹침 숫자는 원래 원문자)
    /// 장평 조절: textLength 속성으로 숫자 문자열을 도형 내부 폭에 맞춤
    pub(crate) fn draw_char_overlap_combined(
        &mut self,
        style: &TextStyle,
        overlap: &CharOverlapInfo,
        number_str: &str,
        bbox_x: f64,
        bbox_y: f64,
        bbox_w: f64,
        bbox_h: f64,
    ) {
        let font_size = if style.font_size > 0.0 {
            style.font_size
        } else {
            12.0
        };
        let box_size = font_size;

        // border_type=0이고 PUA 숫자이면 원형으로 자동 렌더링
        let effective_border = if overlap.border_type == 0 {
            1u8
        } else {
            overlap.border_type
        };
        let is_reversed = effective_border == 2 || effective_border == 4;
        let is_circle = effective_border == 1 || effective_border == 2;
        let is_rect = effective_border == 3 || effective_border == 4;

        // draw_char_overlap와 동일 규칙. 여기서는 effective_border 가 0이 아니므로
        // (border_type=0 → 원형 승격) 축소 게이트에 걸리지 않는다 (#4085).
        let size_ratio = char_overlap_size_ratio(effective_border, overlap.inner_char_size);
        let inner_font_size = font_size * size_ratio;

        let glyph_color = color_to_svg(style.color);
        let fill_color = if is_reversed { "#000000" } else { "none" };
        let stroke_color: &str = if is_reversed { "#000000" } else { &glyph_color };
        let text_color: &str = if is_reversed { "#FFFFFF" } else { &glyph_color };

        let font_family_str = if style.font_family.is_empty() {
            "sans-serif".to_string()
        } else {
            // [#3314] 요청 face → base family → generic 체인.
            super::super::render_font_family_chain(&style.font_family)
        };
        let mut font_attrs = format!(
            "font-family=\"{}\" font-size=\"{:.2}\"",
            escape_xml(&font_family_str),
            inner_font_size
        );
        if style.is_visually_bold() {
            font_attrs.push_str(" font-weight=\"bold\"");
        } else if style.is_medium_weight() {
            font_attrs.push_str(" font-weight=\"500\"");
        }
        if style.italic {
            font_attrs.push_str(" font-style=\"italic\"");
        }

        let cx = bbox_x + box_size / 2.0;
        let cy = bbox_y + bbox_h / 2.0;

        // 도형 렌더링 — 세로로 긴 타원 (한컴 정합, rx=ry*0.85)
        if is_circle {
            let ry = box_size / 2.0;
            let rx = ry * 0.85;
            self.output.push_str(&format!(
                "<ellipse cx=\"{:.2}\" cy=\"{:.2}\" rx=\"{:.2}\" ry=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"0.8\"/>\n",
                cx, cy, rx, ry, fill_color, stroke_color,
            ));
        } else if is_rect {
            let rx = cx - box_size / 2.0;
            let ry = cy - box_size / 2.0;
            self.output.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"0.8\"/>\n",
                rx, ry, box_size, box_size, fill_color, stroke_color,
            ));
        }

        // 장평 조절: 숫자 자릿수에 따라 textLength로 폭 압축
        let text_width = box_size * 0.7; // 도형 내부 여백 고려
                                         // 다자리 숫자는 baseline을 살짝 올려 시각적 중앙 맞춤
        let text_y = cy - font_size * 0.08;
        self.output.push_str(&format!(
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"{}\" {} text-anchor=\"middle\" dominant-baseline=\"central\" textLength=\"{:.2}\" lengthAdjust=\"spacingAndGlyphs\">{}</text>\n",
            cx, text_y, text_color, font_attrs, text_width, escape_xml(number_str),
        ));
    }


    /// 선 모양(shape)에 따라 SVG line/group을 출력한다.
    /// shape: 0=실선, 1=긴점선, 2=점선, 3=일점쇄선, 4=이점쇄선, 5=긴파선,
    ///        6=원형점, 7=이중선, 8=가는+굵은, 9=굵은+가는, 10=삼중선
    pub(crate) fn draw_line_shape(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, shape: u8) {
        match shape {
            7 => {
                // 이중선
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.7\"/>\n",
                    x1, y1 - 1.0, x2, y2 - 1.0, color));
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.7\"/>\n",
                    x1, y1 + 1.0, x2, y2 + 1.0, color));
            }
            8 => {
                // 가는+굵은 이중선
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.5\"/>\n",
                    x1, y1 - 1.2, x2, y2 - 1.2, color));
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1.2\"/>\n",
                    x1, y1 + 0.8, x2, y2 + 0.8, color));
            }
            9 => {
                // 굵은+가는 이중선
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1.2\"/>\n",
                    x1, y1 - 0.8, x2, y2 - 0.8, color));
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.5\"/>\n",
                    x1, y1 + 1.2, x2, y2 + 1.2, color));
            }
            10 => {
                // 삼중선
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.5\"/>\n",
                    x1, y1 - 1.5, x2, y2 - 1.5, color));
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.5\"/>\n",
                    x1, y1, x2, y2, color));
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.5\"/>\n",
                    x1, y1 + 1.5, x2, y2 + 1.5, color));
            }
            11 => {
                // 물결선
                let wave_h = 1.5;
                let wave_w = 6.0;
                let mut d = format!("M{:.2},{:.2}", x1, y1);
                let mut cx = x1;
                let mut up = true;
                while cx < x2 {
                    let next = (cx + wave_w).min(x2);
                    let cy = if up { y1 - wave_h } else { y1 + wave_h };
                    d.push_str(&format!(
                        " Q{:.2},{:.2} {:.2},{:.2}",
                        (cx + next) / 2.0,
                        cy,
                        next,
                        y1
                    ));
                    cx = next;
                    up = !up;
                }
                self.output.push_str(&format!(
                    "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"0.7\"/>\n",
                    d, color
                ));
            }
            12 => {
                // 이중물결선
                for offset in [-1.0f64, 1.0] {
                    let wy = y1 + offset;
                    let wave_h = 1.2;
                    let wave_w = 6.0;
                    let mut d = format!("M{:.2},{:.2}", x1, wy);
                    let mut cx = x1;
                    let mut up = true;
                    while cx < x2 {
                        let next = (cx + wave_w).min(x2);
                        let cy = if up { wy - wave_h } else { wy + wave_h };
                        d.push_str(&format!(
                            " Q{:.2},{:.2} {:.2},{:.2}",
                            (cx + next) / 2.0,
                            cy,
                            next,
                            wy
                        ));
                        cx = next;
                        up = !up;
                    }
                    self.output.push_str(&format!(
                        "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"0.5\"/>\n",
                        d, color
                    ));
                }
            }
            _ => {
                // 단선 (dasharray로 모양 표현)
                // 0=실선, 1=파선, 2=점선, 3=일점쇄선, 4=이점쇄선, 5=긴파선, 6=원형점선
                let dasharray = match shape {
                    1 => " stroke-dasharray=\"3 3\"",
                    2 => " stroke-dasharray=\"1 2\"",
                    3 => " stroke-dasharray=\"6 2 1 2\"",
                    4 => " stroke-dasharray=\"6 2 1 2 1 2\"",
                    5 => " stroke-dasharray=\"8 4\"",
                    6 => " stroke-dasharray=\"0.1 2.5\" stroke-linecap=\"round\"",
                    _ => "", // 0=실선
                };
                self.output.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"{}/>\n",
                    x1, y1, x2, y2, color, dasharray));
            }
        }
    }

}
