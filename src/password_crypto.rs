//! HWP3, HWP5, HWPX 비밀번호 보호의 공통 암호 primitive.
//!
//! 포맷 parser와 serializer는 컨테이너·레이아웃만 담당한다. 이 모듈은 비밀번호
//! 키 유도와 암호문 변환을 단일 위치에 둔다. 현재 HWP5 stream 경로를 먼저
//! 연결했으며 HWP3·HWPX package 경로를 같은 파일에 순차적으로 이관한다.

use aes::cipher::{Block, BlockEncrypt, KeyInit};
use aes::Aes128;
use sha1::{Digest, Sha1};

/// 한글 7.0 이후 HWP5 비밀번호 보호의 FileHeader EncryptVersion 값.
pub const HWP5_ENCRYPT_VERSION: u32 = 4;

/// HWP5 비밀번호에서 AES-128 키를 유도한다.
///
/// 한글 호환 형식은 password 바이트와 직전 바이트의 1-bit rotate 값을
/// interleave한 뒤 SHA-1 앞 16바이트를 사용한다.
fn derive_hwp5_key(password: &[u8]) -> [u8; 16] {
    let mut input = vec![0_u8; password.len() * 2];
    for (index, &byte) in password.iter().enumerate() {
        let previous = if index == 0 {
            0xec
        } else {
            password[index - 1]
        };
        input[index * 2] = previous.rotate_left(1);
        input[index * 2 + 1] = byte;
    }
    let digest = Sha1::digest(input);
    let mut key = [0_u8; 16];
    key.copy_from_slice(&digest[..16]);
    key
}

fn hwp5_padded(data: &[u8]) -> Vec<u8> {
    let remainder = data.len() % 16;
    if remainder == 0 {
        return data.to_vec();
    }
    let padding = 16 - remainder;
    let mut output = data.to_vec();
    output.resize(data.len() + padding, padding as u8);
    output
}

fn hwp5_shift_register(register: &mut [u8; 16], feedback_bit: u8) {
    for index in 0..15 {
        register[index] = (register[index] << 1) | (register[index + 1] >> 7);
    }
    register[15] = (register[15] << 1) | (feedback_bit & 1);
}

fn aes_msb(cipher: &Aes128, register: &[u8; 16]) -> u8 {
    let mut block = Block::<Aes128>::clone_from_slice(register);
    cipher.encrypt_block(&mut block);
    block[0] >> 7
}

/// HWP5 EncryptVersion 4 stream을 복호화한다.
///
/// HWP5의 비트 단위 CFB는 마지막 미완성 AES block도 16바이트로 확장해 처리한 뒤
/// 원래 길이로 자른다. 압축 해제는 호출자가 stream의 포맷 속성에 맞춰 수행한다.
pub fn decrypt_hwp5_stream(ciphertext: &[u8], password: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new_from_slice(&derive_hwp5_key(password)).expect("AES-128 key size");
    let padded = hwp5_padded(ciphertext);
    let mut register = [0_u8; 16];
    let mut plaintext = Vec::with_capacity(ciphertext.len());

    for block in padded.chunks_exact(16) {
        let mut output = [0_u8; 16];
        for bit_index in 0..128 {
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            let ciphertext_bit = (block[byte_index] >> (7 - bit_offset)) & 1;
            let plaintext_bit = ciphertext_bit ^ aes_msb(&cipher, &register);
            hwp5_shift_register(&mut register, ciphertext_bit);
            output[byte_index] |= plaintext_bit << (7 - bit_offset);
        }
        plaintext.extend_from_slice(&output);
    }
    plaintext.truncate(ciphertext.len());
    plaintext
}

/// HWP5 EncryptVersion 4 stream을 암호화한다.
pub fn encrypt_hwp5_stream(plaintext: &[u8], password: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new_from_slice(&derive_hwp5_key(password)).expect("AES-128 key size");
    let padded = hwp5_padded(plaintext);
    let mut register = [0_u8; 16];
    let mut ciphertext = Vec::with_capacity(plaintext.len());

    for block in padded.chunks_exact(16) {
        let mut output = [0_u8; 16];
        for bit_index in 0..128 {
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            let plaintext_bit = (block[byte_index] >> (7 - bit_offset)) & 1;
            let ciphertext_bit = plaintext_bit ^ aes_msb(&cipher, &register);
            hwp5_shift_register(&mut register, ciphertext_bit);
            output[byte_index] |= ciphertext_bit << (7 - bit_offset);
        }
        ciphertext.extend_from_slice(&output);
    }
    ciphertext.truncate(plaintext.len());
    ciphertext
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hwp5_stream_roundtrip_preserves_partial_block() {
        for plaintext in [b"HWP5 password".as_slice(), b"0123456789abcdef".as_slice()] {
            let ciphertext = encrypt_hwp5_stream(plaintext, b"password");
            assert_ne!(ciphertext, plaintext);
            assert_eq!(decrypt_hwp5_stream(&ciphertext, b"password"), plaintext);
        }
    }

    #[test]
    fn hwp5_decrypt_matches_external_vector() {
        let ciphertext: Vec<u8> = (0_u8..32).collect();
        assert_eq!(
            decrypt_hwp5_stream(&ciphertext, b"helloworld"),
            [
                0x00, 0x01, 0x3e, 0xec, 0x90, 0x3d, 0xbc, 0x26, 0xfa, 0xff, 0x9c, 0x6c, 0xfb, 0x35,
                0x48, 0x00, 0xbc, 0xaa, 0x14, 0x7b, 0x0e, 0xd1, 0x5c, 0x32, 0x21, 0x17, 0x37, 0xfa,
                0x97, 0x1d, 0xe3, 0x79,
            ]
        );
    }
}
