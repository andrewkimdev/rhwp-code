//! chart_types — table_layout.rs 에서 무변동 이동
use super::*;

/// 3D 막대 1개(또는 누적 세그먼트 1개) — 압출 벡터 (+dx, −dy) 3면:
/// top 평행사변형(밝게) + right 평행사변형(어둡게) + front rect(원색).
/// dx, dy ≥ 0 전제(shear_proj 클램프 — 음수 성분은 페인트 순서 은면 제거와
/// 상충하여 정의역 밖). 은면 제거는 호출측 페인트 순서(누적: 아래→위/왼→오른쪽)가
/// 담당하므로 루프 순서 변경 금지. w/h ≤ 0(0값 세그먼트)이면 무방출 — 누적에서
/// 이웃 세그먼트의 캡 재도색 방지. 압출 퇴화(dx,dy < 0.01)면 front만. (C2b #2278 v2)
pub(crate) fn push_bar_3d(svg: &mut String, x: f64, y: f64, w: f64, h: f64, dx: f64, dy: f64, color: u32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    debug_assert!(dx >= 0.0 && dy >= 0.0, "시어 성분은 클램프로 비음수");
    if dx > 0.01 || dy > 0.01 {
        svg.push_str(&format!(
            "<polygon class=\"hwp-bar3d-top\" points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\"/>\n",
            x, y, x + dx, y - dy, x + w + dx, y - dy, x + w, y,
            color_hex(shade(color, BAR3D_TOP_SHADE))
        ));
        svg.push_str(&format!(
            "<polygon class=\"hwp-bar3d-side\" points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\"/>\n",
            x + w, y, x + w + dx, y - dy, x + w + dx, y + h - dy, x + w, y + h,
            color_hex(shade(color, BAR3D_SIDE_SHADE))
        ));
    }
    svg.push_str(&format!(
        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
        x,
        y,
        w,
        h,
        color_hex(color)
    ));
}


