//! PDF 렌더러 (Task #21)
//!
//! SVG 렌더러의 출력을 svg2pdf + pdf-writer로 PDF를 생성한다.
//! 단일/다중 페이지 모두 지원. 네이티브 전용 (WASM 미지원).

/// PDF 내보내기 폰트 설정.
///
/// `export-pdf`는 SVG를 usvg/svg2pdf로 변환하므로 generic font family와 수식 SVG
/// font-family를 PDF 변환 직전에 조정한다.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportOptions {
    /// serif generic fallback family.
    pub fallback_serif: String,
    /// sans-serif generic fallback family.
    pub fallback_sans: String,
    /// monospace generic fallback family.
    pub fallback_mono: String,
    /// 사용자 지정 수식 우선 폰트. None이면 기존 수식 font-family 체인을 유지한다.
    pub equation_font: Option<String>,
    /// 사용자 지정 폰트 탐색 디렉토리. 기본 탐색 경로보다 먼저 로드한다.
    pub font_paths: Vec<std::path::PathBuf>,
    /// 텍스트를 PDF 폰트로 임베드할지 여부. `false` 면 글리프를 path 로 변환한다.
    ///
    /// [Task #2264] 임베드 경로(폰트 서브셋)가 PDF 변환 메모리의 지배항이다.
    /// 실측(텍스트 1639개·이미지 2개인 1페이지 기준): `svg2pdf::to_chunk` 최대 RSS 가
    /// 164 MB → 69 MB 로 떨어진다. 대신 **PDF 의 텍스트 선택·검색 기능을 잃는다**
    /// (시각적 출력은 동일). 기본값은 종전 동작인 `true` 다.
    pub embed_text: bool,
    /// (docs/RHWP_GLYPH_OUTLINE_CACHE_PLAN.md) 미리 계산된 glyph outline
    /// 캐시 파일 경로. 지정하면 `usvg::Tree::from_str`의 glyph flatten 단계가
    /// 매 glyph occurrence 마다 `ttf_parser::Face::parse` + `outline_glyph`를
    /// 다시 계산하는 대신 이 캐시를 먼저 조회한다. 파일이 없거나, 캐시가
    /// `--font-path` 폰트 집합과 어긋나면(해시 불일치) 경고만 출력하고 오늘과
    /// 동일하게 매번 새로 계산한다 — 순수 가속 기능이며 새로운 하드 의존성이
    /// 아니다.
    pub glyph_cache_path: Option<std::path::PathBuf>,
    /// (docs/RHWP_GLYPH_OUTLINE_CACHE_PLAN.md) 이번 렌더에서 계산된(또는
    /// `glyph_cache_path`에서 불러온 뒤 이번 렌더로 보강된) glyph outline을
    /// 이 경로에 기록한다 — 캐시 파일을 새로 만들거나 갱신하는 용도.
    /// `glyph_cache_path`와 함께 지정하면 기존 캐시를 기반으로 누적한다.
    pub dump_glyph_cache_path: Option<std::path::PathBuf>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            fallback_serif: default_serif_family().to_string(),
            fallback_sans: default_sans_family().to_string(),
            fallback_mono: default_mono_family().to_string(),
            equation_font: None,
            font_paths: Vec::new(),
            embed_text: true,
            glyph_cache_path: None,
            dump_glyph_cache_path: None,
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn default_serif_family() -> &'static str {
    "바탕"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn default_sans_family() -> &'static str {
    "맑은 고딕"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn default_mono_family() -> &'static str {
    "D2Coding"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
fn default_serif_family() -> &'static str {
    "Noto Serif CJK KR"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
fn default_sans_family() -> &'static str {
    "Noto Sans CJK KR"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
fn default_mono_family() -> &'static str {
    "Noto Sans Mono CJK KR"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
fn default_serif_family() -> &'static str {
    "AppleMyungjo"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
fn default_sans_family() -> &'static str {
    "Apple SD Gothic Neo"
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
fn default_mono_family() -> &'static str {
    "Menlo"
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "windows", target_os = "linux", target_os = "macos"))
))]
fn default_serif_family() -> &'static str {
    "Noto Serif CJK KR"
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "windows", target_os = "linux", target_os = "macos"))
))]
fn default_sans_family() -> &'static str {
    "Noto Sans CJK KR"
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "windows", target_os = "linux", target_os = "macos"))
))]
fn default_mono_family() -> &'static str {
    "Noto Sans Mono CJK KR"
}

