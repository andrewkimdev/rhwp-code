# PR #3749 검토 기록 — export-structure auto confidence 보정

- **PR**: [#3749](https://github.com/edwardkim/rhwp/pull/3749)
- **작성자**: `postmelee` (collaborator self-merge)
- **관련 이슈**: [#3695](https://github.com/edwardkim/rhwp/issues/3695), 상위 [#1528](https://github.com/edwardkim/rhwp/issues/1528)
- **base**: `devel`
- **보정 전 head**: `4df21a0219733d70911373f2824073437213580b`
- **보정 기준 devel**: `3d4863a0d58d9abf93544318e14856d3c72e92ce`
- **보정 code commit**: `fd45184f1`
- **상태**: 로컬 보정·전체 검증 완료, 최신 PR head CI 확인 전

작성 시점의 remote PR은 아직 보정 전 head다. 아래 값은 merge 판단용 확정값이 아니며 보정 push 뒤 다시
확인한다.

| 항목 | 작성 시점 참고값 |
| --- | --- |
| author / base | `postmelee` / `devel` |
| head | `4df21a0219733d70911373f2824073437213580b` |
| draft / mergeable | draft / MERGEABLE |
| merge state | BEHIND |
| 규모 | 12 files, +756 / -20 |
| issue 연결 | PR 본문의 `Closes #3695`, 상위 #1528 |
| 보정 local tree 규모 | 15 files, +1,226 / -19 (`upstream/devel` 대비, push 전) |

## 1. 라우팅

```text
base route: collaborator_self_merge.md
modifiers: intake_and_review.md, local_validation.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
  pr_review/collaborator_self_merge.md, pr_review/intake_and_review.md,
  pr_review/local_validation.md
current head: 보정 전 4df21a021, 보정 후에는 최신 PR head 재확인 필요
```

source 변경은 `src/document_core/queries/structure.rs`와 정책 회귀 테스트에 한정된다. parser/model의
읽기 결과, renderer/layout/paint, serializer, 공개 JSON shape와 CLI exit code는 바뀌지 않는다.

## 2. 원 리뷰와 판정

[GitHub review `4838218628`](https://github.com/edwardkim/rhwp/pull/3749#pullrequestreview-4838218628)는
보정 전 head에서 다음을 지적했다.

| 발견 | 판정 | 처리 |
| --- | --- | --- |
| 시장구조조사 목차·절 marker가 auto를 clause로 전환 | High, 재현 | 같은 PR에서 보정 |
| 본문 `제3조의 규정에 따라`가 auto를 clause로 전환 | High, 재현 | 같은 PR에서 보정 |
| 새 분기의 실문서 positive 부재 | Medium | 실제 협정서 기반 positive 추가 |
| 코퍼스 영향표 부재 | Medium | top-level 351·재귀 668 실측 |
| archive review 문서 부재 | Medium | 이 문서와 review_impl 추가 |
| auto 2-pass 비용 | Low, 방향 유효 | 조 증거 발견 뒤 텍스트 조립 생략, 잔여 trade-off 기록 |

#3744는 auto 선택을 비범위로 선언하고 있으므로 High 두 건을 이관하지 않았다.

## 3. 보정 정책

초기 PR은 편·장·절·관·조를 모두 Number보다 강한 증거로 봤다. 실측 결과 편·장·절·관은 법령뿐 아니라
정부 연구보고서의 일반 container 제목에도 쓰이므로 Number를 뒤집는 독립 증거로 부족하다.

보정 정책은 다음과 같다.

1. explicit `HeadType::Outline`이 있으면 outline이다.
2. Outline이 없고 Number와 충돌하면 confidence를 통과한 `조` 제목이 있을 때만 clause다.
3. 탭+쪽번호 목차 및 marker 뒤 조사형 상호참조는 `조` 증거에서 제외한다.
4. confidence를 통과한 조가 없고 Number가 있으면 outline이다.
5. Outline과 Number가 없으면 기존처럼 clause로 폴백한다.

단순 marker 개수 임계값은 목차 22건을 막지 못하고 단일 조문을 손상하므로 사용하지 않았다. explicit
`--mode outline|clause`와 #3693 `clause_heading_allowed()`는 변경하지 않았다.

## 4. 회귀 증거

테스트 선추가 red는 9 passed / 3 failed였다.

- 시장구조조사: 보정 전 `clause / 51`, 기대 `outline / 3`
- Number + 조사형 상호참조: 보정 전 clause, 기대 outline
- Number + `제1조 목적\t12`: 보정 전 clause, 기대 outline

green은 13 passed다. 실제 협정서에 테스트 안에서 Number style 문단을 추가한 positive는 원문의 `제1조`
구조가 clause로 남는지 확인한다. `제1조의무 규정`도 조사 `의`와 제목 음절을 경계로 구분한다.

## 5. 코퍼스 영향

이전 devel auto와 보정 auto를 같은 parsed document에서 비교했다.

| 범위 | 후보 | parse 성공 | parse 실패 | mode 변화 | node_count 변화 |
| --- | ---: | ---: | ---: | ---: | ---: |
| top-level | 351 | 348 | 3 | 0 | 0 |
| recursive | 668 | 665 | 3 | 0 | 0 |

parse 실패 3건은 password fixture다. 보정 전 PR에서 직접 재현된 시장구조조사
`outline 3 → clause 51` 회귀는 제거됐다. 현재 corpus에서 새 branch의 자연 발생 positive는 없으므로 실제
협정서 기반 controlled Number positive로 정책을 고정했다.

## 6. 로컬 검증

기준 tree는 최신 devel `3d4863a0d`를 merge commit `f2b93b7ee`로 통합한 뒤 보정 code
`fd45184f1`을 적용한 상태다. Cargo는 모두 `CARGO_INCREMENTAL=0`으로 순차 실행했다.

| 명령 | 결과 |
| --- | --- |
| `cargo test --lib document_core::queries::structure -- --nocapture` | 6 passed |
| `cargo test --test issue_3695_structure_auto_policy -- --nocapture` | 13 passed |
| `cargo test --test issue_3693_structure_clause_context -- --nocapture` | 3 passed |
| `cargo test --test cli_json_contract export_structure_ -- --nocapture` | 4 passed |
| `cargo test --profile release-test --tests` | 최종 exit 0, 실패 0 |
| `cargo fmt --check` | 통과 |
| `git diff --check` | 통과 |
| `cargo clippy --all-targets -- -D warnings` | 통과 |

## 7. 시각 검증

query-only selector와 JSON 구조 추출 테스트 변경이다. renderer/layout/paint, 페이지 수, 표·wrap·clipping,
fixture·golden을 건드리지 않으므로 시각 검증은 비대상이다.

## 8. 잔여 risk와 권고

- 조 증거가 없거나 늦게 나오면 auto와 build의 2-pass 비용은 남는다. 이번 보정은 조 증거를 찾은 뒤의
  불필요한 텍스트 조립만 제거한다.
- 편·장·절·관만 있고 Number도 있는 실제 clause 문서는 outline을 선택한다. 현재 corpus에 positive가
  없고 일반 보고서 false positive가 실재하므로 보수적으로 Number를 우선한 trade-off다.
- #3744의 explicit clause 앵커 만료·날짜·목 confidence는 후속으로 남는다.

최종 권고는 **보정 후 merge 후보**다. 단, draft·mergeable·head SHA와 이 문서의 CI 상태는 작성 시점
참고값이며 실제 merge는 최신 PR head GitHub Actions 통과와 작업지시자 승인 뒤에만 진행한다.
