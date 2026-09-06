//! [#3386 카브아웃] 선언 높이 0 + 콘텐츠 없는(빈 `<hp:t/>`) 행은 저장 LINE_SEG 가
//! 있어도 그 저장 줄높이를 성장 근거로 삼지 않고 접힌 채로 남아야 한다.
//!
//! 배경: `ccrlic1dno.hwpx` `#BLOCK` 표(6행×4열)의 rowAddr=4는 편집 흔적으로
//! 구조만 남기고 `cellSz height="0"`으로 접어둔 spacer 행이다 — 세 칸 모두 빈
//! `<hp:t/>`이지만 저장 `<hp:lineseg vertsize="600".../>`를 들고 있다. #3386 성장
//! 규칙은 "선언 cellSz가 저장 줄 흐름보다 작은 모순 셀은 줄 흐름 + 여백으로
//! 재성장한다"는 원칙을 갖고 있는데, 저장은 됐지만 텍스트가 전혀 없는 빈 문단의
//! lineseg를 실콘텐츠와 구분하지 않아 이 spacer 행에도 발동해 한/글의 ~0px 대신
//! ~11px로 부풀었다. 그 결과 (round-1 fix `d73db4e45`가 켠) cellzone 세로 분할선이
//! 이 여분 높이만큼 짝 없는 stub으로 아래에 남아, "학술연구용품 국내제작 곤란물품
//! 추천서" 제목 행 바로 위에 존재해서는 안 될 선 조각이 보였다.
//!
//! 이 규칙은 **두 곳에 미러링되어 있다** — `table_layout.rs`
//! `resolve_row_heights_with_common_fit`(렌더 시점, `MeasuredTable` 부재 경로)와
//! `height_measurer.rs`의 `measure_table_impl`(페이지네이션 시점,
//! `MeasuredTable.row_heights` 산출). ccrlic1dno 실측(`RHWP_DIAG_TAC=1`)은 `#BLOCK`
//! 표가 `measured_table=Some(..)`으로 렌더되는(`mt=true`) 경로를 타는 것을 보였고,
//! `trust_declared_row_heights`(±1.5px/최대(12px,15%) 허용 오차 게이트)는 다른
//! 행들과의 편차 때문에 이 표에 발동하지 않아 `MeasuredTable`의 부풀려진 값이 그대로
//! 최종 렌더에 쓰인다 — 즉 **실제 결함은 `height_measurer.rs` 쪽**이었다.
//! `table_layout.rs`만 고치면 이 값이 애초에 `measured_table`로 대체돼 무의미해진다.
//!
//! 이 테스트는 그 형상(row_span==1, cellSz height=0, 빈 문단 + 저장 lineseg)을
//! `samples/hwpx/basic-table-01.hwpx`(3행×4열, treatAsChar 아님)에 행을 하나 추가해
//! 재현한다 — colSpan=4 rowSpan=1 spacer 행 하나를 표 맨 아래에 덧붙이고, 그 칸의
//! `<hp:t>`를 비운 채 기존 lineseg(vertsize=1000 ≈ 13.3px)는 그대로 둔다
//! (`RHWP_DIAG_TAC=1`로 확인: 이 표도 `mt=true`라 `height_measurer.rs` 경로를 그대로
//! 탄다). 회귀 시 이 행은 text_height(~13.3px) + 상하 padding 만큼 부풀어 표 하단
//! 가로 경계선을 원본(spacer 행 없는 3행 표) 대비 아래로 밀어내고, 수정 후에는
//! 선언대로 ~0px로 접혀 표 하단선이 원본과 같은 y 에 남는다 — 그 y 좌표 일치로
//! 검증한다.

#![cfg(not(target_arch = "wasm32"))]

use std::io::{Read, Write};
use std::path::Path;

const SAMPLE: &str = "samples/hwpx/basic-table-01.hwpx";

fn sample_bytes() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {SAMPLE}: {e}"))
}

/// 표에 rowAddr=3, colSpan=4, cellSz height=0, 빈 `<hp:t/>` + 저장 lineseg(vertsize=1000)
/// spacer 행을 하나 덧붙인 HWPX를 만든다.
fn with_collapsed_spacer_row(bytes: &[u8]) -> Vec<u8> {
    let mut src = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).expect("zip 열기");
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut entry = src.by_index(i).expect("zip 엔트리");
        let name = entry.name().to_string();
        if name == "Contents/section0.xml" {
            let mut xml = String::new();
            entry.read_to_string(&mut xml).expect("section XML");

            let before_rowcnt = xml.clone();
            let xml = xml.replacen("rowCnt=\"3\"", "rowCnt=\"4\"", 1);
            assert_ne!(xml, before_rowcnt, "표 rowCnt=\"3\" 앵커를 찾지 못함");

            let spacer_row = r##"<hp:tr><hp:tc name="" header="0" hasMargin="0" protect="0" editable="0" dirty="0" borderFillIDRef="3"><hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="CENTER" linkListIDRef="0" linkListNextIDRef="0" textWidth="0" textHeight="0" hasTextRef="0" hasNumRef="0"><hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0"><hp:run charPrIDRef="0"><hp:t></hp:t></hp:run><hp:linesegarray><hp:lineseg textpos="0" vertpos="0" vertsize="1000" textheight="1000" baseline="850" spacing="600" horzpos="0" horzsize="9468" flags="393216"/></hp:linesegarray></hp:p></hp:subList><hp:cellAddr colAddr="0" rowAddr="3"/><hp:cellSpan colSpan="4" rowSpan="1"/><hp:cellSz width="41952" height="0"/><hp:cellMargin left="510" right="510" top="141" bottom="141"/></hp:tc></hp:tr>"##;
            let before_insert = xml.clone();
            let xml = xml.replacen("</hp:tbl>", &format!("{spacer_row}</hp:tbl>"), 1);
            assert_ne!(xml, before_insert, "표 종료 태그 앵커를 찾지 못함");

            out.start_file(name, zip::write::SimpleFileOptions::default())
                .expect("start_file");
            out.write_all(xml.as_bytes()).expect("write section");
        } else {
            out.raw_copy_file(entry).expect("raw copy");
        }
    }
    out.finish().expect("finish").into_inner()
}

