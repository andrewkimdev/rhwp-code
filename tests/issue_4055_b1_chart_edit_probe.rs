//! [Issue #4055] B1 스파이크 Stage 1 — 레거시 `Contents` 값 로케이터 실현성 프로브.
//!
//! #3683 Track B 착수 순서 3번("레거시 `Contents` f64 in-place 패치의 실현성 확인")의
//! 실측 하네스다. **프로덕션 동작을 바꾸지 않는다** — `src/` 를 건드리지 않고,
//! 기존 파서 자산만 읽기로 사용한다.
//!
//! ## 왜 새 로케이터가 필요한가
//!
//! 현행 `src/ole_chart/parser.rs` 의 `is_plausible_grid_value` 는 **정수·1 이상·100만
//! 이하**만 값으로 인정한다. 그래서 코퍼스의 실제 값(`4.3`, `2.5`, `1.8` …)을 놓치고
//! 개수 불일치로 파싱 전체가 실패한다. 렌더에서는 OOXML 경로가 먼저 이기므로
//! (`renderer/layout/shape_layout.rs`) 지금껏 드러나지 않았다.
//!
//! 편집을 하려면 값이 아니라 **바이트 오프셋**을 알아야 하고, 값 휴리스틱은
//! 사용자가 값을 `0`·음수·소수로 바꾸는 순간 무너진다. 그래서 이 프로브는
//! 값을 보지 않고 **구조만 보는** 로케이터를 쓴다.
//!
//! ## 구조 (코퍼스 실측으로 확정)
//!
//! `VtDataGrid` 구간 안에서 각 셀은 26바이트이고, f64 값 **바로 뒤**에 언제나
//! 같은 6바이트 트레일러가 붙는다.
//!
//! ```text
//! ... 00 "VtDouble\0" 01 00 | <f64 8B> | FF FF 06 00 00 00     ← 첫 셀
//! ... 04 00 00 00 NN 00 00 00 07 00 00 00 | <f64 8B> | FF FF 06 00 00 00
//! ```
//!
//! 따라서 트레일러를 찾아 그 **직전 8바이트**를 읽으면 값이고, 그 위치가 곧
//! in-place 패치 대상 오프셋이다.

use std::path::{Path, PathBuf};

use rhwp::ooxml_chart::OoxmlChart;
use rhwp::parser::ole_container::parse_ole_container;

/// 레거시 그리드에서 f64 값 뒤에 항상 붙는 트레일러.
const VALUE_TRAILER: &[u8] = &[0xFF, 0xFF, 0x06, 0x00, 0x00, 0x00];
const GRID_MARKER: &[u8] = b"VtDataGrid\0";
const DOUBLE_MARKER: &[u8] = b"VtDouble\0";

/// `VtDataGrid` 다음에 오는 형제 오브젝트 마커들 — 그리드 구간의 끝을 정한다.
/// `src/ole_chart/parser.rs` 의 `find_next_legacy_object_marker` 와 같은 목록이다.
const OBJECT_MARKERS: &[&[u8]] = &[
    b"VtBackdrop\0",
    b"VtBackDrop\0",
    b"VtChartSection\0",
    b"VtFootnote\0",
    b"VtLegend\0",
    b"VtPlot\0",
    b"VtPrintInformation\0",
    b"VtChartTitle\0",
    b"VtTitle\0",
];

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() || needle.len() > haystack.len() - from {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

/// `VtDataGrid` 데이터 구간 `[start, end)`.
///
/// 시작은 마커 + `StoredName` NUL + `StoredVersion`(4B) 뒤다
/// (`legacy_chart_object_data_start` 와 같은 규약).
fn grid_window(contents: &[u8]) -> Option<(usize, usize)> {
    let marker = find_from(contents, GRID_MARKER, 0)?;
    let start = marker + GRID_MARKER.len() + 4;
    if start > contents.len() {
        return None;
    }
    let end = OBJECT_MARKERS
        .iter()
        .filter_map(|m| find_from(contents, m, start))
        .min()
        .unwrap_or(contents.len());
    Some((start, end))
}

/// 그리드 값 셀의 `(바이트 오프셋, 값)` 목록.
///
/// **값의 크기·부호·정수 여부를 보지 않는다.** 오직 트레일러 위치로만 찾는다.
fn locate_grid_values(contents: &[u8]) -> Vec<(usize, f64)> {
    let Some((start, end)) = grid_window(contents) else {
        return Vec::new();
    };
    // 첫 값은 `VtDouble` 선언 뒤에 온다. 그 앞의 트레일러 유사 바이트는 값이 아니다.
    let Some(anchor) = find_from(&contents[..end], DOUBLE_MARKER, start) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut cursor = anchor;
    while let Some(hit) = find_from(&contents[..end], VALUE_TRAILER, cursor + 1) {
        cursor = hit;
        let Some(value_at) = hit.checked_sub(8) else {
            continue;
        };
        if value_at < anchor {
            continue;
        }
        let bytes: [u8; 8] = contents[value_at..hit].try_into().expect("8바이트");
        out.push((value_at, f64::from_le_bytes(bytes)));
    }
    out
}

/// 문서에서 (레거시 `Contents`, OOXML 차트 XML) 을 꺼낸다.
///
/// HWPX 는 `Chart/chartN.xml` 이 `bin_data_content`(id=60000+N, `ooxml_chart`)로,
/// HWP5 는 중첩 CFB 의 `OOXMLChartContents` 로 들어온다. 레거시 `Contents` 는
/// 양쪽 모두 중첩 CFB 안에 있다.
fn chart_streams(doc_bytes: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let doc = rhwp::parse_document(doc_bytes).ok()?;

    let mut legacy = None;
    let mut ooxml = None;

    for content in &doc.bin_data_content {
        if content.extension == "ooxml_chart" {
            ooxml.get_or_insert_with(|| content.data.load());
            continue;
        }
        let bytes = content.data.load();
        if !bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
            continue;
        }
        if let Some(container) = parse_ole_container(&bytes) {
            if let Some(raw) = container.raw_contents {
                legacy.get_or_insert(raw);
            }
            if let Some(xml) = container.ooxml_chart {
                ooxml.get_or_insert(xml);
            }
        }
    }

    Some((legacy?, ooxml?))
}

