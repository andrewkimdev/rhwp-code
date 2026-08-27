//! governance 모듈 — src/main.rs 에서 무변동 이동
use super::*;

/// `rhwp run <계획.json>` — 계획서를 정적 선검증 → 원자 실행 → 저널로 수행한다.
///
/// 다단 체이닝(호출 사이 상태 유실, 중간 실패의 반편집 문서)이 에이전트 실패의
/// 뿌리라서 절차 대신 **의도(계획서)** 를 받는다. 판정은 전부 데이터다:
/// 선검증 위반 = invalid[] + exit 2(실행 0), verify 단언 실패 = exit 3(디스크
/// 무변경), 성공 = step 저널 + verify + exit 0(단 한 번 저장).
/// [#4378 R24] `--expect-sha256` CAS 대조. 불일치는 "검증 단언 실패" 계열(exit 3,
/// #2707 사전)이다 — 문서가 기대 상태가 아니면 한 바이트도 쓰지 않는다.
pub(crate) fn sha256_hex_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let out = Sha256::digest(bytes);
    let mut hex = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}



/// debug 통합 회귀에서 두 별도 프로세스를 잠금 시도 직전까지 모은다. release
/// binary에는 환경변수 기반 파일 쓰기·대기 경로 자체를 컴파일하지 않는다.
#[cfg(debug_assertions)]
pub(crate) fn cas_test_synchronize_before_lock() -> Result<(), String> {
    let Some(directory) = std::env::var_os("RHWP_INTERNAL_TEST_CAS_BARRIER") else {
        return Ok(());
    };
    let directory = std::path::PathBuf::from(directory);
    fs::write(
        directory.join(format!("arrived-{}", std::process::id())),
        b"",
    )
    .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let arrived = fs::read_dir(&directory)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("arrived-"))
            .count();
        if arrived >= 2 {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err("CAS test barrier 에 두 프로세스가 도착하지 않았습니다".to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}



#[cfg(not(debug_assertions))]
pub(crate) fn cas_test_synchronize_before_lock() -> Result<(), String> {
    Ok(())
}



/// 최초 해시 검사를 통과한 프로세스를 표시한다. 잠금이 사라진 mutation에서는 두
/// marker가 생기고, 정상 구현에서는 첫 writer만 이 경계에 도달한다.
#[cfg(debug_assertions)]
pub(crate) fn cas_test_mark_checked_and_wait() {
    let Some(directory) = std::env::var_os("RHWP_INTERNAL_TEST_CAS_BARRIER") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    let _ = fs::write(
        directory.join(format!("checked-{}", std::process::id())),
        b"",
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while std::time::Instant::now() < deadline {
        let checked = fs::read_dir(&directory)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("checked-"))
            .count();
        if checked >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}



#[cfg(not(debug_assertions))]
pub(crate) fn cas_test_mark_checked_and_wait() {}



/// 기대 해시가 주어졌을 때만 검사한다. 형식 오류는 exit 2, 불일치는 exit 3 을
/// 돌려주고 봉투/진단을 직접 낸다. `None` 이면 통과.
pub(crate) fn check_expect_sha256(
    expect: Option<&str>,
    bytes: &[u8],
    source: &str,
    json_mode: bool,
) -> Option<i32> {
    let expect = expect?;
    let normalized = expect.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|b| b.is_ascii_hexdigit()) {
        eprintln!("오류: --expect-sha256 값은 64자리 16진이어야 합니다: {expect}");
        return Some(EXIT_USAGE);
    }
    let actual = sha256_hex_of(bytes);
    if actual == normalized {
        return None;
    }
    if json_mode {
        let envelope = provenance::marked(
            serde_json::json!({
                "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                "source": source,
                "preconditionFailed": {
                    "kind": "inputSha256",
                    "expected": normalized,
                    "actual": actual,
                },
                "error": "입력 문서가 기대 해시와 다릅니다 — 다른 에이전트/사람이 먼저 바꿨을 수 있습니다. 문서를 다시 읽고 계획을 재수립하세요 (#3905 CAS).",
            }),
            "edit",
        );
        println!("{envelope}");
    } else {
        eprintln!("검증 실패: 입력 해시 불일치 (기대 {normalized} / 실제 {actual}) — 저장하지 않았습니다.");
    }
    Some(3) // #2707: 검증 단언 실패
}



/// [#4391] 작업 영수증 — 계획을 **임시 산출**로 재실행해 (입력·계획·산출) SHA-256
/// 3종을 발급(attest)하거나, 기대 산출 해시와 대조해 타인의 작업 주장을
/// 재현 검증(verify)한다. 전제는 실측된 바이트 결정론(같은 계획 = 같은 산출)이고,
/// 사용자 파일은 절대 건드리지 않는다 — 계획의 output 은 임시 경로로 대체된다.
pub(crate) fn replay_sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}


pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}


pub(crate) fn replay_scratch_dir(tag: &str) -> Result<ReplayScratchDir, String> {
    #[cfg(unix)]
    use std::os::unix::fs::DirBuilderExt;

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    for attempt in 0..128_u16 {
        let candidate = std::env::temp_dir().join(format!(
            "rhwp-replay-{}-{nonce:x}-{tag}-{attempt}",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        match builder.create(&candidate) {
            Ok(()) => return Ok(ReplayScratchDir(candidate)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("사용 가능한 임시 폴더 이름이 없습니다".to_string())
}



/// 해시한 입력 바이트를 임시 파일에 고정하고, 엔진에는 그 스냅샷만 넘긴다.
pub(crate) fn with_replay_input_snapshot<T>(
    plan: &mut serde_json::Value,
    input_bytes: &[u8],
    scratch_dir: &std::path::Path,
    execute: impl FnOnce(&serde_json::Value) -> T,
) -> Result<T, String> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let input = plan["input"]
        .as_str()
        .ok_or_else(|| "계획에 input 이 필요합니다".to_string())?;
    let ext = std::path::Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("hwp");
    let snapshot = scratch_dir.join(format!("input.{ext}"));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&snapshot).map_err(|e| e.to_string())?;
    file.write_all(input_bytes).map_err(|e| e.to_string())?;
    drop(file);
    let original_input = plan["input"].clone();
    plan["input"] = serde_json::json!(snapshot.to_string_lossy());
    let result = execute(plan);
    plan["input"] = original_input;
    Ok(result)
}


pub(crate) fn validated_capsule_plan(capsule: &serde_json::Value) -> Result<(serde_json::Value, u64), String> {
    let plan_text = capsule
        .get("planText")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "planText 없음".to_string())?;
    let expected_plan_sha = capsule["receipt"]["planSha256"]
        .as_str()
        .filter(|value| is_sha256_hex(value))
        .ok_or_else(|| "receipt.planSha256 가 없거나 64자리 16진이 아님".to_string())?;
    let actual_plan_sha = replay_sha256_hex(plan_text.as_bytes());
    if actual_plan_sha != expected_plan_sha {
        return Err("planText 와 receipt.planSha256 불일치".to_string());
    }
    let plan: serde_json::Value =
        serde_json::from_str(plan_text).map_err(|e| format!("planText JSON 파싱 실패: {e}"))?;
    if !plan.is_object() {
        return Err("planText 계획 객체 없음".to_string());
    }
    if capsule.get("plan") != Some(&plan) {
        return Err("plan 과 planText 불일치".to_string());
    }
    let steps = capsule["receipt"]["steps"]
        .as_u64()
        .ok_or_else(|| "receipt.steps 가 음이 아닌 정수가 아님".to_string())?;
    let plan_steps = plan["steps"]
        .as_array()
        .ok_or_else(|| "planText.steps/plan.steps 가 배열이 아님".to_string())?
        .len() as u64;
    if steps != plan_steps {
        return Err(
            "receipt.steps 와 planText.steps 길이 불일치 (plan.steps 길이와 receipt.steps 불일치)"
                .to_string(),
        );
    }
    Ok((plan, steps))
}



/// [#4393] replay·audit 공용 실행 코어 — 계획을 **임시 산출**로 실행해 (산출
/// SHA-256, step 수, 입력 SHA-256)를 얻는다. 임시 파일은 성공·실패 모두
/// 정리한다. 계획의 output 은 이 함수가 임시 경로로 덮어쓴다(호출자는 필요 시
/// 사전 clone).
pub(crate) fn replay_execute_to_temp(
    plan: &mut serde_json::Value,
    tag: &str,
) -> Result<(String, usize, String), (String, i32)> {
    let Some(input) = plan["input"].as_str() else {
        return Err(("계획에 input 이 필요합니다".to_string(), EXIT_USAGE));
    };
    let input_bytes = fs::read(input).map_err(|e| {
        (
            format!("입력을 읽을 수 없습니다 - {input}: {e}"),
            EXIT_RUNTIME,
        )
    })?;
    let input_sha = replay_sha256_hex(&input_bytes);
    let scratch = replay_scratch_dir(tag).map_err(|e| {
        (
            format!("재실행 전용 임시 폴더를 만들 수 없습니다 - {e}"),
            EXIT_RUNTIME,
        )
    })?;
    let ext = plan["output"]
        .as_str()
        .and_then(|o| std::path::Path::new(o).extension().and_then(|e| e.to_str()))
        .unwrap_or("hwp")
        .to_string();
    let temp_out = scratch.0.join(format!("output.{ext}"));
    plan["output"] = serde_json::json!(temp_out.to_string_lossy());
    let (engine_env, engine_code) =
        with_replay_input_snapshot(plan, &input_bytes, &scratch.0, run_plan_engine).map_err(
            |e| {
                (
                    format!("재실행 입력 스냅샷을 만들 수 없습니다 - {e}"),
                    EXIT_RUNTIME,
                )
            },
        )?;
    if engine_code != 0 {
        return Err((
            format!("계획 재실행 실패 (engine exit {engine_code})"),
            engine_code,
        ));
    }
    let bytes = match fs::read(&temp_out) {
        Ok(b) => b,
        Err(e) => {
            return Err((
                format!("재실행 산출을 읽을 수 없습니다 - {e}"),
                EXIT_RUNTIME,
            ));
        }
    };
    let steps = engine_env["steps"].as_array().map(|s| s.len()).unwrap_or(0);
    Ok((replay_sha256_hex(&bytes), steps, input_sha))
}


pub(crate) fn cmd_replay(args: &[String]) -> i32 {
    let mut plan_path: Option<&str> = None;
    let mut plan_inline: Option<&str> = None;
    let mut expected: Option<String> = None;
    let mut capsule_path: Option<String> = None;
    let mut parent_path: Option<String> = None;
    let mut sign_key_path: Option<String> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--plan-json" => {
                i += 1;
                match args.get(i) {
                    Some(v) => plan_inline = Some(v.as_str()),
                    None => {
                        eprintln!("오류: --plan-json 뒤에 계획 JSON 이 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--expect-output-sha256" => {
                i += 1;
                match args.get(i) {
                    Some(v) => expected = Some(v.trim().to_ascii_lowercase()),
                    None => {
                        eprintln!(
                            "오류: --expect-output-sha256 뒤에 64자리 16진 해시가 필요합니다."
                        );
                        return EXIT_USAGE;
                    }
                }
            }
            "--parent" => {
                i += 1;
                match args.get(i) {
                    Some(v) => parent_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: --parent 뒤에 부모 캡슐 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--sign-key" => {
                i += 1;
                match args.get(i) {
                    Some(v) => sign_key_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: --sign-key 뒤에 키 파일 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--capsule" => {
                i += 1;
                match args.get(i) {
                    Some(v) => capsule_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: --capsule 뒤에 저장 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if !other.starts_with("--") && plan_path.is_none() => plan_path = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    if let Some(e) = expected.as_deref() {
        if e.len() != 64 || !e.bytes().all(|b| b.is_ascii_hexdigit()) {
            eprintln!("오류: --expect-output-sha256 값은 64자리 16진이어야 합니다: {e}");
            return EXIT_USAGE;
        }
    }
    if sign_key_path.is_some() && capsule_path.is_none() {
        // [#4509] 서명 대상은 캡슐 파일 바이트다 — 캡슐 없이 서명할 것이 없다.
        eprintln!("오류: --sign-key 는 --capsule 과 함께 사용합니다 (서명 대상 = 캡슐 파일).");
        return EXIT_USAGE;
    }
    let plan_text: String = match (plan_inline, plan_path) {
        (Some(inline), _) => inline.to_string(),
        (None, Some(path)) => match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("오류: 계획을 읽을 수 없습니다 - {path}: {e}");
                return EXIT_RUNTIME;
            }
        },
        (None, None) => {
            eprintln!("사용법: rhwp replay <계획.json> [--plan-json <json>] [--expect-output-sha256 <hex>] [--json]");
            return EXIT_USAGE;
        }
    };
    let plan_sha = replay_sha256_hex(plan_text.as_bytes());
    let mut plan: serde_json::Value = match serde_json::from_str(&plan_text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 계획 JSON 파싱 실패 - {e}");
            return EXIT_USAGE;
        }
    };
    let Some(input) = plan["input"].as_str().map(str::to_string) else {
        eprintln!("오류: 계획에 input 이 필요합니다.");
        return EXIT_USAGE;
    };
    let plan_original = plan.clone();
    let (output_sha, steps, input_sha) = match replay_execute_to_temp(&mut plan, &plan_sha[..12]) {
        Ok(v) => v,
        Err((msg, code)) => {
            if json_mode {
                println!(
                    "{}",
                    provenance::marked(
                        serde_json::json!({ "schemaVersion": ENVELOPE_SCHEMA_VERSION, "error": msg }),
                        "replay",
                    )
                );
            } else {
                eprintln!("{msg} — 영수증 없음");
            }
            return code;
        }
    };
    let reproduced = expected.as_deref().map(|e| e == output_sha);
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "mode": if expected.is_some() { "verify" } else { "attest" },
            "input": input,
            "inputSha256": input_sha,
            "planSha256": plan_sha,
            "outputSha256": output_sha,
            "toolVersion": rhwp::version(),
            "steps": steps,
            "reproduced": reproduced,
            "expectedOutputSha256": expected,
        }),
        "replay",
    );
    if let Some(cp) = capsule_path.as_deref() {
        // [#4393] 작업 캡슐 — 계획(원본 output 보존)+영수증의 자기완결 교환 형식.
        // [#4401] --parent 가 있으면 부모 캡슐 파일의 SHA-256 을 내장해 계보
        // 링크를 만든다 — 부모가 나중에 변조되면 lineage 가 이 해시로 폭로한다.
        let parent_link = match parent_path.as_deref() {
            Some(pp) => {
                let parent_abs = match fs::canonicalize(pp) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("오류: 부모 캡슐을 읽을 수 없습니다 - {pp}: {e}");
                        return EXIT_RUNTIME;
                    }
                };
                if paths_refer_to_same_file(std::path::Path::new(cp), &parent_abs) {
                    eprintln!(
                        "오류: --capsule과 --parent가 같은 기존 파일을 가리킵니다 — 부모 캡슐을 덮어쓰지 않습니다."
                    );
                    return EXIT_USAGE;
                }
                let bytes = match fs::read(&parent_abs) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        eprintln!("오류: 부모 캡슐을 읽을 수 없습니다 - {pp}: {e}");
                        return EXIT_RUNTIME;
                    }
                };
                let capsule_dir = std::path::Path::new(cp)
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or(std::path::Path::new("."));
                let capsule_dir_abs = match fs::canonicalize(capsule_dir) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!(
                            "오류: 캡슐 폴더를 확인할 수 없습니다 - {}: {e}",
                            capsule_dir.display()
                        );
                        return EXIT_RUNTIME;
                    }
                };
                let stored_parent = parent_abs
                    .strip_prefix(&capsule_dir_abs)
                    .map(std::path::PathBuf::from)
                    .unwrap_or(parent_abs);
                serde_json::json!({
                    "capsule": stored_parent.to_string_lossy(),
                    "sha256": replay_sha256_hex(&bytes),
                })
            }
            None => serde_json::Value::Null,
        };
        let capsule = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "kind": "workCapsule",
            "parent": parent_link,
            "plan": plan_original,
            "planText": plan_text,
            "receipt": envelope,
        });
        if let Err(e) = fs::write(
            cp,
            serde_json::to_string_pretty(&capsule).unwrap_or_default(),
        ) {
            eprintln!("오류: 캡슐 저장 실패 - {cp}: {e}");
            return EXIT_RUNTIME;
        }
        if let Some(kp) = sign_key_path.as_deref() {
            // [#4509] 분리 서명 — 방금 쓴 캡슐 "파일 바이트"를 봉인한다. 캡슐
            // 안에 서명을 넣으면 정규화 문제가 생기므로 사이드카가 규약이다.
            let (signing, key_id, _) = match capsule_sign::load_signing_key(kp) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("오류: {e}");
                    return EXIT_RUNTIME;
                }
            };
            let capsule_bytes = match fs::read(cp) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("오류: 서명 대상 캡슐 재독 실패 - {cp}: {e}");
                    return EXIT_RUNTIME;
                }
            };
            let capsule_sha = replay_sha256_hex(&capsule_bytes);
            let sidecar =
                capsule_sign::make_sidecar_json(&signing, &key_id, &capsule_sha, &capsule_bytes);
            let sc_path = capsule_sign::sidecar_path(cp);
            if let Err(e) = fs::write(
                &sc_path,
                serde_json::to_string_pretty(&sidecar).unwrap_or_default(),
            ) {
                eprintln!("오류: 서명 저장 실패 - {sc_path}: {e}");
                return EXIT_RUNTIME;
            }
        }
    }
    if json_mode {
        println!("{envelope}");
    } else {
        println!("작업 영수증 — 입력 {input}");
        println!("  inputSha256:  {input_sha}");
        println!("  planSha256:   {plan_sha}");
        println!(
            "  outputSha256: {output_sha}  (steps {steps}, rhwp v{})",
            rhwp::version()
        );
        if let Some(r) = reproduced {
            println!("  reproduced:   {r}");
        }
    }
    match reproduced {
        Some(false) => 3, // #2707: 검증 단언 실패 — 주장된 산출과 재현 산출이 다르다.
        _ => EXIT_OK,
    }
}


pub(crate) fn collect_audit_capsules(
    entries: impl IntoIterator<Item = std::io::Result<std::path::PathBuf>>,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut capsules = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("폴더 항목 읽기 실패: {e}"))?;
        let is_capsule = path
            .file_name()
            .map(|name| name.to_string_lossy().ends_with(".capsule.json"))
            .unwrap_or(false);
        if is_capsule {
            capsules.push(path);
        }
    }
    capsules.sort();
    Ok(capsules)
}



/// [#4543] 앵커 등재 — 캡슐 해시를 append-only 로그 끝에 더한다.
///
/// 등재 전에 로그 전체의 자기 무결(줄 해시 체인)을 검사한다 — 깨진 로그에
/// append 하는 것은 변조 위에 도장을 찍는 일이라 거부한다(exit 3).
pub(crate) fn cmd_anchor_add(args: &[String]) -> i32 {
    let mut capsule: Option<&str> = None;
    let mut log_path: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--log" => {
                i += 1;
                log_path = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && capsule.is_none() => capsule = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(capsule), Some(log_path)) = (capsule, log_path) else {
        eprintln!("사용법: rhwp anchor add <캡슐.json> --log <anchor.ndjson> [--json]");
        return EXIT_USAGE;
    };
    let bytes = match fs::read(capsule) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 캡슐을 읽을 수 없습니다 - {capsule}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let capsule_sha = replay_sha256_hex(&bytes);
    let log = match anchor_log::load(log_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("오류(로그 무결): {e}");
            return 3; // #2707: 깨진 로그에는 등재하지 않는다.
        }
    };
    let line = anchor_log::make_entry_line(&log, &capsule_sha, &capsule_sign::rfc3339_utc_now());
    let mut data = String::new();
    if !log.entries.is_empty() {
        data.push('\n');
    }
    data.push_str(&line);
    use std::io::Write as _;
    let appended = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .and_then(|mut f| f.write_all(data.as_bytes()));
    if let Err(e) = appended {
        eprintln!("오류: 로그 append 실패 - {log_path}: {e}");
        return EXIT_RUNTIME;
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "log": log_path,
            "capsuleSha256": capsule_sha,
            "seq": log.entries.len(),
        }),
        "anchor",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!("앵커 등재 — seq {} ← {capsule}", log.entries.len());
    }
    EXIT_OK
}



