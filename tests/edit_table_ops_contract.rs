//! `edit set-table-props` / `edit set-section-def` / `edit move-table` /
//! `edit transpose-table` / `edit set-column-widths` 계약 회귀 테스트 (upstream
//! #5185/#5192 계열 선별 이식 — Tier 2 라운드).
//!
//! 코어 로직은 기존 네이티브 함수(`set_table_properties_native`,
//! `set_section_def_native`, `move_table_offset_native`,
//! `transpose_table_cells_in_place_native`, `set_table_column_widths_native`)를 그대로
//! 재사용하고 CLI 배선만 신규다. 이 테스트는 그 배선 — 인자 파싱, `--dry-run`(무변경),
//! `--verify`(저장본 재파싱 IR 대조), 종료 코드(#2707 계약) — 을 검증한다.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rhwp::model::control::Control;
use rhwp::wasm_api::HwpDocument;

/// 6x5, 병합 없음 — move-table/transpose-table/set-column-widths 정상 경로용.
const PLAIN_TABLE: &str = "samples/hwpx/143E433F503322BD33.hwpx";
/// 표 0번이 병합 셀을 포함 — transpose-table 거부 경로용.
const MERGED_TABLE: &str = "samples/hwpx/appccr1dlm.hwpx";

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
    let dir = std::env::temp_dir().join(format!("rhwp-edit-table-ops-{}", std::process::id()));
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

/// 최상위 표 `table_no`의 (구역, 문단, 컨트롤) 좌표와 `&Table`을 함께 돌려준다.
fn find_table(doc: &HwpDocument, table_no: usize) -> (usize, usize, usize) {
    use rhwp::document_core::queries::table_extract::extract_tables;
    let grids = extract_tables(doc.document());
    let grid = grids
        .iter()
        .find(|g| g.index == table_no && g.container_path.is_empty())
        .unwrap_or_else(|| panic!("표 {table_no} 없음"));
    (grid.section, grid.paragraph, grid.control)
}

fn load(path: &Path) -> HwpDocument {
    let bytes = std::fs::read(path).expect("샘플 읽기");
    HwpDocument::from_bytes(&bytes).expect("샘플 파싱")
}

// ---------------------------------------------------------------------------
// set-table-props
// ---------------------------------------------------------------------------

