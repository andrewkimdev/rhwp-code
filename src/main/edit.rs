//! edit 모듈 — src/main.rs 에서 무변동 이동
use super::*;

/// [#3346] `fields --json` 과 `batch fields` 가 공유하는 필드 레코드 수집.
///
/// 단건/배치가 같은 스키마를 내도록 한 곳에서 만든다.
pub(crate) fn collect_field_records(doc: &rhwp::wasm_api::HwpDocument) -> Vec<serde_json::Value> {
    use rhwp::document_core::queries::field_query::NestedEntry;

    doc.collect_all_fields()
        .iter()
        .map(|fi| {
            // 중첩 경로: 표 셀·글상자 안의 필드가 어디에 있는지 — 후속 편집의 좌표다.
            let nested: Vec<serde_json::Value> = fi
                .location
                .nested_path
                .iter()
                .map(|e| match e {
                    NestedEntry::TableCell {
                        control_index,
                        cell_index,
                        para_index,
                    } => serde_json::json!({
                        "kind": "tableCell",
                        "control": control_index,
                        "cell": cell_index,
                        "paragraph": para_index,
                    }),
                    NestedEntry::TextBox {
                        control_index,
                        para_index,
                    } => serde_json::json!({
                        "kind": "textBox",
                        "control": control_index,
                        "paragraph": para_index,
                    }),
                })
                .collect();

            serde_json::json!({
                "fieldId": fi.field.field_id,
                "fieldType": format!("{:?}", fi.field.field_type),
                "name": fi.field.field_name().unwrap_or(""),
                "guide": fi.field.guide_text().unwrap_or(""),
                "memo": fi.field.memo_text().unwrap_or_default(),
                "command": fi.field.command,
                "value": fi.value,
                "editableInForm": fi.field.is_editable_in_form(),
                "location": {
                    "section": fi.location.section_index,
                    "paragraph": fi.location.para_index,
                    "nested": nested,
                },
            })
        })
        .collect()
}



/// `fields` — 누름틀/필드 조사 (읽기 전용).
///
/// `edit` — 문서 편집 명령군 (로드맵 #2659 Stage 3).
///
/// 공통 규약: `--dry-run`(변경 요약만 출력, 파일 무변경), 결과 리포트 JSON,
/// **실패 시 원본 불변**(하나라도 실패하면 출력 파일을 쓰지 않는다).
pub(crate) fn run_edit(args: &[String]) -> i32 {
    const USAGE: &str =
        "사용법: rhwp edit <fill-fields|replace-text|set-cell|set-table-props|move-table|transpose-table|set-column-widths|insert-image|redact|sanitize> <파일.hwp|파일.hwpx> [옵션] (rhwp --help 참조)";

    match args.first().map(String::as_str) {
        Some("fill-fields") => edit_fill_fields(&args[1..]),
        Some("replace-text") => edit_replace_text(&args[1..]),
        Some("set-cell") => edit_set_cell(&args[1..]),
        // [upstream #5185/#5192 계열 선별 이식] 표 단위 편집 4종 — 코어는 기존
        // table_ops.rs 네이티브 함수 재사용, CLI 배선만 신규.
        Some("set-table-props") => edit_set_table_props(&args[1..]),
        Some("move-table") => edit_move_table(&args[1..]),
        Some("transpose-table") => edit_transpose_table(&args[1..]),
        Some("set-column-widths") => edit_set_column_widths(&args[1..]),
        Some("insert-image") => edit_insert_image(&args[1..]),
        // [#3719 §6-11] 공개 전 정리 — 개인정보 마스킹 / 메타데이터 제거.
        Some("redact") => edit_redact(&args[1..]),
        Some("sanitize") => edit_sanitize(&args[1..]),
        Some(other) => {
            eprintln!("오류: 알 수 없는 edit 하위 명령 - {}", other);
            eprintln!("{USAGE}");
            EXIT_USAGE
        }
        None => {
            eprintln!("오류: edit 하위 명령을 지정해주세요.");
            eprintln!("{USAGE}");
            EXIT_USAGE
        }
    }
}



/// [#3476] `--data` 키를 `(이름, 순번)` 으로 나눈다.
///
/// `"피규제집단명[3]"` → `("피규제집단명", 3)`, `"제목명"` → `("제목명", 0)`.
/// 실제 제출 서식은 같은 이름을 여러 번 쓰므로(규제 대상 집단 14개 등) 순번으로 지목한다.
/// 순번은 `fields --json` 이 주는 문서 순서와 같다.
pub(crate) fn parse_field_key(key: &str) -> (&str, usize) {
    let Some(open) = key.rfind('[') else {
        return (key, 0);
    };
    if !key.ends_with(']') {
        return (key, 0);
    }
    let inner = &key[open + 1..key.len() - 1];
    match inner.parse::<usize>() {
        Ok(n) => (&key[..open], n),
        // 색인으로 해석되지 않으면 이름의 일부로 둔다 — 대괄호가 든 이름을 깨뜨리지 않는다.
        Err(_) => (key, 0),
    }
}



/// 입력 형식과 사용자가 지정한 `-o` 경로로 `edit` 산출 형식을 정한다 (#3383).
///
/// 기본은 **입력 형식 보존**이다 — HWPX 입력은 HWPX 로, 그 외(HWP5/HWP3)는 HWP5 로.
/// 예외는 하나뿐이다: HWPX 입력에 사용자가 `-o ….hwp` 를 명시한 경우. 이때는 지정한
/// **경로를 그대로 존중해** HWP5 로 저장하되(기존 스크립트 호환), 형식이 바뀐다는 사실과
/// 손실 가능성을 stderr 로 알린다(이슈 제안 2의 과도기 경고).
///
/// 반대 방향(HWP 입력에 `-o ….hwpx`)은 `edit` 의 책임이 아니다 — 형식 변환은
/// `rhwp export-hwpx` 가 담당한다. 여기서는 경고만 하고 형식을 바꾸지 않는다.
pub(crate) fn edit_output_format(input_bytes: &[u8], explicit_out: Option<&str>) -> EditOutputFormat {
    let source_is_hwpx = matches!(
        rhwp::parser::detect_format(input_bytes),
        rhwp::parser::FileFormat::Hwpx
    );
    let explicit_ext = explicit_out.and_then(|path| {
        Path::new(path)
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
    });

    match (source_is_hwpx, explicit_ext.as_deref()) {
        (true, Some("hwp")) => {
            eprintln!(
                "경고: 입력은 HWPX 인데 출력 확장자가 .hwp 라 HWP5 로 저장합니다 — \
                 형식 변환 과정에서 차트·이미지 등이 유실될 수 있습니다 \
                 (형식을 보존하려면 -o 를 생략하거나 .hwpx 로 지정하세요)."
            );
            EditOutputFormat::Hwp
        }
        (true, _) => EditOutputFormat::Hwpx,
        (false, Some("hwpx")) => {
            eprintln!(
                "경고: 입력이 HWPX 가 아니므로 HWP5 로 저장합니다 — 지정한 출력 확장자(.hwpx)와 \
                 실제 형식이 다릅니다 (HWPX 로 변환하려면 `rhwp export-hwpx` 를 쓰세요)."
            );
            EditOutputFormat::Hwp
        }
        (false, _) => EditOutputFormat::Hwp,
    }
}



/// 결정된 형식으로 편집 결과를 직렬화한다 (#3383).
///
/// HWP5 산출은 반드시 **어댑터 경유**(`export_hwp_with_adapter`)다. HWPX 출처 IR 을 HWP
/// 호환 형태로 옮기는 #178 어댑터를 건너뛰면 한컴 호환성과 이미지·차트가 깨진다.
/// [#3702] 편집 저장본 자기검증 — 편집 후 IR 과 저장본 재파싱 IR 을 내부 대조한다.
/// 반환: (verify 봉투 값, exit 3 여부). 비교기는 diff_documents 재사용(신규 로직 없음).
/// HWPX 소스→HWP5 산출은 #3505/#3930 출처 전용 노이즈 제거를 승계한다.
pub(crate) fn edit_verify_report(
    doc: &rhwp::wasm_api::HwpDocument,
    out_bytes: &[u8],
    source_is_hwpx: bool,
) -> (serde_json::Value, bool) {
    let reloaded = match rhwp::wasm_api::HwpDocument::from_bytes(out_bytes) {
        Ok(d) => d,
        Err(e) => {
            // 재파싱 실패는 판정 불가 — identical:false + 사유로 보고(저장물은 남는다).
            return (
                serde_json::json!({ "identical": false, "diffCount": null, "reparseError": e.to_string() }),
                true,
            );
        }
    };
    let diff =
        rhwp::serializer::hwpx::roundtrip::diff_documents(doc.document(), reloaded.document());
    let diff = if source_is_hwpx {
        rhwp::serializer::hwpx::roundtrip::strip_hwpx_to_hwp_noise(diff)
    } else {
        diff
    };
    if diff.is_empty() {
        (
            serde_json::json!({ "identical": true, "diffCount": 0 }),
            false,
        )
    } else {
        (
            serde_json::json!({ "identical": false, "diffCount": diff.differences.len() }),
            true,
        )
    }
}


pub(crate) fn edit_serialize(
    doc: &mut rhwp::wasm_api::HwpDocument,
    format: EditOutputFormat,
) -> Result<Vec<u8>, String> {
    match format {
        EditOutputFormat::Hwpx => doc.export_hwpx_native(),
        EditOutputFormat::Hwp => doc.export_hwp_with_adapter(),
    }
    .map_err(|e| e.to_string())
}


// ─── [#3703] 계획 실행기 — 명령(CLI)·도구(MCP) 위의 3층: 선언적 편집 계획 ───



/// [#3721] dry-run 미리보기 한 줄 — 사람 모드에서 "무엇이 얼마나 바뀌나"를 읽게 한다.
pub(crate) fn preview_line(step: &serde_json::Value) -> String {
    let idx = step["step"].as_u64().unwrap_or(0);
    // [#3719 §6-8] 건너뛸 step 은 다른 필드가 비어 있으므로 액션별 분기보다 먼저 본다.
    if step["skipped"] == true {
        return format!(
            "step {} 건너뜀 예정: {}",
            idx,
            step["reason"].as_str().unwrap_or("")
        );
    }
    match step["action"].as_str().unwrap_or("") {
        "fill_fields" => format!(
            "step {}: 누름틀 {}칸 채움",
            idx,
            step["targets"].as_array().map(|a| a.len()).unwrap_or(0)
        ),
        "replace_text" => format!(
            "step {}: '{}' {}건 중 {}건 치환",
            idx,
            step["find"].as_str().unwrap_or(""),
            step["matches"].as_u64().unwrap_or(0),
            step["willReplace"].as_u64().unwrap_or(0)
        ),
        "set_checkbox" => format!(
            "step {}: 빈 체크박스 {}개 중 {}번째 표시",
            idx,
            step["available"].as_u64().unwrap_or(0),
            step["occurrence"].as_u64().unwrap_or(0)
        ),
        "set_cell" => format!(
            "step {}: 표 {} ({},{}) 기록 — 현재값 {:?}",
            idx,
            step["table"].as_u64().unwrap_or(0),
            step["row"].as_u64().unwrap_or(0),
            step["col"].as_u64().unwrap_or(0),
            step["currentText"].as_str().unwrap_or("")
        ),
        other => format!("step {}: {}", idx, other),
    }
}



/// `edit_serialize` 와 같은 바이트를 내되 **IR 을 건드리지 않는다**.
///
/// 무상태 CLI 는 저장 직후 프로세스가 끝나므로 어댑터가 살아 있는 IR 을 정규화해도
/// 관측되지 않는다. 세션 핸들은 다르다 — 도구 계약이 "핸들은 저장 후에도 열려 있다"
/// 이므로 저장은 스냅숏이어야 한다. 그래서 세션 경로만 이쪽을 쓰고 CLI 의 `&mut`
/// 경로는 그대로 둔다(CLI 에 문서 1회 clone 비용을 지우지 않는다).
pub(crate) fn edit_serialize_snapshot(
    doc: &rhwp::wasm_api::HwpDocument,
    format: EditOutputFormat,
) -> Result<Vec<u8>, String> {
    match format {
        EditOutputFormat::Hwpx => doc.export_hwpx_native(),
        EditOutputFormat::Hwp => doc.export_hwp_with_adapter_snapshot(),
    }
    .map_err(|e| e.to_string())
}



