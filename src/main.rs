use std::env;
use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::process;

mod agent_profiles;
mod anchor_log;
mod atomic_file;
mod audit_standard;
mod capsule_sign;
mod disclose;
mod lineage_bundle;
mod mcp_serve;
mod policy_gate;
mod settle;
#[path = "main/mcp_meta.rs"]
mod mcp_meta;
pub(crate) use mcp_meta::*;
#[path = "main/schema.rs"]
mod schema;
pub(crate) use schema::*;
#[path = "main/export.rs"]
mod export;
pub(crate) use export::*;
#[path = "main/convert.rs"]
mod convert;
pub(crate) use convert::*;
#[path = "main/batch.rs"]
mod batch;
pub(crate) use batch::*;
#[path = "main/edit.rs"]
mod edit;
pub(crate) use edit::*;
#[path = "main/governance.rs"]
mod governance;
pub(crate) use governance::*;
#[path = "main/inspect.rs"]
mod inspect;
pub(crate) use inspect::*;
use rhwp::provenance;
use rhwp::schema_registry::ENVELOPE_SCHEMA_VERSION;

/// [#2707] CLI 종료 코드 계약 — 성공.
const EXIT_OK: i32 = 0;
/// [#2707] CLI 종료 코드 계약 — 런타임 실패(읽기·파싱·렌더·쓰기).
const EXIT_RUNTIME: i32 = 1;
/// [#2707] CLI 종료 코드 계약 — 사용법 오류(인자 없음, 알 수 없는 옵션/명령, 페이지 범위 초과).
///
/// 3(`--verify` IR 차이)·4(`--verify-pages` 페이지 수 불일치)는
/// `mydocs/manual/cli_commands.md` 에 이미 문서화된 계약이므로 상수화 대상에서 제외하고
/// 기존 `process::exit(3)`/`process::exit(4)` 호출부를 그대로 둔다.
const EXIT_USAGE: i32 = 2;

/// [#2707] 명령 함수가 돌려준 종료 코드를 프로세스 종료 코드로 전파한다.
///
/// 0이면 아무것도 하지 않아 `main` 이 정상 종료하고, 그 외에는 즉시 그 코드로 종료한다.
fn exit_with(exit_code: i32) {
    if exit_code != EXIT_OK {
        process::exit(exit_code);
    }
}

// ============================================================================
// 전역 비밀번호 (--password / --password-stdin, --output-password / --output-password-stdin)
//
// main() 의 pre-scan 이 설정하고 load_document/load_document_core 가 읽는다.
// CLI는 단일 스레드이므로 thread_local 로 전역 상태를 안전하게 전달한다.
// 명령 함수 시그니처를 일일이 바꾸지 않아도 일반 문서 로드 명령에
// 비밀번호를 적용할 수 있다.
// ============================================================================

thread_local! {
    static CLI_PASSWORD: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static CLI_OUTPUT_PASSWORD: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn set_cli_password(pw: Option<String>) {
    CLI_PASSWORD.with(|c| *c.borrow_mut() = pw);
}

fn cli_password() -> Option<String> {
    CLI_PASSWORD.with(|c| c.borrow().clone())
}

fn set_cli_output_password(pw: Option<String>) {
    CLI_OUTPUT_PASSWORD.with(|c| *c.borrow_mut() = pw);
}

fn cli_output_password() -> Option<String> {
    CLI_OUTPUT_PASSWORD.with(|c| c.borrow().clone())
}

/// 문서 로드 에러 — 비밀번호 필요/불일치/기타를 구분해 종료 코드를 다르게 매핑.
enum LoadError {
    /// 암호 문서인데 비밀번호가 제공되지 않음 (EXIT_USAGE)
    NeedPassword,
    /// 비밀번호 불일치 (EXIT_RUNTIME)
    WrongPassword,
    /// 그 외 파싱 오류 (EXIT_RUNTIME)
    Other(String),
}

impl LoadError {
    /// stderr 에 메시지를 출력하고 매핑된 종료 코드를 반환한다.
    fn report(self) -> i32 {
        match self {
            LoadError::NeedPassword => {
                eprintln!("오류: 비밀번호가 필요한 암호 문서입니다 (--password <pw> 로 전달).");
                EXIT_USAGE
            }
            LoadError::WrongPassword => {
                eprintln!("오류: 비밀번호가 일치하지 않거나 암호화 데이터가 손상되었습니다.");
                EXIT_RUNTIME
            }
            LoadError::Other(msg) => {
                eprintln!("오류: 문서 파싱 실패 - {}", msg);
                EXIT_RUNTIME
            }
        }
    }
}

/// HwpError Display 메시지에서 비밀번호 관련 에러를 분류한다.
/// CryptoError::WrongPassword → "...비밀번호가 일치하지 않...",
/// ParseError::EncryptedDocument → "...비밀번호가 필요한 암호 문서..." 가
/// HwpError::InvalidFile 로 래핑돼 전해지므로 부분문자열로 판별한다.
fn classify_hwp_error(msg: &str) -> LoadError {
    if msg.contains("비밀번호가 일치하지 않") {
        LoadError::WrongPassword
    } else if msg.contains("비밀번호가 필요한 암호 문서") {
        LoadError::NeedPassword
    } else {
        LoadError::Other(msg.to_string())
    }
}

/// HwpDocument 로드. 전역 비밀번호가 설정돼 있으면 비밀번호 경로로 연다.
fn load_document(data: &[u8]) -> Result<rhwp::wasm_api::HwpDocument, LoadError> {
    let result = match cli_password() {
        Some(pw) => rhwp::wasm_api::HwpDocument::from_bytes_with_password(data, pw.as_bytes()),
        None => rhwp::wasm_api::HwpDocument::from_bytes(data),
    };
    result.map_err(|e| classify_hwp_error(&e.to_string()))
}

/// DocumentCore 로드 (export-pdf/export-hml 등). 동일 분기.
fn load_document_core(data: &[u8]) -> Result<rhwp::document_core::DocumentCore, LoadError> {
    let result = match cli_password() {
        Some(pw) => {
            rhwp::document_core::DocumentCore::from_bytes_with_password(data, pw.as_bytes())
        }
        None => rhwp::document_core::DocumentCore::from_bytes(data),
    };
    result.map_err(|e| classify_hwp_error(&e.to_string()))
}

/// `batch` 명령이 실제로 보이면, 그 뒤·앞 어느 위치의 전역 인증 옵션도 거부한다.
fn has_global_auth_option(args: &[String]) -> bool {
    args.iter().skip(1).any(|arg| {
        matches!(
            arg.as_str(),
            "--password" | "--password-stdin" | "--output-password" | "--output-password-stdin"
        )
    })
}

/// args 전체를 스캔해 입력·출력 인증 옵션을 떼어낸다.
///
/// 뽑아낸 입력 암호와 출력 암호는 이 함수 안에서 thread-local 상태로 소비하고,
/// 반환값에는 해당 토큰이 제거된 args 만 담는다. 두 stdin 옵션을 같이 사용하면
/// stdin 첫 줄은 입력, 둘째 줄은 출력 암호로 고정한다.
///
/// 이름과 반환 형태가 "정제된 args" 인 것은 의도적이다. 비밀번호를 반환값(과거의
/// `(args, password)` 튜플)에 싣거나 함수 이름에 `password` 를 두면 CodeQL
/// `rust/cleartext-logging` 이 이 호출의 결과 전체를 민감 데이터로 보고, 비밀번호
/// 토큰이 이미 제거된 args 를 쓰는 오류·진단 출력까지 sink 로 분류한다
/// (PR #3405 검토에서 41건 과탐지로 확인, PR #3644 에서 alert #119 로 재발).
/// 반환 경로에 비밀번호가 남지 않으므로 이 분류는 실제 유출 경로가 아니다.
fn strip_global_auth_options(mut args: Vec<String>) -> Result<Vec<String>, i32> {
    let mut password: Option<String> = None;
    let mut output_password: Option<String> = None;
    let mut password_stdin = false;
    let mut output_password_stdin = false;
    let mut i = 1; // args[0] 은 프로그램 경로
    while i < args.len() {
        match args[i].as_str() {
            "--password" => {
                if password.is_some() {
                    eprintln!("오류: 비밀번호 옵션은 한 번만 지정할 수 있습니다.");
                    return Err(EXIT_USAGE);
                }
                if i + 1 >= args.len() {
                    eprintln!("오류: --password 뒤에 비밀번호가 필요합니다.");
                    return Err(EXIT_USAGE);
                }
                password = Some(args[i + 1].clone());
                args.drain(i..=i + 1);
            }
            "--password-stdin" => {
                if password.is_some() || password_stdin {
                    eprintln!("오류: 비밀번호 옵션은 한 번만 지정할 수 있습니다.");
                    return Err(EXIT_USAGE);
                }
                password_stdin = true;
                args.remove(i);
            }
            "--output-password" => {
                if output_password.is_some() || output_password_stdin {
                    eprintln!("오류: 출력 비밀번호 옵션은 한 번만 지정할 수 있습니다.");
                    return Err(EXIT_USAGE);
                }
                if i + 1 >= args.len() {
                    eprintln!("오류: --output-password 뒤에 비밀번호가 필요합니다.");
                    return Err(EXIT_USAGE);
                }
                output_password = Some(args[i + 1].clone());
                args.drain(i..=i + 1);
            }
            "--output-password-stdin" => {
                if output_password.is_some() || output_password_stdin {
                    eprintln!("오류: 출력 비밀번호 옵션은 한 번만 지정할 수 있습니다.");
                    return Err(EXIT_USAGE);
                }
                output_password_stdin = true;
                args.remove(i);
            }
            _ => i += 1,
        }
    }

    if password_stdin || output_password_stdin {
        let mut stdin = String::new();
        if let Err(error) = std::io::stdin().read_to_string(&mut stdin) {
            eprintln!("오류: 표준 입력에서 비밀번호 읽기 실패 - {}", error);
            return Err(EXIT_RUNTIME);
        }
        let mut lines = stdin.lines();
        if password_stdin {
            password = Some(lines.next().unwrap_or_default().to_string());
        }
        if output_password_stdin {
            output_password = Some(lines.next().unwrap_or_default().to_string());
        }
    }
    if let Some(value) = output_password.as_deref() {
        if value.is_empty() || value.len() > 4096 || value.contains(['\r', '\n']) {
            eprintln!("오류: 출력 비밀번호는 빈 값·줄바꿈 없이 UTF-8 4096바이트 이하여야 합니다.");
            return Err(EXIT_USAGE);
        }
    }
    set_cli_password(password);
    set_cli_output_password(output_password);
    Ok(args)
}

fn main() {
    let raw_args: Vec<String> = env::args().collect();
    if is_batch_invocation(&raw_args) && has_global_auth_option(&raw_args) {
        eprintln!(
            "오류: batch 는 --password·--password-stdin·--output-password·--output-password-stdin 을 지원하지 않습니다. stdin 은 파일 경로 목록 전용입니다."
        );
        process::exit(EXIT_USAGE);
    }
    // 전역 인증 pre-scan: 어느 위치든 입력/출력 비밀번호 옵션을 뽑아낸다.
    // 비밀번호는 pre-scan 안에서 thread-local 상태로 들어가고 여기로는 돌아오지 않는다.
    let args = match strip_global_auth_options(raw_args) {
        Ok(v) => v,
        Err(code) => process::exit(code),
    };

    match args.get(1).map(|s| s.as_str()) {
        Some("--help") | Some("-h") => print_help(),
        Some("--version") | Some("-V") => println!("rhwp v{}", rhwp::version()),
        Some("export-svg") => exit_with(export_svg(&args[2..])),
        Some("export-render-tree") => exit_with(export_render_tree(&args[2..])),
        Some("export-structure") => exit_with(export_structure(&args[2..])),
        Some("export-png") => exit_with(export_png(&args[2..])),
        Some("export-pdf") => exit_with(export_pdf(&args[2..])),
        Some("export-text") => exit_with(export_text(&args[2..])),
        Some("export-markdown") => exit_with(export_markdown(&args[2..])),
        Some("export-tables") => exit_with(export_tables(&args[2..])),
        Some("table-to-csv") => exit_with(table_to_csv(&args[2..])),
        Some("csv-to-table") => exit_with(csv_to_table(&args[2..])),
        Some("chart-to-csv") => exit_with(chart_to_csv(&args[2..])),
        Some("csv-to-chart") => exit_with(csv_to_chart(&args[2..])),
        Some("export-hwpx") => exit_with(export_hwpx(&args[2..])),
        Some("export-hml") => export_hml(&args[2..]),
        Some("export-doclang") => exit_with(export_doclang(&args[2..])),
        Some("export-ir-schema") => exit_with(cmd_export_ir_schema(&args[2..])),
        Some("export-capabilities-schema") => exit_with(cmd_export_capabilities_schema(&args[2..])),
        Some("export-ontology") => exit_with(cmd_export_ontology(&args[2..])),
        Some("capabilities") => exit_with(show_capabilities(&args[2..])),
        Some("export-provenance-map") => exit_with(export_provenance_map(&args[2..])),
        Some("export-agent-manifest") => exit_with(cmd_export_agent_manifest(&args[2..])),
        Some("mcp-serve") => exit_with(mcp_serve::run(&args[2..])),
        Some("batch") => exit_with(run_batch(&args[2..])),
        Some("scan") => exit_with(cmd_scan(&args[2..])),
        Some("info") => exit_with(show_info(&args[2..])),
        Some("digest") => exit_with(digest_document(&args[2..])),
        Some("dump") => exit_with(dump_controls(&args[2..])),
        Some("dump-note-shape") => exit_with(dump_note_shape(&args[2..])),
        Some("dump-endnote-lines") => exit_with(dump_endnote_lines(&args[2..])),
        Some("dump-pages") => exit_with(dump_pages(&args[2..])),
        Some("dump-extents") => exit_with(dump_extents(&args[2..])),
        Some("diag") => exit_with(diag_document(&args[2..])),
        Some("search") => exit_with(search_document(&args[2..])),
        Some("inspect") => exit_with(inspect_command(&args[2..])),
        Some("extract-data") => exit_with(extract_data_command(&args[2..])),
        Some("convert") => exit_with(convert_hwp(&args[2..])),
        Some("extract-pages") => exit_with(extract_pages(&args[2..])),
        Some("build-from-ingest") => exit_with(build_from_ingest(&args[2..])),
        Some("hwp5-inventory") => exit_with(rhwp::diagnostics::hwp5_inventory::run(&args[2..])),
        Some("hwp5-inventory-diff") => {
            exit_with(rhwp::diagnostics::hwp5_inventory_diff::run(&args[2..]))
        }
        Some("hwp5-contract-analyze") => {
            exit_with(rhwp::diagnostics::hwp5_contract_analyze::run(&args[2..]))
        }
        Some("hwp5-ctrl-data-trace") => {
            exit_with(rhwp::diagnostics::hwp5_ctrl_data_trace::run(&args[2..]))
        }
        Some("hwp5-contract-probe") => {
            exit_with(rhwp::diagnostics::hwp5_contract_probe::run(&args[2..]))
        }
        Some("hwp5-table-probe") => exit_with(rhwp::diagnostics::hwp5_table_probe::run(&args[2..])),
        Some("hwp5-mel-personnel-probe") => {
            exit_with(rhwp::diagnostics::hwp5_mel_personnel_probe::run(&args[2..]))
        }
        Some("hwp5-borderfill-diagonal-probe") => exit_with(
            rhwp::diagnostics::hwp5_borderfill_diagonal_probe::run(&args[2..]),
        ),
        Some("hwp5-first-para-control-probe") => exit_with(
            rhwp::diagnostics::hwp5_first_para_control_probe::run(&args[2..]),
        ),
        Some("hwp5-anchor-trace") => {
            exit_with(rhwp::diagnostics::hwp5_anchor_trace::run(&args[2..]))
        }
        Some("hwp5-char-shape-audit") => {
            exit_with(rhwp::diagnostics::hwp5_char_shape_audit::run(&args[2..]))
        }
        Some("hwp5-cell-header-probe") => {
            exit_with(rhwp::diagnostics::hwp5_cell_header_probe::run(&args[2..]))
        }
        Some("dump-records") => exit_with(dump_raw_records(&args[2..])),
        Some("test-shape") => exit_with(test_shape_roundtrip(&args[2..])),
        Some("test-caption") => exit_with(test_caption(&args[2..])),
        Some("gen-table") => exit_with(gen_table(&args[2..])),
        Some("gen-pua") => exit_with(gen_pua_test(&args[2..])),
        Some("test-field") => exit_with(test_field_roundtrip(&args[2..])),
        Some("ir-diff") => exit_with(ir_diff(&args[2..])),
        Some("ir-sweep") => exit_with(ir_sweep(&args[2..])),
        Some("dump-anchors") => exit_with(dump_anchors(&args[2..])),
        Some("dump-carets") => exit_with(dump_carets(&args[2..])),
        Some("verify") => exit_with(cmd_verify(&args[2..])),
        Some("hwpx-roundtrip") => rhwp::diagnostics::hwpx_roundtrip_batch::run(&args[2..]),
        Some("hwp5-roundtrip") => rhwp::diagnostics::hwp5_roundtrip_batch::run(&args[2..]),
        Some("render-diff") => rhwp::diagnostics::render_geom_diff::run(&args[2..]),
        Some("measure-width") => exit_with(rhwp::diagnostics::text_width_probe::run(&args[2..])),
        Some("core-pages") => exit_with(rhwp::diagnostics::core_pages_probe::run(&args[2..])),
        Some("bench") => exit_with(rhwp::diagnostics::bench::run(&args[2..])),
        Some("thumbnail") => exit_with(extract_thumbnail(&args[2..])),
        Some("fields") => exit_with(show_fields(&args[2..])),
        Some("template-entity") => exit_with(cmd_template_entity(&args[2..])),
        Some("explain") => exit_with(explain_document(&args[2..])),
        Some("edit") => exit_with(run_edit(&args[2..])),
        Some("run") => exit_with(cmd_run_plan(&args[2..])),
        Some("replay") => exit_with(cmd_replay(&args[2..])),
        Some("audit") => exit_with(cmd_audit(&args[2..])),
        Some("lineage") => exit_with(cmd_lineage(&args[2..])),
        Some("keygen") => exit_with(cmd_keygen(&args[2..])),
        Some("verify-signature") => exit_with(cmd_verify_signature(&args[2..])),
        Some("harness") => exit_with(cmd_harness(&args[2..])),
        // [#4537] 통합 판정은 **읽기 전용**이라 쓰기 명령(harness)과 표면을 나눈다 —
        // capabilities 의 category 가 도구 주석(readOnlyHint)의 교차 검증 원천이므로,
        // 한 명령이 쓰기·읽기를 겸하면 MCP 주석 계약이 성립하지 않는다.
        Some("harness-status") => exit_with(cmd_harness_status(&args[2..])),
        Some("anchor") => exit_with(cmd_anchor(&args[2..])),
        Some("gate") => exit_with(cmd_gate(&args[2..])),
        Some("bundle") => exit_with(cmd_bundle(&args[2..])),
        Some("disclose") => exit_with(cmd_disclose(&args[2..])),
        Some("settle") => exit_with(cmd_settle(&args[2..])),
        Some("audit-report") => exit_with(cmd_audit_report(&args[2..])),
        Some("recall-scope") => exit_with(cmd_recall_scope(&args[2..])),
        Some("conformance") => exit_with(cmd_conformance(&args[2..])),
        // [#3719 §6-4] 계획을 *만드는* 쪽의 정답지 — `run` 바로 옆에 둔다.
        Some("export-plan-schema") => exit_with(cmd_export_plan_schema(&args[2..])),
        // [#2707] 알 수 없는 명령·명령 누락은 사용법 오류다. 표준 CLI 관례대로 stderr 로 안내하고
        // 종료 코드 2로 끝낸다(기존에는 stdout + 0이라 오타 낸 명령이 스크립트에서 성공으로 보였다).
        other => {
            // [#4220 T4] 수복 한 줄은 stderr 마지막 줄이어야 하므로(소비자는 마지막
            // `수복: ` 줄 하나만 파싱한다) 산문을 모두 낸 뒤에 방출한다. 두 부류만
            // 결정론적이다: 확신 교정(임계 내 오타)과 명령 누락(발견 경로는 언제나
            // capabilities). 임계 밖 오타는 수복 줄도 침묵한다 — 오제안 0.
            let recovery: Option<(String, &str)> = match other {
                Some(command) => {
                    eprintln!("오류: 알 수 없는 명령입니다 - {}", command);
                    // [#3694] did-you-mean — 후보는 capabilities 단일 출처. 이름 환각을
                    // 교정 단서 없이 돌려보내면 경량 에이전트는 맹목 재시도 루프에 빠진다.
                    let names = capabilities_command_names();
                    let hint = closest_name(command, names.iter().map(String::as_str));
                    if let Some(hint) = &hint {
                        eprintln!("힌트: 가장 가까운 명령은 '{hint}' 입니다");
                    }
                    hint.map(|h| (h, "요청한 이름이 없음 — 가장 가까운 실존 명령으로 교정"))
                }
                None => {
                    eprintln!("오류: 명령을 지정해주세요.");
                    Some((
                        "capabilities".to_string(),
                        "명령이 지정되지 않음 — 실행 가능한 명령 목록·계약은 capabilities 가 자기서술",
                    ))
                }
            };
            eprintln!("rhwp v{}", rhwp::version());
            eprintln!("사용법: rhwp <명령> [옵션]");
            eprintln!("'rhwp --help'로 자세한 사용법을 확인하세요.");
            if let Some((name, why)) = recovery {
                eprint_usage_recovery(&name, None, why);
            }
            process::exit(EXIT_USAGE);
        }
    }
}

/// [#3263] `capabilities --mcp` — MCP 도구 정의 생성.
///
/// MCP 서버 저자(및 함수 호출 클라이언트)가 도구 이름·설명·입력 JSON Schema·실행 배선을
/// 손으로 옮겨 적지 않게 한다. `--json` 계약을 가진 명령이 늘면
/// `capabilities_mcp_covers_every_json_command` 가 누락을 잡는다.
const EDIT_SUBCOMMANDS: [(&str, &str); 6] = [
    (
        "fill-fields",
        "누름틀(필드) 값 채우기 — --data 이름=값, 같은 이름은 [k] 순번 지목",
    ),
    (
        "replace-text",
        "본문 일괄 치환 — --find/--replace, --occurrence 로 k번째만",
    ),
    ("set-cell", "표 셀 텍스트 기록 — --table/--row/--col/--text"),
    (
        "insert-image",
        "도장·서명 그림 삽입 — --image/--page/--x/--y (HWPUNIT)",
    ),
    (
        "redact",
        "개인정보 마스킹 — --kind 선택, findings 봉투, --no-raw",
    ),
    ("sanitize", "메타데이터 제거 — removed 봉투, --in-place"),
];

const INSPECT_SUBCOMMANDS: [(&str, &str); 3] = [
    (
        "hidden-text",
        "은닉 텍스트 탐지 — --threshold-pt 임계·--include-offpage 쪽 밖",
    ),
    (
        "injection",
        "프롬프트 주입 신호 신고 — 문서는 고치지 않고 표시만 한다",
    ),
    (
        "unicode",
        "유니코드 기만 판정 — confusable·bidi·비가시 문자, --kind 필터",
    ),
];

/// 하위 명령 배열을 해당 부모 항목에 단다. 항목 정의 자리(cmd_json 호출)를 건드리지
/// 않는 후처리인 이유: 저 vec 은 거의 모든 표면 PR 이 지나는 자리라, 삽입 지점을
/// 밖으로 빼야 병렬 PR 과의 충돌면이 줄어든다.
fn attach_subcommands(commands: &mut [serde_json::Value]) {
    for entry in commands.iter_mut() {
        let subs: &[(&str, &str)] = match entry["name"].as_str() {
            Some("edit") => &EDIT_SUBCOMMANDS,
            Some("inspect") => &INSPECT_SUBCOMMANDS,
            _ => continue,
        };
        let list: Vec<serde_json::Value> = subs
            .iter()
            .map(|(name, summary)| serde_json::json!({ "name": name, "summary": summary }))
            .collect();
        entry["subcommands"] = serde_json::json!(list);
    }
}

pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// [#3694] 후보 중 가장 가까운 이름 — 임계(길이 대비 1/3, 최소 1·최대 3) 초과면 None.
/// 오제안 0 원칙: 애매하면 제안하지 않는 편이 경량 에이전트에게 안전하다.
pub(crate) fn closest_name<'a, I: IntoIterator<Item = &'a str>>(
    input: &str,
    candidates: I,
) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        let d = levenshtein(input, c);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, c));
        }
    }
    let (d, name) = best?;
    let cap = (input.chars().count() / 3).clamp(1, 3);
    (d <= cap).then(|| name.to_string())
}

