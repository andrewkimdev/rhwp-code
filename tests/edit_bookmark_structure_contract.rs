//! `edit add-bookmark` / `edit delete-bookmark` / `edit rename-bookmark` /
//! `edit delete-table` / `edit insert-page-break` 계약 회귀 테스트 (upstream
//! #5185/#5192 계열 선별 이식 — Tier 2 라운드, "책갈피/구조" 배치).
//!
//! 코어 로직은 기존 네이티브 함수(`add_bookmark_native`, `delete_bookmark_native`,
//! `rename_bookmark_native`, `delete_table_control_native`,
//! `insert_page_break_native`)를 그대로 재사용하고 CLI 배선만 신규다. 이 테스트는
//! 그 배선 — 인자 파싱, `--dry-run`, `--verify`(저장본 재파싱 IR 대조), 종료 코드
//! (#2707 계약) — 을 검증한다.
//!
//! 핵심 비대칭(직전 배치들과 마찬가지로 명령마다 다르다):
//! - 책갈피 3종(`add`/`delete`/`rename`)은 네이티브 함수가 성공/실패를 `Err`가 아니라
//!   반환 JSON의 `{"ok":true}`/`{"ok":false,"error":...}`로 알린다(이름 비어있음·
//!   중복·컨트롤이 책갈피 아님 등은 `ok:false`, 즉 exit 1) — **`--dry-run`에서는
//!   네이티브를 호출하지 않으므로 구역/문단/컨트롤 범위를 미리 검사하지 않는다**.
//! - `delete-table`은 `resolve_table_index`가 dry-run 여부와 무관하게 항상 실행되어
//!   표 번호 범위 초과가 `--dry-run`에서도 잡힌다(exit 1, usage 아님 — 표 인덱스는
//!   "잘못된 옵션"이 아니라 "존재하지 않는 대상"이므로).
//! - `insert-page-break`는 upstream 원본대로 `--section`/`--para` 범위를 인자
//!   파싱 직후, 네이티브 호출과 무관하게 무조건 검사한다(`--dry-run`에서도 exit 2) —
//!   단 `--offset`은 문단 길이 대비 사전 검사하지 않는다(upstream도 안 함).
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::model::control::Control;
use rhwp::model::paragraph::ColumnBreakType;
use rhwp::wasm_api::HwpDocument;

/// 구역0에 최상위 표 1개(표0), 구역0 문단4 컨트롤0에 기존 책갈피("참조")가 있다.
const SAMPLE: &str = "samples/hwpx/143E433F503322BD33.hwpx";

