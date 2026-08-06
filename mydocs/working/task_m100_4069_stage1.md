# Task #4069 Stage 1 완료 보고 — 중첩 RowBreak 재귀 cursor

- Issue: [#4069](https://github.com/edwardkim/rhwp/issues/4069)
- 기준: `upstream/devel` `d76d4e98b`
- 작업 브랜치: `local/task4069-redesign`
- 기준 문서: `samples/basic/issue2007_nested_cell_pagination_42065.hwp`
- 한컴 정답지: `pdf/basic/issue2007_nested_cell_pagination_42065-2020.pdf` (17쪽)

## 결과

수정 전 24쪽이던 42065 문서를 정답지와 같은 17쪽으로 조판했다. 2쪽의 큰 조문 비교 행은
페이지 하단까지 분할되고, 3쪽은 제2호부터 재개한다. 제1호 중복, 마지막 조항 누락, 셀 내부
겹침과 `overflowCellLines`는 없다.

메인 작업 트리의 Claude Code WIP와 stash는 변경하지 않았다. 최신 `upstream/devel`에서 만든
별도 worktree에서 원인을 다시 추적하고 구현했다.

## 원인

rhwp CLI의 render tree·SVG·쪽수 출력을 한컴 PDF와 대조했다. 바깥 셀의 `CellUnit` 원장은
중첩 RowBreak 표의 큰 행을 하나의 원자 높이로만 기록했다. 페이지네이션이 선택한 바깥 컷을
렌더러가 자식 표의 행·셀 컷으로 복원할 수 없어, 첫 조각은 하단을 비우고 후속 조각은 자식 표를
scalar clip으로 다시 계산했다. 깊은 1×1 중첩 흐름도 저장된 강제 쪽 경계를 잃어 24쪽으로
과다 조판됐다.

## 구현

1. 중첩 표 조각에 자식 `row/start_cut/end_cut` cursor를 기록하는 `NestedTableCut`을 추가했다.
2. 빈 host 문단의 auto-height RowBreak 행은 콘텐츠가 가장 긴 셀의 canonical `CellUnit` 경계를
   공통 높이 축으로 삼고, 모든 셀의 누적 cursor를 각 조각에 투영한다. 고정 높이 행과 rowspan은
   기존 원자 경로를 유지한다.
3. 여러 저장 페이지 프레임을 가진 1×1 중첩 표는 자식 셀의 canonical unit과 hard break를 재사용해
   깊은 흐름에서도 같은 원장을 쓴다. 단일 경계 문서는 기존 경로를 유지해 #2279를 보호한다.
4. 부분 렌더러는 페이지마다 자식 split을 다시 추정하지 않고 페이지네이션이 기록한 자식 cursor를
   `layout_partial_table`에 그대로 전달한다.
5. #2007 회귀를 17쪽 정확 일치로 강화하고, 2쪽 제1호/3쪽 제2호 재개와 제1호 비반복,
   3쪽 마지막 조항 존재를 render tree 텍스트로 고정했다.

## 검증

- `cargo test --test issue_2007_nested_cell_pagination`: 2 passed
- focused 회귀: #1073, #1891, #2097, #2279, #3595, #3637, form-002 SVG snapshot 통과
- `cargo test --profile release-test --tests`: exit 0, 전체 test binary 통과
- Native Skia: 라이브러리 58 passed, #2225 2 passed, direct PDF 4 passed
- `cargo fmt --check`, `git diff --check`: 통과
- `cargo clippy --all-targets -- -D warnings`: 통과
- `cargo test --doc`: 4 passed, 2 ignored
- `wasm-pack 0.15.0 build --target web --out-dir pkg`: 통과
- 전체 17쪽 visual sweep: SVG/PDF/review 17쪽 완료, 누락 0, 자동 flagged page 0

로컬 시각 증적은 `output/4069/README.md`에서 기준선 24쪽, 이전 Claude WIP, 최종 17쪽을 함께
확인할 수 있다. 자동 일치율은 폰트·안티앨리어싱 차이에 민감하므로 완료 판정은 정확한 쪽수,
cursor 중복·누락 회귀, overflow 0과 review 이미지의 사람 판정을 함께 사용한다.

## 단계 판정

코드·자동 검증·수행자 시각 검토는 완료했다. 작업지시자의 2~3쪽 시각 판정 전까지 원격 push,
PR 생성과 이슈 close는 수행하지 않는다.
