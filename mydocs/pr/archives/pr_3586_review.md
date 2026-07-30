---
kind: pr_review
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-07-30
---

# PR #3586 리뷰 — 셀프 호스팅 빌드용 외부 웹폰트(CDN) 차단 스위치

- PR: [#3586](https://github.com/edwardkim/rhwp/pull/3586) / Closes [#3585](https://github.com/edwardkim/rhwp/issues/3585) (본인 제안 이슈)
- 작성자: `JamesPsh` — 3번째 PR (#3548 hc: 네임스페이스, #3549 OLE size prefix 어제 merge)
- 역할: maintainer 일반 경로 + local_validation (4.3 rhwp-studio 행)

## 라우팅과 작성 시점

```text
base route: maintainer_general.md / modifiers: intake_and_review.md, local_validation.md
current head: 57f2f4238 / MERGEABLE / behind (참고값)
규모: 5 files, +116/−9 — vite.config·vite-env.d·extension-settings + 신규 테스트 3건 + editor README
```

## 변경 범위와 수용 판단

`RHWP_DISABLE_EXTERNAL_WEBFONTS=1` 빌드 스위치로 셀프 호스팅 배포가 cdn.jsdelivr.net
함초롬 woff 요청(비상업 사용 조건)을 빌드 시점에 차단할 수 있게 한다. 미설정 시 기본
동작 불변.

1. `vite.config.ts` define — `__APP_VERSION__`/`RHWP_SUBSECOND` 기존 관용과 동일 패턴.
2. `extension-settings.ts` — 상수 기본값을 `defaultSettings()` 함수로 전환해 define 상수를
   호출 시점 반영. `typeof` 가드로 번들 밖(node:test) 실행 안전. **확장 storage 명시값
   우선 순위 유지** — 기본값 시드/normalize 경로 검토로 확인.
3. 라이선스 관점에서 올바른 방향 — 함초롬 CDN 비상업 조건을 만족 못 하는 셀프 호스터에게
   공식 차단 경로 제공. 조판(내장 메트릭)은 불변, 표시 글꼴만 폴백.

**수용 판단: merge 권고.**

## 검증 기록

| 검증 | 결과 | 판정 |
| --- | --- | --- |
| 충돌 simulation (devel merge) | clean | — |
| `npm ci` (stale 교정 선행) | 정상 | npm 위생 규칙 준수 |
| `npm test` (통합 트리) | **678/678 통과** (신규 3건 포함, 단독 재확인 3/3) | 스위치 미설정 기본값·켜짐 차단·storage 우선 3계약 |
| `RHWP_DISABLE_EXTERNAL_WEBFONTS=1 npm run build` | 성공 (tsc + vite build, PWA precache 생성) | 스위치 ON 경로 end-to-end 컴파일·번들 확인 |
| PR head CI | 전 check green (Frontend package gates 포함) | — |

브라우저 실기동에서 CDN 요청이 실제로 사라지는지의 네트워크 수준 확인은 수행하지 않음 —
동작 경로가 기존 `disableExternalWebFonts` 옵션 그대로라(설정 주입점만 추가) 위험이
낮다고 판단. 필요 시 작업지시자 실기동 확인 대상.

## 최종 권고

**merge 권고.** merge 후 #3585 auto-close 확인, contributor comment(재기여 톤).
