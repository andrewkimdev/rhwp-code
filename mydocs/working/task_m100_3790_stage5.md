# 작업 기록 — task_m100_3790 Stage 5A

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **브랜치**: `issue-3790-stage5a-codeql-safety`
- **worktree**: `tmp/issue-3790-stage5a-codeql`
- **기준**: `upstream/devel` `e48fe86947fb` (#4310·#4317 merge 포함)
- **상태**: Draft PR #4341 보정 canary 구현·focused 검증 완료, 원격 재측정 대기

## 재개와 보존 경계

- 기존 `tmp/issue-3790-stage5-codeql`은 최신 devel 이전의 미완성 prototype과 보정 설계가 함께 있던
  rejected WIP다. 필요한 근거를 이 기록으로 옮기고 Stage 5A focused 검증을 통과한 뒤 작업지시자의
  명시적 승인을 받아 worktree와 로컬 브랜치 `codex/issue-3790-stage5-codeql`을 정리했다.
- `tmp/issue-3790-stage26`은 어느 원격에도 없는 Stage 2.6 controller prototype의 유일본이므로 이
  단계의 정리 대상이 아니다.
- #4310 merge 뒤 classifier의 `codeql_languages` 계약은 유지됐고 Native Skia 대상만 보강됐다.

## 설계 보정 근거

- Stage 4 canary PR #4078은 wall clock 575초 중 `Analyze (rust)`가 563초여서 CodeQL이 남은 critical
  path임을 확인했다.
- Actions의 `Analyze (...)` job 성공은 GitHub Advanced Security의 최종 CodeQL check 성공을 보장하지
  않는다. PR #4310의 보정 전 candidate에서는 세 Analyze job이 성공했지만 app
  `github-advanced-security`의 `CodeQL` check가 high alert로 실패했고, 보정 candidate에서는 같은
  check가 성공했다.
- 따라서 workflow job만 재사용하는 정적 selector는 폐기한다. 기존 PR workflow run의 candidate SHA와
  현재 attempt 시작 시각을 기준으로 동일 SHA의 현재 보안 check를 식별해 missing·pending·failure를
  모두 닫는다. 재실행의 이전 attempt에서 생성된 check도 재사용하지 않는다.
- Rust `cargo build` 뒤에도 CodeQL이 별도 autobuild와 extraction을 수행했다. Stage 5A는 blocking lane을
  바꾸지 않고 `build-mode: none`, `upload: never`인 별도 shadow를 추가해 prebuild 제거 효과와 SARIF
  동등성을 원격에서 측정한다.

기존 rejected WIP에서 재사용할 실측 근거도 이 문서로 옮겼다. #4310 Rust job의 cache 복원 뒤
`cargo build`는 약 52초였지만 analyze가 다시 `database trace-command --index-traceless-dbs`와 Rust
`autobuild.sh`를 실행했다. blocking 기준선의 추출 결과는 성공 1,097파일·오류 7파일이며, 원격 shadow의
coverage·진단 비교 기준으로 쓴다. 보정 전 GHAS check `93182688114`는 실패했고 보정 candidate의 check
`93186154548`은 성공했다. 폐기한 정적 selector의 로컬 테스트 통과 기록은 잘못된 보안 의미를 검증한
것이므로 새 구현 근거로 재사용하지 않는다.

## 구현 범위

- [x] `codeqlResult`에 candidate-bound GitHub Advanced Security `CodeQL` check 확인 추가
- [x] 기존 세 언어 blocking matrix와 Rust prebuild 기준선 보존
- [x] PR non-fast-pass 전용 Rust no-build shadow와 SARIF artifact 추가
- [x] Stage 5A workflow 계약 테스트와 CI test wiring 추가
- [x] focused 검증 통과

Stage 5B의 동적 언어 matrix, required status 변경, 원격 push·PR·canary는 이번 focused 구현 범위 밖이다.

## focused 검증

- TDD RED: 새 Stage 5A 테스트가 보안 check 조회와 shadow job 부재를 각각 검출했다.
- `python3 -m unittest scripts/tests/test_codeql_stage5a_workflow.py` — 6/6 통과. 세 Analyze job이
  green이어도 GHAS `CodeQL` check가 `failure`면 fast-pass가 거부되고, 모두 성공하면 재사용되는 실행
  mock과 이전 workflow run attempt의 check를 거부하는 mock을 포함한다.
- Stage 5A·review-only fast-pass·wiring·CI impact·Render Diff·cache sweep Python 계약 테스트 —
  74/74 통과.
- `node --test scripts/tests/ci-impact-classifier.test.cjs` — 28/28 통과.
- `actionlint .github/workflows/ci.yml .github/workflows/codeql.yml` — 통과.
- `git diff --check` — 통과.

변경은 workflow·정적 계약 테스트·문서뿐이며 Rust 제품 코드나 Cargo 계약을 바꾸지 않으므로 Cargo
검증은 적용하지 않는다. 원격 shadow의 1차 duration·SARIF 비교 결과는 아래에 기록한다.

## PR #4341 1차 원격 canary

- **candidate**: `f02aadce71e65b11ca29c6d365484abc0c01204b`
- **CodeQL run**: [31311707469](https://github.com/edwardkim/rhwp/actions/runs/31311707469)
- **결론**: workflow·세 Analyze job·GHAS `CodeQL`·shadow가 모두 성공했지만, no-build 활성화 근거는
  불충분하다.

### 시간

| 구간 | Blocking Rust | No-build shadow | 차이 |
| --- | ---: | ---: | ---: |
| 전체 job | 704초 | 658초 | -46초 (-6.5%) |
| checkout | 41초 | 34초 | -7초 |
| CodeQL init | 16초 | 29초 | +13초 |
| Rust toolchain | 2초 | 1초 | -1초 |
| cargo cache 복원 + 사전 build | 62초 | 0초 | -62초 |
| analyze | 576초 | 585초 | +9초 |

양쪽 analyze는 모두 내부 `autobuild.sh`를 실행했다. blocking은 사전 cache 복원 13초와 `cargo build`
49초를 추가로 썼고, no-build analyze 자체는 더 빠르지 않았다. 따라서 관측된 46초는 `build-mode: none`
효과라기보다 사전 build 제거 효과에 가깝다.

### 추출·SARIF

| 항목 | Blocking Rust | No-build shadow |
| --- | ---: | ---: |
| CodeQL CLI | 2.26.2 | 2.26.2 |
| 성공 추출 Rust 파일 | 1,097 | 1,097 |
| 오류 추출 Rust 파일 | 7 | 3 |
| raw diagnostic message | 2 | 2 |
| raw SARIF artifact | 없음 | 있음 |

shadow artifact `rust-no-build-sarif-31311707469-1`은 압축 95,824바이트, raw 1.4MiB다. 1,920개 artifact,
32개 fingerprinted result를 포함하며 `rust/hard-coded-cryptographic-value` 31건과
`rust/weak-cryptographic-algorithm` 1건이다. 성공 추출 수는 같지만 오류 수가 달라 database 동등성을
단정할 수 없다.

blocking Code Scanning analysis `1591823460`은 PR baseline 처리 뒤 `results_count=0`이고 API로 받은
SARIF도 result·artifact가 제거된 server-processed 형태다. 따라서 shadow의 32개 raw result와 blocking의
raw fingerprint를 직접 비교할 수 없다.

### annotation과 판정

shadow check의 annotation 3건은 PR file coverage 중단 안내, CLI 2.26.2 fallback, CodeQL Action feature
API 권한 부재다. 마지막 항목은 shadow에 `security-events` 권한이 없어서 blocking과 feature 입력이 달랐음을
뜻한다.

1차 canary만으로 `build-mode: none`을 활성화하지 않는다. 다음 측정에서는 다음을 모두 만족해야 한다.

1. blocking analyze도 raw Rust SARIF를 artifact로 보존한다.
2. shadow permissions를 blocking과 같게 선언하고 feature API 경고가 사라지는지 확인한다. fork token
   제한으로 경고가 계속되면 동등한 A/B가 아니므로 활성화하지 않는다.
3. 기본 build mode에서 cargo cache·사전 build만 제거한 shadow로 변수를 하나만 바꾼다.
4. 같은 SHA의 raw result·fingerprint, artifact URI, 성공·오류 추출 수와 duration을 비교한다.

이 보정 canary가 동등성을 확인하기 전에는 blocking Rust lane의 cache·prebuild를 제거하지 않고 Stage 5B
동적 언어 matrix로 넘어가지 않는다.

## PR #4341 보정 canary 구현

1차 측정의 비교 불능 요소를 다음처럼 제거했다.

- blocking matrix는 기본 build mode, Rust cache·수동 `cargo build`, Code Scanning upload를 유지한다.
  analyze의 CodeQL CLI SARIF를 `rust-blocking-results`에 출력하고 Rust matrix job에서만
  `rust-blocking-sarif-*` artifact로 7일 보존한다.
- shadow는 `build-mode: none`을 제거해 blocking과 같은 기본 build mode를 사용하고,
  `security-events: write`, `contents: read`를 동일하게 선언한다.
- shadow에서는 cache·수동 `cargo build`만 생략한다. `upload: never`와 별도 raw SARIF artifact를 유지해
  Code Scanning 결과를 오염시키지 않는다. check·artifact 이름은 첫 측정과 구별되도록
  `Rust no-prebuild shadow`, `rust-no-prebuild-sarif-*`로 바꿨다.

계약 테스트는 보정 전 blocking raw artifact와 no-prebuild shadow가 없어 2건 실패하는 RED를 확인했다.
구현 뒤 `python3 -m unittest scripts/tests/test_codeql_stage5a_workflow.py` 6/6, 연관 Python workflow
계약 테스트 74/74, classifier Node 테스트 28/28이 통과했다. `actionlint`와 `git diff --check`도
통과했다. 원격 CI가 완료되면 같은 run의 두 raw SARIF, 추출 통계, annotation과 duration을 비교한다.
