//! `edit apply-char-format` / `edit apply-para-format` / `edit apply-style` /
//! `edit apply-para-format-in-hf` / `edit apply-para-format-in-footnote` /
//! `edit apply-endnote-shape` 계약 회귀 테스트 (upstream #5185/#5192 계열 선별
//! 이식 — Tier 2 라운드, "서식/스타일" 배치).
//!
//! 코어 로직은 기존 네이티브 함수(`apply_char_format_native`,
//! `apply_para_format_native`, `apply_style_native`,
//! `apply_para_format_in_hf_native`, `apply_para_format_in_footnote_native`,
//! `apply_endnote_shape_native`)를 그대로 재사용하고 CLI 배선만 신규다. 이 테스트는
//! 그 배선 — 인자 파싱, `--dry-run`(무변경 + apply-char-format/apply-para-format/
//! apply-style/apply-para-format-in-footnote의 좌표 사전검증), `--verify`(저장본
//! 재파싱 IR 대조), 종료 코드(#2707 계약) — 을 검증한다.
//!
//! `apply-para-format-in-hf`/`apply-endnote-shape`는 upstream 원본과 동일하게
//! `--props`의 JSON 형식을 사전 검사하지 않는다(코어가 알려진 키만 골라 읽으므로
//! 잘못된 JSON도 조용히 무시된다) — 이 비대칭은 이번 배치가 만든 것이 아니라
//! upstream 43개 명령 전체에 걸친 기존 특성이다.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::model::control::Control;
use rhwp::model::footnote::NumberFormat;
use rhwp::wasm_api::HwpDocument;

/// 구역0에 15개 문단. 문단1=본문 텍스트(text_len=417). 문단6 컨트롤0=본문 최상위
/// 각주(각주 내부 문단 1개). 구역0에 머리말 컨트롤 보유.
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
    let dir = std::env::temp_dir().join(format!("rhwp-edit-formatstyle-{}", std::process::id()));
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

fn para_shape_id(doc: &HwpDocument, section: usize, para: usize) -> u16 {
    doc.document().sections[section].paragraphs[para].para_shape_id
}

fn style_id(doc: &HwpDocument, section: usize, para: usize) -> u8 {
    doc.document().sections[section].paragraphs[para].style_id
}

fn char_shape_ids(doc: &HwpDocument, section: usize, para: usize) -> Vec<u32> {
    doc.document().sections[section].paragraphs[para]
        .char_shapes
        .iter()
        .map(|c| c.char_shape_id)
        .collect()
}

fn header_para_shape_id(doc: &HwpDocument, section: usize) -> u16 {
    doc.document().sections[section]
        .paragraphs
        .iter()
        .flat_map(|p| &p.controls)
        .find_map(|c| match c {
            Control::Header(h) => Some(h.paragraphs[0].para_shape_id),
            _ => None,
        })
        .expect("머리말 컨트롤을 찾지 못함")
}

fn footnote_para_shape_id(doc: &HwpDocument, section: usize, para: usize, ctrl: usize) -> u16 {
    let Control::Footnote(fnote) =
        &doc.document().sections[section].paragraphs[para].controls[ctrl]
    else {
        panic!("각주 컨트롤 아님");
    };
    fnote.paragraphs[0].para_shape_id
}

fn endnote_number_format(doc: &HwpDocument, section: usize) -> NumberFormat {
    doc.document().sections[section]
        .section_def
        .endnote_shape
        .number_format
}

// ---------------------------------------------------------------------------
// apply-char-format
// ---------------------------------------------------------------------------

#[test]
fn apply_char_format_rejects_missing_props() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "apply-char-format", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_char_format_rejects_invalid_json_props() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-char-format",
        &file,
        "--props",
        "not-json",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_char_format_rejects_out_of_range_offset_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-char-format",
        &file,
        "--props",
        "{\"bold\":true}",
        "--section",
        "0",
        "--para",
        "1",
        "--offset",
        "999999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_char_format_applies_bold_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("chfmt.hwpx");
    let out_str = path_str(&out_path);
    let before = char_shape_ids(&load(&source_path), 0, 1);

    let args = [
        "edit",
        "apply-char-format",
        &file,
        "--props",
        "{\"bold\":true}",
        "--section",
        "0",
        "--para",
        "1",
        "--offset",
        "0",
        "--count",
        "5",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = char_shape_ids(&load(&out_path), 0, 1);
    assert_ne!(
        before, after,
        "글자 서식 적용으로 charShape 구성이 바뀌어야 한다"
    );
}

// ---------------------------------------------------------------------------
// apply-para-format
// ---------------------------------------------------------------------------