/// [#4543] 머클 체크포인트 — 로그 전체의 루트를 산출한다.
///
/// 공표는 도구 밖 운영 절차다 — 봉투는 루트 산출까지만 책임진다.
pub(crate) fn cmd_anchor_checkpoint(args: &[String]) -> i32 {
    let mut log_path: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--log" => {
                i += 1;
                log_path = args.get(i).map(String::as_str);
            }
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let Some(log_path) = log_path else {
        eprintln!(
            "사용법: rhwp anchor checkpoint --log <anchor.ndjson> [-o <체크포인트.json>] [--json]"
        );
        return EXIT_USAGE;
    };
    let log = match anchor_log::load(log_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("오류(로그 무결): {e}");
            return 3;
        }
    };
    let Some(root) = anchor_log::merkle_root(&log.line_hashes) else {
        eprintln!("오류: 빈 로그에는 체크포인트가 없습니다 - {log_path}");
        return EXIT_USAGE;
    };
    let checkpoint = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "kind": anchor_log::CHECKPOINT_KIND,
        "upToSeq": log.entries.len() - 1,
        "merkleRoot": root,
    });
    if let Some(out) = out {
        if let Err(e) = fs::write(
            out,
            serde_json::to_string_pretty(&checkpoint).unwrap_or_default(),
        ) {
            eprintln!("오류: 체크포인트 저장 실패 - {out}: {e}");
            return EXIT_RUNTIME;
        }
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "log": log_path,
            "upToSeq": log.entries.len() - 1,
            "merkleRoot": root,
            "entries": log.entries.len(),
        }),
        "anchor",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!("체크포인트 — upToSeq {} root {root}", log.entries.len() - 1);
    }
    EXIT_OK
}



/// [#4543] 앵커 검증 — 캡슐이 로그에 있고, 체크포인트에 포함되는가.
pub(crate) fn cmd_anchor_verify(args: &[String]) -> i32 {
    let mut capsule: Option<&str> = None;
    let mut log_path: Option<&str> = None;
    let mut checkpoint_path: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--log" => {
                i += 1;
                log_path = args.get(i).map(String::as_str);
            }
            "--checkpoint" => {
                i += 1;
                checkpoint_path = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && capsule.is_none() => capsule = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(capsule), Some(log_path)) = (capsule, log_path) else {
        eprintln!("사용법: rhwp anchor verify <캡슐.json> --log <anchor.ndjson> [--checkpoint <cp.json>] [--json]");
        return EXIT_USAGE;
    };
    let bytes = match fs::read(capsule) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 캡슐을 읽을 수 없습니다 - {capsule}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let capsule_sha = replay_sha256_hex(&bytes);
    let (log, chain_ok, chain_err) = match anchor_log::load(log_path) {
        Ok(l) => (Some(l), true, serde_json::Value::Null),
        Err(e) => (None, false, serde_json::json!(e)),
    };
    let seq = log.as_ref().and_then(|l| {
        l.entries
            .iter()
            .position(|e| e["capsuleSha256"].as_str() == Some(capsule_sha.as_str()))
    });
    let mut in_checkpoint = serde_json::Value::Null;
    let mut merkle_path_json = serde_json::Value::Null;
    if let (Some(log), Some(seq), Some(cp_path)) = (log.as_ref(), seq, checkpoint_path) {
        match fs::read_to_string(cp_path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string()))
        {
            Ok(cp) => {
                let up_to = cp["upToSeq"].as_u64().map(|v| v as usize);
                let root = cp["merkleRoot"].as_str().unwrap_or("");
                match up_to {
                    Some(up_to) if seq <= up_to && up_to < log.line_hashes.len() => {
                        let leaves = &log.line_hashes[..=up_to];
                        let path = anchor_log::merkle_path(leaves, seq);
                        let ok = anchor_log::merkle_verify(&log.line_hashes[seq], &path, root);
                        in_checkpoint = serde_json::json!(ok);
                        merkle_path_json = serde_json::json!(path
                            .iter()
                            .map(|(h, left)| serde_json::json!({ "sibling": h, "siblingIsLeft": left }))
                            .collect::<Vec<_>>());
                    }
                    _ => in_checkpoint = serde_json::json!(false),
                }
            }
            Err(e) => {
                eprintln!("오류: 체크포인트를 읽을 수 없습니다 - {cp_path}: {e}");
                return EXIT_RUNTIME;
            }
        }
    }
    let logged = seq.is_some();
    let ok = chain_ok && logged && in_checkpoint != serde_json::json!(false);
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "capsule": capsule,
            "log": log_path,
            "capsuleSha256": capsule_sha,
            "logChainOk": chain_ok,
            "logChainError": chain_err,
            "logged": logged,
            "seq": seq,
            "inCheckpoint": in_checkpoint,
            "merklePath": merkle_path_json,
        }),
        "anchor",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "앵커 검증 — {capsule}: logged {logged} · chain {chain_ok} · checkpoint {in_checkpoint}"
        );
    }
    if ok {
        EXIT_OK
    } else {
        3 // #2707: 검증 단언 실패 — 앵커가 시점을 증명하지 못한다.
    }
}



/// [#4543] anchor 디스패치 — add·checkpoint·verify.
pub(crate) fn cmd_anchor(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("add") => cmd_anchor_add(&args[1..]),
        Some("checkpoint") => cmd_anchor_checkpoint(&args[1..]),
        Some("verify") => cmd_anchor_verify(&args[1..]),
        _ => {
            eprintln!("사용법: rhwp anchor <add|checkpoint|verify> …");
            EXIT_USAGE
        }
    }
}



/// [#4558] 공용 — 폴더 캡슐들의 축별 판정 재료를 한 번에 계산한다.
///
/// 반환: 캡슐별 (서명 verdict 문자열 옵션, anchored 옵션, lineage 유효 옵션,
/// 재현 성공 옵션). 옵션 `None` = 해당 축 재료 미지정(판정 밖).
#[allow(clippy::type_complexity)]
pub(crate) fn y10_axis_materials(
    nodes: &[audit_standard::CapsuleNode],
    keyring: Option<&std::collections::BTreeMap<String, capsule_sign::KeyEntry>>,
    anchored_set: Option<&std::collections::BTreeSet<String>>,
    deep: bool,
) -> Vec<(
    Option<String>,
    Option<bool>,
    Option<bool>,
    Option<Result<(), String>>,
)> {
    nodes
        .iter()
        .map(|node| {
            let signer = keyring.map(|kr| {
                let sidecar_file = capsule_sign::sidecar_path(&node.path.to_string_lossy());
                match fs::read(&sidecar_file)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                {
                    Some(sc) => {
                        let bytes = fs::read(&node.path).unwrap_or_default();
                        capsule_sign::verify_sidecar(&sc, &bytes, kr)
                            .verdict
                            .to_string()
                    }
                    None => "unsigned".to_string(),
                }
            });
            let anchored = anchored_set.map(|set| set.contains(&node.file_sha256));
            let lineage_ok = Some(
                audit_standard::walk_ancestry(&node.path, &node.value)
                    .broken_at
                    .is_none(),
            );
            let reproduced = if deep {
                Some(y10_reproduce_one(&node.value))
            } else {
                None
            };
            (signer, anchored, lineage_ok, reproduced)
        })
        .collect()
}



/// [#4558] 캡슐 하나의 deep 재현 — audit 와 같은 실행 코어 재사용.
pub(crate) fn y10_reproduce_one(capsule: &serde_json::Value) -> Result<(), String> {
    let (plan, _steps) = validated_capsule_plan(capsule)?;
    let mut plan = plan;
    let (out_sha, _n, input_sha) = replay_execute_to_temp(&mut plan, "y10").map_err(|(e, _)| e)?;
    let want_in = capsule["receipt"]["inputSha256"].as_str().unwrap_or("");
    let want_out = capsule["receipt"]["outputSha256"].as_str().unwrap_or("");
    if !want_in.is_empty() && want_in != input_sha {
        return Err("입력 해시 불일치(원본이 변했다)".to_string());
    }
    if want_out != out_sha {
        return Err("산출 해시 불일치(재현 실패)".to_string());
    }
    Ok(())
}



/// [#4558] 감사 보고 — 전 수치가 기존 축 검증의 기계 합산인 표준 보고서.
pub(crate) fn cmd_audit_report(args: &[String]) -> i32 {
    let mut dir: Option<&str> = None;
    let mut policy_path: Option<&str> = None;
    let mut keyring_path: Option<&str> = None;
    let mut anchor_path: Option<&str> = None;
    let mut sign_key: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut deep = false;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--deep" => deep = true,
            "--policy" => {
                i += 1;
                policy_path = args.get(i).map(String::as_str);
            }
            "--keyring" => {
                i += 1;
                keyring_path = args.get(i).map(String::as_str);
            }
            "--anchor-log" => {
                i += 1;
                anchor_path = args.get(i).map(String::as_str);
            }
            "--sign-key" => {
                i += 1;
                sign_key = args.get(i).map(String::as_str);
            }
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && dir.is_none() => dir = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(dir), Some(out)) = (dir, out) else {
        eprintln!("사용법: rhwp audit-report <캡슐 폴더> -o <report.json> [--deep] [--keyring <k>] [--anchor-log <l>] [--policy <p>] [--sign-key <키>] [--json]");
        return EXIT_USAGE;
    };
    let nodes = match audit_standard::collect(dir) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    };
    let keyring = match keyring_path {
        Some(kp) => match capsule_sign::load_keyring(kp) {
            Ok(k) => Some(k),
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        },
        None => None,
    };
    let anchored_set: Option<std::collections::BTreeSet<String>> = match anchor_path {
        Some(lp) => match anchor_log::load(lp) {
            Ok(log) => Some(
                log.entries
                    .iter()
                    .filter_map(|e| e["capsuleSha256"].as_str().map(str::to_string))
                    .collect(),
            ),
            Err(e) => {
                eprintln!("오류: 앵커 로그 검증 실패 — {e}");
                return 3;
            }
        },
        None => None,
    };
    let materials = y10_axis_materials(&nodes, keyring.as_ref(), anchored_set.as_ref(), deep);

    // 계보 절 — 머리(자식 없는 노드)별 사슬 판정, graphs = 뿌리 수.
    let (heads, roots) = audit_standard::heads_and_roots(&nodes);
    let mut lineage_valid = 0u64;
    let mut lineage_broken: Vec<serde_json::Value> = Vec::new();
    for &h in &heads {
        let a = audit_standard::walk_ancestry(&nodes[h].path, &nodes[h].value);
        match a.broken_at {
            None => lineage_valid += 1,
            Some(at) => lineage_broken.push(serde_json::json!({
                "head": nodes[h].name, "brokenAt": at,
            })),
        }
    }

    // 재현 절 (--deep opt-in — 재현은 비싸다, 6년 게이트와 같은 문장).
    let reproduction: serde_json::Value = if deep {
        let mut reproduced = 0u64;
        let mut failures: Vec<serde_json::Value> = Vec::new();
        for (node, (_, _, _, rep)) in nodes.iter().zip(&materials) {
            match rep.as_ref().expect("deep 재료") {
                Ok(()) => reproduced += 1,
                Err(e) => failures.push(serde_json::json!({
                    "capsule": node.name, "reason": e,
                })),
            }
        }
        let attempted = nodes.len() as u64;
        serde_json::json!({
            "attempted": attempted,
            "reproduced": reproduced,
            "rate": if attempted == 0 { serde_json::Value::Null }
                    else { serde_json::json!(reproduced as f64 / attempted as f64) },
            "failures": failures,
        })
    } else {
        serde_json::Value::Null
    };

    // 귀속 절 (--keyring opt-in).
    let attribution: serde_json::Value = if keyring.is_some() {
        let (mut signed, mut unsigned, mut valid, mut revoked) = (0u64, 0u64, 0u64, 0u64);
        for (_, (signer, _, _, _)) in nodes.iter().zip(&materials) {
            match signer.as_deref() {
                Some("unsigned") => unsigned += 1,
                Some(v) => {
                    signed += 1;
                    if v == "valid" {
                        valid += 1;
                    }
                    if v == "revoked" {
                        revoked += 1;
                    }
                }
                None => unreachable!("keyring 지정 시 signer 는 항상 계산된다"),
            }
        }
        serde_json::json!({
            "signed": signed, "unsigned": unsigned,
            "validSignatures": valid, "revokedKeyUses": revoked,
        })
    } else {
        serde_json::Value::Null
    };

    // 앵커 절 (--anchor-log opt-in).
    let anchoring: serde_json::Value = match &anchored_set {
        Some(_) => {
            let mut anchored = 0u64;
            for (_, (_, a, _, _)) in nodes.iter().zip(&materials) {
                if a == &Some(true) {
                    anchored += 1;
                }
            }
            serde_json::json!({
                "anchored": anchored,
                "unanchored": nodes.len() as u64 - anchored,
            })
        }
        None => serde_json::Value::Null,
    };

    // 게이트 절 (--policy opt-in) — 캡슐별 판정, 재료는 위 축들의 재사용.
    let gate: serde_json::Value = match policy_path {
        Some(pp) => {
            let text = match fs::read_to_string(pp) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("오류: 정책을 읽을 수 없습니다 - {pp}: {e}");
                    return EXIT_RUNTIME;
                }
            };
            let policy = match policy_gate::parse(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("오류(정책): {e}");
                    return EXIT_USAGE;
                }
            };
            let policy_sha = settle::sha256_hex(text.as_bytes());
            let (mut passed, mut denied) = (0u64, 0u64);
            for (signer, anchored, lineage_ok, rep) in &materials {
                let mut judgments: std::collections::BTreeMap<String, Option<serde_json::Value>> =
                    std::collections::BTreeMap::new();
                judgments.insert(
                    "reproduced".to_string(),
                    rep.as_ref().map(|r| serde_json::json!(r.is_ok())),
                );
                judgments.insert(
                    "lineageValid".to_string(),
                    lineage_ok.map(|v| serde_json::json!(v)),
                );
                judgments.insert(
                    "signerVerdict".to_string(),
                    signer.as_ref().map(|v| serde_json::json!(v)),
                );
                judgments.insert(
                    "anchoredOk".to_string(),
                    anchored.map(|v| serde_json::json!(v)),
                );
                let (ok, _violations) = policy_gate::evaluate(&policy, &judgments);
                if ok {
                    passed += 1;
                } else {
                    denied += 1;
                }
            }
            serde_json::json!({
                "policySha256": policy_sha, "passed": passed, "denied": denied,
            })
        }
        None => serde_json::Value::Null,
    };

    // 도구 버전 절 — 캡슐 영수증의 기록 합산(없으면 "미기록", 정직 보고).
    let mut versions: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for node in &nodes {
        let v = node.value["receipt"]["version"]
            .as_str()
            .unwrap_or("미기록")
            .to_string();
        versions.insert(v);
    }
    let tool_versions = serde_json::json!({
        "rhwp": versions.iter().collect::<Vec<_>>(),
        "mixed": versions.len() > 1,
    });

    let mut report = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "kind": audit_standard::REPORT_KIND,
        "scope": { "root": dir, "capsules": nodes.len() },
        "reproduction": reproduction,
        "lineage": {
            "graphs": roots, "heads": heads.len(),
            "valid": lineage_valid, "broken": lineage_broken,
        },
        "attribution": attribution,
        "anchoring": anchoring,
        "gate": gate,
        "toolVersions": tool_versions,
    });
    let signer = match sign_key {
        Some(k) => match capsule_sign::load_signing_key(k) {
            Ok((signing, key_id, _)) => Some((signing, key_id)),
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        },
        None => None,
    };
    if let Some((_, key_id)) = &signer {
        report["auditor"] = serde_json::json!({ "keyId": key_id });
    }
    let report_text = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Err(e) = fs::write(out, &report_text) {
        eprintln!("오류: 보고서 저장 실패 - {out}: {e}");
        return EXIT_RUNTIME;
    }
    if let Some((signing, key_id)) = &signer {
        let report_sha = settle::sha256_hex(report_text.as_bytes());
        let sidecar =
            capsule_sign::make_sidecar_json(signing, key_id, &report_sha, report_text.as_bytes());
        let sidecar_out = capsule_sign::sidecar_path(out);
        if let Err(e) = fs::write(
            &sidecar_out,
            serde_json::to_string_pretty(&sidecar).unwrap_or_default(),
        ) {
            eprintln!("오류: 보고서 서명 저장 실패 - {sidecar_out}: {e}");
            return EXIT_RUNTIME;
        }
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "report": out,
            "capsules": nodes.len(),
            "reproduction": report["reproduction"],
            "lineage": report["lineage"],
            "attribution": report["attribution"],
            "anchoring": report["anchoring"],
            "gate": report["gate"],
            "toolVersions": report["toolVersions"],
            "signed": signer.is_some(),
        }),
        "audit-report",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "감사 보고 — {out}: 캡슐 {} · 계보 {}/{} (서명 {})",
            nodes.len(),
            lineage_valid,
            heads.len(),
            signer.is_some()
        );
    }
    EXIT_OK
}



/// [#4558] 리콜 범위 — 오염 노드의 후손 폐쇄집합 + 정산 연결.
pub(crate) fn cmd_recall_scope(args: &[String]) -> i32 {
    let mut contaminated: Option<&str> = None;
    let mut among: Option<&str> = None;
    let mut ledger_path: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--contaminated" => {
                i += 1;
                contaminated = args.get(i).map(String::as_str);
            }
            "--among" => {
                i += 1;
                among = args.get(i).map(String::as_str);
            }
            "--ledger" => {
                i += 1;
                ledger_path = args.get(i).map(String::as_str);
            }
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(contaminated), Some(among)) = (contaminated, among) else {
        eprintln!("사용법: rhwp recall-scope --contaminated <캡슐|sha256> --among <폴더> [--ledger <원장>] [--json]");
        return EXIT_USAGE;
    };
    // 오염 정체성 = 파일 해시(64자리 16진이면 해시 그대로, 아니면 파일을 읽어 해시).
    let contaminated_sha =
        if contaminated.len() == 64 && contaminated.chars().all(|c| c.is_ascii_hexdigit()) {
            contaminated.to_lowercase()
        } else {
            match fs::read(contaminated) {
                Ok(b) => settle::sha256_hex(&b),
                Err(e) => {
                    eprintln!("오류: 오염 캡슐을 읽을 수 없습니다 - {contaminated}: {e}");
                    return EXIT_USAGE;
                }
            }
        };
    let nodes = match audit_standard::collect(among) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    };
    let mut affected: Vec<serde_json::Value> = Vec::new();
    let mut affected_shas: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for node in &nodes {
        if node.file_sha256 == contaminated_sha {
            // 오염 노드 자신 — 회수 1호.
            affected_shas.insert(node.file_sha256.clone());
            affected.push(serde_json::json!({
                "capsule": node.name, "path": [node.name],
            }));
            continue;
        }
        let ancestry = audit_standard::walk_ancestry(&node.path, &node.value);
        if let Some(pos) = ancestry
            .ancestors
            .iter()
            .position(|(_, sha)| *sha == contaminated_sha)
        {
            // 경로 = 오염 조상 → … → 이 캡슐 (가까운 순 기록을 뒤집는다).
            let mut path: Vec<String> = ancestry.ancestors[..=pos]
                .iter()
                .map(|(n, _)| n.clone())
                .collect();
            path.reverse();
            path.push(node.name.clone());
            affected_shas.insert(node.file_sha256.clone());
            affected.push(serde_json::json!({ "capsule": node.name, "path": path }));
        }
    }
    let unaffected = nodes.len() - affected.len();
    let mut envelope = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "contaminated": contaminated_sha,
        "affected": affected,
        "unaffected": unaffected,
    });
    if let Some(lp) = ledger_path {
        match anchor_log::load_kind(lp, settle::LEDGER_KIND) {
            Ok(ledger) => {
                let claims: Vec<serde_json::Value> = ledger
                    .entries
                    .iter()
                    .filter(|e| {
                        e["capsuleSha256"]
                            .as_str()
                            .map(|sha| affected_shas.contains(sha))
                            .unwrap_or(false)
                    })
                    .map(|e| {
                        serde_json::json!({
                            "seq": e["seq"], "claimSha256": e["claimSha256"],
                            "verdict": e["verdict"],
                        })
                    })
                    .collect();
                envelope["claims"] = serde_json::json!(claims);
            }
            Err(e) => {
                eprintln!("오류: 원장 검증 실패 — {e}");
                return 3;
            }
        }
    }
    let envelope = provenance::marked(envelope, "recall-scope");
    if json_mode {
        println!("{envelope}");
    } else {
        println!("리콜 범위 — 영향 {} · 미영향 {unaffected}", affected.len());
    }
    EXIT_OK
}



