# PR #3933 review 구현 대조 — review 보정

- **PR**: #3933
- **Issue**: #3744
- **base route**: collaborator self-merge
- **modifiers**: intake, local validation, large-PR rework
- **보정 전 원격 head**: `ce6a23bca`
- **최신 devel 결합 commit**: `c610a0b1a`
- **제품·task 보정 commit**: `dacab077c`
- **review 문서 tail**: 이 문서를 포함한 후속 commit; push 뒤 SHA 재확인

## 1. 계획과 구현 대조

| 정책 | 구현 근거 | 독립 판정 |
| --- | --- | --- |
| stale anchor 만료 | nearest `조|항`별 `expired_ho_anchors` | 기존 SQL 오탐 제거 유지 |
| 정상 목록 복귀 | same-section immediate paragraph + 경계 앞 번호 또는 직전 정상 번호+1 | synthetic 4종 회복, 비인접 Oracle 차단 |
| 날짜 negative | `YYYY. M. D[.]`, 월 1~12·일 1~31 | body 보존, anchor 불변 |
| direct `목` positive | 열린 `장|절`, margin 0, indent 하한, level 0 | 기존 실문서 제목 유지 |
| TOC negative | tab 또는 dotted leader 뒤 숫자 tail | synthetic leader 3종 body 보존 |
| shape 하한 | `DIRECT_MOK_MIN_INDENT_HWPUNIT = -1280` | -1280 허용, -1281 등 거부 |

파일명, style/shape ID, paragraph 좌표는 제품 판정에 사용하지 않았다. 공개 구조체·직렬화·CLI 선택
계약과 `StructureMode::Auto` 선택 로직은 바꾸지 않았다.

## 2. red→green

추가 회귀를 보정 전 코드에서 실행한 결과는 9 passed / 2 failed였다.

- red: 복합 번호 뒤 즉시 이어지는 정상 `호` 회복
- red: dotted-leader+쪽번호 `목` 거부
- 기존 green을 강화: direct `목`의 `indent=-1280` positive와 `-1281`, nonzero margin,
  `para_level=1` negative·body 보존

최종 구현에서 11 passed / 0 failed로 전환했다.

## 3. 기각한 상태 전이

최초 구현은 만료 anchor가 기억한 번호만 맞으면 이후 어느 위치에서도 복귀했다. sample10의 `4.1`
뒤 설명 문단을 건너뛴 SQL 목록이 stale anchor를 다시 열어 세 변형 각각 node 8→1,145,
`호` 4→1,141로 회귀했다.

정상 예시는 복합 번호 바로 다음 문단에서 기존 목록이 이어진다는 최소 공통점이 있다. 최종 구현은
same-section immediate paragraph 조건을 추가했고, 숫자가 같더라도 중간 문단이 있으면 만료 상태를
유지한다. 위치 연산은 `checked_add`로 경계 overflow도 피한다.

## 4. corpus 대조

`ce6a23bca` baseline과 최종 보정 checkout에 서로 다른 Cargo target을 사용했다.

- recursive 후보 673 / parse 성공 670 / 기존 실패 3: 양쪽 동일
- 파일별 node 수와 kind count: 차이 0
- 최초 기각안에서만 sample10 세 변형의 대량 회귀 확인

따라서 최종 보정은 Stage 4 corpus 결과를 변경하지 않고 synthetic 계약의 누락만 채운다.

## 5. 검증과 최신 base

보정 후보에서 structure 8, #3744 11, #3693 3, #3695 13, CLI 4 tests와 전체 release tests,
fmt, diff check, all-targets clippy를 통과했다. 전체 release의 lib 결과는 3,200 passed / 7 ignored이며
모든 test target이 exit 0이었다.

검증 뒤 새 `upstream/devel` `301d0fe5f`를 merge commit `c610a0b1a`로 결합했다. #3744 제품·테스트
파일의 upstream 변경은 없고 focused 5개 gate를 다시 통과했다. 최신 결합 head의 전체 required gate는
push 뒤 GitHub Actions 결과로 판정한다.

## 6. self-merge 조건

- 이 review_impl을 포함한 최신 head SHA 확인
- required GitHub Actions 전부 성공
- draft/ready 상태는 merge 실행자인 작업지시자가 처리
- merge 뒤 merge comment, #3744 close, #1528 통합 검증은 별도 후속 처리

로컬 판정은 merge 가능 후보이며, 이번 승인 범위는 review 문서 commit과 push까지다. GitHub
comment/review, ready 전환, merge는 수행하지 않는다.
