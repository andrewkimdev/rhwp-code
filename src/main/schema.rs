//! 스키마·온톨로지·에이전트 매니페스트 export 명령 (main.rs에서 무변동 이동)
use super::*;

/// [#3762] `export-ir-schema` — 공개 IR 의 JSON Schema 를 낸다 (M18 바인딩 착수 조건).
///
/// 문서를 입력으로 받지 않는다 — 스키마는 **타입의 자기서술**이지 특정 문서의
/// 속성이 아니다. capabilities 가 명령 표면을 설명하듯, 이 명령은 문서 모델을
/// 설명한다. 외부 바인딩 세대가 코드 생성의 단일 출처로 쓴다.
pub(crate) fn cmd_export_ir_schema(args: &[String]) -> i32 {
    let mut out_path: Option<&str> = None;
    let mut json_mode = false;
    let mut bare = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            // 봉투 없이 스키마 본문만 — JSON Schema 도구에 바로 먹이려는 용도.
            "--bare" => bare = true,
            "-o" | "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.as_str()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {}", other);
                return EXIT_USAGE;
            }
        }
        i += 1;
    }

    let payload = if bare {
        // --bare 는 JSON Schema 검증기에 그대로 먹이는 본문이다 — 봉투 표지를 섞지 않는다.
        rhwp::ir_schema::ir_schema()
    } else {
        // [#3885] "표지는 항상 실린다" — 문서를 열지 않는 명령의 봉투도
        // untrustedContent:false 를 명시한다. 키 부재는 "안전"이 아니라
        // "이 빌드는 표지를 모른다"로 읽히기 때문이다.
        provenance::marked(rhwp::ir_schema::envelope(), "export-ir-schema")
    };
    let text = match serde_json::to_string_pretty(&payload) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("오류: 스키마 직렬화 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    if let Some(path) = out_path {
        if let Err(e) = fs::write(path, text.as_bytes()) {
            eprintln!("오류: 스키마를 쓸 수 없습니다 - {}: {}", path, e);
            return EXIT_RUNTIME;
        }
        if json_mode {
            // 파일로 뺐어도 stdout 은 기계 계약을 유지한다 — 어디에 썼는지 알려준다.
            println!(
                "{}",
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                        "irSchemaVersion": rhwp::ir_schema::IR_SCHEMA_VERSION,
                        "output": path,
                        "bytes": text.len(),
                    }),
                    "export-ir-schema"
                )
            );
        } else {
            println!("IR 스키마 저장: {} ({} bytes)", path, text.len());
        }
        return EXIT_OK;
    }

    println!("{text}");
    EXIT_OK
}

/// [#3719 §6-4] `export-plan-schema` — `run` 계획서 문법의 JSON Schema 를 낸다.
///
/// 문서를 입력으로 받지 않는다 — 스키마는 **계획서 문법의 자기서술**이지 특정 문서의
/// 속성이 아니다. `run --json` 이 이미 쓴 계획을 검사한다면, 이 명령은 계획을 **쓰기
/// 전에** 읽는 정답지다. 필드명을 지어내고 `invalid[]` 로 되돌아오는 왕복이 계획 생성
/// 실패의 대부분이라, 그 왕복을 없애는 것이 목적이다.
pub(crate) fn cmd_export_plan_schema(args: &[String]) -> i32 {
    let mut out_path: Option<&str> = None;
    let mut json_mode = false;
    let mut bare = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            // 봉투 없이 스키마 본문만 — JSON Schema 검증기에 바로 먹이려는 용도.
            "--bare" => bare = true,
            "-o" | "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.as_str()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {}", other);
                return EXIT_USAGE;
            }
        }
        i += 1;
    }

    let payload = if bare {
        // --bare 는 JSON Schema 검증기에 그대로 먹이는 본문이다 — 봉투 표지를 섞지 않는다.
        rhwp::plan_schema::plan_schema()
    } else {
        // [#3787 S1] "표지는 항상 실린다" — 문서를 열지 않는 명령의 봉투도
        // untrustedContent:false 를 명시한다는 것이 capabilities 의 선언이다.
        provenance::marked(rhwp::plan_schema::envelope(), "export-plan-schema")
    };
    let text = match serde_json::to_string_pretty(&payload) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("오류: 스키마 직렬화 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    if let Some(path) = out_path {
        if let Err(e) = fs::write(path, text.as_bytes()) {
            eprintln!("오류: 스키마를 쓸 수 없습니다 - {}: {}", path, e);
            return EXIT_RUNTIME;
        }
        if json_mode {
            // 파일로 뺐어도 stdout 은 기계 계약을 유지한다 — 어디에 썼는지 알려준다.
            println!(
                "{}",
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                        "planSchemaVersion": rhwp::plan_schema::PLAN_SCHEMA_VERSION,
                        "output": path,
                        "bytes": text.len(),
                    }),
                    "export-plan-schema"
                )
            );
        } else {
            println!("계획 스키마 저장: {} ({} bytes)", path, text.len());
        }
        return EXIT_OK;
    }

    println!("{text}");
    EXIT_OK
}

