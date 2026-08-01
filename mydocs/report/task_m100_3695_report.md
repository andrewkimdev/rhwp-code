# task_m100_3695 처리결과 보고서 — export-structure auto 선택 정책

- **Issue**: [#3695](https://github.com/edwardkim/rhwp/issues/3695)
- **상위 이슈**: [#1528](https://github.com/edwardkim/rhwp/issues/1528)
- **선행 작업**: [#3693](https://github.com/edwardkim/rhwp/issues/3693), [PR #3715](https://github.com/edwardkim/rhwp/pull/3715), merge commit `fe9749d542f46643e408c23878229c326e341363`
- **브랜치**: `codex/issue-3695-export-structure-auto`
- **상태**: 최신 `devel` 통합·PR 전 전체 검증 완료, 원격 push·PR 승인 대기

## 0. 절차 복구 결과

초기 파일은 #3693의 승인된 단계 완료와 #3695 계획 승인 전에 구현 커밋과 함께 작성돼 WIP
스냅샷으로 재분류했다. 이후 #3693 선행 게이트를 완료하고 #3695 정정 계획 승인 체크포인트
`3de8b1709`을 고정한 뒤, `task_m100_3695_stage2.md`에서 기존 결론을 전제로 삼지 않은 독립 대조와
focused 재검증을 수행했다. 작업지시자가 그 결과를 검토해 `8343c98c6` 채택을 승인했으므로 이 문서를
#3695 최종 결과보고서로 확정한다.

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
- `mydocs/working/task_m100_3695_stage2.md`, `mydocs/working/task_m100_3695_stage3.md`
- `mydocs/orders/20260801.md`, `mydocs/orders/20260802.md`

## 4. 검증 결과

- structure 단위: 6 passed
- #3695 auto 정책 통합: 8 passed
- #3693 clause 실문서 회귀: 3 passed
- export-structure CLI JSON 계약: 4 passed
- 전체 release-test: 406 test binaries, 4,480 passed / 0 failed / 26 ignored
- fmt, clippy `-D warnings`, diff check: 통과

모든 Cargo 실행은 `CARGO_INCREMENTAL=0`으로 순차 수행했다. 신규 테스트의 red 기준은 5 passed /
1 failed였고, selector 적용 뒤 8건 전체가 통과했다.

Stage 2에서 같은 focused 게이트를 다시 실행해 모두 통과했고, `8343c98c6..HEAD`의 구현 파일 차이가
0임을 확인했다. 계획 대비 12개 항목도 모두 충족으로 판정했다.

Stage 3에서 `upstream/devel` `fe9749d54`를 WIP 감사 이력을 유지하는 merge 방식으로 통합했다. 소스
충돌은 없었고, 오늘할일 문서 한 곳만 양쪽의 2026-08-01 종료 시점 기록을 보존해 해결했다. 최신 devel
대비 net diff는 기존 #3695의 9개 파일·593 insertions/19 deletions 범위와 일치하며, #3715의 clause
marker·문맥 테스트와 #3695 selector 테스트가 focused·전체 프로필에서 함께 통과했다.

## 5. 호환성과 남은 작업

- `8343c98c6`은 작업지시자 승인으로 #3695 구현에 채택됐다.
- PR 전 full release-test는 최신 `devel` 통합 트리에서 완료했다.
- 원격 push·PR 생성과 GitHub CI는 다음 승인 뒤 수행한다.
- #3695를 merge해도 후속 #3744와 최종 통합 검증 전에는 상위 #1528을 close하지 않는다.