/// 폰트 데이터베이스를 초기화 (시스템 폰트 + 프로젝트 폰트 로드)
#[cfg(not(target_arch = "wasm32"))]
/// fontdb 는 `Source::File` 폰트의 바이트를 `with_face_data` 호출마다(=텍스트
/// 셰이핑마다) 매번 새로 open+mmap+munmap 한다(fontdb 0.23 `with_face_data`,
/// `load_font_file_impl` 참고 — 같은 프로세스 안에서 같은 파일을 반복해서
/// 열고 닫는다). 문서 하나에 텍스트런이 많으면 이 반복 open/mmap이 export-pdf
/// convert 단계 self-time의 상당 부분을 차지한다(hwpx-template-engine 리포
/// docs/rhwp-convert-분석.md 3절, samply flamegraph 실측 — self-time의 44%가
/// `__munmap`/`__open`/`__mmap`).
///
/// fontdb는 이미 이 문제의 해법을 제공한다: `make_shared_face_data`가
/// `Source::File`을 한 번만 mmap한 `Arc<Mmap>` 기반 `Source::SharedFile`로
/// 바꿔주면, 이후의 모든 `with_face_data` 호출이 그 매핑을 재사용한다. 매핑된
/// 파일이 프로세스 실행 중 디스크에서 바뀔 수 있어 `unsafe`로 표시돼 있지만,
/// 이 프로세스는 렌더링 한 번을 위해 짧게 살고 죽으며 `--font-path`로 넘어온
/// 폰트 파일은 그 사이 다른 프로세스가 바꿀 일이 없으므로 안전하다. 이미
/// `Source::Binary`/`Source::SharedFile`인 얼굴에는 이 호출이 그냥 기존
/// 참조를 복제해 돌려줄 뿐이라 무해하다.
#[cfg(not(target_arch = "wasm32"))]
fn share_face_data(fontdb: &mut usvg::fontdb::Database) {
    let ids: Vec<usvg::fontdb::ID> = fontdb.faces().map(|face| face.id).collect();
    for id in ids {
        unsafe {
            fontdb.make_shared_face_data(id);
        }
    }
}

fn create_fontdb(options: &PdfExportOptions) -> usvg::fontdb::Database {
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    for dir in &options.font_paths {
        if dir.exists() {
            fontdb.load_fonts_dir(dir);
        } else {
            eprintln!(
                "WARN: PDF font path '{}' not found. 해당 경로의 폰트는 로드하지 않습니다.",
                dir.display()
            );
        }
    }
    for dir in &["ttfs", "ttfs/windows", "ttfs/hwp"] {
        if std::path::Path::new(dir).exists() {
            fontdb.load_fonts_dir(dir);
        }
    }
    if std::path::Path::new("/mnt/c/Windows/Fonts").exists() {
        fontdb.load_fonts_dir("/mnt/c/Windows/Fonts");
    }
    share_face_data(&mut fontdb);
    fontdb.set_serif_family(options.fallback_serif.as_str());
    fontdb.set_sans_serif_family(options.fallback_sans.as_str());
    fontdb.set_monospace_family(options.fallback_mono.as_str());
    warn_missing_family(
        &fontdb,
        "serif",
        &options.fallback_serif,
        "--fallback-serif",
    );
    warn_missing_family(
        &fontdb,
        "sans-serif",
        &options.fallback_sans,
        "--fallback-sans",
    );
    warn_missing_family(
        &fontdb,
        "monospace",
        &options.fallback_mono,
        "--fallback-mono",
    );
    if let Some(equation_font) = options.equation_font.as_deref() {
        let family = first_font_family(equation_font);
        if !family.is_empty() {
            warn_missing_family(&fontdb, "equation", &family, "--equation-font");
        }
    }
    fontdb
}

