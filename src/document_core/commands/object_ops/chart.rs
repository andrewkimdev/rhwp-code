//! [#4100] 차트 숫자 데이터 native 명령 (B1 엔진축).
//!
//! 주소 지정은 기존 개체 API 와 동형이다 — `(section_idx, parent_para_idx, control_idx)`
//! 3인자(`picture.rs` 선례). **주소·타입 오류만 `Err`** 이고, 데이터 문제는 `Ok` + 부정
//! 봉투(`{"ok":false,"invalid":[…]}`)로 돌려준다. 검증기를 코어와 CLI 두 곳으로 가르지
//! 않기 위해서다 — CLI 는 이 봉투를 그대로 실어 나른다.

use crate::document_core::queries::chart_extract::{
    chart_xml, collect_charts, ChartRef, ChartSource,
};
use crate::document_core::DocumentCore;
use crate::error::HwpError;
use crate::ooxml_chart::data::{scan_chart_values, ChartData, SeriesAxis};

/// 부정 봉투 한 건. CLI `invalid[]` 와 같은 모양(`reason` + `message`)이다.
fn invalid(reason: &str, message: String) -> serde_json::Value {
    serde_json::json!({ "reason": reason, "message": message })
}

fn refused(reason: &str, message: String) -> String {
    serde_json::json!({ "ok": false, "invalid": [invalid(reason, message)] }).to_string()
}

/// 계열의 라벨(카테고리 또는 분산형 X)이 전 계열에서 같은가.
///
/// OOXML 은 계열마다 다른 라벨/ X 를 허용하지만 CSV 는 한 열로 표현한다. 코퍼스는 전건
/// 일치하지만 **포맷의 보장이 아니라서** 표지를 실어 CSV 층이 거부할 수 있게 한다.
fn labels_shared(data: &ChartData) -> bool {
    let Some(first) = data.series.first() else {
        return true;
    };
    let head: Vec<&str> = first.labels.iter().map(|p| p.text.as_str()).collect();
    data.series
        .iter()
        .all(|s| s.labels.iter().map(|p| p.text.as_str()).eq(head.iter().copied()))
}

fn chart_data_json(chart: &ChartRef, data: &ChartData, source: ChartSource) -> serde_json::Value {
    let axis = match data.series.first().map(|s| s.axis) {
        Some(SeriesAxis::Scatter) => "scatter",
        _ => "category",
    };
    let labels: Vec<&str> = data
        .series
        .first()
        .map(|s| s.labels.iter().map(|p| p.text.as_str()).collect())
        .unwrap_or_default();

    serde_json::json!({
        "ok": true,
        "chart": chart.index + 1,
        "axis": axis,
        "source": match source {
            ChartSource::ZipPart => "zipPart",
            ChartSource::NestedCopy => "nestedCopy",
        },
        "representations": {
            "zipPart": chart.zip_part.is_some(),
            "nestedCopy": chart.nested_copy.is_some(),
        },
        "labelsShared": labels_shared(data),
        "labelsMultiLevel": data.series.iter().any(|s| s.labels_multi_level),
        "labels": labels,
        "series": data
            .series
            .iter()
            .map(|s| serde_json::json!({
                "name": s.name,
                "values": s.values.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
    })
}

impl DocumentCore {
    /// 주소가 가리키는 **본문 직속** 차트를 찾는다.
    ///
    /// 컨테이너(글상자·머리말·표 셀) 안의 차트는 이 3인자 주소로 표현할 수 없다 —
    /// 그쪽은 문서 순번(`collect_charts` 의 `index`)으로 지목한다. 그림 API 와 같은
    /// 한계이며, 편집 자체는 슬롯 바이트만 건드리므로 순번 경로로는 문제없이 된다.
    fn resolve_chart_ref(
        &self,
        section_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
    ) -> Result<ChartRef, HwpError> {
        let section = self.document.sections.get(section_idx).ok_or_else(|| {
            HwpError::RenderError(format!("구역 인덱스 {} 범위 초과", section_idx))
        })?;
        let para = section.paragraphs.get(parent_para_idx).ok_or_else(|| {
            HwpError::RenderError(format!("문단 인덱스 {} 범위 초과", parent_para_idx))
        })?;
        if control_idx >= para.controls.len() {
            return Err(HwpError::RenderError(format!(
                "컨트롤 인덱스 {} 범위 초과",
                control_idx
            )));
        }

        collect_charts(&self.document)
            .into_iter()
            .find(|c| {
                c.is_top_level()
                    && c.section == section_idx
                    && c.paragraph == parent_para_idx
                    && c.control == control_idx
            })
            .ok_or_else(|| HwpError::RenderError("지정된 컨트롤이 차트가 아닙니다".to_string()))
    }

    /// 차트의 숫자 데이터를 JSON 으로 읽는다.
    ///
    /// 값은 **원본 텍스트 그대로** 싣는다(`"4.3"`) — 실수로 파싱했다가 되쓰면 표기가
    /// 달라져 무편집 왕복의 바이트 동일이 깨진다.
    pub fn get_chart_data_native(
        &self,
        section_idx: usize,
        parent_para_idx: usize,
        control_idx: usize,
    ) -> Result<String, HwpError> {
        let chart = self.resolve_chart_ref(section_idx, parent_para_idx, control_idx)?;
        Ok(self.chart_data_at(&chart))
    }

    /// 문서 순번(0-based)으로 차트 데이터를 읽는다 — CLI `--chart N` 의 뒷면.
    pub fn get_chart_data_by_index_native(&self, index: usize) -> Result<String, HwpError> {
        let charts = collect_charts(&self.document);
        let chart = charts.get(index).ok_or_else(|| {
            HwpError::RenderError(format!(
                "차트 순번 {} 범위 초과 (차트 {}개)",
                index + 1,
                charts.len()
            ))
        })?;
        Ok(self.chart_data_at(chart))
    }

    fn chart_data_at(&self, chart: &ChartRef) -> String {
        let Some((xml, source)) = chart_xml(&self.document, chart) else {
            return refused(
                "chartStreamMissing",
                "차트 XML 을 읽을 수 없습니다 — 두 표현 모두 비어 있습니다.".to_string(),
            );
        };
        match scan_chart_values(&xml) {
            Ok(data) => chart_data_json(chart, &data, source).to_string(),
            Err(e) => refused("chartScan", e.to_string()),
        }
    }
}
