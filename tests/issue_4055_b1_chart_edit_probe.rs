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

// ─────────────────────────────────────────────────────────────────────────────
// Stage 2·3 — 변이 생성기와 중첩 CFB 재포장 (S2·S3 재료, S4)
// ─────────────────────────────────────────────────────────────────────────────

use std::io::{Read as _, Write as _};

/// sentinel — 원본 최대값이 `5` 라 `91.7` 이면 첫 막대가 차트를 뚫고 솟는다.
const SENTINEL: f64 = 91.7;
const SENTINEL_TEXT: &str = "91.7";
const EMF_STREAM: &str = "/\u{2}OlePres000";

/// 기준 샘플 — 한컴 육안 판정용 변종의 원본.
const BASE_SAMPLE: &str = "samples/chart/세로막대형/묶은세로막대형";

fn manifest(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// 중첩 CFB 의 **모든** 스트림을 열거한다.
///
/// `parse_ole_container` 는 아는 4종만 뽑으므로 재포장에 쓸 수 없다 — 나머지가
/// 소실된다. S4 는 "그 4종 밖에 무엇이 있는가"를 세는 것이기도 하다.
fn all_streams(cfb_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut comp =
        cfb::CompoundFile::open(std::io::Cursor::new(cfb_bytes.to_vec())).expect("CFB 열기");
    let paths: Vec<std::path::PathBuf> = comp
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_path_buf())
        .collect();
    let mut out = Vec::new();
    for path in paths {
        let mut buf = Vec::new();
        comp.open_stream(&path)
            .expect("스트림 열기")
            .read_to_end(&mut buf)
            .expect("스트림 읽기");
        // Windows 에서 `cfb` 는 `/BinData\BIN0001.OLE` 처럼 구분자를 섞어 돌려준다.
        // `mini_cfb::build_entries` 는 `/` 로만 쪼개므로, 정규화하지 않으면 스토리지가
        // 사라지고 `BinData\BIN0001.OLE` 라는 루트 스트림 하나로 뭉개진다.
        out.push((path.to_string_lossy().replace('\\', "/"), buf));
    }
    out
}

/// 스트림 목록으로 CFB 를 다시 만든다. `mini_cfb` 를 쓰는 이유는
/// `cfb::CompoundFile::create()` 가 `SystemTime::now()` 로 wasm32 에서 panic 하기
/// 때문이다 (`src/parser/hwp3/ole.rs` 선례와 같다).
fn rebuild_cfb(streams: &[(String, Vec<u8>)]) -> Vec<u8> {
    let refs: Vec<(&str, &[u8])> = streams
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    rhwp::serializer::mini_cfb::build_cfb(&refs).expect("CFB 재포장")
}

/// CFB 루트 디렉터리 엔트리의 바이트 오프셋. 헤더의 `_uSectorShift`(0x1E)와
/// `_sectDirStart`(0x30)로 첫 디렉터리 섹터를 구하고, 루트는 그 섹터의 첫 엔트리다.
fn root_dir_entry_offset(cfb: &[u8]) -> usize {
    let sector_shift = u16::from_le_bytes([cfb[0x1E], cfb[0x1F]]);
    let sector_size = 1usize << sector_shift;
    let dir_start = u32::from_le_bytes(cfb[0x30..0x34].try_into().expect("4바이트")) as usize;
    (dir_start + 1) * sector_size
}

/// 루트 스토리지의 CLSID (디렉터리 엔트리 +80, 16바이트).
fn root_clsid(cfb: &[u8]) -> [u8; 16] {
    let at = root_dir_entry_offset(cfb) + 80;
    cfb[at..at + 16].try_into().expect("16바이트 CLSID")
}

/// 재포장본 루트에 CLSID 를 되박는다.
///
/// `mini_cfb::build_cfb` 는 CLSID 를 0 으로 고정한다
/// (`src/serializer/mini_cfb.rs:422,513` — "CLSID (16바이트 zero)"). 그런데 OLE 개체는
/// **루트 CLSID 로 서버를 식별**한다. 이 코퍼스의 차트는
/// `{4C3DA137-DC90-47B9-9BED-59DAE352A280}` 를 달고 있고, 그게 비면 한컴은 개체를
/// 알아보지 못해 **틀과 선택 핸들만 그리고 내용을 비운다**(2026-08-05 한컴 실측).
/// rhwp 는 스트림 이름으로 판별하므로 이 손실을 눈치채지 못한다.
fn stamp_root_clsid(cfb: &mut [u8], clsid: [u8; 16]) {
    let at = root_dir_entry_offset(cfb) + 80;
    cfb[at..at + 16].copy_from_slice(&clsid);
}

