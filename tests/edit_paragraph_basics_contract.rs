//! `edit insert-text` / `edit delete-text` / `edit insert-paragraph` /
//! `edit delete-paragraph` / `edit merge-paragraph` / `edit split-paragraph`
//! 계약 회귀 테스트 (upstream #5185/#5192 계열 선별 이식 — Tier 2 라운드,
//! "문단 기본" 배치).
//!
//! 코어 로직은 기존 네이티브 함수(`insert_text_native`, `delete_text_native`,
//! `insert_paragraph_native`, `delete_paragraph_native`, `merge_paragraph_native`,
//! `split_paragraph_native`)를 그대로 재사용하고 CLI 배선만 신규다. 이 테스트는
//! 그 배선 — 인자 파싱, `--dry-run`(무변경 + insert-text/insert-paragraph의 좌표
//! 사전검증), `--verify`(저장본 재파싱 IR 대조), 종료 코드(#2707 계약) — 을
//! 검증한다.
//!
//! `merge-paragraph`는 병합 대상 두 문단의 첫 글자 서식(char_shape) id가 우연히
//! 같을 때 `--verify`가 `identical:false`(diffCount:1, `ParagraphCharShapes`)를
//! 보고할 수 있다 — 실측 결과 이는 병합 로직이 만드는 중복 경계(같은 id를 가리키는
//! 인접 run 2개)가 저장→재파싱 과정에서 하나로 정규화되는 것뿐으로,
//! `render-diff`(PASS, 0px, structureMismatch:false)로 확인한 바 렌더링·데이터
//! 손실은 없다. 이 배치의 다른 5개 명령이나 서로 다른 char_shape id를 가진
//! 문단끼리의 병합에서는 재현되지 않는 기존 네이티브 함수의 국소적 특이 케이스라
//! CLI 배선 범위를 벗어난다(별도 코어 로직 변경 없이 이식한다는 이번 라운드
//! 원칙). 세부는 `mydocs/report/upstream_devel_sync_candidates_20260901.md` 참조.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::wasm_api::HwpDocument;

/// 구역0에 15개 문단. 문단0=빈 문단(구역나누기 등 컨트롤 4개, cc=33),
/// 문단1=본문 텍스트(cc=426, text_len=417, 인라인 그림 1개), 문단2=빈 문단
/// (cc=1, 컨트롤 없음), 문단3="중산층 이상 연금 줄어"(text_len=12).
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
    let dir = std::env::temp_dir().join(format!("rhwp-edit-parabasics-{}", std::process::id()));
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

fn para_count(doc: &HwpDocument, section: usize) -> usize {
    doc.document().sections[section].paragraphs.len()
}

fn para_text(doc: &HwpDocument, section: usize, para: usize) -> String {
    doc.document().sections[section].paragraphs[para].text.clone()
}

// ---------------------------------------------------------------------------
// insert-text
// ---------------------------------------------------------------------------

#[test]
fn insert_text_rejects_empty_string() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "insert-text", &file, "--text", "", "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_text_rejects_out_of_range_section_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-text",
        &file,
        "--text",
        "x",
        "--section",
        "99",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_text_rejects_out_of_range_offset_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-text",
        &file,
        "--text",
        "x",
        "--section",
        "0",
        "--para",
        "3",
        "--offset",
        "9999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_text_inserts_at_offset_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("instext.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "insert-text",
        &file,
        "--text",
        "속보! ",
        "--section",
        "0",
        "--para",
        "3",
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
    assert_eq!(env["insertedChars"], 4, "{env}");

    let after = load(&out_path);
    let text = para_text(&after, 0, 3);
    assert!(
        text.starts_with("속보! "),
        "텍스트가 앞에 삽입돼야 한다: {text:?}"
    );
    assert!(text.contains("중산층"), "기존 텍스트도 남아야 한다: {text:?}");
}

// ---------------------------------------------------------------------------
// delete-text
// ---------------------------------------------------------------------------