/// [#3776] `export-capabilities-schema` — capabilities 자체의 JSON Schema 를 낸다.
pub(crate) fn cmd_export_capabilities_schema(args: &[String]) -> i32 {
    let mut out_path: Option<&str> = None;
    let mut json_mode = false;
    let mut bare = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--bare" => bare = true,
            "-o" | "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.as_str()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {}", other);
                return EXIT_USAGE;
            }
        }
        i += 1;
    }

    let payload = if bare {
        // --bare 는 JSON Schema 검증기에 그대로 먹이는 본문이다 — 봉투 표지를 섞지 않는다.
        rhwp::capabilities_schema::capabilities_schema()
    } else {
        // [#3885] export-ir-schema 와 같은 사유 — 문서를 열지 않아도 표지는 싣는다.
        provenance::marked(
            rhwp::capabilities_schema::envelope(),
            "export-capabilities-schema",
        )
    };
    let text = match serde_json::to_string_pretty(&payload) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("오류: 스키마 직렬화 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    if let Some(path) = out_path {
        if let Err(e) = fs::write(path, text.as_bytes()) {
            eprintln!("오류: 스키마를 쓸 수 없습니다 - {}: {}", path, e);
            return EXIT_RUNTIME;
        }
        if json_mode {
            println!(
                "{}",
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                        "capabilitiesSchemaVersion":
                            rhwp::capabilities_schema::CAPABILITIES_SCHEMA_VERSION,
                        "output": path,
                        "bytes": text.len(),
                    }),
                    "export-capabilities-schema"
                )
            );
        } else {
            println!("capabilities 스키마 저장: {} ({} bytes)", path, text.len());
        }
        return EXIT_OK;
    }

    println!("{text}");
    EXIT_OK
}

/// [#3907 O1] `export-ontology` — 자기서술에서 JSON-LD 온톨로지를 기계 유도한다.
///
/// 문서를 입력으로 받지 않는다 — 온톨로지는 rhwp 라는 **도구 자신**(IR 타입·명령
/// 표면·신뢰 경계)의 서술이지 특정 문서의 속성이 아니다. 유도 원천은 전부 같은
/// 크레이트의 단일 출처 함수다: `ir_schema()`·`capabilities_value()`·
/// `mcp_tool_definitions()`·`provenance::MAP`. 손 나열 상수가 없으므로 원천이
/// 바뀌면 온톨로지가 함께 바뀐다 — 드리프트 구조적 불가능이 이 명령의 논지다.
/// 문서 인스턴스 모드(O2)는 후속이다.
pub(crate) fn cmd_export_ontology(args: &[String]) -> i32 {
    let mut out_path: Option<&str> = None;
    let mut json_mode = false;
    let mut bare = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            // 봉투 없이 JSON-LD 본문만 — RDF/JSON-LD 도구에 바로 먹이려는 용도.
            "--bare" => bare = true,
            "-o" | "--out" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v.as_str()),
                    None => {
                        eprintln!("오류: -o 뒤에 출력 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {}", other);
                return EXIT_USAGE;
            }
        }
        i += 1;
    }

    let caps = capabilities_value();
    let tools = mcp_tool_definitions();
    let payload = if bare {
        // --bare 는 JSON-LD 처리기에 그대로 먹이는 본문이다 — 봉투 표지를 섞지 않는다.
        rhwp::ontology::ontology(&caps, &tools)
    } else {
        // [#3885] "표지는 항상 실린다" — 문서를 열지 않는 명령의 봉투도
        // untrustedContent:false 를 명시한다.
        provenance::marked(rhwp::ontology::envelope(&caps, &tools), "export-ontology")
    };
    let text = match serde_json::to_string_pretty(&payload) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("오류: 온톨로지 직렬화 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    if let Some(path) = out_path {
        if let Err(e) = fs::write(path, text.as_bytes()) {
            eprintln!("오류: 온톨로지를 쓸 수 없습니다 - {}: {}", path, e);
            return EXIT_RUNTIME;
        }
        if json_mode {
            // 파일로 뺐어도 stdout 은 기계 계약을 유지한다 — 어디에 썼는지 알려준다.
            println!(
                "{}",
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                        "ontologyVersion": rhwp::ontology::ONTOLOGY_VERSION,
                        "output": path,
                        "bytes": text.len(),
                    }),
                    "export-ontology"
                )
            );
        } else {
            println!("온톨로지 저장: {} ({} bytes)", path, text.len());
        }
        return EXIT_OK;
    }

    println!("{text}");
    EXIT_OK
}

