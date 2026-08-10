# 작업 기록 — task_m100_3790 Stage 5B

- **이슈**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **브랜치**: `issue-3790-stage5b-codeql-languages`
- **worktree**: `tmp/issue-3790-stage5b-codeql`
- **기준**: `upstream/devel` `8ea92cdad120` (#4341 merge)
- **상태**: 설계 보정·구현·focused 검증 완료, 커밋 전

## 선행 정리

- Stage 5A PR #4341은 2026-08-11 merge commit `8ea92cdad120d2db2c9097dc2ffd2df804939f74`로
  `devel`에 반영됐다.
- merge 뒤 Stage 5A 전용 worktree, 로컬 브랜치와 fork 원격 브랜치를 정리했다.
- Stage 2.6 controller prototype의 유일본인 `tmp/issue-3790-stage26`과 로컬 브랜치는 보존했다.

## required status 확인

2026-08-11 작업지시자의 WRITE collaborator 인증으로 GitHub API를 직접 조회했다.

- repository permission은 `WRITE`이며 `admin=false`, `maintain=false`다.
- 상세 `GET /branches/devel/protection`은 404이고 ruleset·GraphQL protection rule은 노출되지 않았다.
- 공개 `GET /repos/edwardkim/rhwp/branches/devel` 응답은 `protected=true`와 required context
  `Build & Test` 하나를 반환했다. app id는 GitHub Actions `15368`이다.
- 따라서 현재 `Analyze (javascript-typescript)`, `Analyze (python)`, `Analyze (rust)`, GHAS `CodeQL`은
  branch protection required check가 아니다. collaborator도 branch metadata로 이 값을 직접 확인할 수
  있으며, 상세 관리 구성 열람·변경은 admin 권한이 필요하다.
- 보호 규칙은 바뀔 수 있으므로 PR 생성 전 같은 branch metadata를 다시 조회한다.

## 설계 보정

- PR head가 자기 분석 언어를 줄이지 못하도록 `pull_request.base.sha`의 classifier만 sparse checkout해
  실행한다. PR 파일 목록은 API로 수집하며 credential은 checkout에 남기지 않는다.
- push·schedule·workflow_dispatch와 checkout·API·classifier·출력 검증 실패는
  `javascript-typescript,python,rust` full로 닫는다.
- matrix는 세 언어를 계속 생성해 `Analyze (...)` check identity를 보존한다. 선택되지 않은 언어 job은
  명시적 no-op success로 끝내 check 부재에 따른 영구 pending 가능성을 제거한다.
- checkout·CodeQL init·Rust toolchain·analysis는 선택된 언어에서만 실행한다.
- Stage 5A의 candidate-bound 재사용은 세 Analyze job과 GHAS `CodeQL`을 계속 독립 확인한다.
  `codeql_languages=none`에서 GHAS check가 없으면 후속 review-only 재사용은 fail-closed한다.

## 구현

- `.github/workflows/codeql.yml` preflight에 trusted classifier checkout, PR 파일 수집, classifier 실행과
  허용 언어 집합 finalizer를 추가했다.
- preflight output으로 `codeql_languages`, `classification_status`, `impact_reason`, `impact_authority`를
  노출하고 Job Summary에 판정 근거를 남긴다.
- 고정 세 언어 matrix의 선택되지 않은 lane에는 no-op step을 두고 실제 분석 step을 정확한 token
  membership 조건으로 묶었다.
- `.github/workflows/ci.yml`의 기존 classifier 설명을 Stage 5B 소비 관계와 일치시켰다.
- `scripts/tests/test_codeql_workflow.py`는 trusted-base·full fallback·허용 집합 검증·고정 job 이름과
  선택 step wiring을 장기 workflow 계약으로 고정한다.

## focused 검증

- TDD RED에서 trusted classifier/fail-closed wiring과 선택 언어/no-op wiring 부재를 검출하는 2건이
  예상대로 실패했다.
- `python3 -m unittest scripts/tests/test_codeql_workflow.py` — 11/11 통과.
- CI가 실행하는 연관 Python workflow 계약 10개 파일 — 90/90 통과.
- `node --test scripts/tests/ci-impact-classifier.test.cjs` — 28/28 통과.
- `actionlint .github/workflows/ci.yml .github/workflows/codeql.yml` — 통과.
- `git diff --check` — 통과.

변경 범위가 workflow·정적 계약 테스트·문서뿐이므로 Cargo와 제품 테스트는 적용하지 않는다. 원격 push와
PR 생성은 별도 승인 뒤 진행한다.
