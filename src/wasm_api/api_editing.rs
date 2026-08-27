//! api_editing — table_layout.rs 에서 무변동 이동
use super::*;

#[wasm_bindgen]
impl HwpDocument {
    /// 빈 문서 생성 (테스트/미리보기용)
    ///
    /// 기본 A4 구역 1개 + 빈 문단 1개를 포함한다. 구역 0개 문서는 모든
    /// 편집/조회 API가 "구역 인덱스 0 범위 초과"로 실패해 사용 불가하므로
    /// 생성 직후 바로 편집 가능한 최소 구조를 보장한다 (#1386).
    ///
    /// 여기서 만든 문단은 **구역 정의·단 정의를 안 진다** — 실제 HWP 문서는 예외 없이 그
    /// 둘을 첫 문단에 지므로 이 문서는 그 점에서 실물과 다르다. 한글 호환이 필요한 자리
    /// (`Clear`)는 번들 템플릿을 쓰는 [`create_blank_document`](Self::create_blank_document)
    /// 를 쓴다. 여기에 그 둘을 넣으면 `char_shapes` 자리가 16칸씩 밀려 기존 호출부가 깨진다.
    #[wasm_bindgen(js_name = createEmpty)]
    pub fn create_empty() -> HwpDocument {
        let mut core = DocumentCore::new_empty();
        let mut section = Section::default();
        // set_document가 styles/composed 재구성 + paginate까지 수행한다.
        section.section_def.page_def = crate::model::page::PageDef::a4_default();
        section.paragraphs.push(Paragraph::new_empty());
        let mut document = Document::default();
        document.sections.push(section);
        core.set_document(document);
        HwpDocument { core }
    }


    /// 내장 템플릿에서 빈 문서를 생성한다.
    ///
    /// saved/blank2010.hwp를 WASM 바이너리에 포함하여 유효한 HWP 문서를 즉시 생성.
    /// DocInfo raw_stream이 온전하므로 FIX-4 워크어라운드와 호환됨.
    #[wasm_bindgen(js_name = createBlankDocument)]
    pub fn create_blank_document(&mut self) -> Result<String, JsValue> {
        self.create_blank_document_native().map_err(|e| e.into())
    }


    /// 문단부호(¶) 표시 여부를 설정한다.
    #[wasm_bindgen(js_name = setShowParagraphMarks)]
    pub fn set_show_paragraph_marks(&mut self, enabled: bool) {
        self.show_paragraph_marks = enabled;
        self.invalidate_page_tree_cache();
    }


    /// 조판부호 표시 여부를 설정한다 (개체 마커 + 문단부호 포함).
    #[wasm_bindgen(js_name = setShowControlCodes)]
    pub fn set_show_control_codes(&mut self, enabled: bool) {
        self.show_control_codes = enabled;
        self.invalidate_page_tree_cache();
    }


    /// 투명선 표시 여부를 설정한다.
    #[wasm_bindgen(js_name = setShowTransparentBorders)]
    pub fn set_show_transparent_borders(&mut self, enabled: bool) {
        self.show_transparent_borders = enabled;
        self.invalidate_page_tree_cache();
    }


    #[wasm_bindgen(js_name = setClipEnabled)]
    pub fn set_clip_enabled(&mut self, enabled: bool) {
        self.clip_enabled = enabled;
        self.invalidate_page_tree_cache();
    }


    /// 디버그 오버레이 표시 여부를 설정한다.
    pub fn set_debug_overlay(&mut self, enabled: bool) {
        self.debug_overlay = enabled;
    }


    /// LINE_SEG vpos-reset 강제 분리 적용 여부를 설정한다.
    /// 변경 시 페이지네이션 결과가 달라지므로 모든 섹션을 재페이지네이션한다.
    pub fn set_respect_vpos_reset(&mut self, enabled: bool) {
        if self.respect_vpos_reset != enabled {
            self.respect_vpos_reset = enabled;
            // 모든 섹션 dirty 마킹 후 즉시 재페이지네이션
            for d in self.core.dirty_sections.iter_mut() {
                *d = true;
            }
            self.invalidate_page_tree_cache();
            self.core.paginate();
        }
    }


