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
6. CI가 발견한 Node binding 표면 누락을 보정했다. capability 생성 봉투를 31개로
   재생성하고 `export-provenance-map`, 표 CSV 입출력, `extract-data`, 세 `inspect`
   하위 명령의 래퍼와 argv 테스트를 추가했다.
7. stale run 취소 workflow가 종료된 run의 409을 실패로 처리하던 경합을 보정했다.
   대상 run 재조회 결과가 `completed`일 때만 취소 성공으로 간주한다.

## 통합 보정 근거

| 보정 | 이유 | 회귀 가드 |
| --- | --- | --- |
| `inspect` provenance map | `findings`의 문맥·표시·raw·hidden은 문서 파생값 | `provenance_contract` |
| `inspect --kind` capability | capability 합집합의 flag가 injection 축에 잘못 전달되면 usage error | `injection_scan_contract` |
| password MCP 배선 | 비밀값을 optional argv로 취급하면 프로세스 목록에 노출 | `unicode_deception_contract`, `mcp_password_contract` |
| provenance sweep recipes | 새 JSON 명령을 실제 문서로 실행하지 않으면 출처 선언 drift를 발견할 수 없음 | `provenance_contract` |
| Node binding parity | capability에 있는 명령을 Node 표면에서 누락하면 빌드 CI가 실패 | `gen:check`, `commands.test`, `parity.integration.test` |
| stale run 409 | 목록 조회 뒤 대상 run이 끝나도 reaper가 실패하면 새 head 판단이 혼동됨 | workflow 재조회 및 JavaScript 구문 검사 |

## 남은 순서

1. 로컬 Node binding 검증(`gen:check`, `typecheck`, 439 tests, build, pack dry-run)과
   stale run workflow 구문 검증을 완료했다.
2. 이 보정 commit을 통합 branch에 push하고 새 head의 GitHub Actions 전체 완료를 확인한다. 코드·테스트 보정이 있으므로
   review-only fast-pass를 적용하지 않는다.
3. merge 직전에 `upstream/devel`, PR head SHA, source PR head, mergeability를 재확인한다.
4. 작업지시자 승인 뒤 draft 해제, merge, 원 PR 후속 상태 정리 및 작업 트리 정리를 수행한다.

## 롤백

통합 PR을 닫아도 원 PR과 원 contributor branch는 그대로 남는다. 기능별 분리가 필요하면
이 통합 branch의 해당 source commit 범위를 새 `devel` 기준 branch에 다시 cherry-pick한다.
