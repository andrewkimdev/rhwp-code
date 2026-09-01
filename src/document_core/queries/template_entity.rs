//! hwpx-template-engine의 엔티티 코드젠 질의 — 템플릿 마커 스키마 → Java record 초안.
//!
//! hwpx-template-engine(Java)의 세 도구를 하나의 읽기 전용 질의로 옮긴다:
//!
//! - `TableRoleMarker` — 표 첫 행 첫 셀 평문 텍스트로 authoring된 역할 마커
//!   (`#REPEAT-TITLE:`/`#REPEAT-HEADER:`/`#REPEAT-BODY:`/`#REPEAT-FOOTER:` + `-NESTED:` 계열,
//!   `#PAGENO`)의 발견·검증.
//! - `FieldSchemaExtractor` — 누름틀(`hp:fieldBegin@name`)을 문서 순서로 스캔해 최상위 필드 /
//!   반복 블록 항목 필드 / 계산 필드(`#seq:`·`#sum:`·페이지 필드)로 분류.
//! - `TemplateEntityGenerator` — 스키마에서 Java record 데이터 클래스 + 모듈 클래스 초안을
//!   문자열로 만든다.
//!
//! 검증 에러 메시지와 방출 텍스트는 Java 구현과 바이트 단위로 같다 —
//! `tests/template_entity_contract.rs`가 Java 생성기가 낸 golden(`tests/fixtures/template-entity/`)과
//! 비교해 패리티를 고정한다. Java 쪽은 section XML만 보므로 이 질의도 본문 문단(표 셀·머리말/꼬리말
//! 컨트롤 내부 포함)만 걷고, 가상 셀 필드(`tc@name`)는 세지 않는다.
//!
//! Java의 DOM 순회와 IR 순회의 대응: Java `descendants(doc, "hp:tbl")`은 중첩 표 포함 문서
//! 순서(전위 순회)라서 부모 표가 자식 표보다 앞이다. 필드 분류는 *가장 가까운* 조상 표 하나만
//! 본다(`OwpmlDom.ancestor(fb, "hp:tbl")`) — 재귀 걷기에서 현재 표 인덱스 하나만 들고 내려가면
//! 같은 결과가 나온다.

use crate::document_core::DocumentCore;
use crate::model::control::Control;
use crate::model::paragraph::Paragraph;
use crate::model::table::Table;
use serde_json::{json, Value};

const SEQ_FIELD_PREFIX: &str = "#seq:";
const SUM_FIELD_PREFIX: &str = "#sum:";
const CURRENT_PAGE_FIELD: &str = "현재_페이지";
const TOTAL_PAGES_FIELD: &str = "전체_페이지";

const PAGENO: &str = "#PAGENO";
const REPEAT_TITLE_PREFIX: &str = "#REPEAT-TITLE:";
const REPEAT_HEADER_PREFIX: &str = "#REPEAT-HEADER:";
const REPEAT_BODY_PREFIX: &str = "#REPEAT-BODY:";
const REPEAT_FOOTER_PREFIX: &str = "#REPEAT-FOOTER:";
const REPEAT_TITLE_NESTED_PREFIX: &str = "#REPEAT-TITLE-NESTED:";
const REPEAT_HEADER_NESTED_PREFIX: &str = "#REPEAT-HEADER-NESTED:";
const REPEAT_BODY_NESTED_PREFIX: &str = "#REPEAT-BODY-NESTED:";
const REPEAT_FOOTER_NESTED_PREFIX: &str = "#REPEAT-FOOTER-NESTED:";

/// Java 언어 키워드 — `TemplateEntityGenerator.JAVA_KEYWORDS` 와 같은 56항목(본 키워드 50개 +
/// true/false/null/var/record/yield). 누름틀 이름이 식별자로 쓸 수 없는 경우의 안전장치.
const JAVA_KEYWORDS: [&str; 56] = [
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "var",
    "record",
    "yield",
];

fn is_java_keyword(s: &str) -> bool {
    JAVA_KEYWORDS.contains(&s)
}

/// 구역 1개의 걷기 결과 — 평탄화된 표(문서 순서, 중첩 포함)와 필드(문서 순서).
struct SectionScan {
    /// 표별 마커 텍스트(첫 행 첫 셀 텍스트 trim). 마커가 없거나 표가 비었으면 `None`.
    tables: Vec<Option<String>>,
    /// 문서 순서 필드. `table`은 가장 가까운 조상 표의 인덱스(표 밖이면 `None`).
    fields: Vec<FieldRef>,
}

struct FieldRef {
    name: String,
    table: Option<usize>,
}

fn scan_paragraphs(paras: &[Paragraph], current_table: Option<usize>, scan: &mut SectionScan) {
    for para in paras {
        scan_paragraph(para, current_table, scan);
    }
}

fn scan_paragraph(para: &Paragraph, current_table: Option<usize>, scan: &mut SectionScan) {
    for ctrl in &para.controls {
        match ctrl {
            Control::Field(field) => {
                // Java는 `hp:fieldBegin@name`만 본다(빈 이름 건너뜀). HWPX 파서는 @name을
                // 항상 ctrl_data_name에 옮겨 담는다(section.rs parse_field_begin_attrs).
                if let Some(name) = field.ctrl_data_name.as_deref() {
                    if !name.is_empty() {
                        scan.fields.push(FieldRef {
                            name: name.to_string(),
                            table: current_table,
                        });
                    }
                }
            }
            Control::Table(table) => {
                let idx = scan.tables.len();
                scan.tables.push(role_marker_text(table));
                for cell in &table.cells {
                    scan_paragraphs(&cell.paragraphs, Some(idx), scan);
                }
            }
            // 머리말/꼬리말/각주/미주/숨은 설명 안의 표·필드도 Java DOM 순회(descendants)는
            // 본다 — 가장 가까운 조상 표는 그대로 유지한 채 재귀한다. (#PAGENO 표가 승격
            // 전 hp:header 안에 있을 때가 실측 사례다.)
            Control::Header(h) => scan_paragraphs(&h.paragraphs, current_table, scan),
            Control::Footer(f) => scan_paragraphs(&f.paragraphs, current_table, scan),
            Control::Footnote(n) => scan_paragraphs(&n.paragraphs, current_table, scan),
            Control::Endnote(n) => scan_paragraphs(&n.paragraphs, current_table, scan),
            Control::HiddenComment(c) => scan_paragraphs(&c.paragraphs, current_table, scan),
            _ => {}
        }
    }
}

