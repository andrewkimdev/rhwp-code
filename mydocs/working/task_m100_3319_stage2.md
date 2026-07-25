# Task M100 #3319 Stage 2 — Studio 선택 및 회귀 검증

- Issue: [#3319](https://github.com/edwardkim/rhwp/issues/3319)
- Branch: `task/3319-hwpx-ole-selection`

## Studio 실측

최신 `pkg`를 로드한 headless Chrome에서
`rhwp-studio/e2e/issue-2069-ole-object-selection.test.mjs --mode=headless`를 실행했다.

`SO-SUEOP.hwpx` 1쪽 HMapsi preview는 다음 layout control로 노출됐다.

| 항목 | 값 |
| --- | --- |
| type | `ole` |
| bbox | `x=132.6`, `y=142.5`, `w=197.5`, `h=660.4` |
| 원본 ref | `secIdx=0`, `paraIdx=0`, `controlIdx=2` |
| 클릭 뒤 선택 ref | `sec=0`, `ppi=0`, `ci=2`, `type=ole` |

E2E는 모든 assertion을 통과했다. 실제 선택 테두리·회전 핸들이 표시된 화면은
[`so_sueop_hwpx_ole_selected.png`](../report/assets/task_m100_3319/so_sueop_hwpx_ole_selected.png)에
보관했다.

## 검증 결과

| 명령 또는 범위 | 결과 |
| --- | --- |
| `cargo test --profile release-test --test issue_2069_ole_object_selection -- --nocapture` | 10 passed |
| `cargo test --profile release-test --test issue_1692 -- --nocapture` | 11 passed |
| 사용자 수동 `wasm-pack build --target web --out-dir pkg` | 성공, 최신 `pkg/rhwp_bg.wasm` 사용 |
| `node e2e/issue-2069-ole-object-selection.test.mjs --mode=headless` | 통과, #3319 click/ref assertion 포함 |
| `cargo fmt --check`, `npx tsc --noEmit`, `git diff --check` | 통과 |

## 다음 단계

현재 변경은 PR 생성 전 준비 상태다. PR 생성·push·Issue close는 작업지시자 승인 뒤 진행한다.
