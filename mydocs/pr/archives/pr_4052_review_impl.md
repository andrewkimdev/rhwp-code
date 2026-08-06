# PR #4052 메인터너 보정 계획

## 대상과 경로

- PR: [#4052](https://github.com/edwardkim/rhwp/pull/4052)
- Issue: [#4051](https://github.com/edwardkim/rhwp/issues/4051)
- contributor source: `kevin9327/rhwp:pr/feature-batch-convert-tool`
- 보정 시작 기준 source SHA: `30ff9736d73f342d36dfc3aabf63b60f5080b990`
- 가시성 검토 branch: `review/kevin9327-4052-20260806`
- 메인터너 보정 commit: `f36434eb3`

```text
base route: collaborator_external_pr.md
modifiers: intake_and_review.md, local_validation.md, rework_and_exceptions.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_external_pr.md, intake_and_review.md,
                  local_validation.md, rework_and_exceptions.md
```

## 보정 범위

1. 같은 상대 경로·stem의 `.hwp`/`.hwpx`가 같은 PDF·페이지별 출력 폴더를 공유하지 않도록,
   변환 시작 전 충돌을 거부한다. 기존의 비충돌 출력 이름은 바꾸지 않는다.
2. 활성 포맷 중 하나라도 최종 실패하면 파일 전체를 Failed와 exit 1로 판정한다. 이미 성공한
   산출물은 지우지 않는다.
3. rhwp CLI 종료 코드 1만 재시도하고, 사용법·feature 부재인 종료 코드 2와 spawn 오류는 즉시 실패한다.
4. mock 기반 회귀 테스트와 README 계약을 같은 code commit에 포함한다.

## 완료한 검증

- `CARGO_TARGET_DIR=target/review-kevin9327-4052-20260806 CARGO_INCREMENTAL=0 cargo test -p batch-convert`
- `CARGO_INCREMENTAL=0 cargo fmt --check`
- `CARGO_TARGET_DIR=target/review-kevin9327-4052-20260806 CARGO_INCREMENTAL=0 cargo clippy -p batch-convert --all-targets -- -D warnings`
- 실제 `rhwp`의 native-skia 없는 all-formats 실행에서 PNG 실패가 exit 1로 반영되는지 확인
- 동일 stem HWP/HWPX가 rhwp 호출과 output root 생성 없이 거부되는지 확인
- `git diff --check`

통합 테스트 18건, fmt, clippy, diff 검사가 모두 통과했다. 실제 `field-01.hwp` all-formats
실행에서는 PDF·SVG·텍스트 산출물이 생성된 뒤 PNG가 exit 2로 실패했고, batch-convert는 재시도 없이
Failed 1 및 exit 1을 반환했다. 충돌 입력은 mock rhwp 호출 0회와 output root 미생성으로 거부됐다.

## source head 정렬

초기 최신 `devel` 가시화용 검토 이력에는 contributor 변경의 로컬 복사 commit이 들어 있었으므로,
원격 반영 전에 메인터너 보정만 실제 source `30ff9736d` 위로 replay했다. 현재 branch의 직접 부모는
`30ff9736d`이며, contributor 원 commit과 원격 ref는 변경하지 않았다.

## 원격 반영 게이트

보정 code/test commit은 contributor의 현재 source head가 보정 시작 SHA와 동일한지 재확인하고,
LFS 사전 판독과 dry-run 뒤 contributor fork의 같은 branch에 push했다. code 변경이므로
review-only fast-pass를 사용하지 않고 최신 head의 full CI를 확인했다. CI `31023915543`,
CodeQL `31023922453`, Render Diff `31023915292`는 모두 성공했다. 이제 review·오늘할일
trailing commit은 동일 source head에서 공용 fast-pass 조건으로 확인한다.