/// 표 첫 행 첫 셀의 평문 텍스트(trim) — `TableRoleMarker.roleMarkerText` 에 대응.
///
/// Java는 DOM 텍스트 노드(`hp:t`)만 이어붙인다. IR 문단 텍스트는 컨트롤 자리표시자
/// (U+0002 등)와 탭(U+0009)·줄바꿈도 섞고 있으므로 C0 제어 문자를 모두 걷어내 맞춘다.
fn role_marker_text(table: &Table) -> Option<String> {
    let first_cell = table.cells.iter().find(|c| c.row == 0)?;
    let mut text = String::new();
    for para in &first_cell.paragraphs {
        text.push_str(&para.text);
    }
    let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 반복 블록을 이루는 표 하나의 역할.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepeatRole {
    Title,
    Header,
    Body,
    Footer,
}

impl RepeatRole {
    fn as_str(self) -> &'static str {
        match self {
            RepeatRole::Title => "TITLE",
            RepeatRole::Header => "HEADER",
            RepeatRole::Body => "BODY",
            RepeatRole::Footer => "FOOTER",
        }
    }
}

fn role_of(marker: Option<&str>) -> Option<RepeatRole> {
    let m = marker?;
    if m.starts_with(REPEAT_TITLE_PREFIX) || m.starts_with(REPEAT_TITLE_NESTED_PREFIX) {
        return Some(RepeatRole::Title);
    }
    if m.starts_with(REPEAT_HEADER_PREFIX) || m.starts_with(REPEAT_HEADER_NESTED_PREFIX) {
        return Some(RepeatRole::Header);
    }
    if m.starts_with(REPEAT_BODY_PREFIX) || m.starts_with(REPEAT_BODY_NESTED_PREFIX) {
        return Some(RepeatRole::Body);
    }
    if m.starts_with(REPEAT_FOOTER_PREFIX) || m.starts_with(REPEAT_FOOTER_NESTED_PREFIX) {
        return Some(RepeatRole::Footer);
    }
    None
}

fn is_nested_marker(marker: Option<&str>) -> bool {
    match marker {
        Some(m) => {
            m.starts_with(REPEAT_TITLE_NESTED_PREFIX)
                || m.starts_with(REPEAT_HEADER_NESTED_PREFIX)
                || m.starts_with(REPEAT_BODY_NESTED_PREFIX)
                || m.starts_with(REPEAT_FOOTER_NESTED_PREFIX)
        }
        None => false,
    }
}

fn prefix_for(role: RepeatRole) -> &'static str {
    match role {
        RepeatRole::Title => REPEAT_TITLE_PREFIX,
        RepeatRole::Header => REPEAT_HEADER_PREFIX,
        RepeatRole::Body => REPEAT_BODY_PREFIX,
        RepeatRole::Footer => REPEAT_FOOTER_PREFIX,
    }
}

fn nested_prefix_for(role: RepeatRole) -> &'static str {
    match role {
        RepeatRole::Title => REPEAT_TITLE_NESTED_PREFIX,
        RepeatRole::Header => REPEAT_HEADER_NESTED_PREFIX,
        RepeatRole::Body => REPEAT_BODY_NESTED_PREFIX,
        RepeatRole::Footer => REPEAT_FOOTER_NESTED_PREFIX,
    }
}

/// 반복 블록 마커 접두사 뒤의 값(일반 마커는 블록명, `-NESTED:`는 '부모/자식').
fn repeat_block_name(marker: &str) -> Result<String, String> {
    let role = role_of(Some(marker))
        .ok_or_else(|| format!("'{marker}'는 반복 블록 마커(#REPEAT-TITLE:/#REPEAT-HEADER:/#REPEAT-BODY:/#REPEAT-FOOTER:, 또는 그 '-NESTED:' 계열)가 아닙니다."))?;
    let prefix = if is_nested_marker(Some(marker)) {
        nested_prefix_for(role)
    } else {
        prefix_for(role)
    };
    Ok(marker[prefix.len()..].to_string())
}