/// 그리드 값이 놓이는 두 가지 순서.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    /// 한 행 = 한 계열 (계열의 값들이 연속)
    SeriesMajor,
    /// 한 행 = 한 카테고리 (같은 카테고리의 계열별 값이 연속)
    CategoryMajor,
    /// 값이 하나뿐이라 두 순서가 구분되지 않음
    Degenerate,
}

fn classify(found: &[f64], series: &[Vec<f64>]) -> Option<Orientation> {
    let series_major: Vec<f64> = series.iter().flatten().copied().collect();
    if series_major.len() == 1 && found == series_major.as_slice() {
        return Some(Orientation::Degenerate);
    }
    let cols = series.first()?.len();
    let category_major: Vec<f64> = (0..cols)
        .flat_map(|c| series.iter().map(move |s| s[c]))
        .collect();

    if found == series_major.as_slice() {
        Some(Orientation::SeriesMajor)
    } else if found == category_major.as_slice() {
        Some(Orientation::CategoryMajor)
    } else {
        None
    }
}

/// 정답지 — OOXML `c:val`(분산형은 `c:yVal`)의 계열별 값.
fn ground_truth(ooxml: &[u8]) -> Option<Vec<Vec<f64>>> {
    let chart = OoxmlChart::parse(ooxml)?;
    let series: Vec<Vec<f64>> = chart
        .series
        .iter()
        .map(|s| s.values.clone())
        .filter(|v| !v.is_empty())
        .collect();
    if series.is_empty() {
        return None;
    }
    // 계열별 길이가 다르면 직사각 그리드가 아니므로 이 프로브의 대상이 아니다.
    let cols = series[0].len();
    if series.iter().any(|s| s.len() != cols) {
        return None;
    }
    Some(series)
}

fn chart_samples(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("samples/chart 읽기") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            chart_samples(&path, out);
        } else if path.extension().is_some_and(|e| e == "hwpx") {
            out.push(path);
        }
    }
}

fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/chart");
    let mut out = Vec::new();
    chart_samples(&root, &mut out);
    out.sort();
    assert!(!out.is_empty(), "samples/chart 코퍼스가 비어 있다");
    out
}

