//! param_frame — table_layout.rs 에서 무변동 이동
use super::*;

/// `parse_field_parameters` 트리 빌더의 스택 프레임 — 열린 파라미터 요소 하나.
/// `listParam`/루트 `parameters` 는 `List`, 나머지 4종은 스칼라 텍스트를 누적한다.
pub(crate) enum ParamFrame {
    List {
        name: Option<String>,
        items: Vec<Parameter>,
    },
    Boolean {
        name: Option<String>,
        text: String,
    },
    Integer {
        name: Option<String>,
        text: String,
    },
    Float {
        name: Option<String>,
        text: String,
    },
    String {
        name: Option<String>,
        text: String,
        preserve_space: bool,
    },
}


#[derive(Default)]
pub(crate) struct HwpxSubListLayout {
    pub(crate) paragraphs: Vec<Paragraph>,
    pub(crate) list_attr: u32,
    pub(crate) text_width: u32,
    pub(crate) text_height: u32,
    pub(crate) text_ref: u8,
    pub(crate) num_ref: u8,
}

impl ParamFrame {
    pub(crate) fn push_text(&mut self, s: &str) {
        match self {
            ParamFrame::Boolean { text, .. }
            | ParamFrame::Integer { text, .. }
            | ParamFrame::Float { text, .. }
            | ParamFrame::String { text, .. } => text.push_str(s),
            ParamFrame::List { .. } => {}
        }
    }

    /// 프레임을 닫아 `Parameter` 로 만든다. 루트 프레임(List)은 호출부가 별도로
    /// `ParameterList` 로 직접 소비하므로 이 경로를 타지 않는다.
    pub(crate) fn finish(self) -> Parameter {
        match self {
            ParamFrame::List { name, items } => Parameter::List(ParameterList { name, items }),
            ParamFrame::Boolean { name, text } => Parameter::Boolean {
                name,
                value: matches!(text.trim(), "1" | "true"),
            },
            ParamFrame::Integer { name, text } => Parameter::Integer {
                name,
                value: text.trim().parse::<i64>().unwrap_or(0),
            },
            ParamFrame::Float { name, text } => Parameter::Float {
                name,
                value: text.trim().parse::<f32>().unwrap_or(0.0),
            },
            ParamFrame::String {
                name,
                text,
                preserve_space,
            } => Parameter::String {
                name,
                value: text,
                preserve_space,
            },
        }
    }
}
