# Task #4069 Stage 2/3 최종 보고 — 저장 프레임 경계 보존

- Issue: [#4069](https://github.com/edwardkim/rhwp/issues/4069)
- 기준: `upstream/devel` `d76d4e98b`
- 작업 브랜치: `local/task4069-redesign`
- Stage 1 중간 커밋: `7c9ce05e6`
- 기준 문서: `samples/basic/issue2007_nested_cell_pagination_42065.hwp`
- 한컴 정답지: `pdf/basic/issue2007_nested_cell_pagination_42065-2020.pdf` (17쪽)

## 최종 결과

Stage 1의 2·3쪽 재귀 cursor를 유지하면서 두 종류의 저장 프레임 경계를 추가로 복원했다.

- 10쪽: 셀 안 같은 문단의 저장 `lineseg`가 `58620→0HU`로 되감기는 지점에서 쪽을 나눈다.
  다음 프레임의 문장은 10쪽 셀 상단에 겹치지 않고 11쪽에서 재개한다.
- 15쪽: `조달청` 제목 다음의 짧은 1×1 자식 표를 원자로 미루지 않고 현재 저장 프레임의
  남은 공간에서 시작한다. 표의 말미까지 15쪽에 이어지고 `<이해관계자 협의>`는 16쪽에서 시작한다.
- 전체: 한컴 정답지와 같은 17쪽이며, 2·3·10·11·15·16쪽의 누락·중복·겹침 계약을 모두 만족한다.

## 구현 계약

### 문단 내부 저장 프레임

`CellUnit`과 재귀 `NestedFlowFragment`에 `stored_frame_break_before`를 별도로 기록했다. HWP5 또는
HWP5-origin HWPX의 비합성 저장 `lineseg`가 역행하고, 직전 줄의 끝이 현재 body 높이 절반 이상에
도달한 경우만 저장 프레임 경계로 인정한다. 이 의미 경계는 일반 문단 사이 hard break의
orphan/sliver 완화 규칙으로 흡수하지 않는다.

### 저장 프레임 말미의 짧은 자식 표

문단이 정확히 하나의 1×1 자식 표를 host하고 다음 문단이 저장 프레임으로 rewind하는 경우에는,
한 페이지보다 짧은 자식 표도 canonical fragment로 푼다. 다음 프레임 경계는 엄격히 보존하되,
일반 단일 페이지 중첩 표에는 기존 원자 배치를 유지한다.

### 빈 Enter와 #2430 회귀

작업지시자가 #2430 정답지의 14쪽을 한컴 편집기와 대조해 셀 안의 빈 Enter가 무시되는 차이를
확인했다. 진단 결과 그 빈 문단은 비선형 부모 셀에서 `0HU`로 rewind하지만 한컴의 저장 프레임
증거가 아니다. 이를 경계로 승격하면 39쪽 정본이 38쪽으로 줄었다.

따라서 텍스트·control이 없는 실제 빈 문단은 1×1 선형 RowBreak 부모의 저장 vpos를 보존하는
경우에만 프레임 증거로 쓴다. 텍스트 또는 control이 있는 다음 문단은 기존 의미 판정을 따른다.
`issue_2430_cell_rewrap_threshold_no_oversplit`의 39쪽 계약이 과다분할 40쪽과 과소분할 38쪽을
모두 검출하도록 설명을 보강했다.

## 검증

### 자동 테스트

- `cargo test --profile release-test --tests`: exit 0
  - 라이브러리 3,290개: 3,282 passed, 8 ignored, 0 failed
  - 모든 통합 test binary 통과
- 핵심 focused 회귀 33개 통과
  - #4069 4개: 17쪽, 2·3쪽 cursor, 10·11쪽 frame, 15·16쪽 child table
  - #2430 1개: 39쪽
  - #1891, #2279, #3637, `issue_rowbreak_chart_overlap` 포함
- Native Skia: 라이브러리 58 passed, #2225 2 passed, direct PDF 4 passed
- `cargo fmt --check`, `git diff --check`: 통과
- `cargo clippy --all-targets -- -D warnings`: 통과
- `cargo test --doc`: 4 passed, 2 ignored

### WASM

프로젝트 표준 Docker Compose 절차로 `wasm-pack 0.15.0` 빌드를 새로 수행했다.

- 최종 `pkg/rhwp_bg.wasm` SHA-256:
  `6a9efdc4bc1d4931043447ff0c6cf5c6ab4916c2ef55c8c5c5bdc5d956dfcb68`
- 생성된 `rhwp.js`와 `rhwp.d.ts`: 기존 바인딩과 차이 없음
- Node 직접 로드: #4069 17쪽, #2430 39쪽
- WASM render: 15쪽에 조달청 자식 표 시작·말미 존재, 다음 프레임 부재;
  16쪽에 `<이해관계자 협의>` 존재

일반 `rhwp-studio npm run dev`는 루트 `pkg`를 alias한다. 빌드 산출물은 메인 작업공간의
`pkg/rhwp_bg.wasm`에 적용했으며, 실행 중인 WASM 인스턴스는 dev 서버 재시작과 브라우저 강력
새로고침 후 교체된다. `dev:subsecond`는 별도 WASM 경로이므로 이 적용 대상이 아니다.

### 시각 검증

`output/4069/stage3-final-validated/`에서 한컴 2020 PDF 17쪽 전부를 비교했다.

- SVG·render tree·PDF raster·compare·overlay·review: 각각 17쪽
- 누락 페이지: 0
- 자동 구조 후보 `flagged_page_count`: 0
- 수행자 직접 검토: 2·3·10·11·15·16쪽 흐름 정합

폰트·안티앨리어싱 차이로 pixel/ink 일치율 자체는 완료 판정으로 쓰지 않았다. 정확 쪽수,
render-tree 텍스트 계약, 자동 구조 후보, 한컴 PDF review를 함께 판정 근거로 사용했다.

## 단계 판정

로컬 구현·회귀·WASM·시각 검증은 완료했다. 메인 작업공간의 기존 Claude Code/user WIP는
되돌리거나 덮어쓰지 않았다. 원격 push, PR 생성, GitHub comment와 이슈 close는 별도 승인을
받기 전까지 수행하지 않는다.