/// S1 — 구조 기반 로케이터가 코퍼스 전건에서 정답지와 일치하는가.
///
/// `.hwpx` 와 `.hwp` 는 같은 차트라도 바이트가 다르므로(한컴이 각각 따로 저장)
/// 양쪽을 독립적으로 판정한다.
#[test]
fn legacy_grid_locator_matches_ooxml_ground_truth_across_corpus() {
    let mut checked = 0usize;
    let mut failures = Vec::new();

    for hwpx in corpus() {
        let hwp = hwpx.with_extension("hwp");
        for path in [&hwpx, &hwp] {
            let label = path
                .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                .unwrap_or(path)
                .display()
                .to_string();

            let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{label} 읽기: {e}"));
            let Some((legacy, ooxml)) = chart_streams(&bytes) else {
                failures.push(format!("{label}: 차트 스트림 추출 실패"));
                continue;
            };
            let Some(series) = ground_truth(&ooxml) else {
                failures.push(format!("{label}: OOXML 정답지 추출 실패"));
                continue;
            };

            let located = locate_grid_values(&legacy);
            let values: Vec<f64> = located.iter().map(|(_, v)| *v).collect();
            let expected: usize = series.iter().map(Vec::len).sum();

            if values.len() != expected {
                failures.push(format!(
                    "{label}: 값 개수 {} != 정답지 {expected}",
                    values.len()
                ));
                continue;
            }
            if classify(&values, &series).is_none() {
                failures.push(format!(
                    "{label}: 값이 어느 순서와도 불일치 — 실측 {values:?} / 정답지 {series:?}"
                ));
                continue;
            }

            // 패치 대상 오프셋이 실제로 그 값을 담고 있는지 되읽어 확인한다.
            for (offset, value) in &located {
                let round_trip =
                    f64::from_le_bytes(legacy[*offset..*offset + 8].try_into().expect("8바이트"));
                assert_eq!(round_trip, *value, "{label}: 오프셋 {offset} 재독 불일치");
            }
            checked += 1;
        }
    }

    assert!(
        failures.is_empty(),
        "구조 기반 로케이터 실패 {}건:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(checked, 56, "코퍼스 28종 × 2포맷을 전건 검사해야 한다");
}

/// S1 곁가지 — 그리드 순서는 문서마다 다르다. 편집기가 `(계열, 카테고리)` 를
/// 셀로 옮기려면 순서를 **판정해야 하고 가정하면 안 된다**는 근거를 고정한다.
#[test]
fn legacy_grid_orientation_is_not_fixed() {
    let modern =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/chart/세로막대형/묶은세로막대형.hwp");
    let bytes = std::fs::read(&modern).expect("모던 샘플 읽기");
    let (legacy, ooxml) = chart_streams(&bytes).expect("차트 스트림");
    let series = ground_truth(&ooxml).expect("정답지");
    let values: Vec<f64> = locate_grid_values(&legacy)
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    assert_eq!(
        classify(&values, &series),
        Some(Orientation::SeriesMajor),
        "모던 코퍼스는 계열-major 로 저장된다"
    );

    // 레거시 `Contents` 단독 문서는 카테고리-major 다.
    let control = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/143E433F503322BD33.hwp");
    let bytes = std::fs::read(&control).expect("대조군 읽기");
    let doc = rhwp::parse_document(&bytes).expect("대조군 파싱");
    let ole = doc
        .bin_data_content
        .iter()
        .find(|c| c.extension == "OLE")
        .expect("대조군 OLE");
    let container = parse_ole_container(&ole.data.load()).expect("중첩 컨테이너");
    assert!(
        container.ooxml_chart.is_none(),
        "대조군은 OOXML 사본이 없는 레거시 단독 문서다"
    );
    let legacy = container.raw_contents.expect("레거시 Contents");
    let values: Vec<f64> = locate_grid_values(&legacy)
        .into_iter()
        .map(|(_, v)| v)
        .collect();

    // #1251 이 고정한 정답: 적립금·수입·지출 3계열 × 4카테고리.
    let series = vec![
        vec![328.0, 812.0, 1702.0, 1477.0],
        vec![50.0, 70.0, 189.0, 191.0],
        vec![11.0, 15.0, 201.0, 289.0],
    ];
    assert_eq!(values.len(), 12, "대조군은 값 12개다");
    assert_eq!(
        classify(&values, &series),
        Some(Orientation::CategoryMajor),
        "대조군은 카테고리-major 로 저장된다 — 순서는 고정이 아니다"
    );
}

/// 로케이터를 `VtDataGrid` 구간으로 묶지 않으면 그리드 밖 `VtDouble` 까지
/// 주워 온다는 것을 고정한다. 창 밖 값을 패치하면 축 눈금 같은 무관한 속성을
/// 덮어쓰게 된다.
#[test]
fn locator_must_be_bounded_to_the_data_grid_window() {
    let control = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/143E433F503322BD33.hwp");
    let bytes = std::fs::read(&control).expect("대조군 읽기");
    let doc = rhwp::parse_document(&bytes).expect("대조군 파싱");
    let ole = doc
        .bin_data_content
        .iter()
        .find(|c| c.extension == "OLE")
        .expect("대조군 OLE");
    let legacy = parse_ole_container(&ole.data.load())
        .expect("중첩 컨테이너")
        .raw_contents
        .expect("레거시 Contents");

    let bounded = locate_grid_values(&legacy).len();

    // 구간 제한 없이 스트림 전체를 훑으면 값이 더 붙는다.
    let anchor = find_from(&legacy, DOUBLE_MARKER, 0).expect("VtDouble 앵커");
    let mut unbounded = 0usize;
    let mut cursor = anchor;
    while let Some(hit) = find_from(&legacy, VALUE_TRAILER, cursor + 1) {
        cursor = hit;
        if hit >= anchor + 8 {
            unbounded += 1;
        }
    }

    assert_eq!(bounded, 12, "그리드 구간 안에서는 12개다");
    assert!(
        unbounded > bounded,
        "구간 제한이 없으면 그리드 밖 VtDouble 까지 잡힌다 (제한 {bounded} / 무제한 {unbounded})"
    );
}
