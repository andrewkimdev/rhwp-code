//! WASM ↔ JavaScript 공개 API
//!
//! wasm-bindgen을 통해 JavaScript에서 호출 가능한 API를 정의한다.
//! 주요 API:
//! - `HwpDocument::new(data)` - HWP 파일 로드
//! - `HwpDocument::page_count()` - 페이지 수 조회
//! - `HwpDocument::render_page_svg(page_num)` - SVG로 렌더링
//! - `HwpDocument::render_page_html(page_num)` - HTML로 렌더링

// 하위 호환성: tests.rs에서 super::json_escape 등으로 접근 가능하도록 재내보내기
pub(crate) use crate::document_core::helpers::*;

use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

use crate::document_core::helpers::parse_removed_para_meta;
use crate::document_core::{
    DeferredPaginationJobState, DeferredPaginationStepResult, DocumentCore, DEFAULT_FALLBACK_FONT,
};
use crate::error::HwpError;
use crate::model::control::Control;
use crate::model::document::{Document, Section};
use crate::model::page::ColumnDef;
use crate::model::paragraph::Paragraph;
use crate::model::path::{path_from_flat, DocumentPath, PathSegment};
use crate::model::shape::ShapeObject;
use crate::renderer::canvas::CanvasRenderer;
use crate::renderer::composer::{
    compose_paragraph, compose_section, reflow_line_segs, ComposedParagraph,
};
use crate::renderer::height_measurer::{HeightMeasurer, MeasuredSection, MeasuredTable};
use crate::renderer::html::HtmlRenderer;
use crate::renderer::layout::LayoutEngine;
use crate::renderer::page_layout::PageLayoutInfo;
use crate::renderer::pagination::{PaginationResult, Paginator};
use crate::renderer::render_tree::PageRenderTree;
use crate::renderer::scheduler::{RenderEvent, RenderObserver, RenderScheduler, Viewport};
use crate::renderer::style_resolver::{
    resolve_font_substitution, resolve_styles, ResolvedStyleSet,
};
use crate::renderer::svg::SvgRenderer;
use crate::renderer::DEFAULT_DPI;
mod api_queries;
pub(crate) use api_queries::*;
mod api_export;
pub(crate) use api_export::*;
mod api_editing;
pub(crate) use api_editing::*;
mod api_clipboard;
pub(crate) use api_clipboard::*;



/// 어떤 렌더 export 가 Subsecond 핫패치 경계 뒤에 있는지 선언하는 곳 (#4577).
mod subsecond_boundary;

impl From<HwpError> for JsValue {
    fn from(err: HwpError) -> Self {
        JsValue::from_str(&err.to_string())
    }
}

/// WASM 경계의 u32 행 인덱스를 u16 으로 변환한다. 묵시적 `as u16` 절단은
/// 65537 을 1 로 바꿔 요청 밖 행에서 표를 조작하게 되므로 명시적으로 거부한다.
fn row_index_from_u32(v: u32) -> Result<u16, HwpError> {
    u16::try_from(v)
        .map_err(|_| HwpError::RenderError(format!("행 인덱스 {} 가 최대치(65535)를 넘습니다", v)))
}

fn deferred_pagination_result_json(result: DeferredPaginationStepResult) -> String {
    let status = match result.state {
        DeferredPaginationJobState::None => "none",
        DeferredPaginationJobState::Pending => "pending",
        DeferredPaginationJobState::Complete => "complete",
        DeferredPaginationJobState::Fallback => "fallback",
        DeferredPaginationJobState::Stale => "stale",
    };
    serde_json::json!({
        "ok": true,
        "status": status,
        "revision": result.revision,
        "fragmentsProcessed": result.fragments_processed,
        "pageCount": result.page_count,
    })
    .to_string()
}

