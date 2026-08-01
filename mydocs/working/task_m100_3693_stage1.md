# task_m100_3693 Stage 1 완료보고서 — clause marker·문맥 정확도

- **Issue**: #3693
- **상위 이슈**: #1528
- **브랜치**: `codex/issue-3693-export-structure-clause`
- **기준**: `upstream/devel` `79551f42f`
- **완료 시각**: 2026-08-01 18:44 KST

## 1. red 기준

수정 전에 신규 회귀 테스트로 현재 결함을 고정했다.

| 테스트 | 수정 전 결과 |
| --- | --- |
| `제1조의2` marker 전체 보존 | 실패: `제1조` 반환 |
| `1)`/`가)` 후보 검출 | 실패: `None` |
| synthetic `조 → 항 → 호 → 목` | 실패: 4개 기대, 2개 검출 |
| standalone 번호 후보 거부 | 실패: 0개 기대, 2개 검출 |
| 실제 업무계획 날짜형 `2022. 1.` 거부 | 실패: `호`로 검출 |
| 실제 협정서 `제1조 → 1./2./3.` | 기존에도 통과 |

단위 focused 실행은 1 passed / 4 failed, 실문서 통합은 1 passed / 1 failed였다.

## 2. 구현

- `제N조` 뒤의 선택적 `의M` suffix를 marker에 포함했다.
- 숫자/한글 marker의 `.`와 `)`를 모두 후보 구분자로 인식했다.
- 텍스트 후보와 구조 채택을 분리했다.
  - `호`: 열린 `조|항` 문맥에서만 채택
  - `목`: 열린 `호` 문맥에서만 채택
  - strong marker(`편|장|절|관|조|항`)는 기존처럼 채택
- 문맥에서 거부된 문단은 삭제하지 않고 기존 preamble/body 경로에 보존했다.

## 3. 실제 sample 검증

| sample | 기준선 | 수정 후 |
| --- | --- | --- |
| `samples/2022년 국립국어원 업무계획.hwp` | 날짜형 `2022. 1.`을 `호` 1개로 검출 | clause node 0개 |
| `samples/2025 행정업무운영 편람(최종).hwp` 목차 | 장·절 아래 `1.` 목차를 `호`로 검출 | 장·절은 보존, 목차 `1.`은 비노드 |
| `samples/hwp3-sample16-hwp5.hwp` 협정서 | `제1조` 아래 `1.`~`3.` | 같은 `조 → 호` 구조 보존 |

편람 본문의 실제 괄호형 목록은 부모 문맥 안에서 `1)`/`가)`로 검출된다. 새 sample은 추가하지 않았고
기존 재배포 가능 코퍼스만 사용했다.

## 4. green 검증

| 명령 | 결과 |
| --- | --- |
| `cargo test --lib document_core::queries::structure` | 5 passed |
| `cargo test --test issue_3693_structure_clause_context` | 3 passed |
| `cargo test --test cli_json_contract export_structure_` | 4 passed |
| `cargo fmt --check` | 통과 |
| `cargo clippy --all-targets -- -D warnings` | 통과 |
| `git diff --check` | 통과 |

모든 Cargo 실행은 `CARGO_INCREMENTAL=0`으로 순차 실행했다. 파서·렌더·레이아웃·직렬화와 JSON shape는
변경하지 않았다.

## 5. 다음 단계

- 현재 단계 변경을 #3693 커밋으로 고정한다.
- PR 전체 release-test와 PR 생성은 별도 승인 게이트에서 진행한다.
- #3695는 이 커밋을 선행 기준으로 별도 브랜치에서 시작한다.
