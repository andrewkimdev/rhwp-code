# PR #3855 검토 기록 — des 0.8.1 → 0.9.0

## PR metadata

| 항목 | 내용 |
|---|---|
| 원 PR | [#3855](https://github.com/edwardkim/rhwp/pull/3855) |
| 작성자 / base | `dependabot[bot]` / `devel` |
| 변경 분류 | RustCrypto production |
| source commit | `3c4b0350e844eddcc4195496214810d777d8534d` |
| integration commit | `f02038e09` |

## 라우팅과 판단

원 PR은 `maintainerCanModify=false`인 Dependabot head이므로 직접 수정하거나 직접 merge하지 않는다.
검토자는 fetch 전에 `@jangster77`로 지정했다. source author·`Signed-off-by`·원 SHA를 보존하는
`git cherry-pick -x`로 Route B 통합 branch `review/dependabot-20260803`에 수용했다.

## 변경과 검증

- des 0.8.1 → 0.9.0.
- HWP3 DES block slice conversion만 새 API에 맞추고 byte 계약을 보존했다.
- 통합 head에서 Rust check/fmt/clippy, 전체 release-test, WASM package, 영향 package의 clean install·build/typecheck,
  CI frontend contract, actionlint 및 `git diff --check`를 명시적 성공으로 확인했다.

## 권고

전체 source mapping, maintainer 보정, audit과 remote/CI 승인 경계는 [통합 구현·검토 기록](pr_3844_review_impl.md)을 따른다.
최종 merge 판단은 통합 PR의 최신 head CI와 작업지시자 승인 뒤에만 한다.
