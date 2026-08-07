---
kind: investigation
status: in_progress
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-08
---

# Task #3820 Stage 57 — 최신 devel 리베이스 후 PR 게이트

## 목적

Stage 56에서 바로잡은 issue2007 p11-p13 제목 소유권을 포함한 전체 변경을 최신
`upstream/devel` 위의 정확한 PR 후보로 확정한다. 이 단계는 로컬 검증과 PR 본문 준비까지
수행하며, 원격 push와 PR 생성은 사용자 승인 뒤에 수행한다.

## 리베이스

- 이전 기준: `d9c530ee8ed4bd0830ff35bc47e552bb0f32274f`
- 최신 기준: `5a4f26d0d0a4e2fc96f4b73510d2aecdad916722`
- upstream 신규: 12 commits
- 작업 커밋: 76 commits
- 결과: 충돌 없이 완료, ahead 76 / behind 0 / clean

upstream 신규 변경은 오라클 보고서와 도구 문서 정합 보정이며 renderer 파일과 직접
겹치지 않았다. 그래도 검증은 리베이스된 정확한 HEAD에서 다시 수행한다.

## 검증 순서

1. 전용 `CARGO_TARGET_DIR=target/task-3820-stage57-exact-head`에서 release-test 바이너리 빌드
2. issue2007 focused 15개 회귀와 p11-p13 144dpi PDF 재대조
3. `cargo build --release`
4. `cargo test --release --lib`
5. `cargo test --profile release-test --tests` — 최종 summary까지 대기
6. Native Skia 공식 회귀 3종
7. `cargo fmt --all -- --check`, `git diff --check`
8. `cargo clippy --all-targets -- -D warnings`
9. `cargo test --doc`
10. `wasm-pack build --target web --out-dir pkg`

전체 integration은 장시간 걸리는 것이 정상이며 중간 무출력만으로 종료하지 않는다.
다른 작업의 빌드 산출물은 지우지 않고 이 단계 전용 target만 사용한다.

## 완료 조건

- issue2007: 17쪽, p11 `[168,223)`, p12 `[223,271)`, p13 `[271,282)` 유지
- p11에는 exact 제목 `중앙선거관리위원회`가 없고 p12에는 존재
- focused 및 전체 회귀 실패 0
- fmt, diff-check, Clippy, rustdoc, Native Skia, WASM gate 통과
- 최종 SHA와 명령별 결과를 본 문서와 PR 본문 초안에 반영
