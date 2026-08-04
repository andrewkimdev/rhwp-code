# PR #3949 검토

## 결론

**수용 권고.** 최신 원 head `99cd22d`에서 재현 HWPX, 수정 전 실패/수정 후 통과하는
focused regression, overflow-cell 기준선 행, 진단 도구가 모두 갖춰졌다. 최신 GitHub Actions도
필수 검사 전체가 성공했고 병합 상태는 `CLEAN`/`MERGEABLE`이다.

`table_layout.rs`의 설명 주석에는 이전 조사 대상인 29쪽 및 이전 y 범위가 남아 있다. 실제
fixture와 테스트의 대상은 30쪽이므로 이 주석은 다음 보완에서 정정하는 편이 맞다. 다만 실행
경로와 회귀 계약에는 영향을 주지 않으므로 병합 차단 사유로 보지 않는다.

## 접수 및 최신 기준

| 항목 | 내용 |
| --- | --- |
| PR | #3949 `fix(layout): 중첩 표를 부모 셀 안에서 시작하게` |
| 작성자 | `planet6897` (Jaeuk Ryu) |
| 대상 | `devel` |
| 최초 원 head | `54863489377712683bef3a083a7d5b5f9e292a03` |
| 최종 검토 원 head | `99cd22d84dc1c8957cd5aef9e8e67e4de160621b` |
| 최종 검토 기준 devel | `aeb5805cb93c92b8c44036f4c1fe1f2df420119f` |
| 관련 이슈 | #3637 (closed) |

```text
base route: maintainer_general
modifiers: intake_and_review.md, local_validation.md,
           visual_fixture_evidence.md, rework_and_exceptions.md,
           multi_pr_update_branch.md
```

초기 head에는 재현 fixture와 이 변경을 직접 검출하는 회귀 테스트가 없어 2026-08-04에
fixture를 요청했다.

- 요청 comment: https://github.com/edwardkim/rhwp/pull/3949#issuecomment-5177241517
- 작성자 보완 응답: https://github.com/edwardkim/rhwp/pull/3949#issuecomment-5177488508

작성자는 최신 `devel`을 source branch에 병합하고 다음을 추가했다.

- `samples/issue3637/regulatory_impact_nested_table_escape.hwpx`
- `tests/issue_3637_nested_table_starts_inside_parent_cell.rs`
- `tools/nested_table_containment.py`
- `tests/fixtures/overflow_cell_baseline.tsv`의 fixture 기준선 `601`

검토 브랜치 `review/planet6897-20260804`는 source head를 일반 merge로 반영하여 VS Code
그래프에서 기준 `devel`과 PR 변경의 관계를 보존했다. `git diff --check`와 최신 기준
`devel`과의 정합도 확인했다.

## 변경 내용

비-TAC 경로의 `Control::Table`에서 계산한 중첩 표 anchor `nested_y`를 부모 셀 콘텐츠 영역의
바닥 이하로 내려가지 않게 제한한다. 이 y 값은 이어서 `LayoutRect`와 `layout_table` 호출에
사용되므로, 부모 셀 아래에서 표 컨테이너가 시작하는 overflow를 줄이는 변경이다.

## Fixture 및 한컴 기준 PDF

| 항목 | 결과 |
| --- | --- |
| fixture | `samples/issue3637/regulatory_impact_nested_table_escape.hwpx` |
| fixture SHA-256 | `e7b147f7cea66c97bed79085a3d89c2656037e0f711232f659ed3c7344984f62` |
| 한컴 2020 기준 PDF | [`pr_3949_planet6897_nested_table_hancom2020_reference.pdf`](../assets/pr_3949_planet6897_nested_table_hancom2020_reference.pdf) |
| 기준 PDF SHA-256 | `5b78a0bb9e66edd6c0469e524a5656e1dfccaf40718c75c28a71ae1693baa7b5` |
| 기준 PDF | 31쪽, A4, 561,034 bytes, `PrintToPDFEx`/`PrintMethod=0` |

기준 PDF 생성은 원격 HWP 2020 MCP에 실제 fixture를 전달해 수행했다. 변환 결과는
`validation=ok`, 편집기/출력 PDF 쪽 수 `31/31`, PDF 본문 검증 `ok`였다. 서버 URL, 인증 값,
서버 내부 경로는 검토 기록에 포함하지 않는다.

## 수정 전후 검증

PR의 여섯 줄만 일시적으로 역적용한 별도 target과 현재 patch target을 비교했다. 역적용 뒤에는
같은 commit patch를 적용해 작업트리가 원래 상태로 복구됐음을 확인했다.

