//! tests — renderer.rs 에서 무변동 이동
use super::*;

#[test]
fn test_render_empty_chart() {
    let chart = OoxmlChart::default();
    let svg = render_chart_svg(&chart, 0.0, 0.0, 100.0, 100.0);
    assert!(svg.contains("fallback"));
}

#[test]
fn test_pie_legend_reserves_space_regardless_of_series_name() {
    // 파이 범례는 카테고리 기반이라 시리즈 이름과 무관하게 항상 그려진다
    // (render_chart_svg 파이 분기는 legend_h/legend_w 계산과 무관하게
    // render_legend를 무조건 호출). 그런데 legend_h/legend_w는
    // `legend_visible`(= 시리즈 이름 존재 여부)로만 계산되므로, 시리즈
    // 이름이 없으면 범례가 그려지는데도 plot_h가 legend 공간을 빼지 않고
    // 계산되어(버그) 파이 반지름이 이름 있는 경우보다 부당하게 커진다.
    fn pie(name: &str) -> OoxmlChart {
        OoxmlChart {
            chart_type: OoxmlChartType::Pie,
            series: vec![OoxmlSeries {
                name: name.to_string(),
                values: vec![1.0, 2.0, 3.0],
                series_type: OoxmlChartType::Pie,
                ..Default::default()
            }],
            categories: vec!["가".into(), "나".into(), "다".into()],
            ..Default::default()
        }
    }
    fn pie_radius(svg: &str) -> f64 {
        // path의 `A{r},{r}` 반지름 값을 첫 슬라이스에서 추출
        let a_pos = svg.find(" A").expect("파이 path 없음");
        let rest = &svg[a_pos + 2..];
        let comma = rest.find(',').unwrap();
        rest[..comma].parse::<f64>().unwrap()
    }

    let named_svg = render_chart_svg(&pie("판매"), 0.0, 0.0, 400.0, 300.0);
    let unnamed_svg = render_chart_svg(&pie(""), 0.0, 0.0, 400.0, 300.0);

    // 두 경우 모두 범례가 그려진다 (카테고리 기반이므로 시리즈 이름 무관)
    assert!(named_svg.contains("hwp-chart-legend"));
    assert!(unnamed_svg.contains("hwp-chart-legend"));

    let named_r = pie_radius(&named_svg);
    let unnamed_r = pie_radius(&unnamed_svg);
    // 범례가 동일하게 그려지므로 예약 공간도 동일해야 하고, 따라서 두
    // 반지름이 같아야 한다. 버그 상태에서는 unnamed_r > named_r (범례
    // 공간이 예약되지 않아 파이가 legend와 겹치도록 더 크게 그려짐).
    assert_eq!(
        named_r, unnamed_r,
        "시리즈 이름 유무와 무관하게 파이 범례 공간이 동일하게 예약되어야 함 (named={named_r}, unnamed={unnamed_r})"
    );
}

#[test]
fn test_render_column() {
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Column,
        title: Some("test".to_string()),
        series: vec![OoxmlSeries {
            name: "A".to_string(),
            values: vec![1.0, 2.0, 3.0],
            series_type: OoxmlChartType::Column,
            ..Default::default()
        }],
        categories: vec!["x".to_string(), "y".to_string(), "z".to_string()],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert!(svg.contains("<rect"));
    assert!(svg.contains("test"));
}

#[test]
fn test_render_combo_dual_axis() {
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Column,
        has_secondary_axis: true,
        series: vec![
            OoxmlSeries {
                name: "금액".into(),
                values: vec![100.0, 200.0],
                series_type: OoxmlChartType::Column,
                axis_group: 0,
                color: Some(0x70AD47),
                ..Default::default()
            },
            OoxmlSeries {
                name: "건수".into(),
                values: vec![5.0, 10.0],
                series_type: OoxmlChartType::Line,
                axis_group: 1,
                color: Some(0x4472C4),
                ..Default::default()
            },
        ],
        categories: vec!["1월".into(), "2월".into()],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 500.0, 300.0);
    assert!(svg.contains("<rect")); // 막대
    assert!(svg.contains("<path")); // 라인
    assert!(svg.contains("금액"));
    assert!(svg.contains("건수"));
}

#[test]
fn test_format_num() {
    assert_eq!(format_num(1234.0, Some("#,##0")), "1,234");
    assert_eq!(format_num(-1234567.0, Some("#,##0")), "-1,234,567");
    assert_eq!(format_num(0.0, Some("#,##0")), "0");
    assert_eq!(format_num(123.0, None), "123");
}

#[test]
fn test_color_hex() {
    assert_eq!(color_hex(0xFFFF00FF), "#ff00ff");
}

// --- C1c (#1882) 갭②: 한컴 2022 기본 팔레트 ---

#[test]
fn test_default_palette_hancom_order() {
    // 색 미지정 3시리즈 → 팔레트 순환: 파랑 → 주황 → 회색 (한컴 2022 실측)
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Column,
        series: (0..3)
            .map(|i| OoxmlSeries {
                values: vec![1.0 + i as f64, 2.0],
                series_type: OoxmlChartType::Column,
                ..Default::default()
            })
            .collect(),
        categories: vec!["a".into(), "b".into()],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let i_blue = svg.find("#6183d7").expect("시리즈1 파랑");
    let i_orange = svg.find("#fe813b").expect("시리즈2 주황");
    let i_gray = svg.find("#b0b0b0").expect("시리즈3 회색");
    assert!(
        i_blue < i_orange && i_orange < i_gray,
        "팔레트 순서: 파랑→주황→회색"
    );
    assert!(!svg.contains("#70ad47"), "구 녹색-우선 팔레트 미사용");
}

// --- C1a Part B (#1453): 막대 누적 기하 ---

/// 데이터 막대(fill="#...", stroke 없음)의 x 좌표 목록. 배경/플롯 rect 제외.
/// (시리즈 name 비움 → 범례 미렌더 → 데이터 막대만 남음)
fn data_bar_xs(svg: &str) -> Vec<i64> {
    let mut xs = Vec::new();
    for chunk in svg.split("<rect ").skip(1) {
        let end = chunk.find('>').unwrap_or(chunk.len());
        let tag = &chunk[..end];
        // 배경/플롯 rect(stroke) + 범례 swatch(10×10) 제외 → 데이터 막대만.
        if tag.contains("stroke")
            || !tag.contains("fill=\"#")
            || tag.contains("width=\"10\" height=\"10\"")
        {
            continue;
        }
        if let Some(p) = tag.find("x=\"") {
            let s = p + 3;
            if let Some(e) = tag[s..].find('"') {
                if let Ok(v) = tag[s..s + e].parse::<f64>() {
                    xs.push((v * 10.0).round() as i64); // 0.1 단위 라운드
                }
            }
        }
    }
    xs
}

fn distinct(mut v: Vec<i64>) -> usize {
    v.sort_unstable();
    v.dedup();
    v.len()
}

fn bars_chart(grouping: BarGrouping) -> OoxmlChart {
    OoxmlChart {
        chart_type: OoxmlChartType::Column,
        grouping,
        // name 비움 → 범례 미렌더
        series: vec![
            OoxmlSeries {
                values: vec![4.0, 3.0],
                ..Default::default()
            },
            OoxmlSeries {
                values: vec![2.0, 1.0],
                ..Default::default()
            },
            OoxmlSeries {
                values: vec![2.0, 4.0],
                ..Default::default()
            },
        ],
        categories: vec!["a".into(), "b".into()],
        ..Default::default()
    }
}

#[test]
fn test_stacked_bars_share_x_per_category() {
    // 누적: 카테고리(2)당 단일 컬럼 → 서로 다른 x = 2개 (시리즈가 같은 x 공유)
    let svg = render_chart_svg(&bars_chart(BarGrouping::Stacked), 0.0, 0.0, 400.0, 300.0);
    assert_eq!(
        distinct(data_bar_xs(&svg)),
        2,
        "stacked는 카테고리당 단일 x"
    );
}

#[test]
fn test_clustered_bars_distinct_x() {
    // 묶은: 카테고리(2) × 시리즈(3) = 6개 서로 다른 x (무회귀 가드)
    let svg = render_chart_svg(&bars_chart(BarGrouping::Clustered), 0.0, 0.0, 400.0, 300.0);
    assert_eq!(
        distinct(data_bar_xs(&svg)),
        6,
        "clustered는 시리즈별 x 분리"
    );
}

