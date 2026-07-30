---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3564 리뷰 — 10k 서베이 r26 보고서 (review-only fast-pass)

- PR: [#3564](https://github.com/edwardkim/rhwp/pull/3564)
- 작성자: `planet6897` (재기여자 — r23~r26 서베이 시리즈, 어제 #3558 통합 6건)
- 역할: maintainer 일반 경로 + **review_only_fast_pass** (PR 전체가 허용 review-only 범위)

## 라우팅과 작성 시점

```text
base route: maintainer_general.md
modifiers: intake_and_review.md, review_only_fast_pass.md, local_validation.md(4.3 mydocs행)
current head: 73c1cd4b822674fe6a8311f0d7cb927fc38a4c69
mergeable / merge state: MERGEABLE / behind (작성 시점 참고값)
```

## 변경 범위

`mydocs/report/survey_10k_r26_20260729.md` 신규 1파일(+162) — 회차형 측정 기록 네이밍
규약(`{주제}_{회차}_{YYYYMMDD}.md`, report/ 직배치) 준수. fast-pass 허용 범위
(mydocs 전체) 안이며 source/test/workflow/골든 변경 없음.

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| fast-pass 허용 범위 | mydocs 1파일 신규 | 허용 범위 안 |
| `git diff --check` (devel...head) | 통과 | 공백 오류 없음 |
| head CI | preflight 2종 success, heavy 전부 skipped, **Build & Test aggregate success** | fast-pass 정상 동작, required check 충족 |
| 수치 내부 정합 재계산 | PI 분모/분자(9,905·9,286), 실패 619=390+229, 코호트 합 619·문서 합 9,905, ±1쪽 75.1%, 상한 +1.8pp | 전부 일치 |
| 기준선 규율 | r15~r25 동일 표본(seed), `RHWP_FONT_PATH` 명시(#2898), PI 정의 기준선 동일 명시 | r23 폰트-클린 계보 준수 |
| 관련 이슈 | 회귀 1건 → [#3561](https://github.com/edwardkim/rhwp/issues/3561) 등록 확인(open) | 보고-이슈 연결 정합 |

Cargo 게이트는 4.3 mydocs행에 따라 생략. 시각 검증 비적용(문서 PR).

## 내용 판단

1. **핵심 결론(제출 PR 6건 + 보류 패치 2건 = 쪽수·배치 무영향)** 은 문서 단위 대조
   (회귀 1 · 개선 0)로 뒷받침된다. 전체 비율이 아닌 문서 단위 대조를 쓴 방법이 옳다.
2. **회귀 1건 귀속이 모범적** — 193커밋을 8단계 이분법으로 좁혀 `f8e0c37fd`(표 셀 중첩
   머리말 수집) 특정, 자기 패치 전부 제거한 순수 devel 재현 확인, 타 기여자 축이라
   수정하지 않고 #3561 보고에 그침.
3. **80쪽+ 코호트 해부(6절)가 이번 회차의 실질 기여** — "누적 드리프트가 아니라 앞 20%에서
   갈리는 국소 오차의 상쇄 잔차"라는 구조 진단과 "국소 레버 상한 +1.8pp" 정량화는 #2279
   knife-edge 종료 구조와 정합하며, 이후 세로 공간 재설계 논의의 근거 자료가 된다.
4. 하니스 함정 6종 기록(7절)은 재현 실패의 재발 방지 가치가 있다.
5. 보류 skia 패치 2건은 "무해하나 효과 미확인" — 채택 판단은 작업지시자 몫으로 남는다.

## 최종 권고

**merge 권고 (fast-pass).** 허용 범위·CI aggregate·수치 정합·기준선 규율 모두 확인.
behind 상태는 docs-only 신규 파일이라 충돌 축이 없고, admin merge 또는 update branch
(재실행도 fast-pass라 저비용) 중 택일. merge 후 후속: 보류 skia 패치 2건 채택 여부와
#3561 처리 방침은 별도 결정 대상.