/// [#4220 T4] 사용법 오류(exit 2)의 stderr **마지막 줄**에 싣는 정형 수복 한 줄.
///
/// 문법: `수복: ` 접두어 + 한 줄 JSON `{"nextCall":{"name":<명령>,"subcommand"?:<하위>,"why":<이유>}}`.
/// `nextCall` 어휘는 MCP 오류 봉투(R72, `tool_error_with_next`)와 같다 — CLI 와 MCP 가
/// 같은 모양을 쓰면 소비자가 한 어휘로 수복 루프를 짠다. 계약 3면:
///
/// 1. **오제안 0(R72)** — 다음 호출이 결정론적으로 정해지는 실패 부류에서만 호출한다.
///    애매하면 이 줄 자체가 없어야 하므로, 호출부가 확신 판정(#3694 임계 등)을 먼저 한다.
/// 2. **`name` 실존** — 호출부 책임이고 계약 테스트(`tests/nextcall_cli_contract.rs`)가
///    capabilities 단일 출처와 대조해 고정한다. `arguments` 는 싣지 않는다: CLI 는
///    호출자의 나머지 argv 가 옳다고 검증한 바 없고(오제안 0 은 인자에도 적용된다),
///    비밀번호 같은 민감 인자를 stderr 로 되울리지 않는 뜻도 겸한다.
/// 3. **stdout 무침해** — 실패 3면 계약(#2707: exit 2·stdout 0 B·stderr 안내)에
///    stderr 한 줄만 더하는 추가 전용 확장이다. 산문(오류·힌트·사용법)을 모두 낸 뒤
///    마지막에 호출해야 한다 — 소비자는 "마지막 `수복: ` 줄 하나"만 파싱한다.
fn eprint_usage_recovery(next_command: &str, subcommand: Option<&str>, why: &str) {
    let mut next = serde_json::json!({ "name": next_command, "why": why });
    if let Some(sub) = subcommand {
        next["subcommand"] = serde_json::json!(sub);
    }
    eprintln!("수복: {}", serde_json::json!({ "nextCall": next }));
}

/// [#3828 B1] `capabilities --search <키워드...> [--json]` — commands[].name·summary 를
/// 대소문자 무시 부분 문자열로 필터한다. 결정론적 매칭(유사도 점수·LLM 없음).
///
/// 키워드를 공백으로 여러 개 주면(예: `--search "표 병합"`) **AND** 조건으로 좁힌다 —
/// 검색 도구의 통상 관례(모든 검색어를 만족해야 좁혀진다)를 따르고, 사용자가 한
/// 단어로는 너무 넓은 결과를 받고 두 번째 단어로 더 좁히고 싶을 때 OR 보다 AND 가
/// 직관과 맞는다. OR 이 필요하면 `--search` 를 두 번 호출하면 된다(별도 결과 두 묶음).
/// [#3787 S1] `export-provenance-map` — 어느 명령의 어느 봉투 필드가 **문서에서 온
/// 값**인지의 기계 가독 지도.
///
/// 봉투 표지(`untrustedContent`/`untrustedFields`)는 한 봉투가 지금 무엇을 담았는지만
/// 말한다. 에이전트 프레임워크가 **호출 전에** 정책을 세우려면(예: 이 필드는 절대
/// 프롬프트에 이어 붙이지 않는다) 전체 지도가 필요하다. 그리고 이 지도가 있어야
/// `tests/provenance_contract.rs` 의 드리프트 가드를 걸 수 있다 — 선언 없는 계약은
/// 시간이 지나면 조용히 거짓말이 된다.
fn export_provenance_map(args: &[String]) -> i32 {
    let mut json_mode = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json_mode = true,
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
    }

    let map = provenance::map_json(&rhwp::version());
    if json_mode {
        println!("{}", provenance::marked(map, "export-provenance-map"));
        return EXIT_OK;
    }

    println!("rhwp 봉투 출처 지도 (문서 파생 = 데이터, 지시 아님)");
    println!();
    for entry in provenance::MAP {
        if entry.untrusted.is_empty() {
            println!("  {} — 문서 파생 필드 없음", entry.command);
            continue;
        }
        println!("  {}", entry.command);
        for field in entry.untrusted {
            println!("      {}  ← {}", field.path, field.origin);
        }
    }
    println!();
    println!("기계 계약은 --json 을 쓰세요.");
    EXIT_OK
}

