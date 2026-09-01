//! export 모듈 — src/main.rs 에서 무변동 이동
use super::*;

pub(crate) fn allows_implicit_sibling_resources(format: rhwp::parser::FileFormat) -> bool {
    // HML sibling paths are untrusted input and require an explicit resolver policy.
    !matches!(format, rhwp::parser::FileFormat::Hml)
}


pub(crate) fn export_svg(args: &[String]) -> i32 {
    // [#3359] 위치 인자 파싱은 export-structure/export-text(#3349) 규약과 동일 —
    // 첫 비플래그 토큰이 파일이고 옵션은 위치 무관이다.
    let mut file_path: Option<&str> = None;
    let mut output_dir = "output".to_string();
    let mut target_page: Option<u32> = None;
    let mut show_para_marks = false;
    let mut show_control_codes = false;
    let mut debug_overlay = false;
    let mut grid_mm: Option<f64> = None;
    let mut grid_origin = GridOriginOption::Fixed((0.0_f64, 0.0_f64));
    let mut respect_vpos_reset = false;
    let mut font_embed_mode = rhwp::renderer::svg::FontEmbedMode::None;
    let mut font_paths: Vec<std::path::PathBuf> = Vec::new();
    let mut render_profile: Option<rhwp::paint::RenderProfile> = None;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_dir = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("오류: --output 뒤에 폴더 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--page" | "-p" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u32>() {
                        Ok(n) => target_page = Some(n),
                        Err(_) => {
                            eprintln!("오류: 페이지 번호가 올바르지 않습니다.");
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --page 뒤에 페이지 번호가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--profile" => {
                if i + 1 < args.len() {
                    render_profile = rhwp::paint::RenderProfile::parse(&args[i + 1]);
                    if render_profile.is_none() {
                        eprintln!(
                            "오류: --profile 값이 올바르지 않습니다 (screen|print|high-quality|fast-preview)."
                        );
                        return EXIT_USAGE;
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --profile 뒤에 프로필 이름이 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--show-para-marks" => {
                show_para_marks = true;
                i += 1;
            }
            "--show-control-codes" => {
                show_control_codes = true;
                i += 1;
            }
            "--debug-overlay" => {
                debug_overlay = true;
                i += 1;
            }
            "--respect-vpos-reset" => {
                respect_vpos_reset = true;
                i += 1;
            }
            arg if arg == "--show-grid" || arg.starts_with("--show-grid=") => {
                grid_mm = if let Some(value) = arg.strip_prefix("--show-grid=") {
                    match parse_grid_mm(value) {
                        Some(v) => Some(v),
                        None => {
                            eprintln!(
                                "오류: --show-grid 값이 올바르지 않습니다. 예: --show-grid=3mm"
                            );
                            return EXIT_USAGE;
                        }
                    }
                } else {
                    Some(1.0)
                };
                i += 1;
            }
            arg if arg == "--grid-origin" || arg == "--grid-paper-origin" => {
                if i + 1 < args.len() {
                    match parse_grid_origin_option(&args[i + 1]) {
                        Some(v) => grid_origin = v,
                        None => {
                            eprintln!(
                                "오류: --grid-origin 값이 올바르지 않습니다. 예: --grid-origin=15mm,20mm 또는 --grid-origin=auto"
                            );
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --grid-origin 뒤에 가로,세로 값이 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            arg if arg.starts_with("--grid-origin=") || arg.starts_with("--grid-paper-origin=") => {
                let value = arg
                    .strip_prefix("--grid-origin=")
                    .or_else(|| arg.strip_prefix("--grid-paper-origin="))
                    .unwrap_or_default();
                match parse_grid_origin_option(value) {
                    Some(v) => grid_origin = v,
                    None => {
                        eprintln!(
                            "오류: --grid-origin 값이 올바르지 않습니다. 예: --grid-origin=15mm,20mm 또는 --grid-origin=auto"
                        );
                        return EXIT_USAGE;
                    }
                }
                i += 1;
            }
            "--font-style" => {
                font_embed_mode = rhwp::renderer::svg::FontEmbedMode::Style;
                i += 1;
            }
            "--embed-fonts" => {
                font_embed_mode = rhwp::renderer::svg::FontEmbedMode::Subset;
                i += 1;
            }
            "--embed-fonts=full" => {
                font_embed_mode = rhwp::renderer::svg::FontEmbedMode::Full;
                i += 1;
            }
            "--font-path" => {
                if i + 1 < args.len() {
                    font_paths.push(std::path::PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("오류: --font-path 뒤에 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--json" => {
                // [#3286] 산출물 매니페스트를 stdout 에 JSON 으로 — 에이전트가
                // 어떤 파일이 생겼는지 파싱 없이 알 수 있게 한다.
                json_mode = true;
                i += 1;
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
                i += 1;
            }
        }
    }

    let Some(file_path) = file_path else {
        eprintln!("오류: 문서 파일 경로를 지정해주세요.");
        eprintln!(
            "사용법: rhwp export-svg <파일.hwp|파일.hwpx|파일.hml> [옵션] (rhwp --help 참조)"
        );
        return EXIT_USAGE;
    };

    if render_profile.is_some() && font_embed_mode != rhwp::renderer::svg::FontEmbedMode::None {
        eprintln!("오류: --profile은 --font-style/--embed-fonts와 함께 사용할 수 없습니다.");
        return EXIT_USAGE;
    }

    // 파일 읽기
    let read_start = std::time::Instant::now();
    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let read_ms = read_start.elapsed().as_millis();

    let source_format = rhwp::parser::detect_format(&data);

    // 문서 로드
    let parse_start = std::time::Instant::now();
    let mut doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };
    let parse_ms = parse_start.elapsed().as_millis();

    // [Task #741 후속] 외부 file path 그림 영역 영역 HWP file 영역 영역 같은 dir 영역
    // 영역 image 영역 영역 자동 load (basename 매칭).
    if allows_implicit_sibling_resources(source_format) {
        if let Some(parent) = std::path::Path::new(file_path).parent() {
            let _loaded = doc.populate_external_images_from_dir(parent);
        }
    }

    if show_para_marks {
        doc.set_show_paragraph_marks(true);
    }
    if show_control_codes {
        doc.set_show_control_codes(true);
    }
    if debug_overlay {
        doc.set_debug_overlay(true);
    }
    if respect_vpos_reset {
        doc.set_respect_vpos_reset(true);
    }

    let page_count = doc.page_count();
    if !json_mode {
        // stdout 순수성: --json 모드에서는 데이터(JSON)만 나간다.
        println!("문서 로드 완료: {} ({}페이지)", file_path, page_count);
    }

    // 출력 폴더 생성
    let output_path = Path::new(&output_dir);
    if !output_path.exists() {
        if let Err(e) = fs::create_dir_all(output_path) {
            eprintln!(
                "오류: 출력 폴더를 생성할 수 없습니다 - {}: {}",
                output_dir, e
            );
            return EXIT_RUNTIME;
        }
    }

    // 페이지 범위 결정
    let pages: Vec<u32> = match target_page {
        Some(p) => {
            if p >= page_count {
                eprintln!(
                    "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                    page_count - 1
                );
                return EXIT_USAGE;
            }
            vec![p]
        }
        None => (0..page_count).collect(),
    };

    // SVG 내보내기
    let file_stem = Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");

    // [#2707] 요청한 페이지 수가 아니라 실제로 저장에 성공한 페이지 수를 센다.
    let mut manifest: Vec<serde_json::Value> = Vec::new();
    let mut written = 0usize;
    // [#3668] LAYOUT_OVERFLOW_CELL 집계 — 페이지 렌더 직후 take 로 페이지 귀속.
    let mut overflow_cell_total: u64 = 0;
    let mut render_ms_total: u128 = 0;
    let mut write_ms_total: u128 = 0;

    for page_num in &pages {
        let render_start = std::time::Instant::now();
        let svg_result = if let Some(profile) = render_profile {
            doc.render_page_svg_layer_with_profile_native(*page_num, profile)
        } else if font_embed_mode != rhwp::renderer::svg::FontEmbedMode::None {
            doc.render_page_svg_with_fonts(*page_num, font_embed_mode, &font_paths)
        } else {
            doc.render_page_svg_native(*page_num)
        };
        render_ms_total += render_start.elapsed().as_millis();
        let page_overflow_cell_lines = doc.take_overflow_cell_lines();
        overflow_cell_total += u64::from(page_overflow_cell_lines);
        match svg_result {
            Ok(mut svg) => {
                // 격자 오버레이 삽입 (진단용 부가 기능 — render 시간에 포함하지 않는다)
                if let Some(mm) = grid_mm {
                    let origin_mm = match grid_origin {
                        GridOriginOption::Fixed(origin) => origin,
                        GridOriginOption::AutoPaper => {
                            match grid_paper_origin_mm(&doc, *page_num) {
                                Some(origin) => origin,
                                None => {
                                    eprintln!(
                                        "오류: 페이지 {}의 격자 기준 위치를 계산할 수 없습니다.",
                                        page_num
                                    );
                                    continue;
                                }
                            }
                        }
                    };
                    svg = insert_grid_overlay(&svg, mm, origin_mm);
                }
                let svg_filename = if page_count == 1 {
                    format!("{}.svg", file_stem)
                } else {
                    format!("{}_{:03}.svg", file_stem, page_num + 1)
                };
                let svg_path = output_path.join(&svg_filename);

                let write_start = std::time::Instant::now();
                let write_result = fs::write(&svg_path, &svg);
                write_ms_total += write_start.elapsed().as_millis();
                match write_result {
                    Ok(_) => {
                        if json_mode {
                            manifest.push(serde_json::json!({
                                "page": page_num,
                                "path": svg_path.display().to_string(),
                                "bytes": svg.len(),
                                "overflowCellLines": page_overflow_cell_lines,
                            }));
                        } else {
                            println!("  → {}", svg_path.display());
                        }
                        written += 1;
                    }
                    Err(e) => eprintln!("오류: SVG 저장 실패 - {}: {}", svg_path.display(), e),
                }
            }
            Err(e) => {
                eprintln!("오류: 페이지 {} 렌더링 실패 - {:?}", page_num, e);
            }
        }
    }

    eprintln!(
        "RHWP_TIMING {{\"cmd\":\"export-svg\",\"readMs\":{},\"parseMs\":{},\"renderMs\":{},\"writeMs\":{}}}",
        read_ms, parse_ms, render_ms_total, write_ms_total
    );

    // 단건 JSON 명령의 실패는 stdout 을 비워야 한다. 부분 매니페스트를 출력하면
    // 소비자가 성공 결과로 오인하거나 stdout JSON을 파싱한 뒤 실패를 놓친다.
    if written != pages.len() {
        if !json_mode {
            println!("내보내기 완료: {}개 SVG 파일 → {}/", written, output_dir);
        }
        return EXIT_RUNTIME;
    }

    if json_mode {
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "format": "svg",
            "outputDir": output_dir,
            "pageCount": page_count,
            "renderedCount": written,
            "overflowCellLines": overflow_cell_total,
            "pages": manifest,
        });
        println!("{}", provenance::marked(envelope, "export-svg"));
    } else {
        println!("내보내기 완료: {}개 SVG 파일 → {}/", written, output_dir);
    }

    EXIT_OK
}


pub(crate) fn export_render_tree(args: &[String]) -> i32 {
    // [#3359] 위치 인자 파싱은 export-structure/export-text(#3349) 규약과 동일.
    let mut file_path: Option<&str> = None;
    let mut output_dir = "output".to_string();
    let mut target_page: Option<u32> = None;
    let mut show_para_marks = false;
    let mut show_control_codes = false;
    let mut respect_vpos_reset = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_dir = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("오류: --output 뒤에 폴더 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--page" | "-p" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u32>() {
                        Ok(n) => target_page = Some(n),
                        Err(_) => {
                            eprintln!("오류: 페이지 번호가 올바르지 않습니다.");
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --page 뒤에 페이지 번호가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--show-para-marks" => {
                show_para_marks = true;
                i += 1;
            }
            "--show-control-codes" => {
                show_control_codes = true;
                i += 1;
            }
            "--respect-vpos-reset" => {
                respect_vpos_reset = true;
                i += 1;
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
                i += 1;
            }
        }
    }

    let Some(file_path) = file_path else {
        eprintln!("오류: HWP 파일 경로를 지정해주세요.");
        eprintln!("사용법: rhwp export-render-tree <파일.hwp> [옵션] (rhwp --help 참조)");
        return EXIT_USAGE;
    };

    let read_start = std::time::Instant::now();
    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let read_ms = read_start.elapsed().as_millis();
    let source_format = rhwp::parser::detect_format(&data);

    let parse_start = std::time::Instant::now();
    let mut doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };
    let parse_ms = parse_start.elapsed().as_millis();

    if allows_implicit_sibling_resources(source_format) {
        if let Some(parent) = std::path::Path::new(file_path).parent() {
            let _loaded = doc.populate_external_images_from_dir(parent);
        }
    }

    if show_para_marks {
        doc.set_show_paragraph_marks(true);
    }
    if show_control_codes {
        doc.set_show_control_codes(true);
    }
    if respect_vpos_reset {
        doc.set_respect_vpos_reset(true);
    }

    let page_count = doc.page_count();
    println!("문서 로드 완료: {} ({}페이지)", file_path, page_count);

    let output_path = Path::new(&output_dir);
    if !output_path.exists() {
        if let Err(e) = fs::create_dir_all(output_path) {
            eprintln!(
                "오류: 출력 폴더를 생성할 수 없습니다 - {}: {}",
                output_dir, e
            );
            return EXIT_RUNTIME;
        }
    }

    let pages: Vec<u32> = match target_page {
        Some(p) => {
            if p >= page_count {
                eprintln!(
                    "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                    page_count - 1
                );
                return EXIT_USAGE;
            }
            vec![p]
        }
        None => (0..page_count).collect(),
    };

    // [#2707] 요청한 페이지 수가 아니라 실제로 저장에 성공한 페이지 수를 센다.
    let mut written = 0usize;
    let mut render_ms_total: u128 = 0;
    let mut write_ms_total: u128 = 0;

    for page_num in &pages {
        let render_start = std::time::Instant::now();
        let tree_result = doc.build_page_render_tree(*page_num);
        render_ms_total += render_start.elapsed().as_millis();
        match tree_result {
            Ok(tree) => {
                let json_path = output_path.join(format!("render_tree_{:03}.json", page_num + 1));
                let json = tree.root.to_json();
                let write_start = std::time::Instant::now();
                let write_result = fs::write(&json_path, json);
                write_ms_total += write_start.elapsed().as_millis();
                match write_result {
                    Ok(_) => {
                        println!("  → {}", json_path.display());
                        written += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "오류: render tree 저장 실패 - {}: {}",
                            json_path.display(),
                            e
                        )
                    }
                }
            }
            Err(e) => {
                eprintln!("오류: 페이지 {} render tree 생성 실패 - {:?}", page_num, e);
            }
        }
    }

    eprintln!(
        "RHWP_TIMING {{\"cmd\":\"export-render-tree\",\"readMs\":{},\"parseMs\":{},\"renderMs\":{},\"writeMs\":{}}}",
        read_ms, parse_ms, render_ms_total, write_ms_total
    );

    println!(
        "내보내기 완료: {}개 render tree JSON 파일 → {}/",
        written, output_dir
    );

    // [#2707] 한 장이라도 못 썼으면 런타임 실패다.
    if written == pages.len() {
        EXIT_OK
    } else {
        EXIT_RUNTIME
    }
}



/// `export-structure` — 문서 개요/조문 계층을 중첩 JSON 트리로 추출 (조문 DB화용).
pub(crate) fn export_structure(args: &[String]) -> i32 {
    use rhwp::document_core::queries::structure::{build_structure, StructureMode};

    let mut file_path: Option<&str> = None;
    let mut out_path: Option<String> = None;
    let mut mode = StructureMode::Auto;
    // [#3261] --json: 계약 봉투(schemaVersion·source)를 씌운 한 줄 JSON.
    // 기본 출력(무봉투 pretty JSON·-o 파일 저장)은 기존 소비자 계약이라 건드리지 않는다.
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "-o" | "--out" => {
                i += 1;
                match args.get(i) {
                    Some(p) => out_path = Some(p.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--mode" => {
                i += 1;
                match args.get(i).and_then(|s| StructureMode::parse(s)) {
                    Some(m) => mode = m,
                    None => {
                        eprintln!("오류: --mode 는 auto|outline|clause");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }

    let Some(file_path) = file_path else {
        eprintln!(
            "사용법: rhwp export-structure <파일> [--mode auto|outline|clause] [-o out.json]"
        );
        return EXIT_USAGE;
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let st = build_structure(doc.document(), mode);

    if json_mode {
        // [#3261] 봉투는 한 줄 — NDJSON(batch)과 같은 스키마로 단건/배치 동일 소비.
        let envelope = structure_json_value(file_path, &st);
        println!("{envelope}");
        return EXIT_OK;
    }

    let json = match serde_json::to_string_pretty(&st) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("오류: JSON 직렬화 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    match out_path {
        Some(p) => match fs::write(&p, &json) {
            Ok(_) => {
                println!(
                    "구조 추출 완료: mode={} 노드={} → {}",
                    st.mode, st.node_count, p
                );
                EXIT_OK
            }
            Err(e) => {
                eprintln!("오류: 출력 쓰기 실패 - {}: {}", p, e);
                // [#2707] 출력 파일을 못 쓴 실행은 실패다.
                EXIT_RUNTIME
            }
        },
        None => {
            println!("{json}");
            EXIT_OK
        }
    }
}


pub(crate) fn parse_grid_mm(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let number = trimmed
        .strip_suffix("mm")
        .or_else(|| trimmed.strip_suffix("MM"))
        .unwrap_or(trimmed)
        .trim();
    let mm = number.parse::<f64>().ok()?;
    if mm.is_finite() && mm > 0.0 {
        Some(mm)
    } else {
        None
    }
}



pub(crate) fn parse_grid_origin_option(value: &str) -> Option<GridOriginOption> {
    if value.eq_ignore_ascii_case("auto") {
        return Some(GridOriginOption::AutoPaper);
    }
    parse_grid_origin_mm(value).map(GridOriginOption::Fixed)
}


pub(crate) fn parse_grid_origin_mm(value: &str) -> Option<(f64, f64)> {
    let (x, y) = value.split_once(',')?;
    Some((parse_grid_mm(x)?, parse_grid_mm(y)?))
}


pub(crate) fn grid_paper_origin_mm(doc: &rhwp::wasm_api::HwpDocument, page_num: u32) -> Option<(f64, f64)> {
    let page_info = doc.get_page_info_native(page_num).ok()?;
    let page_info: serde_json::Value = serde_json::from_str(&page_info).ok()?;
    let section_idx = page_info.get("sectionIndex")?.as_u64()? as usize;
    let page_def = &doc
        .document()
        .sections
        .get(section_idx)?
        .section_def
        .page_def;
    Some((
        hu_to_mm(page_def.margin_left),
        hu_to_mm(page_def.margin_top + page_def.margin_header),
    ))
}



/// SVG에 mm 단위 점 격자 오버레이를 삽입한다.
/// export-svg 디버그용 격자는 한컴오피스의 "종이 기준 위치"를 옵션으로 맞출 수 있다.
pub(crate) fn insert_grid_overlay(svg: &str, grid_mm: f64, origin_mm: (f64, f64)) -> String {
    // SVG viewBox에서 크기 추출
    let (width, height) = extract_svg_dimensions(svg);
    // 96dpi: 1inch = 25.4mm, 1px = 25.4/96 = 0.2646mm.
    let grid_size = 96.0 / 25.4 * grid_mm;
    let origin_x = 96.0 / 25.4 * origin_mm.0;
    let origin_y = 96.0 / 25.4 * origin_mm.1;

    let g = format!("{:.4}", grid_size);
    let ox = format!("{:.4}", origin_x);
    let oy = format!("{:.4}", origin_y);
    let w = format!("{:.2}", width);
    let h = format!("{:.2}", height);
    let defs_part = format!(
        "<defs><pattern id=\"rhwp-grid\" x=\"{ox}\" y=\"{oy}\" width=\"{g}\" height=\"{g}\" patternUnits=\"userSpaceOnUse\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\" fill=\"#002096\" fill-opacity=\"0.9\"/></pattern></defs>"
    );
    let grid_rect = format!("\n<rect width=\"{w}\" height=\"{h}\" fill=\"url(#rhwp-grid)\"/>");
    let grid_defs =
        format!("{defs_part}\n<rect width=\"{w}\" height=\"{h}\" fill=\"url(#rhwp-grid)\"/>\n");

    // 페이지 배경(fill="#ffffff") rect 직후에 격자를 삽입
    // 이렇게 해야 흰색 배경 위에, 본문 컨텐츠 아래에 격자가 표시됨
    let bg_pattern = "fill=\"#ffffff\"/>";
    if let Some(pos) = svg.find(bg_pattern) {
        let insert_pos = pos + bg_pattern.len();
        // defs는 SVG 시작 부분에, 격자 rect는 배경 뒤에
        // defs를 <svg> 태그 직후에 삽입
        let mut result = svg.to_string();
        // 배경 rect 뒤에 격자 rect 삽입
        result.insert_str(insert_pos, &grid_rect);
        // <svg ...>\n 직후에 defs 삽입
        if let Some(svg_end) = result.find(">\n") {
            result.insert_str(svg_end + 2, &format!("{}\n", defs_part));
        }
        result
    } else {
        // 배경 rect가 없으면 기존 방식
        if let Some(pos) = svg.find(">\n") {
            let insert_pos = pos + 2;
            format!("{}{}{}", &svg[..insert_pos], grid_defs, &svg[insert_pos..])
        } else {
            svg.to_string()
        }
    }
}



/// SVG의 width/height 속성 또는 viewBox에서 크기를 추출한다.
pub(crate) fn extract_svg_dimensions(svg: &str) -> (f64, f64) {
    // viewBox="0 0 W H" 패턴에서 추출
    if let Some(vb_start) = svg.find("viewBox=\"") {
        let vb = &svg[vb_start + 9..];
        if let Some(vb_end) = vb.find('"') {
            let parts: Vec<&str> = vb[..vb_end].split_whitespace().collect();
            if parts.len() == 4 {
                let w: f64 = parts[2].parse().unwrap_or(800.0);
                let h: f64 = parts[3].parse().unwrap_or(1100.0);
                return (w, h);
            }
        }
    }
    // width/height 속성에서 추출
    let w = extract_attr_f64(svg, "width").unwrap_or(800.0);
    let h = extract_attr_f64(svg, "height").unwrap_or(1100.0);
    (w, h)
}


pub(crate) fn extract_attr_f64(svg: &str, attr: &str) -> Option<f64> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = svg.find(&pattern) {
        let val = &svg[start + pattern.len()..];
        if let Some(end) = val.find('"') {
            return val[..end].trim_end_matches("px").parse().ok();
        }
    }
    None
}



#[cfg(not(feature = "native-skia"))]
pub(crate) fn export_png(_args: &[String]) -> i32 {
    eprintln!("오류: export-png 명령은 native-skia feature 가 활성화되어야 합니다.");
    eprintln!("       cargo build --release --features native-skia");
    // [#2707] 기능이 아예 빌드되지 않은 바이너리다. 0으로 끝내면 스크립트가 성공으로 읽는다.
    EXIT_USAGE
}



#[cfg(feature = "native-skia")]
pub(crate) fn export_png(args: &[String]) -> i32 {
    use rhwp::document_core::queries::rendering::{PngExportOptions, VlmTarget};

    // [#3359] 위치 인자 파싱은 export-structure/export-text(#3349) 규약과 동일.
    let mut file_path: Option<&str> = None;
    let mut output_dir = "output".to_string();
    let mut target_page: Option<u32> = None;
    let mut font_paths: Vec<std::path::PathBuf> = Vec::new();
    let mut scale: Option<f64> = None;
    let mut max_dimension: Option<i32> = None;
    let mut vlm_target: Option<VlmTarget> = None;
    let mut dpi: Option<f64> = None;
    // PNG export is print-equivalent output. Editor visuals require an explicit screen profile.
    let mut render_profile = rhwp::paint::RenderProfile::HighQuality;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_dir = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("오류: --output 뒤에 폴더 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--page" | "-p" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u32>() {
                        Ok(n) => target_page = Some(n),
                        Err(_) => {
                            eprintln!("오류: 페이지 번호가 올바르지 않습니다.");
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --page 뒤에 페이지 번호가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--profile" => {
                if i + 1 < args.len() {
                    let Some(profile) = rhwp::paint::RenderProfile::parse(&args[i + 1]) else {
                        eprintln!(
                            "오류: --profile 값이 올바르지 않습니다 (screen|print|high-quality|fast-preview)."
                        );
                        return EXIT_USAGE;
                    };
                    render_profile = profile;
                    i += 2;
                } else {
                    eprintln!("오류: --profile 뒤에 프로필 이름이 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--font-path" => {
                if i + 1 < args.len() {
                    font_paths.push(std::path::PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("오류: --font-path 뒤에 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--scale" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<f64>() {
                        Ok(s) if s.is_finite() && s > 0.0 => scale = Some(s),
                        _ => {
                            eprintln!("오류: --scale 값이 올바르지 않습니다 (양수 실수 필요).");
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --scale 뒤에 배율 값이 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--max-dimension" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<i32>() {
                        Ok(n) if n > 0 => max_dimension = Some(n),
                        _ => {
                            eprintln!(
                                "오류: --max-dimension 값이 올바르지 않습니다 (양수 정수 필요)."
                            );
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --max-dimension 뒤에 픽셀 값이 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--dpi" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<f64>() {
                        Ok(d) if d.is_finite() && d > 0.0 => dpi = Some(d),
                        _ => {
                            eprintln!("오류: --dpi 값이 올바르지 않습니다 (양수 실수 필요).");
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --dpi 뒤에 DPI 값이 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--vlm-target" => {
                if i + 1 < args.len() {
                    match VlmTarget::from_str(&args[i + 1]) {
                        Some(t) => vlm_target = Some(t),
                        None => {
                            eprintln!(
                                "오류: --vlm-target 값이 올바르지 않습니다 (지원: {}).",
                                VlmTarget::all_names()
                            );
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --vlm-target 뒤에 프리셋 이름이 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
                i += 1;
            }
        }
    }

    let Some(file_path) = file_path else {
        eprintln!("오류: HWP 파일 경로를 지정해주세요.");
        eprintln!("사용법: rhwp export-png <파일.hwp> [옵션] (rhwp --help 참조)");
        return EXIT_USAGE;
    };

    let png_options = PngExportOptions {
        scale,
        max_dimension,
        vlm_target,
        dpi,
        font_paths: font_paths.clone(),
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let mut core = match load_document_core(&data) {
        Ok(c) => c,
        Err(e) => return e.report(),
    };

    // [#3302] 외부 연결 그림(HWP3 pic_type=0 등)의 같은 디렉터리 자동 적재 — export-svg
    // 의 #741 규칙과 동일. 누락 시 skia 렌더가 회색 placeholder 를 그린다 (SO-SUEOP 1쪽 실측).
    if allows_implicit_sibling_resources(rhwp::parser::detect_format(&data)) {
        if let Some(parent) = Path::new(file_path).parent() {
            let _loaded = core.populate_external_images_from_dir(parent);
        }
    }

    let page_count = core.page_count();
    println!("문서 로드 완료: {} ({}페이지)", file_path, page_count);

    let output_path = Path::new(&output_dir);
    if !output_path.exists() {
        if let Err(e) = fs::create_dir_all(output_path) {
            eprintln!(
                "오류: 출력 폴더를 생성할 수 없습니다 - {}: {}",
                output_dir, e
            );
            return EXIT_RUNTIME;
        }
    }

    let pages: Vec<u32> = match target_page {
        Some(p) => {
            if p >= page_count as u32 {
                eprintln!(
                    "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                    page_count - 1
                );
                return EXIT_USAGE;
            }
            vec![p]
        }
        None => (0..page_count as u32).collect(),
    };

    let file_stem = Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");

    let total_pages = pages.len();
    let mut success = 0;
    let mut total_bytes = 0usize;

    for page_num in &pages {
        let has_options = png_options.scale.is_some()
            || png_options.max_dimension.is_some()
            || png_options.vlm_target.is_some()
            || png_options.dpi.is_some()
            || render_profile != rhwp::paint::RenderProfile::Screen;
        let result = if has_options {
            core.render_page_png_native_with_profile_and_export_options(
                *page_num,
                render_profile,
                &png_options,
            )
        } else if !font_paths.is_empty() {
            core.render_page_png_native_with_fonts(*page_num, &font_paths)
        } else {
            core.render_page_png_native(*page_num)
        };
        match result {
            Ok(png_bytes) => {
                let png_filename = if total_pages == 1 {
                    format!("{}.png", file_stem)
                } else {
                    format!("{}_{:03}.png", file_stem, page_num + 1)
                };
                let png_path = output_path.join(&png_filename);
                if let Err(e) = fs::write(&png_path, &png_bytes) {
                    eprintln!("오류: 페이지 {} PNG 저장 실패 - {}", page_num + 1, e);
                    continue;
                }
                println!("  → {} ({} bytes)", png_path.display(), png_bytes.len());
                total_bytes += png_bytes.len();
                success += 1;
            }
            Err(e) => {
                eprintln!("오류: 페이지 {} 렌더링 실패 - {:?}", page_num + 1, e);
            }
        }
    }

    println!(
        "내보내기 완료: {}개 PNG 파일 → {}/ ({:.1} MB)",
        success,
        output_dir,
        total_bytes as f64 / 1024.0 / 1024.0
    );

    // [#2707] 성공 수 집계는 이미 정확했지만 종료 코드가 항상 0이었다.
    if success == total_pages {
        EXIT_OK
    } else {
        EXIT_RUNTIME
    }
}


pub(crate) fn export_pdf(args: &[String]) -> i32 {
    if args.first().is_some_and(|a| a == "--help" || a == "-h") {
        print_export_pdf_usage();
        return 0;
    }

    #[cfg(target_arch = "wasm32")]
    {
        eprintln!("오류: PDF 내보내기는 native 빌드에서만 지원됩니다.");
        return 1;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // [#3359] 위치 인자 파싱은 export-structure/export-text(#3349) 규약과 동일.
        let mut file_path: Option<&str> = None;
        let mut output_file = String::new();
        let mut target_page: Option<u32> = None;
        let mut pdf_backend = rhwp::renderer::pdf::PdfBackend::default();
        let mut pdf_options = rhwp::renderer::pdf::PdfExportOptions::default();
        let mut direct_pdf_options = rhwp::renderer::pdf::DirectPdfExportOptions::default();
        let mut render_profile: Option<rhwp::paint::RenderProfile> = None;
        let mut compatibility_only_options = Vec::new();
        let mut direct_raster_dpi_was_set = false;
        // [#3596] --json: 산출물 매니페스트를 stdout 순수 JSON 으로. 렌더 동작 무변경.
        let mut json_mode = false;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--json" => {
                    json_mode = true;
                    i += 1;
                }
                "--output" | "-o" => {
                    if i + 1 < args.len() {
                        output_file = args[i + 1].clone();
                        i += 2;
                    } else {
                        eprintln!("오류: --output 뒤에 파일 경로가 필요합니다.");
                        return 2;
                    }
                }
                "--page" | "-p" => {
                    if i + 1 < args.len() {
                        match args[i + 1].parse::<u32>() {
                            Ok(n) => target_page = Some(n),
                            Err(_) => {
                                eprintln!("오류: 페이지 번호가 올바르지 않습니다.");
                                return 2;
                            }
                        }
                        i += 2;
                    } else {
                        eprintln!("오류: --page 뒤에 페이지 번호가 필요합니다.");
                        return 2;
                    }
                }
                "--profile" => {
                    if i + 1 < args.len() {
                        render_profile = rhwp::paint::RenderProfile::parse(&args[i + 1]);
                        if render_profile.is_none() {
                            eprintln!(
                                "오류: --profile 값이 올바르지 않습니다 (screen|print|high-quality|fast-preview)."
                            );
                            return 2;
                        }
                        i += 2;
                    } else {
                        eprintln!("오류: --profile 뒤에 프로필 이름이 필요합니다.");
                        return 2;
                    }
                }
                "--backend" => {
                    if i + 1 < args.len() {
                        let Some(backend) = rhwp::renderer::pdf::PdfBackend::parse(&args[i + 1])
                        else {
                            eprintln!("오류: --backend 값이 올바르지 않습니다 (svg|direct).");
                            return 2;
                        };
                        pdf_backend = backend;
                        i += 2;
                    } else {
                        eprintln!("오류: --backend 뒤에 backend 이름이 필요합니다.");
                        return 2;
                    }
                }
                arg if arg.starts_with("--backend=") => {
                    let Some(backend) = rhwp::renderer::pdf::PdfBackend::parse(
                        arg.trim_start_matches("--backend="),
                    ) else {
                        eprintln!("오류: --backend 값이 올바르지 않습니다 (svg|direct).");
                        return 2;
                    };
                    pdf_backend = backend;
                    i += 1;
                }
                "--raster-dpi" => {
                    if i + 1 < args.len() {
                        let Ok(raster_dpi) = args[i + 1].parse::<f32>() else {
                            eprintln!("오류: --raster-dpi 값은 양수여야 합니다.");
                            return 2;
                        };
                        if !raster_dpi.is_finite() || raster_dpi <= 0.0 {
                            eprintln!("오류: --raster-dpi 값은 양수여야 합니다.");
                            return 2;
                        }
                        direct_pdf_options.raster_dpi = raster_dpi;
                        direct_raster_dpi_was_set = true;
                        i += 2;
                    } else {
                        eprintln!("오류: --raster-dpi 뒤에 DPI 값이 필요합니다.");
                        return 2;
                    }
                }
                arg if arg.starts_with("--raster-dpi=") => {
                    let Ok(raster_dpi) = arg.trim_start_matches("--raster-dpi=").parse::<f32>()
                    else {
                        eprintln!("오류: --raster-dpi 값은 양수여야 합니다.");
                        return 2;
                    };
                    if !raster_dpi.is_finite() || raster_dpi <= 0.0 {
                        eprintln!("오류: --raster-dpi 값은 양수여야 합니다.");
                        return 2;
                    }
                    direct_pdf_options.raster_dpi = raster_dpi;
                    direct_raster_dpi_was_set = true;
                    i += 1;
                }
                "--font-path" => {
                    if i + 1 < args.len() {
                        pdf_options
                            .font_paths
                            .push(std::path::PathBuf::from(&args[i + 1]));
                        i += 2;
                    } else {
                        eprintln!("오류: --font-path 뒤에 경로가 필요합니다.");
                        return 2;
                    }
                }
                // (docs/RHWP_GLYPH_OUTLINE_CACHE_PLAN.md) 미리 계산된 glyph
                // outline 캐시를 조회 — 파일이 없거나 --font-path 폰트 집합과
                // 어긋나면(해시 불일치) 경고 후 오늘과 동일하게 매번 새로 계산한다.
                "--glyph-cache" => {
                    if i + 1 < args.len() {
                        pdf_options.glyph_cache_path = Some(std::path::PathBuf::from(&args[i + 1]));
                        i += 2;
                    } else {
                        eprintln!("오류: --glyph-cache 뒤에 파일 경로가 필요합니다.");
                        return 2;
                    }
                }
                // 이번 렌더에서 계산된(또는 --glyph-cache로 불러온 뒤 이번
                // 렌더로 보강된) glyph outline을 이 경로에 기록한다.
                "--dump-glyph-cache" => {
                    if i + 1 < args.len() {
                        pdf_options.dump_glyph_cache_path =
                            Some(std::path::PathBuf::from(&args[i + 1]));
                        i += 2;
                    } else {
                        eprintln!("오류: --dump-glyph-cache 뒤에 파일 경로가 필요합니다.");
                        return 2;
                    }
                }
                "--fallback-serif" => {
                    compatibility_only_options.push("--fallback-serif");
                    if i + 1 < args.len() {
                        pdf_options.fallback_serif = args[i + 1].clone();
                        i += 2;
                    } else {
                        eprintln!("오류: --fallback-serif 뒤에 폰트 family가 필요합니다.");
                        return 2;
                    }
                }
                arg if arg.starts_with("--fallback-serif=") => {
                    compatibility_only_options.push("--fallback-serif");
                    pdf_options.fallback_serif =
                        arg.trim_start_matches("--fallback-serif=").to_string();
                    i += 1;
                }
                "--fallback-sans" | "--fallback-sans-serif" => {
                    compatibility_only_options.push("--fallback-sans");
                    if i + 1 < args.len() {
                        pdf_options.fallback_sans = args[i + 1].clone();
                        i += 2;
                    } else {
                        eprintln!("오류: --fallback-sans 뒤에 폰트 family가 필요합니다.");
                        return 2;
                    }
                }
                arg if arg.starts_with("--fallback-sans=")
                    || arg.starts_with("--fallback-sans-serif=") =>
                {
                    compatibility_only_options.push("--fallback-sans");
                    pdf_options.fallback_sans = arg
                        .strip_prefix("--fallback-sans=")
                        .or_else(|| arg.strip_prefix("--fallback-sans-serif="))
                        .unwrap_or_default()
                        .to_string();
                    i += 1;
                }
                "--fallback-mono" | "--fallback-monospace" => {
                    compatibility_only_options.push("--fallback-mono");
                    if i + 1 < args.len() {
                        pdf_options.fallback_mono = args[i + 1].clone();
                        i += 2;
                    } else {
                        eprintln!("오류: --fallback-mono 뒤에 폰트 family가 필요합니다.");
                        return 2;
                    }
                }
                arg if arg.starts_with("--fallback-mono=")
                    || arg.starts_with("--fallback-monospace=") =>
                {
                    compatibility_only_options.push("--fallback-mono");
                    pdf_options.fallback_mono = arg
                        .strip_prefix("--fallback-mono=")
                        .or_else(|| arg.strip_prefix("--fallback-monospace="))
                        .unwrap_or_default()
                        .to_string();
                    i += 1;
                }
                // [Task #2264] 텍스트를 PDF 폰트로 임베드하지 않고 path 로 변환한다.
                // 폰트 서브셋 경로를 건너뛰어 메모리를 크게 줄이는 대신,
                // PDF 의 텍스트 선택·검색 기능을 잃는다 (시각적 출력은 동일).
                "--text-as-paths" => {
                    compatibility_only_options.push("--text-as-paths");
                    pdf_options.embed_text = false;
                    i += 1;
                }
                "--equation-font" | "--equation-font-family" => {
                    compatibility_only_options.push("--equation-font");
                    if i + 1 < args.len() {
                        pdf_options.equation_font = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("오류: --equation-font 뒤에 폰트 family가 필요합니다.");
                        return 2;
                    }
                }
                arg if arg.starts_with("--equation-font=")
                    || arg.starts_with("--equation-font-family=") =>
                {
                    compatibility_only_options.push("--equation-font");
                    pdf_options.equation_font = Some(
                        arg.strip_prefix("--equation-font=")
                            .or_else(|| arg.strip_prefix("--equation-font-family="))
                            .unwrap_or_default()
                            .to_string(),
                    );
                    i += 1;
                }
                other if other.starts_with('-') => {
                    eprintln!("알 수 없는 옵션: {other}");
                    print_export_pdf_usage();
                    return 2;
                }
                other => {
                    if file_path.replace(other).is_some() {
                        eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                        return 2;
                    }
                    i += 1;
                }
            }
        }

        let Some(file_path) = file_path else {
            eprintln!("오류: 문서 파일 경로를 지정해주세요.");
            print_export_pdf_usage();
            return 2;
        };

        compatibility_only_options.sort_unstable();
        compatibility_only_options.dedup();
        if pdf_backend == rhwp::renderer::pdf::PdfBackend::DirectLayer
            && !compatibility_only_options.is_empty()
        {
            eprintln!(
                "오류: direct PDF backend는 다음 SVG 호환 옵션을 지원하지 않습니다: {}",
                compatibility_only_options.join(", ")
            );
            return 2;
        }
        if pdf_backend == rhwp::renderer::pdf::PdfBackend::CompatibilitySvg
            && direct_raster_dpi_was_set
        {
            eprintln!("오류: --raster-dpi는 direct PDF backend에서만 사용할 수 있습니다.");
            return 2;
        }

        // 기본 출력 파일명
        if output_file.is_empty() {
            let stem = Path::new(file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            output_file = format!("output/{}.pdf", stem);
        }

        let read_start = std::time::Instant::now();
        let data = match fs::read(file_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
                return 1;
            }
        };
        let read_ms = read_start.elapsed().as_millis();

        let parse_start = std::time::Instant::now();
        let mut doc = match load_document(&data) {
            Ok(d) => d,
            Err(e) => return e.report(),
        };
        let parse_ms = parse_start.elapsed().as_millis();

        // [#3302] 외부 연결 그림 같은 디렉터리 자동 적재 — export-svg/export-png 와 동일 규칙.
        if allows_implicit_sibling_resources(rhwp::parser::detect_format(&data)) {
            if let Some(parent) = Path::new(file_path).parent() {
                let _loaded = doc.populate_external_images_from_dir(parent);
            }
        }

        let page_count = doc.page_count();
        if !json_mode {
            println!("문서 로드 완료: {} ({}페이지)", file_path, page_count);
        }
        if page_count == 0 {
            eprintln!("오류: PDF로 내보낼 페이지가 없습니다.");
            return 1;
        }

        // 출력 디렉토리 생성
        if let Some(parent) = Path::new(&output_file).parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!("오류: 출력 디렉토리를 만들 수 없습니다 - {}", e);
                    return 1;
                }
            }
        }

        // 페이지 범위 결정
        let pages: Vec<u32> = match target_page {
            Some(p) => {
                if p >= page_count {
                    eprintln!(
                        "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                        page_count - 1
                    );
                    return 2;
                }
                vec![p]
            }
            None => (0..page_count).collect(),
        };

        // [벤더 패치: phase 타이밍] SVG 레이아웃/생성(render)과 SVG→PDF 변환(convert,
        // usvg::Tree::from_str + svg2pdf::to_chunk — export-pdf 총 시간의 ~75%를 차지하는
        // 구간, src/renderer/pdf.rs의 병렬화 패치가 최적화한 바로 그 구간)을 분리 계측한다.
        // render_pages_pdf_native_with_*options()가 내부에서 하던 두 단계를 여기서 그대로
        // 풀어써서, 라이브러리 공개 API(반환 타입 등)는 건드리지 않는다. DirectLayer
        // backend는 SVG 중간 표현 없이 레이어 트리를 직접 PDF로 그리므로 render/convert로
        // 나눌 지점이 없다 — 전체를 convertMs 로 계상한다 (renderMs=0).
        let render_ms: u128;
        let convert_ms: u128;
        let pdf_result = match pdf_backend {
            rhwp::renderer::pdf::PdfBackend::CompatibilitySvg => {
                let render_start = std::time::Instant::now();
                let svg_pages_result: Result<Vec<String>, _> = match render_profile {
                    Some(profile) => pages
                        .iter()
                        .map(|&p| doc.render_page_svg_layer_with_profile_native(p, profile))
                        .collect(),
                    None => pages
                        .iter()
                        .map(|&p| doc.render_page_svg_native(p))
                        .collect(),
                };
                render_ms = render_start.elapsed().as_millis();
                let svg_pages = match svg_pages_result {
                    Ok(pages) => pages,
                    Err(e) => {
                        eprintln!("오류: PDF 변환 실패 - {}", e);
                        return 1;
                    }
                };
                let convert_start = std::time::Instant::now();
                let result =
                    rhwp::renderer::pdf::svgs_to_pdf_with_options(&svg_pages, &pdf_options)
                        .map_err(rhwp::error::HwpError::RenderError);
                convert_ms = convert_start.elapsed().as_millis();
                result
            }
            rhwp::renderer::pdf::PdfBackend::DirectLayer => {
                render_ms = 0;
                let convert_start = std::time::Instant::now();
                let result = {
                    #[cfg(feature = "native-skia")]
                    {
                        direct_pdf_options.font_paths = pdf_options.font_paths.clone();
                        doc.render_pages_pdf_direct_native_with_profile_and_options(
                            &pages,
                            render_profile.unwrap_or(rhwp::paint::RenderProfile::Print),
                            &direct_pdf_options,
                        )
                    }
                    #[cfg(not(feature = "native-skia"))]
                    {
                        Err(rhwp::error::HwpError::RenderError(
                            "direct PDF backend requires a build with the native-skia feature"
                                .to_string(),
                        ))
                    }
                };
                convert_ms = convert_start.elapsed().as_millis();
                result
            }
        };
        let pdf_bytes = match pdf_result {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("오류: PDF 변환 실패 - {}", e);
                return 1;
            }
        };

        let write_start = std::time::Instant::now();
        if let Err(e) = fs::write(&output_file, &pdf_bytes) {
            eprintln!("오류: PDF 저장 실패 - {}", e);
            return 1;
        }
        let write_ms = write_start.elapsed().as_millis();

        eprintln!(
            "RHWP_TIMING {{\"cmd\":\"export-pdf\",\"readMs\":{},\"parseMs\":{},\"renderMs\":{},\"convertMs\":{},\"writeMs\":{}}}",
            read_ms, parse_ms, render_ms, convert_ms, write_ms
        );

        if json_mode {
            let backend_name = match pdf_backend {
                rhwp::renderer::pdf::PdfBackend::CompatibilitySvg => "svg",
                rhwp::renderer::pdf::PdfBackend::DirectLayer => "direct",
            };
            println!(
                "{}",
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                        "source": file_path,
                        "format": "pdf",
                        "backend": backend_name,
                        "output": output_file,
                        "bytes": pdf_bytes.len(),
                        "pageCount": page_count,
                        "renderedCount": pages.len(),
                    }),
                    "export-pdf",
                )
            );
        } else {
            println!(
                "  → {} ({}KB, {}페이지)",
                output_file,
                pdf_bytes.len() / 1024,
                pages.len()
            );
            if pdf_backend == rhwp::renderer::pdf::PdfBackend::DirectLayer {
                println!("PDF backend: direct");
            }
            println!("PDF 내보내기 완료");
        }
        0
    }
}


pub(crate) fn print_export_pdf_usage() {
    eprintln!("사용법: rhwp export-pdf <파일.hwp|파일.hwpx|파일.hml> [옵션]");
    eprintln!("  -o, --output <파일>       출력 PDF 파일");
    eprintln!("  -p, --page <번호>        특정 페이지만 내보내기 (0부터 시작)");
    eprintln!("      --json               산출물 매니페스트를 stdout 에 JSON 으로 출력");
    eprintln!("      --backend <svg|direct> PDF backend (기본값: svg)");
    eprintln!(
        "      --profile <프로필>   layer 출력 프로필 (screen|print|high-quality|fast-preview)"
    );
    eprintln!("      --raster-dpi <DPI>    direct backend fallback raster DPI (기본값: 144)");
    eprintln!("      --font-path <경로>   폰트 파일 탐색 경로 (여러 번 지정 가능)");
    eprintln!("      --glyph-cache <파일>       미리 계산된 glyph outline 캐시 조회");
    eprintln!("      --dump-glyph-cache <파일>  이번 렌더의 glyph outline을 캐시 파일로 기록");
    eprintln!("      --fallback-serif <명>");
    eprintln!("      --fallback-sans <명>");
    eprintln!("      --fallback-mono <명>");
    eprintln!("      --equation-font <명>");
    eprintln!("  direct backend는 native-skia feature로 빌드한 native CLI가 필요합니다.");
    eprintln!("  참고: <...>는 자리표시자이며, 실제 입력에는 꺾쇠괄호를 쓰지 않습니다.");
    eprintln!("        공백 없는 값: --font-path ./ttfs");
    eprintln!(
        "        공백 포함 값은 큰따옴표 권장: --font-path \"./My Fonts\", --fallback-sans \"Apple SD Gothic Neo\""
    );
    eprintln!("        작은따옴표는 zsh/bash/PowerShell에서 literal 값이 필요할 때만 사용합니다.");
}


pub(crate) fn export_text(args: &[String]) -> i32 {
    // [#3237] --json: 결과를 파일 대신 stdout JSON 으로 낸다. stdout 은 순수 JSON 이어야
    // 하므로 이 모드에서는 진행 메시지를 찍지 않는다. 위치 무관 플래그다 (info 와 동일 규약).
    let json_mode = args.iter().any(|a| a == "--json");
    let args: Vec<String> = args
        .iter()
        .filter(|a| a.as_str() != "--json")
        .cloned()
        .collect();
    // [#3349] 위치 인자 파싱을 export-structure/export-tables 규약으로 통일 —
    // 첫 비플래그 토큰이 파일이고 옵션은 위치 무관이다. 파일 선행을 강제하면
    // `-p 0 --json 파일` 에서 `-p` 가 파일로 잡혀 "알 수 없는 옵션: 0" 이 된다.
    let mut file_path: Option<&str> = None;
    let mut output_dir = "output".to_string();
    let mut target_page: Option<u32> = None;
    // [#3787 S7] 기본은 **무제한**이다 — 종전 호출의 산출을 조용히 줄이지 않는다.
    let mut max_chars: Option<usize> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                match args.get(i) {
                    Some(p) => output_dir = p.clone(),
                    None => {
                        eprintln!("오류: --output 뒤에 폴더 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--max-chars" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(n) if n >= 1 => max_chars = Some(n),
                    _ => {
                        eprintln!("오류: --max-chars 뒤에 1 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--page" | "-p" => {
                i += 1;
                match args.get(i) {
                    Some(v) => match v.parse::<u32>() {
                        Ok(n) => target_page = Some(n),
                        Err(_) => {
                            eprintln!("오류: 페이지 번호가 올바르지 않습니다.");
                            return EXIT_USAGE;
                        }
                    },
                    None => {
                        eprintln!("오류: --page 뒤에 페이지 번호가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }

    let Some(file_path) = file_path else {
        eprintln!("오류: HWP 파일 경로를 지정해주세요.");
        eprintln!("사용법: rhwp export-text <파일.hwp> [옵션] (rhwp --help 참조)");
        return EXIT_USAGE;
    };

    // [#3787 S7] `--max-chars` 는 **에이전트 컨텍스트**를 지키는 상한이다. 파일
    // 저장 모드에는 지킬 컨텍스트가 없고, 거기서 조용히 잘린 .txt 를 남기면 절단
    // 사실을 실을 봉투조차 없다. 아무 일도 안 하는 플래그는 함정이므로 거부한다.
    if max_chars.is_some() && !json_mode {
        eprintln!(
            "오류: --max-chars 는 --json 과 함께 써야 합니다 (봉투에 절단 사실을 싣는 옵션)."
        );
        return EXIT_USAGE;
    }

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let page_count = doc.page_count();
    if !json_mode {
        println!("문서 로드 완료: {} ({}페이지)", file_path, page_count);
    }
    if page_count == 0 {
        eprintln!("오류: 문서에 페이지가 없습니다.");
        return EXIT_RUNTIME;
    }

    let output_path = Path::new(&output_dir);
    if !json_mode && !output_path.exists() {
        if let Err(e) = fs::create_dir_all(output_path) {
            eprintln!(
                "오류: 출력 폴더를 생성할 수 없습니다 - {}: {}",
                output_dir, e
            );
            return EXIT_RUNTIME;
        }
    }

    let pages: Vec<u32> = match target_page {
        Some(p) => {
            if p >= page_count {
                eprintln!(
                    "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                    page_count - 1
                );
                return EXIT_USAGE;
            }
            vec![p]
        }
        None => (0..page_count).collect(),
    };

    // [#3237] JSON 모드: 파일을 쓰지 않고 요청 페이지 전체를 stdout JSON 하나로 낸다.
    if json_mode {
        let mut extracted = Vec::with_capacity(pages.len());
        for page_num in &pages {
            match doc.extract_page_text_native(*page_num) {
                Ok(text) => extracted.push((*page_num, text)),
                Err(e) => {
                    eprintln!("오류: 페이지 {} 텍스트 추출 실패 - {}", page_num, e);
                    return EXIT_RUNTIME;
                }
            }
        }
        // [#3787 S7] 총량을 보고하려면 전수 추출이 불가피하다 — `--max-chars` 의 목적은
        // 추출 시간이 아니라 **출력 컨텍스트** 절약이므로 추출 후 표시만 절단한다
        // (`search --limit` 이 전수 grep 후 절단하는 것과 같은 이유, #3353).
        let (page_objs, omitted_count) = truncate_page_texts(&extracted, max_chars);
        let result = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "pageCount": page_objs.len(),
            "truncated": omitted_count > 0,
            "omittedCount": omitted_count,
            "pages": page_objs,
        });
        println!("{}", provenance::marked(result, "export-text"));
        return EXIT_OK;
    }

    let file_stem = Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");

    // [#2707] 요청한 페이지 수가 아니라 실제로 저장에 성공한 페이지 수를 센다.
    let mut written = 0usize;

    for page_num in &pages {
        match doc.extract_page_text_native(*page_num) {
            Ok(mut text) => {
                if !text.ends_with('\n') {
                    text.push('\n');
                }

                let txt_filename = if page_count == 1 {
                    format!("{}.txt", file_stem)
                } else {
                    format!("{}_{:03}.txt", file_stem, page_num + 1)
                };
                let txt_path = output_path.join(&txt_filename);

                match fs::write(&txt_path, text.as_bytes()) {
                    Ok(_) => {
                        println!("  → {}", txt_path.display());
                        written += 1;
                    }
                    Err(e) => eprintln!("오류: TXT 저장 실패 - {}: {}", txt_path.display(), e),
                }
            }
            Err(e) => {
                eprintln!("오류: 페이지 {} 텍스트 추출 실패 - {:?}", page_num, e);
            }
        }
    }

    println!(
        "텍스트 내보내기 완료: {}개 TXT 파일 → {}/",
        written, output_dir
    );

    // [#2707] 한 장이라도 못 썼으면 런타임 실패다.
    if written == pages.len() {
        EXIT_OK
    } else {
        EXIT_RUNTIME
    }
}



/// `export-tables` — 표를 격자 JSON 으로 추출 (병합·중첩 보존).
///
/// 평문·Markdown 추출은 병합(rowSpan/colSpan)을 잃어 소비자가 덮인 칸을 별개 열로
/// 오독한다. 본 명령은 `Table.cells`(앵커 셀 + span)를 그대로 직역해 격자를 보존한다.
pub(crate) fn export_tables(args: &[String]) -> i32 {
    use rhwp::document_core::queries::table_extract::extract_tables;

    let mut file_path: Option<&str> = None;
    let mut out_path: Option<String> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "-o" | "--out" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(p) => out_path = Some(p.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다.");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }

    let Some(file_path) = file_path else {
        eprintln!("사용법: rhwp export-tables <파일.hwp|파일.hwpx> [--json] [-o <출력.json>]");
        return EXIT_USAGE;
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let tables = extract_tables(doc.document());
    let envelope = tables_json_value(file_path, &tables);

    if let Some(p) = out_path {
        let json = match serde_json::to_string_pretty(&envelope) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("오류: JSON 직렬화 실패 - {}", e);
                return EXIT_RUNTIME;
            }
        };
        return match fs::write(&p, &json) {
            Ok(_) => {
                println!("표 추출 완료: {}개 → {}", tables.len(), p);
                EXIT_OK
            }
            Err(e) => {
                eprintln!("오류: 출력 쓰기 실패 - {}: {}", p, e);
                EXIT_RUNTIME
            }
        };
    }

    if json_mode {
        println!("{}", provenance::marked(envelope, "export-tables"));
        return EXIT_OK;
    }

    // 기본 출력은 사람용 요약 — 기계 소비는 --json 이 담당한다.
    println!("문서 로드: {} (표 {}개)", file_path, tables.len());
    for t in &tables {
        let merged = t
            .cells
            .iter()
            .filter(|c| c.row_span > 1 || c.col_span > 1)
            .count();
        let nested = t.cells.iter().filter(|c| !c.nested.is_empty()).count();
        println!(
            "  표{} [구역{}:문단{}]: {}행×{}열, 셀 {}개 (병합 {}개, 중첩 {}개)",
            t.index, t.section, t.paragraph, t.rows, t.cols, t.cell_count, merged, nested
        );
    }
    EXIT_OK
}



/// `table-to-csv` — 본문 최상위 표를 RFC 4180 CSV 로 내보낸다 (#3719 §6).
///
/// `export-tables` 의 격자 JSON 은 병합을 span 으로 보존하지만 표 계산기는 직사각
/// 격자만 먹는다. 앵커 셀을 그대로 이어 붙이면 병합 행에서 열이 밀리므로,
/// `table_csv::grid_to_csv` 가 격자를 채워서(덮인 칸 = 빈 문자열) 낸다.
pub(crate) fn table_to_csv(args: &[String]) -> i32 {
    use rhwp::document_core::queries::table_csv::grid_to_csv;
    use rhwp::document_core::queries::table_extract::extract_tables;

    let mut file_path: Option<&str> = None;
    let mut table_arg: Option<usize> = None;
    let mut out_path: Option<String> = None;
    let mut bom = false;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--bom" => bom = true,
            "--table" => {
                i += 1;
                match args.get(i).map(|v| v.parse::<usize>()) {
                    Some(Ok(value)) => table_arg = Some(value),
                    _ => {
                        eprintln!("오류: --table 뒤에 0 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--out" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(p) => out_path = Some(p.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }

    let Some(file_path) = file_path else {
        eprintln!(
            "사용법: rhwp table-to-csv <파일.hwp|파일.hwpx> [--table <번호>] [-o <경로>] [--bom] [--json]"
        );
        return EXIT_USAGE;
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: HWP 파싱 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    // 본문 최상위 표만 다룬다 — `edit set-cell`(resolve_table_cell)과 같은 좌표계라야
    // 내보낸 CSV 의 표 번호를 그대로 되돌려 쓸 수 있다. 중첩 표는 v1 범위 밖이다.
    let grids = extract_tables(doc.document());
    let top_level: Vec<&_> = grids
        .iter()
        .filter(|g| g.container_path.is_empty())
        .collect();
    let selected: Vec<&_> = match table_arg {
        Some(n) => match top_level.iter().find(|g| g.index == n) {
            Some(g) => vec![*g],
            None => {
                eprintln!(
                    "오류: 본문 최상위 표 {} 번이 없습니다 (최상위 표 {}개; 중첩 표는 v1 범위 밖).",
                    n,
                    top_level.len()
                );
                return EXIT_RUNTIME;
            }
        },
        None => top_level.clone(),
    };

    // 표별 CSV 본문. 격자 채움과 인용은 전부 코어(table_csv)가 한다.
    let bodies: Vec<(usize, u16, u16, String)> = selected
        .iter()
        .map(|g| (g.index, g.rows, g.cols, grid_to_csv(g)))
        .collect();

    // -o 의 뜻은 --table 유무로 갈린다: 한 표면 그 경로가 파일, 전부면 표별 파일을
    // 담을 디렉터리다(export-svg 의 -o 규약과 같은 이유 — 산출물이 여러 개다).
    let mut written: Vec<Option<String>> = vec![None; bodies.len()];
    if let Some(dest) = out_path.as_deref() {
        if table_arg.is_some() {
            let body = &bodies[0].3;
            if let Err(e) = write_csv_file(dest, body, bom) {
                eprintln!("오류: 출력 쓰기 실패 - {}: {}", dest, e);
                return EXIT_RUNTIME;
            }
            written[0] = Some(dest.to_string());
        } else {
            if let Err(e) = fs::create_dir_all(dest) {
                eprintln!("오류: 출력 폴더 생성 실패 - {}: {}", dest, e);
                return EXIT_RUNTIME;
            }
            for (slot, (index, _, _, body)) in written.iter_mut().zip(bodies.iter()) {
                let path = Path::new(dest).join(format!("table{index}.csv"));
                let shown = path.to_string_lossy().to_string();
                if let Err(e) = write_csv_file(&shown, body, bom) {
                    eprintln!("오류: 출력 쓰기 실패 - {}: {}", shown, e);
                    return EXIT_RUNTIME;
                }
                *slot = Some(shown);
            }
        }
    }

    if json_mode {
        let tables: Vec<serde_json::Value> = bodies
            .iter()
            .zip(written.iter())
            .map(|((index, rows, cols, body), out)| {
                let mut entry = serde_json::json!({
                    "index": index,
                    "rowCount": rows,
                    "colCount": cols,
                    "csv": body,
                });
                if let Some(p) = out {
                    entry["output"] = serde_json::Value::String(p.clone());
                }
                entry
            })
            .collect();
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "tableCount": tables.len(),
            "tables": tables,
            // BOM 은 **파일 인코딩** 표식이라 봉투의 csv 문자열에는 붙이지 않는다.
            // 붙이면 JSON 을 그대로 파싱하는 소비자가 첫 셀 앞의 U+FEFF 를 값으로 읽는다.
            "bom": bom,
        });
        if let Some(p) = out_path {
            envelope["output"] = serde_json::Value::String(p);
            envelope["outputFormat"] = serde_json::Value::String("csv".to_string());
        }
        println!("{}", provenance::marked(envelope, "table-to-csv"));
        return EXIT_OK;
    }

    if out_path.is_some() {
        println!("CSV 내보내기 완료: {} (표 {}개)", file_path, bodies.len());
        for out in written.iter().flatten() {
            println!("  {out}");
        }
        return EXIT_OK;
    }

    // -o 도 --json 도 없으면 CSV 본문을 그대로 stdout 으로 흘린다 — 파이프 사용.
    for (index, rows, cols, body) in &bodies {
        if bodies.len() > 1 {
            println!("# table{index} ({rows}x{cols})");
        }
        print!("{body}");
    }
    EXIT_OK
}



/// 차트 읽기 봉투에서 `(라벨, 계열명, 계열값, 분산형 여부)` 를 꺼낸다.
///
/// 값은 **문자열 그대로** 옮긴다 — 실수로 바꿨다 되쓰면 표기가 달라져 무편집 왕복의
/// 바이트 동일이 깨진다(코어가 문자열만 받는 이유와 같다).
pub(crate) fn chart_matrix_from_envelope(
    read: &serde_json::Value,
) -> (Vec<String>, Vec<String>, Vec<Vec<String>>, bool) {
    let strings = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|x| x.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    let labels = strings(&read["labels"]);
    let series = read["series"].as_array().cloned().unwrap_or_default();
    let names: Vec<String> = series
        .iter()
        .map(|s| s["name"].as_str().unwrap_or_default().to_string())
        .collect();
    let values: Vec<Vec<String>> = series.iter().map(|s| strings(&s["values"])).collect();
    let scatter = read["axis"].as_str() == Some("scatter");
    (labels, names, values, scatter)
}



/// `chart-to-csv` — 차트 숫자 데이터를 RFC 4180 CSV 로 내보낸다 (#4100).
///
/// 행 = 카테고리(분산형은 X), 열 = 계열. `table-to-csv` 의 `-o`·`--bom`·`--json` 규약을
/// 그대로 따른다 — 같은 도구로 왕복시킬 수 있어야 한다.
pub(crate) fn chart_to_csv(args: &[String]) -> i32 {
    use rhwp::document_core::queries::chart_csv::to_csv;
    use rhwp::document_core::queries::chart_extract::collect_charts;

    let mut file_path: Option<&str> = None;
    let mut chart_arg: Option<usize> = None;
    let mut out_path: Option<String> = None;
    let mut bom = false;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--bom" => bom = true,
            "--chart" => {
                i += 1;
                match args.get(i).map(|v| v.parse::<usize>()) {
                    Some(Ok(value)) if value >= 1 => chart_arg = Some(value),
                    _ => {
                        eprintln!("오류: --chart 뒤에 1 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--out" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(p) => out_path = Some(p.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }

    let Some(file_path) = file_path else {
        eprintln!(
            "사용법: rhwp chart-to-csv <파일.hwp|파일.hwpx> [--chart <번호>] [-o <경로>] [--bom] [--json]"
        );
        return EXIT_USAGE;
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: HWP 파싱 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    let chart_count = collect_charts(doc.document()).len();
    let selected: Vec<usize> = match chart_arg {
        Some(n) if n <= chart_count => vec![n - 1],
        Some(n) => {
            eprintln!("오류: 차트 {} 번이 없습니다 (차트 {}개).", n, chart_count);
            return EXIT_RUNTIME;
        }
        None => (0..chart_count).collect(),
    };

    let mut bodies: Vec<(usize, usize, usize, String)> = Vec::new();
    for index in selected {
        let read: serde_json::Value = match doc.get_chart_data_by_index_native(index).map(|s| {
            serde_json::from_str::<serde_json::Value>(&s).unwrap_or(serde_json::Value::Null)
        }) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: 차트 {} 읽기 실패 - {:?}", index + 1, e);
                return EXIT_RUNTIME;
            }
        };
        if read["ok"] != true {
            eprintln!(
                "오류: 차트 {} 를 읽을 수 없습니다 - {}",
                index + 1,
                read["invalid"][0]["message"]
                    .as_str()
                    .unwrap_or("사유 미상")
            );
            return EXIT_RUNTIME;
        }
        if read["labelsShared"] != true {
            let axis = if read["axis"].as_str() == Some("scatter") {
                "X 값"
            } else {
                "카테고리 라벨"
            };
            eprintln!(
                "오류: 차트 {}는 계열마다 {}이 달라 CSV 한 열로 안전하게 표현할 수 없습니다.",
                index + 1,
                axis
            );
            return EXIT_RUNTIME;
        }
        let (labels, names, values, scatter) = chart_matrix_from_envelope(&read);
        // 행 수는 값이 정한다 — 라벨이 없거나 짧은 차트가 실재한다(chart_csv::to_csv 주석).
        let rows = values
            .iter()
            .map(|s| s.len())
            .chain(std::iter::once(labels.len()))
            .max()
            .unwrap_or(0);
        bodies.push((
            index + 1,
            rows,
            names.len(),
            to_csv(&labels, &names, &values, scatter),
        ));
    }

    // -o 의 뜻은 --chart 유무로 갈린다 — `table-to-csv` 와 같은 규약(산출물이 여러 개다).
    let mut written: Vec<Option<String>> = vec![None; bodies.len()];
    if let Some(dest) = out_path.as_deref() {
        if chart_arg.is_some() {
            if let Err(e) = write_csv_file(dest, &bodies[0].3, bom) {
                eprintln!("오류: 출력 쓰기 실패 - {}: {}", dest, e);
                return EXIT_RUNTIME;
            }
            written[0] = Some(dest.to_string());
        } else {
            if let Err(e) = fs::create_dir_all(dest) {
                eprintln!("오류: 출력 폴더 생성 실패 - {}: {}", dest, e);
                return EXIT_RUNTIME;
            }
            for (slot, (number, _, _, body)) in written.iter_mut().zip(bodies.iter()) {
                let path = Path::new(dest).join(format!("chart{number}.csv"));
                let shown = path.to_string_lossy().to_string();
                if let Err(e) = write_csv_file(&shown, body, bom) {
                    eprintln!("오류: 출력 쓰기 실패 - {}: {}", shown, e);
                    return EXIT_RUNTIME;
                }
                *slot = Some(shown);
            }
        }
    }

    if json_mode {
        let charts: Vec<serde_json::Value> = bodies
            .iter()
            .zip(written.iter())
            .map(|((number, rows, cols, body), out)| {
                let mut entry = serde_json::json!({
                    "chart": number,
                    "rowCount": rows,
                    "colCount": cols,
                    "csv": body,
                });
                if let Some(p) = out {
                    entry["output"] = serde_json::Value::String(p.clone());
                }
                entry
            })
            .collect();
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "chartCount": charts.len(),
            "charts": charts,
            // BOM 은 파일 인코딩 표식이라 봉투의 csv 문자열에는 붙이지 않는다
            // (`table-to-csv` 와 같은 이유).
            "bom": bom,
        });
        if let Some(p) = out_path {
            envelope["output"] = serde_json::Value::String(p);
            envelope["outputFormat"] = serde_json::Value::String("csv".to_string());
        }
        println!("{}", provenance::marked(envelope, "chart-to-csv"));
        return EXIT_OK;
    }

    if out_path.is_some() {
        println!(
            "차트 CSV 내보내기 완료: {} (차트 {}개)",
            file_path,
            bodies.len()
        );
        for out in written.iter().flatten() {
            println!("  {out}");
        }
        return EXIT_OK;
    }

    for (number, rows, cols, body) in &bodies {
        if bodies.len() > 1 {
            println!("# chart{number} ({rows}x{cols})");
        }
        print!("{body}");
    }
    EXIT_OK
}



/// CSV 본문 하나를 파일로 쓴다 (선택적 UTF-8 BOM — 엑셀 한글 깨짐 방지).
pub(crate) fn write_csv_file(path: &str, body: &str, bom: bool) -> std::io::Result<()> {
    use rhwp::document_core::queries::table_csv::UTF8_BOM;
    let mut bytes = Vec::with_capacity(body.len() + 3);
    if bom {
        bytes.extend_from_slice(UTF8_BOM.as_bytes());
    }
    bytes.extend_from_slice(body.as_bytes());
    fs::write(path, bytes)
}



pub(crate) fn export_markdown(args: &[String]) -> i32 {
    // [#3359] 위치 인자 파싱은 export-structure/export-text(#3349) 규약과 동일.
    let mut file_path: Option<&str> = None;
    let mut output_dir = "output".to_string();
    let mut target_page: Option<u32> = None;
    // [#3596] --json: 산출물 매니페스트를 stdout 순수 JSON 으로. 추출 동작 무변경.
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_mode = true;
                i += 1;
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_dir = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("오류: --output 뒤에 폴더 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--page" | "-p" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u32>() {
                        Ok(n) => target_page = Some(n),
                        Err(_) => {
                            eprintln!("오류: 페이지 번호가 올바르지 않습니다.");
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: --page 뒤에 페이지 번호가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
                i += 1;
            }
        }
    }

    let Some(file_path) = file_path else {
        eprintln!("오류: HWP 파일 경로를 지정해주세요.");
        eprintln!("사용법: rhwp export-markdown <파일.hwp> [옵션] (rhwp --help 참조)");
        return EXIT_USAGE;
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let page_count = doc.page_count();
    if !json_mode {
        println!("문서 로드 완료: {} ({}페이지)", file_path, page_count);
    }
    if page_count == 0 {
        eprintln!("오류: 문서에 페이지가 없습니다.");
        return EXIT_RUNTIME;
    }

    let output_path = Path::new(&output_dir);
    if !output_path.exists() {
        if let Err(e) = fs::create_dir_all(output_path) {
            eprintln!(
                "오류: 출력 폴더를 생성할 수 없습니다 - {}: {}",
                output_dir, e
            );
            return EXIT_RUNTIME;
        }
    }

    let pages: Vec<u32> = match target_page {
        Some(p) => {
            if p >= page_count {
                eprintln!(
                    "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                    page_count - 1
                );
                return EXIT_USAGE;
            }
            vec![p]
        }
        None => (0..page_count).collect(),
    };

    let file_stem = Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");

    let assets_dir_name = format!("{}_assets", file_stem);
    let assets_dir_path = output_path.join(&assets_dir_name);
    let mut written_image_count: usize = 0;
    // [#2707] 요청한 페이지 수가 아니라 실제로 저장에 성공한 MD 페이지 수를 센다.
    // 이미지 실패는 경고로 남기고 MD 자체는 저장되므로 페이지 실패로 세지 않는다.
    let mut written_page_count = 0usize;
    // [#3596] --json 매니페스트용 페이지별 산출물 기록.
    let mut manifest: Vec<serde_json::Value> = Vec::new();

    let mime_to_ext = |mime: &str| -> &'static str {
        match mime {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/bmp" => "bmp",
            "image/webp" => "webp",
            _ => "bin",
        }
    };

    for page_num in &pages {
        match doc.extract_page_markdown_with_images_native(*page_num) {
            Ok((mut markdown, image_refs)) => {
                for (img_idx, (sec_idx, para_idx, control_idx, bin_data_id)) in
                    image_refs.iter().enumerate()
                {
                    let token = format!("[[RHWP_IMAGE:{}]]", img_idx + 1);

                    let try_control = match (sec_idx, para_idx, control_idx) {
                        (Some(si), Some(pi), Some(ci)) => Some((*si, *pi, *ci)),
                        _ => None,
                    };

                    let (mime, image_data) = if let Some((si, pi, ci)) = try_control {
                        match (
                            doc.get_control_image_mime_native(si, pi, &[], ci),
                            doc.get_control_image_data_native(si, pi, &[], ci),
                        ) {
                            (Ok(m), Ok(d)) => (m, d),
                            _ => {
                                if *bin_data_id == 0 {
                                    eprintln!(
                                        "경고: 페이지 {} 이미지 추출 실패 (s{} p{} c{}), fallback bin_data_id 없음",
                                        page_num, si, pi, ci
                                    );
                                    markdown = markdown.replace(&token, "");
                                    continue;
                                }
                                let fb_mime = match doc.get_bin_data_image_mime_native(*bin_data_id)
                                {
                                    Ok(m) => m,
                                    Err(e) => {
                                        eprintln!(
                                            "경고: 페이지 {} 이미지 MIME fallback 실패 (bin={}): {:?}",
                                            page_num, bin_data_id, e
                                        );
                                        markdown = markdown.replace(&token, "");
                                        continue;
                                    }
                                };
                                let fb_data = match doc.get_bin_data_image_data_native(*bin_data_id)
                                {
                                    Ok(d) => d,
                                    Err(e) => {
                                        eprintln!(
                                            "경고: 페이지 {} 이미지 데이터 fallback 실패 (bin={}): {:?}",
                                            page_num, bin_data_id, e
                                        );
                                        markdown = markdown.replace(&token, "");
                                        continue;
                                    }
                                };
                                (fb_mime, fb_data)
                            }
                        }
                    } else {
                        if *bin_data_id == 0 {
                            eprintln!(
                                "경고: 페이지 {} 이미지 추출 실패 (문서 좌표 없음, bin_data_id=0)",
                                page_num
                            );
                            markdown = markdown.replace(&token, "");
                            continue;
                        }
                        let fb_mime = match doc.get_bin_data_image_mime_native(*bin_data_id) {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!(
                                    "경고: 페이지 {} 이미지 MIME fallback 실패 (bin={}): {:?}",
                                    page_num, bin_data_id, e
                                );
                                markdown = markdown.replace(&token, "");
                                continue;
                            }
                        };
                        let fb_data = match doc.get_bin_data_image_data_native(*bin_data_id) {
                            Ok(d) => d,
                            Err(e) => {
                                eprintln!(
                                    "경고: 페이지 {} 이미지 데이터 fallback 실패 (bin={}): {:?}",
                                    page_num, bin_data_id, e
                                );
                                markdown = markdown.replace(&token, "");
                                continue;
                            }
                        };
                        (fb_mime, fb_data)
                    };

                    if !assets_dir_path.exists() {
                        if let Err(e) = fs::create_dir_all(&assets_dir_path) {
                            eprintln!(
                                "오류: 이미지 출력 폴더 생성 실패 - {}: {}",
                                assets_dir_path.display(),
                                e
                            );
                            markdown = markdown.replace(&token, "");
                            continue;
                        }
                    }

                    let ext = mime_to_ext(&mime);
                    let image_filename = format!(
                        "{}_p{:03}_img{:03}.{}",
                        file_stem,
                        page_num + 1,
                        img_idx + 1,
                        ext
                    );
                    let image_path = assets_dir_path.join(&image_filename);

                    if let Err(e) = fs::write(&image_path, &image_data) {
                        eprintln!("경고: 이미지 저장 실패 - {}: {}", image_path.display(), e);
                        markdown = markdown.replace(&token, "");
                        continue;
                    }

                    let image_link = format!(
                        "![image {}]({}/{})",
                        img_idx + 1,
                        assets_dir_name,
                        image_filename
                    );
                    markdown = markdown.replace(&token, &image_link);
                    written_image_count += 1;
                }

                if !markdown.ends_with('\n') {
                    markdown.push('\n');
                }

                let md_filename = if page_count == 1 {
                    format!("{}.md", file_stem)
                } else {
                    format!("{}_{:03}.md", file_stem, page_num + 1)
                };
                let md_path = output_path.join(&md_filename);

                match fs::write(&md_path, markdown.as_bytes()) {
                    Ok(_) => {
                        if json_mode {
                            manifest.push(serde_json::json!({
                                "page": page_num,
                                "path": md_path.display().to_string(),
                                "bytes": markdown.len(),
                            }));
                        } else {
                            println!("  → {}", md_path.display());
                        }
                        written_page_count += 1;
                    }
                    Err(e) => eprintln!("오류: Markdown 저장 실패 - {}: {}", md_path.display(), e),
                }
            }
            Err(e) => {
                eprintln!("오류: 페이지 {} Markdown 생성 실패 - {:?}", page_num, e);
            }
        }
    }

    // [#2707] 한 장이라도 못 썼으면 런타임 실패다. [#3596] JSON 모드의 실패는
    // stdout 을 비워 부분 매니페스트를 성공으로 오인하지 않게 한다(export-svg 규약).
    if written_page_count != pages.len() {
        if !json_mode {
            println!(
                "Markdown 내보내기 완료: {}개 MD 파일 → {}/",
                written_page_count, output_dir
            );
        }
        return EXIT_RUNTIME;
    }

    if json_mode {
        println!(
            "{}",
            provenance::marked(
                serde_json::json!({
                    "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                    "source": file_path,
                    "format": "markdown",
                    "outputDir": output_dir,
                    "pageCount": page_count,
                    "renderedCount": written_page_count,
                    "imageCount": written_image_count,
                    "pages": manifest,
                }),
                "export-markdown",
            )
        );
    } else if written_image_count > 0 {
        println!(
            "Markdown 내보내기 완료: {}개 MD 파일, {}개 이미지 → {}/",
            written_page_count, written_image_count, output_dir
        );
    } else {
        println!(
            "Markdown 내보내기 완료: {}개 MD 파일 → {}/",
            written_page_count, output_dir
        );
    }

    EXIT_OK
}



/// [#5932] HWPX `version.xml`(원본 바이트 보존, IR 미모델링)에서 마지막 저장
/// 한컴오피스 애플리케이션 이름·버전을 읽는다. HWPX가 아니거나 엔트리가 없거나
/// 파싱에 실패하면 `None` — `info` 출력/JSON 계약 모두 이 함수 하나를 공유한다.
pub(crate) fn hwpx_last_saved_application(
    document: &rhwp::model::document::Document,
) -> Option<(String, String)> {
    let version_xml = document.hwpx_aux_entry("version.xml")?;
    let version_xml = std::str::from_utf8(version_xml).ok()?;
    rhwp::parser::hwpx::header::parse_hwpx_application_version(version_xml)
}

pub(crate) fn show_info(args: &[String]) -> i32 {
    // [#3237] --json은 위치와 무관하다. 단일 입력 명령이므로 추가 경로를 무시하지 않는다.
    let mut json_mode = false;
    let mut file_path: Option<&str> = None;
    for arg in args {
        match arg.as_str() {
            "--json" => json_mode = true,
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
    }
    let Some(file_path) = file_path else {
        eprintln!("오류: 문서 파일 경로를 지정해주세요.");
        return EXIT_USAGE;
    };

    // 파일 읽기
    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let file_size = data.len();
    let detected_format = rhwp::parser::detect_format(&data);

    // 문서 파싱
    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let document = doc.document();

    // [#3237] JSON 모드: 핵심 메타를 stdout JSON 하나로 낸다. `schemaVersion` 이 계약이며
    // 필드 추가는 허용, 기존 필드 변경·삭제는 `tests/cli_json_contract.rs` 가 잡는다.
    if json_mode {
        let info = info_json_value(file_path, file_size, detected_format, &doc);
        println!("{info}");
        return EXIT_OK;
    }

    if detected_format == rhwp::parser::FileFormat::Hml {
        println!("format: HML");
        println!(
            "hwpml_version: {}",
            document
                .doc_info
                .hwpml_version
                .as_deref()
                .unwrap_or("unknown")
        );
        println!("sections: {}", document.sections.len());
        println!("pages: {}", doc.page_count());
        if let Some(metadata) = doc.hml_metadata() {
            let encoding = match metadata.encoding {
                rhwp::parser::hml::HmlEncoding::Utf8 => "UTF-8",
                rhwp::parser::hml::HmlEncoding::Utf16Le => "UTF-16LE",
                rhwp::parser::hml::HmlEncoding::Utf16Be => "UTF-16BE",
            };
            println!("encoding: {encoding}");
            println!("resources: {}", metadata.resource_count);
            println!("warnings: {}", metadata.warnings.len());
            for warning in &metadata.warnings {
                eprintln!(
                    "warning [{:?}] {}: {}",
                    warning.code, warning.xml_path, warning.message
                );
            }
        }
    }

    println!("파일: {}", file_path);
    println!("크기: {} bytes", file_size);
    if detected_format != rhwp::parser::FileFormat::Hml {
        println!(
            "버전: {}.{}.{}.{}",
            document.header.version.major,
            document.header.version.minor,
            document.header.version.build,
            document.header.version.revision,
        );
        println!(
            "압축: {}",
            if document.header.compressed {
                "예"
            } else {
                "아니오"
            }
        );
        println!(
            "암호화: {}",
            if document.header.encrypted {
                "예"
            } else {
                "아니오"
            }
        );
        println!(
            "배포용: {}",
            if document.header.distribution {
                "예"
            } else {
                "아니오"
            }
        );
    }
    // [#5932] HWPX version.xml에 보존된 마지막 저장 한컴오피스 애플리케이션/버전 표시.
    // version.xml은 IR로 모델링하지 않는 보조 엔트리라 여기서만 조회한다.
    if detected_format == rhwp::parser::FileFormat::Hwpx {
        if let Some((application, app_version)) = hwpx_last_saved_application(document) {
            println!("마지막 저장 애플리케이션: {application} ({app_version})");
        }
    }
    println!("구역 수: {}", document.sections.len());
    println!("페이지 수: {}", doc.page_count());

    // 용지 정보
    for (sec_idx, section) in document.sections.iter().enumerate() {
        let page_def = &section.section_def.page_def;
        let orientation = if page_def.landscape {
            "가로"
        } else {
            "세로"
        };
        println!(
            "구역{} 용지: {}×{} HWPUNIT, 방향={} (여백: 좌{} 우{} 상{} 하{})",
            sec_idx,
            page_def.width,
            page_def.height,
            orientation,
            page_def.margin_left,
            page_def.margin_right,
            page_def.margin_top,
            page_def.margin_bottom,
        );
        println!(
            "  머리말여백={} 꼬리말여백={} 제본여백={}",
            page_def.margin_header, page_def.margin_footer, page_def.margin_gutter
        );
        if section.section_def.hide_empty_line {
            println!("  빈 줄 감추기: 활성");
        }
    }

    // 폰트 목록
    let lang_names = ["한글", "영어", "한자", "일어", "기타", "기호", "사용자"];
    for (i, fonts) in document.doc_info.font_faces.iter().enumerate() {
        if !fonts.is_empty() {
            let name = if i < lang_names.len() {
                lang_names[i]
            } else {
                "기타"
            };
            let font_names: Vec<String> = fonts
                .iter()
                .enumerate()
                .map(|(idx, f)| format!("[{}]{}", idx, f.name))
                .collect();
            println!("폰트({}): {}", name, font_names.join(", "));
        }
    }

    // 스타일 목록
    if !document.doc_info.styles.is_empty() {
        let style_names: Vec<&str> = document
            .doc_info
            .styles
            .iter()
            .map(|s| s.local_name.as_str())
            .collect();
        println!("스타일: {}", style_names.join(", "));
    }

    // 문단 통계
    let total_paras: usize = document.sections.iter().map(|s| s.paragraphs.len()).sum();
    println!("총 문단 수: {}", total_paras);

    // [Task #554] HWP3 → HWP5 변환본 식별 휴리스틱 정보
    // 한컴이 HWP3 → HWP5 변환 시 ParaShape/CharShape 를 거의 재사용하지 않고 매우 적은
    // 수만 생성한다. 직접 작성본은 작성자가 다양한 스타일을 사용하므로 비율이 paragraph
    // 와 비슷하거나 더 높다. 임계값 < 0.05 / < 0.15 로 27 fixture 100% 분류 (Stage 1).
    let ps_count = document.doc_info.para_shapes.len();
    let cs_count = document.doc_info.char_shapes.len();
    if total_paras > 0 {
        let ps_ratio = ps_count as f64 / total_paras as f64;
        let cs_ratio = cs_count as f64 / total_paras as f64;
        let origin = if total_paras > 50 && ps_ratio < 0.05 && cs_ratio < 0.15 {
            "HWP3 변환본 추정 (margin_bottom -1600 HU 보정 적용)"
        } else if total_paras <= 50 {
            "판정 불가 (문단 수 ≤ 50, 비율 왜곡 회피)"
        } else {
            "한컴 한글 직접 작성 추정"
        };
        println!("ParaShape: {} (PS/문단 = {:.3})", ps_count, ps_ratio);
        println!("CharShape: {} (CS/문단 = {:.3})", cs_count, cs_ratio);
        println!("Origin 추정: {}", origin);
    }

    // BinData 정보
    if !document.doc_info.bin_data_list.is_empty() {
        println!("BinData:");
        for (idx, bd) in document.doc_info.bin_data_list.iter().enumerate() {
            let type_str = match bd.data_type {
                rhwp::model::bin_data::BinDataType::Link => "Link",
                rhwp::model::bin_data::BinDataType::Embedding => "Embedding",
                rhwp::model::bin_data::BinDataType::Storage => "Storage",
            };
            let ext = bd.extension.as_deref().unwrap_or("?");
            // 로드된 데이터 크기 확인
            let loaded_size = document
                .bin_data_content
                .iter()
                .find(|c| c.id == bd.storage_id)
                .map(|c| c.data.len())
                .unwrap_or(0);
            println!(
                "  [{}] {} (ID: {}, ext: {}, loaded: {} bytes)",
                idx, type_str, bd.storage_id, ext, loaded_size
            );
        }
    }

    // 테이블 및 그림 정보
    use rhwp::model::control::Control;
    let mut table_idx = 0;
    let mut picture_idx = 0;

    fn count_pictures(ctrl: &Control, picture_idx: &mut usize, location: &str) {
        match ctrl {
            Control::Picture(pic) => {
                *picture_idx += 1;
                println!(
                    "그림{} [{}]: bin_data_id={}, size={}×{}",
                    *picture_idx,
                    location,
                    pic.image_attr.bin_data_id,
                    pic.common.width,
                    pic.common.height,
                );
            }
            Control::Table(table) => {
                // 표 내부 셀의 문단에서도 그림 검색
                for (cell_idx, cell) in table.cells.iter().enumerate() {
                    for (cp_idx, cp) in cell.paragraphs.iter().enumerate() {
                        for cc in &cp.controls {
                            let loc = format!("{}→셀{}:문단{}", location, cell_idx, cp_idx);
                            count_pictures(cc, picture_idx, &loc);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for (sec_idx, section) in document.sections.iter().enumerate() {
        for (para_idx, para) in section.paragraphs.iter().enumerate() {
            for ctrl in &para.controls {
                let location = format!("구역{}:문단{}", sec_idx, para_idx);
                match ctrl {
                    Control::Table(table) => {
                        table_idx += 1;
                        let page_break_str = match table.page_break {
                            rhwp::model::table::TablePageBreak::None => "나누지 않음",
                            rhwp::model::table::TablePageBreak::CellBreak => "셀 단위 나눔",
                            rhwp::model::table::TablePageBreak::RowBreak => "나눔(행 단위)",
                        };
                        println!(
                            "표{} [{}]: {}행×{}열, 셀 {}개, 쪽나눔={} (attr=0x{:08x}), 제목반복={}",
                            table_idx,
                            location,
                            table.row_count,
                            table.col_count,
                            table.cells.len(),
                            page_break_str,
                            table.raw_table_record_attr,
                            table.repeat_header,
                        );
                        count_pictures(ctrl, &mut picture_idx, &location);
                    }
                    Control::Picture(_) => {
                        count_pictures(ctrl, &mut picture_idx, &location);
                    }
                    Control::Shape(shape) => {
                        use rhwp::model::shape::ShapeObject;
                        let s = shape.as_ref();
                        let shape_type = s.shape_name();
                        let common = s.common();
                        let border_info = match shape.as_ref() {
                            ShapeObject::Rectangle(r) => format!(
                                ", border(color={:#010x}, width={}, attr={:#010x})",
                                r.drawing.border_line.color,
                                r.drawing.border_line.width,
                                r.drawing.border_line.attr,
                            ),
                            ShapeObject::Line(l) => format!(
                                ", border(color={:#010x}, width={}, attr={:#010x})",
                                l.drawing.border_line.color,
                                l.drawing.border_line.width,
                                l.drawing.border_line.attr,
                            ),
                            _ => String::new(),
                        };
                        println!(
                            "도형 [{}]: {}, size={}×{}, treat_as_char={}{}",
                            location,
                            shape_type,
                            common.width,
                            common.height,
                            common.treat_as_char,
                            border_info,
                        );
                        // 그룹 자식 상세 정보
                        if let ShapeObject::Group(g) = shape.as_ref() {
                            for (i, child) in g.children.iter().enumerate() {
                                let ctype = child.shape_name();
                                let cattr = child.shape_attr();
                                let eff_w = (cattr.current_width as f64 * cattr.render_sx) as i32;
                                let eff_h = (cattr.current_height as f64 * cattr.render_sy) as i32;
                                println!("  자식[{}]: {}, orig={}×{}, scale=({:.3},{:.3}), eff={}×{} at ({:.0},{:.0})",
                                    i, ctype,
                                    cattr.current_width, cattr.current_height,
                                    cattr.render_sx, cattr.render_sy,
                                    eff_w, eff_h,
                                    cattr.render_tx, cattr.render_ty);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    EXIT_OK
}



/// HWPUNIT(u32)을 mm로 변환
pub(crate) fn hu_to_mm(hu: u32) -> f64 {
    hu as f64 * 25.4 / 7200.0
}



/// HWPUNIT(i32)을 mm로 변환
pub(crate) fn hu_to_mm_i(hu: i32) -> f64 {
    hu as f64 * 25.4 / 7200.0
}


pub(crate) fn dump_note_shape(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("사용법: rhwp dump-note-shape <파일.hwp|파일.hwpx>");
        return EXIT_USAGE;
    }

    let file_path = &args[0];
    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let sections: Vec<serde_json::Value> = doc
        .document()
        .sections
        .iter()
        .enumerate()
        .map(|(idx, section)| {
            serde_json::json!({
                "section": idx,
                "footnoteShape": note_shape_json(&section.section_def.footnote_shape),
                "endnoteShape": note_shape_json(&section.section_def.endnote_shape),
            })
        })
        .collect();

    let value = serde_json::json!({
        "file": file_path,
        "sections": sections,
    });
    match serde_json::to_string_pretty(&value) {
        Ok(text) => {
            println!("{}", text);
            EXIT_OK
        }
        Err(e) => {
            eprintln!("오류: JSON 생성 실패 - {}", e);
            EXIT_RUNTIME
        }
    }
}


pub(crate) fn note_shape_json(shape: &rhwp::model::footnote::FootnoteShape) -> serde_json::Value {
    serde_json::json!({
        "raw": {
            "attr": shape.attr,
            "numberFormat": format!("{:?}", shape.number_format),
            "userChar": shape.user_char.to_string(),
            "prefixChar": shape.prefix_char.to_string(),
            "suffixChar": shape.suffix_char.to_string(),
            "startNumber": shape.start_number,
            "separatorLength": hu_json(shape.separator_length as i32),
            "separatorMarginTop": hu_json(shape.separator_margin_top as i32),
            "separatorMarginBottom": hu_json(shape.separator_margin_bottom as i32),
            "noteSpacing": hu_json(shape.note_spacing as i32),
            "separatorLineType": shape.separator_line_type,
            "separatorLineWidth": shape.separator_line_width,
            "separatorColor": format!("0x{:08x}", shape.separator_color),
            "numbering": format!("{:?}", shape.numbering),
            "placement": format!("{:?}", shape.placement),
            "numberCodeSuperscript": shape.number_code_superscript,
            "printInlineAfterText": shape.print_inline_after_text,
            "rawUnknown": hu_json(shape.raw_unknown as i32),
        },
        "ui": {
            "separatorAbove": hu_json(shape.separator_above_margin_hu() as i32),
            "separatorBelow": hu_json(shape.separator_below_margin_hu() as i32),
            "betweenNotes": hu_json(shape.between_notes_margin_hu() as i32),
        },
    })
}


pub(crate) fn hu_json(hu: i32) -> serde_json::Value {
    serde_json::json!({
        "hu": hu,
        "mm": rounded_mm(hu),
    })
}


pub(crate) fn rounded_mm(hu: i32) -> f64 {
    (hu_to_mm_i(hu) * 1000.0).round() / 1000.0
}



/// 레이아웃 트리의 항목별 **실제 extent** 를 덤프한다.
///
/// `dump-pages` 는 쪽 나눔이 **의도한** 항목 목록과 저장 좌표를 보여준다. 그런데 쪽 밖
/// 배치를 조사할 때 필요한 것은 레이아웃이 **실제로 차지한** 영역이다. 둘이 어긋나는
/// 것이 결함의 실체이기 때문이다 (#3637).
///
/// 종전에는 SVG 의 `<text>`·`<rect>` y 좌표로 이를 역산했는데, **테두리 없는 표는
/// `<rect>` 를 만들지 않아** 그 자리를 "빈 공간" 으로 오판했다. 이 명령은 렌더 트리를
/// 직접 걸어 그 한계를 없앤다.
///
/// 사용법:
/// ```text
/// rhwp dump-extents <파일> [-p <쪽번호>] [--min-h <px>] [--outside] [--gaps]
/// ```
///
/// - `--outside` : 쪽 경계를 넘는 노드만 출력
/// - `--gaps`    : 콘텐츠 사이 세로 빈 구간만 출력 (무엇이 자리를 먹는지)
/// - `--min-h`   : 이 높이 미만 노드 생략 (기본 0)
pub(crate) fn dump_extents(args: &[String]) -> i32 {
    use rhwp::renderer::render_tree::{RenderNode, RenderNodeType};

    if args.is_empty() {
        eprintln!(
            "사용법: rhwp dump-extents <파일.hwp> [-p <쪽번호>] [--min-h <px>] [--outside] [--gaps]"
        );
        return EXIT_USAGE;
    }

    let file_path = &args[0];
    let mut target_page: Option<u32> = None;
    let mut min_h = 0.0f64;
    let mut only_outside = false;
    let mut show_gaps = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--page" | "-p" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("오류: {} 뒤에 쪽 번호가 필요합니다.", args[i]);
                    return EXIT_USAGE;
                };
                match v.parse::<u32>() {
                    Ok(n) => target_page = Some(n),
                    Err(_) => {
                        eprintln!("오류: 쪽 번호가 올바르지 않습니다.");
                        return EXIT_USAGE;
                    }
                }
                i += 2;
            }
            "--min-h" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("오류: --min-h 뒤에 값이 필요합니다.");
                    return EXIT_USAGE;
                };
                match v.parse::<f64>() {
                    Ok(n) => min_h = n,
                    Err(_) => {
                        eprintln!("오류: --min-h 값이 올바르지 않습니다.");
                        return EXIT_USAGE;
                    }
                }
                i += 2;
            }
            "--outside" => {
                only_outside = true;
                i += 1;
            }
            "--gaps" => {
                show_gaps = true;
                i += 1;
            }
            _ => {
                eprintln!("알 수 없는 옵션: {}", args[i]);
                return EXIT_USAGE;
            }
        }
    }

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let page_count = doc.page_count();
    println!("문서 로드: {} ({}쪽)", file_path, page_count);

    // 노드 종류를 짧은 이름과 (문단/컨트롤) 요약으로 바꾼다.
    fn describe(n: &RenderNode) -> (&'static str, String) {
        match &n.node_type {
            RenderNodeType::Page(_) => ("Page", String::new()),
            RenderNodeType::PageBackground(_) => ("PageBg", String::new()),
            RenderNodeType::MasterPage => ("MasterPage", String::new()),
            RenderNodeType::Header => ("Header", String::new()),
            RenderNodeType::Footer => ("Footer", String::new()),
            RenderNodeType::Body { .. } => ("Body", String::new()),
            RenderNodeType::Column(c) => ("Column", format!("col={c}")),
            RenderNodeType::FootnoteArea => ("FootnoteArea", String::new()),
            RenderNodeType::TextLine(t) => (
                "TextLine",
                format!(
                    "pi={} line={} vpos={}",
                    t.para_index.map(|v| v as i64).unwrap_or(-1),
                    t.line_index.map(|v| v as i64).unwrap_or(-1),
                    t.vpos.unwrap_or(-1)
                ),
            ),
            RenderNodeType::TextRun(t) => (
                "TextRun",
                format!(
                    "pi={} {:?}",
                    t.para_index.map(|v| v as i64).unwrap_or(-1),
                    t.text.chars().take(14).collect::<String>()
                ),
            ),
            RenderNodeType::Table(t) => (
                "Table",
                format!(
                    "pi={} ci={} {}x{}",
                    t.para_index.map(|v| v as i64).unwrap_or(-1),
                    t.control_index.map(|v| v as i64).unwrap_or(-1),
                    t.row_count,
                    t.col_count
                ),
            ),
            RenderNodeType::TableCell(c) => ("TableCell", format!("r={} c={}", c.row, c.col)),
            _ => ("기타", String::new()),
        }
    }

    // 깊이 우선으로 걸으며 visit 를 호출한다.
    fn walk(n: &RenderNode, depth: usize, visit: &mut impl FnMut(&RenderNode, usize)) {
        visit(n, depth);
        for c in &n.children {
            walk(c, depth + 1, visit);
        }
    }

    // -p 는 다른 dump 명령과 같이 0-based 쪽 인덱스다. 범위를 벗어나면 렌더 트리 생성
    // 실패 메시지 대신 사용법 오류로 끊는다.
    let pages: Vec<u32> = match target_page {
        Some(p) => {
            if p >= page_count {
                eprintln!(
                    "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                    page_count.saturating_sub(1)
                );
                return EXIT_USAGE;
            }
            vec![p]
        }
        None => (0..page_count).collect(),
    };

    for p in pages {
        let tree = match doc.build_page_render_tree(p) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("오류: {}쪽 렌더 트리 생성 실패 - {:?}", p + 1, e);
                return EXIT_RUNTIME;
            }
        };
        let page_h = tree.root.bbox.height;
        let page_w = tree.root.bbox.width;
        println!("\n=== {}쪽 (트리 {:.1}x{:.1}px) ===", p + 1, page_w, page_h);

        let mut outside: Vec<(f64, f64, &'static str, String)> = Vec::new();
        let mut spans: Vec<(f64, f64, &'static str, String)> = Vec::new();

        walk(&tree.root, 0, &mut |n, depth| {
            let b = &n.bbox;
            if b.height < min_h {
                return;
            }
            let (kind, idx) = describe(n);
            let bottom = b.y + b.height;
            let is_outside = bottom > page_h + 0.5;
            if is_outside {
                outside.push((b.y, bottom, kind, idx.clone()));
            }
            // 빈 구간 계산에는 **잎 콘텐츠**만 쓴다.
            //
            // 컨테이너는 자기 안의 공백을 통째로 가린다. Body·Column 뿐 아니라 **표도**
            // 그렇다 — 본문 전체를 담은 1×1 표는 쪽 전체를 덮어 내부 201px 공백을
            // "구간 없음" 으로 만들었다(#3637 조사에서 실제로 겪은 오판이다).
            //
            // 그래서 TextLine 과, **자손에 TextLine 이 없는** 표(= 빈 표)만 센다.
            let has_text_descendant = {
                fn any_text(n: &RenderNode) -> bool {
                    if matches!(n.node_type, RenderNodeType::TextLine(_)) {
                        return true;
                    }
                    n.children.iter().any(any_text)
                }
                n.children.iter().any(any_text)
            };
            if matches!(n.node_type, RenderNodeType::TextLine(_))
                || (matches!(n.node_type, RenderNodeType::Table(_)) && !has_text_descendant)
            {
                spans.push((b.y, bottom, kind, idx.clone()));
            }
            if show_gaps || (only_outside && !is_outside) {
                return;
            }
            println!(
                "{:indent$}{kind:12} y={:8.1}..{:8.1} h={:7.1} x={:7.1} w={:7.1}  {idx}",
                "",
                b.y,
                bottom,
                b.height,
                b.x,
                b.width,
                indent = depth * 2,
            );
        });

        if show_gaps {
            spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            println!("  -- 콘텐츠 사이 세로 빈 구간 (30px 이상) --");
            let mut cursor = 0.0f64;
            let mut cursor_src = String::from("(쪽 시작)");
            for (y, bottom, kind, idx) in &spans {
                if *y - cursor > 30.0 {
                    println!(
                        "     빈 구간 {:8.1}..{:8.1} ({:6.1}px)  직전={cursor_src} → 다음={kind} {idx}",
                        cursor,
                        y,
                        y - cursor,
                    );
                }
                if *bottom > cursor {
                    cursor = *bottom;
                    cursor_src = format!("{kind} {idx}");
                }
            }
        }

        if outside.is_empty() {
            println!("  쪽 경계를 넘는 노드 없음");
        } else {
            let worst = outside
                .iter()
                .map(|(_, b, _, _)| *b - page_h)
                .fold(0.0f64, f64::max);
            println!(
                "  ** 쪽 경계를 넘는 노드 {}개 · 최대 초과 {:.1}px **",
                outside.len(),
                worst
            );
            for (y, bottom, kind, idx) in outside.iter().take(8) {
                println!(
                    "     {kind:12} y={y:8.1}..{bottom:8.1} 초과 {:7.1}px  {idx}",
                    bottom - page_h
                );
            }
        }
    }
    EXIT_OK
}


pub(crate) fn dump_pages(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!(
            "사용법: rhwp dump-pages <파일.hwp> [-p <페이지번호>] [--respect-vpos-reset] [--json]"
        );
        return EXIT_USAGE;
    }

    let file_path = &args[0];
    let mut target_page: Option<u32> = None;
    let mut respect_vpos_reset = false;
    let mut json_mode = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--page" | "-p" => {
                if i + 1 < args.len() {
                    // 형제 명령(export_svg/export_png/export_text)과 동일하게 파싱 실패를
                    // 오류로 처리한다. 종전 `.parse().ok()` 는 잘못된 인자를 조용히 삼켜
                    // 한 쪽만 요청했는데 문서 전체를 덤프했다.
                    match args[i + 1].parse::<u32>() {
                        Ok(n) => target_page = Some(n),
                        Err(_) => {
                            eprintln!("오류: 페이지 번호가 올바르지 않습니다: {}", args[i + 1]);
                            return EXIT_USAGE;
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("오류: {} 뒤에 페이지 번호가 필요합니다.", args[i]);
                    return EXIT_USAGE;
                }
            }
            "--respect-vpos-reset" => {
                respect_vpos_reset = true;
                i += 1;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => {
                eprintln!("알 수 없는 옵션: {}", args[i]);
                return EXIT_USAGE;
            }
        }
    }

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let mut doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    if respect_vpos_reset {
        doc.set_respect_vpos_reset(true);
    }

    let page_count = doc.page_count();

    // 형제 명령(export_svg)과 동일한 범위 검사. 종전엔 검사가 없어 -p 999 가
    // 아무것도 매칭하지 않은 빈 출력을 내, 잘못된 인자가 아니라 "쪽이 없는 문서"
    // 처럼 보였다.
    if let Some(p) = target_page {
        if p >= page_count {
            eprintln!(
                "오류: 페이지 번호가 범위를 벗어났습니다 (0~{})",
                page_count.saturating_sub(1)
            );
            return EXIT_USAGE;
        }
    }

    if json_mode {
        // [#3697] 페이지네이션 진단 기계 계약 (#3608 1-C). stdout 은 순수 JSON 단건 봉투 —
        // 진행/요약 출력은 내지 않는다 (jsonContract 규약).
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "pageCount": page_count,
            "pageFilter": target_page,
            "respectVposReset": respect_vpos_reset,
            "pages": doc.dump_page_items_json(target_page),
        });
        println!("{}", provenance::marked(envelope, "dump-pages"));
    } else {
        println!("문서 로드: {} ({}페이지)", file_path, page_count);
        print!("{}", doc.dump_page_items(target_page));
    }
    EXIT_OK
}


pub(crate) fn dump_endnote_lines(args: &[String]) -> i32 {
    if args.len() < 4 {
        eprintln!(
            "사용법: rhwp dump-endnote-lines <파일.hwp> <section> <para> <control> [note-para]"
        );
        return EXIT_USAGE;
    }

    let file_path = &args[0];
    let section_idx = match args[1].parse::<usize>() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: section 인덱스 파싱 실패 - {}", e);
            return EXIT_USAGE;
        }
    };
    let para_idx = match args[2].parse::<usize>() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: para 인덱스 파싱 실패 - {}", e);
            return EXIT_USAGE;
        }
    };
    let control_idx = match args[3].parse::<usize>() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: control 인덱스 파싱 실패 - {}", e);
            return EXIT_USAGE;
        }
    };
    let target_note_para = if args.len() >= 5 {
        match args[4].parse::<usize>() {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("오류: note-para 인덱스 파싱 실패 - {}", e);
                return EXIT_USAGE;
            }
        }
    } else {
        None
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let document = doc.document();
    let Some(section) = document.sections.get(section_idx) else {
        eprintln!("오류: section {} 범위 초과", section_idx);
        return EXIT_USAGE;
    };
    let Some(source_para) = section.paragraphs.get(para_idx) else {
        eprintln!("오류: para {} 범위 초과", para_idx);
        return EXIT_USAGE;
    };
    let Some(ctrl) = source_para.controls.get(control_idx) else {
        eprintln!("오류: control {} 범위 초과", control_idx);
        return EXIT_USAGE;
    };

    let rhwp::model::control::Control::Endnote(endnote) = ctrl else {
        eprintln!(
            "오류: s{}:p{}:ci{} 는 미주가 아닙니다 ({})",
            section_idx,
            para_idx,
            control_idx,
            control_kind(ctrl)
        );
        return EXIT_USAGE;
    };

    println!(
        "문서: {} source=s{}:p{}:ci{} endnote_no={} note_paras={}",
        file_path,
        section_idx,
        para_idx,
        control_idx,
        endnote.number,
        endnote.paragraphs.len()
    );
    println!("source_text={}", brief_text(&source_para.text, 120));
    println!(
        "source_control_positions={}",
        format_control_positions(source_para)
    );

    for (note_para_idx, para) in endnote.paragraphs.iter().enumerate() {
        if target_note_para.is_some_and(|target| target != note_para_idx) {
            continue;
        }
        println!(
            "\n-- note_para={} source=s{}:p{}:ci{}:note{} --",
            note_para_idx, section_idx, para_idx, control_idx, note_para_idx
        );
        dump_paragraph_line_trace(para);
    }
    EXIT_OK
}


pub(crate) fn dump_paragraph_line_trace(para: &rhwp::model::paragraph::Paragraph) {
    use rhwp::model::control::Control;

    let composed = rhwp::renderer::composer::compose_paragraph(para);
    let control_positions = para.control_text_positions();

    println!(
        "para text_len={} char_count={} controls={} line_segs={} char_offsets={} text={}",
        para.text.chars().count(),
        para.char_count,
        para.controls.len(),
        para.line_segs.len(),
        format_u32_list(&para.char_offsets),
        brief_text(&para.text, 160)
    );
    for (i, seg) in para.line_segs.iter().enumerate() {
        println!(
            "  line_seg[{i}] ts={} char={} vpos={} lh={} th={} bl={} gap={} cs={} sw={} tag=0x{:08x}",
            seg.text_start,
            para.utf16_pos_to_char_idx(seg.text_start),
            seg.vertical_pos,
            seg.line_height,
            seg.text_height,
            seg.baseline_distance,
            seg.line_spacing,
            seg.column_start,
            seg.segment_width,
            seg.tag
        );
    }

    if para.controls.is_empty() {
        println!("  controls=[]");
    } else {
        for (ci, ctrl) in para.controls.iter().enumerate() {
            let pos = control_positions.get(ci).copied().unwrap_or(usize::MAX);
            match ctrl {
                Control::Equation(eq) => println!(
                    "  control[{ci}] kind=Equation pos={} tac=true size={}x{} font={} baseline={} script={}",
                    pos,
                    eq.common.width,
                    eq.common.height,
                    eq.font_size,
                    eq.baseline,
                    brief_text(&eq.script, 100)
                ),
                Control::Picture(pic) => println!(
                    "  control[{ci}] kind=Picture pos={} tac={} size={}x{}",
                    pos, pic.common.treat_as_char, pic.common.width, pic.common.height
                ),
                Control::Shape(shape) => {
                    let common = shape.common();
                    println!(
                        "  control[{ci}] kind=Shape pos={} tac={} size={}x{}",
                        pos, common.treat_as_char, common.width, common.height
                    );
                }
                Control::Table(table) => println!(
                    "  control[{ci}] kind=Table pos={} tac={} rows={} cols={}",
                    pos,
                    table.common.treat_as_char,
                    table.row_count,
                    table.col_count
                ),
                other => println!(
                    "  control[{ci}] kind={} pos={} tac=false",
                    control_kind(other),
                    pos
                ),
            }
        }
    }

    println!("  composed_lines={}", composed.lines.len());
    for (li, line) in composed.lines.iter().enumerate() {
        let next_start = composed
            .lines
            .get(li + 1)
            .map(|next| next.char_start)
            .unwrap_or_else(|| {
                line.char_start
                    + line
                        .runs
                        .iter()
                        .map(|run| run.text.chars().count())
                        .sum::<usize>()
                    + usize::from(line.has_line_break)
            });
        println!(
            "    line[{li}] char={}..{} runs={} break={} lh={} bl={} gap={} cs={} sw={} layout_tacs={}",
            line.char_start,
            next_start,
            format_runs(&line.runs),
            line.has_line_break,
            line.line_height,
            line.baseline_distance,
            line.line_spacing,
            line.column_start,
            line.segment_width,
            format_layout_tac_hits(&composed, li)
        );
    }

    if composed.tac_controls.is_empty() {
        println!("  tac_controls=[]");
    } else {
        println!("  tac_controls:");
        for (pos, width_hu, ci) in &composed.tac_controls {
            let line_hits = composed
                .lines
                .iter()
                .enumerate()
                .filter_map(|(li, line)| {
                    let start = line.char_start;
                    let end = composed
                        .lines
                        .get(li + 1)
                        .map(|next| next.char_start)
                        .unwrap_or_else(|| {
                            line.char_start
                                + line
                                    .runs
                                    .iter()
                                    .map(|run| run.text.chars().count())
                                    .sum::<usize>()
                                + usize::from(line.has_line_break)
                        });
                    if if end > start {
                        *pos >= start && *pos < end
                    } else {
                        *pos == start
                    } {
                        Some(li.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "    tac ci={} pos={} width={} strict_line_candidates=[{}]",
                ci, pos, width_hu, line_hits
            );
        }
    }
}


pub(crate) fn format_layout_tac_hits(
    composed: &rhwp::renderer::composer::ComposedParagraph,
    line_idx: usize,
) -> String {
    let Some(line) = composed.lines.get(line_idx) else {
        return "[]".to_string();
    };
    if composed.tac_controls.is_empty() {
        return "[]".to_string();
    }

    let mut hits = Vec::new();
    if line.runs.is_empty() {
        let start = line.char_start;
        let end = composed
            .lines
            .get(line_idx + 1)
            .map(|next| next.char_start)
            .unwrap_or(usize::MAX);
        for (pos, _, ci) in &composed.tac_controls {
            if *pos >= start && *pos < end {
                hits.push(format!("ci{}@{}:empty", ci, pos));
            }
        }
    } else {
        let mut run_start = line.char_start;
        for (run_idx, run) in line.runs.iter().enumerate() {
            let run_len = run.text.chars().count();
            let run_end = run_start + run_len;
            let next_line_starts_at_run_end = composed
                .lines
                .get(line_idx + 1)
                .is_some_and(|next| next.char_start == run_end);
            let allow_end = run_idx == line.runs.len() - 1 && !next_line_starts_at_run_end;
            for (pos, _, ci) in &composed.tac_controls {
                if *pos >= run_start && (*pos < run_end || (allow_end && *pos == run_end)) {
                    hits.push(format!(
                        "ci{}@{}:run{}+{}",
                        ci,
                        pos,
                        run_idx,
                        pos.saturating_sub(run_start)
                    ));
                }
            }
            run_start = run_end;
        }
    }

    if hits.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", hits.join(","))
    }
}


pub(crate) fn control_kind(ctrl: &rhwp::model::control::Control) -> &'static str {
    use rhwp::model::control::Control;
    match ctrl {
        Control::SectionDef(_) => "SectionDef",
        Control::ColumnDef(_) => "ColumnDef",
        Control::Table(_) => "Table",
        Control::Shape(_) => "Shape",
        Control::Picture(_) => "Picture",
        Control::Header(_) => "Header",
        Control::Footer(_) => "Footer",
        Control::Footnote(_) => "Footnote",
        Control::Endnote(_) => "Endnote",
        Control::AutoNumber(_) => "AutoNumber",
        Control::NewNumber(_) => "NewNumber",
        Control::PageNumberPos(_) => "PageNumberPos",
        Control::Bookmark(_) => "Bookmark",
        Control::Hyperlink(_) => "Hyperlink",
        Control::Ruby(_) => "Ruby",
        Control::CharOverlap(_) => "CharOverlap",
        Control::PageHide(_) => "PageHide",
        Control::HiddenComment(_) => "HiddenComment",
        Control::Equation(_) => "Equation",
        Control::Field(_) => "Field",
        Control::Form(_) => "Form",
        Control::Unknown(_) => "Unknown",
    }
}


pub(crate) fn format_control_positions(para: &rhwp::model::paragraph::Paragraph) -> String {
    let positions = para.control_text_positions();
    if positions.is_empty() {
        return "[]".to_string();
    }
    positions
        .iter()
        .enumerate()
        .map(|(ci, pos)| {
            let kind = para.controls.get(ci).map(control_kind).unwrap_or("?");
            format!("{ci}:{kind}@{pos}")
        })
        .collect::<Vec<_>>()
        .join(",")
}


pub(crate) fn format_runs(runs: &[rhwp::renderer::composer::ComposedTextRun]) -> String {
    if runs.is_empty() {
        return "[]".to_string();
    }
    let parts = runs
        .iter()
        .map(|run| {
            format!(
                "cs{}:l{}:'{}'",
                run.char_style_id,
                run.lang_index,
                brief_text(&run.text, 40)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", parts.join("|"))
}


pub(crate) fn format_u32_list(values: &[u32]) -> String {
    if values.is_empty() {
        return "[]".to_string();
    }
    if values.len() <= 16 {
        return format!("{:?}", values);
    }
    let head = values
        .iter()
        .take(8)
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let tail = values
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}...{};len={}]", head, tail, values.len())
}


pub(crate) fn brief_text(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (count, ch) in text.chars().enumerate() {
        if count >= max_chars {
            out.push('…');
            break;
        }
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{FFFC}' => out.push('□'),
            c if c.is_control() => out.push_str(&format!("\\u{{{:04X}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}


pub(crate) fn dump_controls(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("오류: 문서 파일 경로를 지정해주세요.");
        eprintln!(
            "사용법: rhwp dump <파일.hwp|파일.hwpx|파일.hml> [--section <번호>] [--para <번호>]"
        );
        return EXIT_USAGE;
    }

    let file_path = &args[0];
    // [#3884 G2] 첫 인자 자리에 플래그가 오면 "파일을 읽을 수 없습니다 - --json" 같은
    // 오독 메시지로 새지 않게 사용법 오류로 끊는다.
    if file_path.starts_with('-') {
        eprintln!("오류: 알 수 없는 옵션입니다 - {file_path}");
        eprintln!(
            "사용법: rhwp dump <파일.hwp|파일.hwpx|파일.hml> [--section <번호>] [--para <번호>]"
        );
        return EXIT_USAGE;
    }
    let mut filter_section: Option<usize> = None;
    let mut filter_para: Option<usize> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--section" | "-s" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    eprintln!("오류: --section 뒤에 0 이상의 구역 번호가 필요합니다.");
                    return EXIT_USAGE;
                };
                match value.parse::<usize>() {
                    Ok(section) => filter_section = Some(section),
                    Err(_) => {
                        eprintln!(
                            "오류: --section 뒤에는 0 이상의 구역 번호가 필요합니다 - {value}"
                        );
                        return EXIT_USAGE;
                    }
                }
            }
            "--para" | "-p" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    eprintln!("오류: --para 뒤에 0 이상의 문단 번호가 필요합니다.");
                    return EXIT_USAGE;
                };
                match value.parse::<usize>() {
                    Ok(para) => filter_para = Some(para),
                    Err(_) => {
                        eprintln!("오류: --para 뒤에는 0 이상의 문단 번호가 필요합니다 - {value}");
                        return EXIT_USAGE;
                    }
                }
            }
            // [#3884 G2] 미지 플래그 침묵 무시 금지 — `--json` 을 붙이면 JSON 이 나올
            // 거라 믿는 소비자에게 사람용 텍스트를 exit 0 으로 돌려주던 구멍이다.
            other if other.starts_with('-') => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {other}");
                eprintln!(
                    "사용법: rhwp dump <파일.hwp|파일.hwpx|파일.hml> [--section <번호>] [--para <번호>]"
                );
                return EXIT_USAGE;
            }
            _ => {
                i += 1;
            }
        }
    }

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };

    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let document = doc.document();

    // border_fill 상세 덤프 (필터 없을 때 전체, 필터 있을 때 관련 bf만)
    if filter_section.is_none() && filter_para.is_none() {
        for (i, bf) in document.doc_info.border_fills.iter().enumerate() {
            let fill = &bf.fill;
            let solid_info = fill
                .solid
                .as_ref()
                .map(|s| {
                    format!(
                        "bg=#{:06X} pat_type={} pat_color=#{:06X}",
                        s.background_color, s.pattern_type, s.pattern_color
                    )
                })
                .unwrap_or_default();
            let grad_info = if fill.gradient.is_some() {
                " gradient"
            } else {
                ""
            };
            let img_info = fill
                .image
                .as_ref()
                .map(|img| {
                    format!(
                        " image(bin_id={}, mode={:?}, brightness={}, contrast={}, effect={})",
                        img.bin_data_id, img.fill_mode, img.brightness, img.contrast, img.effect
                    )
                })
                .unwrap_or_default();
            println!(
                "  border_fill[{}] fill_type={:?} {}{}{}",
                i, fill.fill_type, solid_info, grad_info, img_info
            );
        }
    }

    use rhwp::model::control::Control;
    use rhwp::model::paragraph::ColumnBreakType;
    use rhwp::model::shape::{HorzRelTo, ShapeObject, TextWrap, VertRelTo};

    let vert_str = |v: &VertRelTo| -> &str {
        match v {
            VertRelTo::Paper => "용지",
            VertRelTo::Page => "쪽",
            VertRelTo::Para => "문단",
        }
    };
    let horz_str = |h: &HorzRelTo| -> &str {
        match h {
            HorzRelTo::Paper => "용지",
            HorzRelTo::Page => "쪽",
            HorzRelTo::Column => "단",
            HorzRelTo::Para => "문단",
        }
    };
    let wrap_str = |w: &TextWrap| -> &str {
        match w {
            TextWrap::Square => "어울림",
            TextWrap::Tight => "빈 공간 채움",
            TextWrap::Through => "통과",
            TextWrap::TopAndBottom => "자리차지",
            TextWrap::BehindText => "글뒤로",
            TextWrap::InFrontOfText => "글앞으로",
        }
    };
    let break_str = |b: &ColumnBreakType| -> &str {
        match b {
            ColumnBreakType::None => "",
            ColumnBreakType::Section => "[구역나누기]",
            ColumnBreakType::MultiColumn => "[다단나누기]",
            ColumnBreakType::Page => "[쪽나누기]",
            ColumnBreakType::Column => "[단나누기]",
        }
    };

    // 도형 공통 속성 출력 헬퍼
    let dump_common = |c: &rhwp::model::shape::CommonObjAttr, indent: &str| {
        println!(
            "{}  크기: {:.1}mm × {:.1}mm ({}×{} HU)",
            indent,
            hu_to_mm(c.width),
            hu_to_mm(c.height),
            c.width,
            c.height
        );
        println!(
            "{}  위치: 가로={} 오프셋={:.1}mm({}) 정렬={:?}, 세로={} 오프셋={:.1}mm({}) 정렬={:?}",
            indent,
            horz_str(&c.horz_rel_to),
            hu_to_mm(c.horizontal_offset),
            c.horizontal_offset,
            c.horz_align,
            vert_str(&c.vert_rel_to),
            hu_to_mm(c.vertical_offset),
            c.vertical_offset,
            c.vert_align
        );
        println!(
            "{}  배치: {}, 글자처럼={}, z={}",
            indent,
            wrap_str(&c.text_wrap),
            c.treat_as_char,
            c.z_order
        );
        println!(
            "{}  바깥 여백: left={:.2}mm({}) right={:.2}mm({}) top={:.2}mm({}) bottom={:.2}mm({})",
            indent,
            hu_to_mm_i(c.margin.left as i32),
            c.margin.left,
            hu_to_mm_i(c.margin.right as i32),
            c.margin.right,
            hu_to_mm_i(c.margin.top as i32),
            c.margin.top,
            hu_to_mm_i(c.margin.bottom as i32),
            c.margin.bottom
        );
    };

    // 도형 요소 속성 출력 헬퍼
    let dump_shape_attr = |sa: &rhwp::model::shape::ShapeComponentAttr, indent: &str| {
        let eff_w = (sa.current_width as f64 * sa.render_sx) as u32;
        let eff_h = (sa.current_height as f64 * sa.render_sy) as u32;
        println!("{}  요소: orig={}×{}, curr={}×{}, M=[{:.3},{:.3},{:.0}; {:.3},{:.3},{:.0}], offset=({},{}), eff={:.1}mm×{:.1}mm",
            indent, sa.original_width, sa.original_height,
            sa.current_width, sa.current_height,
            sa.render_sx, sa.render_b, sa.render_tx,
            sa.render_c, sa.render_sy, sa.render_ty,
            sa.offset_x, sa.offset_y,
            hu_to_mm(eff_w), hu_to_mm(eff_h));
        if sa.horz_flip || sa.vert_flip || sa.rotation_angle != 0 {
            println!(
                "{}  변환: 뒤집기=({},{}), 회전={}",
                indent, sa.horz_flip, sa.vert_flip, sa.rotation_angle
            );
        }
    };

    // 재귀적 도형 덤프
    fn dump_shape(
        shape: &ShapeObject,
        indent: &str,
        dump_common_fn: &dyn Fn(&rhwp::model::shape::CommonObjAttr, &str),
        dump_sa_fn: &dyn Fn(&rhwp::model::shape::ShapeComponentAttr, &str),
    ) {
        match shape {
            ShapeObject::Line(s) => {
                println!(
                    "{}[직선] start=({},{}) end=({},{})",
                    indent, s.start.x, s.start.y, s.end.x, s.end.y
                );
                println!(
                    "{}  선: color={:#010x}, width={}, style={:#06x}",
                    indent,
                    s.drawing.border_line.color,
                    s.drawing.border_line.width,
                    s.drawing.border_line.attr
                );
                dump_common_fn(&s.common, indent);
                dump_sa_fn(&s.drawing.shape_attr, indent);
            }
            ShapeObject::Rectangle(s) => {
                println!("{}[사각형] round={}%", indent, s.round_rate);
                println!(
                    "{}  선: color={:#010x}, width={}, style={:#06x}",
                    indent,
                    s.drawing.border_line.color,
                    s.drawing.border_line.width,
                    s.drawing.border_line.attr
                );
                println!(
                    "{}  채우기: {:?}{}",
                    indent,
                    s.drawing.fill.fill_type,
                    if let Some(ref img) = s.drawing.fill.image {
                        format!(
                            ", image=bin_data_id={}, mode={:?}",
                            img.bin_data_id, img.fill_mode
                        )
                    } else {
                        String::new()
                    }
                );
                dump_common_fn(&s.common, indent);
                dump_sa_fn(&s.drawing.shape_attr, indent);
                if let Some(tb) = &s.drawing.text_box {
                    println!("{}  글상자: list_attr={:#010x}, margins=({},{},{},{}), max_width={}, paras={}",
                        indent, tb.list_attr, tb.margin_left, tb.margin_right, tb.margin_top, tb.margin_bottom,
                        tb.max_width, tb.paragraphs.len());
                    for (tpi, tp) in tb.paragraphs.iter().enumerate() {
                        let text_preview = if tp.text.is_empty() {
                            "(빈)".to_string()
                        } else if tp.text.chars().count() > 60 {
                            let end = tp
                                .text
                                .char_indices()
                                .nth(60)
                                .map(|(i, _)| i)
                                .unwrap_or(tp.text.len());
                            format!("\"{}...\"", &tp.text[..end])
                        } else {
                            format!("\"{}\"", tp.text)
                        };
                        println!(
                            "{}    p[{}]: ps_id={}, cc={}, text={}, ls_count={}, ctrls={}",
                            indent,
                            tpi,
                            tp.para_shape_id,
                            tp.char_count,
                            text_preview,
                            tp.line_segs.len(),
                            tp.controls.len()
                        );
                        for (li, ls) in tp.line_segs.iter().enumerate() {
                            println!(
                                "{}      ls[{}]: vpos={}, lh={}, th={}, bl={}, cs={}, sw={}",
                                indent,
                                li,
                                ls.vertical_pos,
                                ls.line_height,
                                ls.text_height,
                                ls.baseline_distance,
                                ls.column_start,
                                ls.segment_width
                            );
                        }
                    }
                }
            }
            ShapeObject::Ellipse(s) => {
                println!("{}[타원]", indent);
                dump_common_fn(&s.common, indent);
                dump_sa_fn(&s.drawing.shape_attr, indent);
            }
            ShapeObject::Arc(s) => {
                println!("{}[호]", indent);
                dump_common_fn(&s.common, indent);
                dump_sa_fn(&s.drawing.shape_attr, indent);
            }
            ShapeObject::Polygon(s) => {
                println!("{}[다각형] points={}", indent, s.points.len());
                dump_common_fn(&s.common, indent);
                dump_sa_fn(&s.drawing.shape_attr, indent);
                // 좌표 범위 출력
                if !s.points.is_empty() {
                    let min_x = s.points.iter().map(|p| p.x).min().unwrap();
                    let max_x = s.points.iter().map(|p| p.x).max().unwrap();
                    let min_y = s.points.iter().map(|p| p.y).min().unwrap();
                    let max_y = s.points.iter().map(|p| p.y).max().unwrap();
                    println!(
                        "{}  좌표범위: x=[{},{}], y=[{},{}]",
                        indent, min_x, max_x, min_y, max_y
                    );
                }
            }
            ShapeObject::Curve(s) => {
                println!("{}[곡선] points={}", indent, s.points.len());
                dump_common_fn(&s.common, indent);
                dump_sa_fn(&s.drawing.shape_attr, indent);
            }
            ShapeObject::Group(g) => {
                println!("{}[묶음] children={}", indent, g.children.len());
                dump_common_fn(&g.common, indent);
                dump_sa_fn(&g.shape_attr, indent);
                let child_indent = format!("{}  ", indent);
                for (ci, child) in g.children.iter().enumerate() {
                    print!("{}child[{}] ", child_indent, ci);
                    dump_shape(child, &child_indent, dump_common_fn, dump_sa_fn);
                }
            }
            ShapeObject::Picture(p) => {
                println!("{}[그림] bin_data_id={}", indent, p.image_attr.bin_data_id);
                dump_common_fn(&p.common, indent);
                dump_sa_fn(&p.shape_attr, indent);
            }
            ShapeObject::Chart(c) => {
                println!(
                    "{}[차트] type={:?} series={} raw_chart_data={}B",
                    indent,
                    c.chart_type,
                    c.series.len(),
                    c.raw_chart_data.len()
                );
                dump_common_fn(&c.common, indent);
                dump_sa_fn(&c.drawing.shape_attr, indent);
            }
            ShapeObject::Ole(o) => {
                println!(
                    "{}[OLE] bin_data_id={} extent={}x{} flags=0x{:02X} raw={}B",
                    indent,
                    o.bin_data_id,
                    o.extent_x,
                    o.extent_y,
                    o.flags,
                    o.raw_tag_data.len()
                );
                dump_common_fn(&o.common, indent);
                dump_sa_fn(&o.drawing.shape_attr, indent);
            }
        }
    }

    for (sec_idx, section) in document.sections.iter().enumerate() {
        if let Some(fs) = filter_section {
            if sec_idx != fs {
                continue;
            }
        }

        let pd = &section.section_def.page_def;
        println!("=== 구역 {} ===", sec_idx);
        println!(
            "  용지: {:.1}mm × {:.1}mm ({}×{} HU), {}",
            hu_to_mm(pd.width),
            hu_to_mm(pd.height),
            pd.width,
            pd.height,
            if pd.landscape { "가로" } else { "세로" }
        );
        println!(
            "  여백: 좌={:.1} 우={:.1} 상={:.1} 하={:.1} 머리말={:.1} 꼬리말={:.1} mm",
            hu_to_mm(pd.margin_left),
            hu_to_mm(pd.margin_right),
            hu_to_mm(pd.margin_top),
            hu_to_mm(pd.margin_bottom),
            hu_to_mm(pd.margin_header),
            hu_to_mm(pd.margin_footer)
        );

        // 바탕쪽 정보
        if !section.section_def.master_pages.is_empty() {
            println!("  바탕쪽: {}개", section.section_def.master_pages.len());
            for (mi, mp) in section.section_def.master_pages.iter().enumerate() {
                println!("    [{}] {:?}, 문단 {}개, 영역 {}×{} HU, is_ext={}, overlap={}, ext_flags=0x{:04X}, text_ref={}, num_ref={}",
                    mi, mp.apply_to, mp.paragraphs.len(), mp.text_width, mp.text_height,
                    mp.is_extension, mp.overlap, mp.ext_flags, mp.text_ref, mp.num_ref);
                for (pi, para) in mp.paragraphs.iter().enumerate() {
                    println!(
                        "      p[{}]: cc={}, text=\"{}\"",
                        pi,
                        para.controls.len(),
                        if para.text.is_empty() {
                            "(빈 문단)".to_string()
                        } else {
                            para.text.chars().take(30).collect::<String>()
                        }
                    );
                    for (ci, ctrl) in para.controls.iter().enumerate() {
                        let ctrl_name = match ctrl {
                            Control::Table(t) => {
                                let cell_texts: Vec<String> = t
                                    .cells
                                    .iter()
                                    .take(3)
                                    .map(|c| {
                                        c.paragraphs
                                            .iter()
                                            .map(|p| p.text.chars().take(20).collect::<String>())
                                            .collect::<Vec<_>>()
                                            .join("|")
                                    })
                                    .collect();
                                format!("표({}x{}, tac={}, wrap={:?}, vert={:?}/{}, horz={:?}/{}, size={}x{}, cells=[{}])",
                                    t.row_count, t.col_count, t.common.treat_as_char,
                                    t.common.text_wrap, t.common.vert_rel_to, t.common.vertical_offset,
                                    t.common.horz_rel_to, t.common.horizontal_offset,
                                    t.common.width, t.common.height,
                                    cell_texts.join("; "))
                            }
                            Control::Shape(s) => {
                                let mut desc = format!("도형(ctrl_id=0x{:08X}, w={}, h={}, attr=0x{:08X}, wc={:?}, hc={:?})",
                                    s.common().ctrl_id, s.common().width, s.common().height,
                                    s.common().attr, s.common().width_criterion, s.common().height_criterion);
                                // TextBox 내용 출력
                                if let Some(tb) = s.drawing().and_then(|d| d.text_box.as_ref()) {
                                    desc += &format!(" 글상자({}문단)", tb.paragraphs.len());
                                    for (tpi, tp) in tb.paragraphs.iter().enumerate() {
                                        let tp_text: String = tp.text.chars().take(20).collect();
                                        desc += &format!(
                                            "\n          tb_p[{}]: cc={} text=\"{}\"",
                                            tpi,
                                            tp.controls.len(),
                                            tp_text
                                        );
                                        for (tci, tc) in tp.controls.iter().enumerate() {
                                            let tc_name = match tc {
                                                Control::AutoNumber(an) => {
                                                    format!("자동번호({:?})", an.number_type)
                                                }
                                                _ => format!("{:?}", std::mem::discriminant(tc)),
                                            };
                                            desc += &format!(
                                                "\n            tb_ctrl[{}]: {}",
                                                tci, tc_name
                                            );
                                        }
                                    }
                                }
                                desc
                            }
                            Control::Picture(p) => {
                                let wm = p
                                    .image_attr
                                    .watermark_preset()
                                    .map(|s| format!(", watermark={}", s))
                                    .unwrap_or_default();
                                format!(
                                    "그림(bin_id={}, w={}, h={}, tac={}{})",
                                    p.image_attr.bin_data_id,
                                    p.common.width,
                                    p.common.height,
                                    p.common.treat_as_char,
                                    wm
                                )
                            }
                            Control::Header(_) => "머리말".to_string(),
                            Control::Footer(_) => "꼬리말".to_string(),
                            _ => format!("{:?}", std::mem::discriminant(ctrl)),
                        };
                        println!("        ctrl[{}]: {}", ci, ctrl_name);
                    }
                }
            }
        }
        if section.section_def.hide_master_page {
            println!("  바탕쪽 감추기: true");
        }

        for (para_idx, para) in section.paragraphs.iter().enumerate() {
            if let Some(fp) = filter_para {
                if para_idx != fp {
                    continue;
                }
            }

            let text_preview = if para.text.is_empty() {
                "(빈 문단)".to_string()
            } else {
                let preview = if para.text.chars().count() > 50 {
                    let end = para
                        .text
                        .char_indices()
                        .nth(50)
                        .map(|(i, _)| i)
                        .unwrap_or(para.text.len());
                    format!("\"{}...\"", &para.text[..end])
                } else {
                    format!("\"{}\"", para.text)
                };
                preview
            };

            let break_info = break_str(&para.column_type);
            println!(
                "\n--- 문단 {}.{} --- cc={}, text_len={}, controls={} {}",
                sec_idx,
                para_idx,
                para.char_count,
                para.text.chars().count(),
                para.controls.len(),
                break_info
            );
            println!("  텍스트: {}", text_preview);
            // char_shapes 출력
            if !para.char_shapes.is_empty() {
                let text_chars: Vec<char> = para.text.chars().collect();
                for (ci, cs) in para.char_shapes.iter().enumerate() {
                    let next_pos = para
                        .char_shapes
                        .get(ci + 1)
                        .map(|n| n.start_pos)
                        .unwrap_or(u32::MAX);
                    let char_at = text_chars
                        .iter()
                        .enumerate()
                        .find(|(i, _)| {
                            if *i < para.char_offsets.len() {
                                para.char_offsets[*i] >= cs.start_pos
                                    && para.char_offsets[*i] < next_pos
                            } else {
                                false
                            }
                        })
                        .map(|(_, c)| *c);
                    if let Some(chs) = document.doc_info.char_shapes.get(cs.char_shape_id as usize)
                    {
                        let bold = (chs.attr & 0x02) != 0;
                        let spacing = chs.spacings[0]; // 한국어 자간
                        let ratio = chs.ratios[0]; // 한국어 장평
                        println!(
                            "  [CS] pos={} id={} bold={} spacing={}% ratio={}% base={} attr=0x{:08X} text=#{:06X} shade=#{:06X} shadow=#{:06X} border_fill_id={} shadow_type={} shadow_off=({}, {}) char={:?}",
                            cs.start_pos,
                            cs.char_shape_id,
                            bold,
                            spacing,
                            ratio,
                            chs.base_size,
                            chs.attr,
                            chs.text_color,
                            chs.shade_color,
                            chs.shadow_color,
                            chs.border_fill_id,
                            chs.shadow_type,
                            chs.shadow_offset_x,
                            chs.shadow_offset_y,
                            char_at.map(|c| c.to_string()).unwrap_or_default()
                        );
                    }
                }
            }
            if let Some(ps) = document
                .doc_info
                .para_shapes
                .get(para.para_shape_id as usize)
            {
                // 문단 모양 기본 정보 (항상 출력)
                println!(
                    "  [PS] ps_id={} align={:?} spacing: before={} after={} line={}/{:?}",
                    para.para_shape_id,
                    ps.alignment,
                    ps.spacing_before,
                    ps.spacing_after,
                    ps.line_spacing,
                    ps.line_spacing_type
                );
                println!(
                    "       margins: left={} right={} indent={} border_fill_id={}",
                    ps.margin_left, ps.margin_right, ps.indent, ps.border_fill_id
                );
                println!(
                    "       keep: with_next={} keep_lines={} widow_orphan={} pbreak_before={} (attr1=0x{:08X} attr2=0x{:08X})",
                    (ps.attr1 >> 17) & 1 != 0 || (ps.attr2 >> 6) & 1 != 0,
                    (ps.attr1 >> 18) & 1 != 0 || (ps.attr2 >> 7) & 1 != 0,
                    (ps.attr1 >> 16) & 1 != 0 || (ps.attr2 >> 5) & 1 != 0,
                    (ps.attr1 >> 19) & 1 != 0 || (ps.attr2 >> 8) & 1 != 0,
                    ps.attr1, ps.attr2
                );
                if ps.border_fill_id > 0 {
                    println!(
                        "       border_spacing: left={} right={} top={} bottom={}",
                        ps.border_spacing[0],
                        ps.border_spacing[1],
                        ps.border_spacing[2],
                        ps.border_spacing[3]
                    );
                }
                if ps.head_type != rhwp::model::style::HeadType::None {
                    println!("       head={:?} level={} num_id={} attr1=0x{:08X} attr2=0x{:08X} raw_extra={:?}",
                        ps.head_type, ps.para_level, ps.numbering_id, ps.attr1, ps.attr2,
                        &para.raw_header_extra);
                }
                {
                    let td_id = ps.tab_def_id;
                    if let Some(td) = document.doc_info.tab_defs.get(td_id as usize) {
                        let tabs_str: Vec<String> = td
                            .tabs
                            .iter()
                            .enumerate()
                            .map(|(i, t)| {
                                format!(
                                    "tab[{}] pos={} ({:.1}mm) type={} fill={}",
                                    i,
                                    t.position,
                                    hu_to_mm(t.position),
                                    t.tab_type,
                                    t.fill_type
                                )
                            })
                            .collect();
                        println!(
                            "       tab_def_id={} auto_left={} auto_right={} tabs=[{}]",
                            td_id,
                            td.auto_tab_left,
                            td.auto_tab_right,
                            if tabs_str.is_empty() {
                                "(없음)".to_string()
                            } else {
                                tabs_str.join(", ")
                            }
                        );
                    } else {
                        println!("       tab_def_id={} (정의 없음)", td_id);
                    }
                }
            }
            // line_segs 출력
            if !para.line_segs.is_empty() {
                for (li, ls) in para.line_segs.iter().enumerate() {
                    println!("  ls[{}]: ts={}, vpos={}, lh={}, th={}, bl={}, ls={}, cs={}, sw={}, tag=0x{:08X}",
                        li, ls.text_start, ls.vertical_pos, ls.line_height, ls.text_height,
                        ls.baseline_distance, ls.line_spacing, ls.column_start, ls.segment_width, ls.tag);
                }
            }

            for (ctrl_idx, ctrl) in para.controls.iter().enumerate() {
                let prefix = format!("  [{}] ", ctrl_idx);
                match ctrl {
                    Control::ColumnDef(cd) => {
                        let ct = match cd.column_type {
                            rhwp::model::page::ColumnType::Normal => "일반",
                            rhwp::model::page::ColumnType::Distribute => "배분",
                            rhwp::model::page::ColumnType::Parallel => "병행",
                        };
                        println!(
                            "{}단정의: {}단, 유형={}, 간격={:.1}mm({}), 같은너비={}",
                            prefix,
                            cd.column_count,
                            ct,
                            hu_to_mm_i(cd.spacing as i32),
                            cd.spacing,
                            cd.same_width
                        );
                        if !cd.widths.is_empty() {
                            // 비례값일 경우 body_width 기준으로 실제 mm 변환
                            let body_width_hu = {
                                let spd = &section.section_def.page_def;
                                let (pw, _) = if spd.landscape {
                                    (spd.height, spd.width)
                                } else {
                                    (spd.width, spd.height)
                                };
                                (pw - spd.margin_left - spd.margin_right - spd.margin_gutter) as f64
                            };
                            let total: f64 = if cd.proportional_widths {
                                cd.widths
                                    .iter()
                                    .chain(cd.gaps.iter())
                                    .map(|&v| (v as u16) as f64)
                                    .sum()
                            } else {
                                1.0
                            };
                            let cols_info: Vec<String> = cd
                                .widths
                                .iter()
                                .enumerate()
                                .map(|(i, w)| {
                                    let gap = cd.gaps.get(i).copied().unwrap_or(0);
                                    if cd.proportional_widths && total > 0.0 {
                                        let w_hu = (*w as u16) as f64 / total * body_width_hu;
                                        let g_hu = (gap as u16) as f64 / total * body_width_hu;
                                        format!(
                                            "너비={:.1}mm 간격={:.1}mm",
                                            w_hu * 25.4 / 7200.0,
                                            g_hu * 25.4 / 7200.0
                                        )
                                    } else {
                                        format!(
                                            "너비={:.1}mm 간격={:.1}mm",
                                            hu_to_mm_i(*w as i32),
                                            hu_to_mm_i(gap as i32)
                                        )
                                    }
                                })
                                .collect();
                            println!("{}  단별: [{}]", prefix, cols_info.join(", "));
                        }
                        if cd.separator_type > 0 {
                            println!(
                                "{}  구분선: type={}, width={}, color={:#010x}",
                                prefix, cd.separator_type, cd.separator_width, cd.separator_color
                            );
                        }
                    }
                    Control::SectionDef(sd) => {
                        let spd = &sd.page_def;
                        println!(
                            "{}구역정의: 용지 {:.1}×{:.1}mm, {}, flags=0x{:08X}",
                            prefix,
                            hu_to_mm(spd.width),
                            hu_to_mm(spd.height),
                            if spd.landscape { "가로" } else { "세로" },
                            sd.flags
                        );
                        if sd.hide_header || sd.hide_footer || sd.hide_master_page {
                            println!(
                                "{}  감추기: 머리말={} 꼬리말={} 바탕쪽={}",
                                prefix, sd.hide_header, sd.hide_footer, sd.hide_master_page
                            );
                        }
                    }
                    Control::Table(table) => {
                        println!("{}표: {}행×{}열, 셀={}, 쪽나눔={:?} (attr=0x{:08x}), padding=({},{},{},{}), cs={}",
                            prefix, table.row_count, table.col_count,
                            table.cells.len(), table.page_break, table.raw_table_record_attr,
                            table.padding.left, table.padding.right, table.padding.top, table.padding.bottom,
                            table.cell_spacing);
                        if !table.zones.is_empty() {
                            for (zi, z) in table.zones.iter().enumerate() {
                                println!(
                                    "{}  zone[{}] row={}..{} col={}..{} bf={}",
                                    prefix,
                                    zi,
                                    z.start_row,
                                    z.end_row,
                                    z.start_col,
                                    z.end_col,
                                    z.border_fill_id
                                );
                            }
                        }
                        {
                            let c = &table.common;
                            println!("{}  [common] treat_as_char={}, wrap={}, vert={}({}={:.1}mm), horz={}({}={:.1}mm)",
                                prefix, c.treat_as_char, wrap_str(&c.text_wrap),
                                vert_str(&c.vert_rel_to), c.vertical_offset, hu_to_mm(c.vertical_offset),
                                horz_str(&c.horz_rel_to), c.horizontal_offset, hu_to_mm(c.horizontal_offset));
                            println!(
                                "{}  [common] size={}×{}({:.1}×{:.1}mm), valign={:?}, halign={:?}",
                                prefix,
                                c.width,
                                c.height,
                                hu_to_mm(c.width),
                                hu_to_mm(c.height),
                                c.vert_align,
                                c.horz_align
                            );
                            println!("{}  [outer_margin] left={:.1}mm({}) right={:.1}mm({}) top={:.1}mm({}) bottom={:.1}mm({})",
                                prefix,
                                hu_to_mm_i(table.outer_margin_left as i32), table.outer_margin_left,
                                hu_to_mm_i(table.outer_margin_right as i32), table.outer_margin_right,
                                hu_to_mm_i(table.outer_margin_top as i32), table.outer_margin_top,
                                hu_to_mm_i(table.outer_margin_bottom as i32), table.outer_margin_bottom);
                            if table.raw_ctrl_data.len() >= 20 {
                                println!(
                                    "{}  [raw] {:02X?}",
                                    prefix,
                                    &table.raw_ctrl_data[..20.min(table.raw_ctrl_data.len())]
                                );
                            }
                        }
                        // 셀 상세 출력
                        fn dump_table_deep(
                            table: &rhwp::model::table::Table,
                            indent: &str,
                            depth: usize,
                        ) {
                            for (ci, cell) in table.cells.iter().enumerate() {
                                let text_preview: String = cell
                                    .paragraphs
                                    .iter()
                                    .map(|p| p.text.chars().take(30).collect::<String>())
                                    .collect::<Vec<_>>()
                                    .join("|");
                                println!("{}셀[{}] r={},c={} rs={},cs={} h={} w={} pad=({},{},{},{}) valign={:?} aim={} hdr={} bf={} paras={} text=\"{}\"",
                                    indent, ci, cell.row, cell.col, cell.row_span, cell.col_span,
                                    cell.height, cell.width,
                                    cell.padding.left, cell.padding.right, cell.padding.top, cell.padding.bottom,
                                    cell.vertical_align,
                                    cell.apply_inner_margin,
                                    cell.is_header,
                                    cell.border_fill_id, cell.paragraphs.len(), text_preview);
                                if let Some(ref fname) = cell.field_name {
                                    println!("{}  field=\"{}\"", indent, fname);
                                }
                                // 셀 내 LINE_SEG 상세
                                for (pi, cp) in cell.paragraphs.iter().enumerate() {
                                    if !cp.line_segs.is_empty() || !cp.controls.is_empty() {
                                        let ls_info: Vec<String> = cp
                                            .line_segs
                                            .iter()
                                            .enumerate()
                                            .map(|(li, ls)| {
                                                format!(
                                                    "ls[{}] vpos={} lh={} ls={}",
                                                    li,
                                                    ls.vertical_pos,
                                                    ls.line_height,
                                                    ls.line_spacing
                                                )
                                            })
                                            .collect();
                                        println!(
                                            "{}  p[{}] ps_id={} ctrls={} text_len={} {}",
                                            indent,
                                            pi,
                                            cp.para_shape_id,
                                            cp.controls.len(),
                                            cp.text.len(),
                                            ls_info.join(", ")
                                        );
                                    }
                                    // 셀 내부 컨트롤 상세
                                    for (ci, ctrl) in cp.controls.iter().enumerate() {
                                        match ctrl {
                                            Control::Picture(p) => {
                                                println!("{}    ctrl[{}] 그림: bin_id={}, w={} h={} ({:.1}×{:.1}mm), tac={}, wrap={:?}, vert={:?}(off={}), horz={:?}(off={}), orig={}×{}, cur={}×{}, crop=({},{},{},{})",
                                                    indent, ci, p.image_attr.bin_data_id,
                                                    p.common.width, p.common.height,
                                                    p.common.width as f64 / 7200.0 * 25.4,
                                                    p.common.height as f64 / 7200.0 * 25.4,
                                                    p.common.treat_as_char,
                                                    p.common.text_wrap, p.common.vert_rel_to, p.common.vertical_offset,
                                                    p.common.horz_rel_to, p.common.horizontal_offset,
                                                    p.shape_attr.original_width, p.shape_attr.original_height,
                                                    p.shape_attr.current_width, p.shape_attr.current_height,
                                                    p.crop.left, p.crop.top, p.crop.right, p.crop.bottom);
                                                println!("{}      [image_attr] effect={:?} brightness={} contrast={} watermark={}",
                                                    indent, p.image_attr.effect, p.image_attr.brightness, p.image_attr.contrast,
                                                    p.image_attr.watermark_preset().unwrap_or("none"));
                                            }
                                            Control::Shape(s) => {
                                                println!(
                                                    "{}    ctrl[{}] {}: tac={}, wrap={:?}",
                                                    indent,
                                                    ci,
                                                    s.shape_name(),
                                                    s.common().treat_as_char,
                                                    s.common().text_wrap
                                                );
                                            }
                                            Control::PageHide(ph) => {
                                                println!("{}    ctrl[{}] PageHide: header={} footer={} master={} border={} fill={} page_num={}",
                                                    indent, ci,
                                                    ph.hide_header, ph.hide_footer, ph.hide_master_page,
                                                    ph.hide_border, ph.hide_fill, ph.hide_page_num);
                                            }
                                            _ => {}
                                        }
                                    }
                                    // 내부 표 재귀
                                    if depth < 3 {
                                        for ctrl in &cp.controls {
                                            if let Control::Table(inner) = ctrl {
                                                println!("{}  p[{}] 내부표: {}행×{}열, 셀={}, cs={}, pad=({},{},{},{})",
                                                    indent, pi, inner.row_count, inner.col_count,
                                                    inner.cells.len(), inner.cell_spacing,
                                                    inner.padding.left, inner.padding.right, inner.padding.top, inner.padding.bottom);
                                                let next_indent = format!("{}    ", indent);
                                                dump_table_deep(inner, &next_indent, depth + 1);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        dump_table_deep(table, &format!("{}  ", prefix), 0);
                    }
                    Control::Shape(shape) => {
                        print!("{}", prefix);
                        dump_shape(shape, "  ", &dump_common, &dump_shape_attr);
                    }
                    Control::Picture(pic) => {
                        let sa = &pic.shape_attr;
                        println!("{}그림: bin_id={}, common={}×{} ({:.1}×{:.1}mm), orig={}×{} ({:.1}×{:.1}mm), cur={}×{} ({:.1}×{:.1}mm), tac={}",
                            prefix, pic.image_attr.bin_data_id, pic.common.width, pic.common.height,
                            pic.common.width as f64 / 7200.0 * 25.4, pic.common.height as f64 / 7200.0 * 25.4,
                            sa.original_width, sa.original_height,
                            sa.original_width as f64 / 7200.0 * 25.4, sa.original_height as f64 / 7200.0 * 25.4,
                            sa.current_width, sa.current_height,
                            sa.current_width as f64 / 7200.0 * 25.4, sa.current_height as f64 / 7200.0 * 25.4,
                            pic.common.treat_as_char);
                        println!(
                            "{}  [placement] wrap={:?} vert={:?}(off={}) horz={:?}(off={}) vert_align={:?}",
                            prefix, pic.common.text_wrap, pic.common.vert_rel_to, pic.common.vertical_offset,
                            pic.common.horz_rel_to, pic.common.horizontal_offset, pic.common.vert_align);
                        println!(
                            "{}  [image_attr] effect={:?} brightness={} contrast={} watermark={}{}",
                            prefix,
                            pic.image_attr.effect,
                            pic.image_attr.brightness,
                            pic.image_attr.contrast,
                            pic.image_attr.watermark_preset().unwrap_or("none"),
                            pic.image_attr
                                .external_path
                                .as_ref()
                                .map(|p| format!(" external_path=\"{}\"", p))
                                .unwrap_or_default()
                        );
                        println!("{}  border_x={:?} border_y={:?} border_color=#{:06X} border_width={} ({:.2}mm) border_attr={:?}",
                            prefix, pic.border_x, pic.border_y,
                            pic.border_color, pic.border_width, pic.border_width as f64 / 7200.0 * 25.4,
                            pic.border_attr);
                        println!(
                            "{}  crop=({},{},{},{}) crop_mm=({:.2},{:.2},{:.2},{:.2})",
                            prefix,
                            pic.crop.left,
                            pic.crop.top,
                            pic.crop.right,
                            pic.crop.bottom,
                            pic.crop.left as f64 / 7200.0 * 25.4,
                            pic.crop.top as f64 / 7200.0 * 25.4,
                            pic.crop.right as f64 / 7200.0 * 25.4,
                            pic.crop.bottom as f64 / 7200.0 * 25.4
                        );
                        if let Some(ref cap) = pic.caption {
                            let cap_text: String = cap
                                .paragraphs
                                .iter()
                                .map(|p| p.text.clone())
                                .collect::<Vec<_>>()
                                .join("|");
                            println!(
                                "{}  caption: dir={:?} width={} paras={} text={:?}",
                                prefix,
                                cap.direction,
                                cap.width,
                                cap.paragraphs.len(),
                                cap_text
                            );
                        }
                        let shape_indent = format!("{}  ", prefix);
                        dump_shape_attr(sa, &shape_indent);
                        dump_common(&pic.common, "  ");
                    }
                    Control::Header(h) => {
                        let text: String = h
                            .paragraphs
                            .iter()
                            .filter(|p| !p.text.is_empty())
                            .map(|p| p.text.clone())
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!(
                            "{}머리말({:?}): paras={} \"{}\"",
                            prefix,
                            h.apply_to,
                            h.paragraphs.len(),
                            text
                        );
                        for (hpi, hp) in h.paragraphs.iter().enumerate() {
                            if !hp.controls.is_empty() {
                                for (hci, hc) in hp.controls.iter().enumerate() {
                                    let cn = match hc {
                                        Control::AutoNumber(an) => {
                                            format!("자동번호({:?})", an.number_type)
                                        }
                                        Control::Shape(s) => {
                                            let c = s.common();
                                            let mut desc = format!(
                                                "Shape horz={:?}/{} halign={:?} w={} h={}",
                                                c.horz_rel_to,
                                                c.horizontal_offset,
                                                c.horz_align,
                                                c.width,
                                                c.height
                                            );
                                            if let Some(tb) =
                                                s.drawing().and_then(|d| d.text_box.as_ref())
                                            {
                                                let text: String = tb
                                                    .paragraphs
                                                    .iter()
                                                    .flat_map(|p| p.text.chars().take(20))
                                                    .collect();
                                                desc += &format!(" text={:?}", text);
                                            }
                                            desc
                                        }
                                        Control::Table(t) => {
                                            let mut desc = format!(
                                                "표 {}행×{}열 셀={}",
                                                t.row_count,
                                                t.col_count,
                                                t.cells.len()
                                            );
                                            for (si, cell) in t.cells.iter().enumerate() {
                                                let cell_text: String = cell
                                                    .paragraphs
                                                    .iter()
                                                    .flat_map(|p| p.text.chars().take(20))
                                                    .collect();
                                                desc += &format!(
                                                    "\n{}    셀[{}] text={:?}",
                                                    prefix, si, cell_text
                                                );
                                                for (cpi, cp) in cell.paragraphs.iter().enumerate()
                                                {
                                                    for (cci, cc) in cp.controls.iter().enumerate()
                                                    {
                                                        let ccn = match cc {
                                                            Control::AutoNumber(an) => format!(
                                                                "자동번호({:?})",
                                                                an.number_type
                                                            ),
                                                            Control::Shape(s) => {
                                                                let c = s.common();
                                                                let mut d = format!("Shape vert={:?}/{} valign={:?} horz={:?}/{} halign={:?} w={} h={}",
                                                c.vert_rel_to, c.vertical_offset, c.vert_align,
                                                c.horz_rel_to, c.horizontal_offset, c.horz_align, c.width, c.height);
                                                                if let Some(tb) =
                                                                    s.drawing().and_then(|dd| {
                                                                        dd.text_box.as_ref()
                                                                    })
                                                                {
                                                                    for (tpi, tp) in tb
                                                                        .paragraphs
                                                                        .iter()
                                                                        .enumerate()
                                                                    {
                                                                        let t: String = tp
                                                                            .text
                                                                            .chars()
                                                                            .take(30)
                                                                            .collect();
                                                                        d += &format!(" tb_p[{}] ps_id={} text={:?}", tpi, tp.para_shape_id, t);
                                                                    }
                                                                }
                                                                d
                                                            }
                                                            _ => format!(
                                                                "{:?}",
                                                                std::mem::discriminant(cc)
                                                            ),
                                                        };
                                                        desc += &format!(
                                                            "\n{}      p[{}]c[{}]: {}",
                                                            prefix, cpi, cci, ccn
                                                        );
                                                    }
                                                }
                                            }
                                            desc
                                        }
                                        Control::Picture(pic) => {
                                            let sa = &pic.shape_attr;
                                            format!("그림: bin_id={}, common={}×{} ({:.1}×{:.1}mm), orig={}×{} ({:.1}×{:.1}mm), cur={}×{} ({:.1}×{:.1}mm), tac={}, crop=({},{},{},{}) crop_mm=({:.2},{:.2},{:.2},{:.2})",
                                            pic.image_attr.bin_data_id, pic.common.width, pic.common.height,
                                            pic.common.width as f64 / 7200.0 * 25.4, pic.common.height as f64 / 7200.0 * 25.4,
                                            sa.original_width, sa.original_height,
                                            sa.original_width as f64 / 7200.0 * 25.4, sa.original_height as f64 / 7200.0 * 25.4,
                                            sa.current_width, sa.current_height,
                                            sa.current_width as f64 / 7200.0 * 25.4, sa.current_height as f64 / 7200.0 * 25.4,
                                            pic.common.treat_as_char,
                                            pic.crop.left, pic.crop.top, pic.crop.right, pic.crop.bottom,
                                            pic.crop.left as f64 / 7200.0 * 25.4, pic.crop.top as f64 / 7200.0 * 25.4,
                                            pic.crop.right as f64 / 7200.0 * 25.4, pic.crop.bottom as f64 / 7200.0 * 25.4)
                                        }
                                        _ => format!("{:?}", std::mem::discriminant(hc)),
                                    };
                                    let display = if cn.chars().count() > 30 {
                                        format!(
                                            "{}...(truncated)",
                                            cn.chars().take(30).collect::<String>()
                                        )
                                    } else {
                                        cn
                                    };
                                    println!("{}  hp[{}] ctrl[{}]: {}", prefix, hpi, hci, display);
                                }
                            }
                        }
                    }
                    Control::Footer(f) => {
                        let text: String = f
                            .paragraphs
                            .iter()
                            .filter(|p| !p.text.is_empty())
                            .map(|p| p.text.clone())
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!(
                            "{}꼬리말({:?}): paras={} \"{}\"",
                            prefix,
                            f.apply_to,
                            f.paragraphs.len(),
                            text
                        );
                        for (fpi, fp) in f.paragraphs.iter().enumerate() {
                            if !fp.controls.is_empty() {
                                for (fci, fc) in fp.controls.iter().enumerate() {
                                    let cn = match fc {
                                        Control::Picture(pic) => {
                                            let sa = &pic.shape_attr;
                                            format!("그림: bin_id={}, common={}×{} ({:.1}×{:.1}mm), orig={}×{} ({:.1}×{:.1}mm), cur={}×{} ({:.1}×{:.1}mm), tac={}, crop=({},{},{},{}) crop_mm=({:.2},{:.2},{:.2},{:.2})",
                                            pic.image_attr.bin_data_id, pic.common.width, pic.common.height,
                                            pic.common.width as f64 / 7200.0 * 25.4, pic.common.height as f64 / 7200.0 * 25.4,
                                            sa.original_width, sa.original_height,
                                            sa.original_width as f64 / 7200.0 * 25.4, sa.original_height as f64 / 7200.0 * 25.4,
                                            sa.current_width, sa.current_height,
                                            sa.current_width as f64 / 7200.0 * 25.4, sa.current_height as f64 / 7200.0 * 25.4,
                                            pic.common.treat_as_char,
                                            pic.crop.left, pic.crop.top, pic.crop.right, pic.crop.bottom,
                                            pic.crop.left as f64 / 7200.0 * 25.4, pic.crop.top as f64 / 7200.0 * 25.4,
                                            pic.crop.right as f64 / 7200.0 * 25.4, pic.crop.bottom as f64 / 7200.0 * 25.4)
                                        }
                                        _ => format!("{:?}", std::mem::discriminant(fc)),
                                    };
                                    println!("{}  fp[{}] ctrl[{}]: {}", prefix, fpi, fci, cn);
                                }
                            }
                        }
                    }
                    Control::Footnote(fn_) => {
                        println!("{}각주: paragraphs={}", prefix, fn_.paragraphs.len());
                    }
                    Control::Endnote(en) => {
                        println!("{}미주: paragraphs={}", prefix, en.paragraphs.len());
                    }
                    Control::AutoNumber(an) => {
                        println!(
                            "{}자동번호: type={:?}, number={}",
                            prefix, an.number_type, an.number
                        );
                    }
                    Control::NewNumber(nn) => {
                        println!(
                            "{}새번호: type={:?}, number={}",
                            prefix, nn.number_type, nn.number
                        );
                    }
                    Control::PageNumberPos(pn) => {
                        println!(
                            "{}쪽번호위치: format={}, pos={}",
                            prefix, pn.format, pn.position
                        );
                    }
                    Control::Bookmark(bm) => {
                        println!("{}책갈피: \"{}\"", prefix, bm.name);
                    }
                    Control::Hyperlink(hl) => {
                        println!("{}하이퍼링크: \"{}\"", prefix, hl.url);
                    }
                    Control::Ruby(r) => {
                        println!("{}덧말: \"{}\"", prefix, r.ruby_text);
                    }
                    Control::PageHide(ph) => {
                        println!("{}감추기: header={}, footer={}, master={}, border={}, fill={}, page_num={}",
                            prefix, ph.hide_header, ph.hide_footer, ph.hide_master_page, ph.hide_border, ph.hide_fill, ph.hide_page_num);
                    }
                    Control::HiddenComment(_) => {
                        println!("{}숨은설명", prefix);
                    }
                    Control::Field(f) => {
                        let name = f.field_name().unwrap_or("(이름없음)");
                        println!(
                            "{}필드: {:?} name=\"{}\" cmd=\"{}\"",
                            prefix, f.field_type, name, f.command
                        );
                    }
                    Control::CharOverlap(co) => {
                        println!("{}글자겹침: {:?}", prefix, co.chars);
                    }
                    Control::Equation(eq) => {
                        println!(
                            "{}수식: script=\"{}\" font_size={} font=\"{}\" size={}x{} tac={}",
                            prefix,
                            eq.script,
                            eq.font_size,
                            eq.font_name,
                            eq.common.width,
                            eq.common.height,
                            eq.common.treat_as_char
                        );
                    }
                    Control::Form(f) => {
                        println!(
                            "{}양식개체: {:?} name=\"{}\" caption=\"{}\" {}x{}",
                            prefix, f.form_type, f.name, f.caption, f.width, f.height
                        );
                    }
                    Control::Unknown(u) => {
                        println!("{}알수없음: ctrl_id={:#010x}", prefix, u.ctrl_id);
                    }
                }
            }
        }
    }

    println!(
        "\n=== 완료: {} 구역, {} 문단 ===",
        document.sections.len(),
        document
            .sections
            .iter()
            .map(|s| s.paragraphs.len())
            .sum::<usize>()
    );

    EXIT_OK
}



/// `search` — 주소(구역·문단·**페이지**)를 가진 문서 검색.
///
/// 평문을 뽑아 외부에서 찾으면 주소가 소멸해 근거 제시가 불가능하다. rhwp 는 조판 엔진이
/// 있어 "몇 쪽"에 답할 수 있는 유일한 도구인데, 그 출구가 없었다.
pub(crate) fn search_document(args: &[String]) -> i32 {
    let mut file_path: Option<&str> = None;
    let mut query: Option<&str> = None;
    let mut json_mode = false;
    let mut ignore_case = false;
    let mut limit: Option<usize> = None;
    let mut context: Option<usize> = None;

    // POSIX 옵션 종결자. 검색어가 '-' 로 시작하면 종전에는 플래그로 먹혔다 —
    // `-i` 는 대소문자 축을 **조용히** 뒤집고(리터럴 "-i" 를 찾으려던 호출이 다음
    // 위치 인자를 대소문자 무시로 검색한다), 그 외에는 "알 수 없는 옵션" 으로 죽어
    // 하이픈으로 시작하는 문자열은 아예 검색할 수 없었다.
    let mut end_of_options = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--" if !end_of_options => end_of_options = true,
            "--json" if !end_of_options => json_mode = true,
            "--ignore-case" | "-i" if !end_of_options => ignore_case = true,
            // [#3787 S7] `--max-matches` 는 자원 상한 어휘를 텍스트 축
            // (`export-text --max-chars`)과 맞춘 이름이고, `--limit`(#3353)은 같은
            // 축의 기존 이름이다. 두 이름이 같은 변수를 채우므로 의미 분기는 없다.
            "--limit" | "--max-matches" if !end_of_options => {
                let flag = args[i].clone();
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(n) if n >= 1 => limit = Some(n),
                    _ => {
                        eprintln!("오류: {flag} 뒤에 1 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            // [#3835] 매치 앞뒤 문단을 함께 보고 싶은 에이전트용 — 매치가 속한 문단의
            // 앞뒤 N개 문단 텍스트를 matches[].contextBefore/contextAfter 로 얹는다.
            // 기본(플래그 없음)은 종전과 완전히 동일하다.
            "--context" if !end_of_options => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(n) if n >= 1 => context = Some(n),
                    _ => {
                        eprintln!("오류: --context 뒤에 1 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if !end_of_options && other.starts_with('-') => {
                // 옵션 오타는 계속 거부한다(삼키면 오타가 검색어가 되어 조용히 0건이 된다).
                // 다만 검색어가 정말 '-' 로 시작하는 경우 빠져나갈 길을 알려줘야 한다 —
                // 안내가 없으면 에이전트는 "고치라"는 exit 2 를 받고도 고칠 방법을 모른다.
                eprintln!(
                    "알 수 없는 옵션: {other}\n\
                     힌트: 검색어가 '-' 로 시작한다면 `--` 뒤에 두세요 — \
                     rhwp search <파일> --json -- <검색어>"
                );
                return EXIT_USAGE;
            }
            other => {
                if file_path.is_none() {
                    file_path = Some(other);
                } else if query.is_none() {
                    query = Some(other);
                } else {
                    eprintln!("오류: 인자가 너무 많습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }

    let (Some(file_path), Some(query)) = (file_path, query) else {
        eprintln!(
            "사용법: rhwp search <파일.hwp|파일.hwpx> <검색어> [--json] [--ignore-case] \
             [--max-matches <N>] [--context <N>]"
        );
        return EXIT_USAGE;
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    // [#3353] 총량을 보고하려면 전수 스캔이 불가피하다 — `--limit` 의 목적은 스캔 시간이
    // 아니라 출력 컨텍스트 절약이므로, 전수 grep 후 표시만 절단한다. 절단 사실을 숨기면
    // 에이전트가 "정확히 N건"과 "N건만 표시(실제 그 이상)"를 구별할 수 없다.
    let all_matches = doc.grep_with_context(query, !ignore_case, None, context);
    let total_match_count = all_matches.len();
    let matches: Vec<_> = match limit {
        Some(n) => all_matches.into_iter().take(n).collect(),
        None => all_matches,
    };
    let truncated = matches.len() < total_match_count;

    if json_mode {
        // [#3353] matchCount 는 반환된 매치 수이고, 추가-전용 totalMatchCount·truncated가
        // 전체 수와 절단 여부를 표현한다. #3346 batch와 하나의 helper를 공유한다.
        let envelope =
            search_json_value(file_path, query, !ignore_case, &matches, total_match_count);
        println!("{envelope}");
        // 매치 0건은 실패가 아니다 — 1은 런타임 실패 전용이다(#2707).
        return EXIT_OK;
    }

    if truncated {
        println!(
            "검색: {:?} in {} — {}건 중 {}건 표시 (--max-matches)",
            query,
            file_path,
            total_match_count,
            matches.len()
        );
    } else {
        println!("검색: {:?} in {} — {}건", query, file_path, matches.len());
    }
    for m in &matches {
        let page = m
            .page
            .map(|p| format!("{}쪽", p + 1))
            .unwrap_or_else(|| "쪽 미배치".to_string());
        println!(
            "  [{}] 구역{}:문단{} +{}  {}",
            page, m.section, m.paragraph, m.char_offset, m.context
        );
    }
    EXIT_OK
}



/// [#3565] `extract-pages` — 쪽 범위만 남겨 저장한다.
///
/// 대형 문서의 결함을 이분법으로 좁히기 위한 도구다. 384쪽 문서가 저장 후 한컴에서
/// 열리지 않을 때, 절반씩 잘라 재현 여부를 보면 방아쇠를 특정할 수 있다.
///
/// 쪽 단위로 자르되 **문단 단위로** 지운다 — 여러 쪽에 걸친 문단은 한 쪽이라도 범위 안이면
/// 남긴다. 결과 쪽수가 요청 범위와 정확히 같지 않을 수 있다(레이아웃이 다시 흐른다).
pub(crate) fn extract_pages(args: &[String]) -> i32 {
    let mut input: Option<&str> = None;
    let mut output: Option<&str> = None;
    let mut from: Option<u32> = None;
    let mut to: Option<u32> = None;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--from" | "--to" => {
                // 옵션 이름을 리터럴로 고정하고 인자 값은 에코하지 않는다.
                // 같은 `args` 에 `--password` 가 실릴 수 있어, 인자에서 온 문자열을
                // 그대로 찍으면 비밀번호가 로그에 남는다 (CodeQL: cleartext logging).
                let opt: &'static str = if args[i] == "--from" {
                    "--from"
                } else {
                    "--to"
                };
                i += 1;
                let Some(v) = args.get(i) else {
                    eprintln!("오류: {opt} 뒤에 쪽 번호가 필요합니다.");
                    return EXIT_USAGE;
                };
                let Ok(n) = v.parse::<u32>() else {
                    eprintln!("오류: {opt} 값이 숫자가 아닙니다.");
                    return EXIT_USAGE;
                };
                if opt == "--from" {
                    from = Some(n)
                } else {
                    to = Some(n)
                }
            }
            "-o" | "--output" => {
                i += 1;
                output = args.get(i).map(|s| s.as_str());
            }
            "--json" => json_mode = true,
            v if v.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {v}");
                return EXIT_USAGE;
            }
            v => {
                if input.is_none() {
                    input = Some(v)
                } else if output.is_none() {
                    output = Some(v)
                }
            }
        }
        i += 1;
    }

    let (Some(input), Some(output)) = (input, output) else {
        eprintln!("사용법: rhwp extract-pages <입력> <출력.hwp> --from N --to M [--json]");
        return EXIT_USAGE;
    };
    let from = from.unwrap_or(1);
    let Some(to) = to else {
        eprintln!("오류: --to 가 필요합니다.");
        return EXIT_USAGE;
    };

    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {input}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };
    let report = match doc.extract_page_range(from, to) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("오류: 쪽 추출 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    let bytes = match doc.export_hwp_with_adapter() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: HWP 직렬화 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    if let Err(e) = fs::write(output, &bytes) {
        eprintln!("오류: 출력 쓰기 실패 - {output}: {e}");
        return EXIT_RUNTIME;
    }

    if json_mode {
        println!(
            "{}",
            provenance::marked(
                serde_json::json!({
                    "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                    "source": input,
                    "output": output,
                    "from": from,
                    "to": to,
                    "pagesBefore": report.pages_before,
                    "pagesAfter": report.pages_after,
                    "paragraphsKept": report.kept,
                    "paragraphsRemoved": report.removed,
                }),
                "extract-pages",
            )
        );
    } else {
        println!(
            "추출 완료: {output} ({}~{}쪽) — {}쪽 → {}쪽, 문단 {}개 남기고 {}개 제거",
            from, to, report.pages_before, report.pages_after, report.kept, report.removed
        );
    }
    EXIT_OK
}


/// `rhwp export-hwpx <입력.hwp|입력.hwpx> [출력.hwpx]` — HWP→HWPX 직접 변환 (#1868).
///
/// 파서가 포맷을 자동 감지(HWP5/HWP3/HWPX)해 `Document` IR 로 읽고
/// `export_hwpx_native()` 로 HWPX(ZIP) 직렬화한다. `convert`(배포용 해제 → .hwp 출력)와
/// 별개의 포맷 변환 명령. 출력 생략 시 입력과 같은 폴더에 `<stem>.hwpx`.
pub(crate) fn export_doclang(args: &[String]) -> i32 {
    // [#3359] 위치 인자 파싱은 export-structure/export-text(#3349) 규약과 동일.
    let mut file_path: Option<&str> = None;
    let mut output_override: Option<std::path::PathBuf> = None;
    let mut assets_dir: Option<std::path::PathBuf> = None;
    // [#3696] --json: 산출 봉투를 stdout 순수 JSON 으로. 변환 동작 무변경.
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_mode = true;
                i += 1;
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_override = Some(std::path::PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("오류: --output 뒤에 파일 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            "--assets-dir" => {
                if i + 1 < args.len() {
                    assets_dir = Some(std::path::PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("오류: --assets-dir 뒤에 디렉터리 경로가 필요합니다.");
                    return EXIT_USAGE;
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
                i += 1;
            }
        }
    }

    let Some(file_path) = file_path else {
        eprintln!("오류: 문서 파일 경로를 지정해주세요.");
        eprintln!(
            "사용법: rhwp export-doclang <파일.hwp|파일.hwpx> [-o <출력.xml>] [--assets-dir <디렉터리>] [--json] (rhwp --help 참조)"
        );
        return EXIT_USAGE;
    };

    // 기본 출력 경로: 입력 stem + `.dclg.xml` (입력 파일 옆).
    let input_path = std::path::Path::new(file_path);
    let output_path = output_override.unwrap_or_else(|| input_path.with_extension("dclg.xml"));
    if paths_refer_to_same_file(input_path, &output_path) {
        eprintln!("오류: 입력과 출력 경로가 같습니다. 원본을 덮어쓰지 않습니다.");
        return EXIT_USAGE;
    }

    let data = match fs::read(input_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "오류: 파일을 읽을 수 없습니다 - {}: {}",
                input_path.display(),
                e
            );
            return EXIT_RUNTIME;
        }
    };

    // 자원 정책: --assets-dir 지정 시 AssetDir(디렉터리 경로를 URI 접두어로), 아니면 인라인.
    let mut opts = rhwp::doclang::ConvertOptions::default();
    if let Some(dir) = &assets_dir {
        opts.resource_policy =
            rhwp::doclang::ResourcePolicy::asset_dir(dir.to_string_lossy().into_owned());
    }

    let outcome = match rhwp::doclang::convert(&data, &opts) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("오류: DocLang 변환 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    // 이진 자원을 먼저 기록한다(있을 때만) — XML 저장 전에 실패를 드러내기 위함.
    if let Some(dir) = &assets_dir {
        if !outcome.assets.is_empty() {
            if let Err(e) = fs::create_dir_all(dir) {
                eprintln!(
                    "오류: 에셋 디렉터리를 만들 수 없습니다 - {}: {}",
                    dir.display(),
                    e
                );
                return EXIT_RUNTIME;
            }
            for asset in &outcome.assets {
                let asset_path = dir.join(&asset.path);
                if let Err(e) = fs::write(&asset_path, &asset.data) {
                    eprintln!("오류: 에셋 저장 실패 - {}: {}", asset_path.display(), e);
                    return EXIT_RUNTIME;
                }
            }
        }
    }

    match fs::write(&output_path, outcome.xml.as_bytes()) {
        Ok(_) => {
            if json_mode {
                // [#3696] 산출 봉투 — 사람용 출력(크기·에셋·손실 건수)의 기계 대응물.
                // assetsDir 는 --assets-dir 를 준 경우에만 문자열, 아니면 null.
                println!(
                    "{}",
                    provenance::marked(
                        serde_json::json!({
                            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                            "source": file_path,
                            "output": output_path.display().to_string(),
                            "format": "doclang",
                            "doclangVersion": rhwp::doclang::DOCLANG_VERSION,
                            "bytes": outcome.xml.len(),
                            "assetsDir": assets_dir.as_ref().map(|d| d.display().to_string()),
                            "assetCount": outcome.assets.len(),
                            "lossCount": outcome.loss.len(),
                        }),
                        "export-doclang",
                    )
                );
                return EXIT_OK;
            }
            println!(
                "저장 완료: {} ({}KB)",
                output_path.display(),
                outcome.xml.len() / 1024
            );
            if let Some(dir) = &assets_dir {
                if !outcome.assets.is_empty() {
                    println!("에셋 {}개 저장: {}", outcome.assets.len(), dir.display());
                }
            }
            let loss_count = outcome.loss.len();
            if loss_count > 0 {
                println!(
                    "손실 보고: {}건 (DocLang v0.6 으로 표현할 수 없는 정보)",
                    loss_count
                );
            }
            EXIT_OK
        }
        Err(e) => {
            eprintln!("오류: 파일 저장 실패 - {}: {}", output_path.display(), e);
            EXIT_RUNTIME
        }
    }
}


pub(crate) fn export_hwpx(args: &[String]) -> i32 {
    let (positionals, verify_options) = match parse_conversion_verify_args(
        args,
        "rhwp export-hwpx <입력.hwp|입력.hwpx> [출력.hwpx] [--verify] [--verify-pages] [--json]",
        1,
        2,
        true,
    ) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{}", message);
            return EXIT_USAGE;
        }
    };

    let input_path = std::path::Path::new(&positionals[0]);
    let output_path = match positionals.get(1) {
        Some(p) => std::path::PathBuf::from(p),
        None => input_path.with_extension("hwpx"),
    };
    if output_path
        .extension()
        .map(|e| !e.eq_ignore_ascii_case("hwpx"))
        .unwrap_or(true)
    {
        eprintln!(
            "경고: 출력 확장자가 .hwpx 가 아닙니다: {}",
            output_path.display()
        );
    }
    if output_path == input_path {
        eprintln!("오류: 입력과 출력 경로가 같습니다. 원본을 덮어쓰지 않습니다.");
        return EXIT_USAGE;
    }

    let data = match fs::read(input_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "오류: 파일을 읽을 수 없습니다 - {}: {}",
                input_path.display(),
                e
            );
            return EXIT_RUNTIME;
        }
    };

    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let page_count_before = if verify_options.verify_pages {
        Some(doc.page_count())
    } else {
        None
    };

    // [#3596] JSON 봉투: 판정(verify/verifyPages)까지 채운 뒤 한 번에 낸다.
    // 종료 코드 계약(0/1/3/4)은 무변경 — 차이가 검출되어도 봉투를 stdout 에 내고
    // exit 3/4 로 끝난다(ir-diff --json 과 같은 "판정은 데이터" 규약).
    let json_mode = verify_options.json;
    let output_password = cli_output_password();
    let emit_envelope =
        |bytes_len: usize, verify: serde_json::Value, verify_pages: serde_json::Value| {
            println!(
                "{}",
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                        "source": positionals[0],
                        "output": output_path.display().to_string(),
                        "format": "hwpx",
                        "bytes": bytes_len,
                        "passwordProtected": output_password.is_some(),
                        "verify": verify,
                        "verifyPages": verify_pages,
                    }),
                    "export-hwpx",
                )
            );
        };

    let serialized = match output_password.as_deref() {
        Some(password) => doc.export_hwpx_native_with_password(password.as_bytes()),
        None => doc.export_hwpx_native(),
    };
    match serialized {
        Ok(bytes) => match fs::write(&output_path, &bytes) {
            Ok(_) => {
                if !json_mode {
                    println!(
                        "저장 완료: {} ({}KB)",
                        output_path.display(),
                        bytes.len() / 1024
                    );
                }
                let mut verify_report = serde_json::Value::Null;
                let mut verify_pages_report = serde_json::Value::Null;
                let mut exit_code = EXIT_OK;
                if verify_options.enabled() {
                    let reloaded = match output_password.as_deref() {
                        Some(password) => rhwp::wasm_api::HwpDocument::from_bytes_with_password(
                            &bytes,
                            password.as_bytes(),
                        ),
                        None => rhwp::wasm_api::HwpDocument::from_bytes(&bytes),
                    };
                    let reloaded = match reloaded {
                        Ok(d) => d,
                        Err(e) => {
                            // 재파싱 실패는 판정 불가 — JSON 모드에서도 stdout 을 비운다.
                            eprintln!("검증 실패: 저장된 HWPX 재파싱 실패 - {}", e);
                            process::exit(verify_reparse_failed_exit_code(verify_options));
                        }
                    };

                    if let Some(before) = page_count_before {
                        let after = reloaded.page_count();
                        if before != after {
                            eprintln!(
                                "검증 실패(--verify-pages): 변환 전 {}쪽, 재파싱 후 {}쪽",
                                before, after
                            );
                            verify_pages_report = serde_json::json!({
                                "before": before, "after": after, "identical": false,
                            });
                            // [#3915] 여기서 곧장 종료하면 `--verify` 를 함께 준 경우 IR
                            // 비교가 아예 돌지 않아 **IR 차이가 있어도 보고되지 않는다.**
                            // 쪽수와 IR 은 서로 다른 결함을 재므로, 한쪽이 실패해도 다른
                            // 쪽을 마저 재고 함께 보고한다. 종료 코드는 종전대로 쪽수
                            // 실패를 우선한다(4) — 계약 무변경.
                            exit_code = 4;
                        } else {
                            if !json_mode {
                                println!("검증 통과(--verify-pages): {}쪽", before);
                            }
                            verify_pages_report = serde_json::json!({
                                "before": before, "after": after, "identical": true,
                            });
                        }
                    }

                    if verify_options.verify {
                        let diff = rhwp::serializer::hwpx::roundtrip::diff_documents(
                            doc.document(),
                            reloaded.document(),
                        );
                        if !diff.is_empty() {
                            print_ir_verify_failure(&diff, &output_path.display().to_string());
                            verify_report = serde_json::json!({
                                "identical": false, "diffCount": diff.differences.len(),
                            });
                            // [#3915] 쪽수 실패(4)가 이미 잡혔으면 그 코드를 유지한다 —
                            // 두 축이 함께 실패해도 종전 계약대로 4 로 끝난다.
                            if exit_code == EXIT_OK {
                                exit_code = 3;
                            }
                        } else {
                            if !json_mode {
                                println!("검증 통과(--verify): IR 차이 없음");
                            }
                            verify_report = serde_json::json!({
                                "identical": true, "diffCount": 0,
                            });
                        }
                    }
                }
                if json_mode {
                    emit_envelope(bytes.len(), verify_report, verify_pages_report);
                }
                if exit_code != EXIT_OK {
                    process::exit(exit_code);
                }
                EXIT_OK
            }
            Err(e) => {
                eprintln!("오류: 파일 저장 실패 - {}: {}", output_path.display(), e);
                // [#2707] 출력 파일이 아예 안 만들어졌는데 0으로 끝나던 경로.
                EXIT_RUNTIME
            }
        },
        Err(e) => {
            eprintln!("오류: HWPX 직렬화 실패 - {}", e);
            EXIT_RUNTIME
        }
    }
}


pub(crate) fn parse_hml_export_args(args: &[String]) -> Result<HmlExportArgs, String> {
    let usage = "rhwp export-hml <입력.hml> -o <출력.hml> [--json]";
    let mut input = None;
    let mut output = None;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "-o" | "--output" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("출력 경로가 필요합니다\n사용법: {usage}"))?;
                if value.starts_with('-') {
                    return Err(format!("출력 경로가 필요합니다\n사용법: {usage}"));
                }
                if output.replace(std::path::PathBuf::from(value)).is_some() {
                    return Err(format!("출력 경로를 한 번만 지정하세요\n사용법: {usage}"));
                }
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("알 수 없는 옵션: {value}\n사용법: {usage}"));
            }
            value => {
                if input.replace(std::path::PathBuf::from(value)).is_some() {
                    return Err(format!("입력 파일을 하나만 지정하세요\n사용법: {usage}"));
                }
                index += 1;
            }
        }
    }
    Ok(HmlExportArgs {
        json,
        input: input.ok_or_else(|| format!("입력 파일이 필요합니다\n사용법: {usage}"))?,
        output: output.ok_or_else(|| format!("출력 경로가 필요합니다\n사용법: {usage}"))?,
    })
}


pub(crate) fn paths_refer_to_same_file(input: &Path, output: &Path) -> bool {
    input == output
        || paths_have_same_file_identity(input, output)
        || match (input.canonicalize(), output.canonicalize()) {
            (Ok(input), Ok(output)) => input == output,
            _ => false,
        }
}



#[cfg(unix)]
pub(crate) fn paths_have_same_file_identity(input: &Path, output: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    match (input.metadata(), output.metadata()) {
        (Ok(input), Ok(output)) => input.dev() == output.dev() && input.ino() == output.ino(),
        _ => false,
    }
}



#[cfg(not(unix))]
pub(crate) fn paths_have_same_file_identity(_input: &Path, _output: &Path) -> bool {
    false
}


pub(crate) fn print_hml_export_error(error: &rhwp::serializer::hml::HmlExportError) {
    eprintln!("오류: {error}");
    for blocker in error.blockers() {
        eprintln!(
            "  [{}] {}: {}",
            blocker.code, blocker.xml_path, blocker.message
        );
    }
}


pub(crate) fn export_hml(args: &[String]) {
    let paths = parse_hml_export_args(args).unwrap_or_else(|message| {
        eprintln!("{message}");
        process::exit(2);
    });
    if paths_refer_to_same_file(&paths.input, &paths.output) {
        eprintln!("오류: 입력과 출력 경로가 같습니다. 원본을 덮어쓰지 않습니다.");
        process::exit(2);
    }
    let data = fs::read(&paths.input).unwrap_or_else(|error| {
        eprintln!(
            "오류: 파일을 읽을 수 없습니다 - {}: {error}",
            paths.input.display()
        );
        process::exit(1);
    });
    let core = match load_document_core(&data) {
        Ok(c) => c,
        Err(e) => process::exit(e.report()),
    };
    let bytes = core.export_hml_native().unwrap_or_else(|error| {
        print_hml_export_error(&error);
        process::exit(1);
    });
    atomic_file::write_atomically(&paths.output, &bytes).unwrap_or_else(|error| {
        eprintln!("오류: 파일 저장 실패 - {}: {error}", paths.output.display());
        process::exit(1);
    });
    if paths.json {
        println!(
            "{}",
            provenance::marked(
                serde_json::json!({
                    "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                    "source": paths.input.display().to_string(),
                    "output": paths.output.display().to_string(),
                    "format": "hml",
                    "bytes": bytes.len(),
                }),
                "export-hml",
            )
        );
    } else {
        println!(
            "저장 완료: {} ({}KB)",
            paths.output.display(),
            bytes.len() / 1024
        );
    }
}



pub(crate) fn dump_raw_records(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("사용법: rhwp dump-records <파일.hwp>");
        return EXIT_USAGE;
    }
    let data = match fs::read(&args[0]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: {}", e);
            return EXIT_RUNTIME;
        }
    };
    use rhwp::parser::cfb_reader::CfbReader;
    use rhwp::parser::record::Record;
    let mut cfb = match CfbReader::open(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("오류: {:?}", e);
            return EXIT_RUNTIME;
        }
    };
    // 공통 파서와 같은 FileHeader 계약(플래그 + EncryptVersion)을 적용한다.
    let header = match cfb.read_file_header() {
        Ok(header) => header,
        Err(e) => {
            eprintln!("오류: FileHeader 읽기 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };
    let file_header = match rhwp::parser::header::parse_file_header(&header) {
        Ok(header) => header,
        Err(e) => {
            eprintln!("오류: FileHeader 파싱 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };
    let compressed = file_header.flags.compressed;
    let encrypted = file_header.flags.encrypted;
    if encrypted
        && file_header.encrypt_version != rhwp::parser::crypto::SUPPORTED_PASSWORD_ENCRYPT_VERSION
    {
        eprintln!(
            "오류: 지원하지 않는 암호화 방식 - EncryptVersion {} (지원: {})",
            file_header.encrypt_version,
            rhwp::parser::crypto::SUPPORTED_PASSWORD_ENCRYPT_VERSION
        );
        return EXIT_RUNTIME;
    }
    let section = if encrypted {
        // 비밀번호 암호 문서: raw 섹션을 읽어 복호화한다.
        let Some(pwd) = cli_password() else {
            eprintln!("오류: 비밀번호가 필요한 암호 문서입니다 (--password <pw> 로 전달).");
            return EXIT_USAGE;
        };
        let raw =
            match cfb.read_body_text_section_raw_limited(0, MAX_DUMP_RECORDS_STREAM_OUTPUT_BYTES) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("오류: {:?}", e);
                    return EXIT_RUNTIME;
                }
            };
        match rhwp::parser::crypto::decrypt_password_protected_limited(
            &raw,
            pwd.as_bytes(),
            compressed,
            MAX_DUMP_RECORDS_STREAM_OUTPUT_BYTES,
        ) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("오류: 비밀번호 불일치 또는 복호화 실패 - {}", e);
                return EXIT_RUNTIME;
            }
        }
    } else {
        match cfb.read_body_text_section_limited(
            0,
            compressed,
            MAX_DUMP_RECORDS_STREAM_OUTPUT_BYTES,
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("오류: {:?}", e);
                return EXIT_RUNTIME;
            }
        }
    };
    let records = match Record::read_all(&section) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("오류: {:?}", e);
            return EXIT_RUNTIME;
        }
    };
    let tag_name = |id: u16| -> &str {
        match id {
            66 => "PARA_HEADER",
            67 => "PARA_TEXT",
            68 => "PARA_CHAR_SHAPE",
            69 => "PARA_LINE_SEG",
            70 => "PARA_RANGE_TAG",
            71 => "CTRL_HEADER",
            72 => "LIST_HEADER",
            73 => "PAGE_DEF",
            74 => "FOOTNOTE_SHAPE",
            75 => "PAGE_BORDER_FILL",
            76 => "SHAPE_COMPONENT",
            77 => "TABLE",
            78 => "SC_LINE",
            79 => "SC_RECT",
            80 => "SC_ELLIPSE",
            81 => "SC_ARC",
            82 => "SC_POLYGON",
            83 => "SC_CURVE",
            85 => "SC_PICTURE",
            86 => "SC_CONTAINER",
            89 => "CTRL_DATA",
            _ => "?",
        }
    };
    for (i, rec) in records.iter().enumerate() {
        let indent = "  ".repeat(rec.level as usize);
        println!(
            "[{:3}] {}tag={:<3} {:16} lv={} sz={}",
            i,
            indent,
            rec.tag_id,
            tag_name(rec.tag_id),
            rec.level,
            rec.data.len()
        );
        // shape 관련 레코드만 hex 덤프
        if matches!(rec.tag_id, 71 | 72 | 76 | 79 | 85 | 89) {
            // 16바이트씩 나눠서 hex 출력
            for chunk in rec.data.chunks(16) {
                let hex: String = chunk
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("       {}  {}", indent, hex);
            }
        }
    }
    EXIT_OK
}



pub(crate) fn extract_thumbnail(args: &[String]) -> i32 {
    // [#3366] 계약 정합 — 파싱은 #3349 규약(위치 무관, 미지 플래그 즉시 exit 2,
    // 중복 positional exit 2), 종료 코드는 #2707(사용법 오류 = 2). 종전에는 알 수
    // 없는 옵션을 조용히 무시한 채 산출물까지 만들고, 인자 없음이 1 로 끝났다.
    let mut input_path: Option<&str> = None;
    let mut output_path: Option<String> = None;
    let mut mode = "file"; // "file", "base64", "data-uri"
                           // [#3600] --json: 봉투를 stdout 순수 JSON 으로. 추출 동작 무변경.
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(p) => output_path = Some(p.clone()),
                    None => {
                        eprintln!("오류: --output 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--base64" => mode = "base64",
            "--data-uri" => mode = "data-uri",
            "--json" => json_mode = true,
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if input_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }

    let Some(input_path) = input_path else {
        eprintln!("사용법: rhwp thumbnail <파일.hwp> [옵션]");
        eprintln!("  -o, --output <파일>   출력 파일 경로");
        eprintln!("  --base64              base64 문자열 출력");
        eprintln!("  --data-uri            data:image/... URI 출력");
        return EXIT_USAGE;
    };

    let data = match fs::read(input_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다: {} ({})", input_path, e);
            return EXIT_RUNTIME;
        }
    };

    let result = match rhwp::parser::extract_thumbnail_only(&data) {
        Some(r) => r,
        None => {
            eprintln!("오류: PrvImage 썸네일이 없습니다: {}", input_path);
            return EXIT_RUNTIME;
        }
    };

    let mime = match result.format.as_str() {
        "png" => "image/png",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    };

    // [#3600] JSON 봉투 공통부 — 모드별로 output/base64/dataUri 만 달라진다.
    let envelope_base = |extra: serde_json::Value| {
        let mut v = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": input_path,
            "format": result.format,
            "mime": mime,
            "width": result.width,
            "height": result.height,
            "bytes": result.data.len(),
            "output": serde_json::Value::Null,
        });
        if let (Some(obj), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            for (k, val) in e {
                obj.insert(k.clone(), val.clone());
            }
        }
        // [#3787 S1] base64/dataUri 는 문서에 내장된 미리보기 이미지다 — extra 를
        // 합친 **뒤에** 표지를 찍어야 그 모드의 봉투가 맞게 표시된다.
        provenance::marked(v, "thumbnail")
    };

    match mode {
        "base64" => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&result.data);
            if json_mode {
                println!("{}", envelope_base(serde_json::json!({ "base64": b64 })));
            } else {
                println!("{}", b64);
            }
        }
        "data-uri" => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&result.data);
            let uri = format!("data:{};base64,{}", mime, b64);
            if json_mode {
                println!("{}", envelope_base(serde_json::json!({ "dataUri": uri })));
            } else {
                println!("{}", uri);
            }
        }
        _ => {
            // 파일 출력
            let out = output_path.unwrap_or_else(|| {
                let stem = Path::new(input_path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                let ext = &result.format;
                format!("output/{}_thumb.{}", stem, ext)
            });

            // 출력 디렉토리 생성
            if let Some(parent) = Path::new(&out).parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent).ok();
                }
            }

            match fs::write(&out, &result.data) {
                Ok(_) => {
                    if json_mode {
                        println!("{}", envelope_base(serde_json::json!({ "output": out })));
                    } else {
                        println!(
                            "썸네일 추출 완료: {} ({}x{}, {} bytes, {})",
                            out,
                            result.width,
                            result.height,
                            result.data.len(),
                            result.format
                        );
                    }
                }
                Err(e) => {
                    eprintln!("오류: 파일 저장 실패: {} ({})", out, e);
                    return EXIT_RUNTIME;
                }
            }
        }
    }
    EXIT_OK
}
