//! [#2550] BinData deflate bomb — 저장·클립보드·렌더 경로 상한 회귀.
//!
//! `/BinData/BIN0001.*` 에 zeros 를 deflate 한 작은 스트림(해제 시 수 GB)을 넣으면,
//! 파싱은 지연 등록이라 저렴하게 성공하고 **저장하거나 그림을 복사·렌더하는 순간**
//! 무제한 `load()` 가 전량 materialize 해 OOM 이 났다. wasm32 에서는 모듈 abort 로
//! 열려 있는 다른 문서까지 함께 죽는 실패 양상이다.
//!
//! 수정 방향은 이슈 합의(C안: 경로별 차등)다.
//!
//! - **저장(HWP5)**: 상한 초과 시 압축 해제를 포기하고 **원본 저장 바이트를 그대로**
//!   기록한다. 정상 대용량 개체는 무손실이고, 폭탄은 애초에 해제하지 않는다.
//! - **렌더·클립보드·질의**: 상한 초과는 placeholder(빈 값/None) — 이미지 누락과 같은 경로.
//!
//! 공격 문서는 저장소에 커밋하지 않고 **시험 시점에 합성한다**
//! (`tests/security_corpus_regression.rs` 와 같은 방침).
#![cfg(not(target_arch = "wasm32"))]

use std::io::Write;
use std::path::{Path, PathBuf};

use rhwp::model::bin_data::MAX_BIN_DATA_BYTES;
use rhwp::parser::cfb_reader::CfbReader;
use rhwp::serializer::mini_cfb;
use rhwp::{parse_document, serialize_document, DocumentCore};

/// BinData 를 가진 실문서 — 폭탄을 심을 숙주다.
const HOST_SAMPLE: &str = "samples/143E433F503322BD33.hwp";

/// 폭탄의 압축 해제 크기. 상한(256MB)의 4배로, 해제되면 즉시 관측 가능한 규모다.
const BOMB_PLAIN_BYTES: usize = 1024 * 1024 * 1024;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// zeros 를 raw deflate 로 압축해 폭탄 스트림을 만든다.
///
/// HWP5 BinData 스트림의 압축 형식과 같은 raw deflate(wbits=-15)다. 입력이 전부
/// 0 이라 산출물은 수 KB 이며, 해제하면 [`BOMB_PLAIN_BYTES`] 가 된다.
fn deflate_bomb() -> Vec<u8> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    let chunk = vec![0_u8; 1024 * 1024];
    let mut written = 0;
    while written < BOMB_PLAIN_BYTES {
        let n = chunk.len().min(BOMB_PLAIN_BYTES - written);
        encoder.write_all(&chunk[..n]).expect("deflate write");
        written += n;
    }
    encoder.finish().expect("deflate finish")
}

/// 숙주 문서의 첫 `/BinData/*` 스트림을 폭탄으로 갈아끼운 CFB 를 만든다.
///
/// DocInfo 는 그대로 두므로 `HWPTAG_BIN_DATA` 레코드와 스트림 대응이 유지된다 —
/// 파서가 정상 이미지로 지연 등록하고, 실제 폭발은 소비 경로에서 일어난다.
fn synthesize_bomb_document() -> (Vec<u8>, String, Vec<u8>) {
    let host = std::fs::read(repo(HOST_SAMPLE)).expect("숙주 표본 읽기");
    let mut reader = CfbReader::open(&host).expect("숙주 CFB 열기");

    let bin_name = reader
        .list_bin_data()
        .into_iter()
        .next()
        .expect("숙주에 BinData 스트림이 있어야 함");
    let bomb_path = format!("/BinData/{}", bin_name);
    let bomb = deflate_bomb();

    let paths = reader.list_streams();
    let mut streams: Vec<(String, Vec<u8>)> = Vec::new();
    for path in paths {
        let data = if path == bomb_path {
            bomb.clone()
        } else {
            reader.read_stream_raw(&path).expect("스트림 읽기")
        };
        streams.push((path, data));
    }

    let named: Vec<(&str, &[u8])> = streams
        .iter()
        .map(|(p, d)| (p.as_str(), d.as_slice()))
        .collect();
    let bytes = mini_cfb::build_cfb(&named).expect("공격 문서 CFB 조립");
    (bytes, bomb_path, bomb)
}

