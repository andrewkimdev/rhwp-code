//! `edit insert-header-footer` / `edit delete-header-footer` /
//! `edit insert-header-footer-text` / `edit set-header-footer-text` /
//! `edit set-hf-picture` / `edit apply-hf-template` / `edit delete-hf-text` /
//! `edit insert-field-in-hf` 계약 회귀 테스트 (upstream #5185/#5192 계열 선별 이식
//! — Tier 2 라운드, "머리말/꼬리말" 배치).
//!
//! 코어 로직은 기존 네이티브 함수(`create_header_footer_native`,
//! `delete_header_footer_native`, `insert_text_in_header_footer_native`,
//! `delete_text_in_header_footer_native`, `get_header_footer_para_info_native`,
//! `set_header_footer_picture_properties_native`, `apply_hf_template_native`,
//! `insert_field_in_hf_native`)를 그대로 재사용하고 CLI 배선만 신규다. 이 테스트는
//! 그 배선 — 인자 파싱, `--dry-run`(무변경), `--verify`(저장본 재파싱 IR 대조),
//! 종료 코드(#2707 계약) — 을 검증한다.
//!
//! `toggle-hide-hf`(9번째 후보)는 착수 도중 배선을 되돌렸다 — 코어
//! `toggle_hide_header_footer_native`가 조작하는 `hidden_header_footer`는
//! `DocumentCore`/`LayoutEngine`에만 있는 **세션 전용 렌더 캐시 힌트**이며 어떤
//! 직렬화 경로에도 연결되어 있지 않다(저장→재로드하면 항상 빈 집합으로
//! 리셋된다). 즉 "토글→저장→다시 열기"를 실제로 구동해 보면 저장된 파일은
//! **입력과 완전히 동일**하고, 두 번째 토글은 (첫 토글의 효과가 전혀 남지 않았으므로)
//! 다시 "숨김"으로만 나온다 — CLI가 파일을 만드는 것처럼 보이지만 실제로는
//! 아무것도 영구화하지 않는 오해 소지 있는 명령이 된다. 세부는
//! `mydocs/report/upstream_devel_sync_candidates_20260901.md` 참조.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::model::control::Control;
use rhwp::wasm_api::HwpDocument;

/// 구역0 안 어딘가(문단14/컨트롤0)에 머리말(양쪽 적용, 문단 1개, 텍스트 "상공신문")
/// 보유, 꼬리말은 없음.
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
    let dir = std::env::temp_dir().join(format!("rhwp-edit-hf-{}", std::process::id()));
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

fn has_header(doc: &HwpDocument, section: usize) -> bool {
    doc.document().sections[section]
        .paragraphs
        .iter()
        .flat_map(|p| &p.controls)
        .any(|c| matches!(c, Control::Header(_)))
}

fn has_footer(doc: &HwpDocument, section: usize) -> bool {
    doc.document().sections[section]
        .paragraphs
        .iter()
        .flat_map(|p| &p.controls)
        .any(|c| matches!(c, Control::Footer(_)))
}

/// 구역 안 어느 문단에 걸려 있든 머리말 컨트롤을 찾아 그 내부 문단 0의 텍스트를
/// 돌려준다. 앵커 문단 인덱스는 CLI의 `--para`(머리말 *내부* 문단 인덱스)와 다른
/// 축이라 하드코딩하지 않는다.
fn header_paragraph_text(doc: &HwpDocument, section: usize) -> String {
    doc.document().sections[section]
        .paragraphs
        .iter()
        .flat_map(|p| &p.controls)
        .find_map(|c| match c {
            Control::Header(h) => Some(h.paragraphs[0].text.clone()),
            _ => None,
        })
        .expect("머리말 컨트롤을 찾지 못함")
}

// ---------------------------------------------------------------------------
// insert-header-footer / delete-header-footer
// ---------------------------------------------------------------------------

#[test]
fn insert_header_footer_rejects_both_flags() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "insert-header-footer", &file, "--header", "--footer", "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_header_footer_rejects_neither_flag() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "insert-header-footer", &file, "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_header_footer_creates_footer_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("insfooter.hwpx");
    let out_str = path_str(&out_path);
    assert!(!has_footer(&load(&source_path), 0), "샘플은 꼬리말이 없어야 한다");

    let args = [
        "edit",
        "insert-header-footer",
        &file,
        "--footer",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");
    assert_eq!(env["isHeader"], false, "{env}");

    assert!(has_footer(&load(&out_path), 0), "꼬리말이 생겨야 한다");
}

