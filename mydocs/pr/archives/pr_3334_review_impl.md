# PR #3334 umbrella implementation 계획 — VS Code 의존성 예외 3건

## 목적과 상태

- 대상: Dependabot PR [#3334](https://github.com/edwardkim/rhwp/pull/3334),
  [#3337](https://github.com/edwardkim/rhwp/pull/3337),
  [#3344](https://github.com/edwardkim/rhwp/pull/3344)
- 역할: 세 원 PR을 직접 merge하지 않고 최신 `upstream/devel` 기반 Route B integration PR로 대체한다.
- 현재 상태: source cherry-pick, collaborator 보정, local verification 완료. review 문서 작성 단계.
- 이 문서는 integration PR merge 전 실행 계획이다. push, PR 생성, CI 성공, merge, 원 PR close를
  완료 사실로 기록하지 않는다.

## 라우팅과 기준선

```text
base route: collaborator 매개 외부 PR (Route B: 통합 PR)
modifiers: 접수·리뷰 기록, 로컬 검증, 다수 PR·update branch, 재작업·예외(Dependabot)
integration branch: review/dependabot-vscode-20260726
base: upstream/devel@e7dffced399e45685ae746bd2ea21d37542ea95e
verified local code head: fdc0af5b5c2d8b266990419fbc87cc0543589717
```

세 원 PR은 모두 `maintainerCanModify=false`이므로 contributor/Dependabot head에는 commit을 push하지 않는다.
원 PR별 reviewer는 local fetch 전에 `@postmelee`로 지정했다. labels는 기존
`dependencies`, `javascript`를 유지하고 milestone은 만들지 않았다. Dependabot bot은 assignable actor가
아니어서 assignee는 비워 두었다.

## source와 integration commit

| 순서 | 원 PR | source commit | integration commit | 최종 처리 |
|---|---|---|---|---|
| 1 | #3334 | `9a77d8362d5bd86701c68a4799d434736742c62e` | `3039b4d8e08d4c2a847a601554cf6d6fe508ba02` | TypeScript 7 CLI 유지, compiler API는 TypeScript 6 alias로 보정 |
| 2 | #3337 | `ea5580b77378d685193aed76c880bb5732af2f79` | `beba924eb4f4e8f64fec55e8f1d6927c9b6d8676` | 1.125 type은 제거하고 VS Code 1.82 type으로 계약 고정 |
| 3 | #3344 | `568d7d2932ec98e769175f8bbdf236832b59b7e7` | `22222092feab59ee67fc6784ce0b6ce72f8caf23` | Node 26 type은 제거하고 Node 18.15 type으로 계약 고정 |
| 4 | collaborator fix | 해당 없음 | `fdc0af5b5c2d8b266990419fbc87cc0543589717` | build bridge, TS config, type policy, lockfile 보안 patch |

모든 source commit은 `git cherry-pick -x`로 적용했다. author는 `dependabot[bot]`으로 유지되고
`Signed-off-by`와 source SHA provenance가 commit message에 보존된다. collaborator fix는 별도 commit이다.

## 충돌과 보정 범위

### Cherry-pick 충돌

- #3334와 #3337은 순서대로 자동 적용됐다.
- #3344는 세 PR이 같은 manifest/lockfile 기준선에서 갈라져
  `@types/node`/`@types/vscode` root dependency 문맥에 content conflict가 발생했다.
- conflict 해소 commit에는 두 원 bump를 모두 유지했다. 지원 계약 변경은 이후 collaborator fix로
  분리해 원 commit의 의미와 보정 책임을 섞지 않았다.

### Collaborator fix

- `rhwp-vscode/package.json`
  - TypeScript 7 CLI를 `@typescript/native` alias로 설치한다.
  - `typescript` import는 `@typescript/typescript6` API alias로 제공한다.
  - extension/webview TypeScript 7 `typecheck` 뒤 webpack을 실행한다.
  - VS Code/Node type을 각각 1.82/18.15 line에 고정한다.
- `rhwp-vscode/package-lock.json`
  - alias와 runtime type 해소를 반영한다.
  - `fast-uri`를 취약한 3.1.3에서 compatible patch 3.1.4로 갱신한다.
- `rhwp-vscode/tsconfig.json`
  - Extension Host의 Node type을 명시한다.
- `rhwp-vscode/tsconfig.webview.json`
  - TypeScript 7에서 제거된 `baseUrl`을 없애고 paths를 명시적 상대 경로로 바꾼다.
  - 공유하는 `rhwp-studio` source를 포함하도록 `rootDir`을 명시한다.
- `rhwp-vscode/src/webview/viewer.ts`
  - 최신 DOM `hidden` type을 boolean으로 정규화한다.
- `.github/dependabot.yml`
  - `engines.vscode ^1.82.0`을 올리기 전 newer VS Code/Node type PR 생성을 막는다.

## 단계

| 단계 | 상태 | 작업 |
|---|---|---|
| 0. metadata 정렬 | 완료 | 세 원 PR reviewer 지정, labels/milestone 확인, assignee 제한 기록 |
| 1. Route B branch | 완료 | 최신 `upstream/devel`에서 visibility/integration branch 생성 |
| 2. source 통합 | 완료 | 세 source commit을 `-x` cherry-pick하고 #3344 conflict 해소 |
| 3. collaborator 보정 | 완료 | build bridge, type/runtime 계약, Dependabot 정책을 별도 commit |
| 4. local verification | 완료 | fresh WASM, clean npm install, typecheck, webpack, contract, VSIX, audit |
| 5. review 문서 | 현재 단계 | 원 PR별 review와 umbrella report를 별도 docs commit으로 작성 |
| 6. remote integration PR | 승인 대기 | origin push와 `devel` 대상 integration PR 생성 |
| 7. authoritative CI/review | 미실행 | 최신 integration head의 full CI 확인 후 GitHub review 판단 |
| 8. merge/후속 | 별도 승인 필요 | integration PR merge, 원 PR 설명 comment/close, issue 상태 확인 |

## 검증 순서와 결과

1. `CARGO_INCREMENTAL=0 wasm-pack build --target web --dev`: 성공.
2. 보정 lockfile `npm ci`: 성공, 159 package audit 취약점 0건.
3. compiler selection: TypeScript 7 CLI 7.0.2, `typescript` compiler API 6.0.3.
4. `npm run typecheck`: extension/webview 모두 성공.
5. `npm run compile`: 두 webpack config 모두 성공.
6. VS Code font/license contract: 3/3 성공.
7. VSIX package: 35 files, 17.35 MB, 성공.
8. Dependabot YAML parse와 `git diff --check`: 성공.

전체 font contract의 Studio/browser distribution 1건은 `rhwp-studio/dist/fonts` prerequisite가 없어
실패했다. integration PR CI는 Studio build를 선행하므로 latest full `Frontend package gates` 성공을
필수 조건으로 둔다.

## Remote PR 계획

사용자 승인 뒤에만 현재 branch를 `origin`에 push하고 `devel` 대상 integration PR을 만든다.
PR body에는 다음을 포함한다.

- `Supersedes #3334`, `Supersedes #3337`, `Supersedes #3344`
- source PR/commit과 integration commit mapping
- Dependabot author 및 `Signed-off-by` 보존
- TypeScript 7/6 bridge와 VS Code 1.82/Node 18.15 지원 계약
- local verification과 원 PR CI 스냅샷
- integration PR이 merge된 뒤 원 PR을 close한다는 명시

연결된 issue가 없으므로 `Closes #...`는 넣지 않는다. full CI가 필요한 code/config 변경이므로
review-only fast-pass를 적용하지 않는다.

## 승인·rollback 경계

- docs commit 뒤 사용자 검토와 remote push/PR 생성 승인을 다시 받는다.
- integration PR CI가 실패하면 원 source commit을 rewrite하지 않고 collaborator fix를 별도 commit으로
  보완한다.
- merge, 원 PR comment/close, issue close는 각각 해당 단계의 명시 승인을 받는다.
- remote push 전 rollback은 local branch를 보존하거나 삭제하는 것으로 끝나며 원 PR에는 영향이 없다.