| 측정 | 수정 전 | 수정 후 | 판정 |
| --- | ---: | ---: | --- |
| 부모 셀 밖에서 시작한 중첩 표 | 46건 / 10쪽 | 42건 / 9쪽 | 감소 |
| 30쪽의 쪽 아래 최대 깊이 | 1,293.6px | 951.1px | 342.5px 감소 |
| focused regression | 실패 | 통과 | 변경을 직접 검출 |

수정 전 focused test는 다음 계약 위반으로 실패했다.

```text
렌더 트리가 쪽 아래로 1293.6px 넘어갔다(상한 1100, 30쪽 y=2416.1 / 쪽 높이 1122.5)
```

수정 후 같은 테스트는 0.37초에 통과했다.

```bash
CARGO_TARGET_DIR=/home/tsjang/rhwp/target/review-planet6897-20260804 \
  CARGO_INCREMENTAL=0 cargo test --profile release-test \
  --test issue_3637_nested_table_starts_inside_parent_cell -- \
  --exact nested_table_starts_inside_its_parent_cell --nocapture
```

새 fixture의 기존 overflow가 기준선에 명시됐는지도 전체 baseline으로 확인했다.

```bash
CARGO_TARGET_DIR=/home/tsjang/rhwp/target/review-planet6897-20260804 \
  CARGO_INCREMENTAL=0 cargo test --profile release-test --test overflow_cell_baseline
```

결과는 `1 passed`, 284.43초다. 이는 fixture의 `601`줄을 허용하되 이후 증가를 실패로
검출하는 기존 baseline 정책과 맞는다.

추가 정적 검증도 통과했다.

```bash
cargo fmt --check
python3 -m py_compile tools/nested_table_containment.py
git diff --check
```

## 시각 검토와 잔여 위험

한컴 PDF를 기준으로 1~31쪽 전체 visual sweep을 수행했다. rhwp export/render tree는 36쪽으로
기준 PDF보다 5쪽 많아, 문서 전체 픽셀 점수는 이번 한 지점의 변경만으로 품질을 판정하는
지표가 될 수 없다.

- 결과 경로: `output/review-pr3949-nested-table/` (로컬 검토 산출)
- sweep 상태: 요청 1~31쪽 모두 완료
- 30쪽 단일 비교 diff: 수정 전 `15.96%`, 수정 후 `16.02%`

30쪽에는 이미 넓은 범위의 내용 흐름 차이가 존재하므로 위 0.06%p 차이를 전역 fidelity 개선이나
회귀로 해석하지 않았다. 대신 이 PR이 겨냥한 부모 셀 하단 초과량과 focused regression을
수용 근거로 사용했다. 한컴 PDF와 rhwp의 전역 페이지 수/내용 흐름 정합은 별도 레이아웃 과제로
남는다.

## CI 상태

최신 source head `99cd22d`의 GitHub Actions를 확인했다.

- `CI preflight`, lint, Canvas visual diff, Native Skia, slow shard, regular shard 1~3,
  `Build & Test`, CodeQL: 모두 성공
- review-only fast-pass 후보: `99cd22d`의
  [CI / Build & Test](https://github.com/edwardkim/rhwp/actions/runs/30901405999) 성공
- `mergeStateStatus`: `CLEAN`
- `mergeable`: `MERGEABLE`

초기 fixture 추가 직후 발생했던 slow shard 실패는 baseline에 새 fixture `601`행이 없었기
때문이며, `32a5f2f` 보완 뒤 최신 CI와 로컬 전체 baseline에서 모두 해소됐다.

## 비차단 보완사항

`src/renderer/layout/table_layout.rs`의 `[ #3637 ]` 설명은 이전 29쪽/이전 y 범위를 가리킨다.
작성자 응답과 실제 fixture/회귀 테스트는 30쪽의 `2075.0..2415.6`에서 `887.4..1228.0`으로
정정된 사례를 기준으로 한다. 설명 주석만 최신 재현 정보로 정리하면 추후 조사자가 혼동하지
않는다. 이 문구 차이는 runtime 동작, baseline, focused regression 또는 CI 결과를 바꾸지
않으므로 병합을 막지 않는다.

## 최종 권고

추가 변경 없이 PR #3949를 병합해도 된다. 병합 뒤에는 별도 후속 작업에서 전체 문서의
페이지 수와 내용 흐름 fidelity를 추적하면 된다.
