---
kind: implementation-plan
status: active
issue: 3604
last_verified: 2026-08-01
---

# #3604 후속 구현 계획: 공통 비밀번호 암호 모듈

## 모듈 경계

`src/password_crypto.rs`는 다음을 소유한다.

- HWP3 UTF-16LE 비밀번호 키 유도, DES-ECB, raw-DEFLATE/CRC32/ISIZE 검증과 암호화
- HWP5 `EncryptVersion=4` SHA-1 키 유도와 AES bit-CFB 스트림 암·복호화
- HWPX SHA-256 start key, PBKDF2-HMAC-SHA1, AES-256-CBC no-padding, raw-DEFLATE,
  ODF manifest `encryption-data` 조립과 검증

parser와 serializer는 파일 형식의 레이아웃, CFB/ZIP 읽기·쓰기를 담당하고 위 모듈에
평문·암호문 바이트와 비밀번호만 넘긴다. 포맷별 crypto 파일에는 알고리즘 구현을 두지
않는다.

## Stage 4 변경

1. 공통 오류형과 HWP5 공개 암호 스트림 함수를 도입한다.
2. HWP5 serializer가 FileHeader의 암호 플래그/버전과 DocInfo, BodyText, BinData, 추가
   스트림의 암호문을 생성하도록 별도 저장 entrypoint를 추가한다.
3. parser의 기존 HWP5 복호화 호출을 공통 모듈로 바꾼다.
4. 암호 HWP5 저장 후 일반 열기 거부, 올바른 비밀번호 재열기, 오류 비밀번호 거부,
   embedded BinData와 extra stream 보존을 검증한다.

## Stage 4 결과

- HWP5 key derivation과 AES bit-CFB는 `src/password_crypto.rs`로 이관했다.
- `src/parser/crypto.rs`에는 배포용 ViewText 특수 암호 처리와 공통 모듈 adapter만
  남겼다.
- HWP5 저장 entrypoint는 `serialize_hwp_with_password()`로 추가했다. 아직 public CLI와
  MCP에는 노출하지 않는다.

## Stage 5 결과

1. `src/password_crypto.rs`가 HWP3 UTF-16LE/DES-ECB/raw-DEFLATE/CRC32/ISIZE,
   HWP5 SHA-1/AES bit-CFB, HWPX ODF manifest/SHA-256/PBKDF2/AES-256-CBC를 모두
   소유하도록 이관했다.
2. HWP3 parser와 HWPX parser의 기존 crypto 파일은 기존 오류 유형으로 바꾸는 얇은
   adapter만 남겼다.
3. `encrypt_hwp3_password_document()`는 압축되지 않은 raw HWP3를 password
   payload로 만들고, `encrypt_hwpx_package()`는 한컴 ODF 보호 대상 엔트리를
   raw-DEFLATE 후 AES-256-CBC no-padding으로 암호화해 ZIP `Stored`로 기록한다.
4. `serialize_hwpx_with_password()`를 추가했다. salt와 IV는 `getrandom`의
   운영체제 난수로 생성하며 password·파생 키는 함수 밖에 보관하지 않는다.
5. HWP3 실제 password fixture 재암호화/재열기, HWPX 실제 평문 fixture 암호화,
   manifest/Stored 검사, 올바른·오류 비밀번호 재열기를 테스트로 고정했다.

## Stage 6 예정

- Hancom Office 2020의 직접 열기·저장 경로로 새 HWPX/HWP5 보호 산출물의
  외부 호환성을 교차 검증한다.
- 그 결과가 고정된 뒤에만 CLI와 MCP의 암호 저장 입력을 노출한다.

## 보안 규칙

- HWP5 암호화 알고리즘은 한글 파일 호환용 legacy 형식이다. 새 난수나 salt가 없는
  format 계약을 임의로 추가하지 않는다.
- HWPX는 salt와 IV에 운영체제 난수를 사용한다. deterministic fixture는 테스트 전용
  난수 주입으로만 만든다.
- password와 파생 키는 함수 범위 밖에 보존하지 않고, 오류 문자열에도 포함하지 않는다.