/// `-NESTED:` 마커 값을 (부모블록명, 자식블록명)으로 분해.
fn parse_nested_block_name(marker: &str) -> Result<(String, String), String> {
    if !is_nested_marker(Some(marker)) {
        return Err(format!(
            "'{marker}'는 중첩 반복 블록 마커(-NESTED:)가 아닙니다."
        ));
    }
    let value = repeat_block_name(marker)?;
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!(
            "'{marker}' 마커는 '부모블록명/자식블록명' 형태여야 합니다(예: '{REPEAT_BODY_NESTED_PREFIX}수입물품내역/물품상세내역')."
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// 같은 블록명을 공유하는 반복 블록 표 묶음. `title`/`header`/`footer`는 선택, `body`는 필수.
#[derive(Debug)]
struct RepeatGroup {
    block_name: String,
    title: Option<usize>,
    header: Option<usize>,
    body: usize,
    footer: Option<usize>,
    child: Option<Box<RepeatGroup>>,
}

/// `TableRoleMarker.findRepeatBlockGroup(List)` 포트 — 표 인덱스 목록(`indices`) 안에서
/// 마커 표들을 묶고 검증한다. 연속성은 *목록 안 위치* 기준으로 잰다(호출부가 중첩 마커 표를
/// 걸러 만든 목록이면 그 목록 기준 — Java와 같다).
fn find_group(tables: &[Option<String>], indices: &[usize]) -> Result<Option<RepeatGroup>, String> {
    let mut block_name: Option<String> = None;
    let mut title = None;
    let mut header = None;
    let mut body = None;
    let mut footer = None;
    let (mut title_count, mut header_count, mut body_count, mut footer_count) = (0, 0, 0, 0);
    let mut member_pos: Vec<usize> = Vec::new();
    let mut member_roles: Vec<RepeatRole> = Vec::new();

    for (pos, &i) in indices.iter().enumerate() {
        let marker = tables[i].as_deref();
        let Some(role) = role_of(marker) else {
            continue;
        };
        let name = repeat_block_name(marker.unwrap())?;
        if name.is_empty() {
            let m = marker.unwrap();
            return Err(format!("'{m}' 마커에 블록명이 없습니다."));
        }
        match &block_name {
            None => block_name = Some(name),
            Some(b) if b != &name => {
                return Err(format!(
                    "서로 다른 블록명의 반복 블록 마커가 있습니다('{b}', '{name}') — 문서당 반복 블록은 1개만 지원합니다."
                ));
            }
            _ => {}
        }
        member_pos.push(pos);
        member_roles.push(role);
        match role {
            RepeatRole::Title => {
                title_count += 1;
                title = Some(i);
            }
            RepeatRole::Header => {
                header_count += 1;
                header = Some(i);
            }
            RepeatRole::Body => {
                body_count += 1;
                body = Some(i);
            }
            RepeatRole::Footer => {
                footer_count += 1;
                footer = Some(i);
            }
        }
    }

    let Some(block_name) = block_name else {
        return Ok(None);
    };
    if title_count > 1 {
        return Err(format!(
            "여러 개의 표에 '#REPEAT-TITLE:{block_name}' 마커가 있습니다 — 블록당 1개만 지원합니다."
        ));
    }
    if header_count > 1 {
        return Err(format!(
            "여러 개의 표에 '#REPEAT-HEADER:{block_name}' 마커가 있습니다 — 블록당 1개만 지원합니다."
        ));
    }
    if footer_count > 1 {
        return Err(format!(
            "여러 개의 표에 '#REPEAT-FOOTER:{block_name}' 마커가 있습니다 — 블록당 1개만 지원합니다."
        ));
    }
    if body_count == 0 {
        return Err(format!(
            "'#REPEAT-BODY:{block_name}' 표를 찾지 못했습니다 — 반복 블록에는 반드시 BODY 표가 있어야 합니다."
        ));
    }
    if body_count > 1 {
        return Err(format!(
            "여러 개의 표에 '#REPEAT-BODY:{block_name}' 마커가 있습니다 — 문서당 반복 블록은 1개만 지원합니다."
        ));
    }

    let first = member_pos[0];
    let last = member_pos[member_pos.len() - 1];
    if last - first + 1 != member_pos.len() {
        return Err(format!(
            "'{block_name}' 반복 블록의 표들(#REPEAT-TITLE:/#REPEAT-HEADER:/#REPEAT-BODY:/#REPEAT-FOOTER:)이 서로 떨어져 있습니다 — 다른 표가 사이에 끼면 안 되고 반드시 연속된 표여야 합니다."
        ));
    }

    let mut expected: Vec<&str> = Vec::new();
    if title.is_some() {
        expected.push("TITLE");
    }
    if header.is_some() {
        expected.push("HEADER");
    }
    expected.push("BODY");
    if footer.is_some() {
        expected.push("FOOTER");
    }
    let actual: Vec<&str> = member_roles.iter().map(|r| r.as_str()).collect();
    if actual != expected {
        let actual_str = actual
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "'{block_name}' 반복 블록의 표 순서가 잘못되었습니다 — 있는 표만 골라 #REPEAT-TITLE: → #REPEAT-HEADER: → #REPEAT-BODY: → #REPEAT-FOOTER: 순서여야 합니다. 실제 순서: [{actual_str}]"
        ));
    }

    Ok(Some(RepeatGroup {
        block_name,
        title,
        header,
        body: body.unwrap(),
        footer,
        child: None,
    }))
}

/// `TableRoleMarker.findChildRepeatBlockGroup` 포트 — 부모 BODY 표 바로 뒤에 연속된,
/// 같은 부모를 선언한 `-NESTED:` 표들을 자식 그룹으로 묶는다. 3단계 중첩은 에러.
fn find_child_group(
    tables: &[Option<String>],
    body: usize,
    parent_block_name: &str,
) -> Result<Option<RepeatGroup>, String> {
    let mut run: Vec<usize> = Vec::new();
    for i in (body + 1)..tables.len() {
        let marker = tables[i].as_deref();
        if !is_nested_marker(marker) {
            break;
        }
        let (declared_parent, _) = parse_nested_block_name(marker.unwrap())?;
        if declared_parent != parent_block_name {
            break;
        }
        run.push(i);
    }
    if run.is_empty() {
        return Ok(None);
    }

    let raw = find_group(tables, &run)?.expect("run이 비어있지 않으면 그룹이 항상 성립한다");
    let own_name = parse_nested_block_name(tables[raw.body].as_deref().unwrap())?.1;

    if find_child_group(tables, raw.body, &own_name)?.is_some() {
        return Err(format!(
            "'{own_name}' 블록 안에 또 다른 중첩 반복 블록이 있습니다 — 2단계까지만 지원합니다."
        ));
    }

    Ok(Some(RepeatGroup {
        block_name: own_name,
        title: raw.title,
        header: raw.header,
        body: raw.body,
        footer: raw.footer,
        child: None,
    }))
}

