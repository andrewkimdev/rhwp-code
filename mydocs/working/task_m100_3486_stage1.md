---
kind: investigation
status: active
canonical: mydocs/manual/verification/visual_verification_governance.md
last_verified: 2026-07-28
---

# Task #3486 Stage 1 — HWP3 암호 문서 렌더링 기준선

Issue: [#3486](https://github.com/edwardkim/rhwp/issues/3486)

## 문제 분리

- #3483은 HWP3 암호 해제와 문서 열기 경로를 검증했다. 그 증적 PNG는 문서가 열린 상태일 뿐,
  한컴 출력과의 조판 정합을 보증하지 않는다.
- 현재 관찰한 본문 흐름과 삽입 그림 위치의 큰 차이는 암호 처리 문제가 아니다. 복호화 뒤의
  HWP3 parser → 공통 IR → layout/renderer 경로를 별도 대상으로 조사·개선한다.

## 기준선과 판정 계획

1. `samples/HWP3-password-123456.hwp`의 SHA-256을 기록하고 HWP 2020 MCP로 기준 PDF를
   `pdf/` 아래에 생성한다. 서버 주소·토큰·비밀번호는 공개 기록에 남기지 않는다.
2. 기준 PDF와 rhwp의 SVG/PNG 출력을 visual sweep으로 페이지별 대조한다. 페이지 수, 본문
   줄 흐름, 그림 geometry를 분리해 후보를 좁힌다.
3. 유의미한 차이가 폰트 메트릭 차이인지 HWP3 parse/IR/layout 구조 결함인지 원인 경로를
   확정한 후 수정과 회귀 검증을 추가한다.
4. 수정 전후의 3-way/overlay 자료와 자동 지표는 PR asset으로 남기되, 최종 시각 판정은
   한컴 기준을 확인하는 작업지시자에게 요청한다.

## 입력 기준

- HWP3 fixture SHA-256:
  `db743d084efc9e08e839a5b4d978b16b8676434011776e090e4cda43e57304be`
- 기존 열기 증적:
  `mydocs/pr/assets/pr_3483_hwp3_password_open_review.png`
- 기존 증적은 기준 PDF 대조 자료가 아니므로, #3486의 renderer 수용 근거로 재사용하지 않는다.
