//! `edit insert-column-break` / `edit split-paragraph-in-hf` /
//! `edit merge-paragraph-in-hf` 계약 회귀 테스트 (upstream #5185/#5192 계열
//! 선별 이식 — Tier 2 라운드, "나머지" 배치).
//!
//! 코어 로직은 기존 네이티브 함수(`insert_column_break_native`,
//! `split_paragraph_in_header_footer_native`,
//! `merge_paragraph_in_header_footer_native`)를 그대로 재사용하고 CLI 배선만
//! 신규다. 이 테스트는 그 배선 — 인자 파싱, `--dry-run`, `--verify`(저장본
//! 재파싱 IR 대조), 종료 코드(#2707 계약) — 을 검증한다.
//!
//! 파일명은 착수 당시 "나머지" 배치 4개(`insert-column-break`/
//! `split-paragraph-in-hf`/`merge-paragraph-in-hf`/`set-numbering-restart`)를
//! 다룰 예정으로 지었으나, `set-numbering-restart`는 이 파일에서 다루지 않는다
//! — 순수 CLI 배선이 아니라 코어 재작성이 먼저 필요해 별도 계약 테스트
//! `tests/edit_numbering_restart_contract.rs`로 분리했다(코어 `set_numbering_restart_native`가
//! 원래 세팅하던 `Paragraph.numbering_restart` 필드는 어느 직렬화기에도 연결돼
//! 있지 않은 세션 전용 렌더 힌트였다 — `toggle-hide-hf`와 같은 성격. 지금은
//! `ParaShape.numbering_id`가 가리키는 `Numbering` 테이블 자체를 갈아 끼우는
//! 방식으로 영속화한다). 상세는
//! `mydocs/manual/cli_commands.md`의 "나머지 4종" 절과
//! `mydocs/report/upstream_devel_sync_candidates_20260901.md` §8 참고.
//!
//! 핵심 비대칭(직전 배치들과 마찬가지로 명령마다 다르다):
//! - `insert-column-break`는 직전 배치의 `insert-page-break`와 완전히 같은
//!   패턴 — `--section`/`--para` 범위를 `--dry-run`에서도 무조건 검사한다
//!   (exit 2), `--offset`은 검사하지 않는다.
//! - `split-paragraph-in-hf`/`merge-paragraph-in-hf`는 네이티브 호출 자체가
//!   `--dry-run`에서 생략되므로 좌표 범위를 미리 검사하지 않는다.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::model::control::Control;
use rhwp::model::paragraph::ColumnBreakType;
use rhwp::wasm_api::HwpDocument;

/// 구역0에 머리말 컨트롤 1개(문단 1개, 꼬리말 없음).
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
    let dir = std::env::temp_dir().join(format!("rhwp-edit-colbreakhf-{}", std::process::id()));
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

fn para_column_type(doc: &HwpDocument, section: usize, para: usize) -> ColumnBreakType {
    doc.document().sections[section].paragraphs[para].column_type
}

fn header_para_count(doc: &HwpDocument, section: usize) -> usize {
    doc.document().sections[section]
        .paragraphs
        .iter()
        .flat_map(|p| &p.controls)
        .find_map(|c| match c {
            Control::Header(h) => Some(h.paragraphs.len()),
            _ => None,
        })
        .expect("머리말 컨트롤을 찾지 못함")
}

// ---------------------------------------------------------------------------
// insert-column-break
// ---------------------------------------------------------------------------

#[test]
fn insert_column_break_rejects_out_of_range_section_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-column-break",
        &file,
        "--section",
        "999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_column_break_rejects_out_of_range_para_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-column-break",
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
fn insert_column_break_dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE));
    let out_path = tmp_dir().join("colbreak_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "insert-column-break",
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
    assert_eq!(env["changedPages"], serde_json::Value::Null, "{env}");
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn insert_column_break_splits_paragraph_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("colbreak.hwpx");
    let out_str = path_str(&out_path);
    let before_para_count = load(&source_path).document().sections[0].paragraphs.len();

    let args = [
        "edit",
        "insert-column-break",
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
        ColumnBreakType::Column,
        "분할로 새로 생긴 문단에 단 나누기가 설정되어야 한다"
    );
}

// ---------------------------------------------------------------------------
// split-paragraph-in-hf / merge-paragraph-in-hf (왕복)
// ---------------------------------------------------------------------------

#[test]
fn split_paragraph_in_hf_rejects_missing_header_or_footer() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "split-paragraph-in-hf", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn split_paragraph_in_hf_rejects_both_header_and_footer() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "split-paragraph-in-hf",
        &file,
        "--header",
        "--footer",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn split_paragraph_in_hf_dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE));
    let out_path = tmp_dir().join("hfsplit_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "split-paragraph-in-hf",
        &file,
        "--header",
        "--section",
        "0",
        "--para",
        "0",
        "--offset",
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
fn merge_paragraph_in_hf_rejects_missing_header_or_footer() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "merge-paragraph-in-hf", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn split_then_merge_paragraph_in_hf_roundtrips_back_to_one_paragraph() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let split_path = tmp_dir().join("hfsplit.hwpx");
    let split_str = path_str(&split_path);
    let before = header_para_count(&load(&source_path), 0);
    assert_eq!(before, 1, "표본 머리말은 문단 1개여야 한다");

    let split_args = [
        "edit",
        "split-paragraph-in-hf",
        &file,
        "--header",
        "--section",
        "0",
        "--para",
        "0",
        "--offset",
        "0",
        "-o",
        &split_str,
        "--verify",
        "--json",
    ];
    let split_out = run(&split_args);
    assert_eq!(
        split_out.status.code(),
        Some(0),
        "{}",
        describe(&split_args, &split_out)
    );
    let split_env = json_of(&split_out);
    assert_eq!(split_env["verify"]["identical"], true, "{split_env}");

    let after_split = header_para_count(&load(&split_path), 0);
    assert_eq!(after_split, 2, "분할 후 머리말 문단이 2개가 되어야 한다");

    let merge_path = tmp_dir().join("hfmerge.hwpx");
    let merge_str = path_str(&merge_path);
    let merge_args = [
        "edit",
        "merge-paragraph-in-hf",
        &split_str,
        "--header",
        "--section",
        "0",
        "--para",
        "1",
        "-o",
        &merge_str,
        "--verify",
        "--json",
    ];
    let merge_out = run(&merge_args);
    assert_eq!(
        merge_out.status.code(),
        Some(0),
        "{}",
        describe(&merge_args, &merge_out)
    );
    let merge_env = json_of(&merge_out);
    assert_eq!(merge_env["verify"]["identical"], true, "{merge_env}");

    let after_merge = header_para_count(&load(&merge_path), 0);
    assert_eq!(
        after_merge, before,
        "병합 후 머리말 문단 수가 원래대로 돌아와야 한다"
    );
}

#[test]
fn merge_paragraph_in_hf_rejects_first_paragraph_at_runtime() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "merge-paragraph-in-hf",
        &file,
        "--header",
        "--section",
        "0",
        "--para",
        "0",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}
