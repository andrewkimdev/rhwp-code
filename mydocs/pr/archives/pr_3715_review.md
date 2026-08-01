---
kind: review
status: fixup-applied-ci-pending
canonical: mydocs/pr/archives/pr_3715_review.md
last_verified: 2026-08-02
---

# PR #3715 검토 기록 — export-structure clause marker·문맥 정확도

## 결론과 범위

[PR #3715](https://github.com/edwardkim/rhwp/pull/3715)는 `export-structure --mode clause`의
가지번호 marker 절단과 괄호형 marker 미인식을 보정하고, 텍스트만으로 모호한 `호`/`목` 후보를
열린 조문 문맥에서만 채택하도록 바꾼다. 작성자는 collaborator `@postmelee`이고 관련 이슈는
[#3693](https://github.com/edwardkim/rhwp/issues/3693), 상위 추적은
[#1528](https://github.com/edwardkim/rhwp/issues/1528)이다.

기본 경로는 **collaborator self-merge**이고 보조 경로는 접수·리뷰 기록과 로컬 검증이다.
시각 검증은 적용하지 않는다. 변경 대상은 읽기 전용 질의이고 renderer·layout·typeset·pagination
경로를 건드리지 않으므로 [접수와 리뷰 기록](../../manual/pr_review/intake_and_review.md)
2.6의 네 조건에 모두 해당하지 않는다.

원 구현은 코드 품질과 검증 게이트를 통과했고 #3693의 완료 조건 다섯 항목을 모두 충족한다.
검토에서 확인한 사항 중 저비용 항목은 이 PR 안에서 보정했고, 설계 판단이 필요한 항목은
아래 "남은 한계"에 기록해 후속 이슈로 넘긴다.

## PR metadata

| 항목 | 값 |
| --- | --- |
| PR / 작성자 | [#3715](https://github.com/edwardkim/rhwp/pull/3715) / `@postmelee` (collaborator) |
| base | `devel` |
| 검토 시작 시점 head | `1f10b7b929e66515d8a62b239aec031a99e2d40f` |
| 검토 시작 시점 mergeable | `CONFLICTING` / `DIRTY` — `mydocs/orders/20260801.md` 단독 충돌 |
| 원 규모 | +628 / −6, 9 files |
| 연결 이슈 | `Closes #3693`, `Parent: #1528` |

draft·mergeable·head SHA·CI 상태는 모두 이 문서 작성 시점 참고값이다. 최종 merge 조건은 최신
PR head의 GitHub Actions 통과와 작업지시자 승인이다.

## 변경 범위

핵심 변경은 `src/document_core/queries/structure.rs` 한 파일이다.

1. `classify_clause()`가 `제N조의M` 가지번호 suffix를 marker에 보존한다.
2. `1)`·`가)` 괄호형 구분자를 `호`/`목` 후보로 인식한다.
3. 새 `clause_heading_allowed()`가 열린 clause stack을 보고 약한 후보를 채택한다.
   `호`는 열린 `조|항`, `목`은 열린 `호`를 요구한다.
4. `build_structure()`가 clause 모드에서만 이 필터를 적용한다.

나머지는 #3693의 계획·단계·보고 문서와 오늘할일이다. 공개 `StructureDoc`/`StructureNode`
필드, CLI JSON 봉투, exit code는 불변이다.

## 렌더 영향과 시각 검증 판정

**해당 없음.** `get_structure_native`와 `export-structure`는 읽기 전용 질의다. 소비자는
CLI(`export-structure`, `batch export-structure`)뿐이고 Studio·WASM 바인딩 표면에는 아직
연결되어 있지 않다. golden/baseline, 신규 fixture, 기준 PDF 변경도 없다.

## 로컬 검증

`upstream/devel` `8277320be` 위에 PR head를 병합한 tree에서 실행했다. 변경 범위가
Rust parser/model/CLI 이므로 [로컬 검증](../../manual/pr_review/local_validation.md) 4.3의
focused → release-test 전체 → fmt → clippy 게이트를 적용했다.

| 게이트 | 결과 |
| --- | --- |
| `cargo test --lib document_core::queries::structure` | 6 passed |
| `cargo test --test issue_3693_structure_clause_context` | 3 passed |
| `cargo test --profile release-test --tests` | exit 0 — 405 binaries, 4,471 passed / 0 failed / 26 ignored |
| `cargo fmt --check` | 통과 |
| `git diff --check` | 통과 |
| `cargo clippy --all-targets -- -D warnings` | 통과 |

### 회귀 테스트의 판별력 확인

새 통합 테스트를 devel 코드에 대고 실행해 실제로 결함을 잡는지 확인했다.

| 테스트 | devel 코드에서 |
| --- | --- |
| `real_work_plan_date_is_not_a_clause_item` | **FAILED** — `2022.` marker 검출됨 |
| `real_handbook_toc_numbers_are_not_clause_items` | **FAILED** — 목차 항목 `(0,9)` 검출됨 |
| `real_agreement_keeps_items_under_article` | ok — 무회귀 가드 역할 |

두 negative 테스트는 겨냥한 결함을 실제로 잡는다.

## 코퍼스 영향 실측

`samples/` 351개 문서를 devel과 이 PR 양쪽에서 `--mode clause`로 실행해 비교했다.

| 항목 | 값 |
| --- | --- |
| 구조를 가진 문서 | 117 → 79 |
| 구조가 완전히 사라진 문서 | 38건 |
| 노드가 줄어든 문서 | 35건 |
| 노드가 늘어난 문서 | 9건 |
| 변화 없는 문서 | 35건 |
| 총 노드 | 11,313 → 10,446 (**−867, −7.7%**) |

감소분의 상당수는 의도한 과검출 제거다. 예를 들어 `hwp3-sample16-hwp5.hwp`(협정서)에서
사라진 60개 노드는 전부 첫 `제1조`(문단 945) **이전** 구간의 일반 번호 목록이고, 조문 구간의
계층은 그대로 유지된다. `hwpspec.hwp`(221 `호`), `exam_science.hwp`(20 `호`)처럼 조문 문서가
아닌 것들도 여기 속한다.

증가분은 `1)`·`가)` 신규 인식에서 온다. `2025 행정업무운영 편람(최종).hwp`는 288 → 591로
늘었고, 새로 잡힌 노드는 `1) 법령서식(영 제27조제1항)`, `가) 제도 개요`처럼 실제 조문형
항목이다.

**이 규모는 필드·봉투 계약과 별개다.** `export-structure`를 쓰는 조문 DB 파이프라인은 같은
입력에서 다른 출력을 받는다. `tests/cli_json_contract.rs`의 계약 테스트는 봉투만 고정하므로
이 변화를 검출하지 않는다.

## 이 PR 안에서 보정한 항목

| 항목 | commit |
| --- | --- |
| 최신 `upstream/devel` `8277320be` 병합, `mydocs/orders/20260801.md` 충돌 해소(양쪽 절 보존) | `0244cae23` |
| 가지번호 보존을 편/장/절/관까지 확장 | `21a99600d` |
| negative 회귀에 positive anchor 추가 | `2c226c25f` |

### 가지번호 단위 비대칭

원 구현은 `unit == '조'` 조건으로 가지번호 보존을 조에만 적용해 `제5장의2` → `제5장`,
`제2절의3` → `제2절`로 marker가 절단됐다. 가지번호는 조 전용이 아니므로 단위 조건을 제거했다.
`의` 뒤에 숫자를 요구하는 기존 `k > j + 2` 조건이 `제1조의무`·`제3조의 규정` 같은 오검출을
그대로 막고, `제3조의2의 규정` → `제3조의2`처럼 뒤따르는 조사도 marker에 포함하지 않는다.
단위 5종 positive와 오검출 4종 negative를 `clause_marker_keeps_variant_number_for_every_unit`
으로 고정했다.

### negative 회귀의 공허한 통과 방지

`real_work_plan_date_is_not_a_clause_item`은 부재 단언만 있었고 해당 샘플의 `node_count`는
0이다. 즉 clause 파이프라인이 무관한 이유로 노드를 만들지 못하게 되어도 계속 통과한다.
거부된 후보가 preamble/body 텍스트로는 남아 있음을 함께 고정했다. 편람 목차 테스트에도 같은
anchor를 더했다.

## 남은 한계 — 후속 이슈 대상

아래 두 항목은 `clause_heading_allowed()`의 설계 판단이 필요하므로 이 PR에서 고치지 않는다.
어느 쪽도 devel 대비 회귀는 아니며, 이번 변경으로 드러나거나 폭이 커진 기존 한계다.

### 1. 문맥 게이트가 한 방향으로만 작동하고 만료되지 않는다

앵커(`조`/`항`)는 같거나 낮은 level의 heading이 와야 stack에서 pop된다. 따라서 장/절/조
경계가 없는 문서에서는 한 번 열린 앵커가 문서 끝까지 유지되고, 게이트는 **첫 앵커 이전만**
막는다.

`samples/hwp3-sample10.hwp`(Oracle DBA 기술문서)가 이 경계를 드러낸다. 문단 2269·2270의
`①②`가 유일한 앵커이고, 이 PR 기준 `호` 노드 1,228건이 전부 문단 2269 이후이며 그 이전은
0건이다. 이번 변경으로 새로 추가된 260건에는 다음이 포함된다.

~~~text
para=2303  호 '1)'  '1) back up the datafiles'
para=2312  호 '1)'  '1) startup nomount;'
para=2313  호 '2)'  '2) alter database mount standby database'
~~~

devel에서도 1,016건이 있었으므로 회귀는 아니지만, 괄호형 인식 확대가 이 채널을 키운다.
앵커 만료 규칙(구역 경계 초기화, 거리 상한, 번호 연속성 중 하나)이 후속 과제다.

### 2. 날짜 오검출은 앵커 안에서 그대로 남는다

`clause_heading_allowed()`의 doc comment는 `2022. 1.`을 동기 사례로 들지만, 조가 열린
상태에서는 여전히 `호`로 검출된다.

~~~text
제1조(목적) → 호 marker="2022."  heading="2022. 1. 1. 일부개정"
~~~

부칙·개정연혁에서 흔한 형태다. 업무계획 negative 테스트가 통과하는 것은 그 샘플에 열린
조/항 앵커가 없기 때문이고, 날짜 형태 자체를 거르지는 않는다. devel도 동일하게 검출하므로
회귀는 아니다.

### 3. `목` 게이트가 `호`만 앵커로 인정한다

장/절 직속 `가.`/`나.` 본문 제목이 body로 강등된다. 편람에서 `가. ‘업무’의 개념`,
`나. 문서의 필요성` 같은 44건이 여기 해당한다. 앵커를 `조|항|호`로 완화해도 3건만 회복되고
(PR의 테스트 8건은 모두 통과), `장|절`까지 넣으면 negative 테스트가 겨냥한 목차 항목이 다시
들어온다. **clause 조상만으로는 목차와 본문을 구분할 수 없다**는 것이 실측 결론이고, 해결에는
문맥 외 신호(들여쓰기, ParaShape, 쪽번호 tail)가 필요하다. 편람 전체 `목` 노드 수는
189 → 327로 오히려 늘어 순증이지만, 특정 계층의 제목이 사라지는 것은 별개 문제다.

## CI 관찰

검토 시작 시점 head `1f10b7b`의 required 집계는 review-only fast-pass로 상속된 것이다
(candidate `08123790332d`, reason `build-and-test-green:success`). 그 candidate의
[CI run 30697006170](https://github.com/edwardkim/rhwp/actions/runs/30697006170)은 Lint,
Native Skia, 8개 default-feature shard가 모두 실제로 실행된 full run이므로 코드는 CI 검증을
받았다. 다만 기준이 `devel@f80b910aa`였다.

이번 보정으로 `8277320be` 병합과 source·test 변경이 들어갔으므로 fast-pass 조건이 성립하지
않고, 새 head에서 full CI가 다시 실행되어야 한다. 최종 판단은 그 결과를 확인한 뒤에 한다.

## 최종 권고

**보정 반영 후 merge 후보.** 코드 품질, 검증 게이트, 이슈 완료 조건을 모두 충족한다.
merge 전 조건은 다음과 같다.

- 최신 PR head의 GitHub Actions 통과
- 작업지시자 승인
- 위 "남은 한계" 1~3에 대한 후속 이슈 등록 (#1528 하위)

`--mode clause` 출력이 실문서에서 −7.7% 변하는 것은 의도된 정확도 교환이며, 근거와 규모를
이 문서와 최종 보고서에 남긴다.
