//! HWPX ODF password-protection 해제.
//!
//! 한글이 만든 암호 HWPX는 ZIP 엔트리를 raw-deflate 한 뒤 AES-256-CBC로
//! 암호화하고, 평문 `META-INF/manifest.xml`의 ODF `encryption-data`에
//! PBKDF2·checksum 정보를 둔다. 이 모듈은 암호 패키지를 메모리 안에서
//! 평문 HWPX ZIP으로 재구성한 뒤 기존 HWPX 파서에 넘긴다.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};

use aes::Aes256;
use base64::Engine as _;
use cbc::Decryptor;
use cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use flate2::read::DeflateDecoder;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use roxmltree::{Document, Node};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::reader::{MAX_BINDATA_SIZE, MAX_XML_SIZE};
use super::HwpxError;

const MANIFEST_PATH: &str = "META-INF/manifest.xml";
const AES_256_CBC: &str = "http://www.w3.org/2001/04/xmlenc#aes256-cbc";
const SHA_256_START_KEY: &str = "http://www.w3.org/2000/09/xmldsig#sha256";
const SHA_256_1K_SUFFIX: &str = "#sha256-1k";
/// 정상 한글 HWPX는 1,024회다. 악성 manifest가 과도한 CPU 작업을 강제하지
/// 못하도록 넉넉한 상한을 둔다.
const MAX_PBKDF2_ITERATIONS: u32 = 1_000_000;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pbkdf2Prf {
    HmacSha1,
    HmacSha256,
}

#[derive(Debug)]
struct EntryCrypto {
    path: String,
    checksum: Vec<u8>,
    iv: Vec<u8>,
    salt: Vec<u8>,
    iterations: u32,
    key_size: usize,
}

#[derive(Debug)]
enum EntryDecryptError {
    Invalid,
    LimitExceeded { max_bytes: usize },
}

fn attribute(node: Node<'_, '_>, local: &str) -> Option<String> {
    node.attributes()
        .find(|attribute| attribute.name().rsplit(':').next() == Some(local))
        .map(|attribute| attribute.value().to_string())
}

fn child<'a, 'input>(node: Node<'a, 'input>, local: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|candidate| candidate.is_element() && candidate.tag_name().name() == local)
}

fn decode_base64(value: Option<String>, field: &str) -> Result<Vec<u8>, HwpxError> {
    let value = value.ok_or_else(|| {
        HwpxError::UnsupportedEncryption(format!("manifest {field} 값이 없습니다"))
    })?;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| {
            HwpxError::UnsupportedEncryption(format!("manifest {field} 값이 base64가 아닙니다"))
        })
}

fn parse_manifest(data: &[u8]) -> Result<Vec<EntryCrypto>, HwpxError> {
    let text = std::str::from_utf8(data).map_err(|_| {
        HwpxError::UnsupportedEncryption("manifest.xml이 UTF-8이 아닙니다".to_string())
    })?;
    let document = Document::parse(text).map_err(|_| {
        HwpxError::UnsupportedEncryption("manifest.xml 파싱에 실패했습니다".to_string())
    })?;
    let mut entries = Vec::new();
    let mut paths = HashSet::new();

    for file_entry in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "file-entry")
    {
        let Some(encryption) = child(file_entry, "encryption-data") else {
            continue;
        };
        let algorithm = child(encryption, "algorithm").ok_or_else(|| {
            HwpxError::UnsupportedEncryption("manifest 암호 알고리즘 정보가 없습니다".to_string())
        })?;
        let derivation = child(encryption, "key-derivation").ok_or_else(|| {
            HwpxError::UnsupportedEncryption("manifest 키 파생 정보가 없습니다".to_string())
        })?;
        let start_key = child(encryption, "start-key-generation").ok_or_else(|| {
            HwpxError::UnsupportedEncryption("manifest 시작 키 정보가 없습니다".to_string())
        })?;
        let path = attribute(file_entry, "full-path").ok_or_else(|| {
            HwpxError::UnsupportedEncryption("manifest 암호화 경로가 없습니다".to_string())
        })?;
        if path.is_empty() || !paths.insert(path.clone()) {
            return Err(HwpxError::UnsupportedEncryption(
                "manifest 암호화 경로가 비어 있거나 중복됩니다".to_string(),
            ));
        }
        let iterations = attribute(derivation, "iteration-count")
            .ok_or_else(|| {
                HwpxError::UnsupportedEncryption("manifest 반복 횟수가 없습니다".to_string())
            })?
            .parse::<u32>()
            .map_err(|_| {
                HwpxError::UnsupportedEncryption(
                    "manifest 반복 횟수가 올바르지 않습니다".to_string(),
                )
            })?;
        let key_size = attribute(derivation, "key-size")
            .ok_or_else(|| {
                HwpxError::UnsupportedEncryption("manifest 키 크기가 없습니다".to_string())
            })?
            .parse::<usize>()
            .map_err(|_| {
                HwpxError::UnsupportedEncryption("manifest 키 크기가 올바르지 않습니다".to_string())
            })?;
        let checksum_type = attribute(encryption, "checksum-type").unwrap_or_default();
        let algorithm_name = attribute(algorithm, "algorithm-name").unwrap_or_default();
        let start_key_algorithm =
            attribute(start_key, "start-key-generation-name").unwrap_or_default();
        let checksum = decode_base64(attribute(encryption, "checksum"), "checksum")?;
        let iv = decode_base64(
            attribute(algorithm, "initialisation-vector"),
            "initialisation-vector",
        )?;
        let salt = decode_base64(attribute(derivation, "salt"), "salt")?;

        if algorithm_name != AES_256_CBC
            || key_size != 32
            || iv.len() != 16
            || salt.is_empty()
            || iterations == 0
            || iterations > MAX_PBKDF2_ITERATIONS
            || start_key_algorithm != SHA_256_START_KEY
            || !checksum_type.ends_with(SHA_256_1K_SUFFIX)
            || checksum.len() != 32
        {
            return Err(HwpxError::UnsupportedEncryption(
                "AES-256-CBC / SHA-256 / PBKDF2 ODF 계약과 다릅니다".to_string(),
            ));
        }
        entries.push(EntryCrypto {
            path,
            checksum,
            iv,
            salt,
            iterations,
            key_size,
        });
    }

    Ok(entries)
}