/// 원본과 같은 루트 CLSID 를 유지하며 재포장한다. 중첩 OLE CFB 는 반드시 이 쪽을 쓴다.
fn rebuild_cfb_preserving_clsid(original: &[u8], streams: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut rebuilt = rebuild_cfb(streams);
    stamp_root_clsid(&mut rebuilt, root_clsid(original));
    rebuilt
}

/// OOXML 의 **첫 `c:val` 안 첫 `c:v`** 텍스트만 바꾼다.
///
/// 전체 재직렬화를 하지 않는 이유: `src/ooxml_chart/parser.rs` 는 `c:pt idx`·
/// `c:f`·`c:externalData`·`extLst` 를 읽지 않는다. 파싱→재방출로 왕복시키면
/// 모델에 없는 것이 전부 사라진다. 바이트 수술이 최소 diff 다.
fn patch_ooxml_first_value(xml: &[u8], sentinel: &str) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(xml).ok()?;
    let val_at = text.find("<c:val>")?;
    let v_open = text[val_at..].find("<c:v>")? + val_at + "<c:v>".len();
    let v_close = text[v_open..].find("</c:v>")? + v_open;
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(&text[..v_open]);
    out.push_str(sentinel);
    out.push_str(&text[v_close..]);
    Some(out.into_bytes())
}

/// 레거시 `Contents` 의 **첫 그리드 값**을 8바이트 제자리 덮어쓰기 한다.
fn patch_legacy_first_value(contents: &[u8], sentinel: f64) -> Option<Vec<u8>> {
    let (offset, _) = *locate_grid_values(contents).first()?;
    let mut out = contents.to_vec();
    out[offset..offset + 8].copy_from_slice(&sentinel.to_le_bytes());
    Some(out)
}

/// HWPX zip 을 엔트리 단위로 다시 쓴다. 손대지 않는 엔트리는 `raw_copy_file` 로
/// 압축 방식까지 보존한다 (`mimetype` 은 stored 여야 한다).
fn rewrite_hwpx(original: &[u8], replacements: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut src =
        zip::ZipArchive::new(std::io::Cursor::new(original.to_vec())).expect("원본 HWPX 열기");
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let entry = src.by_index(i).expect("zip 엔트리");
        let name = entry.name().to_string();
        match replacements.iter().find(|(n, _)| *n == name) {
            Some((_, bytes)) => {
                out.start_file(&name, zip::write::SimpleFileOptions::default())
                    .expect("start_file");
                out.write_all(bytes).expect("엔트리 쓰기");
            }
            None => {
                out.raw_copy_file(entry).expect("raw copy");
            }
        }
    }
    out.finish().expect("zip finish").into_inner()
}

/// HWP5 바깥 CFB 를 다시 쓴다. 지정한 스트림만 갈고 나머지는 바이트 그대로 옮긴다.
fn rewrite_hwp(original: &[u8], replacements: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut streams = all_streams(original);
    for (name, bytes) in replacements {
        let slot = streams
            .iter_mut()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("교체 대상 스트림 {name} 없음"));
        slot.1 = bytes.clone();
    }
    rebuild_cfb(&streams)
}

/// HWP5 `BinData` OLE Storage 스트림에서 중첩 CFB 를 꺼낸다.
fn hwp_nested_cfb(stream: &[u8]) -> Vec<u8> {
    let inflated = rhwp::parser::cfb_reader::decompress_stream(stream).expect("BinData 압축 해제");
    inflated[4..].to_vec()
}

/// 중첩 CFB 를 HWP5 `BinData` OLE Storage 스트림으로 되싼다 — 4바이트 LE size
/// prefix 를 붙이고 raw deflate 로 압축한다
/// (`src/serializer/cfb_writer.rs:232-255` 와 같은 규약).
fn hwp_ole_stream(nested_cfb: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(nested_cfb.len() + 4);
    payload.extend_from_slice(&(nested_cfb.len() as u32).to_le_bytes());
    payload.extend_from_slice(nested_cfb);

    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&payload).expect("deflate 쓰기");
    encoder.finish().expect("deflate 완료")
}

