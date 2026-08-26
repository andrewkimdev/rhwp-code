//! api_queries — table_layout.rs 에서 무변동 이동
use super::*;

#[wasm_bindgen]
impl HwpDocument {
    /// 문단부호(¶) 표시 여부를 반환한다.
    #[wasm_bindgen(js_name = getShowParagraphMarks)]
    pub fn get_show_paragraph_marks(&self) -> bool {
        self.show_paragraph_marks
    }


    /// 조판부호 표시 여부를 반환한다.
    #[wasm_bindgen(js_name = getShowControlCodes)]
    pub fn get_show_control_codes(&self) -> bool {
        self.show_control_codes
    }


    /// 투명선 표시 여부를 반환한다.
    #[wasm_bindgen(js_name = getShowTransparentBorders)]
    pub fn get_show_transparent_borders(&self) -> bool {
        self.show_transparent_borders
    }


    /// 페이지 렌더 트리를 JSON 문자열로 반환한다.
    #[wasm_bindgen(js_name = getPageRenderTree)]
    pub fn get_page_render_tree(&self, page_num: u32) -> Result<String, JsValue> {
        let tree = self
            .build_page_tree_cached(page_num)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(tree.root.to_json())
    }


    /// 페이지 레이어 트리를 JSON 문자열로 반환한다.
    ///
    /// screen profile 기본값이므로 `getPageLayerTreeWithProfile` 로 위임한다 — 같은 핫패치
    /// 경계를 지나야 한다. PageRenderer 가 좁은 질의를 못 쓸 때 되돌아오는 경로다.
    #[wasm_bindgen(js_name = getPageLayerTree)]
    pub fn get_page_layer_tree(&self, page_num: u32) -> Result<String, JsValue> {
        self.get_page_layer_tree_with_profile(page_num, "screen", Some(false))
    }


    /// 페이지 레이어 트리를 profile 별로 반환한다.
    ///
    /// [Task #3315] `omit_image_bytes` 를 `true` 로 주면 `sourceImageKey`를 낼 수 있는 그림만
    /// base64를 생략하고, 바이트는 `getSourceImageBytes(key)`로 따로 받는다. 키 없는 합성 그림은
    /// 소비자가 되찾을 방법이 없으므로 같은 `byKey` 요청에서도 인라인 base64를 유지한다.
    /// 인자를 생략하면(`undefined`) 그림 payload는 inline으로 유지하지만, schema minor 21과
    /// 최상위 `imageBytes:"inline"` 메타데이터가 있으므로 JSON 전체의 byte identity는 보장하지 않는다.
    #[wasm_bindgen(js_name = getPageLayerTreeWithProfile)]
    pub fn get_page_layer_tree_with_profile(
        &self,
        page_num: u32,
        profile: &str,
        omit_image_bytes: Option<bool>,
    ) -> Result<String, JsValue> {
        let omit_image_bytes = omit_image_bytes.unwrap_or(false);
        subsecond_boundary::get_page_layer_tree_with_profile(
            self,
            page_num,
            profile,
            omit_image_bytes,
        )
    }


    /// 지금 컴파일되어 있는 렌더 코드의 식별자. 값이 바뀌면 코드가 교체된 것이다.
    ///
    /// 소비자는 이 문자열을 해석하지 않고 이전 값과 비교만 한다. 오늘 그 값을 바꾸는 것은
    /// Subsecond 핫패치뿐이지만, 이름은 그 사실이 아니라 소비자가 알아야 하는 것을 말한다 —
    /// 벤더가 바뀌어도 "렌더 코드의 리비전"이라는 질문은 그대로다 (#4580). 벤더를 아는 곳은
    /// 몸통이 부르는 `subsecond_boundary` 하나다.
    ///
    /// 값은 경계 목록(`subsecond_boundary`)에서 바로 나오므로 경계를 더할 때 여기를 같이 고칠
    /// 일이 없다 — 리비전이 경계 하나를 놓쳐 재도색이 안 도는 구멍이 생기지 않는다.
    ///
    /// 아래 재구성과 **한 쌍이다.** 리비전이 바뀐 것을 보고 재구성을 부르는 것이 TS 계약
    /// (`rhwp-studio/src/core/subsecond-runtime.ts`)이므로 한쪽만 있는 빌드는 그 계약을 반만
    /// 만족한다. 그래서 게이트도 둘이 같아야 한다 — 이 함수의 몸통이 wasm32 전용 경계를
    /// 가리키므로 `wasm32` 가 조건에 들어가고, 짝인 재구성도 같은 조건을 쓴다.
    #[cfg(all(feature = "subsecond-dev", target_arch = "wasm32"))]
    #[wasm_bindgen(js_name = getRenderCodeRevision)]
    pub fn get_render_code_revision(&self) -> String {
        subsecond_boundary::patch_revision()
    }