pub(crate) fn render_bars(
    svg: &mut String,
    chart: &OoxmlChart,
    px: f64,
    py: f64,
    pw: f64,
    ph: f64,
    horizontal: bool,
) {
    let stacked = matches!(
        chart.grouping,
        BarGrouping::Stacked | BarGrouping::PercentStacked
    );
    let percent = chart.grouping == BarGrouping::PercentStacked;

    let cat_count = chart.categories.len().max(
        chart
            .series
            .iter()
            .map(|s| s.values.len())
            .max()
            .unwrap_or(0),
    );
    if cat_count == 0 {
        return;
    }
    let ser_count = chart.series.len().max(1);

    // 값축 범위: clustered=개별값, stacked=카테고리 합의 최대, percent=0~100%
    // (percent는 step 20 고정 = 종전 5등분 라벨 0/20/…/100%와 동일)
    // 눈금 밀도는 값축 방향 기준: 세로막대=세로 값축(3칸), 가로막대=가로 값축(5칸)
    let ticks = if horizontal {
        HORIZONTAL_AXIS_TICKS
    } else {
        VERTICAL_AXIS_TICKS
    };
    let (vmin, vmax, vstep) = if percent {
        (0.0, 100.0, 20.0)
    } else if stacked {
        let max_sum = (0..cat_count)
            .map(|ci| category_positive_sum(chart, ci))
            .fold(0.0_f64, f64::max);
        let (mn, mx, st) = nice_axis(0.0, max_sum.max(1.0), ticks);
        if chart.is_3d && !horizontal {
            // 한컴 3D 누적'세로' 실측: 2D(0~15) + 1 step = 0~20. 가로는 2D와 동일(0~14).
            (mn, mx + st, st)
        } else {
            (mn, mx, st)
        }
    } else if chart.is_3d {
        // 한컴 3D 묶은막대 실측: 세로·가로 모두 촘촘 눈금(5칸) + 경계 headroom 없음
        // (max 5.0 → 0~5 step 1; 2D의 0~6과 다름)
        let (mn, mx) = raw_value_bounds(chart.series.iter());
        nice_axis_no_headroom(mn, mx, HORIZONTAL_AXIS_TICKS)
    } else {
        let (mn, mx, st) = value_range(chart, ticks);
        // 특이케이스 실측(C1c v2 기록 → #2277 stage5): 가로막대 1카테고리 미니차트는
        // 축 범위 유지·step 절반 (4.3 → 0~5 step 0.5, 라벨 11개). 기존 가로축 앵커
        // (12.3→2 / 5.0→1 / 2.6→0.5)와 단일 규칙 불성립 — 단일 샘플 근거라
        // 가로·1카테고리(·이 분기 자체로 비누적·비3D)로 좁게 게이트. 세로 1카테고리는
        // 미실측이라 불변.
        if horizontal && cat_count == 1 {
            (mn, mx, st / 2.0)
        } else {
            (mn, mx, st)
        }
    };

    if chart.is_3d {
        // 3D는 시어 투영 기반 별도 경로 — 축 범위(vmin/vmax/vstep)는 위에서
        // 플롯 rect 기준으로 이미 확정(#1882 앵커 무접촉). 배치·방·압출만
        // 투영 좌표로 수행. (C2b #2278 v2)
        render_bars_3d(
            svg, chart, px, py, pw, ph, horizontal, stacked, percent, cat_count, ser_count, vmin,
            vmax, vstep,
        );
        return;
    }

    svg.push_str(&format!(
        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#ffffff\" stroke=\"#cccccc\" stroke-width=\"0.5\"/>\n",
        px, py, pw, ph
    ));

    render_value_grid(
        svg,
        px,
        py,
        pw,
        ph,
        vmin,
        vmax,
        vstep,
        chart.series.first().and_then(|s| s.format_code.as_deref()),
        horizontal,
        false,
        percent,
        false,
    );

    let (cat_span, bar_span_total) = if horizontal {
        let span = ph / cat_count as f64;
        (span, span * 0.7)
    } else {
        let span = pw / cat_count as f64;
        (span, span * 0.7)
    };

    // 가로 막대는 카테고리를 아래→위로 배치 (한컴 실측: 항목 1이 맨 아래).
    // 세로는 왼→오른쪽 그대로.
    let cat_slot = |ci: usize| -> f64 {
        let idx = if horizontal { cat_count - 1 - ci } else { ci };
        cat_span * idx as f64
    };

    if stacked {
        // 누적: 카테고리당 단일 막대, 시리즈를 아래/왼쪽부터 쌓음.
        // percent → 카테고리 합으로 정규화(전체 길이 = 100%), stacked → vmax로 정규화.
        for ci in 0..cat_count {
            let denom = if percent {
                let s = category_positive_sum(chart, ci);
                if s > 0.0 {
                    s
                } else {
                    1.0
                }
            } else {
                (vmax - vmin).max(1e-9)
            };
            let mut acc = 0.0_f64; // 지금까지 쌓인 픽셀 길이
            for (si, ser) in chart.series.iter().enumerate() {
                let v = ser.values.get(ci).copied().unwrap_or(0.0).max(0.0);
                let color = series_color(ser, si);
                let base = px;
                // 셀 시작: 가로=세로축(py) 기준, 세로=가로축(px) 기준
                let cell = if horizontal { py } else { px }
                    + cat_slot(ci)
                    + (cat_span - bar_span_total) / 2.0;
                if horizontal {
                    let seg = pw * (v / denom);
                    svg.push_str(&format!(
                        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
                        base + acc, cell, seg.max(0.0), bar_span_total, color
                    ));
                    acc += seg;
                } else {
                    let seg = ph * (v / denom);
                    let by = py + ph - acc - seg;
                    svg.push_str(&format!(
                        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
                        cell, by, bar_span_total, seg.max(0.0), color
                    ));
                    acc += seg;
                }
            }
        }
    } else {
        let bar_w = bar_span_total / ser_count as f64;
        for ci in 0..cat_count {
            for (si, ser) in chart.series.iter().enumerate() {
                let v = *ser.values.get(ci).unwrap_or(&0.0);
                let t = if vmax > vmin {
                    (v - vmin) / (vmax - vmin)
                } else {
                    0.0
                };
                let color = series_color(ser, si);
                if horizontal {
                    // 슬롯 내 세로 배치: 계열1이 맨 아래 (정답지 실측 — 위→아래 =
                    // 계열3→1, 범례 역순과 시각 일치. #2277 stage3)
                    let cy = py
                        + cat_slot(ci)
                        + (cat_span - bar_span_total) / 2.0
                        + bar_w * (ser_count - 1 - si) as f64;
                    let bw = pw * t;
                    svg.push_str(&format!(
                        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
                        px, cy, bw.max(0.0), bar_w * 0.95, color
                    ));
                } else {
                    let cx =
                        px + cat_slot(ci) + (cat_span - bar_span_total) / 2.0 + bar_w * si as f64;
                    let bh = ph * t;
                    let by = py + ph - bh;
                    svg.push_str(&format!(
                        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
                        cx, by, bar_w * 0.95, bh.max(0.0), color
                    ));
                }
            }
        }
    }

    render_category_labels(svg, chart, px, py, pw, ph, cat_count, horizontal);
}


