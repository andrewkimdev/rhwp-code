//! rhwp-studio(GUI 편집기)로 작성한 문서가 HWPX 로 저장될 때의 두 내보내기 결함
//! 회귀 가드 — hwpx-template-engine 쪽 조사(docs/claude/investigations/
//! rhwp-studio-export-defects-investigation.md)에서 확정된 원인:
//!
//! 1. 표 생성이 instance_id 를 raw_ctrl_data 에만 쓰고 CommonObjAttr 는
//!    기본값 0 으로 남겨, HWPX 직렬화(common 쪽을 읽음)가 **모든 표를
//!    `id="0"`으로** 내보냈다. 차원 해시는 같은 모양 표끼리 충돌하므로 대체
//!    안도 아니었다 — 이제 문서 내 저장 최댓값 위에서 증가하는 고유 비-0 id 를
//!    내린다.
//! 2. 누름틀 삽입이 guide_residue 를 기록하지 않아, 저장 시 안내문 본문 run
//!    (#3545 emit_guide_residue)이 방출되지 않았다. fieldBegin/fieldEnd 가
//!    붙어 나간 파일은 채울 텍스트 run 이 없어 소비자가 값을 넣을 수 없다.
//!    이제 삽입 시점부터 잔재를 기록한다.
#![cfg(not(target_arch = "wasm32"))]

use std::io::Read;

use rhwp::wasm_api::HwpDocument;

/// 저장본의 본문 section XML 을 연결해 돌려준다 (issue_3545 테스트와 동일 도구).
fn section_xml(hwpx: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(hwpx.to_vec())).expect("저장본 ZIP");
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    let mut xml = String::new();
    for name in names {
        if name.starts_with("Contents/section") && name.ends_with(".xml") {
            zip.by_name(&name)
                .expect("section 엔트리")
                .read_to_string(&mut xml)
                .expect("section XML 은 UTF-8");
        }
    }
    xml
}

/// 문서 순서대로 모든 `<hp:tbl>`의 `id` 속성값.
fn table_ids(xml: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<hp:tbl ") {
        let tag_end = rest[i..].find('>').expect("hp:tbl 태그 닫힘");
        let tag = &rest[i..i + tag_end];
        let id_start = tag.find("id=\"").expect("hp:tbl id 속성");
        let val_start = id_start + 4;
        let val_end = tag[val_start..].find('"').expect("id 속성 닫따옴표");
        ids.push(tag[val_start..val_start + val_end].to_string());
        rest = &rest[i + tag_end..];
    }
    ids
}

/// create_table_native 반환 JSON 에서 paraIdx 를 뽑는다 (main.rs gen_table 관용).
fn para_idx(json: &str) -> usize {
    json.split("\"paraIdx\":")
        .nth(1)
        .and_then(|s| s.split(&[',', '}'][..]).next())
        .and_then(|s| s.trim().parse().ok())
        .expect("paraIdx 파싱")
}

/// `name="X"` 누름틀의 fieldBegin부터 대응 fieldEnd 직전까지의 XML 조각.
fn field_span_by_name<'a>(xml: &'a str, name: &str) -> &'a str {
    let n = xml
        .find(&format!("name=\"{name}\""))
        .unwrap_or_else(|| panic!("fieldBegin name={name} 없음"));
    let begin = xml[..n].rfind("<hp:fieldBegin").expect("name 을 감싼 fieldBegin");
    let id_start = begin + xml[begin..].find("id=\"").expect("fieldBegin id 속성") + 4;
    let id_end = id_start + xml[id_start..].find('"').expect("id 닫따옴표");
    let id = &xml[id_start..id_end];
    let end = xml
        .find(&format!("beginIDRef=\"{id}\""))
        .unwrap_or_else(|| panic!("fieldEnd beginIDRef={id} 없음"));
    assert!(begin < end, "fieldBegin 이 fieldEnd 앞이어야 함 (name={name})");
    &xml[begin..end]
}

/// 조각 안에 내용이 있는 `<hp:t>` 가 존재하는지 (빈 `<hp:t></hp:t>`/`<hp:t/>` 는 무시).
fn contains_nonempty_hp_t(fragment: &str) -> bool {
    let mut rest = fragment;
    while let Some(i) = rest.find("<hp:t>") {
        rest = &rest[i + "<hp:t>".len()..];
        if !rest.starts_with("</hp:t>") {
            return true;
        }
    }
    false
}

// =====================================================================
// 결함 1 — 표 id 고유성
// =====================================================================

