# PR #3857 검토 기록 — getrandom 0.3.4 → 0.4.3

## PR metadata

| 항목 | 내용 |
|---|---|
| 원 PR | [#3857](https://github.com/edwardkim/rhwp/pull/3857) |
| 작성자 / base | `dependabot[bot]` / `devel` |
| 변경 분류 | Rust/WASM production |
| source commit | `49cc59c03a8b8d0bdd439dafed68b45027bc1054` |
| integration commit | `327005efd` |

## 라우팅과 판단

원 PR은 `maintainerCanModify=false`인 Dependabot head이므로 직접 수정하거나 직접 merge하지 않는다.
검토자는 fetch 전에 `@jangster77`로 지정했다. source author·`Signed-off-by`·원 SHA를 보존하는
`git cherry-pick -x`로 Route B 통합 branch `review/dependabot-20260803`에 수용했다.

## 변경과 검증

- getrandom 0.3.4 → 0.4.3.
- wasm_js feature를 유지하고 실제 wasm-pack build로 확인했다.
- 통합 head에서 Rust check/fmt/clippy, 전체 release-test, WASM package, 영향 package의 clean install·build/typecheck,
  CI frontend contract, actionlint 및 `git diff --check`를 명시적 성공으로 확인했다.

## 권고

전체 source mapping, maintainer 보정, audit과 remote/CI 승인 경계는 [통합 구현·검토 기록](pr_3844_review_impl.md)을 따른다.
최종 merge 판단은 통합 PR의 최신 head CI와 작업지시자 승인 뒤에만 한다.