/// `TableRoleMarker.findRepeatBlockGroup(Document)` 포트 — 구역 스캔 1건 분량.
fn find_repeat_block_group(tables: &[Option<String>]) -> Result<Option<RepeatGroup>, String> {
    let all: Vec<usize> = (0..tables.len()).collect();
    let filtered: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&i| !is_nested_marker(tables[i].as_deref()))
        .collect();
    let outer = find_group(tables, &filtered)?;
    let child = match &outer {
        Some(g) => find_child_group(tables, g.body, &g.block_name)?,
        None => None,
    };

    // '알 수 없는 부모' 검증은 자식 그룹 계산 뒤에 한다 — 3단계 중첩 실수가 '알 수 없는
    // 부모' 대신 더 정확한 '2단계까지만 지원합니다' 에러로 잡히도록(Java와 같은 순서).
    let child_name = child.as_ref().map(|c| c.block_name.clone());
    let outer_name = outer.as_ref().map(|g| g.block_name.clone());
    for &i in &all {
        let marker = tables[i].as_deref();
        if !is_nested_marker(marker) {
            continue;
        }
        let declared_parent = parse_nested_block_name(marker.unwrap())?.0;
        if Some(&declared_parent) != outer_name.as_ref()
            && Some(&declared_parent) != child_name.as_ref()
        {
            let known = match &outer_name {
                None => " — 문서에 최상위 반복 블록이 없습니다.".to_string(),
                Some(o) => format!(
                    " — 이 문서에서 알려진 블록명: '{o}{}'",
                    child_name
                        .as_ref()
                        .map(|c| format!("', '{c}'"))
                        .unwrap_or_else(|| "'".to_string())
                ),
            };
            return Err(format!(
                "'{}' 마커가 알 수 없는 부모 블록('{declared_parent}')을 참조합니다{known}",
                marker.unwrap()
            ));
        }
    }

    match outer {
        None => Ok(None),
        Some(mut g) => {
            g.child = child.map(Box::new);
            Ok(Some(g))
        }
    }
}

/// `TableRoleMarker.findPageNoTable` 포트.
fn find_pageno_table(tables: &[Option<String>]) -> Result<Option<usize>, String> {
    let mut found: Option<usize> = None;
    for (i, marker) in tables.iter().enumerate() {
        if marker.as_deref() != Some(PAGENO) {
            continue;
        }
        if found.is_some() {
            return Err(
                "여러 개의 표에 '#PAGENO' 마커가 있습니다 — 문서당 #PAGENO는 1개만 지원합니다."
                    .to_string(),
            );
        }
        found = Some(i);
    }
    Ok(found)
}

/// `TemplateSchema` 에 대응하는 분류 결과(방출에 필요한 만큼만).
struct Schema {
    top_level: Vec<String>,
    current_page: Vec<String>,
    total_pages: Vec<String>,
    /// (블록명, 항목 필드, seq 필드, sum 필드) — 첫 등장 순(Java LinkedHashMap 순서).
    blocks: Vec<Block>,
}

struct Block {
    name: String,
    fields: Vec<String>,
    seq_fields: Vec<String>,
    sum_fields: Vec<String>,
    /// 자식 블록 이름('-NESTED:' 마커가 선언한 부모→자식). classify 시 채운다.
    child_name: Option<String>,
    /// blocks 안 자식 인덱스 — `link_children`이 `child_name`을 해석해 채운다.
    child: Option<usize>,
}

/// 삽입 순서를 유지하는 블록 확보(Java LinkedHashMap.computeIfAbsent 대응).
fn block_slot<'a>(blocks: &'a mut Vec<Block>, name: &str) -> usize {
    if let Some(i) = blocks.iter().position(|b| b.name == name) {
        return i;
    }
    blocks.push(Block {
        name: name.to_string(),
        fields: Vec::new(),
        seq_fields: Vec::new(),
        sum_fields: Vec::new(),
        child_name: None,
        child: None,
    });
    blocks.len() - 1
}

/// `FieldSchemaExtractor.scanSection` + `#sum:` 스코프 검증 포트 — 구역 1건 분류.
fn classify_section(scan: &SectionScan, schema: &mut Schema) -> Result<(), String> {
    let group = find_repeat_block_group(&scan.tables)?;
    let pageno_table = find_pageno_table(&scan.tables)?;
    let child = group.as_ref().and_then(|g| g.child.as_deref());

    let mut block_name: Option<String> = None;
    let mut child_block_name: Option<String> = None;
    if let Some(g) = &group {
        block_slot(&mut schema.blocks, &g.block_name);
        block_name = Some(g.block_name.clone());
        if let Some(c) = child {
            block_slot(&mut schema.blocks, &c.block_name);
            child_block_name = Some(c.block_name.clone());
            // Java의 childBlockNameByParent.put(blockName, childBlockName) 대응 —
            // 부모→자식 관계를 이름으로 기록해 link_children이 인덱스로 바꾼다.
            let parent_idx = schema
                .blocks
                .iter()
                .position(|b| b.name == g.block_name)
                .unwrap();
            schema.blocks[parent_idx].child_name = Some(c.block_name.clone());
        }
    }

    for field in &scan.fields {
        let name = field.name.as_str();
        let tbl = field.table;
        let in_body = group.as_ref().is_some_and(|g| tbl == Some(g.body));
        let in_footer_of_repeat = group
            .as_ref()
            .is_some_and(|g| g.footer.is_some_and(|f| tbl == Some(f)));
        let in_pageno_table = pageno_table.is_some_and(|p| tbl == Some(p));
        let in_child_body = child.is_some_and(|c| tbl == Some(c.body));
        let in_child_footer_of_repeat =
            child.is_some_and(|c| c.footer.is_some_and(|f| tbl == Some(f)));
        // 주의: 자식의 title/header/footer는 Option이고 tbl도 Option이라 `==`로 묶으면
        // 둘 다 None일 때 참이 되어 표 밖 필드가 잘못 분류된다 — Some일 때만 비교한다.
        let in_child_title_header_or_footer_of_repeat = child.is_some_and(|c| {
            c.title.is_some_and(|t| tbl == Some(t))
                || c.header.is_some_and(|h| tbl == Some(h))
                || c.footer.is_some_and(|f| tbl == Some(f))
        });

        if name.starts_with(SEQ_FIELD_PREFIX) && group.is_some() && (in_body || in_child_body) {
            let scope = if in_child_body {
                child_block_name.as_deref().unwrap()
            } else {
                block_name.as_deref().unwrap()
            };
            let idx = block_slot(&mut schema.blocks, scope);
            schema.blocks[idx].seq_fields.push(name.to_string());
        } else if name.starts_with(SEQ_FIELD_PREFIX) && group.is_some() {
            return Err(format!(
                "'#seq:' 필드는 반복 블록의 '#REPEAT-BODY:' 표 안에서만 사용할 수 있습니다: {name}"
            ));
        } else if name.starts_with(SUM_FIELD_PREFIX)
            && group.is_some()
            && (in_footer_of_repeat || in_child_footer_of_repeat)
        {
            let scope = if in_child_footer_of_repeat {
                child_block_name.as_deref().unwrap()
            } else {
                block_name.as_deref().unwrap()
            };
            let idx = block_slot(&mut schema.blocks, scope);
            schema.blocks[idx].sum_fields.push(name.to_string());
        } else if name.starts_with(SUM_FIELD_PREFIX) && group.is_some() {
            return Err(format!(
                "'#sum:' 필드는 반복 블록의 '#REPEAT-FOOTER:' 표 안에서만 사용할 수 있습니다: {name}"
            ));
        } else if in_pageno_table {
            if name == CURRENT_PAGE_FIELD {
                schema.current_page.push(name.to_string());
            } else if name == TOTAL_PAGES_FIELD {
                schema.total_pages.push(name.to_string());
            } else {
                return Err(format!(
                    "'#PAGENO' 표 안에는 '{CURRENT_PAGE_FIELD}'/'{TOTAL_PAGES_FIELD}' 필드만 사용할 수 있습니다: {name}"
                ));
            }
        } else if in_body {
            let idx = block_slot(&mut schema.blocks, block_name.as_deref().unwrap());
            schema.blocks[idx].fields.push(name.to_string());
        } else if in_child_body {
            let idx = block_slot(&mut schema.blocks, child_block_name.as_deref().unwrap());
            schema.blocks[idx].fields.push(name.to_string());
        } else if in_child_title_header_or_footer_of_repeat {
            // 자식의 TITLE/HEADER/FOOTER-of-repeat 표는 항목마다가 아니라 그룹마다 한 번만
            // 렌더된다 — 부모 블록 자신의 필드로 분류한다(Java scanSection 주석 참조).
            let idx = block_slot(&mut schema.blocks, block_name.as_deref().unwrap());
            schema.blocks[idx].fields.push(name.to_string());
        } else {
            schema.top_level.push(name.to_string());
        }
    }
    Ok(())
}

