---
kind: pr-review
status: local-pass
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-12
---

# PR #4647 검토 - 문서 열기 압축 해제 예산

## 판정

로컬 수용. 완전 문서 열기 경계가 HWP5 DocInfo·본문의 누적 예산과 HWP3 압축 본문 상한을 선택하며,
strict·lenient·비밀번호·배포용 ViewText 경로가 같은 명시적 제한을 전달한다. 하위 CFB/crypto API는
전역 제품 정책을 숨기지 않고 caller 제공 상한만 집행한다.

## 검토 기준

- 원격 head: `a274e67e782480c84adee25ffbfab28d559f4356`
- 로컬 누적 검토 브랜치: `review/humdrum00001010-20260812`
- 적용 순서: #4646 다음에 #4647의 6개 commit을 적용했다.
- 충돌 해소: #4646의 thumbnail 회귀를 유지한 채 `src/parser/mod.rs`에 누적 예산 회귀를 추가했다.

## 확인

- `cargo test --profile release-test --lib open_decompression_`: 10 passed.
- 통합 전체 Rust 회귀: 5,906 passed, 37 skipped.
- `cargo clippy --all-targets -- -D warnings`, doctest, `cargo fmt --check`, `git diff --check` 통과.

## 범위

제한 초과는 빈 섹션이나 preview fallback으로 본문을 계속 열지 않고 명시적 오류가 된다. 별도 BinData와
thumbnail의 정책은 이 문서 열기 누적 예산에 섞지 않으며, raw-record diagnostics는 자체 이름 붙은
제한을 선택한다.