/// 한 카테고리의 (양수) 시리즈 값 합. 누적 막대 축/정규화에 사용.
pub(crate) fn category_positive_sum(chart: &OoxmlChart, ci: usize) -> f64 {
    chart
        .series
        .iter()
        .map(|s| s.values.get(ci).copied().unwrap_or(0.0).max(0.0))
        .sum()
}

// ---------------- Line (단일 축) ----------------


pub(crate) fn render_line(svg: &mut String, chart: &OoxmlChart, px: f64, py: f64, pw: f64, ph: f64) {
    let stacked = matches!(
        chart.line_grouping,
        BarGrouping::Stacked | BarGrouping::PercentStacked
    );
    let percent = chart.line_grouping == BarGrouping::PercentStacked;

    let max_len = chart
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0);
    if max_len < 2 {
        return;
    }

    // 값축: 비누적=개별값, 누적=카테고리 합의 최대, 백프로=0~100% step 20
    // (render_bars 누적 정책 미러 — 정답지 실측 누적 0~15 step 5. C1d #2129)
    let (vmin, vmax, vstep) = if percent {
        (0.0, 100.0, 20.0)
    } else if stacked {
        let max_sum = (0..max_len)
            .map(|ci| category_positive_sum(chart, ci))
            .fold(0.0_f64, f64::max);
        nice_axis(0.0, max_sum.max(1.0), VERTICAL_AXIS_TICKS)
    } else {
        value_range(chart, VERTICAL_AXIS_TICKS)
    };

    svg.push_str(&format!(
        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#ffffff\" stroke=\"#cccccc\" stroke-width=\"0.5\"/>\n",
        px, py, pw, ph
    ));
    render_value_grid(
        svg,
        px,
        py,
        pw,
        ph,
        vmin,
        vmax,
        vstep,
        chart.series.first().and_then(|s| s.format_code.as_deref()),
        false,
        false,
        percent,
        false,
    );

    // x 배치: 카테고리 슬롯 중앙 (한컴 정합, XML crossBetween=between —
    // 첫/끝 점이 플롯 가장자리가 아닌 반 슬롯 안쪽. 카테고리 라벨과 동일 공식.
    // 작업지시자 시각판정 반영, C1d #2129)
    let cat_span = pw / max_len as f64;
    let mut cum = vec![0.0_f64; max_len]; // 카테고리별 누적값 (값공간)
    for (si, ser) in chart.series.iter().enumerate() {
        let color = series_color(ser, si);
        let mut points: Vec<(f64, f64)> = Vec::with_capacity(ser.values.len());
        for (i, &v) in ser.values.iter().enumerate() {
            let val = if stacked {
                cum[i] += v.max(0.0); // 음수 clamp — render_bars 누적과 동일 정책
                if percent {
                    let sum = category_positive_sum(chart, i);
                    if sum > 0.0 {
                        cum[i] / sum * 100.0
                    } else {
                        0.0 // 합 0 카테고리 → 0% (막대 denom=1.0 가드와 동등)
                    }
                } else {
                    cum[i]
                }
            } else {
                v
            };
            let t = if vmax > vmin {
                (val - vmin) / (vmax - vmin)
            } else {
                0.0
            };
            points.push((px + cat_span * (i as f64 + 0.5), py + ph - ph * t));
        }
        svg.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>\n",
            polyline_path(&points),
            color
        ));
        if chart.line_markers {
            for &(mx, my) in &points {
                push_line_marker(svg, si, mx, my, &color);
            }
        }
    }

    render_category_labels(svg, chart, px, py, pw, ph, max_len, false);
}

// ---------------- Stock (주식형, hiLowLines/upDownBars) ----------------


