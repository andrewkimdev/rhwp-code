---
kind: investigation
status: in_progress
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-08
---

# Task #3820 Stage 63 — issue2007 최종 PR 게이트

## 기준 상태

- 기준 브랜치: `upstream/devel`
- 기준 SHA: `fcc3b2135fa782699b66b583ddf11fe9f748306e`
- Stage 62 수정 SHA: `1a9e05356`
- 작업 브랜치: `task/3820-3821-fidelity`
- 상태: 최신 `upstream/devel` 위 ahead 87, behind 0, clean

Stage 62에서 issue2007 물리 p14의 terminal 중첩 표 뒤 과잉 여백을 PDF 기준으로
복원했다. 이 Stage에서는 코드를 더 넓히지 않고, 최신 기준 위 최종 결과를 새 전용
target에서 처음부터 검증한다.

## 검증 범위

1. issue2007과 같은 분할 표 경로의 focused 회귀
2. issue2007 p7–p17 페이지별 visual sweep 및 PDF 직접 대조
3. 전체 release-test integration과 overflow-cell baseline
4. Native Skia, fmt, clippy, rustdoc, Studio TypeScript·unit
5. 새 WASM 빌드와 브라우저 E2E
6. Markdown link, LFS, branch ancestry와 clean 상태

Cargo 검증은 `CARGO_INCREMENTAL=0`과
`CARGO_TARGET_DIR=target/task-3820-stage63-final-pr-gate`를 공통으로 사용한다.
`cargo test --profile release-test --tests`는 장시간 실행을 정상으로 보고 최종 exit
code와 summary가 나올 때까지 종료하지 않는다.

## 완료 조건

- 모든 공식 게이트 실패 0, clippy warning 0
- issue2007 전체 17쪽과 p7–p17의 requested/completed/missing 일치
- p12·p14·p15의 블록 간격, p16·p17 상단 continuation, 표 경계가 공식 PDF와 일치
- 최종 검증 SHA·바이너리 SHA-256·명령별 결과와 증적을 이 문서에 기록
- 오늘할일과 PR review 문서를 archive 상태로 같은 PR에 포함할 준비 완료