/// [#3828 B2] `export-agent-manifest` 조립 코어 — capabilities·irSchema·provenanceMap·
/// planSchema 를 왕복 1회로 묶는다.
///
/// 각 서브필드는 해당 명령의 기존 산출 함수를 그대로 불러 조립만 한다 — 스키마·지도
/// 로직을 여기서 다시 만들지 않는다. `missingAxes` 는 네 축이 모두 실린 지금 빈
/// 배열이지만 필드 자체는 남긴다 — 앞으로 축이 늘 때 "아직 없는 축"을 이 배열로
/// 알리는 것이 B2 의 계약이고, null 로 채우면 "값이 비었다"와 "명령이 아직 없다"를
/// 소비자가 구분할 수 없다.
pub(crate) fn agent_manifest_value(bare: bool) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "capabilities".to_string(),
        provenance::marked(capabilities_value(), "capabilities"),
    );
    fields.insert("irSchema".to_string(), rhwp::ir_schema::ir_schema());
    fields.insert(
        "provenanceMap".to_string(),
        provenance::marked(
            provenance::map_json(&rhwp::version()),
            "export-provenance-map",
        ),
    );
    // [#3808] planSchema 축 — irSchema 처럼 bare 본문을 싣는다. 본문이 `$id`·
    // `planSchemaVersion` 을 자체 내장하므로 봉투 메타를 중복하지 않는다.
    fields.insert("planSchema".to_string(), rhwp::plan_schema::plan_schema());
    fields.insert("missingAxes".to_string(), serde_json::json!([]));

    if bare {
        return serde_json::Value::Object(fields);
    }
    let mut envelope = serde_json::Map::new();
    envelope.insert(
        "schemaVersion".to_string(),
        serde_json::json!(ENVELOPE_SCHEMA_VERSION),
    );
    envelope.extend(fields);
    serde_json::Value::Object(envelope)
}

/// [#3828 B2] `export-agent-manifest` — 처음 붙는 에이전트가 capabilities →
/// export-ir-schema → export-provenance-map → export-plan-schema 를 각각 따로
/// 호출하던 왕복 4회를 1회로 줄인다.
pub(crate) fn cmd_export_agent_manifest(args: &[String]) -> i32 {
    let mut json_mode = false;
    let mut bare = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json_mode = true,
            "--bare" => bare = true,
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
    }

    let manifest = provenance::marked(agent_manifest_value(bare), "export-agent-manifest");

    if json_mode {
        let text = match serde_json::to_string_pretty(&manifest) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("오류: 매니페스트 직렬화 실패 - {}", e);
                return EXIT_RUNTIME;
            }
        };
        println!("{text}");
        return EXIT_OK;
    }

    println!("rhwp 에이전트 매니페스트 (capabilities + irSchema + provenanceMap 조립)");
    println!();
    println!("  capabilities     포함");
    println!("  irSchema         포함");
    println!("  provenanceMap    포함");
    println!("  planSchema       포함");
    println!();
    println!("기계 계약은 --json 을 쓰세요 (--bare 로 최상위 표지 없이).");
    EXIT_OK
}