/// [#4558] 적합성 자가진단 — L1~L5 누적 요건, 판정기 재사용(발명 0).
pub(crate) fn cmd_conformance(args: &[String]) -> i32 {
    let mut dir: Option<&str> = None;
    let mut level: Option<&str> = None;
    let mut keyring_path: Option<&str> = None;
    let mut anchor_path: Option<&str> = None;
    let mut policy_path: Option<&str> = None;
    let mut ledger_path: Option<&str> = None;
    let mut deep = false;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--deep" => deep = true,
            "--level" => {
                i += 1;
                level = args.get(i).map(String::as_str);
            }
            "--keyring" => {
                i += 1;
                keyring_path = args.get(i).map(String::as_str);
            }
            "--anchor-log" => {
                i += 1;
                anchor_path = args.get(i).map(String::as_str);
            }
            "--policy" => {
                i += 1;
                policy_path = args.get(i).map(String::as_str);
            }
            "--ledger" => {
                i += 1;
                ledger_path = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && dir.is_none() => dir = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(dir), Some(level)) = (dir, level) else {
        eprintln!("사용법: rhwp conformance <캡슐 폴더> --level <L1..L5> [--deep] [--keyring] [--anchor-log] [--policy] [--ledger] [--json]");
        return EXIT_USAGE;
    };
    let want: u8 = match level {
        "L1" => 1,
        "L2" => 2,
        "L3" => 3,
        "L4" => 4,
        "L5" => 5,
        _ => {
            eprintln!("--level 은 L1..L5 만 받는다");
            return EXIT_USAGE;
        }
    };
    // 등급이 요구하는 재료의 선검사 — 없으면 판정이 아니라 사용법 오류다.
    if want >= 3 && (keyring_path.is_none() || anchor_path.is_none()) {
        eprintln!("L3 이상은 --keyring 과 --anchor-log 가 필요하다 (서명 귀속 + 앵커 운영이 요건)");
        return EXIT_USAGE;
    }
    if want >= 4 && policy_path.is_none() {
        eprintln!("L4 이상은 --policy 가 필요하다 (게이트 상시 배치가 요건)");
        return EXIT_USAGE;
    }
    if want >= 5 && ledger_path.is_none() {
        eprintln!("L5 는 --ledger 가 필요하다 (정산 원장 운영이 요건)");
        return EXIT_USAGE;
    }
    let nodes = match audit_standard::collect(dir) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    };
    if nodes.is_empty() {
        eprintln!("오류: 캡슐이 없다 — 빈 폴더의 적합성은 판정 대상이 아니다");
        return EXIT_USAGE;
    }
    let mut checks: Vec<serde_json::Value> = Vec::new();
    let mut push = |checks: &mut Vec<serde_json::Value>, id: &str, ok: bool, detail: String| {
        checks.push(serde_json::json!({ "id": id, "ok": ok, "detail": detail }));
        ok
    };
    // L1 — 산출물마다 영수증 (receipt 3해시).
    let bad_receipt = nodes
        .iter()
        .filter(|n| {
            !(n.value["receipt"]["inputSha256"].is_string()
                && n.value["receipt"]["outputSha256"].is_string()
                && n.value["receipt"]["planSha256"].is_string())
        })
        .count();
    let mut achieved = push(
        &mut checks,
        "L1-영수증",
        bad_receipt == 0,
        format!("영수증 미비 {bad_receipt}/{}", nodes.len()),
    );
    // L2 — 계획 정합(감사 가능) + 계보 유효.
    if want >= 2 {
        let bad_plan = nodes
            .iter()
            .filter(|n| validated_capsule_plan(&n.value).is_err())
            .count();
        achieved &= push(
            &mut checks,
            "L2-감사가능",
            bad_plan == 0,
            format!("계획 정합 실패 {bad_plan}/{}", nodes.len()),
        );
        let broken = nodes
            .iter()
            .filter(|n| {
                audit_standard::walk_ancestry(&n.path, &n.value)
                    .broken_at
                    .is_some()
            })
            .count();
        achieved &= push(
            &mut checks,
            "L2-계보",
            broken == 0,
            format!("계보 파손 {broken}/{}", nodes.len()),
        );
        if deep {
            let failed = nodes
                .iter()
                .filter(|n| y10_reproduce_one(&n.value).is_err())
                .count();
            achieved &= push(
                &mut checks,
                "L2-재현(deep)",
                failed == 0,
                format!("재현 실패 {failed}/{}", nodes.len()),
            );
        }
    }
    // L3 — 서명 전건 valid + 앵커 전건 포함.
    if want >= 3 {
        let keyring = match capsule_sign::load_keyring(keyring_path.expect("선검사")) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        };
        let anchored_set: std::collections::BTreeSet<String> =
            match anchor_log::load(anchor_path.expect("선검사")) {
                Ok(log) => log
                    .entries
                    .iter()
                    .filter_map(|e| e["capsuleSha256"].as_str().map(str::to_string))
                    .collect(),
                Err(e) => {
                    eprintln!("오류: 앵커 로그 검증 실패 — {e}");
                    return 3;
                }
            };
        let materials = y10_axis_materials(&nodes, Some(&keyring), Some(&anchored_set), false);
        let unsigned_or_bad = materials
            .iter()
            .filter(|(s, _, _, _)| s.as_deref() != Some("valid"))
            .count();
        achieved &= push(
            &mut checks,
            "L3-귀속",
            unsigned_or_bad == 0,
            format!("서명 미비/무효 {unsigned_or_bad}/{}", nodes.len()),
        );
        let unanchored = materials
            .iter()
            .filter(|(_, a, _, _)| *a != Some(true))
            .count();
        achieved &= push(
            &mut checks,
            "L3-앵커",
            unanchored == 0,
            format!("미앵커 {unanchored}/{}", nodes.len()),
        );
        // L4 — 게이트 전건 allow (재료는 위 축 재사용 — 판정기 발명 0).
        if want >= 4 {
            let text = match fs::read_to_string(policy_path.expect("선검사")) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("오류: 정책을 읽을 수 없습니다: {e}");
                    return EXIT_RUNTIME;
                }
            };
            let policy = match policy_gate::parse(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("오류(정책): {e}");
                    return EXIT_USAGE;
                }
            };
            let mut denied = 0usize;
            for (node, (signer, anchored, _, _)) in nodes.iter().zip(&materials) {
                let lineage_ok = audit_standard::walk_ancestry(&node.path, &node.value)
                    .broken_at
                    .is_none();
                let mut judgments: std::collections::BTreeMap<String, Option<serde_json::Value>> =
                    std::collections::BTreeMap::new();
                judgments.insert(
                    "reproduced".to_string(),
                    if deep {
                        Some(serde_json::json!(y10_reproduce_one(&node.value).is_ok()))
                    } else {
                        None
                    },
                );
                judgments.insert(
                    "lineageValid".to_string(),
                    Some(serde_json::json!(lineage_ok)),
                );
                judgments.insert(
                    "signerVerdict".to_string(),
                    signer.as_ref().map(|v| serde_json::json!(v)),
                );
                judgments.insert(
                    "anchoredOk".to_string(),
                    anchored.map(|v| serde_json::json!(v)),
                );
                let (ok, _) = policy_gate::evaluate(&policy, &judgments);
                if !ok {
                    denied += 1;
                }
            }
            achieved &= push(
                &mut checks,
                "L4-게이트",
                denied == 0,
                format!("게이트 거부 {denied}/{}", nodes.len()),
            );
        }
    }
    // L5 — 정산 원장 무결·비어있지 않음. (8년 공개 "운영"은 기계 판정 밖 — 정직 명시.)
    if want >= 5 {
        let ledger_ok =
            match anchor_log::load_kind(ledger_path.expect("선검사"), settle::LEDGER_KIND) {
                Ok(l) => !l.entries.is_empty(),
                Err(_) => false,
            };
        achieved &= push(
            &mut checks,
            "L5-원장",
            ledger_ok,
            "원장 체인 무결 + 기입 1건 이상".to_string(),
        );
        checks.push(serde_json::json!({
            "id": "L5-공개(판정 밖)", "ok": serde_json::Value::Null,
            "detail": "선택적 공개 '운영'은 조직 절차라 기계 판정 밖 — 수동 확인 항목",
        }));
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "level": level,
            "capsules": nodes.len(),
            "checks": checks,
            "achieved": achieved,
            "verdict": if achieved { "conformant" } else { "nonconformant" },
        }),
        "conformance",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "적합성 {level} — {} (캡슐 {})",
            if achieved {
                "conformant"
            } else {
                "nonconformant"
            },
            nodes.len()
        );
    }
    if achieved {
        EXIT_OK
    } else {
        3 // #2707: 판정 데이터 — 미달 항목은 checks 가 말한다.
    }
}



/// [#4553] 청구 발급 — 명세서·캡슐·게이트 봉투를 3해시로 고정한다.
pub(crate) fn cmd_settle_propose(args: &[String]) -> i32 {
    let mut workorder: Option<&str> = None;
    let mut capsule: Option<&str> = None;
    let mut gate_env: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut sign_key: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--workorder" => {
                i += 1;
                workorder = args.get(i).map(String::as_str);
            }
            "--capsule" => {
                i += 1;
                capsule = args.get(i).map(String::as_str);
            }
            "--gate-envelope" => {
                i += 1;
                gate_env = args.get(i).map(String::as_str);
            }
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            "--sign-key" => {
                i += 1;
                sign_key = args.get(i).map(String::as_str);
            }
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(workorder), Some(capsule), Some(gate_env), Some(out)) =
        (workorder, capsule, gate_env, out)
    else {
        eprintln!("사용법: rhwp settle propose --workorder <wo.json> --capsule <c.json> --gate-envelope <g.json> -o <claim.json> [--sign-key <키>] [--json]");
        return EXIT_USAGE;
    };
    let read = |p: &str, what: &str| -> Result<Vec<u8>, i32> {
        fs::read(p).map_err(|e| {
            eprintln!("오류: {what}을(를) 읽을 수 없습니다 - {p}: {e}");
            EXIT_RUNTIME
        })
    };
    let wo_bytes = match read(workorder, "명세서") {
        Ok(b) => b,
        Err(c) => return c,
    };
    let cap_bytes = match read(capsule, "캡슐") {
        Ok(b) => b,
        Err(c) => return c,
    };
    let gate_bytes = match read(gate_env, "게이트 봉투") {
        Ok(b) => b,
        Err(c) => return c,
    };
    // 검수 기준 없는 명세서는 발급 단계에서 거부 — 분쟁을 산문으로 되돌리지 않는다.
    let wo = match settle::parse_workorder(&String::from_utf8_lossy(&wo_bytes)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_USAGE;
        }
    };
    let wo_sha = settle::sha256_hex(&wo_bytes);
    let cap_sha = settle::sha256_hex(&cap_bytes);
    let gate_sha = settle::sha256_hex(&gate_bytes);
    let signer = match sign_key {
        Some(k) => match capsule_sign::load_signing_key(k) {
            Ok((signing, key_id, _)) => Some((signing, key_id)),
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        },
        None => None,
    };
    let mut claim = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "kind": settle::CLAIM_KIND,
        "workorderId": wo["workorderId"],
        "workorderSha256": wo_sha,
        "capsuleSha256": cap_sha,
        "gateEnvelopeSha256": gate_sha,
        // 주장 필드 — 시점 증명은 원장 체크포인트 공표의 몫(5년 축 동형).
        "claimedAt": capsule_sign::rfc3339_utc_now(),
    });
    if let Some((_, key_id)) = &signer {
        claim["claimant"] = serde_json::json!({ "keyId": key_id });
    }
    let claim_text = serde_json::to_string_pretty(&claim).unwrap_or_default();
    if let Err(e) = fs::write(out, &claim_text) {
        eprintln!("오류: 청구 저장 실패 - {out}: {e}");
        return EXIT_RUNTIME;
    }
    if let Some((signing, key_id)) = &signer {
        let claim_sha = settle::sha256_hex(claim_text.as_bytes());
        let sidecar =
            capsule_sign::make_sidecar_json(signing, key_id, &claim_sha, claim_text.as_bytes());
        let sidecar_out = capsule_sign::sidecar_path(out);
        if let Err(e) = fs::write(
            &sidecar_out,
            serde_json::to_string_pretty(&sidecar).unwrap_or_default(),
        ) {
            eprintln!("오류: 청구 서명 저장 실패 - {sidecar_out}: {e}");
            return EXIT_RUNTIME;
        }
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "claim": out,
            "workorderSha256": wo_sha,
            "capsuleSha256": cap_sha,
            "gateEnvelopeSha256": gate_sha,
            "signed": signer.is_some(),
        }),
        "settle",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!("청구 발급 — {out}: 3해시 고정 (서명 {})", signer.is_some());
    }
    EXIT_OK
}



/// [#4553] 청구 검증 — 3해시 대조 + 서명·이중 청구 opt-in 축.
pub(crate) fn cmd_settle_verify(args: &[String]) -> i32 {
    let mut claim_path: Option<&str> = None;
    let mut workorder: Option<&str> = None;
    let mut capsule: Option<&str> = None;
    let mut gate_env: Option<&str> = None;
    let mut keyring_path: Option<&str> = None;
    let mut ledger_path: Option<&str> = None;
    let mut sig_path: Option<String> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--workorder" => {
                i += 1;
                workorder = args.get(i).map(String::as_str);
            }
            "--capsule" => {
                i += 1;
                capsule = args.get(i).map(String::as_str);
            }
            "--gate-envelope" => {
                i += 1;
                gate_env = args.get(i).map(String::as_str);
            }
            "--keyring" => {
                i += 1;
                keyring_path = args.get(i).map(String::as_str);
            }
            "--ledger" => {
                i += 1;
                ledger_path = args.get(i).map(String::as_str);
            }
            "--sig" => {
                i += 1;
                sig_path = args.get(i).map(String::from);
            }
            other if !other.starts_with("--") && claim_path.is_none() => claim_path = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(claim_path), Some(workorder), Some(capsule), Some(gate_env)) =
        (claim_path, workorder, capsule, gate_env)
    else {
        eprintln!("사용법: rhwp settle verify <claim.json> --workorder <wo> --capsule <c> --gate-envelope <g> [--keyring <k>] [--ledger <l>] [--sig <서명>] [--json]");
        return EXIT_USAGE;
    };
    let claim_bytes = match fs::read(claim_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 청구를 읽을 수 없습니다 - {claim_path}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let claim: serde_json::Value = match serde_json::from_slice(&claim_bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 청구 파싱 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    if claim["kind"] != settle::CLAIM_KIND {
        eprintln!("오류: kind 가 {} 가 아닙니다.", settle::CLAIM_KIND);
        return EXIT_USAGE;
    }
    let sha_of = |p: &str| fs::read(p).map(|b| settle::sha256_hex(&b));
    let check = |p: &str, pinned: &serde_json::Value| -> bool {
        matches!((sha_of(p), pinned.as_str()), (Ok(actual), Some(exp)) if actual == exp)
    };
    let workorder_ok = check(workorder, &claim["workorderSha256"]);
    let capsule_ok = check(capsule, &claim["capsuleSha256"]);
    let gate_ok = check(gate_env, &claim["gateEnvelopeSha256"]);
    // 게이트 봉투의 verdict 재확인 — 해시가 맞아도 판정이 allow 가 아니면 검수 미통과다.
    let gate_verdict: serde_json::Value = fs::read(gate_env)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .map(|v| v["verdict"].clone())
        .unwrap_or(serde_json::Value::Null);
    let mut envelope = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "claim": claim_path,
        "workorderOk": workorder_ok,
        "capsuleOk": capsule_ok,
        "gateOk": gate_ok,
        "gateVerdict": gate_verdict,
    });
    let mut ok = workorder_ok && capsule_ok && gate_ok && gate_verdict == "allow";
    if let Some(kr_path) = keyring_path {
        let keyring = match capsule_sign::load_keyring(kr_path) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        };
        // 청구 서명 — 사이드카 부재는 false (청구 귀속은 이 축의 본질).
        let sidecar_file = sig_path.unwrap_or_else(|| capsule_sign::sidecar_path(claim_path));
        let signer_ok = match fs::read(&sidecar_file)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            Some(sc) => {
                capsule_sign::verify_sidecar(&sc, &claim_bytes, &keyring).verdict == "valid"
            }
            None => false,
        };
        // 명세서 서명 — 사이드카 부재는 null(미서명 보고), 있으면 판정.
        let wo_sidecar = capsule_sign::sidecar_path(workorder);
        let workorder_signer_ok: serde_json::Value = match fs::read(&wo_sidecar)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            Some(sc) => match fs::read(workorder) {
                Ok(wo_bytes) => serde_json::json!(
                    capsule_sign::verify_sidecar(&sc, &wo_bytes, &keyring).verdict == "valid"
                ),
                Err(_) => serde_json::json!(false),
            },
            None => serde_json::Value::Null,
        };
        ok = ok && signer_ok && workorder_signer_ok != serde_json::json!(false);
        envelope["signerOk"] = serde_json::json!(signer_ok);
        envelope["workorderSignerOk"] = workorder_signer_ok;
    }
    if let Some(lp) = ledger_path {
        match anchor_log::load_kind(lp, settle::LEDGER_KIND) {
            Ok(ledger) => {
                let dup =
                    settle::find_accepted(&ledger, claim["capsuleSha256"].as_str().unwrap_or(""))
                        .is_some();
                envelope["ledgerOk"] = serde_json::json!(true);
                envelope["duplicate"] = serde_json::json!(dup);
                ok = ok && !dup;
            }
            Err(e) => {
                eprintln!("경고: 원장 검증 실패 — {e}");
                envelope["ledgerOk"] = serde_json::json!(false);
                envelope["duplicate"] = serde_json::Value::Null;
                ok = false;
            }
        }
    }
    envelope["verdict"] = serde_json::json!(if ok { "ok" } else { "rejected" });
    let envelope = provenance::marked(envelope, "settle");
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "청구 검증 — 명세서 {workorder_ok} · 캡슐 {capsule_ok} · 게이트 {gate_ok} → {}",
            if ok { "ok" } else { "rejected" }
        );
    }
    if ok {
        EXIT_OK
    } else {
        3 // #2707: 판정 데이터 — 어떤 축이 무너졌는지는 봉투가 말한다.
    }
}