fn derive_key(prf: Pbkdf2Prf, password: &[u8], entry: &EntryCrypto) -> Result<Vec<u8>, ()> {
    let start_key = Sha256::digest(password);
    let mut key = vec![0_u8; entry.key_size];
    let result = match prf {
        Pbkdf2Prf::HmacSha1 => {
            pbkdf2::<HmacSha1>(&start_key, &entry.salt, entry.iterations, &mut key)
        }
        Pbkdf2Prf::HmacSha256 => {
            pbkdf2::<HmacSha256>(&start_key, &entry.salt, entry.iterations, &mut key)
        }
    };
    result.map_err(|_| ())?;
    Ok(key)
}

fn plaintext_limit(path: &str) -> usize {
    if path.to_ascii_lowercase().starts_with("bindata/") {
        MAX_BINDATA_SIZE
    } else {
        MAX_XML_SIZE
    }
}

fn inflate_raw_deflate_limited(
    data: &[u8],
    max_bytes: usize,
) -> Result<Vec<u8>, EntryDecryptError> {
    let mut plaintext = Vec::new();
    let limit = (max_bytes as u64).saturating_add(1);
    DeflateDecoder::new(Cursor::new(data))
        .take(limit)
        .read_to_end(&mut plaintext)
        .map_err(|_| EntryDecryptError::Invalid)?;
    if plaintext.len() > max_bytes {
        return Err(EntryDecryptError::LimitExceeded { max_bytes });
    }
    Ok(plaintext)
}

fn decrypt_with_prf(
    prf: Pbkdf2Prf,
    password: &[u8],
    entry: &EntryCrypto,
    ciphertext: &[u8],
) -> Result<Vec<u8>, EntryDecryptError> {
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
        return Err(EntryDecryptError::Invalid);
    }
    let key = derive_key(prf, password, entry).map_err(|_| EntryDecryptError::Invalid)?;
    let mut blocks = ciphertext.to_vec();
    let decrypted = Decryptor::<Aes256>::new_from_slices(&key, &entry.iv)
        .map_err(|_| EntryDecryptError::Invalid)?
        // 한글의 CHncAES 호출은 모든 block을 그대로 raw-deflate에 전달한다.
        // ODF PKCS#7 제거를 적용하면 정상 HWPX가 손상된다.
        .decrypt_padded_mut::<NoPadding>(&mut blocks)
        .map_err(|_| EntryDecryptError::Invalid)?;
    let plaintext = inflate_raw_deflate_limited(decrypted, plaintext_limit(&entry.path))?;
    let checksum = Sha256::digest(&plaintext[..plaintext.len().min(1024)]);
    if checksum.as_slice() != entry.checksum.as_slice() {
        return Err(EntryDecryptError::Invalid);
    }
    Ok(plaintext)
}

fn decrypt_entry(
    password: &[u8],
    entry: &EntryCrypto,
    ciphertext: &[u8],
) -> Result<Vec<u8>, HwpxError> {
    for prf in [Pbkdf2Prf::HmacSha1, Pbkdf2Prf::HmacSha256] {
        match decrypt_with_prf(prf, password, entry, ciphertext) {
            Ok(plaintext) => return Ok(plaintext),
            Err(EntryDecryptError::LimitExceeded { max_bytes }) => {
                return Err(HwpxError::DecryptedEntryLimitExceeded {
                    path: entry.path.clone(),
                    max_bytes,
                });
            }
            Err(EntryDecryptError::Invalid) => {}
        }
    }
    Err(HwpxError::WrongPasswordOrCorruptPayload)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn strip_encryption_data(manifest: &[u8]) -> Result<Vec<u8>, HwpxError> {
    let mut reader = Reader::from_reader(manifest);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut skipped_depth = 0_u32;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == b"encryption-data" => {
                skipped_depth = 1;
            }
            Ok(Event::Start(_)) if skipped_depth > 0 => skipped_depth += 1,
            Ok(Event::End(_)) if skipped_depth > 1 => skipped_depth -= 1,
            Ok(Event::End(_)) if skipped_depth == 1 => skipped_depth = 0,
            Ok(Event::Empty(event)) if skipped_depth == 0 => writer
                .write_event(Event::Empty(event.into_owned()))
                .map_err(|_| HwpxError::XmlError("manifest 암호화 정보 제거 실패".to_string()))?,
            Ok(event) if skipped_depth == 0 => writer
                .write_event(event.into_owned())
                .map_err(|_| HwpxError::XmlError("manifest 암호화 정보 제거 실패".to_string()))?,
            Ok(_) => {}
            Err(_) => {
                return Err(HwpxError::XmlError(
                    "manifest 암호화 정보 제거 중 XML 오류".to_string(),
                ));
            }
        }
        buffer.clear();
    }
    Ok(writer.into_inner())
}

