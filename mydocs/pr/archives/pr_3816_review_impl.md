---
kind: plan
status: active
pr: 3816
---

# PR #3816 누적 통합 구현 기록

## 목적과 경계

Kevin의 열린 PR 13개를 하나의 최신 `devel` 기반 통합 후보로 검증한다. 원 contributor
branch를 변경하거나 원 commit을 rewrite하지 않고, 충돌 해소와 계약 보정은 `jangster77`의
통합 branch에 별도 commit으로 남긴다.

## 적용 순서

1. `upstream/devel` `f82bff4c0` 위에서 #3788, #3791, #3795, #3797, #3799, #3800,
   #3802, #3804, #3805, #3806, #3807, #3809 순으로 누적했다.
2. #3811을 마지막 보안 검사 축으로 적용했다.
3. 출처 표지, `inspect` 하위 명령의 capabilities/MCP, `extract-data` JSON 봉투를
   통합 보정 commit으로 정렬했다.
4. 최신 `upstream/devel` `3b0349a56`으로 rebase했다. 새 base commit은 문서 변경만으로,
   rebase 충돌은 없었다.
5. source PR의 GitHub head와 로컬 고정 ref를 비교했다. 13개 모두 일치했다.

## 통합 보정 근거

| 보정 | 이유 | 회귀 가드 |
| --- | --- | --- |
| `inspect` provenance map | `findings`의 문맥·표시·raw·hidden은 문서 파생값 | `provenance_contract` |
| `inspect --kind` capability | capability 합집합의 flag가 injection 축에 잘못 전달되면 usage error | `injection_scan_contract` |
| password MCP 배선 | 비밀값을 optional argv로 취급하면 프로세스 목록에 노출 | `unicode_deception_contract`, `mcp_password_contract` |
| provenance sweep recipes | 새 JSON 명령을 실제 문서로 실행하지 않으면 출처 선언 drift를 발견할 수 없음 | `provenance_contract` |

## 남은 순서

1. review/오늘할일 commit을 통합 branch에 push한다.
2. 새 head의 GitHub Actions 전체 완료를 확인한다. 코드·테스트 보정이 있으므로
   review-only fast-pass를 적용하지 않는다.
3. merge 직전에 `upstream/devel`, PR head SHA, source PR head, mergeability를 재확인한다.
4. 작업지시자 승인 뒤 draft 해제, merge, 원 PR 후속 상태 정리 및 작업 트리 정리를 수행한다.

## 롤백

통합 PR을 닫아도 원 PR과 원 contributor branch는 그대로 남는다. 기능별 분리가 필요하면
이 통합 branch의 해당 source commit 범위를 새 `devel` 기준 branch에 다시 cherry-pick한다.
