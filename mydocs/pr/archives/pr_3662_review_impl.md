---
kind: review-plan
status: active
canonical: mydocs/pr/archives/pr_3662_review_impl.md
last_verified: 2026-08-01
---

# lpaiu-cs 통합 검토·반영 계획 (PR #3662, #3672)

기준은 `upstream/devel`의 `c588c8240331a181271c6551e124aa1ff770d900`이고, 통합 후보는
[#3680](https://github.com/edwardkim/rhwp/pull/3680) `integrate/lpaiu-cs-20260801`이다.
원 PR별 판단은 [#3662 review](pr_3662_review.md), [#3672 review](pr_3672_review.md)에 분리한다.

| 순서 | 원 PR/성격 | source 또는 base 반영 | #3680 반영 | author 보존 |
| --- | --- | --- | --- | --- |
| 1 | #3662 tone-variant 회귀 | `f07d86a5…` | `fc06bf23…` | `lpaiu-cs` |
| 2 | #3662 계약·rollback guard | `0f6ccaa6…` | `2deb2820…` | `lpaiu-cs` |
| 3 | #3672 retry narrowing | `81306ef0…` → base `c588c824…` | source 중복 적용 없음 | 이미 devel merge |
| 4 | #3672 RawSvg P1 보정 | — | `01a572c3…` | maintainer |
| 5 | #3670 hwpctl guard 재검증 | base `b2b5c449…` | `94a37788…` | maintainer |
| 6 | #3315 계약 설명 범위 정정 | — | `71ccfeaa…` | maintainer |

`-x` trailer로 #3662 source와 integration commit을 추적한다. #3672는 #3680 code CI 중 별도 merge돼
최신 base에 들어갔으므로 rebase에서 source patch가 자동 제외됐다. 그 결과 #3680은 contributor 기능을
중복하지 않고, 발견한 P1 보정과 #3662의 author-preserving contract guard만 담는다.

## 수용 계약과 경계

- #3662의 계약은 "기존 image JSON field 보존 + schema minor 21의 additive metadata"다. byte-identical
  default serialization이나 PCX·모든 watermark public round-trip을 주장하지 않는다.
- #3672의 raster retry key narrowing은 유지하되, RawSvg의 비동기 decode 상태는 그 key로 판정하지 않는다.
  `rawSvgCount > 0`에서는 재사용을 포기해 timer/fallback을 다시 무장한다.
- `replaceAll` guard는 확정된 문자열 receiver만 제외하고 새로운 raw document 별칭은 guard failure로
  드러낸다. complex receiver expression parsing은 후속 P2다.
- `imageRetryCounts`의 page pool release 누적은 후속 P2로 남긴다. 이번 merge가 이를 해소했다고
  기록하지 않는다.

## 검증·tail·merge 순서

1. code candidate `71ccfeaaa6c911340d18371e348a6b53ff33f4a0`은 최신 base에서 full CI를 완료했다:
   [CI](https://github.com/edwardkim/rhwp/actions/runs/30687707964)의 lint, frontend gate, Native Skia,
   test archive, default-feature 8 shards, `Build & Test`,
   [CodeQL](https://github.com/edwardkim/rhwp/actions/runs/30687707945),
   [Canvas visual diff](https://github.com/edwardkim/rhwp/actions/runs/30687707952)가 모두 success다.
2. 이 archive review 두 건, 이 공유 계획, 오늘할일만 single-parent review-only commit으로 같은
   #3680 head에 추가한다. source/test/workflow/golden/baseline/기존 fixture는 tail에 섞지 않는다.
3. 추가 전 planned Markdown 네 경로의 `filter`·`diff`·`merge` attribute가 모두 `unspecified`이고
   `git lfs status`에 push/commit/staged object가 없음을 확인했다. 따라서 LFS 대상이 아님을 먼저
   판정한 뒤 `GIT_LFS_SKIP_PUSH=1` dry-run과 same-branch push를 한다.
4. `71ccfeaa`의 성공 `Build & Test`를 candidate로 삼아 최신 review-only head의 CI preflight와
   `Build & Test` aggregate가 success인지 확인한다. fast-pass가 fallback하면 full CI 종료까지 기다린다.
5. latest head가 `CLEAN`·`MERGEABLE`이고 required check가 성공하면, 기존 자동 승인 범위에 따라
   #3680을 **merge commit**으로 merge한다. #3315에는 closing keyword를 쓰지 않는다.
6. merge SHA 확인 뒤 열린 원 #3662에는 실제 줄바꿈 body-file comment로 author-preserving 통합·검토
   근거·검증을 남기고 supersede close한다. 이미 merge된 #3672는 close하지 않고 RawSvg 보정 사실과
   감사의 review comment만 남긴다. #3315는 open 상태를 재확인한다. #3648은 이미 closed이며 #3670의
   maintainer 기록이 있으므로 중복 close하지 않는다.
7. `devel` fast-forward sync 후, 이번 작업이 만든 exact integration/review/local PR refs와
   `target/review-lpaiu-cs-20260801`만 post-merge 절차에 따라 정리한다. contributor fork branches는
   삭제하지 않는다.