    /// CanvasKit direct replay 정책 진단을 JSON 문자열로 반환한다.
    ///
    /// `mode` 는 `"default"` 또는 `"compat"` 를 받는다. 빈 문자열은 `"default"` 로 처리한다.
    /// 현재 두 mode 모두 hidden Canvas2D overlay 없이 direct replay required 정책을 따른다.
    /// `compat` 는 API/URL 호환성과 이후 보수적인 direct replay 튜닝을 위해 남겨 둔 선택지다.
    #[wasm_bindgen(js_name = getCanvasKitReplayPlan)]
    pub fn get_canvaskit_replay_plan(&self, page_num: u32, mode: &str) -> Result<String, JsValue> {
        self.get_canvaskit_replay_plan_native(page_num, mode)
            .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = getCanvasKitReplayPlanWithProfile)]
    pub fn get_canvaskit_replay_plan_with_profile(
        &self,
        page_num: u32,
        mode: &str,
        profile: &str,
    ) -> Result<String, JsValue> {
        let profile = crate::paint::RenderProfile::parse(profile)
            .ok_or_else(|| JsValue::from_str(&format!("unsupported render profile: {profile}")))?;
        self.get_canvaskit_replay_plan_with_profile_native(page_num, mode, profile)
            .map_err(|error| error.into())
    }


    /// 문서 전체의 bounded CanvasKit direct replay capability를 compact JSON으로 반환한다.
    #[wasm_bindgen(js_name = getCanvasKitDocumentPreflight)]
    pub fn get_canvaskit_document_preflight(
        &self,
        mode: &str,
        profile: &str,
    ) -> Result<String, JsValue> {
        let profile = crate::paint::RenderProfile::parse(profile)
            .ok_or_else(|| JsValue::from_str(&format!("unsupported render profile: {profile}")))?;
        self.get_canvaskit_document_preflight_native(mode, profile)
            .map_err(|error| error.into())
    }


    /// 페이지 overlay 이미지 정보만 JSON 문자열로 반환한다.
    #[wasm_bindgen(js_name = getPageOverlayImages)]
    pub fn get_page_overlay_images(&self, page_num: u32) -> Result<String, JsValue> {
        subsecond_boundary::get_page_overlay_images(self, page_num)
    }


    /// 페이지가 그리는 그림들의 신원 키만 작은 JSON 으로 반환한다 (Task #3315).
    #[wasm_bindgen(js_name = getPageSourceImageKeys)]
    pub fn get_page_source_image_keys(&self, page_num: u32) -> Result<String, JsValue> {
        self.get_page_source_image_keys_native(page_num)
            .map_err(|e| e.into())
    }


    /// 본문(flow) 그림의 배치 정보만 작은 JSON 으로 반환한다 (Task #3315).
    ///
    /// 전체 레이어 트리를 받아 flow 그림을 걸러내던 studio 경로를 대체한다. 바이트는 빠져
    /// 있고 `sourceImageKey` 로 `getSourceImageBytes` 를 부르면 된다.
    #[wasm_bindgen(js_name = getPageFlowImageOps)]
    pub fn get_page_flow_image_ops(&self, page_num: u32) -> Result<String, JsValue> {
        subsecond_boundary::get_page_flow_image_ops(self, page_num)
    }


    /// 그림 신원 키로 바이트를 Uint8Array 로 반환한다 (Task #3315).
    ///
    /// `getPageLayerTreeWithProfile(page, profile, true)` 로 base64 를 생략했을 때 바이트를
    /// 받는 경로다. mime 은 레이어 트리의 그림 op 이 계속 싣고 있으므로 여기서 되풀이하지
    /// 않는다.
    ///
    /// 키를 풀 수 없으면 던진다 — 세대가 바뀐 낡은 키이거나 없는 그림이다. 호출부는 잡아서
    /// 레이어 트리를 다시 받는 쪽으로 되돌아가면 된다.
    #[wasm_bindgen(js_name = getSourceImageBytes)]
    pub fn get_source_image_bytes(&self, key: &str) -> Result<Vec<u8>, JsValue> {
        match self.get_source_image_bytes_native(key) {
            Some((_mime, bytes)) => Ok(bytes),
            None => Err(JsValue::from_str(&format!(
                "unresolvable source image key: {key}"
            ))),
        }
    }


    /// 페이지 정보를 JSON 문자열로 반환한다.
    #[wasm_bindgen(js_name = getPageInfo)]
    pub fn get_page_info(&self, page_num: u32) -> Result<String, JsValue> {
        self.get_page_info_native(page_num).map_err(|e| e.into())
    }


    /// 구역의 용지 설정(PageDef)을 HWPUNIT 원본값으로 반환한다.
    #[wasm_bindgen(js_name = getPageDef)]
    pub fn get_page_def(&self, section_idx: u32) -> Result<String, JsValue> {
        self.get_page_def_native(section_idx as usize)
            .map_err(|e| e.into())
    }


    /// 구역 정의(SectionDef)를 JSON으로 반환한다.
    #[wasm_bindgen(js_name = getSectionDef)]
    pub fn get_section_def(&self, section_idx: u32) -> Result<String, JsValue> {
        self.get_section_def_native(section_idx as usize)
            .map_err(|e| e.into())
    }


    /// 구역의 쪽 테두리/배경 설정을 JSON으로 반환한다.
    #[wasm_bindgen(js_name = getPageBorderFill)]
    pub fn get_page_border_fill(&self, section_idx: u32) -> Result<String, JsValue> {
        self.get_page_border_fill_native(section_idx as usize)
            .map_err(|e| e.into())
    }


    /// 현재 구역의 다단 설정을 JSON으로 반환한다.
    #[wasm_bindgen(js_name = getColumnDef)]
    pub fn get_column_def(&self, section_idx: u32) -> Result<String, JsValue> {
        let sec = self
            .core
            .document
            .sections
            .get(section_idx as usize)
            .ok_or_else(|| JsValue::from_str("구역 인덱스 범위 초과"))?;
        let col_def = HwpDocument::find_initial_column_def(&sec.paragraphs);
        let col_type = match col_def.column_type {
            crate::model::page::ColumnType::Normal => 0,
            crate::model::page::ColumnType::Distribute => 1,
            crate::model::page::ColumnType::Parallel => 2,
        };
        Ok(format!(
            "{{\"columnCount\":{},\"columnType\":{},\"sameWidth\":{},\"spacing\":{}}}",
            col_def.column_count, col_type, col_def.same_width, col_def.spacing,
        ))
    }


    /// 문서 정보를 JSON 문자열로 반환한다.
    #[wasm_bindgen(js_name = getDocumentInfo)]
    pub fn get_document_info(&self) -> String {
        self.core.get_document_info()
    }


    /// 특정 페이지의 텍스트 레이아웃 정보를 JSON 문자열로 반환한다.
    ///
    /// 각 TextRun의 위치, 텍스트, 글자별 X 좌표 경계값을 포함한다.
    #[wasm_bindgen(js_name = getPageTextLayout)]
    pub fn get_page_text_layout(&self, page_num: u32) -> Result<String, JsValue> {
        self.get_page_text_layout_native(page_num)
            .map_err(|e| e.into())
    }


    /// 컨트롤(표, 이미지 등) 레이아웃 정보를 반환한다.
    #[wasm_bindgen(js_name = getPageControlLayout)]
    pub fn get_page_control_layout(&self, page_num: u32) -> Result<String, JsValue> {
        self.get_page_control_layout_native(page_num)
            .map_err(|e| e.into())
    }


    /// 현재 DPI를 반환한다.
    #[wasm_bindgen(js_name = getDpi)]
    pub fn get_dpi(&self) -> f64 {
        self.dpi
    }


    /// 현재 대체 폰트 경로를 반환한다.
    #[wasm_bindgen(js_name = getFallbackFont)]
    pub fn get_fallback_font(&self) -> String {
        self.fallback_font.clone()
    }


    /// 문단의 논리적 길이를 반환한다 (텍스트 문자 + 인라인 컨트롤 수).
    #[wasm_bindgen(js_name = getLogicalLength)]
    pub fn get_logical_length(&self, section_idx: u32, para_idx: u32) -> Result<u32, JsValue> {
        let sec = section_idx as usize;
        let pi = para_idx as usize;
        if sec >= self.document.sections.len() || pi >= self.document.sections[sec].paragraphs.len()
        {
            return Err(JsValue::from_str("인덱스 범위 초과"));
        }
        Ok(crate::document_core::helpers::logical_paragraph_length(
            &self.document.sections[sec].paragraphs[pi],
        ) as u32)
    }


    #[wasm_bindgen(js_name = getTextInCellByPath)]
    pub fn get_text_in_cell_by_path_api(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.get_text_in_cell_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
            count as usize,
        )
        .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 조회
    ///
    /// 반환: JSON `{"ok":true,"exists":true/false,...}`
    #[wasm_bindgen(js_name = getHeaderFooter)]
    pub fn get_header_footer(
        &self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
    ) -> Result<String, JsValue> {
        self.get_header_footer_native(section_idx as usize, is_header, apply_to)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 문단 정보 조회
    ///
    /// 반환: JSON `{"ok":true,"paraCount":N,"charCount":N}`
    #[wasm_bindgen(js_name = getHeaderFooterParaInfo)]
    pub fn get_header_footer_para_info(
        &self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_header_footer_para_info_native(
            section_idx as usize,
            is_header,
            apply_to,
            hf_para_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 구역(Section) 수를 반환한다.
    #[wasm_bindgen(js_name = getSectionCount)]
    pub fn get_section_count(&self) -> u32 {
        self.document.sections.len() as u32
    }


    /// 구역 내 문단 수를 반환한다.
    #[wasm_bindgen(js_name = getParagraphCount)]
    pub fn get_paragraph_count(&self, section_idx: u32) -> Result<u32, JsValue> {
        self.get_paragraph_count_native(section_idx as usize)
            .map(|v| v as u32)
            .map_err(|e| e.into())
    }


    /// 문단의 글자 수(char 개수)를 반환한다.
    #[wasm_bindgen(js_name = getParagraphLength)]
    pub fn get_paragraph_length(&self, section_idx: u32, para_idx: u32) -> Result<u32, JsValue> {
        self.get_paragraph_length_native(section_idx as usize, para_idx as usize)
            .map(|v| v as u32)
            .map_err(|e| e.into())
    }


    /// 문단에 텍스트박스가 있는 Shape 컨트롤이 있으면 해당 control_index를 반환한다.
    /// 없으면 -1을 반환한다.
    #[wasm_bindgen(js_name = getTextBoxControlIndex)]
    pub fn get_textbox_control_index(&self, section_idx: u32, para_idx: u32) -> i32 {
        self.get_textbox_control_index_native(section_idx as usize, para_idx as usize)
    }


    /// 문단 내 컨트롤의 텍스트 위치 배열을 반환한다.
    #[wasm_bindgen(js_name = getControlTextPositions)]
    pub fn get_control_text_positions(&self, section_idx: u32, para_idx: u32) -> String {
        let sections = &self.document.sections;
        if let Some(sec) = sections.get(section_idx as usize) {
            if let Some(para) = sec.paragraphs.get(para_idx as usize) {
                let positions = crate::document_core::find_control_text_positions(para);
                return format!(
                    "[{}]",
                    positions
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
        "[]".to_string()
    }


    /// 문단에서 텍스트 부분 문자열을 반환한다 (Undo용 텍스트 보존).
    #[wasm_bindgen(js_name = getTextRange)]
    pub fn get_text_range(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        self.get_text_range_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            count as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내 문단 수를 반환한다.
    #[wasm_bindgen(js_name = getCellParagraphCount)]
    pub fn get_cell_paragraph_count(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
    ) -> Result<u32, JsValue> {
        self.get_cell_paragraph_count_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
        )
        .map(|v| v as u32)
        .map_err(|e| e.into())
    }


    /// 표 셀 내 문단의 글자 수를 반환한다.
    #[wasm_bindgen(js_name = getCellParagraphLength)]
    pub fn get_cell_paragraph_length(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
    ) -> Result<u32, JsValue> {
        self.get_cell_paragraph_length_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
        )
        .map(|v| v as u32)
        .map_err(|e| e.into())
    }


    /// 경로 기반: 셀/글상자 내 문단 수를 반환한다 (중첩 표/글상자 지원).
    #[wasm_bindgen(js_name = getCellParagraphCountByPath)]
    pub fn get_cell_paragraph_count_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
    ) -> Result<u32, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        let count = self
            .resolve_container_para_count_by_path(
                section_idx as usize,
                parent_para_idx as usize,
                &path,
            )
            .map_err(|e| -> JsValue { e.into() })?;
        Ok(count as u32)
    }


    /// 경로 기반: 셀 내 문단의 글자 수를 반환한다 (중첩 표 지원).
    #[wasm_bindgen(js_name = getCellParagraphLengthByPath)]
    pub fn get_cell_paragraph_length_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
    ) -> Result<u32, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        let para = self
            .resolve_paragraph_by_path(section_idx as usize, parent_para_idx as usize, &path)
            .map_err(|e| -> JsValue { e.into() })?;
        Ok(para.text.chars().count() as u32)
    }


    /// 표 셀의 텍스트 방향을 반환한다 (0=가로, 1=세로/영문눕힘, 2=세로/영문세움).
    #[wasm_bindgen(js_name = getCellTextDirection)]
    pub fn get_cell_text_direction(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
    ) -> Result<u32, JsValue> {
        let para = self
            .document
            .sections
            .get(section_idx as usize)
            .ok_or_else(|| JsValue::from_str("구역 인덱스 범위 초과"))?
            .paragraphs
            .get(parent_para_idx as usize)
            .ok_or_else(|| JsValue::from_str("문단 인덱스 범위 초과"))?;
        match para.controls.get(control_idx as usize) {
            Some(Control::Table(table)) => {
                let cell = table
                    .cells
                    .get(cell_idx as usize)
                    .ok_or_else(|| JsValue::from_str("셀 인덱스 범위 초과"))?;
                Ok(cell.text_direction as u32)
            }
            _ => Ok(0), // 글상자 등은 가로쓰기
        }
    }


    /// 표 셀 내 문단에서 텍스트 부분 문자열을 반환한다.
    #[wasm_bindgen(js_name = getTextInCell)]
    pub fn get_text_in_cell(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        self.get_text_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            count as usize,
        )
        .map_err(|e| e.into())
    }


    /// `getTextInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, count }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = getTextInCellEx)]
    pub fn get_text_in_cell_ex(&self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.get_text_in_cell_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            json_u32(options_json, "count").unwrap_or(0) as usize,
        )
        .map_err(|e| e.into())
    }


    /// 커서 위치의 픽셀 좌표를 반환한다.
    ///
    /// 반환: JSON `{"pageIndex":N,"x":F,"y":F,"height":F}`
    #[wasm_bindgen(js_name = getCursorRect)]
    pub fn get_cursor_rect(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_cursor_rect_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 줄 경계 offset을 특정 시각 줄 기준으로 해석한 커서 좌표를 반환한다.
    ///
    /// `at_end=false`이면 lineIndex 줄의 시작, `at_end=true`이면 lineIndex 줄의 끝을 반환한다.
    /// soft-wrap 경계에서는 같은 charOffset이 이전 줄 끝과 다음 줄 시작을 동시에 뜻할 수 있어
    /// Home/End가 이 API로 시각 줄 affinity를 명시한다.
    #[wasm_bindgen(js_name = getCursorRectOnLine)]
    pub fn get_cursor_rect_on_line(
        &self,
        section_idx: u32,
        para_idx: u32,
        line_index: u32,
        at_end: bool,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
    ) -> Result<String, JsValue> {
        let cell_ctx = if parent_para_idx == u32::MAX {
            None
        } else {
            Some((
                parent_para_idx as usize,
                control_idx as usize,
                cell_idx as usize,
                cell_para_idx as usize,
            ))
        };
        self.get_cursor_rect_on_line_native(
            section_idx as usize,
            para_idx as usize,
            line_index as usize,
            at_end,
            cell_ctx,
        )
        .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 내 커서 위치의 픽셀 좌표를 반환한다.
    ///
    /// preferred_page: 선호 페이지 (더블클릭한 페이지). -1이면 첫 번째 발견 페이지 사용.
    /// 반환: JSON `{"pageIndex":N,"x":F,"y":F,"height":F}`
    #[wasm_bindgen(js_name = getCursorRectInHeaderFooter)]
    pub fn get_cursor_rect_in_header_footer(
        &self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: u32,
        char_offset: u32,
        preferred_page: i32,
    ) -> Result<String, JsValue> {
        self.get_cursor_rect_in_header_footer_native(
            section_idx as usize,
            is_header,
            apply_to,
            hf_para_idx as usize,
            char_offset as usize,
            preferred_page,
        )
        .map_err(|e| e.into())
    }


    /// 이 쪽에서 머리말/꼬리말을 편집할 때 대상이 되는 (구역, applyTo) 를 반환한다.
    ///
    /// 좌표 없이 쪽만으로 묻는 경로(툴바 `머리말`/`꼬리말`)용 — 히트테스트와 같은 답을 쓴다.
    /// 반환: JSON `{"ok":true,"sectionIndex":N,"applyTo":N}`
    #[wasm_bindgen(js_name = getHeaderFooterEditTarget)]
    pub fn get_header_footer_edit_target(
        &self,
        page_num: u32,
        is_header: bool,
    ) -> Result<String, JsValue> {
        self.get_header_footer_edit_target_native(page_num, is_header)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 문단의 문단 속성을 조회한다.
    #[wasm_bindgen(js_name = getParaPropertiesInHf)]
    pub fn get_para_properties_in_hf(
        &self,
        section_idx: usize,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: usize,
    ) -> Result<String, JsValue> {
        self.get_para_properties_in_hf_native(section_idx, is_header, apply_to, hf_para_idx)
            .map_err(|e| e.into())
    }


    /// 문서 전체의 머리말/꼬리말 목록을 반환한다.
    #[wasm_bindgen(js_name = getHeaderFooterList)]
    pub fn get_header_footer_list(
        &self,
        current_section_idx: u32,
        current_is_header: bool,
        current_apply_to: u32,
    ) -> Result<String, JsValue> {
        self.get_header_footer_list_native(
            current_section_idx as usize,
            current_is_header,
            current_apply_to as u8,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내부 커서 위치의 픽셀 좌표를 반환한다.
    ///
    /// 반환: JSON `{"pageIndex":N,"x":F,"y":F,"height":F}`
    #[wasm_bindgen(js_name = getCursorRectInCell)]
    pub fn get_cursor_rect_in_cell(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_cursor_rect_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 문단 내 줄 정보를 반환한다 (커서 수직 이동/Home/End용).
    ///
    /// 반환: JSON `{"lineIndex":N,"lineCount":N,"charStart":N,"charEnd":N}`
    #[wasm_bindgen(js_name = getLineInfo)]
    pub fn get_line_info(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_line_info_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내 문단의 줄 정보를 반환한다.
    ///
    /// 반환: JSON `{"lineIndex":N,"lineCount":N,"charStart":N,"charEnd":N}`
    #[wasm_bindgen(js_name = getLineInfoInCell)]
    pub fn get_line_info_in_cell(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_line_info_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 문서에 저장된 캐럿 위치를 반환한다 (문서 로딩 시 캐럿 자동 배치용).
    ///
    /// 반환: JSON `{"sectionIndex":N,"paragraphIndex":N,"charOffset":N}`
    #[wasm_bindgen(js_name = getCaretPosition)]
    pub fn get_caret_position(&self) -> Result<String, JsValue> {
        self.get_caret_position_native().map_err(|e| e.into())
    }


    /// 표의 행/열/셀 수를 반환한다.
    ///
    /// 반환: JSON `{"rowCount":N,"colCount":N,"cellCount":N}`
    #[wasm_bindgen(js_name = getTableDimensions)]
    pub fn get_table_dimensions(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_table_dimensions_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀의 행/열/병합 정보를 반환한다.
    ///
    /// 반환: JSON `{"row":N,"col":N,"rowSpan":N,"colSpan":N}`
    #[wasm_bindgen(js_name = getCellInfo)]
    pub fn get_cell_info(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_cell_info_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 셀 속성을 조회한다.
    ///
    /// 반환: JSON `{width, height, paddingLeft, paddingRight, paddingTop, paddingBottom, applyInnerMargin, verticalAlign, textDirection, isHeader, cellProtect, fieldName, editableInForm, ...borderFill}`
    #[wasm_bindgen(js_name = getCellProperties)]
    pub fn get_cell_properties(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_cell_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 셀 고유 속성을 조회한다.
    ///
    /// cellzone overlay를 합성하지 않고 셀 자체의 borderFill만 반환한다.
    #[wasm_bindgen(js_name = getCellOwnProperties)]
    pub fn get_cell_own_properties(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_cell_own_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표 속성을 조회한다.
    ///
    /// 반환: JSON `{cellSpacing, paddingLeft, paddingRight, paddingTop, paddingBottom, pageBreak, repeatHeader}`
    #[wasm_bindgen(js_name = getTableProperties)]
    pub fn get_table_properties(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_table_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표의 모든 셀 bbox를 반환한다 (F5 셀 선택 모드용).
    ///
    /// 반환: JSON `[{cellIdx, row, col, rowSpan, colSpan, pageIndex, x, y, w, h}, ...]`
    #[wasm_bindgen(js_name = getTableCellBboxes)]
    pub fn get_table_cell_bboxes(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        page_hint: Option<u32>,
    ) -> Result<String, JsValue> {
        self.get_table_cell_bboxes_from_page(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            page_hint.unwrap_or(0) as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표 전체의 바운딩박스를 반환한다.
    ///
    /// 반환: JSON `{"pageIndex":<N>,"x":<f>,"y":<f>,"width":<f>,"height":<f>}`
    #[wasm_bindgen(js_name = getTableBBox)]
    pub fn get_table_bbox(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_table_bbox_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 지정 page 에 배치된 표 fragment 의 바운딩박스를 반환한다 (#2400).
    ///
    /// 반환: JSON `{"pageIndex":<N>,"x":<f>,"y":<f>,"width":<f>,"height":<f>}`
    #[wasm_bindgen(js_name = getTableBBoxAtPage)]
    pub fn get_table_bbox_at_page(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        page_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_table_bbox_at_page_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            page_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// [Task #919] 글상자/도형 컨트롤의 페이지 좌표 바운딩박스를 반환한다.
    ///
    /// 반환: JSON `{"pageIndex":<N>,"x":<f>,"y":<f>,"width":<f>,"height":<f>}`
    /// studio 의 `isShapeBorderClick` 헬퍼에서 외곽 경계선 클릭 판별에 사용.
    #[wasm_bindgen(js_name = getShapeBBox)]
    pub fn get_shape_bbox(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_shape_bbox_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// [Task #1142] 외부 file path 그림 reference 목록을 구조화된 JSON 배열로 반환한다.
    ///
    /// 반환: JSON 배열 `[{ key, binDataId, originalPath, basename, extension, loaded }, ...]`
    #[wasm_bindgen(js_name = getExternalImageReferences)]
    pub fn get_external_image_references(&self) -> String {
        serde_json::to_string(&collect_external_image_references(self.document()))
            .unwrap_or_else(|_| "[]".to_string())
    }


    /// [Task #741 후속] 외부 file path 그림 영역 영역 영역 영역 basename 목록 영역 반환.
    ///
    /// HWP3 파일 영역 image 영역 영역 절대 경로 영역 저장 영역. WASM 환경 영역 영역 file
    /// system access 부재 영역, JS 영역 영역 영역 영역 fetch 영역 영역 영역 file 영역 load
    /// 영역 후 `injectExternalImage` 영역 영역 영역 inject 영역.
    ///
    /// 반환: JSON 배열 `["oracle.gif", "rdb02.gif", ...]` (중복 제거)
    #[wasm_bindgen(js_name = getExternalImageBasenames)]
    pub fn get_external_image_basenames(&self) -> String {
        use std::collections::BTreeSet;

        let mut names: BTreeSet<String> = BTreeSet::new();
        for reference in collect_external_image_references(self.document()) {
            if !reference.loaded {
                names.insert(reference.basename);
            }
        }
        let arr: Vec<String> = names.into_iter().collect();
        serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
    }


    /// 그림 컨트롤의 속성을 조회한다.
    ///
    /// 반환: JSON `{ width, height, treatAsChar, ... }`
    #[wasm_bindgen(js_name = getPictureProperties)]
    pub fn get_picture_properties(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_picture_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// [Task #825] 머리말/꼬리말 안 그림의 속성 조회.
    /// path: section[si].paragraphs[outer_para].controls[outer_ctrl] = Header/Footer
    ///       → .paragraphs[inner_para].controls[inner_ctrl] = Picture
    #[wasm_bindgen(js_name = getHeaderFooterPictureProperties)]
    pub fn get_header_footer_picture_properties(
        &self,
        section_idx: u32,
        outer_para_idx: u32,
        outer_control_idx: u32,
        inner_para_idx: u32,
        inner_control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_header_footer_picture_properties_native(
            section_idx as usize,
            outer_para_idx as usize,
            outer_control_idx as usize,
            inner_para_idx as usize,
            inner_control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// [Task #1138] 표 셀 내 Shape(글상자/사각형/도형) 속성 조회 (by_path).
    #[wasm_bindgen(js_name = getCellShapePropertiesByPath)]
    pub fn get_cell_shape_properties_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        cell_path_json: &str,
        inner_control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_cell_shape_properties_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            cell_path_json,
            inner_control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// [Task #1151 v4] 표 셀 내 Picture 속성 조회 (by_path). Shape 패턴 정합.
    #[wasm_bindgen(js_name = getCellPicturePropertiesByPath)]
    pub fn get_cell_picture_properties_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        cell_path_json: &str,
        inner_control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_cell_picture_properties_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            cell_path_json,
            inner_control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 수식 컨트롤의 속성을 조회한다.
    ///
    /// 반환: JSON `{ script, fontSize, color, baseline, fontName }`
    #[wasm_bindgen(js_name = getEquationProperties)]
    pub fn get_equation_properties(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: i32,
        cell_para_idx: i32,
    ) -> Result<String, JsValue> {
        let ci = if cell_idx >= 0 {
            Some(cell_idx as usize)
        } else {
            None
        };
        let cpi = if cell_para_idx >= 0 {
            Some(cell_para_idx as usize)
        } else {
            None
        };
        self.get_equation_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            ci,
            cpi,
        )
        .map_err(|e| e.into())
    }


    /// 각주/미주 내부 수식 컨트롤의 속성을 조회한다.
    #[wasm_bindgen(js_name = getNoteEquationProperties)]
    pub fn get_note_equation_properties(
        &self,
        kind: &str,
        section_idx: u32,
        parent_para_idx: u32,
        note_control_idx: u32,
        note_para_idx: u32,
        inner_control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_note_equation_properties_native(
            kind,
            section_idx as usize,
            parent_para_idx as usize,
            note_control_idx as usize,
            note_para_idx as usize,
            inner_control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// Shape(글상자) 속성을 조회한다.
    ///
    /// 반환: JSON `{ width, height, treatAsChar, tbMarginLeft, ... }`
    #[wasm_bindgen(js_name = getShapeProperties)]
    pub fn get_shape_properties(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_shape_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 미주 모양을 조회한다.
    #[wasm_bindgen(js_name = getEndnoteShape)]
    pub fn get_endnote_shape(&self, section_idx: u32) -> Result<String, JsValue> {
        self.get_endnote_shape_native(section_idx as usize)
            .map_err(|e| e.into())
    }


    /// 각주 정보를 조회한다.
    #[wasm_bindgen(js_name = getFootnoteInfo)]
    pub fn get_footnote_info(
        &self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_footnote_info_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 본문 커서 위치의 각주 마커를 조회한다.
    ///
    /// direction: "backward" 또는 "forward"
    #[wasm_bindgen(js_name = getFootnoteAtCursor)]
    pub fn get_footnote_at_cursor(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        direction: &str,
    ) -> Result<String, JsValue> {
        self.get_footnote_at_cursor_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            direction,
        )
        .map_err(|e| e.into())
    }


    /// 페이지의 각주 참조 정보
    #[wasm_bindgen(js_name = getPageFootnoteInfo)]
    pub fn get_page_footnote_info(
        &self,
        page_num: u32,
        footnote_index: u32,
    ) -> Result<String, JsValue> {
        self.get_page_footnote_info_native(page_num, footnote_index as usize)
            .map_err(|e| e.into())
    }


    /// 각주 내 커서 렉트 계산
    #[wasm_bindgen(js_name = getCursorRectInFootnote)]
    pub fn get_cursor_rect_in_footnote(
        &self,
        page_num: u32,
        footnote_index: u32,
        fn_para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_cursor_rect_in_footnote_native(
            page_num,
            footnote_index as usize,
            fn_para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 각주/미주 편집 모드 진입 대상 조회
    #[wasm_bindgen(js_name = getNoteEditInfo)]
    pub fn get_note_edit_info(
        &self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_note_edit_info_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 각주/미주 내부 커서 렉트 계산
    #[wasm_bindgen(js_name = getCursorRectInNote)]
    pub fn get_cursor_rect_in_note(
        &self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
        note_para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_cursor_rect_in_note_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
            note_para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 각주/미주 내부 문단 속성 조회
    #[wasm_bindgen(js_name = getParaPropertiesInFootnote)]
    pub fn get_para_properties_in_footnote(
        &self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
        fn_para_idx: u32,
    ) -> Result<String, JsValue> {
        self.get_para_properties_in_footnote_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
            fn_para_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 문서 내 모든 필드 목록을 JSON 배열로 반환한다.
    ///
    /// 반환: `[{fieldId, fieldType, name, guide, command, value, location}]`
    #[wasm_bindgen(js_name = getFieldList)]
    pub fn get_field_list(&self) -> String {
        self.get_field_list_json()
    }


    /// field_id로 필드 값을 조회한다.
    ///
    /// 반환: `{ok, value}`
    #[wasm_bindgen(js_name = getFieldValue)]
    pub fn get_field_value(&self, field_id: u32) -> Result<String, JsValue> {
        self.get_field_value_by_id(field_id).map_err(|e| e.into())
    }


    /// 필드 이름으로 값을 조회한다.
    ///
    /// 반환: `{ok, fieldId, value}`
    #[wasm_bindgen(js_name = getFieldValueByName)]
    pub fn get_field_value_by_name_api(&self, name: &str) -> Result<String, JsValue> {
        self.get_field_value_by_name(name).map_err(|e| e.into())
    }


    /// 페이지 좌표에서 양식 개체를 찾는다.
    ///
    /// 반환: `{found, sec, para, ci, formType, name, value, caption, text, bbox}`
    #[wasm_bindgen(js_name = getFormObjectAt)]
    pub fn get_form_object_at(&self, page_num: u32, x: f64, y: f64) -> Result<String, JsValue> {
        self.core
            .get_form_object_at_native(page_num, x, y)
            .map_err(|e| e.into())
    }


    /// 양식 개체 값을 조회한다.
    ///
    /// 반환: `{ok, formType, name, value, text, caption, enabled}`
    #[wasm_bindgen(js_name = getFormValue)]
    pub fn get_form_value(&self, sec: u32, para: u32, ci: u32) -> Result<String, JsValue> {
        self.core
            .get_form_value_native(sec as usize, para as usize, ci as usize)
            .map_err(|e| e.into())
    }


    /// 양식 개체 상세 정보를 반환한다 (properties 포함).
    ///
    /// 반환: `{ok, formType, name, value, text, caption, enabled, width, height, foreColor, backColor, properties}`
    #[wasm_bindgen(js_name = getFormObjectInfo)]
    pub fn get_form_object_info(&self, sec: u32, para: u32, ci: u32) -> Result<String, JsValue> {
        self.core
            .get_form_object_info_native(sec as usize, para as usize, ci as usize)
            .map_err(|e| e.into())
    }


    /// 글로벌 쪽 번호에 해당하는 첫 문단 위치 반환
    #[wasm_bindgen(js_name = getPositionOfPage)]
    pub fn get_position_of_page(&self, global_page: u32) -> Result<String, JsValue> {
        self.core
            .get_position_of_page_native(global_page as usize)
            .map_err(|e| e.into())
    }


    /// 위치에 해당하는 글로벌 쪽 번호 반환
    #[wasm_bindgen(js_name = getPageOfPosition)]
    pub fn get_page_of_position(&self, section_idx: u32, para_idx: u32) -> Result<String, JsValue> {
        self.core
            .get_page_of_position_native(section_idx as usize, para_idx as usize)
            .map_err(|e| e.into())
    }


    /// 커서 위치의 필드 범위 정보를 조회한다 (본문 문단).
    ///
    /// 반환: `{inField, fieldId?, startCharIdx?, endCharIdx?, isGuide?, guideName?, editableInForm?}`
    #[wasm_bindgen(js_name = getFieldInfoAt)]
    pub fn get_field_info_at_api(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> String {
        self.get_field_info_at(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
    }


    /// 커서 위치의 필드 범위 정보를 조회한다 (셀/글상자 내 문단).
    #[wasm_bindgen(js_name = getFieldInfoAtInCell)]
    pub fn get_field_info_at_in_cell_api(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        is_textbox: bool,
    ) -> String {
        self.get_field_info_at_in_cell(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            is_textbox,
        )
    }


    /// `getFieldInfoAtInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, isTextbox? }`. positional 과 동일 동작(String 반환).
    #[wasm_bindgen(js_name = getFieldInfoAtInCellEx)]
    pub fn get_field_info_at_in_cell_ex(&self, options_json: &str) -> String {
        use crate::document_core::helpers::{json_bool, json_u32};
        self.get_field_info_at_in_cell(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            json_bool(options_json, "isTextbox").unwrap_or(false),
        )
    }


    /// path 기반: 중첩 표 셀의 필드 범위 정보를 조회한다.
    #[wasm_bindgen(js_name = getFieldInfoAtByPath)]
    pub fn get_field_info_at_by_path_api(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
    ) -> String {
        match DocumentCore::parse_cell_path(path_json) {
            Ok(path) => self.get_field_info_at_by_path(
                section_idx as usize,
                parent_para_idx as usize,
                &path,
                char_offset as usize,
            ),
            Err(_) => r#"{"inField":false}"#.to_string(),
        }
    }


    /// 누름틀 필드의 속성을 조회한다.
    ///
    /// 반환: JSON `{"ok":true,"guide":"안내문","memo":"메모","name":"이름","editable":true}`
    #[wasm_bindgen(js_name = getClickHereProps)]
    pub fn get_click_here_props(&self, field_id: u32) -> String {
        use crate::model::control::{Control, FieldType};
        // 문서 전체에서 fieldId로 필드 찾기
        for sec in &self.document.sections {
            for para in &sec.paragraphs {
                for ctrl in &para.controls {
                    if let Control::Field(f) = ctrl {
                        if f.field_id == field_id && f.field_type == FieldType::ClickHere {
                            return self.format_click_here_props(f);
                        }
                    }
                }
                // 표/글상자 내부도 탐색
                for ctrl in &para.controls {
                    let paras: Vec<&crate::model::paragraph::Paragraph> = match ctrl {
                        Control::Table(t) => t.cells.iter().flat_map(|c| &c.paragraphs).collect(),
                        Control::Shape(s) => s
                            .drawing()
                            .and_then(|d| d.text_box.as_ref())
                            .map(|tb| tb.paragraphs.iter().collect())
                            .unwrap_or_default(),
                        _ => Vec::new(),
                    };
                    for p in paras {
                        for c in &p.controls {
                            if let Control::Field(f) = c {
                                if f.field_id == field_id && f.field_type == FieldType::ClickHere {
                                    return self.format_click_here_props(f);
                                }
                            }
                        }
                    }
                }
            }
        }
        r#"{"ok":false}"#.to_string()
    }


    /// 문서가 담은 컨트롤 사슬 — `HeadCtrl`·`LastCtrl` 과 `Next`·`Prev` 가 딛는다.
    #[wasm_bindgen(js_name = getControls)]
    pub fn get_controls(&self) -> String {
        self.controls_json()
    }


    /// 문서 글 전체 — `GetTextFile("TEXT")`. CP949 수치 참조를 적용한 JSON 문자열이다.
    #[wasm_bindgen(js_name = getTextFileText)]
    pub fn get_text_file_text(&self) -> String {
        self.text_file_json()
    }


    /// 문서 글 전체 — `GetTextFile("UNICODE")`. 원문 Unicode JSON 문자열이다.
    #[wasm_bindgen(js_name = getTextFileUnicode)]
    pub fn get_text_file_unicode(&self) -> String {
        self.text_file_unicode_json()
    }


    /// 문서 글을 한글 스캔 차례로 — `InitScan`·`GetText`·`ReleaseScan` 이 쓴다.
    #[wasm_bindgen(js_name = getScanItems)]
    pub fn get_scan_items(&self) -> String {
        self.scan_items_json()
    }


    /// 구역마다 첫 본문 문단 번호 — `MoveSectionUp`·`MoveSectionDown` 이 딛는다.
    #[wasm_bindgen(js_name = getSectionStarts)]
    pub fn get_section_starts(&self) -> String {
        self.section_starts_json()
    }


    /// 커서가 든 필드의 상태 — 웹한글컨트롤 `CurFieldState`.
    #[wasm_bindgen(js_name = getCurFieldState)]
    pub fn get_cur_field_state(&self, list_id: u32, para_in_list: u32, pos: u32) -> u32 {
        self.cur_field_state(list_id, para_in_list as usize, pos as usize)
    }


    /// 커서가 든 셀의 모양 — 웹한글컨트롤 `CellShape` 파라미터셋.
    #[wasm_bindgen(js_name = getCellShapeSet)]
    pub fn get_cell_shape_set(&self, list_id: u32) -> String {
        self.cell_shape_set_json(list_id)
    }


    /// 본문에 놓인 개체 목록 — `Run("ShapeObjNextObject")` 따위가 딛는다.
    #[wasm_bindgen(js_name = getObjects)]
    pub fn get_objects(&self) -> String {
        self.objects_json()
    }


    /// 지금 단어의 끝 — `MoveWordEnd` 가 가는 자리(다음 공백 글자의 자리).
    #[wasm_bindgen(js_name = getWordEnd)]
    pub fn get_word_end(&self, list_id: u32, para_in_list: u32, pos: u32) -> String {
        self.word_end_json(list_id, para_in_list as usize, pos as usize)
    }


    /// 단어가 시작하는 자리들 — `MoveNextWord` 류가 딛는 눈금(코드 유닛).
    #[wasm_bindgen(js_name = getWordStarts)]
    pub fn get_word_starts(&self, list_id: u32, para_in_list: u32) -> String {
        self.word_starts_json(list_id, para_in_list as usize)
    }


    /// 줄이 시작하는 자리들 — `MoveLineBegin`·`MoveLineEnd` 가 딛는 값(코드 유닛).
    #[wasm_bindgen(js_name = getLineStarts)]
    pub fn get_line_starts(&self, list_id: u32, para_in_list: u32) -> String {
        self.line_starts_json(list_id, para_in_list as usize)
    }


    /// 캐럿이 설 수 있는 자리들 — 한 글자 이동(`MoveNextChar` 류)이 딛는 눈금.
    #[wasm_bindgen(js_name = getCaretStops)]
    pub fn get_caret_stops(&self, list_id: u32, para_in_list: u32) -> String {
        self.caret_stops_json(list_id, para_in_list as usize)
    }


    /// 문단 하나의 캐럿 경계 — `MoveParaBegin`·`MoveParaEnd`·`MoveListBegin/End` 가 딛는 값.
    #[wasm_bindgen(js_name = getParaBounds)]
    pub fn get_para_bounds(&self, list_id: u32, para_in_list: u32) -> String {
        self.para_bounds_json(list_id, para_in_list as usize)
    }


    /// 커서 자리의 글자 모양 — 웹한글컨트롤 `CharShape` 파라미터셋 값(§8.2.2).
    ///
    /// 항목 이름과 단위는 한글 것이다(`Height` 는 HWPUNIT, `AlignType` 은 코드값).
    #[wasm_bindgen(js_name = getCharShapeSet)]
    pub fn get_char_shape_set(&self, list_id: u32, para_in_list: u32, pos: u32) -> String {
        self.char_shape_set_json(list_id, para_in_list as usize, pos as usize)
    }


    /// 커서 자리의 문단 모양 — 웹한글컨트롤 `ParaShape` 파라미터셋 값(§8.2.11).
    #[wasm_bindgen(js_name = getParaShapeSet)]
    pub fn get_para_shape_set(&self, list_id: u32, para_in_list: u32) -> String {
        self.para_shape_set_json(list_id, para_in_list as usize)
    }


    /// 한글 커서 좌표계(`list`/`para`/`pos`)를 쓰는 데 필요한 문서 사실.
    ///
    /// 리스트 표와 루트 리스트의 시작·끝 위치를 함께 준다. 자세한 계약은
    /// `DocumentCore::get_cursor_model_json`.
    #[wasm_bindgen(js_name = getCursorModel)]
    pub fn get_cursor_model(&self) -> String {
        self.get_cursor_model_json()
    }


    /// 문서에 저장된 캐럿 위치를 **원본 값 그대로** 돌려준다.
    ///
    /// 한글은 문서를 열면 이 자리에 캐럿을 놓는다(`GetPos` 첫 답과 일치). studio 의
    /// `getCaretPosition` 은 이 값을 구역/문단으로 해석하지만, 여기서는 해석하지 않는다 —
    /// `list` 는 구역 번호가 아니라 리스트 아이디다.
    #[wasm_bindgen(js_name = getStoredCaret)]
    pub fn get_stored_caret(&self) -> String {
        let props = &self.document.doc_properties;
        format!(
            "{{\"list\":{},\"para\":{},\"pos\":{}}}",
            props.caret_list_id, props.caret_para_id, props.caret_char_pos,
        )
    }


    /// 경로 기반 커서 좌표 조회 (중첩 표용).
    ///
    /// path_json: `[{"controlIndex":N,"cellIndex":N,"cellParaIndex":N}, ...]`
    /// 반환: JSON `{"pageIndex":N,"x":F,"y":F,"height":F}`
    #[wasm_bindgen(js_name = getCursorRectByPath)]
    pub fn get_cursor_rect_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_cursor_rect_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            path_json,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// [#2021] 경로 기반 커서 좌표 조회 + 페이지 힌트 — 직전 캐럿 페이지를 전달하면
    /// 해당 페이지(±1)를 먼저 탐색해, 거대 표 문서에서 캐시 무효화 직후의 선형 페이지
    /// 재빌드 비용을 피한다. 힌트가 틀려도 종전 전체 탐색으로 fallback (좌표 불변).
    #[wasm_bindgen(js_name = getCursorRectByPathNear)]
    pub fn get_cursor_rect_by_path_near(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        hint_page: u32,
    ) -> Result<String, JsValue> {
        self.get_cursor_rect_by_path_with_hint(
            section_idx as usize,
            parent_para_idx as usize,
            path_json,
            char_offset as usize,
            Some(hint_page),
        )
        .map_err(|e| e.into())
    }


    /// 경로 기반 셀 정보 조회 (중첩 표용).
    ///
    /// 반환: JSON `{"row":N,"col":N,"rowSpan":N,"colSpan":N}`
    #[wasm_bindgen(js_name = getCellInfoByPath)]
    pub fn get_cell_info_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
    ) -> Result<String, JsValue> {
        self.get_cell_info_by_path_native(section_idx as usize, parent_para_idx as usize, path_json)
            .map_err(|e| e.into())
    }


    /// 경로 기반 표 차원 조회 (중첩 표용).
    ///
    /// 반환: JSON `{"rowCount":N,"colCount":N,"cellCount":N}`
    #[wasm_bindgen(js_name = getTableDimensionsByPath)]
    pub fn get_table_dimensions_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
    ) -> Result<String, JsValue> {
        self.get_table_dimensions_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            path_json,
        )
        .map_err(|e| e.into())
    }


    /// 경로 기반 표 셀 바운딩박스 조회 (중첩 표용).
    ///
    /// 반환: JSON 배열 `[{"cellIdx":N,"row":N,"col":N,...,"x":F,"y":F,"w":F,"h":F}, ...]`
    #[wasm_bindgen(js_name = getTableCellBboxesByPath)]
    pub fn get_table_cell_bboxes_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
    ) -> Result<String, JsValue> {
        self.get_table_cell_bboxes_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            path_json,
        )
        .map_err(|e| e.into())
    }


    /// 본문 선택 영역의 줄별 사각형을 반환한다.
    ///
    /// 반환: JSON 배열 `[{"pageIndex":N,"x":F,"y":F,"width":F,"height":F}, ...]`
    #[wasm_bindgen(js_name = getSelectionRects)]
    pub fn get_selection_rects(
        &self,
        section_idx: u32,
        start_para_idx: u32,
        start_char_offset: u32,
        end_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_selection_rects_native(
            section_idx as usize,
            start_para_idx as usize,
            start_char_offset as usize,
            end_para_idx as usize,
            end_char_offset as usize,
            None,
            None,
        )
        .map_err(|e| e.into())
    }


    /// 셀 내 선택 영역의 줄별 사각형을 반환한다.
    ///
    /// 반환: JSON 배열 `[{"pageIndex":N,"x":F,"y":F,"width":F,"height":F}, ...]`
    #[wasm_bindgen(js_name = getSelectionRectsInCell)]
    pub fn get_selection_rects_in_cell(
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
        self.get_selection_rects_native(
            section_idx as usize,
            start_cell_para_idx as usize,
            start_char_offset as usize,
            end_cell_para_idx as usize,
            end_char_offset as usize,
            Some((
                parent_para_idx as usize,
                control_idx as usize,
                cell_idx as usize,
            )),
            None,
        )
        .map_err(|e| e.into())
    }


    /// `getSelectionRectsInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, startCellParaIdx,
    /// startCharOffset, endCellParaIdx, endCharOffset, startPageHint?, endPageHint? }`.
    /// page hint가 누락되거나 유효하지 않으면 positional 과 동일한 전체 탐색을 사용한다.
    #[wasm_bindgen(js_name = getSelectionRectsInCellEx)]
    pub fn get_selection_rects_in_cell_ex(&self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.get_selection_rects_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startCellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startCharOffset").unwrap_or(0) as usize,
            json_u32(options_json, "endCellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "endCharOffset").unwrap_or(0) as usize,
            Some((
                json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
                json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
                json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            )),
            json_u32(options_json, "startPageHint").zip(json_u32(options_json, "endPageHint")),
        )
        .map_err(|e| e.into())
    }


    /// 전체 cellPath로 중첩 셀 선택 영역의 줄별 사각형을 반환한다(#4272).
    ///
    /// `path_json`의 마지막 엔트리는 선택 대상 셀을 지정하며, 시작·끝 문단 인덱스는
    /// 별도 인자로 받아 여러 문단 선택도 같은 컨테이너 경로에서 처리한다.
    #[wasm_bindgen(js_name = getSelectionRectsInCellByPath)]
    pub fn get_selection_rects_in_cell_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        start_cell_para_idx: u32,
        start_char_offset: u32,
        end_cell_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_selection_rects_in_cell_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            path_json,
            start_cell_para_idx as usize,
            start_char_offset as usize,
            end_cell_para_idx as usize,
            end_char_offset as usize,
            None,
        )
        .map_err(|e| e.into())
    }


    /// `getSelectionRectsInCellByPath`의 page hint options 변형(#4272).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, path, startCellParaIdx,
    /// startCharOffset, endCellParaIdx, endCharOffset, startPageHint?, endPageHint? }`.
    /// `path`는 cellPath JSON 문자열이다.
    #[wasm_bindgen(js_name = getSelectionRectsInCellByPathEx)]
    pub fn get_selection_rects_in_cell_by_path_ex(
        &self,
        options_json: &str,
    ) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_str, json_u32};
        let path_json = json_str(options_json, "path").unwrap_or_default();
        self.get_selection_rects_in_cell_by_path_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            &path_json,
            json_u32(options_json, "startCellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startCharOffset").unwrap_or(0) as usize,
            json_u32(options_json, "endCellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "endCharOffset").unwrap_or(0) as usize,
            json_u32(options_json, "startPageHint").zip(json_u32(options_json, "endPageHint")),
        )
        .map_err(|e| e.into())
    }


    /// 각주/미주 내부 선택 영역의 줄별 사각형을 반환한다.
    #[wasm_bindgen(js_name = getSelectionRectsInFootnote)]
    pub fn get_selection_rects_in_footnote(
        &self,
        page_num: u32,
        footnote_index: u32,
        start_fn_para_idx: u32,
        start_char_offset: u32,
        end_fn_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.get_selection_rects_in_footnote_native(
            page_num,
            footnote_index as usize,
            start_fn_para_idx as usize,
            start_char_offset as usize,
            end_fn_para_idx as usize,
            end_char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 원본 파일 형식을 반환한다 ("hwp", "hwpx", 또는 "hml").
    #[wasm_bindgen(js_name = getSourceFormat)]
    pub fn get_source_format(&self) -> String {
        source_format_name(self.core.source_format).to_string()
    }


    /// HML 열기 메타데이터와 손실 진단을 JSON으로 반환한다.
    /// 다른 입력 포맷에서는 `null`을 반환한다.
    #[wasm_bindgen(js_name = getHmlOpenMetadata)]
    pub fn get_hml_open_metadata(&self) -> String {
        let Some(metadata) = self.core.hml_metadata() else {
            return "null".to_string();
        };
        let encoding = match metadata.encoding {
            crate::parser::hml::HmlEncoding::Utf8 => "utf-8",
            crate::parser::hml::HmlEncoding::Utf16Le => "utf-16le",
            crate::parser::hml::HmlEncoding::Utf16Be => "utf-16be",
        };
        let warnings = metadata
            .warnings
            .iter()
            .map(hml_warning_json)
            .collect::<Vec<_>>();
        let save_state = hml_save_state(&self.core);
        serde_json::json!({
            "format": "hml",
            "hwpmlVersion": metadata.hwpml_version,
            "encoding": encoding,
            "resourceCount": metadata.resource_count,
            "warnings": warnings,
            "hmlSavable": save_state.hml_savable,
            "saveBlockers": save_state.blockers,
        })
        .to_string()
    }


    /// HML 저장 가능 여부와 모든 차단 진단을 canonical JSON DTO로 반환한다.
    #[wasm_bindgen(js_name = getHmlSaveState)]
    pub fn get_hml_save_state(&self) -> String {
        serde_json::to_string(&hml_save_state(&self.core))
            .expect("HML save-state DTO serialization cannot fail")
    }


    /// HWPX 비표준 감지 경고를 JSON 문자열로 반환한다 (#177).
    ///
    /// ## 반환 형식
    ///
    /// ```json
    /// {
    ///   "count": 3,
    ///   "summary": {
    ///     "lineseg 배열이 비어있음": 1,
    ///     "lineseg 가 미계산 상태 (line_height=0)": 2
    ///   },
    ///   "warnings": [
    ///     {
    ///       "section": 0,
    ///       "paragraph": 5,
    ///       "kind": "LinesegArrayEmpty",
    ///       "cell": null
    ///     },
    ///     {
    ///       "section": 0,
    ///       "paragraph": 10,
    ///       "kind": "LinesegUncomputed",
    ///       "cell": {"ctrl": 0, "row": 0, "col": 1, "innerPara": 0}
    ///     }
    ///   ]
    /// }
    /// ```
    #[wasm_bindgen(js_name = getValidationWarnings)]
    pub fn get_validation_warnings(&self) -> String {
        let report = self.core.validation_report();

        // summary 직렬화 (HashMap 순서 안정화를 위해 키 정렬)
        let mut summary_parts: Vec<String> = Vec::new();
        let mut entries: Vec<(String, usize)> = report.summary().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in &entries {
            // 경고 메시지는 한국어 고정 문자열이므로 `"` / `\` 만 escape.
            let escaped = k.replace('\\', "\\\\").replace('"', "\\\"");
            summary_parts.push(format!("\"{}\":{}", escaped, v));
        }

        // warnings 직렬화
        let mut warning_parts: Vec<String> = Vec::new();
        for w in &report.warnings {
            let cell_part = match &w.cell_path {
                Some(cp) => format!(
                    r#"{{"ctrl":{},"row":{},"col":{},"innerPara":{}}}"#,
                    cp.table_ctrl_idx, cp.row, cp.col, cp.inner_para_idx,
                ),
                None => "null".to_string(),
            };
            let kind_name = match &w.kind {
                crate::document_core::validation::WarningKind::LinesegArrayEmpty => {
                    "LinesegArrayEmpty"
                }
                crate::document_core::validation::WarningKind::LinesegUncomputed => {
                    "LinesegUncomputed"
                }
                crate::document_core::validation::WarningKind::LinesegTextRunReflow => {
                    "LinesegTextRunReflow"
                }
            };
            warning_parts.push(format!(
                r#"{{"section":{},"paragraph":{},"kind":"{}","cell":{}}}"#,
                w.section_idx, w.paragraph_idx, kind_name, cell_part,
            ));
        }

        format!(
            r#"{{"count":{},"summary":{{{}}},"warnings":[{}]}}"#,
            report.len(),
            summary_parts.join(","),
            warning_parts.join(","),
        )
    }


    /// 현재 이벤트 로그를 JSON으로 반환한다.
    #[wasm_bindgen(js_name = getEventLog)]
    pub fn get_event_log(&self) -> String {
        self.serialize_event_log()
    }


    /// 캐럿 위치의 글자 속성을 조회한다.
    ///
    /// 반환값: JSON 객체 (fontFamily, fontSize, bold, italic, underline, strikethrough, textColor 등)
    #[wasm_bindgen(js_name = getCharPropertiesAt)]
    pub fn get_char_properties_at(
        &self,
        sec_idx: usize,
        para_idx: usize,
        char_offset: usize,
    ) -> Result<String, JsValue> {
        self.get_char_properties_at_native(sec_idx, para_idx, char_offset)
            .map_err(|e| e.into())
    }


    /// 셀 내부 문단의 글자 속성을 조회한다.
    #[wasm_bindgen(js_name = getCellCharPropertiesAt)]
    pub fn get_cell_char_properties_at(
        &self,
        sec_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
        cell_idx: usize,
        cell_para_idx: usize,
        char_offset: usize,
    ) -> Result<String, JsValue> {
        self.get_cell_char_properties_at_native(
            sec_idx,
            parent_para_idx,
            control_idx,
            cell_idx,
            cell_para_idx,
            char_offset,
        )
        .map_err(|e| e.into())
    }


    /// 캐럿 위치의 문단 속성을 조회한다.
    ///
    /// 반환값: JSON 객체 (alignment, lineSpacing, marginLeft, marginRight, indent 등)
    #[wasm_bindgen(js_name = getParaPropertiesAt)]
    pub fn get_para_properties_at(
        &self,
        sec_idx: usize,
        para_idx: usize,
    ) -> Result<String, JsValue> {
        self.get_para_properties_at_native(sec_idx, para_idx)
            .map_err(|e| e.into())
    }


    /// 셀 내부 문단의 문단 속성을 조회한다.
    #[wasm_bindgen(js_name = getCellParaPropertiesAt)]
    pub fn get_cell_para_properties_at(
        &self,
        sec_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
        cell_idx: usize,
        cell_para_idx: usize,
    ) -> Result<String, JsValue> {
        self.get_cell_para_properties_at_native(
            sec_idx,
            parent_para_idx,
            control_idx,
            cell_idx,
            cell_para_idx,
        )
        .map_err(|e| e.into())
    }


    /// 문서에 정의된 스타일 목록을 조회한다.
    ///
    /// 반환값: JSON 배열 [{ id, name, englishName, type, paraShapeId, charShapeId }, ...]
    #[wasm_bindgen(js_name = getStyleList)]
    pub fn get_style_list(&self) -> String {
        let styles = &self.core.document.doc_info.styles;
        let mut items = Vec::new();
        for (i, s) in styles.iter().enumerate() {
            items.push(format!(
                "{{\"id\":{},\"name\":\"{}\",\"englishName\":\"{}\",\"type\":{},\"nextStyleId\":{},\"paraShapeId\":{},\"charShapeId\":{}}}",
                i,
                json_escape(&s.local_name),
                json_escape(&s.english_name),
                s.style_type,
                s.next_style_id,
                s.para_shape_id,
                s.char_shape_id
            ));
        }
        format!("[{}]", items.join(","))
    }


    /// 특정 스타일의 CharShape/ParaShape 속성을 상세 조회한다.
    ///
    /// 반환값: JSON { charProps: {...}, paraProps: {...} }
    #[wasm_bindgen(js_name = getStyleDetail)]
    pub fn get_style_detail(&self, style_id: u32) -> String {
        let styles = &self.core.document.doc_info.styles;
        let style = match styles.get(style_id as usize) {
            Some(s) => s,
            None => return "{}".to_string(),
        };
        let char_json = self
            .core
            .build_char_properties_json_by_id(style.char_shape_id);

        // 스타일의 기본 ParaShape에 번호 정보가 없으면,
        // 이 스타일을 사용하는 실제 문단의 ParaShape에서 조회
        let effective_psid =
            self.find_effective_para_shape_for_style(style_id, style.para_shape_id);
        let para_json = self.core.build_para_properties_json(effective_psid, 0);
        format!(
            "{{\"charProps\":{},\"paraProps\":{}}}",
            char_json, para_json
        )
    }


    /// 문서에 정의된 문단 번호(Numbering) 목록을 조회한다.
    ///
    /// 반환값: JSON 배열 [{ id, levelFormats: [...] }, ...]
    /// id는 1-based (ParaShape.numbering_id와 동일)
    #[wasm_bindgen(js_name = getNumberingList)]
    pub fn get_numbering_list(&self) -> String {
        let numberings = &self.core.document.doc_info.numberings;
        let mut items = Vec::new();
        for (i, n) in numberings.iter().enumerate() {
            let formats: Vec<String> = n
                .level_formats
                .iter()
                .map(|f| format!("\"{}\"", json_escape(f)))
                .collect();
            items.push(format!(
                "{{\"id\":{},\"levelFormats\":[{}],\"startNumber\":{}}}",
                i + 1,
                formats.join(","),
                n.start_number
            ));
        }
        format!("[{}]", items.join(","))
    }


    /// 문서에 정의된 글머리표(Bullet) 목록을 조회한다.
    ///
    /// 반환값: JSON 배열 [{ id, char }, ...]
    /// id는 1-based (ParaShape.numbering_id와 동일)
    #[wasm_bindgen(js_name = getBulletList)]
    pub fn get_bullet_list(&self) -> String {
        let bullets = &self.core.document.doc_info.bullets;
        let mut items = Vec::new();
        for (i, b) in bullets.iter().enumerate() {
            let mapped = crate::renderer::layout::map_pua_bullet_char(b.bullet_char);
            let raw_code = b.bullet_char as u32;
            items.push(format!(
                "{{\"id\":{},\"char\":\"{}\",\"rawCode\":{}}}",
                i + 1,
                mapped,
                raw_code
            ));
        }
        format!("[{}]", items.join(","))
    }


    /// 특정 문단의 스타일을 조회한다.
    ///
    /// 반환값: JSON { id, name }
    #[wasm_bindgen(js_name = getStyleAt)]
    pub fn get_style_at(&self, sec_idx: u32, para_idx: u32) -> String {
        let sec = sec_idx as usize;
        let para = para_idx as usize;
        let style_id = self
            .core
            .document
            .sections
            .get(sec)
            .and_then(|s| s.paragraphs.get(para))
            .map(|p| p.style_id as usize)
            .unwrap_or(0);
        let name = self
            .core
            .document
            .doc_info
            .styles
            .get(style_id)
            .map(|s| s.local_name.as_str())
            .unwrap_or("");
        format!("{{\"id\":{},\"name\":\"{}\"}}", style_id, json_escape(name))
    }


    /// 셀 내부 문단의 스타일을 조회한다.
    #[wasm_bindgen(js_name = getCellStyleAt)]
    pub fn get_cell_style_at(
        &self,
        sec_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
    ) -> String {
        let style_id = self
            .core
            .get_cell_paragraph_ref(
                sec_idx as usize,
                parent_para_idx as usize,
                control_idx as usize,
                cell_idx as usize,
                cell_para_idx as usize,
            )
            .map(|p| p.style_id as usize)
            .unwrap_or(0);
        let name = self
            .core
            .document
            .doc_info
            .styles
            .get(style_id)
            .map(|s| s.local_name.as_str())
            .unwrap_or("");
        format!("{{\"id\":{},\"name\":\"{}\"}}", style_id, json_escape(name))
    }


    #[wasm_bindgen(js_name = getCellCharPropertiesAtByPath)]
    pub fn get_cell_char_properties_at_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.get_cell_char_properties_at_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 감추기 조회
    #[wasm_bindgen(js_name = getPageHide)]
    pub fn get_page_hide(&self, sec: u32, para: u32) -> Result<String, JsValue> {
        self.get_page_hide_native(sec as usize, para as usize)
            .map_err(|e| e.into())
    }


    /// 내부 클립보드의 플레인 텍스트를 반환한다.
    #[wasm_bindgen(js_name = getClipboardText)]
    pub fn get_clipboard_text(&self) -> String {
        self.get_clipboard_text_native()
    }


    /// 컨트롤의 이미지 바이너리 데이터를 반환한다 (Uint8Array).
    #[wasm_bindgen(js_name = getControlImageData)]
    pub fn get_control_image_data(
        &self,
        section_idx: u32,
        para_idx: u32,
        cell_path_json: &str,
        control_idx: u32,
    ) -> Result<Vec<u8>, JsValue> {
        let cell_path = parse_cell_path_arg(cell_path_json)?;
        self.get_control_image_data_native(
            section_idx as usize,
            para_idx as usize,
            &cell_path,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 컨트롤의 이미지 MIME 타입을 반환한다.
    #[wasm_bindgen(js_name = getControlImageMime)]
    pub fn get_control_image_mime(
        &self,
        section_idx: u32,
        para_idx: u32,
        cell_path_json: &str,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        let cell_path = parse_cell_path_arg(cell_path_json)?;
        self.get_control_image_mime_native(
            section_idx as usize,
            para_idx as usize,
            &cell_path,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 문서 내 모든 책갈피 목록 반환
    #[wasm_bindgen(js_name = getBookmarks)]
    pub fn get_bookmarks(&self) -> Result<String, JsValue> {
        self.core.get_bookmarks_native().map_err(|e| e.into())
    }


    /// 문서 구조(개요/조문) 트리를 JSON으로 반환 (사이드바 목차 네비게이션용)
    ///
    /// `mode`: `"auto"` | `"outline"` | `"clause"` (인식 불가 시 `auto`).
    #[wasm_bindgen(js_name = getStructure)]
    pub fn get_structure(&self, mode: &str) -> Result<String, JsValue> {
        self.core.get_structure_native(mode).map_err(|e| e.into())
    }


    /// 문단 모양의 개요 번호만 탐색 정보로 반환한다.
    ///
    /// 일반 문단의 `1.` 같은 텍스트는 분석하지 않는다.
    #[wasm_bindgen(js_name = getOutlineNavigation)]
    pub fn get_outline_navigation(&self) -> Result<String, JsValue> {
        self.core
            .get_outline_navigation_native()
            .map_err(|e| e.into())
    }

}
