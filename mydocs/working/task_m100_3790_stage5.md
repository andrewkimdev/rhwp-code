# 작업 기록 — task_m100_3790 Stage 5A

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **브랜치**: `issue-3790-stage5a-codeql-safety`
- **worktree**: `tmp/issue-3790-stage5a-codeql`
- **기준**: `upstream/devel` `e48fe86947fb` (#4310·#4317 merge 포함)
- **상태**: 구현·focused 검증 및 승인된 rejected worktree 정리 완료

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
검증은 적용하지 않는다. 원격 shadow의 duration·SARIF 비교는 push·PR 승인 뒤 수행한다.