/// stock (주식형). 계열 역할 = XML 순서 규약: 3계열=고/저/종, 4계열=시/고/저/종
/// (코퍼스 실측 — 그 외 계열 수는 render_line 폴백으로 placeholder 재발 방지).
/// 정답지 정합: 검정 고저선 + (OHLC) 시가↔종가 캔들(하락=진회색 채움/상승=흰
/// 채움+검정 테두리) + 종가 마커(마커 사이클·팔레트 폴백이 ▲회색/×노랑을 자동
/// 결정 — 시/고/저는 `c:symbol val="none"`이라 무마커). (C2a #2277)
pub(crate) fn render_stock(svg: &mut String, chart: &OoxmlChart, px: f64, py: f64, pw: f64, ph: f64) {
    let (hi_i, lo_i, close_i, open_i) = match chart.series.len() {
        3 => (0usize, 1usize, 2usize, None),
        4 => (1, 2, 3, Some(0usize)),
        _ => return render_line(svg, chart, px, py, pw, ph),
    };
    let cat_count = chart.categories.len().max(
        chart
            .series
            .iter()
            .map(|s| s.values.len())
            .max()
            .unwrap_or(0),
    );
    if cat_count == 0 {
        return;
    }

    // 값축: stock 전용 무조건 +1 step 헤드룸 — 정답지 실측 max 59 → 0~80 step 20.
    // nice_axis의 경계 조건부 +1로는 0~60이라 부족. 3D 누적세로(+1 step)와 동형 패턴.
    let (raw_min, raw_max) = raw_value_bounds(chart.series.iter());
    let (vmin, mx, vstep) = nice_axis_no_headroom(raw_min, raw_max, VERTICAL_AXIS_TICKS);
    let vmax = mx + vstep;

    svg.push_str(&format!(
        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#ffffff\" stroke=\"#cccccc\" stroke-width=\"0.5\"/>\n",
        px, py, pw, ph
    ));
    render_value_grid(
        svg,
        px,
        py,
        pw,
        ph,
        vmin,
        vmax,
        vstep,
        chart.series.first().and_then(|s| s.format_code.as_deref()),
        false,
        false,
        false,
        false,
    );

    let y_of = |v: f64| -> f64 {
        let t = if vmax > vmin {
            (v - vmin) / (vmax - vmin)
        } else {
            0.0
        };
        py + ph - ph * t
    };
    let val = |si: usize, ci: usize| -> Option<f64> {
        chart.series.get(si).and_then(|s| s.values.get(ci)).copied()
    };
    let cat_span = pw / cat_count as f64;
    // 캔들 폭 = cat_span / (1 + gapWidth/100) — 정답지 gapWidth=150 → 슬롯의 40%
    let gap = chart.up_down_gap_width.unwrap_or(150.0).max(0.0);
    let candle_w = cat_span / (1.0 + gap / 100.0);

    for ci in 0..cat_count {
        let x = px + cat_span * (ci as f64 + 0.5);
        if chart.has_hi_low_lines {
            if let (Some(hi), Some(lo)) = (val(hi_i, ci), val(lo_i, ci)) {
                svg.push_str(&format!(
                    "<line class=\"hwp-stock-hilow\" x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#000000\" stroke-width=\"1\"/>\n",
                    x,
                    y_of(hi),
                    x,
                    y_of(lo)
                ));
            }
        }
        if chart.has_up_down_bars {
            if let (Some(open), Some(close)) = (open_i.and_then(|oi| val(oi, ci)), val(close_i, ci))
            {
                let top = y_of(open.max(close));
                let bot = y_of(open.min(close));
                // 하락(종<시)=진회색 채움(#404040 근사 — 시각판정에서 픽셀 실측 확정),
                // 상승·동률=흰 채움+검정 테두리 (동률은 미실측 — 상승 처리 고정).
                let (fill, stroke) = if close < open {
                    ("#404040", "none")
                } else {
                    ("#ffffff", "#000000")
                };
                svg.push_str(&format!(
                    "<rect class=\"hwp-stock-candle\" x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>\n",
                    x - candle_w / 2.0,
                    top,
                    candle_w,
                    (bot - top).max(0.5),
                    fill,
                    stroke
                ));
            }
        }
    }

    // 마커: Auto/Named 계열만 (코퍼스 = 종가만 Auto). 고저선/캔들 위에 그린다.
    for (si, ser) in chart.series.iter().enumerate() {
        if !matches!(
            ser.marker_symbol,
            SeriesMarker::Auto | SeriesMarker::Named(_)
        ) {
            continue;
        }
        let color = series_color(ser, si);
        for (ci, &v) in ser.values.iter().enumerate() {
            let x = px + cat_span * (ci as f64 + 0.5);
            push_marker(svg, "hwp-chart-marker", si, x, y_of(v), 3.5, &color);
        }
    }

    render_category_labels(svg, chart, px, py, pw, ph, cat_count, false);
}

