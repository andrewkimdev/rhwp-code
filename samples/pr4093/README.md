# PR #4093 개요 탐색 확인 자료

PR #4093(VS Code 개요 탐색 패널) 리뷰 지적을 재현·검증하는 HWPX 자료를 모아 둔 자리다.

## outline_navigation_table_cell_number.hwpx — 표 셀 번호 경계

- Fixture: `outline_navigation_table_cell_number.hwpx`
- 생성기: `scripts/generate_outline_navigation_fixture.py` (결정적 — zip 타임스탬프 고정)
- 기준 문서: `samples/hwpx/ref/ref_empty.hwpx` (한컴 2020 이 저장한 빈 문서)
- 기준 문서 SHA-256: `c58144645069f7d1258e91404730618ad568bc4d47680ad5f891d3050aa308c7`
- Fixture SHA-256: `a033627d1c48817201bee8edd1b2d71b8c49a407af97346b6f1c7fb912602b1c`
- 변환: 기준 문서의 머리말에 paraPr 20 번(개요와 같은 번호 정의 id 1 을 쓰는 `NUMBER` 문단
  모양) 하나를 추가하고, 본문 문단 5개를 합성했다. 글꼴·번호 정의·구역 설정은 기준 문서
  그대로다. 실문서 내용은 들어 있지 않다.
- 담은 경계 (PR #4093 리뷰 지적):
  1. 개요 계층 — `개요`(수준 1, `1.`) 아래 `목적`(수준 2, `가.`).
  2. 표 셀 번호 문단 — 앞 개요와 뒤 개요 사이의 표 셀에 같은 번호 정의를 쓰는 `NUMBER`
     문단이 있어 뒤 개요는 `3.` 이다. 셀 문단을 카운터에 반영하지 않는 구현은 `2.` 를 낸다.
  3. 정규식 과발동 유혹 — 개요 속성 없이 `1. 일반 본문` 텍스트로 시작하는 문단. 탐색
     목록에 나오면 안 된다.
- 소비 테스트: `tests/outline_navigation_table_cell_number.rs` (탐색 질의 번호 ↔ 렌더된 SVG
  번호 대조)

## 게이트 주의

`samples/` 아래는 overflow-cell 원장(`tests/overflow_cell_baseline.rs`)의 전수 대상이다.
이 디렉터리의 fixture 를 추가·교체하면 그 게이트를 함께 돌려 신규 행이 없는지 확인한다
(`mydocs/manual/pr_review/local_validation.md` 4.3.1). IR field sweep 의 HWPX lane 은
`samples/hwpx/` 만 훑으므로 이 디렉터리는 대상이 아니다.
