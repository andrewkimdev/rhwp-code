---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3825 검토 - 텍스트 순서·글리프 겹침 POC

## 접수와 적용

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3825](https://github.com/edwardkim/rhwp/pull/3825) / @planet6897 |
| 원 head | 747ba744c709331a8ad4031aca11b742ce2b4326 |
| base / 작성 시점 상태 | devel / MERGEABLE, BEHIND |
| 규모 | 1,207 additions / 5 files — 대형 PR 예외 경로 적용 |
| 누적 순서와 적용 | 1: 142515ba → ab5256827, 2: 747ba744 → e77402d4 |
| 충돌 / 의존성 | 없음 / 독립 POC |

PDF 콘텐츠 stream의 읽기 순서와 렌더 SVG의 한글 glyph advance를 각각 검사하는 POC다.
프로덕션 renderer 동작은 바꾸지 않으며, 기존 페이지 수·픽셀 비교가 보지 못하는 복사·검색
순서 및 글자 뭉침을 별도 축으로 계측한다.

## 검토와 보정

glob이 Windows 역슬래시 패턴에 의존해 macOS/Linux에서 입력을 놓치던 부분을 maintainer commit
26b94dc70에서 os.path.join 기반 재귀 glob으로 보정했다. 또한 glyph 왕복 비교기가 exporter
실패를 차이 0처럼 보이게 하던 경로를 종료 코드 2의 명시적 오류로 바꿨다. TSV의 고정 열 마지막
빈값과 보관 patch의 유효한 빈 hunk context에는 해당 경로만 whitespace 검사에서 제외했다.
이는 원 contributor POC와 구분된 maintainer 보정이다.

## 검증

- scan_overlap smoke: exam-kor-4p.hwp 4쪽, 비교 쌍 1, overprint 0 / crush 0.
- scan_order smoke: 격리 PyMuPDF 1.28.0 환경에서 같은 4쪽, 비교 쌍 8,974, inversion 0.
- glyph_roundtrip_compare: 2022년 국립국어원 업무계획 HWP → HWPX 35/35쪽,
  23,994 glyph 차이 0.
- 실패 executable(/usr/bin/false)에서는 오류를 출력하고 종료 코드 2를 반환했다.
- Python py_compile, cargo fmt --check, diff --check, clippy -D warnings을 통과했다.

text-order POC는 PyMuPDF가 필요하다. 스크립트는 미설치 시 명시적으로 중단하며, 본 검토에서는
임시 격리 환경에서 재현했다. 300문서 TSV는 기여자의 측정 snapshot이며 독립 corpus 재실행
근거로 과장하지 않는다.

## 판정

**누적 통합 수용.** 이식성·실패 판정 보정을 포함해 POC가 macOS에서 동작하고, renderer
출력의 새 관찰 축을 제공한다. 최신 integration PR에는 이 문서와 통합 기록을 함께 둔다.

