//! [#4100] OOXML 차트 XML 의 숫자 값을 **바이트 구간으로** 특정한다.
//!
//! ## 왜 별도 스캐너인가
//!
//! 같은 모듈의 [`parser`](super::parser) 는 렌더용 **손실 파서**다 —
//! `c:pt idx`·`c:f`·`c:externalData`·`extLst` 를 읽지 않는다. 파싱→재방출로
//! 왕복시키면 모델에 없는 것이 전부 사라지는데, `samples/chart` 코퍼스 28종은
//! 전건이 `c:extLst` 와 `ho:hncChartStyle` 을 갖고 있다. 그래서 편집은 재직렬화가
//! 아니라 **`c:v` 텍스트 구간만 바꾸는 최소 diff 바이트 수술**로 한다(#4055 실측).
//!
//! ## 구간을 얻는 방법
//!
//! 문자열 검색이 아니라 `quick_xml` + [`Reader::buffer_position`] 오프셋 추적이다.
//! `src/parser/hwpx/header.rs` 의 `paraHead` 원본 보존과 같은 관용구다 — 여는 태그
//! 직후 위치를 시작으로, **이번 이터레이션 read 직전 위치**를 끝으로 잡아 닫는 태그
//! 길이를 계산하지 않고 byte-exact 구간을 얻는다. 이 방식이라야
//!
//! - 접두어(`c:`)를 고정하지 않고 (`local_name` 비교),
//! - 코퍼스에 1건 있는 `c:numLit`/`c:strLit` 리터럴도 캐시형과 같은 경로로
//!
//! 잡힌다.
//!
//! `Reader::from_str` 이어야 `buffer_position()` 이 곧 입력 바이트 오프셋이다.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::ops::Range;

/// 값 점 하나 — `<c:v>` 텍스트의 원본 바이트 구간과 그 텍스트.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartPoint {
    /// `c:pt idx` 속성값. 코퍼스는 전건 `0..n-1` 순차지만 해석하지 않고 그대로 싣는다.
    pub idx: u32,
    /// 원본 바이트 안 텍스트 구간 `[start, end)`.
    pub span: Range<usize>,
    /// 구간의 바이트 그대로. **이스케이프를 해제하지 않는다** — 이 문자열을 되쓰면
    /// 바이트가 원본과 같아야 한다는 것이 최소 diff 의 기준이다.
    pub text: String,
}

/// 계열의 가로축 표현.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesAxis {
    /// `c:cat` + `c:val` — 라벨은 문자열이고 B1 에서 편집 대상이 아니다.
    Category,
    /// `c:xVal` + `c:yVal` (분산형) — X 도 수치이고 편집 대상이다.
    Scatter,
}

/// 계열 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartSeries {
    /// `c:tx` 의 계열명. 참조도 리터럴도 없으면 `None`.
    pub name: Option<String>,
    pub axis: SeriesAxis,
    /// [`SeriesAxis::Category`] 면 `c:cat` 라벨, [`SeriesAxis::Scatter`] 면 `c:xVal`.
    pub labels: Vec<ChartPoint>,
    /// [`SeriesAxis::Category`] 면 `c:val`, [`SeriesAxis::Scatter`] 면 `c:yVal`.
    pub values: Vec<ChartPoint>,
    /// 라벨이 `c:multiLvlStrRef`(다층 카테고리)에서 왔는가.
    ///
    /// 코퍼스 0건이다. 스캐너는 층을 평탄화해 싣기만 하고 판단하지 않는다 — 라벨을
    /// 행 머리로 쓰는 CSV 층이 이 표지를 보고 거부한다. 값 편집 자체는 막지 않는다.
    pub labels_multi_level: bool,
}

/// 차트 하나의 편집 가능한 데이터 지도.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChartData {
    pub series: Vec<ChartSeries>,
}

/// 스캔 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChartScanError {
    /// 입력이 UTF-8 이 아니다. OOXML 차트 파트는 UTF-8 XML 이어야 한다.
    NotUtf8,
    /// XML 파싱 오류.
    Xml(String),
    /// `c:ser` 가 하나도 없다 — 편집할 데이터가 없다.
    NoSeries,
    /// 빈 요소 `<c:v/>` 를 만났다.
    ///
    /// 텍스트 구간이 없어 제자리 치환으로 값을 넣을 수 없다. 코퍼스 0건이며, 반쪽만
    /// 새 값인 파일을 내보내느니 스캔 단계에서 드러낸다.
    EmptyValueElement { series: usize },
}