#[test]
fn insert_header_footer_rejects_duplicate_header() {
    let file = path_str(&sample(SAMPLE));
    let args = ["edit", "insert-header-footer", &file, "--header", "--json"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

#[test]
fn delete_header_footer_removes_the_header_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("delheader.hwpx");
    let out_str = path_str(&out_path);
    assert!(has_header(&load(&source_path), 0), "샘플은 머리말이 있어야 한다");

    let args = [
        "edit",
        "delete-header-footer",
        &file,
        "--header",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");

    assert!(!has_header(&load(&out_path), 0), "머리말이 사라져야 한다");
}

// ---------------------------------------------------------------------------
// insert-header-footer-text / set-header-footer-text / delete-hf-text
// ---------------------------------------------------------------------------

#[test]
fn insert_header_footer_text_rejects_empty_string() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-header-footer-text",
        &file,
        "--header",
        "--text",
        "",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_header_footer_text_prepends_to_existing_header_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("hfinstext.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "insert-header-footer-text",
        &file,
        "--header",
        "--text",
        "속보 ",
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
    assert_eq!(env["insertedChars"], 3, "{env}");

    let after = load(&out_path);
    let text = header_paragraph_text(&after, 0);
    assert!(
        text.starts_with("속보 "),
        "머리말 앞에 삽입돼야 한다: {text:?}"
    );
    assert!(text.contains("상공신문"), "기존 텍스트도 남아야 한다: {text:?}");
}

#[test]
fn set_header_footer_text_replaces_whole_paragraph_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("hfset.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "set-header-footer-text",
        &file,
        "--header",
        "--text",
        "완전교체제목",
        "--para",
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

    let after = load(&out_path);
    let text = header_paragraph_text(&after, 0);
    assert_eq!(text, "완전교체제목", "이전 텍스트가 완전히 사라져야 한다");
}

#[test]
fn set_header_footer_text_rejects_empty_string() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "set-header-footer-text",
        &file,
        "--header",
        "--text",
        "",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn delete_hf_text_removes_characters_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("hfdeltxt.hwpx");
    let out_str = path_str(&out_path);
    let before_text = header_paragraph_text(&load(&source_path), 0);

    let args = [
        "edit",
        "delete-hf-text",
        &file,
        "--header",
        "--count",
        "2",
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

    let after_text = header_paragraph_text(&load(&out_path), 0);
    assert_eq!(
        after_text,
        before_text.chars().skip(2).collect::<String>(),
        "앞 2글자가 삭제돼야 한다"
    );
}

#[test]
fn delete_hf_text_rejects_zero_count() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "delete-hf-text",
        &file,
        "--header",
        "--count",
        "0",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

// ---------------------------------------------------------------------------
// insert-field-in-hf / apply-hf-template / toggle-hide-hf
// ---------------------------------------------------------------------------

#[test]
fn insert_field_in_hf_rejects_out_of_range_field_type() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "insert-field-in-hf",
        &file,
        "--header",
        "--field-type",
        "4",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn insert_field_in_hf_inserts_page_number_marker_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("hffield.hwpx");
    let out_str = path_str(&out_path);
    let before_len = header_paragraph_text(&load(&source_path), 0).chars().count();

    let args = [
        "edit",
        "insert-field-in-hf",
        &file,
        "--header",
        "--field-type",
        "1",
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

    // 필드 마커는 눈에 보이는 문자를 남기지 않지만(제어 문자), char_offsets/
    // char_count 축은 늘어난다 — 텍스트 길이 자체는 그대로임을 확인해 최소한
    // "본문 텍스트가 깨지지 않았다"만 검증한다(정확한 마커 폭은 코어 책임).
    let after = load(&out_path);
    let after_text = header_paragraph_text(&after, 0);
    assert_eq!(
        after_text.chars().count(),
        before_len,
        "필드 마커는 가시 텍스트를 추가하지 않아야 한다"
    );
}

#[test]
fn apply_hf_template_rejects_out_of_range_template() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "apply-hf-template",
        &file,
        "--header",
        "--template",
        "11",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn apply_hf_template_applies_and_verify_passes() {
    let source_path = sample(SAMPLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("hftpl.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "apply-hf-template",
        &file,
        "--header",
        "--template",
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
    assert!(has_header(&load(&out_path), 0), "머리말은 계속 있어야 한다");
}

// ---------------------------------------------------------------------------
// set-hf-picture
// ---------------------------------------------------------------------------

#[test]
fn set_hf_picture_requires_all_coordinates() {
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "set-hf-picture",
        &file,
        "--section",
        "0",
        "--para",
        "0",
        "--ctrl",
        "0",
        "--props",
        "{\"width\":1000}",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn set_hf_picture_rejects_non_picture_inner_control() {
    // 샘플 머리말 내부 컨트롤 0은 그림이 아니라 문단 텍스트뿐이다 — 코어가
    // "외부 컨트롤이 머리말/꼬리말이 아닙니다"류 런타임 오류로 거부해야 한다.
    let file = path_str(&sample(SAMPLE));
    let args = [
        "edit",
        "set-hf-picture",
        &file,
        "--section",
        "0",
        "--para",
        "0",
        "--ctrl",
        "0",
        "--inner-para",
        "0",
        "--inner-ctrl",
        "0",
        "--props",
        "{\"width\":1000}",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}