// ---------------- Scatter (분산형, 2 수치축) ----------------


pub(crate) fn render_scatter(svg: &mut String, chart: &OoxmlChart, px: f64, py: f64, pw: f64, ph: f64) {
    // 전 시리즈가 (x,y) 쌍을 못 만들면 격자도 의미 없음 → 조기 종료.
    // (상위 <g class="hwp-ooxml-chart">는 이미 출력되어 placeholder는 안 뜸)
    if chart
        .series
        .iter()
        .all(|s| s.x_values.is_empty() || s.values.is_empty())
    {
        return;
    }

    let (xmin, xmax, xstep) =
        scatter_range(chart.series.iter().flat_map(|s| s.x_values.iter().copied()));
    let (ymin, ymax, ystep) =
        scatter_range(chart.series.iter().flat_map(|s| s.values.iter().copied()));
    let xspan = (xmax - xmin).max(1e-9);
    let yspan = (ymax - ymin).max(1e-9);

    // 플롯 배경
    svg.push_str(&format!(
        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#ffffff\" stroke=\"#cccccc\" stroke-width=\"0.5\"/>\n",
        px, py, pw, ph
    ));
    // X축(하단, 수직 격자선) + Y축(좌측, 수평 격자선) — 둘 다 수치축, 소수 라벨
    render_value_grid(
        svg, px, py, pw, ph, xmin, xmax, xstep, None, true, false, false, true,
    );
    render_value_grid(
        svg, px, py, pw, ph, ymin, ymax, ystep, None, false, false, false, true,
    );

    let (show_line, smooth, show_markers) = chart.scatter_style.flags();

    for (si, ser) in chart.series.iter().enumerate() {
        let color = series_color(ser, si);
        // (x,y) 픽셀 좌표. 데이터 순서 유지(x 정렬 안 함), 길이 불일치 시 짧은 쪽으로 절단.
        let points: Vec<(f64, f64)> = ser
            .x_values
            .iter()
            .zip(ser.values.iter())
            .map(|(&x, &y)| {
                (
                    px + pw * (x - xmin) / xspan,
                    py + ph - ph * (y - ymin) / yspan,
                )
            })
            .collect();
        if points.is_empty() {
            continue;
        }

        if show_line && points.len() >= 2 {
            let d = if smooth {
                smooth_path(&points)
            } else {
                polyline_path(&points)
            };
            svg.push_str(&format!(
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                d, color
            ));
        }
        if show_markers {
            // 계열 사이클 글리프 ◆■▲× — 정답지 실측(표식만있는분산형: 계열1 ◆/계열2 ■).
            // 반경 4.5 = 라인(3.5)보다 큰 실측 근사, 시각판정 조정 여지. (C2a #2277)
            for &(xp, yp) in &points {
                push_marker(svg, "hwp-chart-marker", si, xp, yp, 4.5, &color);
            }
        }
    }
}


/// 마커 경로. 계열 인덱스 사이클 ◆■▲× — 한컴 기본 (정답지 실측: 라인 ◆■▲
/// C1d #2129, OHLC 종가 ×·scatter ◆■ C2a #2277). 반환 `(d, stroke 기반 여부)` —
/// ×는 채움 없는 열린 경로라 stroke=계열색으로 그린다. `r`=명목 반경, ■/×는
/// 하프폭 `r-0.5` (종전 ◆3.5/■3.0 비율 유지 — 출력 바이트 보존).
pub(crate) fn marker_path(si: usize, cx: f64, cy: f64, r: f64) -> (String, bool) {
    let h = r - 0.5;
    match si % 4 {
        0 => (
            // ◆ 다이아몬드
            format!(
                "M{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2} Z",
                cx,
                cy - r,
                cx + r,
                cy,
                cx,
                cy + r,
                cx - r,
                cy
            ),
            false,
        ),
        1 => (
            // ■ 정사각형
            format!(
                "M{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2} Z",
                cx - h,
                cy - h,
                cx + h,
                cy - h,
                cx + h,
                cy + h,
                cx - h,
                cy + h
            ),
            false,
        ),
        2 => (
            // ▲ 삼각형
            format!(
                "M{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2} Z",
                cx,
                cy - r,
                cx + r,
                cy + r * 0.8,
                cx - r,
                cy + r * 0.8
            ),
            false,
        ),
        _ => (
            // × 두 대각선 열린 경로 — OHLC 종가 정답지 실측 (C2a #2277, 종전 원 폴백 교체)
            format!(
                "M{:.2},{:.2} L{:.2},{:.2} M{:.2},{:.2} L{:.2},{:.2}",
                cx - h,
                cy - h,
                cx + h,
                cy + h,
                cx + h,
                cy - h,
                cx - h,
                cy + h
            ),
            true,
        ),
    }
}