impl std::fmt::Display for ChartScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotUtf8 => write!(f, "차트 XML 이 UTF-8 이 아니다"),
            Self::Xml(msg) => write!(f, "차트 XML 파싱 실패: {msg}"),
            Self::NoSeries => write!(f, "차트에 c:ser 가 없다"),
            Self::EmptyValueElement { series } => {
                write!(f, "계열 {series} 에 빈 <c:v/> 가 있어 제자리 치환이 불가능하다")
            }
        }
    }
}

/// `c:ser` 안에서 지금 어느 자식 블록에 있는가.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    /// `c:tx` — 계열명
    Tx,
    /// `c:cat` — 카테고리 라벨
    Cat,
    /// `c:val` — 값
    Val,
    /// `c:xVal` — 분산형 X
    XVal,
    /// `c:yVal` — 분산형 Y
    YVal,
}

/// 스캔 진행 상태.
struct ScanState {
    series: Vec<ChartSeries>,
    cur: Option<ChartSeries>,
    section: Section,
    /// `numCache`/`numLit`/`strCache`/`strLit` 안인가.
    in_cache: bool,
    /// 현재 `c:pt` 의 `idx`.
    cur_idx: Option<u32>,
    /// `<c:v>` 여는 태그 직후 오프셋.
    v_start: Option<usize>,
    /// 현재 계열의 라벨이 다층 참조에서 왔는가.
    multi_level: bool,
}

impl ScanState {
    fn new() -> Self {
        Self {
            series: Vec::new(),
            cur: None,
            section: Section::None,
            in_cache: false,
            cur_idx: None,
            v_start: None,
            multi_level: false,
        }
    }

    /// 지금 `<c:v>` 를 값으로 거둘 문맥인가.
    ///
    /// `c:tx` 는 캐시·`c:pt` 없이 `<c:tx><c:v>이름</c:v></c:tx>` 로도 쓰이므로
    /// 블록 안이면 곧바로 거둔다. 나머지 블록은 캐시/리터럴 안의 `c:pt` 로 제한해
    /// `c:f`(참조 수식)·`c:formatCode` 같은 형제 텍스트를 값으로 오인하지 않는다.
    fn capturing(&self) -> bool {
        if self.cur.is_none() {
            return false;
        }
        match self.section {
            Section::None => false,
            Section::Tx => true,
            _ => self.in_cache && self.cur_idx.is_some(),
        }
    }
}