fn print_help() {
    println!("rhwp v{} - HWP 파일 뷰어", rhwp::version());
    println!();
    println!("사용법: rhwp <명령> [옵션]");
    println!();
    println!("전역 옵션 (일반 HWP5 열기·내보내기·변환 명령):");
    println!("      --password <pw>         EncryptVersion 4 암호 문서 열기");
    println!("      --password-stdin        표준 입력 첫 줄에서 비밀번호 읽기 (권장)");
    println!("                              --password 값은 프로세스 목록에 노출될 수 있음");
    println!();
    println!("명령:");
    println!("  export-svg <파일.hwp|파일.hwpx|파일.hml> [옵션]");
    println!("      HWP/HWPX/HML 문서를 SVG로 내보내기");
    println!();
    println!("      -o, --output <폴더>     출력 폴더 (기본: output/)");
    println!("      -p, --page <번호>       특정 페이지만 내보내기 (0부터 시작)");
    println!(
        "      --profile <프로필>      layer 출력 프로필: screen|print|high-quality|fast-preview"
    );
    println!("      --show-para-marks       문단부호(↵/↓) 표시");
    println!("      --show-control-codes    조판부호 보이기 (문단부호 + 개체 마커 등)");
    println!("      --debug-overlay         디버그 오버레이 (문단/표 경계 + 인덱스 라벨)");
    println!("      --respect-vpos-reset    LINE_SEG vpos=0 리셋을 단/페이지 강제 경계로 처리");
    println!("      --show-grid[=Nmm]       격자 오버레이 (기본: 1mm, 예: --show-grid=3mm)");
    println!("      --grid-origin=X,Y|auto  격자 종이 기준 위치 (예: --grid-origin=15mm,20mm)");
    println!("      --font-style            @font-face local() 참조 삽입 (폰트 데이터 미포함)");
    println!("      --embed-fonts           폰트 서브셋 임베딩 (사용 글자만 base64)");
    println!("      --embed-fonts=full      폰트 전체 임베딩 (base64)");
    println!("      --font-path <경로>      폰트 파일 탐색 경로 (여러 번 지정 가능)");
    println!("      --json                  산출물 매니페스트를 JSON으로 stdout에 출력");
    println!();
    println!("  export-render-tree <파일.hwp> [옵션]");
    println!("      페이지별 render tree bbox JSON을 내보내기 (레이아웃 시각 분석용)");
    println!();
    println!("      -o, --output <폴더>     출력 폴더 (기본: output/)");
    println!("      -p, --page <번호>       특정 페이지만 내보내기 (0부터 시작)");
    println!("      --show-para-marks       문단부호(↵/↓) 표시 상태의 트리 생성");
    println!("      --show-control-codes    조판부호 보이기 상태의 트리 생성");
    println!("      --respect-vpos-reset    LINE_SEG vpos=0 리셋을 단/페이지 강제 경계로 처리");
    println!();
    println!("  export-structure <파일> [--mode auto|outline|clause] [-o out.json] [--json]");
    println!("      문서 개요/조문(편·장·절·관·조·항·호·목) 계층을 중첩 JSON 트리로 추출");
    println!();
    println!("      --mode <방식>           분류 방식 auto|outline|clause (기본: auto)");
    println!("      -o, --out <파일>        출력 JSON 파일 경로 (생략 시 stdout)");
    println!();
    println!("  export-png <파일.hwp> [옵션]   (native-skia feature 필요)");
    println!("      HWP 파일을 PNG로 내보내기 (Skia raster backend, AI 파이프라인 + VLM 연동)");
    println!();
    println!("      -o, --output <폴더>     출력 폴더 (기본: output/)");
    println!("      -p, --page <번호>       특정 페이지만 내보내기 (0부터 시작)");
    println!(
        "      --profile <프로필>      출력 프로필: screen|print|high-quality|fast-preview (기본: high-quality)"
    );
    println!("      --font-path <경로>      폰트 파일 탐색 경로 (여러 번 지정 가능)");
    println!("                              한컴 전용 폰트 (HY견명조 등) 가 시스템에 없을 때 ttfs 디렉토리 지정");
    println!("      --scale <배율>          렌더링 배율 (기본: 1.0)");
    println!("      --max-dimension <픽셀>  한 변 최대 픽셀 (longest edge). VLM 입력 한도용.");
    println!(
        "                              명시 --scale 이 없으면 자동 scale 계산 (페이지 → 한도 안)"
    );
    println!("      --dpi <값>              DPI 메타데이터 (PNG pHYs chunk). 실제 픽셀 수 무관.");
    println!("                              --scale 미지정 시 scale = dpi/96 자동 계산");
    println!("      --vlm-target <프리셋>   VLM 입력 프리셋 (하이픈/밑줄 모두 허용):");
    println!("                              claude:     1568 px / 1.15 MP (Claude Vision)");
    println!("                              gpt4v-low:  512 px (GPT-4V low detail)");
    println!(
        "                              gpt4v-high: 2000 px / 1.54 MP (GPT-4V high, 별칭: gpt4v)"
    );
    println!("                              gemini:     3072 px (Google Gemini)");
    println!("                              qwen-vl:    2240 px (Qwen-VL, 별칭: qwen)");
    println!("                              llava:      672 px (LLaVA / OSS CLIP)");
    println!();
    println!("  export-text <파일.hwp> [옵션]");
    println!("      페이지별 텍스트를 TXT로 내보내기");
    println!();
    println!("      -o, --output <폴더>     출력 폴더 (기본: output/)");
    println!("      -p, --page <번호>       특정 페이지만 내보내기 (0부터 시작)");
    println!("      --json                  결과를 JSON으로 stdout에 출력 (파일 저장 안 함)");
    println!("      --max-chars <N>         본문 문자 상한 (--json 전용, 기본: 무제한). 넘으면");
    println!("                              봉투에 truncated:true·omittedCount 를 남긴다");
    println!();
    println!("  scan <경로...> [--probe] [--max-depth <N>] [--limit <N>] [--json]");
    println!("      디렉터리를 재귀로 걸어 HWP 계열 파일을 발견·분류 (batch 목록의 원천)");
    println!("      확장자 주장과 매직 감지가 어긋나면 extMismatch 로 알린다");
    println!("      --probe                 파일을 실제로 열어 파싱 가능·암호 필요·쪽수 기록");
    println!("      --max-depth <N>         재귀 최대 깊이 (1 = 지정 폴더만)");
    println!("      --limit <N>             최대 파일 수 — 넘으면 봉투에 truncated:true");
    println!("      --json                  발견 목록·요약 봉투를 stdout 으로 출력");
    println!();
    println!("  batch <export-text|info|export-structure|export-tables|fields|search|extract-data|convert> --json [--threads <N>]");
    println!(
        "      stdin의 파일 목록(한 줄당 하나)을 한 프로세스로 전건 처리해 NDJSON 스트림 출력"
    );
    println!("      --threads <N>           파일 간 병렬 스레드 수 (기본: CPU 코어 수)");
    println!("      --mode <m>              export-structure 전용: auto|outline|clause");
    println!("      --query <검색어>        search 전용: 찾을 문자열");
    println!("      --kind <종류>           extract-data 전용: date|amount|number|all (기본 all)");
    println!(
        "      --limit <N>             extract-data 전용: 문서당 최대 반환 건수 (배치 전체가 아님)"
    );
    println!("      --out-dir <폴더>        convert 전용(필수): 산출물을 모을 폴더");
    println!("                              산출 이름은 <입력이름>.hwp — 이름이 겹치면");
    println!("                              한 건도 쓰지 않고 사용법 오류(2)로 끝낸다");
    println!("      --verify                convert·fill 전용: 재파싱 IR 비교 (차이 → 3)");
    println!("      --verify-pages          convert 전용: 재파싱 쪽수 비교 (전건 불일치 → 4)");
    println!();
    println!("  batch fill --form <서식> --data <행.jsonl|행.csv> --out-dir <폴더> --json [옵션]");
    println!("      서식 1개 + 데이터 N행 → 산출 N개 (메일머지). 행마다 NDJSON 레코드 하나");
    println!("      이 축만 stdin 을 읽지 않는다 — 다른 batch 축은 stdin 으로 파일 경로");
    println!("      목록을 받지만, fill 의 입력은 경로가 아니라 --data 파일의 '행'이다");
    println!();
    println!("      --form <서식>           누름틀이 있는 템플릿 문서 (필수)");
    println!("      --data <행 파일>        .jsonl: 한 줄에 {{\"필드이름\":\"값\"}} 객체 하나");
    println!("                              .csv:   첫 줄 헤더 = 누름틀 이름 (BOM·따옴표 허용)");
    println!("      --out-dir <폴더>        산출물을 모을 폴더 (필수)");
    println!("      --name-field <필드>     산출 파일 이름으로 쓸 데이터 필드");
    println!("                              생략 시 0001.hwp 순번. 파일명 금지 문자는 _ 로");
    println!("                              치환하고, 이름이 겹치면 뒤에 _2 를 붙인다");
    println!("      --verify                행마다 저장 직후 자기검증 (차이 → 3)");
    println!("      --dry-run               파일을 만들지 않고 각 행의 채움 가능 여부만 판정");
    println!();
    println!("  export-markdown <파일.hwp> [옵션]");
    println!("      페이지별 텍스트를 Markdown(.md)으로 내보내기");
    println!();
    println!("      -o, --output <폴더>     출력 폴더 (기본: output/)");
    println!("      -p, --page <번호>       특정 페이지만 내보내기 (0부터 시작)");
    println!();
    println!("  export-tables <파일.hwp|파일.hwpx> [--json] [-o <출력.json>]");
    println!("      표를 격자 JSON으로 추출 (병합 rowSpan/colSpan·중첩 표 보존)");
    println!();
    println!("      --json                  계약 봉투 JSON을 stdout에 출력");
    println!("      -o, --output <파일>     JSON을 파일로 저장");
    println!();
    println!("  table-to-csv <파일.hwp|파일.hwpx> [--table <번호>] [-o <경로>] [--bom] [--json]");
    println!("      본문 최상위 표를 RFC 4180 CSV로 내보내기 (병합 격자를 채워 열이 밀리지 않음)");
    println!();
    println!("      --table <번호>          한 표만 (export-tables 의 index — 0부터 시작하지");
    println!("                              않을 수 있음). 생략하면 최상위 표 전부");
    println!("      -o, --output <경로>     --table 지정 시 CSV 파일, 생략 시 표별 파일");
    println!("                              (table<N>.csv)을 담을 폴더");
    println!("      --bom                   파일 출력에 UTF-8 BOM 추가 (엑셀 한글 깨짐 방지)");
    println!("      --json                  계약 봉투 JSON을 stdout에 출력");
    println!("      -o 도 --json 도 없으면 CSV 본문을 stdout으로 그대로 흘린다 (파이프용)");
    println!();
    println!("  csv-to-table <파일.hwp|파일.hwpx> --csv <경로.csv> --table <번호> [옵션]");
    println!("      CSV 내용으로 기존 표 N의 셀을 덮어쓰기 (표 크기는 바꾸지 않음)");
    println!();
    println!("      --csv <경로>            읽을 CSV 파일 (UTF-8, 선두 BOM 허용)");
    println!("      --table <번호>          덮어쓸 표 (export-tables 의 index)");
    println!("      -o, --output <파일>     출력 경로 (기본: <입력 stem>_csv.hwp/.hwpx)");
    println!("      --dry-run               파일을 쓰지 않고 바뀔 칸만 보고");
    println!("      --verify                저장 직후 재파싱 IR 자기검증 (차이 시 exit 3)");
    println!("      --json                  계약 봉투 JSON을 stdout에 출력");
    println!("      행·열 수가 표와 다르거나 병합으로 덮인 칸에 값이 있으면 한 칸도 쓰지 않고");
    println!("      invalid[] 로 보고하며 사용법 오류(2)로 끝낸다 — 조용히 잘라내지 않는다");
    println!();
    println!("  chart-to-csv <파일.hwp|파일.hwpx> [--chart <번호>] [-o <경로>] [--bom] [--json]");
    println!("      차트 숫자 데이터를 RFC 4180 CSV로 내보내기 (행=카테고리, 열=계열)");
    println!();
    println!("      --chart <번호>          한 차트만 (문서 순서, 1부터). 생략하면 전부");
    println!("      -o, --output <경로>     --chart 지정 시 CSV 파일, 생략 시 차트별 파일");
    println!("                              (chart<N>.csv)을 담을 폴더");
    println!("      --bom                   파일 출력에 UTF-8 BOM 추가 (엑셀 한글 깨짐 방지)");
    println!("      --json                  계약 봉투 JSON을 stdout에 출력");
    println!("      분산형은 첫 열이 X 값이고 머리 행 첫 칸이 X 로 표시된다");
    println!();
    println!("  csv-to-chart <파일.hwp|파일.hwpx> --csv <경로.csv> --chart <번호> [옵션]");
    println!("      CSV 내용으로 기존 차트 N의 값을 덮어쓰기 (계열·값 개수는 바꾸지 않음)");
    println!();
    println!("      --csv <경로>            읽을 CSV 파일 (UTF-8, 선두 BOM 허용)");
    println!("      --chart <번호>          덮어쓸 차트 (문서 순서, 1부터)");
    println!("      -o, --output <파일>     출력 경로 (기본: <입력 stem>_chart.hwp/.hwpx)");
    println!("      --dry-run               파일을 쓰지 않고 바뀔 칸만 보고");
    println!("      --verify                저장 직후 재파싱 IR 자기검증 (차이 시 exit 3)");
    println!("      --json                  계약 봉투 JSON을 stdout에 출력");
    println!("      값은 OOXML 두 표현(zip 파트·중첩 CFB)에 함께 쓴다 — 한쪽만 쓰면 HWP");
    println!("      변환에서 편집이 사라진다. 어디에 썼는지는 봉투의 wrote[] 로 드러난다");
    println!("      계열·값 개수나 계열명·라벨이 다르면 한 칸도 쓰지 않고 invalid[] + exit 2");
    println!();
    println!("  export-pdf <파일.hwp|파일.hwpx|파일.hml> [옵션]");
    println!("      HWP/HWPX/HML 문서를 PDF로 내보내기 (기본: SVG 호환 backend)");
    println!();
    println!("      -o, --output <파일>      출력 PDF 파일 (기본: output/<입력명>.pdf)");
    println!("      -p, --page <번호>       특정 페이지만 내보내기 (0부터 시작)");
    println!("      --backend <svg|direct>  PDF backend (기본값: svg)");
    println!(
        "      --profile <프로필>      layer 출력 프로필: screen|print|high-quality|fast-preview"
    );
    println!("      --raster-dpi <DPI>      direct backend fallback raster DPI (기본값: 144)");
    println!("      --font-path <경로>      폰트 파일 탐색 경로 (여러 번 지정 가능)");
    println!("      --fallback-serif <명>   PDF serif generic fallback family");
    println!("      --fallback-sans <명>    PDF sans-serif generic fallback family");
    println!("      --fallback-mono <명>    PDF monospace generic fallback family");
    println!("      --equation-font <명>    PDF 수식 SVG 우선 font-family");
    println!("      --text-as-paths         텍스트를 폰트 임베드 대신 path로 변환");
    println!("                              (메모리 대폭 절감, 텍스트 선택·검색 불가)");
    println!(
        "                              <...>는 자리표시자이며, 실제 입력에는 꺾쇠괄호를 쓰지 않음"
    );
    println!(
        "                              경로/폰트명에 공백이 있으면 큰따옴표 권장: --font-path \"./My Fonts\""
    );
    println!("                              예: --fallback-sans \"Apple SD Gothic Neo\"");
    println!();
    println!("  extract-pages <입력> <출력.hwp> --from N --to M [--json]");
    println!("      쪽 범위만 남겨 저장 (대형 문서 결함 이분법·부분 발췌)");
    println!();
    println!("      --from <N>              시작 쪽 (1부터, 기본: 1)");
    println!("      --to <M>                끝 쪽 (필수)");
    println!("      -o, --output <파일>     출력 경로 (위치 인자 대신 지정 가능)");
    println!("      --json                  전후 쪽수·문단 수 요약을 JSON으로 출력");
    println!("      쪽 단위로 자르되 문단 단위로 지운다 — 결과 쪽수가 범위와 다를 수 있음");
    println!();
    println!("  export-hwpx <입력.hwp|입력.hwpx> [출력.hwpx] [--verify] [--verify-pages]");
    println!("      HWP 문서를 HWPX(ZIP+XML)로 변환 저장. 출력 생략 시 <입력 stem>.hwpx");
    println!(
        "      --verify              변환 후 산출물을 재파싱해 IR 차이를 검출 (차이 시 exit 3)"
    );
    println!("      --verify-pages        변환 전/후 렌더 페이지 수를 비교 (불일치 시 exit 4)");
    println!();
    println!("  export-hml <입력.hml> -o <출력.hml>");
    println!("      HML 원본 문서를 의미 보존 HWPML 2.91 XML로 저장");
    println!("      -o, --output <파일>    출력 HML 파일 (필수, 원본 덮어쓰기 금지)");
    println!();
    println!(
        "  export-doclang <파일.hwp|파일.hwpx> [-o <출력.xml>] [--assets-dir <디렉터리>] [--json]"
    );
    println!("      HWP/HWPX 문서를 DocLang v0.6 XML로 내보내기");
    println!();
    println!("      -o, --output <파일>     출력 XML 파일 (기본: <입력 stem>.dclg.xml)");
    println!("      --assets-dir <디렉터리> 그림 등 이진 자원을 이 디렉터리에 파일로 기록");
    println!("                              (생략 시 base64 data URI로 XML에 인라인)");
    println!("      --json                  산출 봉투를 stdout 에 JSON 으로 출력");
    println!();
    println!("  info <파일.hwp|파일.hwpx|파일.hml> [--json]");
    println!("      HWP/HWPX/HML 문서 정보 표시");
    println!();
    println!("      --json                  문서 정보를 JSON으로 stdout에 출력");
    println!();
    println!("  digest <파일> [--sections | --pages a..b] [--max-chars N] [--json]");
    println!("      문서 요약 봉투 한 줄 출력 — 메타(info)·개요 상위 노드·첫 페이지 발췌·");
    println!("      nextStep 유도문을 한 번 호출로 묶은 매크로 (초소형 모델용, #3633)");
    println!();
    println!("      --sections              페이지 발췌 대신 절 단위 청크 sections:[{{title,");
    println!("                              page,charCount,excerpt}}] 출력 — 쪽 주소 보존,");
    println!("                              구조 없는 문서는 쪽 단위 폴백(sectionsMode:page)");
    println!("      --pages <a..b>          해당 쪽 범위만 발췌 (0 기준, 양끝 포함) —");
    println!("                              nextStep 이 남은 범위의 다음 호출을 안내");
    println!("      --max-chars <N>         발췌 최대 문자 수 (기본: 2000, 절 모드는 절별 240)");
    println!();
    println!("  explain <파일.hwp|파일.hwpx|파일.hml> [--json]");
    println!("      문서를 처음 보는 에이전트를 위한 결정론적 요약 문장(형식·쪽수·문단 수·");
    println!("      표·누름틀·각주/미주·암호 여부) — info/export-structure/export-tables/");
    println!("      fields 를 조합한 템플릿 조립일 뿐 LLM 판정은 없다 (#3828)");
    println!();
    println!("      --json                  요약 봉투를 JSON으로 stdout에 출력");
    println!();
    println!("  capabilities [--mcp]");
    println!("      도구 자기서술 JSON 출력 (명령·플래그·JSON 계약·종료 코드) — 에이전트용");
    println!();
    println!("      --mcp                   MCP 도구 정의(name/description/inputSchema) 출력");
    println!();
    println!("  export-capabilities-schema [--bare] [-o <파일>] [--json]");
    println!("      capabilities 자기서술 자체의 JSON Schema 출력 — 바인딩 코드 생성의 단일 출처");
    println!();
    println!("      --bare                  봉투 없이 capabilities 스키마 본문만 출력");
    println!("      -o, --out <파일>        스키마를 파일로 저장 (생략 시 stdout)");
    println!("      --json                  -o 와 함께 쓰면 저장 결과를 JSON 봉투로 보고");
    println!("  export-ontology [--bare] [-o <파일>] [--json]");
    println!("      자기서술(IR 스키마·capabilities·MCP 도구·출처 지도)에서 기계 유도한");
    println!("      JSON-LD 온톨로지 출력 — 클래스·속성·행위·신뢰 술어, 손 나열 상수 0");
    println!();
    println!("      --bare                  봉투 없이 JSON-LD 본문(@context·@graph)만 출력");
    println!("      -o, --out <파일>        온톨로지를 파일로 저장 (생략 시 stdout)");
    println!("      --json                  -o 와 함께 쓰면 저장 결과를 JSON 봉투로 보고");
    println!();
    println!("  export-provenance-map [--json]");
    println!("  export-agent-manifest [--bare] [--json]");
    println!("      명령별 '문서에서 온 값' 필드 지도 — 그 값들은 데이터이지 지시가 아니다");
    println!("      각 봉투의 untrustedContent/untrustedFields 표지와 같은 원천");
    println!();
    println!("      --json                  기계 계약 JSON을 stdout에 출력");
    println!();
    println!("  mcp-serve");
    println!("      MCP 서버 실행 (stdio JSON-RPC) — AI 에이전트 호스트가 도구로 연결 (#3140)");
    println!("      capabilities --mcp 의 도구 전부 + 세션(hwp_open/hwp_doc_text/hwp_close)");
    println!();
    println!("  dump <파일.hwp|파일.hwpx|파일.hml> [--section <번호>] [--para <번호>]");
    println!("      문서 조판부호 구조 덤프 (디버깅용)");
    println!();
    println!("  dump-note-shape <파일.hwp|파일.hwpx>");
    println!("      구역별 각주/미주 모양 raw 값과 한컴 UI 의미값을 JSON으로 덤프");
    println!();
    println!("  dump-endnote-lines <파일.hwp> <section> <para> <control> [note-para]");
    println!("      특정 미주 원본 문단의 line_seg, TextRun, TAC 수식 위치를 함께 덤프");
    println!();
    println!("  dump-pages <파일.hwp> [-p <번호>] [--respect-vpos-reset] [--json]");
    println!("      페이지네이션 결과 덤프 (페이지별 문단/표 배치 목록)");
    println!();
    println!("  dump-records <파일.hwp>");
    println!("      HWP5 raw record 덤프 (DocInfo/BodyText 레코드 트리)");
    println!();
    println!("  diag <파일.hwp>");
    println!("      문서 구조 진단 (번호/글머리표/개요 분석)");
    println!();
    println!("  search <파일.hwp|파일.hwpx> <검색어> [옵션]");
    println!("      문서 검색 — 매치마다 구역·문단·페이지·문자 오프셋을 함께 반환");
    println!();
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      --ignore-case             대소문자 무시");
    println!("      --max-matches <N>         최대 매치 수 (기본: 무제한). 절단되면 봉투에");
    println!("                                truncated:true·omittedCount 가 남는다");
    println!("      --limit <N>               --max-matches 의 기존 이름 (#3353, 동의어)");
    println!();
    println!("  extract-data <파일.hwp|파일.hwpx> [옵션]");
    println!("      날짜·금액·수량 추출 — 값마다 구역·문단·페이지·문자 오프셋을 함께 반환");
    println!();
    println!("      --kind <종류>             date|amount|number|all (기본: all)");
    println!("      --limit <N>               최대 항목 수 (총량은 totalItemCount)");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      정규화할 수 없으면 normalized 는 null 이고 raw 만 남는다");
    println!("      (두 자리 연도 '26.8.2·한글 수사 금액은 세기·값을 추정하지 않음)");
    println!();
    println!("  hwp5-inventory <파일.hwp> [--format jsonl|md] [--section N] [--out <path>]");
    println!("      HWP5 DocInfo/BodyText record inventory 생성 (HWPX→HWP contract 분석용)");
    println!();
    println!("  hwp5-inventory-diff <oracle.hwp> <generated.hwp> [--align index|lcs] [--report diff|hints|bundles|table-fields|table-probe-plan] [--focus all|table|shape|ctrl|missing|docinfo] [--window N] [--format jsonl|md] [--section N] [--out <path>]");
    println!("      HWP5 inventory 비교 결과, contract 후보 힌트, 후보 주변 bundle 생성");
    println!();
    println!("  hwp5-contract-analyze <source.hwpx> <oracle.hwp> <generated.hwp> --out-dir <폴더>");
    println!("      HWPX/HWP oracle/generated record-control contract graph 분석 보고서 생성");
    println!();
    println!("  hwp5-ctrl-data-trace <oracle.hwp> <generated.hwp> --out <path> [--section N] [--record-index N]");
    println!("      oracle/generated CTRL_DATA ParameterSet 구조 추적 보고서 생성");
    println!();
    println!("  hwp5-contract-probe <oracle.hwp> <generated.hwp> --out-dir <폴더>");
    println!("      DocInfo MEMO_SHAPE/ID_MAPPINGS와 누락 CTRL_DATA 축별 판정용 HWP probe 생성");
    println!();
    println!("  hwp5-table-probe <oracle.hwp> <generated.hwp> --out-dir <폴더>");
    println!("      TABLE/CTRL_HEADER(Table) field 축별 판정용 HWP probe 생성");
    println!();
    println!("  hwp5-mel-personnel-probe <oracle.hwp> <generated.hwp> --out-dir <폴더>");
    println!("      mel-001 인원현황 표 TABLE/LIST_HEADER/PARA_HEADER 축별 판정용 HWP probe 생성");
    println!();
    println!("  hwp5-borderfill-diagonal-probe <oracle.hwp> <generated.hwp> --out-dir <폴더>");
    println!("      DocInfo BORDER_FILL 대각선 attr/payload 축별 판정용 HWP probe 생성");
    println!();
    println!("  hwp5-first-para-control-probe <oracle.hwp> <generated.hwp> --out-dir <폴더>");
    println!("      첫 문단 control/PARA_TEXT/PARA_CHAR_SHAPE 계약 축별 판정용 HWP probe 생성");
    println!();
    println!("  hwp5-anchor-trace <파일.hwp> --needle <텍스트> [--section N] [--window N] [--out <path>]");
    println!("      특정 텍스트를 포함한 PARA_TEXT 주변의 raw HWP5 record를 추적");
    println!();
    println!("  hwp5-char-shape-audit <hancom-oracle.hwp> <generated.hwp> --out <보고서.md> [--source-hwpx <원본.hwpx>]");
    println!("      CHAR_SHAPE sentinel 차이와 PARA_CHAR_SHAPE 사용 위치를 분석");
    println!();
    println!("  hwp5-cell-header-probe <oracle.hwp> <generated.hwp> --out-dir <폴더>");
    println!("      표 셀 LIST_HEADER/PARA_HEADER 계약 축별 판정용 HWP probe 생성");
    println!();
    println!("  convert <입력.hwp|입력.hwpx> <출력.hwp> [--verify] [--verify-pages]");
    println!("      배포용(읽기전용) HWP를 편집 가능한 HWP로 변환");
    println!("      --verify              저장 후 재파싱 IR 차이를 검출 (차이 시 exit 3)");
    println!("      --verify-pages        저장 전/후 렌더 페이지 수를 비교 (불일치 시 exit 4)");
    println!();
    println!("  build-from-ingest <ingest.json> [--media-dir <dir>] -o <out.hwpx>");
    println!("      ingest JSON(시험문제 등)을 HWPX로 생성 (rhwp-exam-ingest 파이프라인)");
    println!();
    println!("  ir-diff <파일A.hwpx> <파일B.hwp> [-s <구역>] [-p <문단>] [--json]");
    println!("  verify <파일> --expect-pages <N> | --expect-min-pages <N> | --expect-max-pages <N> | --expect-min-chars <N> | --expect-min-tables <N> | --expect-table-count <N> | --expect-contains <문자열> | --expect-not-contains <문자열> | --expect-field <이름=값> | --expect-format <형식> [--json]");
    println!("      두 파일의 IR(중간표현) 비교 (HWPX↔HWP 불일치 검출)");
    println!("      --json                  판정 봉투 JSON 한 줄 출력, 차이 발견 시 exit 3");
    println!("      비교 항목: text, char_count, char_offsets, char_shapes, line_segs,");
    println!("                 controls(타입+속성), tab_extended, ParaShape, TabDef");
    println!("      표: page_break, outer_margin, treat_as_char, wrap, size, v_offset/h_offset");
    println!("      그림/도형: treat_as_char, wrap, size, v_offset/h_offset, vert_rel/horz_rel");
    println!();
    println!("  hwpx-roundtrip <파일.hwpx | --batch 폴더> [-o <출력폴더>] [--lineseg-report]");
    println!("      HWPX → IR → HWPX roundtrip 검증 (Task #1315 baseline)");
    println!("      재조립 .hwpx와 inventory.tsv를 출력 폴더(기본 output/poc/task1315)에 생성");
    println!("      --lineseg-report: 문단별 lineseg diff를 lineseg_diff.tsv로 산출 (#1380 측정)");
    println!("  hwp5-roundtrip <파일.hwp | --batch 폴더> [-o <출력폴더>]");
    println!("      HWP5 → IR → HWP5 roundtrip 무손실 검증 (Task #1552)");
    println!("      재조립 .rt.hwp와 inventory.tsv를 출력 폴더(기본 output/poc/task1552)에 생성");
    println!("  render-diff <파일> [--via hwpx|hwp] [-p <페이지>] [--max-disp <px>] [--json]");
    println!("  render-diff <파일A> <파일B> [-p <페이지>] [--max-disp <px>] [--json]");
    println!(
        "  render-diff --batch <폴더> [--via hwpx] [-o <출력폴더>] [--max-disp <px>] [--json]"
    );
    println!("      라운드트립 시각 정합성 게이트 — 페이지별 RenderNode bbox 변위(px) 정량화");
    println!("      자기 라운드트립(원본 IR vs 직렬화→재로드 IR) 또는 두 파일 직접 비교");
    println!("      배치: geom_inventory.tsv 산출(기본 output/poc/render_diff)");
    println!("      --json: 단건은 한 줄 봉투, --batch 는 NDJSON(로드 실패도 error 레코드로 남김)");
    println!("      --json 회귀 검출은 종료 코드 3(검증 단언 실패) — 사람 모드는 종전대로 1");
    println!("  bench <파일...> | --batch <폴더> [-n <반복수>] [--tsv <출력.tsv>]");
    println!("      단계별 처리 성능 계측 — parse/layout/render/serialize median(ms)");
    println!("      워밍업 1회 후 N회(기본 3) 반복. 파일별 크기/쪽수 + total 표 + TSV");
    println!("      주의: 절대 수치는 머신·빌드 의존, 동일 환경 상대·재현 지표로 해석");
    println!();
    println!("  thumbnail <파일.hwp> [옵션]");
    println!("      HWP 파일에서 썸네일(PrvImage) 추출");
    println!();
    println!("      -o, --output <파일>       출력 파일 경로 (기본: 입력명_thumb.png)");
    println!("      --base64                  base64 문자열을 stdout에 출력");
    println!("      --data-uri                data:image/... URI 형식으로 stdout에 출력");
    println!();
    println!("  fields <파일.hwp|파일.hwpx> [--json]");
    println!("      누름틀/필드 조사 (읽기 전용) — 이름·안내문·지시문·현재값·위치");
    println!();
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!();
    println!("  template-entity <파일.hwpx> --code <코드> [옵션]");
    println!("      hwpx 표 역할 마커(#REPEAT-*, #PAGENO)·누름틀에서 서버 없이 Java record");
    println!("      데이터/모듈 클래스 초안을 생성한다. 마커 검증 실패는 크래시가 아니라");
    println!("      errors 데이터(--json에서는 빈 소스 + errors[]).");
    println!();
    println!("      --code <코드>             생성할 템플릿 코드 — 클래스 이름의 근거 (필수)");
    println!("      --package <패키지>       Java 패키지 이름 (기본: com.example.hwpx.templates)");
    println!("      --out-dir <디렉터리>      생성한 .java 2개를 저장 (--json과 함께 쓰면 무시됨)");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!();
    println!("  inspect hidden-text <파일.hwp|파일.hwpx> [--json] [옵션]");
    println!("      은닉 텍스트 조사 (읽기 전용) — 사람 눈에는 안 보이는데 텍스트 추출기가");
    println!("      읽어 LLM 프롬프트로 흘러드는 문자열을 찾는다 (간접 프롬프트 인젝션 대비).");
    println!("      흰 배경에 흰 글씨·0pt 글자처럼 조판 정보가 있어야만 보이는 은닉을 잡는다.");
    println!();
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      --threshold-pt <N>        near_invisible 임계 pt (기본: 1.0)");
    println!("      --include-offpage         쪽 경계 완전히 밖에 놓인 문단도 보고 (기본: 끔)");
    println!("  inspect injection <파일.hwp|파일.hwpx> [--json] [옵션]");
    println!("      프롬프트 주입 신호 탐지 (읽기 전용, 문서를 고치지 않는다) — 문서 텍스트가");
    println!("      LLM 에이전트에게 지시를 내리는 형태인지 판정해 신뢰도·근거와 함께 신고한다");
    println!("      기본 검사 범위: 본문·표 셀·글상자·수식·각주·미주·머리말·꼬리말");
    println!("      검사하지 않는 범위: 요약정보(제목·작성자)·바탕쪽·OLE 내부·이미지 속 글자");
    println!();
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      --min-confidence <등급>   low|medium|high 미만 신호 제외 (기본: low = 전부)");
    println!("      --include-fields          누름틀 이름·안내문·command 와 숨은 설명(메모)까지");
    println!("                                확장 검사 (기본: 끔 — 본문 축만 훑는다)");
    println!();
    println!("  inspect unicode <파일.hwp|파일.hwpx> [--json] [--kind <축>]");
    println!("      유니코드 기만 탐지 — 제로폭 문자·표시순서 역전·태그 문자·동형자를 검사하고");
    println!("      탐지마다 화면 표시(rendered)와 실제 순서(raw)를 나란히 출력한다.");
    println!();
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      --kind <축>               zero-width|bidi|tag|confusable|all (기본: all)");
    println!();
    println!("  edit fill-fields <파일.hwp|파일.hwpx> --data <JSON|@파일> [-o <출력>] [옵션]");
    println!("      누름틀에 값을 채운다 (서식 자동 작성/메일머지)");
    println!();
    println!("      --data <JSON|@파일>       {{\"필드이름\":\"값\"}} 형식. @경로면 파일에서 읽음");
    println!(
        "      -o, --output <파일>       출력 파일 (기본: 입력명_filled.<입력과 같은 확장자>)"
    );
    println!("      --dry-run                 파일을 쓰지 않고 변경 예정 내역만 보고");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!();
    println!("  edit replace-text <파일.hwp|파일.hwpx> --find <문자열> --replace <문자열> [옵션]");
    println!("      문서 전체 일괄 치환 (기관명 변경·연도 갱신·용어 정비). 본문+표 셀");
    println!();
    println!("      --find <문자열>           찾을 문자열 (빈 문자열 불가)");
    println!("      --replace <문자열>        바꿀 문자열 (\"\" 이면 삭제)");
    println!("      --ignore-case             대소문자 무시");
    println!(
        "      -o, --output <파일>       출력 파일 (기본: 입력명_replaced.<입력과 같은 확장자>)"
    );
    println!("      --dry-run                 파일을 쓰지 않고 치환 예정 건수만 보고");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      치환 0건이면 출력 파일을 만들지 않음");
    println!();
    println!("  edit set-cell <파일> --table <번호> --row <행> --col <열> --text <문자열> [옵션]");
    println!("      표 격자 좌표로 셀 값을 바꾼다 (실물 표 양식 채우기)");
    println!();
    println!("      --table/--row/--col       export-tables 격자와 같은 좌표 (0부터)");
    println!("      --text <문자열>           셀에 넣을 값 (비우기는 \"\", 줄바꿈·탭 불가)");
    println!("      --keep-style              셀 안내문 스타일 상속(기본: 검정 글씨로 기록)");
    println!("      -o, --output <파일>       출력 파일 (기본: 입력명_cell.<입력과 같은 확장자>)");
    println!("      --dry-run                 파일을 쓰지 않고 old→new 만 보고");
    println!("      (값이 칸 폭을 넘치면 --json 응답의 overflow 로 알린다 — 채우기는 막지 않음)");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      병합으로 덮인 칸은 앵커 좌표 안내와 함께 오류 종료");
    println!();
    println!("  edit insert-image <파일> --image <그림> [옵션]");
    println!("      도장·서명 그림을 쪽 좌표에 붙인다 (용지 기준 떠 있는 그림)");
    println!();
    println!("      --image <경로>            png·jpg·jpeg·bmp·tif·tiff (그 밖은 인자 오류)");
    println!("      --page <번호>             붙일 쪽 (0부터, 기본 0)");
    println!("      --x/--y <값>              용지 왼쪽 위 모서리 기준 위치 (기본 0)");
    println!("      --width/--height <값>     그림 크기 (생략: 원본 픽셀 ×75, 한쪽만: 비율 유지)");
    println!(
        "      길이 단위는 모두 **HWPUNIT(1/7200 inch)** — 픽셀이 아니다 (A4 세로 59528×84188)"
    );
    println!("      -o, --output <파일>       출력 파일 (기본: 입력명_image.<입력과 같은 확장자>)");
    println!("      --dry-run                 파일을 쓰지 않고 배치 예정만 보고");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      (쪽 밖으로 나가면 자르지 않고 --json 응답의 overflow 로 알린다)");
    println!();
    println!("      edit 명령 공통: 산출물은 **입력 형식을 보존**한다 (HWPX 입력 → HWPX 산출).");
    println!(
        "  edit redact <파일.hwp|파일.hwpx> [--kind …] [--dry-run] [--no-raw] [-o <출력>|--in-place]"
    );
    println!("      공개 전 개인정보 마스킹 — 주민등록번호·전화·이메일·카드번호");
    println!();
    println!("      --kind <목록>             ssn|phone|email|card|all (쉼표 구분, 기본 all)");
    println!("      --mask <문자>             마스킹 문자 한 글자 (기본 *, 자릿수 보존)");
    println!("      --dry-run                 **권장 첫 단계** — 무엇이 지워질지만 보고");
    println!("      --no-raw                  findings[].raw(원문 개인정보)를 봉투에서 뺀다");
    println!("      -o, --output <파일>       출력 파일");
    println!("      --in-place                원본을 덮어쓴다 (되돌릴 수 없음)");
    println!("      --verify                  저장 직후 IR 자기검증 (차이 시 exit 3)");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      되돌릴 수 없는 작업이다 — 먼저 원본을 복사해 두고, --dry-run 으로 확인하라.");
    println!("      -o 도 --in-place 도 없으면 원본 보호를 위해 실행을 거부한다 (exit 2).");
    println!("      탐지는 보수적이다: 주민등록번호는 검증 숫자, 카드는 Luhn 을 통과해야 하고");
    println!(
        "      전화는 하이픈이 있는 이동전화·서울(02) 번호만 본다 (오탐이 본문을 훼손하므로)."
    );
    println!("      --dry-run 출력에는 원문 개인정보가 그대로 들어간다 — 로그에 남기지 말 것.");
    println!("      로그·이슈에 봉투를 그대로 붙여야 한다면 --no-raw 를 함께 써서 raw 를 빼라.");
    println!();
    println!("  edit sanitize <파일.hwp|파일.hwpx> [--keep-preview] [-o <출력>] [--json]");
    println!("      문서 메타데이터 제거 — 작성자·제목·최종수정자·작성일·미리보기");
    println!();
    println!("      --keep-preview            미리보기 이미지를 남긴다 (기본: 제거)");
    println!("      -o, --output <파일>       출력 파일 (기본: 입력명_sanitized.<입력 확장자>)");
    println!("      --json                    계약 봉투 JSON을 stdout에 출력");
    println!("      본문 내용은 건드리지 않는다. 지운 항목은 removed[] 로 보고한다.");
    println!();
    println!("      edit 명령 공통: 산출물은 **입력 형식을 보존**한다 (HWPX 입력 → HWPX 산출).");
    println!("      HWPX 입력에 -o ….hwp 를 지정하면 그 경로를 존중해 HWP5 로 저장하되");
    println!("      형식 변경(이미지·차트 유실 가능)을 stderr 로 경고한다.");
    println!();
    println!("  export-ir-schema [--bare] [-o <파일>] [--json]");
    println!("      공개 IR 의 JSON Schema — 외부 바인딩 코드 생성의 단일 출처");
    println!("      --bare 는 봉투 없이 스키마 본문만 (JSON Schema 도구 입력용)");
    println!("  run <계획.json> [--json]              선언적 편집 계획 실행 (#3703)");
    println!("  replay <계획.json> [--expect-output-sha256 <hex>] [--sign-key <키.json>] [--json]  작업 영수증 발급·재현 검증 (#4391)");
    println!("  audit <캡슐 폴더> [--json]            작업 캡슐 전수 재검증 — 재현율 회계 (#4393)");
    println!("  lineage <캡슐.json> [--deep] [--keyring <키링.json>] [--anchor-log <로그>] [--json]  작업 계보(해시 체인) 연대기 검증 (#4401)");
    println!("  keygen --key-id <id> --out <키.json>   Ed25519 서명키 발급 (#4509)");
    println!("  verify-signature <캡슐> --keyring <키링.json> [--sig <서명.json>] [--json]  캡슐 서명 검증 (#4509)");
    println!("  harness init <폴더> [--key-id <id>]     검증 작업장 생성 (#4537)");
    println!("  harness wrap --plan <JSON|@파일> --dir <작업장> [--sign-key <키>]  실행+영수증+캡슐+체인+서명 한 방 (#4537)");
    println!("  harness-status <작업장> [--keyring <키링>] [--deep] [--json]  체인·서명·재현 통합 판정 (읽기 전용) (#4537)");
    println!("  anchor add <캡슐> --log <anchor.ndjson>   투명성 로그 등재 (#4543)");
    println!("  anchor checkpoint --log <로그> [-o <파일>]  머클 체크포인트 산출 (#4543)");
    println!("  anchor verify <캡슐> --log <로그> [--checkpoint <파일>] [--json]  등재·무결·머클 경로 판정 (#4543)");
    println!("  gate <캡슐> --policy <policy.json> [--keyring][--anchor-log][--deep]  반입 정책 기계 판정 (#4545)");
    println!("  bundle export <머리캡슐> -o <x.lineage-bundle> [--anchor-log --checkpoint][--domain]  연합 번들 내보내기 (#4549)");
    println!(
        "  bundle verify <번들> --trust-domain <domain.json> [--json]  5단 오프라인 검증 (#4549)"
    );
    println!(
        "  disclose redact <캡슐> -o <가림> --opening-out <개봉>  salt 커밋 가림 발급 (#4551)"
    );
    println!(
        "  disclose verify <가림> --opening <부분개봉> [--json]   필드 단위 커밋 대조 (#4551)"
    );
    println!("  disclose restore <가림> --opening <전체개봉> -o <복원>  바이트 완전 복원 (#4551)");
    println!("  settle propose --workorder <wo> --capsule <c> --gate-envelope <g> -o <청구>  3해시 고정 청구 발급 (#4553)");
    println!("  settle verify <청구> --workorder <wo> --capsule <c> --gate-envelope <g> [--keyring] [--ledger]  청구 검증 (#4553)");
    println!("  settle record <청구> --ledger <원장>  이중 청구 검사 후 원장 기입 (#4553)");
    println!("  audit-report <캡슐 폴더> -o <보고서> [--deep] [--keyring] [--anchor-log] [--policy] [--sign-key]  감사 보고 표준 (#4558)");
    println!("  recall-scope --contaminated <캡슐|sha256> --among <폴더> [--ledger]  오염 후손 폐쇄집합 (#4558)");
    println!("  conformance <캡슐 폴더> --level <L1..L5> [--deep] [--keyring] [--anchor-log] [--policy] [--ledger]  적합성 자가진단 (#4558)");
    println!("      전 step 을 정적 선검증(불가 시 실행 0·exit 2)하고 인메모리로 원자");
    println!("      실행해 단언(verify) 통과 시에만 단 한 번 저장한다 — 실패 시 디스크 무변경.");
    println!("      steps: fill_fields{{data}} · replace_text{{find,replace[,occurrence]}}");
    println!("             · set_cell{{table,row,col,text}} · set_checkbox{{occurrence}}");
    println!("      --plan-json '<JSON>'      파일 대신 인라인 계획 (MCP hwp_run_plan 경로)");
    println!("      --dry-run                 선검증만 — preview 저널, 디스크 무변경 (계획서 dryRun:true 와 동일)");
    println!("      step 마다 if 조건 가능: {{fieldExists}}·{{fieldEquals:{{name,value}}}}·{{textFound}}");
    println!("      조건이 거짓이면 그 step 만 건너뛰고 저널에 skipped:true·reason 으로 남긴다");
    println!("      (거짓인 step 은 선검증도 면제 — 없는 필드를 채우는 step 도 위반이 아니다)");
    println!("      단언 실패는 exit 3 — 저널(steps[]·verify)로 판정을 데이터로 보고");
    println!();
    println!("  export-plan-schema [--bare] [-o <파일>] [--json]");
    println!("      run 계획서 문법의 JSON Schema 출력 — 계획을 쓰기 전에 읽는 정답지");
    println!();
    println!("      --bare                  봉투 없이 계획 스키마 본문만 출력");
    println!("      -o, --out <파일>        스키마를 파일로 저장 (생략 시 stdout)");
    println!("      --json                  -o 와 함께 쓰면 저장 결과를 JSON 봉투로 보고");
    println!();
    println!("내부 개발·회귀 도구 (일반 사용자 대상 아님):");
    println!("  test-caption <파일.hwp> [-o <폴더>] 캡션 라운드트립 검증");
    println!("  test-field <파일.hwp>               필드 라운드트립 검증");
    println!("  test-shape <입력.hwp> <출력.hwp>    도형 라운드트립 검증");
    println!("  gen-table                           표 테스트 HWP 생성");
    println!("  gen-pua                             PUA 문자 테스트 HWP 생성");
    println!();
    println!("옵션:");
    println!("  -h, --help      도움말 표시");
    println!("  -V, --version   버전 표시");
}