#[cfg(not(target_arch = "wasm32"))]
fn warn_missing_family(
    fontdb: &usvg::fontdb::Database,
    kind: &str,
    family: &str,
    option_name: &str,
) {
    if !font_family_exists(fontdb, family) {
        eprintln!(
            "WARN: fallback {kind} font '{family}' not found.\n      한글 또는 수식이 빈칸으로 렌더링될 수 있습니다.\n      {option_name} \"<family>\" 로 설치된 폰트를 지정하세요."
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn font_family_exists(fontdb: &usvg::fontdb::Database, family: &str) -> bool {
    fontdb.faces().any(|face| {
        face.families
            .iter()
            .any(|(name, _)| name == family || name.eq_ignore_ascii_case(family))
    })
}

/// SVG에서 없는 한글 폰트명에 fallback 추가
#[cfg(not(target_arch = "wasm32"))]
fn add_font_fallbacks(svg: &str, options: &PdfExportOptions) -> String {
    let serif = css_family_for_attr(&options.fallback_serif);
    let sans = css_family_for_attr(&options.fallback_sans);
    svg.replace(
        "font-family=\"휴먼명조\"",
        &format!("font-family=\"휴먼명조, {serif}, serif\""),
    )
    .replace(
        "font-family=\"HCI Poppy\"",
        &format!("font-family=\"HCI Poppy, {sans}, sans-serif\""),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_pdf_font_options(svg: &str, options: &PdfExportOptions) -> String {
    let svg = add_font_fallbacks(svg, options);
    if let Some(equation_font) = options.equation_font.as_deref() {
        let attr = format!(
            "font-family=\"{}\"",
            escape_xml_attr(&equation_font_chain(equation_font))
        );
        svg.replace(
            crate::renderer::equation::svg_render::DEFAULT_EQUATION_FONT_FAMILY_ATTR,
            &attr,
        )
    } else {
        svg
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn equation_font_chain(equation_font: &str) -> String {
    if equation_font.contains(',') {
        return equation_font.trim().to_string();
    }
    let first = css_family_for_attr(equation_font);
    let default =
        "'Latin Modern Math', 'STIX Two Text', 'STIX Two Math', 'Times New Roman', 'Times', serif";
    if first == "'Latin Modern Math'" {
        default.to_string()
    } else {
        format!("{first}, {default}")
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn first_font_family(value: &str) -> String {
    value
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn css_family_for_attr(family: &str) -> String {
    let family = family.trim();
    if family.eq_ignore_ascii_case("serif")
        || family.eq_ignore_ascii_case("sans-serif")
        || family.eq_ignore_ascii_case("monospace")
    {
        return family.to_string();
    }
    let escaped = escape_xml_attr(family);
    format!("'{escaped}'")
}

#[cfg(not(target_arch = "wasm32"))]
fn escape_xml_attr(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// 단일 SVG를 PDF로 변환
#[cfg(not(target_arch = "wasm32"))]
pub fn svg_to_pdf(svg_content: &str) -> Result<Vec<u8>, String> {
    svgs_to_pdf(&[svg_content.to_string()])
}

/// 단일 SVG를 옵션 기반 PDF로 변환
#[cfg(not(target_arch = "wasm32"))]
pub fn svg_to_pdf_with_options(
    svg_content: &str,
    options: &PdfExportOptions,
) -> Result<Vec<u8>, String> {
    svgs_to_pdf_with_options(&[svg_content.to_string()], options)
}

/// 여러 SVG 페이지를 단일 다중 페이지 PDF로 생성
#[cfg(not(target_arch = "wasm32"))]
pub fn svgs_to_pdf(svg_pages: &[String]) -> Result<Vec<u8>, String> {
    svgs_to_pdf_with_options(svg_pages, &PdfExportOptions::default())
}

struct PageData {
    chunk: pdf_writer::Chunk,
    svg_ref: pdf_writer::Ref,
    width: f32,
    height: f32,
}

/// 페이지 하나를 파싱 + PDF chunk로 변환한다. 페이지 간 상태 공유가 없어
/// 병렬 호출이 안전하다 (`options`의 fontdb는 Arc로 읽기 전용 공유).
///
/// `font_data_cache`: 벤더 패치(font-data-cache). `render_one_page`는 페이지마다
/// 독립된 `svg2pdf::Context`를 새로 만들기 때문에(=페이지 간 상태 공유 없음, 위
/// 설명대로 병렬화의 전제), `svg2pdf::render::text::fill_fonts`가 폰트 파일 전체
/// 바이트를 `Vec::from(data)`로 매 페이지마다 새로 복사한다 — 4페이지 문서면 같은
/// 폰트를 4번 복사하는 셈(`docs/rhwp-convert-분석.md` 후속 조사에서 self-time의
/// ~15%로 측정, `fontdb`의 mmap 공유 자체는 이미 font-mmap-cache 패치로 해결됐지만
/// 이건 그 위에서 svg2pdf가 별도로 복사하는 다른 지점이다). 모든 페이지가 같은
/// `fontdb::Database`를 읽기 전용으로 공유하므로, 복사된 바이트도 페이지 간에
/// 안전하게 공유할 수 있다 — `Ref`/`glyph_set`/`glyph_remapper`처럼 페이지마다
/// 달라야 하는 상태는 그대로 페이지 로컬로 유지된다(캐시는 원본 폰트 바이트 +
/// `units_per_em` + `face_index`만 담는다). `svgs_to_pdf_with_options`가 문서당
/// 하나만 만들어 모든 페이지(스레드)에 공유한다.
#[cfg(not(target_arch = "wasm32"))]
fn render_one_page(
    svg: &str,
    export_options: &PdfExportOptions,
    options: &usvg::Options,
    font_data_cache: &Option<svg2pdf::FontDataCache>,
) -> Result<PageData, String> {
    let svg_with_fallback = apply_pdf_font_options(svg, export_options);
    let tree = usvg::Tree::from_str(&svg_with_fallback, options)
        .map_err(|e| format!("SVG 파싱 실패: {}", e))?;

    // [Task #2264] 텍스트 임베드(폰트 서브셋)가 PDF 변환 메모리의 지배항이다.
    // `embed_text=false` 면 글리프를 path 로 변환해 서브셋 경로를 통째로 건너뛴다.
    let mut conversion = svg2pdf::ConversionOptions::default();
    conversion.embed_text = export_options.embed_text;
    conversion.font_data_cache = font_data_cache.clone();

    let (chunk, svg_ref) = svg2pdf::to_chunk(&tree, conversion)
        .map_err(|e| format!("SVG→chunk 변환 실패: {:?}", e))?;

    let dpi_ratio = 72.0 / 96.0; // 96 DPI → 72 pt
    let w = tree.size().width() * dpi_ratio;
    let h = tree.size().height() * dpi_ratio;

    Ok(PageData {
        chunk,
        svg_ref,
        width: w,
        height: h,
    })
}

/// 모든 페이지를 파싱+변환한다. 페이지가 2개 이상이면 `available_parallelism()`
/// 기준으로 청크를 나눠 스레드에 분산한다 (스레드 수가 페이지 수만큼 무한정
/// 늘지 않도록 상한). 페이지 순서는 청크 순서 + 청크 내부 순차 처리로 그대로
/// 보존된다 — 이후 재채번/조립 단계는 이 순서에 의존한다.
#[cfg(not(target_arch = "wasm32"))]
fn render_pages_to_page_data(
    svg_pages: &[String],
    export_options: &PdfExportOptions,
    options: &usvg::Options,
    font_data_cache: &Option<svg2pdf::FontDataCache>,
) -> Result<Vec<PageData>, String> {
    if svg_pages.len() <= 1 {
        return svg_pages
            .iter()
            .map(|svg| render_one_page(svg, export_options, options, font_data_cache))
            .collect();
    }

    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(svg_pages.len());
    let chunk_size = svg_pages.len().div_ceil(worker_count.max(1));

    let chunk_results: Vec<Result<Vec<PageData>, String>> = std::thread::scope(|scope| {
        svg_pages
            .chunks(chunk_size.max(1))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|svg| render_one_page(svg, export_options, options, font_data_cache))
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().expect("페이지 렌더 스레드 패닉"))
            .collect()
    });

    let mut page_datas = Vec::with_capacity(svg_pages.len());
    for chunk_result in chunk_results {
        page_datas.extend(chunk_result?);
    }
    Ok(page_datas)
}

/// 여러 SVG 페이지를 옵션 기반 단일 다중 페이지 PDF로 생성
#[cfg(not(target_arch = "wasm32"))]
pub fn svgs_to_pdf_with_options(
    svg_pages: &[String],
    export_options: &PdfExportOptions,
) -> Result<Vec<u8>, String> {
    if svg_pages.is_empty() {
        return Err("페이지가 없습니다".to_string());
    }
    use pdf_writer::{Finish, Pdf, Ref};
    use std::collections::HashMap;

    let fontdb = create_fontdb(export_options);
    let mut options = usvg::Options::default();
    options.fontdb = std::sync::Arc::new(fontdb);

    // (docs/RHWP_GLYPH_OUTLINE_CACHE_PLAN.md) A cache is only constructed if
    // either flag is present -- the common case (neither flag set) takes
    // the exact same `glyph_outline_cache: None` path as before this patch.
    // `--dump-glyph-cache` continues accumulating on top of whatever
    // `--glyph-cache` loaded (or starts empty if that load failed/was
    // absent), so `export-pdf --glyph-cache X --dump-glyph-cache X` is a
    // valid "top up an existing cache" invocation.
    let glyph_cache: Option<std::sync::Arc<usvg::GlyphOutlineCache>> =
        if export_options.glyph_cache_path.is_some()
            || export_options.dump_glyph_cache_path.is_some()
        {
            let loaded = export_options
                .glyph_cache_path
                .as_deref()
                .and_then(|path| super::glyph_cache_file::load(path, &export_options.font_paths));
            Some(std::sync::Arc::new(
                loaded.unwrap_or_else(usvg::GlyphOutlineCache::empty),
            ))
        } else {
            None
        };
    options.glyph_outline_cache = glyph_cache.clone();

    // docs/design/RHWP_TEXT_FLATTEN_SKIP_SPIKE_PLAN.md (hwpx-template-engine
    // repo) — svg2pdf's embed_text=true path draws text from usvg's
    // metrics-based Text.bounding_box and never reads Text.flattened (the
    // per-glyph outline geometry usvg computes unconditionally), so skip
    // computing it whenever we're embedding real fonts anyway. No new CLI
    // flag: derives from the existing --embed-text option, so
    // export-svg/export-render-tree/embed_text=false are untouched by
    // construction (they never set this field, defaulting to `false`).
    options.skip_text_flatten = export_options.embed_text;

    // [벤더 패치: font-data-cache] render_one_page 문서 참고 — 모든 페이지가
    // 공유하는 문서(요청) 스코프 캐시. `embed_text=false`면 svg2pdf가 fill_fonts를
    // 아예 안 타므로 채워지지 않지만, 빈 HashMap 할당 자체는 무시할 수준이라
    // 조건 분기 없이 항상 만든다.
    let font_data_cache: Option<svg2pdf::FontDataCache> =
        Some(std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())));

    let mut alloc = Ref::new(1);
    let catalog_ref = alloc.bump();
    let page_tree_ref = alloc.bump();

    // 각 페이지의 SVG를 파싱하여 chunk + page 정보 수집
    //
    // [벤더 패치: 페이지 병렬화] usvg::Tree::from_str(텍스트/글리프 셰이핑)이
    // export-pdf 총 시간의 ~75%를 차지한다는 것이 실측으로 확인됐고, 각 페이지의
    // 파싱+변환(chunk 채번은 여기서 페이지 로컬로만 일어나고, 전역 alloc은 이 뒤
    // 재채번 루프에서만 쓰인다)은 페이지 간 상태 공유가 없어 병렬화해도 안전하다.
    // `options`(내부 Arc<fontdb>)는 읽기 전용으로 스레드 간 공유. 페이지 수가
    // 코어 수를 크게 넘어도 스레드가 무한정 늘지 않도록 available_parallelism()
    // 기준으로 청크를 나눠 스레드 수를 상한한다. 0/1페이지는 스레드 생성 비용이
    // 이득보다 커 순차 경로를 그대로 둔다.
    let page_datas: Vec<PageData> =
        render_pages_to_page_data(svg_pages, export_options, &options, &font_data_cache)?;

    // 각 chunk를 재번호화하고 페이지 참조 수집
    let mut page_refs: Vec<Ref> = Vec::new();
    let mut renumbered_chunks: Vec<pdf_writer::Chunk> = Vec::new();
    let mut svg_refs_remapped: Vec<Ref> = Vec::new();

    for pd in &page_datas {
        let page_ref = alloc.bump();
        let content_ref = alloc.bump();
        page_refs.push(page_ref);

        // chunk 재번호화
        let mut map = HashMap::new();
        let renumbered = pd
            .chunk
            .renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));

        let remapped_svg_ref = map.get(&pd.svg_ref).copied().unwrap_or(pd.svg_ref);
        svg_refs_remapped.push(remapped_svg_ref);
        renumbered_chunks.push(renumbered);
    }

    // PDF 생성
    let mut pdf = Pdf::new();
    pdf.catalog(catalog_ref).pages(page_tree_ref);
    pdf.pages(page_tree_ref)
        .count(page_refs.len() as i32)
        .kids(page_refs.iter().copied());

    // 각 페이지 생성
    let svg_name = pdf_writer::Name(b"S1");

    for (i, pd) in page_datas.iter().enumerate() {
        let page_ref = page_refs[i];
        let content_ref = alloc.bump();
        let svg_ref = svg_refs_remapped[i];

        let mut page = pdf.page(page_ref);
        page.media_box(pdf_writer::Rect::new(0.0, 0.0, pd.width, pd.height));
        page.parent(page_tree_ref);
        page.contents(content_ref);

        let mut resources = page.resources();
        resources.x_objects().pair(svg_name, svg_ref);
        resources.finish();
        page.finish();

        // 컨텐츠 스트림: SVG XObject를 페이지 크기에 맞게 배치
        let mut content = pdf_writer::Content::new();
        content.transform([pd.width, 0.0, 0.0, pd.height, 0.0, 0.0]);
        content.x_object(svg_name);

        pdf.stream(content_ref, &content.finish());
    }

    // 모든 chunk를 PDF에 추가
    for chunk in &renumbered_chunks {
        pdf.extend(chunk);
    }

    // 문서 정보
    let info_ref = alloc.bump();
    pdf.document_info(info_ref)
        .producer(pdf_writer::TextStr("rhwp"));

    if let Some(dump_path) = &export_options.dump_glyph_cache_path {
        // `glyph_cache` is always `Some` here: it's constructed unconditionally
        // whenever `dump_glyph_cache_path` is set (see above).
        let cache = glyph_cache
            .as_deref()
            .expect("dump_glyph_cache_path implies glyph_cache was constructed");
        if let Err(e) = super::glyph_cache_file::write(dump_path, cache, &export_options.font_paths)
        {
            eprintln!(
                "WARN: glyph cache '{}' 기록 실패 - {}",
                dump_path.display(),
                e
            );
        }
    }

    Ok(pdf.finish())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn default_pdf_font_options_are_os_specific_and_non_empty() {
        let options = PdfExportOptions::default();
        assert!(!options.fallback_serif.is_empty());
        assert!(!options.fallback_sans.is_empty());
        assert!(!options.fallback_mono.is_empty());
        assert!(options.equation_font.is_none());
    }

    #[test]
    fn pdf_font_options_replace_generic_fallbacks_and_equation_font() {
        let options = PdfExportOptions {
            fallback_serif: "Noto Serif CJK KR".to_string(),
            fallback_sans: "Noto Sans CJK KR".to_string(),
            fallback_mono: "Noto Sans Mono CJK KR".to_string(),
            equation_font: Some("STIX Two Math".to_string()),
            font_paths: Vec::new(),
            embed_text: true,
            glyph_cache_path: None,
            dump_glyph_cache_path: None,
        };
        let svg = format!(
            r#"<svg><text font-family="휴먼명조">가</text><text font-family="HCI Poppy">A</text><text {}>x</text></svg>"#,
            crate::renderer::equation::svg_render::DEFAULT_EQUATION_FONT_FAMILY_ATTR
        );

        let out = apply_pdf_font_options(&svg, &options);

        assert!(out.contains(r#"font-family="휴먼명조, 'Noto Serif CJK KR', serif""#));
        assert!(out.contains(r#"font-family="HCI Poppy, 'Noto Sans CJK KR', sans-serif""#));
        assert!(out
            .contains(r#"font-family="&apos;STIX Two Math&apos;, &apos;Latin Modern Math&apos;"#));
    }

    #[test]
    fn equation_font_accepts_full_family_chain() {
        let chain = equation_font_chain("'Custom Math', 'Fallback Math', serif");
        assert_eq!(chain, "'Custom Math', 'Fallback Math', serif");
    }
}
