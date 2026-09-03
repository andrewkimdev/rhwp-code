//! `edit set-numbering-restart` 계약 회귀 테스트.
//!
//! 43개 upstream #5185/#5192 배치 중 마지막 1개. 다른 배치들과 달리 이 명령은
//! **순수 CLI 배선이 아니다** — 코어 `set_numbering_restart_native`
//! (`src/document_core/commands/formatting.rs`)가 옛 `Paragraph.numbering_restart`
//! 필드(세션 전용, 저장 후 재파싱하면 항상 사라짐) 대신 `ParaShape.numbering_id`가
//! 가리키는 `Numbering` 테이블 자체를 갈아 끼우는 방식으로 재작성됐다. 그래서
//! 이 테스트는 `--verify`(개수만 비교, `level_start_numbers` 값은 못 잡음)에
//! 의존하지 않고 재파싱된 모델을 직접 읽어 단언한다.
//!
//! 픽스처 `para-head-num-2.hwp`(HWPX 사본은 `samples/hwpx/para-head-num-2.hwpx`)는
//! 실제 한컴 문서로, 구역0 문단1~6이 번호 매기기 문단이다:
//! 문단1·2(num_id=3, "가"/"나"), 문단3(num_id=2, "다"), 문단4(num_id=3, "라"),
//! 문단5·6(num_id=4, "마"/"바"). "다"가 다른 목록이라 문단1→2까지만 같은 목록이
//! 이어지고("전진 전파" 경계), "라"는 문단3 뒤에 있어 문단1에 건 mode=2가
//! 넘어가면 안 된다.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::model::style::HeadType;
use rhwp::wasm_api::HwpDocument;

const SAMPLE_HWP: &str = "rhwp-studio/public/samples/para-head-num-2.hwp";
const SAMPLE_HWPX: &str = "samples/hwpx/para-head-num-2.hwpx";

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
    let dir = std::env::temp_dir().join(format!("rhwp-edit-numrst-{}", std::process::id()));
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

/// (para_shape_id, numbering_id, level_start_numbers[level]) 를 뽑아온다.
fn numbering_of(doc: &HwpDocument, section: usize, para: usize) -> (u16, u16, u32) {
    let d = doc.document();
    let ps_id = d.sections[section].paragraphs[para].para_shape_id;
    let ps = &d.doc_info.para_shapes[ps_id as usize];
    assert!(
        matches!(ps.head_type, HeadType::Number | HeadType::Outline),
        "대상 문단은 번호 매기기 문단이어야 한다"
    );
    let level_idx = (ps.para_level as usize).min(6);
    let start = ps
        .numbering_id
        .checked_sub(1)
        .and_then(|i| d.doc_info.numberings.get(i as usize))
        .map(|n| n.level_start_numbers[level_idx])
        .unwrap_or(0);
    (ps_id, ps.numbering_id, start)
}

// ---------------------------------------------------------------------------
// 범위 검사 / --dry-run / --json 배선
// ---------------------------------------------------------------------------

