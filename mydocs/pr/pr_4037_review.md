---
kind: reference
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-06
---

# PR #4037 검토 기록 — 에이전트 자동화 도구킷 메인터너 보정

## 메타

| 항목 | 값 |
| --- | --- |
| 원 PR | [#4037](https://github.com/edwardkim/rhwp/pull/4037) |
| 작성자 | `kevin9327` |
| base | `devel` |
| 원 head | `48b41ca9c9303967d6333d2f161c9b798b6acba0` (보정 전, 작성 시점 참고값) |
| 규모 | 12 파일, +1,615/-0; 대형 PR(1,000줄 초과) |
| 검토 branch | `review/kevin9327-4037-20260806` |
| 최신 devel | `d722c1161e03a0bf58beebc7b0d9c638f7a52b8b` (병합 시뮬레이션 기준) |

원 head는 최신 `devel`의 조상이 아니었으나, 최신 `devel` 위 병합 시뮬레이션은 충돌 없이
구성됐다. 렌더러·레이아웃·fixture·기준 PDF 변경은 없으므로 시각 증적 경로는 적용하지 않았다.

## 발견 사항과 보정

| 우선순위 | 발견 사항 | 메인터너 보정 |
| --- | --- | --- |
| 높음 | `form_filling`과 `table_harvest`가 검증 실패 때 기존 `-o` 산출물도 삭제할 수 있었다. | 공통 출력 충돌 검사를 추가하고, 이번 호출이 새로 확보한 경로만 정리하도록 변경했다. |
| 높음 | `bulk_sweep`이 레코드 없이 `batch` 프로세스가 실패해도 exit 0을 보고했다. | 레코드 없는 비정상 batch 종료를 `batchFailures`로 기록하고 exit 1로 승격했다. |
| 중간 | `archive_search --report`가 최종 `exit`를 넣기 전에 파일을 저장했다. | 종료 판정 뒤에 보고서를 저장하고 batch 종료 코드·stderr를 함께 기록했다. |
| 중간 | 도구킷의 실제 rhwp 회귀 21건이 GitHub CI에서 실행되지 않았다. | 기존 Lint job에서 release-test `rhwp`를 빌드한 뒤 도구킷 회귀를 실행하도록 추가했다. |
| 낮음 | PR 본문은 구현·테스트가 후속 PR이라고 설명하지만 실제 head에는 구현과 회귀가 포함돼 있었다. | 코드 보정과 별도로 PR 본문을 현재 범위에 맞게 갱신해야 한다. |

출력 충돌 계약은 모든 워크플로에 일관되게 적용했다. 기존 파일·보고서·계획된 CSV·NDJSON이
있으면 exit 2로 중단하며, 기존 `bulk_sweep` 폴더는 비충돌 파일에 한해 계속 사용할 수 있다.

## 로컬 검증

| 검증 | 결과 |
| --- | --- |
| 최신 devel 병합 시뮬레이션 | 충돌 없음 |
| `cargo build --profile release-test --bin rhwp` | 실행 완료 후 도구킷 실바이너리 검증에 사용 |
| `RHWP_BIN=…/rhwp python3 tools/agent-toolkit/tests/test_workflows.py` | 27건 통과 |
| `node --test scripts/tests/ci-impact-classifier.test.cjs` | 27건 통과 |
| `python3 -m unittest scripts/tests/test_ci_impact_workflow.py` | 18건 통과 |
| `python3 -m py_compile …` 및 `git diff --check` | 통과 |

## 최종 권고

**메인터너 보정 commit을 원 PR head에 추가한 뒤 최신 head의 GitHub Actions를 재검증한다.**
보정 commit은 기존 출력 보호, batch 실패 계약, 보고서 계약, 회귀 CI 연결만 포함한다.
GitHub push·PR 본문 수정·merge·comment는 이 문서 작성 시점에 수행하지 않았다.

실행 순서와 rollback은 [PR #4037 implementation 계획](pr_4037_review_impl.md)을 따른다.