fn sample(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn rhwp_bin() -> String {
    std::env::var("CARGO_BIN_EXE_rhwp").unwrap_or_else(|_| env!("CARGO_BIN_EXE_rhwp").to_string())
}

fn run(args: &[&str]) -> Output {
    Command::new(rhwp_bin())
        .args(args)
        .output()
        .expect("rhwp 실행 실패")
}

fn tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rhwp-edit-bmstruct-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn describe(args: &[&str], output: &Output) -> String {
    format!(
        "명령: rhwp {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn json_of(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout JSON 파싱 실패: {e}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn path_str(p: &Path) -> String {
    p.to_str().expect("경로").to_string()
}

fn load(path: &Path) -> HwpDocument {
    let bytes = std::fs::read(path).expect("샘플 읽기");
    HwpDocument::from_bytes(&bytes).expect("샘플 파싱")
}

fn bookmark_names(doc: &HwpDocument) -> Vec<String> {
    doc.document()
        .sections
        .iter()
        .flat_map(|s| s.paragraphs.iter())
        .flat_map(|p| p.controls.iter())
        .filter_map(|c| match c {
            Control::Bookmark(b) => Some(b.name.clone()),
            _ => None,
        })
        .collect()
}

fn table_count(doc: &HwpDocument) -> usize {
    doc.document()
        .sections
        .iter()
        .flat_map(|s| s.paragraphs.iter())
        .flat_map(|p| p.controls.iter())
        .filter(|c| matches!(c, Control::Table(_)))
        .count()
}

fn para_column_type(doc: &HwpDocument, section: usize, para: usize) -> ColumnBreakType {
    doc.document().sections[section].paragraphs[para].column_type
}

// ---------------------------------------------------------------------------
// add-bookmark
// ---------------------------------------------------------------------------

#[test]
fn add_bookmark_rejects_missing_name() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "add-bookmark", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn add_bookmark_rejects_empty_name() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "add-bookmark", &file, "--name", "", "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn add_bookmark_dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE));
    let out_path = tmp_dir().join("addbm_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "add-bookmark",
        &file,
        "--name",
        "새책갈피",
        "--section",
        "0",
        "--para",
        "1",
        "-o",
        &out_str,
        "--dry-run",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));

    let env = json_of(&out);
    assert_eq!(env["dryRun"], true, "{env}");
    assert_eq!(env["changedPages"], serde_json::Value::Null, "{env}");
    assert!(
        env.get("output").is_none(),
        "dry-run은 output을 내면 안 된다: {env}"
    );
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn add_bookmark_adds_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("addbm.hwpx");
    let out_str = path_str(&out_path);
    let before = bookmark_names(&load(&source_path));

    let args = [
        "edit",
        "add-bookmark",
        &file,
        "--name",
        "새책갈피",
        "--section",
        "0",
        "--para",
        "1",
        "--offset",
        "0",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = bookmark_names(&load(&out_path));
    assert_eq!(after.len(), before.len() + 1, "책갈피가 하나 늘어야 한다");
    assert!(
        after.contains(&"새책갈피".to_string()),
        "새 책갈피 이름이 없다: {after:?}"
    );
}

#[test]
fn add_bookmark_rejects_duplicate_name_at_runtime() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "add-bookmark",
        &file,
        "--name",
        "참조",
        "--section",
        "0",
        "--para",
        "1",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

// ---------------------------------------------------------------------------
// delete-bookmark
// ---------------------------------------------------------------------------

#[test]
fn delete_bookmark_rejects_missing_args() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "delete-bookmark",
        &file,
        "--section",
        "0",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn delete_bookmark_dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE));
    let out_path = tmp_dir().join("delbm_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "delete-bookmark",
        &file,
        "--section",
        "0",
        "--para",
        "4",
        "--ctrl",
        "0",
        "-o",
        &out_str,
        "--dry-run",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));

    let env = json_of(&out);
    assert_eq!(env["dryRun"], true, "{env}");
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn delete_bookmark_removes_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("delbm.hwpx");
    let out_str = path_str(&out_path);
    let before = bookmark_names(&load(&source_path));
    assert!(
        before.contains(&"참조".to_string()),
        "표본에 '참조' 책갈피가 있어야 한다"
    );

    let args = [
        "edit",
        "delete-bookmark",
        &file,
        "--section",
        "0",
        "--para",
        "4",
        "--ctrl",
        "0",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = bookmark_names(&load(&out_path));
    assert_eq!(after.len(), before.len() - 1, "책갈피가 하나 줄어야 한다");
    assert!(
        !after.contains(&"참조".to_string()),
        "삭제한 책갈피 이름이 남아 있다: {after:?}"
    );
}

#[test]
fn delete_bookmark_rejects_non_bookmark_ctrl_at_runtime() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "delete-bookmark",
        &file,
        "--section",
        "0",
        "--para",
        "0",
        "--ctrl",
        "0",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

// ---------------------------------------------------------------------------
// rename-bookmark
// ---------------------------------------------------------------------------

#[test]
fn rename_bookmark_rejects_missing_name() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "rename-bookmark",
        &file,
        "--section",
        "0",
        "--para",
        "4",
        "--ctrl",
        "0",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn rename_bookmark_rejects_empty_name() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "rename-bookmark",
        &file,
        "--section",
        "0",
        "--para",
        "4",
        "--ctrl",
        "0",
        "--name",
        "",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn rename_bookmark_dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE));
    let out_path = tmp_dir().join("renbm_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "rename-bookmark",
        &file,
        "--section",
        "0",
        "--para",
        "4",
        "--ctrl",
        "0",
        "--name",
        "참조_변경",
        "-o",
        &out_str,
        "--dry-run",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));

    let env = json_of(&out);
    assert_eq!(env["dryRun"], true, "{env}");
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn rename_bookmark_renames_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("renbm.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "rename-bookmark",
        &file,
        "--section",
        "0",
        "--para",
        "4",
        "--ctrl",
        "0",
        "--name",
        "참조_변경",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    // ir-diff는 책갈피 이름 변경을 diff 카테고리로 잡지 않는다(기존 도구 한계, 이번
    // 배치가 만든 문제가 아님) — 저장본을 직접 재파싱해 이름을 확인해야 한다.
    let after = bookmark_names(&load(&out_path));
    assert!(
        after.contains(&"참조_변경".to_string()),
        "새 이름이 없다: {after:?}"
    );
    assert!(
        !after.contains(&"참조".to_string()),
        "옛 이름이 남아 있다: {after:?}"
    );
}