/// 한 파일 안 각 표현의 **첫 값**과 EMF 유무. 변종이 라벨대로 조립됐는지 검증한다.
#[derive(Debug, PartialEq)]
struct Representations {
    /// ① HWPX `Chart/chartN.xml` — HWP5 에는 없다.
    zip_part: Option<f64>,
    /// ② 중첩 CFB `OOXMLChartContents`
    nested_ooxml: Option<f64>,
    /// ③ 레거시 `Contents`
    legacy: Option<f64>,
    /// ④ `\x02OlePres000` EMF 프리뷰
    has_emf: bool,
}

fn representations(doc_bytes: &[u8]) -> Representations {
    let doc = rhwp::parse_document(doc_bytes).expect("문서 파싱");
    let mut out = Representations {
        zip_part: None,
        nested_ooxml: None,
        legacy: None,
        has_emf: false,
    };
    for content in &doc.bin_data_content {
        let bytes = content.data.load();
        if content.extension == "ooxml_chart" {
            out.zip_part = ground_truth(&bytes).map(|g| g[0][0]);
            continue;
        }
        if !bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
            continue;
        }
        if let Some(container) = parse_ole_container(&bytes) {
            if let Some(xml) = &container.ooxml_chart {
                out.nested_ooxml = ground_truth(xml).map(|g| g[0][0]);
            }
            if let Some(raw) = &container.raw_contents {
                out.legacy = locate_grid_values(raw).first().map(|(_, v)| *v);
            }
            out.has_emf = container.preview_emf.is_some() || container.preview_wmf.is_some();
        }
    }
    out
}

/// 중첩 CFB 안에서 OOXML 사본·레거시를 패치하고 EMF 를 뺄지 정한다.
fn mutate_nested(nested: &[u8], patch_ooxml: bool, patch_legacy: bool, drop_emf: bool) -> Vec<u8> {
    let mut streams = all_streams(nested);
    streams.retain(|(name, _)| !(drop_emf && name == EMF_STREAM));
    for (name, data) in streams.iter_mut() {
        if patch_ooxml && name == "/OOXMLChartContents" {
            *data = patch_ooxml_first_value(data, SENTINEL_TEXT).expect("중첩 OOXML 패치");
        }
        if patch_legacy && name == "/Contents" {
            *data = patch_legacy_first_value(data, SENTINEL).expect("레거시 패치");
        }
    }
    rebuild_cfb_preserving_clsid(nested, &streams)
}

/// S4 — 중첩 CFB 를 스트림 단위로 열거해 그대로 다시 싸면 내용이 보존되는가.
///
/// 바이트 동일성이 아니라 **스트림 집합과 각 스트림 바이트의 동일성**을 본다.
/// CFB 컨테이너 레이아웃(섹터 배치)은 빌더마다 달라도 무방하다.
#[test]
fn nested_cfb_repack_preserves_every_stream() {
    // parse_ole_container 가 아는 스트림 — 이 밖의 것이 있으면 그대로 재포장할 때 소실된다.
    let known = [
        "/Contents",
        "/OOXMLChartContents",
        EMF_STREAM,
        "/\u{1}Ole10Native",
    ];

    let mut checked = 0usize;
    let mut unknown_seen: Vec<String> = Vec::new();

    for hwpx in corpus() {
        let bytes = std::fs::read(&hwpx).expect("HWPX 읽기");
        let doc = rhwp::parse_document(&bytes).expect("HWPX 파싱");
        let Some(nested) = doc
            .bin_data_content
            .iter()
            .map(|c| c.data.load())
            .find(|b| b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]))
        else {
            continue;
        };

        let before = all_streams(&nested);
        for (name, _) in &before {
            if !known.contains(&name.as_str()) && !unknown_seen.contains(name) {
                unknown_seen.push(name.clone());
            }
        }

        let rebuilt = rebuild_cfb_preserving_clsid(&nested, &before);
        assert_eq!(
            root_clsid(&rebuilt),
            root_clsid(&nested),
            "{}: 재포장이 OLE 클래스 ID 를 보존해야 한다",
            hwpx.display()
        );
        let after = all_streams(&rebuilt);
        assert_eq!(
            before.len(),
            after.len(),
            "{}: 재포장 전후 스트림 개수가 같아야 한다",
            hwpx.display()
        );
        for (name, data) in &before {
            let found = after
                .iter()
                .find(|(n, _)| n == name)
                .unwrap_or_else(|| panic!("{}: 재포장본에 {name} 없음", hwpx.display()));
            assert_eq!(
                &found.1,
                data,
                "{}: {name} 바이트가 달라졌다",
                hwpx.display()
            );
        }

        // 재포장본이 프로덕션 소비 경로를 그대로 탄다.
        let container = parse_ole_container(&rebuilt).expect("재포장본 컨테이너 파싱");
        assert!(container.ooxml_chart.is_some(), "재포장본 OOXML 유지");
        assert!(container.raw_contents.is_some(), "재포장본 레거시 유지");
        assert!(container.preview_emf.is_some(), "재포장본 EMF 프리뷰 유지");
        checked += 1;
    }

    assert_eq!(checked, 28, "코퍼스 28종을 전건 재포장 검사해야 한다");
    assert!(
        unknown_seen.is_empty(),
        "parse_ole_container 가 모르는 스트림이 코퍼스에 있다 — 4종만 뽑아 재포장하면 소실된다: {unknown_seen:?}"
    );
}

