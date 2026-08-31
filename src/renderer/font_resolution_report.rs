//! 폰트 해석 결과 경고 — "pixel-perfect" 목표 대비 이탈을 감지하고 사용자에게 고지한다.
//!
//! `document_core::validation`(비표준 lineseg 감지)과 같은 설계 원칙을 폰트에
//! 적용한다: 비표준/불확실한 상태를 **감지하고 고지**하되, 렌더링을 막거나
//! rhwp 가 스스로 문서 내용에 경고를 새기지는 않는다(원본 무훼손). 서버
//! 배포처럼 데스크톱 편의(추가 폰트 설치, `--font-path`)가 없는 환경에서는
//! 폰트 대체가 "일부 사용자만 겪는 예외"가 아니라 "항상 일어나는 기본값"이므로,
//! 조용히 넘어가지 않고 구조화된 형태로 드러낸다.
//!
//! 두 축을 독립적으로 추적한다:
//! - **메트릭**(레이아웃 정확도): 선언된 폰트의 실측 글자 폭 데이터
//!   (`font_metrics_data`)가 있는가? 없으면 줄바꿈·셀 넘침 등 구조적 배치
//!   자체가 어긋날 위험이 있다 — 단순히 "다르게 보이는" 문제가 아니다.
//! - **글리프**(시각적 정확도): 실제로 어떤 typeface 로 글자를 그렸는가?
//!   메트릭이 정확해도(레이아웃은 안전해도) 그려지는 모양은 다를 수 있다.
//!
//! 문자 단위로 기록하면 신호가 아니라 소음이 되므로, `(폰트명, bold, italic)`
//! 조합별로 집계한다.

use std::collections::HashMap;

use super::font_metrics_data;

/// 레이아웃이 사용한 글자 폭 메트릭이 선언된 폰트와 얼마나 정확히 일치하는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsTier {
    /// 선언된 폰트(또는 표기 정규화 별칭)의 실측 메트릭을 그대로 사용.
    Exact,
    /// 코드에 알려진 목록에 따라 다른 폰트의 메트릭으로 의도적으로 근사.
    Approximated,
    /// 메트릭 DB에 해당 폰트 항목이 전혀 없음 — 레이아웃(줄바꿈·셀 넘침 등)이
    /// 부정확할 위험이 있다.
    Unknown,
}

/// 실제로 어떤 typeface 로 글리프를 그렸는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphTier {
    /// 선언된 폰트와 이름이 일치하는 실제 typeface 로 그림.
    Exact,
    /// 코드에 내장된 의도적 후보(한글 폴백 목록 등)로 그림 — 시각적으로
    /// 가깝도록 고른 대체이나 원본은 아니다.
    DesignedSubstitute,
    /// 요청과 무관한 최후 수단 typeface 로 그림 — glyph 는 있으나 모양은 임의.
    GenericFallback,
    /// 어떤 typeface 에도 해당 glyph 가 없어 그리지 못함(두부/공백) — 실질적
    /// 내용 손실.
    Missing,
}

/// 렌더링 백엔드가 typeface 후보 목록에서 실제로 고른 후보의 출처.
/// `GlyphTier` 판정의 입력이다 — 어떤 후보군에서 골랐는지가 판정을 정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphMatchSource {
    /// 문서가 선언한 이름(또는 그 접미사 제거형) 자체로 찾음.
    Requested,
    /// 코드가 미리 정해 둔 대체 후보 목록(한글 고딕/명조 폴백 등)에서 찾음.
    CuratedFallback,
    /// 그 외 모든 후보가 실패한 뒤의 최후 수단.
    Legacy,
}

/// `(폰트명, bold, italic)` — 경고 집계 키.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontKey {
    family: String,
    bold: bool,
    italic: bool,
}

/// 폰트 해석 경고 한 건 — 특정 `(폰트명, bold, italic)` 조합에 대한 집계.
#[derive(Debug, Clone)]
pub struct FontResolutionWarning {
    pub requested_family: String,
    pub bold: bool,
    pub italic: bool,
    pub metrics_tier: MetricsTier,
    /// 메트릭은 있었지만 bold 요청에 Regular 메트릭을 대신 썼는가(약한 편차 —
    /// `Approximated`/`Unknown` 만큼 심각하지 않아 별도 플래그로 둔다).
    pub metrics_bold_fallback: bool,
    pub glyph_tier: GlyphTier,
    /// 실제로 그리는 데 쓰인 typeface 이름(알 수 없거나 `Missing` 이면 `None`).
    pub resolved_family: Option<String>,
    /// 이 조합이 적용된 글자 수 누적(첫 표본이 아니라 전체 집계).
    pub affected_chars: usize,
    /// 최초 발견된 텍스트의 짧은 표본 — 로그에서 위치를 가늠하는 용도.
    pub sample_text: String,
}

