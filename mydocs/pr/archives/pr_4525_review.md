---
kind: pr-review
status: pending-push
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-11
---

# PR #4525 리뷰 - stableIndex 문서 경로 정렬 계약

## 라우팅과 접수

```text
base route: collaborator_external_pr.md
modifiers: intake_and_review.md, local_validation.md,
  visual_fixture_evidence.md, multi_pr_update_branch.md,
  rework_and_exceptions.md
```

| 항목 | 문서 작성 시점 참고값 |
| --- | --- |
| PR | [#4525](https://github.com/edwardkim/rhwp/pull/4525) |
| 작성자 / source | @humdrum00001010 / `task_m100_4334_structural_node_id` |
| 원 code head | `dc42d10b5b2e52ce0bc455446ef3f2f72ba6dbdd` |
| 메인터너 보정 | `c4e3a08839fcee53b4e18308cc0b0a526d1cc7b6` |
| 로컬 가시성 브랜치 | `review/humdrum00001010-4525-20260811` |
| source 수정 권한 | `maintainerCanModify=true` |
| 규모 | 원 PR 16파일, 1,100 추가 / 32 삭제; 보정 1파일, 5 추가 / 2 삭제 |
| merge tree | 최신 `upstream/devel` 기준 충돌 없음, whitespace 오류 없음 |

## 검토와 메인터너 보정

원 PR은 `paper_node_sort_key`와 Studio hit-test의 `stableIndex`를 스칼라에서 문서 경로 배열로
바꿨다. Rust와 TypeScript 본 구현은 배열 사전식 비교를 사용해 정합했지만,
`topmost-hittest.test.mjs`는 여전히 `typeof stableIndex === 'number'`를 요구했다. 이 수동 E2E를
실행하면 정상 WASM 응답도 실패하므로 merge 차단 결함이었다.

`c4e3a0883`은 `zOrder`의 숫자 계약을 유지하면서 `shapeStable`과 `imageStable`이 정수 배열임을
검증한다. 정렬 구현, 렌더 출력, fixture, CI workflow와 원 기여자 commit은 변경하지 않는다.

## 완료한 검증

- `cargo nextest run --cargo-profile release-test --target-dir /home/tsjang/rhwp/target/pr-review --lib issue_4334`:
  정렬 경로 관련 3건 통과.
- `wasm-pack build --target web --dev`: 통과.
- `VITE_URL=http://127.0.0.1:7702 node e2e/topmost-hittest.test.mjs --mode=headless`:
  실제 WASM 응답의 `shapeStable=[0,0,2]`, `imageStable=[0,0,3]`와 겹침 클릭의 `shape` 선택을 확인.
- 최신 `upstream/devel` merge tree 및 `git diff --check upstream/devel...HEAD`: 통과.

원 PR의 기존 CI는 원 code head 기준이다. 이번 보정은 code/test 변경이므로 source branch를
rebase하지 않고, push 뒤 생성되는 최신 head의 Full CI를 별도로 통과해야 한다.

## 최종 권고

**push 승인 후 메인터너 보정을 source branch에 반영하고, 새 head의 Full CI 통과를 조건으로 merge 권고.**
그 전에는 GitHub review, comment, push, merge를 수행하지 않는다.
