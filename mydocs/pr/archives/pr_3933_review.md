# PR #3933 검토 — clause 문맥 confidence 경계

- **PR**: [#3933](https://github.com/edwardkim/rhwp/pull/3933)
- **Issue**: [#3744](https://github.com/edwardkim/rhwp/issues/3744)
- **상위 이슈**: [#1528](https://github.com/edwardkim/rhwp/issues/1528)
- **작성자**: `postmelee` (collaborator self PR)
- **base / head**: `devel` / `task_m100_3744`
- **review 보정 commit**: `dacab077c`
- **최신 결합 devel**: `301d0fe5f`
- **작성 시점 상태**: draft; review 문서 commit·push 전
- **작성 시점 기능 diff**: 11 files, +1,692 / -12

## 1. 절차 경로

- base route: `collaborator_self_merge.md`
- modifiers: `intake_and_review.md`, `local_validation.md`, `rework_and_exceptions.md` 대형 PR 경로
- loaded documents: `pr_review_workflow.md`, `pr_review/README.md`, 위 기본·보조 문서
- visual modifier: 비대상. query-only 변경이며 renderer/layout/paint, sample, golden을 바꾸지 않는다.

PR 작성자와 로그인한 collaborator가 같아 자기 자신을 reviewer로 요청하지 않았다. 최초 review 뒤
제품 보정이 추가됐으므로 구현 대조는 `pr_3933_review_impl.md`로 분리했다. 이 문서 commit과 push가
head를 다시 바꾸므로 mergeable·CI·head SHA는 push 뒤 최신 값을 확인해야 한다.

## 2. 범위와 최신 base

#3744는 #1528의 세 번째 하위 작업으로, #3693 clause marker/stack과 #3695 auto 선택 뒤 남은 explicit
clause confidence 경계를 다룬다.

- 오래된 `조|항` anchor 뒤 SQL·일반 숫자 목록 `호` 오탐 제거
- 조문 안 `YYYY. M. D[.]` 날짜 `호` 오탐 제거와 body 보존
- `장|절` 아래 실제 `가.`~`하.` direct `목` 회복
- review에서 확인된 복합 번호 뒤 정상 목록 회복, dotted-leader TOC 거부, shape 하한 경계 명문화
- 영구 회귀 11건과 단계별 계획·근거·corpus 보고

원격 head `ce6a23bca` 뒤 새 `upstream/devel` `301d0fe5f`를 merge commit `c610a0b1a`로 충돌 없이
결합했다. 그 사이 upstream은 #3744 제품·테스트 파일을 바꾸지 않았다. 공개 JSON 필드·봉투,
CLI option·exit code, explicit outline, auto selector, parser, serializer, renderer는 범위 밖이다.

## 3. review 지적과 보정 판정

| review 항목 | 보정 | 판정 |
| --- | --- | --- |
| 복합 번호가 이후 정상 `호`도 영구 차단 | 같은 section의 경계 바로 다음 문단에서만 경계 앞 번호 또는 직전 정상 번호+1 복귀 | 해결 |
| dotted leader TOC가 direct `목`으로 승격 가능 | `.`, `·`, `‥`, `…` leader 3점 이상 + 뒤쪽 숫자를 TOC tail로 거부 | 해결 |
| shape negative 경계 부족 | `indent=-1280` 허용, `-1281`·nonzero margin·nested level 거부와 body 보존 | 해결 |
| `-1280` magic number | `DIRECT_MOK_MIN_INDENT_HWPUNIT` 상수와 약 -4.52 mm/corpus 근거 주석 | 해결 |

날짜 마지막 점 생략형, explicit clause corpus 측정, auto가 clause를 선택했을 때 결과를 상속한다는
문서 의미도 수행계획서와 결과 보고서에 보정했다. GitHub comment 원문은 수정하지 않았다.

## 4. corpus 영향과 기각안

번호만 맞으면 만료 anchor를 재개하는 최초 보정안은 sample10 세 변형에서 node 8→1,145,
`호` 4→1,141로 대량 오탐을 되살려 기각했다. 최종 인접 문단 정책은 보정 전 head `ce6a23bca`와
recursive 673개(670 parse)를 비교했을 때 파일별 node/kind 결과가 모두 같고 기존 password parse
실패 3개도 동일했다.

따라서 Stage 4의 제품 변화—오래된 anchor 제거 6문서와 실제 direct 제목 회복 5문서—는 유지되고,
review 보정으로 설명되지 않는 corpus 변화는 없다. 측정 대상은 auto가 아니라 명시적
`StructureMode::Clause` 출력이다.

## 5. 로컬 검증

review 보정 후보 `ce6a23bca` 기준으로 다음을 순차 통과했다.

| 게이트 | 결과 |
| --- | --- |
| structure 단위 | 8 passed |
| `issue_3744_structure_clause_confidence` | 11 passed |
| `issue_3693_structure_clause_context` | 3 passed |
| `issue_3695_structure_auto_policy` | 13 passed |
| `cli_json_contract export_structure_` | 4 passed |
| `cargo test --profile release-test --tests` | lib 3,200 passed / 7 ignored; 전체 target exit 0 |
| `cargo fmt --check` / `git diff --check` | 통과 |
| `cargo clippy --all-targets -- -D warnings` | 통과 |

최신 `upstream/devel` `301d0fe5f` 결합 뒤 focused 5개 gate도 8/11/3/13/4로 다시 통과했다. 결합
head의 전체 검증은 push 뒤 GitHub Actions를 최종 merge gate로 사용한다. 시각 검증은 structure query
분류만 바뀌고 renderer/layout/paint·fixture에 영향이 없어 생략했다.

## 6. 규모와 risk

1,000줄을 넘지만 제품 코드는 `structure.rs`의 제한된 내부 gate이고, 나머지는 회귀 테스트와
하이퍼-워터폴 계획·단계·최종·review 근거다. 대형 PR 예외 규칙에 따라 latest-head CI 없이는 merge하지
않는다.

핵심 risk는 정상 반복 목록의 과도한 제거와 direct `목`의 일반 목록 오인이다. 인접 복귀 조건,
dotted-leader negative, 전체 corpus 무회귀, #3693·#3695 테스트가 해당 위험을 제한한다. Oracle 경계
전 네 `호`, 더 깊은 shape의 direct `목`, 실제 달력 유효성은 승인된 잔여 trade-off다.

## 7. merge 권고와 경계

로컬 구현·독립 대조 결과는 **merge 가능 후보**다. 다만 이 review 문서 commit을 포함해 push된 최신
head의 required GitHub Actions가 모두 통과해야 한다. CI 통과 뒤 merge 실행자는 작업지시자이며,
이번 단계에서는 GitHub comment/review, ready 전환, merge를 수행하지 않는다.

merge 뒤 PR merge comment, #3744 close comment, 상위 #1528의 세 하위 작업 통합 검증·종료 여부는
별도 후속 절차에서 처리한다.