#[test]
fn delete_text_requires_count() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "delete-text", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn delete_text_rejects_zero_count() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "delete-text", &file, "--count", "0", "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn delete_text_removes_leading_chars_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("deltext.hwpx");
    let out_str = path_str(&out_path);

    let before = para_text(&load(&source_path), 0, 3);
    assert!(before.starts_with("중산"), "{before:?}");

    let args = [
        "edit",
        "delete-text",
        &file,
        "--count",
        "2",
        "--section",
        "0",
        "--para",
        "3",
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

    let after = para_text(&load(&out_path), 0, 3);
    assert!(!after.starts_with("중산"), "삭제된 글자가 남으면 안 된다: {after:?}");
    assert_eq!(after.chars().count(), before.chars().count() - 2);
}

// ---------------------------------------------------------------------------
// insert-paragraph
// ---------------------------------------------------------------------------

#[test]
fn insert_paragraph_rejects_out_of_range_para_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-paragraph",
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
fn insert_paragraph_allows_append_at_para_count_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let before_count = para_count(&load(&source_path), 0);
    let out_path = tmp_dir().join("appendpara.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "insert-paragraph",
        &file,
        "--section",
        "0",
        "--para",
        &before_count.to_string(),
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
    assert_eq!(para_count(&after, 0), before_count + 1);
}

#[test]
fn insert_paragraph_inserts_in_middle_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let before_count = para_count(&load(&source_path), 0);
    let out_path = tmp_dir().join("midpara.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "insert-paragraph",
        &file,
        "--section",
        "0",
        "--para",
        "2",
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
    assert_eq!(para_count(&after, 0), before_count + 1);
    assert_eq!(para_text(&after, 0, 2), "", "삽입된 새 문단은 비어 있어야 한다");
    // 원래 문단2("빈 문단")가 밀려 문단3이 되고, 원래 문단3(텍스트)은 문단4가 된다.
    assert!(para_text(&after, 0, 4).contains("중산층"));
}

// ---------------------------------------------------------------------------
// delete-paragraph
// ---------------------------------------------------------------------------

#[test]
fn delete_paragraph_removes_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let before_count = para_count(&load(&source_path), 0);
    let out_path = tmp_dir().join("delpara.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "delete-paragraph",
        &file,
        "--section",
        "0",
        "--para",
        "2",
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
    assert_eq!(para_count(&after, 0), before_count - 1);
    // 원래 문단3(텍스트)이 문단2 자리로 당겨온다.
    assert!(para_text(&after, 0, 2).contains("중산층"));
}

// ---------------------------------------------------------------------------
// merge-paragraph
// ---------------------------------------------------------------------------

#[test]
fn merge_paragraph_rejects_first_paragraph() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "merge-paragraph", &file, "--section", "0", "--para", "0"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

#[test]
fn merge_paragraph_merges_into_previous_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let before_count = para_count(&load(&source_path), 0);
    let out_path = tmp_dir().join("mergepara.hwpx");
    let out_str = path_str(&out_path);

    // 문단3(텍스트, char_shape id=8)을 문단2(빈 문단, char_shape id=0)에 병합 —
    // id가 서로 달라 병합 경계 정규화 특이 케이스(모듈 문서 참조)를 피한다.
    let args = [
        "edit",
        "merge-paragraph",
        &file,
        "--section",
        "0",
        "--para",
        "3",
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
    assert_eq!(para_count(&after, 0), before_count - 1);
    assert!(para_text(&after, 0, 2).contains("중산층"));
}

// ---------------------------------------------------------------------------
// split-paragraph
// ---------------------------------------------------------------------------

#[test]
fn split_paragraph_splits_at_offset_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let before_count = para_count(&load(&source_path), 0);
    let before_text = para_text(&load(&source_path), 0, 3);
    let out_path = tmp_dir().join("splitpara.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "split-paragraph",
        &file,
        "--section",
        "0",
        "--para",
        "3",
        "--offset",
        "3",
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
    assert_eq!(para_count(&after, 0), before_count + 1);
    let head: String = before_text.chars().take(3).collect();
    let tail: String = before_text.chars().skip(3).collect();
    assert_eq!(para_text(&after, 0, 3), head);
    assert_eq!(para_text(&after, 0, 4), tail);
}
