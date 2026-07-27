---
kind: pr_review
status: active
canonical: mydocs/pr/archives/pr_3405_review.md
last_verified: 2026-07-27
---

# PR #3405 검토 — HWP5 비밀번호 암호 문서 복호화

Issue: #3474 (후속 UI 열기 경로)

base route: collaborator_external_pr
modifiers: intake_and_review, local_validation, visual_fixture_evidence, rework_and_exceptions

## 1. Metadata (작성 시점 참고값, merge 전 재확인)

| 항목 | 값 |
|---|---|
| PR | #3405 — `feat(crypto): HWP5 비밀번호 암호 문서 복호화 지원` |
| 작성자 | `scari` (external contributor) |
| base / 원 head | `devel` `7995786bf` / `5a41caff10` |
| 규모 | 원 PR +1,679 / -149, 15 파일, 기능 commit 1개 + devel merge 2개 |
| mergeable / 상태 | 작성 시점 `MERGEABLE` / `UNSTABLE`; merge 직전 재확인 필요 |
| 관련 이슈 | #1946은 이미 closed이며, 본 PR은 이를 닫지 않는다고 명시한다. UI 후속은 #3474 |
| reviewer | `jangster77` assign 완료 |
| collaborator 보정 | `ecdfd9ca4` (원 head 위 별도 commit, 아직 remote push 안 함) |

`maintainerCanModify=true`를 source head, `ls-remote`, local fetch의 동일 SHA로 확인한 뒤
`review/pr3405-maintainer`에서만 보정을 준비했다. 원 contributor commit은 rewrite하지 않았다.

## 2. 변경 범위와 보정

원 변경은 HWP5 `EncryptVersion 4`의 키 파생·CFB 복호화, DocInfo/BodyText/BinData/Scripts 처리,
Rust·WASM·CLI 공개 경로, 평문 HWP 저장, 라이선스 고지를 추가한다.

collaborator 보정 `ecdfd9ca4`은 다음만 추가·정리한다.

- 작업지시자 지시에 따라 maintainer 전용 최상위 `CHANGELOG.md`, `README.md`, `README_EN.md`의
  PR 추가 내용을 제거했다. 상세 사용법은 변경 범위의 component 문서에 남긴다.
- 실제 HWP5 암호 fixture `samples/hwp3-sample16-hwp5-2024-password.hwp`를 유지하고,
  정답·오답·미입력, Rust 공개 API, CLI stdin, 일반 HWP 저장 후 재파싱을 고정하는
  `tests/hwp5_password_fixture.rs`를 추가했다. fixture SHA-256은
  `59d4bed335b9552fe78fa68d2a56f7cfa3d586bcdeaaba839af80df13f3e08dc`다.
- 일반 HWP5 roundtrip gate는 비밀번호 입력을 받지 않으므로 암호 fixture를 자동 제외하고, 전용
  fixture test가 책임진다는 범위와 근거를 `tests/hwp5_roundtrip_baseline.rs` 및 가이드에 기록했다.
  IR field-sweep TSV는 암호 fixture가 일반 no-password sweep에서 제외되어 신규 baseline 행이 없다.

## 3. 시각·fixture 판정

새 HWP fixture와 WASM 공개 API가 있어 fixture 경로를 확인했다. 다만 renderer/layout/paint,
페이지 배치 또는 시각 충실도는 변경하지 않고 그런 주장을 하지 않는다. 실제 fixture를 CLI로 SVG
64페이지까지 내보낸 결과는 parser 경로 smoke evidence일 뿐, 기준 PDF와의 visual sweep 또는 merge
판정 근거로 사용하지 않았다. 따라서 대표 PNG·기준 PDF를 새로 만들지 않는다.

## 4. 검증

모든 Cargo 검증은 `CARGO_TARGET_DIR=target/review-scari-hwp5-password-20260727`,
`CARGO_INCREMENTAL=0`에서 수행했다. 공유 `target` 경로는 건드리지 않았다.

| 검증 | 결과 |
|---|---|
| 최신 `upstream/devel` 위 merge simulation | `--no-commit --no-ff` 충돌 0, abort 후 clean |
| `cargo fmt --check`, `git diff --check` | 통과 |
| `cargo clippy --all-targets -- -D warnings` | 통과 |
| `cargo check --target wasm32-unknown-unknown --lib` | 통과 |
| 실제 fixture 회귀 (`hwp5_password_fixture`) | 2 passed — 열기·오답·CLI exit contract·평문 저장 재열기 |
| HWP5 roundtrip baseline | 3 passed |
| IR field sweep baseline | 2 passed, baseline diff 0 |
| `cargo test --profile release-test --tests` | 최종 `ecdfd9ca4` 기준 exit 0 |
| 원 PR GitHub CI | CI 모든 실행 check 성공. CodeQL Action의 언어 분석도 성공했으나 아래 security check는 실패 상태 |

## 5. 발견한 차단 사유

### 5.1 CodeQL security check 실패

원 head의 CodeQL 분석은 완료됐지만 PR aggregate의 `CodeQL` check는 실패다. 현재 PR merge ref에
새 open alert 57건이 있다.

- `src/main.rs`와 `src/renderer/pdf.rs`의 무관한 오류 출력으로 이어지는
  `rust/cleartext-logging` 41건: 전역 옵션 pre-scan의 반환 tuple 전체를 비밀번호 값으로 taint한
  것으로 보인다. 실제로 `--password` 토큰을 제거한다는 구현과 정적 분석 모델의 정밀도를 재검증해야 한다.
- parser test vector의 `rust/hard-coded-cryptographic-value` 16건: test-only 공개 벡터인지와
  suppression/dismissal 정책을 security reviewer가 판정해야 한다. 심각도 표기를 이유로 무검토
  dismiss하지 않는다.

CodeQL failure가 branch protection을 만족하지 않으므로, 원인을 구조적으로 해소하거나 근거 있는
security 예외 처리가 완료되기 전에는 merge하지 않는다.

### 5.2 압축 해제 제한 주장과 구현 범위 불일치

PR 본문은 "복호화·압축 해제 크기 제한"을 말하지만 `decode_encrypted_stream_limited()`의 상한은
lazy BinData `resolve_limited()`에만 적용된다. 필수 `DocInfo`와 `BodyText`는
`decrypt_password_protected()`에서 제한 없는 `decompress_stream()`을 호출한다. 알려진 비밀번호를
가진 악성 압축 문서는 이 경로에서 메모리를 과도하게 사용하게 할 수 있다.

수용 전에는 (a) DocInfo·BodyText·즉시 materialize BinData까지 일관된 상한과 초과 회귀 테스트를
구현하거나, (b) 기능 범위를 축소하고 PR 본문의 제한 주장을 삭제한 뒤 보안 정책상 허용 여부를
명시적으로 승인받아야 한다. 현재 상태에서 (b)를 임의로 선택하지 않는다.

## 6. 최종 권고

**보류.** 실제 fixture, API·CLI·저장 경로와 Rust 전체 회귀는 통과했고 maintainer 범위 정리도
준비됐다. 그러나 최신 CodeQL security check 실패와 핵심 스트림의 압축 해제 상한 불일치가 남아 있다.

merge 전 조건:

1. 5.1 CodeQL alert의 구조적 수정 또는 security reviewer의 근거 있는 처리와 최신 head 재분석 성공
2. 5.2의 전 스트림 상한 구현·회귀 또는 작업지시자의 명시적 보안 범위 결정
3. source head가 다시 바뀌지 않았음을 확인한 뒤 collaborator 보정 commit·review 기록을 push하고,
   최신 head full CI와 작업지시자 승인을 재확인