#[derive(Clone, Copy)]
enum GridOriginOption {
    Fixed((f64, f64)),
    AutoPaper,
}


/// [#3238] batch — 파일 목록을 stdin(한 줄당 하나)으로 받아 한 프로세스에서 전건 처리하고
/// NDJSON 스트림을 stdout 으로 낸다. 건별 실패는 `error` 레코드로 스트림을 계속하되,
/// 하나라도 실패하면 [#2707] 계약대로 종료 코드 1 로 끝난다.
/// [#3918 승격 3호] `scan` — 코퍼스 발견·분류. `batch` 의 앞 단계.
///
/// `batch` 는 "경로 목록을 이미 갖고 있다"는 전제에서 시작한다. 이 명령이 그 목록을
/// 만든다: 디렉터리를 재귀로 걸어 HWP 계열 파일을 찾고, 확장자 주장과 매직 감지를
/// 대조하고(`extMismatch`), `--probe` 면 실제로 열어 파싱 가능/암호 필요/쪽수를
/// 기록한다. rhwp-agent 실험 표면의 `scan`(#3922)이 검증해 둔 축의 승격이며, 실측은
/// 전부 기존 코어 재사용이다: `parser::detect_format`·`load_document`·`page_count`.
///
/// 발견은 판정이 아니므로 게이트 종료 코드(3)가 없다 — 파싱 실패·확장자 불일치도
/// exit 0 의 데이터다(판정은 데이터, #2707). 실행 실패는 stdout 을 비우고 exit 1,
/// 조립 오류는 exit 2. 결정성: 파일 순서는 경로 문자열 오름차순으로 고정한다 —
/// 같은 트리는 언제나 같은 순서로 나온다(재현 가능한 코퍼스 목록).
fn cmd_scan(args: &[String]) -> i32 {
    const USAGE: &str =
        "사용법: rhwp scan <경로...> [--probe] [--max-depth <N>] [--limit <N>] [--json]";

    /// 확장자가 주장하는 포맷. `.hwp` 는 HWP5/HWP3 겸용 확장자라 "hwp"(모호)로 둔다.
    fn ext_claim(path: &std::path::Path) -> Option<&'static str> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "hwp" => Some("hwp"),
            "hwpx" => Some("hwpx"),
            "hml" => Some("hml"),
            _ => None,
        }
    }

    /// 확장자 주장과 매직 감지가 어긋나는가. `.hwp` 는 hwp5·hwp3 둘 다 정상이다.
    fn ext_mismatch(claim: &str, magic: &str) -> bool {
        match claim {
            "hwp" => !matches!(magic, "hwp5" | "hwp3"),
            other => other != magic,
        }
    }

    /// `parser::FileFormat` → `info --json` 의 `format` 토큰 (verify 와 같은 지도).
    fn format_token(format: rhwp::parser::FileFormat) -> &'static str {
        use rhwp::parser::FileFormat;
        match format {
            FileFormat::Hwp => "hwp5",
            FileFormat::Hwpx => "hwpx",
            FileFormat::Hwp3 => "hwp3",
            FileFormat::Hml => "hml",
            FileFormat::DrmProtected => "drm-protected",
            FileFormat::Empty => "empty",
            FileFormat::Unknown => "unknown",
        }
    }

    /// 재귀 걷기 — 심볼릭 링크는 따라가지 않는다(순환 방지).
    fn walk(
        dir: &std::path::Path,
        depth: usize,
        max_depth: Option<usize>,
        out: &mut Vec<std::path::PathBuf>,
    ) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("폴더를 읽을 수 없습니다 - {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("항목을 읽을 수 없습니다 - {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("파일 유형을 읽을 수 없습니다 - {}: {e}", path.display()))?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if max_depth.map(|m| depth < m).unwrap_or(true) {
                    walk(&path, depth + 1, max_depth, out)?;
                }
            } else if file_type.is_file() && ext_claim(&path).is_some() {
                out.push(path);
            }
        }
        Ok(())
    }

    let mut json_mode = false;
    let mut probe = false;
    let mut max_depth: Option<usize> = None;
    let mut limit: Option<usize> = None;
    let mut roots: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--probe" => probe = true,
            "--max-depth" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(n) if n >= 1 => max_depth = Some(n),
                    _ => {
                        eprintln!("오류: --max-depth 뒤에 1 이상의 정수가 필요합니다.");
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                }
            }
            "--limit" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(n) if n >= 1 => limit = Some(n),
                    _ => {
                        eprintln!("오류: --limit 뒤에 1 이상의 정수가 필요합니다.");
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {other}");
                eprintln!("{USAGE}");
                return EXIT_USAGE;
            }
            path => roots.push(path.to_string()),
        }
        i += 1;
    }
    if roots.is_empty() {
        eprintln!("오류: 검색할 경로를 하나 이상 지정해주세요.");
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    }

    // ① 대상 수집 — 루트마다 걷고, 전체를 경로 문자열로 정렬해 결정적 순서를 만든다.
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for root in &roots {
        let path = std::path::Path::new(root);
        if path.is_file() {
            files.push(path.to_path_buf());
            continue;
        }
        if !path.is_dir() {
            eprintln!("오류: 경로가 존재하지 않습니다 - {root}");
            return EXIT_RUNTIME;
        }
        if let Err(message) = walk(path, 1, max_depth, &mut files) {
            eprintln!("오류: {message}");
            return EXIT_RUNTIME;
        }
    }
    files.sort_by_key(|p| p.to_string_lossy().to_string());
    files.dedup();

    // 상한은 정렬 **뒤에** 적용한다 — 남는 부분집합도 결정적이어야 한다.
    let mut truncated = false;
    if let Some(limit) = limit {
        if files.len() > limit {
            files.truncate(limit);
            truncated = true;
        }
    }

    // ② 파일별 레코드.
    let mut records: Vec<serde_json::Value> = Vec::new();
    let mut by_format: std::collections::BTreeMap<String, u64> = Default::default();
    let mut mismatch_count = 0u64;
    let mut probe_failed = 0u64;
    // 암호로 잠긴 파일 **개수** — 자격증명이 아니다. 변수명에 password 를 쓰면
    // CodeQL cleartext-logging 이 요약 출력을 민감정보 기록으로 오탐한다.
    let mut locked_count = 0u64;

    for file in &files {
        let display = file.to_string_lossy().to_string();
        let meta = match fs::metadata(file) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("오류: 파일 정보를 읽을 수 없습니다 - {display}: {e}");
                return EXIT_RUNTIME;
            }
        };
        let data = match fs::read(file) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("오류: 파일을 읽을 수 없습니다 - {display}: {e}");
                return EXIT_RUNTIME;
            }
        };
        let claim = ext_claim(file).unwrap_or("hwp");
        let magic = format_token(rhwp::parser::detect_format(&data));
        let mismatch = ext_mismatch(claim, magic);

        let probe_value = if probe {
            let started = std::time::Instant::now();
            match load_document(&data) {
                Ok(doc) => serde_json::json!({
                    "parseOk": true,
                    "needsPassword": false,
                    "pageCount": doc.page_count(),
                    "ms": started.elapsed().as_millis() as u64,
                }),
                Err(fail) => {
                    probe_failed += 1;
                    let (needs, message) = match fail {
                        LoadError::NeedPassword => {
                            (true, "비밀번호가 필요한 암호 문서입니다".to_string())
                        }
                        LoadError::WrongPassword => (
                            true,
                            "비밀번호가 일치하지 않거나 암호화 데이터가 손상되었습니다".to_string(),
                        ),
                        LoadError::Other(msg) => (false, msg),
                    };
                    if needs {
                        locked_count += 1;
                    }
                    serde_json::json!({
                        "parseOk": false,
                        "needsPassword": needs,
                        "error": message,
                        "ms": started.elapsed().as_millis() as u64,
                    })
                }
            }
        } else {
            serde_json::Value::Null
        };

        *by_format.entry(magic.to_string()).or_insert(0) += 1;
        if mismatch {
            mismatch_count += 1;
        }
        let modified_unix = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        records.push(serde_json::json!({
            "path": display,
            "bytes": meta.len(),
            "modifiedUnix": modified_unix,
            "extFormat": claim,
            "magicFormat": magic,
            "extMismatch": mismatch,
            "probe": probe_value,
        }));
    }

    let summary = serde_json::json!({
        "total": records.len(),
        "byFormat": by_format,
        "extMismatch": mismatch_count,
        "probed": probe,
        "probeFailed": if probe { serde_json::json!(probe_failed) } else { serde_json::Value::Null },
        "needsPassword": if probe { serde_json::json!(locked_count) } else { serde_json::Value::Null },
        "truncated": truncated,
    });

    // ③ 출력.
    if json_mode {
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "roots": roots,
            "files": records,
            "summary": summary,
        });
        println!("{}", provenance::marked(envelope, "scan"));
        return EXIT_OK;
    }

    println!("rhwp scan — {}개 파일", records.len());
    for record in &records {
        let mut notes: Vec<&str> = Vec::new();
        if record["extMismatch"].as_bool() == Some(true) {
            notes.push("확장자 불일치");
        }
        if record["probe"]["needsPassword"].as_bool() == Some(true) {
            notes.push("암호 필요");
        } else if record["probe"]["parseOk"].as_bool() == Some(false) {
            notes.push("파싱 실패");
        }
        let notes = if notes.is_empty() {
            String::new()
        } else {
            format!("  [{}]", notes.join(", "))
        };
        println!(
            "  {}  {}  {}바이트{notes}",
            record["magicFormat"].as_str().unwrap_or("?"),
            record["path"].as_str().unwrap_or("?"),
            record["bytes"].as_u64().unwrap_or(0),
        );
    }
    println!(
        "합계: {} · 확장자 불일치 {}{}",
        records.len(),
        mismatch_count,
        if probe {
            format!(" · 파싱 실패 {probe_failed} (암호 필요 {locked_count})")
        } else {
            String::new()
        }
    );
    EXIT_OK
}

/// [#3238→#3719] batch 축이 공유하는 스트리밍 집계 결과.
struct BatchStreamTally {
    emitted: usize,
    failed: usize,
    verify_diff: usize,
    verify_pages_diff: usize,
    /// stdout 소비자가 끊겨(broken pipe 등) 스트림을 끝까지 내지 못했다.
    aborted: bool,
}

impl BatchStreamTally {
    /// [#3626] 종료 코드 집계. 하드 실패(산출물이 아예 없음)가 가장 나쁘므로 기존 규약대로
    /// 1 이 우선한다. 그 아래는 단건 convert 의 우선순위를 그대로 따른다 — 단건도 쪽수
    /// 검사를 IR 검사보다 먼저 해 exit 4 로 끊는다. 검증 판정을 1 로 접지 않는 이유는
    /// 소비자가 재실행 대상(1)과 검토 대상(3/4)을 갈라야 하기 때문이다.
    fn exit_code(&self) -> i32 {
        if self.failed > 0 {
            EXIT_RUNTIME
        } else if self.verify_pages_diff > 0 {
            4
        } else if self.verify_diff > 0 {
            3
        } else {
            EXIT_OK
        }
    }
}

/// 데이터 파일의 한 행. 읽지 못한 행도 **버리지 않고** 들고 간다 — 스트림에서 조용히
/// 사라지면 소비자는 N행을 넣고 N-1건을 받고도 그것을 성공으로 읽는다.
enum FillRow {
    Data(serde_json::Map<String, serde_json::Value>),
    /// 이 행을 읽지 못한 사유. 그대로 `error` 레코드가 된다.
    Broken(String),
}


/// [#3238] batch 가 처리하는 서브커맨드 축.
#[derive(Clone, Copy)]
enum BatchMode<'a> {
    ExportText,
    Info,
    /// [#3261] 문서 개요/조문 구조 — `export-structure --json` 과 스키마 공유.
    Structure(rhwp::document_core::queries::structure::StructureMode),
    /// [#3346] 표 격자 — `export-tables --json` 과 스키마 공유.
    Tables,
    /// [#3346] 누름틀 조사 — `fields --json` 과 스키마 공유.
    Fields,
    /// [#3346] 주소를 가진 검색 — `search --json` 과 스키마 공유.
    Search {
        query: &'a str,
    },
    /// [#3626] 편집 가능 HWP5 변환 저장 — `convert --json` 봉투와 스키마 공유.
    /// 읽기 전용인 다른 축과 달리 입력마다 파일을 쓰므로 목적지(`out_dir`)를 들고 다닌다.
    Convert {
        out_dir: &'a Path,
        verify: ConversionVerifyOptions,
    },
    /// [#3830] 날짜·금액·수량 추출 — `extract-data --json` 봉투와 스키마 공유.
    /// `limit` 은 **문서마다** 적용되는 상한이다(§6-10) — 전건을 이 축에서 훑어 상한을
    /// 적용하면 뒤쪽 문서가 조용히 0건이 되므로, 문서 하나를 처리하는 이 함수 내부에서
    /// 매 문서마다 독립적으로 절단한다.
    ExtractData {
        kind: &'a str,
        limit: Option<usize>,
    },
}


/// [#3407] `title` 이 훑는 앞쪽 페이지 수 상한 — 표지가 이미지·빈 쪽인 문서의
/// fallback 범위. digest 발췌(`DIGEST_EXCERPT_PAGES`)와 같은 "앞 3쪽" 어휘를 쓴다.
const TITLE_SCAN_PAGES: u32 = 3;

/// [#3633] `nextStep` 고정 문자열 계약 — 봉투를 받은 초소형 모델이 다음 행동을
/// 지어내지 않고 받아 적게 하는 유도문. 문구 변경은 계약 테스트
/// (`tests/digest_macro_contract.rs`)가 잡는 의도적 결정이어야 한다.
const DIGEST_NEXT_STEP: &str = "더 읽으려면 export-text --json -p <쪽>, 찾으려면 search --json";
/// [#3633 후속] sections 모드 nextStep — 절 청크를 받은 모델이 쪽 주소로 원문을
/// 되짚게 하는 고정 유도문. v1 과 같은 고정 문자열 계약이다.
const DIGEST_SECTIONS_NEXT_STEP: &str =
    "절 원문은 export-text --json -p <쪽>, 찾으려면 search --json";
