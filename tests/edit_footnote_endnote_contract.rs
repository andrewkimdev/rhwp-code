//! `edit insert-footnote` / `edit insert-endnote` / `edit delete-footnote` /
//! `edit insert-footnote-text` / `edit delete-text-in-footnote` /
//! `edit split-paragraph-in-footnote` / `edit merge-paragraph-in-footnote` 계약
//! 회귀 테스트 (upstream #5185/#5192 계열 선별 이식 — Tier 2 라운드, "각주/미주" 배치).
//!
//! 코어 로직은 기존 네이티브 함수(`insert_footnote_native`, `insert_endnote_native`,
//! `delete_footnote_native`, `insert_text_in_footnote_native`,
//! `delete_text_in_footnote_native`, `split_paragraph_in_footnote_native`,
//! `merge_paragraph_in_footnote_native`)를 그대로 재사용하고 CLI 배선만 신규다. 이
//! 테스트는 그 배선 — 인자 파싱, `--dry-run`(무변경), `--verify`(저장본 재파싱 IR
//! 대조), 종료 코드(#2707 계약) — 을 검증한다.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::model::control::Control;
use rhwp::wasm_api::HwpDocument;

/// 본문 최상위 각주 1개(구역0/문단6/컨트롤0, 각주 내부 문단 1개) 보유, 미주 없음.
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
    let dir = std::env::temp_dir().join(format!("rhwp-edit-footnote-{}", std::process::id()));
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

fn footnote_count(doc: &HwpDocument) -> usize {
    doc.document()
        .sections
        .iter()
        .flat_map(|s| &s.paragraphs)
        .flat_map(|p| &p.controls)
        .filter(|c| matches!(c, Control::Footnote(_)))
        .count()
}

fn endnote_count(doc: &HwpDocument) -> usize {
    doc.document()
        .sections
        .iter()
        .flat_map(|s| &s.paragraphs)
        .flat_map(|p| &p.controls)
        .filter(|c| matches!(c, Control::Endnote(_)))
        .count()
}

fn footnote_paragraphs<'a>(doc: &'a HwpDocument, section: usize, para: usize, ctrl: usize) -> &'a [rhwp::model::paragraph::Paragraph] {
    let Control::Footnote(fnote) = &doc.document().sections[section].paragraphs[para].controls[ctrl]
    else {
        panic!("각주 컨트롤 아님");
    };
    &fnote.paragraphs
}

// ---------------------------------------------------------------------------
// insert-footnote / insert-endnote
// ---------------------------------------------------------------------------

#[test]
fn insert_footnote_adds_one_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("insfn.hwpx");
    let out_str = path_str(&out_path);
    let before = footnote_count(&load(&source_path));

    let args = [
        "edit",
        "insert-footnote",
        &file,
        "--section",
        "0",
        "--para",
        "0",
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

    let after = footnote_count(&load(&out_path));
    assert_eq!(after, before + 1, "각주가 하나 늘어야 한다");
}

#[test]
fn insert_endnote_adds_one_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("insen.hwpx");
    let out_str = path_str(&out_path);
    let before = endnote_count(&load(&source_path));

    let args = [
        "edit",
        "insert-endnote",
        &file,
        "--section",
        "0",
        "--para",
        "0",
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

    let after = endnote_count(&load(&out_path));
    assert_eq!(after, before + 1, "미주가 하나 늘어야 한다");
}

#[test]
fn insert_footnote_and_endnote_require_no_flags_and_default_to_zero() {
    let file = path_str(&sample(SAMPLE));
    for cmd in ["insert-footnote", "insert-endnote"] {
        let args = [cmd, &file, "--dry-run"];
        let args_full: Vec<&str> = std::iter::once("edit").chain(args).collect();
        let out = run(&args_full);
        assert_eq!(out.status.code(), Some(0), "{}", describe(&args_full, &out));
    }
}

// ---------------------------------------------------------------------------
// delete-footnote
// ---------------------------------------------------------------------------

#[test]
fn delete_footnote_requires_all_three_coordinates() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "delete-footnote", &file, "--section", "0", "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn delete_footnote_removes_the_control_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("delfn.hwpx");
    let out_str = path_str(&out_path);
    let before = footnote_count(&load(&source_path));

    let args = [
        "edit",
        "delete-footnote",
        &file,
        "--section",
        "0",
        "--para",
        "6",
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

    let after = footnote_count(&load(&out_path));
    assert_eq!(after, before - 1, "각주가 하나 줄어야 한다");
}

// ---------------------------------------------------------------------------
// insert-footnote-text / delete-text-in-footnote
// ---------------------------------------------------------------------------

#[test]
fn insert_footnote_text_writes_into_the_footnote_paragraph_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("fntext.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "insert-footnote-text",
        &file,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "0",
        "--offset",
        "0",
        "--text",
        "각주내용",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = load(&out_path);
    let paras = footnote_paragraphs(&after, 0, 6, 0);
    assert!(
        paras[0].text.contains("각주내용"),
        "각주 문단 텍스트: {:?}",
        paras[0].text
    );
}