/// `#sum:` 스코프 검증 — 각 블록 스코프의 `#sum:<필드명>`이 자기 BODY 필드(또는 자식의 BODY
/// 필드 = 총합계)를 가리키는지 확인한다. Java와 달리 통화 짝(companion) 탐색은 하지 않는다 —
/// record 방출에 쓰이지 않기 때문(필요해지면 `findAdjacentFieldName`을 추가 포팅한다).
fn validate_sum_scope(schema: &mut Schema) -> Result<(), String> {
    for i in 0..schema.blocks.len() {
        let child_fields: Vec<String> = match schema.blocks[i].child {
            Some(c) => schema.blocks[c].fields.clone(),
            None => Vec::new(),
        };
        let body_fields = schema.blocks[i].fields.clone();
        let sum_fields = schema.blocks[i].sum_fields.clone();
        let mut kept: Vec<String> = Vec::new();
        for sum_field in &sum_fields {
            let source = &sum_field[SUM_FIELD_PREFIX.len()..];
            if source.starts_with(SEQ_FIELD_PREFIX) || source.starts_with(SUM_FIELD_PREFIX) {
                return Err(format!(
                    "'{sum_field}'는 계산 필드('{source}')를 가리킬 수 없습니다 — 연쇄 금지."
                ));
            }
            if body_fields.iter().any(|f| f == source) {
                kept.push(sum_field.clone());
            } else if child_fields.iter().any(|f| f == source) {
                // 총합계(grand total)로 재분류 — record 방출에는 어차피 제외된다.
            } else {
                return Err(format!(
                    "'{sum_field}'가 가리키는 필드 '{source}'를 '{}' 블록의 '#REPEAT-BODY:' 표에서 찾지 못했습니다.",
                    schema.blocks[i].name
                ));
            }
        }
        // 스코프 안에 그대로 둔다(방출은 fields만 쓴다).
        schema.blocks[i].sum_fields = kept;
    }
    Ok(())
}

// ───────────────────────────── 방출(Generator 포트) ─────────────────────────────

/// 누름틀 이름 하나 = Java 식별자(가능하면 원본 그대로) + 필요하면 `@JsonProperty` 오버라이드.
struct Identifier {
    original: String,
    java_name: String,
}

impl Identifier {
    fn of(original: &str) -> Self {
        Self {
            original: original.to_string(),
            java_name: sanitize(original),
        }
    }

    fn needs_json_property(&self) -> bool {
        self.original != self.java_name
    }

    fn declaration(&self, java_type: &str) -> String {
        if self.needs_json_property() {
            format!(
                "@JsonProperty(\"{}\") {} {}",
                self.original, java_type, self.java_name
            )
        } else {
            format!("{java_type} {}", self.java_name)
        }
    }
}

/// `Character.isJavaIdentifierStart/Part` 근사 — UTF-16 코드 유닛 단위로 판정한다(서로게이트
/// 반쪽은 Java처럼 무조건 `_`). 한글(Lo)·ASCII·`$`·`_`·숫자·흔한 기호에서 Java와 완전히
/// 같고, Other_Alphabetic 조합 표기나 형식 문자(Cf) 같은 외딴 영역에서만 근사다.
fn u16_is_java_start(u: u16) -> bool {
    if (0xD800..0xE000).contains(&u) {
        return false; // 서로게이트 — Java도 false
    }
    match char::from_u32(u as u32) {
        Some(c) => c == '$' || c == '_' || c.is_alphabetic(),
        None => false,
    }
}

fn u16_is_java_part(u: u16) -> bool {
    if u16_is_java_start(u) {
        return true;
    }
    match char::from_u32(u as u32) {
        Some(c) => c.is_ascii_digit() || c.is_numeric() || c.is_alphabetic(),
        None => false,
    }
}

fn sanitize(name: &str) -> String {
    let mut sb = String::new();
    for (i, unit) in name.encode_utf16().enumerate() {
        let ok = if i == 0 {
            u16_is_java_start(unit)
        } else {
            u16_is_java_part(unit)
        };
        if ok {
            if let Some(c) = char::from_u32(unit as u32) {
                sb.push(c);
            } else {
                sb.push('_');
            }
        } else {
            sb.push('_');
        }
    }
    if is_java_keyword(&sb) {
        sb.push('_');
    }
    sb
}

