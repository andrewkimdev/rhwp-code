//! api_export — table_layout.rs 에서 무변동 이동
use super::*;

#[wasm_bindgen]
impl HwpDocument {
    /// 특정 페이지를 SVG 문자열로 렌더링한다.
    #[wasm_bindgen(js_name = renderPageSvg)]
    pub fn render_page_svg(&self, page_num: u32) -> Result<String, JsValue> {
        self.render_page_svg_native(page_num).map_err(|e| e.into())
    }


    /// 명시적인 출력 profile로 특정 페이지를 SVG 문자열로 렌더링한다.
    #[wasm_bindgen(js_name = renderPageSvgWithProfile)]
    pub fn render_page_svg_with_profile(
        &self,
        page_num: u32,
        profile: &str,
    ) -> Result<String, JsValue> {
        let profile = crate::paint::RenderProfile::parse(profile)
            .ok_or_else(|| JsValue::from_str(&format!("unsupported render profile: {profile}")))?;
        self.render_page_svg_layer_with_profile_native(page_num, profile)
            .map_err(Into::into)
    }


    /// 특정 페이지를 HTML 문자열로 렌더링한다.
    #[wasm_bindgen(js_name = renderPageHtml)]
    pub fn render_page_html(&self, page_num: u32) -> Result<String, JsValue> {
        self.render_page_html_native(page_num).map_err(|e| e.into())
    }


