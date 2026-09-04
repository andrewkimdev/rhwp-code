//! [Issue #6619] `hp:cellzone` 의 **테두리**를 렌더러가 한 번도 방출하지 않아, 오직
//! zone 만 참조하는 선이 통째로 사라지던 결함의 가드 (upstream `950300ea7b`+`6161b4a571`).
//!
//! zone 은 셀 고유 `borderFillIDRef` 위에 얹는 **영역 덮어쓰기**다. 종전 렌더러는
//! zone 에 대해 배경과 대각선만 그리고 네 변을 방출하지 않아, zone 만 참조하는 선이
//! 통째로 사라지거나(축 ①), 셀 고유 테두리가 그대로 남는(축 ②) 결함이 있었다.
//! 두 번째 커밋은 첫 수정이 병합 칸 끝 주소를 그리드 좌표로 오인해 반쪽 틀만 그리던
//! 후속 결함을 고쳤다(축 ③).

use rhwp::wasm_api::HwpDocument;
use serde_json::Value;

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

/// `(x1, y1, x2, y2, stroke)`
fn rendered_lines(svg: &str) -> Vec<(f64, f64, f64, f64, String)> {
    svg.split("<line")
        .skip(1)
        .filter_map(|part| {
            let tag = part.split('>').next()?;
            Some((
                line_f64(tag, "x1")?,
                line_f64(tag, "y1")?,
                line_f64(tag, "x2")?,
                line_f64(tag, "y2")?,
                line_attr(tag, "stroke").unwrap_or_default(),
            ))
        })
        .collect()
}

const NONE_BORDER_PROPS: &str = r##"{
    "borderFillId":1,
    "borderLeft":{"type":0,"width":0,"color":"#000000"},
    "borderRight":{"type":0,"width":0,"color":"#000000"},
    "borderTop":{"type":0,"width":0,"color":"#000000"},
    "borderBottom":{"type":0,"width":0,"color":"#000000"},
    "fillType":"none",
    "centerLine":"NONE"
}"##;

/// 축 ① — 칸이 전부 NONE 테두리일 때, zone 의 L/R/B 세 변(zone 은 위만 NONE)이
/// 렌더돼야 한다. 종전에는 zone 이 배경·대각선만 그리고 네 변을 한 번도 방출하지
/// 않아 이 세 변이 통째로 소실됐다.
#[test]
fn issue_6619_cellzone_border_draws_sides_no_cell_owns() {
    let mut doc = HwpDocument::create_empty();
    let created = doc
        .create_table_ex_native(
            0,
            0,
            0,
            2,
            2,
            true,
            Some(&[10000, 10000]),
            Some(&[7200, 7200]),
        )
        .expect("표 생성");
    let created: Value = serde_json::from_str(&created).expect("createTable JSON");
    let ppi = created["paraIdx"].as_u64().expect("paraIdx") as u32;
    let ci = created["controlIdx"].as_u64().expect("controlIdx") as u32;

    // 칸 4개 모두 테두리 없음으로 만든다 — zone 만 참조하는 선인지 명확히 하기 위함.
    for cell_idx in 0..4 {
        doc.set_cell_properties(0, ppi, ci, cell_idx, NONE_BORDER_PROPS)
            .unwrap_or_else(|err| panic!("cell {cell_idx} 테두리 제거 실패: {err:?}"));
    }

    let zone_props = r##"{
        "borderFillId":1,
        "borderLeft":{"type":1,"width":0,"color":"#FF0000"},
        "borderRight":{"type":1,"width":0,"color":"#FF0000"},
        "borderTop":{"type":0,"width":0,"color":"#000000"},
        "borderBottom":{"type":1,"width":0,"color":"#FF0000"},
        "fillType":"none",
        "centerLine":"NONE"
    }"##;
    doc.set_cell_zone_properties(0, ppi, ci, 0, 0, 1, 1, zone_props)
        .expect("cellzone 테두리 적용");

    let svg = doc.render_page_svg_native(0).expect("SVG 렌더");
    let lines = rendered_lines(&svg);

    let red_lines: Vec<_> = lines.iter().filter(|(.., c)| c == "#ff0000").collect();
    assert!(
        !red_lines.is_empty(),
        "zone 의 빨간 테두리가 전혀 렌더되지 않음 — #6619 회귀. lines={lines:?}"
    );

    let has_vertical = red_lines
        .iter()
        .any(|(x1, y1, x2, y2, _)| (x1 - x2).abs() < 0.5 && (y1 - y2).abs() > 20.0);
    let has_horizontal = red_lines
        .iter()
        .any(|(x1, y1, x2, y2, _)| (y1 - y2).abs() < 0.5 && (x1 - x2).abs() > 20.0);
    assert!(
        has_vertical && has_horizontal,
        "zone 의 세로+가로 변이 모두 렌더돼야 한다 — #6619 회귀. \
         vertical={has_vertical} horizontal={has_horizontal} red_lines={red_lines:?}"
    );
}