/// `TemplateEntityGenerator.toPascalCase` — `_`/`-`는 버리고 다음 글자를 대문자로.
/// code는 `[a-z0-9_-]+` 관례라 ASCII 처리로 충분하다.
fn to_pascal_case(code: &str) -> String {
    let mut sb = String::new();
    let mut capitalize_next = true;
    for c in code.chars() {
        if c == '_' || c == '-' {
            capitalize_next = true;
            continue;
        }
        sb.push(if capitalize_next {
            c.to_ascii_uppercase()
        } else {
            c
        });
        capitalize_next = false;
    }
    sb
}

/// 방출에 필요한 스키마 투영 — 블록 트리(부모 → 자식)를 그대로 옮긴다.
struct EmitBlock<'a> {
    name: &'a str,
    fields: &'a [String],
    child: Option<Box<EmitBlock<'a>>>,
}

fn uses_json_property(ids: &[Identifier]) -> bool {
    ids.iter().any(|i| i.needs_json_property())
}

fn block_uses_json_property(block: &EmitBlock) -> bool {
    let item_fields: Vec<Identifier> = block.fields.iter().map(|f| Identifier::of(f)).collect();
    if uses_json_property(&item_fields) {
        return true;
    }
    match &block.child {
        None => false,
        Some(c) => Identifier::of(c.name).needs_json_property() || block_uses_json_property(c),
    }
}

/// 반복 블록의 항목 1개 기준 record를 `decl_indent` 들여쓰기로 만든다. 자식이 있으면(2단계)
/// 그 자식용 `List<...>` 컴포넌트를 하나 더 붙이고 재귀를 한 번 더 돌린다(최대 2단계).
fn emit_item_record(decl_indent: &str, block: &EmitBlock) -> String {
    let block_field = Identifier::of(block.name);
    let item_record_name = block_field.java_name.clone();
    let item_fields: Vec<Identifier> = block.fields.iter().map(|f| Identifier::of(f)).collect();
    let component_indent = format!("{decl_indent}        ");

    let mut sb = String::new();
    sb.push('\n');
    sb.push_str(decl_indent);
    sb.push_str("public record ");
    sb.push_str(&item_record_name);
    sb.push_str("(\n");
    let mut lines: Vec<String> = Vec::new();
    for field in &item_fields {
        lines.push(format!("{component_indent}{}", field.declaration("String")));
    }
    if let Some(child) = &block.child {
        let child_field = Identifier::of(child.name);
        let child_item = child_field.java_name.clone();
        lines.push(format!(
            "{component_indent}{}",
            child_field.declaration(&format!("List<{child_item}>"))
        ));
    }
    sb.push_str(&lines.join(",\n"));
    sb.push_str(") {\n");
    if let Some(child) = &block.child {
        sb.push_str(&emit_item_record(&format!("{decl_indent}    "), child));
    }
    sb.push_str(decl_indent);
    sb.push_str("}\n");
    sb
}

/// `TemplateEntityGenerator.generateDataClass` 포트.
fn generate_data_class(
    package: &str,
    data_class_name: &str,
    code: &str,
    schema: &Schema,
) -> String {
    let top_level: Vec<Identifier> = schema.top_level.iter().map(|f| Identifier::of(f)).collect();
    let blocks = &schema.blocks;
    let root_blocks: Vec<usize> = (0..blocks.len())
        .filter(|&i| !blocks.iter().any(|b| b.child == Some(i)))
        .collect();
    // Java는 repeatBlocks.get(0)만 쓴다(추출기가 문서당 1블록을 보장).
    let first_root = root_blocks.first().copied();

    let emit_block = first_root.map(|i| EmitBlock {
        name: &blocks[i].name,
        fields: &blocks[i].fields,
        child: blocks[i].child.map(|c| {
            Box::new(EmitBlock {
                name: &blocks[c].name,
                fields: &blocks[c].fields,
                child: None,
            })
        }),
    });

    let needs_json_property =
        uses_json_property(&top_level) || emit_block.as_ref().is_some_and(block_uses_json_property);

    let mut sb = String::new();
    sb.push_str(&format!("package {package};\n\n"));
    if emit_block.is_some() {
        sb.push_str("import java.util.List;\n\n");
    }
    if needs_json_property {
        sb.push_str("import com.fasterxml.jackson.annotation.JsonProperty;\n\n");
    }
    sb.push_str(&format!(
        "/** '{code}' 템플릿의 타입 데이터 클래스 초안 — TemplateEntityGenerator가 생성함. 리뷰 후 사용하세요. */\n"
    ));
    sb.push_str(&format!("public record {data_class_name}(\n"));

    let mut component_lines: Vec<String> = Vec::new();
    for field in &top_level {
        component_lines.push(format!("        {}", field.declaration("String")));
    }
    if let Some(block) = &emit_block {
        let block_field = Identifier::of(block.name);
        let item_record_name = block_field.java_name.clone();
        component_lines.push(format!(
            "        {}",
            block_field.declaration(&format!("List<{item_record_name}>"))
        ));
    }
    sb.push_str(&component_lines.join(",\n"));
    sb.push_str(") {\n");

    if let Some(block) = &emit_block {
        sb.push_str(&emit_item_record("    ", block));
    }

    sb.push_str("}\n");
    sb
}