/// `mini_cfb::build_cfb` 가 루트 CLSID 를 잃는다는 사실을 못박는다.
///
/// **B1 본구현의 선결 과제다.** 중첩 OLE CFB 를 재포장하는 순간 OLE 서버 식별자가
/// 사라지고, 한컴은 개체를 알아보지 못해 틀만 그리고 내용을 비운다(2026-08-05 실측).
/// rhwp 는 `parse_ole_container` 가 스트림 이름으로 판별하므로 이 손실을 감지하지
/// 못한다 — 자체 왕복 검증으로는 절대 안 잡히는 종류다.
#[test]
fn mini_cfb_repack_drops_the_ole_class_id() {
    let hwpx = std::fs::read(manifest(&format!("{BASE_SAMPLE}.hwpx"))).expect("HWPX 읽기");
    let doc = rhwp::parse_document(&hwpx).expect("HWPX 파싱");
    let nested = doc
        .bin_data_content
        .iter()
        .map(|c| c.data.load())
        .find(|b| b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]))
        .expect("중첩 CFB");

    let original = root_clsid(&nested);
    assert_ne!(
        original, [0u8; 16],
        "코퍼스 차트는 OLE 클래스 ID 를 달고 있다"
    );

    let naive = rebuild_cfb(&all_streams(&nested));
    assert_eq!(
        root_clsid(&naive),
        [0u8; 16],
        "mini_cfb 는 CLSID 를 0 으로 고정한다 (mini_cfb.rs:422,513)"
    );

    let preserved = rebuild_cfb_preserving_clsid(&nested, &all_streams(&nested));
    assert_eq!(
        root_clsid(&preserved),
        original,
        "되박으면 원본 CLSID 가 유지된다"
    );
}

/// 두 패치가 **같은 논리 셀**을 겨냥하는지 확인한다. 그래야 변종 사이 비교가
/// 성립한다.
#[test]
fn both_patches_target_the_same_logical_cell() {
    let bytes = std::fs::read(manifest(&format!("{BASE_SAMPLE}.hwpx"))).expect("HWPX 읽기");
    let (legacy, ooxml) = chart_streams(&bytes).expect("차트 스트림");

    let legacy_first = locate_grid_values(&legacy).first().expect("레거시 첫 값").1;
    let ooxml_first = ground_truth(&ooxml).expect("정답지")[0][0];
    assert_eq!(
        legacy_first, ooxml_first,
        "레거시 첫 그리드 값과 OOXML 첫 값이 같은 셀이어야 한다"
    );

    let patched_legacy = patch_legacy_first_value(&legacy, SENTINEL).expect("레거시 패치");
    assert_eq!(
        patched_legacy.len(),
        legacy.len(),
        "레거시 패치는 길이 불변이어야 한다"
    );
    let changed = legacy
        .iter()
        .zip(&patched_legacy)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed <= 8,
        "레거시 패치가 8바이트를 넘게 바꿨다: {changed}"
    );

    let before: Vec<f64> = locate_grid_values(&legacy)
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    let after: Vec<f64> = locate_grid_values(&patched_legacy)
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    assert_eq!(after[0], SENTINEL, "첫 값이 sentinel 로 바뀌어야 한다");
    assert_eq!(after[1..], before[1..], "나머지 값은 그대로여야 한다");

    let patched_ooxml = patch_ooxml_first_value(&ooxml, SENTINEL_TEXT).expect("OOXML 패치");
    let gt = ground_truth(&patched_ooxml).expect("패치본 정답지");
    assert_eq!(gt[0][0], SENTINEL, "OOXML 첫 값이 sentinel 이어야 한다");
    assert_eq!(
        gt[0][1..],
        ground_truth(&ooxml).expect("원본 정답지")[0][1..],
        "OOXML 나머지 값은 그대로여야 한다"
    );
}

