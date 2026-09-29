//! 누름틀 채움 뒤 같은 문단에 뒤따르는 텍스트가 있을 때 채운 값의 글자모양 회귀 테스트.
//!
//! 증상: `edit fill-fields` 로 뒤에 텍스트(예: " (인)")가 붙은 누름틀을 채우면 값의
//! **앞 8글자**가 주변 런 서식(필드 밖 글자모양)을 이어받고 나머지만 컨텐츠 서식을 받았다.
//! 8 = FIELD_END 컨트롤 한 개의 UTF-16 폭. 문단 끝 누름틀은 정상이었다.
//!
//! 실제 HWPX(`samples/hwpx/form-01.hwpx` 의 `myMsg01`: 주변 charPr 1, 컨텐츠 charPr 6)를
//! 적재해 필드 뒤에 텍스트를 덧붙인 변형으로 재현한다. 적재 정규화가 안내문을 지운 빈
//! 필드(guide_residue 경로)와 안내문과 다른 기존 값이 있는 필드(count>0 경로)를 모두 본다.
#![cfg(not(target_arch = "wasm32"))]

use std::io::{Read, Write};
use std::path::Path;

const SAMPLE: &str = "samples/hwpx/form-01.hwpx";
const FIELD: &str = "myMsg01";
/// 누름틀 컨텐츠 런의 글자모양 (샘플 원본 `<hp:run charPrIDRef="6"><hp:t>여기에 입력</hp:t>`).
const CONTENT_SHAPE: &str = "6";
/// 누름틀 앞뒤 주변 런의 글자모양 (fieldBegin/fieldEnd 런).
const SURROUNDING_SHAPE: &str = "1";
const TRAILING: &str = " (인)";
/// FIELD_END 폭(8)보다 긴 값 — 앞 8글자만 주변 서식을 받는 증상이 드러난다.
const LONG_VALUE: &str = "(주)한빛반도체 김대표";

fn sample_bytes() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {SAMPLE}: {e}"))
}

/// section0.xml 에 치환을 적용한 HWPX 를 만든다.
fn patch_section(bytes: &[u8], replacements: &[(&str, &str)]) -> Vec<u8> {
    let mut src = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).expect("zip 열기");
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut entry = src.by_index(i).expect("zip 엔트리");
        let name = entry.name().to_string();
        if name == "Contents/section0.xml" {
            let mut xml = String::new();
            entry.read_to_string(&mut xml).expect("section XML");
            let mut patched = xml.clone();
            for (from, to) in replacements {
                assert_eq!(
                    patched.matches(from).count(),
                    1,
                    "치환 앵커는 section 에 정확히 한 번 나와야 한다: {from}"
                );
                patched = patched.replacen(from, to, 1);
            }
            out.start_file(name, zip::write::SimpleFileOptions::default())
                .expect("start_file");
            out.write_all(patched.as_bytes()).expect("write section");
        } else {
            out.raw_copy_file(entry).expect("raw copy");
        }
    }
    out.finish().expect("finish").into_inner()
}

/// 필드 fieldEnd 뒤 빈 `<hp:t/>` 를 뒤따르는 텍스트로 바꾼다.
const FIELD_END_TAIL: &str = r#"fieldid="627272811"/></hp:ctrl><hp:t/></hp:run>"#;
const FIELD_END_TAIL_WITH_TEXT: &str =
    r#"fieldid="627272811"/></hp:ctrl><hp:t> (인)</hp:t></hp:run>"#;
/// 안내문과 같은 컨텐츠(적재 시 지워짐)를 안내문과 다른 기존 값으로 바꾼다.
const CONTENT_GUIDE: &str = r#"<hp:run charPrIDRef="6"><hp:t>여기에 입력</hp:t></hp:run>"#;
const CONTENT_EXISTING: &str = r#"<hp:run charPrIDRef="6"><hp:t>기존 값</hp:t></hp:run>"#;

fn fill_and_export(hwpx: &[u8], value: &str) -> String {
    let mut doc = rhwp::wasm_api::HwpDocument::from_bytes(hwpx).expect("parse");
    doc.set_field_value_by_name_api(FIELD, value)
        .expect("set_field_value_by_name");
    let saved = doc.export_hwpx().expect("export_hwpx");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(saved)).expect("저장본 ZIP");
    let mut xml = String::new();
    zip.by_name("Contents/section0.xml")
        .expect("section0")
        .read_to_string(&mut xml)
        .expect("section XML 은 UTF-8");
    xml
}