/// `edit fill-fields` — 누름틀에 값을 채운다 (메일머지).
///
/// 검증된 코어 경로(`set_field_value_by_name`)를 재사용하므로 새 편집 로직이 없다.
/// 필드 값만 바꾸므로 레이아웃·구조는 불변이다.
pub(crate) fn edit_fill_fields(args: &[String]) -> i32 {
    let mut file_path: Option<&str> = None;
    let mut data_arg: Option<&str> = None;
    let mut out_path: Option<String> = None;
    let mut dry_run = false;
    let mut json_mode = false;
    // [#3702] 저장 직후 자기검증 — 판정은 데이터, 차이 시 exit 3.
    let mut verify_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--data" => {
                i += 1;
                match args.get(i) {
                    Some(v) => data_arg = Some(v),
                    None => {
                        eprintln!("오류: --data 뒤에 JSON 또는 @파일경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => file_path = Some(other),
        }
        i += 1;
    }

    let (Some(file_path), Some(data_arg)) = (file_path, data_arg) else {
        eprintln!("사용법: rhwp edit fill-fields <파일.hwp|파일.hwpx> --data <JSON|@파일> [-o <출력>] [--dry-run] [--json]");
        return EXIT_USAGE;
    };

    // `@경로` 면 파일에서 읽는다 — 대량 메일머지에서 셸 인용 지옥을 피한다.
    let data_text = if let Some(path) = data_arg.strip_prefix('@') {
        match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("오류: --data 파일을 읽을 수 없습니다 - {}: {}", path, e);
                return EXIT_RUNTIME;
            }
        }
    } else {
        data_arg.to_string()
    };

    let data: serde_json::Map<String, serde_json::Value> =
        match serde_json::from_str::<serde_json::Value>(&data_text) {
            Ok(serde_json::Value::Object(m)) => m,
            Ok(_) => {
                eprintln!("오류: --data 는 {{\"필드이름\":\"값\"}} 형식의 JSON 객체여야 합니다.");
                return EXIT_USAGE;
            }
            Err(e) => {
                eprintln!("오류: --data JSON 파싱 실패 - {}", e);
                return EXIT_USAGE;
            }
        };

    let outcome = match fill_fields_core(file_path, &data, out_path, dry_run, verify_mode) {
        Ok(o) => o,
        Err(message) => {
            eprintln!("오류: {message}");
            return EXIT_RUNTIME;
        }
    };
    let FillOutcome {
        envelope,
        output_path,
        verify_failed,
        ..
    } = outcome;

    if json_mode {
        println!("{envelope}");
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    let empty: Vec<serde_json::Value> = Vec::new();
    let filled = envelope["filled"].as_array().unwrap_or(&empty);
    let not_found: Vec<&str> = envelope["notFound"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let confusable = envelope["confusable"].as_array().unwrap_or(&empty);

    if dry_run {
        println!("변경 예정: {} (필드 {}개)", file_path, filled.len());
    } else {
        println!(
            "채우기 완료: {} → {} (필드 {}개)",
            file_path,
            output_path,
            filled.len()
        );
    }
    for f in filled {
        println!(
            "  {} = {:?}",
            f["name"].as_str().unwrap_or(""),
            f["value"].as_str().unwrap_or("")
        );
    }
    if !not_found.is_empty() {
        println!("  문서에 없는 필드 이름: {}", not_found.join(", "));
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    for c in confusable {
        // 사람에게도 알린다 — 화면상 같은 이름이라 눈으로는 잡을 수 없는 축이다.
        eprintln!(
            "경고: '{}' 과(와) 화면상 구별되지 않는 이름의 누름틀이 문서에 함께 있습니다 \
             — 채운 칸이 의도한 칸인지 확인하세요.",
            c["name"].as_str().unwrap_or("")
        );
    }
    EXIT_OK
}



/// [#3719 §6-6] 누름틀 채움의 **단 하나의** 구현. 단건 CLI 도 배치도 이 함수만 부른다.
///
/// 배치를 위해 새 편집 로직을 쓰지 않는다 — 채움 규칙(순번 지목·모호성 보고·혼동 이름
/// 경고·형식 보존·저장 직후 자기검증·changedPages)이 두 곳으로 갈라지면 단건으로 검증한
/// 서식이 배치에서 다르게 채워지고, 그 차이는 산출물 N개가 나온 뒤에야 드러난다.
///
/// 실패는 `Err(사람이 읽는 사유)` 다. 단건은 stderr + exit 1 로, 배치는 그 행의 `error`
/// 레코드로 바꾼다 — 프로세스를 끊지 않는 이유는 뒤 행이 남아 있기 때문이다.
pub(crate) fn fill_fields_core(
    file_path: &str,
    data: &serde_json::Map<String, serde_json::Value>,
    out_path: Option<String>,
    dry_run: bool,
    verify_mode: bool,
) -> Result<FillOutcome, String> {
    let bytes = fs::read(file_path)
        .map_err(|e| format!("파일을 읽을 수 없습니다 - {}: {}", file_path, e))?;
    let mut doc = load_document(&bytes).map_err(|e| match e {
        LoadError::NeedPassword => {
            "비밀번호가 필요한 암호 문서입니다 (--password <pw> 로 전달)".to_string()
        }
        LoadError::WrongPassword => {
            "비밀번호가 일치하지 않거나 암호화 데이터가 손상되었습니다".to_string()
        }
        LoadError::Other(msg) => format!("HWP 파싱 실패 - {}", msg),
    })?;

    // [#3476] 이름별 **개수**를 센다. 실제 제출 서식은 같은 항목 묶음을 여러 번 요구해
    // (규제영향분석서의 `피규제집단명` ×14 등) 이름만으로는 하나만 지목된다.
    let mut name_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    // [#3712] 같은 순회에서 문단 주소도 담는다 — changedPages 산출 근거.
    let mut name_locs: std::collections::HashMap<String, Vec<(usize, usize)>> =
        std::collections::HashMap::new();
    for fi in doc.collect_all_fields().iter() {
        if let Some(n) = fi.field.field_name() {
            *name_counts.entry(n.to_string()).or_insert(0) += 1;
            name_locs
                .entry(n.to_string())
                .or_default()
                .push((fi.location.section_index, fi.location.para_index));
        }
    }
    let mut changed_paras: Vec<(usize, usize)> = Vec::new();

    let mut filled: Vec<serde_json::Value> = Vec::new();
    let mut not_found: Vec<String> = Vec::new();
    // 이름만 준 키가 여러 곳에 해당하면 그 사실을 보고한다 — 침묵하면 소비자가
    // 불완전한 산출물을 완성본으로 판단한다.
    let mut ambiguous: Vec<serde_json::Value> = Vec::new();
    // [#3707] 바이트가 달라 위 개수 판정을 통과하지만 **화면상 구별되지 않는** 이름
    // 쌍은 별도 축이다. 지목한 이름에 그런 쌍둥이가 있으면 채우되 반드시 보고한다 —
    // 침묵하면 "엉뚱한 칸을 채우고 완벽한 성공을 보고"하는 상태가 된다.
    let all_names: Vec<String> = name_counts.keys().cloned().collect();
    let confusable_groups = rhwp::document_core::text_security::confusable_collisions(&all_names);
    let mut confusable: Vec<serde_json::Value> = Vec::new();

    for (key, value) in data {
        let value_str = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let (name, occurrence) = parse_field_key(key);
        let total = name_counts.get(name).copied().unwrap_or(0);

        // 이름이 없거나, 지정한 순번이 범위를 벗어나면 채우지 않고 보고한다.
        if total == 0 || occurrence >= total {
            not_found.push(key.clone());
            continue;
        }
        if occurrence == 0 && total > 1 && !key.contains('[') {
            ambiguous.push(serde_json::json!({
                "name": name,
                "matched": 1,
                "total": total,
            }));
        }
        if let Some((_, group)) = confusable_groups
            .iter()
            .find(|(_, g)| g.iter().any(|n| n == name))
        {
            let others: Vec<&String> = group.iter().filter(|n| *n != name).collect();
            confusable.push(serde_json::json!({
                "name": name,
                "lookalikes": others,
                "note": "화면상 구별되지 않는 이름의 누름틀이 이 문서에 함께 있습니다 — 채운 칸이 의도한 칸인지 확인하세요.",
            }));
        }

        if dry_run {
            // 파일을 건드리지 않고 무엇이 바뀔지만 기록한다.
            filled.push(
                serde_json::json!({ "name": name, "occurrence": occurrence, "value": value_str }),
            );
            continue;
        }
        // 실패 시 원본 불변 — 출력 파일을 쓰지 않고 즉시 끝낸다.
        doc.set_field_value_by_name_at(name, occurrence, &value_str)
            .map_err(|e| format!("필드 '{}' 설정 실패 - {}", key, e))?;
        if let Some(loc) = name_locs.get(name).and_then(|l| l.get(occurrence)) {
            changed_paras.push(*loc);
        }
        filled.push(
            serde_json::json!({ "name": name, "occurrence": occurrence, "value": value_str }),
        );
    }

    // [#3383] 입력 형식을 보존한다 — 기본 확장자도 산출 형식을 따른다.
    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        // [#3469] 기본 산출물은 **입력 파일 옆**에 만든다. 종전에는 파일명만 써서
        // 현재 작업 디렉터리에 떨어졌는데, 임의 경로의 문서를 다루는 에이전트·MCP
        // 클라이언트에게는 산출물이 엉뚱한 곳에 생기는 셈이었다.
        let input = Path::new(file_path);
        let stem = input
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        let name = format!("{}_filled.{}", stem, out_format.ext());
        match input.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => {
                dir.join(name).to_string_lossy().to_string()
            }
            _ => name,
        }
    });

    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if !dry_run {
        let out_bytes = edit_serialize(&mut doc, out_format)
            .map_err(|e| format!("{} 직렬화 실패 - {}", out_format.label().to_uppercase(), e))?;
        fs::write(&output_path, &out_bytes)
            .map_err(|e| format!("출력 쓰기 실패 - {}: {}", output_path, e))?;
        // [#3702] 저장 직후 자기검증 — 편집 후 IR ↔ 저장본 재파싱 IR.
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    // [#3712] 눈검증 대상 페이지 — 편집 반영 후 조판 기준. 확정 불가면 null(부분 목록 금지).
    let changed_pages = if dry_run {
        serde_json::Value::Null
    } else {
        match doc.pages_covering_paragraphs(&changed_paras) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    };

    let mut envelope = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "source": file_path,
        "dryRun": dry_run,
        "changedPages": changed_pages,
        "filledCount": filled.len(),
        "filled": filled,
        "notFound": not_found,
        "ambiguous": ambiguous,
        "confusable": confusable,
    });
    if !dry_run {
        envelope["output"] = serde_json::Value::String(output_path.clone());
        envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
        envelope["verify"] = verify_report;
    }

    Ok(FillOutcome {
        envelope: provenance::marked(envelope, "edit"),
        output_path,
        output_format: out_format,
        verify_failed,
    })
}



