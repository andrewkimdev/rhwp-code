---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# PR #3829 검토 - PDF ToUnicode PUA 텍스트 표면 보정

## 접수와 적용

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3829](https://github.com/edwardkim/rhwp/pull/3829) / @planet6897 |
| 관련 이슈 | [#3824](https://github.com/edwardkim/rhwp/issues/3824) |
| 원 head / 적용 | 9bfac9c6b57fdbf4db807795e5c207608f6692ee / 3fe575ac7 |
| base / 작성 시점 상태 | devel / MERGEABLE, BEHIND |
| 규모 / 충돌 | 178 additions, 1 deletion, 1 file / 없음 |

PDF가 화면에 그리는 glyph를 바꾸지 않고, 생성 직후 ToUnicode CMap의 한컴 PUA 원숫자만
표준 Unicode 표면으로 다시 매핑한다. 원래 코드 길이를 보존하고 CMap block 밖 바이트를
건드리지 않아 PDF object offset 계약을 유지한다.

## 검증

- renderer::pdf ToUnicode unit 3 / 3 통과: 길이 보존, 비대상 보존, block 경계 보존.
- issue_3385b_text_surface_full_pua 3 / 3 통과.
- samples/pua-test.hwp를 PDF로 실제 export했다: 1쪽, 49,597 bytes, PDF 1.7.
  pdftotext 결과는 Plane 15 PUA 0개이며 텍스트 1,101자를 추출했다.
- 변경은 콘텐츠 stream glyph paint가 아니라 ToUnicode 사후 표면에만 있으므로, PDF 래스터
  외관을 바꾸지 않는 단위 경계와 실제 PDF text extraction을 함께 근거로 삼았다.

## 판정

**누적 통합 수용.** 읽기·검색·복사 가능성을 고치되 PDF 그림 결과를 바꾸지 않는 최소 변경이며,
관련 PUA text-surface 회귀와 실제 추출 검증을 통과했다.

