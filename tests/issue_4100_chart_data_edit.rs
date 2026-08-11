//! [#4100] B1 엔진축 — 차트 숫자 데이터 편집.
//!
//! 구조 스캐너·최소 diff 패처(Stage 1), 중첩 CFB 스트림 교체(Stage 2), 주소→①② 슬롯
//! 해석과 `get_chart_data_native`(Stage 3), `set_chart_data_native` 의 ①② 동시
//! 기록(Stage 4), 그리고 실사용 문서 회귀(Stage 5)를 검증한다. CLI 계약은
//! `tests/chart_csv_contract.rs` 에 있다.
//!
//! 스캐너를 따로 만드는 이유는 `src/ooxml_chart/parser.rs` 가 **손실 파서**이기
//! 때문이다 — `c:pt idx`·`c:f`·`c:externalData`·`extLst` 를 읽지 않아 파싱→재방출로
//! 왕복시키면 모델에 없는 것이 전부 사라진다. 코퍼스 28종 전건이 `c:extLst` 와
//! `ho:hncChartStyle` 을 갖고 있다. 그래서 `c:v` 텍스트 구간만 바꾸는 바이트 수술을 한다.

#[path = "support/issue_4055_chart_probe.rs"]
mod chart_probe_support;

use chart_probe_support::{chart_streams, corpus, manifest, root_clsid};

use rhwp::ooxml_chart::data::{scan_chart_values, SeriesAxis};
use rhwp::ooxml_chart::patch::{apply_value_edits, EditTarget, PatchError, ValueEdit};
use rhwp::ooxml_chart::OoxmlChart;
use rhwp::parser::ole_container::{all_ole_streams, ole_root_clsid};
use rhwp::serializer::ole_container::{replace_ole_stream, OleRepackError};

/// 중첩 CFB 안 OOXML 차트 스트림의 이름.
const OOXML_STREAM: &str = "OOXMLChartContents";

/// 코퍼스 28종 × 2포맷. `samples/chart/` 에 파일을 커밋하면 이 수가 깨진다 —
/// `issue_4055_b1_chart_edit_probe.rs` 의 `checked == 56` 과 같은 고정이다.
const CORPUS_FILES: usize = 56;

/// `(경로, OOXML 차트 XML)` 전건. HWPX 는 `Chart/chartN.xml`, HWP5 는 중첩 CFB 의
/// `OOXMLChartContents` 에서 온다 — `chart_streams` 가 그 차이를 흡수한다.
fn corpus_charts() -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for hwpx in corpus() {
        for path in [hwpx.with_extension("hwpx"), hwpx.with_extension("hwp")] {
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("{} 읽기 실패: {e}", path.display()));
            let (_legacy, ooxml) = chart_streams(&bytes)
                .unwrap_or_else(|| panic!("{}: 차트 스트림을 꺼내지 못했다", path.display()));
            out.push((path, ooxml));
        }
    }
    assert_eq!(out.len(), CORPUS_FILES, "코퍼스 28종 × 2포맷");
    out
}

/// `(경로, 중첩 CFB 바이트)` 전건.
///
/// IR 의 `bin_data_content` 에는 **접두어 없는 맨 중첩 CFB** 가 들어 있다 — 4바이트 LE
/// 크기 접두어는 직렬화기가 붙이고 파서가 뗀다(`serializer/cfb_writer.rs`).
fn corpus_nested_cfbs() -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for hwpx in corpus() {
        for path in [hwpx.with_extension("hwpx"), hwpx.with_extension("hwp")] {
            let bytes = std::fs::read(&path).expect("샘플 읽기");
            let doc = rhwp::parse_document(&bytes)
                .unwrap_or_else(|e| panic!("{}: 파싱 {e:?}", path.display()));
            let nested = doc
                .bin_data_content
                .iter()
                .map(|c| c.data.load())
                .find(|b| b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]))
                .unwrap_or_else(|| panic!("{}: 중첩 CFB 를 못 찾았다", path.display()));
            out.push((path, nested));
        }
    }
    assert_eq!(out.len(), CORPUS_FILES, "코퍼스 28종 × 2포맷");
    out
}

/// 중첩 CFB 에서 스트림 하나를 꺼낸다.
fn stream_of(cfb: &[u8], name: &str) -> Option<Vec<u8>> {
    all_ole_streams(cfb)?
        .into_iter()
        .find(|(p, _)| p.trim_start_matches('/') == name)
        .map(|(_, d)| d)
}

/// 모든 값 점을 **자기 텍스트 그대로** 쓰는 편집 목록. 무편집 왕복의 재료다.
fn identity_edits(data: &rhwp::ooxml_chart::data::ChartData) -> Vec<ValueEdit> {
    let mut edits = Vec::new();
    for (si, series) in data.series.iter().enumerate() {
        for (pi, point) in series.values.iter().enumerate() {
            edits.push(ValueEdit {
                series: si,
                point: pi,
                target: EditTarget::Value,
                text: point.text.clone(),
            });
        }
        if series.axis == SeriesAxis::Scatter {
            for (pi, point) in series.labels.iter().enumerate() {
                edits.push(ValueEdit {
                    series: si,
                    point: pi,
                    target: EditTarget::Label,
                    text: point.text.clone(),
                });
            }
        }
    }
    edits
}