    /// 구역의 용지 설정(PageDef)을 변경하고 재페이지네이션한다.
    #[wasm_bindgen(js_name = setPageDef)]
    pub fn set_page_def(&mut self, section_idx: u32, json: &str) -> Result<String, JsValue> {
        self.set_page_def_native(section_idx as usize, json)
            .map_err(|e| e.into())
    }


    /// 구역 정의(SectionDef)를 변경하고 재페이지네이션한다.
    #[wasm_bindgen(js_name = setSectionDef)]
    pub fn set_section_def(&mut self, section_idx: u32, json: &str) -> Result<String, JsValue> {
        self.set_section_def_native(section_idx as usize, json)
            .map_err(|e| e.into())
    }


    /// 모든 구역의 SectionDef를 일괄 변경하고 재페이지네이션한다.
    #[wasm_bindgen(js_name = setSectionDefAll)]
    pub fn set_section_def_all(&mut self, json: &str) -> Result<String, JsValue> {
        self.set_section_def_all_native(json).map_err(|e| e.into())
    }


    /// 구역의 쪽 테두리/배경 설정을 변경하고 재페이지네이션한다.
    #[wasm_bindgen(js_name = setPageBorderFill)]
    pub fn set_page_border_fill(
        &mut self,
        section_idx: u32,
        json: &str,
    ) -> Result<String, JsValue> {
        self.set_page_border_fill_native(section_idx as usize, json)
            .map_err(|e| e.into())
    }


    /// DPI를 설정한다.
    #[wasm_bindgen(js_name = setDpi)]
    pub fn set_dpi(&mut self, dpi: f64) {
        self.core.set_dpi(dpi);
    }


    /// 파일 이름을 설정한다 (머리말/꼬리말 필드 치환용).
    #[wasm_bindgen(js_name = setFileName)]
    pub fn set_file_name(&mut self, name: &str) {
        if self.core.file_name != name {
            self.core.file_name = name.to_string();
            self.core.invalidate_page_tree_cache();
        }
    }


    /// 대체 폰트 경로를 설정한다.
    #[wasm_bindgen(js_name = setFallbackFont)]
    pub fn set_fallback_font(&mut self, path: &str) {
        self.fallback_font = path.to_string();
    }


