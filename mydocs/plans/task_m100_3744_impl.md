# 구현계획서 — task_m100_3744

- **이슈**: #3744
- **상위 이슈**: #1528
- **브랜치**: `codex/issue-3744-clause-context-confidence`
- **수행계획서**: `mydocs/plans/task_m100_3744.md`
- **기준 commit**: `upstream/devel` `0889974a01db3585df8ad2c1f13203e3cb9f51f8`
- **절차 상태**: Stage 3 구현·focused 검증 완료, Stage 4 승인 대기
- **다음 승인 경계**: 승인 C — corpus 영향·전체 release 검증 승인

## 1. 변경 경계

주 변경점은 `src/document_core/queries/structure.rs`의 explicit clause weak-marker 채택 경로다.
`classify_clause()`의 marker 문법과 공개 `StructureNode`/`StructureDoc` 형태는 유지한다.

예상 변경 단위는 다음과 같다.

1. 문단 위치·텍스트·ParaShape와 열린 계층을 담는 비공개 clause evidence/context
2. 날짜형 `호` negative 판정 helper
3. `호` anchor 만료/연속성 판정 helper
4. `장|절` 아래 `목` confidence와 TOC tail 판정 helper
5. `build_structure()`의 section/paragraph 순회 중 context 갱신

정확한 필드와 helper 이름은 Stage 2 정책 선택 뒤 고정한다. 공개 API나 직렬화 필드는 추가하지 않는다.

## 2. Stage 2 — red 고정과 정책 선택

### 2.1 영구 회귀 테스트

새 통합 테스트 `tests/issue_3744_structure_clause_confidence.rs`에 최소 다음을 둔다.

1. `제1조 → ① → 일반 body 간격 → 1)`에서 승인된 만료 경계 뒤 후보가 body로 남는 synthetic negative
2. `hwp3-sample10.hwp` 문단 2303·2312·2313 SQL negative와 문단 2269·2270 anchor 존재 확인
3. `제1조(목적) → 2022. 1. 1. 일부개정` 날짜 negative와 body 보존 단언
4. `제1장 → 제1절 → 가. 본문 제목` direct `목` synthetic positive
5. `제1장 → 가. 개요\t9` TOC negative와 body 보존 단언
6. 편람에서 발견 코멘트와 일치한 44개 shape 후보의 대표 좌표·positive 가설과 목차 대표 negative
7. 협정서 `조 → 호 → 목` 및 #3693 괄호형 marker 무회귀

숫자 총량만 단언하지 않고 대표 좌표, marker, parent kind, body 보존을 함께 확인해 공허한 통과를 막는다.

### 2.2 앵커 정책 비교

각 후보를 제품 코드에 바로 넣지 않고 조사 helper 또는 test-local evaluator로 비교한다.

- section reset 단독
- 마지막 `조|항`에서의 거리 상한 후보
- 첫 번호·연속 번호·중단 후 재시작 신호
- section reset + 거리/연속성 조합

각 후보마다 다음을 표로 남긴다.

- `hwp3-sample10.hwp`의 1,228 `호` 중 제거/잔존 수와 SQL 3좌표 판정
- `hwp3-sample16-hwp5.hwp` 정상 조문 `호` 보존
- top-level 351개와 recursive 668개 sample의 문서별 `호` 증감
- false positive로 설명할 수 없는 감소 목록

최소 감소율 자체를 목표로 하지 않는다. 정상 조문을 보존하면서 명시한 false positive를 결정적으로
설명하는 정책을 선택한다.

### 2.3 `목` evidence 비교

편람의 broad-allow 후보 128개를 현재 계층, `margin_left`, `indent`, `para_level`, `head_type`,
탭+쪽번호 tail별로 분류한다.

- 발견 코멘트의 44개와 일치한 `(0, 0, 0)` 41개 + `(0, -1280, 0)` 3개는 positive 가설이다.
- style/shape ID는 문서 로컬 식별자이므로 판정에 쓰지 않는다.
- 탭+숫자 tail은 weak marker negative 후보로 평가하되 strong marker에는 blanket 적용하지 않는다.
- 다른 sample에서 같은 shape가 목차·일반 목록에 쓰이는지 반드시 역대조한다.

선택 규칙과 기각한 대안을 Stage 2 보고서 및 수행계획서 4절에 반영한 뒤 승인 B를 받는다.

### 2.4 Stage 2 선택 결과

1. anchor: `N.N` 복합 번호를 body로 거부하고 현재 nearest `조|항`의 weak-`호` 상태를 만료한다.
2. date: 유효 범위의 `YYYY. M. D.`를 suffix와 무관하게 거부한다.
3. direct `목`: TOC tail이 없고 `margin_left=0`, `indent>=-1280`, `para_level=0`인 문단만
   열린 `장|절` 조상 아래에서 허용한다.
