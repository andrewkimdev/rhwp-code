//! api_clipboard — table_layout.rs 에서 무변동 이동
use super::*;

#[wasm_bindgen]
impl HwpDocument {
    /// 셀 내부 문단을 분할한다 (셀 내 Enter 키).
    ///
    /// 반환값: JSON `{"ok":true,"cellParaIndex":<new_idx>,"charOffset":0}`
    ///
    /// `removed_para_meta` 는 병합 undo 가 되돌려주는 값이다 — 본문 `splitParagraph`
    /// 와 같은 규약이다 (Task #2342).
    #[wasm_bindgen(js_name = splitParagraphInCell)]
    pub fn split_paragraph_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        removed_para_meta: Option<String>,
    ) -> Result<String, JsValue> {
        self.split_paragraph_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            parse_removed_para_meta(removed_para_meta)?,
        )
        .map_err(|e| e.into())
    }


    /// 셀 내부 문단을 이전 문단에 병합한다 (셀 내 Backspace at start).
    ///
    /// 반환값: JSON `{"ok":true,"cellParaIndex":<prev_idx>,"charOffset":<merge_point>}`
    #[wasm_bindgen(js_name = mergeParagraphInCell)]
    pub fn merge_paragraph_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
    ) -> Result<String, JsValue> {
        self.merge_paragraph_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
        )
        .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = splitParagraphInCellByPath)]
    pub fn split_paragraph_in_cell_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        removed_para_meta: Option<String>,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.split_paragraph_in_cell_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
            parse_removed_para_meta(removed_para_meta)?,
        )
        .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = mergeParagraphInCellByPath)]
    pub fn merge_paragraph_in_cell_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.merge_paragraph_in_cell_by_path(section_idx as usize, parent_para_idx as usize, &path)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 내 문단 분할 (Enter 키)
    ///
    /// 반환: JSON `{"ok":true,"hfParaIndex":<new_idx>,"charOffset":0}`
    #[wasm_bindgen(js_name = splitParagraphInHeaderFooter)]
    pub fn split_paragraph_in_header_footer(
        &mut self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: u32,
        char_offset: u32,
        removed_para_meta: Option<String>,
    ) -> Result<String, JsValue> {
        self.split_paragraph_in_header_footer_native(
            section_idx as usize,
            is_header,
            apply_to,
            hf_para_idx as usize,
            char_offset as usize,
            parse_removed_para_meta(removed_para_meta)?,
        )
        .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 내 문단 병합 (Backspace at start)
    ///
    /// 반환: JSON `{"ok":true,"hfParaIndex":<prev_idx>,"charOffset":<merge_point>}`
    #[wasm_bindgen(js_name = mergeParagraphInHeaderFooter)]
    pub fn merge_paragraph_in_header_footer(
        &mut self,
        section_idx: u32,
        is_header: bool,
        apply_to: u8,
        hf_para_idx: u32,
    ) -> Result<String, JsValue> {
        self.merge_paragraph_in_header_footer_native(
            section_idx as usize,
            is_header,
            apply_to,
            hf_para_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표를 지정 행에서 두 개로 나눈다 (한컴 [표-표 나누기]).
    ///
    /// 반환값: JSON `{"ok":true,"frontRows":<N>,"backParaIdx":<P>}`
    #[wasm_bindgen(js_name = splitTable)]
    pub fn split_table(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        at_row: u32,
    ) -> Result<String, JsValue> {
        let at_row = row_index_from_u32(at_row)?;
        self.split_table_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            at_row,
        )
        .map_err(|e| e.into())
    }


    /// 현재 표에 다음 표를 이어 붙인다 (한컴 [표-표 붙이기]).
    ///
    /// 반환값: JSON `{"ok":true,"rowCount":<N>}`
    #[wasm_bindgen(js_name = mergeTableWithNext)]
    pub fn merge_table_with_next(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.merge_table_with_next_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 표의 셀을 병합한다.
    ///
    /// 반환값: JSON `{"ok":true,"cellCount":<N>}`
    #[wasm_bindgen(js_name = mergeTableCells)]
    pub fn merge_table_cells(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<String, JsValue> {
        self.merge_table_cells_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            start_row as u16,
            start_col as u16,
            end_row as u16,
            end_col as u16,
        )
        .map_err(|e| e.into())
    }


    /// `mergeTableCells` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, startRow, startCol,
    /// endRow, endCol }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = mergeTableCellsEx)]
    pub fn merge_table_cells_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.merge_table_cells_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startRow").unwrap_or(0) as u16,
            json_u32(options_json, "startCol").unwrap_or(0) as u16,
            json_u32(options_json, "endRow").unwrap_or(0) as u16,
            json_u32(options_json, "endCol").unwrap_or(0) as u16,
        )
        .map_err(|e| e.into())
    }


    /// 병합된 셀을 나눈다 (split).
    ///
    /// 반환값: JSON `{"ok":true,"cellCount":<N>}`
    #[wasm_bindgen(js_name = splitTableCell)]
    pub fn split_table_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        row: u32,
        col: u32,
    ) -> Result<String, JsValue> {
        self.split_table_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            row as u16,
            col as u16,
        )
        .map_err(|e| e.into())
    }


    /// 셀을 N줄 × M칸으로 분할한다.
    ///
    /// 반환값: JSON `{"ok":true,"cellCount":<N>}`
    #[wasm_bindgen(js_name = splitTableCellInto)]
    pub fn split_table_cell_into(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        row: u32,
        col: u32,
        n_rows: u32,
        m_cols: u32,
        equal_row_height: bool,
        merge_first: bool,
    ) -> Result<String, JsValue> {
        self.split_table_cell_into_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            row as u16,
            col as u16,
            n_rows as u16,
            m_cols as u16,
            equal_row_height,
            merge_first,
        )
        .map_err(|e| e.into())
    }


    /// `splitTableCellInto` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, row, col, nRows, mCols,
    /// equalRowHeight?, mergeFirst? }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = splitTableCellIntoEx)]
    pub fn split_table_cell_into_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_u32};
        self.split_table_cell_into_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "row").unwrap_or(0) as u16,
            json_u32(options_json, "col").unwrap_or(0) as u16,
            json_u32(options_json, "nRows").unwrap_or(1) as u16,
            json_u32(options_json, "mCols").unwrap_or(1) as u16,
            json_bool(options_json, "equalRowHeight").unwrap_or(false),
            json_bool(options_json, "mergeFirst").unwrap_or(false),
        )
        .map_err(|e| e.into())
    }


    /// 범위 내 셀들을 각각 N줄 × M칸으로 분할한다.
    ///
    /// 반환값: JSON `{"ok":true,"cellCount":<N>}`
    #[wasm_bindgen(js_name = splitTableCellsInRange)]
    pub fn split_table_cells_in_range(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
        n_rows: u32,
        m_cols: u32,
        equal_row_height: bool,
    ) -> Result<String, JsValue> {
        self.split_table_cells_in_range_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            start_row as u16,
            start_col as u16,
            end_row as u16,
            end_col as u16,
            n_rows as u16,
            m_cols as u16,
            equal_row_height,
        )
        .map_err(|e| e.into())
    }


    /// `splitTableCellsInRange` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, startRow, startCol,
    /// endRow, endCol, nRows, mCols, equalRowHeight? }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = splitTableCellsInRangeEx)]
    pub fn split_table_cells_in_range_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_u32};
        self.split_table_cells_in_range_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startRow").unwrap_or(0) as u16,
            json_u32(options_json, "startCol").unwrap_or(0) as u16,
            json_u32(options_json, "endRow").unwrap_or(0) as u16,
            json_u32(options_json, "endCol").unwrap_or(0) as u16,
            json_u32(options_json, "nRows").unwrap_or(1) as u16,
            json_u32(options_json, "mCols").unwrap_or(1) as u16,
            json_bool(options_json, "equalRowHeight").unwrap_or(false),
        )
        .map_err(|e| e.into())
    }


    /// 선택된 표 셀 범위를 행/열 바꿈 복사용 내부 버퍼에 저장한다.
    ///
    /// 반환값: JSON `{"ok":true,"sourceRows":N,"sourceCols":N,"targetRows":N,"targetCols":N}`
    #[wasm_bindgen(js_name = copyTableCellsTransposed)]
    pub fn copy_table_cells_transposed(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<String, JsValue> {
        self.copy_table_cells_transposed_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            start_row as u16,
            start_col as u16,
            end_row as u16,
            end_col as u16,
        )
        .map_err(|e| e.into())
    }


    /// 행/열 바꿈 복사 버퍼를 대상 시작 셀부터 붙여넣는다.
    ///
    /// 반환값: JSON `{"ok":true,"sourceRows":N,"sourceCols":N,"targetRows":N,"targetCols":N}`
    #[wasm_bindgen(js_name = pasteTableCellsTransposed)]
    pub fn paste_table_cells_transposed(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        start_row: u32,
        start_col: u32,
    ) -> Result<String, JsValue> {
        self.paste_table_cells_transposed_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            start_row as u16,
            start_col as u16,
        )
        .map_err(|e| e.into())
    }


    /// 선택된 전체 표를 제자리에서 전치한다.
    ///
    /// 반환값: JSON `{"ok":true,"sourceRows":N,"sourceCols":N,"targetRows":N,"targetCols":N}`
    #[wasm_bindgen(js_name = transposeTableCellsInPlace)]
    pub fn transpose_table_cells_in_place(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.transpose_table_cells_in_place_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }


    /// 행/열 바꿈 복사 버퍼를 커서 위치에 새 표로 생성해 붙여넣는다.
    ///
    /// 반환값: JSON `{"ok":true,"paraIdx":N,"controlIdx":N,"sourceRows":N,"sourceCols":N,"targetRows":N,"targetCols":N}`
    #[wasm_bindgen(js_name = pasteTableCellsTransposedAsTable)]
    pub fn paste_table_cells_transposed_as_table(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.paste_table_cells_transposed_as_new_table_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 캐럿 위치에서 문단을 분할한다 (Enter 키).
    ///
    /// char_offset 이후의 텍스트가 새 문단으로 이동한다.
    /// 반환값: JSON `{"ok":true,"paraIdx":<new_para_idx>,"charOffset":0}`
    #[wasm_bindgen(js_name = splitParagraph)]
    pub fn split_paragraph(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        removed_para_meta: Option<String>,
    ) -> Result<String, JsValue> {
        self.split_paragraph_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            parse_removed_para_meta(removed_para_meta)?,
        )
        .map_err(|e| e.into())
    }


    /// 현재 문단을 이전 문단에 병합한다 (Backspace at start).
    ///
    /// para_idx의 텍스트가 para_idx-1에 결합되고 para_idx는 삭제된다.
    /// 반환값: JSON `{"ok":true,"paraIdx":<merged_para_idx>,"charOffset":<merge_point>}`
    #[wasm_bindgen(js_name = mergeParagraph)]
    pub fn merge_paragraph(&mut self, section_idx: u32, para_idx: u32) -> Result<String, JsValue> {
        self.merge_paragraph_native(section_idx as usize, para_idx as usize)
            .map_err(|e| e.into())
    }

}