impl FontResolutionWarning {
    /// 두 축 모두 완전히 정확한 경우는 "경고"가 아니다.
    pub fn is_notable(&self) -> bool {
        !matches!(self.metrics_tier, MetricsTier::Exact)
            || !matches!(self.glyph_tier, GlyphTier::Exact)
    }
}

/// 문서(또는 렌더 1회) 단위로 누적하는 폰트 해석 경고 모음.
#[derive(Debug, Clone, Default)]
pub struct FontResolutionReport {
    entries: HashMap<FontKey, FontResolutionWarning>,
}

impl FontResolutionReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// 눈에 띄는(둘 중 하나라도 `Exact` 가 아닌) 경고가 하나도 없는가.
    pub fn is_empty(&self) -> bool {
        !self.entries.values().any(FontResolutionWarning::is_notable)
    }

    /// 눈에 띄는 경고만 반환한다(순서 비보장 — 호출측이 필요하면 정렬한다).
    pub fn notable_warnings(&self) -> Vec<&FontResolutionWarning> {
        self.entries.values().filter(|w| w.is_notable()).collect()
    }

    /// 다른 리포트를 흡수한다 — 예: 여러 페이지 리포트를 문서 리포트 하나로 합칠 때.
    /// 같은 키가 있으면 글자 수만 더한다(등급은 같은 입력에 대해 결정적이므로
    /// 재계산하지 않는다).
    pub fn merge(&mut self, other: FontResolutionReport) {
        for (key, warning) in other.entries {
            self.entries
                .entry(key)
                .and_modify(|w| w.affected_chars += warning.affected_chars)
                .or_insert(warning);
        }
    }

    /// 텍스트 런(또는 그 일부) 렌더링 시점에 메트릭·글리프 두 축을 함께 기록한다.
    ///
    /// `requested_family`: 문서가 선언한 폰트명(정규화 전, 원본 그대로) —
    /// 이후 경고를 사람이 읽을 때 문서 XML 과 대조할 수 있어야 하므로 별칭
    /// 정규화는 이 함수 내부(`classify_metrics`)에서만 한다.
    pub fn record(
        &mut self,
        requested_family: &str,
        bold: bool,
        italic: bool,
        glyph_tier: GlyphTier,
        resolved_family: Option<&str>,
        sample_text: &str,
        char_count: usize,
    ) {
        if requested_family.trim().is_empty() || char_count == 0 {
            return;
        }
        let (metrics_tier, metrics_bold_fallback) =
            classify_metrics(requested_family, bold, italic);
        let key = FontKey {
            family: requested_family.to_string(),
            bold,
            italic,
        };
        self.entries
            .entry(key)
            .and_modify(|w| w.affected_chars += char_count)
            .or_insert_with(|| FontResolutionWarning {
                requested_family: requested_family.to_string(),
                bold,
                italic,
                metrics_tier,
                metrics_bold_fallback,
                glyph_tier,
                resolved_family: resolved_family.map(str::to_string),
                affected_chars: char_count,
                sample_text: sample_text.chars().take(24).collect(),
            });
    }
}

/// 코드에 알려진, "다른 폰트로의 의도적 근사"인 메트릭 별칭 원본명.
///
/// `font_metrics_data::resolve_metric_alias` 의 나머지 매핑은 전부 같은 폰트의
/// 표기 정규화(한국어 UI명 → 영문 DB 키, 예: "굴림체"→"GulimChe")이고 실측
/// 메트릭 자체는 동일 폰트에서 나온다. 이 목록에 있는 원본명만 실제로 **다른**
/// 폰트의 메트릭을 빌려 쓴다 — 해당 함수의 "근사" 주석을 그대로 옮긴 것이므로,
/// 그쪽에 새 근사 항목이 생기면 이 목록도 함께 갱신해야 한다.
const APPROXIMATED_METRIC_SOURCES: &[&str] = &[
    "HY각헤드라인M",
    "본한글",
    "본한글vf",
    "본한글 Medium",
    "본한글M",
    "본고딕",
    "본고딕vf",
    "Source Han Sans",
    "Source Han Sans K",
    "Source Han Sans KR",
    "SourceHanSans",
    "SourceHanSansKR",
    "SourceHanSansK",
    "Noto Sans CJK KR",
    "본명조",
    "본명조vf",
    "본명조M",
    "Source Han Serif",
    "Source Han Serif K",
    "Source Han Serif KR",
    "SourceHanSerif",
    "SourceHanSerifKR",
    "SourceHanSerifK",
    "Noto Serif CJK KR",
];

