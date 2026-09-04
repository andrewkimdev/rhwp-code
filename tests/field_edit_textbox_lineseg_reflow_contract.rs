//! [결함 회귀] 글상자(TextBox) 내부 ClickHere 필드 값 교체 후 line_segs 재계산 검증.
//!
//! `document_core::queries::field_query::set_field_text_at`가 `NestedEntry::TextBox`
//! 경로(글상자 내부 필드)를 다룰 때도 값 길이 변화로 줄바꿈 경계가 바뀌면 line_segs를
//! 재계산해야 한다. 수정 전에는 insert_text_at/delete_text_at가 기존 line_segs의
//! text_start만 시프트할 뿐 줄 수를 다시 계산하지 않아, 텍스트가 글상자 폭을 넘겨도
//! `ls_count`가 편집 전 그대로(=1) 저장됐다 — 이 시나리오를 수정 전 커밋으로 되돌려
//! 직접 재현 확인함(`ls_count=1`, sw=29444로 stale). 수정 후에는 `ls_count=3`으로
//! 올바르게 재계산된다. 상세: `mydocs/working/field_edit_lineseg_reflow_stage1.md`.
//!
//! 실측 전제(`rhwp fields samples/field-01.hwp --json`로 확인): "목차1[0]"은 section 1,
//! paragraph 0, 글상자 컨트롤 `[3]` 안의 ClickHere 필드다. 원본 텍스트는 "01  " 뒤에
//! 필드 범위가 이어지는 형태라, 값 교체 후에도 접두 텍스트가 남는다.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SAMPLE: &str = "samples/field-01.hwp";

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE)
}

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rhwp-textbox-lineseg-{tag}-{}-{}.hwp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rhwp"))
        .args(args)
        .output()
        .expect("rhwp")
}

/// `rhwp dump` 텍스트 출력에서 지정 컨트롤(`control_marker`, 예: `"[3]"`) 블록 안의
/// 첫 `ls_count=`를 뽑는다. `dump`는 사람이 읽는 디버그 포맷이라 안정된 스키마가
/// 아니므로 정규식 대신 줄 단위 최소 파싱만 한다.
fn textbox_ls_count(dump_text: &str, control_marker: &str) -> usize {
    let mut in_target_control = false;
    for line in dump_text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(control_marker) {
            in_target_control = true;
            continue;
        }
        if in_target_control {
            // 다음 컨트롤 헤더(`  [N]`, 얕은 들여쓰기)를 만나면 대상 블록을 벗어난 것.
            if line.starts_with("  [") && !trimmed.starts_with(control_marker) {
                break;
            }
            if let Some(idx) = trimmed.find("ls_count=") {
                let rest = &trimmed[idx + "ls_count=".len()..];
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                return digits.parse().expect("ls_count 숫자");
            }
        }
    }
    panic!("dump 출력에서 {control_marker} 의 ls_count를 찾지 못함:\n{dump_text}");
}

#[test]
fn textbox_field_edit_reflows_line_segs_and_survives_roundtrip() {
    let p = sample();
    if !p.exists() {
        eprintln!("샘플 없음 — 건너뜀");
        return;
    }

    // 글상자 폭(max_width=30011 HWPUNIT, `dump`로 실측)에 절대 한 줄로 안 들어가는 길이.
    let long_value = "가".repeat(40);
    let out = temp_path("textbox");
    let data = format!(r#"{{"목차1[0]":"{long_value}"}}"#);
    let args = [
        "edit",
        "fill-fields",
        p.to_str().unwrap(),
        "--data",
        &data,
        "-o",
        out.to_str().unwrap(),
        "--verify",
        "--json",
    ];
    let output = run(&args);
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("envelope");
    assert_eq!(output.status.code(), Some(0), "{v}");
    assert_eq!(v["filledCount"], 1, "{v}");
    assert_eq!(
        v["verify"]["identical"], true,
        "라운드트립(저장→재파싱) 자기 일치 실패 — #1380 게이트 회귀: {v}"
    );

    let dump = run(&["dump", out.to_str().unwrap(), "-s", "1", "-p", "0"]);
    assert_eq!(dump.status.code(), Some(0));
    let dump_text = String::from_utf8_lossy(&dump.stdout);
    let ls_count = textbox_ls_count(&dump_text, "[3]");
    assert!(
        ls_count >= 2,
        "글상자(컨트롤 3, max_width=30011 HWPUNIT)에 40자가 들어가려면 line_segs가 \
         여러 줄이어야 하는데 {ls_count}줄로 남음(stale line_segs 결함 재발) — dump:\n{dump_text}"
    );

    let _ = std::fs::remove_file(&out);
}
