//! [Issue #6577] EMF 플레이어가 `EMR_POLYLINETO16` · `EMR_POLYBEZIERTO16` ·
//! `EMR_EXTCREATEPEN` · `EMR_INTERSECTCLIPRECT` · `EMR_EXTSELECTCLIPRGN` ·
//! `SetWorldTransform`/`ModifyWorldTransform` 을 처리하지 않아, **패스 기반 EMF**
//! (Office·Illustrator 산출물)의 도형이 통째로 사라지거나 뭉치던 결함의 가드.
//!
//! 패스 기반 EMF는 도형을 거의 전부
//! `BeginPath → MoveTo → {PolylineTo16 | PolyBezierTo16}* → CloseFigure → FillPath`
//! 로 표현한다. 종전에는
//!
//! - `PolylineTo16`/`PolyBezierTo16`/`ExtCreatePen`/`IntersectClipRect`/
//!   `ExtSelectClipRgn` 레코드가 `Record::Unknown` 으로 버려지고,
//! - `MoveToEx` 는 `current_pos` 만 갱신하고 `path_d` 를 건드리지 않아
//! - `SetWorldTransform`/`ModifyWorldTransform` 을 받고도 DC 저장만 하고 출력에는
//!   적용하지 않았다.
//!
//! upstream `origin/devel` 실측(156627451 내장 EMF, 306,980B)의 레코드 분포:
//!
//! ```text
//! POLYBEZIERTO16  758      POLYLINETO16  644
//! SELECTOBJECT    338(스톡 197)  EMR_EXTCREATEPEN 16(EMR_CREATEPEN 0)
//! INTERSECTCLIPRECT 112
//! ```

#![cfg(not(target_arch = "wasm32"))]

use rhwp::emf::parser::records::Record;

