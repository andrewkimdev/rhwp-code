# task_m100_3790 Stage 2 1차 결과 — shadow 판정 실측

- **Issue**: [#3790](https://github.com/edwardkim/rhwp/issues/3790)
- **Stage 1 PR**: [#3792](https://github.com/edwardkim/rhwp/pull/3792)
- **브랜치**: `codex/issue-3790-shadow-observation`
- **기준**: `upstream/devel` `ec051ec6c61f`
- **live 관찰 시작**: 2026-08-02 17:28:53 UTC, #3792 merge 직후
- **기록 시각**: 2026-08-03 KST
- **상태**: 1차 실측 완료, live frontend·render 축 표본 부족으로 Stage 3 활성화 보류

## 1. 측정 방법과 해석 경계

live 표본은 #3792 merge 뒤 생성된 `pull_request` CI run을 대상으로 했다. `CI preflight` log의
`SHADOW_*` 환경값, 같은 PR의 실제 파일 목록, legacy CI worker 결과를 대조했다. Job Summary 본문은
별도 API가 없어도 완료된 preflight log의 summary step 환경값에서 판정 결과를 회수할 수 있었다.

historical replay는 측정 시점의 최근 종료 PR 60개를 `gh pr list --state closed --limit 60`으로 가져와
merge된 classifier version 1에 다시 입력했다. PR `headRefOid`와 Actions `headSha`가 같은 최신 run을
연결하고, `gh run view --json jobs`의 `startedAt`부터 `completedAt`까지를 runner 사용 시간으로 합산했다.

수집 명령의 핵심 표면은 다음과 같다.

```bash
gh pr list --repo edwardkim/rhwp --state closed --limit 60 \
  --json number,title,headRefOid,changedFiles,files,url
gh run list --repo edwardkim/rhwp --workflow ci.yml --event pull_request --limit 400 \
  --json databaseId,headSha,status,conclusion,createdAt,updatedAt,url
gh run view RUN_ID --repo edwardkim/rhwp --json jobs
```

GraphQL `files`가 100개만 반환한 PR은 부분 목록으로 분류하지 않고 수집기에서 `full`로 닫았다. 아래의
`graphql-file-list-truncated`는 이 replay 수집기의 안전 경계이며, 실제 workflow의 REST pagination 및
PR 3,000개 경계 reason과는 구분한다.

runner-minute는 실제 실행된 job 시간을 더한 값이다. 병렬 job 합계이므로 wall time 절감과 같지 않으며,
아직 구현하지 않은 frontend `unit` gate 자체의 시간도 추정하지 않았다.

## 2. merge 이후 live shadow

관찰 시점에 merge 이후 생성된 live CI는 고유 PR 4건이다. #3819의 직전 run 1건은 새 push로 취소되어
중복 표본에서 제외했다.

| PR | CI run | 실제 파일 축 | shadow 결과 | legacy 결과 |
| --- | --- | --- | --- | --- |
| [#3749](https://github.com/edwardkim/rhwp/pull/3749) | [30759050941](https://github.com/edwardkim/rhwp/actions/runs/30759050941) | Rust 비렌더 2개 + 문서 | `rust=true`, `frontend=none`, `render=false`, `native_skia=false`, `CodeQL=rust`, `classified:rust` | full CI 성공 |
| [#3771](https://github.com/edwardkim/rhwp/pull/3771) | [30759557019](https://github.com/edwardkim/rhwp/actions/runs/30759557019) | Studio unit + Rust + `src/wasm_api/tests.rs` | `full`, `fail-closed:wasm-contract` | full CI 성공 |
| [#3819](https://github.com/edwardkim/rhwp/pull/3819) | [30759875509](https://github.com/edwardkim/rhwp/actions/runs/30759875509) | Rust renderer + Python fixture tool | `full`, `fail-closed:unclassified-path` | 기록 시점 실행 중 |
| [#3740](https://github.com/edwardkim/rhwp/pull/3740) | [30759959177](https://github.com/edwardkim/rhwp/actions/runs/30759959177) | Rust renderer + Python fixture/census tool | `full`, `fail-closed:unclassified-path` | 기록 시점 실행 중 |

#3749 preflight는 shadow input 15개를 수집했고 checkout·수집·분류·summary step이 모두 성공했다. 실제
legacy CI도 lint, archive, 8개 shard, Native Skia, aggregate가 모두 성공했다. classifier가 생략 후보로
본 Native Skia는 5분 21초, Rust 외 CodeQL 두 job은 3분 38초를 사용했지만 둘 다 성공했다.

#3771은 frontend 파일이 있어도 WASM 경계를 함께 바꾸므로 mode를 부분 승격하지 않고 모든 축을 `full`로
닫았다. 이는 mixed 변경이 가장 보수적인 축으로 승격되어야 한다는 계약과 일치하며, 실제 legacy
worker도 전부 성공했다.

#3819와 #3740은 renderer가 있어 Rust render 축 자체는 명확하지만, `tools/make_*.py`와 census tool을
현재 classifier가 별도 lane으로 분류하지 않는다. 일부 파일만 보고 좁히지 않고 최종 결과를 full로
승격한 것은 의도한 미분류 fail-closed 동작이다.

live 고유 PR 표본은 4건이고 그중 완료 표본은 2건이다. 저장소의 기존 CI 측정 기준상 1~4건은 소수
관측값이므로 P50/P90이나 활성화 판단에 사용하지 않는다.

## 3. 최근 종료 PR 60건 historical replay

### 3.1 판정 분포

| 판정 | 건수 | 비율 |
| --- | ---: | ---: |
| `full` | 38 | 63.3% |
| Rust 비렌더 | 16 | 26.7% |
| Rust 렌더 | 4 | 6.7% |
| frontend `unit` | 1 | 1.7% |
| review-only | 1 | 1.7% |
| frontend `package` | 0 | 0% |
| frontend render | 0 | 0% |

`full` 38건의 reason은 다음과 같다.

| reason | 건수 | 근거 |
| --- | ---: | --- |
| `main-render-boundary` | 29 | `src/main.rs`; #3789 완료 전 보수적 full |
| `workflow-contract` | 4 | `.github/workflows/**` |
| `graphql-file-list-truncated` | 2 | replay 수집 결과 100/161, 100/169개로 잘림 |
| `wasm-contract` | 1 | `src/wasm_api.rs` 계열 |
| `classifier-contract` | 1 | #3792 자체 |
| `unclassified-path` | 1 | #3795의 `tools/agent_preflight.py` |

실제 workflow·WASM·classifier·미분류·mixed 경로가 모두 full로 닫혔다. rename과 PR 3,000개 API 경계는
이 60건에서 자연 표본이 없었고 Stage 1 단위·workflow 계약 테스트로만 확인된 상태다.

`src/main.rs`가 60건 중 29건을 full로 만든 것은 [#3789](https://github.com/edwardkim/rhwp/issues/3789)의
모듈 경계 분리가 후속 확장성에 실제 가치가 있음을 보여준다. 다만 #3789가 완료되기 전에는 이 29건을
부분 판정으로 바꾸지 않는다.

### 3.2 false negative 대조

- Rust 비렌더 16건 중 15건은 Native Skia가 실제 실행되어 모두 성공했고, 1건(#3778)은 기존
  review-only commit 재사용 fast-pass로 worker 전체가 이미 skipped 됐다.
- frontend `unit` 표본 #3785에서는 classifier가 생략 후보로 본 Rust lint/archive/8 shard/Native Skia와
  Canvas가 모두 실제 실행되어 성공했다.
- 비-full 코드 PR 21건의 CodeQL을 대조했다. Rust-only 20건 중 19건은 JavaScript/Python/Rust matrix가
  실제 실행되어 모두 성공했고 1건은 기존 fast-pass였다. frontend-only #3785의 세 언어도 모두 성공했다.
- #3722의 CI failure는 classifier가 유지하는 default-feature shard 4에서 발생했다. 같은 run의
  Native Skia는 성공했다. 즉 `rust_required=true`가 실제 Rust 회귀 차단을 유지하면서
  `native_skia_required=false`만 좁히는 판정과 충돌하지 않는다.
- classifier가 생략 후보로 판정한 실제 worker에서 code failure는 관찰되지 않았다.

따라서 이 표본에서 관찰된 false negative는 0건이다. 다만 successful legacy worker는 해당 worker가
불필요했다는 완전한 증명이 아니므로, 이는 경로 계약과 실패 결과를 함께 본 활성화 전 근거로만 쓴다.

## 4. 실제 runner-minute 절감 후보

| 축·표본 | n | 실제 생략 후보 시간 | 해석 |
| --- | ---: | ---: | --- |
| Rust 비렌더의 Native Skia | 15 | 합계 83분 29초, P50 5분 33초, P90 6분 18초 | Stage 4 runner 비용 절감; 대부분 critical path 밖 |
| #3785 frontend-only의 Rust worker | 1 | 33분 41초 | lint + archive + Native Skia + 8 shard 합계 |
| Rust-only의 JavaScript/Python CodeQL | 19 | 합계 66분 17초, P50 3분 28초, P90 3분 41초 | Stage 5 runner 비용 절감 |
| #3785의 Rust/Python CodeQL | 1 | 11분 24초 | JavaScript/TypeScript만 유지 |
| #3785의 Canvas visual diff | 1 | 5분 24초 | Stage 3 render 비영향 skip |
| **측정된 생략 후보 합계** | — | **200분 15초** | 최근 60 PR replay 기준, 단계 전체 활성화 상한 |

#3785의 main CI는 16분 4초였고 frontend package gate는 시작 후 2분 14초에 끝났다. Rust worker를 생략하면
aggregate 전제상 main CI critical path가 대략 2분 30초 수준까지 줄 수 있지만, 이는 queue가 같고 새
`unit` gate가 현재 package gate보다 느리지 않다는 조건부 추정이다. 반대로 live #3749에서는 Native
Skia가 archive/shard보다 먼저 끝났으므로 5분 21초를 생략해도 CI wall time은 거의 줄지 않고
runner-minute만 줄어든다.

200분 15초에는 frontend `unit`과 `package` 자체를 나눈 추가 절감은 포함하지 않았다. 반대로 full 38건은
절감 0으로 계산했으며, #3789 완료 뒤 재분류될 가능성도 포함하지 않았다.

## 5. 판정과 다음 관찰 게이트

classifier version 1을 보정해야 할 false negative는 발견하지 못했다. 그러나 Stage 3 활성화는 아직
진행하지 않는다.

1. merge 이후 live 고유 PR 표본이 4건뿐이고 완료 non-full 표본은 Rust 비렌더 1건뿐이다.
2. live non-full frontend `unit`, `package`와 `render_required=true`가 모두 0건이다.
3. historical replay 60건에도 `package`와 frontend render가 없어, Stage 1 fixture 외 운영 표본이 없다.
4. rename과 PR 3,000개 경계는 합성 계약 테스트만 있고 live 관측은 없다.

다음 판정은 completed live `classified` code run이 최소 5건 쌓인 뒤 갱신한다. 그 안에 frontend
`unit|package`와 `render_required=true`를 각각 한 건 이상 확보하지 못하면 해당 축은 계속 advisory로
남긴다. mixed full은 #3771의 WASM과 #3819/#3740의 미분류 tooling에서 관찰했으며, 추가 표본에서도
shadow step 실패·생략 후보 worker failure가 없어야 한다.

관찰 게이트가 충족되기 전에는 worker `if`, Render Diff trigger, CodeQL matrix를 변경하지 않는다.
다음 실측 갱신에서도 false negative가 0이고 frontend 두 mode와 render 축이 모두 관찰되면 Stage 3 PR을
시작한다.