/// 축 ② — zone 은 칸 고유 테두리를 **이긴다**. 칸이 검정 실선을 갖고 있어도 zone
/// 의 색(빨강)이 외곽에 나와야 한다. 종전에는 zone 자체가 무시돼 칸 고유 검정
/// 테두리만 남았다.
#[test]
fn issue_6619_cellzone_border_overrides_cell_own_border() {
    let mut doc = HwpDocument::create_empty();
    let created = doc
        .create_table_ex_native(
            0,
            0,
            0,
            2,
            2,
            true,
            Some(&[10000, 10000]),
            Some(&[7200, 7200]),
        )
        .expect("표 생성");
    let created: Value = serde_json::from_str(&created).expect("createTable JSON");
    let ppi = created["paraIdx"].as_u64().expect("paraIdx") as u32;
    let ci = created["controlIdx"].as_u64().expect("controlIdx") as u32;
    // create_table_ex_native 기본 셀 테두리는 이미 검정 실선(Solid) — 별도 설정 불필요.

    let before_svg = doc.render_page_svg_native(0).expect("SVG 렌더(전)");
    let before_red = rendered_lines(&before_svg)
        .iter()
        .filter(|(.., c)| c == "#ff0000")
        .count();
    assert_eq!(
        before_red, 0,
        "zone 적용 전에는 빨간 선이 없어야 함(테스트 전제)"
    );

    let zone_props = r##"{
        "borderFillId":1,
        "borderLeft":{"type":1,"width":0,"color":"#FF0000"},
        "borderRight":{"type":1,"width":0,"color":"#FF0000"},
        "borderTop":{"type":1,"width":0,"color":"#FF0000"},
        "borderBottom":{"type":1,"width":0,"color":"#FF0000"},
        "fillType":"none",
        "centerLine":"NONE"
    }"##;
    doc.set_cell_zone_properties(0, ppi, ci, 0, 0, 1, 1, zone_props)
        .expect("cellzone 테두리 적용");

    let svg = doc.render_page_svg_native(0).expect("SVG 렌더(후)");
    let lines = rendered_lines(&svg);
    let red_count = lines.iter().filter(|(.., c)| c == "#ff0000").count();
    assert!(
        red_count >= 4,
        "zone 이 칸 고유 검정 테두리를 이기고 외곽 4변에 빨간 선을 그려야 한다 — \
         #6619 회귀. red_count={red_count} lines={lines:?}"
    );
}

/// 축 ③ — zone 의 끝 주소가 **병합 칸**을 가리키면 그 span 끝까지가 바깥 변이다.
/// `startColAddr`/`endColAddr` 는 그리드 좌표가 아니라 칸 주소다. 2행 3열 표의
/// 아래 행 3칸을 병합한 뒤, zone 이 그 병합 칸의 origin(row=1,col=0)만 가리켜도
/// 렌더된 아래 변은 표 오른쪽 끝까지 가야 한다. 종전에는 `end_col+1` 로 계산해
/// 오른쪽 1/3 지점에서 끊겼다.
#[test]
fn issue_6619_cellzone_end_address_follows_merged_span() {
    let mut doc = HwpDocument::create_empty();
    let created = doc
        .create_table_ex_native(
            0,
            0,
            0,
            2,
            3,
            true,
            Some(&[10000, 10000, 10000]),
            Some(&[7200, 7200]),
        )
        .expect("표 생성");
    let created: Value = serde_json::from_str(&created).expect("createTable JSON");
    let ppi = created["paraIdx"].as_u64().expect("paraIdx") as u32;
    let ci = created["controlIdx"].as_u64().expect("controlIdx") as u32;

    // 아래 행(row=1) 세 칸을 하나로 병합 — origin (row=1, col=0), col_span=3.
    doc.merge_table_cells(0, ppi, ci, 1, 0, 1, 2)
        .expect("아래 행 병합");

    let zone_props = r##"{
        "borderFillId":1,
        "borderLeft":{"type":0,"width":0,"color":"#000000"},
        "borderRight":{"type":0,"width":0,"color":"#000000"},
        "borderTop":{"type":0,"width":0,"color":"#000000"},
        "borderBottom":{"type":1,"width":0,"color":"#FF0000"},
        "fillType":"none",
        "centerLine":"NONE"
    }"##;
    // zone 은 병합 칸의 origin 주소(1,0)~(1,0)만 가리킨다 — 실제 병합 span 은 3칸.
    doc.set_cell_zone_properties(0, ppi, ci, 1, 0, 1, 0, zone_props)
        .expect("cellzone 테두리 적용");

    let svg = doc.render_page_svg_native(0).expect("SVG 렌더");
    let lines = rendered_lines(&svg);
    let red_lines: Vec<_> = lines.iter().filter(|(.., c)| c == "#ff0000").collect();
    assert!(
        !red_lines.is_empty(),
        "병합 칸을 가리키는 zone 의 빨간 아래 변이 렌더되지 않음. lines={lines:?}"
    );

    // 표 전체 폭에 해당하는 가로선(가장 긴 가로선 기준 ≥90%)이 하나는 있어야
    // 한다 — 병합 무시 버그는 1/3 지점(짧은 선)에서 끊긴다.
    let max_horizontal_span = lines
        .iter()
        .filter(|(x1, y1, x2, y2, _)| (y1 - y2).abs() < 0.5 && (x1 - x2).abs() > 1.0)
        .map(|(x1, _, x2, _, _)| (x2 - x1).abs())
        .fold(0.0_f64, f64::max);
    let full_width_red = red_lines.iter().any(|(x1, y1, x2, y2, _)| {
        (y1 - y2).abs() < 0.5 && (x2 - x1).abs() >= max_horizontal_span * 0.9
    });
    assert!(
        full_width_red,
        "병합 span 무시 회귀 — zone 아래 변이 표 전체 폭(가장 긴 가로선의 ≥90%)까지 \
         가야 한다. max_horizontal_span={max_horizontal_span} red_lines={red_lines:?}"
    );
}
