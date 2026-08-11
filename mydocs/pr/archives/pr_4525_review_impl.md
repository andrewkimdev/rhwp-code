---
kind: review-implementation
status: pending-push
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-11
---

# PR #4525 메인터너 보정 이행 기록

## 고정 기준

- 원 PR: [#4525](https://github.com/edwardkim/rhwp/pull/4525)
- 원 기여자 / source: @humdrum00001010 / `task_m100_4334_structural_node_id`
- 원 code head: `dc42d10b5b2e52ce0bc455446ef3f2f72ba6dbdd`
- 로컬 가시성 브랜치: `review/humdrum00001010-4525-20260811`
- 보정 권한: PR metadata의 `maintainerCanModify=true`

## 보정 범위

1. `stableIndex`가 숫자에서 문서 경로 배열로 바뀐 #4334 계약에 맞춰
   `rhwp-studio/e2e/topmost-hittest.test.mjs`의 E2E 단언을 갱신한다.
2. `zOrder`는 숫자, `shapeStable`과 `imageStable`은 정수 배열임을 함께 검증한다.
   단순 배열 존재만 확인해 잘못된 JSON 값을 수용하지 않는다.
3. Rust 정렬 구현, 문서 경로 생성, fixture, CI workflow, 원 기여자 commit은 변경하지 않는다.

## 완료한 보정과 검증

1. `c4e3a0883`에서 E2E assertion을 배열 계약으로 바꿨다. `stableIndex`가 존재만 하는
   비정상 배열이 통과하지 않도록 양쪽 배열의 정수 원소도 함께 확인한다.
2. `cargo nextest run --cargo-profile release-test --target-dir /home/tsjang/rhwp/target/pr-review --lib issue_4334`를
   실행해 정렬 경로 관련 3건을 통과했다.
3. `wasm-pack build --target web --dev`를 실행해 Studio용 웹 WASM을 재생성했다.
4. `VITE_URL=http://127.0.0.1:7702 node e2e/topmost-hittest.test.mjs --mode=headless`를
   실행해 `shapeStable=[0,0,2]`, `imageStable=[0,0,3]`, 겹침 클릭의 `shape` 선택을 확인했다.
5. 최신 `upstream/devel`과 merge tree를 만들고 `git diff --check upstream/devel...HEAD`를 통과했다.
6. `dc42d10..c4e3a0883`의 변경은 LFS 대상이 아닌
   `rhwp-studio/e2e/topmost-hittest.test.mjs` 한 파일뿐임을 확인했다. 원격 source head와
   PR head도 모두 `dc42d10`으로 일치한다.

## 원격 반영 단계

1. 작업지시자의 push 승인 뒤에만 LFS dry-run을 수행하고 contributor source branch에
   `c4e3a0883`을 push한다.
2. 보정은 code/test commit이므로 review-only fast-pass를 적용하지 않고, 새 PR head의
   Full CI를 확인한다.
3. CI 뒤 최신 head, mergeability, 작업지시자 merge 승인을 다시 확인한다.

## 롤백 경계

- 보정은 E2E assertion 한 곳에 한정한다. 검증 실패 시 이 commit을 source branch에 push하지 않는다.
- 원 기여자 commit은 rebase, amend, reset, force-push하지 않는다.