fn line_attr(tag: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn line_f64(tag: &str, key: &str) -> Option<f64> {
    line_attr(tag, key)?.parse().ok()
}

/// 표의 가로 경계선(검정, 길이 > 1px) y좌표를 오름차순 정렬·중복 제거해 돌려준다.
fn horizontal_border_ys(svg: &str) -> Vec<f64> {
    let mut ys: Vec<f64> = svg
        .split("<line")
        .skip(1)
        .filter_map(|part| {
            let tag = part.split('>').next()?;
            let (x1, y1, x2, y2) = (
                line_f64(tag, "x1")?,
                line_f64(tag, "y1")?,
                line_f64(tag, "x2")?,
                line_f64(tag, "y2")?,
            );
            if (y1 - y2).abs() < 0.5 && (x2 - x1).abs() > 1.0 {
                Some(y1)
            } else {
                None
            }
        })
        .collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.3);
    ys
}

#[test]
fn declared_zero_height_empty_paragraph_row_collapses_to_zero() {
    // 기준선: spacer 행을 붙이기 전 원본 표(3행)의 표 하단 가로선 y — 이 표는
    // treatAsChar 도 depth>0 도 아니지만(diag 확인: mt=true) MeasuredTable 경로를
    // 통해서도 이 y 는 변하지 않아야 한다("선언 0 + 빈 콘텐츠" 행이 진짜로 접히면
    // 4번째 행이 표 총 높이에 아무것도 더하지 않으므로).
    let baseline_bytes = sample_bytes();
    let baseline_doc =
        rhwp::wasm_api::HwpDocument::from_bytes(&baseline_bytes).expect("parse 원본 HWPX");
    let baseline_svg = baseline_doc
        .render_page_svg_native(0)
        .expect("SVG 렌더(원본)");
    let baseline_ys = horizontal_border_ys(&baseline_svg);
    assert!(
        baseline_ys.len() >= 2,
        "원본 표 가로 경계선이 최소 2개 필요 — 전제 자체가 비정상. ys={baseline_ys:?}"
    );
    let baseline_bottom = *baseline_ys.last().unwrap();

    let hwpx = with_collapsed_spacer_row(&baseline_bytes);
    let doc = rhwp::wasm_api::HwpDocument::from_bytes(&hwpx).expect("parse 합성 HWPX");
    let svg = doc
        .render_page_svg_native(0)
        .expect("SVG 렌더(spacer 행 추가)");
    let ys = horizontal_border_ys(&svg);
    assert!(
        ys.len() >= 2,
        "표 가로 경계선이 최소 2개 필요 — spacer 행 추가 후 표 자체가 비정상. ys={ys:?}"
    );
    let patched_bottom = *ys.last().unwrap();

    // spacer 행(rowAddr=3, cellSz height=0, 빈 `<hp:t/>` + 저장 lineseg)이 선언대로
    // 접히면 표 하단 경계선은 원본 3행 표와 같은 y 에 남는다(원본 대비 라인 개수도
    // 늘지 않음 — row2/row3 경계와 표 하단이 같은 y 로 합쳐진다). 카브아웃이 빠지면
    // 저장 lineseg(vertsize=1000 ≈ 13.3px)+padding 만큼(≈17px) 표 하단이 내려간다.
    let delta = patched_bottom - baseline_bottom;
    assert!(
        delta.abs() < 1.5,
        "선언 높이 0 + 빈 문단 spacer 행이 접히지 않고 표 하단을 밀어냈다 — #3386 카브아웃 \
         회귀. 원본 표 하단 y={baseline_bottom:.2}, spacer 행 추가 후 y={patched_bottom:.2} \
         (delta={delta:.2}px, 기대: <1.5px, 회귀 시 저장 lineseg 13.3px + padding ≈ 17px \
         근방). baseline_ys={baseline_ys:?} ys={ys:?}"
    );
    assert_eq!(
        ys.len(),
        baseline_ys.len(),
        "spacer 행이 진짜로 접혔다면 가로 경계선 개수는 원본과 같아야 한다(row2/row3 \
         경계와 표 하단이 합쳐짐) — 개수가 늘었다면 spacer 행이 별도 높이를 차지한 것.\
         baseline_ys={baseline_ys:?} ys={ys:?}"
    );
}