/// [Task #1161] 클립보드 API 의 cellPath JSON 인자 파싱.
/// 빈 문자열 또는 `"[]"` 면 본문(빈 경로), 그 외에는
/// `[{"controlIndex","cellIndex","cellParaIndex"}, ...]` 를 파싱한다.
fn parse_cell_path_arg(cell_path_json: &str) -> Result<Vec<(usize, usize, usize)>, JsValue> {
    if cell_path_json.is_empty() || cell_path_json == "[]" {
        Ok(Vec::new())
    } else {
        DocumentCore::parse_cell_path(cell_path_json).map_err(JsValue::from)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const MAX_CANVAS_DIMENSION: f64 = 16_384.0;

#[cfg(any(target_arch = "wasm32", test))]
fn normalize_canvas_scale(
    page_width: f64,
    page_height: f64,
    requested_scale: f64,
) -> Result<f64, &'static str> {
    if !page_width.is_finite()
        || !page_height.is_finite()
        || page_width <= 0.0
        || page_height <= 0.0
    {
        return Err("invalid page dimensions");
    }

    let scale = if requested_scale <= 0.0 || !requested_scale.is_finite() {
        1.0
    } else {
        requested_scale.clamp(0.25, 12.0)
    };

    let scaled_width = page_width * scale;
    let scaled_height = page_height * scale;
    if !scaled_width.is_finite() || !scaled_height.is_finite() {
        return Ok((MAX_CANVAS_DIMENSION / page_width)
            .min(MAX_CANVAS_DIMENSION / page_height)
            .min(scale));
    }

    if scaled_width > MAX_CANVAS_DIMENSION || scaled_height > MAX_CANVAS_DIMENSION {
        Ok((MAX_CANVAS_DIMENSION / page_width)
            .min(MAX_CANVAS_DIMENSION / page_height)
            .min(scale))
    } else {
        Ok(scale)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn scaled_canvas_extent(page_extent: f64, scale: f64) -> u32 {
    // Canvas의 bitmap 크기는 정수여야 한다. 절사하면 A4 같은 분수 CSS px 페이지를
    // 고배율로 그릴 때 우·하단 한 줄이 잘린다. 실제 콘텐츠의 scale은 그대로 두고
    // bitmap 경계만 올림해 페이지 전체를 담는다.
    (page_extent * scale)
        .ceil()
        .clamp(1.0, MAX_CANVAS_DIMENSION) as u32
}

#[cfg(target_arch = "wasm32")]
fn canvas_layer_filter(
    layer_kind: &str,
) -> Result<crate::renderer::web_canvas::LayerFilter, JsValue> {
    use crate::model::shape::TextWrap;
    use crate::renderer::web_canvas::LayerFilter;

    match layer_kind {
        "all" => Ok(LayerFilter::All),
        "background" => Ok(LayerFilter::BackgroundOnly),
        "flow" => Ok(LayerFilter::FlowOnly),
        "flow-dynamic" => Ok(LayerFilter::FlowDynamic),
        "flow-static" => Ok(LayerFilter::FlowStatic),
        "behind" => Ok(LayerFilter::WrapOnly(TextWrap::BehindText)),
        "front" => Ok(LayerFilter::WrapOnly(TextWrap::InFrontOfText)),
        _ => Err(JsValue::from_str(
            "invalid layer_kind: 'all' | 'background' | 'flow' | 'flow-dynamic' | 'flow-static' | 'behind' | 'front'",
        )),
    }
}

#[cfg(target_arch = "wasm32")]
fn render_page_to_canvas_filtered_with_profile_impl(
    document: &HwpDocument,
    page_num: u32,
    canvas: &HtmlCanvasElement,
    scale: f64,
    layer_kind: &str,
    profile: &str,
) -> Result<(), JsValue> {
    use crate::paint::RenderProfile;
    use crate::renderer::layer_renderer::LayerRenderer;
    use crate::renderer::web_canvas::WebCanvasRenderer;

    let filter = canvas_layer_filter(layer_kind)?;

    let profile = RenderProfile::parse(profile)
        .ok_or_else(|| JsValue::from_str(&format!("unsupported render profile: {profile}")))?;
    let tree = document
        .build_page_layer_tree_with_profile(page_num, profile)
        .map_err(JsValue::from)?;

    let scale = normalize_canvas_scale(tree.page_width, tree.page_height, scale)
        .map_err(JsValue::from_str)?;

    canvas.set_width(scaled_canvas_extent(tree.page_width, scale));
    canvas.set_height(scaled_canvas_extent(tree.page_height, scale));

    let mut renderer = WebCanvasRenderer::new(canvas)?;
    renderer.show_paragraph_marks = document.show_paragraph_marks;
    renderer.show_control_codes = document.show_control_codes;
    renderer.set_scale(scale);
    renderer.set_layer_filter(filter);
    renderer.render_page(&tree).map_err(JsValue::from)?;
    Ok(())
}

/// 부분 재도색 본체.
///
/// `patch` 는 page-space 요청 사각형이다 — `x/y/width/height` 를 편 인자로 받으면 이 함수의
/// 인자가 10개가 되어 `subsecond::HotFn` 이 붙지 못한다(`HotFunction` 은 9개까지). 경계를
/// 유지하려면 사각형을 한 값으로 접어야 한다.
#[cfg(target_arch = "wasm32")]
fn render_page_patch_to_canvas_filtered_with_profile_impl(
    document: &HwpDocument,
    page_num: u32,
    canvas: &HtmlCanvasElement,
    scale: f64,
    layer_kind: &str,
    profile: &str,
    patch: crate::renderer::render_tree::BoundingBox,
) -> Result<(), JsValue> {
    use crate::paint::RenderProfile;
    use crate::renderer::layer_renderer::LayerRenderer;
    use crate::renderer::render_tree::BoundingBox;
    use crate::renderer::web_canvas::WebCanvasRenderer;

    let BoundingBox {
        x,
        y,
        width,
        height,
    } = patch;
    if ![x, y, width, height].into_iter().all(f64::is_finite) || width <= 0.0 || height <= 0.0 {
        return Err(JsValue::from_str("invalid page patch rectangle"));
    }

    let filter = canvas_layer_filter(layer_kind)?;
    let profile = RenderProfile::parse(profile)
        .ok_or_else(|| JsValue::from_str(&format!("unsupported render profile: {profile}")))?;
    let tree = document
        .build_page_layer_tree_with_profile(page_num, profile)
        .map_err(JsValue::from)?;
    let scale = normalize_canvas_scale(tree.page_width, tree.page_height, scale)
        .map_err(JsValue::from_str)?;

    let expected_width = scaled_canvas_extent(tree.page_width, scale);
    let expected_height = scaled_canvas_extent(tree.page_height, scale);
    if canvas.width() != expected_width || canvas.height() != expected_height {
        return Err(JsValue::from_str(
            "page patch canvas extent does not match the current page render",
        ));
    }

    let left = x.max(0.0).min(tree.page_width);
    let top = y.max(0.0).min(tree.page_height);
    let right = (x + width).max(left).min(tree.page_width);
    let bottom = (y + height).max(top).min(tree.page_height);
    if right <= left || bottom <= top {
        return Err(JsValue::from_str(
            "page patch rectangle does not intersect the page",
        ));
    }

    let mut renderer = WebCanvasRenderer::new(canvas)?;
    renderer.show_paragraph_marks = document.show_paragraph_marks;
    renderer.show_control_codes = document.show_control_codes;
    renderer.set_scale(scale);
    renderer.set_layer_filter(filter);
    renderer.set_partial_clip(BoundingBox::new(left, top, right - left, bottom - top));
    renderer.render_page(&tree).map_err(JsValue::from)?;
    Ok(())
}

fn get_page_layer_tree_with_profile_impl(
    document: &HwpDocument,
    page_num: u32,
    profile: &str,
    omit_image_bytes: bool,
) -> Result<String, JsValue> {
    let profile = crate::paint::RenderProfile::parse(profile)
        .ok_or_else(|| JsValue::from_str(&format!("unsupported render profile: {profile}")))?;
    document
        .get_page_layer_tree_with_options_native(
            page_num,
            profile,
            crate::paint::LayerJsonOptions { omit_image_bytes },
        )
        .map_err(|error| error.into())
}

/// 레이어 평면 요약 본체. 합성 판정(`getLayerPlaneSummary`)이 이 결과로 정해지므로 페인트와
/// 같은 패치 세대를 봐야 한다.
fn get_page_overlay_images_impl(document: &HwpDocument, page_num: u32) -> Result<String, JsValue> {
    document
        .get_page_overlay_images_native(page_num)
        .map_err(|error| error.into())
}

/// 본문 그림 배치 본체. 경계 뒤에서 그린 캔버스 위에 DOM `<img>` 로 합성되는 값이다.
fn get_page_flow_image_ops_impl(document: &HwpDocument, page_num: u32) -> Result<String, JsValue> {
    document
        .get_page_flow_image_ops_native(page_num)
        .map_err(|error| error.into())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalImageReference {
    key: String,
    bin_data_id: u16,
    original_path: String,
    basename: String,
    extension: String,
    loaded: bool,
}

fn external_path_basename(path: &str) -> &str {
    path.rsplit(|c| c == '/' || c == '\\')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

fn external_path_extension(basename: &str) -> String {
    std::path::Path::new(basename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

fn parse_external_image_key(key: &str) -> Option<u16> {
    let bin_data_id = key.strip_prefix("binData:")?.parse::<u16>().ok()?;
    (bin_data_id != 0).then_some(bin_data_id)
}

fn collect_external_image_references(document: &Document) -> Vec<ExternalImageReference> {
    let mut references = std::collections::BTreeMap::new();

    for section in &document.sections {
        for para in &section.paragraphs {
            for ctrl in &para.controls {
                let pic = match ctrl {
                    Control::Picture(pic) => pic,
                    Control::Shape(shape) => match shape.as_ref() {
                        ShapeObject::Picture(pic) => pic,
                        _ => continue,
                    },
                    _ => continue,
                };

                let Some(original_path) = pic.image_attr.external_path.as_ref() else {
                    continue;
                };

                let bin_data_id = pic.image_attr.bin_data_id;
                references.entry(bin_data_id).or_insert_with(|| {
                    let basename = external_path_basename(original_path).to_string();
                    ExternalImageReference {
                        key: format!("binData:{bin_data_id}"),
                        bin_data_id,
                        extension: external_path_extension(&basename),
                        basename,
                        original_path: original_path.clone(),
                        loaded: document.external_image_loaded(bin_data_id),
                    }
                });
            }
        }
    }

    references.into_values().collect()
}

/// WASM에서 사용할 HWP 문서 래퍼
///
/// 도메인 로직은 `DocumentCore`에 구현되어 있으며,
/// `Deref`/`DerefMut`를 통해 투명하게 접근한다.
#[wasm_bindgen]
pub struct HwpDocument {
    core: DocumentCore,
}

/// 한 번의 문서 내보내기 결과.
///
/// 바이트와 content-loss 보고서가 같은 객체에 있어 다른 저장의 상태와 섞이지 않는다.
/// `takeBytes()`는 Rust 결과의 바이트 소유권을 한 번만 소비하며, 보고서는 그 전후 어느
/// 순서로든 읽을 수 있다. 바이트를 두 번 꺼내는 것은 명시적 오류다.
#[wasm_bindgen]
pub struct DocumentExport {
    bytes: Option<Vec<u8>>,
    content_loss_json: String,
}

impl From<crate::serializer::SerializedDocument> for DocumentExport {
    fn from(serialized: crate::serializer::SerializedDocument) -> Self {
        let (bytes, content_loss) = serialized.into_parts();
        Self {
            bytes: Some(bytes),
            content_loss_json: content_loss.to_json(),
        }
    }
}

#[wasm_bindgen]
impl DocumentExport {
    /// 이번 산출물의 content-loss 보고서(JSON). `takeBytes()` 뒤에도 읽을 수 있다.
    #[wasm_bindgen(js_name = contentLoss)]
    pub fn content_loss(&self) -> String {
        self.content_loss_json.clone()
    }

    /// 아직 JS로 옮기지 않은 바이트를 소유하는지 반환한다.
    #[wasm_bindgen(js_name = hasBytes)]
    pub fn has_bytes(&self) -> bool {
        self.bytes.is_some()
    }

    /// 산출 바이트 소유권을 한 번 꺼낸다.
    #[wasm_bindgen(js_name = takeBytes)]
    pub fn take_bytes(&mut self) -> Result<Vec<u8>, JsValue> {
        self.bytes
            .take()
            .ok_or_else(|| JsValue::from_str("이 내보내기 결과의 바이트를 이미 가져갔습니다"))
    }
}

impl std::ops::Deref for HwpDocument {
    type Target = DocumentCore;
    fn deref(&self) -> &DocumentCore {
        &self.core
    }
}

impl std::ops::DerefMut for HwpDocument {
    fn deref_mut(&mut self) -> &mut DocumentCore {
        &mut self.core
    }
}

/// 네이티브(비-WASM) 환경용 래퍼 메서드.
///
/// 테스트 및 CLI 환경에서 `HwpDocument::from_bytes()` 등을 직접 호출할 수 있도록 한다.
impl HwpDocument {
    pub fn from_bytes(data: &[u8]) -> Result<HwpDocument, HwpError> {
        DocumentCore::from_bytes(data).map(|core| HwpDocument { core })
    }

    pub fn from_bytes_with_password(data: &[u8], password: &[u8]) -> Result<HwpDocument, HwpError> {
        DocumentCore::from_bytes_with_password(data, password).map(|core| HwpDocument { core })
    }

    pub fn find_initial_column_def(paragraphs: &[Paragraph]) -> ColumnDef {
        DocumentCore::find_initial_column_def(paragraphs)
    }

    pub fn find_column_def_for_paragraph(paragraphs: &[Paragraph], para_idx: usize) -> ColumnDef {
        DocumentCore::find_column_def_for_paragraph(paragraphs, para_idx)
    }

    fn inject_external_image_by_bin_data_id(
        &mut self,
        bin_data_id: u16,
        data: &[u8],
        display_path: &str,
        fallback_basename: Option<&str>,
    ) -> u32 {
        let Some(reference) = collect_external_image_references(self.document())
            .into_iter()
            .find(|reference| reference.bin_data_id == bin_data_id)
        else {
            return 0;
        };

        if reference.loaded {
            return 0;
        }

        if !self.document_mut().inject_external_image_data(
            bin_data_id,
            data.to_vec(),
            reference.extension.clone(),
        ) {
            return 0;
        }

        let basename = fallback_basename.unwrap_or(&reference.basename);
        let resolved = if display_path.is_empty() {
            format!("/samples/{basename}")
        } else {
            display_path.to_string()
        };
        self.document_mut()
            .update_external_image_display_path(bin_data_id, &resolved);

        1
    }
}

fn hml_warning_code(code: crate::parser::hml::HmlWarningCode) -> &'static str {
    use crate::parser::hml::HmlWarningCode;

    match code {
        HmlWarningCode::UnsupportedElement => "UnsupportedElement",
        HmlWarningCode::UnsupportedAttribute => "UnsupportedAttribute",
        HmlWarningCode::UnsupportedEquationSemantics => "UnsupportedEquationSemantics",
        HmlWarningCode::MissingResource => "MissingResource",
        HmlWarningCode::ExternalResourceBlocked => "ExternalResourceBlocked",
        HmlWarningCode::InvalidReference => "InvalidReference",
        HmlWarningCode::LossyConversion => "LossyConversion",
    }
}

fn hml_warning_json(warning: &crate::parser::hml::HmlWarning) -> serde_json::Value {
    serde_json::json!({
        "code": hml_warning_code(warning.code),
        "xmlPath": warning.xml_path,
        "message": warning.message,
        "preserved": warning.preserved,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HmlSaveState {
    source_format: &'static str,
    hml_savable: bool,
    blockers: Vec<HmlSaveBlockerDto>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HmlSaveBlockerDto {
    code: String,
    xml_path: String,
    message: String,
    preserved: bool,
}

fn hml_save_state(core: &DocumentCore) -> HmlSaveState {
    let source_format = source_format_name(core.source_format);
    match core.hml_export_preflight() {
        Ok(()) => HmlSaveState {
            source_format,
            hml_savable: true,
            blockers: Vec::new(),
        },
        Err(error) => {
            let blockers = error
                .blockers()
                .iter()
                .map(|blocker| HmlSaveBlockerDto {
                    code: blocker.code.to_string(),
                    xml_path: blocker.xml_path.clone(),
                    message: blocker.message.clone(),
                    preserved: false,
                })
                .collect();
            HmlSaveState {
                source_format,
                hml_savable: false,
                blockers,
            }
        }
    }
}

fn source_format_name(format: crate::parser::FileFormat) -> &'static str {
    match format {
        crate::parser::FileFormat::Hwpx => "hwpx",
        crate::parser::FileFormat::Hml => "hml",
        _ => "hwp",
    }
}

fn format_hml_export_error(error: &crate::serializer::hml::HmlExportError) -> String {
    use std::fmt::Write;

    let mut message = error.to_string();
    for blocker in error.blockers() {
        let _ = write!(
            message,
            "\n[{}] {}: {}",
            blocker.code, blocker.xml_path, blocker.message
        );
    }
    message
}

#[wasm_bindgen]
impl HwpDocument {
    /// HWP 파일 바이트를 로드하여 문서 객체를 생성한다.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<HwpDocument, JsValue> {
        DocumentCore::from_bytes(data)
            .map(|core| HwpDocument { core })
            .map_err(|e| e.into())
    }

    /// 비밀번호로 보호된 HWP/HWPX 파일을 비밀번호와 함께 로드한다.
    ///
    /// HWP5 EncryptVersion 4, 압축 HWP3와 ODF AES-256-CBC HWPX를 지원한다.
    /// 구버전/비압축 HWP3 암호화와 DRM은 지원하지 않는다.
    /// 비밀번호가 틀린 경우 JS 측에서 잡을 수 있도록 에러 메시지에
    /// "비밀번호가 일치하지 않"이 포함된 `JsValue` 를 반환한다.
    /// 암호화되지 않은 일반 문서에 비밀번호를 전달해도 정상 로드된다.
    #[wasm_bindgen(js_name = openWithPassword)]
    pub fn open_with_password(data: &[u8], password: &str) -> Result<HwpDocument, JsValue> {
        Self::from_bytes_with_password(data, password.as_bytes()).map_err(|e| e.into())
    }


    /// 총 페이지 수를 반환한다.
    #[wasm_bindgen(js_name = pageCount)]
    pub fn page_count(&self) -> u32 {
        self.core.page_count()
    }


    /// 바이트는 그대로인데 화면용으로 파생해 둔 것이 더는 원본과 대응하지 않을 때 다시 만든다.
    ///
    /// 몸통에 벤더는 없다 — `DocumentCore::rebuild_derived_state` 를 그대로 위임하고, 그 연산
    /// 자체는 렌더 코드 교체 말고도 쓰일 수 있는 것이다. studio 가 같은 사건을 부르는 말도
    /// `document-view-changed`("바이트는 안 바뀌었고 화면용으로 파생한 것이 바뀌었다")다.
    /// 그래서 이름을 벤더가 아니라 그 어휘에 맞춘다 (#4580).
    ///
    /// `&mut self` 여야 한다. 페이지 트리 캐시만 비우면 다시 그리는 값은 새 코드가 내지만
    /// 그 값을 앉히는 페이지 박스와 문단 조합은 `pagination`·`composed`·측정 캐시에 남은
    /// 패치 이전 코드의 결과라, 소스의 어느 버전에도 대응하지 않는 화면이 나온다 (#4576).
    /// 그 셋은 모두 `&mut self` 를 요구하므로 `&self` 로는 계약 자체를 표현할 수 없다.
    ///
    /// 위 리비전 조회와 짝이라 게이트도 같다. 네이티브에는 호출부가 하나도 없다 — 몸통은
    /// 타깃과 무관하지만, 이 export 를 부르는 계약 자체가 브라우저의 것이다 (#4580).
    #[cfg(all(feature = "subsecond-dev", target_arch = "wasm32"))]
    #[wasm_bindgen(js_name = rebuildDerivedState)]
    pub fn rebuild_derived_state(&mut self) {
        self.core.rebuild_derived_state();
    }


    /// 논리적 오프셋 → 텍스트 오프셋 변환.
    #[wasm_bindgen(js_name = logicalToTextOffset)]
    pub fn logical_to_text_offset(
        &self,
        section_idx: u32,
        para_idx: u32,
        logical_offset: u32,
    ) -> Result<u32, JsValue> {
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
        Ok(text_offset as u32)
    }

    /// 텍스트 오프셋 → 논리적 오프셋 변환.
    #[wasm_bindgen(js_name = textToLogicalOffset)]
    pub fn text_to_logical_offset(
        &self,
        section_idx: u32,
        para_idx: u32,
        text_offset: u32,
    ) -> Result<u32, JsValue> {
        let sec = section_idx as usize;
        let pi = para_idx as usize;
        if sec >= self.document.sections.len() || pi >= self.document.sections[sec].paragraphs.len()
        {
            return Err(JsValue::from_str("인덱스 범위 초과"));
        }
        Ok(crate::document_core::helpers::text_to_logical_offset(
            &self.document.sections[sec].paragraphs[pi],
            text_offset as usize,
        ) as u32)
    }


    #[wasm_bindgen(js_name = replaceBodyTextLocal)]
    pub fn replace_body_text_local(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        delete_count: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.replace_body_text_local_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            delete_count as usize,
            text,
        )
        .map_err(|e| e.into())
    }


    /// 표 셀 내부의 짧은 IME 조합 문자열을 원자적으로 교체하고 전체 페이지네이션은 지연한다.
    #[wasm_bindgen(js_name = replaceTextInCellDeferredPagination)]
    pub fn replace_text_in_cell_deferred_pagination(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        delete_count: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.replace_text_in_cell_native_deferred_pagination(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            delete_count as usize,
            text,
        )
        .map_err(|e| e.into())
    }


    /// 대형 표 continuation을 fragment budget만큼 전진한다.
    #[wasm_bindgen(js_name = stepDeferredPagination)]
    pub fn step_deferred_pagination(&mut self, fragment_budget: u32) -> Result<String, JsValue> {
        Ok(deferred_pagination_result_json(
            self.core
                .step_deferred_pagination((fragment_budget as usize).max(1)),
        ))
    }

    #[wasm_bindgen(js_name = cancelDeferredPagination)]
    pub fn cancel_deferred_pagination(&mut self) -> bool {
        self.core.cancel_deferred_pagination()
    }

    /// 지연된 페이지네이션을 동기 barrier로 flush하고 최신 페이지 수를 반환한다.
    #[wasm_bindgen(js_name = flushDeferredPagination)]
    pub fn flush_deferred_pagination(&mut self) -> Result<String, JsValue> {
        Ok(deferred_pagination_result_json(
            self.core.flush_deferred_pagination(),
        ))
    }


    // ─── 중첩 표 path 기반 편집 API ──────────────────────────


    // ─── 머리말/꼬리말 API ──────────────────────────────────


    /// 행/열 바꿈 복사 버퍼 보유 여부를 반환한다.
    #[wasm_bindgen(js_name = hasTableTransposeClipboard)]
    pub fn has_table_transpose_clipboard(&self) -> bool {
        self.has_table_transpose_clipboard_native()
    }


    // ─── Phase 1: 기본 편집 보조 API ───────────────────────────


    /// 문서 트리에서 다음 편집 가능한 컨트롤/본문을 찾는다.
    /// delta=+1(앞), delta=-1(뒤). ctrl_idx=-1이면 본문 텍스트에서 출발.
    #[wasm_bindgen(js_name = findNextEditableControl)]
    pub fn find_next_editable_control(
        &self,
        section_idx: u32,
        para_idx: u32,
        ctrl_idx: i32,
        delta: i32,
    ) -> String {
        self.find_next_editable_control_native(
            section_idx as usize,
            para_idx as usize,
            ctrl_idx,
            delta,
        )
    }

    /// 커서에서 이전 방향으로 가장 가까운 선택 가능 컨트롤을 찾는다 (F11 키).
    #[wasm_bindgen(js_name = findNearestControlBackward)]
    pub fn find_nearest_control_backward(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> String {
        self.find_nearest_control_backward_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
    }

    /// 현재 위치 이후의 가장 가까운 선택 가능 컨트롤을 찾는다 (Shift+F11).
    ///
    /// `inclusive`: `find_nearest_control_forward_native`와 동일한 의미. wasm-bindgen은 인자
    /// 기본값을 지원하지 않으므로 JS 쪽 기본값은 `wasm-bridge.ts`의 래퍼가 담당한다(기존 유일한
    /// 실사용 호출자 Shift+F11은 `false`로 고정해 회귀 없음).
    #[wasm_bindgen(js_name = findNearestControlForward)]
    pub fn find_nearest_control_forward(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        inclusive: bool,
    ) -> String {
        self.find_nearest_control_forward_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            inclusive,
        )
    }


    /// 문서 트리 DFS 기반 다음/이전 편집 가능 위치를 반환한다.
    /// context_json: NavContextEntry 배열의 JSON (빈 배열 "[]" = body)
    #[wasm_bindgen(js_name = navigateNextEditable)]
    pub fn navigate_next_editable_wasm(
        &self,
        sec: u32,
        para: u32,
        char_offset: u32,
        delta: i32,
        context_json: &str,
    ) -> String {
        let raw_context = DocumentCore::parse_nav_context(context_json);
        // TypeScript에서 ctrl_text_pos=0으로 전달되므로 실제 값으로 보정
        let context = DocumentCore::fix_context_text_positions(
            &self.core.document.sections,
            sec as usize,
            &raw_context,
        );

        // 오버플로우 링크 계산 (캐시됨)
        let overflow_links = self.core.get_overflow_links(sec as usize);

        // 컨텍스트가 있으면 (컨테이너 내부) 렌더링된 마지막 문단 인덱스를 조회
        let max_para = if !context.is_empty() {
            let last = &context[context.len() - 1];
            self.core.last_rendered_para_in_container(
                sec as usize,
                last.parent_para,
                last.ctrl_idx,
                last.cell_idx,
            )
        } else {
            None
        };

        let result = self.core.navigate_next_editable(
            sec as usize,
            para as usize,
            char_offset as usize,
            delta,
            &context,
            max_para,
            &overflow_links,
        );
        DocumentCore::nav_result_to_json(&result)
    }


    // ─── Phase 1 끝 ─────────────────────────────────────────

    // ─── Phase 2: 커서/히트 테스트 API ──────────────────────────


    /// 페이지 좌표에서 문서 위치를 찾는다.
    ///
    /// 반환: JSON `{"sectionIndex":N,"paragraphIndex":N,"charOffset":N}`
    #[wasm_bindgen(js_name = hitTest)]
    pub fn hit_test(&self, page_num: u32, x: f64, y: f64) -> Result<String, JsValue> {
        self.hit_test_native(page_num, x, y).map_err(|e| e.into())
    }


    /// 페이지 좌표가 머리말/꼬리말 영역에 해당하는지 판별한다.
    ///
    /// 반환: JSON `{"hit":true/false,"isHeader":bool,"sectionIndex":N,"applyTo":N}`
    #[wasm_bindgen(js_name = hitTestHeaderFooter)]
    pub fn hit_test_header_footer(&self, page_num: u32, x: f64, y: f64) -> Result<String, JsValue> {
        self.hit_test_header_footer_native(page_num, x, y)
            .map_err(|e| e.into())
    }


    /// 머리말/꼬리말 내부 텍스트 히트테스트.
    ///
    /// 편집 모드에서 클릭한 좌표의 문단·문자 위치를 반환.
    /// 반환: JSON `{"hit":true,"paraIndex":N,"charOffset":N,"cursorRect":{...}}`
    #[wasm_bindgen(js_name = hitTestInHeaderFooter)]
    pub fn hit_test_in_header_footer(
        &self,
        page_num: u32,
        is_header: bool,
        x: f64,
        y: f64,
    ) -> Result<String, JsValue> {
        self.hit_test_in_header_footer_native(page_num, is_header, x, y)
            .map_err(|e| e.into())
    }


    /// 페이지 단위로 이전/다음 머리말·꼬리말로 이동한다.
    ///
    /// 반환: JSON `{"ok":true,"pageIndex":N,"sectionIdx":N,"isHeader":bool,"applyTo":N}`
    /// 또는 더 이상 이동할 페이지가 없으면 `{"ok":false}`
    #[wasm_bindgen(js_name = navigateHeaderFooterByPage)]
    pub fn navigate_header_footer_by_page(
        &self,
        current_page: u32,
        is_header: bool,
        direction: i32,
    ) -> Result<String, JsValue> {
        self.navigate_header_footer_by_page_native(current_page, is_header, direction)
            .map_err(|e| e.into())
    }


    // ─── Phase 3: 커서 이동 API ──────────────────────────────


    /// 여러 셀의 width/height를 한 번에 조절한다 (배치).
    ///
    /// json: `[{"cellIdx":0,"widthDelta":150},{"cellIdx":2,"heightDelta":-100}]`
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = resizeTableCells)]
    pub fn resize_table_cells(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        json: &str,
    ) -> Result<String, JsValue> {
        self.resize_table_cells_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            json,
        )
        .map_err(|e| e.into())
    }


    /// [Task #2230] 기존 Picture 컨트롤에 이미지를 지정한다 — 그림 미지정
    /// placeholder(missing image 컨트롤)의 편집 뷰 그림 삽입.
    ///
    /// `cell_path_json` 이 빈 문자열 또는 `"[]"` 면 본문 문단의 컨트롤,
    /// 그 외에는 셀/글상자 안 문단의 컨트롤을 대상으로 한다. 개체 틀 크기는
    /// 유지되고(한컴 placeholder 는 틀에 그림을 맞춤) BinData 등록 규칙은
    /// insertPicture 와 공유한다.
    ///
    /// 반환: `{"ok":true,"binDataId":<N>}`
    #[wasm_bindgen(js_name = assignPictureImage)]
    #[allow(clippy::too_many_arguments)]
    pub fn assign_picture_image(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        cell_path_json: &str,
        control_idx: u32,
        image_data: &[u8],
        natural_width_px: u32,
        natural_height_px: u32,
        extension: &str,
    ) -> Result<String, JsValue> {
        let cell_path: Vec<(usize, usize, usize)> =
            if cell_path_json.is_empty() || cell_path_json == "[]" {
                Vec::new()
            } else {
                DocumentCore::parse_cell_path(cell_path_json).map_err(JsValue::from)?
            };
        self.assign_picture_image_native(
            section_idx as usize,
            parent_para_idx as usize,
            &cell_path,
            control_idx as usize,
            image_data,
            natural_width_px,
            natural_height_px,
            extension,
        )
        .map_err(|e| e.into())
    }


    /// [Task #741 후속] 외부 file path 그림 영역 영역 binary data 영역 inject.
    ///
    /// JS 영역 영역 영역 fetch 영역 영역 영역 file 영역 load 영역 후 본 메서드 영역 호출 영역
    /// IR 영역 영역 영역 image binary 영역 영역 → renderer 영역 영역 표시.
    ///
    /// `basename`: 영역 영역 file 영역 영역 (예: "oracle.gif")
    /// `data`: 영역 영역 binary 영역
    /// `display_path`: dialog 영역 영역 영역 영역 표시 영역 영역 path. 빈 문자열 ("") 영역
    ///                 영역 영역 fallback 영역 영역 `/samples/<basename>` 영역 사용. 한컴 viewer
    ///                 정합 영역 영역 OS 영역 절대 경로 영역 영역 (예: "/Users/.../samples/rdb02.gif")
    #[wasm_bindgen(js_name = injectExternalImage)]
    pub fn inject_external_image(
        &mut self,
        basename: &str,
        data: &[u8],
        display_path: &str,
    ) -> u32 {
        use crate::model::control::Control;
        use crate::model::shape::ShapeObject;
        use std::collections::BTreeSet;

        let mut injected: u32 = 0;
        // 영역 외부 image 영역 영역 영역 영역 basename 매칭 영역 영역 id 수집
        let mut targets: BTreeSet<u16> = BTreeSet::new();
        for section in &self.document().sections {
            for para in &section.paragraphs {
                for ctrl in &para.controls {
                    let pic = match ctrl {
                        Control::Picture(p) => p,
                        Control::Shape(s) => match s.as_ref() {
                            ShapeObject::Picture(p) => p,
                            _ => continue,
                        },
                        _ => continue,
                    };
                    if let Some(ref path) = pic.image_attr.external_path {
                        let path_basename = path
                            .rsplit(|c| c == '/' || c == '\\')
                            .next()
                            .unwrap_or(path);
                        if path_basename != basename {
                            continue;
                        }
                        let id = pic.image_attr.bin_data_id;
                        if self.document().external_image_loaded(id) {
                            continue;
                        }
                        targets.insert(id);
                    }
                }
            }
        }

        for id in targets {
            injected +=
                self.inject_external_image_by_bin_data_id(id, data, display_path, Some(basename));
        }

        if injected > 0 {
            self.invalidate_page_tree_cache();
        }

        injected
    }

    /// [Task #1143] `getExternalImageReferences()` 의 key로 외부 이미지 bytes를 주입한다.
    ///
    /// 지원 key: `binData:<bin_data_id>`.
    /// 잘못된 key, 존재하지 않는 key, 이미 loaded 상태인 reference는 0을 반환한다.
    #[wasm_bindgen(js_name = injectExternalImageByKey)]
    pub fn inject_external_image_by_key(
        &mut self,
        key: &str,
        data: &[u8],
        display_path: &str,
    ) -> u32 {
        let Some(bin_data_id) = parse_external_image_key(key) else {
            return 0;
        };

        let injected =
            self.inject_external_image_by_bin_data_id(bin_data_id, data, display_path, None);
        if injected > 0 {
            self.invalidate_page_tree_cache();
        }
        injected
    }


    // ─── Equation(수식) API ──────────────────────────────


    /// JSON에서 polygonPoints 배열 파싱
    fn parse_polygon_points(json: &str) -> Vec<crate::model::Point> {
        // 간단한 파싱: "polygonPoints":[{"x":1,"y":2},{"x":3,"y":4}]
        let key = "\"polygonPoints\":[";
        if let Some(start) = json.find(key) {
            let rest = &json[start + key.len()..];
            if let Some(end) = rest.find(']') {
                let arr = &rest[..end];
                return arr
                    .split("},")
                    .filter_map(|item| {
                        let item = item.trim().trim_start_matches('{').trim_end_matches('}');
                        let x =
                            crate::document_core::helpers::json_i32(&format!("{{{}}}", item), "x")?;
                        let y =
                            crate::document_core::helpers::json_i32(&format!("{{{}}}", item), "y")?;
                        Some(crate::model::Point { x, y })
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    // ─── Shape(글상자) API ───────────────────────────────

    /// 커서 위치에 글상자(Rectangle + TextBox)를 삽입한다.
    ///
    /// json: `{"sectionIdx":N,"paraIdx":N,"charOffset":N,"width":N,"height":N,
    ///         "horzOffset":N,"vertOffset":N,"treatAsChar":bool,"textWrap":"Square"}`
    /// 반환: JSON `{"ok":true,"paraIdx":<N>,"controlIdx":0}`
    #[wasm_bindgen(js_name = createShapeControl)]
    pub fn create_shape_control(&mut self, json: &str) -> Result<String, JsValue> {
        let sec = json_u32(json, "sectionIdx").unwrap_or(0) as usize;
        let para = json_u32(json, "paraIdx").unwrap_or(0) as usize;
        let offset = json_u32(json, "charOffset").unwrap_or(0) as usize;
        let width = json_u32(json, "width").unwrap_or(8504);
        let height = json_u32(json, "height").unwrap_or(8504);
        let horz_offset = json_u32(json, "horzOffset").unwrap_or(0);
        let vert_offset = json_u32(json, "vertOffset").unwrap_or(0);
        let shape_type = json_str(json, "shapeType").unwrap_or_else(|| "rectangle".to_string());
        // 글상자는 기본적으로 treat_as_char=true (한컴 기본값)
        let default_tac = shape_type == "textbox";
        let treat_as_char = json_bool(json, "treatAsChar").unwrap_or(default_tac);
        let text_wrap = json_str(json, "textWrap").unwrap_or_else(|| "Square".to_string());
        let line_flip_x = json_bool(json, "lineFlipX").unwrap_or(false);
        let line_flip_y = json_bool(json, "lineFlipY").unwrap_or(false);
        // 다각형 꼭짓점: "polygonPoints":[{"x":N,"y":N},...]
        let polygon_points: Vec<crate::model::Point> = if shape_type == "polygon" {
            Self::parse_polygon_points(json)
        } else {
            Vec::new()
        };
        let result = self.create_shape_control_native(
            sec,
            para,
            offset,
            width,
            height,
            horz_offset,
            vert_offset,
            treat_as_char,
            &text_wrap,
            &shape_type,
            line_flip_x,
            line_flip_y,
            &polygon_points,
        )?;

        // 연결선: SubjectID + 제어점 라우팅 설정 (생성 후)
        if shape_type.starts_with("connector-") {
            let ssid = json_u32(json, "startSubjectID").unwrap_or(0);
            let ssidx = json_u32(json, "startSubjectIndex").unwrap_or(0);
            let esid = json_u32(json, "endSubjectID").unwrap_or(0);
            let esidx = json_u32(json, "endSubjectIndex").unwrap_or(0);
            let pi = json_u32(&result, "paraIdx");
            let ci = json_u32(&result, "controlIdx");
            if let (Some(pi), Some(ci)) = (pi, ci) {
                self.update_connector_subject_ids(
                    sec,
                    pi as usize,
                    ci as usize,
                    ssid,
                    ssidx,
                    esid,
                    esidx,
                );
                self.recalculate_connector_routing(sec, pi as usize, ci as usize, ssidx, esidx);
            }
        }

        Ok(result)
    }


    /// Shape(글상자) 속성을 변경한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = setShapeProperties)]
    pub fn set_shape_properties(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.set_shape_properties_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }

    /// Shape(글상자) 컨트롤을 문단에서 삭제한다.
    ///
    /// 반환: JSON `{"ok":true}`
    #[wasm_bindgen(js_name = deleteShapeControl)]
    pub fn delete_shape_control(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_shape_control_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }

    /// Shape z-order 변경
    /// operation: "front" | "back" | "forward" | "backward"
    #[wasm_bindgen(js_name = changeShapeZOrder)]
    pub fn change_shape_z_order(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        operation: &str,
    ) -> Result<String, JsValue> {
        self.change_shape_z_order_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            operation,
        )
        .map_err(|e| e.into())
    }

    /// 선택된 개체들을 하나의 GroupShape로 묶는다.
    /// json: `{"sectionIdx":N, "targets":[{"paraIdx":N,"controlIdx":N},...]}`
    /// 반환: JSON `{"ok":true, "paraIdx":N, "controlIdx":N}`
    #[wasm_bindgen(js_name = groupShapes)]
    pub fn group_shapes(&mut self, json: &str) -> Result<String, JsValue> {
        let sec = json_u32(json, "sectionIdx").unwrap_or(0) as usize;
        // targets 배열 파싱
        let targets: Vec<(usize, usize)> = {
            let mut result = Vec::new();
            // 간단한 JSON 배열 파싱: "targets":[{"paraIdx":N,"controlIdx":N},...]
            if let Some(start) = json.find("\"targets\"") {
                let rest = &json[start..];
                if let Some(arr_start) = rest.find('[') {
                    if let Some(arr_end) = rest.find(']') {
                        let arr = &rest[arr_start + 1..arr_end];
                        // 각 {} 블록에서 paraIdx, controlIdx 추출
                        let mut pos = 0;
                        while let Some(obj_start) = arr[pos..].find('{') {
                            let obj_start = pos + obj_start;
                            if let Some(obj_end) = arr[obj_start..].find('}') {
                                let obj = &arr[obj_start..obj_start + obj_end + 1];
                                let pi = json_u32(obj, "paraIdx").unwrap_or(0) as usize;
                                let ci = json_u32(obj, "controlIdx").unwrap_or(0) as usize;
                                result.push((pi, ci));
                                pos = obj_start + obj_end + 1;
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
            result
        };
        self.group_shapes_native(sec, &targets)
            .map_err(|e| e.into())
    }

    /// GroupShape를 풀어 자식 개체들을 개별로 복원한다.
    #[wasm_bindgen(js_name = ungroupShape)]
    pub fn ungroup_shape(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.ungroup_shape_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }

    /// 직선 끝점 이동 (글로벌 HWPUNIT 좌표)
    #[wasm_bindgen(js_name = moveLineEndpoint)]
    pub fn move_line_endpoint(
        &mut self,
        sec: u32,
        para: u32,
        ci: u32,
        sx: i32,
        sy: i32,
        ex: i32,
        ey: i32,
    ) -> Result<String, JsValue> {
        self.move_line_endpoint_native(sec as usize, para as usize, ci as usize, sx, sy, ex, ey)
            .map_err(|e| e.into())
    }

    /// `moveLineEndpoint` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sec, para, ci, sx, sy, ex, ey }` (좌표는 i32). positional 과 동일 동작.
    #[wasm_bindgen(js_name = moveLineEndpointEx)]
    pub fn move_line_endpoint_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_i32, json_u32};
        self.move_line_endpoint_native(
            json_u32(options_json, "sec").unwrap_or(0) as usize,
            json_u32(options_json, "para").unwrap_or(0) as usize,
            json_u32(options_json, "ci").unwrap_or(0) as usize,
            json_i32(options_json, "sx").unwrap_or(0),
            json_i32(options_json, "sy").unwrap_or(0),
            json_i32(options_json, "ex").unwrap_or(0),
            json_i32(options_json, "ey").unwrap_or(0),
        )
        .map_err(|e| e.into())
    }

    /// 구역 내 모든 연결선의 좌표를 연결된 도형 위치에 맞게 갱신한다.
    #[wasm_bindgen(js_name = updateConnectorsInSection)]
    pub fn update_connectors_in_section_wasm(&mut self, section_idx: u32) {
        self.update_connectors_in_section(section_idx as usize);
    }

    /// 각주를 삽입한다.
    #[wasm_bindgen(js_name = insertFootnote)]
    pub fn insert_footnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.insert_footnote_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }

    /// 미주를 삽입한다.
    #[wasm_bindgen(js_name = insertEndnote)]
    pub fn insert_endnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.insert_endnote_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// 미주 모양을 적용한다.
    #[wasm_bindgen(js_name = applyEndnoteShape)]
    pub fn apply_endnote_shape(
        &mut self,
        section_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_endnote_shape_native(section_idx as usize, props_json)
            .map_err(|e| e.into())
    }

    /// 수식을 삽입한다.
    #[wasm_bindgen(js_name = insertEquation)]
    pub fn insert_equation(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        script: &str,
        font_size: u32,
        color: u32,
    ) -> Result<String, JsValue> {
        self.insert_equation_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            script,
            font_size,
            color,
        )
        .map_err(|e| e.into())
    }


    /// 본문 각주 컨트롤을 삭제한다.
    #[wasm_bindgen(js_name = deleteFootnote)]
    pub fn delete_footnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        self.delete_footnote_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }

    /// 각주 내 텍스트를 삽입한다.
    #[wasm_bindgen(js_name = insertTextInFootnote)]
    pub fn insert_text_in_footnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
        fn_para_idx: u32,
        char_offset: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.insert_text_in_footnote_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
            fn_para_idx as usize,
            char_offset as usize,
            text,
        )
        .map_err(|e| e.into())
    }

    /// 각주 내 텍스트를 삭제한다.
    #[wasm_bindgen(js_name = deleteTextInFootnote)]
    pub fn delete_text_in_footnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
        fn_para_idx: u32,
        char_offset: u32,
        count: u32,
    ) -> Result<String, JsValue> {
        self.delete_text_in_footnote_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
            fn_para_idx as usize,
            char_offset as usize,
            count as usize,
        )
        .map_err(|e| e.into())
    }

    /// 각주 내 문단을 분할한다 (Enter).
    #[wasm_bindgen(js_name = splitParagraphInFootnote)]
    pub fn split_paragraph_in_footnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
        fn_para_idx: u32,
        char_offset: u32,
        removed_para_meta: Option<String>,
    ) -> Result<String, JsValue> {
        self.split_paragraph_in_footnote_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
            fn_para_idx as usize,
            char_offset as usize,
            parse_removed_para_meta(removed_para_meta)?,
        )
        .map_err(|e| e.into())
    }

    /// 각주 내 문단을 병합한다 (Backspace at start).
    #[wasm_bindgen(js_name = mergeParagraphInFootnote)]
    pub fn merge_paragraph_in_footnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
        fn_para_idx: u32,
    ) -> Result<String, JsValue> {
        self.merge_paragraph_in_footnote_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
            fn_para_idx as usize,
        )
        .map_err(|e| e.into())
    }

    /// 페이지에 각주 영역이 있는지 빠르게 확인 (hitTestFootnote fast-reject).
    /// 페이지네이션 메타데이터만 조회하므로 render tree build가 필요 없다 (#2428).
    #[wasm_bindgen(js_name = pageHasFootnoteFootholds)]
    pub fn page_has_footnote_footholds(&self, page_num: u32) -> bool {
        self.page_has_footnote_footholds_native(page_num)
    }

    /// 각주 영역 히트테스트
    #[wasm_bindgen(js_name = hitTestFootnote)]
    pub fn hit_test_footnote(&self, page_num: u32, x: f64, y: f64) -> Result<String, JsValue> {
        self.hit_test_footnote_native(page_num, x, y)
            .map_err(|e| e.into())
    }

    /// 각주 내부 텍스트 히트테스트
    #[wasm_bindgen(js_name = hitTestInFootnote)]
    pub fn hit_test_in_footnote(&self, page_num: u32, x: f64, y: f64) -> Result<String, JsValue> {
        self.hit_test_in_footnote_native(page_num, x, y)
            .map_err(|e| e.into())
    }


    /// 각주/미주 내부 문단 속성 적용
    #[wasm_bindgen(js_name = applyParaFormatInFootnote)]
    pub fn apply_para_format_in_footnote(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        control_idx: u32,
        fn_para_idx: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_para_format_in_footnote_native(
            section_idx as usize,
            para_idx as usize,
            control_idx as usize,
            fn_para_idx as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }

    /// 본문 인라인 각주 마커 히트테스트
    #[wasm_bindgen(js_name = hitTestBodyFootnoteMarker)]
    pub fn hit_test_body_footnote_marker(
        &self,
        page_num: u32,
        x: f64,
        y: f64,
    ) -> Result<String, JsValue> {
        self.hit_test_body_footnote_marker_native(page_num, x, y)
            .map_err(|e| e.into())
    }

    /// 수직 커서 이동 (ArrowUp/Down) — 단일 호출로 줄/문단/표/구역 경계를 모두 처리한다.
    ///
    /// delta: -1=위, +1=아래
    /// preferred_x: 이전 반환값의 preferredX (최초 이동 시 -1.0 전달)
    /// 셀 컨텍스트: 본문이면 모두 0xFFFFFFFF 전달
    ///
    /// 반환: JSON `{DocumentPosition + CursorRect + preferredX}`
    #[wasm_bindgen(js_name = moveVertical)]
    pub fn move_vertical(
        &self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        delta: i32,
        preferred_x: f64,
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
        self.move_vertical_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            delta,
            preferred_x,
            cell_ctx,
        )
        .map_err(|e| e.into())
    }

    /// `moveVertical` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, paraIdx, charOffset?, delta, preferredX,
    /// parentParaIdx?, controlIdx?, cellIdx?, cellParaIdx? }`. cell 컨텍스트 키가 모두
    /// 생략되면 본문 이동(parentParaIdx=MAX 동작과 동일). positional 과 동일 동작.
    #[wasm_bindgen(js_name = moveVerticalEx)]
    pub fn move_vertical_ex(&self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_f64, json_i32, json_u32};
        // parentParaIdx 부재 시 u32::MAX(본문) — positional 분기와 동일.
        let parent_para_idx = json_u32(options_json, "parentParaIdx").unwrap_or(u32::MAX);
        let cell_ctx = if parent_para_idx == u32::MAX {
            None
        } else {
            Some((
                parent_para_idx as usize,
                json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
                json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
                json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            ))
        };
        self.move_vertical_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "paraIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            json_i32(options_json, "delta").unwrap_or(0),
            json_f64(options_json, "preferredX").unwrap_or(0.0),
            cell_ctx,
        )
        .map_err(|e| e.into())
    }

    // ─── 필드 API (Task 230) ─────────────────────────────────


    /// 문서 누름틀 스키마에서 hwpx-template-engine `TemplateEntityGenerator`와 같은
    /// Java record 데이터 클래스 + 모듈 클래스 초안을 만든다(서버 왕복 없이 클라이언트에서).
    ///
    /// 반환: `{code, packageName, dataClassName, moduleClassName, dataClassSource,
    /// moduleClassSource, errors}` — `errors`가 비어있지 않으면 두 소스는 빈 문자열이다.
    /// 읽기 전용 질의라 문서를 바꾸지 않는다.
    #[wasm_bindgen(js_name = generateTemplateEntity)]
    pub fn generate_template_entity(&self, code: &str, package: &str) -> String {
        self.template_entity_json(code, package)
    }


    /// field_id로 필드 값을 설정한다.
    ///
    /// 반환: `{ok, fieldId, oldValue, newValue}`
    #[wasm_bindgen(js_name = setFieldValue)]
    pub fn set_field_value(&mut self, field_id: u32, value: &str) -> Result<String, JsValue> {
        self.set_field_value_by_id(field_id, value)
            .map_err(|e| e.into())
    }

    /// 필드 이름으로 값을 설정한다.
    ///
    /// 반환: `{ok, fieldId, oldValue, newValue}`
    #[wasm_bindgen(js_name = setFieldValueByName)]
    pub fn set_field_value_by_name_api(
        &mut self,
        name: &str,
        value: &str,
    ) -> Result<String, JsValue> {
        self.set_field_value_by_name(name, value)
            .map_err(|e| e.into())
    }

    /// 현재 본문 위치에 ClickHere 누름틀 필드를 삽입한다.
    #[wasm_bindgen(js_name = insertClickHereField)]
    pub fn insert_click_here_field_api(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        guide: &str,
        memo: &str,
        name: &str,
        editable: bool,
    ) -> Result<String, JsValue> {
        self.insert_click_here_field_at(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            guide,
            memo,
            name,
            editable,
        )
        .map_err(|e| e.into())
    }

    /// `insertClickHereField` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, paraIdx, charOffset?, guide?, memo?, name?, editable? }`.
    /// positional 과 동일 동작.
    #[wasm_bindgen(js_name = insertClickHereFieldEx)]
    pub fn insert_click_here_field_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_str, json_u32};
        self.insert_click_here_field_at(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "paraIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            &json_str(options_json, "guide").unwrap_or_default(),
            &json_str(options_json, "memo").unwrap_or_default(),
            &json_str(options_json, "name").unwrap_or_default(),
            json_bool(options_json, "editable").unwrap_or(false),
        )
        .map_err(|e| e.into())
    }

    /// 현재 셀/글상자 위치에 ClickHere 누름틀 필드를 삽입한다.
    #[wasm_bindgen(js_name = insertClickHereFieldInCell)]
    pub fn insert_click_here_field_in_cell_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        is_textbox: bool,
        guide: &str,
        memo: &str,
        name: &str,
        editable: bool,
    ) -> Result<String, JsValue> {
        self.insert_click_here_field_at_in_cell(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            is_textbox,
            guide,
            memo,
            name,
            editable,
        )
        .map_err(|e| e.into())
    }

    /// `insertClickHereFieldInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, isTextbox?, guide?, memo?, name?, editable? }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = insertClickHereFieldInCellEx)]
    pub fn insert_click_here_field_in_cell_ex(
        &mut self,
        options_json: &str,
    ) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_str, json_u32};
        self.insert_click_here_field_at_in_cell(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            json_bool(options_json, "isTextbox").unwrap_or(false),
            &json_str(options_json, "guide").unwrap_or_default(),
            &json_str(options_json, "memo").unwrap_or_default(),
            &json_str(options_json, "name").unwrap_or_default(),
            json_bool(options_json, "editable").unwrap_or(false),
        )
        .map_err(|e| e.into())
    }

    /// 현재 중첩 표 cellPath 위치에 ClickHere 누름틀 필드를 삽입한다.
    #[wasm_bindgen(js_name = insertClickHereFieldByPath)]
    pub fn insert_click_here_field_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        guide: &str,
        memo: &str,
        name: &str,
        editable: bool,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.insert_click_here_field_at_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
            guide,
            memo,
            name,
            editable,
        )
        .map_err(|e| e.into())
    }

    /// `insertClickHereFieldByPath` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, path: string, charOffset?, guide?,
    /// memo?, name?, editable? }`. `path` 는 cell_path JSON 문자열. positional 과 동일 동작.
    #[wasm_bindgen(js_name = insertClickHereFieldByPathEx)]
    pub fn insert_click_here_field_by_path_ex(
        &mut self,
        options_json: &str,
    ) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_str, json_u32};
        let path_json = json_str(options_json, "path").unwrap_or_default();
        let path = DocumentCore::parse_cell_path(&path_json)?;
        self.insert_click_here_field_at_by_path(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            &path,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            &json_str(options_json, "guide").unwrap_or_default(),
            &json_str(options_json, "memo").unwrap_or_default(),
            &json_str(options_json, "name").unwrap_or_default(),
            json_bool(options_json, "editable").unwrap_or(false),
        )
        .map_err(|e| e.into())
    }

    // ─────────────────────────────────────────────
    // 양식 개체(Form Object) API
    // ─────────────────────────────────────────────


    /// 양식 개체 값을 설정한다.
    ///
    /// value_json: `{"value":1}` 또는 `{"text":"입력값"}`
    /// 반환: `{ok}`
    #[wasm_bindgen(js_name = setFormValue)]
    pub fn set_form_value(
        &mut self,
        sec: u32,
        para: u32,
        ci: u32,
        value_json: &str,
    ) -> Result<String, JsValue> {
        self.core
            .set_form_value_native(sec as usize, para as usize, ci as usize, value_json)
            .map_err(|e| e.into())
    }

    /// 셀 내부 양식 개체 값을 설정한다.
    ///
    /// table_para: 표를 포함한 최상위 문단 인덱스
    /// table_ci: 표 컨트롤 인덱스
    /// cell_idx: 셀 인덱스
    /// cell_para: 셀 내 문단 인덱스
    /// form_ci: 셀 내 양식 컨트롤 인덱스
    /// value_json: `{"value":1}` 또는 `{"text":"입력값"}`
    /// 반환: `{ok}`
    #[wasm_bindgen(js_name = setFormValueInCell)]
    pub fn set_form_value_in_cell(
        &mut self,
        sec: u32,
        table_para: u32,
        table_ci: u32,
        cell_idx: u32,
        cell_para: u32,
        form_ci: u32,
        value_json: &str,
    ) -> Result<String, JsValue> {
        self.core
            .set_form_value_in_cell_native(
                sec as usize,
                table_para as usize,
                table_ci as usize,
                cell_idx as usize,
                cell_para as usize,
                form_ci as usize,
                value_json,
            )
            .map_err(|e| e.into())
    }

    /// `setFormValueInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sec, tablePara, tableCi, cellIdx, cellPara, formCi, value: object }`.
    /// positional 과 동일 동작.
    #[wasm_bindgen(js_name = setFormValueInCellEx)]
    pub fn set_form_value_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_object, json_u32};
        let value_json = json_object(options_json, "value").unwrap_or_else(|| "{}".to_string());
        self.core
            .set_form_value_in_cell_native(
                json_u32(options_json, "sec").unwrap_or(0) as usize,
                json_u32(options_json, "tablePara").unwrap_or(0) as usize,
                json_u32(options_json, "tableCi").unwrap_or(0) as usize,
                json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
                json_u32(options_json, "cellPara").unwrap_or(0) as usize,
                json_u32(options_json, "formCi").unwrap_or(0) as usize,
                &value_json,
            )
            .map_err(|e| e.into())
    }


    // ── 검색/치환 API ──

    /// 문서 텍스트 검색
    ///
    /// [#3865] `include_cells` 를 참으로 주면 표 셀 안의 일반 텍스트 매치도 돌려준다. 그 경우
    /// 결과에 `cellContext`(parentPara·ctrlIdx·cellIdx·cellPara)가 실리므로, 호출자는
    /// 그 좌표로 커서를 옮길 수 있어야 한다. 생략하면 종전대로 본문만 본다.
    #[wasm_bindgen(js_name = searchText)]
    pub fn search_text(
        &self,
        query: &str,
        from_sec: u32,
        from_para: u32,
        from_char: u32,
        forward: bool,
        case_sensitive: bool,
        include_cells: Option<bool>,
    ) -> Result<String, JsValue> {
        self.core
            .search_text_native(
                query,
                from_sec as usize,
                from_para as usize,
                from_char as usize,
                forward,
                case_sensitive,
                // [#3865] 미지정이면 종전 동작(본문만) — 인자를 6개만 넘기던 기존 호출자 무회귀.
                include_cells.unwrap_or(false),
            )
            .map_err(|e| e.into())
    }

    /// 문서 전체 검색 (모든 매치 반환)
    #[wasm_bindgen(js_name = searchAllText)]
    pub fn search_all_text(
        &self,
        query: &str,
        case_sensitive: bool,
        include_cells: bool,
    ) -> Result<String, JsValue> {
        self.core
            .search_all_text_native(query, case_sensitive, include_cells)
            .map_err(|e| e.into())
    }

    /// 텍스트 치환 (단일)
    #[wasm_bindgen(js_name = replaceText)]
    pub fn replace_text(
        &mut self,
        sec: u32,
        para: u32,
        char_offset: u32,
        length: u32,
        new_text: &str,
    ) -> Result<String, JsValue> {
        self.core
            .replace_text_native(
                sec as usize,
                para as usize,
                char_offset as usize,
                length as usize,
                new_text,
            )
            .map_err(|e| e.into())
    }

    /// 단일 치환 (검색어 기반) — 첫 번째 매치만 교체
    #[wasm_bindgen(js_name = replaceOne)]
    pub fn replace_one(
        &mut self,
        query: &str,
        new_text: &str,
        case_sensitive: bool,
    ) -> Result<String, JsValue> {
        self.core
            .replace_one_native(query, new_text, case_sensitive)
            .map_err(|e| e.into())
    }

    /// 전체 치환
    #[wasm_bindgen(js_name = replaceAll)]
    pub fn replace_all(
        &mut self,
        query: &str,
        new_text: &str,
        case_sensitive: bool,
    ) -> Result<String, JsValue> {
        self.core
            .replace_all_native(query, new_text, case_sensitive)
            .map_err(|e| e.into())
    }


    /// 커서 위치의 누름틀 필드를 제거한다 (본문 문단).
    #[wasm_bindgen(js_name = removeFieldAt)]
    pub fn remove_field_at_api(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> String {
        match self.remove_field_at(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        ) {
            Ok(s) => s,
            Err(e) => {
                let escaped = e.to_string().replace('\\', "\\\\").replace('"', "\\\"");
                format!("{{\"ok\":false,\"error\":\"{}\"}}", escaped)
            }
        }
    }

    /// 커서 위치의 누름틀 필드를 제거한다 (셀/글상자 내 문단).
    #[wasm_bindgen(js_name = removeFieldAtInCell)]
    pub fn remove_field_at_in_cell_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        is_textbox: bool,
    ) -> String {
        match self.remove_field_at_in_cell(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            is_textbox,
        ) {
            Ok(s) => s,
            Err(e) => {
                let escaped = e.to_string().replace('\\', "\\\\").replace('"', "\\\"");
                format!("{{\"ok\":false,\"error\":\"{}\"}}", escaped)
            }
        }
    }

    /// `removeFieldAtInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, isTextbox? }`. positional 과 동일 동작(String 반환).
    #[wasm_bindgen(js_name = removeFieldAtInCellEx)]
    pub fn remove_field_at_in_cell_ex(&mut self, options_json: &str) -> String {
        use crate::document_core::helpers::{json_bool, json_u32};
        match self.remove_field_at_in_cell(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            json_bool(options_json, "isTextbox").unwrap_or(false),
        ) {
            Ok(s) => s,
            Err(e) => {
                let escaped = e.to_string().replace('\\', "\\\\").replace('"', "\\\"");
                format!("{{\"ok\":false,\"error\":\"{}\"}}", escaped)
            }
        }
    }

    /// 활성 필드를 설정한다 (본문 문단 — 안내문 숨김용).
    #[wasm_bindgen(js_name = setActiveField)]
    pub fn set_active_field_api(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> bool {
        self.set_active_field(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
    }

    /// 활성 필드를 설정한다 (셀/글상자 내 문단 — 안내문 숨김용).
    /// 변경이 발생하면 true를 반환한다.
    #[wasm_bindgen(js_name = setActiveFieldInCell)]
    pub fn set_active_field_in_cell_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        is_textbox: bool,
    ) -> bool {
        self.set_active_field_in_cell(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            is_textbox,
        )
    }

    /// `setActiveFieldInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, isTextbox? }`. positional 과 동일 동작(bool 반환).
    #[wasm_bindgen(js_name = setActiveFieldInCellEx)]
    pub fn set_active_field_in_cell_ex(&mut self, options_json: &str) -> bool {
        use crate::document_core::helpers::{json_bool, json_u32};
        self.set_active_field_in_cell(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            json_bool(options_json, "isTextbox").unwrap_or(false),
        )
    }


    /// path 기반: 중첩 표 셀 내 활성 필드를 설정한다.
    #[wasm_bindgen(js_name = setActiveFieldByPath)]
    pub fn set_active_field_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
    ) -> bool {
        match DocumentCore::parse_cell_path(path_json) {
            Ok(path) => self.set_active_field_by_path(
                section_idx as usize,
                parent_para_idx as usize,
                &path,
                char_offset as usize,
            ),
            Err(_) => false,
        }
    }

    /// 활성 필드를 해제한다 (안내문 다시 표시).
    #[wasm_bindgen(js_name = clearActiveField)]
    pub fn clear_active_field_api(&mut self) {
        self.clear_active_field();
    }

    // ─── 누름틀 속성 조회/수정 API ──────────────────────────────


    /// ClickHere 필드 속성을 JSON으로 포맷한다.
    fn format_click_here_props(&self, f: &crate::model::control::Field) -> String {
        let guide = f.guide_text().unwrap_or("");
        let memo = f.memo_text().unwrap_or("");
        // 필드 이름: ctrl_data_name → command Name: 키 순서
        let name = f
            .ctrl_data_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| f.extract_wstring_value("Name:"))
            .unwrap_or("");
        let editable = f.is_editable_in_form();
        format!(
            "{{\"ok\":true,\"guide\":\"{}\",\"memo\":\"{}\",\"name\":\"{}\",\"editable\":{}}}",
            json_escape(guide),
            json_escape(memo),
            json_escape(name),
            editable,
        )
    }

    /// 누름틀 필드의 속성을 수정한다.
    ///
    /// 반환: JSON `{"ok":true}` 또는 `{"ok":false}`
    #[wasm_bindgen(js_name = updateClickHereProps)]
    pub fn update_click_here_props(
        &mut self,
        field_id: u32,
        guide: &str,
        memo: &str,
        name: &str,
        editable: bool,
    ) -> String {
        use crate::model::control::{Control, Field, FieldType};

        let new_props_bit = if editable { 1u32 } else { 0u32 };

        // 필드를 찾아 수정하고, ctrl_data_records 바이너리도 갱신
        fn update_field_in_para(
            para: &mut crate::model::paragraph::Paragraph,
            field_id: u32,
            guide: &str,
            memo: &str,
            new_props_bit: u32,
            new_name: &str,
        ) -> bool {
            for (ci, ctrl) in para.controls.iter_mut().enumerate() {
                if let Control::Field(f) = ctrl {
                    if f.field_id == field_id && f.field_type == FieldType::ClickHere {
                        // guide/memo가 원본과 동일하면 command 문자열을 보존한다.
                        // 원본 command에는 trailing space 등이 포함될 수 있으므로
                        // 불필요한 재구축을 피해야 한컴 호환성이 유지된다.
                        let orig_guide = f.guide_text().unwrap_or("").to_string();
                        let orig_memo = f.memo_text().unwrap_or("").to_string();
                        if guide != orig_guide || memo != orig_memo {
                            // guide 또는 memo가 변경되었으므로 command 재구축
                            let new_command = Field::build_clickhere_command(guide, memo);
                            f.command = new_command;
                        }
                        // command가 변경되지 않았으면 원본 보존

                        f.properties = (f.properties & !1) | new_props_bit;
                        f.ctrl_data_name = if new_name.is_empty() {
                            None
                        } else {
                            Some(new_name.to_string())
                        };
                        // ctrl_data_records 바이너리 갱신
                        crate::document_core::queries::field_query::write_ctrl_data_name(
                            &mut para.ctrl_data_records,
                            ci,
                            new_name,
                        );
                        return true;
                    }
                }
            }
            false
        }

        for sec in &mut self.document.sections {
            sec.raw_stream = None;
            for para in &mut sec.paragraphs {
                if update_field_in_para(para, field_id, guide, memo, new_props_bit, name) {
                    self.invalidate_page_tree_cache();
                    return r#"{"ok":true}"#.to_string();
                }
                // 표/글상자 내부
                for ctrl in &mut para.controls {
                    let found = match ctrl {
                        Control::Table(t) => t.cells.iter_mut().any(|c| {
                            c.paragraphs.iter_mut().any(|p| {
                                update_field_in_para(p, field_id, guide, memo, new_props_bit, name)
                            })
                        }),
                        Control::Shape(s) => {
                            if let Some(tb) = s.drawing_mut().and_then(|d| d.text_box.as_mut()) {
                                tb.paragraphs.iter_mut().any(|p| {
                                    update_field_in_para(
                                        p,
                                        field_id,
                                        guide,
                                        memo,
                                        new_props_bit,
                                        name,
                                    )
                                })
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    if found {
                        self.invalidate_page_tree_cache();
                        return r#"{"ok":true}"#.to_string();
                    }
                }
            }
        }
        r#"{"ok":false}"#.to_string()
    }

    /// 커서 좌표(list/para/pos)로 글자 서식을 건다 — 웹한글컨트롤 `Run("CharShape*")`.
    ///
    /// `endPos` 가 문단 길이를 넘으면 끝까지로 자른다. `pos` 는 코드 유닛이다.
    #[wasm_bindgen(js_name = applyCharFormatAtCursor)]
    pub fn apply_char_format_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        start_pos: u32,
        end_pos: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_char_format_at_cursor(
            list_id,
            para_in_list as usize,
            start_pos as usize,
            end_pos as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }

    /// 커서 좌표(list/para/pos)로 글자를 지운다 — 웹한글컨트롤 `Run("Delete*")`.
    ///
    /// `pos` 는 코드 유닛이고, 빈 범위면 아무 일도 하지 않는다.
    #[wasm_bindgen(js_name = deleteAtCursor)]
    pub fn delete_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        start_pos: u32,
        end_pos: u32,
    ) -> Result<String, JsValue> {
        self.delete_at_cursor(
            list_id,
            para_in_list as usize,
            start_pos as usize,
            end_pos as usize,
        )
        .map_err(|e| e.into())
    }

    /// 커서가 든 셀을 기준으로 표를 고친다 — 웹한글컨트롤 `Run("TableInsert*"·"TableDelete*")`.
    ///
    /// `op` 는 `insertRowAbove`·`insertRowBelow`·`insertColLeft`·`insertColRight`·
    /// `deleteRow`·`deleteCol`.
    #[wasm_bindgen(js_name = tableEditAtCursor)]
    pub fn table_edit_at_cursor_api(&mut self, list_id: u32, op: &str) -> Result<String, JsValue> {
        self.table_edit_at_cursor(list_id, op).map_err(|e| e.into())
    }


    /// 컨트롤 하나를 지운다 — 웹한글컨트롤 `DeleteCtrl`.
    #[wasm_bindgen(js_name = deleteControlAt)]
    pub fn delete_control_at_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        control_index: u32,
    ) -> Result<String, JsValue> {
        self.delete_control_at(list_id, para_in_list as usize, control_index as usize)
            .map_err(|e| e.into())
    }

    /// 자동 번호 끼우기 — `InsertPageNum`·`InsertCpNo`·`InsertTpNo`. `kind` 는
    /// `page`·`current`·`total`.
    #[wasm_bindgen(js_name = insertAutoNumberAtCursor)]
    pub fn insert_auto_number_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        pos: u32,
        kind: &str,
    ) -> Result<String, JsValue> {
        self.insert_auto_number_at_cursor(list_id, para_in_list as usize, pos as usize, kind)
            .map_err(|e| e.into())
    }


    /// 나누기 — 웹한글컨트롤 `Run("BreakPage"·"BreakColumn"·"BreakColDef"·"BreakSection")`.
    ///
    /// `kind` 는 `page`·`column`·`colDef`·`section`.
    #[wasm_bindgen(js_name = breakAtCursor)]
    pub fn break_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        pos: u32,
        kind: &str,
    ) -> Result<String, JsValue> {
        self.break_at_cursor(list_id, para_in_list as usize, pos as usize, kind)
            .map_err(|e| e.into())
    }

    /// 개체를 한 걸음 옮긴다 — 웹한글컨트롤 `ShapeObjMove*`(걸음 56 HWPUNIT).
    #[wasm_bindgen(js_name = moveControlAt)]
    pub fn move_control_at_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
        dx: i32,
        dy: i32,
    ) -> Result<String, JsValue> {
        self.move_control_at(para_in_list as usize, control_index as usize, dx, dy)
            .map_err(|e| e.into())
    }

    /// 개체의 앞뒤 순서를 바꾼다 — 웹한글컨트롤 `Run("ShapeObjBringToFront")` 계열.
    ///
    /// `mode` 는 `front`·`back`·`forward`·`backward`·`inFrontOfText`·`behindText`.
    #[wasm_bindgen(js_name = setControlZOrderAt)]
    pub fn set_control_z_order_at_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
        mode: &str,
    ) -> Result<String, JsValue> {
        self.set_control_z_order_at(para_in_list as usize, control_index as usize, mode)
            .map_err(|e| e.into())
    }

    /// 개체를 뒤집는다 — 웹한글컨트롤 `Run("ShapeObjHorzFlip")` 계열.
    #[wasm_bindgen(js_name = setControlFlipAt)]
    pub fn set_control_flip_at_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
        vertical: bool,
        org_state: bool,
    ) -> Result<String, JsValue> {
        self.set_control_flip_at(
            para_in_list as usize,
            control_index as usize,
            vertical,
            org_state,
        )
        .map_err(|e| e.into())
    }

    /// 쪽 하나의 글 — 웹한글컨트롤 `GetPageText`.
    #[wasm_bindgen(js_name = getPageText)]
    pub fn page_text_api(&self, page_index: u32) -> Result<String, JsValue> {
        self.page_text(page_index as usize).map_err(|e| e.into())
    }

    /// 개체 사이를 도는 차례(쪽·z) — 웹한글컨트롤 `Run("ShapeObjNext/PrevObject")` 용.
    #[wasm_bindgen(js_name = getObjectCycle)]
    pub fn object_cycle_api(&self) -> Result<String, JsValue> {
        self.object_cycle_json().map_err(|e| e.into())
    }

    /// 스트림 자리를 글자 번호로 옮긴다 — 글자 번호를 받는 코어 API 에 넘길 때 쓴다.
    #[wasm_bindgen(js_name = getCharIndexAtStreamPos)]
    pub fn char_index_at_api(
        &self,
        list_id: u32,
        para_in_list: u32,
        pos: u32,
    ) -> Result<String, JsValue> {
        self.char_index_at(list_id, para_in_list as usize, pos as usize)
            .map_err(|e| e.into())
    }

    /// 쪽마다 캐럿이 설 수 있는 첫 자리 — 웹한글컨트롤 `Run("MovePage*")` 용.
    #[wasm_bindgen(js_name = getPageCaretStarts)]
    pub fn page_caret_starts_api(&self) -> Result<String, JsValue> {
        self.page_caret_starts().map_err(|e| e.into())
    }

    /// 개체에 글상자를 붙이거나 뗀다 — 웹한글컨트롤 `Run("ShapeObjAttach/DetachTextBox")`.
    #[wasm_bindgen(js_name = setTextBoxAt)]
    pub fn set_text_box_at_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
        attach: bool,
    ) -> Result<String, JsValue> {
        self.set_text_box_at(para_in_list as usize, control_index as usize, attach)
            .map_err(|e| e.into())
    }

    /// 개체에 캡션을 붙인다 — 웹한글컨트롤 `Run("ShapeObjAttachCaption")`.
    #[wasm_bindgen(js_name = attachCaptionAt)]
    pub fn attach_caption_at_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
    ) -> Result<String, JsValue> {
        self.attach_caption_at(para_in_list as usize, control_index as usize)
            .map_err(|e| e.into())
    }

    /// 개체에서 캡션을 뗀다 — 웹한글컨트롤 `Run("ShapeObjDetachCaption")`.
    #[wasm_bindgen(js_name = detachCaptionAt)]
    pub fn detach_caption_at_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
    ) -> Result<String, JsValue> {
        self.detach_caption_at(para_in_list as usize, control_index as usize)
            .map_err(|e| e.into())
    }

    /// 개체 크기를 한 걸음 바꾼다 — 웹한글컨트롤 `ShapeObjResize*`(걸음 283 HWPUNIT).
    #[wasm_bindgen(js_name = resizeControlAt)]
    pub fn resize_control_at_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
        d_width: i32,
        d_height: i32,
    ) -> Result<String, JsValue> {
        self.resize_control_at(
            para_in_list as usize,
            control_index as usize,
            d_width,
            d_height,
        )
        .map_err(|e| e.into())
    }

    /// 개체의 잠금을 켜고 끈다 — 웹한글컨트롤 `ShapeObjLock`·`ShapeObjUnlockAll`.
    ///
    /// 문단·컨트롤 번호에 `u32::MAX` 를 주면 "모두"라는 뜻이다(모두 풀기가 쓴다).
    #[wasm_bindgen(js_name = setControlLock)]
    pub fn set_control_lock_api(
        &mut self,
        para_in_list: u32,
        control_index: u32,
        locked: bool,
    ) -> Result<String, JsValue> {
        let some = |v: u32| (v != u32::MAX).then_some(v as usize);
        self.set_control_lock(some(para_in_list), some(control_index), locked)
            .map_err(|e| e.into())
    }


    /// 커서 자리에서 문단을 가른다 — 웹한글컨트롤 `Run("BreakPara")`.
    #[wasm_bindgen(js_name = splitParaAtCursor)]
    pub fn split_para_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        pos: u32,
    ) -> Result<String, JsValue> {
        self.split_para_at_cursor(list_id, para_in_list as usize, pos as usize)
            .map_err(|e| e.into())
    }

    /// 커서 좌표(list/para/pos)에 글자를 끼운다 — 웹한글컨트롤 `Run("Insert*Space")`.
    #[wasm_bindgen(js_name = insertTextAtCursor)]
    pub fn insert_text_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        pos: u32,
        text: &str,
    ) -> Result<String, JsValue> {
        self.insert_text_at_cursor(list_id, para_in_list as usize, pos as usize, text)
            .map_err(|e| e.into())
    }

    /// 커서가 든 셀에서 `(endRow, endCol)` 까지를 하나로 합친다 — `Run("TableMergeCell")`.
    #[wasm_bindgen(js_name = tableMergeAtCursor)]
    pub fn table_merge_at_cursor_api(
        &mut self,
        list_id: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<String, JsValue> {
        self.table_merge_at_cursor(list_id, end_row as u16, end_col as u16)
            .map_err(|e| e.into())
    }

    /// 셀 블록이 덮은 칸들의 글을 비운다 — `Run("TableDeleteCell")`. 규약은 merge 와 같다.
    #[wasm_bindgen(js_name = clearTableCellsAtCursor)]
    pub fn clear_table_cells_at_cursor_api(
        &mut self,
        list_id: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<String, JsValue> {
        self.clear_table_cells_at_cursor(list_id, end_row as u16, end_col as u16)
            .map_err(|e| e.into())
    }

    /// 커서 좌표(list/para)로 문단 서식을 건다 — 웹한글컨트롤 `Run("ParagraphShape*")`.
    #[wasm_bindgen(js_name = applyParaFormatAtCursor)]
    pub fn apply_para_format_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_para_format_at_cursor(list_id, para_in_list as usize, props_json)
            .map_err(|e| e.into())
    }


    /// 아무 내용도 없는 빈 문서인가 — 웹한글컨트롤 `IsEmpty`(§8.2.7).
    #[wasm_bindgen(js_name = isEmptyDocument)]
    pub fn is_empty_document_api(&self) -> bool {
        self.is_empty_document()
    }

    /// 한글 커서 좌표(list/para/pos)에 누름틀을 넣는다 — 웹한글컨트롤 `CreateField`.
    ///
    /// `pos` 는 코드 유닛이다(확장 컨트롤 하나가 8칸). 글자 번호를 받는
    /// `insertClickHereField` 와 좌표계가 다르다.
    #[wasm_bindgen(js_name = insertClickHereFieldAtCursor)]
    pub fn insert_click_here_field_at_cursor_api(
        &mut self,
        list_id: u32,
        para_in_list: u32,
        pos: u32,
        guide: &str,
        memo: &str,
        name: &str,
        editable: bool,
    ) -> Result<String, JsValue> {
        self.insert_click_here_field_at_cursor(
            list_id,
            para_in_list as usize,
            pos as usize,
            guide,
            memo,
            name,
            editable,
        )
        .map_err(|e| e.into())
    }


    /// 필드 이름을 바꾼다 — 누름틀과 셀 필드를 모두 다룬다.
    ///
    /// `updateClickHereProps` 는 누름틀 전용이라 셀 필드에서 `{"ok":false}` 를 돌려준다.
    /// 웹한글컨트롤 `RenameField`(§8.3.36)의 계약은 두 갈래를 가리지 않는다.
    ///
    /// 반환: JSON `{"ok":true,"renamed":N}` / `{"ok":false,"renamed":0}`
    #[wasm_bindgen(js_name = renameField)]
    pub fn rename_field_api(&mut self, oldname: &str, newname: &str) -> String {
        match self.rename_field_by_name(oldname, newname) {
            Ok(json) => json,
            Err(_) => r#"{"ok":false,"renamed":0}"#.to_string(),
        }
    }

    // ─── 경로 기반 중첩 표 API ───────────────────────────────


    /// 경로 기반 수직 커서 이동 (중첩 표용).
    ///
    /// 반환: JSON `{DocumentPosition + CursorRect + preferredX}`
    #[wasm_bindgen(js_name = moveVerticalByPath)]
    pub fn move_vertical_by_path(
        &self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        delta: i32,
        preferred_x: f64,
    ) -> Result<String, JsValue> {
        self.move_vertical_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            path_json,
            char_offset as usize,
            delta,
            preferred_x,
        )
        .map_err(|e| e.into())
    }

    // ─── Phase 4: Selection API ──────────────────────────────


    /// 본문 선택 영역을 삭제한다.
    ///
    /// 반환: JSON `{"ok":true,"paraIdx":N,"charOffset":N}`
    #[wasm_bindgen(js_name = deleteRange)]
    pub fn delete_range(
        &mut self,
        section_idx: u32,
        start_para_idx: u32,
        start_char_offset: u32,
        end_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.delete_range_native(
            section_idx as usize,
            start_para_idx as usize,
            start_char_offset as usize,
            end_para_idx as usize,
            end_char_offset as usize,
            None,
        )
        .map_err(|e| e.into())
    }

    /// 셀 내 선택 영역을 삭제한다.
    ///
    /// 반환: JSON `{"ok":true,"paraIdx":N,"charOffset":N}`
    #[wasm_bindgen(js_name = deleteRangeInCell)]
    pub fn delete_range_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        start_cell_para_idx: u32,
        start_char_offset: u32,
        end_cell_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.delete_range_native(
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
        )
        .map_err(|e| e.into())
    }

    /// `deleteRangeInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, startCellParaIdx,
    /// startCharOffset, endCellParaIdx, endCharOffset }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = deleteRangeInCellEx)]
    pub fn delete_range_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.delete_range_native(
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
        )
        .map_err(|e| e.into())
    }

    // ─── Phase 4 끝 ─────────────────────────────────────────

    // ─── Phase 3 끝 ─────────────────────────────────────────

    // ─── Phase 2 끝 ─────────────────────────────────────────


    /// 사용자 명시 요청에 의한 lineseg 전체 reflow (#177).
    ///
    /// `reflow_zero_height_paragraphs` 의 자동 경로와 달리, "빈 line_segs + text 존재"
    /// 케이스까지 포함해 재계산한다. 반환값은 실제로 reflow 된 문단 개수.
    ///
    /// 호출 이후 렌더 캐시·페이지네이션이 갱신되므로 즉시 렌더링하면 보정된 결과가 보인다.
    #[wasm_bindgen(js_name = reflowLinesegs)]
    pub fn reflow_linesegs(&mut self) -> usize {
        self.core.reflow_linesegs_on_demand()
    }

    /// 배포용(읽기전용) 문서를 편집 가능한 일반 문서로 변환한다.
    ///
    /// 반환값: JSON `{"ok":true,"converted":true}` 또는 `{"ok":true,"converted":false}`
    #[wasm_bindgen(js_name = convertToEditable)]
    pub fn convert_to_editable(&mut self) -> Result<String, JsValue> {
        self.convert_to_editable_native().map_err(|e| e.into())
    }

    /// Batch 모드를 시작한다. 이후 Command 호출 시 paginate()를 건너뛴다.
    #[wasm_bindgen(js_name = beginBatch)]
    pub fn begin_batch(&mut self) -> Result<String, JsValue> {
        self.begin_batch_native().map_err(|e| e.into())
    }

    /// Batch 모드를 종료하고 누적된 이벤트를 반환한다.
    #[wasm_bindgen(js_name = endBatch)]
    pub fn end_batch(&mut self) -> Result<String, JsValue> {
        self.end_batch_native().map_err(|e| e.into())
    }


    // ─── Undo/Redo 스냅샷 API ──────────────────────────

    /// Document 스냅샷을 저장하고 ID를 반환한다.
    #[wasm_bindgen(js_name = saveSnapshot)]
    pub fn save_snapshot(&mut self) -> u32 {
        self.save_snapshot_native()
    }

    /// 지정 ID의 스냅샷으로 Document를 복원한다.
    #[wasm_bindgen(js_name = restoreSnapshot)]
    pub fn restore_snapshot(&mut self, id: u32) -> Result<String, JsValue> {
        self.restore_snapshot_native(id).map_err(|e| e.into())
    }

    /// 지정 ID의 스냅샷을 제거하여 메모리를 해제한다.
    #[wasm_bindgen(js_name = discardSnapshot)]
    pub fn discard_snapshot(&mut self, id: u32) {
        self.discard_snapshot_native(id)
    }


    /// 스타일의 실효 ParaShape ID를 찾는다.
    /// 스타일 정의의 ParaShape에 번호 정보가 없으면, 이 스타일을 사용하는 문단에서 조회한다.
    fn find_effective_para_shape_for_style(&self, style_id: u32, base_psid: u16) -> u16 {
        use crate::model::style::HeadType;
        // 기본 ParaShape에 이미 번호 정보가 있으면 그대로 사용
        if let Some(ps) = self
            .core
            .document
            .doc_info
            .para_shapes
            .get(base_psid as usize)
        {
            if ps.head_type != HeadType::None {
                return base_psid;
            }
        }
        // 이 스타일을 사용하는 첫 번째 문단의 para_shape_id에서 번호 정보 탐색
        let sid = style_id as u8;
        for section in &self.core.document.sections {
            for para in &section.paragraphs {
                if para.style_id == sid {
                    if let Some(ps) = self
                        .core
                        .document
                        .doc_info
                        .para_shapes
                        .get(para.para_shape_id as usize)
                    {
                        if ps.head_type != HeadType::None {
                            return para.para_shape_id;
                        }
                    }
                }
            }
        }
        base_psid
    }

    /// 스타일의 메타 정보(이름/영문이름/nextStyleId)를 수정한다.
    ///
    /// json: {"name":"...", "englishName":"...", "nextStyleId":0}
    #[wasm_bindgen(js_name = updateStyle)]
    pub fn update_style(&mut self, style_id: u32, json: &str) -> bool {
        use crate::document_core::helpers::json_i32;
        let styles = &mut self.core.document.doc_info.styles;
        let style = match styles.get_mut(style_id as usize) {
            Some(s) => s,
            None => return false,
        };
        // 이름 파싱
        if let Some(name) = crate::document_core::helpers::json_str(json, "name") {
            style.local_name = name;
        }
        if let Some(en) = crate::document_core::helpers::json_str(json, "englishName") {
            style.english_name = en;
        }
        if let Some(v) = json_i32(json, "nextStyleId") {
            style.next_style_id = v as u8;
        }
        // raw_data 무효화 (수정됨)
        style.raw_data = None;
        // DocInfo 스트림 무효화. serialize_doc_info 는 raw_stream_dirty 가 false 이면
        // 원본 스트림을 그대로 반환하고(레코드 raw_data 는 그 이전에 단락됨), 이름/nextStyleId
        // 변경이 .hwp 저장에서 유실된다. 형제 update_style_shapes 는 이미 이 플래그를 세운다.
        self.core.document.doc_info.raw_stream_dirty = true;
        true
    }

    /// 스타일의 CharShape/ParaShape를 수정한다.
    ///
    /// charMods/paraMods는 기존 parse_char_shape_mods/parse_para_shape_mods와 동일한 JSON 형식
    #[wasm_bindgen(js_name = updateStyleShapes)]
    pub fn update_style_shapes(
        &mut self,
        style_id: u32,
        char_mods_json: &str,
        para_mods_json: &str,
    ) -> bool {
        let styles = &self.core.document.doc_info.styles;
        let style = match styles.get(style_id as usize) {
            Some(s) => s.clone(),
            None => return false,
        };
        let old_csid = style.char_shape_id as u32;
        let old_psid = style.para_shape_id;
        let style_type = style.style_type;

        // CharShape 수정
        if !char_mods_json.is_empty() && char_mods_json != "{}" {
            let char_mods = crate::document_core::helpers::parse_char_shape_mods(char_mods_json);
            if let Some(cs) = self
                .core
                .document
                .doc_info
                .char_shapes
                .get(style.char_shape_id as usize)
            {
                let new_cs = char_mods.apply_to(cs);
                // 새 CharShape를 추가하고 스타일에 연결
                self.core.document.doc_info.char_shapes.push(new_cs);
                let new_id = (self.core.document.doc_info.char_shapes.len() - 1) as u16;
                self.core.document.doc_info.styles[style_id as usize].char_shape_id = new_id;
            }
        }

        // ParaShape 수정
        if !para_mods_json.is_empty() && para_mods_json != "{}" {
            let para_mods = crate::document_core::helpers::parse_para_shape_mods(para_mods_json);
            if let Some(ps) = self
                .core
                .document
                .doc_info
                .para_shapes
                .get(style.para_shape_id as usize)
            {
                let new_ps = para_mods.apply_to(ps);
                self.core.document.doc_info.para_shapes.push(new_ps);
                let new_id = (self.core.document.doc_info.para_shapes.len() - 1) as u16;
                self.core.document.doc_info.styles[style_id as usize].para_shape_id = new_id;
            }
        }

        // raw_data 무효화
        self.core.document.doc_info.styles[style_id as usize].raw_data = None;
        self.core.document.doc_info.raw_stream_dirty = true;

        let sid = style_id as u8;
        let mut body_targets = Vec::new();
        let mut cell_targets = Vec::new();
        for (sec_idx, section) in self.core.document.sections.iter().enumerate() {
            for (para_idx, para) in section.paragraphs.iter().enumerate() {
                if para.style_id == sid {
                    body_targets.push((sec_idx, para_idx));
                }
                for (control_idx, ctrl) in para.controls.iter().enumerate() {
                    if let Control::Table(table) = ctrl {
                        for (cell_idx, cell) in table.cells.iter().enumerate() {
                            for (cell_para_idx, cpara) in cell.paragraphs.iter().enumerate() {
                                if cpara.style_id == sid {
                                    cell_targets.push((
                                        sec_idx,
                                        para_idx,
                                        control_idx,
                                        cell_idx,
                                        cell_para_idx,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── 스타일 변경을 해당 스타일을 사용하는 모든 문단에 전파 ──
        let updated_style = self.core.document.doc_info.styles[style_id as usize].clone();
        let new_csid = updated_style.char_shape_id as u32;
        let new_psid = updated_style.para_shape_id;

        for (sec_idx, para_idx) in body_targets {
            if let Some(para) = self
                .core
                .document
                .sections
                .get_mut(sec_idx)
                .and_then(|s| s.paragraphs.get_mut(para_idx))
            {
                if style_type == 0 && para.para_shape_id == old_psid {
                    para.para_shape_id = new_psid;
                }
                para.replace_style_char_shape_preserving_overrides(old_csid, new_csid);
            }
            self.core.reflow_body_paragraph(sec_idx, para_idx);
            if let Some(section) = self.core.document.sections.get_mut(sec_idx) {
                section.raw_stream = None;
            }
        }

        for (sec_idx, para_idx, control_idx, cell_idx, cell_para_idx) in cell_targets {
            if let Ok(cpara) = self.core.get_cell_paragraph_mut(
                sec_idx,
                para_idx,
                control_idx,
                cell_idx,
                cell_para_idx,
            ) {
                if style_type == 0 && cpara.para_shape_id == old_psid {
                    cpara.para_shape_id = new_psid;
                }
                cpara.replace_style_char_shape_preserving_overrides(old_csid, new_csid);
            }
            self.core.reflow_cell_paragraph(
                sec_idx,
                para_idx,
                control_idx,
                cell_idx,
                cell_para_idx,
            );
            self.core
                .mark_cell_control_dirty(sec_idx, para_idx, control_idx);
            if let Some(section) = self.core.document.sections.get_mut(sec_idx) {
                section.raw_stream = None;
            }
        }

        // 스타일 캐시 무효화 + 전체 리빌드
        let num_sections = self.core.document.sections.len();
        for sec_idx in 0..num_sections {
            self.core.rebuild_section(sec_idx);
        }
        true
    }

    /// 새 스타일을 생성한다.
    ///
    /// json: {"name":"...", "englishName":"...", "type":0, "nextStyleId":0}
    /// 반환값: 새 스타일 ID (0-based)
    #[wasm_bindgen(js_name = createStyle)]
    pub fn create_style(&mut self, json: &str) -> i32 {
        use crate::document_core::helpers::{json_i32, json_str};
        use crate::model::style::Style;

        let name = json_str(json, "name").unwrap_or_default();
        let english_name = json_str(json, "englishName").unwrap_or_default();
        let style_type = json_i32(json, "type").unwrap_or(0) as u8;
        let next_style_id = json_i32(json, "nextStyleId").unwrap_or(0) as u8;

        // 한컴 스타일 추가 흐름은 현재 문단의 모양을 기본값으로 삼는다.
        // 호출자가 base ID를 넘기지 않으면 기존 호환성을 위해 바탕글을 사용한다.
        let base_style = self.core.document.doc_info.styles.first();
        let (fallback_char_shape_id, fallback_para_shape_id) = match base_style {
            Some(s) => (s.char_shape_id, s.para_shape_id),
            None => (0, 0),
        };
        let char_shape_id = json_i32(json, "baseCharShapeId")
            .filter(|id| *id >= 0)
            .map(|id| id as u16)
            .filter(|id| (*id as usize) < self.core.document.doc_info.char_shapes.len())
            .unwrap_or(fallback_char_shape_id);
        let para_shape_id = json_i32(json, "baseParaShapeId")
            .filter(|id| *id >= 0)
            .map(|id| id as u16)
            .filter(|id| (*id as usize) < self.core.document.doc_info.para_shapes.len())
            .unwrap_or(fallback_para_shape_id);

        let new_style = Style {
            raw_data: None,
            local_name: name,
            english_name,
            style_type,
            next_style_id,
            lang_id: 1042, // 한국어 default (HWP5 spec 표 47)
            para_shape_id,
            char_shape_id,
            lock_form: false,
        };
        self.core.document.doc_info.styles.push(new_style);
        self.core.document.doc_info.raw_stream_dirty = true;
        let new_id = (self.core.document.doc_info.styles.len() - 1) as i32;
        // 스타일 캐시 갱신
        self.core.styles = crate::renderer::style_resolver::resolve_styles(
            &self.core.document.doc_info,
            self.core.dpi,
        );
        new_id
    }

    /// 스타일을 삭제한다.
    ///
    /// 바탕글(ID 0)은 삭제할 수 없다.
    /// 삭제된 스타일을 사용 중인 문단은 바탕글(ID 0)로 변경된다.
    #[wasm_bindgen(js_name = deleteStyle)]
    pub fn delete_style(&mut self, style_id: u32) -> bool {
        if style_id == 0 {
            return false; // 바탕글은 삭제 불가
        }
        let styles = &self.core.document.doc_info.styles;
        if style_id as usize >= styles.len() {
            return false;
        }
        let sid = style_id as u8;
        // 해당 스타일을 사용 중인 문단을 바탕글(0)로 변경
        for section in &mut self.core.document.sections {
            for para in &mut section.paragraphs {
                if para.style_id == sid {
                    para.style_id = 0;
                }
            }
        }
        // 스타일 삭제 (인덱스 기반이므로 뒤의 ID가 변경됨에 주의)
        self.core.document.doc_info.styles.remove(style_id as usize);
        // 삭제된 ID보다 큰 style_id를 가진 문단들 보정
        for section in &mut self.core.document.sections {
            for para in &mut section.paragraphs {
                if para.style_id > sid {
                    para.style_id -= 1;
                }
            }
        }
        // next_style_id 보정
        for s in &mut self.core.document.doc_info.styles {
            if s.next_style_id == sid {
                s.next_style_id = 0;
            } else if s.next_style_id > sid {
                s.next_style_id -= 1;
            }
        }
        // 스타일 캐시 갱신
        self.core.styles = crate::renderer::style_resolver::resolve_styles(
            &self.core.document.doc_info,
            self.core.dpi,
        );
        // DocInfo(styles 목록)와 문단 style_id 가 함께 바뀌었으므로 저장 스트림을 무효화한다.
        // raw_stream_dirty 미설정 시 DocInfo 가, 섹션 raw_stream 잔존 시 본문이 각각 원본
        // 바이트로 재방출돼 스타일 삭제·문단 재배정이 .hwp 저장에서 유실된다.
        self.core.document.doc_info.raw_stream_dirty = true;
        for section in &mut self.core.document.sections {
            section.raw_stream = None;
        }
        true
    }


    /// 문서에 기본 문단 번호 정의가 없으면 생성한다.
    ///
    /// 반환값: Numbering ID (1-based)
    #[wasm_bindgen(js_name = ensureDefaultNumbering)]
    pub fn ensure_default_numbering(&mut self) -> u16 {
        let numberings = &self.core.document.doc_info.numberings;
        if !numberings.is_empty() {
            return 1; // 이미 있으면 첫 번째 반환
        }
        // 기본 7수준 번호 형식 생성 (한컴 기본 패턴)
        use crate::model::style::{Numbering, NumberingHead};
        let mut n = Numbering::default();
        n.level_formats = [
            "^1.".to_string(), // 1.
            "^2)".to_string(), // 가)
            "^3)".to_string(), // (1)
            "^4)".to_string(), // (가)
            "^5)".to_string(), // ①
            "^6)".to_string(), // ㄱ)
            "^7)".to_string(), // a)
        ];
        n.start_number = 1;
        n.level_start_numbers = [1; 7];
        // 수준별 번호 형식 코드 설정
        n.heads[0] = NumberingHead {
            number_format: 0,
            ..Default::default()
        }; // 1,2,3
        n.heads[1] = NumberingHead {
            number_format: 8,
            ..Default::default()
        }; // 가,나,다
        n.heads[2] = NumberingHead {
            number_format: 0,
            ..Default::default()
        }; // 1,2,3
        n.heads[3] = NumberingHead {
            number_format: 8,
            ..Default::default()
        }; // 가,나,다
        n.heads[4] = NumberingHead {
            number_format: 1,
            ..Default::default()
        }; // ①②③
        n.heads[5] = NumberingHead {
            number_format: 10,
            ..Default::default()
        }; // ㄱ,ㄴ,ㄷ
        n.heads[6] = NumberingHead {
            number_format: 5,
            ..Default::default()
        }; // a,b,c
        self.core.document.doc_info.numberings.push(n);
        1
    }

    /// JSON으로 지정된 번호 형식으로 Numbering 정의를 생성한다.
    ///
    /// json: {"levelFormats":["^1.","^2)",...],"numberFormats":[0,8,...],"startNumber":1}
    /// 반환값: Numbering ID (1-based)
    #[wasm_bindgen(js_name = createNumbering)]
    pub fn create_numbering(&mut self, json: &str) -> u16 {
        use crate::document_core::helpers::json_i32;
        use crate::model::style::{Numbering, NumberingHead};

        let mut n = Numbering::default();

        // levelFormats 배열 파싱
        if let Some(arr_start) = json.find("\"levelFormats\"") {
            let rest = &json[arr_start..];
            if let Some(bracket_start) = rest.find('[') {
                if let Some(bracket_end) = rest[bracket_start..].find(']') {
                    let arr_str = &rest[bracket_start + 1..bracket_start + bracket_end];
                    let mut level = 0;
                    for part in arr_str.split(',') {
                        if level >= 7 {
                            break;
                        }
                        let trimmed = part.trim().trim_matches('"');
                        if !trimmed.is_empty() {
                            n.level_formats[level] = trimmed.to_string();
                            level += 1;
                        }
                    }
                }
            }
        }

        // numberFormats 배열 파싱
        if let Some(arr_start) = json.find("\"numberFormats\"") {
            let rest = &json[arr_start..];
            if let Some(bracket_start) = rest.find('[') {
                if let Some(bracket_end) = rest[bracket_start..].find(']') {
                    let arr_str = &rest[bracket_start + 1..bracket_start + bracket_end];
                    let mut level = 0;
                    for part in arr_str.split(',') {
                        if level >= 7 {
                            break;
                        }
                        if let Ok(code) = part.trim().parse::<u8>() {
                            n.heads[level] = NumberingHead {
                                number_format: code,
                                ..Default::default()
                            };
                            level += 1;
                        }
                    }
                }
            }
        }

        n.start_number = json_i32(json, "startNumber").unwrap_or(1) as u16;
        n.level_start_numbers = [n.start_number as u32; 7];
        self.core.document.doc_info.numberings.push(n);
        self.core.document.doc_info.numberings.len() as u16
    }

    /// 특정 문자의 글머리표 정의가 없으면 생성한다.
    ///
    /// 반환값: Bullet ID (1-based)
    #[wasm_bindgen(js_name = ensureDefaultBullet)]
    pub fn ensure_default_bullet(&mut self, bullet_char_str: &str) -> u16 {
        let bullet_ch = bullet_char_str.chars().next().unwrap_or('●');
        // 이미 해당 문자의 Bullet이 있는지 검색
        let bullets = &self.core.document.doc_info.bullets;
        for (i, b) in bullets.iter().enumerate() {
            let mapped = crate::renderer::layout::map_pua_bullet_char(b.bullet_char);
            if mapped == bullet_ch {
                return (i + 1) as u16;
            }
        }
        // 없으면 새로 생성
        use crate::model::style::Bullet;
        let b = Bullet {
            bullet_char: bullet_ch,
            text_distance: 50,
            ..Default::default()
        };
        self.core.document.doc_info.bullets.push(b);
        self.core.document.doc_info.bullets.len() as u16
    }


    /// 스타일을 적용한다 (본문 문단).
    #[wasm_bindgen(js_name = applyStyle)]
    pub fn apply_style(
        &mut self,
        sec_idx: u32,
        para_idx: u32,
        style_id: u32,
    ) -> Result<String, JsValue> {
        self.core
            .apply_style_native(sec_idx as usize, para_idx as usize, style_id as usize)
            .map_err(|e| e.into())
    }

    /// 스타일을 적용한다 (셀 내 문단).
    #[wasm_bindgen(js_name = applyCellStyle)]
    pub fn apply_cell_style(
        &mut self,
        sec_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        style_id: u32,
    ) -> Result<String, JsValue> {
        self.core
            .apply_cell_style_native(
                sec_idx as usize,
                parent_para_idx as usize,
                control_idx as usize,
                cell_idx as usize,
                cell_para_idx as usize,
                style_id as usize,
            )
            .map_err(|e| e.into())
    }

    /// 표 셀에서 계산식을 실행한다.
    ///
    /// formula: "=SUM(A1:A5)", "=A1+B2*3" 등
    /// write_result: true이면 결과를 셀에 기록
    #[wasm_bindgen(js_name = evaluateTableFormula)]
    pub fn evaluate_table_formula(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        target_row: u32,
        target_col: u32,
        formula: &str,
        write_result: bool,
    ) -> Result<String, JsValue> {
        self.core
            .evaluate_table_formula(
                section_idx as usize,
                parent_para_idx as usize,
                control_idx as usize,
                target_row as usize,
                target_col as usize,
                formula,
                write_result,
            )
            .map_err(|e| e.into())
    }

    /// `evaluateTableFormula` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, targetRow, targetCol,
    /// formula: string, writeResult? }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = evaluateTableFormulaEx)]
    pub fn evaluate_table_formula_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_str, json_u32};
        self.core
            .evaluate_table_formula(
                json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
                json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
                json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
                json_u32(options_json, "targetRow").unwrap_or(0) as usize,
                json_u32(options_json, "targetCol").unwrap_or(0) as usize,
                &json_str(options_json, "formula").unwrap_or_default(),
                json_bool(options_json, "writeResult").unwrap_or(false),
            )
            .map_err(|e| e.into())
    }

    /// 글꼴 이름으로 font_id를 조회하거나 새로 생성한다.
    ///
    /// 한글(0번) 카테고리에서 이름 검색 → 없으면 7개 전체 카테고리에 신규 등록.
    /// 반환값: font_id (u16), 실패 시 -1
    #[wasm_bindgen(js_name = findOrCreateFontId)]
    pub fn find_or_create_font_id(&mut self, name: &str) -> i32 {
        self.find_or_create_font_id_native(name)
    }

    /// 특정 언어 카테고리에서 글꼴 이름으로 ID를 찾거나 등록한다.
    #[wasm_bindgen(js_name = findOrCreateFontIdForLang)]
    pub fn wasm_find_or_create_font_id_for_lang(&mut self, lang: u32, name: &str) -> i32 {
        self.core
            .find_or_create_font_id_for_lang(lang as usize, name)
    }

    /// 글자 서식을 적용한다 (본문 문단).
    #[wasm_bindgen(js_name = applyCharFormat)]
    pub fn apply_char_format(
        &mut self,
        sec_idx: usize,
        para_idx: usize,
        start_offset: usize,
        end_offset: usize,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_char_format_native(sec_idx, para_idx, start_offset, end_offset, props_json)
            .map_err(|e| e.into())
    }

    /// 글자 서식 ID를 직접 복원한다 (본문 문단).
    #[wasm_bindgen(js_name = setCharShapeId)]
    pub fn set_char_shape_id(
        &mut self,
        sec_idx: usize,
        para_idx: usize,
        start_offset: usize,
        end_offset: usize,
        char_shape_id: u32,
    ) -> Result<String, JsValue> {
        self.set_char_shape_id_native(sec_idx, para_idx, start_offset, end_offset, char_shape_id)
            .map_err(|e| e.into())
    }

    /// 글자 서식을 적용한다 (셀 내 문단).
    #[wasm_bindgen(js_name = applyCharFormatInCell)]
    pub fn apply_char_format_in_cell(
        &mut self,
        sec_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
        cell_idx: usize,
        cell_para_idx: usize,
        start_offset: usize,
        end_offset: usize,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_char_format_in_cell_native(
            sec_idx,
            parent_para_idx,
            control_idx,
            cell_idx,
            cell_para_idx,
            start_offset,
            end_offset,
            props_json,
        )
        .map_err(|e| e.into())
    }

    /// `applyCharFormatInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ secIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// startOffset, endOffset, props: object }`. `props` 는 글자 서식 JSON 객체(positional
    /// 의 props_json 과 동일). positional 과 동일 동작.
    #[wasm_bindgen(js_name = applyCharFormatInCellEx)]
    pub fn apply_char_format_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_object, json_u32};
        let props_json = json_object(options_json, "props").unwrap_or_else(|| "{}".to_string());
        self.apply_char_format_in_cell_native(
            json_u32(options_json, "secIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startOffset").unwrap_or(0) as usize,
            json_u32(options_json, "endOffset").unwrap_or(0) as usize,
            &props_json,
        )
        .map_err(|e| e.into())
    }

    #[wasm_bindgen(js_name = applyCharFormatInCellByPath)]
    #[allow(clippy::too_many_arguments)]
    pub fn apply_char_format_in_cell_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        start_offset: u32,
        end_offset: u32,
        props_json: &str,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.apply_char_format_in_cell_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            start_offset as usize,
            end_offset as usize,
            props_json,
        )
        .map_err(|e| e.into())
    }


    #[wasm_bindgen(js_name = setCharShapeIdInCellByPath)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_char_shape_id_in_cell_by_path_api(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        start_offset: u32,
        end_offset: u32,
        char_shape_id: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.set_char_shape_id_in_cell_by_path(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            start_offset as usize,
            end_offset as usize,
            char_shape_id,
        )
        .map_err(|e| e.into())
    }

    /// 글자 서식 ID를 직접 복원한다 (셀 내 문단).
    #[wasm_bindgen(js_name = setCharShapeIdInCell)]
    pub fn set_char_shape_id_in_cell(
        &mut self,
        sec_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
        cell_idx: usize,
        cell_para_idx: usize,
        start_offset: usize,
        end_offset: usize,
        char_shape_id: u32,
    ) -> Result<String, JsValue> {
        self.set_char_shape_id_in_cell_native(
            sec_idx,
            parent_para_idx,
            control_idx,
            cell_idx,
            cell_para_idx,
            start_offset,
            end_offset,
            char_shape_id,
        )
        .map_err(|e| e.into())
    }

    /// `setCharShapeIdInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ secIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// startOffset, endOffset, charShapeId }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = setCharShapeIdInCellEx)]
    pub fn set_char_shape_id_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.set_char_shape_id_in_cell_native(
            json_u32(options_json, "secIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "startOffset").unwrap_or(0) as usize,
            json_u32(options_json, "endOffset").unwrap_or(0) as usize,
            json_u32(options_json, "charShapeId").unwrap_or(0),
        )
        .map_err(|e| e.into())
    }

    /// 감추기 설정
    #[wasm_bindgen(js_name = setPageHide)]
    pub fn set_page_hide(
        &mut self,
        sec: u32,
        para: u32,
        hide_header: bool,
        hide_footer: bool,
        hide_master: bool,
        hide_border: bool,
        hide_fill: bool,
        hide_page_num: bool,
    ) -> Result<String, JsValue> {
        self.set_page_hide_native(
            sec as usize,
            para as usize,
            hide_header,
            hide_footer,
            hide_master,
            hide_border,
            hide_fill,
            hide_page_num,
        )
        .map_err(|e| e.into())
    }

    /// `setPageHide` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sec, para, hideHeader?, hideFooter?, hideMaster?, hideBorder?,
    /// hideFill?, hidePageNum? }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = setPageHideEx)]
    pub fn set_page_hide_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_bool, json_u32};
        self.set_page_hide_native(
            json_u32(options_json, "sec").unwrap_or(0) as usize,
            json_u32(options_json, "para").unwrap_or(0) as usize,
            json_bool(options_json, "hideHeader").unwrap_or(false),
            json_bool(options_json, "hideFooter").unwrap_or(false),
            json_bool(options_json, "hideMaster").unwrap_or(false),
            json_bool(options_json, "hideBorder").unwrap_or(false),
            json_bool(options_json, "hideFill").unwrap_or(false),
            json_bool(options_json, "hidePageNum").unwrap_or(false),
        )
        .map_err(|e| e.into())
    }


    /// 문단 서식을 적용한다 (본문 문단).
    /// 문단 번호 시작 방식 설정
    #[wasm_bindgen(js_name = setNumberingRestart)]
    pub fn set_numbering_restart(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        mode: u8,
        start_num: u32,
    ) -> Result<String, JsValue> {
        self.set_numbering_restart_native(section_idx as usize, para_idx as usize, mode, start_num)
            .map_err(|e| e.into())
    }

    #[wasm_bindgen(js_name = applyParaFormat)]
    pub fn apply_para_format(
        &mut self,
        sec_idx: usize,
        para_idx: usize,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_para_format_native(sec_idx, para_idx, props_json)
            .map_err(|e| e.into())
    }

    /// 문단의 paraShapeId를 직접 설정한다.
    #[wasm_bindgen(js_name = setParaShapeId)]
    pub fn set_para_shape_id(
        &mut self,
        sec_idx: usize,
        para_idx: usize,
        para_shape_id: u16,
    ) -> Result<String, JsValue> {
        self.set_para_shape_id_native(sec_idx, para_idx, para_shape_id)
            .map_err(|e| e.into())
    }

    /// 문단 서식을 적용한다 (셀 내 문단).
    #[wasm_bindgen(js_name = applyParaFormatInCell)]
    pub fn apply_para_format_in_cell(
        &mut self,
        sec_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
        cell_idx: usize,
        cell_para_idx: usize,
        props_json: &str,
    ) -> Result<String, JsValue> {
        self.apply_para_format_in_cell_native(
            sec_idx,
            parent_para_idx,
            control_idx,
            cell_idx,
            cell_para_idx,
            props_json,
        )
        .map_err(|e| e.into())
    }

    /// 셀 내 문단의 paraShapeId를 직접 설정한다.
    #[wasm_bindgen(js_name = setCellParaShapeId)]
    pub fn set_cell_para_shape_id(
        &mut self,
        sec_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
        cell_idx: usize,
        cell_para_idx: usize,
        para_shape_id: u16,
    ) -> Result<String, JsValue> {
        self.set_cell_para_shape_id_native(
            sec_idx,
            parent_para_idx,
            control_idx,
            cell_idx,
            cell_para_idx,
            para_shape_id,
        )
        .map_err(|e| e.into())
    }

    // =====================================================================
    // 클립보드 API (WASM 바인딩)
    // =====================================================================

    /// 내부 클립보드에 데이터가 있는지 확인한다.
    #[wasm_bindgen(js_name = hasInternalClipboard)]
    pub fn has_internal_clipboard(&self) -> bool {
        self.has_internal_clipboard_native()
    }


    /// 내부 클립보드를 초기화한다.
    #[wasm_bindgen(js_name = clearClipboard)]
    pub fn clear_clipboard(&mut self) {
        self.clear_clipboard_native()
    }

    /// 선택 영역을 내부 클립보드에 복사한다.
    ///
    /// 반환값: JSON `{"ok":true,"text":"<plain_text>"}`
    #[wasm_bindgen(js_name = copySelection)]
    pub fn copy_selection(
        &mut self,
        section_idx: u32,
        start_para_idx: u32,
        start_char_offset: u32,
        end_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.copy_selection_native(
            section_idx as usize,
            start_para_idx as usize,
            start_char_offset as usize,
            end_para_idx as usize,
            end_char_offset as usize,
        )
        .map_err(|e| e.into())
    }

    /// 표 셀 내부 선택 영역을 내부 클립보드에 복사한다.
    #[wasm_bindgen(js_name = copySelectionInCell)]
    pub fn copy_selection_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        start_cell_para_idx: u32,
        start_char_offset: u32,
        end_cell_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        self.copy_selection_in_cell_native(
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

    /// `copySelectionInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, startCellParaIdx,
    /// startCharOffset, endCellParaIdx, endCharOffset }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = copySelectionInCellEx)]
    pub fn copy_selection_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::json_u32;
        self.copy_selection_in_cell_native(
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

    /// 전체 cellPath가 가리키는 중첩 셀의 선택 영역을 내부 클립보드에 복사한다(#4272).
    #[wasm_bindgen(js_name = copySelectionInCellByPath)]
    pub fn copy_selection_in_cell_by_path(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        start_cell_para_idx: u32,
        start_char_offset: u32,
        end_cell_para_idx: u32,
        end_char_offset: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.copy_selection_in_cell_by_path_native(
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

    /// 컨트롤 객체(표, 이미지, 도형)를 내부 클립보드에 복사한다.
    ///
    /// [Task #1161] `cell_path_json` 이 빈 문자열/`"[]"` 면 본문, 그 외에는 셀/글상자
    /// 경로(`[{"controlIndex","cellIndex","cellParaIndex"}, ...]`)의 컨트롤을 복사한다.
    #[wasm_bindgen(js_name = copyControl)]
    pub fn copy_control(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        cell_path_json: &str,
        control_idx: u32,
    ) -> Result<String, JsValue> {
        let cell_path = parse_cell_path_arg(cell_path_json)?;
        self.copy_control_native(
            section_idx as usize,
            para_idx as usize,
            &cell_path,
            control_idx as usize,
        )
        .map_err(|e| e.into())
    }

    /// 내부 클립보드에 컨트롤(표/그림/도형)이 포함되어 있는지 확인한다.
    #[wasm_bindgen(js_name = clipboardHasControl)]
    pub fn clipboard_has_control(&self) -> bool {
        self.clipboard_has_control_native()
    }

    /// 내부 클립보드의 컨트롤 객체를 캐럿 위치에 붙여넣는다.
    ///
    /// 반환값: JSON `{"ok":true,"paraIdx":<idx>,"controlIdx":0}`
    #[wasm_bindgen(js_name = pasteControl)]
    pub fn paste_control(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.paste_control_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }

    /// 내부 클립보드의 내용을 캐럿 위치에 붙여넣는다 (본문 문단).
    ///
    /// 반환값: JSON `{"ok":true,"paraIdx":<idx>,"charOffset":<offset>}`
    #[wasm_bindgen(js_name = pasteInternal)]
    pub fn paste_internal(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.paste_internal_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }

    /// 내부 클립보드의 내용을 표 셀 내부에 붙여넣는다.
    ///
    /// 반환값: JSON `{"ok":true,"cellParaIdx":<idx>,"charOffset":<offset>}`
    #[wasm_bindgen(js_name = pasteInternalInCell)]
    pub fn paste_internal_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        self.paste_internal_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }

    /// 내부 클립보드의 내용을 cellPath가 가리키는 중첩 표 셀에 붙여넣는다.
    ///
    /// 반환값: JSON `{"ok":true,"cellParaIdx":<idx>,"charOffset":<offset>}`
    #[wasm_bindgen(js_name = pasteInternalInCellByPath)]
    pub fn paste_internal_in_cell_by_path(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.paste_internal_in_cell_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
        )
        .map_err(|e| e.into())
    }


    /// HTML 문자열을 파싱하여 캐럿 위치에 삽입한다 (본문).
    #[wasm_bindgen(js_name = pasteHtml)]
    pub fn paste_html(
        &mut self,
        section_idx: u32,
        para_idx: u32,
        char_offset: u32,
        html: &str,
    ) -> Result<String, JsValue> {
        self.paste_html_native(
            section_idx as usize,
            para_idx as usize,
            char_offset as usize,
            html,
        )
        .map_err(|e| e.into())
    }

    /// HTML 문자열을 파싱하여 셀 내부 캐럿 위치에 삽입한다.
    #[wasm_bindgen(js_name = pasteHtmlInCell)]
    pub fn paste_html_in_cell(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        control_idx: u32,
        cell_idx: u32,
        cell_para_idx: u32,
        char_offset: u32,
        html: &str,
    ) -> Result<String, JsValue> {
        self.paste_html_in_cell_native(
            section_idx as usize,
            parent_para_idx as usize,
            control_idx as usize,
            cell_idx as usize,
            cell_para_idx as usize,
            char_offset as usize,
            html,
        )
        .map_err(|e| e.into())
    }

    /// `pasteHtmlInCell` 의 options object 변형 (#1413).
    ///
    /// options JSON 키: `{ sectionIdx, parentParaIdx, controlIdx, cellIdx, cellParaIdx,
    /// charOffset?, html: string }`. positional 과 동일 동작.
    #[wasm_bindgen(js_name = pasteHtmlInCellEx)]
    pub fn paste_html_in_cell_ex(&mut self, options_json: &str) -> Result<String, JsValue> {
        use crate::document_core::helpers::{json_str, json_u32};
        self.paste_html_in_cell_native(
            json_u32(options_json, "sectionIdx").unwrap_or(0) as usize,
            json_u32(options_json, "parentParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "controlIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellIdx").unwrap_or(0) as usize,
            json_u32(options_json, "cellParaIdx").unwrap_or(0) as usize,
            json_u32(options_json, "charOffset").unwrap_or(0) as usize,
            &json_str(options_json, "html").unwrap_or_default(),
        )
        .map_err(|e| e.into())
    }

    /// HTML 문자열을 파싱하여 cellPath가 가리키는 중첩 표 셀에 삽입한다.
    #[wasm_bindgen(js_name = pasteHtmlInCellByPath)]
    pub fn paste_html_in_cell_by_path(
        &mut self,
        section_idx: u32,
        parent_para_idx: u32,
        path_json: &str,
        char_offset: u32,
        html: &str,
    ) -> Result<String, JsValue> {
        let path = DocumentCore::parse_cell_path(path_json)?;
        self.paste_html_in_cell_by_path_native(
            section_idx as usize,
            parent_para_idx as usize,
            &path,
            char_offset as usize,
            html,
        )
        .map_err(|e| e.into())
    }

    /// 문단별 줄 폭 측정 진단 (WASM)
    #[wasm_bindgen(js_name = measureWidthDiagnostic)]
    pub fn measure_width_diagnostic(
        &self,
        section_idx: u32,
        para_idx: u32,
    ) -> Result<String, JsValue> {
        self.measure_width_diagnostic_native(section_idx as usize, para_idx as usize)
            .map_err(|e| e.into())
    }
}

pub(crate) mod event;

/// WASM 뷰어 컨트롤러 (뷰포트 관리 + 스케줄링)
#[wasm_bindgen]
pub struct HwpViewer {
    /// 문서 참조 (소유)
    document: HwpDocument,
    /// 렌더링 스케줄러
    scheduler: RenderScheduler,
}

#[wasm_bindgen]
impl HwpViewer {
    /// 뷰어 생성
    #[wasm_bindgen(constructor)]
    pub fn new(document: HwpDocument) -> Self {
        let page_count = document.page_count();
        let scheduler = RenderScheduler::new(page_count);
        Self {
            document,
            scheduler,
        }
    }

    /// 뷰포트 업데이트 (스크롤/리사이즈 시 호출)
    #[wasm_bindgen(js_name = updateViewport)]
    pub fn update_viewport(&mut self, scroll_x: f64, scroll_y: f64, width: f64, height: f64) {
        let event = RenderEvent::ViewportChanged(Viewport {
            scroll_x,
            scroll_y,
            width,
            height,
            zoom: self.scheduler_zoom(),
        });
        self.scheduler.on_event(&event);
    }

    /// 줌 변경
    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, zoom: f64) {
        let event = RenderEvent::ZoomChanged(zoom);
        self.scheduler.on_event(&event);
    }

    /// 현재 보이는 페이지 목록 반환
    #[wasm_bindgen(js_name = visiblePages)]
    pub fn visible_pages(&self) -> Vec<u32> {
        self.scheduler.visible_pages()
    }

    /// 대기 중인 렌더링 작업 수
    #[wasm_bindgen(js_name = pendingTaskCount)]
    pub fn pending_task_count(&self) -> u32 {
        self.scheduler.pending_count() as u32
    }

    /// 총 페이지 수
    #[wasm_bindgen(js_name = pageCount)]
    pub fn page_count(&self) -> u32 {
        self.document.page_count()
    }

    /// 특정 페이지 SVG 렌더링
    #[wasm_bindgen(js_name = renderPageSvg)]
    pub fn render_page_svg(&self, page_num: u32) -> Result<String, JsValue> {
        self.document.render_page_svg(page_num)
    }

    /// 명시적인 출력 profile로 특정 페이지 SVG 렌더링
    #[wasm_bindgen(js_name = renderPageSvgWithProfile)]
    pub fn render_page_svg_with_profile(
        &self,
        page_num: u32,
        profile: &str,
    ) -> Result<String, JsValue> {
        self.document
            .render_page_svg_with_profile(page_num, profile)
    }

    /// 특정 페이지 HTML 렌더링
    #[wasm_bindgen(js_name = renderPageHtml)]
    pub fn render_page_html(&self, page_num: u32) -> Result<String, JsValue> {
        self.document.render_page_html(page_num)
    }
}

impl HwpViewer {
    fn scheduler_zoom(&self) -> f64 {
        1.0
    }
}

#[wasm_bindgen]
impl HwpDocument {
    // ── 책갈피 API ──


    /// 책갈피 추가
    #[wasm_bindgen(js_name = addBookmark)]
    pub fn add_bookmark(
        &mut self,
        sec: u32,
        para: u32,
        char_offset: u32,
        name: &str,
    ) -> Result<String, JsValue> {
        self.core
            .add_bookmark_native(sec as usize, para as usize, char_offset as usize, name)
            .map_err(|e| e.into())
    }

    /// 책갈피 삭제
    #[wasm_bindgen(js_name = deleteBookmark)]
    pub fn delete_bookmark(
        &mut self,
        sec: u32,
        para: u32,
        ctrl_idx: u32,
    ) -> Result<String, JsValue> {
        self.core
            .delete_bookmark_native(sec as usize, para as usize, ctrl_idx as usize)
            .map_err(|e| e.into())
    }

    /// 책갈피 이름 변경
    #[wasm_bindgen(js_name = renameBookmark)]
    pub fn rename_bookmark(
        &mut self,
        sec: u32,
        para: u32,
        ctrl_idx: u32,
        new_name: &str,
    ) -> Result<String, JsValue> {
        self.core
            .rename_bookmark_native(sec as usize, para as usize, ctrl_idx as usize, new_name)
            .map_err(|e| e.into())
    }
}

// ─── 독립 함수 (문서 로드 없이 사용 가능) ───────────────

/// HWP 파일에서 썸네일 이미지만 경량 추출 (전체 파싱 없이)
///
/// 반환: JSON `{ "format": "png"|"gif", "base64": "...", "width": N, "height": N }`
/// PrvImage가 없으면 `null` 반환
#[wasm_bindgen(js_name = extractThumbnail)]
pub fn extract_thumbnail(data: &[u8]) -> JsValue {
    match crate::parser::extract_thumbnail_only(data) {
        Some(result) => {
            let base64 = base64_encode(&result.data);
            let mime = match result.format.as_str() {
                "png" => "image/png",
                "bmp" => "image/bmp",
                "gif" => "image/gif",
                _ => "application/octet-stream",
            };
            let json = format!(
                r#"{{"format":"{}","base64":"{}","dataUri":"data:{};base64,{}","width":{},"height":{}}}"#,
                result.format, base64, mime, base64, result.width, result.height
            );
            JsValue::from_str(&json)
        }
        None => JsValue::NULL,
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[cfg(test)]
mod tests;