fn record_types(emf: &[u8]) -> Vec<String> {
    rhwp::emf::parser::parse(emf)
        .map(|records| {
            records
                .iter()
                .map(|record| match record {
                    Record::PolylineTo16 { .. } => "PolylineTo16".to_string(),
                    Record::PolyBezierTo16 { .. } => "PolyBezierTo16".to_string(),
                    Record::IntersectClipRect(_) => "IntersectClipRect".to_string(),
                    Record::ExtSelectClipRgn { .. } => "ExtSelectClipRgn".to_string(),
                    Record::Unknown { .. } => "Unknown".to_string(),
                    other => format!("{other:?}")
                        .split_whitespace()
                        .next()
                        .unwrap_or("?")
                        .trim_end_matches('(')
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_record(out: &mut Vec<u8>, kind: u32, payload: &[u8]) {
    let size = 8 + payload.len() as u32;
    push_u32(out, kind);
    push_u32(out, size);
    out.extend_from_slice(payload);
}

#[test]
fn polyline_to16_and_polybezier_to16_are_parsed() {
    // EMR_HEADER: iType=1, 최소 88바이트. offset 40 에 " EMF" 서명.
    let mut emf = Vec::new();
    let mut header = vec![0u8; 80];
    header[32..36].copy_from_slice(b" EMF"); // payload 기준 offset 40
    push_record(&mut emf, 1, &header);

    // EMR_POLYLINETO16 (0x59): bounds(16) + count(4) + points
    let mut payload = vec![0u8; 16];
    push_u32(&mut payload, 1);
    payload.extend_from_slice(&20i16.to_le_bytes());
    payload.extend_from_slice(&10i16.to_le_bytes());
    push_record(&mut emf, 0x59, &payload);

    // EMR_POLYBEZIERTO16 (0x58): 제어점 2 + 끝점 1
    let mut payload = vec![0u8; 16];
    push_u32(&mut payload, 3);
    for (x, y) in [(25i16, 5i16), (35, 5), (40, 10)] {
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
    }
    push_record(&mut emf, 0x58, &payload);

    // EMR_EXTCREATEPEN (0x5F): ihPen + offBmi/cbBmi/offBits/cbBits + LogPenEx 앞 4필드
    let mut payload = Vec::new();
    push_u32(&mut payload, 7); // ihPen
    for _ in 0..4 {
        push_u32(&mut payload, 0);
    }
    push_u32(&mut payload, 0); // PenStyle = PS_SOLID
    push_u32(&mut payload, 3); // Width
    push_u32(&mut payload, 0); // BrushStyle = BS_SOLID
    push_u32(&mut payload, 0x0000_00FF); // ColorRef
    push_u32(&mut payload, 0); // BrushHatch
    push_u32(&mut payload, 0); // NumStyleEntries
    push_record(&mut emf, 0x5F, &payload);

    // EMR_INTERSECTCLIPRECT (0x1E): RectL(16B)
    let mut payload = Vec::new();
    for v in [10i32, 20, 110, 220] {
        payload.extend_from_slice(&v.to_le_bytes());
    }
    push_record(&mut emf, 0x1E, &payload);

    // EMR_EXTSELECTCLIPRGN (0x4B): RgnDataSize=0 + RegionMode=RGN_COPY(5) = 클립 해제
    let mut payload = Vec::new();
    push_u32(&mut payload, 0);
    push_u32(&mut payload, 5);
    push_record(&mut emf, 0x4B, &payload);

    push_record(&mut emf, 14, &[]); // EMR_EOF (내용 없이도 파서가 멈추면 됨)

    let types = record_types(&emf);
    assert!(
        types.iter().any(|t| t == "PolylineTo16"),
        "EMR_POLYLINETO16 이 Unknown 으로 버려졌다 — #6577 회귀. 파싱 결과: {types:?}"
    );
    assert!(
        types.iter().any(|t| t == "PolyBezierTo16"),
        "EMR_POLYBEZIERTO16 이 Unknown 으로 버려졌다 — #6577 회귀. 파싱 결과: {types:?}"
    );
    assert!(
        types.iter().any(|t| t == "CreatePen"),
        "EMR_EXTCREATEPEN 이 Unknown 으로 버려졌다 — #6577 회귀. 파싱 결과: {types:?}"
    );
    assert!(
        types.iter().any(|t| t == "IntersectClipRect"),
        "EMR_INTERSECTCLIPRECT 가 Unknown 으로 버려졌다 — #6577 회귀. 파싱 결과: {types:?}"
    );
    assert!(
        types.iter().any(|t| t == "ExtSelectClipRgn"),
        "EMR_EXTSELECTCLIPRGN 이 Unknown 으로 버려졌다 — #6577 회귀. 파싱 결과: {types:?}"
    );
}

/// SVG 층 잠금 — 클립이 걸린 채 그려진 패스에는 `clip-path` 가 붙어야 한다.
///
/// 파싱만 잠그면 플레이어가 레코드를 받고도 **버리는** 회귀를 못 잡는다. 실제로
/// 종전 변환기에는 클리핑 개념이 아예 없었다(`clip` 문자열이 한 번도 안 나온다).
#[test]
fn intersect_clip_rect_reaches_the_svg() {
    let mut emf = Vec::new();
    let mut header = vec![0u8; 80];
    // Bounds/Frame 를 채워야 변환기가 viewBox 를 만든다(0 이면 None).
    for (i, v) in [0i32, 0, 200, 300].iter().enumerate() {
        header[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    for (i, v) in [0i32, 0, 5000, 7500].iter().enumerate() {
        header[16 + i * 4..16 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    header[32..36].copy_from_slice(b" EMF");
    push_record(&mut emf, 1, &header);

    // 클립 (10,20)-(110,220)
    let mut payload = Vec::new();
    for v in [10i32, 20, 110, 220] {
        payload.extend_from_slice(&v.to_le_bytes());
    }
    push_record(&mut emf, 0x1E, &payload);

    push_record(&mut emf, 0x3B, &[]); // BeginPath

    // MoveToEx(10,10)
    let mut payload = Vec::new();
    payload.extend_from_slice(&10i32.to_le_bytes());
    payload.extend_from_slice(&10i32.to_le_bytes());
    push_record(&mut emf, 0x1B, &payload);

    // PolylineTo16 → (90,90)
    let mut payload = vec![0u8; 16];
    push_u32(&mut payload, 1);
    payload.extend_from_slice(&90i16.to_le_bytes());
    payload.extend_from_slice(&90i16.to_le_bytes());
    push_record(&mut emf, 0x59, &payload);

    push_record(&mut emf, 0x3C, &[]); // EndPath
    push_record(&mut emf, 0x40, &vec![0u8; 16]); // StrokePath(bounds)
    push_record(&mut emf, 14, &[]);

    let svg = rhwp::emf::convert_to_standalone_svg(&emf)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();

    assert!(
        svg.contains("<clipPath") && svg.contains("clip-path=\"url(#"),
        "EMR_INTERSECTCLIPRECT 가 SVG 에 도달하지 못했다 — #6577 ④ 회귀. SVG: {svg}"
    );
}

/// 월드 변환 잠금 — `SetWorldTransform` 이 SVG 에 도달해야 한다.
///
/// 종전 플레이어는 `SetWorldTransform`·`ModifyWorldTransform` 을 **받고도 버렸다**.
/// 그 결과 축소·확대 변환이 걸린 도형이 왜곡된 크기로 그려진다.
#[test]
fn world_transform_reaches_the_svg() {
    let mut emf = Vec::new();
    let mut header = vec![0u8; 80];
    for (i, v) in [0i32, 0, 200, 300].iter().enumerate() {
        header[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    for (i, v) in [0i32, 0, 5000, 7500].iter().enumerate() {
        header[16 + i * 4..16 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    header[32..36].copy_from_slice(b" EMF");
    push_record(&mut emf, 1, &header);

    // EMR_SETWORLDTRANSFORM (0x23): XForm 6×f32 — 0.5 배 축소
    let mut payload = Vec::new();
    for v in [0.5f32, 0.0, 0.0, 0.5, 0.0, 0.0] {
        payload.extend_from_slice(&v.to_le_bytes());
    }
    push_record(&mut emf, 0x23, &payload);

    // EMR_RECTANGLE (0x2B): RectL
    let mut payload = Vec::new();
    for v in [10i32, 10, 100, 100] {
        payload.extend_from_slice(&v.to_le_bytes());
    }
    push_record(&mut emf, 0x2B, &payload);
    push_record(&mut emf, 14, &[]);

    let svg = rhwp::emf::convert_to_standalone_svg(&emf)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();

    assert!(
        svg.contains("matrix(0.500000"),
        "SetWorldTransform 이 SVG 에 도달하지 못했다 — #6577 ④ 회귀. SVG: {svg}"
    );
}