/// [#3633 후속] pages 모드에서 남은 범위가 없을 때의 고정 유도문.
const DIGEST_PAGES_DONE_NEXT_STEP: &str = "범위 발췌 완료 — 더 찾으려면 search --json";
/// [#3633] excerpt 기본 절단 길이(문자 수) — 4B급 모델의 컨텍스트 예산에 맞춘 보수값.
const DIGEST_DEFAULT_MAX_CHARS: usize = 2000;
/// [#3633 후속] sections 모드의 절별 발췌 기본 상한(문자 수) — 절이 수십 개일 수 있어
/// v1 의 2000자보다 훨씬 보수적으로 잡는다. `--max-chars` 로 절별 상한을 바꾼다.
const DIGEST_SECTION_EXCERPT_CHARS: usize = 240;
/// [#3633 후속] sections 봉투에 싣는 청크 최대 개수 — 전체 개수는 sectionCount 로
/// 따로 실어, 봉투만 보고 누락 여부를 판정할 수 있게 한다.
const DIGEST_SECTIONS_LIMIT: usize = 50;
/// [#3633] outline 에 싣는 최상위 노드 제목 최대 개수 — 트리 전체를 싣지 않는다.
const DIGEST_OUTLINE_LIMIT: usize = 20;
/// [#3633] excerpt 원천 페이지 수 — 앞쪽 페이지 0~2 만 발췌한다.
const DIGEST_EXCERPT_PAGES: u32 = 3;


#[derive(Debug, Default, Clone, Copy)]
struct ConversionVerifyOptions {
    verify: bool,
    verify_pages: bool,
    /// [#3596] 봉투를 stdout 순수 JSON 으로. export-hwpx 만 허용한다(`allow_json`).
    json: bool,
}

impl ConversionVerifyOptions {
    fn enabled(self) -> bool {
        self.verify || self.verify_pages
    }
}


struct HmlExportArgs {
    input: std::path::PathBuf,
    output: std::path::PathBuf,
    /// [#3616] 봉투를 stdout 순수 JSON 으로.
    json: bool,
}


/// `dump-records`가 독립적으로 소비하는 단일 본문 스트림 상한.
///
/// 완전 Document 열기 정책을 재사용하지 않고 이 CLI consumer가 명시적으로 선택한다.
const MAX_DUMP_RECORDS_STREAM_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

/// 옵션을 받지 않는 내부 개발 명령의 위치 인자를 엄격히 검증한다.
///
/// 이 명령들은 capabilities 에도 노출되어 있다. 플래그처럼 보이는 값을 위치 인자로
/// 삼키거나 여분 인자를 무시하면, 호출자는 오타 난 자동화를 성공으로 오인한다.
fn validate_internal_positionals(command: &str, args: &[String], max: usize) -> Result<(), i32> {
    if let Some(flag) = args.iter().find(|arg| arg.starts_with('-')) {
        eprintln!("오류: {command} 은 알 수 없는 옵션을 받지 않습니다 - {flag}");
        return Err(EXIT_USAGE);
    }
    if args.len() > max {
        eprintln!("오류: {command} 은 위치 인자를 최대 {max}개만 받습니다.");
        return Err(EXIT_USAGE);
    }
    Ok(())
}

fn test_shape_roundtrip(args: &[String]) -> i32 {
    if let Err(code) = validate_internal_positionals("test-shape", args, 2) {
        return code;
    }
    let input = if args.is_empty() {
        "saved/g555-s.hwp"
    } else {
        &args[0]
    };
    let output = if args.len() > 1 {
        &args[1]
    } else {
        "/tmp/test-shape-out.hwp"
    };

    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("입력 파일 읽기 오류: {}", e);
            return EXIT_RUNTIME;
        }
    };

    let mut doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("HWP 파싱 오류: {:?}", e);
            return EXIT_RUNTIME;
        }
    };

    let _ = doc.convert_to_editable_native();

    // 글상자 생성 (9000 x 6750 HWPUNIT)
    let result = doc.create_shape_control_native(
        0,
        0,
        0,
        9000,
        6750,
        0,
        0,
        false,
        "InFrontOfText",
        "rectangle",
        false,
        false,
        &[],
    );
    match &result {
        Ok(r) => eprintln!("글상자 생성 성공: {}", r),
        Err(e) => {
            eprintln!("글상자 생성 실패: {:?}", e);
            return EXIT_RUNTIME;
        }
    }

    match doc.export_hwp_native() {
        Ok(bytes) => {
            if let Err(e) = fs::write(output, &bytes) {
                eprintln!("파일 저장 오류: {}", e);
                return EXIT_RUNTIME;
            }
            eprintln!("저장 완료: {} ({}KB)", output, bytes.len() / 1024);
            EXIT_OK
        }
        Err(e) => {
            eprintln!("직렬화 오류: {:?}", e);
            EXIT_RUNTIME
        }
    }
}

/// 캡션 방향별 테스트: 4개 이미지에 각각 Bottom/Top/Left/Right 캡션을 설정하고 SVG 출력
fn test_caption(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("사용법: rhwp test-caption <파일.hwp> [-o <출력 폴더>]");
        return EXIT_USAGE;
    }
    if args[0].starts_with('-') {
        eprintln!(
            "오류: test-caption 입력 파일 자리에 옵션을 쓸 수 없습니다 - {}",
            args[0]
        );
        return EXIT_USAGE;
    }

    let input = &args[0];
    let mut output_dir = Path::new("output/caption-test");
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("오류: {} 뒤에 출력 폴더 경로가 필요합니다.", args[i]);
                    return EXIT_USAGE;
                };
                if value.starts_with('-') {
                    eprintln!("오류: {} 뒤에 출력 폴더 경로가 필요합니다.", args[i]);
                    return EXIT_USAGE;
                }
                output_dir = Path::new(value);
                i += 2;
            }
            option => {
                eprintln!("오류: 알 수 없는 test-caption 옵션입니다 - {option}");
                return EXIT_USAGE;
            }
        }
    }

    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("파일 읽기 오류: {}", e);
            return EXIT_RUNTIME;
        }
    };

    let mut doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("파싱 오류: {}", e);
            return EXIT_RUNTIME;
        }
    };

    if doc.document().sections.is_empty() {
        eprintln!("문서 오류: 캡션을 검사할 section이 없습니다.");
        return EXIT_RUNTIME;
    }

    // 문단 0: 컨트롤 2,3 / 문단 1: 컨트롤 0,1
    let pic_refs: [(usize, usize); 4] = [(0, 2), (0, 3), (1, 0), (1, 1)];

    // 4개 이미지에 각각 다른 캡션 방향 설정
    let directions = [
        ("Bottom", "Top"),
        ("Top", "Top"),
        ("Left", "Center"),
        ("Right", "Center"),
    ];

    for (i, ((para, ci), (dir, va))) in pic_refs.iter().zip(directions.iter()).enumerate() {
        let json = format!(
            r#"{{"hasCaption":true,"captionDirection":"{}","captionVertAlign":"{}","captionWidth":8504,"captionSpacing":850}}"#,
            dir, va
        );
        println!("[{}] para={}, ci={}, dir={}, va={}", i, para, ci, dir, va);
        match doc.set_picture_properties_native(0, *para, *ci, &json) {
            Ok(r) => println!("  결과: {}", r),
            Err(e) => println!("  오류: {:?}", e),
        }
    }

    // 캡션 상태 확인
    // [CLI 계약 정합] capabilities 가 "internal" 카테고리로도 <파일.hwp> 를 받는
    // 일반 명령처럼 자기서술한다 — 에이전트가 임의 문서로 호출할 수 있다는 뜻이다.
    // 이 도구는 원래 para=0/1·control 2/3/0/1 을 가진 고정 fixture 전용이었는데,
    // 그 인덱스를 경계검사 없이 바로 인덱싱해 다른 문서를 주면 패닉(exit 101)했다.
    // "안 죽는다"는 CLI 자기서술 계약을 어기므로, 범위를 벗어나면 패닉 대신
    // 제어된 오류를 출력하고 다음 항목으로 넘어간다.
    for (i, (para, ci)) in pic_refs.iter().enumerate() {
        let Some(section) = doc.document().sections.first() else {
            eprintln!("문서 오류: 캡션을 검사할 section이 없습니다.");
            return EXIT_RUNTIME;
        };
        let Some(p) = section.paragraphs.get(*para) else {
            println!(
                "[{}] 건너뜀: para={} 가 문서 범위를 벗어남(문단 {}개)",
                i,
                para,
                section.paragraphs.len()
            );
            continue;
        };
        let Some(ctrl) = p.controls.get(*ci) else {
            println!(
                "[{}] 건너뜀: para={} ci={} 가 범위를 벗어남(컨트롤 {}개)",
                i,
                para,
                ci,
                p.controls.len()
            );
            continue;
        };
        if let rhwp::model::control::Control::Picture(pic) = ctrl {
            println!(
                "[{}] caption={:?}",
                i,
                pic.caption.as_ref().map(|c| {
                    format!(
                        "dir={:?}, paras={}, text={:?}",
                        c.direction,
                        c.paragraphs.len(),
                        c.paragraphs.first().map(|p| &p.text)
                    )
                })
            );
        }
    }

    // SVG 출력
    if let Err(e) = fs::create_dir_all(output_dir) {
        eprintln!("출력 폴더 생성 오류: {}: {}", output_dir.display(), e);
        return EXIT_RUNTIME;
    }
    let page_count = doc.page_count();
    println!("페이지 수: {}", page_count);
    for p in 0..page_count {
        let svg = match doc.render_page_svg(p) {
            Ok(svg) => svg,
            Err(e) => {
                eprintln!("SVG 렌더링 오류(page {}): {:?}", p, e);
                return EXIT_RUNTIME;
            }
        };
        let path = output_dir.join(format!("caption-test-p{}.svg", p));
        if let Err(e) = fs::write(&path, &svg) {
            eprintln!("SVG 저장 오류: {}: {}", path.display(), e);
            return EXIT_RUNTIME;
        }
        println!("  → {}", path.display());
    }
    println!("완료");
    EXIT_OK
}

fn gen_table(args: &[String]) -> i32 {
    if let Err(code) = validate_internal_positionals("gen-table", args, 3) {
        return code;
    }
    let rows = match args.first() {
        Some(value) => match value.parse::<u16>() {
            Ok(value) => value,
            Err(_) => {
                eprintln!("오류: gen-table 행 수는 0~65535 정수여야 합니다 - {value}");
                return EXIT_USAGE;
            }
        },
        None => 1000,
    };
    let cols = match args.get(1) {
        Some(value) => match value.parse::<u16>() {
            Ok(value) => value,
            Err(_) => {
                eprintln!("오류: gen-table 열 수는 0~65535 정수여야 합니다 - {value}");
                return EXIT_USAGE;
            }
        },
        None => 6,
    };
    let output = args
        .get(2)
        .map(|s| s.as_str())
        .unwrap_or("output/gen_table.hwp");

    println!("{}행 × {}열 표 생성 중...", rows, cols);

    let mut core = rhwp::document_core::DocumentCore::new_empty();
    core.create_blank_document_native()
        .expect("빈 문서 생성 실패");

    // 표 생성
    let result = core
        .create_table_native(0, 0, 0, rows, cols)
        .expect("표 생성 실패");
    println!("  표 생성: {}", result);

    // 결과에서 paraIdx 파싱
    let table_para_idx: usize = result
        .split("\"paraIdx\":")
        .nth(1)
        .and_then(|s| s.split(&[',', '}'][..]).next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1);
    println!("  표 문단 인덱스: {}", table_para_idx);

    // 배치 모드로 셀 내용 채우기
    core.begin_batch_native().expect("배치 시작 실패");

    let headers = ["번호", "이름", "부서", "직급", "연락처", "비고"];
    // 헤더 행
    for (ci, header) in headers.iter().enumerate().take(cols as usize) {
        let _ = core.insert_text_in_cell_native(0, table_para_idx, 0, ci, 0, 0, header);
    }

    // 데이터 행
    let departments = ["개발팀", "기획팀", "디자인팀", "영업팀", "인사팀", "재무팀"];
    let positions = ["사원", "대리", "과장", "차장", "부장"];
    for row in 1..rows as usize {
        for col in 0..cols as usize {
            let cell_idx = row * cols as usize + col;
            let text = match col {
                0 => format!("{}", row),
                1 => format!("홍길동{}", row),
                2 => departments[row % departments.len()].to_string(),
                3 => positions[row % positions.len()].to_string(),
                4 => format!(
                    "010-{:04}-{:04}",
                    1000 + row % 9000,
                    1000 + (row * 7) % 9000
                ),
                5 => {
                    if row % 3 == 0 {
                        "특이사항 없음".to_string()
                    } else {
                        String::new()
                    }
                }
                _ => format!("R{}C{}", row, col),
            };
            if !text.is_empty() {
                let _ =
                    core.insert_text_in_cell_native(0, table_para_idx, 0, cell_idx, 0, 0, &text);
            }
        }
        if row % 100 == 0 {
            println!("  {} / {} 행 완료", row, rows);
        }
    }

    core.end_batch_native().expect("배치 종료 실패");
    println!("  셀 내용 입력 완료");

    // 저장
    let bytes = core.export_hwp_native().expect("HWP 내보내기 실패");
    let out_path = Path::new(output);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Err(e) = fs::write(out_path, bytes) {
        // 종료 코드 계약: 쓰기 실패는 런타임 오류(1)다. 종전에는 .expect() 로 패닉해
        // 계약에 없는 101 로 끝났다.
        eprintln!("오류: 파일 저장 실패 - {}: {}", output, e);
        return EXIT_RUNTIME;
    }
    println!("저장 완료: {} ({}행 × {}열)", output, rows, cols);
    EXIT_OK
}

/// PUA (Private Use Area) 문자 셋트를 입력한 HWP 테스트 문서 생성.
///
/// Task #509 (PUA 회귀 정정) 의 한컴 정답지 확보용. 본 라이브러리가 발견한
/// 14 샘플 광범위 PUA 코드포인트 18 종을 한 문서에 입력 → 한컴 편집기로 PDF
/// 출력 + rhwp SVG 출력 시각 비교.
///
/// 사용:
///   rhwp gen-pua [output_path]
///   기본 출력: output/pua-test.hwp
fn gen_pua_test(args: &[String]) -> i32 {
    if let Err(code) = validate_internal_positionals("gen-pua", args, 1) {
        return code;
    }
    // gen-pua 의 positional 은 입력이 아니라 **출력** 경로다. capabilities 가 다른
    // 진단 명령과 나란히 노출하는 탓에 `rhwp gen-pua 문서.hwp` 를 "이 파일을 조사"로
    // 읽은 호출이 실제로 원본을 말없이 덮어썼다(#3691 조사 중 발생). 사용자가 명시한
    // 경로가 이미 있으면 거부한다 — 기본 경로는 재생성 대상이라 검사에서 제외한다.
    let explicit = args.first().map(|s| s.as_str());
    if let Some(path) = explicit {
        if Path::new(path).exists() {
            eprintln!("오류: gen-pua 의 인자는 생성할 **출력** 경로입니다 (입력 파일이 아닙니다).");
            eprintln!("      이미 존재하는 파일을 덮어쓰지 않습니다: {}", path);
            eprintln!("사용법: rhwp gen-pua [출력경로]   # 기본 output/pua-test.hwp");
            return EXIT_USAGE;
        }
    }
    let output = explicit.unwrap_or("output/pua-test.hwp");

    println!("PUA 문자 셋트 입력 HWP 문서 생성 중...");

    let mut core = rhwp::document_core::DocumentCore::new_empty();
    core.create_blank_document_native()
        .expect("빈 문서 생성 실패");

    // PUA 코드포인트 셋트 (Task #509 Stage 1 의 14 샘플 광범위 통계 정합)
    // (codepoint, 영역 분류, 사용 샘플, 본 라이브러리 현재 매핑)
    let pua_set: &[(u32, &str, &str, &str)] = &[
        // ── Basic PUA (0xF020~0xF0FF) — 매핑 표 적용 영역 ──
        (0x0F076, "Basic", "mel-001", "❖ U+2756"),
        (0x0F09F, "Basic", "biz_plan", "• U+2022"),
        (0x0F0A0, "Basic", "synam-001", "▪ U+25AA"),
        (0x0F0A7, "Basic", "kps-ai", "▪ U+25AA"),
        (0x0F0E8, "Basic", "kps-ai", "(미정의)"),
        (0x0F0F2, "Basic", "KTX", "⇩ U+21E9 (의도 정정 후보)"),
        (0x0F0FE, "Basic", "k-water-rfp", "☑ U+2611"),
        // ── Basic PUA — 매핑 표 외 영역 ──
        (0x0F53A, "Basic-out", "hwpspec", "(매핑 표 외)"),
        // ── Supplementary PUA-A (0xF0000~0xFFFFD) — 매핑 표 미지원 영역 ──
        (0xF02B1, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B2, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B3, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B4, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B5, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B6, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B7, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B8, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02B9, "Suppl-A", "mel-001", "(매핑 표 외)"),
        (0xF02EF, "Suppl-A", "KTX (회귀)", "(매핑 표 외) ★"),
    ];

    println!("  PUA 코드포인트 {} 종 입력", pua_set.len());

    core.begin_batch_native().expect("배치 시작 실패");

    // 첫 paragraph (0번) 에 제목 입력
    let title = "[PUA 회귀 검증 — Task #509]";
    core.insert_text_native(0, 0, 0, title)
        .expect("제목 입력 실패");

    // 각 PUA 글자별로 paragraph 추가:
    // "U+0F0F2 (Basic, KTX): {char}    ← 한컴 정답지 / rhwp 비교"
    // 빈 paragraph 추가 + 텍스트 입력 패턴
    for (i, &(cp, area, sample, mapping)) in pua_set.iter().enumerate() {
        let pi = i + 1; // 0번은 제목, 1번부터 PUA paragraphs

        // 새 paragraph 추가 (pi 위치에 새 문단 삽입)
        core.insert_paragraph_native(0, pi)
            .unwrap_or_else(|e| panic!("paragraph 추가 실패 (pi={}): {:?}", pi, e));

        // PUA 글자 char 변환 (i32 unsafe 회피)
        let pua_char =
            char::from_u32(cp).unwrap_or_else(|| panic!("invalid codepoint U+{:05X}", cp));

        // 텍스트: "U+0F0F2 (Basic, KTX, ⇩ U+21E9 매핑): " + PUA + "  ← 한컴 PDF 글리프 정답지"
        let text = format!(
            "U+{:05X} ({}, {}, {}): {}  ← 한컴 PDF 정답지",
            cp, area, sample, mapping, pua_char
        );

        core.insert_text_native(0, pi, 0, &text)
            .unwrap_or_else(|e| panic!("텍스트 입력 실패 (pi={}): {:?}", pi, e));
    }

    core.end_batch_native().expect("배치 종료 실패");

    // 저장
    let bytes = core.export_hwp_native().expect("HWP 내보내기 실패");
    let out_path = Path::new(output);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Err(e) = fs::write(out_path, bytes) {
        // 종료 코드 계약: 쓰기 실패는 런타임 오류(1)다. 종전에는 .expect() 로 패닉해
        // 계약에 없는 101 로 끝났다.
        eprintln!("오류: 파일 저장 실패 - {}: {}", output, e);
        return EXIT_RUNTIME;
    }
    println!("저장 완료: {} ({} 종 PUA)", output, pua_set.len());
    println!();
    println!("다음 단계:");
    println!("  1. 한컴 2022 편집기에서 본 파일 열기 → PDF 출력 (정답지)");
    println!("  2. rhwp export-svg {} → SVG 출력 비교", output);
    println!("  3. 시각 비교로 매핑 정합 확정");
    EXIT_OK
}

fn test_field_roundtrip(args: &[String]) -> i32 {
    // 인자를 생략하면 저장소에 없는 하드코딩 경로("hwp_webctl/bsbc01_10_000.hwp")를
    // `.expect()` 로 읽어 패닉(exit 101)했다 — 계약(cli_commands.md)에 없는 종료 코드라
    // CI 게이트가 분류할 수 없다. 형제 명령 test-caption 과 같은 모양으로 맞춘다
    // (tests/issue_cli_test_caption_no_panic.rs 가 그쪽을 이미 고정하고 있다).
    if args.is_empty() {
        eprintln!("사용법: rhwp test-field <파일.hwp> [출력.hwp]");
        return EXIT_USAGE;
    }
    if let Err(code) = validate_internal_positionals("test-field", args, 2) {
        return code;
    }
    let input = args[0].as_str();
    let output = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("output/field_test.hwp");

    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일 읽기 실패 - {}: {}", input, e);
            return EXIT_RUNTIME;
        }
    };
    let mut core = match rhwp::document_core::DocumentCore::from_bytes(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("오류: 문서 파싱 실패 - {}: {:?}", input, e);
            return EXIT_RUNTIME;
        }
    };

    // 1. 필드 목록 출력
    let fields = core.collect_all_fields();
    println!("=== 필드 목록 ({}개) ===", fields.len());
    for fi in &fields {
        let name = fi.field.field_name().unwrap_or("(이름없음)");
        println!("  {} = \"{}\"", name, fi.value);
    }

    // 2. 필드에 값 설정
    let test_data = [
        ("mbizNm", "청소년 자립지원사업"),
        ("newCtnuTxt", "계속"),
        ("chargerNm", "홍길동"),
        ("telno", "02-1234-5678"),
        ("sFisYear", "2026"),
        // 셀 필드
        ("bizPurps", "청소년 자립 역량 강화"),
        ("bizPrdTxt", "2026.01 ~ 2026.12"),
        ("insttNm", "시청 복지과"),
    ];

    println!("\n=== 필드 값 설정 ===");
    for (name, value) in &test_data {
        match core.set_field_value_by_name(name, value) {
            Ok(r) => println!("  ✓ {} = \"{}\" → {}", name, value, r),
            Err(e) => println!("  ✗ {} = \"{}\" → {}", name, value, e),
        }
    }

    // 3. 설정 후 확인
    println!("\n=== 설정 후 확인 ===");
    let fields2 = core.collect_all_fields();
    for fi in &fields2 {
        let name = fi.field.field_name().unwrap_or("(이름없음)");
        println!("  {} = \"{}\"", name, fi.value);
    }

    // 3.5 pi=0 문단 텍스트 직접 확인
    let para0 = &core.document().sections[0].paragraphs[0];

    // 4. 직렬화 → 저장
    let saved = match core.export_hwp_native() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 직렬화 실패 - {:?}", e);
            return EXIT_RUNTIME;
        }
    };
    if let Err(e) = std::fs::write(output, &saved) {
        eprintln!("오류: 저장 실패 - {}: {}", output, e);
        return EXIT_RUNTIME;
    }
    println!("\n저장: {} ({}바이트)", output, saved.len());

    // 5. 재로딩 → 필드 확인
    let mut core2 = match rhwp::document_core::DocumentCore::from_bytes(&saved) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("오류: 재로딩 실패 - {:?}", e);
            return EXIT_RUNTIME;
        }
    };
    let fields3 = core2.collect_all_fields();
    println!("\n=== 재로딩 후 확인 ===");
    for fi in &fields3 {
        let name = fi.field.field_name().unwrap_or("(이름없음)");
        println!("  {} = \"{}\"", name, fi.value);
    }
    EXIT_OK
}