#[test]
fn set_table_props_rejects_non_object_props_even_in_dry_run() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = [
        "edit",
        "set-table-props",
        &file,
        "--table",
        "0",
        "--props",
        "200",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn set_table_props_applies_cell_spacing_and_verify_passes() {
    let source_path = sample(PLAIN_TABLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("props.hwpx");
    let out_str = path_str(&out_path);

    let args = [
        "edit",
        "set-table-props",
        &file,
        "--table",
        "0",
        "--props",
        "{\"cellSpacing\":333,\"treatAsChar\":true}",
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");
    assert!(env["changedPages"].is_array(), "{env}");

    let after = load(&out_path);
    let (sec, para, ctrl) = find_table(&after, 0);
    let Control::Table(after_table) = &after.document().sections[sec].paragraphs[para].controls[ctrl]
    else {
        panic!("표 컨트롤 아님");
    };
    assert_eq!(after_table.cell_spacing, 333, "cellSpacing이 반영돼야 한다");
    assert_ne!(after_table.attr & 0x01, 0, "treatAsChar 비트가 설정돼야 한다");
}

#[test]
fn set_table_props_rejects_missing_table_index() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = [
        "edit",
        "set-table-props",
        &file,
        "--table",
        "999",
        "--props",
        "{\"cellSpacing\":100}",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

// ---------------------------------------------------------------------------
// move-table
// ---------------------------------------------------------------------------

#[test]
fn move_table_dry_run_writes_no_file_and_reports_null_changed_pages() {
    let file = path_str(&sample(PLAIN_TABLE));
    let out_path = tmp_dir().join("move_dry.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "move-table",
        &file,
        "--table",
        "0",
        "--dh",
        "500",
        "--dv",
        "300",
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
    assert!(env.get("output").is_none(), "dry-run은 output을 내면 안 된다: {env}");
    assert!(!out_path.exists(), "dry-run은 파일을 쓰면 안 된다");
}

#[test]
fn move_table_updates_offsets_and_verify_passes() {
    let source_path = sample(PLAIN_TABLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("moved.hwpx");
    let out_str = path_str(&out_path);

    let before = load(&source_path);
    let (sec, para, ctrl) = find_table(&before, 0);
    let Control::Table(before_table) = &before.document().sections[sec].paragraphs[para].controls[ctrl]
    else {
        panic!("표 컨트롤 아님");
    };
    let (before_h, before_v) = (
        before_table.common.horizontal_offset,
        before_table.common.vertical_offset,
    );
    // treat_as_char(본문배치) 표는 v_offset이 문단 경계를 넘으면 다음/이전 문단으로
    // 옮기며 line_height 기준으로 재계산한다 — 그 경로에서는 "before_v + dv"로 단순
    // 검증할 수 없다(코어 자체 동작은 wasm_api::tests::table_tests가 검증). CLI 배선만
    // 확인하는 이 테스트는 그 경로를 피해 --dv 를 0으로 두고 --dh 만 검증한다.
    let is_treat_as_char = (before_table.attr & 0x01) != 0;
    // treat_as_char 표는 --dv 를 0으로 둬 문단 경계 재계산 경로를 피한다.
    let dv_arg = if is_treat_as_char { "0" } else { "-200" };

    let args = [
        "edit",
        "move-table",
        &file,
        "--table",
        "0",
        "--dh",
        "700",
        "--dv",
        dv_arg,
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));

    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");
    assert!(env["changedPages"].is_array(), "{env}");

    let after = load(&out_path);
    let (sec2, para2, ctrl2) = find_table(&after, 0);
    let Control::Table(after_table) = &after.document().sections[sec2].paragraphs[para2].controls[ctrl2]
    else {
        panic!("표 컨트롤 아님");
    };
    assert_eq!(
        after_table.common.horizontal_offset as i64,
        before_h as i64 + 700,
        "가로 오프셋이 --dh 만큼 이동해야 한다"
    );
    let expected_v = if is_treat_as_char {
        before_v as i64
    } else {
        before_v as i64 - 200
    };
    assert_eq!(
        after_table.common.vertical_offset as i64, expected_v,
        "세로 오프셋이 --dv 만큼 이동해야 한다"
    );
}

#[test]
fn move_table_rejects_missing_table_index() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = [
        "edit", "move-table", &file, "--table", "999", "--dh", "100", "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

#[test]
fn move_table_rejects_zero_delta() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = ["edit", "move-table", &file, "--table", "0", "--dry-run"];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

// ---------------------------------------------------------------------------
// transpose-table
// ---------------------------------------------------------------------------

#[test]
fn transpose_table_swaps_dimensions_and_preserves_content() {
    let source_path = sample(PLAIN_TABLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("transposed.hwpx");
    let out_str = path_str(&out_path);

    let before = load(&source_path);
    let (sec, para, ctrl) = find_table(&before, 0);
    let Control::Table(before_table) = &before.document().sections[sec].paragraphs[para].controls[ctrl]
    else {
        panic!("표 컨트롤 아님");
    };
    let (before_rows, before_cols) = (before_table.row_count, before_table.col_count);
    // 전치 전 (0,0)열의 텍스트 — 전치 후 (0,0)행의 텍스트와 같아야 한다.
    let before_col0_texts: Vec<String> = (0..before_rows)
        .map(|r| {
            before_table
                .cells
                .iter()
                .find(|c| c.row == r && c.col == 0)
                .map(|c| c.paragraphs.iter().map(|p| p.text.as_str()).collect::<String>())
                .unwrap_or_default()
        })
        .collect();

    let args = [
        "edit",
        "transpose-table",
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
    assert_eq!(env["sourceRows"], before_rows, "{env}");
    assert_eq!(env["sourceCols"], before_cols, "{env}");
    assert_eq!(env["targetRows"], before_cols, "{env}");
    assert_eq!(env["targetCols"], before_rows, "{env}");

    let after = load(&out_path);
    let (sec2, para2, ctrl2) = find_table(&after, 0);
    let Control::Table(after_table) = &after.document().sections[sec2].paragraphs[para2].controls[ctrl2]
    else {
        panic!("표 컨트롤 아님");
    };
    assert_eq!(after_table.row_count, before_cols, "행/열이 뒤바뀌어야 한다");
    assert_eq!(after_table.col_count, before_rows, "행/열이 뒤바뀌어야 한다");

    let after_row0_texts: Vec<String> = (0..before_rows)
        .map(|c| {
            after_table
                .cells
                .iter()
                .find(|cell| cell.row == 0 && cell.col == c)
                .map(|cell| cell.paragraphs.iter().map(|p| p.text.as_str()).collect::<String>())
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(
        before_col0_texts, after_row0_texts,
        "원래 첫 열의 내용이 전치 후 첫 행이 되어야 한다"
    );
}

#[test]
fn transpose_table_rejects_merged_table_and_writes_nothing() {
    let file = path_str(&sample(MERGED_TABLE));
    let out_path = tmp_dir().join("merged_transpose_should_not_exist.hwpx");
    let out_str = path_str(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let args = [
        "edit",
        "transpose-table",
        &file,
        "--table",
        "0",
        "-o",
        &out_str,
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
    assert!(!out_path.exists(), "실패 시 원본 불변 — 출력 파일을 쓰면 안 된다");
}

// ---------------------------------------------------------------------------
// set-column-widths
// ---------------------------------------------------------------------------

#[test]
fn set_column_widths_rejects_mismatched_count_even_in_dry_run() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = [
        "edit",
        "set-column-widths",
        &file,
        "--table",
        "0",
        "--widths",
        "1000,2000",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn set_column_widths_applies_widths_and_verify_passes() {
    let source_path = sample(PLAIN_TABLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("widths.hwpx");
    let out_str = path_str(&out_path);

    let before = load(&source_path);
    let (sec, para, ctrl) = find_table(&before, 0);
    let Control::Table(before_table) = &before.document().sections[sec].paragraphs[para].controls[ctrl]
    else {
        panic!("표 컨트롤 아님");
    };
    let col_count = before_table.col_count as usize;
    let widths: Vec<u32> = (0..col_count).map(|i| 1500 + i as u32 * 200).collect();
    let widths_arg = widths
        .iter()
        .map(|w| w.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let expected_total: u32 = widths.iter().sum();

    let args = [
        "edit",
        "set-column-widths",
        &file,
        "--table",
        "0",
        "--widths",
        &widths_arg,
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");
    assert_eq!(env["colCount"], col_count as u64, "{env}");
    assert_eq!(env["tableWidth"], expected_total, "{env}");

    let after = load(&out_path);
    let (sec2, para2, ctrl2) = find_table(&after, 0);
    let Control::Table(after_table) = &after.document().sections[sec2].paragraphs[para2].controls[ctrl2]
    else {
        panic!("표 컨트롤 아님");
    };
    assert_eq!(after_table.get_column_widths(), widths, "열 폭이 그대로 반영돼야 한다");
}

#[test]
fn set_column_widths_rejects_missing_table_index() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = [
        "edit",
        "set-column-widths",
        &file,
        "--table",
        "999",
        "--widths",
        "1000,2000,3000,4000,5000",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

// ---------------------------------------------------------------------------
// set-section-def
// ---------------------------------------------------------------------------

#[test]
fn set_section_def_rejects_non_object_props_even_in_dry_run() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = [
        "edit",
        "set-section-def",
        &file,
        "--props",
        "true",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(2), "{}", describe(&args, &out));
}

#[test]
fn set_section_def_rejects_out_of_range_section_even_in_dry_run() {
    let file = path_str(&sample(PLAIN_TABLE));
    let args = [
        "edit",
        "set-section-def",
        &file,
        "--props",
        "{\"hideHeader\":true}",
        "--section",
        "99",
        "--dry-run",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&args, &out));
}

#[test]
fn set_section_def_applies_hide_header_and_verify_passes() {
    let source_path = sample(PLAIN_TABLE);
    let file = path_str(&source_path);
    let out_path = tmp_dir().join("secdef.hwpx");
    let out_str = path_str(&out_path);

    let before = load(&source_path);
    let before_hide_header = before.document().sections[0].section_def.hide_header;

    let args = [
        "edit",
        "set-section-def",
        &file,
        "--props",
        &format!("{{\"hideHeader\":{},\"columnSpacing\":777}}", !before_hide_header),
        "-o",
        &out_str,
        "--verify",
        "--json",
    ];
    let out = run(&args);
    assert_eq!(out.status.code(), Some(0), "{}", describe(&args, &out));
    let env = json_of(&out);
    assert_eq!(env["verify"]["identical"], true, "{env}");
    assert!(env["changedPages"].is_array(), "{env}");
    assert!(env["pageCount"].as_u64().is_some(), "{env}");

    let after = load(&out_path);
    let after_sd = &after.document().sections[0].section_def;
    assert_eq!(
        after_sd.hide_header, !before_hide_header,
        "hideHeader가 토글돼야 한다"
    );
    assert_eq!(after_sd.column_spacing, 777, "columnSpacing이 반영돼야 한다");
}