    /// 문단에 텍스트를 삽입한다.
    ///
    /// 삽입 후 구역을 재구성하고 재페이지네이션한다.
    /// 반환값: JSON `{"ok":true,"charOffset":<new_offset>}`
    #[wasm_bindgen(js_name = insertText)]
    pub fn insert_text(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.insert_text_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            text,
        )
        .map_err(|e| e.into())
    }


    /// 논리적 오프셋으로 텍스트를 삽입한다.
    ///
    /// logical_offset: 텍스트 문자 + 인라인 컨트롤을 각각 1로 세는 위치.
    /// 예: "abc[표]XYZ" → a(0) b(1) c(2) [표](3) X(4) Y(5) Z(6)
    /// logical_offset=4이면 표 뒤의 X 앞에 삽입.
    /// 반환값: JSON `{"ok":true,"logicalOffset":<new_logical_offset>}`
    #[wasm_bindgen(js_name = insertTextLogical)]
    pub fn insert_text_logical(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        logical_offset: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        let sec = section_idx as usize;
        let pi = para_idx as usize;
        if sec >= self.document.sections.len() || pi >= self.document.sections[sec].paragraphs.len()
        {
            return Err(JsValue::from_str("인덱스 범위 초과"));
        }
        let (text_offset, _) = crate::document_core::helpers::logical_to_text_offset(
            &self.document.sections[sec].paragraphs[pi],
            logical_offset as usize,
        );
        let result = self.insert_text_native(sec, pi, text_offset, text)?;
        // 삽입 후 논리적 오프셋 반환
        let new_text_offset = text_offset + text.chars().count();
        let new_logical = crate::document_core::helpers::text_to_logical_offset(
            &self.document.sections[sec].paragraphs[pi],
            new_text_offset,
        );
        Ok(format!("{{\"ok\":true,\"logicalOffset\":{}}}", new_logical))
    }


    /// 문단에서 텍스트를 삭제한다.
    ///
    /// 삭제 후 구역을 재구성하고 재페이지네이션한다.
    /// 반환값: JSON `{"ok":true,"charOffset":<offset_after_delete>}`
    #[wasm_bindgen(js_name = deleteText)]
    pub fn delete_text(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        self.delete_text_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            count as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내부 문단에 텍스트를 삽입한다.
    ///
    /// 반환값: JSON `{"ok":true,"charOffset":<new_offset>}`
    #[wasm_bindgen(js_name = insertTextInCell)]
    pub fn insert_text_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.insert_text_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            text,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내부 문단에 텍스트를 삽입하되 전체 페이지네이션은 호출자가 지연한다.
    ///
    /// Studio의 page-local 단일 입력처럼 현재 페이지를 먼저 갱신하고 idle 시점에
    /// 전체 페이지네이션을 한 번만 수행하는 경로에서 사용한다.
    /// 결과 JSON은 `charOffset`과 상대 cell-flow 변화 신호 `cellFlowChanged`를 포함한다.
    #[wasm_bindgen(js_name = insertTextInCellDeferredPagination)]
    pub fn insert_text_in_cell_deferred_pagination(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.insert_text_in_cell_native_deferred_pagination(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            text,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내부 문단에서 텍스트를 삭제하되 전체 페이지네이션은 호출자가 지연한다.
    ///
    /// 결과 JSON은 `charOffset`과 상대 cell-flow 변화 신호 `cellFlowChanged`를 포함한다.
    #[wasm_bindgen(js_name = deleteTextInCellDeferredPagination)]
    pub fn delete_text_in_cell_deferred_pagination(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        self.delete_text_in_cell_native_deferred_pagination(
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


    /// 대형 표 continuation shadow job을 시작한다. 공개 페이지는 완료 전까지 유지된다.
    #[wasm_bindgen(js_name = beginDeferredPagination)]
    pub fn begin_deferred_pagination(&mut self, fragment_budget: u32) -> Result<String, JsValue> {
        Ok(deferred_pagination_result_json(
            self.core
                .begin_deferred_pagination((fragment_budget as usize).max(1)),
        ))
    }


    /// `insertTextInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, text: string }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = insertTextInCellEx)]
    pub fn insert_text_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_str, json_u32};
        self.insert_text_in_cell_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            &json_str(options_json, "text").unwrap_or_default(),
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내부 문단에서 텍스트를 삭제한다.
    ///
    /// 반환값: JSON `{"ok":true,"charOffset":<offset_after_delete>}`
    #[wasm_bindgen(js_name = deleteTextInCell)]
    pub fn delete_text_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        self.delete_text_in_cell_native(
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


    /// `deleteTextInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, count }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = deleteTextInCellEx)]
    pub fn delete_text_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.delete_text_in_cell_native(
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


    #[wasm_bindgen(js_name = insertTextInCellByPath)]
    pub fn insert_text_in_cell_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.insert_text_in_cell_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
            text,
        )
        .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = deleteTextInCellByPath)]
    pub fn delete_text_in_cell_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.delete_text_in_cell_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
            count as usize,
        )
        .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = deleteRangeInCellByPath)]
    #[allow(clippy::too_many_arguments)]
    pub fn delete_range_in_cell_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        start_para: u32,
        start_offset: u32,
        end_para: u32,
        end_offset: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.delete_range_in_cell_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            start_para as usize,
            start_offset as usize,
            end_para as usize,
            end_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 생성 (빈 문단 1개 포함)
    ///
    /// 반환: JSON `{"ok":true,"kind":"header/footer","applyTo":N,...}`
    #[wasm_bindgen(js_name = createHeaderFooter)]
    pub fn create_header_footer(
        &mut self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
    ) -> Result<String, JsValue> {
        self.create_header_footer_native(section_idx as usize, is_header, apply_to)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 내 텍스트 삽입
    ///
    /// 반환: JSON `{"ok":true,"charOffset":<new_offset>}`
    #[wasm_bindgen(js_name = insertTextInHeaderFooter)]
    pub fn insert_text_in_header_footer(
        &mut self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: u32,
        char_offset: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.insert_text_in_header_footer_native(
            section_idx as usize,
            is_header,
            apply_to,
            hf_para_idx as usize,
            char_offset as usize,
            text,
        )
        .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 내 텍스트 삭제
    ///
    /// 반환: JSON `{"ok":true,"charOffset":<offset>}`
    #[wasm_bindgen(js_name = deleteTextInHeaderFooter)]
    pub fn delete_text_in_header_footer(
        &mut self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: u32,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        self.delete_text_in_header_footer_native(
            section_idx as usize,
            is_header,
            apply_to,
            hf_para_idx as usize,
            char_offset as usize,
            count as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표에 행을 삽입한다.
    ///
    /// 반환값: JSON `{"ok":true,"rowCount":<N>,"colCount":<M>}`
    #[wasm_bindgen(js_name = insertTableRow)]
    pub fn insert_table_row(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        row_idx: u32,
        below: bool,
    ) -> Result<String, JsValue> {
        self.insert_table_row_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            row_idx as u16,
            below,
        )
        .map_err(|e| e.into())
    }


    /// 표에 열을 삽입한다.
    ///
    /// 반환값: JSON `{"ok":true,"rowCount":<N>,"colCount":<M>}`
    #[wasm_bindgen(js_name = insertTableColumn)]
    pub fn insert_table_column(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        col_idx: u32,
        right: bool,
    ) -> Result<String, JsValue> {
        self.insert_table_column_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            col_idx as u16,
            right,
        )
        .map_err(|e| e.into())
    }


    /// 표에서 행을 삭제한다.
    ///
    /// 반환값: JSON `{"ok":true,"rowCount":<N>,"colCount":<M>}`
    #[wasm_bindgen(js_name = deleteTableRow)]
    pub fn delete_table_row(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        row_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_table_row_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            row_idx as u16,
        )
        .map_err(|e| e.into())
    }


    /// 표에서 열을 삭제한다.
    ///
    /// 반환값: JSON `{"ok":true,"rowCount":<N>,"colCount":<M>}`
    #[wasm_bindgen(js_name = deleteTableColumn)]
    pub fn delete_table_column(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        col_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_table_column_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            col_idx as u16,
        )
        .map_err(|e| e.into())
    }


    /// 강제 쪽 나누기 삽입 (Ctrl+Enter)
    #[wasm_bindgen(js_name = insertPageBreak)]
    pub fn insert_page_break(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.insert_page_break_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 단 나누기 삽입 (Ctrl+Shift+Enter)
    #[wasm_bindgen(js_name = insertColumnBreak)]
    pub fn insert_column_break(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.insert_column_break_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 새 번호 지정 컨트롤 삽입 (쪽 > 새 번호로 시작)
    #[wasm_bindgen(js_name = insertNewNumber)]
    pub fn insert_new_number(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        start_num: u32,
    ) -> Result<String, JsValue> {
        if start_num == 0 || start_num > 65535 {
            return Err(JsValue::from_str("start_num must be 1~65535"));
        }
        self.insert_new_number_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            start_num as u16,
        )
        .map_err(|e| e.into())
    }


    /// 다단 설정 변경
    /// column_type: 0=일반, 1=배분, 2=평행
    /// same_width: 0=다른 너비, 1=같은 너비
    #[wasm_bindgen(js_name = setColumnDef)]
    pub fn set_column_def(
        &mut self,
        section_idx: u32,
        column_count: u32,
        column_type: u32,
        same_width: u32,
        spacing_hu: i32,
    ) -> Result<String, JsValue> {
        self.set_column_def_native(
            section_idx as usize,
            column_count as u16,
            column_type as u8,
            same_width != 0,
            spacing_hu as i16,
        )
        .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = deleteParagraph)]
    pub fn delete_paragraph(&mut self, section_idx: u32, para_idx: u32) -> Result<String, JsValue> {
        self.delete_paragraph_native(section_idx as usize, para_idx as usize)
            .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = insertParagraph)]
    pub fn insert_paragraph(&mut self, section_idx: u32, para_idx: u32) -> Result<String, JsValue> {
        self.insert_paragraph_native(section_idx as usize, para_idx as usize)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 문단에 문단 서식을 적용한다.
    #[wasm_bindgen(js_name = applyParaFormatInHf)]
    pub fn apply_para_format_in_hf(
        &mut self,
        section_idx: usize,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: usize,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_para_format_in_hf_native(
            section_idx,
            is_header,
            apply_to,
            hf_para_idx,
            props_json,
        )
        .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 문단에 필드 마커를 삽입한다.
    #[wasm_bindgen(js_name = insertFieldInHf)]
    pub fn insert_field_in_hf(
        &mut self,
        section_idx: usize,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: usize,
        char_offset: usize,
        field_type: u8,
    ) -> Result<String, JsValue> {
        self.insert_field_in_hf_native(
            section_idx,
            is_header,
            apply_to,
            hf_para_idx,
            char_offset,
            field_type,
        )
        .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 마당(템플릿)을 적용한다.
    #[wasm_bindgen(js_name = applyHfTemplate)]
    pub fn apply_hf_template(
        &mut self,
        section_idx: usize,
        is_header: bool,
        apply_to: u8,
        template_id: u8,
    ) -> Result<String, JsValue> {
        self.apply_hf_template_native(section_idx, is_header, apply_to, template_id)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말을 삭제한다 (컨트롤 자체 제거).
    #[wasm_bindgen(js_name = deleteHeaderFooter)]
    pub fn delete_header_footer(
        &mut self,
        section_idx: u32,
        is_header: bool,
        apply_to: u32,
    ) -> Result<String, JsValue> {
        self.delete_header_footer_native(section_idx as usize, is_header, apply_to as u8)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 감추기를 토글한다 (현재 쪽만).
    ///
    /// 반환: JSON `{"hidden":true/false}` — 토글 후 상태
    #[wasm_bindgen(js_name = toggleHideHeaderFooter)]
    pub fn toggle_hide_header_footer(
        &mut self,
        page_index: u32,
        is_header: bool,
    ) -> Result<String, JsValue> {
        self.toggle_hide_header_footer_native(page_index, is_header)
            .map_err(|e| e.into())
    }


    /// [#4180] 저장 직전 UI 캐럿을 문서 캐럿 메타데이터에 반영한다
    /// (한컴 의미론: 저장 시점 캐럿). 범위 밖 위치는 무시 — 저장을 막지 않는다.
    #[wasm_bindgen(js_name = setCaretPosition)]
    pub fn set_caret_position(&mut self, section_idx: u32, para_idx: u32, char_offset: u32) {
        self.set_caret_position_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        );
    }


    /// 셀 속성을 수정한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = setCellProperties)]
    pub fn set_cell_properties(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        json: &str,
    ) -> Result<String, JsValue> {
        self.set_cell_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            json,
        )
        .map_err(|e| e.into())
    }


    /// 선택 영역을 하나의 셀처럼 취급하는 cellzone 테두리/배경 속성을 적용한다.
    ///
    /// 반환: JSON `{"ok":true,"startRow":...,"borderFillId":...}`
    #[wasm_bindgen(js_name = setCellZoneProperties)]
    pub fn set_cell_zone_properties(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
        json: &str,
    ) -> Result<String, JsValue> {
        self.set_cell_zone_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            start_row as u16,
            start_col as u16,
            end_row as u16,
            end_col as u16,
            json,
        )
        .map_err(|e| e.into())
    }


    /// 표의 위치 오프셋(vertical_offset, horizontal_offset)을 이동한다.
    ///
    /// delta_h, delta_v: HWPUNIT 단위 이동량 (양수=오른쪽/아래, 음수=왼쪽/위)
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = moveTableOffset)]
    pub fn move_table_offset(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        delta_h: i32,
        delta_v: i32,
    ) -> Result<String, JsValue> {
        self.move_table_offset_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            delta_h,
            delta_v,
        )
        .map_err(|e| e.into())
    }


    /// 표 속성을 수정한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = setTableProperties)]
    pub fn set_table_properties(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        json: &str,
    ) -> Result<String, JsValue> {
        self.set_table_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            json,
        )
        .map_err(|e| e.into())
    }


    /// 표 컨트롤을 문단에서 삭제한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = deleteTableControl)]
    pub fn delete_table_control(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_table_control_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 커서 위치에 새 표를 삽입한다.
    ///
    /// 반환: JSON `{"ok":true,"paraIdx":<N>,"controlIdx":0}`
    #[wasm_bindgen(js_name = createTable)]
    pub fn create_table(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        row_count: u32,
        col_count: u32,
    ) -> Result<String, JsValue> {
        self.create_table_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            row_count as u16,
            col_count as u16,
        )
        .map_err(|e| e.into())
    }


    /// 커서 위치에 표를 삽입한다 (확장, JSON 옵션).
    ///
    /// options JSON: { sectionIdx, paraIdx, charOffset, rowCount, colCount,
    ///                 treatAsChar?: bool, colWidths?: [u32, ...] }
    #[wasm_bindgen(js_name = createTableEx)]
    pub fn create_table_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_u32};
        let section_idx = json_u32(options_json, "sectionIdx").unwrap_or(0) as usize;
        let para_idx = json_u32(options_json, "paraIdx").unwrap_or(0) as usize;
        let char_offset = json_u32(options_json, "charOffset").unwrap_or(0) as usize;
        let row_count = json_u32(options_json, "rowCount").unwrap_or(2) as u16;
        let col_count = json_u32(options_json, "colCount").unwrap_or(2) as u16;
        let treat_as_char = json_bool(options_json, "treatAsChar").unwrap_or(false);
        fn parse_u32_array(json: &str, key: &str) -> Option<Vec<u32>> {
            if let Some(start) = json.find(&format!("\"{}\"", key)) {
                let rest = &json[start..];
                if let Some(arr_start) = rest.find('[') {
                    if let Some(arr_end) = rest[arr_start..].find(']') {
                        let arr_str = &rest[arr_start + 1..arr_start + arr_end];
                        let nums: Vec<u32> = arr_str
                            .split(',')
                            .filter_map(|s| s.trim().parse::<u32>().ok())
                            .collect();
                        if !nums.is_empty() {
                            Some(nums)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        let col_widths = parse_u32_array(options_json, "colWidths");
        let row_heights = parse_u32_array(options_json, "rowHeights");

        self.create_table_ex_native(
            section_idx,
            para_idx,
            char_offset,
            row_count,
            col_count,
            treat_as_char,
            col_widths.as_deref(),
            row_heights.as_deref(),
        )
        .map_err(|e| e.into())
    }


    /// 커서 위치에 그림을 삽입한다.
    ///
    /// image_data: 이미지 바이너리 데이터 (PNG/JPG/GIF/BMP 등)
    /// width, height: HWPUNIT 단위 크기
    /// extension: 파일 확장자 (jpg, png 등)
    ///
    /// 반환:
    /// - 본문 inline: `{"ok":true,"paraIdx":<N>,"controlIdx":0}`
    /// - 셀 floating (#1151): `{"ok":true,"paraIdx":<table_para>,"controlIdx":<new_sibling_idx>}`
    ///
    /// `cell_path_json` 이 빈 문자열 또는 `"[]"` 면 본문 inline 삽입. 그 외에는
    /// 표 셀 영역에 floating picture (한컴 정합) 로 삽입한다.
    /// 예: `[{"controlIndex":0,"cellIndex":2,"cellParaIndex":0}]`
    /// [Task #1151 v8 결함 C] `paper_offset_x_hu / paper_offset_y_hu` 는 사용자가 셀 안에
    /// 클릭/드래그한 위치 (paper-relative HU). studio 의 finishImagePlacement 가 drag 좌표를
    /// 변환하여 전달. JS 측에서 `undefined` 전달 시 (또는 음수) wasm 이 셀 좌상단을 default 사용
    /// — 기존 동작 호환.
    #[wasm_bindgen(js_name = insertPicture)]
    #[allow(clippy::too_many_arguments)]
    pub fn insert_picture(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        cell_path_json: &str,
        image_data: &[u8],
        width: u32,
        height: u32,
        natural_width_px: u32,
        natural_height_px: u32,
        extension: &str,
        description: &str,
        paper_offset_x_hu: Option<i32>,
        paper_offset_y_hu: Option<i32>,
    ) -> Result<String, JsValue> {
        let cell_path: Vec<(usize, usize, usize)> =
            if cell_path_json.is_empty() || cell_path_json == "[]" {
                Vec::new()
            } else {
                DocumentCore::parse_cell_path(cell_path_json).map_err(JsValue::from)?
            };
        self.insert_picture_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            &cell_path,
            image_data,
            width,
            height,
            natural_width_px,
            natural_height_px,
            extension,
            description,
            paper_offset_x_hu,
            paper_offset_y_hu,
        )
        .map_err(|e| e.into())
    }


    /// 커서 위치에 그림을 삽입한다 (확장, options object — #1413).
    ///
    /// positional `insertPicture` 와 동일 동작의 얇은 어댑터. 이미지 바이너리는 별도
    /// `image_data` 인자(Uint8Array)로 받고, 나머지는 JSON options 로 받는다. 필드 추가/
    /// 순서 변경 시 호출부 영향이 작다.
    ///
    /// options JSON 키 (positional 과 동일 의미, camelCase):
    /// `{ sectionIdx, paraIdx, charOffset?, cellPath?: string, width, height,
    ///    naturalWidthPx, naturalHeightPx, extension?, description?,
    ///    paperOffsetXHu?: number|null, paperOffsetYHu?: number|null }`
    /// - `cellPath` 는 cell_path_json 문자열(빈 문자열/`"[]"` 이면 본문 inline).
    /// - 반환값은 `insertPicture` 와 동일.
    #[wasm_bindgen(js_name = insertPictureEx)]
    pub fn insert_picture_ex(
        &mut self,
        options_json: &str,
        image_data: &[u8],
    ) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_i32, json_str, json_u32};
        let section_idx = json_u32(options_json, "sectionIdx").unwrap_or(0);
        let para_idx = json_u32(options_json, "paraIdx").unwrap_or(0);
        let char_offset = json_u32(options_json, "charOffset").unwrap_or(0);
        let cell_path_json = json_str(options_json, "cellPath").unwrap_or_default();
        let width = json_u32(options_json, "width").unwrap_or(0);
        let height = json_u32(options_json, "height").unwrap_or(0);
        let natural_width_px = json_u32(options_json, "naturalWidthPx").unwrap_or(0);
        let natural_height_px = json_u32(options_json, "naturalHeightPx").unwrap_or(0);
        let extension = json_str(options_json, "extension").unwrap_or_default();
        let description = json_str(options_json, "description").unwrap_or_default();
        // paperOffset 은 키 부재 시 None(셀 좌상단 default) — positional 의 Option 동작과 동일.
        let paper_offset_x_hu = json_i32(options_json, "paperOffsetXHu");
        let paper_offset_y_hu = json_i32(options_json, "paperOffsetYHu");

        let cell_path: Vec<(usize, usize, usize)> =
            if cell_path_json.is_empty() || cell_path_json == "[]" {
                Vec::new()
            } else {
                DocumentCore::parse_cell_path(&cell_path_json).map_err(JsValue::from)?
            };
        self.insert_picture_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            &cell_path,
            image_data,
            width,
            height,
            natural_width_px,
            natural_height_px,
            &extension,
            &description,
            paper_offset_x_hu,
            paper_offset_y_hu,
        )
        .map_err(|e| e.into())
    }


    /// 그림 컨트롤의 속성을 변경한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = setPictureProperties)]
    pub fn set_picture_properties(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.set_picture_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }


    /// [Task #825] 머리말/꼬리말 안 그림 속성 변경.
    #[wasm_bindgen(js_name = setHeaderFooterPictureProperties)]
    pub fn set_header_footer_picture_properties(
        &mut self,
        section_idx: u32,
        outer_para_idx: u32,
        outer_control_idx: u32,
        inner_para_idx: u32,
        inner_control_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.set_header_footer_picture_properties_native(
            section_idx as usize,
            outer_para_idx as usize,
            outer_control_idx as usize,
            inner_para_idx as usize,
            inner_control_idx as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }


    /// 그림 컨트롤을 문단에서 삭제한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = deletePictureControl)]
    pub fn delete_picture_control(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_picture_control_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// [Task #1171 / PR #1254] 표 셀/글상자 내부 Picture 삭제 (by_path).
    #[wasm_bindgen(js_name = deleteCellPictureControlByPath)]
    pub fn delete_cell_picture_control_by_path(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        cell_path_json: &str,
        inner_control_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_cell_picture_control_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            cell_path_json,
            inner_control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// [Task #1138] 표 셀 내 Shape 속성 변경 (by_path).
    #[wasm_bindgen(js_name = setCellShapePropertiesByPath)]
    pub fn set_cell_shape_properties_by_path(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        cell_path_json: &str,
        inner_control_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.set_cell_shape_properties_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            cell_path_json,
            inner_control_idx as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }


    /// [Task #1151 v4] 표 셀 내 Picture 속성 변경 (by_path). Shape 패턴 정합.
    #[wasm_bindgen(js_name = setCellPicturePropertiesByPath)]
    pub fn set_cell_picture_properties_by_path(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        cell_path_json: &str,
        inner_control_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.set_cell_picture_properties_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            cell_path_json,
            inner_control_idx as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }


    /// 수식 컨트롤을 문단에서 삭제한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = deleteEquationControl)]
    pub fn delete_equation_control(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_equation_control_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 수식 컨트롤의 속성을 변경한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = setEquationProperties)]
    pub fn set_equation_properties(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: i32,
        cell_para_idx: i32,
        props_json: &str,
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
        self.set_equation_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            ci,
            cpi,
            props_json,
        )
        .map_err(|e| e.into())
    }


    /// 각주/미주 내부 수식 컨트롤의 속성을 변경한다.
    #[wasm_bindgen(js_name = setNoteEquationProperties)]
    pub fn set_note_equation_properties(
        &mut self,
        kind: &str,
        section_idx: u32,
        parent_para_idx: u32,
        note_control_idx: u32,
        note_para_idx: u32,
        inner_control_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.set_note_equation_properties_native(
            kind,
            section_idx as usize,
            parent_para_idx as usize,
            note_control_idx as usize,
            note_para_idx as usize,
            inner_control_idx as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }


    /// `setNoteEquationProperties` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ kind, sectionIdx, parentParaIdx, noteControlIdx, noteParaIdx,
    /// innerControlIdx, props: object }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = setNoteEquationPropertiesEx)]
    pub fn set_note_equation_properties_ex(
        &mut self,
        options_json: &str,
    ) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_object, json_str, json_u32};
        let props_json = json_object(options_json, "props").unwrap_or_else(|| "{}".to_string());
        self.set_note_equation_properties_native(
            &json_str(options_json, "kind").unwrap_or_default(),
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "noteControlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "noteParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "innerControlIdx").unwrap_or(0) as usize,
            &props_json,
        )
        .map_err(|e| e.into())
    }

}