/// [#4553] 원장 기입 — 이중 청구 전역 검사 후 append-only 등재.
pub(crate) fn cmd_settle_record(args: &[String]) -> i32 {
    let mut claim_path: Option<&str> = None;
    let mut ledger_path: Option<&str> = None;
    let mut verdict = "accepted";
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--ledger" => {
                i += 1;
                ledger_path = args.get(i).map(String::as_str);
            }
            "--verdict" => {
                i += 1;
                verdict = match args.get(i).map(String::as_str) {
                    Some(v @ ("accepted" | "rejected")) => v,
                    _ => {
                        eprintln!("--verdict 는 accepted|rejected 만 받는다");
                        return EXIT_USAGE;
                    }
                };
            }
            other if !other.starts_with("--") && claim_path.is_none() => claim_path = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(claim_path), Some(ledger_path)) = (claim_path, ledger_path) else {
        eprintln!("사용법: rhwp settle record <claim.json> --ledger <ledger.ndjson> [--verdict accepted|rejected] [--json]");
        return EXIT_USAGE;
    };
    let claim_bytes = match fs::read(claim_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 청구를 읽을 수 없습니다 - {claim_path}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let claim: serde_json::Value = match serde_json::from_slice(&claim_bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 청구 파싱 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    if claim["kind"] != settle::CLAIM_KIND {
        eprintln!("오류: kind 가 {} 가 아닙니다.", settle::CLAIM_KIND);
        return EXIT_USAGE;
    }
    let Some(capsule_sha) = claim["capsuleSha256"].as_str().filter(|s| !s.is_empty()) else {
        eprintln!("오류: 청구에 capsuleSha256 이 없습니다.");
        return EXIT_USAGE;
    };
    // 깨진 원장에는 기입하지 않는다 — 5년 앵커 add 와 같은 문장.
    let ledger = match anchor_log::load_kind(ledger_path, settle::LEDGER_KIND) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("오류: 원장이 깨져 있어 기입을 거부합니다 — {e}");
            return 3;
        }
    };
    if verdict == "accepted" {
        if let Some(seq) = settle::find_accepted(&ledger, capsule_sha) {
            let envelope = provenance::marked(
                serde_json::json!({
                    "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                    "ledger": ledger_path,
                    "capsuleSha256": capsule_sha,
                    "duplicate": true,
                    "existingSeq": seq,
                }),
                "settle",
            );
            if json_mode {
                println!("{envelope}");
            } else {
                println!("이중 청구 — 같은 캡슐이 seq {seq} 에 이미 accepted (기입 거부)");
            }
            return 3; // #2707: 판정 데이터 — P3 이중 청구.
        }
    }
    let claim_sha = settle::sha256_hex(&claim_bytes);
    let line = settle::make_ledger_line(
        &ledger,
        &claim_sha,
        capsule_sha,
        verdict,
        &capsule_sign::rfc3339_utc_now(),
    );
    let mut text = String::new();
    if !ledger.entries.is_empty() {
        // 기존 파일 끝에 개행이 보장되지 않으므로 원문을 다시 읽어 이어붙인다.
        text = fs::read_to_string(ledger_path).unwrap_or_default();
        if !text.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
    }
    text.push_str(&line);
    text.push('\n');
    if let Err(e) = fs::write(ledger_path, text) {
        eprintln!("오류: 원장 저장 실패 - {ledger_path}: {e}");
        return EXIT_RUNTIME;
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "ledger": ledger_path,
            "seq": ledger.entries.len(),
            "claimSha256": claim_sha,
            "capsuleSha256": capsule_sha,
            "verdict": verdict,
            "duplicate": false,
        }),
        "settle",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "원장 기입 — {ledger_path} seq {} ({verdict})",
            ledger.entries.len()
        );
    }
    EXIT_OK
}



/// [#4553] settle 디스패치 — propose·verify·record.
pub(crate) fn cmd_settle(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("propose") => cmd_settle_propose(&args[1..]),
        Some("verify") => cmd_settle_verify(&args[1..]),
        Some("record") => cmd_settle_record(&args[1..]),
        _ => {
            eprintln!("사용법: rhwp settle <propose|verify|record> …");
            EXIT_USAGE
        }
    }
}



/// [#4551] 가림 발급 — plan 문자열 잎 전부를 salt 커밋으로 치환한다.
pub(crate) fn cmd_disclose_redact(args: &[String]) -> i32 {
    let mut capsule: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut opening_out: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            "--opening-out" => {
                i += 1;
                opening_out = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && capsule.is_none() => capsule = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(capsule), Some(out), Some(opening_out)) = (capsule, out, opening_out) else {
        eprintln!("사용법: rhwp disclose redact <캡슐.json> -o <가림.json> --opening-out <opening.json> [--json]");
        return EXIT_USAGE;
    };
    let bytes = match fs::read(capsule) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 캡슐을 읽을 수 없습니다 - {capsule}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let original_sha = replay_sha256_hex(&bytes);
    let mut cap: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 캡슐 파싱 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    if cap["kind"] != "workCapsule" {
        eprintln!("오류: kind 가 workCapsule 이 아닙니다.");
        return EXIT_USAGE;
    }
    let plan_text = cap["planText"].as_str().unwrap_or_default().to_string();
    let mut plan = cap["plan"].clone();
    let mut openings: Vec<(String, String, String)> = Vec::new();
    if let Err(e) = disclose::redact_plan(&mut plan, "", "", &mut openings) {
        eprintln!("오류: {e}");
        return EXIT_RUNTIME;
    }
    cap["plan"] = plan;
    // planText 원문은 개봉 파일로 이사한다 — 가림본에 남기면 전부 샌다.
    cap["planText"] = serde_json::json!("(redacted — 개봉 파일 보유자만 복원 가능)");
    cap["planRedacted"] = serde_json::json!(true);
    cap["originalCapsuleSha256"] = serde_json::json!(original_sha);
    if let Err(e) = fs::write(out, serde_json::to_string_pretty(&cap).unwrap_or_default()) {
        eprintln!("오류: 가림 캡슐 저장 실패 - {out}: {e}");
        return EXIT_RUNTIME;
    }
    let opening_map: serde_json::Map<String, serde_json::Value> = openings
        .iter()
        .map(|(p, v, salt)| (p.clone(), serde_json::json!({ "value": v, "salt": salt })))
        .collect();
    let opening = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "kind": disclose::OPENING_KIND,
        "originalCapsuleSha256": original_sha,
        "planText": plan_text,
        "openings": opening_map,
    });
    if let Err(e) = fs::write(
        opening_out,
        serde_json::to_string_pretty(&opening).unwrap_or_default(),
    ) {
        eprintln!("오류: 개봉 파일 저장 실패 - {opening_out}: {e}");
        return EXIT_RUNTIME;
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "capsule": capsule,
            "redacted": out,
            "opening": opening_out,
            "committedFields": openings.len(),
            "originalCapsuleSha256": original_sha,
        }),
        "disclose",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "가림 발급 — {out}: 커밋 {}개 (개봉은 비밀 보관: {opening_out})",
            openings.len()
        );
    }
    EXIT_OK
}



/// [#4551] 부분 개봉 검증 — 필드 단위 커밋 대조.
pub(crate) fn cmd_disclose_verify(args: &[String]) -> i32 {
    let mut redacted: Option<&str> = None;
    let mut opening_path: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--opening" => {
                i += 1;
                opening_path = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && redacted.is_none() => redacted = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(redacted), Some(opening_path)) = (redacted, opening_path) else {
        eprintln!("사용법: rhwp disclose verify <가림.json> --opening <opening.json> [--json]");
        return EXIT_USAGE;
    };
    let cap: serde_json::Value = match fs::read(redacted)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 가림 캡슐을 읽을 수 없습니다 - {redacted}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let opening: serde_json::Value = match fs::read(opening_path)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 개봉 파일을 읽을 수 없습니다 - {opening_path}: {e}");
            return EXIT_RUNTIME;
        }
    };
    if opening["kind"] != disclose::OPENING_KIND {
        eprintln!("오류: 개봉 kind 가 {} 가 아닙니다.", disclose::OPENING_KIND);
        return EXIT_USAGE;
    }
    let plan = &cap["plan"];
    let mut verified: Vec<String> = Vec::new();
    let mut mismatched: Vec<String> = Vec::new();
    if let Some(map) = opening["openings"].as_object() {
        for (pointer, entry) in map {
            let (Some(value), Some(salt)) = (entry["value"].as_str(), entry["salt"].as_str())
            else {
                mismatched.push(format!("{pointer} (개봉 형식 오류)"));
                continue;
            };
            match disclose::committed_at(plan, pointer) {
                Some(committed) if disclose::commit(value, salt) == committed => {
                    verified.push(pointer.clone())
                }
                Some(_) => mismatched.push(pointer.clone()),
                None => mismatched.push(format!("{pointer} (커밋 잎 없음)")),
            }
        }
    }
    let total = disclose::committed_count(plan);
    let unopened = total.saturating_sub(verified.len() + mismatched.len());
    let ok = mismatched.is_empty();
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "redacted": redacted,
            "verifiedFields": verified,
            "mismatched": mismatched,
            "unopened": unopened,
            "verdict": if ok { "ok" } else { "mismatch" },
        }),
        "disclose",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "부분 개봉 — 검증 {} · 불일치 {} · 미개봉 {unopened}",
            verified.len(),
            mismatched.len()
        );
    }
    if ok {
        EXIT_OK
    } else {
        3 // #2707: 개봉이 커밋과 다르다 — 위조 또는 값 변경.
    }
}



/// [#4551] 전체 복원 — 바이트 단위 원본 재현 (원본 서명이 그대로 valid).
pub(crate) fn cmd_disclose_restore(args: &[String]) -> i32 {
    let mut redacted: Option<&str> = None;
    let mut opening_path: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--opening" => {
                i += 1;
                opening_path = args.get(i).map(String::as_str);
            }
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && redacted.is_none() => redacted = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(redacted), Some(opening_path), Some(out)) = (redacted, opening_path, out) else {
        eprintln!("사용법: rhwp disclose restore <가림.json> --opening <전체개봉.json> -o <복원.json> [--json]");
        return EXIT_USAGE;
    };
    let mut cap: serde_json::Value = match fs::read(redacted)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 가림 캡슐을 읽을 수 없습니다 - {redacted}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let opening: serde_json::Value = match fs::read(opening_path)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 개봉 파일을 읽을 수 없습니다 - {opening_path}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let expected_sha = cap["originalCapsuleSha256"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let Some(plan_text) = opening["planText"].as_str() else {
        eprintln!("오류: 전체 개봉에 planText 가 필요합니다 (부분 개봉으로는 복원 불가).");
        return EXIT_USAGE;
    };
    // 전체 커버리지 검사 — 커밋 잎마다 개봉이 있어야 한다.
    let total = disclose::committed_count(&cap["plan"]);
    let provided = opening["openings"]
        .as_object()
        .map(|m| m.len())
        .unwrap_or(0);
    if provided < total {
        eprintln!("오류: 개봉 {provided}/{total} — 전체 개봉이 아니면 복원할 수 없습니다.");
        return 3;
    }
    let plan: serde_json::Value = match serde_json::from_str(plan_text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 개봉 planText 파싱 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    cap["plan"] = plan;
    cap["planText"] = serde_json::json!(plan_text);
    if let Some(map) = cap.as_object_mut() {
        map.remove("planRedacted");
        map.remove("originalCapsuleSha256");
    }
    let restored = serde_json::to_string_pretty(&cap).unwrap_or_default();
    if let Err(e) = fs::write(out, &restored) {
        eprintln!("오류: 복원 저장 실패 - {out}: {e}");
        return EXIT_RUNTIME;
    }
    let restored_sha = replay_sha256_hex(restored.as_bytes());
    let byte_identical = !expected_sha.is_empty() && restored_sha == expected_sha;
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "redacted": redacted,
            "restored": out,
            "restoredSha256": restored_sha,
            "originalCapsuleSha256": expected_sha,
            "byteIdentical": byte_identical,
        }),
        "disclose",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!("복원 — {out}: 바이트 동일 {byte_identical}");
    }
    if byte_identical {
        EXIT_OK
    } else {
        3 // #2707: 복원이 원본 바이트를 재현하지 못했다 — 개봉이 원본과 다르다.
    }
}



/// [#4551] disclose 디스패치 — redact·verify·restore.
pub(crate) fn cmd_disclose(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("redact") => cmd_disclose_redact(&args[1..]),
        Some("verify") => cmd_disclose_verify(&args[1..]),
        Some("restore") => cmd_disclose_restore(&args[1..]),
        _ => {
            eprintln!("사용법: rhwp disclose <redact|verify|restore> …");
            EXIT_USAGE
        }
    }
}



/// [#4549] 연합 번들 내보내기 — 계보 폐쇄집합+서명+머클 증명을 zip 하나로.
pub(crate) fn cmd_bundle_export(args: &[String]) -> i32 {
    let mut head: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut anchor_log_path: Option<&str> = None;
    let mut checkpoint_path: Option<&str> = None;
    let mut domain_path: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            "--anchor-log" => {
                i += 1;
                anchor_log_path = args.get(i).map(String::as_str);
            }
            "--checkpoint" => {
                i += 1;
                checkpoint_path = args.get(i).map(String::as_str);
            }
            "--domain" => {
                i += 1;
                domain_path = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && head.is_none() => head = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(head), Some(out)) = (head, out) else {
        eprintln!("사용법: rhwp bundle export <머리캡슐> -o <x.lineage-bundle> [--anchor-log <로그> --checkpoint <cp.json>] [--domain <domain.json>] [--json]");
        return EXIT_USAGE;
    };
    let closure = match lineage_bundle::closure(head) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    };
    let mut files: Vec<serde_json::Value> = Vec::new();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut signatures = 0usize;
    for (name, path) in &closure {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        };
        files.push(serde_json::json!({
            "path": format!("capsules/{name}"),
            "sha256": replay_sha256_hex(&bytes),
        }));
        entries.push((format!("capsules/{name}"), bytes));
        let sc_path = capsule_sign::sidecar_path(&path.to_string_lossy());
        if let Ok(sc) = fs::read(&sc_path) {
            files.push(serde_json::json!({
                "path": format!("signatures/{name}.sig.json"),
                "sha256": replay_sha256_hex(&sc),
            }));
            entries.push((format!("signatures/{name}.sig.json"), sc));
            signatures += 1;
        }
    }
    // 머클 증명 — 로그+체크포인트가 있으면 캡슐별 (로그 줄, 경로) 동봉.
    let mut proofs = 0usize;
    if let (Some(log_path), Some(cp_path)) = (anchor_log_path, checkpoint_path) {
        let log = match anchor_log::load(log_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("오류(로그 무결): {e}");
                return 3;
            }
        };
        let cp_bytes = match fs::read(cp_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("오류: 체크포인트를 읽을 수 없습니다 - {cp_path}: {e}");
                return EXIT_RUNTIME;
            }
        };
        let cp: serde_json::Value = match serde_json::from_slice(&cp_bytes) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: 체크포인트 파싱 실패 - {e}");
                return EXIT_RUNTIME;
            }
        };
        let up_to = cp["upToSeq"].as_u64().unwrap_or(0) as usize;
        let log_text = fs::read_to_string(log_path).unwrap_or_default();
        let lines: Vec<&str> = log_text.lines().filter(|l| !l.trim().is_empty()).collect();
        let mut proof_list = Vec::new();
        for (name, path) in &closure {
            let sha = replay_sha256_hex(&fs::read(path).unwrap_or_default());
            if let Some(seq) = log
                .entries
                .iter()
                .position(|e| e["capsuleSha256"].as_str() == Some(sha.as_str()))
            {
                if seq <= up_to && up_to < log.line_hashes.len() {
                    let leaves = &log.line_hashes[..=up_to];
                    let path_json: Vec<serde_json::Value> = anchor_log::merkle_path(leaves, seq)
                        .into_iter()
                        .map(|(h, left)| serde_json::json!({ "sibling": h, "siblingIsLeft": left }))
                        .collect();
                    proof_list.push(serde_json::json!({
                        "capsule": name,
                        "seq": seq,
                        "line": lines.get(seq).copied().unwrap_or(""),
                        "path": path_json,
                    }));
                    proofs += 1;
                }
            }
        }
        let proofs_json = serde_json::json!({ "checkpoint": cp, "proofs": proof_list });
        let bytes = serde_json::to_vec_pretty(&proofs_json).unwrap_or_default();
        files.push(serde_json::json!({
            "path": "anchor/proofs.json",
            "sha256": replay_sha256_hex(&bytes),
        }));
        entries.push(("anchor/proofs.json".to_string(), bytes));
    }
    let mut domain_name = serde_json::Value::Null;
    if let Some(dp) = domain_path {
        let bytes = match fs::read(dp) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("오류: 도메인 파일을 읽을 수 없습니다 - {dp}: {e}");
                return EXIT_RUNTIME;
            }
        };
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            domain_name = v["domain"].clone();
        }
        files.push(serde_json::json!({
            "path": "domain.json",
            "sha256": replay_sha256_hex(&bytes),
        }));
        entries.push(("domain.json".to_string(), bytes));
    }
    let manifest = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "kind": lineage_bundle::BUNDLE_KIND,
        "head": format!("capsules/{}", closure[0].0),
        "domain": domain_name,
        "files": files,
    });
    let file = match fs::File::create(out) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("오류: 번들 생성 실패 - {out}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let mut zipw = zip::ZipWriter::new(file);
    if let Err(e) = lineage_bundle::zip_put(
        &mut zipw,
        "manifest.json",
        &serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    ) {
        eprintln!("오류: {e}");
        return EXIT_RUNTIME;
    }
    for (path, bytes) in &entries {
        if let Err(e) = lineage_bundle::zip_put(&mut zipw, path, bytes) {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    }
    if let Err(e) = zipw.finish() {
        eprintln!("오류: 번들 마감 실패 - {e}");
        return EXIT_RUNTIME;
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "bundle": out,
            "head": closure[0].0,
            "capsules": closure.len(),
            "signatures": signatures,
            "proofs": proofs,
            "domain": manifest["domain"],
        }),
        "bundle",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "번들 내보내기 — {out}: 캡슐 {} · 서명 {signatures} · 증명 {proofs}",
            closure.len()
        );
    }
    EXIT_OK
}



