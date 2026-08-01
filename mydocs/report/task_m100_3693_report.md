# task_m100_3693 처리결과 보고서 — export-structure clause 정확도

- **Issue**: [#3693](https://github.com/edwardkim/rhwp/issues/3693)
- **상위 이슈**: [#1528](https://github.com/edwardkim/rhwp/issues/1528)
- **브랜치**: `codex/issue-3693-export-structure-clause`
- **기준 devel**: `79551f42f`
- **상태**: 로컬 구현·focused 검증 완료, PR 통합 대기

## 1. 결과

`export-structure --mode clause`의 확정 결함과 과검출을 함께 보정했다.

- `제1조의2` marker를 더 이상 `제1조`로 절단하지 않는다.
- `1)`/`가)` 괄호형 marker를 `호`/`목` 후보로 인식한다.
- standalone 숫자·한글 목록은 법령 부모 증거가 없으면 구조 노드로 만들지 않는다.
- 실제 협정서의 `조 → 항/호 → 목` 구조는 유지한다.

## 2. 설계

패턴 인식만 넓히면 일반 문서의 과검출이 증가한다. 따라서 `classify_clause()`는 marker 후보를 반환하고,
`build_structure()`가 현재 열린 clause stack을 보고 약한 후보를 채택한다. `호`는 `조|항`, `목`은 `호`
증거를 요구한다. 거부된 문단의 텍스트는 preamble/body로 보존되어 정보 손실이 없다.

이 정책은 명시적 clause 모드의 구조 정확도만 바꾸며 #3695의 auto/outline 선택 정책은 건드리지 않는다.

## 3. 변경 파일

- `src/document_core/queries/structure.rs`
  - 가지번호 marker와 괄호형 구분자
  - clause 부모 문맥 수용 함수
  - synthetic 단위 회귀 테스트
- `tests/issue_3693_structure_clause_context.rs`
  - 실제 협정서 positive
  - 실제 업무계획 날짜형 negative
  - 실제 편람 목차 번호 negative
- `mydocs/plans/task_m100_3693*.md`, `mydocs/working/task_m100_3693_stage1.md`
- `mydocs/orders/20260801.md`

## 4. 검증 결과

- structure 단위: 5 passed
- #3693 실문서 통합: 3 passed
- export-structure CLI JSON 계약: 4 passed
- fmt, clippy `-D warnings`, diff check: 통과

새 HWP/HWPX fixture와 golden/baseline 변경은 없다. 렌더 경로를 변경하지 않아 시각 검증 대상이 아니다.

## 5. 호환성과 남은 작업

- `StructureDoc`/`StructureNode` 필드, JSON 봉투, CLI exit code는 불변이다.
- strong marker와 명시적 outline mode는 불변이다.
- full release-test와 GitHub CI는 PR 승인 뒤 수행한다.
- 후속 #3695에서 auto mode confidence와 `HeadType::Number` 정책을 다룬다.
- #3693 변경이 `devel`에 포함되기 전에는 이슈를 close하지 않는다.