fn control_tag(c: &rhwp::model::control::Control) -> &'static str {
    use rhwp::model::control::Control;
    match c {
        Control::SectionDef(_) => "secd",
        Control::ColumnDef(_) => "cold",
        Control::Table(_) => "tbl",
        Control::Shape(_) => "shape",
        Control::Picture(_) => "pic",
        Control::Header(_) => "head",
        Control::Footer(_) => "foot",
        Control::Footnote(_) => "fn",
        Control::Endnote(_) => "en",
        Control::AutoNumber(_) => "atno",
        Control::NewNumber(_) => "nwno",
        Control::PageNumberPos(_) => "pgnp",
        Control::Bookmark(_) => "bokm",
        Control::Hyperlink(_) => "hlk",
        Control::Ruby(_) => "ruby",
        Control::CharOverlap(_) => "tcps",
        Control::PageHide(_) => "pghd",
        Control::HiddenComment(_) => "tcmt",
        Control::Equation(_) => "eqed",
        Control::Field(_) => "field",
        Control::Form(_) => "form",
        Control::Unknown(_) => "unknown",
    }
}

fn diff_table(
    diffs: &mut Vec<String>,
    ci: usize,
    a: &rhwp::model::table::Table,
    b: &rhwp::model::table::Table,
) {
    if a.row_count != b.row_count {
        diffs.push(format!(
            "ctrl[{}] tbl rows: A={} vs B={}",
            ci, a.row_count, b.row_count
        ));
    }
    if a.col_count != b.col_count {
        diffs.push(format!(
            "ctrl[{}] tbl cols: A={} vs B={}",
            ci, a.col_count, b.col_count
        ));
    }
    if a.page_break != b.page_break {
        diffs.push(format!(
            "ctrl[{}] tbl page_break: A={:?} vs B={:?}",
            ci, a.page_break, b.page_break
        ));
    }
    if a.repeat_header != b.repeat_header {
        diffs.push(format!(
            "ctrl[{}] tbl repeat_header: A={} vs B={}",
            ci, a.repeat_header, b.repeat_header
        ));
    }
    if a.cell_spacing != b.cell_spacing {
        diffs.push(format!(
            "ctrl[{}] tbl cell_spacing: A={} vs B={}",
            ci, a.cell_spacing, b.cell_spacing
        ));
    }
    if a.border_fill_id != b.border_fill_id {
        diffs.push(format!(
            "ctrl[{}] tbl border_fill_id: A={} vs B={}",
            ci, a.border_fill_id, b.border_fill_id
        ));
    }
    if a.outer_margin_left != b.outer_margin_left
        || a.outer_margin_right != b.outer_margin_right
        || a.outer_margin_top != b.outer_margin_top
        || a.outer_margin_bottom != b.outer_margin_bottom
    {
        diffs.push(format!(
            "ctrl[{}] tbl outer_margin: A=({},{},{},{}) vs B=({},{},{},{})",
            ci,
            a.outer_margin_left,
            a.outer_margin_top,
            a.outer_margin_right,
            a.outer_margin_bottom,
            b.outer_margin_left,
            b.outer_margin_top,
            b.outer_margin_right,
            b.outer_margin_bottom,
        ));
    }
    diff_common_obj(diffs, ci, "tbl", &a.common, &b.common);
    // [#3469] 셀 문단 재귀 비교 — 표 속성만 보면 셀 안의 텍스트 변경이 보이지 않는다.
    // 글상자는 #1807 이 같은 구멍(#1795 "소거망 구멍")을 이미 막았는데 표는 열려 있었다.
    // ir-diff 는 `convert --verify` 게이트의 근거이고 한국 문서는 표가 본체라,
    // 이 구멍은 변환이 표 안의 모든 텍스트를 손상시켜도 통과시킨다.
    diff_table_cells(diffs, ci, a, b);
}

/// [#3469] 표 셀 안의 문단을 재귀 비교한다.
///
/// 셀 목록 길이가 다르면 그 사실만 보고하고, 공통 구간의 셀은 문단 단위로
/// `diff_textbox_paragraph_lists`(글상자와 같은 비교기)로 내려간다. 셀 문단 안의
/// 중첩 표는 그 안에서 다시 이 경로를 타므로 임의 깊이가 자연히 커버된다.
fn diff_table_cells(
    diffs: &mut Vec<String>,
    ci: usize,
    a: &rhwp::model::table::Table,
    b: &rhwp::model::table::Table,
) {
    use rhwp::model::control::Control;

    if a.cells.len() != b.cells.len() {
        diffs.push(format!(
            "ctrl[{}] tbl 셀 수: A={} vs B={}",
            ci,
            a.cells.len(),
            b.cells.len()
        ));
    }
    for (k, (ca, cb)) in a.cells.iter().zip(b.cells.iter()).enumerate() {
        let prefix = format!("ctrl[{}] tbl cell[{}:{},{}]", ci, k, ca.row, ca.col);
        diff_textbox_paragraph_lists(diffs, &prefix, &ca.paragraphs, &cb.paragraphs);
        // 셀 문단이 품은 중첩 표도 같은 규칙으로 내려간다.
        for (pi, (pa, pb)) in ca.paragraphs.iter().zip(cb.paragraphs.iter()).enumerate() {
            for (cj, (na, nb)) in pa.controls.iter().zip(pb.controls.iter()).enumerate() {
                if let (Control::Table(ta), Control::Table(tb)) = (na, nb) {
                    diff_table_cells(diffs, ci, ta, tb);
                    let _ = (pi, cj);
                }
            }
        }
    }
}

fn diff_common_obj(
    diffs: &mut Vec<String>,
    ci: usize,
    tag: &str,
    a: &rhwp::model::shape::CommonObjAttr,
    b: &rhwp::model::shape::CommonObjAttr,
) {
    if a.treat_as_char != b.treat_as_char {
        diffs.push(format!(
            "ctrl[{}] {} tac: A={} vs B={}",
            ci, tag, a.treat_as_char, b.treat_as_char
        ));
    }
    if a.text_wrap != b.text_wrap {
        diffs.push(format!(
            "ctrl[{}] {} wrap: A={:?} vs B={:?}",
            ci, tag, a.text_wrap, b.text_wrap
        ));
    }
    if a.width != b.width || a.height != b.height {
        diffs.push(format!(
            "ctrl[{}] {} size: A={}x{} vs B={}x{}",
            ci, tag, a.width, a.height, b.width, b.height
        ));
    }
    if a.vertical_offset != b.vertical_offset {
        diffs.push(format!(
            "ctrl[{}] {} v_offset: A={} vs B={}",
            ci, tag, a.vertical_offset, b.vertical_offset
        ));
    }
    if a.horizontal_offset != b.horizontal_offset {
        diffs.push(format!(
            "ctrl[{}] {} h_offset: A={} vs B={}",
            ci, tag, a.horizontal_offset, b.horizontal_offset
        ));
    }
    if a.vert_rel_to != b.vert_rel_to {
        diffs.push(format!(
            "ctrl[{}] {} vert_rel: A={:?} vs B={:?}",
            ci, tag, a.vert_rel_to, b.vert_rel_to
        ));
    }
    if a.horz_rel_to != b.horz_rel_to {
        diffs.push(format!(
            "ctrl[{}] {} horz_rel: A={:?} vs B={:?}",
            ci, tag, a.horz_rel_to, b.horz_rel_to
        ));
    }
}

/// [#1807] 글상자 문단 한 쌍의 핵심 필드 비교 — 본문 문단 비교의 축약판.
/// 직렬화 결함(#1795: FIELD_END 갭 선점 → char_offsets 시프트)이 글상자 안에서
/// 발생해도 ir-diff 가 검출하도록 text/cc/char_offsets/char_shapes/line_segs/
/// field_ranges 를 비교한다.
fn diff_textbox_paragraph_fields(
    diffs: &mut Vec<String>,
    prefix: &str,
    pa: &rhwp::model::paragraph::Paragraph,
    pb: &rhwp::model::paragraph::Paragraph,
) {
    if pa.text != pb.text {
        diffs.push(format!(
            "{} text: A={:?} vs B={:?}",
            prefix,
            pa.text.chars().take(30).collect::<String>(),
            pb.text.chars().take(30).collect::<String>()
        ));
    }
    if pa.char_count != pb.char_count {
        diffs.push(format!(
            "{} cc: A={} vs B={}",
            prefix, pa.char_count, pb.char_count
        ));
    }
    if pa.char_offsets != pb.char_offsets {
        if pa.char_offsets.len() != pb.char_offsets.len() {
            diffs.push(format!(
                "{} char_offsets len: A={} vs B={}",
                prefix,
                pa.char_offsets.len(),
                pb.char_offsets.len()
            ));
        } else if let Some((idx, (a, b))) = pa
            .char_offsets
            .iter()
            .zip(pb.char_offsets.iter())
            .enumerate()
            .find(|(_, (a, b))| a != b)
        {
            diffs.push(format!(
                "{} char_offsets[{}]: A={} vs B={}",
                prefix, idx, a, b
            ));
        }
    }
    if pa.char_shapes.len() != pb.char_shapes.len() {
        diffs.push(format!(
            "{} char_shapes count: A={} vs B={}",
            prefix,
            pa.char_shapes.len(),
            pb.char_shapes.len()
        ));
    } else if let Some((idx, (ca, cb))) = pa
        .char_shapes
        .iter()
        .zip(pb.char_shapes.iter())
        .enumerate()
        .find(|(_, (ca, cb))| ca.start_pos != cb.start_pos || ca.char_shape_id != cb.char_shape_id)
    {
        diffs.push(format!(
            "{} cs[{}]: A=({},{}) vs B=({},{})",
            prefix, idx, ca.start_pos, ca.char_shape_id, cb.start_pos, cb.char_shape_id
        ));
    }
    if pa.line_segs.len() != pb.line_segs.len() {
        diffs.push(format!(
            "{} line_segs count: A={} vs B={}",
            prefix,
            pa.line_segs.len(),
            pb.line_segs.len()
        ));
    } else if let Some((idx, (la, lb))) = pa
        .line_segs
        .iter()
        .zip(pb.line_segs.iter())
        .enumerate()
        .find(|(_, (la, lb))| la.text_start != lb.text_start || la.vertical_pos != lb.vertical_pos)
    {
        diffs.push(format!(
            "{} ls[{}]: A=(ts={},vpos={}) vs B=(ts={},vpos={})",
            prefix, idx, la.text_start, la.vertical_pos, lb.text_start, lb.vertical_pos
        ));
    }
    if pa.field_ranges.len() != pb.field_ranges.len() {
        diffs.push(format!(
            "{} field_ranges count: A={} vs B={}",
            prefix,
            pa.field_ranges.len(),
            pb.field_ranges.len()
        ));
    } else if let Some((idx, (fa, fb))) = pa
        .field_ranges
        .iter()
        .zip(pb.field_ranges.iter())
        .enumerate()
        .find(|(_, (fa, fb))| {
            fa.start_char_idx != fb.start_char_idx
                || fa.end_char_idx != fb.end_char_idx
                || fa.control_idx != fb.control_idx
        })
    {
        diffs.push(format!(
            "{} field_ranges[{}]: A=({}..{},c{}) vs B=({}..{},c{})",
            prefix,
            idx,
            fa.start_char_idx,
            fa.end_char_idx,
            fa.control_idx,
            fb.start_char_idx,
            fb.end_char_idx,
            fb.control_idx
        ));
    }
}

/// [#1807] 글상자 문단 목록 재귀 비교. 중첩 글상자(Shape in Shape)도 재귀한다.
fn diff_textbox_paragraph_lists(
    diffs: &mut Vec<String>,
    prefix: &str,
    pas: &[rhwp::model::paragraph::Paragraph],
    pbs: &[rhwp::model::paragraph::Paragraph],
) {
    use rhwp::model::control::Control;
    if pas.len() != pbs.len() {
        diffs.push(format!(
            "{} tb 문단 수: A={} vs B={}",
            prefix,
            pas.len(),
            pbs.len()
        ));
    }
    for (k, (pa, pb)) in pas.iter().zip(pbs.iter()).enumerate() {
        let p = format!("{} tb_p[{}]", prefix, k);
        diff_textbox_paragraph_fields(diffs, &p, pa, pb);
        for (cj, (ca, cb)) in pa.controls.iter().zip(pb.controls.iter()).enumerate() {
            if let (Control::Shape(sa), Control::Shape(sb)) = (ca, cb) {
                diff_shape_textbox(diffs, &format!("{}.ctrl[{}]", p, cj), sa, sb);
            }
        }
    }
}

/// [#1807] Shape 글상자 유무 + 내부 문단 재귀 비교 진입점.
fn diff_shape_textbox(
    diffs: &mut Vec<String>,
    prefix: &str,
    sa: &rhwp::model::shape::ShapeObject,
    sb: &rhwp::model::shape::ShapeObject,
) {
    let ta = sa.drawing().and_then(|d| d.text_box.as_ref());
    let tb = sb.drawing().and_then(|d| d.text_box.as_ref());
    match (ta, tb) {
        (Some(ta), Some(tb)) => {
            diff_textbox_paragraph_lists(diffs, prefix, &ta.paragraphs, &tb.paragraphs);
        }
        (Some(_), None) | (None, Some(_)) => {
            diffs.push(format!(
                "{} text_box 유무: A={} vs B={}",
                prefix,
                ta.is_some(),
                tb.is_some()
            ));
        }
        (None, None) => {}
    }
}

/// `tab_extended`(`[u16; 7]`) 두 인라인 탭 레코드가 **의미 있는** 필드에서 다른지 판정.
///
/// HWPX 파서(`parse_tab_extension`)는 인라인 탭을 `ext[0]`=width,
/// `ext[2]`=`type<<8 | leader`(leader 는 low byte), `ext[6]`=0x0009 마커로만 채우고
/// `ext[1]`·`ext[3]`·`ext[4]`·`ext[5]`는 0 으로 둔다. HWPX 직렬화(`render_hp_t_content`)도
/// width/leader/type 를 오직 `ext[0]`·`ext[2]`에서만 읽는다. 반면 HWP5 인라인 탭(8 WCHAR
/// 블록)은 `ext[1]`을 leader/fill 슬롯으로, `ext[3]`·`ext[4]`·`ext[5]`를 WCHAR 4~6 원본
/// 바이트(보통 0x20)로 채운다 — 이들은 HWPX `<hp:tab>`에 대응 속성이 없어 HWPX 쪽이 항상
/// 0 이라, HWPX↔HWP5 parity 비교에서 거의 모든 탭에 거짓 차이(0 vs leader, 0 vs 32)를 만들어
/// 실제 차이(width/type/leader)를 가린다. 따라서 두 포맷이 공통으로 쓰는 필드
/// [0]=width, [2]=type/leader 팩, [6]=마커만 비교하고 [1],[3],[4],[5]는 제외한다.
/// (HWP5 직렬화는 [1],[3..6]을 그대로 보존하므로 self-roundtrip 충실도에는 영향 없음 —
/// 도구 비교에서만 제외.)
fn tab_ext_semantic_differs(a: &[u16; 7], b: &[u16; 7]) -> bool {
    // 두 포맷 공통 필드만: [0]=width, [2]=type<<8|leader, [6]=0x0009 마커.
    // [1](HWP5 leader/fill 슬롯, HWPX=0)·[3]·[4]·[5](HWP5 예약 바이트, HWPX=0)는 제외.
    const SEMANTIC: [usize; 3] = [0, 2, 6];
    SEMANTIC.iter().any(|&k| a[k] != b[k])
}

/// [Task #2122] ir-diff 출력 상태 — 종전 fn-지역 macro(emit_header/emit_diff) 본문을
/// 메서드로 이관 (동작·출력 불변, macro 확장 인라인 제거).
struct IrDiffEmitter {
    summary_mode: bool,
    max_lines: Option<usize>,
    printed_lines: usize,
    truncated: bool,
    summary_buckets: std::collections::BTreeMap<String, u32>,
}

impl IrDiffEmitter {
    fn println_guarded(&mut self, line: String) {
        match self.max_lines {
            Some(limit) if self.printed_lines >= limit => {
                if !self.truncated {
                    println!("... 이하 생략 (--max-lines {} 도달)", limit);
                    self.truncated = true;
                }
            }
            _ => {
                println!("{}", line);
                self.printed_lines += 1;
            }
        }
    }
    /// paragraph/섹션 헤더. summary 모드에서는 출력 안 함, max_lines 초과 시 truncate.
    fn header(&mut self, line: String) {
        if !self.summary_mode {
            self.println_guarded(line);
        }
    }
    /// 차이 라인. summary 모드에서는 카테고리별 카운트, 일반 모드에서는 "  [차이] {}" 형식.
    /// 카테고리 추출: ":" 앞쪽 첫 토큰. controls[N].xxx 는 ".xxx" 만 추출.
    fn diff(&mut self, body: String) {
        if self.summary_mode {
            let prefix = body.split(':').next().unwrap_or(&body);
            let cat = if let Some(pos) = prefix.rfind(']') {
                prefix[pos + 1..].trim_start_matches('.').trim().to_string()
            } else {
                prefix.trim().to_string()
            };
            let key = if cat.is_empty() { body.clone() } else { cat };
            *self.summary_buckets.entry(key).or_insert(0) += 1;
        } else {
            self.println_guarded(format!("  [차이] {}", body));
        }
    }
}

/// [Task #2122] ir-diff 문단 단위 필드 비교 — 차이 문자열 목록 생산 (원본 무변경 이동).
fn ir_diff_paragraph_fields(
    pa: &rhwp::model::paragraph::Paragraph,
    pb: &rhwp::model::paragraph::Paragraph,
    doc_a: &rhwp::model::document::Document,
    doc_b: &rhwp::model::document::Document,
) -> Vec<String> {
    let mut diffs: Vec<String> = Vec::new();

    // 텍스트 비교
    if pa.text != pb.text {
        diffs.push(format!(
            "text: A={:?} vs B={:?}",
            pa.text.chars().take(30).collect::<String>(),
            pb.text.chars().take(30).collect::<String>()
        ));
    }

    // char_count 비교
    if pa.char_count != pb.char_count {
        diffs.push(format!("cc: A={} vs B={}", pa.char_count, pb.char_count));
    }

    // char_offsets 비교
    if pa.char_offsets != pb.char_offsets {
        let len_a = pa.char_offsets.len();
        let len_b = pb.char_offsets.len();
        if len_a != len_b {
            diffs.push(format!("char_offsets len: A={} vs B={}", len_a, len_b));
        } else {
            let first_diff = pa
                .char_offsets
                .iter()
                .zip(pb.char_offsets.iter())
                .enumerate()
                .find(|(_, (a, b))| a != b);
            if let Some((idx, (a, b))) = first_diff {
                diffs.push(format!("char_offsets[{}]: A={} vs B={}", idx, a, b));
            }
        }
    }

    // para_shape_id 비교
    if pa.para_shape_id != pb.para_shape_id {
        diffs.push(format!(
            "ps_id: A={} vs B={}",
            pa.para_shape_id, pb.para_shape_id
        ));
    }

    // tab_extended 비교
    if pa.tab_extended.len() != pb.tab_extended.len() {
        diffs.push(format!(
            "tab_ext count: A={} vs B={}",
            pa.tab_extended.len(),
            pb.tab_extended.len()
        ));
    } else {
        for (ti, (ta, tb)) in pa
            .tab_extended
            .iter()
            .zip(pb.tab_extended.iter())
            .enumerate()
        {
            if tab_ext_semantic_differs(ta, tb) {
                diffs.push(format!("tab_ext[{}]: A={:?} vs B={:?}", ti, ta, tb));
                break;
            }
        }
    }

    // LINE_SEG 비교
    if pa.line_segs.len() != pb.line_segs.len() {
        diffs.push(format!(
            "line_segs count: A={} vs B={}",
            pa.line_segs.len(),
            pb.line_segs.len()
        ));
    } else {
        for (li, (la, lb)) in pa.line_segs.iter().zip(pb.line_segs.iter()).enumerate() {
            if la.text_start != lb.text_start {
                diffs.push(format!(
                    "ls[{}].ts: A={} vs B={}",
                    li, la.text_start, lb.text_start
                ));
            }
            if la.vertical_pos != lb.vertical_pos {
                diffs.push(format!(
                    "ls[{}].vpos: A={} vs B={}",
                    li, la.vertical_pos, lb.vertical_pos
                ));
            }
            if la.line_height != lb.line_height {
                diffs.push(format!(
                    "ls[{}].lh: A={} vs B={}",
                    li, la.line_height, lb.line_height
                ));
            }
            if la.text_height != lb.text_height {
                diffs.push(format!(
                    "ls[{}].th: A={} vs B={}",
                    li, la.text_height, lb.text_height
                ));
            }
            if la.baseline_distance != lb.baseline_distance {
                diffs.push(format!(
                    "ls[{}].bl: A={} vs B={}",
                    li, la.baseline_distance, lb.baseline_distance
                ));
            }
            if la.line_spacing != lb.line_spacing {
                diffs.push(format!(
                    "ls[{}].ls: A={} vs B={}",
                    li, la.line_spacing, lb.line_spacing
                ));
            }
            if la.column_start != lb.column_start {
                diffs.push(format!(
                    "ls[{}].cs: A={} vs B={}",
                    li, la.column_start, lb.column_start
                ));
            }
            if la.segment_width != lb.segment_width {
                diffs.push(format!(
                    "ls[{}].sw: A={} vs B={}",
                    li, la.segment_width, lb.segment_width
                ));
            }
        }
    }

    // 컨트롤 식별 비교
    if pa.controls.len() != pb.controls.len() {
        diffs.push(format!(
            "controls count: A={} vs B={}",
            pa.controls.len(),
            pb.controls.len()
        ));
    }
    {
        use rhwp::model::control::Control;
        let ctrl_count = pa.controls.len().min(pb.controls.len());
        for ci in 0..ctrl_count {
            let ca = &pa.controls[ci];
            let cb = &pb.controls[ci];
            match (ca, cb) {
                (Control::Table(ta), Control::Table(tb)) => {
                    diff_table(&mut diffs, ci, ta, tb);
                }
                (Control::Picture(pic_a), Control::Picture(pic_b)) => {
                    diff_common_obj(&mut diffs, ci, "pic", &pic_a.common, &pic_b.common);
                }
                (Control::Shape(sa), Control::Shape(sb)) => {
                    diff_common_obj(&mut diffs, ci, "shape", sa.common(), sb.common());
                    // [#1807] 글상자 내부 문단 재귀 비교 — 직렬화 결함이
                    // 글상자 안에서 발생해도 검출되도록 (#1795 소거망 구멍)
                    diff_shape_textbox(&mut diffs, &format!("ctrl[{}] shape", ci), sa, sb);
                }
                _ if control_tag(ca) != control_tag(cb) => {
                    diffs.push(format!(
                        "ctrl[{}] type: A={} vs B={}",
                        ci,
                        control_tag(ca),
                        control_tag(cb)
                    ));
                }
                _ => {}
            }
        }
    }

    // char_shapes 비교
    if pa.char_shapes.len() != pb.char_shapes.len() {
        diffs.push(format!(
            "char_shapes count: A={} vs B={}",
            pa.char_shapes.len(),
            pb.char_shapes.len()
        ));
    } else {
        for (ci, (ca, cb)) in pa.char_shapes.iter().zip(pb.char_shapes.iter()).enumerate() {
            if ca.start_pos != cb.start_pos {
                diffs.push(format!(
                    "cs[{}].pos: A={} vs B={}",
                    ci, ca.start_pos, cb.start_pos
                ));
                break;
            }
            if ca.char_shape_id != cb.char_shape_id {
                diffs.push(format!(
                    "cs[{}].id: A={} vs B={}",
                    ci, ca.char_shape_id, cb.char_shape_id
                ));
                break;
            }
        }
    }
    diffs
}