fn read_zip_entry_limited<R: Read>(
    reader: &mut R,
    path: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, HwpxError> {
    let mut data = Vec::new();
    reader
        .take((max_bytes as u64).saturating_add(1))
        .read_to_end(&mut data)
        .map_err(|error| HwpxError::ZipError(format!("{path} 읽기 실패: {error}")))?;
    if data.len() > max_bytes {
        return Err(HwpxError::ZipError(format!(
            "{path} 읽기 실패: HWPX entry exceeds {max_bytes} byte limit (possible decompression bomb)"
        )));
    }
    Ok(data)
}

/// 암호 HWPX만 메모리의 평문 ZIP으로 바꾼다. 평문 HWPX이면 `None`을 반환해
/// 기존 파서 경로가 바이트 단위로 변하지 않게 한다.
pub(super) fn decrypt_hwpx_package(
    data: &[u8],
    password: &[u8],
) -> Result<Option<Vec<u8>>, HwpxError> {
    let mut source = ZipArchive::new(Cursor::new(data.to_vec()))?;
    let manifest = match source.by_name(MANIFEST_PATH) {
        Ok(mut file) => read_zip_entry_limited(&mut file, MANIFEST_PATH, MAX_XML_SIZE)?,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(HwpxError::ZipError(error.to_string())),
    };
    let protected = parse_manifest(&manifest)?;
    if protected.is_empty() {
        return Ok(None);
    }
    let protected_by_path: HashMap<&str, &EntryCrypto> = protected
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let plain_manifest = strip_encryption_data(&manifest)?;
    let mut selected = HashSet::new();
    let mut destination = ZipWriter::new(Cursor::new(Vec::new()));

    for index in 0..source.len() {
        let mut input = source.by_index(index)?;
        let name = input.name().to_string();
        if input.is_dir() {
            destination
                .add_directory(name, SimpleFileOptions::default())
                .map_err(|error| HwpxError::ZipError(error.to_string()))?;
            continue;
        }

        // ZIP 내부 암호문 자체도 한 엔트리로 제한한다. 이어서 decrypt_entry가
        // raw-deflate 평문에도 XML/BinData 정책 상한을 다시 적용한다.
        let bytes = read_zip_entry_limited(&mut input, &name, MAX_BINDATA_SIZE)?;
        let source_method = input.compression();
        let (payload, method) = if name == MANIFEST_PATH {
            (plain_manifest.clone(), CompressionMethod::Deflated)
        } else if let Some(entry) = protected_by_path.get(name.as_str()) {
            selected.insert(entry.path.as_str());
            (
                decrypt_entry(password, entry, &bytes)?,
                CompressionMethod::Deflated,
            )
        } else {
            (bytes, source_method)
        };
        destination
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(method),
            )
            .map_err(|error| HwpxError::ZipError(error.to_string()))?;
        destination
            .write_all(&payload)
            .map_err(|error| HwpxError::ZipError(error.to_string()))?;
    }

    if selected.len() != protected.len() {
        let missing = protected
            .iter()
            .find(|entry| !selected.contains(entry.path.as_str()))
            .map(|entry| entry.path.as_str())
            .unwrap_or("알 수 없는 경로");
        return Err(HwpxError::MissingFile(format!(
            "암호화 manifest 항목이 ZIP에 없습니다: {missing}"
        )));
    }
    let output = destination
        .finish()
        .map_err(|error| HwpxError::ZipError(error.to_string()))?;
    Ok(Some(output.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_entry_limit_uses_existing_hwpx_reader_policy() {
        assert_eq!(plaintext_limit("Contents/section0.xml"), MAX_XML_SIZE);
        assert_eq!(plaintext_limit("BinData/image1.bmp"), MAX_BINDATA_SIZE);
    }

    #[test]
    fn raw_deflate_expansion_is_limited_before_materialization() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;

        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&vec![b'A'; 1025]).unwrap();
        let compressed = encoder.finish().unwrap();

        assert!(matches!(
            inflate_raw_deflate_limited(&compressed, 1024),
            Err(EntryDecryptError::LimitExceeded { max_bytes: 1024 })
        ));
    }
}