#[test]
fn test_percent_stacked_axis_and_single_column() {
    // 백프로: % 축 라벨 + 카테고리당 단일 컬럼
    let svg = render_chart_svg(
        &bars_chart(BarGrouping::PercentStacked),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert!(svg.contains("100%"), "percentStacked는 % 축 라벨");
    assert!(svg.contains("0%"));
    assert_eq!(
        distinct(data_bar_xs(&svg)),
        2,
        "percent도 카테고리당 단일 x"
    );
}

// --- C1d (#2129): 라인 누적/백프로 기하 ---

/// 데이터 라인 path(fill="none" stroke-width="2")의 d 문자열 목록 (시리즈 순서).
/// 마커 path(fill=색)·격자선(line)·배경(rect)은 제외됨.
fn data_line_paths(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in svg.split("<path ").skip(1) {
        let end = chunk.find("/>").unwrap_or(chunk.len());
        let tag = &chunk[..end];
        if !tag.contains("fill=\"none\"") || !tag.contains("stroke-width=\"2\"") {
            continue;
        }
        if let Some(p) = tag.find("d=\"") {
            let s = p + 3;
            if let Some(e) = tag[s..].find('"') {
                out.push(tag[s..s + e].to_string());
            }
        }
    }
    out
}

/// path d의 (x,y) 점 목록 (`M`/`L` 접두 제거).
fn path_points(d: &str) -> Vec<(f64, f64)> {
    d.split_whitespace()
        .filter_map(|tok| {
            let t = tok.trim_start_matches(['M', 'L']);
            let (x, y) = t.split_once(',')?;
            Some((x.parse().ok()?, y.parse().ok()?))
        })
        .collect()
}

/// 3계열×4카테고리, 카테고리 합 최대 12.3 (합: 8.7/8.9/8.3/12.3 — 코퍼스 라인
/// 샘플과 동일 스케일). 개별값 최대 5.0 → 비누적 축 0~6, 누적 축 0~15로 구분됨.
fn line_chart(line_grouping: BarGrouping) -> OoxmlChart {
    OoxmlChart {
        chart_type: OoxmlChartType::Line,
        line_grouping,
        // name 비움 → 범례 미렌더
        series: vec![
            OoxmlSeries {
                values: vec![4.3, 2.5, 3.5, 4.5],
                ..Default::default()
            },
            OoxmlSeries {
                values: vec![2.4, 4.4, 1.8, 2.8],
                ..Default::default()
            },
            OoxmlSeries {
                values: vec![2.0, 2.0, 3.0, 5.0],
                ..Default::default()
            },
        ],
        categories: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        ..Default::default()
    }
}

#[test]
fn test_line_stacked_axis_from_category_sum() {
    // 누적 축 = 카테고리 합 최대(12.3) 기반 0~15 step 5 — 정답지 실측.
    // 개별값 최대(5.0) 기반 0~6이 아님.
    let svg = render_chart_svg(&line_chart(BarGrouping::Stacked), 0.0, 0.0, 400.0, 300.0);
    assert!(svg.contains(">15<"), "누적 축 max 15");
    assert!(!svg.contains(">6<"), "개별값 축(0~6) 미사용");
    assert!(!svg.contains(">14<"), "step 5 유지 (경계 headroom 미발동)");
}

#[test]
fn test_line_stacked_series_order() {
    // 누적: 시리즈2 첫 점(누적 6.7)이 시리즈1 첫 점(4.3) 위 (화면 y 작음)
    let svg = render_chart_svg(&line_chart(BarGrouping::Stacked), 0.0, 0.0, 400.0, 300.0);
    let paths = data_line_paths(&svg);
    assert_eq!(paths.len(), 3, "데이터 라인 3개");
    let y0 = path_points(&paths[0])[0].1;
    let y1 = path_points(&paths[1])[0].1;
    assert!(y1 < y0, "누적이면 시리즈2(y={y1})가 시리즈1(y={y0})보다 위");
}

#[test]
fn test_line_percent_axis_labels() {
    // 백프로: 축 0%~100% step 20% — 정답지 실측 (막대 percent와 동일 정책)
    let svg = render_chart_svg(
        &line_chart(BarGrouping::PercentStacked),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert!(svg.contains("100%"), "percent 축 100% 라벨");
    assert!(svg.contains("20%"), "step 20%");
}

#[test]
fn test_line_percent_top_series_flat() {
    // 최상위 시리즈 누적 = 카테고리 합 = 100% → 수평선 (정답지: 계열3이 100% 평행선)
    let svg = render_chart_svg(
        &line_chart(BarGrouping::PercentStacked),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    let paths = data_line_paths(&svg);
    let pts = path_points(&paths[2]);
    assert_eq!(pts.len(), 4);
    assert!(
        pts.windows(2).all(|w| (w[0].1 - w[1].1).abs() < 1e-6),
        "최상위 시리즈 y 전부 동일해야: {pts:?}"
    );
}

#[test]
fn test_line_percent_zero_sum_category_no_nan() {
    // 합 0 카테고리 → cum/0 NaN 방지 가드 (0%로 렌더)
    let mut chart = line_chart(BarGrouping::PercentStacked);
    chart.series = vec![
        OoxmlSeries {
            values: vec![1.0, 0.0],
            ..Default::default()
        },
        OoxmlSeries {
            values: vec![1.0, 0.0],
            ..Default::default()
        },
    ];
    chart.categories = vec!["a".into(), "b".into()];
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert!(!svg.contains("NaN"), "합 0 카테고리 NaN 가드");
}

/// `>{label}<` 텍스트 요소의 x 좌표.
fn text_label_x(svg: &str, label: &str) -> f64 {
    let i = svg
        .find(&format!(">{label}<"))
        .unwrap_or_else(|| panic!("라벨 {label} 없음"));
    let start = svg[..i].rfind("<text ").expect("text 태그");
    let tag = &svg[start..i];
    let p = tag.find("x=\"").expect("x 속성") + 3;
    let e = p + tag[p..].find('"').expect("닫는 따옴표");
    tag[p..e].parse().expect("x 파싱")
}

#[test]
fn test_line_points_at_category_slot_centers() {
    // 한컴 정합(작업지시자 시각판정 2026-07-10): 라인 점은 카테고리 슬롯 중앙 —
    // 첫/끝 점이 플롯 가장자리에 붙지 않고 반 슬롯 안쪽 (XML crossBetween=between).
    // 카테고리 라벨(슬롯 중앙, text-anchor=middle)과 x가 일치해야 한다.
    let svg = render_chart_svg(&line_chart(BarGrouping::Clustered), 0.0, 0.0, 400.0, 300.0);
    let pts = path_points(&data_line_paths(&svg)[0]);
    assert!(
        (pts[0].0 - text_label_x(&svg, "a")).abs() < 0.5,
        "첫 점 x={} ≠ 첫 카테고리 라벨 x={} (슬롯 중앙 아님)",
        pts[0].0,
        text_label_x(&svg, "a")
    );
    assert!(
        (pts[3].0 - text_label_x(&svg, "d")).abs() < 0.5,
        "끝 점 x={} ≠ 끝 카테고리 라벨 x={} (슬롯 중앙 아님)",
        pts[3].0,
        text_label_x(&svg, "d")
    );
}

/// `hwp-chart-marker` path의 d 문자열 목록 (시리즈×점 순서).
fn marker_ds(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in svg.split("<path ").skip(1) {
        let end = chunk.find("/>").unwrap_or(chunk.len());
        let tag = &chunk[..end];
        if !tag.contains("hwp-chart-marker") {
            continue;
        }
        if let Some(p) = tag.find("d=\"") {
            let s = p + 3;
            if let Some(e) = tag[s..].find('"') {
                out.push(tag[s..s + e].to_string());
            }
        }
    }
    out
}

#[test]
fn test_line_markers_rendered() {
    // line_markers=true → 마커 수 = 계열(3) × 점(4) = 12 (누적에서도 동일)
    let mut chart = line_chart(BarGrouping::Stacked);
    chart.line_markers = true;
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(marker_ds(&svg).len(), 12, "3계열×4점 마커");
}

#[test]
fn test_line_marker_shape_cycle() {
    // 계열별 기본 표식 사이클 ◆■▲ (정답지 실측 — 표식이있는누적꺽은선형)
    let mut chart = line_chart(BarGrouping::Clustered);
    chart.line_markers = true;
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let ds = marker_ds(&svg);
    assert_eq!(ds.len(), 12);
    let skel = |d: &str| {
        d.chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect::<String>()
    };
    // 시리즈별 첫 마커: [0]=◆, [4]=■, [8]=▲
    assert_eq!(skel(&ds[0]), "MLLLZ", "◆ 4각형");
    assert_eq!(skel(&ds[4]), "MLLLZ", "■ 4각형");
    assert_eq!(skel(&ds[8]), "MLLZ", "▲ 3각형");
    // ◆ vs ■ 구분: 첫 세그먼트가 ◆는 대각(y 변화), ■는 수평(y 동일)
    let dia = path_points(&ds[0]);
    assert!((dia[0].1 - dia[1].1).abs() > 1e-6, "◆ 첫 세그먼트 대각");
    let sq = path_points(&ds[4]);
    assert!((sq[0].1 - sq[1].1).abs() < 1e-6, "■ 첫 세그먼트 수평");
}

#[test]
fn test_line_marker_x_series4() {
    // 사이클 4번째는 × — OHLC 종가 정답지 실측 (C2a #2277, 종전 원 폴백 교체)
    let mut chart = line_chart(BarGrouping::Clustered);
    chart.line_markers = true;
    chart.series.push(OoxmlSeries {
        values: vec![1.0, 1.0, 1.0, 1.0],
        ..Default::default()
    });
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let ds = marker_ds(&svg);
    assert_eq!(ds.len(), 16);
    let skel: String = ds[12].chars().filter(|c| c.is_ascii_alphabetic()).collect();
    assert_eq!(skel, "MLML", "계열4는 × (두 대각선 열린 경로): {}", ds[12]);
    // × 는 stroke 기반 — 채움이면 안 보임 (열린 경로)
    assert!(
        svg.contains(&format!("d=\"{}\" fill=\"none\"", ds[12])),
        "× 마커는 fill=none + stroke=계열색"
    );
}

#[test]
fn test_line_no_markers_by_default() {
    // 기본값(line_markers=false) → 마커 없음 (꺽은선형/누적꺽은선형 무회귀)
    let svg = render_chart_svg(&line_chart(BarGrouping::Stacked), 0.0, 0.0, 400.0, 300.0);
    assert!(!svg.contains("hwp-chart-marker"), "기본은 무마커");
}

#[test]
fn test_line_clustered_unchanged() {
    // 비누적(기본, 꺽은선형 무회귀 핀): 개별값 축 0~6 + 시리즈1(4.3)이 시리즈2(2.4) 위
    let svg = render_chart_svg(&line_chart(BarGrouping::Clustered), 0.0, 0.0, 400.0, 300.0);
    assert!(svg.contains(">6<"), "개별값 축 max 6");
    assert!(!svg.contains(">15<"), "누적 축 미사용");
    let paths = data_line_paths(&svg);
    assert_eq!(paths.len(), 3);
    let y0 = path_points(&paths[0])[0].1;
    let y1 = path_points(&paths[1])[0].1;
    assert!(y0 < y1, "비누적: 개별값 기준 시리즈1이 위");
}

// --- C1b (#1660): 분산형(scatter) 렌더 ---

fn scatter_chart(style: ScatterStyle) -> OoxmlChart {
    OoxmlChart {
        chart_type: OoxmlChartType::Scatter,
        scatter_style: style,
        series: vec![OoxmlSeries {
            name: "Y1".into(),
            x_values: vec![0.7, 1.8, 2.6],
            values: vec![2.7, 3.2, 0.8],
            series_type: OoxmlChartType::Scatter,
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn test_render_scatter_marker_only() {
    // marker: 점만(사이클 글리프 — C2a #2277, 종전 circle 교체), 연결선 없음.
    let svg = render_chart_svg(&scatter_chart(ScatterStyle::Marker), 0.0, 0.0, 400.0, 300.0);
    assert!(
        !svg.contains("<circle"),
        "표식은 circle이 아니라 사이클 글리프"
    );
    assert_eq!(marker_ds(&svg).len(), 3, "1계열×3점 마커");
    assert!(data_line_paths(&svg).is_empty(), "marker는 연결선 없어야");
    assert!(!svg.contains("차트 (미지원)"));
    assert!(svg.contains("hwp-ooxml-chart\""));
}

#[test]
fn test_render_scatter_line_only() {
    // line: 직선만, 표식 없음.
    let svg = render_chart_svg(&scatter_chart(ScatterStyle::Line), 0.0, 0.0, 400.0, 300.0);
    assert_eq!(data_line_paths(&svg).len(), 1, "line은 연결선 있어야");
    assert!(marker_ds(&svg).is_empty(), "line은 표식 없어야");
    assert!(!svg.contains("<circle"));
    assert!(!svg.contains(" C"), "line은 직선(C 베지어 없음)");
}

#[test]
fn test_render_scatter_line_marker() {
    // lineMarker: 직선 + 표식.
    let svg = render_chart_svg(
        &scatter_chart(ScatterStyle::LineMarker),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert_eq!(data_line_paths(&svg).len(), 1);
    assert_eq!(marker_ds(&svg).len(), 3);
    assert!(!svg.contains(" C"), "lineMarker는 직선");
}

#[test]
fn test_render_scatter_smooth() {
    // smoothMarker: 곡선(cubic Bézier C) + 표식.
    let svg = render_chart_svg(
        &scatter_chart(ScatterStyle::SmoothMarker),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert_eq!(marker_ds(&svg).len(), 3);
    assert!(svg.contains(" C"), "smooth는 cubic Bézier(C) 곡선");
}

#[test]
fn test_scatter_markers_use_cycle() {
    // scatter 마커 = 라인과 동일 계열 사이클 (정답지 실측: 계열1 ◆ / 계열2 ■ —
    // 표식만있는분산형-2022.pdf. C2a #2277)
    let mut chart = scatter_chart(ScatterStyle::Marker);
    chart.series.push(OoxmlSeries {
        name: "Y2".into(),
        x_values: vec![0.7, 1.8, 2.6],
        values: vec![1.0, 2.0, 4.0],
        series_type: OoxmlChartType::Scatter,
        ..Default::default()
    });
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let ds = marker_ds(&svg);
    assert_eq!(ds.len(), 6, "2계열×3점");
    let skel = |d: &str| {
        d.chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect::<String>()
    };
    // 계열1=◆, 계열2=■ (둘 다 4각형 path — 첫 세그먼트 대각/수평으로 구분)
    assert_eq!(skel(&ds[0]), "MLLLZ", "계열1 ◆");
    assert_eq!(skel(&ds[3]), "MLLLZ", "계열2 ■");
    let dia = path_points(&ds[0]);
    assert!((dia[0].1 - dia[1].1).abs() > 1e-6, "◆ 첫 세그먼트 대각");
    let sq = path_points(&ds[3]);
    assert!((sq[0].1 - sq[1].1).abs() < 1e-6, "■ 첫 세그먼트 수평");
}

// --- C2a (#2277): stock (주식형) 렌더 ---

/// 코퍼스 실측 스케일 미러: 고가 max 59 → stock 전용 축 0~80 step 20.
/// n=3: 고/저/종(HLC), n=4: 시/고/저/종(OHLC — 1월만 하락(시44>종32), 나머지 상승).
fn stock_chart(n: usize) -> OoxmlChart {
    let ser = |name: &str, values: Vec<f64>, marker: SeriesMarker| OoxmlSeries {
        name: name.into(),
        values,
        marker_symbol: marker,
        series_type: OoxmlChartType::Stock,
        ..Default::default()
    };
    let mut series = Vec::new();
    if n == 4 {
        series.push(ser(
            "시가",
            vec![44.0, 32.0, 33.0, 34.0],
            SeriesMarker::None,
        ));
    }
    series.push(ser(
        "고가",
        vec![55.0, 57.0, 57.0, 59.0],
        SeriesMarker::None,
    ));
    series.push(ser(
        "저가",
        vec![11.0, 12.0, 13.0, 21.0],
        SeriesMarker::None,
    ));
    series.push(ser(
        "종가",
        vec![32.0, 35.0, 34.0, 35.0],
        SeriesMarker::Auto,
    ));
    OoxmlChart {
        chart_type: OoxmlChartType::Stock,
        has_hi_low_lines: true,
        has_up_down_bars: n == 4,
        up_down_gap_width: (n == 4).then_some(150.0),
        categories: vec!["1월".into(), "2월".into(), "3월".into(), "4월".into()],
        series,
        ..Default::default()
    }
}

#[test]
fn test_stock_axis_unconditional_headroom() {
    // 정답지 실측: max 59 → 0~80 step 20. 경계 조건부 headroom(nice_axis)이면 0~60.
    let svg = render_chart_svg(&stock_chart(3), 0.0, 0.0, 400.0, 300.0);
    assert!(
        svg.contains(">80<"),
        "stock 전용 +1 step 헤드룸 → 축 max 80"
    );
    assert!(svg.contains(">20<"), "step 20");
    assert!(!svg.contains(">100<"), "과확장 금지");
}

#[test]
fn test_stock_hilow_lines_per_category() {
    let svg = render_chart_svg(&stock_chart(3), 0.0, 0.0, 400.0, 300.0);
    assert_eq!(
        svg.matches("hwp-stock-hilow").count(),
        4,
        "카테고리당 고저선 1"
    );
    assert_eq!(
        svg.matches("hwp-stock-candle").count(),
        0,
        "HLC는 캔들 없음"
    );
}

#[test]
fn test_stock_ohlc_candles() {
    let svg = render_chart_svg(&stock_chart(4), 0.0, 0.0, 400.0, 300.0);
    let candles: Vec<&str> = svg
        .split("<rect ")
        .skip(1)
        .filter(|c| c[..c.find("/>").unwrap_or(c.len())].contains("hwp-stock-candle"))
        .collect();
    assert_eq!(candles.len(), 4, "카테고리당 캔들 1");
    let down = candles.iter().filter(|c| c.contains("#404040")).count();
    let up = candles
        .iter()
        .filter(|c| c.contains("fill=\"#ffffff\"") && c.contains("stroke=\"#000000\""))
        .count();
    assert_eq!(down, 1, "1월(시44>종32)만 하락 = 진회색 채움");
    assert_eq!(up, 3, "상승 = 흰 채움 + 검정 테두리");
}

#[test]
fn test_stock_close_marker_only() {
    // 종가(Auto)만 마커 — HLC 종가는 3번째 계열(si=2 → ▲), OHLC는 4번째(si=3 → ×)
    let skel = |d: &str| {
        d.chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect::<String>()
    };
    let svg3 = render_chart_svg(&stock_chart(3), 0.0, 0.0, 400.0, 300.0);
    let ds3 = marker_ds(&svg3);
    assert_eq!(ds3.len(), 4, "HLC: 종가 4점만 (고/저 무마커)");
    assert_eq!(skel(&ds3[0]), "MLLZ", "HLC 종가 ▲");
    let svg4 = render_chart_svg(&stock_chart(4), 0.0, 0.0, 400.0, 300.0);
    let ds4 = marker_ds(&svg4);
    assert_eq!(ds4.len(), 4, "OHLC: 종가 4점만");
    assert_eq!(skel(&ds4[0]), "MLML", "OHLC 종가 ×");
}

#[test]
fn test_stock_unusual_series_count_line_fallback() {
    // 계열 수 3/4 외 → render_line 폴백 (placeholder 재발 방지)
    let mut chart = stock_chart(3);
    chart.series.truncate(2);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(svg.matches("hwp-stock-hilow").count(), 0);
    assert!(!data_line_paths(&svg).is_empty(), "라인 폴백으로 렌더");
    assert!(!svg.contains("hwp-ooxml-chart-fallback"));
}

// --- C2a (#2277) stage3: 범례 순서 규칙 (정답지 28종 전수 실측 — 예외 0) ---

/// 이름 있는 3계열 차트 (범례 순서 검증용). 우측 범례 = 코퍼스 전 샘플 legendPos=r.
fn named3(chart_type: OoxmlChartType, grouping: BarGrouping) -> OoxmlChart {
    let ser = |i: usize| OoxmlSeries {
        name: format!("계열 {}", i + 1),
        values: vec![4.3, 2.5, 3.5, 4.5],
        series_type: chart_type,
        ..Default::default()
    };
    let mut c = OoxmlChart {
        chart_type,
        legend_pos: LegendPos::Right,
        series: vec![ser(0), ser(1), ser(2)],
        categories: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        ..Default::default()
    };
    match chart_type {
        OoxmlChartType::Line => c.line_grouping = grouping,
        _ => c.grouping = grouping,
    }
    c
}

fn first_legend_label(chart: &OoxmlChart) -> String {
    legend_items(chart)
        .first()
        .map(|(l, _, _)| l.clone())
        .unwrap_or_default()
}

#[test]
fn test_legend_order_rule_table() {
    use BarGrouping::*;
    use OoxmlChartType::*;
    // 역순 = (세로 값축 && 누적/백프로) || (가로막대 && 묶음) — 실측 28종 예외 0
    let cases: &[(OoxmlChartType, BarGrouping, bool)] = &[
        (Column, Stacked, true),
        (Column, PercentStacked, true),
        (Column, Clustered, false),
        (Bar, Clustered, true),
        (Bar, Stacked, false),
        (Bar, PercentStacked, false),
        (Line, Stacked, true),
        (Line, PercentStacked, true),
        (Line, Clustered, false), // standard 라인
    ];
    for &(t, g, reversed) in cases {
        let expect = if reversed { "계열 3" } else { "계열 1" };
        assert_eq!(
            first_legend_label(&named3(t, g)),
            expect,
            "{:?}/{:?} → 역순={}",
            t,
            g,
            reversed
        );
    }
}

#[test]
fn test_legend_order_3d_same_as_2d() {
    // 실측: 3D누적세로·3D묶은가로=역순, 3D묶은세로·3D누적가로=정순 — 2D와 동일 규칙
    let mut c = named3(OoxmlChartType::Column, BarGrouping::Stacked);
    c.is_3d = true;
    assert_eq!(first_legend_label(&c), "계열 3", "3D 누적세로 역순");
    let mut c = named3(OoxmlChartType::Bar, BarGrouping::Clustered);
    c.is_3d = true;
    assert_eq!(first_legend_label(&c), "계열 3", "3D 묶은가로 역순");
}

#[test]
fn test_legend_order_forward_for_stock_and_bottom_legend() {
    // stock = 정순 (실측: 고가→저가→종가)
    let mut c = stock_chart(3);
    c.legend_pos = LegendPos::Right;
    assert_eq!(first_legend_label(&c), "고가");
    // 하단 가로 범례는 코퍼스 미실측 — 역순 규칙 미적용 (현행 정순 유지)
    let mut c2 = named3(OoxmlChartType::Column, BarGrouping::Stacked);
    c2.legend_pos = LegendPos::Bottom;
    assert_eq!(first_legend_label(&c2), "계열 1");
}

#[test]
fn test_legend_order_combo_forward() {
    // 콤보(막대+라인)는 정순 고정 — 역순 규칙에서 명시 제외
    let mut c = named3(OoxmlChartType::Column, BarGrouping::Stacked);
    c.series[2].series_type = OoxmlChartType::Line;
    assert_eq!(first_legend_label(&c), "계열 1");
}

#[test]
fn test_hbar_clustered_slot_series1_at_bottom() {
    // 묶은가로 실측: 슬롯 내 위→아래 = 계열3→2→1 (계열1이 맨 아래 = y 최대).
    // 범례 역순과 세트로 시각 일치 (#2277 stage3).
    let c = named3(OoxmlChartType::Bar, BarGrouping::Clustered);
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    let rect_y = |color: &str| -> f64 {
        let tag = svg
            .split("<rect ")
            .skip(1)
            .map(|ch| &ch[..ch.find("/>").unwrap_or(ch.len())])
            .find(|t| t.contains(&format!("fill=\"{}\"", color)))
            .unwrap_or_else(|| panic!("{color} 막대 없음"));
        let p = tag.find("y=\"").unwrap() + 3;
        let e = tag[p..].find('"').unwrap();
        tag[p..p + e].parse().unwrap()
    };
    assert!(
        rect_y("#6183d7") > rect_y("#b0b0b0"),
        "계열1(파랑)이 슬롯 맨 아래 (y가 계열3(회색)보다 커야)"
    );
}

// --- C2a (#2277) stage5: 특이케이스 1카테고리 미니차트 0.5축 ---

#[test]
fn test_hbar_single_category_half_step() {
    // 특이케이스 실측(C1c v2 기록 → #2277 반영): 가로막대 1카테고리 미니차트는
    // 축 범위 유지·step 절반 (4.3 → 0~5 step 0.5, 라벨 11개). 단일 샘플 근거라
    // 가로·1카테고리·비누적·비3D로 좁게 게이트 — 코퍼스 나머지 27종(전부
    // 4카테고리) 무영향.
    let mut c = named3(OoxmlChartType::Bar, BarGrouping::Clustered);
    c.series.truncate(1);
    c.series[0].values = vec![4.3];
    c.categories = vec!["항목 1".into()];
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    for want in [">0.5<", ">4.5<", ">5<"] {
        assert!(svg.contains(want), "미니차트 0.5 step 라벨 {want} 없음");
    }
    // 다카테고리 무회귀 핀: step 1 유지
    let svg4 = render_chart_svg(
        &named3(OoxmlChartType::Bar, BarGrouping::Clustered),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert!(!svg4.contains(">0.5<"), "다카테고리는 step 절반 미적용");
}

// --- C2a (#2277) stage4: 범례 스와치 글리프 (SwatchKind) ---

/// 범례 그룹(`hwp-chart-legend`) 조각만 잘라 반환.
fn legend_fragment(svg: &str) -> &str {
    let start = svg
        .find("<g class=\"hwp-chart-legend\">")
        .expect("범례 그룹 없음");
    let end = svg[start..].find("</g>").expect("범례 그룹 종료") + start;
    &svg[start..end]
}

#[test]
fn test_legend_swatch_marker_line_has_glyph() {
    // 실측(표식이있는꺽은선형): 스와치 = 선분 + 플롯 마커와 동일 글리프 (—◆—)
    let mut c = named3(OoxmlChartType::Line, BarGrouping::Clustered);
    c.line_markers = true;
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    let legend = legend_fragment(&svg);
    assert_eq!(
        legend.matches("hwp-legend-glyph").count(),
        3,
        "계열별 글리프 1개"
    );
    assert_eq!(
        legend.matches("stroke-width=\"2\"").count(),
        3,
        "선분 스와치 유지"
    );
    // 플롯 마커 카운트 무오염 (issue_2129 보호 — 별도 클래스)
    assert_eq!(
        svg.matches("hwp-chart-marker").count(),
        12,
        "플롯 마커 12개 불변"
    );
}

#[test]
fn test_legend_swatch_plain_line_no_glyph() {
    // 실측(꺽은선형): 무표식 라인 스와치 = 선분만 (글리프 없음, 현행 유지)
    let c = named3(OoxmlChartType::Line, BarGrouping::Clustered);
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    let legend = legend_fragment(&svg);
    assert_eq!(legend.matches("hwp-legend-glyph").count(), 0);
    assert_eq!(legend.matches("stroke-width=\"2\"").count(), 3);
}

#[test]
fn test_legend_swatch_scatter_marker_only_glyph_only() {
    // 실측(표식만있는분산형): 스와치 = 마커 글리프만 (선분 없음)
    let mut c = scatter_chart(ScatterStyle::Marker);
    c.legend_pos = LegendPos::Right;
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    let legend = legend_fragment(&svg);
    assert_eq!(
        legend.matches("hwp-legend-glyph").count(),
        1,
        "1계열 글리프"
    );
    assert_eq!(
        legend.matches("stroke-width=\"2\"").count(),
        0,
        "표식만은 선분 스와치 없음"
    );
}

#[test]
fn test_legend_swatch_scatter_line_marker_line_glyph() {
    // 실측(직선및표식/곡선및표식): 스와치 = 선분 + 글리프
    let mut c = scatter_chart(ScatterStyle::LineMarker);
    c.legend_pos = LegendPos::Right;
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    let legend = legend_fragment(&svg);
    assert_eq!(legend.matches("hwp-legend-glyph").count(), 1);
    assert_eq!(
        legend.matches("stroke-width=\"2\"").count(),
        1,
        "선분 스와치 동반"
    );
}

#[test]
fn test_legend_swatch_stock_blank_except_close() {
    // 실측(stock 2종): 시/고/저 스와치 없음(라벨 정렬 유지), 종가만 글리프
    let mut c = stock_chart(4);
    c.legend_pos = LegendPos::Right;
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    let legend = legend_fragment(&svg);
    assert_eq!(
        legend.matches("hwp-legend-glyph").count(),
        1,
        "종가(Auto)만 글리프"
    );
    assert_eq!(
        legend.matches("<rect ").count(),
        0,
        "stock 범례에 색 사각형 스와치 없음"
    );
    assert_eq!(
        legend.matches("stroke-width=\"2\"").count(),
        0,
        "stock 범례에 선분 스와치 없음"
    );
    for name in ["시가", "고가", "저가", "종가"] {
        assert!(
            legend.contains(&format!(">{name}</text>")),
            "{name} 라벨 유지"
        );
    }
}

#[test]
fn test_legend_swatch_square_unchanged_for_bars() {
    // issue_1882 보호: 막대 범례 스와치 = 10×10 색 사각형 문자열 불변
    let c = named3(OoxmlChartType::Column, BarGrouping::Clustered);
    let svg = render_chart_svg(&c, 0.0, 0.0, 400.0, 300.0);
    let legend = legend_fragment(&svg);
    assert_eq!(legend.matches("width=\"10\" height=\"10\"").count(), 3);
    assert_eq!(legend.matches("hwp-legend-glyph").count(), 0);
}

#[test]
fn test_render_scatter_decimal_axis_labels() {
    // 소수 데이터 → 소수 축 라벨 (format_num 정수 반올림이 아니라 format_axis_num).
    // 0-baseline clamp 후 X 0~3(step 0.5) → 눈금 0.5/1.5/2.5 등 (소수 라벨). — C1c 갭④
    let svg = render_chart_svg(&scatter_chart(ScatterStyle::Marker), 0.0, 0.0, 400.0, 300.0);
    assert!(
        svg.contains(">2.5<"),
        "분산형 축은 소수 라벨이어야 (정수 반올림 시 '2'로 손상)",
    );
    assert!(!svg.contains("차트 (미지원)"));
}

#[test]
fn test_render_scatter_zero_baseline() {
    // 양수 데이터 → 축이 0부터 (한컴 분산형 PDF 정합). 0 라벨이 X·Y에 존재.
    let svg = render_chart_svg(&scatter_chart(ScatterStyle::Marker), 0.0, 0.0, 400.0, 300.0);
    assert!(svg.contains(">0<"), "분산형 축은 0 기준선이어야");
}

// --- C1c (#1882) 갭①: 자동 제목 ---

#[test]
fn test_render_auto_title_placeholder() {
    // c:title 요소 존재 + autoTitleDeleted=0 + 명시 텍스트 없음 →
    // 한컴처럼 자동 제목 "차트 제목" 렌더 (regular weight).
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Column,
        has_title_elem: true,
        series: vec![OoxmlSeries {
            values: vec![1.0, 2.0],
            series_type: OoxmlChartType::Column,
            ..Default::default()
        }],
        categories: vec!["a".into(), "b".into()],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert!(svg.contains("차트 제목"), "자동 제목 placeholder 렌더");
    assert!(
        !svg.contains("font-weight=\"600\""),
        "한컴 제목은 regular weight (600 아님)"
    );
}

#[test]
fn test_render_no_auto_title_when_deleted_or_absent() {
    // autoTitleDeleted=1 또는 c:title 요소 자체가 없으면 자동 제목 없음.
    let base = OoxmlChart {
        chart_type: OoxmlChartType::Column,
        series: vec![OoxmlSeries {
            values: vec![1.0, 2.0],
            series_type: OoxmlChartType::Column,
            ..Default::default()
        }],
        categories: vec!["a".into(), "b".into()],
        ..Default::default()
    };
    let deleted = OoxmlChart {
        has_title_elem: true,
        auto_title_deleted: true,
        ..base.clone()
    };
    assert!(!render_chart_svg(&deleted, 0.0, 0.0, 400.0, 300.0).contains("차트 제목"));
    // has_title_elem=false (기본값) → 자동 제목 없음
    assert!(!render_chart_svg(&base, 0.0, 0.0, 400.0, 300.0).contains("차트 제목"));
}

// --- #1882 v2: 단일 시리즈 이름 자동 제목 fallback ---

/// 제목 텍스트(font-size 13 — 범례/축 라벨(10px)과 구분)만 추출
fn title_text(svg: &str) -> Option<String> {
    let chunk = svg.split("font-size=\"13\"").nth(1)?;
    let s = chunk.find('>')? + 1;
    let e = s + chunk[s..].find('<')?;
    Some(chunk[s..e].to_string())
}

fn single_series_chart(name: &str, chart_type: OoxmlChartType) -> OoxmlChart {
    OoxmlChart {
        chart_type,
        has_title_elem: true,
        series: vec![OoxmlSeries {
            name: name.into(),
            values: vec![4.3, 2.5],
            series_type: chart_type,
            ..Default::default()
        }],
        categories: vec!["a".into(), "b".into()],
        ..Default::default()
    }
}

#[test]
fn test_render_auto_title_single_series_uses_name() {
    // 한컴 실측: 단일 시리즈면 자동 제목 = 시리즈 이름 (원형 5종 "판매",
    // 단일 시리즈 가로막대 "계열 1" — 차트 종류 불문 시리즈 수 기준 규칙).
    for chart_type in [
        OoxmlChartType::Pie,
        OoxmlChartType::Bar,
        OoxmlChartType::Column,
    ] {
        let svg = render_chart_svg(
            &single_series_chart("판매", chart_type),
            0.0,
            0.0,
            400.0,
            300.0,
        );
        assert_eq!(
            title_text(&svg).as_deref(),
            Some("판매"),
            "{chart_type:?}: 단일 시리즈 이름이 제목이어야"
        );
    }
}

#[test]
fn test_render_auto_title_single_series_fallbacks() {
    // 단일 시리즈여도 이름이 비면 placeholder 유지.
    let unnamed = single_series_chart("", OoxmlChartType::Column);
    let svg = render_chart_svg(&unnamed, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(title_text(&svg).as_deref(), Some("차트 제목"));

    // 명시 제목이 있으면 시리즈 이름보다 우선.
    let mut explicit = single_series_chart("판매", OoxmlChartType::Column);
    explicit.title = Some("명시 제목".into());
    let svg = render_chart_svg(&explicit, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(title_text(&svg).as_deref(), Some("명시 제목"));

    // autoTitleDeleted=1이면 시리즈 이름 fallback도 억제 (제목 요소 없음).
    let mut suppressed = single_series_chart("판매", OoxmlChartType::Column);
    suppressed.auto_title_deleted = true;
    let svg = render_chart_svg(&suppressed, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(title_text(&svg), None);

    // 다계열이면 종전대로 placeholder (이름 있는 2계열).
    let mut multi = single_series_chart("판매", OoxmlChartType::Column);
    multi.series.push(OoxmlSeries {
        name: "재고".into(),
        values: vec![1.0, 2.0],
        series_type: OoxmlChartType::Column,
        ..Default::default()
    });
    let svg = render_chart_svg(&multi, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(title_text(&svg).as_deref(), Some("차트 제목"));
}

// --- C1c (#1882) 갭③: 범례 우측 배치 ---

/// `hwp-chart-legend` 그룹 안 첫 `<text>`의 지정 속성 값
fn legend_first_text_attr(svg: &str, attr: &str) -> f64 {
    let g = svg
        .split("class=\"hwp-chart-legend\"")
        .nth(1)
        .expect("범례 그룹");
    let text = g.split("<text ").nth(1).expect("범례 텍스트");
    let pat = format!("{attr}=\"");
    let s = text.find(&pat).expect("attr") + pat.len();
    let e = s + text[s..].find('"').expect("attr close");
    text[s..e].parse().expect("f64")
}

fn named_chart(legend_pos: LegendPos) -> OoxmlChart {
    OoxmlChart {
        chart_type: OoxmlChartType::Column,
        legend_pos,
        series: vec![
            OoxmlSeries {
                name: "계열 1".into(),
                values: vec![1.0, 2.0],
                series_type: OoxmlChartType::Column,
                ..Default::default()
            },
            OoxmlSeries {
                name: "계열 2".into(),
                values: vec![3.0, 4.0],
                series_type: OoxmlChartType::Column,
                ..Default::default()
            },
        ],
        categories: vec!["a".into(), "b".into()],
        ..Default::default()
    }
}

#[test]
fn test_render_legend_right_vertical() {
    // legendPos=Right → 범례가 플롯 우측(x > 차트 폭 65%)에 세로 스택.
    let svg = render_chart_svg(&named_chart(LegendPos::Right), 0.0, 0.0, 400.0, 300.0);
    let tx = legend_first_text_attr(&svg, "x");
    assert!(tx > 260.0, "우측 범례 텍스트 x={tx} > 260 이어야");
    let ty = legend_first_text_attr(&svg, "y");
    assert!(ty < 250.0, "우측 범례는 플롯 세로 중앙부(y={ty} < 250)여야");
}

#[test]
fn test_render_legend_bottom_default_unchanged() {
    // 기본(Bottom) → 종전 하단 가로 배치 유지.
    let svg = render_chart_svg(&named_chart(LegendPos::Bottom), 0.0, 0.0, 400.0, 300.0);
    let ty = legend_first_text_attr(&svg, "y");
    assert!(ty > 270.0, "하단 범례 텍스트 y={ty} > 270 이어야");
}

#[test]
fn test_horizontal_bar_category_labels_not_clipped() {
    // 가로 막대: 좌측은 숫자 값축이 아니라 카테고리 라벨("항목 1" 등) —
    // left_pad를 값축 숫자 폭(2자≈32px)으로 잡으면 라벨이 차트 왼쪽 밖으로 잘림.
    // 카테고리 라벨 anchor x(= plot_x - 4)가 라벨 폭 이상 확보돼야 한다.
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Bar,
        series: vec![OoxmlSeries {
            values: vec![4.3, 2.5, 3.5, 4.5],
            series_type: OoxmlChartType::Bar,
            ..Default::default()
        }],
        categories: vec![
            "항목 1".into(),
            "항목 2".into(),
            "항목 3".into(),
            "항목 4".into(),
        ],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let chunk = svg.split(">항목 1<").next().expect("카테고리 라벨");
    let tag_start = chunk.rfind("<text ").expect("text 태그");
    let x = attr_f64_of(&chunk[tag_start..], "x=\"").expect("x 속성");
    assert!(
        x >= 45.0,
        "카테고리 라벨 anchor x={x} — 라벨 폭(≈40px)만큼 왼쪽 여백 필요"
    );
}

fn attr_f64_of(tag: &str, pat: &str) -> Option<f64> {
    let s = tag.find(pat)? + pat.len();
    let e = s + tag[s..].find('"')?;
    tag[s..e].parse().ok()
}

#[test]
fn test_render_legend_right_narrow_chart_no_panic() {
    // 폭이 좁으면(w*0.30 < 50) clamp(50, w*0.30)이 min>max로 패닉하던 결함 가드 —
    // 하단 폴백으로 렌더되고 패닉하지 않아야 한다. NaN 폭도 패닉 금지.
    let svg = render_chart_svg(&named_chart(LegendPos::Right), 0.0, 0.0, 100.0, 80.0);
    assert!(
        svg.contains("hwp-chart-legend"),
        "좁은 차트는 하단 폴백 범례"
    );
    let _ = render_chart_svg(&named_chart(LegendPos::Right), 0.0, 0.0, f64::NAN, 80.0);
}

// --- C1c (#1882) 갭④: Y축 headroom + step 기반 눈금 (한컴 실측 앵커 3점) ---

#[test]
fn test_axis_headroom_bar_max_on_boundary() {
    // 한컴 실측 앵커: 세로막대 max 5.0 → 축 0~6, 세로 값축 3칸 정책으로
    // step 2 → 성긴 라벨 0,2,4,6 (묶은세로막대형-2022.pdf).
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Column,
        series: vec![
            OoxmlSeries {
                values: vec![4.3, 2.5, 3.5, 4.5],
                series_type: OoxmlChartType::Column,
                ..Default::default()
            },
            OoxmlSeries {
                values: vec![2.0, 2.0, 3.0, 5.0],
                series_type: OoxmlChartType::Column,
                ..Default::default()
            },
        ],
        categories: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    for want in [">0<", ">2<", ">4<", ">6<"] {
        assert!(svg.contains(want), "라벨 {want} 있어야 (0~6, step 2)");
    }
    for absent in [">1<", ">3<", ">5<"] {
        assert!(!svg.contains(absent), "라벨 {absent} 없어야 (성긴 라벨)");
    }
}

#[test]
fn test_axis_vertical_stacked_coarse_ticks() {
    // 한컴 실측: 누적'세로'막대(합 max 12.3) → 축 0~15 step 5 (세로 값축은 ~3칸).
    // 같은 데이터의 누적'가로'막대는 0~14 step 2 — 방향별 눈금 밀도가 다름.
    let mut chart = bars_chart(BarGrouping::Stacked);
    chart.series[0].values = vec![4.3, 2.5, 3.5, 4.5];
    chart.series[1].values = vec![2.4, 4.4, 1.8, 2.8];
    chart.series[2].values = vec![2.0, 2.0, 3.0, 5.0];
    chart.categories = vec!["a".into(), "b".into(), "c".into(), "d".into()];
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    for want in [">5<", ">10<", ">15<"] {
        assert!(
            svg.contains(want),
            "세로 누적 라벨 {want} 있어야 (0~15 step 5)"
        );
    }
    for absent in [">14<", ">2<", ">4<"] {
        assert!(!svg.contains(absent), "세로 누적 라벨 {absent} 없어야");
    }
}

#[test]
fn test_axis_horizontal_stacked_fine_ticks() {
    // 한컴 실측: 누적'가로'막대(합 max 12.3) → 축 0~14 step 2 (가로 값축은 ~5칸).
    let mut chart = bars_chart(BarGrouping::Stacked);
    chart.chart_type = OoxmlChartType::Bar;
    chart.series[0].values = vec![4.3, 2.5, 3.5, 4.5];
    chart.series[1].values = vec![2.4, 4.4, 1.8, 2.8];
    chart.series[2].values = vec![2.0, 2.0, 3.0, 5.0];
    for s in &mut chart.series {
        s.series_type = OoxmlChartType::Bar;
    }
    chart.categories = vec!["a".into(), "b".into(), "c".into(), "d".into()];
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    for want in [">2<", ">14<"] {
        assert!(
            svg.contains(want),
            "가로 누적 라벨 {want} 있어야 (0~14 step 2)"
        );
    }
    assert!(!svg.contains(">15<"), "가로 누적은 0~14 (15 아님)");
}

#[test]
fn test_axis_horizontal_clustered_headroom_keeps_step() {
    // 한컴 실측: 묶은'가로'막대(max 5.0, step 1 경계) → 0~6 **step 1 유지**
    // (라벨 0~6 전부 — 경계 headroom 후 step 재계산하지 않음).
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Bar,
        series: vec![
            OoxmlSeries {
                values: vec![4.3, 2.5, 3.5, 4.5],
                series_type: OoxmlChartType::Bar,
                ..Default::default()
            },
            OoxmlSeries {
                values: vec![2.0, 2.0, 3.0, 5.0],
                series_type: OoxmlChartType::Bar,
                ..Default::default()
            },
        ],
        categories: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    for want in [">1<", ">3<", ">5<", ">6<"] {
        assert!(
            svg.contains(want),
            "가로 묶은 라벨 {want} 있어야 (0~6 step 1)"
        );
    }
}

#[test]
fn test_axis_3d_clustered_no_headroom() {
    // 한컴 실측: 3D 묶은막대는 세로·가로 모두 0~5 step 1 — 촘촘 눈금 + 경계
    // headroom 없음 (2D 묶은세로 0~6 step 2 / 2D 묶은가로 0~6 step 1과 다름).
    for chart_type in [OoxmlChartType::Column, OoxmlChartType::Bar] {
        let chart = OoxmlChart {
            chart_type,
            is_3d: true,
            series: vec![OoxmlSeries {
                values: vec![4.3, 2.5, 3.5, 5.0],
                series_type: chart_type,
                ..Default::default()
            }],
            categories: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            ..Default::default()
        };
        let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
        for want in [">1<", ">4<", ">5<"] {
            assert!(
                svg.contains(want),
                "{chart_type:?}: 3D 묶은 라벨 {want} (0~5 step 1)"
            );
        }
        assert!(
            !svg.contains(">6<"),
            "{chart_type:?}: 3D 묶은은 headroom 없음 (0~5)"
        );
    }
}

#[test]
fn test_axis_3d_stacked_vertical_extra_headroom() {
    // 한컴 실측: 3D 누적'세로'(합 max 12.3) → 0~20 step 5 (2D 15 + 1 step).
    let mut chart = bars_chart(BarGrouping::Stacked);
    chart.is_3d = true;
    chart.series[0].values = vec![4.3, 2.5, 3.5, 4.5];
    chart.series[1].values = vec![2.4, 4.4, 1.8, 2.8];
    chart.series[2].values = vec![2.0, 2.0, 3.0, 5.0];
    chart.categories = vec!["a".into(), "b".into(), "c".into(), "d".into()];
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert!(svg.contains(">20<"), "3D 누적세로는 0~20 (2D 15 + 1 step)");
    assert!(!svg.contains(">14<"));

    // 3D 누적'가로'는 2D 가로와 동일 (0~14 step 2, 실측).
    let mut hchart = chart.clone();
    hchart.chart_type = OoxmlChartType::Bar;
    for s in &mut hchart.series {
        s.series_type = OoxmlChartType::Bar;
    }
    let hsvg = render_chart_svg(&hchart, 0.0, 0.0, 400.0, 300.0);
    assert!(hsvg.contains(">14<"), "3D 누적가로는 2D와 동일 0~14");
    assert!(!hsvg.contains(">16<") && !hsvg.contains(">20<"));
}

#[test]
fn test_horizontal_bar_categories_bottom_up() {
    // 한컴 실측: 가로막대는 카테고리를 아래→위로 배치 (항목 1이 맨 아래).
    let chart = OoxmlChart {
        chart_type: OoxmlChartType::Bar,
        series: vec![OoxmlSeries {
            values: vec![1.0, 2.0],
            series_type: OoxmlChartType::Bar,
            ..Default::default()
        }],
        categories: vec!["catA".into(), "catB".into()],
        ..Default::default()
    };
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let y_of = |label: &str| -> f64 {
        let chunk = svg.split(&format!(">{label}<")).next().expect("라벨");
        let tag = &chunk[chunk.rfind("<text ").expect("text")..];
        attr_f64_of(tag, "y=\"").expect("y")
    };
    assert!(
        y_of("catA") > y_of("catB"),
        "첫 카테고리(catA)가 아래쪽(y 큼)이어야: catA={} catB={}",
        y_of("catA"),
        y_of("catB"),
    );
}

#[test]
fn test_stacked_vertical_bars_align_with_category_labels() {
    // 누적 세로 막대 x가 plot_y 기반으로 계산되던 결함 가드 — 막대 중심이
    // 카테고리 라벨 중심과 일치해야 한다 (y 오프셋이 있는 배치에서 검증).
    let svg = render_chart_svg(&bars_chart(BarGrouping::Stacked), 0.0, 100.0, 400.0, 300.0);
    let label_chunk = svg.split(">a<").next().expect("라벨 a");
    let label_x = attr_f64_of(
        &label_chunk[label_chunk.rfind("<text ").expect("text")..],
        "x=\"",
    )
    .expect("라벨 x");
    let bar_chunk = svg.split("fill=\"#6183d7\"").next().expect("첫 파랑 막대");
    let bar_tag = &bar_chunk[bar_chunk.rfind("<rect ").expect("rect")..];
    let bar_center = attr_f64_of(bar_tag, "x=\"").expect("x")
        + attr_f64_of(bar_tag, "width=\"").expect("w") / 2.0;
    assert!(
        (bar_center - label_x).abs() < 2.0,
        "누적 막대 중심({bar_center})과 라벨 중심({label_x}) 불일치",
    );
}

#[test]
fn test_axis_headroom_scatter_y_on_boundary() {
    // 한컴 실측 앵커: scatter Y max 4.0(step 1 경계) → 축 0~5, 라벨 1 간격
    // (표식만있는분산형-2022.pdf).
    let mut chart = scatter_chart(ScatterStyle::Marker);
    chart.series[0].values = vec![2.7, 3.2, 4.0];
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert!(svg.contains(">5<"), "Y축 headroom: max 4.0 → 축 0~5");
    assert!(svg.contains(">4<"), "step 1 라벨 유지");
}

#[test]
fn test_axis_no_headroom_when_max_off_boundary() {
    // 한컴 실측 앵커: scatter X max 2.6(경계 아님) → 축 0~3, step 0.5 유지
    // (무조건 step 재계산 시 1.0으로 승격되는 회귀 방지).
    let svg = render_chart_svg(&scatter_chart(ScatterStyle::Marker), 0.0, 0.0, 400.0, 300.0);
    for want in [">0.5<", ">2.5<", ">3<"] {
        assert!(svg.contains(want), "X축 {want} 있어야 (0~3, step 0.5)");
    }
}

// --- C2b (#2278) Stage 1: 3D 막대 압출 ---

fn bars3d_chart(chart_type: OoxmlChartType, grouping: BarGrouping) -> OoxmlChart {
    let mut chart = bars_chart(grouping);
    chart.chart_type = chart_type;
    chart.is_3d = true;
    chart
}

#[test]
fn test_shade_lighten_darken() {
    // 채널별 선형 보간: +0.25 = 흰색 방향 25%, -0.25 = 검정 방향 25%
    assert_eq!(shade(0x006183D7, 0.25), 0x0089A2E1);
    assert_eq!(shade(0x006183D7, -0.25), 0x004962A1);
    // 극단값 클램프
    assert_eq!(shade(0x00123456, 1.0), 0x00FFFFFF);
    assert_eq!(shade(0x00123456, -1.0), 0x00000000);
    // factor 0 항등 + 상위(알파) 바이트 보존
    assert_eq!(shade(0xFF6183D7, 0.0), 0xFF6183D7);
    assert_eq!(shade(0xFF6183D7, 0.25) >> 24, 0xFF);
}

#[test]
fn test_bar3d_clustered_faces_both_orientations() {
    // 3D 묶은: 막대(2cat×3ser=6)마다 top/side 면 1쌍 (정답지: 윗면 밝게 +
    // 우측면 어둡게 사선 압출). 2D는 면 없음.
    // [지원 범위 — PR #2500 리뷰 P2] 이 시어 투영은 rAngAx=1(직각 축)
    // 코퍼스 한정 근사다. rAngAx=0(회전 투영)·rotX/rotY 임의 조합은
    // 동일 시어로 폴백하며 정답지 검증이 없다 — 별도 후속 트랙.
    for chart_type in [OoxmlChartType::Column, OoxmlChartType::Bar] {
        let chart = bars3d_chart(chart_type, BarGrouping::Clustered);
        let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
        assert_eq!(
            svg.matches("hwp-bar3d-top").count(),
            6,
            "{chart_type:?}: top 면 6개"
        );
        assert_eq!(
            svg.matches("hwp-bar3d-side").count(),
            6,
            "{chart_type:?}: side 면 6개"
        );
    }
    let svg2d = render_chart_svg(&bars_chart(BarGrouping::Clustered), 0.0, 0.0, 400.0, 300.0);
    assert!(!svg2d.contains("hwp-bar3d-"), "2D 묶은막대에 3D 면 없어야");
}

#[test]
fn test_bar3d_stacked_all_segments_extrude() {
    // 3D 누적: 모든 세그먼트가 자기 색 top/side를 그림 (2cat×3ser=6쌍).
    // 은면 제거는 페인트 순서(세로: 아래→위 = 계열1 먼저)가 담당 — 순서 핀.
    let chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Stacked);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(svg.matches("hwp-bar3d-top").count(), 6, "top 면 6개");
    assert_eq!(svg.matches("hwp-bar3d-side").count(), 6, "side 면 6개");
    let s1 = color_hex(shade(palette(0), BAR3D_SIDE_SHADE));
    let s3 = color_hex(shade(palette(2), BAR3D_SIDE_SHADE));
    assert!(
        svg.find(&s1).expect("계열1 side 색") < svg.find(&s3).expect("계열3 side 색"),
        "누적 페인트 순서: 계열1(아래) 먼저 → 계열3(위) 나중"
    );
}

#[test]
fn test_bar3d_zero_segment_skipped() {
    // 0값 세그먼트는 면 무방출 — 이웃 세그먼트의 캡(top) 재도색 방지.
    let mut chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Stacked);
    chart.series[1].values = vec![0.0, 1.0]; // 카테고리 a의 계열2 = 0
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert_eq!(
        svg.matches("hwp-bar3d-top").count(),
        5,
        "0값 세그먼트는 스킵 (6-1)"
    );
}

// --- C2b (#2278) v2: 투영 기하 헬퍼 ---

/// room 그룹 조각 (첫 </g>까지)
fn room_slice(svg: &str) -> &str {
    let s = svg.find("hwp-bar3d-room").expect("room");
    let room = &svg[s..];
    &room[..room.find("</g>").expect("room 닫힘")]
}

/// polygon points="..." → (x,y) 목록
fn poly_points(chunk: &str) -> Vec<(f64, f64)> {
    let pts = &chunk[chunk.find("points=\"").expect("points") + 8..];
    let pts = &pts[..pts.find('"').expect("닫는 따옴표")];
    pts.split_whitespace()
        .map(|p| {
            let mut it = p.split(',');
            (
                it.next().unwrap().parse().unwrap(),
                it.next().unwrap().parse().unwrap(),
            )
        })
        .collect()
}

/// 방 바닥 폴리곤(room 첫 polygon) 4점: p1=(fx,fyb) p2=(fx+dxf,fyb−dyf)
/// p3=(fx+fw+dxf,·) p4=(fx+fw,fyb) — 씬 파라미터를 SVG에서 역산하는 기준
fn floor_points(svg: &str) -> Vec<(f64, f64)> {
    let room = room_slice(svg);
    let start = room.find("<polygon").expect("바닥 폴리곤");
    poly_points(&room[start..])
}

/// 첫 top 면 폴리곤 → 막대 압출 벡터 (bdx, bdy)
fn bar_extrusion(svg: &str) -> (f64, f64) {
    let chunk = svg.split("hwp-bar3d-top").nth(1).expect("top 폴리곤");
    let pts = poly_points(chunk);
    (pts[1].0 - pts[0].0, pts[0].1 - pts[1].1)
}

/// 파랑(계열1) front rect (x, y, w, h) 목록 — 범례 swatch(10×10) 제외
fn blue_fronts(svg: &str) -> Vec<(f64, f64, f64, f64)> {
    let parts: Vec<&str> = svg.split("fill=\"#6183d7\"").collect();
    parts[..parts.len() - 1]
        .iter()
        .filter_map(|chunk| {
            let tag = &chunk[chunk.rfind("<rect ")?..];
            if tag.contains("width=\"10\" height=\"10\"") {
                return None;
            }
            Some((
                attr_f64_of(tag, "x=\"")?,
                attr_f64_of(tag, "y=\"")?,
                attr_f64_of(tag, "width=\"")?,
                attr_f64_of(tag, "height=\"")?,
            ))
        })
        .collect()
}

#[test]
fn test_bar3d_shear_direction() {
    // 시어 방향: fit 역산으로 pre-fit 성분비 oy/ox == sin(rotX)/sin(rotY)
    // (기본 카메라 15/20). 비등방 fit(sx≠sy) 때문에 화면 비율은 순수 sin비가
    // 아님 — pw=fw+dxf, ph=fh+dyf 복원으로 역산 (v2 설계 리뷰 반영).
    let chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Clustered);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let fl = floor_points(&svg);
    let dxf = fl[1].0 - fl[0].0;
    let dyf = fl[0].1 - fl[1].1;
    let fw = fl[3].0 - fl[0].0;
    // 뒷벽 rect height = fh
    let room = room_slice(&svg);
    let wall = &room[room.find("<rect").expect("뒷벽")..];
    let fh = attr_f64_of(wall, "height=\"").expect("fh");
    let ox = dxf * (fw + dxf) / fw;
    let oy = dyf * (fh + dyf) / fh;
    let expected = 15.0_f64.to_radians().sin() / 20.0_f64.to_radians().sin();
    assert!(
        (oy / ox - expected).abs() < 2e-3,
        "pre-fit 시어 성분비 sin15/sin20={expected}, 실제 {}",
        oy / ox
    );
    // 막대 압출은 방 깊이 벡터와 평행
    let (bdx, bdy) = bar_extrusion(&svg);
    assert!(
        (bdy / bdx - dyf / dxf).abs() < 2e-3,
        "막대 압출({},{})이 방 깊이({dxf},{dyf})와 평행해야",
        bdx,
        bdy
    );
}

#[test]
fn test_bar3d_room_depth_ratio() {
    // 방 깊이 / 막대 깊이 = 1 + gapDepth/100 (기본 150 → 2.5) — 센터링과
    // 무관하게 성립(dxf/bdx = D/b).
    let chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Clustered);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let fl = floor_points(&svg);
    let dxf = fl[1].0 - fl[0].0;
    let (bdx, _) = bar_extrusion(&svg);
    assert!(
        (dxf / bdx - 2.5).abs() < 1e-2,
        "기본 gapDepth 150 → D/b = 2.5, 실제 {}",
        dxf / bdx
    );
    // gapDepth=300 → 4.0
    let mut chart2 = bars3d_chart(OoxmlChartType::Column, BarGrouping::Clustered);
    chart2.gap_depth = Some(300.0);
    let svg2 = render_chart_svg(&chart2, 0.0, 0.0, 400.0, 300.0);
    let fl2 = floor_points(&svg2);
    let dxf2 = fl2[1].0 - fl2[0].0;
    let (bdx2, _) = bar_extrusion(&svg2);
    assert!(
        (dxf2 / bdx2 - 4.0).abs() < 1e-2,
        "gapDepth 300 → 4.0, 실제 {}",
        dxf2 / bdx2
    );
}

#[test]
fn test_bar3d_thickness_from_gap_width() {
    // 두께 규칙 slot/(n_eff+gapWidth/100) — v3 눈대중 상수(누적 0.4)의 유도
    // 원형. 기본 150: 누적 1/2.5=0.4, 묶은 3계열 bar_w = slot/4.5.
    let cat_span_of = |fronts: &[(f64, f64, f64, f64)]| fronts[1].0 - fronts[0].0;

    let stacked3d = bars3d_chart(OoxmlChartType::Column, BarGrouping::Stacked);
    let svg = render_chart_svg(&stacked3d, 0.0, 0.0, 400.0, 300.0);
    let f = blue_fronts(&svg);
    assert_eq!(f.len(), 2, "2 카테고리 파랑 front");
    let ratio = f[0].2 / cat_span_of(&f);
    assert!(
        (ratio - 0.4).abs() < 1e-3,
        "누적 두께/슬롯 0.4, 실제 {ratio}"
    );

    let clustered3d = bars3d_chart(OoxmlChartType::Column, BarGrouping::Clustered);
    let svg_c = render_chart_svg(&clustered3d, 0.0, 0.0, 400.0, 300.0);
    let fc = blue_fronts(&svg_c);
    let ratio_c = fc[0].2 / cat_span_of(&fc);
    assert!(
        (ratio_c - 1.0 / 4.5).abs() < 1e-3,
        "묶은 3계열 bar_w/슬롯 = 1/4.5, 실제 {ratio_c}"
    );

    // gapWidth=300 누적 → 1/4 = 0.25
    let mut wide_gap = bars3d_chart(OoxmlChartType::Column, BarGrouping::Stacked);
    wide_gap.bar_gap_width = Some(300.0);
    let svg_g = render_chart_svg(&wide_gap, 0.0, 0.0, 400.0, 300.0);
    let fg = blue_fronts(&svg_g);
    let ratio_g = fg[0].2 / cat_span_of(&fg);
    assert!(
        (ratio_g - 0.25).abs() < 1e-3,
        "gapWidth 300 누적 → 0.25, 실제 {ratio_g}"
    );

    // 2D 대조군: 0.7 유지 (바이트 불변 가드)
    let svg2d = render_chart_svg(&bars_chart(BarGrouping::Stacked), 0.0, 0.0, 400.0, 300.0);
    let f2 = blue_fronts(&svg2d);
    let ratio2 = f2[0].2 / cat_span_of(&f2);
    assert!(
        (ratio2 - 0.7).abs() < 1e-3,
        "2D 누적 0.7 유지, 실제 {ratio2}"
    );
}

#[test]
fn test_bar3d_bars_depth_centered() {
    // 막대 깊이 센터링: 세로 막대 하단 y = 앞면 하단(fyb) − bdy0,
    // bdy0 = (dyf − bdy)/2 (z0 = (D−b)/2).
    let chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Clustered);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let fl = floor_points(&svg);
    let fyb = fl[0].1;
    let dyf = fl[0].1 - fl[1].1;
    let (_, bdy) = bar_extrusion(&svg);
    let f = blue_fronts(&svg);
    let bottom = f[0].1 + f[0].3;
    let expected_off = (dyf - bdy) / 2.0;
    assert!(
        ((fyb - bottom) - expected_off).abs() < 2e-2,
        "센터링 오프셋 (dyf−bdy)/2 = {expected_off}, 실제 {}",
        fyb - bottom
    );
}

#[test]
fn test_bar3d_faces_within_plot() {
    // fit 스모크: 모든 3D 면 좌표가 차트 bbox(0..400, 0..300) 안.
    for grouping in [BarGrouping::Clustered, BarGrouping::Stacked] {
        for chart_type in [OoxmlChartType::Column, OoxmlChartType::Bar] {
            let chart = bars3d_chart(chart_type, grouping);
            let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
            for class in ["hwp-bar3d-top", "hwp-bar3d-side"] {
                for chunk in svg.split(class).skip(1) {
                    for (x, y) in poly_points(chunk) {
                        assert!(
                            (-0.5..=400.5).contains(&x) && (-0.5..=300.5).contains(&y),
                            "{chart_type:?}/{grouping:?}: 면 좌표({x},{y}) 이탈"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_bar3d_degenerate_cameras() {
    // 퇴화·경계 카메라 무패닉 + NaN 무방출 + front 존재.
    // 음수/역방향 성분은 shear_proj 클램프(정의역 방어)로 0 처리.
    let cases: &[(f64, f64, f64)] = &[
        (0.0, 0.0, 100.0),    // 시어 없음 → front만
        (90.0, 20.0, 100.0),  // rotX 최대
        (-15.0, 20.0, 100.0), // rotX<0 → 수직 성분 클램프
        (15.0, 200.0, 100.0), // sin(rotY)<0 → 수평 성분 클램프
        (15.0, 20.0, 0.0),    // depthPercent=0 → d_scene=0 NaN 가드
    ];
    for &(rx, ry, dp) in cases {
        let mut chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Clustered);
        chart.view3d = Some(View3D {
            rot_x: rx,
            rot_y: ry,
            depth_percent: dp,
            ..Default::default()
        });
        let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
        assert!(!svg.contains("NaN"), "({rx},{ry},{dp}): NaN 방출");
        assert!(
            svg.contains("fill=\"#6183d7\""),
            "({rx},{ry},{dp}): front 미방출"
        );
    }
}

#[test]
fn test_bar3d_room_only_when_3d() {
    // 3D 방(뒷벽+바닥+커넥터)은 is_3d 막대에만 1회 — 2D는 부재 (시각판정 보정
    // 2026-07-16: 방 표현 추가, 정답지 4종 공통).
    for chart_type in [OoxmlChartType::Column, OoxmlChartType::Bar] {
        for grouping in [BarGrouping::Clustered, BarGrouping::Stacked] {
            let chart = bars3d_chart(chart_type, grouping);
            let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
            assert_eq!(
                svg.matches("hwp-bar3d-room").count(),
                1,
                "{chart_type:?}/{grouping:?}: 방 1회"
            );
            // 방 그룹 안: 바닥 평행사변형(첫 polygon) + 커넥터/뒷벽 격자 라인
            let room = &svg[svg.find("hwp-bar3d-room").unwrap()..];
            let room = &room[..room.find("</g>").expect("방 그룹 닫힘")];
            assert!(room.contains("<polygon"), "바닥 평행사변형");
            assert!(room.matches("<line").count() >= 5, "커넥터+뒷벽 격자");
        }
    }
    let svg2d = render_chart_svg(&bars_chart(BarGrouping::Clustered), 0.0, 0.0, 400.0, 300.0);
    assert!(!svg2d.contains("hwp-bar3d-room"), "2D에 방 없음");
}

#[test]
fn test_bar3d_room_grid_on_back_wall() {
    // 뒷벽 격자선은 (+d,-d) 오프셋 — 세로 차트의 격자 y가 앞면 눈금보다 d만큼
    // 위(작음). 라벨 텍스트/위치는 2D와 동일(#1882 — test_axis_3d_*가 문자열 핀).
    let chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Clustered);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let room = &svg[svg.find("hwp-bar3d-room").unwrap()..];
    let room = &room[..room.find("</g>").unwrap()];
    // 뒷벽 수평 격자(x1≠x2, y1==y2)의 x1 = px+d — 앞면(px)보다 오른쪽
    let grid_line = room
        .split("<line ")
        .skip(1)
        .find(|l| {
            let y1 = attr_f64_of(l, "y1=\"");
            let y2 = attr_f64_of(l, "y2=\"");
            let x1 = attr_f64_of(l, "x1=\"");
            let x2 = attr_f64_of(l, "x2=\"");
            y1.is_some() && y1 == y2 && x1 != x2
        })
        .expect("뒷벽 수평 격자선");
    let gx1 = attr_f64_of(grid_line, "x1=\"").unwrap();
    // 방 뒷벽 rect의 x와 격자 x1 일치 (= px+d)
    let wall_x = attr_f64_of(&room[room.find("<rect").expect("뒷벽")..], "x=\"").unwrap();
    assert!(
        (gx1 - wall_x).abs() < 1e-6,
        "뒷벽 격자 x1({gx1}) == 뒷벽 x({wall_x})"
    );
}

// --- Stage 1R v2: 방 선 처리 한컴 정합 (시각판정 피드백 2026-07-19) ---

/// room 내 `<line>`들의 (x1,y1,x2,y2) 목록
fn room_lines(room: &str) -> Vec<(f64, f64, f64, f64)> {
    room.split("<line ")
        .skip(1)
        .map(|l| {
            (
                attr_f64_of(l, "x1=\"").unwrap(),
                attr_f64_of(l, "y1=\"").unwrap(),
                attr_f64_of(l, "x2=\"").unwrap(),
                attr_f64_of(l, "y2=\"").unwrap(),
            )
        })
        .collect()
}

#[test]
fn test_bar3d_room_hancom_line_style() {
    // 정답지 임베드(2702px) 픽셀 실측: 축선·조그·격자·틱 전부 #808080 균일
    // (실측 gray 126~148)·0.72pt≈0.75 — 2D의 축/격자 명암 구분과 다름.
    // 뒷벽 테두리·바닥 채움 없음(흰 면 + #808080 외곽선만).
    for chart_type in [OoxmlChartType::Column, OoxmlChartType::Bar] {
        let chart = bars3d_chart(chart_type, BarGrouping::Stacked);
        let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
        let room = room_slice(&svg);
        for stale in ["#cccccc", "#e8e8e8", "#f2f2f2"] {
            assert!(
                !room.contains(stale),
                "{chart_type:?}: 연회색 어휘 잔존 {stale}"
            );
        }
        for l in room.split("<line ").skip(1) {
            let l = &l[..l.find("/>").expect("line 닫힘")];
            assert!(
                l.contains("stroke=\"#808080\"") && l.contains("stroke-width=\"0.75\""),
                "{chart_type:?}: 균일 선 스타일 아님: {l}"
            );
        }
        let wall = &room[room.find("<rect").expect("뒷벽")..];
        let wall = &wall[..wall.find("/>").unwrap()];
        assert!(!wall.contains("stroke"), "{chart_type:?}: 뒷벽 무테두리");
        let floor = &room[room.find("<polygon").expect("바닥")..];
        let floor = &floor[..floor.find("/>").unwrap()];
        assert!(
            floor.contains("fill=\"#ffffff\"") && floor.contains("stroke=\"#808080\""),
            "{chart_type:?}: 바닥 흰 면 + #808080 외곽선"
        );
    }
}

#[test]
fn test_bar3d_axis_ticks_vertical() {
    // 세로형: 값 눈금마다 좌측 틱(fx−5→fx, 길이 실측 44px≈5.3pt) + 카테고리
    // 경계 하단 틱(fyb→fyb+4, 실측 31px≈3.7pt) — 경계+양끝 = cat_count+1개.
    let chart = bars3d_chart(OoxmlChartType::Column, BarGrouping::Stacked);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let room = room_slice(&svg);
    let fl = floor_points(&svg);
    let (fx, fyb) = fl[0];
    let lines = room_lines(room);
    let left_ticks = lines
        .iter()
        .filter(|(x1, y1, x2, y2)| {
            (y1 - y2).abs() < 1e-6 && (x2 - fx).abs() < 1e-6 && (fx - x1 - 5.0).abs() < 1e-6
        })
        .count();
    let back_grids = lines
        .iter()
        .filter(|(x1, y1, x2, y2)| (y1 - y2).abs() < 1e-6 && (x2 - x1) > 10.0)
        .count();
    assert!(back_grids >= 2, "뒷벽 수평 격자 존재");
    assert_eq!(left_ticks, back_grids, "값 눈금마다 좌측 틱");
    let down_ticks = lines
        .iter()
        .filter(|(x1, y1, x2, y2)| {
            (x1 - x2).abs() < 1e-6 && (y1 - fyb).abs() < 1e-6 && (y2 - fyb - 4.0).abs() < 1e-6
        })
        .count();
    assert_eq!(down_ticks, 3, "카테고리 경계 하단 틱 = cat_count(2)+1");
}

#[test]
fn test_bar3d_axis_ticks_horizontal() {
    // 가로형: 값 눈금마다 하단 틱 + 카테고리 경계 좌측 틱(cat_count+1) —
    // 한컴 실측(누적가로: 하단 값틱 8개 등간격 225px, 좌측 경계틱 5개).
    let chart = bars3d_chart(OoxmlChartType::Bar, BarGrouping::Stacked);
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    let room = room_slice(&svg);
    let fl = floor_points(&svg);
    let (fx, fyb) = fl[0];
    let lines = room_lines(room);
    let down_ticks = lines
        .iter()
        .filter(|(x1, y1, x2, y2)| {
            (x1 - x2).abs() < 1e-6 && (y1 - fyb).abs() < 1e-6 && (y2 - fyb - 4.0).abs() < 1e-6
        })
        .count();
    // 뒷벽 세로 격자(x1==x2, 앞면 축선(x==fx)보다 오른쪽, 길이 > 틱)
    let back_grids = lines
        .iter()
        .filter(|(x1, y1, x2, y2)| {
            (x1 - x2).abs() < 1e-6 && *x1 > fx + 1e-6 && (y1 - y2).abs() > 10.0
        })
        .count();
    assert!(back_grids >= 2, "뒷벽 세로 격자 존재");
    assert_eq!(down_ticks, back_grids, "값 눈금마다 하단 틱");
    let left_ticks = lines
        .iter()
        .filter(|(x1, y1, x2, y2)| {
            (y1 - y2).abs() < 1e-6 && (x2 - fx).abs() < 1e-6 && (fx - x1 - 5.0).abs() < 1e-6
        })
        .count();
    assert_eq!(left_ticks, 3, "카테고리 경계 좌측 틱 = cat_count(2)+1");
}

// --- C2b (#2278) Stage 2: 3D 원형 (rAngAx=0 회전+원근) ---

fn pie3d_chart(values: Vec<f64>, rot_x: f64, perspective: f64) -> OoxmlChart {
    OoxmlChart {
        chart_type: OoxmlChartType::Pie,
        is_3d: true,
        view3d: Some(View3D {
            rot_x,
            rot_y: 0.0,
            perspective,
            r_ang_ax: false,
            ..View3D::default()
        }),
        series: vec![OoxmlSeries {
            values,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// 첫 top 타원호의 (cx, cy, rx, ry) — "M{cx},{cy} … A{rx},{ry}" 파싱
fn pie3d_top_geom(svg: &str) -> (f64, f64, f64, f64) {
    let chunk = svg.split("hwp-pie3d-top").nth(1).expect("top 경로");
    let d = &chunk[chunk.find("d=\"M").expect("M") + 4..];
    let (cx, rest) = d.split_once(',').unwrap();
    let (cy, _) = rest.split_once(' ').unwrap();
    let a = &chunk[chunk.find(" A").expect("타원호") + 2..];
    let (rx, rest) = a.split_once(',').unwrap();
    let (ry, _) = rest.split_once(' ').unwrap();
    (
        cx.parse().unwrap(),
        cy.parse().unwrap(),
        rx.parse().unwrap(),
        ry.parse().unwrap(),
    )
}

/// 벽 경로의 좌표쌍 목록 — 순서: M점, A반지름, 호끝, L점, A반지름, 호끝
/// (인덱스 0=시작점, 2=1차 호 끝, 3=벽 하단, 5=복귀 호 끝)
fn wall_pairs(chunk: &str) -> Vec<(f64, f64)> {
    let d = &chunk[chunk.find("d=\"").unwrap() + 3..];
    let d = &d[..d.find('"').unwrap()];
    d.split_whitespace()
        .filter_map(|t| {
            let t = t.trim_start_matches(['M', 'A', 'L', 'Z']);
            let (x, y) = t.split_once(',')?;
            Some((x.parse().ok()?, y.parse().ok()?))
        })
        .collect()
}

#[test]
fn test_pie3d_ellipse_ratio_follows_rotx() {
    // 타원비 = sin(rotX)·cos(perspective/2°) — 정답지(rotX=30/persp=30) 실측
    // ry/rx=0.480, 유도 0.483 (0.5% 이내). 앞뒤 반타원 대칭(원근 비대칭 부재).
    let svg = render_chart_svg(
        &pie3d_chart(vec![25.0, 25.0, 50.0], 30.0, 30.0),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    let (_, _, rx, ry) = pie3d_top_geom(&svg);
    let expected = 30f64.to_radians().sin() * 15f64.to_radians().cos();
    assert!(
        (ry / rx - expected).abs() < 2e-3,
        "rotX=30/persp=30 → ry/rx≈{expected:.4}, 실제 {}",
        ry / rx
    );
    let svg = render_chart_svg(
        &pie3d_chart(vec![25.0, 25.0, 50.0], 60.0, 30.0),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    let (_, _, rx, ry) = pie3d_top_geom(&svg);
    let expected = 60f64.to_radians().sin() * 15f64.to_radians().cos();
    assert!(
        (ry / rx - expected).abs() < 2e-3,
        "rotX=60 → ry/rx≈{expected:.4}, 실제 {}",
        ry / rx
    );
}

#[test]
fn test_pie3d_wall_height_measured() {
    // 측벽 높이 = rx × 0.207 × hPercent/100 — 정답지 실측 175px/846.5px
    let svg = render_chart_svg(
        &pie3d_chart(vec![25.0, 25.0, 50.0], 30.0, 30.0),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    let (_, _, rx, _) = pie3d_top_geom(&svg);
    let wall = svg.split("hwp-pie3d-wall").nth(1).expect("벽");
    let pts = wall_pairs(wall);
    // 복귀 호 끝(xa, ya+wall) − 시작점(xa, ya)
    let wall_h = pts[5].1 - pts[0].1;
    assert!(
        (wall_h / rx - 0.207).abs() < 5e-3,
        "벽 높이/rx ≈ 0.207, 실제 {}",
        wall_h / rx
    );
}

#[test]
fn test_pie3d_wall_lower_half_only() {
    // 하반부(θ∈(0,π))만 벽: [25,25,50] → 슬라이스1(우상) 벽 없음, 2·3만 —
    // 벽 색 = shade(팔레트, SIDE) (윗면은 원색)
    let svg = render_chart_svg(
        &pie3d_chart(vec![25.0, 25.0, 50.0], 30.0, 30.0),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert_eq!(svg.matches("hwp-pie3d-wall").count(), 2, "벽 2개");
    assert_eq!(svg.matches("hwp-pie3d-top").count(), 3, "top 3개");
    let w1 = svg.split("hwp-pie3d-wall").nth(1).unwrap();
    let w2 = svg.split("hwp-pie3d-wall").nth(2).unwrap();
    assert!(
        w1.contains(&color_hex(shade(palette(1), BAR3D_SIDE_SHADE))),
        "벽1 = 팔레트1 음영"
    );
    assert!(
        w2.contains(&color_hex(shade(palette(2), BAR3D_SIDE_SHADE))),
        "벽2 = 팔레트2 음영"
    );
}

#[test]
fn test_pie3d_wall_clipped_at_boundaries() {
    // 첫 벽 시작 = θ=0 클립(cx+rx, cy), 마지막 벽 호 끝 = θ=π 클립(cx−rx, cy)
    let svg = render_chart_svg(
        &pie3d_chart(vec![25.0, 25.0, 50.0], 30.0, 30.0),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    let (cx, cy, rx, _) = pie3d_top_geom(&svg);
    let w1 = wall_pairs(svg.split("hwp-pie3d-wall").nth(1).unwrap());
    assert!(
        (w1[0].0 - (cx + rx)).abs() < 0.05 && (w1[0].1 - cy).abs() < 0.05,
        "첫 벽 시작 (cx+rx, cy), 실제 {:?}",
        w1[0]
    );
    let w2 = wall_pairs(svg.split("hwp-pie3d-wall").nth(2).unwrap());
    assert!(
        (w2[2].0 - (cx - rx)).abs() < 0.05 && (w2[2].1 - cy).abs() < 0.05,
        "마지막 벽 호 끝 (cx−rx, cy), 실제 {:?}",
        w2[2]
    );
}

#[test]
fn test_pie3d_walls_before_tops() {
    // 페인트 순서: 벽 전체 → top 전체 (은면 제거)
    let svg = render_chart_svg(
        &pie3d_chart(vec![25.0, 25.0, 50.0], 30.0, 30.0),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert!(
        svg.rfind("hwp-pie3d-wall").unwrap() < svg.find("hwp-pie3d-top").unwrap(),
        "벽이 top보다 선행"
    );
}

#[test]
fn test_pie_2d_no_pie3d_vocab() {
    // 2D 원형 가드: is_3d=false → 3D 어휘 부재 (2D 바이트 불변의 방증)
    let mut chart = pie3d_chart(vec![25.0, 25.0, 50.0], 30.0, 30.0);
    chart.is_3d = false;
    chart.view3d = None;
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert!(!svg.contains("hwp-pie3d"), "2D에 3D 어휘 없음");
}

// --- C2b (#2278) Stage 3: ofPie 보조플롯 + 팔레트 #5 ---

fn ofpie_chart(of: OfPieInfo) -> OoxmlChart {
    OoxmlChart {
        chart_type: OoxmlChartType::Pie,
        of_pie: Some(of),
        series: vec![OoxmlSeries {
            values: vec![10.0, 3.5, 1.5, 1.2],
            ..Default::default()
        }],
        categories: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        ..Default::default()
    }
}

#[test]
fn test_palette_index4_measured() {
    // [4] = ofPie 결합 슬라이스 실측 초록계 #27A172 (원형대원형·원형대가로막대형
    // 정답지 임베드 픽셀 히스토그램 최빈값 — 두 파일 교차 일치)
    assert_eq!(palette(4), 0xFF27A172, "팔레트 [4] 실측 고정");
}

#[test]
fn test_ofpie_pie_secondary_and_serlines() {
    // 주 원 3(= n−k+1 = 4−2+1) + 보조 원 2(= k) + serLines 2
    let svg = render_chart_svg(
        &ofpie_chart(OfPieInfo {
            has_ser_lines: true,
            ..Default::default()
        }),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert_eq!(svg.matches("hwp-ofpie-main").count(), 3, "주 원 슬라이스 3");
    assert_eq!(
        svg.matches("hwp-ofpie-second").count(),
        2,
        "보조 원 슬라이스 2"
    );
    assert_eq!(svg.matches("hwp-ofpie-serline").count(), 2, "serLines 2");
    // has_ser_lines=false → serline 0
    let svg2 = render_chart_svg(&ofpie_chart(OfPieInfo::default()), 0.0, 0.0, 400.0, 300.0);
    assert_eq!(
        svg2.matches("hwp-ofpie-serline").count(),
        0,
        "serLines 부재"
    );
}

#[test]
fn test_ofpie_combined_slice_uses_palette4() {
    // 결합 슬라이스 = palette(n) (n=4 → 실측 초록계) — hex 하드코딩 대신 참조
    let svg = render_chart_svg(&ofpie_chart(OfPieInfo::default()), 0.0, 0.0, 400.0, 300.0);
    let main = svg.split("hwp-ofpie-main").nth(3).expect("결합 슬라이스");
    let main = &main[..main.find("/>").unwrap()];
    assert!(
        main.contains(&color_hex(palette(4))),
        "결합 슬라이스 fill = palette(4)"
    );
}

#[test]
fn test_ofpie_bar_secondary_first_split_cat_on_top() {
    // Bar형 보조: rect 2개, 첫 분할 카테고리(palette(2))가 맨 위
    let svg = render_chart_svg(
        &ofpie_chart(OfPieInfo {
            of_pie_type: OfPieType::Bar,
            ..Default::default()
        }),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    let rects: Vec<&str> = svg.split("hwp-ofpie-second").skip(1).collect();
    assert_eq!(rects.len(), 2, "보조 rect 2");
    let y_of = |chunk: &str| attr_f64_of(&chunk[..chunk.find("/>").unwrap()], "y=\"").unwrap();
    let c_of =
        |chunk: &str, rgb: u32| chunk[..chunk.find("/>").unwrap()].contains(&color_hex(rgb));
    assert!(
        c_of(rects[0], palette(2)) && c_of(rects[1], palette(3)),
        "보조 색 [2],[3]"
    );
    assert!(y_of(rects[0]) < y_of(rects[1]), "첫 분할 카테고리가 맨 위");
}

#[test]
fn test_ofpie_split_pos_respected() {
    // split_pos=3 → 주 원 2(= 4−3+1) + 보조 3
    let svg = render_chart_svg(
        &ofpie_chart(OfPieInfo {
            split_pos: Some(3.0),
            ..Default::default()
        }),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert_eq!(svg.matches("hwp-ofpie-main").count(), 2, "주 원 2");
    assert_eq!(svg.matches("hwp-ofpie-second").count(), 3, "보조 3");
}

#[test]
fn test_ofpie_non_pos_split_type_falls_back_to_default() {
    // PR #2500 후속: val/percent/cust 의 splitPos 는 count 가 아니므로
    // 무시하고 기본 k=2 로 폴백 — 주 원 3(= 4−2+1) + 보조 2.
    for ty in [
        super::super::OfPieSplitType::Val,
        super::super::OfPieSplitType::Percent,
        super::super::OfPieSplitType::Cust,
    ] {
        let svg = render_chart_svg(
            &ofpie_chart(OfPieInfo {
                split_type: ty,
                split_pos: Some(3.0),
                ..Default::default()
            }),
            0.0,
            0.0,
            400.0,
            300.0,
        );
        assert_eq!(svg.matches("hwp-ofpie-main").count(), 3, "{ty:?}: 주 원 3");
        assert_eq!(svg.matches("hwp-ofpie-second").count(), 2, "{ty:?}: 보조 2");
    }
    // splitType=pos 는 종전대로 count 적용
    let svg = render_chart_svg(
        &ofpie_chart(OfPieInfo {
            split_type: super::super::OfPieSplitType::Pos,
            split_pos: Some(3.0),
            ..Default::default()
        }),
        0.0,
        0.0,
        400.0,
        300.0,
    );
    assert_eq!(svg.matches("hwp-ofpie-main").count(), 2, "pos: 주 원 2");
    assert_eq!(svg.matches("hwp-ofpie-second").count(), 3, "pos: 보조 3");
}

#[test]
fn test_ofpie_legend_categories_in_order_no_combined() {
    // 범례: 카테고리 4개 정순(palette 0..3), 결합 슬라이스(palette 4) 부재
    let svg = render_chart_svg(&ofpie_chart(OfPieInfo::default()), 0.0, 0.0, 400.0, 300.0);
    let legend = &svg[svg.find("hwp-chart-legend").expect("범례")..];
    let mut last = 0usize;
    for i in 0..4 {
        let p = legend
            .find(&color_hex(palette(i)))
            .unwrap_or_else(|| panic!("범례 스와치 {i}"));
        assert!(p >= last, "범례 정순 위반 ({i})");
        last = p;
    }
    assert!(
        !legend.contains(&color_hex(palette(4))),
        "범례에 결합 슬라이스 없음"
    );
}

#[test]
fn test_ofpie_two_values_plain_pie_fallback() {
    // n=2 < 3 → 일반 원형 폴백 (ofpie 어휘·serline 부재)
    let mut chart = ofpie_chart(OfPieInfo {
        has_ser_lines: true,
        ..Default::default()
    });
    chart.series[0].values = vec![7.0, 3.0];
    chart.categories = vec!["a".into(), "b".into()];
    let svg = render_chart_svg(&chart, 0.0, 0.0, 400.0, 300.0);
    assert!(!svg.contains("hwp-ofpie"), "n<3은 일반 원형 폴백");
}

#[test]
fn test_pie_exploded_slices_offset() {
    // 쪼개진원형(계열 explosion 25): 각 슬라이스 꼭짓점이 중심에서 중심각
    // 방향으로 r×0.25 이동, 반지름은 1/(1+0.25)로 축소(벌어진 만큼 fit).
    // 정답지: 한컴 쪼개진원형-2022 — 전 슬라이스 균일 벌어짐.
    let mut plain = OoxmlChart {
        chart_type: OoxmlChartType::Pie,
        series: vec![OoxmlSeries {
            values: vec![4.0, 3.0, 2.0],
            ..Default::default()
        }],
        categories: vec!["a".into(), "b".into(), "c".into()],
        ..Default::default()
    };
    let svg_plain = render_chart_svg(&plain, 0.0, 0.0, 400.0, 300.0);
    // 2D 원형 슬라이스 path: "M{cx},{cy} L..." — 전 슬라이스 동일 꼭짓점 = 중심
    let apex = |chunk: &str| -> (f64, f64) {
        let d = &chunk[chunk.find("d=\"M").unwrap() + 4..];
        let (x, rest) = d.split_once(',').unwrap();
        let (y, _) = rest.split_once(' ').unwrap();
        (x.parse().unwrap(), y.parse().unwrap())
    };
    let arc_r = |chunk: &str| -> f64 {
        let a = &chunk[chunk.find(" A").unwrap() + 2..];
        a.split_once(',').unwrap().0.parse().unwrap()
    };
    // plain 중심/반지름
    let plain_slices: Vec<String> = svg_plain
        .split("<path ")
        .skip(1)
        .filter(|c| c.starts_with("d=\"M"))
        .map(|c| c[..c.find("/>").unwrap()].to_string())
        .collect();
    assert_eq!(plain_slices.len(), 3, "2D 원형 3슬라이스");
    let (cx, cy) = apex(&plain_slices[0]);
    let r_plain = arc_r(&plain_slices[0]);

    plain.series[0].explosion = Some(25.0);
    let svg_ex = render_chart_svg(&plain, 0.0, 0.0, 400.0, 300.0);
    let ex_slices: Vec<String> = svg_ex
        .split("<path ")
        .skip(1)
        .filter(|c| c.starts_with("d=\"M"))
        .map(|c| c[..c.find("/>").unwrap()].to_string())
        .collect();
    assert_eq!(ex_slices.len(), 3);
    let r_ex = arc_r(&ex_slices[0]);
    assert!(
        (r_ex - r_plain / 1.25).abs() < 0.05,
        "반지름 fit 축소: {r_ex} vs {}",
        r_plain / 1.25
    );
    let off = r_ex * 0.25;
    for (i, s) in ex_slices.iter().enumerate() {
        let (ax, ay) = apex(s);
        let d = ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt();
        assert!(
            (d - off).abs() < 0.05,
            "슬라이스 {i} 꼭짓점 오프셋 {d} ≠ {off}"
        );
    }
    // 서로 다른 방향으로 벌어짐 (꼭짓점 전부 상이)
    let a0 = apex(&ex_slices[0]);
    let a1 = apex(&ex_slices[1]);
    let a2 = apex(&ex_slices[2]);
    assert!(a0 != a1 && a1 != a2 && a0 != a2, "슬라이스별 방향 분리");
}

#[test]
fn test_pie_slices_butt_joined_no_white_border() {
    // 시각판정 확정(2026-07-19): 한컴 원형 계열은 슬라이스 밀착 — 2D/3D/ofPie
    // 정답지 원주 전수 스캔 흰 run 0건 → 흰 테두리 미방출 (마커/라인 할로 무관)
    let pie2d = OoxmlChart {
        chart_type: OoxmlChartType::Pie,
        series: vec![OoxmlSeries {
            values: vec![4.0, 3.0, 2.0],
            ..Default::default()
        }],
        categories: vec!["a".into(), "b".into(), "c".into()],
        ..Default::default()
    };
    let charts = [
        pie2d,
        pie3d_chart(vec![25.0, 25.0, 50.0], 30.0, 30.0),
        ofpie_chart(OfPieInfo {
            has_ser_lines: true,
            ..Default::default()
        }),
        ofpie_chart(OfPieInfo {
            of_pie_type: OfPieType::Bar,
            ..Default::default()
        }),
    ];
    for (i, chart) in charts.iter().enumerate() {
        let svg = render_chart_svg(chart, 0.0, 0.0, 400.0, 300.0);
        assert!(
            !svg.contains("stroke=\"#ffffff\""),
            "원형 계열 {i}: 슬라이스 흰 테두리 잔존"
        );
    }
}

#[test]
fn test_pie3d_degenerate_cameras() {
    // 정의역 방어: rotX=0(타원 퇴화)·90(정원)·perspective=240(cos 음수 위험)
    for (rx_deg, persp) in [(0.0, 30.0), (90.0, 30.0), (30.0, 240.0), (-15.0, 0.0)] {
        let svg = render_chart_svg(
            &pie3d_chart(vec![25.0, 25.0, 50.0], rx_deg, persp),
            0.0,
            0.0,
            400.0,
            300.0,
        );
        assert!(
            !svg.contains("NaN"),
            "rotX={rx_deg}/persp={persp}: NaN 없음"
        );
        let (_, _, rx, ry) = pie3d_top_geom(&svg);
        assert!(
            ry > 0.0 && ry <= rx + 1e-6,
            "타원비 (0,1] 유지: {}",
            ry / rx
        );
    }
}
