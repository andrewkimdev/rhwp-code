---
kind: pr_review_plan
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-31
---

# PR #3536 처리 계획 — CanvasKit P40 guard

## 입력과 commit 경계

| 구분 | SHA / 범위 | 처리 원칙 |
| --- | --- | --- |
| contributor code | `e5412d1` .. `fc68f75` (10 commits) | 저자 commit을 rewrite·amend·force-push하지 않는다 |
| base update | `b14d245` | 최신 `devel` `a435f41`을 포함하는 2-parent merge이며 code CI의 정확한 대상이다 |
| review-only tail | archive review, 본 계획, 오늘할일 | `mydocs/`만 추가해 fast-pass A 경로를 사용한다 |

## 실행 순서

1. `b14d245`의 full CI·CodeQL·Canvas visual diff·`Build & Test` success와 `MERGEABLE`·`CLEAN`을 확인한다.
2. review-only tail의 source/local/remote SHA를 대조하고 변경 파일의 LFS attribute를 먼저 판독한다.
   LFS 대상이 없을 때만 `GIT_LFS_SKIP_PUSH=1` dry-run 후 source fork `render-p40`에 push한다.
3. 최신 head에서 CI preflight가 A 경로 fast-pass를 선택하고, preflight와 최종 `Build & Test` aggregate가
   success인지 확인한다. heavy worker skip은 review-only tail에서 정상이다.
4. approval과 merge를 수행한 뒤 merge SHA가 `upstream/devel`에 포함됐는지 확인한다. #536은 P40의 남은 단계를
   추적하므로 닫지 않고, merge·검증·추적 유지 사실을 comment로 남긴다.
5. contributor PR에는 실제 줄바꿈을 담은 body file로 검토·감사·merge 결과를 게시한다. 그 다음 local review
   branch와 정확한 `target/review-seo-rii-3536-20260731`만, Cargo/Rust 작업이 없음을 확인한 뒤 정리한다.

## 중단·rollback 기준

- source head 또는 base가 변하면 SHA·CI·mergeability를 다시 확인하고, review-only 범위를 벗어나면 full CI로
  전환한다.
- fast-pass preflight 또는 final aggregate가 실패·pending이면 merge하지 않는다. contributor code history를
  rewrite하지 않고 원인을 새 head에서 분리한다.
- LFS 대상, object 또는 lock이 발견되면 skip-push 경로를 쓰지 않고 LFS 상태를 확인한다.
- broad sweep의 text/equation hard gate 16건은 P40 image acceptance로 숨기지 않는다. P40 image-crop 결과나
  current Canvas visual diff가 실패하면 merge를 중단하고 별도 renderer 수정으로 처리한다.
