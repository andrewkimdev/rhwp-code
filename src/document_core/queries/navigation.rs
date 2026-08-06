//! 개요 번호 전용 탐색 메타데이터.
//!
//! 텍스트 접두어(`1.`, `1.1` 등)를 추측하지 않는다. 문단 모양의
//! [`HeadType::Outline`]와 번호 정의만 사용하므로, 일반 본문의 숫자 문자열은
//! 탐색 목록에 포함되지 않는다.

use std::collections::HashMap;

use serde::Serialize;

use crate::document_core::queries::rendering::paragraph_text_with_equations;
use crate::document_core::DocumentCore;
use crate::error::HwpError;
use crate::model::document::Document;
use crate::model::style::HeadType;
use crate::renderer::layout::{
    default_outline_numbering, expand_numbering_format, resolve_numbering_id, NumberingState,
};

/// 개요 번호 탐색 결과.
#[derive(Debug, Clone, Serialize)]
pub struct OutlineNavigation {
    pub outline: Vec<OutlineNavigationItem>,
}

/// 개요 번호 문단 하나의 탐색 정보.
#[derive(Debug, Clone, Serialize)]
pub struct OutlineNavigationItem {
    /// 개요 수준(1부터 시작).
    pub level: u8,
    /// 렌더러에 표시되는 실제 개요 번호.
    pub number: String,
    /// 개요 문단의 제목 텍스트.
    pub title: String,
    /// 사용자에게 표시하는 쪽 번호(1부터 시작). 조판 위치가 없으면 0이다.
    pub page: u32,
    /// 내부 이동용 구역 인덱스(0부터 시작).
    pub section: usize,
    /// 내부 이동용 문단 인덱스(0부터 시작).
    pub paragraph: usize,
}

/// 문서 순서대로 개요 번호를 계산한다.
///
/// 번호 카운터와 서식 확장은 렌더러와 같은 구현을 쓴다. `Number` 문단도
/// 카운터에는 반영하지만 결과에는 `Outline` 문단만 넣어, 번호 체계가 섞인
/// 문서에서도 화면에 그려진 개요 번호와 일치시킨다.
fn build_outline_navigation(
    document: &Document,
    paragraph_pages: &HashMap<(usize, usize), u32>,
) -> OutlineNavigation {
    let mut numbering_state = NumberingState::default();
    let mut outline = Vec::new();

    for (section_index, section) in document.sections.iter().enumerate() {
        let outline_numbering_id = section.section_def.outline_numbering_id;

        for (paragraph_index, paragraph) in section.paragraphs.iter().enumerate() {
            let Some(para_shape) = document
                .doc_info
                .para_shapes
                .get(paragraph.para_shape_id as usize)
            else {
                continue;
            };

            if !matches!(para_shape.head_type, HeadType::Outline | HeadType::Number) {
                continue;
            }

            let numbering_id = resolve_numbering_id(
                para_shape.head_type,
                para_shape.numbering_id,
                outline_numbering_id,
            );
            let synthesized_default;
            let numbering = match numbering_id
                .checked_sub(1)
                .and_then(|index| document.doc_info.numberings.get(index as usize))
            {
                Some(numbering) => numbering,
                // 렌더러와 동일하게 정의 없는 Outline은 한컴 기본 모양으로 처리한다.
                None if para_shape.head_type == HeadType::Outline => {
                    synthesized_default = default_outline_numbering();
                    &synthesized_default
                }
                // 정의 없는 Number 문단은 렌더러가 번호를 표시하지 않는다.
                None => continue,
            };

            let counters = numbering_state.advance(
                numbering_id,
                para_shape.para_level,
                paragraph.numbering_restart,
            );
            let level_index = (para_shape.para_level as usize).min(6);
            let format = &numbering.level_formats[level_index];
            if format.is_empty() {
                continue;
            }
            let number = expand_numbering_format(
                format,
                &counters,
                numbering,
                &numbering.level_start_numbers,
                level_index,
            );
            if number.is_empty() || para_shape.head_type != HeadType::Outline {
                continue;
            }

            outline.push(OutlineNavigationItem {
                level: level_index as u8 + 1,
                number,
                title: paragraph_text_with_equations(paragraph).trim().to_owned(),
                page: paragraph_pages
                    .get(&(section_index, paragraph_index))
                    .copied()
                    .map_or(0, |page| page + 1),
                section: section_index,
                paragraph: paragraph_index,
            });
        }
    }

    OutlineNavigation { outline }
}

impl DocumentCore {
    /// 개요 번호 탐색 정보를 JSON으로 반환한다.
    ///
    /// 이 질의는 `HeadType::Outline`만 표시하며 일반 문단의 번호 문자열을 분석하지 않는다.
    pub fn get_outline_navigation_native(&self) -> Result<String, HwpError> {
        let paragraph_pages = self.build_paragraph_page_index();
        let navigation = build_outline_navigation(&self.document, &paragraph_pages);
        serde_json::to_string(&navigation).map_err(|error| {
            HwpError::RenderError(format!("개요 탐색 JSON 직렬화에 실패했습니다: {error}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::model::document::Section;
    use crate::model::paragraph::Paragraph;
    use crate::model::style::{Numbering, ParaShape};

    fn outline_shape(level: u8) -> ParaShape {
        ParaShape {
            head_type: HeadType::Outline,
            para_level: level,
            numbering_id: 0,
            ..ParaShape::default()
        }
    }

    fn paragraph(text: &str, para_shape_id: u16) -> Paragraph {
        Paragraph {
            text: text.to_owned(),
            para_shape_id,
            ..Paragraph::default()
        }
    }

    #[test]
    fn uses_outline_metadata_and_not_text_prefixes() {
        let mut numbering = Numbering::default();
        for format in &mut numbering.level_formats {
            *format = "^N".to_owned();
        }
        numbering.level_start_numbers = [1; 7];

        let mut document = Document::default();
        document.doc_info.para_shapes =
            vec![outline_shape(0), outline_shape(1), ParaShape::default()];
        document.doc_info.numberings.push(numbering);
        document.sections.push(Section {
            section_def: crate::model::document::SectionDef {
                outline_numbering_id: 1,
                ..crate::model::document::SectionDef::default()
            },
            paragraphs: vec![
                paragraph("개요", 0),
                paragraph("목적", 1),
                // 숫자로 시작해도 Outline 속성이 아니면 목록에 넣지 않는다.
                paragraph("1. 일반 본문", 2),
                paragraph("요구사항", 0),
            ],
            ..Section::default()
        });
        let paragraph_pages = HashMap::from([((0, 0), 0), ((0, 1), 1), ((0, 2), 2), ((0, 3), 3)]);

        let navigation = build_outline_navigation(&document, &paragraph_pages);

        assert_eq!(navigation.outline.len(), 3);
        assert_eq!(navigation.outline[0].number, "1.");
        assert_eq!(navigation.outline[0].level, 1);
        assert_eq!(navigation.outline[0].page, 1);
        assert_eq!(navigation.outline[1].number, "1.1.");
        assert_eq!(navigation.outline[1].level, 2);
        assert_eq!(navigation.outline[1].page, 2);
        assert_eq!(navigation.outline[2].number, "2.");
        assert_eq!(navigation.outline[2].title, "요구사항");
        assert_eq!(navigation.outline[2].page, 4);
    }
}
