---
kind: analysis
status: active
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-04
---

# Task #3820·#3821 Stage 6 — p118→p119 그림 앞 문단 owner drift 자동 후보화 분석

## 사용자-visible 재현과 정답 기준

같은 `정책연구용역사업 중간진도보고서`의 사용자 쪽번호 118→119 경계에서 rhwp는 그림 앞
본문의 마지막 절을 p118에 계속 남기고 p119에서 절차 그림을 시작한다. 한컴 2020 기준 PDF는
그 절의 뒷부분(`기록되어야 함. 동의 취득 회의록은 …`)을 p119 상단에 먼저 배치한 뒤 같은
절차 그림을 둔다. 즉 그림 자체의 누락이나 그림 위 문자 충돌이 아니라, **TopAndBottom 그림
앞에서 본문 paragraph owner가 한 페이지 이르게 확정되는 page-boundary fidelity 결함**이다.

비교 기준은 `pdf/pr3740/hwp/정책연구용역사업 중간진도보고서(살아있는 간장 기증자의 의학적
선별기준 연구)-2020.pdf`의 사용자 p118/p119이고, 입력은 동명의 `samples/*.hwp`다. fidelity
도구에는 0-based page 117/118로 넘긴다.

## 기존 자동 판정의 예상 범위

Stage 4의 `square_wrap_text_overlap`은 그림의 물리 box를 본문이 가로지르거나 edge에 맞닿는
경우만 다룬다. 이번 증상은 서로 다른 물리 페이지의 text owner와 TopAndBottom 그림 순서가
어긋난 것이므로 이 규칙으로는 찾을 수 없다.

반면 `fidelity_compare --text-only --export-all-svg --layout-ledger`는 PDF↔SVG 인접 페이지의
reciprocal text difference와 16자 이상 순서 보존 문자열을 `text-owner-*-candidates.tsv`에
기록한다. 먼저 이 기존 ledger가 p118→p119의 문단 절 이동을 실제로 후보화하는지 확인한다.
기존 원장만으로도 확인되면 중복 detector를 만들지 않는다.

## 수용 기준과 다음 단계

1. direct-pair text-only 전수 export에서 p118→p119에 `rhwp_earlier_than_reference` owner
   후보와 이동한 실제 본문 문자열이 남는지 확인한다.
2. candidate가 없거나 짧은 문단·문자 Counter 상쇄로 놓치면, 인접 page의 Body text sequence와
   successor-page TopAndBottom 그림을 결합한 별도 `float_owner_shift` 후보를 fidelity ledger에
   추가한다. 그림 존재만으로 결함으로 판정하지 않고 PDF owner 차이가 함께 있을 때만 낸다.
3. 후보는 PDF 시각 review를 요구하는 triage 신호이며, 자동 불합격·전역 page-break 보정의
   근거로 사용하지 않는다.

이 분석 문서를 커밋한 뒤에만 fidelity 도구·test·사용 문서를 수정한다.
