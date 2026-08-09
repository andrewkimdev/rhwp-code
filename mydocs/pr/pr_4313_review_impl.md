---
kind: review_plan
status: in-progress
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-10
---

# humdrum00001010 누적 PR 검토 계획 - #4313 외 19건

기준은 `upstream/devel`의 `e48fe86947fbf9a44b1b98c7037150751af541ab`이다. 원 PR
20건은 모두 이 commit에서 직접 분기했고 merge commit이 없다. #4315는 의도적인 red 테스트
Draft이므로 이 후보에서 제외한다. 누적 브랜치는 검증 전용이며 원 PR branch나 devel에 push하지 않는다.

## 적용 그룹과 순서

1. renderer/document_core: #4313, #4316, #4360, #4363, #4365, #4374, #4380, #4382, #4394, #4420
2. HWPX: #4409, #4411, #4415, #4417, #4421, #4425
3. clipboard: #4416, #4426
4. 독립 serializer: #4419, #4434

그룹 안에서는 오래된 PR 번호 순서로 기능·문서 commit을 원 순서대로 체리픽한다. 각 PR 적용 뒤
`git diff --check`와 상태를 확인하고 원 head와 누적 commit 대응표를 이 문서에 추가한다.

## 검증 게이트

1. PR별 focused test를 변경 축별로 순차 실행한다.
2. renderer/layout 후보는 WASM build와 대표 fixture 시각 검증을 수행한다.
3. 최종 누적 후보에서 고정 `target/pr-review`를 사용해 전체 release-test, Native Skia 3종,
   fmt, diff check, clippy를 순차 실행한다.
4. 원 PR별 최신 head, mergeable, required checks를 다시 확인하고 작업지시자에게 merge 승인을 요청한다.

## 중단과 rollback

체리픽 충돌은 임의로 해소하지 않는다. 해당 PR 직전 checkpoint에서 중단해 충돌 파일과 양쪽 의도를
보고한다. 원 PR head·기존 local ref·사용자 작업은 재작성하거나 삭제하지 않는다.