#[test]
fn rename_bookmark_rejects_duplicate_name_at_runtime() {
    // 두 번째 책갈피를 먼저 추가한 뒤, 기존 '참조' 책갈피를 그 이름으로 바꾸려 하면
    // 거부되어야 한다(자기 자신 제외 중복 검사).
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let mid_path = tmp_dir().join("renbm_dup_mid.hwpx");
    let mid_str = path_str(&mid_path);

    let add_args = [
        "edit",
        "add-bookmark",
        &file,
        "--name",
        "중복대상",
        "--section",
        "0",
        "--para",
        "1",
        "-o",
        &mid_str,
        "--json",
    ];
    let add_out = run(&add_args);
    assert_eq!(
        add_out.status.code(),
        Some(0),
        "{}",
        describe(&add_args, &add_out)
    );

    // '참조'는 문단4에 그대로 있다 — 추가는 문단1에서 일어나 인덱스가 겹치지 않는다.
    let rename_args = [
        "edit",
        "rename-bookmark",
        &mid_str,
        "--section",
        "0",
        "--para",
        "4",
        "--ctrl",
        "0",
        "--name",
        "중복대상",
        "--json",
    ];
    let rename_out = run(&rename_args);
    assert_eq!(
        rename_out.status.code(),
        Some(1),
        "{}",
        describe(&rename_args, &rename_out)
    );
}

// ---------------------------------------------------------------------------
// delete-table
// ---------------------------------------------------------------------------

#[test]
fn delete_table_rejects_missing_table_arg() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "delete-table", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn delete_table_rejects_out_of_range_table_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "delete-table", &file, "--table", "999", "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

#[test]
fn delete_table_dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE));
    let out_path = tmp_dir().join("deltable_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "delete-table",
        &file,
        "--table",
        "0",
        "-o",
        &out_str,
        "--dry-run",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));

    let env = json_of(&out);
    assert_eq!(env["dryRun"], true, "{env}");
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn delete_table_removes_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("deltable.hwpx");
    let out_str = path_str(&out_path);
    let before = table_count(&load(&source_path));
    assert!(before >= 1, "표본에 표가 있어야 한다");

    let args = [
        "edit",
        "delete-table",
        &file,
        "--table",
        "0",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = table_count(&load(&out_path));
    assert_eq!(after, before - 1, "표가 하나 줄어야 한다");
}

// ---------------------------------------------------------------------------
// insert-page-break
// ---------------------------------------------------------------------------

#[test]
fn insert_page_break_rejects_out_of_range_section_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-page-break",
        &file,
        "--section",
        "999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_page_break_rejects_out_of_range_para_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-page-break",
        &file,
        "--section",
        "0",
        "--para",
        "999999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_page_break_dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE));
    let out_path = tmp_dir().join("pagebreak_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "insert-page-break",
        &file,
        "--section",
        "0",
        "--para",
        "1",
        "-o",
        &out_str,
        "--dry-run",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));

    let env = json_of(&out);
    assert_eq!(env["dryRun"], true, "{env}");
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn insert_page_break_splits_paragraph_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("pagebreak.hwpx");
    let out_str = path_str(&out_path);
    let before_para_count = load(&source_path).document().sections[0].paragraphs.len();

    let args = [
        "edit",
        "insert-page-break",
        &file,
        "--section",
        "0",
        "--para",
        "1",
        "--offset",
        "0",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after_doc = load(&out_path);
    let after_para_count = after_doc.document().sections[0].paragraphs.len();
    assert_eq!(
        after_para_count,
        before_para_count + 1,
        "문단이 분할되어 하나 늘어야 한다"
    );
    assert_eq!(
        para_column_type(&after_doc, 0, 2),
        ColumnBreakType::Page,
        "분할로 새로 생긴 문단에 쪽 나누기가 설정되어야 한다"
    );
}