/// [#4113 / #3918 승격 2호] `verify` — 편집 파이프라인의 독립 사후검증 게이트.
///
/// 기대 조건 집합을 문서 실측과 대조해 전부 만족이면 exit 0, 하나라도 어긋나면
/// **봉투를 먼저 내고** exit 3(판정 — #2707) — 판정은 데이터다(규칙 3). 실행
/// 실패는 stdout 을 비우고 exit 1, 조립 오류는 exit 2. 실측은 전부 기존 코어
/// 재사용이다: `page_count`·`grep`·`collect_field_records`·`detect_format`(규칙 2).
fn cmd_verify(args: &[String]) -> i32 {
    const USAGE: &str = "사용법: rhwp verify <파일.hwp|파일.hwpx> [--expect-pages N] \
[--expect-min-pages N] [--expect-max-pages N] [--expect-min-chars N] \
[--expect-min-tables N] [--expect-table-count N] \
[--expect-contains 문자열]... [--expect-not-contains 문자열]... [--expect-field 이름=값]... \
[--expect-format hwp5|hwpx|hwp3|hml] [--json] — 기대 조건이 최소 1개 필요합니다";

    let mut file_path: Option<&str> = None;
    let mut json_mode = false;
    let mut expect_pages: Option<u64> = None;
    let mut expect_min_pages: Option<u64> = None;
    let mut expect_max_pages: Option<u64> = None;
    let mut expect_min_chars: Option<u64> = None;
    let mut expect_min_tables: Option<u64> = None;
    let mut expect_table_count: Option<u64> = None;
    let mut expect_format: Option<String> = None;
    let mut expect_contains: Vec<String> = Vec::new();
    let mut expect_not_contains: Vec<String> = Vec::new();
    let mut expect_fields: Vec<(String, String)> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            flag @ ("--expect-pages"
            | "--expect-min-pages"
            | "--expect-max-pages"
            | "--expect-min-chars"
            | "--expect-min-tables"
            | "--expect-table-count") => {
                i += 1;
                let n = args.get(i).and_then(|v| v.parse::<u64>().ok());
                match n {
                    Some(n) => {
                        *match flag {
                            "--expect-pages" => &mut expect_pages,
                            "--expect-min-pages" => &mut expect_min_pages,
                            "--expect-max-pages" => &mut expect_max_pages,
                            "--expect-min-chars" => &mut expect_min_chars,
                            "--expect-min-tables" => &mut expect_min_tables,
                            _ => &mut expect_table_count,
                        } = Some(n);
                    }
                    None => {
                        eprintln!("오류: {flag} 뒤에 숫자가 필요합니다.");
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                }
            }
            "--expect-contains" => {
                i += 1;
                match args.get(i) {
                    Some(v) => expect_contains.push(v.clone()),
                    None => {
                        eprintln!("오류: --expect-contains 뒤에 문자열이 필요합니다.");
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                }
            }
            "--expect-not-contains" => {
                i += 1;
                match args.get(i) {
                    Some(v) => expect_not_contains.push(v.clone()),
                    None => {
                        eprintln!("오류: --expect-not-contains 뒤에 문자열이 필요합니다.");
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                }
            }
            "--expect-field" => {
                i += 1;
                match args.get(i).and_then(|v| v.split_once('=')) {
                    Some((k, val)) if !k.is_empty() => {
                        expect_fields.push((k.to_string(), val.to_string()))
                    }
                    _ => {
                        eprintln!("오류: --expect-field 는 이름=값 형식입니다.");
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                }
            }
            "--expect-format" => {
                i += 1;
                match args.get(i).map(String::as_str) {
                    Some(v @ ("hwp5" | "hwpx" | "hwp3" | "hml")) => {
                        expect_format = Some(v.to_string())
                    }
                    Some(v) => {
                        eprintln!(
                            "오류: --expect-format 은 hwp5|hwpx|hwp3|hml 중 하나입니다 - {v}"
                        );
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                    None => {
                        eprintln!("오류: --expect-format 뒤에 형식이 필요합니다.");
                        eprintln!("{USAGE}");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("오류: 알 수 없는 옵션입니다 - {other}");
                eprintln!("{USAGE}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.is_some() {
                    eprintln!("오류: 파일 경로는 하나여야 합니다 - {other}");
                    eprintln!("{USAGE}");
                    return EXIT_USAGE;
                }
                file_path = Some(other);
            }
        }
        i += 1;
    }
    let Some(path) = file_path else {
        eprintln!("오류: 파일 경로가 필요합니다.");
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };
    let expectation_count = usize::from(expect_pages.is_some())
        + usize::from(expect_min_pages.is_some())
        + usize::from(expect_max_pages.is_some())
        + usize::from(expect_min_chars.is_some())
        + usize::from(expect_min_tables.is_some())
        + usize::from(expect_table_count.is_some())
        + usize::from(expect_format.is_some())
        + expect_contains.len()
        + expect_not_contains.len()
        + expect_fields.len();
    if expectation_count == 0 {
        eprintln!("오류: 기대 조건이 없습니다 — --expect-* 로 최소 1개를 지정하세요.");
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    }

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", path, e);
            return EXIT_RUNTIME;
        }
    };
    let actual_format = match rhwp::parser::detect_format(&data) {
        rhwp::parser::FileFormat::Hwp => "hwp5",
        rhwp::parser::FileFormat::Hwpx => "hwpx",
        rhwp::parser::FileFormat::Hwp3 => "hwp3",
        rhwp::parser::FileFormat::Hml => "hml",
        _ => "unknown",
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: HWP 파싱 실패 - {}", e);
            return EXIT_RUNTIME;
        }
    };

    let mut expectations: Vec<serde_json::Value> = Vec::new();
    let mut fail_count = 0usize;
    let mut record = |kind: &str,
                      subject: serde_json::Value,
                      expected: serde_json::Value,
                      actual: serde_json::Value,
                      pass: bool| {
        if !pass {
            fail_count += 1;
        }
        let mut e = serde_json::json!({
            "kind": kind, "expected": expected, "actual": actual, "pass": pass,
        });
        if !subject.is_null() {
            e["subject"] = subject;
        }
        expectations.push(e);
    };

    if let Some(n) = expect_pages {
        let actual = u64::from(doc.page_count());
        record(
            "pages",
            serde_json::Value::Null,
            serde_json::json!(n),
            serde_json::json!(actual),
            actual == n,
        );
    }
    if let Some(n) = expect_min_pages {
        let actual = u64::from(doc.page_count());
        record(
            "minPages",
            serde_json::Value::Null,
            serde_json::json!(n),
            serde_json::json!(actual),
            actual >= n,
        );
    }
    if let Some(n) = expect_max_pages {
        let actual = u64::from(doc.page_count());
        record(
            "maxPages",
            serde_json::Value::Null,
            serde_json::json!(n),
            serde_json::json!(actual),
            actual <= n,
        );
    }
    if let Some(n) = expect_min_chars {
        // 쪽별 추출 텍스트의 문자 수 합 — export-text 와 같은 출처를 쓴다.
        let mut actual = 0u64;
        for page in 0..doc.page_count() {
            match doc.extract_page_text_native(page) {
                Ok(text) => actual += text.chars().count() as u64,
                Err(e) => {
                    eprintln!("오류: 본문 텍스트 추출 실패 - {}쪽: {}", page, e);
                    return EXIT_RUNTIME;
                }
            }
        }
        record(
            "minChars",
            serde_json::Value::Null,
            serde_json::json!(n),
            serde_json::json!(actual),
            actual >= n,
        );
    }
    if expect_min_tables.is_some() || expect_table_count.is_some() {
        use rhwp::document_core::queries::table_extract::extract_tables;
        let actual = extract_tables(doc.document()).len() as u64;
        if let Some(n) = expect_min_tables {
            record(
                "minTables",
                serde_json::Value::Null,
                serde_json::json!(n),
                serde_json::json!(actual),
                actual >= n,
            );
        }
        if let Some(n) = expect_table_count {
            record(
                "tableCount",
                serde_json::Value::Null,
                serde_json::json!(n),
                serde_json::json!(actual),
                actual == n,
            );
        }
    }
    if let Some(f) = expect_format.as_deref() {
        record(
            "format",
            serde_json::Value::Null,
            serde_json::json!(f),
            serde_json::json!(actual_format),
            actual_format == f,
        );
    }
    for s in &expect_contains {
        let n = doc.grep(s, true, None).len();
        record(
            "contains",
            serde_json::json!(s),
            serde_json::json!(">=1"),
            serde_json::json!(n),
            n >= 1,
        );
    }
    for s in &expect_not_contains {
        let n = doc.grep(s, true, None).len();
        record(
            "notContains",
            serde_json::json!(s),
            serde_json::json!(0),
            serde_json::json!(n),
            n == 0,
        );
    }
    if !expect_fields.is_empty() {
        let records = collect_field_records(&doc);
        for (name, want) in &expect_fields {
            let actual = records
                .iter()
                .find(|r| r["name"].as_str() == Some(name.as_str()))
                .map(|r| r["value"].clone())
                .unwrap_or(serde_json::Value::Null);
            let pass = actual.as_str() == Some(want.as_str());
            record(
                "field",
                serde_json::json!(name),
                serde_json::json!(want),
                actual,
                pass,
            );
        }
    }

    let verdict = if fail_count == 0 { "pass" } else { "fail" };
    let pass_count = expectation_count - fail_count;
    if json_mode {
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": path,
            "expectations": expectations,
            "passCount": pass_count,
            "failCount": fail_count,
            "verdict": verdict,
        });
        println!("{}", provenance::marked(envelope, "verify"));
    } else {
        for e in &expectations {
            let mark = if e["pass"].as_bool() == Some(true) {
                "PASS"
            } else {
                "FAIL"
            };
            let subject = e["subject"]
                .as_str()
                .map(|s| format!(" '{s}'"))
                .unwrap_or_default();
            println!(
                "{mark} {}{subject} — 기대 {} / 실측 {}",
                e["kind"].as_str().unwrap_or(""),
                e["expected"],
                e["actual"]
            );
        }
        println!("판정: {verdict} ({pass_count} 통과 / {fail_count} 불일치)");
    }
    if fail_count == 0 {
        EXIT_OK
    } else {
        3 // 판정 불일치 — #2707 의 판정 코드. 봉투는 이미 냈다.
    }
}

/// 두 문서의 IR 을 **전수** 대조한다 — `diagnostics::ir_field_sweep` 을 CLI 로 낸 것.
///
/// `ir-diff` 와 갈리는 점은 **비교 대상이 손으로 나열되지 않는다**는 것이다. `ir-diff` 는
/// 사건 대응으로 쌓인 화이트리스트라 `z_order`·도형 변환 행렬·표 속성 같은 것을 아예 보지
/// 않는다. 실제로 한글이 `ShapeObjBringToFront` 를 저장본에 적어 두었는데 `ir-diff` 는
/// "동일" 이라 답했고, 이 스윕은 `common.z_order` 가 1↔2 로 뒤바뀐 것을 그대로 짚었다.
///
/// 쓰임새는 **편집 액션의 자취를 재는 것**이다. 어떤 API 도 결과를 안 비추는 액션이라도
/// 저장본은 적으므로, 같은 문서의 앞뒤 저장본을 이걸로 대조하면 관측창이 생긴다
/// (`tools/hwpctrl_compat` 의 L3).
/// 문단의 **스트림 좌표**를 찍는다 — 컨트롤 종류·`char_offsets`·컨트롤의 글자 위치.
///
/// 편집 액션이 개체 앵커를 옮기는지 볼 때 쓴다(계획서 §4.24 가 이걸로 나왔다). `ir-sweep`
/// 은 필드 나열이라 "컨트롤과 공백의 순서가 바뀌었다" 같은 **구조** 변화를 읽기 어렵다 —
/// 이 보기는 문단 하나를 스트림 순서 그대로 편다. 여태 임시 테스트 파일로 하던 일이다.
fn dump_anchors(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("사용법: rhwp dump-anchors <파일…> [--all]");
        return EXIT_USAGE;
    }
    let all = args.iter().any(|a| a == "--all");
    for path in args.iter().filter(|a| !a.starts_with('-')) {
        let doc = match std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| rhwp::parser::parse_document(&b).map_err(|e| e.to_string()))
        {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{path}: {e}");
                return EXIT_RUNTIME;
            }
        };
        println!("== {path}");
        for (si, sec) in doc.sections.iter().enumerate() {
            for (pi, para) in sec.paragraphs.iter().enumerate() {
                if !all && para.controls.is_empty() {
                    continue;
                }
                let kinds: Vec<String> = para
                    .controls
                    .iter()
                    .map(|c| match c {
                        rhwp::model::control::Control::SectionDef(_) => "secd".to_string(),
                        rhwp::model::control::Control::ColumnDef(_) => "cold".to_string(),
                        rhwp::model::control::Control::Table(_) => "표".to_string(),
                        rhwp::model::control::Control::Picture(_) => "그림".to_string(),
                        rhwp::model::control::Control::Shape(s) => s.shape_name().to_string(),
                        other => format!("{other:?}")
                            .split(['(', ' '])
                            .next()
                            .unwrap_or("?")
                            .to_string(),
                    })
                    .collect();
                println!(
                    "s{si} p{pi}: chars={} text={:?}",
                    para.char_count, para.text
                );
                println!("   char_offsets={:?}", para.char_offsets);
                println!("   controls={kinds:?}");
                println!("   ctrl_positions={:?}", para.control_text_positions());
            }
        }
    }
    EXIT_OK
}

/// 문단 전 오프셋의 **캐럿 사각형**(x·y·height)을 찍는다 — studio 가 딛는 `getCursorRect`.
///
/// 줌·DPI 무관한 **문서 좌표**의 캐럿 기하다(한글의 화면 캐럿과 달리 안정적이다). 캐럿 높이는
/// 폰트에 달리므로 폰트별 표본으로 돌려 크기를 견준다. `--json` 은 한 줄 계약 봉투.
fn dump_carets(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("사용법: rhwp dump-carets <파일> [--json] [-s <구역>] [-p <문단>]");
        return EXIT_USAGE;
    }
    let path = &args[0];
    let json_mode = args.iter().any(|a| a == "--json");
    let mut sec_filter: Option<usize> = None;
    let mut para_filter: Option<usize> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-s" | "--section" if i + 1 < args.len() => {
                sec_filter = args[i + 1].parse().ok();
                i += 2;
            }
            "-p" | "--para" if i + 1 < args.len() => {
                para_filter = args[i + 1].parse().ok();
                i += 2;
            }
            _ => i += 1,
        }
    }

    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("읽기 실패: {path} — {e}");
            return EXIT_RUNTIME;
        }
    };
    let structure = match rhwp::parser::parse_document(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("파싱 실패: {path} — {e}");
            return EXIT_RUNTIME;
        }
    };
    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => {
            let _ = &e;
            eprintln!("문서 로드 실패: {path}");
            return EXIT_RUNTIME;
        }
    };

    let mut rows: Vec<serde_json::Value> = Vec::new();
    for (si, sec) in structure.sections.iter().enumerate() {
        if sec_filter.is_some_and(|f| f != si) {
            continue;
        }
        for (pi, para) in sec.paragraphs.iter().enumerate() {
            if para_filter.is_some_and(|f| f != pi) {
                continue;
            }
            // 문단 끝까지(포함) 캐럿을 둔다 — 마지막은 문단 부호 앞자리다.
            let last = para.char_count as usize;
            for off in 0..=last {
                let Ok(raw) = doc.get_cursor_rect_native(si, pi, off) else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
                    continue;
                };
                rows.push(serde_json::json!({
                    "section": si,
                    "para": pi,
                    "offset": off,
                    "pageIndex": v.get("pageIndex"),
                    "x": v.get("x"),
                    "y": v.get("y"),
                    "height": v.get("height"),
                }));
            }
        }
    }

    if json_mode {
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "file": path,
            "count": rows.len(),
            "carets": rows,
        });
        println!("{}", provenance::marked(envelope, "dump-carets"));
        return EXIT_OK;
    }
    for r in &rows {
        println!(
            "s{}p{} off{:>3}: page {} x={:>7} y={:>7} h={}",
            r["section"], r["para"], r["offset"], r["pageIndex"], r["x"], r["y"], r["height"]
        );
    }
    println!("\n=== 캐럿 {} 개 ===", rows.len());
    EXIT_OK
}

fn ir_sweep(args: &[String]) -> i32 {
    use rhwp::diagnostics::ir_field_sweep::{sweep_documents, tally};

    if args.len() < 2 {
        eprintln!("사용법: rhwp ir-sweep <파일A> <파일B> [--json] [--max-lines <N>]");
        return EXIT_USAGE;
    }
    let (file_a, file_b) = (&args[0], &args[1]);
    let mut json_mode = false;
    let mut max_lines: Option<usize> = None;
    let is_value = |idx: usize| idx < args.len() && !args[idx].starts_with('-');
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_mode = true;
                i += 1;
            }
            "--max-lines" if is_value(i + 1) => {
                max_lines = args[i + 1].parse().ok();
                i += 2;
            }
            other => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
        }
    }

    let mut load = |path: &String| match std::fs::read(path) {
        Ok(bytes) => match rhwp::parser::parse_document(&bytes) {
            Ok(doc) => Some(doc),
            Err(e) => {
                eprintln!("파싱 실패: {path} — {e}");
                None
            }
        },
        Err(e) => {
            eprintln!("읽기 실패: {path} — {e}");
            None
        }
    };
    let (Some(doc_a), Some(doc_b)) = (load(file_a), load(file_b)) else {
        return EXIT_RUNTIME;
    };

    let report = match sweep_documents(&doc_a, &doc_b) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("전수 비교 실패: {e}");
            return EXIT_RUNTIME;
        }
    };
    // `examples()` 는 진단용 표본이라 상한이 있다 — 건수는 반드시 `total()` 을 쓴다.
    let total = report.total();
    let examples = report.examples();
    if json_mode {
        let rows: Vec<serde_json::Value> = examples
            .iter()
            .take(max_lines.unwrap_or(usize::MAX))
            .map(|d| serde_json::json!({ "path": d.path, "left": d.left, "right": d.right }))
            .collect();
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "a": file_a,
            "b": file_b,
            "identical": report.is_empty(),
            "diffCount": total,
            "truncated": rows.len() < total,
            "categories": tally(&report),
            "divergences": rows,
        });
        println!("{}", provenance::marked(envelope, "ir-sweep"));
        // `ir-diff` 와 같은 규약 — 차이가 있으면 3.
        return if report.is_empty() { EXIT_OK } else { 3 };
    }

    for d in examples.iter().take(max_lines.unwrap_or(200)) {
        println!("{} : {} → {}", d.path, d.left, d.right);
    }
    println!("\n=== 전수 비교 완료: 차이 {total} 건 ===");
    EXIT_OK
}