/// 선언된 폰트의 레이아웃 메트릭 정확도를 판정한다.
///
/// `font_family`는 CSS 스타일 콤마 목록일 수 있으므로(SVG 경로의 fallback
/// chain과 동일한 관례) 첫 항목만 본다 — 실측 메트릭은 문서가 실제로
/// 선언한 폰트 하나에 대한 것이지, 폴백 체인 전체에 대한 것이 아니다.
pub fn classify_metrics(font_family: &str, bold: bool, italic: bool) -> (MetricsTier, bool) {
    let primary_name = font_family.split(',').next().unwrap_or(font_family).trim();
    if primary_name.is_empty() {
        return (MetricsTier::Unknown, false);
    }
    match font_metrics_data::find_metric(primary_name, bold, italic) {
        None => (MetricsTier::Unknown, false),
        Some(m) => {
            let tier = if APPROXIMATED_METRIC_SOURCES.contains(&primary_name) {
                MetricsTier::Approximated
            } else {
                MetricsTier::Exact
            };
            (tier, m.bold_fallback)
        }
    }
}

/// 실제로 고른 typeface 의 출처와 이름으로 `GlyphTier` 를 판정한다.
///
/// `requested`/`resolved_family` 비교는 대소문자를 무시한다 — OS 폰트
/// 매니저가 요청과 같은 폰트를 다른 대소문자로 돌려주는 경우가 있고
/// (예: 파일시스템이 대소문자를 구분하지 않는 macOS), 그 자체는 대체가
/// 아니다. 반대로 `source == Requested` 인데도 이름이 다르면 — 요청한
/// family 로 찾았지만 폰트 매니저가 자체적으로 다른 폰트를 조용히
/// 대신 준 것이므로 `GenericFallback` 으로 본다(요청이 성공한 것으로
/// 오판하지 않는다).
pub fn classify_glyph_source(
    requested: &str,
    resolved_family: &str,
    source: GlyphMatchSource,
) -> GlyphTier {
    match source {
        GlyphMatchSource::Requested if requested.eq_ignore_ascii_case(resolved_family) => {
            GlyphTier::Exact
        }
        GlyphMatchSource::Requested => GlyphTier::GenericFallback,
        GlyphMatchSource::CuratedFallback => GlyphTier::DesignedSubstitute,
        GlyphMatchSource::Legacy => GlyphTier::GenericFallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_metrics_exact_for_known_font() {
        let (tier, bold_fallback) = classify_metrics("GulimChe", false, false);
        assert_eq!(tier, MetricsTier::Exact);
        assert!(!bold_fallback);
    }

    #[test]
    fn classify_metrics_exact_for_korean_alias_of_known_font() {
        // "굴림체"(한국어 표기)는 "GulimChe"(영문 DB 키)의 표기 정규화일 뿐,
        // 다른 폰트로의 근사가 아니다 — Exact 여야 한다.
        let (tier, _) = classify_metrics("굴림체", false, false);
        assert_eq!(tier, MetricsTier::Exact);
    }

    #[test]
    fn classify_metrics_approximated_for_known_cross_font_substitute() {
        let (tier, _) = classify_metrics("본고딕", false, false);
        assert_eq!(tier, MetricsTier::Approximated);
    }

    #[test]
    fn classify_metrics_unknown_for_unregistered_font() {
        let (tier, bold_fallback) = classify_metrics("이런폰트는존재하지않는다XYZ", false, false);
        assert_eq!(tier, MetricsTier::Unknown);
        assert!(!bold_fallback);
    }

    #[test]
    fn classify_metrics_unknown_for_empty_family() {
        let (tier, _) = classify_metrics("", false, false);
        assert_eq!(tier, MetricsTier::Unknown);
    }

    #[test]
    fn classify_metrics_reads_first_family_in_css_style_list() {
        let (tier, _) = classify_metrics(" GulimChe , fallback ", false, false);
        assert_eq!(tier, MetricsTier::Exact);
    }

    #[test]
    fn classify_glyph_source_requested_match_is_exact() {
        assert_eq!(
            classify_glyph_source("GulimChe", "GulimChe", GlyphMatchSource::Requested),
            GlyphTier::Exact
        );
        // 대소문자만 다른 경우도 Exact.
        assert_eq!(
            classify_glyph_source("gulimche", "GulimChe", GlyphMatchSource::Requested),
            GlyphTier::Exact
        );
    }

    #[test]
    fn classify_glyph_source_requested_mismatch_is_generic_fallback() {
        // 요청한 family 로 찾았다고 표시됐는데 실제 typeface 이름이 다르면,
        // OS 가 조용히 다른 폰트를 준 것 — 요청이 성공한 게 아니다.
        assert_eq!(
            classify_glyph_source("GulimChe", "Noto Sans KR", GlyphMatchSource::Requested),
            GlyphTier::GenericFallback
        );
    }

    #[test]
    fn classify_glyph_source_curated_fallback_is_designed_substitute() {
        assert_eq!(
            classify_glyph_source(
                "GulimChe",
                "Noto Sans KR",
                GlyphMatchSource::CuratedFallback
            ),
            GlyphTier::DesignedSubstitute
        );
    }

    #[test]
    fn classify_glyph_source_legacy_is_generic_fallback() {
        assert_eq!(
            classify_glyph_source("GulimChe", "Arial", GlyphMatchSource::Legacy),
            GlyphTier::GenericFallback
        );
    }

    #[test]
    fn report_ignores_exact_exact_entries() {
        let mut report = FontResolutionReport::new();
        report.record(
            "GulimChe",
            false,
            false,
            GlyphTier::Exact,
            Some("GulimChe"),
            "sample",
            10,
        );
        assert!(report.is_empty());
        assert!(report.notable_warnings().is_empty());
    }

    #[test]
    fn report_aggregates_char_counts_across_calls_with_same_key() {
        let mut report = FontResolutionReport::new();
        report.record(
            "GulimChe",
            false,
            false,
            GlyphTier::GenericFallback,
            Some("Noto Sans KR"),
            "first run",
            5,
        );
        report.record(
            "GulimChe",
            false,
            false,
            GlyphTier::GenericFallback,
            Some("Noto Sans KR"),
            "second run",
            7,
        );
        let warnings = report.notable_warnings();
        assert_eq!(warnings.len(), 1);
        let w = warnings[0];
        assert_eq!(w.affected_chars, 12);
        assert_eq!(w.sample_text, "first run"); // 최초 표본을 유지한다
    }

    #[test]
    fn report_keeps_distinct_keys_for_different_bold_italic() {
        let mut report = FontResolutionReport::new();
        report.record(
            "GulimChe",
            false,
            false,
            GlyphTier::GenericFallback,
            Some("Noto Sans KR"),
            "regular",
            3,
        );
        report.record(
            "GulimChe",
            true,
            false,
            GlyphTier::GenericFallback,
            Some("Noto Sans KR"),
            "bold",
            4,
        );
        assert_eq!(report.notable_warnings().len(), 2);
    }

    #[test]
    fn report_merge_combines_char_counts() {
        let mut a = FontResolutionReport::new();
        a.record(
            "GulimChe",
            false,
            false,
            GlyphTier::GenericFallback,
            Some("Noto Sans KR"),
            "page1",
            5,
        );
        let mut b = FontResolutionReport::new();
        b.record(
            "GulimChe",
            false,
            false,
            GlyphTier::GenericFallback,
            Some("Noto Sans KR"),
            "page2",
            9,
        );
        a.merge(b);
        let warnings = a.notable_warnings();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].affected_chars, 14);
    }

    #[test]
    fn record_ignores_zero_char_count() {
        let mut report = FontResolutionReport::new();
        report.record(
            "GulimChe",
            false,
            false,
            GlyphTier::Missing,
            None,
            "unused",
            0,
        );
        assert!(report.is_empty());
    }
}