    /// 특정 페이지를 Canvas 명령 수로 반환한다.
    #[wasm_bindgen(js_name = renderPageCanvas)]
    pub fn render_page_canvas(&self, page_num: u32) -> Result<u32, JsValue> {
        self.render_page_canvas_native(page_num)
            .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = renderPageCanvasLegacy)]
    pub fn render_page_canvas_legacy(&self, page_num: u32) -> Result<u32, JsValue> {
        self.render_page_canvas_legacy_native(page_num)
            .map_err(|e| e.into())
    }


    /// 특정 페이지를 Canvas 2D에 직접 렌더링한다.
    ///
    /// WASM 환경에서만 사용 가능하다. Canvas 크기는 페이지 크기 × scale로 설정된다.
    /// scale이 0 이하이면 1.0으로 처리한다 (하위호환).
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderPageToCanvas)]
    pub fn render_page_to_canvas(
        &self,
        page_num: u32,
        canvas: &HtmlCanvasElement,
        scale: f64,
    ) -> Result<(), JsValue> {
        use crate::renderer::layer_renderer::LayerRenderer;
        use crate::renderer::web_canvas::WebCanvasRenderer;

        let tree = self
            .build_page_layer_tree(page_num)
            .map_err(JsValue::from)?;

        let scale = normalize_canvas_scale(tree.page_width, tree.page_height, scale)
            .map_err(JsValue::from_str)?;

        // 캔버스 크기 = 페이지 크기 × scale
        canvas.set_width(scaled_canvas_extent(tree.page_width, scale));
        canvas.set_height(scaled_canvas_extent(tree.page_height, scale));

        let mut renderer = WebCanvasRenderer::new(canvas)?;
        renderer.show_paragraph_marks = self.show_paragraph_marks;
        renderer.show_control_codes = self.show_control_codes;
        renderer.set_scale(scale);
        renderer.render_page(&tree).map_err(JsValue::from)?;
        Ok(())
    }


    /// 다층 레이어 필터를 적용한 Canvas 렌더링 (Task #516, Stage 5.2).
    ///
    /// `layer_kind`:
    /// - `"all"` → 모든 PaintOp 렌더 (기본 `renderPageToCanvas` 와 동일)
    /// - `"background"` → page background layer
    /// - `"flow"` → 본문 layer (BehindText / InFrontOfText plane 제외)
    /// - `"flow-dynamic"` → 본문 layer 중 Image/RawSvg 제외
    /// - `"flow-static"` → page background + 본문 Image/RawSvg layer
    /// - `"behind"` → BehindText overlay layer
    /// - `"front"` → InFrontOfText overlay layer
    ///
    /// 본문 Canvas 와 overlay 컨테이너를 분리하는 다층 layer 아키텍처에서 사용.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderPageToCanvasFiltered)]
    pub fn render_page_to_canvas_filtered(
        &self,
        page_num: u32,
        canvas: &HtmlCanvasElement,
        scale: f64,
        layer_kind: &str,
    ) -> Result<(), JsValue> {
        self.render_page_to_canvas_filtered_with_profile(
            page_num, canvas, scale, layer_kind, "screen",
        )
    }


    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderPageToCanvasFilteredWithProfile)]
    pub fn render_page_to_canvas_filtered_with_profile(
        &self,
        page_num: u32,
        canvas: &HtmlCanvasElement,
        scale: f64,
        layer_kind: &str,
        profile: &str,
    ) -> Result<(), JsValue> {
        subsecond_boundary::render_page_to_canvas_filtered_with_profile(
            self, page_num, canvas, scale, layer_kind, profile,
        )
    }


    /// [#3137 Stage 4] 기존 Canvas의 page-space 일부만 다시 재생한다.
    ///
    /// Canvas 크기와 나머지 픽셀은 유지한다. 호출 조건이나 크기가 맞지 않으면 오류를
    /// 반환하며 Studio는 기존 full-page repaint로 폴백한다.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderPagePatchToCanvasFilteredWithProfile)]
    #[allow(clippy::too_many_arguments)]
    pub fn render_page_patch_to_canvas_filtered_with_profile(
        &self,
        page_num: u32,
        canvas: &HtmlCanvasElement,
        scale: f64,
        layer_kind: &str,
        profile: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), JsValue> {
        use crate::renderer::render_tree::BoundingBox;

        subsecond_boundary::render_page_patch_to_canvas_filtered_with_profile(
            self,
            page_num,
            canvas,
            scale,
            layer_kind,
            profile,
            BoundingBox::new(x, y, width, height),
        )
    }


    /// 특정 페이지를 기존 PageRenderTree 경로로 Canvas 2D에 직접 렌더링한다.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderPageToCanvasLegacy)]
    pub fn render_page_to_canvas_legacy(
        &self,
        page_num: u32,
        canvas: &HtmlCanvasElement,
        scale: f64,
    ) -> Result<(), JsValue> {
        use crate::renderer::web_canvas::WebCanvasRenderer;

        let tree = self
            .build_page_tree_cached(page_num)
            .map_err(|e| JsValue::from(e))?;

        let scale = normalize_canvas_scale(tree.root.bbox.width, tree.root.bbox.height, scale)
            .map_err(JsValue::from_str)?;

        // 캔버스 크기 = 페이지 크기 × scale
        canvas.set_width(scaled_canvas_extent(tree.root.bbox.width, scale));
        canvas.set_height(scaled_canvas_extent(tree.root.bbox.height, scale));

        let mut renderer = WebCanvasRenderer::new(canvas)?;
        renderer.show_paragraph_marks = self.show_paragraph_marks;
        renderer.show_control_codes = self.show_control_codes;
        renderer.set_scale(scale);
        renderer.render_tree(&tree);
        Ok(())
    }


    /// 수식 스크립트를 SVG로 렌더링하여 반환한다 (미리보기 전용).
    ///
    /// 반환: 완전한 `<svg>` 문자열
    #[wasm_bindgen(js_name = renderEquationPreview)]
    pub fn render_equation_preview(
        &self,
        script: &str,
        font_size_hwpunit: u32,
        color: u32,
    ) -> Result<String, JsValue> {
        self.render_equation_preview_native(script, font_size_hwpunit, color)
            .map_err(|e| e.into())
    }


    /// 문서를 HWP 바이너리로 내보낸다.
    ///
    /// Document IR을 HWP 5.0 CFB 바이너리로 직렬화하여 반환한다.
    /// HWPX 출처 문서는 `export_hwp_with_adapter` 를 통해 HWPX→HWP IR 매핑 어댑터를
    /// 자동 적용하여 한컴 호환성과 자기 재로드 페이지 보존을 보장한다 (#178).
    /// HWP 출처는 어댑터가 no-op 이므로 기존 동작과 동일.
    #[wasm_bindgen(js_name = exportHwp)]
    pub fn export_hwp(&mut self) -> Result<Vec<u8>, JsValue> {
        self.export_hwp_with_adapter_snapshot()
            .map_err(|e| e.into())
    }


    /// HWP 바이트와 이번 산출물의 내용 손실을 같은 결과로 반환한다 (#4430).
    ///
    /// 명시적 Studio 저장은 이 API를 사용한다. 기존 `exportHwp()`는 호환성을 위해
    /// byte-only로 유지되며, autosave/embed/history/compare/hwpctl/digest 등 별도
    /// 소비자는 아직 보고서를 받지 않는다.
    #[wasm_bindgen(js_name = exportHwpWithReport)]
    pub fn export_hwp_with_report(&self) -> Result<DocumentExport, JsValue> {
        self.export_hwp_with_adapter_snapshot_with_report()
            .map(DocumentExport::from)
            .map_err(JsValue::from)
    }


    /// 문서를 HWP5 EncryptVersion 4 비밀번호 문서로 내보낸다.
    ///
    /// browser UI는 암호를 저장하지 않고 저장 시점에만 전달한다. HWPX 출처 문서는 일반
    /// HWP 저장과 동일하게 HWPX-to-HWP adapter를 먼저 적용한다.
    #[wasm_bindgen(js_name = exportHwpWithPassword)]
    pub fn export_hwp_with_password_wasm(&mut self, password: &str) -> Result<Vec<u8>, JsValue> {
        self.export_hwp_with_adapter_with_password(password.as_bytes())
            .map_err(|e| e.into())
    }


    /// 비밀번호 HWP 바이트 + 내용 손실 보고 (#4430).
    #[wasm_bindgen(js_name = exportHwpWithPasswordAndReport)]
    pub fn export_hwp_with_password_and_report_wasm(
        &self,
        password: &str,
    ) -> Result<DocumentExport, JsValue> {
        self.export_hwp_with_adapter_snapshot_with_password_and_report(password.as_bytes())
            .map(DocumentExport::from)
            .map_err(JsValue::from)
    }


    /// Document IR을 HWPX(ZIP+XML)로 직렬화하여 반환한다.
    #[wasm_bindgen(js_name = exportHwpx)]
    pub fn export_hwpx(&self) -> Result<Vec<u8>, JsValue> {
        self.export_hwpx_native().map_err(|e| e.into())
    }


    /// HWPX 바이트와 이번 산출물의 내용 손실을 같은 결과로 반환한다 (#4430).
    #[wasm_bindgen(js_name = exportHwpxWithReport)]
    pub fn export_hwpx_with_report(&self) -> Result<DocumentExport, JsValue> {
        self.export_hwpx_native_with_report()
            .map(DocumentExport::from)
            .map_err(JsValue::from)
    }


    /// 문서를 ODF AES-256-CBC/PBKDF2 비밀번호 보호 HWPX로 내보낸다.
    #[wasm_bindgen(js_name = exportHwpxWithPassword)]
    pub fn export_hwpx_with_password_wasm(&self, password: &str) -> Result<Vec<u8>, JsValue> {
        self.export_hwpx_native_with_password(password.as_bytes())
            .map_err(|e| e.into())
    }


    /// 비밀번호 HWPX 바이트 + 내용 손실 보고 (#4430).
    #[wasm_bindgen(js_name = exportHwpxWithPasswordAndReport)]
    pub fn export_hwpx_with_password_and_report_wasm(
        &self,
        password: &str,
    ) -> Result<DocumentExport, JsValue> {
        self.export_hwpx_native_with_password_and_report(password.as_bytes())
            .map(DocumentExport::from)
            .map_err(JsValue::from)
    }


    /// HML 원본의 공통 IR을 HWPML 2.91 XML로 직렬화하여 반환한다.
    #[wasm_bindgen(js_name = exportHml)]
    pub fn export_hml(&self) -> Result<Vec<u8>, JsValue> {
        self.export_hml_native()
            .map_err(|error| JsValue::from_str(&format_hml_export_error(&error)))
    }


    /// 어댑터 적용 + HWP 직렬화 + 자기 재로드 검증을 수행하고 결과를 JSON 으로 반환한다 (#178).
    ///
    /// 반환 JSON:
    /// ```json
    /// {
    ///   "bytesLen": 678912,
    ///   "pageCountBefore": 9,
    ///   "pageCountAfter": 9,
    ///   "recovered": true
    /// }
    /// ```
    ///
    /// 본 함수는 검증 메타데이터만 반환하며 bytes 자체는 별도 호출 (`exportHwp`) 로 받아야 한다.
    /// 검증과 실제 사용을 분리하여 호출자가 결과에 따라 다른 동작을 취할 수 있도록 한다.
    #[wasm_bindgen(js_name = exportHwpVerify)]
    pub fn export_hwp_verify(&mut self) -> Result<String, JsValue> {
        let v = self.serialize_hwp_with_verify().map_err(JsValue::from)?;
        Ok(format!(
            "{{\"bytesLen\":{},\"pageCountBefore\":{},\"pageCountAfter\":{},\"recovered\":{}}}",
            v.bytes_len, v.page_count_before, v.page_count_after, v.recovered
        ))
    }


    /// 선택 영역을 HTML 문자열로 변환한다 (본문).
    #[wasm_bindgen(js_name = exportSelectionHtml)]
    pub fn export_selection_html(
        &self,
        section_idx: u32,
        start_para_idx: u32,
        start_char_offset: u32,
        end_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.export_selection_html_native(
            section_idx as usize,
            start_para_idx as usize,
            start_char_offset as usize,
            end_para_idx as usize,
            end_char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 선택 영역을 HTML 문자열로 변환한다 (셀 내부).
    #[wasm_bindgen(js_name = exportSelectionInCellHtml)]
    pub fn export_selection_in_cell_html(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        start_cell_para_idx: u32,
        start_char_offset: u32,
        end_cell_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.export_selection_in_cell_html_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            start_cell_para_idx as usize,
            start_char_offset as usize,
            end_cell_para_idx as usize,
            end_char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// `exportSelectionInCellHtml` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, startCellParaIdx,
    /// startCharOffset, endCellParaIdx, endCharOffset }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = exportSelectionInCellHtmlEx)]
    pub fn export_selection_in_cell_html_ex(&self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.export_selection_in_cell_html_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startCellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startCharOffset").unwrap_or(0) as usize,
            json_u32(options_json, "endCellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "endCharOffset").unwrap_or(0) as usize,
        )
        .map_err(|e| e.into())
    }


    /// 전체 cellPath가 가리키는 중첩 셀 선택을 HTML로 변환한다(#4272).
    #[wasm_bindgen(js_name = exportSelectionInCellHtmlByPath)]
    pub fn export_selection_in_cell_html_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        start_cell_para_idx: u32,
        start_char_offset: u32,
        end_cell_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.export_selection_in_cell_html_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            start_cell_para_idx as usize,
            start_char_offset as usize,
            end_cell_para_idx as usize,
            end_char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 컨트롤 객체를 HTML 문자열로 변환한다.
    #[wasm_bindgen(js_name = exportControlHtml)]
    pub fn export_control_html(
        &self,
        section_idx: u32,
        para_idx: u32,
        cell_path_json: &str,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        let cell_path = parse_cell_path_arg(cell_path_json)?;
        self.export_control_html_native(
            section_idx as usize,
            para_idx as usize,
            &cell_path,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }

}