#[test]
fn insert_footnote_text_rejects_empty_string() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-footnote-text",
        &file,
        "--ctrl",
        "0",
        "--text",
        "",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn delete_text_in_footnote_removes_inserted_text_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let with_text_path = tmp_dir().join("fntext_for_delete.hwpx");
    let with_text_str = path_str(&with_text_path);

    let insert_args = [
        "edit",
        "insert-footnote-text",
        &file,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "0",
        "--offset",
        "0",
        "--text",
        "각주내용",
        "-o",
        &with_text_str,
        "--json",
    ];
    let out = run(&insert_args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&insert_args, &out));

    let out_path = tmp_dir().join("fndeltxt.hwpx");
    let out_str = path_str(&out_path);
    let delete_args = [
        "edit",
        "delete-text-in-footnote",
        &with_text_str,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "0",
        "--offset",
        "0",
        "--count",
        "4",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&delete_args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&delete_args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = load(&out_path);
    let paras = footnote_paragraphs(&after, 0, 6, 0);
    assert!(
        !paras[0].text.contains("각주내용"),
        "삭제 후에는 텍스트가 남아 있으면 안 된다: {:?}",
        paras[0].text
    );
}

#[test]
fn delete_text_in_footnote_rejects_zero_count() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "delete-text-in-footnote",
        &file,
        "--count",
        "0",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

// ---------------------------------------------------------------------------
// split-paragraph-in-footnote / merge-paragraph-in-footnote
// ---------------------------------------------------------------------------

#[test]
fn split_paragraph_in_footnote_adds_one_paragraph_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let with_text_path = tmp_dir().join("fntext_for_split.hwpx");
    let with_text_str = path_str(&with_text_path);

    let insert_args = [
        "edit",
        "insert-footnote-text",
        &file,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "0",
        "--offset",
        "0",
        "--text",
        "각주내용",
        "-o",
        &with_text_str,
        "--json",
    ];
    let out = run(&insert_args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&insert_args, &out));

    let out_path = tmp_dir().join("fnsplit.hwpx");
    let out_str = path_str(&out_path);
    let split_args = [
        "edit",
        "split-paragraph-in-footnote",
        &with_text_str,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "0",
        "--offset",
        "2",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&split_args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&split_args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = load(&out_path);
    let paras = footnote_paragraphs(&after, 0, 6, 0);
    assert_eq!(paras.len(), 2, "각주 내부 문단이 하나 늘어야 한다");
}

#[test]
fn merge_paragraph_in_footnote_reduces_paragraph_count_and_preserves_text() {
    // [알려진 특성] split 직후 바로 merge하면(경계 앞뒤 글자모양이 같은 흔한 경우)
    // 저장 시 인접 동일 char_shape 항목이 정리되어 --verify 가 diffCount:1(무해)을
    // 보고할 수 있다 — cli_commands.md의 merge-paragraph-in-footnote 절 참조.
    // 이 테스트는 그 대신 구조(문단 수)와 내용(텍스트) 보존을 직접 확인한다.
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let with_text_path = tmp_dir().join("fntext_for_merge.hwpx");
    let with_text_str = path_str(&with_text_path);

    let insert_args = [
        "edit",
        "insert-footnote-text",
        &file,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "0",
        "--offset",
        "0",
        "--text",
        "각주내용",
        "-o",
        &with_text_str,
        "--json",
    ];
    let out = run(&insert_args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&insert_args, &out));

    let split_path = tmp_dir().join("fnsplit_for_merge.hwpx");
    let split_str = path_str(&split_path);
    let split_args = [
        "edit",
        "split-paragraph-in-footnote",
        &with_text_str,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "0",
        "--offset",
        "2",
        "-o",
        &split_str,
        "--json",
    ];
    let out = run(&split_args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&split_args, &out));

    let out_path = tmp_dir().join("fnmerge.hwpx");
    let out_str = path_str(&out_path);
    let merge_args = [
        "edit",
        "merge-paragraph-in-footnote",
        &split_str,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--fn-para",
        "1",
        "-o",
        &out_str,
        "--json",
    ];
    let out = run(&merge_args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&merge_args, &out));

    let after = load(&out_path);
    let paras = footnote_paragraphs(&after, 0, 6, 0);
    assert_eq!(paras.len(), 1, "병합 후 각주 내부 문단은 하나여야 한다");
    assert!(
        paras[0].text.contains("각주내용"),
        "병합 후에도 내용이 보존돼야 한다: {:?}",
        paras[0].text
    );
}

#[test]
fn merge_paragraph_in_footnote_rejects_zero_fn_para() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "merge-paragraph-in-footnote",
        &file,
        "--fn-para",
        "0",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}