/// 편집한 바이트가 rhwp 자체 저장에서 살아나는지 — 한컴에 넘기기 전 자기 검증.
///
/// `bin_data_content` 의 `ooxml_chart` 항목만 갈아끼우면 저장기가 그 바이트를
/// `Chart/chartN.xml` 로 무가공 방출하므로(`serializer/hwpx/mod.rs:164-180`)
/// 직렬화기를 고치지 않고도 편집이 저장된다. 동시에 중첩 CFB 안의 사본은
/// 손대지 않으므로 **4중 표현이 갈리는 지점**이 그대로 드러난다.
#[test]
fn editing_only_the_zip_part_diverges_from_the_nested_copy() {
    use rhwp::document_core::DocumentCore;

    let bytes = std::fs::read(manifest(&format!("{BASE_SAMPLE}.hwpx"))).expect("HWPX 읽기");
    let mut core = DocumentCore::from_bytes(&bytes).expect("문서 로드");
    {
        let doc = core.document_mut();
        let slot = doc
            .bin_data_content
            .iter_mut()
            .find(|c| c.extension == "ooxml_chart")
            .expect("ooxml_chart 항목");
        let patched = patch_ooxml_first_value(&slot.data.load(), SENTINEL_TEXT).expect("패치");
        slot.data = patched.into();
    }
    let saved = core.export_hwpx_native().expect("HWPX 저장");

    let reparsed = rhwp::parse_document(&saved).expect("저장본 재파싱");
    let zip_part = reparsed
        .bin_data_content
        .iter()
        .find(|c| c.extension == "ooxml_chart")
        .expect("저장본 ooxml_chart");
    assert_eq!(
        ground_truth(&zip_part.data.load()).expect("zip 파트 정답지")[0][0],
        SENTINEL,
        "편집한 값이 Chart/chartN.xml 로 저장돼야 한다"
    );

    let nested_ooxml = reparsed
        .bin_data_content
        .iter()
        .filter(|c| c.extension != "ooxml_chart")
        .filter_map(|c| parse_ole_container(&c.data.load()))
        .find_map(|c| c.ooxml_chart)
        .expect("중첩 OOXML 사본");
    assert_eq!(
        ground_truth(&nested_ooxml).expect("중첩 사본 정답지")[0][0],
        4.3,
        "중첩 CFB 사본은 손대지 않아 옛 값 그대로다 — 여기서 4중 표현이 갈린다"
    );
}