/// 파싱은 폭탄을 해제하지 않는다 (지연 등록) — 공격 전제의 확인.
#[test]
fn parsing_a_bomb_document_stays_cheap() {
    let (attack, _, _) = synthesize_bomb_document();
    let document = parse_document(&attack).expect("공격 문서 파싱은 성공해야 함");
    assert!(
        !document.bin_data_content.is_empty(),
        "폭탄 항목이 BinData 로 등록되어야 소비 경로 시험이 의미를 갖는다"
    );
}

/// 저장 경로는 OOM 없이 **원본 압축 바이트를 그대로** 보존한다.
///
/// 수정 전에는 `cfb_writer` 가 무제한 `load()` 로 1GB 를 materialize 했다.
#[test]
fn saving_a_bomb_document_preserves_original_stream_without_decompressing() {
    let (attack, bomb_path, bomb) = synthesize_bomb_document();
    let document = parse_document(&attack).expect("공격 문서 파싱");

    let saved = serialize_document(&document).expect("저장은 성공해야 함");

    let mut reader = CfbReader::open(&saved).expect("저장 결과 CFB 열기");
    let written = reader
        .read_stream_raw(&bomb_path)
        .expect("폭탄 스트림이 저장 결과에도 있어야 함");
    assert_eq!(
        written, bomb,
        "상한 초과 항목은 해제·재압축 없이 원본 저장 바이트가 그대로 보존되어야 한다 \
         (빈 값으로 유실되면 데이터 손실)"
    );
}

/// 렌더·질의 경로는 상한 초과를 placeholder 로 접는다.
#[test]
fn render_and_query_paths_fold_an_oversized_entry_to_placeholder() {
    let (attack, _, _) = synthesize_bomb_document();
    let core = DocumentCore::from_bytes(&attack).expect("DocumentCore 적재");

    assert_eq!(
        core.get_bin_data(0),
        None,
        "상한 초과 항목의 바이트 질의는 항목 없음과 같아야 한다"
    );

    // 렌더가 폭탄 항목을 만나도 해제하지 않고 완주한다 (쪽 번호 0-based).
    let svg = core
        .render_page_svg_native(0)
        .expect("첫 쪽 렌더는 성공해야 함");
    assert!(!svg.is_empty(), "렌더 산출물이 비어서는 안 된다");
}

/// 클립보드 이미지 질의는 상한 초과를 오류로 돌려준다 (materialize 하지 않는다).
#[test]
fn clipboard_image_queries_reject_an_oversized_entry() {
    let (attack, _, _) = synthesize_bomb_document();
    let core = DocumentCore::from_bytes(&attack).expect("DocumentCore 적재");

    let error = core
        .get_bin_data_image_data_native(1)
        .expect_err("상한 초과 항목은 바이트를 돌려주지 않아야 한다");
    assert!(
        error.to_string().contains("상한"),
        "상한 초과임이 오류 메시지에 드러나야 한다: {error}"
    );

    assert!(
        core.get_bin_data_image_mime_native(1).is_err(),
        "MIME 판별도 전체 해제 없이 실패해야 한다"
    );
}

/// 정상 문서는 상한 도입 전후로 왕복 결과가 같다 (데이터 손실 회귀 가드).
#[test]
fn normal_documents_round_trip_unchanged_under_the_limit() {
    let host = std::fs::read(repo(HOST_SAMPLE)).expect("숙주 표본 읽기");
    let document = parse_document(&host).expect("숙주 파싱");

    let before: Vec<Vec<u8>> = document
        .bin_data_content
        .iter()
        .map(|c| c.data.load())
        .collect();
    assert!(
        before
            .iter()
            .all(|b| !b.is_empty() && b.len() <= MAX_BIN_DATA_BYTES),
        "숙주 표본의 BinData 는 상한 이내의 정상 데이터여야 한다"
    );

    let saved = serialize_document(&document).expect("숙주 저장");
    let reparsed = parse_document(&saved).expect("저장 결과 재파싱");
    let after: Vec<Vec<u8>> = reparsed
        .bin_data_content
        .iter()
        .map(|c| c.data.load())
        .collect();

    assert_eq!(
        after, before,
        "상한 이내 정상 BinData 는 왕복에서 바이트가 보존되어야 한다"
    );
}