4. section reset, 거리 cap, blanket strict sequence는 편람의 정상 반복 목록 손실 때문에 구현하지 않는다.

세부 실측과 잔여 trade-off는 `mydocs/working/task_m100_3744_stage2.md`가 canonical 근거다.

## 3. Stage 3 — 구현 (완료)

### 3.1 context 전달

`clause_heading_allowed()`가 다음 중 승인된 최소 evidence만 받도록 확장한다.

- 원문 `para_text`
- section/paragraph 위치
- `ParaShape`의 문서 독립 속성
- 열린 clause stack
- 마지막 strong/weak anchor와 번호 진행 상태

context는 `build_structure()` 내부 순회에서 갱신하고 section 전환, strong heading 채택, weak 후보 거부
시점의 상태 전이를 단위 테스트로 고정한다.

구현은 공개 context 구조를 추가하지 않고 `ClauseGateState`가 nearest `조|항`의 `(section,
paragraph)` 식별자별 만료 상태만 보관하도록 제한했다. 새 strong anchor는 새 식별자를 사용하므로 이전
anchor의 만료가 전파되지 않는다.

### 3.2 날짜 gate

- 선두 연도 4자리, 월·일 1~2자리와 점 구분자를 파싱한다.
- 월 1~12, 일 1~31 범위를 확인해 일반 `1. 항목`과 분리한다.
- 열린 `조|항`이 있어도 날짜 후보는 heading으로 채택하지 않고 기존 body 경로로 보낸다.
- `일부개정` 같은 특정 suffix가 없어도 같은 날짜 문법은 일관되게 판정한다.

### 3.3 anchor 만료와 `목` confidence

- marker가 `.`으로 끝나고 marker 직후 숫자가 이어지는 `N.N` 후보를 거부하고 현재 nearest
  `조|항`의 weak-`호` 상태를 만료한다.
- 만료는 같은 anchor 아래 후속 weak `호`에만 적용하고 새 `조|항`에서 초기화한다.
- `)` marker 뒤 본문 숫자, 날짜 거부, 일반 body는 anchor를 만료하지 않는다.
- strong 편·장·절·관·조와 원문자 항 분류를 바꾸지 않는다.
- 기존 열린 `호` 아래 `목`은 보존한다.
- `장|절` 아래 `목`은 열린 `호`가 없고 TOC tail이 없으며 ParaShape가
  `margin_left=0`, `indent>=-1280`, `para_level=0`일 때만 허용한다.
- 거부 문단은 삭제하지 않고 현재 node body 또는 preamble에 그대로 보존한다.

## 4. Stage 3 focused 검증 (완료)

- `CARGO_INCREMENTAL=0 cargo test --lib document_core::queries::structure -- --nocapture`
- `CARGO_INCREMENTAL=0 cargo test --test issue_3744_structure_clause_confidence -- --nocapture`
- `CARGO_INCREMENTAL=0 cargo test --test issue_3693_structure_clause_context -- --nocapture`
- `CARGO_INCREMENTAL=0 cargo test --test issue_3695_structure_auto_policy -- --nocapture`
- `CARGO_INCREMENTAL=0 cargo test --test cli_json_contract export_structure_ -- --nocapture`

red→green 결과와 focused 실측은 `mydocs/working/task_m100_3744_stage3.md`에 기록했다. Stage 4의
전체 corpus·release 측정은 아직 실행하지 않았다.

## 5. Stage 4 corpus·release 검증

1. Stage 3 도중 갱신된 최신 `upstream/devel`에 작업 커밋을 동기화하고 focused green을 재확인한다.
2. 동일 입력을 기준/보정 evaluator로 비교해 top-level 351개와 recursive 668개 영향표를 만든다.
3. mode, node_count, kind별 증감, 대표 좌표를 기록하고 각 변화에 의도 근거를 붙인다.
4. 공개 JSON key와 CLI 정상/오류 exit code가 바뀌지 않았음을 기존 계약 테스트로 확인한다.
5. 다음 게이트를 순차 실행한다.

- `CARGO_INCREMENTAL=0 cargo test --profile release-test --tests`
- `CARGO_INCREMENTAL=0 cargo fmt --check`
- `git diff --check`
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets -- -D warnings`

최종 보고서와 필요 최소 CLI 문서를 갱신하고, 원격 작업 승인 전 로컬 커밋에서 중지한다.

## 6. 금지 사항

- 파일명·section/paragraph 좌표·para shape ID를 제품 판정에 하드코딩하지 않는다.
- sample 하나의 44건을 맞추기 위한 임의 threshold를 채택하지 않는다.
- #3695 auto selector와 explicit outline 동작을 함께 리팩터링하지 않는다.
- parser/render/serializer 변경과 시각 fixture 추가를 이 이슈에 섞지 않는다.
- 승인 B 전 제품 소스를 구현하지 않고, 승인 D 전 push·PR·GitHub comment를 수행하지 않는다.