/// [#4549] 연합 번들 검증 — 5단(컨테이너·폐쇄집합·계보·서명·앵커) 오프라인 판정.
pub(crate) fn cmd_bundle_verify(args: &[String]) -> i32 {
    let mut bundle: Option<&str> = None;
    let mut trust_domain: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--trust-domain" => {
                i += 1;
                trust_domain = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && bundle.is_none() => bundle = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(bundle), Some(trust_domain)) = (bundle, trust_domain) else {
        eprintln!(
            "사용법: rhwp bundle verify <x.lineage-bundle> --trust-domain <domain.json> [--json]"
        );
        return EXIT_USAGE;
    };
    let td_text = match fs::read_to_string(trust_domain) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("오류: trust-domain 을 읽을 수 없습니다 - {trust_domain}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let (domain, keyring_value, checkpoints) = match lineage_bundle::parse_trust_domain(&td_text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_USAGE;
        }
    };
    let ring = match capsule_sign::keyring_from_value(&keyring_value, trust_domain) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_USAGE;
        }
    };
    let map = match lineage_bundle::read_all(bundle) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    };
    let mut broken_at = serde_json::Value::Null;
    let mut note = |ok: &mut bool, why: String, broken_at: &mut serde_json::Value| {
        if *ok {
            *ok = false;
            if broken_at.is_null() {
                *broken_at = serde_json::json!(why);
            }
        }
    };
    // ① 컨테이너 — 매니페스트의 전 파일 해시 대조.
    let mut container_ok = true;
    let manifest: serde_json::Value = match map
        .get("manifest.json")
        .and_then(|b| serde_json::from_slice(b).ok())
    {
        Some(m) => m,
        None => {
            eprintln!("오류: manifest.json 이 없거나 파싱 불가");
            return EXIT_RUNTIME;
        }
    };
    if manifest["kind"] != lineage_bundle::BUNDLE_KIND {
        note(
            &mut container_ok,
            "manifest kind 불일치".into(),
            &mut broken_at,
        );
    }
    for f in manifest["files"].as_array().cloned().unwrap_or_default() {
        let (Some(path), Some(sha)) = (f["path"].as_str(), f["sha256"].as_str()) else {
            note(
                &mut container_ok,
                "manifest files 항목 형식 오류".into(),
                &mut broken_at,
            );
            continue;
        };
        match map.get(path) {
            Some(bytes) if replay_sha256_hex(bytes) == sha => {}
            Some(_) => note(
                &mut container_ok,
                format!("{path}: 해시 불일치(운송 중 변조)"),
                &mut broken_at,
            ),
            None => note(
                &mut container_ok,
                format!("{path}: 번들에 없음"),
                &mut broken_at,
            ),
        }
    }
    // ② 폐쇄집합 + ③ 계보 걷기 (머리부터 부모 이름 해소).
    let mut closure_ok = true;
    let mut lineage_valid = true;
    let head_path = manifest["head"].as_str().unwrap_or("");
    let mut current = head_path.to_string();
    let mut recorded: Option<String> = None;
    let mut child_input: Option<String> = None;
    let mut capsule_names: Vec<String> = Vec::new();
    for _ in 0..1000 {
        let Some(bytes) = map.get(&current) else {
            note(
                &mut closure_ok,
                format!("{current}: 폐쇄집합에 없음(부모 누락)"),
                &mut broken_at,
            );
            break;
        };
        let file_sha = replay_sha256_hex(bytes);
        let Ok(capsule) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            note(
                &mut lineage_valid,
                format!("{current}: 캡슐 파싱 실패"),
                &mut broken_at,
            );
            break;
        };
        if let Some(r) = recorded.as_deref() {
            if r != file_sha {
                note(
                    &mut lineage_valid,
                    format!("{current}: 부모 해시 불일치"),
                    &mut broken_at,
                );
                break;
            }
        }
        let out_sha = capsule["receipt"]["outputSha256"].as_str().unwrap_or("");
        if let Some(ci) = child_input.as_deref() {
            if !out_sha.is_empty() && out_sha != ci {
                note(
                    &mut lineage_valid,
                    format!("{current}: 계보 불변식 위반"),
                    &mut broken_at,
                );
                break;
            }
        }
        capsule_names.push(current.trim_start_matches("capsules/").to_string());
        let parent = &capsule["parent"];
        if parent.is_null() {
            break;
        }
        let (Some(pp), Some(psha)) = (parent["capsule"].as_str(), parent["sha256"].as_str()) else {
            note(
                &mut lineage_valid,
                format!("{current}: parent 형식 오류"),
                &mut broken_at,
            );
            break;
        };
        recorded = Some(psha.to_string());
        child_input = capsule["receipt"]["inputSha256"]
            .as_str()
            .map(str::to_string);
        let base = std::path::Path::new(pp)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| pp.to_string());
        current = format!("capsules/{base}");
    }
    // ④ 서명 — trust-domain 의 keyring 으로만 (동봉 keyring 불신, F2).
    let (mut sig_valid, mut sig_bad, mut unsigned) = (0u64, 0u64, 0u64);
    for name in &capsule_names {
        let cap_bytes = map
            .get(&format!("capsules/{name}"))
            .cloned()
            .unwrap_or_default();
        match map
            .get(&format!("signatures/{name}.sig.json"))
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
        {
            Some(sc) => {
                let v = capsule_sign::verify_sidecar(&sc, &cap_bytes, &ring);
                if v.verdict == "valid" {
                    sig_valid += 1;
                } else {
                    sig_bad += 1;
                    note(
                        &mut lineage_valid,
                        format!("{name}: 서명 {}(도메인 키링 기준)", v.verdict),
                        &mut broken_at,
                    );
                }
            }
            None => unsigned += 1,
        }
    }
    // ⑤ 앵커 — 동봉 증명의 루트가 도메인 선언 체크포인트와 일치해야 한다.
    let mut anchored = serde_json::Value::Null;
    if let Some(proofs) = map
        .get("anchor/proofs.json")
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
    {
        let bundle_root = proofs["checkpoint"]["merkleRoot"].as_str().unwrap_or("");
        let trusted = checkpoints
            .iter()
            .any(|c| c["merkleRoot"].as_str() == Some(bundle_root));
        let mut ok_count = 0u64;
        let mut bad = 0u64;
        for pr in proofs["proofs"].as_array().cloned().unwrap_or_default() {
            let line = pr["line"].as_str().unwrap_or("");
            let cap_name = pr["capsule"].as_str().unwrap_or("");
            let cap_sha = map
                .get(&format!("capsules/{cap_name}"))
                .map(|b| replay_sha256_hex(b))
                .unwrap_or_default();
            let line_entry: serde_json::Value =
                serde_json::from_str(line).unwrap_or(serde_json::Value::Null);
            let line_matches = line_entry["capsuleSha256"].as_str() == Some(cap_sha.as_str());
            let leaf = {
                use sha2::{Digest, Sha256};
                let mut h = Sha256::new();
                h.update(line.as_bytes());
                let d = h.finalize();
                let mut hex = String::with_capacity(64);
                for b in d {
                    use std::fmt::Write as _;
                    let _ = write!(hex, "{b:02x}");
                }
                hex
            };
            let path: Vec<(String, bool)> = pr["path"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|p| {
                    Some((
                        p["sibling"].as_str()?.to_string(),
                        p["siblingIsLeft"].as_bool()?,
                    ))
                })
                .collect();
            if trusted && line_matches && anchor_log::merkle_verify(&leaf, &path, bundle_root) {
                ok_count += 1;
            } else {
                bad += 1;
                note(
                    &mut lineage_valid,
                    format!(
                        "{cap_name}: 앵커 증명 실패(신뢰 체크포인트 {trusted}, 줄 일치 {line_matches})"
                    ),
                    &mut broken_at,
                );
            }
        }
        anchored = serde_json::json!({ "ok": ok_count, "bad": bad, "checkpointTrusted": trusted });
    }
    let all_ok = container_ok && closure_ok && lineage_valid && sig_bad == 0;
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "bundle": bundle,
            "trustDomain": domain,
            "containerOk": container_ok,
            "closureOk": closure_ok,
            "lineageValid": lineage_valid,
            "capsules": capsule_names.len(),
            "signed": { "valid": sig_valid, "invalid": sig_bad, "unsigned": unsigned },
            "anchored": anchored,
            "brokenAt": broken_at,
            "verdict": if all_ok { "ok" } else { "broken" },
        }),
        "bundle",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "번들 검증 — {bundle} @ {domain}: {}",
            envelope["verdict"].as_str().unwrap_or("?")
        );
    }
    if all_ok {
        EXIT_OK
    } else {
        3 // #2707: 검증 단언 실패 — 번들이 신뢰를 증명하지 못한다.
    }
}



/// [#4549] bundle 디스패치 — export·verify.
pub(crate) fn cmd_bundle(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("export") => cmd_bundle_export(&args[1..]),
        Some("verify") => cmd_bundle_verify(&args[1..]),
        _ => {
            eprintln!("사용법: rhwp bundle <export|verify> …");
            EXIT_USAGE
        }
    }
}



/// [#4545] 정책 게이트 — 반입 판정의 기계화. 판정 재료는 자기 신고가
/// 아니라 재계산이며, 규칙이 참조하는 판정만 지연 계산한다(비용 회계).
pub(crate) fn cmd_gate(args: &[String]) -> i32 {
    let mut target: Option<&str> = None;
    let mut policy_path: Option<&str> = None;
    let mut keyring_path: Option<&str> = None;
    let mut anchor_log_path: Option<&str> = None;
    let mut policy_keyring: Option<&str> = None;
    let mut deep = false;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--deep" => deep = true,
            "--policy" => {
                i += 1;
                policy_path = args.get(i).map(String::as_str);
            }
            "--keyring" => {
                i += 1;
                keyring_path = args.get(i).map(String::as_str);
            }
            "--anchor-log" => {
                i += 1;
                anchor_log_path = args.get(i).map(String::as_str);
            }
            "--policy-keyring" => {
                i += 1;
                policy_keyring = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && target.is_none() => target = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(target), Some(policy_path)) = (target, policy_path) else {
        eprintln!("사용법: rhwp gate <캡슐.json> --policy <policy.json> [--keyring <키링>] [--anchor-log <로그>] [--policy-keyring <키링>] [--deep] [--json]");
        return EXIT_USAGE;
    };
    let policy_text = match fs::read_to_string(policy_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("오류: 정책을 읽을 수 없습니다 - {policy_path}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let policy = match policy_gate::parse(&policy_text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("오류(정책): {e}");
            return EXIT_USAGE;
        }
    };
    // 정책 자체의 서명 (M3, 4년 축 재사용) — 보고 필드.
    let policy_signed = match policy_keyring {
        Some(kr) => {
            let ring = match capsule_sign::load_keyring(kr) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("오류: {e}");
                    return EXIT_RUNTIME;
                }
            };
            let sc_path = capsule_sign::sidecar_path(policy_path);
            match fs::read_to_string(&sc_path)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            {
                Some(sc) => {
                    let v = capsule_sign::verify_sidecar(&sc, policy_text.as_bytes(), &ring);
                    serde_json::json!(v.verdict == "valid")
                }
                None => serde_json::json!(false),
            }
        }
        None => serde_json::Value::Null,
    };
    let target_bytes = match fs::read(target) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 대상을 읽을 수 없습니다 - {target}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let target_sha = replay_sha256_hex(&target_bytes);
    let capsule: serde_json::Value = match serde_json::from_slice(&target_bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 캡슐 파싱 실패 - {target}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let needed = policy_gate::referenced_keys(&policy);
    let mut judgments: std::collections::BTreeMap<String, Option<serde_json::Value>> =
        std::collections::BTreeMap::new();
    // ── 계보 재계산 (lineageValid·lineageDepth) — 머리부터 뿌리까지 걷는다.
    if needed.contains("lineageValid") || needed.contains("lineageDepth") {
        let mut ok = true;
        let mut depth = 0u64;
        let mut current = std::path::PathBuf::from(target);
        let mut recorded: Option<String> = None;
        let mut child_input: Option<String> = None;
        for _ in 0..1000 {
            let Ok(bytes) = fs::read(&current) else {
                ok = false;
                break;
            };
            let file_sha = replay_sha256_hex(&bytes);
            let Ok(cap) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                ok = false;
                break;
            };
            if cap["kind"] != "workCapsule" {
                ok = false;
                break;
            }
            if let Some(r) = recorded.as_deref() {
                if r != file_sha {
                    ok = false;
                    break;
                }
            }
            let out_sha = cap["receipt"]["outputSha256"].as_str().unwrap_or("");
            if let Some(ci) = child_input.as_deref() {
                if !out_sha.is_empty() && out_sha != ci {
                    ok = false;
                    break;
                }
            }
            depth += 1;
            let parent = &cap["parent"];
            if parent.is_null() {
                break;
            }
            let (Some(pp), Some(psha)) = (parent["capsule"].as_str(), parent["sha256"].as_str())
            else {
                ok = false;
                break;
            };
            recorded = Some(psha.to_string());
            child_input = cap["receipt"]["inputSha256"].as_str().map(str::to_string);
            let pp_path = std::path::PathBuf::from(pp);
            current = if pp_path.is_absolute() {
                pp_path
            } else {
                current
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join(pp_path)
            };
        }
        judgments.insert("lineageValid".into(), Some(serde_json::json!(ok)));
        judgments.insert("lineageDepth".into(), Some(serde_json::json!(depth)));
    }
    // ── 서명 재계산 (signerVerdict·signerKeyId).
    if needed.contains("signerVerdict") || needed.contains("signerKeyId") {
        match keyring_path {
            Some(kr) => match capsule_sign::load_keyring(kr) {
                Ok(ring) => {
                    let sc_path = capsule_sign::sidecar_path(target);
                    match fs::read_to_string(&sc_path)
                        .ok()
                        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    {
                        Some(sc) => {
                            let v = capsule_sign::verify_sidecar(&sc, &target_bytes, &ring);
                            judgments
                                .insert("signerVerdict".into(), Some(serde_json::json!(v.verdict)));
                            judgments
                                .insert("signerKeyId".into(), Some(serde_json::json!(v.key_id)));
                        }
                        None => {
                            judgments.insert(
                                "signerVerdict".into(),
                                Some(serde_json::json!("unsigned")),
                            );
                            judgments.insert("signerKeyId".into(), Some(serde_json::Value::Null));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("오류: {e}");
                    return EXIT_RUNTIME;
                }
            },
            None => {
                judgments.insert("signerVerdict".into(), None);
                judgments.insert("signerKeyId".into(), None);
            }
        }
    }
    // ── 앵커 재계산 (anchoredOk).
    if needed.contains("anchoredOk") {
        match anchor_log_path {
            Some(path) => match anchor_log::load(path) {
                Ok(log) => {
                    let hit = log
                        .entries
                        .iter()
                        .any(|e| e["capsuleSha256"].as_str() == Some(target_sha.as_str()));
                    judgments.insert("anchoredOk".into(), Some(serde_json::json!(hit)));
                }
                Err(e) => {
                    eprintln!("오류(로그 무결): {e}");
                    return 3;
                }
            },
            None => {
                judgments.insert("anchoredOk".into(), None);
            }
        }
    }
    // ── 재현 재계산 (reproduced) — deep 요구.
    if needed.contains("reproduced") {
        if deep {
            let value = match validated_capsule_plan(&capsule) {
                Ok((validated_plan, _)) => {
                    let mut plan = validated_plan;
                    match replay_execute_to_temp(&mut plan, "gate") {
                        Ok((actual, _, _)) => Some(serde_json::json!(
                            capsule["receipt"]["outputSha256"].as_str() == Some(actual.as_str())
                        )),
                        Err(_) => Some(serde_json::json!(false)),
                    }
                }
                Err(_) => Some(serde_json::json!(false)),
            };
            judgments.insert("reproduced".into(), value);
        } else {
            // 재현 판정은 재실행 없이는 말할 수 없다 — 신고를 읽지 않는다.
            judgments.insert("reproduced".into(), None);
        }
    }
    let (allow, violations) = policy_gate::evaluate(&policy, &judgments);
    let evaluated: usize = policy.rules.iter().map(|r| r.require.len()).sum();
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "policy": policy.name,
            "policyPath": policy_path,
            "policySigned": policy_signed,
            "target": target,
            "targetSha256": target_sha,
            "verdict": if allow { "allow" } else { "deny" },
            "evaluated": evaluated,
            "violations": violations,
        }),
        "gate",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "게이트 — {target}: {} (평가 {evaluated}건)",
            envelope["verdict"].as_str().unwrap_or("?")
        );
    }
    if allow {
        EXIT_OK
    } else {
        3 // #2707: 반입 거부는 오류가 아니라 판정 데이터다.
    }
}



/// [#4537] 하네스 작업장 규약 — capsules/ 하위와 키링 골격을 만든다.
pub(crate) fn cmd_harness_init(args: &[String]) -> i32 {
    let mut dir: Option<&str> = None;
    let mut key_id: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--key-id" => {
                i += 1;
                key_id = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && dir.is_none() => dir = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let Some(dir) = dir else {
        eprintln!("사용법: rhwp harness init <폴더> [--key-id <소유/용도#세대>] [--json]");
        return EXIT_USAGE;
    };
    let caps_dir = std::path::Path::new(dir).join("capsules");
    if let Err(e) = fs::create_dir_all(&caps_dir) {
        eprintln!("오류: 작업장 생성 실패 - {dir}: {e}");
        return EXIT_RUNTIME;
    }
    let mut created = vec!["capsules/".to_string()];
    let mut key_file = serde_json::Value::Null;
    let mut public_key = serde_json::Value::Null;
    if let Some(id) = key_id {
        let kp = std::path::Path::new(dir).join("harness.key.json");
        if kp.exists() {
            eprintln!(
                "오류: 키 파일이 이미 있습니다 - {} (덮어쓰기 금지).",
                kp.display()
            );
            return EXIT_USAGE;
        }
        match capsule_sign::generate_key_json(id) {
            Ok(key) => {
                if let Err(e) =
                    fs::write(&kp, serde_json::to_string_pretty(&key).unwrap_or_default())
                {
                    eprintln!("오류: 키 저장 실패 - {}: {e}", kp.display());
                    return EXIT_RUNTIME;
                }
                let ring = serde_json::json!({
                    "schemaVersion": capsule_sign::SIGNING_SCHEMA_VERSION_STR,
                    "kind": "keyring",
                    "keys": [{ "keyId": id, "publicKey": key["publicKey"], "revoked": null }],
                });
                let rp = std::path::Path::new(dir).join("keyring.json");
                if let Err(e) =
                    fs::write(&rp, serde_json::to_string_pretty(&ring).unwrap_or_default())
                {
                    eprintln!("오류: 키링 저장 실패 - {}: {e}", rp.display());
                    return EXIT_RUNTIME;
                }
                created.push("harness.key.json".to_string());
                created.push("keyring.json".to_string());
                public_key = key["publicKey"].clone();
                key_file = serde_json::json!(kp.to_string_lossy());
            }
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        }
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "dir": dir,
            "created": created,
            "keyId": key_id,
            "publicKey": public_key,
            "keyFile": key_file,
        }),
        "harness",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!("하네스 작업장 — {dir}: {}", envelope["created"]);
    }
    EXIT_OK
}



/// [#4537] 한 방 루프 — 실산출 실행 + 영수증 + 캡슐(연번) + 자동 부모 연결 + 서명.
///
/// 에이전트가 매 작업을 이 명령으로 돌리면 capsules/ 안에서 해시 체인이
/// 스스로 자란다 — 사다리 5개 명령의 규약 조합을 한 명령으로 접은 것이
/// 하네스의 정의다.
pub(crate) fn cmd_harness_wrap(args: &[String]) -> i32 {
    let mut plan_arg: Option<&str> = None;
    let mut dir: Option<&str> = None;
    let mut sign_key: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--plan" => {
                i += 1;
                plan_arg = args.get(i).map(String::as_str);
            }
            "--dir" => {
                i += 1;
                dir = args.get(i).map(String::as_str);
            }
            "--sign-key" => {
                i += 1;
                sign_key = args.get(i).map(String::as_str);
            }
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(plan_arg), Some(dir)) = (plan_arg, dir) else {
        eprintln!(
            "사용법: rhwp harness wrap --plan <JSON|@파일> --dir <작업장> [--sign-key <키.json>] [--json]"
        );
        return EXIT_USAGE;
    };
    let plan_text = if let Some(path) = plan_arg.strip_prefix('@') {
        match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("오류: 계획을 읽을 수 없습니다 - {path}: {e}");
                return EXIT_RUNTIME;
            }
        }
    } else {
        plan_arg.to_string()
    };
    let plan: serde_json::Value = match serde_json::from_str(&plan_text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 계획 JSON 파싱 실패 - {e}");
            return EXIT_USAGE;
        }
    };
    let Some(input) = plan["input"].as_str().map(str::to_string) else {
        eprintln!("오류: 계획에 input 이 필요합니다.");
        return EXIT_USAGE;
    };
    let Some(output) = plan["output"].as_str().map(str::to_string) else {
        eprintln!("오류: 계획에 output 이 필요합니다 — wrap 은 실산출을 만든다.");
        return EXIT_USAGE;
    };
    let caps_dir = std::path::Path::new(dir).join("capsules");
    if !caps_dir.is_dir() {
        eprintln!("오류: 작업장이 아닙니다 - {dir} (harness init 먼저: capsules/ 없음)");
        return EXIT_USAGE;
    }
    // 직전 캡슐 = 자동 부모 — 연번 파일명이 정렬 순서를 보증한다.
    let existing = match fs::read_dir(&caps_dir) {
        Ok(rd) => match collect_audit_capsules(rd.map(|e| e.map(|d| d.path()))) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        },
        Err(e) => {
            eprintln!("오류: capsules/ 읽기 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    let input_bytes = match fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 입력을 읽을 수 없습니다 - {input}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let input_sha = replay_sha256_hex(&input_bytes);
    let plan_sha = replay_sha256_hex(plan_text.as_bytes());
    let plan_original = plan.clone();
    // 실산출 실행 — replay 와 달리 계획의 output 경로에 진짜로 쓴다.
    let (engine_env, engine_code) = run_plan_engine(&plan);
    if engine_code != 0 {
        if json_mode {
            println!(
                "{}",
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                        "error": format!("계획 실행 실패 (engine exit {engine_code})"),
                    }),
                    "harness",
                )
            );
        } else {
            eprintln!("계획 실행 실패 (engine exit {engine_code})");
        }
        return engine_code;
    }
    let steps = engine_env["steps"].as_array().map(|s| s.len()).unwrap_or(0);
    let output_bytes = match fs::read(&output) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 산출을 읽을 수 없습니다 - {output}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let output_sha = replay_sha256_hex(&output_bytes);
    let receipt = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "mode": "wrap",
        "input": input,
        "inputSha256": input_sha,
        "planSha256": plan_sha,
        "outputSha256": output_sha,
        "toolVersion": rhwp::version(),
        "steps": steps,
        "reproduced": serde_json::Value::Null,
        "expectedOutputSha256": serde_json::Value::Null,
    });
    let parent_link = match existing.last() {
        Some(prev) => {
            let bytes = match fs::read(prev) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("오류: 직전 캡슐 읽기 실패 - {}: {e}", prev.display());
                    return EXIT_RUNTIME;
                }
            };
            let name = prev.file_name().unwrap().to_string_lossy().into_owned();
            serde_json::json!({ "capsule": name, "sha256": replay_sha256_hex(&bytes) })
        }
        None => serde_json::Value::Null,
    };
    let capsule = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "kind": "workCapsule",
        "parent": parent_link,
        "plan": plan_original,
        "planText": plan_text,
        "receipt": receipt,
    });
    let cap_name = format!("{:04}_{}.capsule.json", existing.len() + 1, &plan_sha[..8]);
    let cap_path = caps_dir.join(&cap_name);
    if let Err(e) = fs::write(
        &cap_path,
        serde_json::to_string_pretty(&capsule).unwrap_or_default(),
    ) {
        eprintln!("오류: 캡슐 저장 실패 - {}: {e}", cap_path.display());
        return EXIT_RUNTIME;
    }
    let mut signed = false;
    if let Some(kp) = sign_key {
        let (signing, key_id, _) = match capsule_sign::load_signing_key(kp) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        };
        let cap_bytes = match fs::read(&cap_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("오류: 캡슐 재독 실패 - {e}");
                return EXIT_RUNTIME;
            }
        };
        let sidecar = capsule_sign::make_sidecar_json(
            &signing,
            &key_id,
            &replay_sha256_hex(&cap_bytes),
            &cap_bytes,
        );
        let sc = capsule_sign::sidecar_path(&cap_path.to_string_lossy());
        if let Err(e) = fs::write(
            &sc,
            serde_json::to_string_pretty(&sidecar).unwrap_or_default(),
        ) {
            eprintln!("오류: 서명 저장 실패 - {sc}: {e}");
            return EXIT_RUNTIME;
        }
        signed = true;
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "dir": dir,
            "capsule": cap_name,
            "output": output,
            "inputSha256": receipt["inputSha256"],
            "planSha256": receipt["planSha256"],
            "outputSha256": receipt["outputSha256"],
            "steps": steps,
            "parent": capsule["parent"]["capsule"].clone(),
            "signed": signed,
        }),
        "harness",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "하네스 wrap — {cap_name} (부모 {}, 서명 {signed})",
            capsule["parent"]["capsule"]
        );
    }
    EXIT_OK
}