/// `TemplateEntityGenerator.generateModuleClass` 포트 — 고정 템플릿.
fn generate_module_class(
    package: &str,
    data_class_name: &str,
    module_class_name: &str,
    code: &str,
) -> String {
    format!(
        "package {package};\n\n\
         import java.io.IOException;\n\n\
         import com.ktnet.aspline.hwpx.tooling.template.HwpxTemplateModule;\n\n\
         /**\n\
         \x20* '{code}' 템플릿 모듈 초안 — TemplateEntityGenerator가 생성함. sampleData()를 채우는 걸 권장합니다.\n\
         \x20* 등록하려면 HwpxTemplateEngineApplication의 모듈 목록에 인스턴스를 추가하세요.\n\
         \x20*/\n\
         public class {module_class_name} implements HwpxTemplateModule<{data_class_name}> {{\n\n\
         \x20   private static final String RESOURCE_PATH = \"/hwpx/{code}.hwpx\";\n\n\
         \x20   @Override\n\
         \x20   public String code() {{\n\
         \x20       return \"{code}\";\n\
         \x20   }}\n\n\
         \x20   @Override\n\
         \x20   public Class<{data_class_name}> dataType() {{\n\
         \x20       return {data_class_name}.class;\n\
         \x20   }}\n\n\
         \x20   @Override\n\
         \x20   public byte[] hwpxBytes() throws IOException {{\n\
         \x20       return HwpxTemplateModule.readClasspathResource({module_class_name}.class, RESOURCE_PATH);\n\
         \x20   }}\n\n\
         \x20   // TODO: 데모/스키마 미리보기용 예시 데이터가 필요하면 sampleData()를 override하세요.\n\
         }}\n"
    )
}

// ───────────────────────────── 공개 질의 ─────────────────────────────

/// 엔티티 코드젠 결과 — 검증 에러는 예외가 아니라 `errors`로 돌려준다(Java는
/// IllegalArgumentException을 던지지만, 질의 API 게이트 밖에서 한국어 메시지를 그대로
/// 보여줘야 하므로 값으로 흘린다).
pub struct TemplateEntityResult {
    pub data_class_name: String,
    pub module_class_name: String,
    pub data_class_source: String,
    pub module_class_source: String,
    pub errors: Vec<String>,
}

impl DocumentCore {
    /// 문서에서 누름틀 스키마를 뽑아 Java record 데이터 클래스 + 모듈 클래스 초안을 만든다.
    pub fn template_entity(&self, code: &str, package: &str) -> TemplateEntityResult {
        let data_class_name = format!("{}Data", to_pascal_case(code));
        let module_class_name = format!("{}TemplateModule", to_pascal_case(code));

        let mut schema = Schema {
            top_level: Vec::new(),
            current_page: Vec::new(),
            total_pages: Vec::new(),
            blocks: Vec::new(),
        };
        let mut errors: Vec<String> = Vec::new();
        for section in &self.document.sections {
            let mut scan = SectionScan {
                tables: Vec::new(),
                fields: Vec::new(),
            };
            scan_paragraphs(&section.paragraphs, None, &mut scan);
            // 첫 구역의 검증 실패로 전체가 죽는 Java와 달리 없다 — Java도 첫 에러에서
            // 바로 던지므로 여기도 첫 에러만 실어 보낸다.
            if let Err(e) = classify_section(&scan, &mut schema) {
                errors.push(e);
                break;
            }
        }
        if errors.is_empty() {
            link_children(&mut schema);
            if let Err(e) = validate_sum_scope(&mut schema) {
                errors.push(e);
            }
        }

        let (data_source, module_source) = if errors.is_empty() {
            (
                generate_data_class(package, &data_class_name, code, &schema),
                generate_module_class(package, &data_class_name, &module_class_name, code),
            )
        } else {
            (String::new(), String::new())
        };

        TemplateEntityResult {
            data_class_name,
            module_class_name,
            data_class_source: data_source,
            module_class_source: module_source,
            errors,
        }
    }

    /// [`Self::template_entity`]의 JSON 봉투 — CLI/WASM 양쪽이 같은 모양을 소비한다.
    ///
    /// 블록 스키마(필드 목록 등) 자체는 아직 이 봉투에 노출하지 않는다 — 방출된 소스
    /// 문자열이 곧 스키마의 표현이라 studio UI는 소스만 있으면 된다. 전체 스키마 JSON
    /// 패리티가 필요해지면(예: 서버 없는 schema.json 미리보기) 별도 필드로 추가한다.
    ///
    /// 출처 표지(`untrustedContent`/`untrustedFields`)는 여기서 붙이지 않는다 — 그건
    /// CLI 출력 계층(`provenance::marked`)의 몫이다. WASM 소비자는 이 메서드를 직접
    /// 쓰므로 표지 없는 순수 데이터 모양을 유지한다. CLI는 [`Self::template_entity_envelope`]
    /// 로 `Value`를 받아 `provenance::marked`로 감싼 뒤 출력한다.
    pub fn template_entity_json(&self, code: &str, package: &str) -> String {
        self.template_entity_envelope(code, package).to_string()
    }

    /// [`Self::template_entity_json`]과 같은 모양의 `Value` — CLI가 `provenance::marked`로
    /// 감싸기 전에 파싱을 왕복하지 않도록 값 형태로 노출한다.
    pub fn template_entity_envelope(&self, code: &str, package: &str) -> Value {
        let result = self.template_entity(code, package);
        Value::Object({
            let mut m = serde_json::Map::new();
            m.insert("code".into(), json!(code));
            m.insert("packageName".into(), json!(package));
            m.insert("dataClassName".into(), json!(result.data_class_name));
            m.insert("moduleClassName".into(), json!(result.module_class_name));
            m.insert("dataClassSource".into(), json!(result.data_class_source));
            m.insert(
                "moduleClassSource".into(),
                json!(result.module_class_source),
            );
            m.insert("errors".into(), json!(result.errors));
            m
        })
    }
}