/// 필드가 든 문단에서 fieldBegin 이후 텍스트를 글자별 (글자, charPrIDRef) 로 펼친다.
fn chars_after_field_begin(xml: &str) -> Vec<(char, String)> {
    let anchor = xml
        .find(&format!(r#"name="{FIELD}""#))
        .expect("저장본에 필드가 있어야 한다");
    let para_end = anchor + xml[anchor..].find("</hp:p>").expect("문단 끝");
    let first_run = xml[..anchor].rfind("<hp:run ").expect("fieldBegin 런");
    let body = &xml[first_run..para_end];

    let mut out = Vec::new();
    let mut rest = body;
    while let Some(run_start) = rest.find("<hp:run ") {
        let run = &rest[run_start..];
        let shape_key = r#"charPrIDRef=""#;
        let shape_start = run.find(shape_key).expect("charPrIDRef") + shape_key.len();
        let shape_len = run[shape_start..].find('"').expect("charPrIDRef 끝");
        let shape = run[shape_start..shape_start + shape_len].to_string();
        let run_end = run.find("</hp:run>").unwrap_or(run.len());
        let mut inner = &run[..run_end];
        while let Some(t) = inner.find("<hp:t>") {
            let text_start = t + "<hp:t>".len();
            let text_len = inner[text_start..].find("</hp:t>").expect("</hp:t>");
            for c in inner[text_start..text_start + text_len].chars() {
                out.push((c, shape.clone()));
            }
            inner = &inner[text_start + text_len..];
        }
        rest = &run[run_end..];
    }
    out
}

fn assert_value_then_trailing(xml: &str, value: &str, trailing: &str) {
    let chars = chars_after_field_begin(xml);
    let text: String = chars.iter().map(|(c, _)| *c).collect();
    assert_eq!(text, format!("{value}{trailing}"), "문단 텍스트");
    let describe = || {
        chars
            .iter()
            .map(|(c, s)| format!("{c}:{s}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let value_len = value.chars().count();
    for (i, (c, shape)) in chars.iter().enumerate() {
        let expected = if i < value_len {
            CONTENT_SHAPE
        } else {
            SURROUNDING_SHAPE
        };
        assert_eq!(
            shape,
            expected,
            "{i}번째 글자 {c:?} 의 글자모양이 틀렸다 (값은 컨텐츠 {CONTENT_SHAPE}, \
             뒤 텍스트는 주변 {SURROUNDING_SHAPE} 이어야 함): {}",
            describe()
        );
    }
}

/// 적재 정규화가 안내문을 지운 빈 필드(guide_residue 경로) + 뒤따르는 텍스트.
#[test]
fn empty_field_with_trailing_text_value_gets_content_shape() {
    let hwpx = patch_section(
        &sample_bytes(),
        &[(FIELD_END_TAIL, FIELD_END_TAIL_WITH_TEXT)],
    );
    for value in [LONG_VALUE, "ABCDEFGHIJKL", "짧은값"] {
        let xml = fill_and_export(&hwpx, value);
        assert_value_then_trailing(&xml, value, TRAILING);
    }
}

/// 안내문과 다른 기존 값을 가진 필드(count>0 경로) + 뒤따르는 텍스트.
#[test]
fn existing_value_field_with_trailing_text_value_gets_content_shape() {
    let hwpx = patch_section(
        &sample_bytes(),
        &[
            (CONTENT_GUIDE, CONTENT_EXISTING),
            (FIELD_END_TAIL, FIELD_END_TAIL_WITH_TEXT),
        ],
    );
    for value in [LONG_VALUE, "ABCDEFGHIJKL", "짧은값"] {
        let xml = fill_and_export(&hwpx, value);
        assert_value_then_trailing(&xml, value, TRAILING);
    }
}

/// 무회귀: 문단 끝 누름틀(원본 샘플)은 종전대로 값 전체가 컨텐츠 서식을 받는다.
#[test]
fn field_at_paragraph_end_value_gets_content_shape() {
    for hwpx in [
        sample_bytes(),
        patch_section(&sample_bytes(), &[(CONTENT_GUIDE, CONTENT_EXISTING)]),
    ] {
        let xml = fill_and_export(&hwpx, LONG_VALUE);
        assert_value_then_trailing(&xml, LONG_VALUE, "");
    }
}