/// [#4537] 작업장 통합 판정 — 체인·서명·(--deep) 재현을 한 봉투로.
pub(crate) fn cmd_harness_status(args: &[String]) -> i32 {
    let mut dir: Option<&str> = None;
    let mut keyring_path: Option<&str> = None;
    let mut deep = false;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--deep" => deep = true,
            "--keyring" => {
                i += 1;
                keyring_path = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && dir.is_none() => dir = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let Some(dir) = dir else {
        eprintln!("사용법: rhwp harness-status <작업장> [--keyring <키링.json>] [--deep] [--json]");
        return EXIT_USAGE;
    };
    let caps_dir = std::path::Path::new(dir).join("capsules");
    let capsules = match fs::read_dir(&caps_dir) {
        Ok(rd) => match collect_audit_capsules(rd.map(|e| e.map(|d| d.path()))) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        },
        Err(e) => {
            eprintln!("오류: 작업장이 아닙니다 - {dir}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let keyring = match keyring_path {
        Some(p) => match capsule_sign::load_keyring(p) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        },
        None => None,
    };
    let mut chain_valid = true;
    let mut broken_at = serde_json::Value::Null;
    let mut prev: Option<(String, String, String)> = None; // (파일명, 파일해시, 산출해시)
    let (mut sig_valid, mut sig_bad, mut unsigned) = (0u64, 0u64, 0u64);
    let (mut deep_checked, mut deep_ok) = (0u64, 0u64);
    for path in &capsules {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut fail = |why: &str, broken_at: &mut serde_json::Value, chain_valid: &mut bool| {
            if *chain_valid {
                *chain_valid = false;
                *broken_at = serde_json::json!(format!("{name}: {why}"));
            }
        };
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(_) => {
                fail("읽기 실패", &mut broken_at, &mut chain_valid);
                continue;
            }
        };
        let file_sha = replay_sha256_hex(&bytes);
        let Ok(capsule) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            fail("JSON 파싱 실패", &mut broken_at, &mut chain_valid);
            continue;
        };
        if capsule["kind"] != "workCapsule" {
            fail("kind 불일치", &mut broken_at, &mut chain_valid);
            continue;
        }
        let input_sha = capsule["receipt"]["inputSha256"].as_str().unwrap_or("");
        let output_sha = capsule["receipt"]["outputSha256"]
            .as_str()
            .unwrap_or("")
            .to_string();
        match (&prev, capsule.get("parent")) {
            (None, Some(p)) if !p.is_null() => {
                fail("첫 캡슐에 부모가 있다", &mut broken_at, &mut chain_valid)
            }
            (Some((pname, psha, pout)), Some(p)) => {
                if p["capsule"].as_str() != Some(pname.as_str()) {
                    fail("부모 파일명 불일치", &mut broken_at, &mut chain_valid);
                } else if p["sha256"].as_str() != Some(psha.as_str()) {
                    fail(
                        "부모 해시 불일치(사후 변조)",
                        &mut broken_at,
                        &mut chain_valid,
                    );
                } else if !input_sha.is_empty() && pout != input_sha && !pout.is_empty() {
                    // 연번 체인에서 산출→입력 연쇄는 선택 규약 — 다른 입력의
                    // 독립 작업도 같은 작업장에 쌓일 수 있으므로 깨짐이 아니라
                    // 참고 수치로만 센다(설계 결정: wrap 은 강제하지 않는다).
                }
            }
            (Some(_), None) => fail("parent 필드 없음", &mut broken_at, &mut chain_valid),
            _ => {}
        }
        if let Some(ring) = keyring.as_ref() {
            let sc_path = format!("{}.sig.json", path.display());
            match fs::read_to_string(&sc_path) {
                Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(sc) => {
                        let v = capsule_sign::verify_sidecar(&sc, &bytes, ring);
                        if v.verdict == "valid" {
                            sig_valid += 1;
                        } else {
                            sig_bad += 1;
                            fail("서명 무효", &mut broken_at, &mut chain_valid);
                        }
                    }
                    Err(_) => {
                        sig_bad += 1;
                        fail("서명 파싱 실패", &mut broken_at, &mut chain_valid);
                    }
                },
                Err(_) => unsigned += 1,
            }
        }
        if deep {
            deep_checked += 1;
            if let Ok((validated_plan, _)) = validated_capsule_plan(&capsule) {
                let mut plan = validated_plan;
                if let Ok((actual, _, _)) =
                    replay_execute_to_temp(&mut plan, &format!("hstat{deep_checked}"))
                {
                    if actual == output_sha {
                        deep_ok += 1;
                    } else {
                        fail("재현 불일치", &mut broken_at, &mut chain_valid);
                    }
                } else {
                    fail("재실행 실패", &mut broken_at, &mut chain_valid);
                }
            } else {
                fail("계획 검증 실패", &mut broken_at, &mut chain_valid);
            }
        }
        prev = Some((name, file_sha, output_sha));
    }
    let verdict_ok = chain_valid && sig_bad == 0 && (!deep || deep_ok == deep_checked);
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "dir": dir,
            "capsules": capsules.len(),
            "chainValid": chain_valid,
            "brokenAt": broken_at,
            "signed": if keyring.is_some() {
                serde_json::json!({ "valid": sig_valid, "invalid": sig_bad, "unsigned": unsigned })
            } else {
                serde_json::Value::Null
            },
            "reproduced": if deep {
                serde_json::json!({ "checked": deep_checked, "ok": deep_ok })
            } else {
                serde_json::Value::Null
            },
            "verdict": if verdict_ok { "ok" } else { "broken" },
        }),
        "harness-status",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "하네스 status — {dir}: 캡슐 {} · {}",
            capsules.len(),
            envelope["verdict"].as_str().unwrap_or("?")
        );
    }
    if verdict_ok {
        EXIT_OK
    } else {
        3 // #2707: 검증 단언 실패 — 작업장이 깨졌다.
    }
}



/// [#4537] harness 디스패치 — init·wrap. 판정(status)은 읽기 전용이라
/// 최상위 `harness-status` 로 나가 있다.
pub(crate) fn cmd_harness(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("init") => cmd_harness_init(&args[1..]),
        Some("wrap") => cmd_harness_wrap(&args[1..]),
        _ => {
            eprintln!("사용법: rhwp harness <init|wrap> …  (판정: rhwp harness-status)");
            EXIT_USAGE
        }
    }
}



/// [#4509] 서명키 발급 — Ed25519 키 파일. 비밀키가 담기므로 기존 파일을
/// 덮어쓰지 않는다(잃어버린 키는 재발급하면 되지만, 덮어쓴 키는 복구 불능).
pub(crate) fn cmd_keygen(args: &[String]) -> i32 {
    let mut key_id: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--key-id" => {
                i += 1;
                key_id = args.get(i).map(String::as_str);
            }
            "--out" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(key_id), Some(out)) = (key_id, out) else {
        eprintln!("사용법: rhwp keygen --key-id <소유/용도#세대> --out <키.json> [--json]");
        return EXIT_USAGE;
    };
    if std::path::Path::new(out).exists() {
        eprintln!("오류: 키 파일이 이미 있습니다 - {out} (덮어쓰기 금지 — 새 경로를 쓰세요).");
        return EXIT_USAGE;
    }
    let key = match capsule_sign::generate_key_json(key_id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    };
    if let Err(e) = fs::write(out, serde_json::to_string_pretty(&key).unwrap_or_default()) {
        eprintln!("오류: 키 저장 실패 - {out}: {e}");
        return EXIT_RUNTIME;
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "keyId": key_id,
            "publicKey": key["publicKey"],
            "keyFile": out,
        }),
        "keygen",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!("서명키 발급 — {key_id}");
        println!("  keyFile:   {out}  (비밀키 포함 — 보관 책임은 소유자에게)");
        println!(
            "  publicKey: {}",
            envelope["publicKey"].as_str().unwrap_or("")
        );
    }
    EXIT_OK
}



/// [#4509] 캡슐 서명 단건 검증 — 분리 서명을 캡슐 파일 바이트·키 등록부와
/// 대조한다. 판정은 봉투 데이터(verdict)이고 유효하지 않으면 exit 3 이다.
pub(crate) fn cmd_verify_signature(args: &[String]) -> i32 {
    let mut capsule: Option<&str> = None;
    let mut sig: Option<String> = None;
    let mut keyring_path: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--sig" => {
                i += 1;
                sig = args.get(i).cloned();
            }
            "--keyring" => {
                i += 1;
                keyring_path = args.get(i).map(String::as_str);
            }
            other if !other.starts_with("--") && capsule.is_none() => capsule = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let (Some(capsule), Some(keyring_path)) = (capsule, keyring_path) else {
        eprintln!(
            "사용법: rhwp verify-signature <캡슐.json> --keyring <키링.json> [--sig <서명.json>] [--json]"
        );
        return EXIT_USAGE;
    };
    let capsule_bytes = match fs::read(capsule) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 캡슐을 읽을 수 없습니다 - {capsule}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let sig_path = sig.unwrap_or_else(|| capsule_sign::sidecar_path(capsule));
    let sig_text = match fs::read_to_string(&sig_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("오류: 서명 파일을 읽을 수 없습니다 - {sig_path}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let keyring = match capsule_sign::load_keyring(keyring_path) {
        Ok(map) => map,
        Err(e) => {
            eprintln!("오류: {e}");
            return EXIT_RUNTIME;
        }
    };
    let capsule_sha = replay_sha256_hex(&capsule_bytes);
    // 서명 파일 파싱 실패는 IO 가 아니라 판정 데이터다 — 위조·손상 서명을
    // 오류로 숨기지 않고 verdict:malformed 로 폭로한다.
    let (verdict_json, exit_valid) = match serde_json::from_str::<serde_json::Value>(&sig_text) {
        Ok(sidecar) => {
            let sha_matches = sidecar["capsuleSha256"] == serde_json::json!(capsule_sha);
            let v = capsule_sign::verify_sidecar(&sidecar, &capsule_bytes, &keyring);
            let ok = v.verdict == "valid" && sha_matches;
            (
                serde_json::json!({
                    "capsuleShaMatches": sha_matches,
                    "signatureOk": v.signature_ok,
                    "keyId": v.key_id,
                    "keyKnown": v.key_known,
                    "revoked": v.revoked,
                    "verdict": v.verdict,
                }),
                ok,
            )
        }
        Err(_) => (
            serde_json::json!({
                "capsuleShaMatches": false,
                "signatureOk": serde_json::Value::Null,
                "keyId": serde_json::Value::Null,
                "keyKnown": false,
                "revoked": serde_json::Value::Null,
                "verdict": "malformed",
            }),
            false,
        ),
    };
    let mut body = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "capsule": capsule,
        "sigPath": sig_path,
        "capsuleSha256": capsule_sha,
    });
    for (k, v) in verdict_json.as_object().unwrap() {
        body[k] = v.clone();
    }
    let envelope = provenance::marked(body, "verify-signature");
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "캡슐 서명 — {capsule}: {}",
            envelope["verdict"].as_str().unwrap_or("?")
        );
    }
    if exit_valid {
        EXIT_OK
    } else {
        3 // #2707: 검증 단언 실패 — 서명이 귀속을 증명하지 못한다.
    }
}



/// [#4401] 작업 계보 — 캡슐 해시 체인을 머리부터 거슬러 검증한다.
///
/// 3중 판정: ① 부모 파일 무결(자식이 기록한 부모 파일 SHA-256 과 실물 대조 —
/// 사후 변조는 여기서 폭로된다) ② 계보 불변식(부모의 산출 해시 == 자식의 입력
/// 해시 — "이전 작업의 산출이 다음 작업의 입력"이라는 연대기의 정의) ③ `--deep`
/// 이면 링크마다 재실행 재현까지. 판정은 봉투 데이터(valid·brokenAt·links[])이고
/// 깨진 체인은 exit 3 이다.
pub(crate) fn cmd_lineage(args: &[String]) -> i32 {
    let mut head: Option<&str> = None;
    let mut deep = false;
    let mut keyring_path: Option<String> = None;
    let mut anchor_log_path: Option<String> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--deep" => deep = true,
            "--keyring" => {
                i += 1;
                match args.get(i) {
                    Some(v) => keyring_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: --keyring 뒤에 키 등록부 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--anchor-log" => {
                i += 1;
                match args.get(i) {
                    Some(v) => anchor_log_path = Some(v.clone()),
                    None => {
                        eprintln!("오류: --anchor-log 뒤에 로그 경로가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if !other.starts_with("--") && head.is_none() => head = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let Some(head) = head else {
        eprintln!("사용법: rhwp lineage <캡슐.json> [--deep] [--keyring <키링.json>] [--anchor-log <로그>] [--json]");
        return EXIT_USAGE;
    };
    // [#4509] 서명 판정은 opt-in — --keyring 없으면 signerOk 축 자체가 봉투에
    // 실리지 않아 기존 소비자가 깨지지 않는다.
    let keyring = match keyring_path.as_deref() {
        Some(path) => match capsule_sign::load_keyring(path) {
            Ok(map) => Some(map),
            Err(e) => {
                eprintln!("오류: {e}");
                return EXIT_RUNTIME;
            }
        },
        None => None,
    };
    // [#4543] 앵커 판정도 opt-in — 로그의 등재 해시 집합을 한 번만 만든다.
    let anchored_set: Option<std::collections::BTreeSet<String>> = match anchor_log_path.as_deref()
    {
        Some(path) => match anchor_log::load(path) {
            Ok(log) => Some(
                log.entries
                    .iter()
                    .filter_map(|e| e["capsuleSha256"].as_str().map(str::to_string))
                    .collect(),
            ),
            Err(e) => {
                eprintln!("오류(로그 무결): {e}");
                return EXIT_RUNTIME;
            }
        },
        None => None,
    };
    let mut links: Vec<serde_json::Value> = Vec::new();
    let mut valid = true;
    let mut broken_at: Option<String> = None;
    let mut current = std::path::PathBuf::from(head);
    // 자식이 기록한 (부모 파일 해시, 자식 입력 해시) — 다음 링크에서 대조한다.
    let mut recorded_parent_sha: Option<String> = None;
    let mut child_input_sha: Option<String> = None;
    let mut guard = 0usize;
    loop {
        guard += 1;
        let name = current.display().to_string();
        if guard > 1000 {
            valid = false;
            broken_at = Some(name);
            links.push(serde_json::json!({ "error": "체인 길이 1000 초과 — 순환 의심" }));
            break;
        }
        let bytes = match fs::read(&current) {
            Ok(b) => b,
            Err(e) => {
                if links.is_empty() {
                    eprintln!("오류: 캡슐을 읽을 수 없습니다 - {name}: {e}");
                    return EXIT_RUNTIME;
                }
                valid = false;
                broken_at = Some(name.clone());
                links.push(serde_json::json!({ "capsule": name, "error": format!("부모 캡슐 읽기 실패: {e}") }));
                break;
            }
        };
        let file_sha = replay_sha256_hex(&bytes);
        let capsule: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                valid = false;
                broken_at = Some(name.clone());
                links.push(
                    serde_json::json!({ "capsule": name, "error": format!("JSON 파싱 실패: {e}") }),
                );
                break;
            }
        };
        if capsule["kind"] != "workCapsule" {
            valid = false;
            broken_at = Some(name.clone());
            links.push(
                serde_json::json!({ "capsule": name, "error": "kind 가 workCapsule 이 아님" }),
            );
            break;
        }
        let Some(input_sha) = capsule["receipt"]["inputSha256"]
            .as_str()
            .filter(|value| is_sha256_hex(value))
            .map(str::to_string)
        else {
            valid = false;
            broken_at = Some(name.clone());
            links.push(serde_json::json!({
                "capsule": name,
                "error": "receipt.inputSha256 가 없거나 64자리 16진이 아님",
            }));
            break;
        };
        let Some(output_sha) = capsule["receipt"]["outputSha256"]
            .as_str()
            .filter(|value| is_sha256_hex(value))
            .map(str::to_string)
        else {
            valid = false;
            broken_at = Some(name.clone());
            links.push(serde_json::json!({
                "capsule": name,
                "error": "receipt.outputSha256 가 없거나 64자리 16진이 아님",
            }));
            break;
        };
        let (validated_plan, expected_steps) = match validated_capsule_plan(&capsule) {
            Ok(value) => value,
            Err(error) => {
                valid = false;
                broken_at = Some(name.clone());
                links.push(serde_json::json!({ "capsule": name, "error": error }));
                break;
            }
        };
        let Some(parent) = capsule.get("parent") else {
            valid = false;
            broken_at = Some(name.clone());
            links.push(serde_json::json!({
                "capsule": name,
                "error": "parent 필드 없음",
            }));
            break;
        };
        let parent_link = if parent.is_null() {
            None
        } else {
            let Some(pp) = parent["capsule"].as_str() else {
                valid = false;
                broken_at = Some(name.clone());
                links.push(serde_json::json!({ "capsule": name, "error": "parent.capsule 없음" }));
                break;
            };
            let Some(parent_sha) = parent["sha256"]
                .as_str()
                .filter(|value| is_sha256_hex(value))
            else {
                valid = false;
                broken_at = Some(name.clone());
                links.push(serde_json::json!({
                    "capsule": name,
                    "error": "parent.sha256 가 없거나 64자리 16진이 아님",
                }));
                break;
            };
            Some((pp.to_string(), parent_sha.to_string()))
        };
        let parent_ok = recorded_parent_sha.as_deref().map(|r| r == file_sha);
        let lineage_ok = child_input_sha.as_deref().map(|ci| output_sha == ci);
        let reproduced = if deep {
            let mut plan = validated_plan;
            match replay_execute_to_temp(&mut plan, &format!("lineage{guard}")) {
                Ok((actual, actual_steps, actual_input)) => Some(
                    actual == output_sha
                        && actual_input == input_sha
                        && actual_steps as u64 == expected_steps,
                ),
                Err(_) => Some(false),
            }
        } else {
            None
        };
        let mut link = serde_json::json!({
            "capsule": name,
            "inputSha256": input_sha,
            "outputSha256": output_sha,
            "parentOk": parent_ok,
            "lineageOk": lineage_ok,
            "reproduced": reproduced,
        });
        let mut signer_broken = false;
        if let Some(ring) = keyring.as_ref() {
            // 사이드카 없음 = null(미서명 — 강제는 게이트의 몫), 있는데 무효·
            // 미등록·폐기·기형 = false(깨진 계보). 읽기 실패는 없음으로 본다.
            let sc_path = format!("{}.sig.json", current.display());
            let (signer_ok, key_id) = match fs::read_to_string(&sc_path) {
                Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(sc) => {
                        let v = capsule_sign::verify_sidecar(&sc, &bytes, ring);
                        if v.verdict != "valid" {
                            signer_broken = true;
                        }
                        (
                            serde_json::json!(v.verdict == "valid"),
                            serde_json::json!(v.key_id),
                        )
                    }
                    Err(_) => {
                        signer_broken = true;
                        (serde_json::json!(false), serde_json::Value::Null)
                    }
                },
                Err(_) => (serde_json::Value::Null, serde_json::Value::Null),
            };
            link["signerOk"] = signer_ok;
            link["keyId"] = key_id;
        }
        if let Some(set) = anchored_set.as_ref() {
            // 미등재 = false 이되 체인을 깨지 않는다 — 등재 강제는 게이트(6년
            // 축)의 직무다. 판정 데이터만 싣는다.
            link["anchoredOk"] = serde_json::json!(set.contains(&file_sha));
        }
        links.push(link);
        if parent_ok == Some(false)
            || lineage_ok == Some(false)
            || reproduced == Some(false)
            || signer_broken
        {
            valid = false;
            broken_at = Some(name);
            break;
        }
        let Some((pp, parent_sha)) = parent_link else {
            break;
        };
        recorded_parent_sha = Some(parent_sha);
        child_input_sha = Some(input_sha);
        let pp_path = std::path::PathBuf::from(pp);
        current = if pp_path.is_absolute() {
            pp_path
        } else {
            current
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(pp_path)
        };
    }
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "head": head,
            "depth": links.len(),
            "valid": valid,
            "brokenAt": broken_at,
            "links": links,
        }),
        "lineage",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!(
            "작업 계보 — {head}: 깊이 {} · {}",
            envelope["depth"],
            if valid { "유효" } else { "깨짐" }
        );
        if let Some(b) = envelope["brokenAt"].as_str() {
            println!("  brokenAt: {b}");
        }
    }
    if valid {
        EXIT_OK
    } else {
        3 // #2707: 검증 단언 실패 — 연대기가 깨졌다.
    }
}