/// Stage 2 — 한컴 육안 판정용 변종 꾸러미를 만든다 (S2·S3).
///
/// `output/` 에 파일을 쓰는 부작용이 있어 기본 실행에서 뺀다. 판정 직전에만 돌린다:
///
/// ```text
/// cargo test --profile release-test --test issue_4055_b1_chart_edit_probe -- --ignored --nocapture
/// ```
///
/// 각 변종은 내보내기 전에 **rhwp 가 다시 열 수 있는지** 스스로 확인한다. 한컴이
/// 못 열었을 때 그게 편집 탓인지 파일 조립 탓인지 헷갈리지 않게 하기 위함이다.
#[test]
#[ignore = "output/ 에 파일을 쓴다 — 한컴 판정 직전에만 실행"]
fn generate_hancom_judgment_bundle() {
    let out_dir = manifest("output/issue_4055_b1_spike");
    std::fs::create_dir_all(&out_dir).expect("출력 디렉터리");

    let hwpx = std::fs::read(manifest(&format!("{BASE_SAMPLE}.hwpx"))).expect("HWPX 원본");
    let hwp = std::fs::read(manifest(&format!("{BASE_SAMPLE}.hwp"))).expect("HWP 원본");

    // ── HWPX 쪽 재료
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(hwpx.clone())).expect("zip");
    let chart_part = zip
        .file_names()
        .find(|n| n.starts_with("Chart/") && n.ends_with(".xml"))
        .expect("Chart 파트")
        .to_string();
    let ole_entry = zip
        .file_names()
        .find(|n| n.starts_with("BinData/") && n.to_ascii_lowercase().ends_with(".ole"))
        .expect("BinData ole")
        .to_string();
    let mut chart_xml = Vec::new();
    zip.by_name(&chart_part)
        .expect("Chart 파트 열기")
        .read_to_end(&mut chart_xml)
        .expect("Chart 파트 읽기");
    let mut ole_raw = Vec::new();
    zip.by_name(&ole_entry)
        .expect("ole 열기")
        .read_to_end(&mut ole_raw)
        .expect("ole 읽기");
    let hwpx_nested = if ole_raw.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        ole_raw.clone()
    } else {
        ole_raw[4..].to_vec()
    };
    let hwpx_prefix = ole_raw.len() - hwpx_nested.len();
    let repack_hwpx_ole = |nested: Vec<u8>| -> Vec<u8> {
        let mut v = Vec::with_capacity(nested.len() + hwpx_prefix);
        if hwpx_prefix == 4 {
            v.extend_from_slice(&(nested.len() as u32).to_le_bytes());
        }
        v.extend_from_slice(&nested);
        v
    };
    let patched_chart =
        patch_ooxml_first_value(&chart_xml, SENTINEL_TEXT).expect("Chart 파트 패치");

    // ── HWP 쪽 재료
    let hwp_ole_path = all_streams(&hwp)
        .into_iter()
        .map(|(n, _)| n)
        .find(|n| n.starts_with("/BinData/") && n.to_ascii_uppercase().ends_with(".OLE"))
        .expect("HWP BinData OLE 스트림");
    let hwp_stream = all_streams(&hwp)
        .into_iter()
        .find(|(n, _)| *n == hwp_ole_path)
        .map(|(_, d)| d)
        .expect("HWP OLE 스트림 바이트");
    let hwp_nested = hwp_nested_cfb(&hwp_stream);

    const OLD: f64 = 4.3;
    let bundle: Vec<(String, Vec<u8>, &str, Representations)> = vec![
        (
            "00-control-원본.hwpx".into(),
            hwpx.clone(),
            "대조군 — 바이트 무수정. 첫 막대가 낮다(4.3)",
            Representations {
                zip_part: Some(OLD),
                nested_ooxml: Some(OLD),
                legacy: Some(OLD),
                has_emf: true,
            },
        ),
        (
            "00-control-원본.hwp".into(),
            hwp.clone(),
            "대조군 — 바이트 무수정. 첫 막대가 낮다(4.3)",
            Representations {
                zip_part: None,
                nested_ooxml: Some(OLD),
                legacy: Some(OLD),
                has_emf: true,
            },
        ),
        (
            "X-A-zip파트만.hwpx".into(),
            rewrite_hwpx(&hwpx, &[(chart_part.clone(), patched_chart.clone())]),
            "① Chart/chart1.xml 만 패치",
            Representations {
                zip_part: Some(SENTINEL),
                nested_ooxml: Some(OLD),
                legacy: Some(OLD),
                has_emf: true,
            },
        ),
        (
            "X-B-레거시만.hwpx".into(),
            rewrite_hwpx(
                &hwpx,
                &[(
                    ole_entry.clone(),
                    repack_hwpx_ole(mutate_nested(&hwpx_nested, false, true, false)),
                )],
            ),
            "③ 레거시 Contents 만 패치",
            Representations {
                zip_part: Some(OLD),
                nested_ooxml: Some(OLD),
                legacy: Some(SENTINEL),
                has_emf: true,
            },
        ),
        (
            "X-C-셋다.hwpx".into(),
            rewrite_hwpx(
                &hwpx,
                &[
                    (chart_part.clone(), patched_chart.clone()),
                    (
                        ole_entry.clone(),
                        repack_hwpx_ole(mutate_nested(&hwpx_nested, true, true, false)),
                    ),
                ],
            ),
            "①②③ 전부 패치, EMF 유지",
            Representations {
                zip_part: Some(SENTINEL),
                nested_ooxml: Some(SENTINEL),
                legacy: Some(SENTINEL),
                has_emf: true,
            },
        ),
        (
            "X-D-셋다+EMF제거.hwpx".into(),
            rewrite_hwpx(
                &hwpx,
                &[
                    (chart_part.clone(), patched_chart.clone()),
                    (
                        ole_entry.clone(),
                        repack_hwpx_ole(mutate_nested(&hwpx_nested, true, true, true)),
                    ),
                ],
            ),
            "①②③ 패치 + ④ EMF 프리뷰 제거",
            Representations {
                zip_part: Some(SENTINEL),
                nested_ooxml: Some(SENTINEL),
                legacy: Some(SENTINEL),
                has_emf: false,
            },
        ),
        (
            // X-A 로 "한컴은 OOXML 표현을 읽는다"가 확인됐으므로, .hwp 에서는
            // 중첩 OOXML 사본과 레거시 중 어느 쪽인지가 남은 질문이다. 이 변종이
            // OOXML 쪽 단독 조건이고 H-B 가 레거시 쪽 단독 조건이다.
            "H-A-중첩OOXML만.hwp".into(),
            rewrite_hwp(
                &hwp,
                &[(
                    hwp_ole_path.clone(),
                    hwp_ole_stream(&mutate_nested(&hwp_nested, true, false, false)),
                )],
            ),
            "② 중첩 OOXMLChartContents 만 패치",
            Representations {
                zip_part: None,
                nested_ooxml: Some(SENTINEL),
                legacy: Some(OLD),
                has_emf: true,
            },
        ),
        (
            "H-B-레거시만.hwp".into(),
            rewrite_hwp(
                &hwp,
                &[(
                    hwp_ole_path.clone(),
                    hwp_ole_stream(&mutate_nested(&hwp_nested, false, true, false)),
                )],
            ),
            "③ 레거시 Contents 만 패치",
            Representations {
                zip_part: None,
                nested_ooxml: Some(OLD),
                legacy: Some(SENTINEL),
                has_emf: true,
            },
        ),
        (
            "H-C-둘다.hwp".into(),
            rewrite_hwp(
                &hwp,
                &[(
                    hwp_ole_path.clone(),
                    hwp_ole_stream(&mutate_nested(&hwp_nested, true, true, false)),
                )],
            ),
            "②③ 패치, EMF 유지",
            Representations {
                zip_part: None,
                nested_ooxml: Some(SENTINEL),
                legacy: Some(SENTINEL),
                has_emf: true,
            },
        ),
        (
            "H-D-둘다+EMF제거.hwp".into(),
            rewrite_hwp(
                &hwp,
                &[(
                    hwp_ole_path.clone(),
                    hwp_ole_stream(&mutate_nested(&hwp_nested, true, true, true)),
                )],
            ),
            "②③ 패치 + ④ EMF 프리뷰 제거",
            Representations {
                zip_part: None,
                nested_ooxml: Some(SENTINEL),
                legacy: Some(SENTINEL),
                has_emf: false,
            },
        ),
    ];

    // 내보내기 전 자기 검증 — rhwp 가 다시 열고 차트를 찾을 수 있어야 한다.
    let mut sheet = String::new();
    sheet.push_str("# #4055 B1 스파이크 — 한컴 육안 판정표\n\n");
    sheet.push_str(
        "원본 `samples/chart/세로막대형/묶은세로막대형` 의 **첫 계열 첫 값**을 \
         `4.3` → `91.7` 로 바꾼 변종들입니다.\n원본 최대값이 `5` 라 반영되면 \
         **첫 막대가 차트를 뚫고 솟습니다.** 확대해서 숫자를 읽을 필요가 없습니다.\n\n",
    );
    sheet.push_str("## 진행 상황\n\n");
    sheet.push_str(
        "**HWPX 는 판정 끝났습니다** — `X-A-zip파트만.hwpx` 에서 한컴이 첫 막대를 91.7 로 \
         그렸습니다(2026-08-05 실측). 중첩 OOXML 사본과 레거시가 옛 값이고 EMF 프리뷰에도 \
         옛 값이 박혀 있는데 새 값이 나왔으므로, 한컴은 `Chart/chartN.xml` 을 읽고 \
         **낡은 EMF 는 앞을 가리지 않습니다.**\n\n\
         **남은 것은 `.hwp` 입니다.** zip 파트가 없으니 한컴이 중첩 OOXML 사본(②)을 읽는지 \
         레거시 `Contents`(③)를 읽는지가 질문입니다. `H-A` 와 `H-B` 두 개만 열면 갈립니다.\n\n",
    );
    sheet.push_str("## 보는 법\n\n");
    sheet.push_str(
        "1. `00-control-원본.hwp` 로 정상 모습(막대 4개가 고만고만)을 눈에 익힙니다.\n\
         2. `H-A-중첩OOXML만.hwp` → `H-B-레거시만.hwp` 순으로 엽니다. **둘 중 하나만 솟습니다.**\n\
         3. 둘 다 안 솟으면 `H-C` 를, 그것도 아니면 `H-D` 를 엽니다.\n\
         4. 각 파일마다 함께 봐 주세요:\n   \
         (a) 열 때 오류·복구 대화상자가 뜨는가  (b) 차트를 더블클릭하면 편집기가 열리는가\n\n",
    );
    sheet.push_str("## 판정표\n\n");
    sheet.push_str("| 파일 | 무엇을 바꿨나 | 막대 솟음? | 오류창? | 더블클릭 편집? |\n");
    sheet.push_str("|---|---|---|---|---|\n");

    for (name, bytes, note, expected) in &bundle {
        // 자기 검증 ① rhwp 가 다시 연다.
        rhwp::parse_document(bytes)
            .unwrap_or_else(|e| panic!("{name}: rhwp 가 다시 열지 못한다 — {e:?}"));
        // 자기 검증 ② 라벨대로 조립됐다 — 바꾸기로 한 표현만 sentinel 이고 나머지는 원본이다.
        let actual = representations(bytes);
        assert_eq!(
            &actual, expected,
            "{name}: 변종이 라벨과 다르게 조립됐다 — 이대로 판정하면 결론이 뒤집힌다"
        );

        // 한컴이 파일을 열어 둔 채로도 돌 수 있게: 내용이 같으면 건드리지 않고,
        // 잠겨 있으면 내용이 이미 최신인지 확인한 뒤 넘어간다.
        let target = out_dir.join(name);
        let on_disk = std::fs::read(&target).ok();
        let status = if on_disk.as_deref() == Some(bytes.as_slice()) {
            "unchanged"
        } else {
            match std::fs::write(&target, bytes) {
                Ok(()) => "wrote",
                Err(e) => panic!(
                    "{name} 쓰기 실패({e}) — 내용이 디스크와 달라 갱신이 필요하다. \
                     이 파일을 연 프로그램(한컴 등)을 닫고 다시 실행하세요."
                ),
            }
        };
        println!(
            "  {status:<9} {name}  ({} bytes)  zip={:?} nested={:?} legacy={:?} emf={}",
            bytes.len(),
            actual.zip_part,
            actual.nested_ooxml,
            actual.legacy,
            actual.has_emf
        );
        sheet.push_str(&format!("| `{name}` | {note} |  |  |  |\n"));
    }

    sheet.push_str("\n## `.hwp` 결과가 뜻하는 것\n\n");
    sheet.push_str(
        "| 솟은 변종 | 해석 | B1 의 `.hwp` 경로 |\n|---|---|---|\n\
         | **H-A** | 한컴이 중첩 `OOXMLChartContents` 를 읽는다 | HWPX 와 같은 표현을 쓰면 된다. \
         중첩 CFB 재포장만 새로 필요(무손실 확인됨) |\n\
         | **H-B** | 한컴이 레거시 `Contents` 를 읽는다 | #3683 원안이 맞다 — 레거시 f64 패치가 \
         `.hwp` 의 본체. Stage 1 로케이터가 그대로 쓰인다 |\n\
         | H-C 만 | 둘을 같이 맞춰야 한다 | 편집 시 ②③ 동시 기록 |\n\
         | H-D 만 | 낡은 EMF 가 앞을 가린다 | `.hwp` 에 한해 EMF 처리가 필요 \
         (HWPX 에서는 안 가렸다) |\n\
         | 아무것도 안 솟음 | `.hwp` 는 EMF 만 그린다 | **`.hwp` 편집 범위 재협의 사유** |\n",
    );

    std::fs::write(out_dir.join("PANJEONG.md"), sheet).expect("판정표 쓰기");
    println!("\n  판정표: {}", out_dir.join("PANJEONG.md").display());
}
