# task_m100_3695 처리결과 보고서 — export-structure auto 선택 정책

- **Issue**: [#3695](https://github.com/edwardkim/rhwp/issues/3695)
- **상위 이슈**: [#1528](https://github.com/edwardkim/rhwp/issues/1528)
- **선행 작업**: [#3693](https://github.com/edwardkim/rhwp/issues/3693), commit `652e2ee27`
- **브랜치**: `codex/issue-3695-export-structure-auto`
- **상태**: 로컬 구현·focused 검증 완료, PR 통합 대기

## 1. 결과

`export-structure --mode auto`의 문서 단위 증거 우선순위를 명시적으로 정의했다.

- 명시적 `HeadType::Outline`은 authoritative outline이다.
- Outline이 없을 때 편·장·절·관·조 marker는 모호한 Number보다 우선해 clause를 선택한다.
- strong clause marker가 없으면 단일·복수 Number 문서 모두 기존처럼 outline을 선택한다.
- 증거가 없으면 기존과 같이 clause로 폴백한다.

따라서 일반 자동번호 문단 하나 때문에 조문 구조가 전부 사라지는 결함을 막으면서, Number 하나만 쓰는
실제 개요 문서도 보존한다.

## 2. 호환성

- explicit `--mode outline|clause` 경로는 불변이다.
- `StructureDoc`/`StructureNode`와 CLI JSON 봉투의 필드·shape는 불변이다.
- effective `mode` 값은 계속 `outline|clause` 중 하나이며 동일 입력에 결정적이다.
- #3693의 clause marker·stack 문맥 정책은 그대로 사용한다.
- 파서·렌더·레이아웃·직렬화 변경은 없어 시각 검증 대상이 아니다.

## 3. 변경 파일

- `src/document_core/queries/structure.rs`
  - `select_auto_mode()`와 증거 우선순위
- `tests/issue_3695_structure_auto_policy.rs`
  - synthetic 혼합/pure/explicit 경계
  - 실제 Outline·단일/복수 Number sample 회귀
- `mydocs/manual/cli_commands.md`
  - auto 선택 정책 설명
- `mydocs/plans/task_m100_3695*.md`, `mydocs/working/task_m100_3695_stage1.md`
- `mydocs/orders/20260801.md`

## 4. 검증 결과

- structure 단위: 5 passed
- #3695 auto 정책 통합: 8 passed
- #3693 clause 실문서 회귀: 3 passed
- export-structure CLI JSON 계약: 4 passed
- fmt, clippy `-D warnings`, diff check: 통과

모든 Cargo 실행은 `CARGO_INCREMENTAL=0`으로 순차 수행했다. 신규 테스트의 red 기준은 5 passed /
1 failed였고, selector 적용 뒤 8건 전체가 통과했다.

## 5. 남은 작업

- 현재 변경을 #3695 커밋으로 고정하고 이슈에 결과를 공유한다.
- full release-test와 GitHub CI는 PR 승인 뒤 수행한다.
- #3693·#3695가 `devel`에 포함되기 전에는 이슈와 상위 #1528을 close하지 않는다.