/// [#4393] 에이전트 노동 감사 — 작업 캡슐(*.capsule.json) 폴더를 전수 재실행해
/// 재현율을 회계한다. 개별 영수증(replay)이 작업 하나의 증명이라면, audit 은
/// 조직 규모의 "에이전트가 한 일" 전체에 대한 회계감사다. 불일치 1건 = exit 3.
pub(crate) fn cmd_audit(args: &[String]) -> i32 {
    let mut dir: Option<&str> = None;
    let mut json_mode = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            other if !other.starts_with("--") && dir.is_none() => dir = Some(other),
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let Some(dir) = dir else {
        eprintln!("사용법: rhwp audit <캡슐 폴더> [--json]  (대상: *.capsule.json)");
        return EXIT_USAGE;
    };
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("오류: 폴더를 읽을 수 없습니다 - {dir}: {e}");
            return EXIT_RUNTIME;
        }
    };
    let capsules =
        match collect_audit_capsules(entries.map(|entry| entry.map(|entry| entry.path()))) {
            Ok(capsules) => capsules,
            Err(e) => {
                eprintln!("오류: {dir} 감사 대상을 전수 열거할 수 없습니다 - {e}");
                return EXIT_RUNTIME;
            }
        };
    if capsules.is_empty() {
        eprintln!("오류: {dir} 에 *.capsule.json 이 없습니다 — 감사 대상 없음.");
        return EXIT_USAGE;
    }
    let mut reproduced_count = 0usize;
    let mut failed: Vec<serde_json::Value> = Vec::new();
    for (idx, path) in capsules.iter().enumerate() {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let fail = |reason: String| serde_json::json!({ "capsule": name, "error": reason });
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                failed.push(fail(format!("읽기 실패: {e}")));
                continue;
            }
        };
        let capsule: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                failed.push(fail(format!("JSON 파싱 실패: {e}")));
                continue;
            }
        };
        if capsule["kind"] != "workCapsule" {
            failed.push(fail("kind 가 workCapsule 이 아님".into()));
            continue;
        }
        let Some(expected) = capsule["receipt"]["outputSha256"]
            .as_str()
            .filter(|value| is_sha256_hex(value))
        else {
            failed.push(fail(
                "receipt.outputSha256 가 없거나 64자리 16진이 아님".into(),
            ));
            continue;
        };
        let Some(expected_input) = capsule["receipt"]["inputSha256"]
            .as_str()
            .filter(|value| is_sha256_hex(value))
        else {
            failed.push(fail(
                "receipt.inputSha256 가 없거나 64자리 16진이 아님".into(),
            ));
            continue;
        };
        let (mut plan, expected_steps) = match validated_capsule_plan(&capsule) {
            Ok(value) => value,
            Err(error) => {
                failed.push(fail(error));
                continue;
            }
        };
        match replay_execute_to_temp(&mut plan, &format!("audit{idx}")) {
            Ok((actual, actual_steps, actual_input)) => {
                if actual_input != expected_input {
                    failed.push(serde_json::json!({
                        "capsule": name,
                        "kind": "inputSha256",
                        "expected": expected_input,
                        "actual": actual_input,
                    }));
                } else if actual_steps as u64 != expected_steps {
                    failed.push(serde_json::json!({
                        "capsule": name,
                        "kind": "steps",
                        "expected": expected_steps,
                        "actual": actual_steps,
                    }));
                } else if actual == expected {
                    reproduced_count += 1;
                } else {
                    failed.push(serde_json::json!({
                        "capsule": name,
                        "expected": expected,
                        "actual": actual,
                    }));
                }
            }
            Err((msg, _code)) => failed.push(fail(msg)),
        }
    }
    let total = capsules.len();
    let rate = reproduced_count as f64 / total as f64;
    let envelope = provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "root": dir,
            "total": total,
            "reproduced": reproduced_count,
            "failed": failed,
            "reproducedRate": rate,
        }),
        "audit",
    );
    if json_mode {
        println!("{envelope}");
    } else {
        println!("에이전트 노동 감사 — {dir}");
        println!(
            "  캡슐 {total} · 재현 {reproduced_count} · 실패 {} · 재현율 {:.1}%",
            total - reproduced_count,
            rate * 100.0
        );
        for f in &failed {
            println!("  [FAIL] {}", f["capsule"].as_str().unwrap_or("?"));
        }
    }
    if failed.is_empty() {
        EXIT_OK
    } else {
        3 // #2707: 검증 단언 실패 — 재현되지 않은 작업이 있다.
    }
}


pub(crate) fn cmd_run_plan(args: &[String]) -> i32 {
    let mut plan_path: Option<&str> = None;
    let mut plan_inline: Option<&str> = None;
    let mut json_mode = false;
    // [#3721] 선검증만 돌리고 디스크는 건드리지 않는다 — 계획을 제출 전에 검사.
    let mut dry_run = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--dry-run" => dry_run = true,
            "--plan-json" => {
                i += 1;
                match args.get(i) {
                    Some(v) => plan_inline = Some(v.as_str()),
                    None => {
                        eprintln!("오류: --plan-json 뒤에 계획 JSON 문자열이 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if !other.starts_with("--") && plan_path.is_none() => plan_path = Some(other),
            other => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {}", other);
                return EXIT_USAGE;
            }
        }
        i += 1;
    }
    let plan_text = match (plan_inline, plan_path) {
        (Some(inline), _) => inline.to_string(),
        (None, Some(path)) => match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("오류: 계획 파일을 읽을 수 없습니다 - {}: {}", path, e);
                return EXIT_RUNTIME;
            }
        },
        (None, None) => {
            eprintln!("사용법: rhwp run <계획.json> [--json] [--dry-run]  (파일 대신 --plan-json '<JSON>')");
            return EXIT_USAGE;
        }
    };
    let mut plan: serde_json::Value = match serde_json::from_str(&plan_text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("오류: 계획 JSON 파싱 실패 - {}", e);
            return EXIT_USAGE;
        }
    };
    // 플래그는 계획서 필드를 덮어쓴다 — 의도의 단일 출처는 계획서이고, CLI 는 그 편의 입구다.
    // (계획서가 dryRun 을 실을 수 있으므로 MCP hwp_run_plan 은 인자 추가 없이 같은 계약을 얻는다.)
    if dry_run {
        if let Some(obj) = plan.as_object_mut() {
            obj.insert("dryRun".to_string(), serde_json::Value::Bool(true));
        }
    }
    let (journal, code) = run_plan_engine(&plan);
    if json_mode {
        println!("{}", journal);
    } else if code == EXIT_OK && journal["dryRun"] == true {
        let preview_all = journal["preview"].as_array().cloned().unwrap_or_default();
        // [#3719 §6-8] 건너뛸 step 은 "실행 가능"에 넣지 않는다 — dry-run 이 예고하는
        // 실행 개수와 run(실제 실행)이 보고할 적용 개수가 같은 말을 해야 한다.
        let skipped_count = preview_all.iter().filter(|s| s["skipped"] == true).count();
        println!(
            "검사 통과: {} step 실행 가능{} (디스크 무변경, 산출 예정 {})",
            preview_all.len() - skipped_count,
            if skipped_count == 0 {
                String::new()
            } else {
                format!(" · {} step 건너뜀 예정", skipped_count)
            },
            journal["output"].as_str().unwrap_or("-")
        );
        for step in &preview_all {
            println!("  - {}", preview_line(step));
        }
    } else if code == EXIT_OK {
        // [#3719 §6-8] 건너뛴 step 을 적용한 것과 같이 세면 "다 됐다"는 보고가 거짓이 된다.
        let skipped: Vec<&serde_json::Value> = journal["steps"]
            .as_array()
            .map(|steps| steps.iter().filter(|s| s["skipped"] == true).collect())
            .unwrap_or_default();
        let total = journal["steps"].as_array().map(|s| s.len()).unwrap_or(0);
        println!(
            "완료: {} step 적용{}, 산출 {}",
            total - skipped.len(),
            if skipped.is_empty() {
                String::new()
            } else {
                format!(" · {} step 건너뜀", skipped.len())
            },
            journal["output"].as_str().unwrap_or("-")
        );
        for step in &skipped {
            println!(
                "  - step {} 건너뜀: {}",
                step["step"].as_u64().unwrap_or(0),
                step["reason"].as_str().unwrap_or("")
            );
        }
        if let Some(steps) = journal["steps"].as_array() {
            for step in steps {
                if let Some(confusable) = step["confusable"].as_array() {
                    for item in confusable {
                        eprintln!(
                            "경고: '{}' 과(와) 화면상 구별되지 않는 이름의 누름틀이 문서에 함께 있습니다 — 채운 칸이 의도한 칸인지 확인하세요.",
                            item["name"].as_str().unwrap_or("")
                        );
                    }
                }
            }
        }
    } else {
        // 사람 모드에서도 판정 근거는 저널 그대로 남긴다 — 달리 설명할 출처가 없다.
        eprintln!("{}", journal);
    }
    code
}