/// `c:pt` 의 `idx` 속성.
fn pt_index(e: &BytesStart) -> u32 {
    for attr in e.attributes().flatten() {
        if attr.key.local_name().as_ref() == b"idx" {
            return std::str::from_utf8(&attr.value)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn open_section(state: &mut ScanState, local: &[u8]) {
    let section = match local {
        b"tx" => Section::Tx,
        b"cat" => Section::Cat,
        b"val" => Section::Val,
        b"xVal" => Section::XVal,
        b"yVal" => Section::YVal,
        _ => return,
    };
    if state.cur.is_some() {
        state.section = section;
        // 블록이 열릴 때 축을 확정한다. xVal/yVal 이 하나라도 나오면 분산형이다.
        if let Some(cur) = state.cur.as_mut() {
            if matches!(section, Section::XVal | Section::YVal) {
                cur.axis = SeriesAxis::Scatter;
            }
        }
    }
}

fn push_point(state: &mut ScanState, point: ChartPoint) {
    let section = state.section;
    let Some(cur) = state.cur.as_mut() else {
        return;
    };
    match section {
        Section::Tx => {
            if cur.name.is_none() {
                cur.name = Some(point.text);
            }
        }
        Section::Cat | Section::XVal => cur.labels.push(point),
        Section::Val | Section::YVal => cur.values.push(point),
        Section::None => {}
    }
}

/// 차트 XML 을 훑어 계열별 값의 바이트 구간을 얻는다.
///
/// 반환된 [`ChartPoint::span`] 은 **입력 `xml` 슬라이스 기준**이며
/// `&xml[span] == text.as_bytes()` 가 항상 성립한다.
pub fn scan_chart_values(xml: &[u8]) -> Result<ChartData, ChartScanError> {
    let text = std::str::from_utf8(xml).map_err(|_| ChartScanError::NotUtf8)?;

    let mut reader = Reader::from_str(text);
    let mut state = ScanState::new();
    let mut buf = Vec::new();

    loop {
        let pos_before = reader.buffer_position() as usize;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.local_name().as_ref() {
                    b"ser" => {
                        state.cur = Some(ChartSeries {
                            name: None,
                            axis: SeriesAxis::Category,
                            labels: Vec::new(),
                            values: Vec::new(),
                            labels_multi_level: false,
                        });
                        state.multi_level = false;
                    }
                    b"multiLvlStrRef" => state.multi_level = true,
                    b"numCache" | b"numLit" | b"strCache" | b"strLit" => state.in_cache = true,
                    b"pt" => state.cur_idx = Some(pt_index(e)),
                    b"v" => {
                        if state.capturing() {
                            // 여는 태그 `<c:v>` 직후 = 텍스트 시작.
                            state.v_start = Some(reader.buffer_position() as usize);
                        }
                    }
                    local => open_section(&mut state, local),
                }
            }
            Ok(Event::Empty(ref e)) => {
                if e.local_name().as_ref() == b"v" && state.capturing() {
                    return Err(ChartScanError::EmptyValueElement {
                        series: state.series.len(),
                    });
                }
            }
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                b"v" => {
                    if let Some(start) = state.v_start.take() {
                        // 닫는 태그 직전 = 텍스트 끝. 닫는 태그 길이를 계산하지 않는다.
                        let span = start..pos_before;
                        let idx = state.cur_idx.unwrap_or(0);
                        let text = text
                            .get(span.clone())
                            .map(str::to_string)
                            .unwrap_or_default();
                        push_point(&mut state, ChartPoint { idx, span, text });
                    }
                }
                b"pt" => state.cur_idx = None,
                b"numCache" | b"numLit" | b"strCache" | b"strLit" => state.in_cache = false,
                b"tx" | b"cat" | b"val" | b"xVal" | b"yVal" => state.section = Section::None,
                b"ser" => {
                    if let Some(mut cur) = state.cur.take() {
                        cur.labels_multi_level = state.multi_level;
                        state.series.push(cur);
                    }
                    state.section = Section::None;
                    state.in_cache = false;
                    state.cur_idx = None;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(ChartScanError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    if state.series.is_empty() {
        return Err(ChartScanError::NoSeries);
    }
    Ok(ChartData {
        series: state.series,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATEGORY_CHART: &str = concat!(
        r#"<c:chartSpace><c:chart><c:plotArea><c:barChart>"#,
        r#"<c:ser><c:idx val="0"/><c:order val="0"/>"#,
        r#"<c:tx><c:strRef><c:f>Sheet1!$B$1</c:f><c:strCache><c:ptCount val="1"/>"#,
        r#"<c:pt idx="0"><c:v>계열 1</c:v></c:pt></c:strCache></c:strRef></c:tx>"#,
        r#"<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/>"#,
        r#"<c:pt idx="0"><c:v>항목 1</c:v></c:pt><c:pt idx="1"><c:v>항목 2</c:v></c:pt>"#,
        r#"</c:strCache></c:strRef></c:cat>"#,
        r#"<c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:numCache>"#,
        r#"<c:formatCode>General</c:formatCode><c:ptCount val="2"/>"#,
        r#"<c:pt idx="0"><c:v>4.3</c:v></c:pt><c:pt idx="1"><c:v>2.5</c:v></c:pt>"#,
        r#"</c:numCache></c:numRef></c:val></c:ser>"#,
        r#"</c:barChart></c:plotArea></c:chart></c:chartSpace>"#,
    );

    #[test]
    fn scans_name_labels_and_values() {
        let data = scan_chart_values(CATEGORY_CHART.as_bytes()).expect("스캔");
        assert_eq!(data.series.len(), 1);
        let s = &data.series[0];
        assert_eq!(s.name.as_deref(), Some("계열 1"));
        assert_eq!(s.axis, SeriesAxis::Category);
        assert_eq!(
            s.labels.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            ["항목 1", "항목 2"]
        );
        assert_eq!(
            s.values.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            ["4.3", "2.5"]
        );
        assert_eq!(s.values[1].idx, 1);
    }

    #[test]
    fn spans_slice_back_to_their_own_text() {
        let xml = CATEGORY_CHART.as_bytes();
        let data = scan_chart_values(xml).expect("스캔");
        for p in data.series[0].values.iter().chain(&data.series[0].labels) {
            assert_eq!(&xml[p.span.clone()], p.text.as_bytes());
        }
    }

    #[test]
    fn reference_formulas_are_not_mistaken_for_values() {
        let data = scan_chart_values(CATEGORY_CHART.as_bytes()).expect("스캔");
        let texts: Vec<&str> = data.series[0]
            .values
            .iter()
            .chain(&data.series[0].labels)
            .map(|p| p.text.as_str())
            .collect();
        assert!(
            !texts.iter().any(|t| t.contains("Sheet1!")),
            "c:f 가 값으로 잡혔다: {texts:?}"
        );
        assert!(!texts.contains(&"General"), "formatCode 가 잡혔다");
    }

    #[test]
    fn scatter_series_use_x_and_y() {
        let xml = concat!(
            r#"<c:chartSpace><c:chart><c:plotArea><c:scatterChart><c:ser>"#,
            r#"<c:xVal><c:numRef><c:numCache><c:ptCount val="2"/>"#,
            r#"<c:pt idx="0"><c:v>0.7</c:v></c:pt><c:pt idx="1"><c:v>1.8</c:v></c:pt>"#,
            r#"</c:numCache></c:numRef></c:xVal>"#,
            r#"<c:yVal><c:numRef><c:numCache><c:ptCount val="2"/>"#,
            r#"<c:pt idx="0"><c:v>2.7</c:v></c:pt><c:pt idx="1"><c:v>3.2</c:v></c:pt>"#,
            r#"</c:numCache></c:numRef></c:yVal>"#,
            r#"</c:ser></c:scatterChart></c:plotArea></c:chart></c:chartSpace>"#,
        );
        let data = scan_chart_values(xml.as_bytes()).expect("스캔");
        let s = &data.series[0];
        assert_eq!(s.axis, SeriesAxis::Scatter);
        assert_eq!(
            s.labels.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            ["0.7", "1.8"]
        );
        assert_eq!(
            s.values.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            ["2.7", "3.2"]
        );
    }

    #[test]
    fn literal_values_take_the_same_path_as_cached_ones() {
        let xml = concat!(
            r#"<c:chartSpace><c:chart><c:plotArea><c:barChart><c:ser>"#,
            r#"<c:cat><c:strLit><c:ptCount val="1"/>"#,
            r#"<c:pt idx="0"><c:v>항목 1</c:v></c:pt></c:strLit></c:cat>"#,
            r#"<c:val><c:numLit><c:ptCount val="1"/>"#,
            r#"<c:pt idx="0"><c:v>7</c:v></c:pt></c:numLit></c:val>"#,
            r#"</c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"#,
        );
        let data = scan_chart_values(xml.as_bytes()).expect("스캔");
        assert_eq!(data.series[0].values[0].text, "7");
        assert_eq!(data.series[0].labels[0].text, "항목 1");
    }

    #[test]
    fn prefix_is_not_hardcoded() {
        let xml = CATEGORY_CHART.replace("c:", "chart:");
        let data = scan_chart_values(xml.as_bytes()).expect("접두어가 달라도 스캔된다");
        assert_eq!(data.series[0].values.len(), 2);
    }

    #[test]
    fn empty_value_element_is_refused() {
        let xml = concat!(
            r#"<c:chartSpace><c:chart><c:plotArea><c:barChart><c:ser>"#,
            r#"<c:val><c:numLit><c:ptCount val="1"/><c:pt idx="0"><c:v/></c:pt>"#,
            r#"</c:numLit></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"#,
        );
        assert_eq!(
            scan_chart_values(xml.as_bytes()),
            Err(ChartScanError::EmptyValueElement { series: 0 })
        );
    }

    #[test]
    fn chart_without_series_is_refused() {
        let xml = r#"<c:chartSpace><c:chart><c:plotArea></c:plotArea></c:chart></c:chartSpace>"#;
        assert_eq!(
            scan_chart_values(xml.as_bytes()),
            Err(ChartScanError::NoSeries)
        );
    }

    #[test]
    fn non_utf8_input_is_refused() {
        assert_eq!(
            scan_chart_values(&[0xff, 0xfe, 0x00]),
            Err(ChartScanError::NotUtf8)
        );
    }
}
