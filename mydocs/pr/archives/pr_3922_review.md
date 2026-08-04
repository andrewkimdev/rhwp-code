---
kind: pr_review
status: local-validation-passed
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-04
---

# PR #3922 검토 — `rhwp-agent` 실험 운영 CLI

| 항목 | 값 |
| --- | --- |
| 원 PR / 작성자 | [#3922](https://github.com/edwardkim/rhwp/pull/3922) / @kevin9327 |
| 원 head | `a3c0351d6194bd4a99484484818ff6a798239cb8` |
| 누적 적용 commits | `db94bde05`, `31fac9e4c`, `90fc51355` (`cherry-pick -x`) |
| 현재 PR 기준 / 누적 branch | `upstream/devel` `874dae394` / `review/kevin9327-20260804` |

## 검토

기존 `rhwp` 명령 등록부와 충돌하지 않는 별도 native binary `rhwp-agent`로 discovery, doctor,
scan, fingerprint, diff-text, verify, PII gate, chunk planning, evidence의 9개 운영 명령을 제공한다.
`caps::COMMANDS` 단일 테이블과 계약 회귀가 자기서술·디스패치·unknown flag exit 2·JSON 봉투·gate
exit 3·PII 기본 마스킹을 왕복으로 고정한다.

누적 검토의 메인터너 보정 `554eebaa6`는 `chunk-plan`의 다음 실행 힌트를 셸 문자열이 아닌
`command.program`/`command.args` 구조화 argv로 교체해 경로의 공백·메타문자가 재해석되지 않게 했다.
동시에 빈 `--expect-field` 이름을 usage exit 2로 거부하고 계약 회귀를 확장했다.
`c6550e426`은 새 장기 매뉴얼의 필수 metadata를 추가해 이번 PR이 전역 metadata 오류를 더 만들지 않게
했다.

## 판정

실험 표면임을 `experimental: true`로 명시하며 본 CLI·중앙 provenance 지도 승격은 후속 범위로 남긴다.
focused 계약 13건, 전체 누적 회귀, clippy, WASM library check와 문서 검증을 확인했다.
**통합 수용 권고.**