/// `edit replace-text` — 문서 전체 일괄 치환 (기관명 변경·연도 갱신·용어 정비).
///
/// [#3373] 검증된 코어 경로(`replace_all` — 역순 치환으로 오프셋 안전, 본문+표 셀)를
/// 재사용하므로 새 편집 로직이 없다. `--dry-run` 은 파일 생성 경로를 타지 않고
/// 읽기 전용 `grep` 으로 치환 예정 건수만 보고한다. **0건이면 출력 파일을 만들지
/// 않는다** — 무변경 산출물이 생기지 않게 한다.
pub(crate) fn edit_replace_text(args: &[String]) -> i32 {
    let mut file_path: Option<&str> = None;
    let mut find_arg: Option<&str> = None;
    let mut replace_arg: Option<&str> = None;
    let mut out_path: Option<String> = None;
    let mut ignore_case = false;
    let mut dry_run = false;
    let mut json_mode = false;
    // [#3702] 저장 직후 자기검증 — 판정은 데이터, 차이 시 exit 3.
    let mut verify_mode = false;
    // [#4378 R24] CAS — 입력이 이 해시일 때만 진행(다른 에이전트의 선행 편집 감지).
    let mut expect_sha256: Option<String> = None;
    // [#3395] 문서 순서 k번째(0 기준) 매치만 치환 — 체크박스류 반복 문자 지목.
    let mut occurrence: Option<usize> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--find" => {
                i += 1;
                match args.get(i) {
                    Some(v) => find_arg = Some(v),
                    None => {
                        eprintln!("오류: --find 뒤에 찾을 문자열이 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--replace" => {
                i += 1;
                match args.get(i) {
                    Some(v) => replace_arg = Some(v),
                    None => {
                        eprintln!("오류: --replace 뒤에 바꿀 문자열이 필요합니다 (삭제는 \"\").");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--ignore-case" => ignore_case = true,
            "--occurrence" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(n) => occurrence = Some(n),
                    None => {
                        eprintln!("오류: --occurrence 뒤에 0 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
            "--expect-sha256" => {
                i += 1;
                match args.get(i) {
                    Some(v) => expect_sha256 = Some(v.clone()),
                    None => {
                        eprintln!("오류: --expect-sha256 뒤에 64자리 16진 해시가 필요합니다.");
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

    let (Some(file_path), Some(find), Some(replace)) = (file_path, find_arg, replace_arg) else {
        eprintln!(
            "사용법: rhwp edit replace-text <파일.hwp|파일.hwpx> --find <문자열> --replace <문자열> [-o <출력>] [--ignore-case] [--dry-run] [--json]"
        );
        return EXIT_USAGE;
    };
    if find.is_empty() {
        eprintln!("오류: --find 는 빈 문자열일 수 없습니다.");
        return EXIT_USAGE;
    }

    let _cas_lock = match expect_sha256.as_ref() {
        Some(_) => {
            if let Err(e) = cas_test_synchronize_before_lock() {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
            match CasPathLock::acquire(Path::new(file_path)) {
                Ok(lock) => Some(lock),
                Err(e) => {
                    eprintln!("오류: 입력 문서 CAS 잠금을 얻을 수 없습니다 - {file_path}: {e}");
                    return EXIT_RUNTIME;
                }
            }
        }
        None => None,
    };
    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    // [#4378 R24] 파싱 전에 CAS 대조 — 기대 상태가 아니면 여기서 끝(디스크 무변경).
    if let Some(code) = check_expect_sha256(expect_sha256.as_deref(), &bytes, file_path, json_mode)
    {
        return code;
    }
    if expect_sha256.is_some() {
        cas_test_mark_checked_and_wait();
    }
    let mut doc = match load_document(&bytes) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    // [#3712] 치환 전 매치 주소를 붙잡는다 — 문자열 치환은 문단 인덱스를 밀지 않는다.
    let changed_paras: Vec<(usize, usize)> = if dry_run {
        Vec::new()
    } else {
        let all = doc.grep(find, !ignore_case, None);
        match occurrence {
            Some(n) => all
                .get(n)
                .map(|m| vec![(m.section, m.paragraph)])
                .unwrap_or_default(),
            None => all.iter().map(|m| (m.section, m.paragraph)).collect(),
        }
    };

    let replaced_count = if dry_run {
        // 파일을 건드리지 않는다 — 읽기 전용 검색으로 치환 예정 건수만 센다.
        match occurrence {
            // dry-run + occurrence: 그 순번이 존재하면 1, 아니면 0.
            Some(n) => usize::from(doc.grep(find, !ignore_case, None).len() > n),
            None => doc.grep(find, !ignore_case, None).len(),
        }
    } else {
        let result = match match occurrence {
            Some(n) => doc.replace_nth_native(find, replace, !ignore_case, n),
            None => doc.replace_all_native(find, replace, !ignore_case),
        } {
            Ok(r) => r,
            Err(e) => {
                eprintln!("오류: 치환 실패 - {:?}", e);
                // 실패 시 원본 불변 — 출력 파일을 쓰지 않고 즉시 끝낸다.
                return EXIT_RUNTIME;
            }
        };
        serde_json::from_str::<serde_json::Value>(&result)
            .ok()
            .and_then(|v| v["count"].as_u64())
            .unwrap_or(0) as usize
    };

    // [#3383] 입력 형식을 보존한다 — 기본 확장자도 산출 형식을 따른다.
    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_replaced.{}", stem, out_format.ext())
    });

    // 0건이면 무변경이다 — 산출물을 만들지 않는다 (dry-run 과 동일하게 파일 경로를 타지 않음).
    let wrote_output = !dry_run && replaced_count > 0;
    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if wrote_output {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if expect_sha256.is_some() {
            let latest = match fs::read(file_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("오류: 저장 직전 입력을 다시 읽을 수 없습니다 - {file_path}: {e}");
                    return EXIT_RUNTIME;
                }
            };
            if let Some(code) =
                check_expect_sha256(expect_sha256.as_deref(), &latest, file_path, json_mode)
            {
                return code;
            }
        }
        if let Err(e) = fs::write(&output_path, &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        // [#3702] 저장 직후 자기검증 — 편집 후 IR ↔ 저장본 재파싱 IR.
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    // [#3712] 눈검증 대상 페이지 — 산출물이 있을 때만 의미가 있다(무산출은 null).
    let changed_pages = if wrote_output {
        match doc.pages_covering_paragraphs(&changed_paras) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    } else {
        serde_json::Value::Null
    };

    if json_mode {
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "find": find,
            "replace": replace,
            "occurrence": occurrence,
            "caseSensitive": !ignore_case,
            "dryRun": dry_run,
            "changedPages": changed_pages,
            "replacedCount": replaced_count,
        });
        if wrote_output {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "변경 예정: {} — {:?} → {:?} ({}건)",
            file_path, find, replace, replaced_count
        );
    } else if replaced_count == 0 {
        println!(
            "치환 0건: {} — {:?} 없음 (출력 파일 미생성)",
            file_path, find
        );
    } else {
        println!(
            "치환 완료: {} → {} — {:?} → {:?} ({}건)",
            file_path, output_path, find, replace, replaced_count
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}

// ─── [#3719 §6-11] 공개 전 정리 — edit redact / edit sanitize ───



/// `edit redact` — 개인정보를 찾아 자릿수를 유지한 채 마스킹한다.
///
/// 탐지는 [`rhwp::document_core::queries::pii_scan`] 의 읽기 전용 판정을 쓰고, 실제
/// 변경은 검증된 치환 경로(`replace_all_native`)를 재사용한다 — 새 편집 로직이 없다.
/// 되돌릴 수 없는 작업이라 ① `--dry-run` 이 권장 흐름이고 ② 산출 경로를 명시하지
/// 않으면 exit 2 로 거부한다.
pub(crate) fn edit_redact(args: &[String]) -> i32 {
    use rhwp::document_core::queries::pii_scan::PiiKind;

    let mut file_path: Option<&str> = None;
    let mut out_path: Option<String> = None;
    let mut kinds: Vec<PiiKind> = Vec::new();
    let mut mask_char: char = '*';
    let mut dry_run = false;
    let mut json_mode = false;
    let mut verify_mode = false;
    let mut in_place = false;
    let mut no_raw = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--kind" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    eprintln!("오류: --kind 뒤에 ssn|phone|email|card|all 이 필요합니다.");
                    return EXIT_USAGE;
                };
                for token in value.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                    if token == "all" {
                        kinds.extend(PiiKind::all());
                        continue;
                    }
                    match PiiKind::parse(token) {
                        Some(k) => kinds.push(k),
                        None => {
                            eprintln!(
                                "오류: 알 수 없는 --kind 값 - {token} (ssn|phone|email|card|all)"
                            );
                            return EXIT_USAGE;
                        }
                    }
                }
            }
            "--mask" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    eprintln!("오류: --mask 뒤에 마스킹 문자 한 글자가 필요합니다.");
                    return EXIT_USAGE;
                };
                let mut chars = value.chars();
                match (chars.next(), chars.next()) {
                    // 두 글자 이상이면 자릿수 보존이 깨진다 — 조용히 자르지 않고 거부한다.
                    (Some(c), None) if !c.is_alphanumeric() => mask_char = c,
                    (Some(_), None) => {
                        eprintln!("오류: --mask 는 영숫자가 아닌 문자여야 합니다 (예: * # ●).");
                        return EXIT_USAGE;
                    }
                    _ => {
                        eprintln!("오류: --mask 는 정확히 한 글자여야 합니다 (자릿수 보존).");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--in-place" => in_place = true,
            "--dry-run" => dry_run = true,
            "--verify" => verify_mode = true,
            "--json" => json_mode = true,
            "--no-raw" => no_raw = true,
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
            "사용법: rhwp edit redact <파일.hwp|파일.hwpx> [--kind ssn|phone|email|card|all] [--mask <문자>] [--dry-run] [--no-raw] [--verify] [-o <출력>|--in-place] [--json]"
        );
        return EXIT_USAGE;
    };
    if kinds.is_empty() {
        kinds.extend(PiiKind::all());
    }
    kinds.sort_unstable();
    kinds.dedup();

    if out_path.is_some() && in_place {
        eprintln!("오류: -o 와 --in-place 는 함께 쓸 수 없습니다 (산출 경로가 모호합니다).");
        return EXIT_USAGE;
    }
    // 원본 보호 — 산출 경로가 없는 실제 실행은 거부한다(--dry-run 은 아무것도 쓰지 않음).
    if !dry_run && out_path.is_none() && !in_place {
        eprintln!("{REDACT_DESTINATION_REQUIRED}");
        return EXIT_USAGE;
    }
    // -o 로 원본을 지목한 경우도 같은 사고다 — 의도를 --in-place 로 말하게 한다.
    if let Some(out) = out_path.as_deref() {
        if !in_place && same_existing_path(file_path, out) {
            eprintln!("{REDACT_DESTINATION_REQUIRED}");
            return EXIT_USAGE;
        }
    }

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match rhwp::wasm_api::HwpDocument::from_bytes(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: HWP 파싱 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    let findings = doc.scan_pii(&kinds, mask_char);
    let changed_paras: Vec<(usize, usize)> = {
        let mut v: Vec<(usize, usize)> =
            findings.iter().map(|f| (f.section, f.paragraph)).collect();
        v.sort_unstable();
        v.dedup();
        v
    };

    // 치환은 값 단위 전량이다. 긴 값을 먼저 바꿔야 짧은 값이 긴 값의 부분열일 때
    // 원문을 깨뜨리지 않는다.
    let mut targets: Vec<(String, String)> = Vec::new();
    for f in &findings {
        if !targets.iter().any(|(raw, _)| *raw == f.raw) {
            targets.push((f.raw.clone(), f.masked.clone()));
        }
    }
    targets.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));

    let mut redacted_count = 0usize;
    if !dry_run {
        for (raw, masked) in &targets {
            match doc.replace_all_native(raw, masked, true) {
                Ok(result) => {
                    redacted_count += serde_json::from_str::<serde_json::Value>(&result)
                        .ok()
                        .and_then(|v| v["count"].as_u64())
                        .unwrap_or(0) as usize;
                }
                Err(e) => {
                    // 실패 시 원본 불변 — 출력 파일을 쓰지 않고 즉시 끝낸다.
                    eprintln!("오류: 마스킹 실패 - {:?}", e);
                    return EXIT_RUNTIME;
                }
            }
        }
    }

    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = match (&out_path, in_place) {
        (Some(p), _) => p.clone(),
        (None, true) => file_path.to_string(),
        // 여기 도달하려면 dry-run 이다 — 산출 경로를 쓰지 않는다.
        (None, false) => String::new(),
    };

    // 탐지 0건이면 무변경이다 — 산출물을 만들지 않는다(원본을 그대로 두는 편이 안전하다).
    let wrote_output = !dry_run && redacted_count > 0;
    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if wrote_output {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if let Err(e) = atomic_file::write_atomically(Path::new(&output_path), &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    let changed_pages = if wrote_output {
        match doc.pages_covering_paragraphs(&changed_paras) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    } else {
        serde_json::Value::Null
    };

    if json_mode {
        // --no-raw: findings[].raw(원문 개인정보)를 봉투에서 아예 뺀다. `null`로 채우지
        // 않는 이유 — 이 코드베이스는 "선택적으로 없을 수 있는 필드"를 스키마 차원에서
        // 생략으로 표현한다(PiiFinding.page 의 skip_serializing_if 가 같은 관례). raw 를
        // null 로 두면 소비자가 "탐지는 됐지만 값이 비었다"와 "일부러 뺐다"를 구분할
        // 근거가 없어지고, jq 같은 파이프라인에서 null 이 그대로 로그에 찍혀 새 유출
        // 경로가 될 수 있다. 필드 자체가 없으면 그 위험이 구조적으로 사라진다.
        let mut findings_value =
            serde_json::to_value(&findings).unwrap_or(serde_json::Value::Array(Vec::new()));
        if no_raw {
            if let serde_json::Value::Array(items) = &mut findings_value {
                for item in items.iter_mut() {
                    if let serde_json::Value::Object(obj) = item {
                        obj.remove("raw");
                    }
                }
            }
        }
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "kinds": kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
            "mask": mask_char.to_string(),
            "dryRun": dry_run,
            "inPlace": in_place,
            "noRaw": no_raw,
            "findingCount": findings.len(),
            "findings": findings_value,
            "redactedCount": redacted_count,
            "changedPages": changed_pages,
        });
        if wrote_output {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        // [#3885] findings[].raw 는 마스킹 전 원문 — 개인정보 그 자체다. 가장 민감한
        // 값을 싣는 봉투가 출처 표지 없이 나가면 S1 계약("표지는 항상 실린다")이
        // 정확히 그 지점에서 무너진다. --no-raw 면 raw 경로가 봉투에 없으므로
        // 표지도 masked 만 선언한다(실재 경로 필터).
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "마스킹 예정: {} — 탐지 {}건 (원문 {}개). 실제 적용은 -o 또는 --in-place.",
            file_path,
            findings.len(),
            targets.len()
        );
        for f in &findings {
            // --no-raw 는 --json 뿐 아니라 이 사람용 출력에도 적용한다 — 콘솔 로그·
            // 터미널 스크롤백도 유출 경로이므로 절반만 가려서는 목적을 달성하지 못한다.
            let shown_raw: &str = if no_raw {
                "(생략됨, --no-raw)"
            } else {
                &f.raw
            };
            println!(
                "  [{}] {} → {} (구역 {}, 문단 {}, 쪽 {})",
                f.kind,
                shown_raw,
                f.masked,
                f.section,
                f.paragraph,
                f.page
                    .map(|p| (p + 1).to_string())
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
    } else if redacted_count == 0 {
        println!("마스킹 0건: {} — 탐지 없음 (출력 파일 미생성)", file_path);
    } else {
        println!(
            "마스킹 완료: {} → {} — {}건",
            file_path, output_path, redacted_count
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}



/// 두 경로가 **이미 존재하는 같은 파일**을 가리키는지. 판정 불가면 `false`.
///
/// 산출 경로는 대개 존재하지 않으므로 정규화가 실패하는 것이 정상이다. 여기서
/// 잡으려는 것은 `-o` 로 원본 자신을 지목한 경우 하나뿐이다.
pub(crate) fn same_existing_path(a: &str, b: &str) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}



/// FILETIME(1601-01-01 UTC 기준 100ns) → `YYYY-MM-DDTHH:MM:SSZ`.
///
/// 감사 기록용이다 — 무엇을 지웠는지 사람이 읽을 수 있어야 "조용히 지우지 않았다"가
/// 성립한다.
pub(crate) fn filetime_to_iso(ft: u64) -> String {
    const SECS_1601_TO_1970: i64 = 11_644_473_600;
    let secs = (ft / 10_000_000) as i64 - SECS_1601_TO_1970;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant, civil_from_days (proleptic Gregorian).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + i64::from(m <= 2);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}



/// `\u{5}HwpSummaryInformation`(OLE 속성 집합)에서 작성자·이력 메타를 지운다.
///
/// **바이트 길이를 바꾸지 않는다** — 속성 오프셋 표가 절대 위치를 담고 있어 크기를
/// 줄이면 나머지 속성이 전부 어긋난다. 문자열은 `cch=1`(NUL 하나)로 만들고 남은
/// 자리를 0으로 덮으며, FILETIME 은 0(미설정)으로 만든다.
///
/// 반환: `(필드 이름, 지우기 전 값)` 목록. 형식을 해석하지 못하면 빈 목록(무변경).
pub(crate) fn sanitize_summary_information(data: &mut [u8]) -> Vec<(String, String)> {
    fn u32_at(d: &[u8], off: usize) -> Option<u32> {
        d.get(off..off + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    let mut removed: Vec<(String, String)> = Vec::new();
    if data.len() < 48 || data[0] != 0xFE || data[1] != 0xFF {
        return removed;
    }
    let Some(section_off) = u32_at(data, 44).map(|v| v as usize) else {
        return removed;
    };
    let Some(count) = u32_at(data, section_off + 4).map(|v| v as usize) else {
        return removed;
    };
    // 병적으로 큰 개수는 해석을 포기한다(손상 파일에서 헛돌지 않게).
    if count > 4096 || section_off + 8 + count * 8 > data.len() {
        return removed;
    }

    for idx in 0..count {
        let pair = section_off + 8 + idx * 8;
        let (Some(pid), Some(rel)) = (u32_at(data, pair), u32_at(data, pair + 4)) else {
            continue;
        };
        let Some((_, field)) = SUMMARY_TARGETS.iter().find(|(p, _)| *p == pid) else {
            continue;
        };
        let abs = section_off + rel as usize;
        let Some(vt) = u32_at(data, abs) else {
            continue;
        };
        match vt {
            // VT_LPWSTR — UTF-16LE, cch 는 종단 NUL 을 포함한 문자 수.
            0x1F => {
                let Some(cch) = u32_at(data, abs + 4).map(|v| v as usize) else {
                    continue;
                };
                let start = abs + 8;
                let Some(raw) = data.get(start..start + cch * 2) else {
                    continue;
                };
                let units: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .take_while(|u| *u != 0)
                    .collect();
                if units.is_empty() {
                    continue;
                }
                removed.push((field.to_string(), String::from_utf16_lossy(&units)));
                data[start..start + cch * 2].fill(0);
                data[abs + 4..abs + 8].copy_from_slice(&1u32.to_le_bytes());
            }
            // VT_FILETIME.
            0x40 => {
                let Some(raw) = data.get(abs + 4..abs + 12) else {
                    continue;
                };
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(raw);
                let value = u64::from_le_bytes(bytes);
                if value == 0 {
                    continue;
                }
                removed.push((field.to_string(), filetime_to_iso(value)));
                data[abs + 4..abs + 12].fill(0);
            }
            _ => {}
        }
    }
    removed
}



/// HWPX `Contents/content.hpf` 의 `<opf:metadata>` 블록을 중립 블록으로 바꾼다.
///
/// 이 블록은 직렬화기가 원본에서 그대로 splice 하는 유일한 저작자 정보 경로다
/// (`serializer::hwpx::content::write_content_hpf`). 지우지 않으면 HWPX 산출물에
/// 작성자·작성일이 그대로 남는다. 반환: 지우기 전 블록(있었을 때만).
pub(crate) fn sanitize_hwpx_metadata(entry: &mut Vec<u8>) -> Option<String> {
    const NEUTRAL: &str =
        "<opf:metadata><opf:title/><opf:language>ko</opf:language></opf:metadata>";
    let text = String::from_utf8(entry.clone()).ok()?;
    let open = text.find("<opf:metadata>")?;
    let close = text[open..].find("</opf:metadata>")? + open + "</opf:metadata>".len();
    let before = text[open..close].to_string();
    if before == NEUTRAL {
        return None;
    }
    let mut rebuilt = String::with_capacity(text.len());
    rebuilt.push_str(&text[..open]);
    rebuilt.push_str(NEUTRAL);
    rebuilt.push_str(&text[close..]);
    *entry = rebuilt.into_bytes();
    Some(before)
}



/// 본문 문단 텍스트를 공백·제어문자를 뺀 한 줄로 잇는다 (미리보기 대조용).
///
/// `serializer::cfb_writer::build_preview_text` 와 같은 범위(본문 문단만, 표·글상자 제외).
pub(crate) fn body_text_signature(document: &rhwp::model::document::Document) -> String {
    const MAX: usize = 4000;
    let mut out = String::new();
    for section in &document.sections {
        for para in &section.paragraphs {
            out.extend(
                para.text
                    .chars()
                    .filter(|c| !c.is_whitespace() && !c.is_control()),
            );
            if out.chars().count() >= MAX {
                return out;
            }
        }
    }
    out
}



/// 미리보기 텍스트가 **지금 본문**의 앞부분과 같은지.
///
/// 같으면 유출이 아니라 본문의 파생물이다(저장 시 어차피 같은 값이 다시 만들어진다).
/// 다르면 예전 판의 잔재 — 본문에서 지운 문장이 미리보기에만 남아 있는 전형적 사고다.
pub(crate) fn preview_text_is_current(preview: &str, body_signature: &str) -> bool {
    let stripped: String = preview
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    stripped.is_empty() || body_signature.starts_with(&stripped)
}



/// `edit sanitize` — 문서 메타데이터를 제거한다 (본문은 건드리지 않는다).
///
/// 작성자·회사·최종수정자·작성일과 미리보기(PrvText/PrvImage)를 지운다. 무엇을
/// 지웠는지 `removed[]` 로 남긴다 — 조용히 지우면 감사할 수 없다.
pub(crate) fn edit_sanitize(args: &[String]) -> i32 {
    let mut file_path: Option<&str> = None;
    let mut out_path: Option<String> = None;
    let mut keep_preview = false;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--keep-preview" => keep_preview = true,
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
        i += 1;
    }

    let Some(file_path) = file_path else {
        eprintln!(
            "사용법: rhwp edit sanitize <파일.hwp|파일.hwpx> [--keep-preview] [-o <출력>] [--json]"
        );
        return EXIT_USAGE;
    };

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match rhwp::wasm_api::HwpDocument::from_bytes(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: HWP 파싱 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    let out_format = edit_output_format(&bytes, out_path.as_deref());
    // HWPX 원본의 `/HwpSummaryInformation` 은 **파일에 없던** 계약 fallback 상수다
    // (`parser::hwpx::contract_streams`). HWPX 로 저장하면 산출물에도 실리지 않으므로
    // 손대지 않는다 — 없던 것을 지웠다고 보고하면 감사 기록이 거짓이 된다. HWP5 로
    // 변환할 때만 실제 산출물에 들어가므로 그때는 지운다.
    let source_is_hwpx = matches!(
        rhwp::parser::detect_format(&bytes),
        rhwp::parser::FileFormat::Hwpx
    );
    let touch_summary = !(source_is_hwpx && out_format == EditOutputFormat::Hwpx);

    let mut removed: Vec<(String, String)> = Vec::new();
    {
        let document = doc.document_mut();

        // ① OLE 요약 정보 (HWP5 원본 · HWPX→HWP5 변환 계약 스트림).
        if touch_summary {
            for (path, data) in document.extra_streams.iter_mut() {
                if !path
                    .trim_start_matches(['/', '\u{5}'])
                    .eq_ignore_ascii_case("HwpSummaryInformation")
                {
                    continue;
                }
                removed.extend(sanitize_summary_information(data));
            }
        }

        // ② HWPX 저작자 메타(content.hpf 의 opf:metadata splice 경로).
        for (path, entry) in document.hwpx_aux_entries.iter_mut() {
            if path != "Contents/content.hpf" {
                continue;
            }
            if let Some(before) = sanitize_hwpx_metadata(entry) {
                removed.push(("hwpx.metadata".to_string(), before));
            }
        }

        // ③ 미리보기 — 예전 판의 잔재가 남는 자리다. 본문에서 이미 지운 문장이
        //    미리보기에만 남아 공개되는 사고가 이 명령의 존재 이유 중 하나다.
        //    지금 본문과 같은 미리보기는 파생물이므로 보고하지 않는다(저장 시 재생성).
        let body_signature = body_text_signature(document);

        if let Some(preview) = document.preview.as_mut() {
            let stale = preview
                .text
                .as_deref()
                .is_some_and(|t| !preview_text_is_current(t, &body_signature));
            if stale {
                if let Some(text) = preview.text.take() {
                    removed.push((
                        "preview.text".to_string(),
                        text.chars().take(60).collect::<String>(),
                    ));
                }
            }
            if !keep_preview {
                if let Some(image) = preview.image.take() {
                    removed.push((
                        "preview.image".to_string(),
                        format!("{:?} {} bytes", image.format, image.data.len()),
                    ));
                }
            }
        }
        if document
            .preview
            .as_ref()
            .is_some_and(|p| p.text.is_none() && p.image.is_none())
        {
            document.preview = None;
        }

        // HWPX 컨테이너의 미리보기 — ZIP 엔트리(HWPX 산출용)와 계약 스트림
        // (HWPX→HWP5 변환용)은 같은 것의 두 표현이므로 함께 지우고 한 번만 보고한다.
        let hwpx_preview_text = document
            .hwpx_aux_entry("Preview/PrvText.txt")
            .and_then(|b| std::str::from_utf8(b).ok())
            .map(str::to_string);
        let drop_hwpx_text = hwpx_preview_text
            .as_deref()
            .is_some_and(|t| !preview_text_is_current(t, &body_signature));
        if drop_hwpx_text {
            if let Some(text) = hwpx_preview_text {
                removed.push((
                    "preview.text".to_string(),
                    text.chars().take(60).collect::<String>(),
                ));
            }
        }
        // 직렬화기는 엔트리가 없으면 빈 자리표시자를 넣는다. 이미 자리표시자면
        // 지울 것이 없다 — 반복 실행이 매번 "지웠다"고 보고하지 않게 한다.
        let drop_hwpx_image = !keep_preview
            && document
                .hwpx_aux_entry("Preview/PrvImage.png")
                .is_some_and(|b| b != rhwp::serializer::hwpx::static_assets::PRV_IMAGE_PNG);
        if drop_hwpx_image {
            if let Some(bytes) = document.hwpx_aux_entry("Preview/PrvImage.png") {
                removed.push((
                    "preview.image".to_string(),
                    format!("Png {} bytes", bytes.len()),
                ));
            }
        }
        document.hwpx_aux_entries.retain(|(path, _)| {
            !(path == "Preview/PrvText.txt" && drop_hwpx_text)
                && !(path == "Preview/PrvImage.png" && drop_hwpx_image)
        });
        document.extra_streams.retain(|(path, _)| {
            !(path == "/PrvText" && drop_hwpx_text) && !(path == "/PrvImage" && !keep_preview)
        });
    }

    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_sanitized.{}", stem, out_format.ext())
    });

    let out_bytes = match edit_serialize(&mut doc, out_format) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "오류: {} 직렬화 실패 - {}",
                out_format.label().to_uppercase(),
                e
            );
            return EXIT_RUNTIME;
        }
    };
    if let Err(e) = atomic_file::write_atomically(Path::new(&output_path), &out_bytes) {
        eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
        return EXIT_RUNTIME;
    }

    if json_mode {
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "keepPreview": keep_preview,
            "removedCount": removed.len(),
            "removed": removed
                .iter()
                .map(|(field, before)| serde_json::json!({ "field": field, "before": before }))
                .collect::<Vec<_>>(),
            "output": output_path,
            "outputFormat": out_format.label(),
        });
        // [#3885] removed[].before 는 지워진 문서 속성 원문이다 — 제목·작성자에
        // 더해 preview.text 는 본문 첫 화면 발췌라 문서 문장이 통째로 실린다.
        println!("{}", provenance::marked(envelope, "edit"));
        return EXIT_OK;
    }

    println!(
        "메타 제거 완료: {} → {} — {}건",
        file_path,
        output_path,
        removed.len()
    );
    for (field, before) in &removed {
        println!("  {field}: {before}");
    }
    EXIT_OK
}



/// `edit set-cell` — 표 격자 좌표로 셀 값을 바꾼다 (실물 표 양식 채우기).
///
/// [#3381] 좌표계는 `export-tables` 격자와 동일하다 — 발견과 편집이 같은 주소를 쓴다.
/// 검증된 코어 셀 편집 경로(delete/insert_text_in_cell)를 재사용하므로 새 편집 로직이
/// 없다. v1 범위: 본문 최상위 표, 셀 첫 문단 교체(중첩 표·다문단 셀은 후속).
/// [#3391] 셀 문단 0 의 글자모양을 검정·비이탤릭·비진하게 글자모양 하나로 덮는다.
/// 안내문(파란 이탤릭)을 지우고 실값을 쓰는 set-cell 의 제출 요건(검정 글씨) 대응.
/// 대상 셀의 첫 글자모양을 복제하므로 글꼴·크기·자간은 보존한다. 같은 모양이 이미 있으면
/// 재사용한다.
/// 반환: 적용 성공 여부(좌표 해석 실패 시 false).
pub(crate) fn recolor_cell_text_black(
    document: &mut rhwp::model::document::Document,
    sec: usize,
    para: usize,
    ctrl: usize,
    cell_idx: usize,
) -> bool {
    use rhwp::model::control::Control;
    use rhwp::model::paragraph::CharShapeRef;

    // 대상 셀의 현재 글자모양을 기준으로 해야 한다. 문서 어딘가의 "검정" 모양을 재사용하면
    // 글꼴·크기까지 바뀔 수 있다.
    let source_id = {
        let Some(section) = document.sections.get(sec) else {
            return false;
        };
        let Some(parent) = section.paragraphs.get(para) else {
            return false;
        };
        let Some(Control::Table(table)) = parent.controls.get(ctrl) else {
            return false;
        };
        let Some(cell) = table.cells.get(cell_idx) else {
            return false;
        };
        let Some(paragraph) = cell.paragraphs.first() else {
            return false;
        };
        let Some(shape) = paragraph.char_shapes.first() else {
            return false;
        };
        shape.char_shape_id as usize
    };
    let Some(base) = document
        .doc_info
        .char_shapes
        .get(source_id)
        .or_else(|| document.doc_info.char_shapes.first())
        .cloned()
    else {
        return false;
    };
    let mut black = base;
    black.raw_data = None; // 원본 바이트를 버려 변경된 필드가 직렬화되게 한다.
    black.text_color = 0;
    black.italic = false;
    black.bold = false;
    black.strikethrough = false;
    black.underline_type = rhwp::model::style::UnderlineType::None;
    let black_id = document
        .doc_info
        .char_shapes
        .iter()
        .position(|candidate| candidate == &black)
        .map(|idx| idx as u32)
        .unwrap_or_else(|| {
            let new_id = document.doc_info.char_shapes.len() as u32;
            document.doc_info.char_shapes.push(black);
            new_id
        });

    let Some(section) = document.sections.get_mut(sec) else {
        return false;
    };
    let Some(parent) = section.paragraphs.get_mut(para) else {
        return false;
    };
    let Some(Control::Table(table)) = parent.controls.get_mut(ctrl) else {
        return false;
    };
    let Some(cell) = table.cells.get_mut(cell_idx) else {
        return false;
    };
    let Some(cell_para) = cell.paragraphs.get_mut(0) else {
        return false;
    };
    // 문단 전체를 하나의 검정 글자모양으로 덮는다.
    cell_para.char_shapes = vec![CharShapeRef {
        start_pos: 0,
        char_shape_id: black_id,
    }];
    true
}



/// [#3480] 셀에 넣을 텍스트가 칸 폭을 넘치는지 잰다.
///
/// 넘치면 `(칸 폭 px, 글자 폭 px, 예상 줄 수)` 를 돌려주고, 들어가면 `None`.
/// 폭은 조판 엔진의 글자 폭 추정(`estimate_text_width_px`)과 IR 의 `Cell.width` 를 쓴다.
/// **채우기를 막지는 않는다** — 여러 줄이 정상인 칸도 있으므로 신호만 준다.
pub(crate) fn measure_cell_overflow(
    doc: &rhwp::wasm_api::HwpDocument,
    sec: usize,
    para: usize,
    ctrl: usize,
    cell_idx: usize,
    text: &str,
) -> Option<(f64, f64, usize)> {
    use rhwp::model::control::Control;
    use rhwp::renderer::hwpunit_to_px;

    if text.is_empty() {
        return None;
    }
    let cell = doc
        .document()
        .sections
        .get(sec)?
        .paragraphs
        .get(para)?
        .controls
        .get(ctrl)
        .and_then(|c| match c {
            Control::Table(t) => t.cells.get(cell_idx),
            _ => None,
        })?;

    // 셀 안여백을 뺀 실제 글자 영역 폭.
    let padding = (cell.padding.left + cell.padding.right) as f64;
    let usable = hwpunit_to_px(
        (cell.width as f64 - padding) as i32,
        rhwp::renderer::DEFAULT_DPI,
    );
    if usable <= 0.0 {
        return None;
    }

    let text_w = estimate_text_width_px(doc, sec, para, ctrl, cell_idx, text);
    if text_w <= usable {
        return None;
    }
    let lines = (text_w / usable).ceil() as usize;
    Some((usable, text_w, lines))
}



/// 셀의 첫 문단 글자 모양을 기준으로 텍스트 폭(px)을 추정한다.
///
/// 정밀 조판이 아니라 **넘침 여부 판정용 근사**다 — 한글은 전각, ASCII 는 반각으로 센다.
pub(crate) fn estimate_text_width_px(
    doc: &rhwp::wasm_api::HwpDocument,
    sec: usize,
    para: usize,
    ctrl: usize,
    cell_idx: usize,
    text: &str,
) -> f64 {
    use rhwp::model::control::Control;
    use rhwp::renderer::hwpunit_to_px;

    // 셀 첫 문단의 글자 크기(HWPUNIT, 1pt = 100). 못 찾으면 10pt 로 본다.
    let size_hwpunit = doc
        .document()
        .sections
        .get(sec)
        .and_then(|s| s.paragraphs.get(para))
        .and_then(|p| p.controls.get(ctrl))
        .and_then(|c| match c {
            Control::Table(t) => t.cells.get(cell_idx),
            _ => None,
        })
        .and_then(|cell| cell.paragraphs.first())
        .and_then(|p| p.char_shapes.first())
        .and_then(|cs| {
            doc.document()
                .doc_info
                .char_shapes
                .get(cs.char_shape_id as usize)
        })
        .map(|cs| cs.base_size as f64)
        .unwrap_or(1000.0);

    let em = hwpunit_to_px(size_hwpunit as i32, rhwp::renderer::DEFAULT_DPI);
    text.chars()
        .map(|c| if c.is_ascii() { em * 0.5 } else { em })
        .sum()
}



/// 셀 값에 제어문자가 있으면 공통 안내문을 돌려준다 (없으면 `None`).
///
/// 문장뿐 아니라 **판정식까지** 공유해야 '문장은 같은데 거부 조건이 다른' 어긋남이 안 생긴다.
pub(crate) fn set_cell_control_char_rejection(text: &str) -> Option<&'static str> {
    text.chars()
        .any(|ch| matches!(ch, '\r' | '\n' | '\t'))
        .then_some(SET_CELL_CONTROL_CHAR_MESSAGE)
}



#[allow(clippy::type_complexity)]
pub(crate) fn resolve_table_cell(
    document: &rhwp::model::document::Document,
    table_no: usize,
    row: u16,
    col: u16,
) -> Result<(usize, usize, usize, usize, Vec<usize>, String), CellResolveError> {
    use rhwp::document_core::queries::table_extract::extract_tables;
    use rhwp::model::control::Control;
    let grids = extract_tables(document);
    let Some(grid) = grids
        .iter()
        .find(|g| g.index == table_no && g.container_path.is_empty())
    else {
        let top_level = grids.iter().filter(|g| g.container_path.is_empty()).count();
        return Err(CellResolveError::Runtime(format!(
            "오류: 본문 최상위 표 {} 번이 없습니다 (최상위 표 {}개; 중첩 표는 v1 범위 밖).",
            table_no, top_level
        )));
    };
    let Some(Control::Table(table)) = document.sections[grid.section].paragraphs[grid.paragraph]
        .controls
        .get(grid.control)
    else {
        return Err(CellResolveError::Runtime(
            "오류: 표 컨트롤 좌표 해석 실패 (내부 불일치).".into(),
        ));
    };
    if row >= table.row_count || col >= table.col_count {
        return Err(CellResolveError::Usage(format!(
            "오류: 좌표가 격자를 벗어났습니다 — 표 {} 는 {}x{} 입니다.",
            table_no, table.row_count, table.col_count
        )));
    }
    match table
        .cells
        .iter()
        .enumerate()
        .find(|(_, c)| c.row == row && c.col == col)
    {
        Some((cell_idx, c)) => {
            let para_lens: Vec<usize> = c
                .paragraphs
                .iter()
                .map(|p| p.text.chars().count())
                .collect();
            let old_text = c
                .paragraphs
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join(
                    "
",
                )
                .trim()
                .to_string();
            Ok((
                grid.section,
                grid.paragraph,
                grid.control,
                cell_idx,
                para_lens,
                old_text,
            ))
        }
        None => {
            let anchor = table.cells.iter().find(|c| {
                c.row <= row && row < c.row + c.row_span && c.col <= col && col < c.col + c.col_span
            });
            Err(CellResolveError::Usage(match anchor {
                Some(a) => format!(
                    "오류: ({},{}) 는 병합으로 덮인 칸입니다 — 앵커 ({},{}) 를 지정하세요.",
                    row, col, a.row, a.col
                ),
                None => format!("오류: ({},{}) 위치에 셀이 없습니다.", row, col),
            }))
        }
    }
}


/// 최상위 표 번호(`--table N`, `set-cell`/`export-tables`와 같은 채번)를 셀 좌표 없이
/// (구역, 부모 문단, 컨트롤) 로만 해석한다. 표 단위 편집(`move-table`/`transpose-table`/
/// `set-column-widths`)이 공유한다 — `resolve_table_cell`에서 row/col 해석만 뺀 부분집합.
pub(crate) fn resolve_table_index(
    document: &rhwp::model::document::Document,
    table_no: usize,
) -> Result<(usize, usize, usize), CellResolveError> {
    use rhwp::document_core::queries::table_extract::extract_tables;
    let grids = extract_tables(document);
    let Some(grid) = grids
        .iter()
        .find(|g| g.index == table_no && g.container_path.is_empty())
    else {
        let top_level = grids.iter().filter(|g| g.container_path.is_empty()).count();
        return Err(CellResolveError::Runtime(format!(
            "오류: 본문 최상위 표 {} 번이 없습니다 (최상위 표 {}개; 중첩 표는 v1 범위 밖).",
            table_no, top_level
        )));
    };
    Ok((grid.section, grid.paragraph, grid.control))
}

/// `resolve_table_index`가 가리키는 표의 (행 수, 열 수) — dry-run 미리보기와
/// 폭 개수 사전 검증에 쓴다(표를 실제로 바꾸지 않는 읽기 전용 조회).
pub(crate) fn table_dimensions(
    document: &rhwp::model::document::Document,
    section_idx: usize,
    parent_para_idx: usize,
    control_idx: usize,
) -> Option<(u16, u16)> {
    use rhwp::model::control::Control;
    match document.sections[section_idx].paragraphs[parent_para_idx]
        .controls
        .get(control_idx)
    {
        Some(Control::Table(t)) => Some((t.row_count, t.col_count)),
        _ => None,
    }
}

pub(crate) fn edit_set_cell(args: &[String]) -> i32 {
    let mut file_path: Option<&str> = None;
    let mut table_arg: Option<usize> = None;
    let mut row_arg: Option<u16> = None;
    let mut col_arg: Option<u16> = None;
    let mut text_arg: Option<&str> = None;
    let mut out_path: Option<String> = None;
    let mut dry_run = false;
    let mut json_mode = false;
    // [#3702] 저장 직후 자기검증 — 판정은 데이터, 차이 시 exit 3.
    let mut verify_mode = false;
    // [#3391] 실물 공고 양식의 기입 칸 안내문은 파란 이탤릭이 흔하다. set-cell 은
    // "안내문을 지우고 실값을 쓰는" 용도이므로 제출 요건(검정 글씨)에 맞춰 기본을
    // 검정·비이탤릭·비진하게로 기록한다. --keep-style 로 셀 스타일 상속을 유지한다.
    let mut keep_style = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--keep-style" => keep_style = true,
            "--table" | "--row" | "--col" => {
                let name = args[i].clone();
                i += 1;
                let Some(v) = args.get(i) else {
                    eprintln!("오류: {} 뒤에 0 이상의 정수가 필요합니다.", name);
                    return EXIT_USAGE;
                };
                match name.as_str() {
                    "--table" => match v.parse::<usize>() {
                        Ok(value) => table_arg = Some(value),
                        Err(_) => {
                            eprintln!("오류: {} 뒤에 0 이상의 정수가 필요합니다.", name);
                            return EXIT_USAGE;
                        }
                    },
                    "--row" => match v.parse::<u16>() {
                        Ok(value) => row_arg = Some(value),
                        Err(_) => {
                            eprintln!("오류: {} 뒤에 0 이상 65535 이하의 정수가 필요합니다.", name);
                            return EXIT_USAGE;
                        }
                    },
                    _ => match v.parse::<u16>() {
                        Ok(value) => col_arg = Some(value),
                        Err(_) => {
                            eprintln!("오류: {} 뒤에 0 이상 65535 이하의 정수가 필요합니다.", name);
                            return EXIT_USAGE;
                        }
                    },
                }
            }
            "--text" => {
                i += 1;
                match args.get(i) {
                    Some(v) => text_arg = Some(v),
                    None => {
                        eprintln!(
                            "오류: --text 뒤에 셀에 넣을 문자열이 필요합니다 (비우기는 \"\")."
                        );
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
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

    let (Some(file_path), Some(table_no), Some(row), Some(col), Some(new_text)) =
        (file_path, table_arg, row_arg, col_arg, text_arg)
    else {
        eprintln!(
            "사용법: rhwp edit set-cell <파일> --table <번호> --row <행> --col <열> --text <문자열> [-o <출력>] [--keep-style] [--dry-run] [--json]"
        );
        return EXIT_USAGE;
    };
    // 판정과 문장 모두 세션 도구(hwp_doc_set_cell)와 공유한다 — 문서를 읽기 전에 끊는다.
    if let Some(message) = set_cell_control_char_rejection(new_text) {
        eprintln!("{message}");
        return EXIT_USAGE;
    }

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match load_document(&bytes) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    // 격자 주소(export-tables 좌표) → 모델 좌표. 병합으로 덮인 칸은 앵커가 아니므로
    // 모델 셀 순회로 (row,col) 앵커를 직접 찾는다 (격자 배열 위치는 손상 방어 필터
    // 때문에 모델 인덱스와 어긋날 수 있어 쓰지 않는다).
    let (sec, para, ctrl, cell_idx, para_lens, old_text) =
        match resolve_table_cell(doc.document(), table_no, row, col) {
            Ok(v) => v,
            Err(CellResolveError::Usage(msg)) => {
                eprintln!("{msg}");
                return EXIT_USAGE;
            }
            Err(CellResolveError::Runtime(msg)) => {
                eprintln!("{msg}");
                return EXIT_RUNTIME;
            }
        };

    // [#3480] 값이 그 칸에 들어가는지 재고 넘치면 알린다.
    // 에이전트는 렌더 결과를 보지 않으므로, 신호가 없으면 표 경계를 벗어난 문서를
    // 완성본으로 판단한다. 조판 엔진이 있어야 답할 수 있는 검사다.
    let overflow = measure_cell_overflow(&doc, sec, para, ctrl, cell_idx, &new_text).map(
        |(cell_w, text_w, lines)| {
            serde_json::json!({
                "target": format!("table{}[{},{}]", table_no, row, col),
                "text": new_text,
                "cellWidthPx": (cell_w * 100.0).round() / 100.0,
                "textWidthPx": (text_w * 100.0).round() / 100.0,
                "lines": lines,
            })
        },
    );

    if !dry_run {
        // 셀의 모든 문단 텍스트를 비운다 (다문단 셀 — 빈 문단 골격은 유지된다).
        for (pi, len) in para_lens.iter().enumerate() {
            if *len == 0 {
                continue;
            }
            if let Err(e) = doc.delete_text_in_cell(
                sec as u32,
                para as u32,
                ctrl as u32,
                cell_idx as u32,
                pi as u32,
                0,
                *len as u32,
            ) {
                eprintln!("오류: 셀 비우기 실패(문단 {}) - {:?}", pi, e);
                return EXIT_RUNTIME;
            }
        }
        if !new_text.is_empty() {
            if let Err(e) = doc.insert_text_in_cell(
                sec as u32,
                para as u32,
                ctrl as u32,
                cell_idx as u32,
                0,
                0,
                new_text,
            ) {
                eprintln!("오류: 셀 쓰기 실패 - {:?}", e);
                // 실패 시 원본 불변 — 출력 파일을 쓰지 않고 즉시 끝낸다.
                return EXIT_RUNTIME;
            }
            // [#3391] 기본은 제출 요건(검정 글씨)에 맞춘다 — 셀 문단 0 의 글자모양을
            // 검정·비이탤릭·비진하게 글자모양 하나로 덮는다. --keep-style 이면 생략.
            if !keep_style
                && !recolor_cell_text_black(doc.document_mut(), sec, para, ctrl, cell_idx)
            {
                eprintln!("경고: 셀 글자색을 검정으로 바꾸지 못했습니다 (상속 스타일 유지).");
            }
        }
    }

    // [#3383] 입력 형식을 보존한다 — 기본 확장자도 산출 형식을 따른다.
    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_cell.{}", stem, out_format.ext())
    });

    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if !dry_run {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if let Err(e) = fs::write(&output_path, &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        // [#3702] 저장 직후 자기검증 — 편집 후 IR ↔ 저장본 재파싱 IR.
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    // [#3712] 눈검증 대상 페이지 — 표 호스트 문단이 걸친 쪽 전부(분할 표 포함).
    let changed_pages = if dry_run {
        serde_json::Value::Null
    } else {
        match doc.pages_covering_paragraphs(&[(sec, para)]) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    };

    if json_mode {
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "table": table_no,
            "row": row,
            "col": col,
            "oldText": old_text,
            "newText": new_text,
            "dryRun": dry_run,
            "changedPages": changed_pages,
            "keepStyle": keep_style,
            "overflow": overflow.clone().map(|o| vec![o]).unwrap_or_default(),
        });
        if !dry_run {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "변경 예정: {} 표{} ({},{}) {:?} → {:?}",
            file_path, table_no, row, col, old_text, new_text
        );
    } else {
        println!(
            "셀 기록 완료: {} → {} — 표{} ({},{}) {:?} → {:?}",
            file_path, output_path, table_no, row, col, old_text, new_text
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}



/// `edit set-table-props` — 표 속성(칸간격·여백·글자처럼·배치·테두리/배경·캡션 등)을
/// 고친다 (upstream #5185/#5192 계열 선별 이식 — 배선은 CLI뿐, 코어 로직은 기존
/// `set_table_properties_native` 재사용).
///
/// `--props`는 JSON 객체 문자열이며 다루는 속성은 그 필드 이름으로 정한다(예:
/// `{"cellSpacing":200}`, `{"treatAsChar":true}`). 지원 필드 전체는
/// `set_table_properties_native`(`document_core/commands/table_ops.rs`) 구현이 정본이다
/// — 이 CLI 배선은 필드를 해석하지 않고 그대로 넘긴다. JSON 객체가 아니면
/// `--dry-run`에서도 사용법 오류(exit 2)로 미리 잡는다.
pub(crate) fn edit_set_table_props(args: &[String]) -> i32 {
    const USAGE: &str = "사용법: rhwp edit set-table-props <파일> --table <번호> --props <JSON> [-o <출력>] [--dry-run] [--verify] [--json]";

    let mut file_path: Option<&str> = None;
    let mut table_arg: Option<usize> = None;
    let mut props: Option<String> = None;
    let mut out_path: Option<String> = None;
    let mut dry_run = false;
    let mut json_mode = false;
    let mut verify_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--table" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) => table_arg = Some(v),
                    None => {
                        eprintln!("오류: --table 뒤에 0 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--props" => {
                i += 1;
                match args.get(i) {
                    Some(v) => props = Some(v.clone()),
                    None => {
                        eprintln!("오류: --props 뒤에 JSON 객체 문자열이 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
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

    let (Some(file_path), Some(table_no), Some(props)) = (file_path, table_arg, props) else {
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };
    if !matches!(
        serde_json::from_str::<serde_json::Value>(&props),
        Ok(serde_json::Value::Object(_))
    ) {
        eprintln!("오류: --props 는 JSON 객체여야 합니다.");
        return EXIT_USAGE;
    }

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match load_document(&bytes) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let (sec, para, ctrl) = match resolve_table_index(doc.document(), table_no) {
        Ok(v) => v,
        Err(CellResolveError::Usage(msg)) => {
            eprintln!("{msg}");
            return EXIT_USAGE;
        }
        Err(CellResolveError::Runtime(msg)) => {
            eprintln!("{msg}");
            return EXIT_RUNTIME;
        }
    };

    let mut caption_char_offset: Option<u64> = None;
    if !dry_run {
        let result = match doc.set_table_properties_native(sec, para, ctrl, &props) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: 표 속성 변경 실패 - {}", e);
                return EXIT_RUNTIME;
            }
        };
        caption_char_offset = serde_json::from_str::<serde_json::Value>(&result)
            .ok()
            .and_then(|v| v["captionCharOffset"].as_u64());
    }

    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_tblprop.{}", stem, out_format.ext())
    });

    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if !dry_run {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if let Err(e) = fs::write(&output_path, &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    let changed_pages = if dry_run {
        serde_json::Value::Null
    } else {
        match doc.pages_covering_paragraphs(&[(sec, para)]) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    };

    if json_mode {
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "table": table_no,
            "props": serde_json::from_str::<serde_json::Value>(&props).unwrap_or(serde_json::Value::Null),
            "dryRun": dry_run,
            "changedPages": changed_pages,
        });
        if let Some(offset) = caption_char_offset {
            envelope["captionCharOffset"] = serde_json::json!(offset);
        }
        if !dry_run {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "표 속성 변경 예정: {} 표{} props={}",
            file_path, table_no, props
        );
    } else {
        println!(
            "표 속성 변경 완료: {} → {} — 표{} props={}",
            file_path, output_path, table_no, props
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}



/// `edit move-table` — 표의 위치 오프셋(HWPUNIT)을 이동한다 (upstream #5185/#5192 계열
/// 선별 이식 — 배선은 CLI뿐, 코어 로직은 기존 `move_table_offset_native` 재사용).
///
/// `--dh`/`--dv` 는 생략하면 0(그 축은 이동하지 않음). treat_as_char(본문배치) 표가
/// 문단 경계를 넘으면 표가 다른 문단으로 옮겨갈 수 있어 `changedPages`는 이동 후
/// 최종 문단 기준으로 계산한다.
pub(crate) fn edit_move_table(args: &[String]) -> i32 {
    const USAGE: &str = "사용법: rhwp edit move-table <파일> --table <번호> [--dh <HWPUNIT>] [--dv <HWPUNIT>] [-o <출력>] [--dry-run] [--verify] [--json]";

    let mut file_path: Option<&str> = None;
    let mut table_arg: Option<usize> = None;
    let mut dh: i32 = 0;
    let mut dv: i32 = 0;
    let mut out_path: Option<String> = None;
    let mut dry_run = false;
    let mut json_mode = false;
    let mut verify_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--table" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) => table_arg = Some(v),
                    None => {
                        eprintln!("오류: --table 뒤에 0 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dh" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<i32>().ok()) {
                    Some(v) => dh = v,
                    None => {
                        eprintln!("오류: --dh 뒤에 정수(HWPUNIT)가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dv" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<i32>().ok()) {
                    Some(v) => dv = v,
                    None => {
                        eprintln!("오류: --dv 뒤에 정수(HWPUNIT)가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
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

    let (Some(file_path), Some(table_no)) = (file_path, table_arg) else {
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };
    if dh == 0 && dv == 0 {
        eprintln!("오류: --dh 또는 --dv 중 하나 이상 0이 아닌 값을 지정해야 합니다.");
        return EXIT_USAGE;
    }

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match load_document(&bytes) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let (sec, para, ctrl) = match resolve_table_index(doc.document(), table_no) {
        Ok(v) => v,
        Err(CellResolveError::Usage(msg)) => {
            eprintln!("{msg}");
            return EXIT_USAGE;
        }
        Err(CellResolveError::Runtime(msg)) => {
            eprintln!("{msg}");
            return EXIT_RUNTIME;
        }
    };

    let mut final_para = para;
    if !dry_run {
        let result = match doc.move_table_offset_native(sec, para, ctrl, dh, dv) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: 표 이동 실패 - {}", e);
                return EXIT_RUNTIME;
            }
        };
        final_para = serde_json::from_str::<serde_json::Value>(&result)
            .ok()
            .and_then(|v| v["ppi"].as_u64())
            .map(|n| n as usize)
            .unwrap_or(para);
    }

    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_moved.{}", stem, out_format.ext())
    });

    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if !dry_run {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if let Err(e) = fs::write(&output_path, &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    let changed_pages = if dry_run {
        serde_json::Value::Null
    } else {
        match doc.pages_covering_paragraphs(&[(sec, final_para)]) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    };

    if json_mode {
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "table": table_no,
            "deltaH": dh,
            "deltaV": dv,
            "dryRun": dry_run,
            "changedPages": changed_pages,
        });
        if !dry_run {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "이동 예정: {} 표{} (dh={}, dv={})",
            file_path, table_no, dh, dv
        );
    } else {
        println!(
            "표 이동 완료: {} → {} — 표{} (dh={}, dv={})",
            file_path, output_path, table_no, dh, dv
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}



/// `edit transpose-table` — 표 전체의 행/열을 제자리에서 바꾼다 (upstream #5185/#5192
/// 계열 선별 이식 — 코어 로직은 기존 `transpose_table_cells_in_place_native` 재사용).
///
/// 병합 셀이 있는 표는 대상이 아니다(코어가 `Err`로 거부) — 부분 선택 행/열 바꿈은
/// 클립보드 경로(`copy_table_cells_transposed_native`/`paste_table_cells_transposed_native`,
/// 스튜디오 전용)가 다루며, 이번 라운드 CLI 배선 범위 밖이다.
pub(crate) fn edit_transpose_table(args: &[String]) -> i32 {
    const USAGE: &str = "사용법: rhwp edit transpose-table <파일> --table <번호> [-o <출력>] [--dry-run] [--verify] [--json]";

    let mut file_path: Option<&str> = None;
    let mut table_arg: Option<usize> = None;
    let mut out_path: Option<String> = None;
    let mut dry_run = false;
    let mut json_mode = false;
    let mut verify_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--table" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) => table_arg = Some(v),
                    None => {
                        eprintln!("오류: --table 뒤에 0 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
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

    let (Some(file_path), Some(table_no)) = (file_path, table_arg) else {
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match load_document(&bytes) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let (sec, para, ctrl) = match resolve_table_index(doc.document(), table_no) {
        Ok(v) => v,
        Err(CellResolveError::Usage(msg)) => {
            eprintln!("{msg}");
            return EXIT_USAGE;
        }
        Err(CellResolveError::Runtime(msg)) => {
            eprintln!("{msg}");
            return EXIT_RUNTIME;
        }
    };
    let Some((source_rows, source_cols)) = table_dimensions(doc.document(), sec, para, ctrl)
    else {
        eprintln!("오류: 표 컨트롤 좌표 해석 실패 (내부 불일치).");
        return EXIT_RUNTIME;
    };

    if !dry_run {
        if let Err(e) = doc.transpose_table_cells_in_place_native(sec, para, ctrl) {
            eprintln!("오류: 표 행/열 바꿈 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    }

    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_transposed.{}", stem, out_format.ext())
    });

    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if !dry_run {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if let Err(e) = fs::write(&output_path, &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    let changed_pages = if dry_run {
        serde_json::Value::Null
    } else {
        match doc.pages_covering_paragraphs(&[(sec, para)]) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    };

    if json_mode {
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "table": table_no,
            "sourceRows": source_rows,
            "sourceCols": source_cols,
            "targetRows": source_cols,
            "targetCols": source_rows,
            "dryRun": dry_run,
            "changedPages": changed_pages,
        });
        if !dry_run {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "행/열 바꿈 예정: {} 표{} ({}x{} → {}x{})",
            file_path, table_no, source_rows, source_cols, source_cols, source_rows
        );
    } else {
        println!(
            "표 행/열 바꿈 완료: {} → {} — 표{} ({}x{} → {}x{})",
            file_path, output_path, table_no, source_rows, source_cols, source_cols, source_rows
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}



/// `edit set-column-widths` — 표의 열별 폭(HWPUNIT)을 절대값으로 설정한다 (upstream
/// #5185/#5192 계열 선별 이식 — 코어 로직은 기존 `set_table_column_widths_native` 재사용).
///
/// `--widths`는 쉼표로 구분한 HWPUNIT 정수 목록이며 개수가 표의 열 수와 정확히 같아야
/// 한다 — 일치하지 않으면 `--dry-run` 에서도 usage 오류로 미리 알린다. 표 전체 폭은
/// 입력한 폭의 합이 된다(페이지를 넘지 않으려면 합을 본문 폭 이하로 맞추거나
/// `edit set-cell` 저장 후 `fit-table-to-page`류 후속 조정을 쓴다).
pub(crate) fn edit_set_column_widths(args: &[String]) -> i32 {
    const USAGE: &str = "사용법: rhwp edit set-column-widths <파일> --table <번호> --widths <W1,W2,...> [-o <출력>] [--dry-run] [--verify] [--json]";

    let mut file_path: Option<&str> = None;
    let mut table_arg: Option<usize> = None;
    let mut widths_arg: Option<Vec<u32>> = None;
    let mut out_path: Option<String> = None;
    let mut dry_run = false;
    let mut json_mode = false;
    let mut verify_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--table" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(v) => table_arg = Some(v),
                    None => {
                        eprintln!("오류: --table 뒤에 0 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--widths" => {
                i += 1;
                match args.get(i) {
                    Some(v) => {
                        let parsed: Result<Vec<u32>, _> =
                            v.split(',').map(|s| s.trim().parse::<u32>()).collect();
                        match parsed {
                            Ok(widths) if !widths.is_empty() => widths_arg = Some(widths),
                            _ => {
                                eprintln!(
                                    "오류: --widths 는 쉼표로 구분한 0 이상의 정수 목록이어야 합니다 (예: 2000,3000,2000)."
                                );
                                return EXIT_USAGE;
                            }
                        }
                    }
                    None => {
                        eprintln!("오류: --widths 뒤에 쉼표로 구분한 HWPUNIT 목록이 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
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

    let (Some(file_path), Some(table_no), Some(widths)) = (file_path, table_arg, widths_arg)
    else {
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match load_document(&bytes) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    let (sec, para, ctrl) = match resolve_table_index(doc.document(), table_no) {
        Ok(v) => v,
        Err(CellResolveError::Usage(msg)) => {
            eprintln!("{msg}");
            return EXIT_USAGE;
        }
        Err(CellResolveError::Runtime(msg)) => {
            eprintln!("{msg}");
            return EXIT_RUNTIME;
        }
    };
    let Some((_, col_count)) = table_dimensions(doc.document(), sec, para, ctrl) else {
        eprintln!("오류: 표 컨트롤 좌표 해석 실패 (내부 불일치).");
        return EXIT_RUNTIME;
    };
    // [set_column_widths 계약] 개수 불일치는 코어도 거부하지만, --dry-run 에서도
    // 같은 오류를 미리 보게 해 실행 전에 잡는다(set-cell 의 격자 범위 사전검증과 동형).
    if widths.len() != col_count as usize {
        eprintln!(
            "오류: --widths 개수 {} 가 표 {} 의 열 수 {} 와 다릅니다.",
            widths.len(),
            table_no,
            col_count
        );
        return EXIT_USAGE;
    }

    let mut col_count_result = col_count as u64;
    let mut table_width_result: u64 = widths.iter().map(|w| *w as u64).sum();
    if !dry_run {
        let result = match doc.set_table_column_widths_native(sec, para, ctrl, widths.clone()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: 열 폭 설정 실패 - {}", e);
                return EXIT_RUNTIME;
            }
        };
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&result) {
            col_count_result = v["colCount"].as_u64().unwrap_or(col_count_result);
            table_width_result = v["tableWidth"].as_u64().unwrap_or(table_width_result);
        }
    }

    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_colwidths.{}", stem, out_format.ext())
    });

    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if !dry_run {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if let Err(e) = fs::write(&output_path, &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    let changed_pages = if dry_run {
        serde_json::Value::Null
    } else {
        match doc.pages_covering_paragraphs(&[(sec, para)]) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    };

    if json_mode {
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "table": table_no,
            "widths": widths,
            "colCount": col_count_result,
            "tableWidth": table_width_result,
            "dryRun": dry_run,
            "changedPages": changed_pages,
        });
        if !dry_run {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "열 폭 설정 예정: {} 표{} widths={:?}",
            file_path, table_no, widths
        );
    } else {
        println!(
            "표 열 폭 설정 완료: {} → {} — 표{} widths={:?} (표 전체 폭 {})",
            file_path, output_path, table_no, widths, table_width_result
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}



/// 그림의 원본 픽셀 크기 — 전체 디코드 없이 헤더만 읽는다.
///
/// 확장자는 거짓말할 수 있으므로 매직 바이트로 형식을 다시 판정한다. 알아보지 못하면
/// `None` — 호출부가 인자 오류(exit 2)로 끊는다.
pub(crate) fn insert_image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    use image::ImageFormat;

    let format = image::guess_format(bytes).ok()?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Tiff
    ) {
        return None;
    }
    let (width, height) = image::ImageReader::with_format(std::io::Cursor::new(bytes), format)
        .into_dimensions()
        .ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}



/// `--page` 가 가리키는 쪽의 **앵커 문단**(구역 인덱스, 문단 인덱스).
///
/// 용지 기준(Paper-relative) floating 그림은 앵커 문단이 놓인 쪽에 그려진다. 그래서
/// "몇 쪽" 을 "어느 문단" 으로 옮겨야 하는데, 그 환산은 이미 조판 결과가 알고 있다 —
/// 기존 진단 질의 `dump_page_items_json` 을 그대로 읽어 그 쪽의 첫 본문 항목을 고른다
/// (새 조판 로직 0). 미주(`isEndnote`)는 구역 뒤에 합성된 문단이라 앵커로 쓰지 않는다.
pub(crate) fn insert_image_page_anchor(
    doc: &rhwp::wasm_api::HwpDocument,
    page: u32,
) -> Option<(usize, usize)> {
    let empty: Vec<serde_json::Value> = Vec::new();
    let pages = doc.dump_page_items_json(Some(page));
    let page_json = pages.as_array()?.first()?;
    let section = page_json["section"].as_u64()? as usize;

    for column in page_json["columns"].as_array().unwrap_or(&empty) {
        for item in column["items"].as_array().unwrap_or(&empty) {
            if item["isEndnote"] == true {
                continue;
            }
            if let Some(para) = item["paraIndex"].as_u64() {
                return Some((section, para as usize));
            }
        }
    }
    // 항목이 하나도 없는 쪽(어울림 문단·감춘 빈 줄만 귀속된 쪽)은 extras 로 온다.
    for extra in page_json["extras"].as_array().unwrap_or(&empty) {
        if let Some(para) = extra["paraIndex"].as_u64() {
            return Some((section, para as usize));
        }
    }
    None
}



/// `edit insert-image` — 도장·서명 같은 그림을 쪽 좌표에 붙인다 (#3719 §6-5).
///
/// 실물 서식 제출의 마지막 조각이다. 채워 넣은 서식에 직인·서명 이미지를 얹지 못하면
/// 사람이 한 번 더 한컴을 열어야 하고, 그 순간 자동화 사슬이 끊긴다.
///
/// 새 삽입 로직을 만들지 않는다 — 검증된 코어 `insert_picture_native` 의 **본문 floating
/// 분기**(용지 기준 offset, `treat_as_char=false`, 한컴 native 기본값)를 그대로 쓴다.
/// 인자 파싱·저장·봉투·`--verify`·`changedPages` 는 `edit set-cell` 과 같은 형태다.
///
/// **길이 단위는 전부 HWPUNIT(1/7200 inch)** 이다 — px 로 오해하면 도장이 점만 하게
/// 찍히거나 아예 안 보인다. A4 세로는 59528 × 84188 HWPUNIT.
pub(crate) fn edit_insert_image(args: &[String]) -> i32 {
    const USAGE: &str = "사용법: rhwp edit insert-image <파일> --image <그림> [--page N] [--x N --y N] [--width N --height N] [-o <출력>] [--dry-run] [--verify] [--json]";

    let mut file_path: Option<&str> = None;
    let mut image_path: Option<&str> = None;
    let mut page_arg: u32 = 0;
    let mut x_hu: u32 = 0;
    let mut y_hu: u32 = 0;
    let mut width_arg: Option<u32> = None;
    let mut height_arg: Option<u32> = None;
    let mut out_path: Option<String> = None;
    let mut dry_run = false;
    let mut json_mode = false;
    // [#3702] 저장 직후 자기검증 — 판정은 데이터, 차이 시 exit 3.
    let mut verify_mode = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--image" => {
                i += 1;
                match args.get(i) {
                    Some(v) => image_path = Some(v),
                    None => {
                        eprintln!("오류: --image 뒤에 그림 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--page" | "--x" | "--y" | "--width" | "--height" => {
                let name = args[i].clone();
                // 단위를 오류 문구에도 박아 둔다 — px 로 넣으면 도장이 사라진다.
                let unit = if name == "--page" {
                    " (0부터)"
                } else {
                    " (HWPUNIT, 1/7200 inch)"
                };
                i += 1;
                let Some(v) = args.get(i) else {
                    eprintln!("오류: {name} 뒤에 0 이상의 정수가 필요합니다{unit}.");
                    return EXIT_USAGE;
                };
                let Ok(value) = v.parse::<u32>() else {
                    eprintln!("오류: {name} 뒤에 0 이상의 정수가 필요합니다{unit}: {v}");
                    return EXIT_USAGE;
                };
                match name.as_str() {
                    "--page" => page_arg = value,
                    "--x" => x_hu = value,
                    "--y" => y_hu = value,
                    "--width" => width_arg = Some(value),
                    _ => height_arg = Some(value),
                }
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--dry-run" => dry_run = true,
            "--json" => json_mode = true,
            "--verify" => verify_mode = true,
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

    let (Some(file_path), Some(image_path)) = (file_path, image_path) else {
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };
    for (name, value) in [("--width", width_arg), ("--height", height_arg)] {
        if value == Some(0) {
            eprintln!("오류: {name} 는 1 이상이어야 합니다 (HWPUNIT, 1/7200 inch).");
            return EXIT_USAGE;
        }
    }

    // ── 그림 선검증 — 문서를 읽기 전에 끊는다 ──
    // 지원하지 않는 형식은 **인자 문제**다(런타임 실패가 아니다) → exit 2.
    let image_ext = Path::new(image_path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !INSERT_IMAGE_FORMATS.contains(&image_ext.as_str()) {
        eprintln!(
            "오류: 지원하지 않는 그림 형식입니다 - {} (지원: {})",
            if image_ext.is_empty() {
                "확장자 없음".to_string()
            } else {
                image_ext.clone()
            },
            INSERT_IMAGE_FORMATS.join(", ")
        );
        return EXIT_USAGE;
    }
    let image_bytes = match fs::read(image_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 그림 파일을 읽을 수 없습니다 - {}: {}", image_path, e);
            return EXIT_RUNTIME;
        }
    };
    // 확장자만 믿지 않는다 — 내용이 그림이 아니면 원본 픽셀 크기를 못 재고,
    // 크기를 모르면 배치 좌표가 의미를 잃는다.
    let Some((natural_w_px, natural_h_px)) = insert_image_dimensions(&image_bytes) else {
        eprintln!(
            "오류: 그림 형식을 알아볼 수 없습니다 - {} (지원: {})",
            image_path,
            INSERT_IMAGE_FORMATS.join(", ")
        );
        return EXIT_USAGE;
    };

    // 크기 결정: 둘 다 없으면 원본 픽셀(96dpi 환산), 하나만 주면 원본 비율 유지.
    // 어느 쪽이든 최종 값은 봉투에 그대로 실어 "조용한 보정" 이 없게 한다.
    let (width_hu, height_hu) = match (width_arg, height_arg) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (
            w,
            ((w as u64 * natural_h_px as u64) / natural_w_px as u64).max(1) as u32,
        ),
        (None, Some(h)) => (
            ((h as u64 * natural_w_px as u64) / natural_h_px as u64).max(1) as u32,
            h,
        ),
        (None, None) => (
            natural_w_px.saturating_mul(HWPUNIT_PER_PX),
            natural_h_px.saturating_mul(HWPUNIT_PER_PX),
        ),
    };
    // 코어는 offset·크기를 i32/u32 로 다룬다. 범위를 넘는 값이 조용히 감기면 도장이
    // 엉뚱한 곳에 찍히므로 인자 오류로 끊는다.
    for (name, value) in [
        ("--x", x_hu),
        ("--y", y_hu),
        ("--width", width_hu),
        ("--height", height_hu),
    ] {
        if value > i32::MAX as u32 {
            eprintln!(
                "오류: {name} 값이 너무 큽니다 (HWPUNIT 최대 {}): {value}",
                i32::MAX
            );
            return EXIT_USAGE;
        }
    }

    let bytes = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let mut doc = match rhwp::wasm_api::HwpDocument::from_bytes(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: HWP 파싱 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    let page_count = doc.page_count();
    if page_arg >= page_count {
        eprintln!(
            "오류: 페이지 번호가 범위를 벗어났습니다 (0~{}): {page_arg}",
            page_count.saturating_sub(1)
        );
        return EXIT_USAGE;
    }
    let Some((sec, para)) = insert_image_page_anchor(&doc, page_arg) else {
        eprintln!("오류: {page_arg}쪽(0 기준)에서 그림을 붙일 본문 문단을 찾지 못했습니다.");
        return EXIT_RUNTIME;
    };

    // [#3480 과 같은 취지] 쪽 밖으로 나가면 **조용히 자르지 않는다**. 에이전트는 렌더
    // 결과를 보지 않으므로 신호가 없으면 잘려 나간 도장을 완성본으로 판단한다.
    let page_def = &doc.document().sections[sec].section_def.page_def;
    let (paper_w, paper_h) = if page_def.landscape {
        (page_def.height as i64, page_def.width as i64)
    } else {
        (page_def.width as i64, page_def.height as i64)
    };
    let right = x_hu as i64 + width_hu as i64;
    let bottom = y_hu as i64 + height_hu as i64;
    let overflow = if right > paper_w || bottom > paper_h {
        Some(serde_json::json!({
            "page": page_arg,
            "paperWidthHu": paper_w,
            "paperHeightHu": paper_h,
            "rightHu": right,
            "bottomHu": bottom,
            "overflowXHu": (right - paper_w).max(0),
            "overflowYHu": (bottom - paper_h).max(0),
        }))
    } else {
        None
    };

    let mut bin_data_id = serde_json::Value::Null;
    if !dry_run {
        // 그림 설명(대체 텍스트)은 파일명 — 한컴이 개체 속성에 보여 주는 값이다.
        let description = Path::new(image_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let inserted = match doc.insert_picture_native(
            sec,
            para,
            0,
            &[],
            &image_bytes,
            width_hu,
            height_hu,
            natural_w_px,
            natural_h_px,
            &image_ext,
            &description,
            Some(x_hu as i32),
            Some(y_hu as i32),
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: 그림 삽입 실패 - {}", e);
                // 실패 시 원본 불변 — 출력 파일을 쓰지 않고 즉시 끝낸다.
                return EXIT_RUNTIME;
            }
        };
        // binDataId 는 새 조회 API 없이 방금 삽입한 컨트롤에서 직접 읽는다 —
        // 같은 그림을 다시 참조하거나(도장 재사용) 산출물을 감사할 때 쓰는 주소다.
        let ctrl_idx = serde_json::from_str::<serde_json::Value>(&inserted)
            .ok()
            .and_then(|v| v["controlIdx"].as_u64())
            .unwrap_or_default() as usize;
        if let Some(rhwp::model::control::Control::Picture(picture)) = doc
            .document()
            .sections
            .get(sec)
            .and_then(|s| s.paragraphs.get(para))
            .and_then(|p| p.controls.get(ctrl_idx))
        {
            bin_data_id = serde_json::json!(picture.image_attr.bin_data_id);
        }
    }

    // [#3383] 입력 형식을 보존한다 — 기본 확장자도 산출 형식을 따른다.
    let out_format = edit_output_format(&bytes, out_path.as_deref());
    let output_path = out_path.unwrap_or_else(|| {
        let stem = Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        format!("{}_image.{}", stem, out_format.ext())
    });

    let mut verify_report = serde_json::Value::Null;
    let mut verify_failed = false;
    if !dry_run {
        let out_bytes = match edit_serialize(&mut doc, out_format) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "오류: {} 직렬화 실패 - {}",
                    out_format.label().to_uppercase(),
                    e
                );
                return EXIT_RUNTIME;
            }
        };
        if let Err(e) = fs::write(&output_path, &out_bytes) {
            eprintln!("오류: 출력 쓰기 실패 - {}: {}", output_path, e);
            return EXIT_RUNTIME;
        }
        // [#3702] 저장 직후 자기검증 — 편집 후 IR ↔ 저장본 재파싱 IR.
        if verify_mode {
            let cross = out_format == EditOutputFormat::Hwp
                && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
            let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
            verify_report = report;
            verify_failed = failed;
        }
    }

    // [#3712] 눈검증 대상 페이지 — 앵커 문단이 걸친 쪽 전부.
    let changed_pages = if dry_run {
        serde_json::Value::Null
    } else {
        match doc.pages_covering_paragraphs(&[(sec, para)]) {
            Some(pages) => serde_json::json!(pages),
            None => serde_json::Value::Null,
        }
    };

    if json_mode {
        let mut envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "image": image_path,
            "page": page_arg,
            "x": x_hu,
            "y": y_hu,
            "width": width_hu,
            "height": height_hu,
            "binDataId": bin_data_id,
            "dryRun": dry_run,
            "changedPages": changed_pages,
            "overflow": overflow.clone().map(|o| vec![o]).unwrap_or_default(),
        });
        if !dry_run {
            envelope["output"] = serde_json::Value::String(output_path.clone());
            envelope["outputFormat"] = serde_json::Value::String(out_format.label().to_string());
            envelope["verify"] = verify_report.clone();
        }
        // [#3885] 이 봉투의 값은 전부 호출자 인자·엔진 판정이라 문서 유래 경로가
        // 없지만, 표지 자체는 항상 싣는다 — 키 부재는 "안전"이 아니라 "판정 안 함"
        // 으로 읽어야 하기 때문이다(S1).
        println!("{}", provenance::marked(envelope, "edit"));
        if verify_failed {
            process::exit(3);
        }
        return EXIT_OK;
    }

    if dry_run {
        println!(
            "배치 예정: {} {}쪽 ({}, {}) 크기 {}×{} HWPUNIT ← {} (원본 {}×{}px)",
            file_path,
            page_arg,
            x_hu,
            y_hu,
            width_hu,
            height_hu,
            image_path,
            natural_w_px,
            natural_h_px
        );
    } else {
        println!(
            "그림 삽입 완료: {} → {} — {}쪽 ({}, {}) 크기 {}×{} HWPUNIT ← {} (원본 {}×{}px)",
            file_path,
            output_path,
            page_arg,
            x_hu,
            y_hu,
            width_hu,
            height_hu,
            image_path,
            natural_w_px,
            natural_h_px
        );
    }
    if overflow.is_some() {
        eprintln!(
            "경고: 그림이 쪽 밖으로 나갑니다 (용지 {}×{} HWPUNIT, 오른쪽 {} 아래 {}) — 상세는 --json 의 overflow",
            paper_w, paper_h, right, bottom
        );
    }
    if verify_failed {
        eprintln!("검증 실패(--verify): 저장본 재파싱 IR 차이 — 상세는 --json 또는 ir-diff");
        process::exit(3);
    }
    EXIT_OK
}



/// rhwp 는 이미 필드에 값을 **쓸 수** 있는데(`set_field_value_by_name`) 조회 API 는
/// WASM/스튜디오 경로에만 있어, 브라우저 밖 에이전트는 "이 서식이 무엇을 요구하는지"
/// 알 방법이 없었다. 기존 `collect_all_fields()` 를 그대로 노출한다(라이브러리 무변경).
pub(crate) fn show_fields(args: &[String]) -> i32 {
    let mut file_path: Option<&str> = None;
    let mut json_mode = false;
    for a in args {
        match a.as_str() {
            "--json" => json_mode = true,
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => file_path = Some(other),
        }
    }

    let Some(file_path) = file_path else {
        eprintln!("사용법: rhwp fields <파일.hwp|파일.hwpx> [--json]");
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

    let fields = collect_field_records(&doc);

    if json_mode {
        let envelope = fields_json_value(file_path, &fields);
        println!("{envelope}");
        return EXIT_OK;
    }

    println!("문서 로드: {} (필드 {}개)", file_path, fields.len());
    for f in &fields {
        let name = f["name"].as_str().unwrap_or("");
        let label = if name.is_empty() {
            "(이름 없음)"
        } else {
            name
        };
        println!(
            "  [{}] {} = {:?}{}",
            f["fieldType"].as_str().unwrap_or("?"),
            label,
            f["value"].as_str().unwrap_or(""),
            if f["editableInForm"] == true {
                ""
            } else {
                " (서식 편집 불가)"
            }
        );
    }
    EXIT_OK
}