#[test]
fn apply_para_format_rejects_missing_props() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "apply-para-format", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_rejects_invalid_json_props() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-para-format",
        &file,
        "--props",
        "not-json",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_rejects_out_of_range_para_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-para-format",
        &file,
        "--props",
        "{\"alignment\":\"center\"}",
        "--section",
        "0",
        "--para",
        "999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_applies_alignment_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("pfmt.hwpx");
    let out_str = path_str(&out_path);
    let before = para_shape_id(&load(&source_path), 0, 1);

    let args = [
        "edit",
        "apply-para-format",
        &file,
        "--props",
        "{\"alignment\":\"center\"}",
        "--section",
        "0",
        "--para",
        "1",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = para_shape_id(&load(&out_path), 0, 1);
    assert_ne!(
        before, after,
        "문단 서식 적용으로 paraShape id가 바뀌어야 한다"
    );
}

// ---------------------------------------------------------------------------
// apply-style
// ---------------------------------------------------------------------------

#[test]
fn apply_style_rejects_missing_style() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "apply-style", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_style_rejects_out_of_range_style_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-style",
        &file,
        "--style",
        "99999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_style_applies_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("style.hwpx");
    let out_str = path_str(&out_path);
    let before = style_id(&load(&source_path), 0, 1);
    let target_style: u8 = if before == 1 { 2 } else { 1 };

    let args = [
        "edit",
        "apply-style",
        &file,
        "--style",
        &target_style.to_string(),
        "--section",
        "0",
        "--para",
        "1",
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
    assert_eq!(
        style_id(&after_doc, 0, 1),
        target_style,
        "문단의 style_id가 요청한 스타일로 바뀌어야 한다"
    );
}

// ---------------------------------------------------------------------------
// apply-para-format-in-hf
// ---------------------------------------------------------------------------

#[test]
fn apply_para_format_in_hf_rejects_both_flags() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-para-format-in-hf",
        &file,
        "--header",
        "--footer",
        "--props",
        "{}",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_in_hf_rejects_neither_flag() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-para-format-in-hf",
        &file,
        "--props",
        "{}",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_in_hf_rejects_missing_props() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-para-format-in-hf",
        &file,
        "--header",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_in_hf_applies_to_header_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("hfpfmt.hwpx");
    let out_str = path_str(&out_path);
    let before = header_para_shape_id(&load(&source_path), 0);

    let args = [
        "edit",
        "apply-para-format-in-hf",
        &file,
        "--header",
        "--props",
        "{\"alignment\":\"center\"}",
        "--section",
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

    let after = header_para_shape_id(&load(&out_path), 0);
    assert_ne!(
        before, after,
        "머리말 문단 서식 적용으로 paraShape id가 바뀌어야 한다"
    );
}

// ---------------------------------------------------------------------------
// apply-para-format-in-footnote
// ---------------------------------------------------------------------------

#[test]
fn apply_para_format_in_footnote_requires_section_para_ctrl_props() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-para-format-in-footnote",
        &file,
        "--section",
        "0",
        "--para",
        "6",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_in_footnote_rejects_invalid_json_props() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-para-format-in-footnote",
        &file,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--props",
        "not-json",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_para_format_in_footnote_applies_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("fnpfmt.hwpx");
    let out_str = path_str(&out_path);
    let before = footnote_para_shape_id(&load(&source_path), 0, 6, 0);

    let args = [
        "edit",
        "apply-para-format-in-footnote",
        &file,
        "--section",
        "0",
        "--para",
        "6",
        "--ctrl",
        "0",
        "--props",
        "{\"alignment\":\"center\"}",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    let after = footnote_para_shape_id(&load(&out_path), 0, 6, 0);
    assert_ne!(
        before, after,
        "각주 안 문단 서식 적용으로 paraShape id가 바뀌어야 한다"
    );
}

// ---------------------------------------------------------------------------
// apply-endnote-shape
// ---------------------------------------------------------------------------

#[test]
fn apply_endnote_shape_rejects_missing_props() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "apply-endnote-shape", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_endnote_shape_applies_number_format_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("enshape.hwpx");
    let out_str = path_str(&out_path);
    assert_eq!(
        endnote_number_format(&load(&source_path), 0),
        NumberFormat::Digit,
        "샘플 문서 기본 미주 번호 서식 전제가 바뀌었다"
    );

    let args = [
        "edit",
        "apply-endnote-shape",
        &file,
        "--props",
        "{\"numberFormat\":\"circledDigit\"}",
        "--section",
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

    let after = endnote_number_format(&load(&out_path), 0);
    assert_eq!(
        after,
        NumberFormat::CircledDigit,
        "미주 번호 서식이 요청값으로 바뀌어야 한다"
    );
}
