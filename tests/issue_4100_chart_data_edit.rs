//! [#4100] B1 엔진축 — 차트 숫자 데이터 편집.
//!
//! **Stage 1 게이트.** OOXML 차트 XML 위의 구조 스캐너와 최소 diff 패처만 검증한다.
//! 슬롯 해석(①`Chart/chartN.xml` ②중첩 CFB)·CSV 왕복·CLI 는 Stage 2 이후다.
//!
//! 스캐너를 따로 만드는 이유는 `src/ooxml_chart/parser.rs` 가 **손실 파서**이기
//! 때문이다 — `c:pt idx`·`c:f`·`c:externalData`·`extLst` 를 읽지 않아 파싱→재방출로
//! 왕복시키면 모델에 없는 것이 전부 사라진다. 코퍼스 28종 전건이 `c:extLst` 와
//! `ho:hncChartStyle` 을 갖고 있다. 그래서 `c:v` 텍스트 구간만 바꾸는 바이트 수술을 한다.

#[path = "support/issue_4055_chart_probe.rs"]
mod chart_probe_support;

use chart_probe_support::{chart_streams, corpus, manifest};

use rhwp::ooxml_chart::data::{scan_chart_values, SeriesAxis};
use rhwp::ooxml_chart::patch::{apply_value_edits, EditTarget, PatchError, ValueEdit};
use rhwp::ooxml_chart::OoxmlChart;

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
                let slice = &ooxml[point.span.clone()];
                assert_eq!(
                    slice,
                    point.text.as_bytes(),
                    "{}: 구간 {:?} 이 텍스트와 다르다",
                    path.display(),
                    point.span
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