#[test]
fn rejects_missing_mode() {
    let file = path_str(&sample(SAMPLE_HWP));
    let args = ["edit", "set-numbering-restart", &file, "--count", "5"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
    assert!(
        out.stdout.is_empty(),
        "usage 오류는 stdout이 비어야 한다: {}",
        describe(&args, &out)
    );
}

#[test]
fn rejects_out_of_range_section_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE_HWP));
    let args = [
        "edit",
        "set-numbering-restart",
        &file,
        "--mode",
        "2",
        "--section",
        "999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn rejects_out_of_range_para_even_in_dry_run() {
    let file = path_str(&sample(SAMPLE_HWP));
    let args = [
        "edit",
        "set-numbering-restart",
        &file,
        "--mode",
        "2",
        "--para",
        "999999",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn dry_run_writes_no_file() {
    let file = path_str(&sample(SAMPLE_HWP));
    let out_path = tmp_dir().join("numrst_dry.hwp");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "set-numbering-restart",
        &file,
        "--mode",
        "2",
        "--para",
        "1",
        "--count",
        "500",
        "-o",
        &out_str,
        "--dry-run",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["dryRun"], true, "{env}");
    assert_eq!(env["count"], 500, "{env}");
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn mcp_tool_is_declared() {
    let out = run(&["capabilities", "--mcp"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        describe(&["capabilities", "--mcp"], &out)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("hwp_set_numbering_restart"),
        "capabilities --mcp 출력에 hwp_set_numbering_restart 가 있어야 한다"
    );
}

// ---------------------------------------------------------------------------
// mode=2: 영속화 + 전진 전파 경계 (핵심 게이트 — --verify로는 못 잡는다)
// ---------------------------------------------------------------------------

fn assert_mode2_persists_and_propagates(sample_rel: &str, out_ext: &str) {
    let source_path = sample(sample_rel);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join(format!("numrst.{out_ext}"));
    let out_str = path_str(&out_path);

    let before = load(&source_path);
    let (before_ps1, before_numid1, _) = numbering_of(&before, 0, 1);
    let (_, before_numid2, _) = numbering_of(&before, 0, 2);
    let (before_ps3, before_numid3, before_start3) = numbering_of(&before, 0, 3);
    let (before_ps4, before_numid4, before_start4) = numbering_of(&before, 0, 4);
    assert_eq!(
        before_numid1, before_numid2,
        "문단1·2는 원래 같은 목록이어야 한다"
    );
    assert_ne!(
        before_numid3, before_numid1,
        "문단3은 원래 다른 목록이어야 한다"
    );
    assert_eq!(
        before_numid4, before_numid1,
        "문단4는 원래 문단1과 같은 numbering_id를 참조해야 한다(픽스처 전제)"
    );

    let args = [
        "edit",
        "set-numbering-restart",
        &file,
        "--mode",
        "2",
        "--count",
        "500",
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

    let after = load(&out_path);

    // 대상 문단(1)과 같은 목록이 이어지는 문단(2)은 새 numbering으로 갈아
    // 끼워지고, 그 level_start_numbers 가 요청한 시작 번호와 일치해야 한다.
    let (after_ps1, after_numid1, after_start1) = numbering_of(&after, 0, 1);
    let (_, after_numid2, after_start2) = numbering_of(&after, 0, 2);
    assert_ne!(
        after_ps1, before_ps1,
        "문단1의 para_shape_id가 바뀌어야 한다"
    );
    assert_ne!(
        after_numid1, before_numid1,
        "문단1이 새 Numbering을 참조해야 한다"
    );
    assert_eq!(
        after_numid1, after_numid2,
        "문단1·2가 같은 새 Numbering을 공유해야 한다"
    );
    assert_eq!(
        after_start1, 500,
        "문단1의 시작 번호가 요청값과 같아야 한다"
    );
    assert_eq!(after_start2, 500, "문단2도 같은 시작 번호를 가져야 한다");

    // 문단3은 애초에 다른 목록이므로 영향받지 않아야 한다(전진 전파 경계).
    let (after_ps3, after_numid3, after_start3) = numbering_of(&after, 0, 3);
    assert_eq!(
        after_ps3, before_ps3,
        "문단3의 para_shape_id는 그대로여야 한다"
    );
    assert_eq!(
        after_numid3, before_numid3,
        "문단3의 numbering_id는 그대로여야 한다"
    );
    assert_eq!(
        after_start3, before_start3,
        "문단3의 시작 번호는 그대로여야 한다"
    );

    // 문단4는 문단3(다른 목록) 뒤에 있어 문단1의 전진 전파가 거기서 끊긴
    // 뒤이므로, 원래 문단1과 같은 numbering_id를 썼더라도 영향받지 않아야 한다.
    let (after_ps4, after_numid4, after_start4) = numbering_of(&after, 0, 4);
    assert_eq!(
        after_ps4, before_ps4,
        "문단4의 para_shape_id는 그대로여야 한다"
    );
    assert_eq!(
        after_numid4, before_numid4,
        "문단4의 numbering_id는 그대로여야 한다"
    );
    assert_eq!(
        after_start4, before_start4,
        "문단4의 시작 번호는 그대로여야 한다"
    );
}

#[test]
fn mode2_persists_and_propagates_hwp5() {
    assert_mode2_persists_and_propagates(SAMPLE_HWP, "hwp");
}

#[test]
fn mode2_persists_and_propagates_hwpx() {
    assert_mode2_persists_and_propagates(SAMPLE_HWPX, "hwpx");
}

// ---------------------------------------------------------------------------
// mode=0/1: no-op (렌더러가 두 값을 구분하지 않는 현재 시맨틱 보존)
// ---------------------------------------------------------------------------

fn assert_mode_is_noop(mode: &str) {
    let source_path = sample(SAMPLE_HWP);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join(format!("numrst_mode{mode}.hwp"));
    let out_str = path_str(&out_path);

    let before = load(&source_path);
    let numberings_before = before.document().doc_info.numberings.len();
    let (before_ps1, before_numid1, before_start1) = numbering_of(&before, 0, 1);

    let args = [
        "edit",
        "set-numbering-restart",
        &file,
        "--mode",
        mode,
        "--section",
        "0",
        "--para",
        "1",
        "-o",
        &out_str,
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));

    let after = load(&out_path);
    let numberings_after = after.document().doc_info.numberings.len();
    let (after_ps1, after_numid1, after_start1) = numbering_of(&after, 0, 1);

    assert_eq!(
        numberings_after, numberings_before,
        "mode={mode}은 Numbering을 추가하면 안 된다"
    );
    assert_eq!(
        after_ps1, before_ps1,
        "mode={mode}은 para_shape_id를 바꾸면 안 된다"
    );
    assert_eq!(
        after_numid1, before_numid1,
        "mode={mode}은 numbering_id를 바꾸면 안 된다"
    );
    assert_eq!(
        after_start1, before_start1,
        "mode={mode}은 시작 번호를 바꾸면 안 된다"
    );
}

#[test]
fn mode0_is_noop() {
    assert_mode_is_noop("0");
}

#[test]
fn mode1_is_noop_in_current_renderer() {
    assert_mode_is_noop("1");
}