fn ir_diff(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("사용법: rhwp ir-diff <파일A> <파일B> [-s <구역>] [-p <문단>] [--summary] [--max-lines <N>] [--json]");
        // [#3274] 인자 부족은 사용법 오류다 — 종전엔 0 으로 끝나 스크립트가 감지 못했다.
        return EXIT_USAGE;
    }

    let file_a = &args[0];
    let file_b = &args[1];
    let mut section_filter: Option<usize> = None;
    let mut para_filter: Option<usize> = None;
    // [Task #653 보강] 출력 가드 옵션
    let mut summary_mode = false;
    let mut max_lines: Option<usize> = None;
    // [#3274] --json: 계약 봉투 한 줄(카테고리 요약 포함), 차이 발견 시 exit 3.
    let mut json_mode = false;

    // [#3274] 값을 받는 옵션은 다음 토큰이 플래그(`-` 시작)면 값으로 삼키지 않는다.
    // 종전엔 `--max-lines --json` 처럼 값을 빠뜨리면 "--json" 이 값으로 소비돼
    // json 모드가 조용히 꺼지고, 게이트를 기대한 스크립트가 차이를 통과로 오판했다.
    // (-s/-p/--max-lines 는 모두 비음수만 받으므로 `-` 로 시작하는 값은 없다.)
    let is_value = |idx: usize| idx < args.len() && !args[idx].starts_with('-');
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-s" | "--section" if is_value(i + 1) => {
                section_filter = args[i + 1].parse().ok();
                i += 2;
            }
            "-p" | "--para" if is_value(i + 1) => {
                para_filter = args[i + 1].parse().ok();
                i += 2;
            }
            "--summary" => {
                summary_mode = true;
                i += 1;
            }
            "--max-lines" if is_value(i + 1) => {
                max_lines = args[i + 1].parse().ok();
                i += 2;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    // [#3274] 읽기·파싱 실패는 exit 1 (#2707 정렬) — 종전엔 0 으로 끝나
    // "비교했고 차이 없음"과 "비교 자체를 못 함"을 구별할 수 없었다.
    let data_a = match fs::read(file_a) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: {} 읽기 실패: {}", file_a, e);
            return EXIT_RUNTIME;
        }
    };
    let data_b = match fs::read(file_b) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: {} 읽기 실패: {}", file_b, e);
            return EXIT_RUNTIME;
        }
    };

    // 일반 열기·내보내기 명령과 동일하게 전역 --password/--password-stdin을
    // 적용한다. 종전에는 ir-diff만 parse_document를 직접 호출해, 암호 문서가
    // 비교 대상이면 복호화 지원이 있어도 EncryptedDocument로 즉시 종료했다.
    // 비암호 문서는 parse_document_with_password가 비밀번호를 무시하므로, 암호/
    // 평문 counterpart 비교에도 하나의 입력 경로를 사용할 수 있다.
    let password = cli_password();
    let parse_for_ir_diff = |data: &[u8]| match password.as_deref() {
        Some(password) => rhwp::parser::parse_document_with_password(data, password.as_bytes()),
        None => rhwp::parser::parse_document(data),
    };

    let doc_a = match parse_for_ir_diff(&data_a) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: {} 파싱 실패", file_a);
            return classify_hwp_error(&e.to_string()).report();
        }
    };
    let doc_b = match parse_for_ir_diff(&data_b) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: {} 파싱 실패", file_b);
            return classify_hwp_error(&e.to_string()).report();
        }
    };

    let name_a = Path::new(file_a)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let name_b = Path::new(file_b)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    if !summary_mode && !json_mode {
        println!("=== IR 비교: {} vs {} ===", name_a, name_b);
    }

    // [Task #653 보강] 출력 가드 상태 — IrDiffEmitter 로 통합 (#2122)
    // [#3274] json 모드는 summary 와 같은 수집 전용 경로(버킷만 쌓고 무출력)를 탄다 —
    // stdout 순수성을 위해 텍스트 라인을 한 줄도 내면 안 된다.
    let mut em = IrDiffEmitter {
        summary_mode: summary_mode || json_mode,
        max_lines,
        printed_lines: 0,
        truncated: false,
        summary_buckets: std::collections::BTreeMap::new(),
    };

    let mut total_diffs = 0u32;

    // 구역 수 비교
    // [#3274] 종전엔 total_diffs 선언이 이 블록 뒤에 있어 구역 수 차이가 집계되지
    // 않았다. 텍스트 모드에선 차이 라인이 화면에 보여 무해했으나, --json 게이트에서는
    // 구역 하나가 덧붙은 변환본이 diffCount=0·identical:true·exit 0 으로 통과하는
    // 치명적 누락이었다(봉투 자기모순). 선언을 앞으로 올리고 여기서도 집계한다.
    if doc_a.sections.len() != doc_b.sections.len() {
        em.diff(format!(
            "구역 수: A={} vs B={}",
            doc_a.sections.len(),
            doc_b.sections.len()
        ));
        total_diffs += 1;
    }

    let sec_count = doc_a.sections.len().min(doc_b.sections.len());

    for sec_idx in 0..sec_count {
        if let Some(sf) = section_filter {
            if sec_idx != sf {
                continue;
            }
        }

        let sec_a = &doc_a.sections[sec_idx];
        let sec_b = &doc_b.sections[sec_idx];

        if sec_a.paragraphs.len() != sec_b.paragraphs.len() {
            em.diff(format!(
                "구역 {}: 문단 수 A={} vs B={}",
                sec_idx,
                sec_a.paragraphs.len(),
                sec_b.paragraphs.len()
            ));
            total_diffs += 1;
        }

        let para_count = sec_a.paragraphs.len().min(sec_b.paragraphs.len());
        for pi in 0..para_count {
            if let Some(pf) = para_filter {
                if pi != pf {
                    continue;
                }
            }

            let pa = &sec_a.paragraphs[pi];
            let pb = &sec_b.paragraphs[pi];
            let diffs = ir_diff_paragraph_fields(pa, pb, &doc_a, &doc_b);

            if !diffs.is_empty() {
                let text_preview: String = pa.text.chars().take(30).collect();
                em.header(format!(
                    "\n--- 문단 {}.{} --- \"{}\"",
                    sec_idx, pi, text_preview
                ));
                for d in &diffs {
                    em.diff(format!("{}", d));
                }
                total_diffs += diffs.len() as u32;
            }
        }
    }

    // doc_info 비교: ParaShape
    {
        let ps_a = &doc_a.doc_info.para_shapes;
        let ps_b = &doc_b.doc_info.para_shapes;
        if ps_a.len() != ps_b.len() {
            em.diff(format!(
                "ParaShape 수: A={} vs B={}",
                ps_a.len(),
                ps_b.len()
            ));
            total_diffs += 1;
        }
        let ps_count = ps_a.len().min(ps_b.len());
        for i in 0..ps_count {
            let a = &ps_a[i];
            let b = &ps_b[i];
            let mut ps_diffs: Vec<String> = Vec::new();
            if a.margin_left != b.margin_left {
                ps_diffs.push(format!("ml: {}vs{}", a.margin_left, b.margin_left));
            }
            if a.margin_right != b.margin_right {
                ps_diffs.push(format!("mr: {}vs{}", a.margin_right, b.margin_right));
            }
            if a.indent != b.indent {
                ps_diffs.push(format!("indent: {}vs{}", a.indent, b.indent));
            }
            if a.tab_def_id != b.tab_def_id {
                ps_diffs.push(format!("tab_def: {}vs{}", a.tab_def_id, b.tab_def_id));
            }
            if a.spacing_before != b.spacing_before {
                ps_diffs.push(format!("sb: {}vs{}", a.spacing_before, b.spacing_before));
            }
            if a.spacing_after != b.spacing_after {
                ps_diffs.push(format!("sa: {}vs{}", a.spacing_after, b.spacing_after));
            }
            if a.line_spacing != b.line_spacing {
                ps_diffs.push(format!("ls: {}vs{}", a.line_spacing, b.line_spacing));
            }
            if !ps_diffs.is_empty() {
                em.diff(format!("PS[{}] {}", i, ps_diffs.join(", ")));
                total_diffs += ps_diffs.len() as u32;
            }
        }
    }

    // doc_info 비교: TabDef
    {
        let td_a = &doc_a.doc_info.tab_defs;
        let td_b = &doc_b.doc_info.tab_defs;
        if td_a.len() != td_b.len() {
            em.diff(format!("TabDef 수: A={} vs B={}", td_a.len(), td_b.len()));
            total_diffs += 1;
        }
        let td_count = td_a.len().min(td_b.len());
        for i in 0..td_count {
            let a = &td_a[i];
            let b = &td_b[i];
            if a.tabs.len() != b.tabs.len() {
                em.diff(format!(
                    "TD[{}] 탭 수: A={} vs B={}",
                    i,
                    a.tabs.len(),
                    b.tabs.len()
                ));
                total_diffs += 1;
            } else {
                for (ti, (ta, tb)) in a.tabs.iter().zip(b.tabs.iter()).enumerate() {
                    if ta.position != tb.position
                        || ta.tab_type != tb.tab_type
                        || ta.fill_type != tb.fill_type
                    {
                        em.diff(format!(
                            "TD[{}][{}] pos: {}vs{}, type: {}vs{}, fill: {}vs{}",
                            i,
                            ti,
                            ta.position,
                            tb.position,
                            ta.tab_type,
                            tb.tab_type,
                            ta.fill_type,
                            tb.fill_type
                        ));
                        total_diffs += 1;
                    }
                }
            }
        }
    }

    // [Task #653 보강] 요약 모드 출력 — 카테고리별 카운트 (내림차순 → 알파벳)
    // [#3274] --summary --json 병용 시 JSON 이 이긴다 — stdout 순수성 우선.
    if summary_mode && !json_mode {
        println!("=== 카테고리별 차이 요약 ===");
        let mut entries: Vec<(String, u32)> = em.summary_buckets.clone().into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (cat, count) in &entries {
            println!("  {:>5}건  {}", count, cat);
        }
    }

    if json_mode {
        // [#3274] 계약 봉투 한 줄 — 카테고리 버킷(BTreeMap)은 키 정렬이 결정적이다.
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "a": file_a,
            "b": file_b,
            "identical": total_diffs == 0,
            "diffCount": total_diffs,
            "categories": em.summary_buckets,
        });
        println!("{}", provenance::marked(envelope, "ir-diff"));
        // 차이 발견 = 3: #2707 의 "--verify IR 차이" 코드와 같은 의미의 게이트 신호.
        return if total_diffs == 0 { EXIT_OK } else { 3 };
    }

    println!("\n=== 비교 완료: 차이 {} 건 ===", total_diffs);
    EXIT_OK
}

/// `edit` 계열 산출 형식 (#3383).
///
/// 종전에는 세 하위 명령이 모두 `export_hwp_native()` 로 HWP5 를 강제 산출했다. 그래서
/// ① HWPX 입력이 조용히 `.hwp` 로 바뀌고(형식 미보존) ② 어댑터 없는 native 경로라
/// HWPX→HWP IR 매핑(#178)조차 타지 않아 산출물에서 차트·이미지가 유실됐다.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EditOutputFormat {
    Hwp,
    Hwpx,
}

impl EditOutputFormat {
    /// 기본 산출 파일의 확장자(점 제외).
    fn ext(self) -> &'static str {
        match self {
            EditOutputFormat::Hwp => "hwp",
            EditOutputFormat::Hwpx => "hwpx",
        }
    }

    /// JSON 봉투의 `outputFormat` 값. **`info --json` 의 `format` 과 같은 어휘**를 쓴다 —
    /// 확장자(`hwp`)가 아니라 형식 이름(`hwp5`)이라야 두 봉투를 그대로 대조할 수 있다.
    fn label(self) -> &'static str {
        match self {
            EditOutputFormat::Hwp => "hwp5",
            EditOutputFormat::Hwpx => "hwpx",
        }
    }
}


/// 같은 입력 경로를 다루는 rhwp writer 사이의 read-check-write 경계를 직렬화한다.
/// 잠금 파일은 rename 뒤에도 같은 inode/handle을 유지해야 하므로 원본 파일이 아니라
/// 정규화한 경로의 해시로 만든 안정적인 temp sidecar를 사용한다.
struct CasPathLock {
    _file: fs::File,
}

impl CasPathLock {
    fn acquire(source: &Path) -> std::io::Result<Self> {
        #[cfg(unix)]
        use std::os::unix::fs::OpenOptionsExt;

        let canonical = fs::canonicalize(source)?;
        let key = sha256_hex_of(canonical.to_string_lossy().as_bytes());
        let lock_path = std::env::temp_dir().join(format!("rhwp-cas-v1-{key}.lock"));
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(lock_path)?;
        file.lock()?;
        Ok(Self { _file: file })
    }
}


struct ReplayScratchDir(std::path::PathBuf);

impl Drop for ReplayScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}


/// [#3719 §6-6] `edit fill-fields`(단건)와 `batch fill`(메일머지)이 공유하는 채움 결과.
struct FillOutcome {
    /// `edit fill-fields --json` 봉투 그대로. 배치 레코드는 여기에 `row` 만 더한다 —
    /// 소비자가 단건과 배치를 같은 코드로 읽게 하기 위함(기존 batch 축 규약).
    envelope: serde_json::Value,
    /// 산출 경로. `--dry-run` 이면 **만들 예정** 경로다(디스크에 파일은 없다).
    output_path: String,
    /// [#3383] 산출 형식 — 입력 형식을 따른다.
    output_format: EditOutputFormat,
    /// `--verify` 판정이 "차이 있음"인가. 단건은 exit 3, 배치는 집계 대상.
    verify_failed: bool,
}

/// `-o` 도 `--in-place` 도 없이 원본을 덮어쓰려 할 때의 거부 메시지.
///
/// 마스킹은 되돌릴 수 없다. "실수로 원본을 잃는" 경로를 아예 만들지 않기 위해,
/// 산출 경로를 **명시하지 않으면 실행하지 않는다**(다른 edit 명령의 `_replaced` 류
/// 기본 이름조차 만들지 않는다 — 어디에 무엇이 생겼는지 모른 채로 두지 않기 위해).
const REDACT_DESTINATION_REQUIRED: &str = "오류: 마스킹은 되돌릴 수 없습니다. \
     산출 경로를 -o <출력> 으로 지정하거나, 원본을 덮어쓸 의도라면 --in-place 를 \
     명시하세요 (먼저 --dry-run 으로 무엇이 지워질지 확인하기를 권합니다).";

/// `\u{5}HwpSummaryInformation` 에서 지울 속성 — `(PID, 봉투 필드 이름)`.
///
/// PID 는 HWP5 사양의 `HWPPIDSI_*` 다. 본문과 무관한 작성자·이력 메타만 고른다.
const SUMMARY_TARGETS: [(u32, &str); 11] = [
    (0x02, "title"),
    (0x03, "subject"),
    (0x04, "author"),
    (0x05, "keywords"),
    (0x06, "comments"),
    (0x08, "lastSavedBy"),
    (0x09, "revisionNumber"),
    (0x0B, "lastPrintedAt"),
    (0x0C, "createdAt"),
    (0x0D, "lastSavedAt"),
    (0x14, "dateString"),
];

/// [#3603] `set-cell` 계열이 셀 값으로 거부하는 제어문자 안내문.
///
/// CLI(`edit set-cell`)와 세션 도구(`hwp_doc_set_cell`)가 **같은 문장**으로 거부해야 한다 —
/// 두 경로가 서로 다른 문장(또는 한쪽만 검사)을 내면 에이전트는 같은 제약을 두 번 배워야
/// 하고, 무엇보다 세션 경로만 통과시키면 한 셀 문단 안에 raw 개행이 박힌 문서가 만들어진다.
/// v1 셀 기록 계약은 '한 줄 값'이다.
const SET_CELL_CONTROL_CHAR_MESSAGE: &str =
    "오류: --text 에 줄바꿈·탭은 넣을 수 없습니다 (한 줄 값 기록).";

/// [#3603] 격자 주소(export-tables 좌표) → 모델 좌표 해석.
/// CLI(edit set-cell)와 세션 도구(hwp_doc_set_cell)가 공유한다 — 병합으로 덮인 칸은
/// 앵커 좌표를 안내하며 실패한다(보호 동작). 반환: (sec, para, ctrl, cell_idx,
/// 문단별 글자 수, 기존 텍스트).
enum CellResolveError {
    Usage(String),
    Runtime(String),
}


/// [#3719 §6-5] `edit insert-image` 가 받는 그림 형식.
///
/// BinData 로 넣을 수 있고 **원본 픽셀 크기를 헤더만 읽어 잴 수 있는** 형식만 담는다.
/// 크기를 못 재면 배율·배치 좌표가 의미를 잃으므로 삽입을 시작하지 않는다.
const INSERT_IMAGE_FORMATS: [&str; 6] = ["png", "jpg", "jpeg", "bmp", "tif", "tiff"];

/// 96dpi 픽셀 1개 = 75 HWPUNIT(7200/96). 코어가 crop 을 `px * 75` 로 잡는 것과 같은 환산비다.
const HWPUNIT_PER_PX: u32 = 75;


#[cfg(test)]
mod tests {
    use super::{
        allows_implicit_sibling_resources, cli_output_password, cli_password,
        collect_audit_capsules, replay_scratch_dir, set_cli_output_password, set_cli_password,
        strip_global_auth_options, tab_ext_semantic_differs, with_replay_input_snapshot,
        EXIT_USAGE,
    };
    use rhwp::parser::FileFormat;

    #[test]
    fn hml_does_not_implicitly_load_sibling_resources() {
        assert!(!allows_implicit_sibling_resources(FileFormat::Hml));
        assert!(allows_implicit_sibling_resources(FileFormat::Hwp));
        assert!(allows_implicit_sibling_resources(FileFormat::Hwpx));
    }

    #[test]
    fn replay_engine_receives_the_hashed_input_snapshot() {
        let original =
            std::env::temp_dir().join(format!("rhwp-replay-original-{}.hwp", std::process::id()));
        std::fs::write(&original, b"original bytes").expect("원본 작성");
        let mut plan = serde_json::json!({ "input": original.to_string_lossy() });
        let scratch = replay_scratch_dir("unit").expect("전용 임시 폴더");
        let scratch_path = scratch.0.clone();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&scratch_path)
                    .expect("전용 임시 폴더 metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        let seen = with_replay_input_snapshot(
            &mut plan,
            b"hashed snapshot",
            &scratch.0,
            |snapshot_plan| {
                std::fs::write(&original, b"changed after hashing").expect("원본 교체");
                let snapshot_path = snapshot_plan["input"].as_str().expect("스냅샷 경로");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    assert_eq!(
                        std::fs::metadata(snapshot_path)
                            .expect("입력 스냅샷 metadata")
                            .permissions()
                            .mode()
                            & 0o777,
                        0o600
                    );
                }
                std::fs::read(snapshot_path).expect("스냅샷 읽기")
            },
        )
        .expect("스냅샷 실행");
        assert_eq!(seen, b"hashed snapshot");
        assert_eq!(plan["input"], original.to_string_lossy().as_ref());
        drop(scratch);
        assert!(!scratch_path.exists(), "전용 임시 폴더는 RAII 정리");
        let _ = std::fs::remove_file(original);
    }

    #[test]
    fn audit_directory_entry_errors_are_not_silently_dropped() {
        let entries: [std::io::Result<std::path::PathBuf>; 1] = [Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ))];
        let error = collect_audit_capsules(entries).expect_err("항목 오류는 fail-closed");
        assert!(error.contains("폴더 항목 읽기 실패"));
    }

    #[test]
    fn tab_ext_reserved_fields_ignored() {
        // 같은 문서의 HWPX(파서가 [1],[3..6]=0) vs HWP5([1]=leader/fill 슬롯, [3..6]=원본 바이트).
        // 이 포맷 비대칭 슬롯들은 모두 무시 → 의미 차이 없음.
        let hwpx = [1640, 0, 256, 0, 0, 0, 9];
        let hwp5 = [1640, 5, 256, 32, 32, 32, 9];
        assert!(!tab_ext_semantic_differs(&hwpx, &hwp5));
    }

    #[test]
    fn tab_ext_semantic_fields_detected() {
        let base = [1640, 0, 256, 0, 0, 0, 9];
        assert!(!tab_ext_semantic_differs(&base, &base));
        // width([0]) 차이 검출
        assert!(tab_ext_semantic_differs(&base, &[1641, 0, 256, 0, 0, 0, 9]));
        // type([2] high byte) 차이 검출 — 256(0x0100)→512(0x0200)
        assert!(tab_ext_semantic_differs(&base, &[1640, 0, 512, 0, 0, 0, 9]));
        // leader([2] low byte, 두 포맷 공통) 차이 검출 — 256(0x0100)→257(0x0101)
        assert!(tab_ext_semantic_differs(&base, &[1640, 0, 257, 0, 0, 0, 9]));
        // HWP5 leader/fill 슬롯([1], HWPX는 항상 0)은 포맷 비대칭이라 무시 — 차이로 치지 않음
        assert!(!tab_ext_semantic_differs(
            &base,
            &[1640, 1, 256, 0, 0, 0, 9]
        ));
        // marker([6]) 차이 검출
        assert!(tab_ext_semantic_differs(&base, &[1640, 0, 256, 0, 0, 0, 0]));
    }

    #[test]
    fn global_password_option_is_removed_from_any_position() {
        let args = vec![
            "rhwp".to_string(),
            "info".to_string(),
            "sample.hwp".to_string(),
            "--password".to_string(),
            "secret".to_string(),
        ];
        set_cli_password(None);
        let clean = strip_global_auth_options(args).unwrap();
        assert_eq!(clean, ["rhwp", "info", "sample.hwp"]);
        // 비밀번호는 반환값이 아니라 CLI_PASSWORD(thread_local)로 전달된다.
        assert_eq!(cli_password().as_deref(), Some("secret"));
        set_cli_password(None);
    }

    #[test]
    fn duplicate_global_password_options_are_rejected() {
        let args = vec![
            "rhwp".to_string(),
            "--password".to_string(),
            "first".to_string(),
            "info".to_string(),
            "sample.hwp".to_string(),
            "--password".to_string(),
            "second".to_string(),
        ];
        assert!(matches!(
            strip_global_auth_options(args),
            Err(code) if code == EXIT_USAGE
        ));
    }

    #[test]
    fn global_output_password_is_removed_without_leaking_into_command_args() {
        let args = vec![
            "rhwp".to_string(),
            "convert".to_string(),
            "source.hwp".to_string(),
            "output.hwp".to_string(),
            "--output-password".to_string(),
            "protected".to_string(),
        ];
        set_cli_password(None);
        set_cli_output_password(None);
        let clean = strip_global_auth_options(args).unwrap();
        assert_eq!(clean, ["rhwp", "convert", "source.hwp", "output.hwp"]);
        assert_eq!(cli_output_password().as_deref(), Some("protected"));
        set_cli_output_password(None);
    }

    #[test]
    fn duplicate_global_output_password_options_are_rejected() {
        let args = vec![
            "rhwp".to_string(),
            "--output-password".to_string(),
            "first".to_string(),
            "convert".to_string(),
            "source.hwp".to_string(),
            "output.hwp".to_string(),
            "--output-password".to_string(),
            "second".to_string(),
        ];
        assert!(matches!(
            strip_global_auth_options(args),
            Err(code) if code == EXIT_USAGE
        ));
    }
}
