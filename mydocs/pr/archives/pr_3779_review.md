---
kind: review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-02
---

# PR #3779 검토 기록

## 판정

[PR #3779](https://github.com/edwardkim/rhwp/pull/3779)는 Node M19 바인딩과
`export-capabilities-schema`를 제공한다. 기능·CI 관련 commit을 `9ecca3bae`부터
`106ff3780`까지 적층하고 source의 `Merge branch 'devel'` commit은 제외했다.

## 누적 보정

- M18 `export-ir-schema`와 M19 command가 `src/main.rs`에서 함께 dispatch되도록 충돌을 해소했다.
- 생성 타입이 새 command 봉투를 빠뜨려 `envelopes.ts`를 재생성했고, package-lock을 추가해 CI의
  네 Node job을 `npm ci`와 npm cache로 고정했다.
- generator는 1,000줄 이하 정책을 지키도록 불필요한 공백 두 줄을 제거했다.

## 검증

- `npm ci`, typecheck, unit 389건, native integration 425건, generator drift check, build와
  `npm pack` 파일 목록을 실행해 모두 통과했다.
- public package/CI 변경이지만 renderer·fixture 영향은 없어 시각 증적은 적용하지 않는다.

전체 gate와 완료 판정은 [통합 반영 기록](pr_3747_3779_kevin_review_impl.md)에 기록한다.
