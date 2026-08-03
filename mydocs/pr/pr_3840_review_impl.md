# PR #3840 메인터너 보정 구현·검토 기록

## 기준선과 범위

- 원 PR: [#3840](https://github.com/edwardkim/rhwp/pull/3840), `JamesPsh/fix/3546-chart-preserved-on-save`.
- 확인한 source head: `822e9057ddd05a5ca1825342984bb3e7da0e52eb`.
- 기준 base: `devel@ad67e5a63a9af3deb39f872720c29f7865651e5d`.
- local branch: `review/pr3840-maintainer`.

contributor의 기존 commit은 rebase·amend·reset하지 않는다. 보정은 source commit 뒤의 독립된
maintainer commit으로만 추가하며, `maintainerCanModify=true` 권한을 사용하되 push 직전 remote head를
다시 확인한다.

## 보정 내용

`6d8232cd1f1e754c7f8f3855bbbf48405da4d858` (`test(hwpx): Chart 파트 암호화 회귀를 고정`):

- `tests/issue_3546_chart_preserved_on_save.rs`에 password 저장 회귀를 추가했다.
- `Chart/` 내부 파일의 암호화 여부와 `META-INF/manifest.xml` 등록 여부를 확인한다.
- 무비밀번호 열기 실패 및 올바른 비밀번호로 열어 다시 저장한 뒤 chart entry의 바이트·구조 보존을 확인한다.

이 검사는 PR의 `is_hwpx_protected_path`가 `Chart/`를 보호 경로에 포함한 동작을 직접 고정한다.

`71fdd2527545691a48984eb2ebc7766ba47e71e8` (`test(hwpx): 고정 비밀번호 CodeQL 경고 제거`):

- GitHub Advanced Security가 test의 고정 비밀번호 리터럴을 critical로 보고한 정확한 annotation을 확인했다.
- 비밀번호를 process ID 기반 런타임 바이트로 바꿨고, focused test 2개와 Clippy를 다시 성공시켰다.

## 검증과 remote 순서

1. 전용 target(`CARGO_TARGET_DIR=target/review-pr3840`, `CARGO_INCREMENTAL=0`)에서 fmt, diff check,
   focused test를 성공시켰다.
2. 전체 `cargo test --profile release-test --tests`와 `cargo clippy --all-targets -- -D warnings`를
   모두 exit code 0으로 확인했다.
3. 본 기록은 source 보정과 분리된 docs commit으로 만든다.
4. `gh pr view`와 `git ls-remote`가 source head `822e9057…`를 계속 가리키는지 확인한 뒤, dry-run 및
   일반 push로 contributor branch에 추가 commit만 보낸다.
5. push 후 원 PR의 새 head CI 성공 및 명시적 merge 승인을 기다린다.

현재 단계에서는 merge, rebase, force-push를 수행하지 않는다.
