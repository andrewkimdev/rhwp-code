# PR #3840 검토 기록 — 차트 HWPX 구조 보존

## PR metadata

| 항목 | 내용 |
|---|---|
| 원 PR | [#3840](https://github.com/edwardkim/rhwp/pull/3840) |
| 작성자 / base | `JamesPsh` / `devel` |
| 변경 분류 | HWPX parser·serializer 및 암호화 저장 |
| source commit | `822e9057ddd05a5ca1825342984bb3e7da0e52eb` |
| maintainer test commit | `6d8232cd1f1e754c7f8f3855bbbf48405da4d858` |
| CodeQL 보정 commit | `71fdd2527545691a48984eb2ebc7766ba47e71e8` |

## 검토와 보정

PR은 `Chart/*.xml`을 `hp:ole`로 치환하지 않고 `hp:chart` 및 필요한 `hp:switch` fallback 구조로
보존한다. 또한 HWPX 암호화 대상 경로에 `Chart/`를 포함한다. contributor head가 최신 `devel`을
merge한 상태임을 확인했고, maintainer 수정 권한을 사용해 reviewer `@jangster77`를 지정했다.

암호화 경로의 회귀를 고정하기 위해 실제 chart fixture 하나를 비밀번호로 저장한 뒤, 모든 `Chart/`
항목이 평문이 아니며 manifest에 등록되고, 비밀번호 없이는 열 수 없고 올바른 비밀번호로 재저장해도
chart 바이트와 구조가 유지되는지를 검사하는 통합 테스트를 별도 maintainer commit으로 추가했다.

최초 CI의 GitHub Advanced Security가 이 테스트의 고정 비밀번호 리터럴을 critical alert로 보고했다.
보정 commit은 이를 실행 process ID에서 만든 런타임 바이트로 바꿔 고정 credential을 제거했다.

## 로컬 검증

| 범위 | 결과 |
|---|---|
| `cargo fmt --check` | 성공 |
| `git diff --check` | 성공 |
| `cargo test --test issue_3546_chart_preserved_on_save` | 2 passed |
| `cargo test --profile release-test --tests` | exit code 0 |
| `cargo clippy --all-targets -- -D warnings` | exit code 0 |
| CodeQL 보정 뒤 focused test·Clippy | 2 passed / exit code 0 |

변경은 HWPX 저장 구조와 암호화 경로만 다루며 renderer/layout 코드를 수정하지 않는다. 따라서 별도
시각 sweep은 수행하지 않고, 원본 chart fixture를 통한 구조·바이트 round-trip 검증으로 대체한다.

## 후속 조건

contributor branch가 source SHA에서 변하지 않았는지 재확인하고, maintainer test와 본 검토 기록을
별도 commit으로 push한다. push 뒤에는 새 head의 GitHub CI가 성공한 뒤에만 최종 review/merge를
판단한다. merge에는 별도 사용자 승인이 필요하다.