/// classify가 기록한 `child_name`(부모→자식)을 blocks 안 인덱스로 바꾼다.
/// Java의 childBlockNameByParent와 같은 그림 — 여러 구역에서 같은 블록명이 오면
/// 첫 슬롯(Java의 computeIfAbsent)이 그대로 링크를 갖는다.
fn link_children(schema: &mut Schema) {
    let links: Vec<Option<usize>> = schema
        .blocks
        .iter()
        .map(|b| {
            b.child_name
                .as_ref()
                .and_then(|cn| schema.blocks.iter().position(|x| &x.name == cn))
        })
        .collect();
    for (i, link) in links.into_iter().enumerate() {
        schema.blocks[i].child = link;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_drops_separators_and_capitalizes() {
        assert_eq!(to_pascal_case("fix-flat"), "FixFlat");
        assert_eq!(to_pascal_case("ccrlic1dno"), "Ccrlic1dno");
        assert_eq!(to_pascal_case("a_b-c"), "ABC");
    }

    #[test]
    fn sanitize_matches_java_rules() {
        // Java golden(fix-flat)에서 실측한 변환.
        assert_eq!(sanitize("신청인_성명"), "신청인_성명");
        assert_eq!(sanitize("신청인_성명(한글)"), "신청인_성명_한글_");
        assert_eq!(sanitize("1차_선택"), "_차_선택");
        assert_eq!(sanitize("record"), "record_");
        assert_eq!(sanitize("$보증금"), "$보증금");
        assert_eq!(sanitize("품목분류번호(HS)"), "품목분류번호_HS_");
    }

    #[test]
    fn needs_json_property_only_when_mangled() {
        assert!(!Identifier::of("신청인_성명").needs_json_property());
        assert!(Identifier::of("신청인_성명(한글)").needs_json_property());
        assert!(Identifier::of("record").needs_json_property());
        assert!(!Identifier::of("$보증금").needs_json_property());
    }

    fn schema_of(
        top: Vec<&str>,
        blocks: Vec<(&str, Vec<&str>, Option<(&str, Vec<&str>)>)>,
    ) -> Schema {
        let mut s = Schema {
            top_level: top.into_iter().map(String::from).collect(),
            current_page: Vec::new(),
            total_pages: Vec::new(),
            blocks: Vec::new(),
        };
        for (name, fields, child) in blocks {
            let child_idx = child.map(|(cn, cf)| {
                s.blocks.push(Block {
                    name: cn.to_string(),
                    fields: cf.into_iter().map(String::from).collect(),
                    seq_fields: Vec::new(),
                    sum_fields: Vec::new(),
                    child_name: None,
                    child: None,
                });
                s.blocks.len() - 1
            });
            s.blocks.push(Block {
                name: name.to_string(),
                fields: fields.into_iter().map(String::from).collect(),
                seq_fields: Vec::new(),
                sum_fields: Vec::new(),
                child_name: None,
                child: child_idx,
            });
        }
        s
    }

    #[test]
    fn emits_flat_record_without_repeat() {
        let schema = schema_of(vec!["신청인_성명", "신청인_성명(한글)"], vec![]);
        let out = generate_data_class("com.example.fix", "FixFlatData", "fix-flat", &schema);
        assert_eq!(
            out,
            "package com.example.fix;\n\n\
             import com.fasterxml.jackson.annotation.JsonProperty;\n\n\
             /** 'fix-flat' 템플릿의 타입 데이터 클래스 초안 — TemplateEntityGenerator가 생성함. 리뷰 후 사용하세요. */\n\
             public record FixFlatData(\n\
             \x20       String 신청인_성명,\n\
             \x20       @JsonProperty(\"신청인_성명(한글)\") String 신청인_성명_한글_) {\n\
             }\n"
        );
    }

    #[test]
    fn emits_doubly_nested_record() {
        let schema = schema_of(
            vec!["신청인_상호"],
            vec![(
                "수입물품내역",
                vec!["수입물품내역_NO", "물품그룹_명칭"],
                Some(("물품상세내역", vec!["물품상세내역_상세명"])),
            )],
        );
        let out = generate_data_class("com.example.fix", "FixNestedData", "fix-nested", &schema);
        assert!(out.contains("import java.util.List;\n\n"));
        assert!(!out.contains("JsonProperty"));
        assert!(out.contains("        List<수입물품내역> 수입물품내역) {\n"));
        assert!(out.contains("    public record 수입물품내역(\n"));
        assert!(out.contains("            List<물품상세내역> 물품상세내역) {\n"));
        assert!(out.contains("        public record 물품상세내역(\n"));
        // 선언 순서: 외부 record → 항목 record → 자식 record.
        let i_outer = out.find("public record FixNestedData").unwrap();
        let i_item = out.find("public record 수입물품내역").unwrap();
        let i_child = out.find("public record 물품상세내역").unwrap();
        assert!(i_outer < i_item && i_item < i_child);
    }

    #[test]
    fn module_class_is_fixed_template() {
        let out = generate_module_class(
            "com.example.fix",
            "FixFlatData",
            "FixFlatTemplateModule",
            "fix-flat",
        );
        assert!(out.starts_with("package com.example.fix;\n\nimport java.io.IOException;\n\n"));
        assert!(out.contains("RESOURCE_PATH = \"/hwpx/fix-flat.hwpx\""));
        assert!(out.ends_with("}\n"));
    }

    #[test]
    fn group_validation_errors_match_java_messages() {
        let tables = vec![
            Some("#REPEAT-BODY:품목내역".to_string()),
            Some("#REPEAT-BODY:품목내역".to_string()),
        ];
        let err = find_repeat_block_group(&tables).unwrap_err();
        assert_eq!(
            err,
            "여러 개의 표에 '#REPEAT-BODY:품목내역' 마커가 있습니다 — 문서당 반복 블록은 1개만 지원합니다."
        );
    }

    #[test]
    fn group_discontiguous_tables_error() {
        let tables = vec![
            Some("#REPEAT-TITLE:품목내역".to_string()),
            None,
            Some("#REPEAT-BODY:품목내역".to_string()),
        ];
        let err = find_repeat_block_group(&tables).unwrap_err();
        assert!(err.starts_with("'품목내역' 반복 블록의 표들(#REPEAT-TITLE:"));
        assert!(err.contains("서로 떨어져 있습니다"));
    }

    #[test]
    fn nested_marker_parse() {
        let (parent, child) =
            parse_nested_block_name("#REPEAT-BODY-NESTED:수입물품내역/물품상세내역").unwrap();
        assert_eq!(parent, "수입물품내역");
        assert_eq!(child, "물품상세내역");
        assert!(parse_nested_block_name("#REPEAT-BODY:품목내역").is_err());
    }

    #[test]
    fn pageno_table_found_and_duplicated_rejected() {
        let tables = vec![Some("#PAGENO".to_string())];
        assert_eq!(find_pageno_table(&tables).unwrap(), Some(0));
        let dup = vec![Some("#PAGENO".to_string()), Some("#PAGENO".to_string())];
        assert!(find_pageno_table(&dup).is_err());
    }
}