/// 스캐너가 모델 파서와 같은 값을 같은 순서로 본다.
///
/// 오라클을 `OoxmlChart::parse` 로 두는 이유: 그것이 렌더러가 실제로 그리는 값이고,
/// 스캐너와 완전히 다른 경로(SAX 모델 빌드 vs 오프셋 추적)라 공모하지 않는다.
#[test]
fn scanner_agrees_with_the_model_parser_across_the_corpus() {
    let mut checked = 0usize;
    for (path, ooxml) in corpus_charts() {
        let scan = scan_chart_values(&ooxml)
            .unwrap_or_else(|e| panic!("{}: 스캔 실패 {e:?}", path.display()));
        let model = OoxmlChart::parse(&ooxml)
            .unwrap_or_else(|| panic!("{}: 모델 파서가 차트를 못 읽었다", path.display()));

        assert_eq!(
            scan.series.len(),
            model.series.len(),
            "{}: 계열 수",
            path.display()
        );

        for (i, (scanned, modeled)) in scan.series.iter().zip(&model.series).enumerate() {
            let values: Vec<f64> = scanned
                .values
                .iter()
                .map(|p| {
                    p.text
                        .parse::<f64>()
                        .unwrap_or_else(|_| panic!("{}: 계열 {i} 값 `{}`", path.display(), p.text))
                })
                .collect();
            assert_eq!(values, modeled.values, "{}: 계열 {i} 값", path.display());

            if scanned.axis == SeriesAxis::Scatter {
                let xs: Vec<f64> = scanned
                    .labels
                    .iter()
                    .map(|p| p.text.parse::<f64>().expect("분산형 X 는 수치"))
                    .collect();
                assert_eq!(xs, modeled.x_values, "{}: 계열 {i} X", path.display());
            } else {
                assert!(
                    modeled.x_values.is_empty(),
                    "{}: 계열 {i} 는 분산형이 아닌데 모델에 X 가 있다",
                    path.display()
                );
            }
        }
        checked += 1;
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// 구간이 자기 텍스트를 정확히 가리킨다 — 패처가 믿는 유일한 계약이다.
#[test]
fn every_span_slices_back_to_its_own_text() {
    let mut points = 0usize;
    for (path, ooxml) in corpus_charts() {
        let scan = scan_chart_values(&ooxml).expect("스캔");
        for series in &scan.series {
            for point in series.values.iter().chain(series.labels.iter()) {
                let span = point
                    .span
                    .clone()
                    .unwrap_or_else(|| panic!("{}: 코퍼스엔 빈 값이 없다", path.display()));
                assert_eq!(
                    &ooxml[span.clone()],
                    point.text.as_bytes(),
                    "{}: 구간 {span:?} 이 텍스트와 다르다",
                    path.display(),
                );
                points += 1;
            }
        }
    }
    // 계열 68 + 카테고리/X — 코퍼스가 바뀌지 않는 한 0 이 될 수 없다.
    assert!(points > 200, "훑은 점이 너무 적다: {points}");
}

/// **무편집 왕복이 바이트 동일하다** (수용 기준 2 의 XML 층).
///
/// 모든 값을 자기 텍스트로 다시 써도 한 바이트도 달라지지 않아야 한다. 여기서
/// 어긋나면 정규화·재직렬화가 어딘가 섞였다는 뜻이다.
#[test]
fn identity_patch_is_byte_identical_across_the_corpus() {
    let mut checked = 0usize;
    for (path, ooxml) in corpus_charts() {
        let scan = scan_chart_values(&ooxml).expect("스캔");
        let edits = identity_edits(&scan);
        assert!(!edits.is_empty(), "{}: 편집 대상 0건", path.display());

        let patched = apply_value_edits(&ooxml, &scan, &edits)
            .unwrap_or_else(|e| panic!("{}: 패치 실패 {e:?}", path.display()));
        assert_eq!(
            patched,
            ooxml,
            "{}: 무편집 왕복이 바이트를 바꿨다",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// 실제 편집이 그 값 **하나만** 바꾼다 — 최소 diff 의 정의.
#[test]
fn a_single_edit_changes_only_that_value() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let bytes = std::fs::read(&path).expect("샘플 읽기");
    let (_legacy, ooxml) = chart_streams(&bytes).expect("차트 스트림");
    let scan = scan_chart_values(&ooxml).expect("스캔");

    let before = scan.series[0].values[0].text.clone();
    assert_ne!(before, "91.7", "센티널이 원본과 같으면 판정이 공허하다");

    let patched = apply_value_edits(
        &ooxml,
        &scan,
        &[ValueEdit {
            series: 0,
            point: 0,
            target: EditTarget::Value,
            text: "91.7".to_string(),
        }],
    )
    .expect("패치");

    let after = scan_chart_values(&patched).expect("재스캔");
    assert_eq!(after.series[0].values[0].text, "91.7");

    // 나머지 값은 전부 그대로다.
    for (si, (a, b)) in after.series.iter().zip(&scan.series).enumerate() {
        for (pi, (x, y)) in a.values.iter().zip(&b.values).enumerate() {
            if (si, pi) == (0, 0) {
                continue;
            }
            assert_eq!(x.text, y.text, "계열 {si} 점 {pi} 이 함께 바뀌었다");
        }
    }

    // 길이 차이는 텍스트 길이 차이뿐 — 그 외 바이트는 손대지 않았다.
    let delta = patched.len() as isize - ooxml.len() as isize;
    assert_eq!(delta, "91.7".len() as isize - before.len() as isize);
}

/// **M3** — `c:numLit`/`c:strLit` 문서도 캐시형과 같은 경로로 잡힌다.
///
/// 코퍼스에서 리터럴을 쓰는 문서는 이 한 건뿐이고, `c:f`·`c:cat` 참조가 아예 없다.
/// 무편집 왕복의 최난도 케이스라 따로 지목해 둔다.
#[test]
fn numeric_literal_chart_is_scanned_like_a_cached_one() {
    let path = manifest("samples/chart/특이케이스/가로막대형_하나만있을떄_단일시리즈제목.hwpx");
    let bytes = std::fs::read(&path).expect("샘플 읽기");
    let (_legacy, ooxml) = chart_streams(&bytes).expect("차트 스트림");

    assert!(
        String::from_utf8_lossy(&ooxml).contains("numLit"),
        "이 샘플은 c:numLit 을 써야 한다 — 아니면 M3 판정이 대상을 잃는다"
    );

    let scan = scan_chart_values(&ooxml).expect("스캔");
    assert_eq!(scan.series.len(), 1);
    assert_eq!(scan.series[0].values.len(), 1);
    assert_eq!(scan.series[0].axis, SeriesAxis::Category);

    let patched = apply_value_edits(&ooxml, &scan, &identity_edits(&scan)).expect("패치");
    assert_eq!(patched, ooxml, "리터럴 문서의 무편집 왕복이 깨졌다");
}

/// **M2** — 분산형의 X 는 편집 대상이고, 코퍼스에서는 계열 간 동일하다.
///
/// 이 동일성은 **코퍼스 성질이지 포맷 보장이 아니다.** OOXML 은 계열마다 다른 X 를
/// 허용하므로 CSV 층(Stage 5)이 `sharedXRequired` 로 거부한다. 여기서는 그 전제가
/// 코퍼스에서 실제로 성립함을 고정한다.
#[test]
fn scatter_series_expose_editable_x_values_shared_across_series() {
    let mut scatter_files = 0usize;
    for (path, ooxml) in corpus_charts() {
        let scan = scan_chart_values(&ooxml).expect("스캔");
        if scan.series.iter().all(|s| s.axis != SeriesAxis::Scatter) {
            continue;
        }
        assert!(
            scan.series.iter().all(|s| s.axis == SeriesAxis::Scatter),
            "{}: 분산형과 카테고리형 계열이 섞였다",
            path.display()
        );

        let first: Vec<&str> = scan.series[0]
            .labels
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        for (i, series) in scan.series.iter().enumerate().skip(1) {
            let xs: Vec<&str> = series.labels.iter().map(|p| p.text.as_str()).collect();
            assert_eq!(xs, first, "{}: 계열 {i} 의 X 가 계열 0 과 다르다", path.display());
        }
        scatter_files += 1;
    }
    assert_eq!(scatter_files, 10, "분산형 5종 × 2포맷");
}

/// 카테고리 라벨은 B1 에서 편집 대상이 아니다 — 구조 변경이라 B2 다.
#[test]
fn category_labels_are_not_editable() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let bytes = std::fs::read(&path).expect("샘플 읽기");
    let (_legacy, ooxml) = chart_streams(&bytes).expect("차트 스트림");
    let scan = scan_chart_values(&ooxml).expect("스캔");
    assert_eq!(scan.series[0].axis, SeriesAxis::Category);

    let err = apply_value_edits(
        &ooxml,
        &scan,
        &[ValueEdit {
            series: 0,
            point: 0,
            target: EditTarget::Label,
            text: "새 라벨".to_string(),
        }],
    )
    .expect_err("카테고리 라벨 편집은 거부되어야 한다");
    assert!(matches!(err, PatchError::LabelNotEditable { series: 0 }), "{err:?}");
}

/// 패처는 주소 오류·중복·XML 안전하지 않은 텍스트를 **쓰기 전에** 거부한다.
#[test]
fn patcher_rejects_bad_addresses_duplicates_and_unsafe_text() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let bytes = std::fs::read(&path).expect("샘플 읽기");
    let (_legacy, ooxml) = chart_streams(&bytes).expect("차트 스트림");
    let scan = scan_chart_values(&ooxml).expect("스캔");

    let value = |series, point, text: &str| ValueEdit {
        series,
        point,
        target: EditTarget::Value,
        text: text.to_string(),
    };

    let cases: Vec<(ValueEdit, &str)> = vec![
        (value(99, 0, "1"), "없는 계열"),
        (value(0, 99, "1"), "없는 점"),
        (value(0, 0, "1 < 2"), "XML 특수문자"),
        (value(0, 0, "a&b"), "XML 특수문자"),
    ];
    for (edit, why) in cases {
        assert!(
            apply_value_edits(&ooxml, &scan, &[edit]).is_err(),
            "{why} 는 거부되어야 한다"
        );
    }

    let err = apply_value_edits(&ooxml, &scan, &[value(0, 0, "1"), value(0, 0, "2")])
        .expect_err("같은 점을 두 번 지목하면 거부");
    assert!(matches!(err, PatchError::DuplicateTarget { .. }), "{err:?}");
}

// ---------------------------------------------------------------------------
// Stage 2 — 중첩 CFB 스트림 교체 (②축의 재료)
// ---------------------------------------------------------------------------

/// **재포장이 아는 4종 밖 스트림까지 살린다.**
///
/// `parse_ole_container` 로 재포장하면 나머지가 소실되므로 `all_ole_streams` 전수
/// 열거 위에 선다. 코퍼스는 `Contents`·`\x02OlePres000`·`OOXMLChartContents` 셋인데,
/// 이름을 고정하지 않고 **집합이 보존되는지**로 판정한다.
#[test]
fn repack_preserves_every_stream_and_leaves_the_others_byte_identical() {
    let mut checked = 0usize;
    for (path, nested) in corpus_nested_cfbs() {
        let before = all_ole_streams(&nested)
            .unwrap_or_else(|| panic!("{}: 중첩 CFB 열거 실패", path.display()));
        assert!(
            before.iter().any(|(p, _)| p.trim_start_matches('/') == OOXML_STREAM),
            "{}: OOXMLChartContents 가 없다",
            path.display()
        );

        let ooxml = stream_of(&nested, OOXML_STREAM).expect("OOXML");
        let scan = scan_chart_values(&ooxml).expect("스캔");
        let patched = apply_value_edits(
            &ooxml,
            &scan,
            &[ValueEdit {
                series: 0,
                point: 0,
                target: EditTarget::Value,
                text: "91.7".to_string(),
            }],
        )
        .expect("패치");

        let repacked = replace_ole_stream(&nested, OOXML_STREAM, &patched)
            .unwrap_or_else(|e| panic!("{}: 재포장 {e}", path.display()));
        let after = all_ole_streams(&repacked).expect("재포장본 열거");

        let names_before: Vec<&str> = before.iter().map(|(p, _)| p.as_str()).collect();
        let names_after: Vec<&str> = after.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(names_after, names_before, "{}: 스트림 집합", path.display());

        for (name, bytes) in &before {
            if name.trim_start_matches('/') == OOXML_STREAM {
                continue;
            }
            let now = stream_of(&repacked, name.trim_start_matches('/')).expect("스트림");
            assert_eq!(&now, bytes, "{}: `{name}` 이 바뀌었다", path.display());
        }

        assert_eq!(
            stream_of(&repacked, OOXML_STREAM).as_deref(),
            Some(patched.as_slice()),
            "{}: 새 OOXML 이 실리지 않았다",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// **루트 CLSID 가 살아남는다** — 떨구면 한컴이 개체를 알아보지 못해 내용을 비운다(#4097).
///
/// 판정은 `cfb` 크레이트 오라클(`root_clsid`)로 한다. rhwp 의 `ole_root_clsid` 로만
/// 재면 읽기·쓰기가 같은 오프셋 오해를 공유해도 통과해 버린다.
#[test]
fn repack_preserves_the_root_class_id() {
    let mut checked = 0usize;
    for (path, nested) in corpus_nested_cfbs() {
        let original = root_clsid(&nested);
        assert_ne!(
            original,
            [0u8; 16],
            "{}: 원본 CLSID 가 0 이면 판정이 공허하다",
            path.display()
        );

        let ooxml = stream_of(&nested, OOXML_STREAM).expect("OOXML");
        let scan = scan_chart_values(&ooxml).expect("스캔");
        let patched = apply_value_edits(
            &ooxml,
            &scan,
            &[ValueEdit {
                series: 0,
                point: 0,
                target: EditTarget::Value,
                text: "91.7".to_string(),
            }],
        )
        .expect("패치");
        let repacked = replace_ole_stream(&nested, OOXML_STREAM, &patched).expect("재포장");

        assert_eq!(root_clsid(&repacked), original, "{}", path.display());
        assert_eq!(ole_root_clsid(&repacked), Some(original), "{}", path.display());
        checked += 1;
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// **바뀐 게 없으면 중첩 CFB 를 다시 쓰지 않는다.**
///
/// 재포장은 섹터 배치가 원본 작성기와 달라 바이트 동일을 보장하지 않는다. 짧은 회로가
/// 없으면 "무편집 왕복 바이트 동일"(수용 기준 2)이 재포장만으로 깨진다.
#[test]
fn unchanged_stream_content_skips_the_repack_entirely() {
    let mut checked = 0usize;
    for (path, nested) in corpus_nested_cfbs() {
        let ooxml = stream_of(&nested, OOXML_STREAM).expect("OOXML");
        let out = replace_ole_stream(&nested, OOXML_STREAM, &ooxml).expect("재포장");
        assert_eq!(out, nested, "{}: 무편집인데 바이트가 바뀌었다", path.display());
        checked += 1;
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// 없는 스트림은 새로 만들지 않고 거부한다 — 이름 오타가 조용히 파일을 망치지 않게.
#[test]
fn repack_refuses_to_invent_a_missing_stream() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let bytes = std::fs::read(&path).expect("샘플 읽기");
    let doc = rhwp::parse_document(&bytes).expect("파싱");
    let nested = doc
        .bin_data_content
        .iter()
        .map(|c| c.data.load())
        .find(|b| b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]))
        .expect("중첩 CFB");

    assert_eq!(
        replace_ole_stream(&nested, "OOXMLChartContent", b"x"),
        Err(OleRepackError::StreamNotFound("OOXMLChartContent".to_string()))
    );
}

// ---------------------------------------------------------------------------
// Stage 3 — 주소 → ①② 슬롯 해석 + get_chart_data_native
// ---------------------------------------------------------------------------

use rhwp::document_core::queries::chart_extract::{chart_xml, collect_charts};
use rhwp::document_core::DocumentCore;

fn core_of(path: &std::path::Path) -> DocumentCore {
    let bytes = std::fs::read(path).expect("샘플 읽기");
    DocumentCore::from_bytes(&bytes).unwrap_or_else(|e| panic!("{}: 코어 {e:?}", path.display()))
}

/// 코퍼스 전건에서 차트가 **정확히 하나** 열거되고, 두 표현이 포맷대로 해소된다.
///
/// HWPX 는 ①②가 다 있고, HWP5 는 `Chart/*.xml` 파트가 없어 ②만 있다.
#[test]
fn every_corpus_document_resolves_its_chart_slots() {
    let mut hwpx_seen = 0usize;
    let mut hwp_seen = 0usize;
    for hwpx in corpus() {
        for path in [hwpx.with_extension("hwpx"), hwpx.with_extension("hwp")] {
            let core = core_of(&path);
            let charts = collect_charts(core.document());
            assert_eq!(charts.len(), 1, "{}: 차트 수", path.display());
            let chart = &charts[0];
            assert!(chart.is_top_level(), "{}: 본문 직속이어야 한다", path.display());
            assert!(chart.nested_copy.is_some(), "{}: ② 미해소", path.display());

            if path.extension().is_some_and(|e| e == "hwpx") {
                assert!(chart.zip_part.is_some(), "{}: ① 미해소", path.display());
                hwpx_seen += 1;
            } else {
                assert!(
                    chart.zip_part.is_none(),
                    "{}: HWP5 에 ① 이 있을 수 없다",
                    path.display()
                );
                hwp_seen += 1;
            }

            let (xml, _) = chart_xml(core.document(), chart).expect("차트 XML");
            assert!(scan_chart_values(&xml).is_ok(), "{}: 스캔", path.display());
        }
    }
    assert_eq!((hwpx_seen, hwp_seen), (28, 28));
}

/// **①==②** — 어느 표현에서 읽어도 같은 XML 이다(#4055 의 SHA-256 전건 일치를 코드로 고정).
#[test]
fn both_representations_carry_the_same_xml() {
    let mut checked = 0usize;
    for hwpx in corpus() {
        let core = core_of(&hwpx);
        let charts = collect_charts(core.document());
        let chart = &charts[0];
        let zip = core.document().bin_data_content[chart.zip_part.expect("①")]
            .data
            .load();
        let nested_cfb = core.document().bin_data_content[chart.nested_copy.expect("②")]
            .data
            .load();
        let nested = stream_of(&nested_cfb, OOXML_STREAM).expect("②의 OOXML");
        assert_eq!(zip, nested, "{}: ① 과 ② 가 다르다", hwpx.display());
        checked += 1;
    }
    assert_eq!(checked, 28);
}

/// `get_chart_data_native` 가 모델 파서와 같은 값을 돌려준다.
#[test]
fn get_chart_data_native_matches_the_model_parser() {
    let mut checked = 0usize;
    for hwpx in corpus() {
        for path in [hwpx.with_extension("hwpx"), hwpx.with_extension("hwp")] {
            let core = core_of(&path);
            let chart = &collect_charts(core.document())[0];
            let json: serde_json::Value = serde_json::from_str(
                &core
                    .get_chart_data_native(chart.section, chart.paragraph, chart.control)
                    .unwrap_or_else(|e| panic!("{}: {e:?}", path.display())),
            )
            .expect("JSON");

            assert_eq!(json["ok"], true, "{}", path.display());
            assert_eq!(json["chart"], 1, "{}", path.display());
            assert_eq!(json["labelsShared"], true, "{}", path.display());

            let (xml, _) = chart_xml(core.document(), chart).expect("XML");
            let model = OoxmlChart::parse(&xml).expect("모델");
            let series = json["series"].as_array().expect("series");
            assert_eq!(series.len(), model.series.len(), "{}", path.display());
            for (s, m) in series.iter().zip(&model.series) {
                let values: Vec<f64> = s["values"]
                    .as_array()
                    .expect("values")
                    .iter()
                    .map(|v| v.as_str().expect("문자열").parse().expect("수치"))
                    .collect();
                assert_eq!(values, m.values, "{}", path.display());
            }

            let is_hwpx = path.extension().is_some_and(|e| e == "hwpx");
            assert_eq!(json["representations"]["zipPart"], is_hwpx, "{}", path.display());
            assert_eq!(json["representations"]["nestedCopy"], true, "{}", path.display());
            assert_eq!(
                json["source"],
                if is_hwpx { "zipPart" } else { "nestedCopy" },
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// 값은 **원본 텍스트 그대로** 실린다 — 실수로 파싱했다가 되쓰면 표기가 달라져
/// 무편집 왕복의 바이트 동일이 깨진다.
#[test]
fn values_keep_their_original_spelling() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let core = core_of(&path);
    let chart = &collect_charts(core.document())[0];
    let (xml, _) = chart_xml(core.document(), chart).expect("XML");
    let scan = scan_chart_values(&xml).expect("스캔");

    let json: serde_json::Value =
        serde_json::from_str(&core.get_chart_data_native(chart.section, chart.paragraph, chart.control).expect("읽기"))
            .expect("JSON");

    for (si, series) in scan.series.iter().enumerate() {
        for (pi, point) in series.values.iter().enumerate() {
            assert_eq!(
                json["series"][si]["values"][pi].as_str(),
                Some(point.text.as_str())
            );
        }
    }
}

/// 주소 오류만 `Err` 다 — 데이터 문제는 `Ok` + 부정 봉투다.
#[test]
fn only_address_errors_are_err() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let core = core_of(&path);
    let chart = &collect_charts(core.document())[0];

    assert!(core.get_chart_data_native(99, 0, 0).is_err(), "없는 구역");
    assert!(core.get_chart_data_native(0, 9999, 0).is_err(), "없는 문단");
    assert!(
        core.get_chart_data_native(chart.section, chart.paragraph, 9999)
            .is_err(),
        "없는 컨트롤"
    );

    // 차트가 아닌 컨트롤을 지목하면 Err — 같은 문단의 다른 컨트롤을 찾아 시험한다.
    let para = &core.document().sections[chart.section].paragraphs[chart.paragraph];
    if let Some(other) = (0..para.controls.len()).find(|&i| i != chart.control) {
        assert!(
            core.get_chart_data_native(chart.section, chart.paragraph, other)
                .is_err(),
            "차트가 아닌 컨트롤"
        );
    }
}

/// 순번 경로는 컨테이너 안의 차트에도 닿는다 — 3인자 주소가 표현하지 못하는 자리다.
#[test]
fn index_addressing_covers_the_same_chart() {
    let path = manifest("samples/chart/원형/쪼개진원형.hwpx");
    let core = core_of(&path);
    let chart = &collect_charts(core.document())[0];
    let by_addr = core
        .get_chart_data_native(chart.section, chart.paragraph, chart.control)
        .expect("주소");
    let by_index = core.get_chart_data_by_index_native(0).expect("순번");
    assert_eq!(by_addr, by_index);
    assert!(core.get_chart_data_by_index_native(9).is_err(), "없는 순번");
}

// ---------------------------------------------------------------------------
// Stage 4 — set_chart_data_native (①② 동시 기록)
// ---------------------------------------------------------------------------

/// 레거시 `Contents` 스트림 이름.
const LEGACY_STREAM: &str = "Contents";
/// EMF 프리뷰 스트림 이름.
const EMF_STREAM: &str = "\u{2}OlePres000";

/// 현재 값 그대로의 편집 입력 — 무편집 왕복의 재료.
fn edits_from(core: &DocumentCore, index: usize) -> serde_json::Value {
    let read: serde_json::Value =
        serde_json::from_str(&core.get_chart_data_by_index_native(index).expect("읽기"))
            .expect("JSON");
    serde_json::json!({
        "labels": read["labels"],
        "series": read["series"]
            .as_array()
            .expect("series")
            .iter()
            .map(|s| serde_json::json!({"name": s["name"], "values": s["values"]}))
            .collect::<Vec<_>>(),
    })
}

fn slot_bytes(core: &DocumentCore) -> Vec<Vec<u8>> {
    core.document()
        .bin_data_content
        .iter()
        .map(|c| c.data.load())
        .collect()
}

fn set_chart(core: &mut DocumentCore, edits: &serde_json::Value) -> serde_json::Value {
    serde_json::from_str(
        &core
            .set_chart_data_by_index_native(0, &edits.to_string())
            .expect("쓰기"),
    )
    .expect("JSON")
}

/// **편집이 ①②에 함께 실린다** (수용 기준 1 의 코어 층).
///
/// HWPX 는 두 표현 모두, HWP5 는 ②가 새 값이다. ③(레거시)·④(EMF)는 바이트 그대로 —
/// B1 은 그것들을 쓰지 않는다.
#[test]
fn an_edit_lands_in_both_representations_and_leaves_the_others_alone() {
    let mut checked = 0usize;
    for hwpx in corpus() {
        for path in [hwpx.with_extension("hwpx"), hwpx.with_extension("hwp")] {
            let mut core = core_of(&path);
            let chart = collect_charts(core.document())[0].clone();
            let nested_idx = chart.nested_copy.expect("②");
            let before_nested = core.document().bin_data_content[nested_idx].data.load();
            let legacy_before = stream_of(&before_nested, LEGACY_STREAM);
            let emf_before = stream_of(&before_nested, EMF_STREAM);

            let mut edits = edits_from(&core, 0);
            edits["series"][0]["values"][0] = serde_json::json!("91.7");
            let out = set_chart(&mut core, &edits);

            assert_eq!(out["ok"], true, "{}: {out}", path.display());
            assert_eq!(out["changedCount"], 1, "{}: {out}", path.display());

            let is_hwpx = path.extension().is_some_and(|e| e == "hwpx");
            let wrote: Vec<&str> = out["wrote"]
                .as_array()
                .expect("wrote")
                .iter()
                .map(|v| v.as_str().expect("문자열"))
                .collect();
            if is_hwpx {
                assert_eq!(wrote, ["zipPart", "nestedCopy"], "{}", path.display());
                let zip = core.document().bin_data_content[chart.zip_part.expect("①")]
                    .data
                    .load();
                assert!(
                    String::from_utf8_lossy(&zip).contains("<c:v>91.7</c:v>"),
                    "{}: ① 에 새 값이 없다",
                    path.display()
                );
            } else {
                assert_eq!(wrote, ["nestedCopy"], "{}", path.display());
            }

            let after_nested = core.document().bin_data_content[nested_idx].data.load();
            let ooxml = stream_of(&after_nested, OOXML_STREAM).expect("②의 OOXML");
            assert!(
                String::from_utf8_lossy(&ooxml).contains("<c:v>91.7</c:v>"),
                "{}: ② 에 새 값이 없다",
                path.display()
            );
            assert_eq!(
                stream_of(&after_nested, LEGACY_STREAM),
                legacy_before,
                "{}: ③ 레거시가 바뀌었다",
                path.display()
            );
            assert_eq!(
                stream_of(&after_nested, EMF_STREAM),
                emf_before,
                "{}: ④ EMF 가 바뀌었다",
                path.display()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// **무편집 왕복이 슬롯 바이트를 건드리지 않는다** (수용 기준 2 의 코어 층).
#[test]
fn writing_the_current_values_back_changes_nothing() {
    let mut checked = 0usize;
    for hwpx in corpus() {
        for path in [hwpx.with_extension("hwpx"), hwpx.with_extension("hwp")] {
            let mut core = core_of(&path);
            let before = slot_bytes(&core);

            let edits = edits_from(&core, 0);
            let out = set_chart(&mut core, &edits);

            assert_eq!(out["ok"], true, "{}", path.display());
            assert_eq!(out["changedCount"], 0, "{}: {out}", path.display());
            assert!(
                out["wrote"].as_array().expect("wrote").is_empty(),
                "{}",
                path.display()
            );
            assert_eq!(slot_bytes(&core), before, "{}: 슬롯 바이트가 바뀌었다", path.display());
            checked += 1;
        }
    }
    assert_eq!(checked, CORPUS_FILES);
}

/// 편집이 저장·재파스를 넘어 살아남는다 — ②까지 함께.
#[test]
fn the_edit_survives_save_and_reparse() {
    for name in [
        "세로막대형/묶은세로막대형",
        "원형/쪼개진원형",
        "분산형/직선이있는분산형",
    ] {
        let path = manifest(&format!("samples/chart/{name}.hwpx"));
        let mut core = core_of(&path);
        let mut edits = edits_from(&core, 0);
        edits["series"][0]["values"][0] = serde_json::json!("91.7");
        assert_eq!(set_chart(&mut core, &edits)["ok"], true, "{name}");

        let saved = core.export_hwpx_native().expect("HWPX 저장");
        let reread = DocumentCore::from_bytes(&saved).expect("재파스");
        let json: serde_json::Value =
            serde_json::from_str(&reread.get_chart_data_by_index_native(0).expect("읽기"))
                .expect("JSON");
        assert_eq!(json["series"][0]["values"][0], "91.7", "{name}");

        let chart = collect_charts(reread.document())[0].clone();
        let nested = reread.document().bin_data_content[chart.nested_copy.expect("②")]
            .data
            .load();
        let ooxml = stream_of(&nested, OOXML_STREAM).expect("②");
        assert!(
            String::from_utf8_lossy(&ooxml).contains("<c:v>91.7</c:v>"),
            "{name}: 저장 후 ② 에 새 값이 없다"
        );
    }
}

/// **HWP5 저장에서도 편집이 살아남는다** — 패스스루 면제의 근거 (#2724 가드).
///
/// `issue_2724_passthrough_invalidation_guard` 는 문서 IR 을 바꾸는 `&mut self` 가
/// `section.raw_stream`/`doc_info.raw_stream_dirty` 를 무효화하지 않으면 저장이 원본
/// 바이트를 그대로 돌려줘 **편집이 조용히 사라진다**고 경고한다. 차트 편집은
/// `bin_data_content` 만 바꾸고 그것은 BodyText·DocInfo 스트림이 아니라 BinData
/// 저장소라 패스스루 대상이 아니다 — 그 주장을 말로 두지 않고 저장→재파스로 판정한다.
#[test]
fn the_edit_survives_hwp5_save_despite_stream_passthrough() {
    for name in ["세로막대형/묶은세로막대형", "기타/고가저가종가"] {
        let path = manifest(&format!("samples/chart/{name}.hwp"));
        let mut core = core_of(&path);
        let mut edits = edits_from(&core, 0);
        edits["series"][0]["values"][0] = serde_json::json!("91.7");
        assert_eq!(set_chart(&mut core, &edits)["ok"], true, "{name}");

        let saved = core.export_hwp_native().expect("HWP5 저장");
        let reread = DocumentCore::from_bytes(&saved).expect("재파스");
        let json: serde_json::Value =
            serde_json::from_str(&reread.get_chart_data_by_index_native(0).expect("읽기"))
                .expect("JSON");
        assert_eq!(
            json["series"][0]["values"][0], "91.7",
            "{name}: HWP5 저장에서 편집이 사라졌다"
        );
    }
}

/// **T4 — 편집이 HWPX→HWP5 변환을 넘어 살아남는다** (수용 기준 4).
///
/// B1 이 존재하는 이유 그 자체다. 대조군(①만 고친 문서)이 변환 후 **옛 값**이라는 것이
/// ②를 함께 써야 하는 이유의 살아 있는 근거다.
///
/// #4099 착지 전에는 잴 대상이 없었다 — 그때의 변환은 차트를 통째로 잃어
/// (`bin_data_id=60001` 이 그대로 나가 참조가 끊긴다) 새 값이든 옛 값이든 읽을 차트가
/// 없었다. 그래서 `#[ignore]` 로 잠들어 있었고, 짝으로 둔 회수 게이트가 "변환이 차트를
/// 보존하는가"를 관측해 착지 순간 실패로 깨웠다.
///
/// #4099 는 PR #4499 가 **머지된 게 아니라 CLOSED 되고** 메인테이너가 다른 SHA 로
/// 재착지시켰다(devel `e6a01730d` 기준). 잠들기 전 측정은 `origin/task4099 = e34e6d8b1`
/// 에 대한 것이었으므로 여기 값은 **착지본 기준 재측정**이다.
#[test]
fn the_edit_survives_conversion_to_hwp5() {
    for name in [
        "세로막대형/묶은세로막대형",
        "원형/쪼개진원형",
        "분산형/직선이있는분산형",
    ] {
        let path = manifest(&format!("samples/chart/{name}.hwpx"));

        let original: String = {
            let core = core_of(&path);
            let json: serde_json::Value =
                serde_json::from_str(&core.get_chart_data_by_index_native(0).expect("읽기"))
                    .expect("JSON");
            json["series"][0]["values"][0]
                .as_str()
                .expect("값")
                .to_string()
        };
        assert_ne!(original, "91.7", "{name}: 센티널이 원본과 같으면 판정이 공허하다");

        // ── 본 시험 — ①② 함께 기록 → 변환 → 새 값이 남는다
        let mut core = core_of(&path);
        let mut edits = edits_from(&core, 0);
        edits["series"][0]["values"][0] = serde_json::json!("91.7");
        assert_eq!(set_chart(&mut core, &edits)["ok"], true, "{name}");

        let hwp = core
            .export_hwp_with_adapter_snapshot()
            .expect("HWP5 변환 저장");
        let converted = DocumentCore::from_bytes(&hwp).expect("변환본 재파스");
        let json: serde_json::Value =
            serde_json::from_str(&converted.get_chart_data_by_index_native(0).expect("읽기"))
                .expect("JSON");
        assert_eq!(
            json["series"][0]["values"][0], "91.7",
            "{name}: 변환 후 새 값이 사라졌다"
        );

        // ── 대조군 — ①만 고치면 변환 후 옛 값이다
        let mut only_zip = core_of(&path);
        {
            let chart = collect_charts(only_zip.document())[0].clone();
            let zip_idx = chart.zip_part.expect("① 은 HWPX 에만 있다");
            let xml = only_zip.document().bin_data_content[zip_idx].data.load();
            let scan = scan_chart_values(&xml).expect("스캔");
            let patched = apply_value_edits(
                &xml,
                &scan,
                &[ValueEdit {
                    series: 0,
                    point: 0,
                    target: EditTarget::Value,
                    text: "91.7".to_string(),
                }],
            )
            .expect("패치");
            only_zip.document_mut().bin_data_content[zip_idx].data = patched.into();
        }
        let hwp = only_zip
            .export_hwp_with_adapter_snapshot()
            .expect("HWP5 변환 저장");
        let converted = DocumentCore::from_bytes(&hwp).expect("변환본 재파스");
        let json: serde_json::Value =
            serde_json::from_str(&converted.get_chart_data_by_index_native(0).expect("읽기"))
                .expect("JSON");
        assert_eq!(
            json["series"][0]["values"][0], original,
            "{name}: 대조군이 새 값이면 ①만 써도 된다는 뜻이라 설계 전제가 무너진다"
        );
    }
}

/// 검증에 걸리면 **한 칸도 쓰지 않는다** — `invalid[]` + `wrote: []`.
#[test]
fn every_refusal_writes_nothing() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let good = edits_from(&core_of(&path), 0);

    let mut cases: Vec<(&str, serde_json::Value)> = Vec::new();

    let mut e = good.clone();
    e["series"].as_array_mut().expect("series").pop();
    cases.push(("seriesCountMismatch", e));

    let mut e = good.clone();
    e["series"][0]["values"].as_array_mut().expect("values").pop();
    cases.push(("valueCountMismatch", e));

    let mut e = good.clone();
    e["series"][0]["values"][0] = serde_json::json!("칠십");
    cases.push(("notANumber", e));

    let mut e = good.clone();
    e["series"][0]["name"] = serde_json::json!("다른 이름");
    cases.push(("seriesNameMismatch", e));

    let mut e = good.clone();
    e["labels"][0] = serde_json::json!("다른 항목");
    cases.push(("categoryMismatch", e));

    for (reason, edits) in cases {
        let mut core = core_of(&path);
        let before = slot_bytes(&core);
        let out = set_chart(&mut core, &edits);

        assert_eq!(out["ok"], false, "{reason}: 거부되어야 한다 — {out}");
        let reasons: Vec<&str> = out["invalid"]
            .as_array()
            .expect("invalid")
            .iter()
            .map(|v| v["reason"].as_str().expect("reason"))
            .collect();
        assert!(reasons.contains(&reason), "{reason} 가 없다: {reasons:?}");
        assert!(out["wrote"].as_array().expect("wrote").is_empty(), "{reason}");
        assert_eq!(slot_bytes(&core), before, "{reason}: 거부했는데 바이트가 바뀌었다");
    }
}

/// `dryRun` 은 diff 만 내고 쓰지 않는다.
#[test]
fn dry_run_reports_the_diff_without_writing() {
    let path = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
    let mut core = core_of(&path);
    let before = slot_bytes(&core);

    let mut edits = edits_from(&core, 0);
    edits["series"][0]["values"][0] = serde_json::json!("91.7");
    edits["dryRun"] = serde_json::json!(true);

    let out = set_chart(&mut core, &edits);
    assert_eq!(out["ok"], true);
    assert_eq!(out["changedCount"], 1);
    assert_eq!(out["dryRun"], true);
    assert!(out["wrote"].as_array().expect("wrote").is_empty());
    assert_eq!(slot_bytes(&core), before, "dry-run 이 바이트를 바꿨다");
}

// ---------------------------------------------------------------------------
// Stage 5 — 실사용 문서 회귀 (코퍼스가 못 보여 준 변종)
// ---------------------------------------------------------------------------

/// 실사용 보고서 — 차트 2개, 계열 6개, **한 계열에 `c:cat` 이 없다**.
const REPORT_SAMPLE: &str = "samples/issue2006/1790387_prep_final_report.hwpx";

/// **라벨이 없는 계열이 있어도 값이 사라지지 않는다.**
///
/// `samples/chart/` 코퍼스는 전건이 계열마다 `c:cat` 을 갖고 있어 이 변종을 못 보여 준다.
/// 실사용 문서에서 첫 계열에 `c:cat` 이 없었고, 그 탓에
///
/// - 코어가 `series[0]` 의 라벨(빈 목록)을 문서 전체 라벨로 보고했고
/// - CSV 내보내기가 행 수를 라벨 수로 잡아 **6계열 × 6값 = 36칸이 통째로 사라진** CSV 를 냈다
///
/// 오류도 경고도 없었다. 죽지 않고 틀린 산출이라 눈으로 알아채기 어렵다.
#[test]
fn charts_with_partial_category_labels_keep_every_value() {
    use rhwp::document_core::queries::chart_csv::{from_csv, to_csv};

    let path = manifest(REPORT_SAMPLE);
    let core = core_of(&path);
    let charts = collect_charts(core.document());
    assert_eq!(charts.len(), 2, "이 문서는 차트 2개다");

    let read: serde_json::Value =
        serde_json::from_str(&core.get_chart_data_by_index_native(0).expect("읽기")).expect("JSON");
    assert_eq!(read["ok"], true);

    let labels: Vec<String> = read["labels"]
        .as_array()
        .expect("labels")
        .iter()
        .map(|v| v.as_str().expect("문자열").to_string())
        .collect();
    let series = read["series"].as_array().expect("series");
    assert_eq!(series.len(), 6, "계열 6개");
    assert!(
        !labels.is_empty(),
        "라벨을 가진 계열이 있는데 빈 목록이 나왔다 — series[0] 만 보고 있다"
    );

    let names: Vec<String> = series
        .iter()
        .map(|s| s["name"].as_str().unwrap_or_default().to_string())
        .collect();
    let values: Vec<Vec<String>> = series
        .iter()
        .map(|s| {
            s["values"]
                .as_array()
                .expect("values")
                .iter()
                .map(|v| v.as_str().expect("문자열").to_string())
                .collect()
        })
        .collect();
    assert!(values.iter().all(|v| v.len() == 6), "계열마다 값 6개");

    let csv = to_csv(&labels, &names, &values, false);
    let back = from_csv(&csv).expect("되읽기");
    assert_eq!(back.values, values, "CSV 왕복에서 값이 사라졌다");
    assert_eq!(back.names, names);
}

/// 그 문서의 무편집 CSV 왕복도 한 칸도 바꾸지 않는다.
#[test]
fn the_real_report_round_trips_without_changes() {
    let mut core = core_of(&manifest(REPORT_SAMPLE));
    let before = slot_bytes(&core);
    let edits = edits_from(&core, 0);
    let out = set_chart(&mut core, &edits);
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["changedCount"], 0, "{out}");
    assert_eq!(slot_bytes(&core), before, "무편집인데 바이트가 바뀌었다");
}

/// 그 문서에서도 편집은 ①② 에 함께 실린다.
#[test]
fn the_real_report_accepts_an_edit_in_both_representations() {
    let mut core = core_of(&manifest(REPORT_SAMPLE));
    let chart = collect_charts(core.document())[0].clone();
    let mut edits = edits_from(&core, 0);
    edits["series"][0]["values"][0] = serde_json::json!("0.999");

    let out = set_chart(&mut core, &edits);
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["changedCount"], 1, "{out}");
    assert_eq!(out["wrote"][0], "zipPart");
    assert_eq!(out["wrote"][1], "nestedCopy");

    let nested = core.document().bin_data_content[chart.nested_copy.expect("②")]
        .data
        .load();
    let ooxml = stream_of(&nested, OOXML_STREAM).expect("②");
    assert!(String::from_utf8_lossy(&ooxml).contains("<c:v>0.999</c:v>"));
}

/// 분산형은 X 도 편집 대상이다 — 계열이 X 를 공유하므로 두 칸이 함께 바뀐다.
#[test]
fn scatter_x_values_are_editable() {
    let path = manifest("samples/chart/분산형/직선이있는분산형.hwpx");
    let mut core = core_of(&path);
    let mut edits = edits_from(&core, 0);
    edits["labels"][0] = serde_json::json!("9.9");

    let out = set_chart(&mut core, &edits);
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["changedCount"], 2, "{out}");

    let json: serde_json::Value =
        serde_json::from_str(&core.get_chart_data_by_index_native(0).expect("읽기")).expect("JSON");
    assert_eq!(json["labels"][0], "9.9");
}

// ---------------------------------------------------------------------------
// Stage 6 — 한컴 육안 판정 번들
// ---------------------------------------------------------------------------

/// 판정 대상 7종. 폴더마다 하나씩 고르되 **첫 판정 축을 반드시 포함**한다.
///
/// #4055 스파이크는 `묶은세로막대형` 1종으로만 한컴 판정을 받았다. 원형(ofPie 포함)·
/// 분산형·주식형은 이번이 처음이라, 폴더의 알파벳 첫 파일이 아니라 **그 축을 대표하는
/// 변종**을 골랐다.
const JUDGMENT_TARGETS: &[(&str, &str)] = &[
    ("가로막대형", "묶은가로막대형"),
    // 4계열 OHLC — 계열 순서 규약(시/고/저/종)이 걸린 축. 첫 판정.
    ("기타", "시가고가저가종가"),
    ("라인", "표식이있는꺽은선형"),
    // 첫 열이 X 값인 유일한 축. 첫 판정.
    ("분산형", "직선및표식이있는분산형"),
    // 스파이크가 판정한 바로 그 문서 — 회귀 기준선.
    ("세로막대형", "묶은세로막대형"),
    // ofPie(원형대원형) — 보조 플롯이 있는 축. 첫 판정.
    ("원형", "원형대원형"),
    // c:numLit 리터럴 + 단일 계열 — 코퍼스에서 이 문서뿐이다.
    ("특이케이스", "가로막대형_하나만있을떄_단일시리즈제목"),
];

/// 눈에 띄는 센티널 — 원본 최대값의 10배.
///
/// 값을 읽지 않고 **모양만으로** 판정할 수 있어야 한다. 10배면 막대가 차트를 뚫고 솟고,
/// 원형은 그 조각이 원을 거의 다 먹는다. #4055 가 `4.3 → 91.7`(최대 5의 약 18배)로 잡은
/// 것과 같은 취지다.
fn sentinel_for(values: &[String]) -> String {
    let max = values
        .iter()
        .filter_map(|v| v.parse::<f64>().ok())
        .fold(0.0_f64, f64::max);
    let candidate = if max > 0.0 { max * 10.0 } else { 91.7 };
    format!("{candidate:.1}")
}

/// 차트 첫 계열의 값들을 읽는다.
fn first_series_values(core: &DocumentCore) -> Vec<String> {
    let read: serde_json::Value =
        serde_json::from_str(&core.get_chart_data_by_index_native(0).expect("읽기")).expect("JSON");
    read["series"][0]["values"]
        .as_array()
        .expect("values")
        .iter()
        .map(|v| v.as_str().expect("문자열").to_string())
        .collect()
}

/// Stage 6 — 한컴 육안 판정용 꾸러미를 만든다.
///
/// `output/` 에 파일을 쓰는 부작용이 있어 기본 실행에서 뺀다. 판정 직전에만 돌린다:
///
/// ```text
/// cargo test --profile release-test --test issue_4100_chart_data_edit -- --ignored --nocapture
/// ```
///
/// 각 산출은 내보내기 전에 **rhwp 가 다시 열 수 있는지**와 **의도한 표현에만 센티널이
/// 들어갔는지**를 스스로 확인한다 — 한컴이 못 열었을 때 그게 편집 탓인지 파일 조립 탓인지
/// 헷갈리지 않게 하기 위함이다(#4055 선례).
#[test]
#[ignore = "output/ 에 파일을 쓴다 — 한컴 판정 직전에만 실행"]
fn generate_hancom_judgment_bundle() {
    let out_dir = manifest("output/issue_4100_b1_judgment");
    std::fs::create_dir_all(&out_dir).expect("출력 디렉터리");

    let mut sheet = String::new();
    sheet.push_str("# #4100 B1 — 한컴 육안 판정표\n\n");
    sheet.push_str(
        "차트의 **첫 계열 첫 값**을 원본 최대값의 **10배**로 바꾼 편집본입니다.\n\
         반영되면 그 막대/조각/점이 차트를 압도하므로 **숫자를 읽을 필요가 없습니다.**\n\n",
    );
    sheet.push_str("## 보는 법\n\n");
    sheet.push_str(
        "1. `*-대조군.hwpx` 로 정상 모습을 눈에 익힙니다.\n\
         2. `*-편집.hwpx` / `*-편집.hwp` 를 엽니다. 첫 값이 압도적으로 커야 합니다.\n\
         3. 파일마다 **세 가지**를 함께 봐 주세요:\n   \
         (a) 열 때 오류·복구 대화상자가 뜨는가\n   \
         (b) 차트가 그려지는가 (틀만 나오고 속이 비면 실패입니다)\n   \
         (c) **차트를 더블클릭하면 편집기가 열리는가**\n\n\
         (b) 는 루트 CLSID 축입니다 — 재포장이 그것을 떨구면 한컴이 개체를 못 알아보고 \
         틀만 그립니다(#4097).\n\n",
    );
    sheet.push_str(
        "## 첫 판정 축\n\n\
         #4055 스파이크는 `묶은세로막대형` **1종**으로만 판정했습니다. \
         **원형(ofPie)·분산형·주식형·특이케이스는 이번이 처음**이라 특히 봐 주세요.\n\n",
    );
    sheet.push_str("## 산출물\n\n");
    sheet.push_str("| 파일 | 종류 | 원본 첫 값 | 편집 후 | 쓴 표현 |\n");
    sheet.push_str("|---|---|---|---|---|\n");

    let mut written = 0usize;
    for (folder, stem) in JUDGMENT_TARGETS {
        let hwpx_src = manifest(&format!("samples/chart/{folder}/{stem}.hwpx"));
        let hwp_src = manifest(&format!("samples/chart/{folder}/{stem}.hwp"));

        // 센티널은 두 포맷이 같아야 대조가 선다 (①==② 이므로 값도 같다).
        let values = first_series_values(&core_of(&hwpx_src));
        let before = values[0].clone();
        let sentinel = sentinel_for(&values);
        assert_ne!(
            before, sentinel,
            "{stem}: 센티널이 원본과 같으면 판정이 공허하다"
        );

        // 대조군 — 원본 그대로.
        std::fs::copy(&hwpx_src, out_dir.join(format!("{stem}-대조군.hwpx"))).expect("대조군 복사");
        written += 1;

        for (src, ext, expect_zip) in [(&hwpx_src, "hwpx", true), (&hwp_src, "hwp", false)] {
            let mut core = core_of(src);
            let mut edits = edits_from(&core, 0);
            edits["series"][0]["values"][0] = serde_json::json!(sentinel);
            let result = set_chart(&mut core, &edits);
            assert_eq!(result["ok"], true, "{stem}.{ext}: {result}");
            assert_eq!(result["changedCount"], 1, "{stem}.{ext}: {result}");

            let wrote: Vec<String> = result["wrote"]
                .as_array()
                .expect("wrote")
                .iter()
                .map(|v| v.as_str().expect("문자열").to_string())
                .collect();
            let expected_wrote: Vec<&str> = if expect_zip {
                vec!["zipPart", "nestedCopy"]
            } else {
                vec!["nestedCopy"]
            };
            assert_eq!(wrote, expected_wrote, "{stem}.{ext}");

            let bytes = if expect_zip {
                core.export_hwpx_native().expect("HWPX 저장")
            } else {
                core.export_hwp_native().expect("HWP5 저장")
            };

            // 자기 검증 — rhwp 가 다시 열고, 새 값이 실제로 들어 있다.
            let reread = DocumentCore::from_bytes(&bytes)
                .unwrap_or_else(|e| panic!("{stem}.{ext}: rhwp 가 다시 열지 못한다 — {e:?}"));
            assert_eq!(
                first_series_values(&reread)[0],
                sentinel,
                "{stem}.{ext}: 저장본에 새 값이 없다"
            );

            let name = format!("{stem}-편집.{ext}");
            std::fs::write(out_dir.join(&name), &bytes).expect("산출 쓰기");
            written += 1;

            sheet.push_str(&format!(
                "| `{name}` | {folder} | {before} | **{sentinel}** | {} |\n",
                wrote.join("+")
            ));
        }
        sheet.push_str(&format!(
            "| `{stem}-대조군.hwpx` | {folder} | {before} | (무편집) | — |\n"
        ));
    }

    // 변환 축 — HWPX 를 편집한 뒤 HWP5 로 변환한다. #4099 착지 전에는 만들 수 없었다
    // (변환이 차트를 통째로 잃었다). ①은 변환에서 사라지므로 이 파일이 보여 주는 값은
    // 곧 ②다 — "①만 고치면 안 된다"의 산 증거다.
    {
        let src = manifest("samples/chart/세로막대형/묶은세로막대형.hwpx");
        let mut core = core_of(&src);
        let sentinel = sentinel_for(&first_series_values(&core));

        let mut edits = edits_from(&core, 0);
        edits["series"][0]["values"][0] = serde_json::json!(sentinel);
        assert_eq!(set_chart(&mut core, &edits)["ok"], true);

        let bytes = core
            .export_hwp_with_adapter_snapshot()
            .expect("HWP5 변환 저장");
        let reread = DocumentCore::from_bytes(&bytes).expect("변환본 재파스");
        assert_eq!(
            first_series_values(&reread)[0],
            sentinel,
            "변환본에 새 값이 없다"
        );

        let name = "묶은세로막대형-편집-HWPX에서변환.hwp";
        std::fs::write(out_dir.join(name), &bytes).expect("변환본 쓰기");
        written += 1;
        sheet.push_str(&format!(
            "| `{name}` | 세로막대형 | — | **{sentinel}** | 변환 후 ② |\n"
        ));
    }

    sheet.push_str(&format!("\n총 {written} 파일.\n\n"));
    sheet.push_str(
        "> **`가로막대형_하나만있을떄_단일시리즈제목` 만 보는 법이 다릅니다.**\n\
         > 값이 **하나뿐**이라 축이 그 값에 맞춰 다시 잡힙니다 — 막대 길이는 그대로이고 \
         **축 눈금 숫자가 바뀝니다**(`0~5` → `0~45`). 이 파일만 축 숫자를 읽어 주세요.\n\n",
    );
    sheet.push_str(
        "## PDF 회신\n\n\
         각 파일을 한컴에서 열어 **같은 폴더에 PDF 로 저장**해 주시면, 대조군과 편집본의 \
         렌더를 144DPI 해시로 갈라 반영 여부를 데이터로 판정하겠습니다(#4055 와 같은 절차).\n\n\
         상세 설계는 `mydocs/plans/task_m100_4100.md`.\n",
    );

    std::fs::write(out_dir.join("PANJEONG.md"), sheet).expect("판정표 쓰기");
    println!("\n  판정 번들: {}", out_dir.display());
    println!("  파일 {written}개 + 판정표 PANJEONG.md");
    assert_eq!(written, 22, "7종 × 3 + 변환본 1");
}