/// 계획 실행 본체 — (저널, 종료 코드). CLI 와 MCP `hwp_run_plan` 이 같은 판정을 공유한다.
pub(crate) fn run_plan_engine(plan: &serde_json::Value) -> (serde_json::Value, i32) {
    fn usage(reason: &str) -> (serde_json::Value, i32) {
        (
            provenance::marked(
                serde_json::json!({ "schemaVersion": ENVELOPE_SCHEMA_VERSION, "error": reason }),
                "run",
            ),
            EXIT_USAGE,
        )
    }
    fn fail(reason: String) -> (serde_json::Value, i32) {
        (
            provenance::marked(
                serde_json::json!({ "schemaVersion": ENVELOPE_SCHEMA_VERSION, "error": reason }),
                "run",
            ),
            EXIT_RUNTIME,
        )
    }

    if plan["planVersion"].as_str() != Some("1.0") {
        return usage("planVersion \"1.0\" 이 필요합니다");
    }
    let Some(input) = plan["input"].as_str() else {
        return usage("input (원본 문서 경로)이 필요합니다");
    };
    let Some(output) = plan["output"].as_str() else {
        return usage("output (산출 경로)이 필요합니다");
    };
    let steps = match plan["steps"].as_array() {
        Some(s) if !s.is_empty() => s,
        _ => return usage("steps 는 비어 있지 않은 배열이어야 합니다"),
    };
    let assert_verify = plan["assertions"]["verify"].as_bool().unwrap_or(false);
    // notFoundEmpty 는 선검증이 구조적으로 보장한다 — 계약 표기로 저널에 남긴다.
    let assert_not_found_empty = plan["assertions"]["notFoundEmpty"]
        .as_bool()
        .unwrap_or(true);
    // [#4378 R22] preconditions.inputSha256 — 형식은 여기서(usage), 대조는 읽기 직후.
    // 키가 있는데 타입이 잘못된 경우를 "전제조건 없음"으로 낮추면 CAS 경계가
    // fail-open 된다. 생략만 허용하고, 명시된 값은 반드시 문자열이어야 한다.
    let expected_input_sha = match plan.get("preconditions") {
        None => None,
        Some(serde_json::Value::Object(preconditions)) => match preconditions.get("inputSha256") {
            None => {
                return usage("preconditions 객체에는 inputSha256 하나가 반드시 필요합니다");
            }
            Some(serde_json::Value::String(raw)) => {
                if preconditions.len() != 1 {
                    return usage("preconditions 에는 inputSha256 외 속성을 둘 수 없습니다");
                }
                let normalized = raw.trim().to_ascii_lowercase();
                if normalized.len() != 64 || !normalized.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return usage("preconditions.inputSha256 은 64자리 16진이어야 합니다");
                }
                Some(normalized)
            }
            Some(_) => {
                return usage("preconditions.inputSha256 은 문자열이어야 합니다");
            }
        },
        Some(_) => return usage("preconditions 는 객체여야 합니다"),
    };

    let _cas_lock = match expected_input_sha.as_ref() {
        Some(_) => {
            if let Err(e) = cas_test_synchronize_before_lock() {
                return fail(e);
            }
            match CasPathLock::acquire(Path::new(input)) {
                Ok(lock) => Some(lock),
                Err(e) => {
                    return fail(format!(
                        "입력 문서 CAS 잠금을 얻을 수 없습니다 - {input}: {e}"
                    ))
                }
            }
        }
        None => None,
    };
    let bytes = match fs::read(input) {
        Ok(d) => d,
        Err(e) => return fail(format!("입력을 읽을 수 없습니다 - {}: {}", input, e)),
    };
    // [#4378 R22] CAS — 계획이 세워진 시점의 문서가 아니면 실행 0·저장 0 으로
    // 거절한다(#3905 M1: 두 exit 0 이 편집 하나를 지우는 경합의 차단기).
    let precondition_failure = |expected: &str, actual: String| {
        (
            provenance::marked(
                serde_json::json!({
                    "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                    "planVersion": "1.0",
                    "input": input,
                    "invalid": [{
                        "step": serde_json::Value::Null,
                        "action": "preconditions",
                        "code": "preconditionFailed",
                        "reason": "입력 문서가 계획의 기대 해시와 다릅니다 — 계획 수립 후 문서가 바뀌었습니다. 실행 0·저장 0. 문서를 다시 읽고 재계획하세요 (#3905 CAS).",
                        "expected": expected,
                        "actual": actual,
                    }],
                }),
                "run",
            ),
            EXIT_USAGE,
        )
    };
    if let Some(expected) = expected_input_sha.as_deref() {
        let actual = sha256_hex_of(&bytes);
        if actual != expected {
            return precondition_failure(expected, actual);
        }
        cas_test_mark_checked_and_wait();
    }
    let mut doc = match rhwp::wasm_api::HwpDocument::from_bytes(&bytes) {
        Ok(d) => d,
        Err(e) => return fail(format!("HWP 파싱 실패 - {}", e)),
    };

    // 1) 정적 선검증 — 실행 0. 위반을 전부 모아 한 번에 보고한다(하나 고치면 다음
    //    위반이 나오는 두더지잡기 방지). 판정자는 실행이 쓰는 바로 그 함수들이다.
    let mut name_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    // [#3712] 같은 순회에서 문단 주소도 담는다 — 저널 changedPages 산출 근거.
    let mut name_locs: std::collections::HashMap<String, Vec<(usize, usize)>> =
        std::collections::HashMap::new();
    // [#3719 §6-8] 조건절 fieldEquals 가 볼 **현재 값**. 같은 순회에서 담아 두면
    // 조건 판정이 문서를 다시 훑지 않는다(동명 필드는 선언 순서 = 순번 순서).
    let mut name_values: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for fi in doc.collect_all_fields().iter() {
        if let Some(n) = fi.field.field_name() {
            *name_counts.entry(n.to_string()).or_insert(0) += 1;
            name_locs
                .entry(n.to_string())
                .or_default()
                .push((fi.location.section_index, fi.location.para_index));
            name_values
                .entry(n.to_string())
                .or_default()
                .push(fi.value.clone());
        }
    }
    // `edit fill-fields`·세션 경로와 같은 text-security 판정이다. 계획 실행만
    // 이 경고를 누락하면 선언적 경로가 화면상 같은 필드 이름을 침묵 속에 통과시킨다.
    let all_names: Vec<String> = name_counts.keys().cloned().collect();
    let confusable_groups = rhwp::document_core::text_security::confusable_collisions(&all_names);
    let mut invalid: Vec<serde_json::Value> = Vec::new();
    // [#3721] 선검증이 이미 계산한 값을 미리보기로 모은다 — dry-run 은 이걸 그대로 낸다.
    // (실행 모드에서는 쓰이지 않지만, 판정자와 미리보기가 같은 계산이라 어긋날 수 없다.)
    let mut preview: Vec<serde_json::Value> = Vec::new();

    // [#3719 §6-8] 조건부 step — 조건은 **입력 문서 기준으로 실행 전에 한 번** 판정한다.
    // 실행 중에 다시 보면 선검증이 통과시킨 step 이 실행에서 조건을 잃는(또는 그 반대)
    // 상태가 생겨, "무엇이 왜 안 바뀌었는지"가 저널만 봐서는 재구성되지 않는다.
    // 판정 결과는 Some(사유) = 건너뜀, None = 실행.
    let mut skip_reasons: Vec<Option<String>> = Vec::with_capacity(steps.len());
    for step in steps.iter() {
        match step.get("if") {
            None => skip_reasons.push(None),
            Some(condition) => {
                match evaluate_step_condition(condition, &doc, &name_counts, &name_values) {
                    Ok(reason) => skip_reasons.push(reason),
                    Err(_) => {
                        // 문법 오류는 아래 선검증 루프에서 다시 판정해 invalid 에 담는다
                        // (사유 메시지를 한 곳에서만 만들기 위함) — 여기서는 자리만 채운다.
                        skip_reasons.push(None);
                    }
                }
            }
        }
    }

    for (idx, step) in steps.iter().enumerate() {
        let action = step["action"].as_str().unwrap_or("");
        // [#3719 §6-8] 조건 문법 오류는 계획 자체가 무효다 — invalid 로 즉시 보고한다.
        if let Some(condition) = step.get("if") {
            if let Err(message) =
                evaluate_step_condition(condition, &doc, &name_counts, &name_values)
            {
                invalid
                    .push(serde_json::json!({ "step": idx, "action": action, "reason": message }));
                continue;
            }
        }
        // 조건이 거짓인 step 은 **실행 가능성 검사를 면제**한다. 없는 필드를 채우는
        // step 이라도 애초에 실행되지 않으므로 위반이 아니다 — 여기서 걸러 내지 않으면
        // 조건절은 "쓸 수는 있으나 쓰면 계획이 통과하지 않는" 장식이 된다.
        if let Some(reason) = &skip_reasons[idx] {
            preview.push(serde_json::json!({
                "step": idx, "action": action, "skipped": true, "reason": reason,
            }));
            continue;
        }
        match action {
            "fill_fields" => {
                let Some(data) = step["data"].as_object() else {
                    invalid.push(serde_json::json!({ "step": idx, "action": action,
                        "reason": "data 는 {\"필드이름\":\"값\"} 객체여야 합니다" }));
                    continue;
                };
                let mut targets: Vec<serde_json::Value> = Vec::new();
                for (key, value) in data.iter() {
                    let (name, occurrence) = parse_field_key(key);
                    let total = name_counts.get(name).copied().unwrap_or(0);
                    if total == 0 || occurrence >= total {
                        invalid.push(serde_json::json!({ "step": idx, "action": action,
                            "reason": format!("필드 '{}' 이(가) 없거나 순번이 범위 밖입니다 (동명 {}개)", key, total) }));
                        continue;
                    }
                    targets.push(serde_json::json!({
                        "name": name, "occurrence": occurrence, "sameNameCount": total,
                        "value": value.as_str().map(|v| v.to_string())
                            .unwrap_or_else(|| value.to_string()),
                    }));
                }
                preview.push(
                    serde_json::json!({ "step": idx, "action": action, "targets": targets }),
                );
            }
            "replace_text" => {
                let Some(find) = step["find"].as_str().filter(|s| !s.is_empty()) else {
                    invalid.push(serde_json::json!({ "step": idx, "action": action,
                        "reason": "find (비어 있지 않은 문자열)가 필요합니다" }));
                    continue;
                };
                if !step["replace"].is_string() {
                    invalid.push(serde_json::json!({ "step": idx, "action": action,
                        "reason": "replace (문자열)가 필요합니다" }));
                    continue;
                }
                let case_sensitive = step["caseSensitive"].as_bool().unwrap_or(true);
                let count = doc.grep(find, case_sensitive, None).len();
                match step["occurrence"].as_u64() {
                    Some(n) if (n as usize) >= count => {
                        invalid.push(serde_json::json!({ "step": idx, "action": action,
                            "reason": format!("occurrence {} 이(가) 범위 밖입니다 ('{}' 일치 {}건)", n, find, count) }));
                    }
                    None if count == 0 => {
                        invalid.push(serde_json::json!({ "step": idx, "action": action,
                            "reason": format!("'{}' 일치 0건 — 치환할 곳이 없습니다", find) }));
                    }
                    // occurrence 지정이면 1건만, 아니면 전건 — 실행 분기와 같은 규칙.
                    occurrence => preview.push(serde_json::json!({
                        "step": idx, "action": action, "find": find,
                        "matches": count,
                        "willReplace": if occurrence.is_some() { 1 } else { count },
                    })),
                }
            }
            "set_checkbox" => {
                let Some(n) = step["occurrence"].as_u64() else {
                    invalid.push(serde_json::json!({ "step": idx, "action": action,
                        "reason": "occurrence (0 기준 순번)가 필요합니다" }));
                    continue;
                };
                let count = doc.grep("□", true, None).len();
                if (n as usize) >= count {
                    invalid.push(serde_json::json!({ "step": idx, "action": action,
                        "reason": format!("occurrence {} 이(가) 범위 밖입니다 (빈 체크박스 □ {}건)", n, count) }));
                } else {
                    preview.push(serde_json::json!({ "step": idx, "action": action,
                        "occurrence": n, "available": count }));
                }
            }
            "set_cell" => {
                let (Some(t), Some(r), Some(c), Some(text)) = (
                    step["table"].as_u64(),
                    step["row"].as_u64(),
                    step["col"].as_u64(),
                    step["text"].as_str(),
                ) else {
                    invalid.push(serde_json::json!({ "step": idx, "action": action,
                        "reason": "table·row·col (정수)과 text (문자열)가 필요합니다" }));
                    continue;
                };
                if text.chars().any(|ch| matches!(ch, '\r' | '\n' | '\t')) {
                    invalid.push(serde_json::json!({ "step": idx, "action": action,
                        "reason": "text 에 줄바꿈·탭은 넣을 수 없습니다 (한 줄 값 기록)" }));
                    continue;
                }
                let table = match usize::try_from(t) {
                    Ok(value) => value,
                    Err(_) => {
                        invalid.push(serde_json::json!({ "step": idx, "action": action,
                            "reason": format!("table {} 이(가) 이 플랫폼의 인덱스 범위를 벗어났습니다", t) }));
                        continue;
                    }
                };
                let row = match u16::try_from(r) {
                    Ok(value) => value,
                    Err(_) => {
                        invalid.push(serde_json::json!({ "step": idx, "action": action,
                            "reason": format!("row {} 이(가) 0..65535 범위를 벗어났습니다", r) }));
                        continue;
                    }
                };
                let col = match u16::try_from(c) {
                    Ok(value) => value,
                    Err(_) => {
                        invalid.push(serde_json::json!({ "step": idx, "action": action,
                            "reason": format!("col {} 이(가) 0..65535 범위를 벗어났습니다", c) }));
                        continue;
                    }
                };
                match resolve_table_cell(doc.document(), table, row, col) {
                    Err(e) => {
                        let (CellResolveError::Usage(msg) | CellResolveError::Runtime(msg)) = e;
                        invalid.push(
                            serde_json::json!({ "step": idx, "action": action, "reason": msg }),
                        );
                    }
                    Ok((.., current)) => preview.push(serde_json::json!({
                        "step": idx, "action": action,
                        "table": table, "row": row, "col": col,
                        "currentText": current, "newText": text,
                    })),
                }
            }
            "" => {
                invalid.push(serde_json::json!({ "step": idx, "reason": "action 이 필요합니다" }))
            }
            other => invalid.push(serde_json::json!({ "step": idx, "action": other,
                "reason": format!("알 수 없는 action: {} (fill_fields·replace_text·set_cell·set_checkbox)", other) })),
        }
    }
    if !invalid.is_empty() {
        return (
            provenance::marked(
                serde_json::json!({
                    "schemaVersion": ENVELOPE_SCHEMA_VERSION, "planVersion": "1.0",
                    "input": input, "output": output, "invalid": invalid,
                }),
                "run",
            ),
            EXIT_USAGE,
        );
    }

    // [#3721] dry-run — 선검증만 하고 여기서 끝낸다. 실행도, 저장도 없다.
    // 계획을 *제출 전에* 검사하는 가장 싼 안전장치이고, 미리보기는 위에서 판정자가
    // 이미 계산한 값 그대로라 "검사 결과와 실제 실행이 다를" 여지가 없다.
    if plan["dryRun"].as_bool().unwrap_or(false) {
        return (
            serde_json::json!({
                "schemaVersion": ENVELOPE_SCHEMA_VERSION, "planVersion": "1.0", "dryRun": true,
                "input": input, "output": output,
                "preview": preview, "invalid": [],
                "assertions": { "notFoundEmpty": assert_not_found_empty, "verify": assert_verify },
            }),
            EXIT_OK,
        );
    }

    // 2) 원자 실행 — 전 step 을 인메모리 IR 에만 적용한다. 디스크는 아직 무변경이라
    //    어느 step 이 실패해도 반편집 문서가 남지 않는다.
    let mut journal_steps: Vec<serde_json::Value> = Vec::new();
    let mut changed_paras: Vec<(usize, usize)> = Vec::new();
    for (idx, step) in steps.iter().enumerate() {
        let action = step["action"].as_str().unwrap_or("");
        // [#3719 §6-8] 건너뛴 step 도 저널에 남긴다. 조용히 사라지면 소비자는 "왜 그
        // 칸이 안 바뀌었는지"를 알 방법이 없다 — 조건이 거짓이었다는 사실 자체가 결과다.
        if let Some(reason) = &skip_reasons[idx] {
            journal_steps.push(serde_json::json!({
                "step": idx, "action": action, "skipped": true, "reason": reason,
            }));
            continue;
        }
        match action {
            "fill_fields" => {
                let data = step["data"].as_object().expect("선검증 통과");
                let mut filled: Vec<serde_json::Value> = Vec::new();
                let mut ambiguous: Vec<serde_json::Value> = Vec::new();
                let mut confusable: Vec<serde_json::Value> = Vec::new();
                for (key, value) in data {
                    let value_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    let (name, occurrence) = parse_field_key(key);
                    let total = name_counts.get(name).copied().unwrap_or(0);
                    if occurrence == 0 && total > 1 && !key.contains('[') {
                        ambiguous.push(
                            serde_json::json!({ "name": name, "matched": 1, "total": total }),
                        );
                    }
                    if let Some((_, group)) = confusable_groups
                        .iter()
                        .find(|(_, group)| group.iter().any(|candidate| candidate == name))
                    {
                        let others: Vec<&String> = group
                            .iter()
                            .filter(|candidate| *candidate != name)
                            .collect();
                        confusable.push(serde_json::json!({
                            "name": name,
                            "lookalikes": others,
                            "note": "화면상 구별되지 않는 이름의 누름틀이 이 문서에 함께 있습니다 — 채운 칸이 의도한 칸인지 확인하세요.",
                        }));
                    }
                    if let Err(e) = doc.set_field_value_by_name_at(name, occurrence, &value_str) {
                        return fail(format!("step {}: 필드 '{}' 설정 실패 - {}", idx, key, e));
                    }
                    if let Some(loc) = name_locs.get(name).and_then(|l| l.get(occurrence)) {
                        changed_paras.push(*loc);
                    }
                    filled.push(serde_json::json!({
                        "name": name, "occurrence": occurrence, "value": value_str,
                    }));
                }
                journal_steps.push(serde_json::json!({
                    "step": idx, "action": "fill_fields",
                    "filledCount": filled.len(), "filled": filled,
                    "notFound": [], "ambiguous": ambiguous, "confusable": confusable,
                }));
            }
            "replace_text" => {
                let find = step["find"].as_str().expect("선검증 통과");
                let replace = step["replace"].as_str().expect("선검증 통과");
                let case_sensitive = step["caseSensitive"].as_bool().unwrap_or(true);
                {
                    // [#3712] 치환 전 매치 주소 — 문자열 치환은 문단 인덱스를 밀지 않는다.
                    let all = doc.grep(find, case_sensitive, None);
                    match step["occurrence"].as_u64() {
                        Some(n) => {
                            if let Some(m) = all.get(n as usize) {
                                changed_paras.push((m.section, m.paragraph));
                            }
                        }
                        None => changed_paras.extend(all.iter().map(|m| (m.section, m.paragraph))),
                    }
                }
                let result = match step["occurrence"].as_u64() {
                    Some(n) => doc.replace_nth_native(find, replace, case_sensitive, n as usize),
                    None => doc.replace_all_native(find, replace, case_sensitive),
                };
                let count = match result {
                    Ok(r) => serde_json::from_str::<serde_json::Value>(&r)
                        .ok()
                        .and_then(|v| v["count"].as_u64())
                        .unwrap_or(0),
                    Err(e) => return fail(format!("step {}: 치환 실패 - {:?}", idx, e)),
                };
                journal_steps.push(serde_json::json!({
                    "step": idx, "action": "replace_text",
                    "find": find, "replacedCount": count,
                }));
            }
            "set_checkbox" => {
                let n = step["occurrence"].as_u64().expect("선검증 통과") as usize;
                if let Some(m) = doc.grep("□", true, None).get(n) {
                    changed_paras.push((m.section, m.paragraph));
                }
                let count = match doc.replace_nth_native("□", "☑", true, n) {
                    Ok(r) => serde_json::from_str::<serde_json::Value>(&r)
                        .ok()
                        .and_then(|v| v["count"].as_u64())
                        .unwrap_or(0),
                    Err(e) => return fail(format!("step {}: 체크박스 기록 실패 - {:?}", idx, e)),
                };
                journal_steps.push(serde_json::json!({
                    "step": idx, "action": "set_checkbox",
                    "occurrence": n, "replacedCount": count,
                }));
            }
            "set_cell" => {
                let t = usize::try_from(step["table"].as_u64().expect("선검증 통과"))
                    .expect("선검증 통과");
                let r =
                    u16::try_from(step["row"].as_u64().expect("선검증 통과")).expect("선검증 통과");
                let c =
                    u16::try_from(step["col"].as_u64().expect("선검증 통과")).expect("선검증 통과");
                let text = step["text"].as_str().expect("선검증 통과");
                let keep_style = step["keepStyle"].as_bool().unwrap_or(false);
                // 앞 step 의 편집으로 좌표가 밀릴 수 있어 실행 시점에 재해석한다.
                let (sec, para, ctrl, cell_idx, para_lens, old_text) =
                    match resolve_table_cell(doc.document(), t, r, c) {
                        Ok(v) => v,
                        Err(CellResolveError::Usage(m) | CellResolveError::Runtime(m)) => {
                            return fail(format!("step {}: {}", idx, m));
                        }
                    };
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
                        return fail(format!(
                            "step {}: 셀 비우기 실패(문단 {}) - {:?}",
                            idx, pi, e
                        ));
                    }
                }
                if !text.is_empty() {
                    if let Err(e) = doc.insert_text_in_cell(
                        sec as u32,
                        para as u32,
                        ctrl as u32,
                        cell_idx as u32,
                        0,
                        0,
                        text,
                    ) {
                        return fail(format!("step {}: 셀 쓰기 실패 - {:?}", idx, e));
                    }
                    if !keep_style
                        && !recolor_cell_text_black(doc.document_mut(), sec, para, ctrl, cell_idx)
                    {
                        eprintln!("경고: step {} 셀 글자색을 검정으로 바꾸지 못했습니다.", idx);
                    }
                }
                changed_paras.push((sec, para));
                journal_steps.push(serde_json::json!({
                    "step": idx, "action": "set_cell",
                    "table": t, "row": r, "col": c, "oldText": old_text,
                }));
            }
            _ => unreachable!("선검증이 막는다"),
        }
    }

    // 3) 사후 단언 → 단 한 번 저장. 단언 실패 시 디스크 무변경 — 자연 트랜잭션.
    // [#3712] 눈검증 대상 페이지 — 편집 반영 후 조판 기준. 확정 불가면 null.
    let changed_pages = match doc.pages_covering_paragraphs(&changed_paras) {
        Some(pages) => serde_json::json!(pages),
        None => serde_json::Value::Null,
    };
    let out_format = edit_output_format(&bytes, Some(output));
    let out_bytes = match edit_serialize(&mut doc, out_format) {
        Ok(b) => b,
        Err(e) => return fail(format!("{} 직렬화 실패 - {}", out_format.label(), e)),
    };
    let mut verify_report = serde_json::Value::Null;
    if assert_verify {
        let cross = out_format == EditOutputFormat::Hwp
            && rhwp::parser::detect_format(&bytes) == rhwp::parser::FileFormat::Hwpx;
        let (report, failed) = edit_verify_report(&doc, &out_bytes, cross);
        verify_report = report;
        if failed {
            return (
                provenance::marked(
                    serde_json::json!({
                        "schemaVersion": ENVELOPE_SCHEMA_VERSION, "planVersion": "1.0",
                        "input": input, "output": output,
                        "steps": journal_steps, "verify": verify_report,
                        "error": "verify 단언 실패 — 디스크 무변경",
                    }),
                    "run",
                ),
                3,
            );
        }
    }
    if let Some(expected) = expected_input_sha.as_deref() {
        let latest = match fs::read(input) {
            Ok(bytes) => bytes,
            Err(e) => {
                return fail(format!(
                    "저장 직전 입력을 다시 읽을 수 없습니다 - {input}: {e}"
                ))
            }
        };
        let actual = sha256_hex_of(&latest);
        if actual != expected {
            return precondition_failure(expected, actual);
        }
    }
    if let Err(e) = fs::write(output, &out_bytes) {
        return fail(format!("출력 파일을 쓸 수 없습니다 - {}: {}", output, e));
    }
    (
        provenance::marked(
            serde_json::json!({
                "schemaVersion": ENVELOPE_SCHEMA_VERSION, "planVersion": "1.0",
                "input": input, "output": output, "outputFormat": out_format.label(),
                "steps": journal_steps, "verify": verify_report,
                "changedPages": changed_pages,
                "assertions": { "notFoundEmpty": assert_not_found_empty, "verify": assert_verify },
            }),
            "run",
        ),
        EXIT_OK,
    )
}



/// [#3719 §6-8] step 조건절 판정 — `Ok(None)` = 조건 참(실행), `Ok(Some(사유))` =
/// 조건 거짓(건너뜀), `Err(사유)` = 조건 **문법** 오류(계획 자체가 무효).
///
/// 거짓과 문법 오류를 같은 축으로 접으면 오타 하나가 "조건이 거짓이었다"로 둔갑해
/// 계획이 조용히 아무 일도 하지 않고 성공을 보고한다. 그래서 두 축을 나눈다 —
/// 거짓은 정상 판정(exit 0, skipped 저널), 문법 오류는 `invalid` + exit 2 다.
///
/// 판정은 **입력 문서** 기준이다. 앞 step 의 편집 결과를 조건이 보게 하면 선검증(실행 전)
/// 과 실행(편집 후)이 서로 다른 답을 낼 수 있고, 그러면 "검사를 통과한 계획이 실행에서
/// 다르게 동작"한다.
pub(crate) fn evaluate_step_condition(
    condition: &serde_json::Value,
    doc: &rhwp::wasm_api::HwpDocument,
    name_counts: &std::collections::HashMap<String, usize>,
    name_values: &std::collections::HashMap<String, Vec<String>>,
) -> Result<Option<String>, String> {
    let Some(map) = condition.as_object() else {
        return Err(
            "if 는 { fieldExists | fieldEquals | textFound } 중 하나를 담은 객체여야 합니다"
                .to_string(),
        );
    };
    // 조건 두 개를 나열하면 and 인지 or 인지가 계획서 어디에도 적혀 있지 않다.
    // 추측해서 실행하는 대신 거절한다 — 되돌릴 수 없는 쓰기의 전제 조건이다.
    if map.len() != 1 {
        return Err(format!(
            "if 는 조건을 정확히 하나만 담아야 합니다 (현재 {}개: {}) — 둘 이상은 and/or 가 정의돼 있지 않습니다",
            map.len(),
            map.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    let (key, value) = map.iter().next().expect("길이 1");
    match key.as_str() {
        "fieldExists" => {
            let Some(spec) = value.as_str().filter(|s| !s.is_empty()) else {
                return Err(
                    "if.fieldExists 는 비어 있지 않은 필드 이름 문자열이어야 합니다".to_string(),
                );
            };
            let (name, occurrence) = parse_field_key(spec);
            let total = name_counts.get(name).copied().unwrap_or(0);
            if occurrence < total {
                Ok(None)
            } else {
                Ok(Some(format!(
                    "조건 fieldExists '{}' 불충족 — 문서의 동명 누름틀 {}개",
                    spec, total
                )))
            }
        }
        "fieldEquals" => {
            let Some(operand) = value.as_object() else {
                return Err(
                    "if.fieldEquals 는 {\"name\":<필드 이름>, \"value\":<비교값>} 객체여야 합니다"
                        .to_string(),
                );
            };
            if let Some(unknown) = operand
                .keys()
                .find(|k| k.as_str() != "name" && k.as_str() != "value")
            {
                return Err(format!(
                    "if.fieldEquals 에 알 수 없는 키: {} (name·value 만 받습니다)",
                    unknown
                ));
            }
            let (Some(spec), Some(expected)) = (
                operand.get("name").and_then(|v| v.as_str()),
                operand.get("value").and_then(|v| v.as_str()),
            ) else {
                return Err("if.fieldEquals 의 name·value 는 둘 다 문자열이어야 합니다".to_string());
            };
            if spec.is_empty() {
                return Err("if.fieldEquals 의 name 이 비어 있습니다".to_string());
            }
            let (name, occurrence) = parse_field_key(spec);
            match name_values.get(name).and_then(|v| v.get(occurrence)) {
                Some(actual) if actual == expected => Ok(None),
                Some(actual) => Ok(Some(format!(
                    "조건 fieldEquals '{}' == '{}' 불충족 — 현재값 '{}'",
                    spec, expected, actual
                ))),
                None => Ok(Some(format!(
                    "조건 fieldEquals '{}' == '{}' 불충족 — 해당 누름틀이 없습니다",
                    spec, expected
                ))),
            }
        }
        "textFound" => {
            let Some(needle) = value.as_str().filter(|s| !s.is_empty()) else {
                return Err("if.textFound 는 비어 있지 않은 문자열이어야 합니다".to_string());
            };
            // 한 건만 확인하면 되므로 limit 1 — 존재 판정에 전건 수집은 낭비다.
            if doc.grep(needle, true, Some(1)).is_empty() {
                Ok(Some(format!(
                    "조건 textFound '{}' 불충족 — 본문에서 찾지 못했습니다",
                    needle
                )))
            } else {
                Ok(None)
            }
        }
        other => Err(format!(
            "알 수 없는 조건: {} (fieldExists·fieldEquals·textFound)",
            other
        )),
    }
}
