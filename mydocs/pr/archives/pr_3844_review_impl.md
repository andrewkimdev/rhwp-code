# Dependabot PR #3844–#3861 통합 구현·검토 기록

## 목적과 현재 상태

- 대상: [#3844](https://github.com/edwardkim/rhwp/pull/3844)부터 [#3861](https://github.com/edwardkim/rhwp/pull/3861)까지 열린 Dependabot PR 18건.
- 방식: 최신 `upstream/devel` 위 `review/dependabot-20260803`에서 source commit을 `git cherry-pick -x`로 누적한 Route B 통합 후보.
- 기준선: `upstream/devel@6fd26c5dac23f28fc8c46b0c3b4a5cd6438ef686`.
- 현재 상태: local validation과 review 기록은 완료했다. remote push와 integration PR 생성은 작업지시자의 별도 승인 전에는 수행하지 않는다.

원 PR은 모두 `maintainerCanModify=false`이며 reviewer `@jangster77`를 지정했다. contributor head에는 push/rebase하지 않는다.

## source와 integration commit

| 원 PR | 의존성 | source commit | integration commit |
|---|---|---|---|
| [#3844](https://github.com/edwardkim/rhwp/pull/3844) | pbkdf2 0.12.2 → 0.13.0 | `ea22e8b24358e273ce263ece5315259bf0f585e2` | `e02a45934` |
| [#3845](https://github.com/edwardkim/rhwp/pull/3845) | Vite 8.1.5 → 8.2.0 (rhwp-studio) | `326f267088cb870147c12cb32a140a3d9421e800` | `6aa9bdcbf` |
| [#3846](https://github.com/edwardkim/rhwp/pull/3846) | roxmltree 0.20.0 → 0.21.1 | `c92946e5ae7dcce9ba59c862c4bddeb8237d0a16` | `853d14195` |
| [#3847](https://github.com/edwardkim/rhwp/pull/3847) | puppeteer-core 25.3.0 → 25.4.0 | `3e5bf891f66baf4e8986449f7edc1933a41e9620` | `5a60a4f1a` |
| [#3848](https://github.com/edwardkim/rhwp/pull/3848) | cbc 0.1.2 → 0.2.1 | `bd8e468d854a4f87835ceeecab8464f0cbbdf274` | `a2d88769b` |
| [#3849](https://github.com/edwardkim/rhwp/pull/3849) | cipher 0.4.4 → 0.5.2 | `0137e4bb5bcf3c0da4f91b5e9a53f6715061af74` | `9fec1c371` |
| [#3850](https://github.com/edwardkim/rhwp/pull/3850) | actions/setup-python v5 → v7 | `985456f4538751199c7b370e9e656e8727da7c39` | `6500a07c9` |
| [#3851](https://github.com/edwardkim/rhwp/pull/3851) | Vite 8.1.5 → 8.2.0 (rhwp-chrome) | `dd139e8ba8c7ae0473b4c8890ae2f612967948c9` | `ca11987c1` |
| [#3852](https://github.com/edwardkim/rhwp/pull/3852) | aes 0.8.4 → 0.9.2 | `05503eccb13231d5e8993a024688c6953ebf92de` | `b560929e6` |
| [#3853](https://github.com/edwardkim/rhwp/pull/3853) | webpack 5.109.0 → 5.109.2 | `5a6800f3da653dd8fe08143e9c64a4dd1e3d9327` | `7dc519d5f` |
| [#3854](https://github.com/edwardkim/rhwp/pull/3854) | webpack-cli 7.2.1 → 7.2.2 | `4e7df7c7edfb324c9c81a72daa3522e7223b09c4` | `9a045c1e6` |
| [#3855](https://github.com/edwardkim/rhwp/pull/3855) | des 0.8.1 → 0.9.0 | `3c4b0350e844eddcc4195496214810d777d8534d` | `f02038e09` |
| [#3856](https://github.com/edwardkim/rhwp/pull/3856) | actions/upload-artifact v4 → v7 | `53c0fb9ea14eaa33927a7f3bb93091688aa1dde3` | `32d044bed` |
| [#3857](https://github.com/edwardkim/rhwp/pull/3857) | getrandom 0.3.4 → 0.4.3 | `49cc59c03a8b8d0bdd439dafed68b45027bc1054` | `327005efd` |
| [#3858](https://github.com/edwardkim/rhwp/pull/3858) | Vite 8.1.5 → 8.2.0 (rhwp-firefox) | `eeaf318c34c95703cc349f08b90f77ddd3f45f98` | `32ca0e77a` |
| [#3859](https://github.com/edwardkim/rhwp/pull/3859) | subsecond 0.7.9 → 0.7.10 | `95c2630c1695a220be939864bd6bf99cb59ce4fc` | `79461bf36` |
| [#3860](https://github.com/edwardkim/rhwp/pull/3860) | hmac 0.12.1 → 0.13.0 | `480eeb6af13b33b97ae24c6f367c226152d2e65b` | `ec4a509fd` |
| [#3861](https://github.com/edwardkim/rhwp/pull/3861) | sha2 0.10.9 → 0.11.0 | `49afc677c09a8a58f3d1a32066a59cc5cca33afe` | `acdb51846` |

모든 source commit은 Dependabot author와 `Signed-off-by` 및 원 SHA provenance를 보존한다.

## 충돌과 maintainer 보정

18건은 서로 다른 stale lockfile 기준선에서 생성돼 Cargo와 VS Code lockfile 문맥이 겹쳤다. 현재 통합 manifest가 선언한
모든 bump를 유지하도록 resolver를 다시 계산했으며 source commit의 저자·provenance를 바꾸지 않았다.

그 뒤 `de2c2c226 fix(deps): RustCrypto와 subsecond 버전 계약 보정`을 별도 maintainer commit으로 추가했다.

- RustCrypto 0.9/0.5 API에서 제거된 block trait와 padded slice API만 현재 API로 이행했다.
- 새 PBKDF2/HMAC digest graph에 맞춰 `sha1`도 0.11로 맞췄다. HWP3 DES, HWP5 AES, HWPX AES-256-CBC의
  알고리즘·IV·salt·NoPadding·wire format은 변경하지 않았다.
- subsecond 0.7.10과 Dioxus CLI install script·runtime expectation을 함께 고정했다.

## 로컬 검증

| 범위 | 결과 |
|---|---|
| Rust | check, fmt, clippy, full `cargo test --profile release-test --tests` 성공 |
| password fixture | HWP3 11, HWP5 2, HWPX 3, multiformat 2, write 1, MCP password 4 성공 |
| WASM | `wasm-pack build --target web --out-dir pkg` 성공 |
| Studio | clean install, test 729, production build 성공 |
| Chrome / Firefox | clean install, 새 WASM 포함 build 및 dist contract 성공 |
| VS Code | clean install, typecheck 및 Webpack 5.109.2 production compile 성공 |
| frontend CI contract | wasm/editor 3, editor 24, service worker 113, options 4, extension dist 3, font asset 6 성공 |
| Actions / hygiene | actionlint, `git diff --check` 성공 |

Studio `npm audit`의 low 1/high 2는 기준선 lockfile에서도 동일한 `@babel/core`, `brace-expansion`,
`fast-uri` 전이 dev dependency 경고였다. 이번 갱신으로 도입·악화된 취약점은 아니며, Chrome·Firefox·VS Code audit은 0건이다.

## Remote PR 계획과 승인 경계

승인 후 제목 `chore(deps): Dependabot 의존성 18건 통합 갱신`의 `devel` 대상 integration PR을 만든다. 본문에는
`Supersedes #3844`부터 `Supersedes #3861`까지, source/integration mapping, maintainer 보정과 local validation을 적는다.

코드·workflow 변경이므로 docs-only fast-pass 대상이 아니다. 최신 integration head CI와 mergeability가 성공한 뒤 review와 merge는 별도로 승인받는다.