/// 마커 1개 방출. 채움형(◆■▲)=fill 계열색+흰 테두리, stroke 기반(×)=fill 없이
/// stroke 계열색. `class`로 데이터 마커("hwp-chart-marker")와 범례 글리프
/// ("hwp-legend-glyph", 4단계)를 구분 — issue_2129 마커 카운트 오염 방지. (C2a #2277)
pub(crate) fn push_marker(svg: &mut String, class: &str, si: usize, cx: f64, cy: f64, r: f64, color: &str) {
    let (d, stroke_based) = marker_path(si, cx, cy, r);
    if stroke_based {
        svg.push_str(&format!(
            "<path class=\"{}\" d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
            class, d, color
        ));
    } else {
        svg.push_str(&format!(
            "<path class=\"{}\" d=\"{}\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"1\"/>\n",
            class, d, color
        ));
    }
}


/// 라인 차트 표식(마커) — `marker_path` 사이클, 반경 3.5(정답지 근사, 시각판정
/// 조정 여지). (C1d #2129)
pub(crate) fn push_line_marker(svg: &mut String, si: usize, cx: f64, cy: f64, color: &str) {
    push_marker(svg, "hwp-chart-marker", si, cx, cy, 3.5, color);
}


/// 직선 폴리라인 path (`M…L…`).
pub(crate) fn polyline_path(points: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, (x, y)) in points.iter().enumerate() {
        d.push_str(&format!(
            "{}{:.2},{:.2} ",
            if i == 0 { "M" } else { "L" },
            x,
            y
        ));
    }
    d.trim().to_string()
}


/// Catmull-Rom → cubic Bézier 곡선 path. 데이터 순서, 끝점 clamp(P₋₁=P₀, Pₙ=Pₙ₋₁). — C1b #1660.
pub(crate) fn smooth_path(points: &[(f64, f64)]) -> String {
    let n = points.len();
    if n < 2 {
        return polyline_path(points);
    }
    let mut d = format!("M{:.2},{:.2}", points[0].0, points[0].1);
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 >= n {
            points[n - 1]
        } else {
            points[i + 2]
        };
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        d.push_str(&format!(
            " C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}",
            c1.0, c1.1, c2.0, c2.1, p2.0, p2.1
        ));
    }
    d
}

// ---------------- Pie ----------------


pub(crate) fn render_pie(svg: &mut String, chart: &OoxmlChart, px: f64, py: f64, pw: f64, ph: f64) {
    let first = match chart.series.first() {
        Some(s) => s,
        None => return,
    };
    let total: f64 = first.values.iter().sum();
    if total <= 0.0 {
        return;
    }
    let cx = px + pw / 2.0;
    let cy = py + ph / 2.0;
    // 쪼개진원형: 계열 explosion(%)만큼 슬라이스를 중심각 방향으로 이동 —
    // 벌어진 extent(r×(1+e))가 기존 fit과 같도록 반지름 축소. explosion 부재
    // (e=0) 시 기존 산식·출력 그대로. (C2b #2278 Stage 3 v3, 정답지 쪼개진원형)
    let explode = first.explosion.unwrap_or(0.0).max(0.0) / 100.0;
    let r = (pw.min(ph) / 2.0) * 0.9 / (1.0 + explode);

    let mut start_angle = -std::f64::consts::FRAC_PI_2;
    for (i, &v) in first.values.iter().enumerate() {
        let sweep = v / total * std::f64::consts::TAU;
        let end_angle = start_angle + sweep;
        let mid = start_angle + sweep / 2.0;
        let (ox, oy) = (cx + r * explode * mid.cos(), cy + r * explode * mid.sin());
        let (x1, y1) = (ox + r * start_angle.cos(), oy + r * start_angle.sin());
        let (x2, y2) = (ox + r * end_angle.cos(), oy + r * end_angle.sin());
        let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
        let color = color_hex(first.color.unwrap_or_else(|| palette(i)));
        svg.push_str(&format!(
            "<path d=\"M{:.2},{:.2} L{:.2},{:.2} A{:.2},{:.2} 0 {} 1 {:.2},{:.2} Z\" fill=\"{}\"/>\n",
            ox, oy, x1, y1, r, r, large, x2, y2, color
        ));
        start_angle = end_angle;
    }
}


