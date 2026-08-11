---
kind: pr-review
status: local-functional-review-complete-visual-fidelity-pending
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-11
---

# PR #4520 리뷰 - 앵커 줄 계상·인라인 표 흐름 보정

| 항목 | 검토 기록 |
| --- | --- |
| 원 PR | [#4520](https://github.com/edwardkim/rhwp/pull/4520) · @planet6897 |
| base / 원 head | `devel` / `b0f82f0fc600ec26c50a79ced0b012fa506b54fc` |
| 규모 | 13 files, 렌더러·HWP3 lineage·재현 fixture·회귀 test |
| 작성 시점 상태 | OPEN, `MERGEABLE`, `CLEAN` (merge 직전 재확인 필요) |

## 범위와 메인터너 보정

글앞/글자처럼 표와 빈 Shape 앵커 문단이 저장 line-seg의 흐름을 예약하도록 보정하고, 재래핑된
인라인 표가 저장 line-height를 줄마다 중복 상속하지 않게 한다. HWP3 출처 표식도 lineage에 연결한다.

기존 앵커 줄 계상 보정 뒤 원 PR에 `b0f82f0`이 추가됐다. 이 변경은 HWP5 네이티브 문서에서만,
절대배치 중첩표가 셀 높이를 과대 계상하는 경우 저장 vpos 사다리 끝점으로 캡한다. HWPX 계산 lineseg와
쪽을 넘는 거대 중첩표에는 적용하지 않아 기존 흐름형 표의 페이지 수 회귀를 막는다. 누적 중 생긴 서식
차이는 메인터너 commit `f92d15e02`, `7565f6820`으로 정규화했으며 기능 의미는 바꾸지 않는다.

## 검증과 시각 증적

- 앵커 흐름 focused test 4건, HWP3 lineage와 관련 `issue_1892`/`issue_1692` 15건이 통과했다.
- release-test nextest는 5,703 passed, 정책 skip 35로 완료했다. 최초 실행의 디스크 부족은 종료된
  review target 정리 뒤 동일 head에서 재실행해 성공했으므로 코드 실패로 판정하지 않았다.
- HWP 2020 MCP 기준 PDF와 rhwp SVG는 #4490 2/2쪽, #4491 38/38쪽으로 페이지 수가 일치했다.
  페이지 경계 owner 후보는 두 fixture 모두 0건이다.
- #4490 p2와 #4491 p9에서 표/도식 뒤 본문이 기준과 같은 쪽에 남고 겹치지 않음을 확인했다.
  #4491 p6의 table/footer 구조 후보는 기준 PDF와 직접 비교해 시각적 겹침이 없는 후보로 분류했다.
- 최신 누적 head는 #4566의 47쪽 `LAYOUT_TABLE_OVERLAP` 통합 회귀도 통과했다. 이는 #4520과
  함께 적용했을 때 최상위 표 겹침을 관측하는 경로가 render tree 실측과 일치함을 확인한다.

기준 PDF와 asset, SHA-256 및 수치는 누적 이행 기록에 보존한다. 최신 누적 head에서 다시 비교한
#4490 p2 / #4491 p9 / p26 / p36의 PDF pixel diff는 각각 `12.60%`, `12.97%`, `25.95%`, `19.33%`다.
특히 p26은 줄바꿈·수직 배치가 기준 PDF와 달라 이 수치를 단순 글꼴 raster 차이로 취급할 수 없다.
앵커 흐름의 기능 회귀와 전체 시각 fidelity는 별도 판정이다.

**최종 권고: #4520의 기능 회귀는 통과했지만, 이 PR을 한컴 PDF 전면 fidelity 수용 근거로 사용하지
않는다. 최신 통합 head의 CI와 작업지시자 승인, 또는 별도 fidelity 과제 분리 판단 뒤에 수용 여부를
결정한다.**
