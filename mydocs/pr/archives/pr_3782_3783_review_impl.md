---
kind: review_plan
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-03
---

# twoLoop-40 누적 검토·반영 기록 — PR #3782, #3783

기준은 `upstream/devel`의 `9095cd52d`다. 열려 있는 twoLoop-40 PR 두 건을 기본 작업트리의
devel 기준 가시성 branch `review/twoloop-40-20260803`에 번호 순서로 `cherry-pick -x` 누적했다.
둘 다 최신 `devel`보다 이전 head였으며 충돌은 없었다.

| 순서 | 원 PR | 관련 이슈 | 원 적용 SHA | 검토 브랜치 commit | 충돌 |
| --- | --- | --- | --- | --- | --- |
| 1 | [#3782](https://github.com/edwardkim/rhwp/pull/3782) | [#3780](https://github.com/edwardkim/rhwp/issues/3780) | `81d1b9bc` | `f46e15245` | 없음 |
| 2 | [#3783](https://github.com/edwardkim/rhwp/pull/3783) | [#3781](https://github.com/edwardkim/rhwp/issues/3781) | `003483a5` | `e11868d15` | 없음 |

원 PR reviewer는 모두 `jangster77`로 지정했다. 원 contributor branch에는 어떤 push도 하지
않았다. 두 기능은 각각 typeset line advance의 panic 방어와 TAC 그림 anchor의 raw attr 동기화로,
같은 renderer/serialization 경로를 쓰되 서로 독립적이며 누적 순서에 의존하지 않는다.

## 검증 결과

1. #3782 focused unit 2 / 2, #3783 helper unit 2 / 2를 통과했다.
2. 기존 TAC migration focused 회귀 1 / 1 및 Hancom scenario 4 / 4를 통과했다.
3. 전체 library는 3,170 통과·7 ignored·실패 0이었다.
4. `cargo test --profile release-test --tests`는 최종 종료 코드 0으로 완료했고 SVG snapshot 8 / 8,
   visual round-trip baseline 3 / 3을 포함한다.
5. Native Skia library 58 / 58, placeholder 2 / 2, direct PDF 4 / 4를 통과했다.
6. `wasm-pack build --target web --out-dir pkg`, `cargo fmt --check`, `git diff --check`,
   `cargo clippy --all-targets -- -D warnings`도 통과했다.

두 이슈의 실제 재현 HWP/HWPX는 비공개 실서비스 자료라 기준 PDF가 없다. 따라서 이 누적 검토는
PDF와의 독립 시각 동일성을 주장하지 않으며, 자동 SVG/visual baseline과 관련 roundtrip 계약만
회귀 근거로 사용한다. 비식별화 fixture가 제공되면 한컴 PDF 기준 visual sweep을 보강한다.

## 다음 단계

1. [통합 PR #3881](https://github.com/edwardkim/rhwp/pull/3881)을 원격
   `review/twoloop-40-20260803` head로 생성했다.
2. code head `b88bd6e80`의 CI·CodeQL·Render Diff가 모두 통과했다. 이 문서-only trailing commit의
   최신 head도 review-only fast-pass로 재검증한다.
3. 최신 head의 fast-pass와 mergeability 확인 뒤 통합 PR을 merge하고, 원 #3782·#3783은 중복 구현을
   막기 위해 close한다.
4. merge 후에만 devel 동기화와 해당 review branch·격리 target 정리를 `post_merge.md` 절차로 수행한다.