/// ofPie — 주 원(앞 n−k 카테고리 + 결합 슬라이스) + 보조 플롯(pie|bar) + serLines.
/// k = split_pos(반올림, 1..=n−1 클램프) 없으면 2. n < 3 → 일반 원형 폴백.
/// 주 원은 결합 슬라이스 중앙이 보조 플롯(3시 방향)을 향하도록 회전. 결합 슬라이스
/// 색 = palette(n)(정답지 실측 초록계 [4]), 보조 플롯 색 = palette(n−k+j)
/// (범례의 카테고리 정순 색과 일치). (C2b #2278 Stage 3)
pub(crate) fn render_of_pie(
    svg: &mut String,
    chart: &OoxmlChart,
    of: &OfPieInfo,
    px: f64,
    py: f64,
    pw: f64,
    ph: f64,
) {
    use std::f64::consts::TAU;
    let first = match chart.series.first() {
        Some(s) => s,
        None => return,
    };
    let n = first.values.len();
    if n < 3 {
        render_pie(svg, chart, px, py, pw, ph);
        return;
    }
    let total: f64 = first.values.iter().sum();
    if total <= 0.0 {
        return;
    }
    // splitPos 의 count 해석은 splitType=pos(및 미지정 auto — 종전 코퍼스 정책
    // 보존)에만 유효. val/percent/cust 는 값·백분율·점별 임계라 count 로 읽으면
    // 오분할 — splitPos 를 무시하고 기본 2로 폴백한다. (PR #2500 후속)
    let split_pos_as_count = matches!(
        of.split_type,
        super::OfPieSplitType::Auto | super::OfPieSplitType::Pos
    );
    let k = of
        .split_pos
        .filter(|_| split_pos_as_count)
        .map(|v| (v.round() as usize).clamp(1, n - 1))
        .unwrap_or(2)
        .min(n - 1);
    let combined: f64 = first.values[n - k..].iter().sum();

    // 레이아웃 — 정답지(원형대원형-2022 임베드 2702×1577) 픽셀 실측 캘리브레이션:
    // 주 원 중심 x≈0.23·플롯폭, 보조 중심 x≈0.80, r1≈0.38·플롯높이(가로는 캡),
    // r2/r1 실측 0.754 = secondPieSize(75)/100 ✓ (스키마 의미 그대로)
    let cx1 = px + pw * 0.23;
    let cy = py + ph / 2.0;
    let r1 = (pw * 0.46).min(ph * 0.76) / 2.0;
    let cx2 = px + pw * 0.80;
    let r2 = r1 * (of.second_pie_size.max(0.0) / 100.0);

    // 주 원 — 값 시퀀스 = values[..n−k] + [combined]. 결합 슬라이스 중앙이 3시(θ=0)
    let sweep_c = combined / total * TAU;
    let start_main = -sweep_c / 2.0 - (total - combined) / total * TAU;
    let main_vals: Vec<(f64, u32)> = first.values[..n - k]
        .iter()
        .enumerate()
        .map(|(ci, &v)| (v, first.color.unwrap_or_else(|| palette(ci))))
        .chain(std::iter::once((
            combined,
            first.color.unwrap_or_else(|| palette(n)),
        )))
        .collect();
    let mut start_angle = start_main;
    for (v, rgb) in &main_vals {
        let sweep = v / total * TAU;
        let end_angle = start_angle + sweep;
        let (x1, y1) = (cx1 + r1 * start_angle.cos(), cy + r1 * start_angle.sin());
        let (x2, y2) = (cx1 + r1 * end_angle.cos(), cy + r1 * end_angle.sin());
        let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
        svg.push_str(&format!(
            "<path class=\"hwp-ofpie-main\" d=\"M{:.2},{:.2} L{:.2},{:.2} A{:.2},{:.2} 0 {} 1 {:.2},{:.2} Z\" fill=\"{}\"/>\n",
            cx1, cy, x1, y1, r1, r1, large, x2, y2, color_hex(*rgb)
        ));
        start_angle = end_angle;
    }

    if combined > 0.0 {
        match of.of_pie_type {
            OfPieType::Pie => {
                // 보조 원 — 시작각 = +sweep_c/2 (결합 슬라이스 아래 모서리 각도와
                // 정렬, 정답지 실측 경계 26°≈유도 30°; 12시 시작 아님)
                let mut s = sweep_c / 2.0;
                for (j, &v) in first.values[n - k..].iter().enumerate() {
                    let sweep = v / combined * TAU;
                    let e = s + sweep;
                    let (x1, y1) = (cx2 + r2 * s.cos(), cy + r2 * s.sin());
                    let (x2, y2) = (cx2 + r2 * e.cos(), cy + r2 * e.sin());
                    let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
                    let rgb = first.color.unwrap_or_else(|| palette(n - k + j));
                    svg.push_str(&format!(
                        "<path class=\"hwp-ofpie-second\" d=\"M{:.2},{:.2} L{:.2},{:.2} A{:.2},{:.2} 0 {} 1 {:.2},{:.2} Z\" fill=\"{}\"/>\n",
                        cx2, cy, x1, y1, r2, r2, large, x2, y2, color_hex(rgb)
                    ));
                    s = e;
                }
            }
            OfPieType::Bar => {
                // 보조 누적 막대 — 첫 분할 카테고리가 맨 위, 위→아래 누적
                let bar_h = 2.0 * r2;
                let bar_w = bar_h * 0.45;
                let bx = cx2 - bar_w / 2.0;
                let top = cy - bar_h / 2.0;
                let mut acc = 0.0;
                for (j, &v) in first.values[n - k..].iter().enumerate() {
                    let seg = v / combined * bar_h;
                    let rgb = first.color.unwrap_or_else(|| palette(n - k + j));
                    svg.push_str(&format!(
                        "<rect class=\"hwp-ofpie-second\" x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
                        bx,
                        top + acc,
                        bar_w,
                        seg,
                        color_hex(rgb)
                    ));
                    acc += seg;
                }
            }
        }

        // serLines — 결합 슬라이스 양 모서리 → 보조 플롯. 색은 검정(정답지 실측
        // 코어 (8,8,8)). Pie는 보조 원 **접선점**(실측: 원 상/하단 아님 —
        // 모서리에서 원에 접하는 선), Bar는 막대 좌변 상/하단.
        if of.has_ser_lines {
            let (ux, uy) = (
                cx1 + r1 * (-sweep_c / 2.0).cos(),
                cy + r1 * (-sweep_c / 2.0).sin(),
            );
            let (lx, ly) = (
                cx1 + r1 * (sweep_c / 2.0).cos(),
                cy + r1 * (sweep_c / 2.0).sin(),
            );
            // 외부점 P → 원(cx2, cy, r2) 접점: 중심→P 각 α, β=acos(r2/d), α±β 중
            // 위 연결선은 위쪽(작은 y) 접점 선택. d ≤ r2 퇴화 시 원 좌측점 폴백.
            let tangent = |pxp: f64, pyp: f64, upper: bool| -> (f64, f64) {
                let (dx, dy) = (pxp - cx2, pyp - cy);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= r2 {
                    return (cx2 - r2, cy);
                }
                let alpha = dy.atan2(dx);
                let beta = (r2 / d).acos();
                let p1 = (
                    cx2 + r2 * (alpha + beta).cos(),
                    cy + r2 * (alpha + beta).sin(),
                );
                let p2 = (
                    cx2 + r2 * (alpha - beta).cos(),
                    cy + r2 * (alpha - beta).sin(),
                );
                if (p1.1 < p2.1) == upper {
                    p1
                } else {
                    p2
                }
            };
            let ((tx, ty), (bx2, by2)) = match of.of_pie_type {
                OfPieType::Pie => (tangent(ux, uy, true), tangent(lx, ly, false)),
                OfPieType::Bar => {
                    let bar_h = 2.0 * r2;
                    let bar_w = bar_h * 0.45;
                    let bx = cx2 - bar_w / 2.0;
                    ((bx, cy - r2), (bx, cy + r2))
                }
            };
            for ((x1, y1), (x2, y2)) in [((ux, uy), (tx, ty)), ((lx, ly), (bx2, by2))] {
                svg.push_str(&format!(
                    "<line class=\"hwp-ofpie-serline\" x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#000000\" stroke-width=\"0.75\"/>\n",
                    x1, y1, x2, y2
                ));
            }
        }
    }
}