/// 같은 모양 표 여러 개(기본+TAC 경로 혼합)를 만들어 저장하면 id 가 서로 다른
/// 비-0 값이어야 한다. 결함 당시에는 전부 id="0"이었다.
#[test]
fn same_shape_tables_get_distinct_nonzero_ids() {
    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document().expect("빈 문서 초기화");
    let r1 = doc.create_table_native(0, 0, 0, 2, 3).expect("1번째 표");
    let p1 = para_idx(&r1);
    let r2 = doc.create_table_native(0, p1 + 1, 0, 2, 3).expect("2번째 표(같은 모양)");
    let p2 = para_idx(&r2);
    doc.create_table_ex_native(0, p2 + 1, 0, 2, 3, true, None, None)
        .expect("3번째 표(treatAsChar)");

    let xml = section_xml(&doc.export_hwpx_native().expect("export_hwpx"));
    let ids = table_ids(&xml);
    assert_eq!(ids.len(), 3, "표 3개가 내보내져야 한다: {ids:?}");
    assert!(
        ids.iter().all(|id| id != "0"),
        "어떤 표도 id=\"0\"이면 안 된다: {ids:?}"
    );
    let mut uniq = ids.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), ids.len(), "표 id 가 충돌했다: {ids:?}");
}

/// HWPX 저장본을 다시 적재하면 id 가 보존되어야 한다 — 파서가 id 를
/// common.instance_id 로 읽으므로, 저장→적재→저장에서 id 가 유지되면 두 축이
/// 새 id 체계로 맞물린 것이다.
#[test]
fn table_ids_survive_hwpx_reload() {
    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document().expect("빈 문서 초기화");
    let r1 = doc.create_table_native(0, 0, 0, 2, 2).expect("1번째 표");
    let p1 = para_idx(&r1);
    doc.create_table_native(0, p1 + 1, 0, 2, 2).expect("2번째 표");

    let saved = doc.export_hwpx_native().expect("export_hwpx");
    let xml1 = section_xml(&saved);
    let reloaded = HwpDocument::from_bytes(&saved).expect("재적재");
    let xml2 = section_xml(&reloaded.export_hwpx_native().expect("재저장"));

    assert_eq!(table_ids(&xml1), table_ids(&xml2), "재적재 후 표 id 가 변했다");
}

// =====================================================================
// 결함 2 — 누름틀 안내문 본문 run
// =====================================================================

/// 새로 삽입한 누름틀의 begin~end 사이에 안내문 본문 run 이 있어야 한다
/// (한컴 정준형 — form-01.hwpx). 결함 당시에는 fieldBegin/fieldEnd 가 붙어
/// 나갔다.
#[test]
fn inserted_field_exports_guide_display_run() {
    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document().expect("빈 문서 초기화");
    doc.insert_click_here_field_at(0, 0, 0, "입력하세요", "", "신청일자", true)
        .expect("누름틀 삽입");

    let xml = section_xml(&doc.export_hwpx().expect("export_hwpx"));
    let span = field_span_by_name(&xml, "신청일자");
    assert!(
        span.contains("<hp:t>입력하세요</hp:t>"),
        "삽입한 누름틀의 begin~end 사이에 안내문 본문 run 이 없다: {span}"
    );
}

/// 고정점: 저장→재적재→재저장에서 안내문 run 이 정확히 1회 유지되고, 값 API 는
/// 적재 정규화 계약대로 빈 값을 유지한다 (#3545 잔재 의미론 그대로).
#[test]
fn inserted_field_guide_run_save_reload_save_fixed_point() {
    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document().expect("빈 문서 초기화");
    doc.insert_click_here_field_at(0, 0, 0, "입력하세요", "", "신청일자", true)
        .expect("누름틀 삽입");

    let saved = doc.export_hwpx_native().expect("export 1");
    let reloaded = HwpDocument::from_bytes(&saved).expect("재적재");
    let field_json = reloaded.get_field_list();
    let v: serde_json::Value = serde_json::from_str(&field_json).expect("field list JSON");
    let fields = v["fields"].as_array().or_else(|| v.as_array()).expect("fields 배열");
    let value = fields
        .iter()
        .find(|f| f["name"] == "신청일자")
        .map(|f| f["value"].as_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert_eq!(value, "", "재적재 후 값 API 는 빈 값이어야 한다 (정규화 계약)");

    let xml2 = section_xml(&reloaded.export_hwpx_native().expect("export 2"));
    assert_eq!(
        xml2.matches("<hp:t>입력하세요</hp:t>").count(),
        1,
        "고정점에서 안내문 run 이 소실/중복됐다"
    );
}

/// 안내문을 비워 삽입한 누름틀에는 텍스트 run 을 주입하지 않는다 — 원본부터
/// 빈 스팬은 #3545 의 gov_form 가드와 같은 계약.
#[test]
fn inserted_field_without_guide_emits_no_display_run() {
    let mut doc = HwpDocument::create_empty();
    doc.create_blank_document().expect("빈 문서 초기화");
    doc.insert_click_here_field_at(0, 0, 0, "", "", "메모만", true)
        .expect("누름틀 삽입");

    let xml = section_xml(&doc.export_hwpx().expect("export_hwpx"));
    let span = field_span_by_name(&xml, "메모만");
    assert!(
        !contains_nonempty_hp_t(span),
        "안내문 없는 누름틀에 텍스트가 주입됐다: {span}"
    );
}
