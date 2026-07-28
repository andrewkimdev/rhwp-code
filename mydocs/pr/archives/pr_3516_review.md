---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-28
---

# PR #3516 리뷰 — 암호 HWPX 열기와 HWP3 아래아 보존

- PR: [#3516](https://github.com/edwardkim/rhwp/pull/3516)
- Related issue: [#3486](https://github.com/edwardkim/rhwp/issues/3486) (이 PR은 close하지 않음)
- 역할: `jangster77` collaborator self-review

## 라우팅과 작성 시점

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, multi_pr_update_branch.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
  collaborator_self_merge.md, intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, multi_pr_update_branch.md
current head: 2b334aa22b503508bb86a4ff44cd98153b8cfb74
  (review 기록 추가 전, Update branch가 만든 참고 SHA)
```

이 PR은 collaborator 자신의 변경이다. 이 self-review는 독립 승인, 최신 head의 CI, 메인터너 검토를
대체하지 않는다. 최종 merge 조건은 review·오늘할일을 포함한 최신 head의 required check 성공,
`MERGEABLE` 상태 재확인, 메인터너 검토와 작업지시자 승인이다.

## PR metadata (작성 시점 참고값)

| 항목 | 값 |
| --- | --- |
| 작성자·검토 기록 작성자 | `jangster77` (collaborator self PR) |
| base → head | `devel` → `task_m100_3486_hwp3_render_fidelity` |
| 최초 source head | `bfe38e121afc2b53f90f82ca915adac3367b42ba` |
| base 동기화 참고 head | `2b334aa22b503508bb86a4ff44cd98153b8cfb74` |
| 최초 규모 | 18 files, +929 / -26 |
| mergeable / merge state | `MERGEABLE` / `BLOCKED` (최신 CI 전 참고값) |
| reviewer | `edwardkim` 요청됨 |
| 관련 이슈 | `#3486` 관련 — `Closes` 미사용 |

## 변경 범위와 수용 판단

1. HWP3 Johab 중성 인덱스 30(아래아)을 옛한글 자모열로 보존한다. 실제 암호 HWP3와 구조 대조용
   HWPX fixture에서 첫 제목 텍스트가 보존되는 회귀 계약을 추가했다.
2. ODF `encryption-data`의 AES-256-CBC, SHA-256 start key, PBKDF2, SHA-256-1k checksum 계약을
   쓰는 HWPX만 메모리에서 복호화한 뒤 기존 HWPX parser로 넘긴다. HMAC-SHA1·HMAC-SHA256 PBKDF2
   호환을 순서대로 확인하되, manifest의 PBKDF2 반복 횟수와 XML/BinData 평문 크기에 상한을 둔다.
3. 공개 Rust API·CLI·WASM `openWithPassword`를 같은 parser 경로에 연결하고, 실제 암호 HWPX와
   같은 문서의 평문 HWPX를 대조한다. 틀린 비밀번호와 손상 암호문은 구분하지 않는 일반 오류로 처리한다.
4. 소비자 API·CLI 문서의 HWP3/HWP5/HWPX 암호 입력 지원표를 현재 구현으로 갱신했다.

수용 판단은 **기본 복호화·파싱·공개 API 열기**에 한정하면 조건부 수용 가능이다. 지원 계약 밖의 ODF
암호화 방식, 암호화 저장, 암호 HWPX용 Studio 입력 UI는 포함하지 않는다.

## 검증 기록

모든 Cargo 명령은 전용 `CARGO_TARGET_DIR=target/task_3486_hwpx_password*`와
`CARGO_INCREMENTAL=0`에서 순차 실행했다. 공유 target 경로는 제거하거나 검증 결과에 사용하지 않았다.

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| `cargo fmt --check`, `git diff --check` | passed | 형식·공백 오류 없음 |
| `cargo clippy --all-targets -- -D warnings` | passed | Rust lint 경고 없음 |
| HWPX crypto module unit test | 2 passed | 기존 HWPX XML/BinData 상한과 raw-deflate 확장 제한 확인 |
| 실제 `hwpx_password_fixture` | 2 passed | 미입력·오입력·정상 입력, 평문 대조, 공개 API·CLI stdin 계약 |
| 실제 `hwp3_password_fixture` | 2 passed | HWP3 아래아 보존 회귀와 기존 암호 열기 계약 |
| `cargo test --profile release-test --tests` | passed | 전체 Rust 회귀 및 IR field-sweep baseline 포함 |
| `wasm-pack build --target web --out-dir pkg` | passed | 새 WASM 패키지 생성 |
| npm tarball 설치 smoke | passed | 별도 임시 Node 프로젝트에서 생성 tarball 설치 후 HWP3 24쪽·암호 HWPX 23쪽을 `openWithPassword`로 열기 |

새 HWPX 두 파일은 `samples/` 루트의 구조·복호화 비교 fixture다. 현재 IR field-sweep의 HWPX corpus
수집 경로가 아니며, release-test의 baseline 검사는 통과했고 TSV 행을 추가하지 않았다. 이를 일반 HWPX
corpus로 이동하거나 승격하는 후속 변경에서는 전체 sweep과 baseline 재판정이 필요하다.

Native Skia 3종은 실행하지 않았다. native-skia backend·renderer 구현은 바꾸지 않았고, 이 PR은 아래의
명시된 비시각 수용 범위만 판정한다.

## 시각 검증 제외와 fixture 증적

**이 PR은 HWP3/HWPX의 한컴 PDF 대비 시각적 정합을 검증하거나 수용하지 않는다.** 기본 복호화 후
문서가 parser·CLI·WASM API에서 열리는지만 검증했다. 따라서 HWPX 기준 PDF, pixel match, visual sweep
asset, GitHub 이미지 comment를 이 PR의 merge 근거로 사용하지 않는다.

- 암호 HWP3 구조/조사 fixture: `samples/HWP3-password-123456.hwp`
  - SHA-256: `db743d084efc9e08e839a5b4d978b16b8676434011776e090e4cda43e57304be`
- HWP3 한컴 오라클 PDF: `pdf/HWP3-password-123456.pdf`
  - SHA-256: `3ced5ad95ad30331e2756b5b34509c1ac91dfe3c72013c8e14f2556ca6bd5776`
  - 역할: #3486 원인 조사 기준선. 이번 PR의 visual acceptance 증적이 아님.
- 암호 HWPX fixture: `samples/HWP5-password-123456.hwpx`
  - SHA-256: `93e7a62565e0f3efa4feee2812aaf518347dbbcc09d2f26a0d9385f9a4e26060`
- HWPX 평문 대조 fixture: `samples/HWP5-nopassword-123456.hwpx`
  - SHA-256: `20ed90f48c6501cad99f6aa1f82d81d2a2132eb04f2d1d32805ac251749e4d0e`

## 위험과 후속 보완

- #3486의 본문 흐름·폰트 메트릭·그림 배치와 한컴 PDF 정합은 남아 있다. 기존 HWP3 sweep은 조사
  기준선일 뿐, 이 PR이 renderer fidelity를 해결했다는 뜻이 아니다. #3486은 열린 상태로 유지한다.
- 암호 HWPX의 Studio 비밀번호 입력 UI와 브라우저 사용자 흐름은 이 PR에 없다. 현재 공개 API를 호출하는
  소비자가 비밀번호를 제공할 수 있는 기반만 추가했다.
- 이 구현은 명시한 ODF AES-256-CBC/PBKDF2 계약만 지원한다. 다른 ODF 암호화 profile·DRM·암호화
  저장은 명시적으로 지원 범위 밖이다.
- 복호화 패키지는 기존 lazy BinData parser에 넘기기 전에 메모리 ZIP으로 재구성된다. entry별 XML/BinData
  상한과 PBKDF2 반복 상한은 적용하지만, 대형 입력의 전체 프로세스 메모리 예산은 별도 하드닝 대상이다.

## 최종 권고

**조건부 merge 권고.** 실제 HWP3/HWPX fixture, Rust parser·CLI·WASM API와 생성 npm tarball에서 기본
복호화·열기를 확인했고, 허용 범위 안에서는 blocker를 발견하지 못했다. 단, 최신 review 기록 head의 CI,
mergeable 상태, 메인터너 검토와 작업지시자 승인을 다시 확인해야 하며, 시각적 품질을 근거로 merge를
요청하지 않는다.
