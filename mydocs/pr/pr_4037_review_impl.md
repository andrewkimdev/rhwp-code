---
kind: reference
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-06
---

# PR #4037 implementation 계획 — 도구킷 출력·실패 계약 보정

## 적용 순서

1. 원 head `48b41ca9c`를 유지한 `review/kevin9327-4037-20260806`에서 공통 출력 충돌 검사를 추가한다.
2. `form_filling`과 `table_harvest`는 새 호출의 산출물만 정리하고, 기존 산출물은 exit 2로 보존한다.
3. `archive_search`는 최종 exit와 batch 정보를 포함한 보고서를 저장하고, `bulk_sweep`은 레코드 없는
   batch 실패를 `batchFailures`로 분리한다.
4. 기존 출력 보존 5건, batch 프로세스 실패 1건, 보고서 종료 코드 검증을 추가해 회귀를 21건에서
   27건으로 확장한다.
5. 단순 보조 도구의 회귀는 CI에 넣지 않고, 실제 release-test `rhwp`로 로컬 실행한다. README·가이드의
   출력·실패 계약을 갱신한다.
6. 로컬 검증을 완료한 일반 commit을 만든다. 원 PR source branch로 push한 뒤 최신 head의 CI를 확인한다.
7. CI 통과 뒤 review 기록·오늘할일 후속 기록과 PR 본문 정합을 확인하고, 작업지시자 승인 뒤 merge한다.

## rollback

- 출력 계약·batch 처리·회귀는 하나의 메인터너 보정 commit으로 되돌릴 수 있다.
- rollback 시 기존 PR 구현 commit `48b41ca9c`은 보존되며, 보정 commit만 역순 revert한다.
- review 문서만 archive로 이동하거나 제거할 때도 코드·CI 보정과 분리된 후속 commit으로 처리한다.
