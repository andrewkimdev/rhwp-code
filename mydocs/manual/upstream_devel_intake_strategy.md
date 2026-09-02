---
kind: guide
status: active
canonical: mydocs/manual/upstream_devel_intake_strategy.md
last_verified: 2026-09-01
---

# upstream(edwardkim/rhwp) devel 변경을 지속적으로 들여오는 전략

이 문서는 **세 번째 방향**이다 — [포크 수확 규약](fork_harvest_convention.md)은 "우리
개선을 upstream이 거둬 가는 방향", [로컬 패치 스택 upstream 재적용 절차](patch_stack_upstream_sync.md)는
"우리 자신의 patch 브랜치를 우리 `main` 위에 얹는 방향"이다. 이 문서는 "upstream `devel`의
변경을 지속적으로 골라 우리 `main`에 들여오는 방향"을 다룬다. 세 문서는 서로 다른 문제를
다루므로 통합하지 않고 상호 참조만 건다.

## 왜 `git merge`가 아니라 지속적 선별 반영인가

2026-09-01 실측: `rhwp-code`와 upstream은 2026-08-12(`6f70cd1b6`)까지 커밋 오브젝트를
물리적으로 공유했지만, 그 이후 **양쪽이 독립적으로 같은 대형 파일들을 서로 다르게
쪼개는 리팩터**(`main.rs`, `table_layout.rs`, `wasm_api.rs`, `hwpx/section.rs` 등)를
진행해 구조가 크게 갈라졌다. 같은 기간 upstream `devel`은 20일간 1,971개 커밋(소스코드만
769개 파일, +238K/-90K줄)을 쌓았다 — 이 규모에서 `git merge`나 전체 `git cherry-pick`은
비현실적이다. 대신 이번 Tier 1 도입에서 실제로 검증한 절차를 아래에 일반화한다.

## 라운드 절차

1. **직전 라운드 확인**: `mydocs/report/upstream_devel_sync_candidates_*.md` 중 최신
   파일을 읽어 마지막으로 조사한 기준점과, 그때 보류된 항목("다음 라운드 재검토
   대상")을 파악한다.
2. **fetch**: `../rhwp`(또는 upstream remote)를 최신화한다.
3. **규모·영역 파악**: 직전 기준점부터 최신까지
   `git log <기준점>..<최신> --oneline --stat --dirstat`로 총 커밋 수, 기간, 디렉터리별
   변경 비중을 뽑는다.
4. **우선순위 분류**: 다음 순서로 Tier 1(High)/Tier 2(Mid)/Tier 3(Low)를 나눈다.
   - Tier 1: 우리 도메인과 직접 겹치는 것 — HWP3 파서 격리(`src/parser/hwp3/`),
     HWPX↔HWP5 fidelity, 표/미주 등 렌더러 레이아웃 정밀도. 특히 실제 상호운용성
     결함(한글이 파일을 못 열거나 크래시)은 항상 최우선.
   - Tier 2: 기능 격차가 크지만 이식 비용·구조 충돌 위험이 있는 것(신규 CLI 명령군,
     신규 파일 포맷 하위 기능 등).
   - Tier 3: 에이전트 툴링(`rhwp-q-*`, `gym/`), 문서/PR 아카이브 문화 등 코어 엔진
     fidelity와 무관한 것.
5. **결과 기록**: `mydocs/report/upstream_devel_sync_candidates_YYYYMMDD.md`로 라운드마다
   새 파일을 남긴다(과거 라운드는 지우지 않고 누적 — historical 스냅샷). `mydocs/report/`는
   front matter가 필수가 아니므로(`mydocs/README.md`의 필수 목록에 없음) 기존
   `fork_harvest_r0_20260808.md`와 같은 방식으로 제목 헤더만 붙인다.

## 후보 커밋 선확인 필수 규칙

후보로 보이는 커밋마다 **반드시** 다음을 먼저 확인한다.

```bash
git -C ../rhwp merge-base --is-ancestor <hash> <rhwp-code 쪽 대응 브랜치>
```

전역 fork-point(예: `6f70cd1b6`) 하나만 보고 "그 이후 커밋은 전부 미반영"이라고
가정하지 않는다. `rhwp-code`가 과거에 upstream을 부분적으로 merge/cherry-pick 했을
수 있고, 실제로 이번 조사에서 `#4514`(`718ce06d0`)를 후보로 분류했다가 실제로는
이미 rhwp-code에 byte-identical로 존재함을 뒤늦게 발견했다 — 전역 시점이 아니라
**커밋 단위**로 재확인해야 한다.

## 의존 관계 선확인 규칙

대상 커밋의 diff가 참조하는 함수·IR 필드가 rhwp-code에 이미 있는지 먼저 `grep`으로
확인한다. 없다면 그 커밋이 어떤 선행 커밋(같은 이슈 번호의 이전 단계이거나 완전히
별도 이슈)에 의존하는지 upstream 이력에서 역추적한다.

- **선행 기능이 있고 이식 가능한 경우**: 먼저 이식한 뒤 대상 커밋을 이어 적용한다.
  예: `#6303`(셀 자간 자동 축소 수렴)은 `#6196`/`#6389`(저장 사다리 증언 게이트)에
  의존했고, 두 선행 기능이 참조하는 IR 필드(`LineSeg.tag`/`segment_width`)가
  rhwp-code에 이미 있어 그대로 이식할 수 있었다. `#5861`(사용자 정의 기호 0xA807)도
  `#5140`(BMP↔평면15 매핑 전체)에 의존했지만 마찬가지로 쉽게 풀렸다.
- **선행 조건이 코드 몇 줄이 아니라 아키텍처 전제인 경우**: 실제로 이식해 보고 통합
  테스트로 검증하기 전까지는 "쉬워 보임"을 믿지 않는다. `#5251`(HWP3 원본 char_shapes
  경계 보존)은 diff만 보면 3곳의 국소 수정처럼 보였지만, 실제로 이식·테스트해 보니
  **rhwp-code의 네이티브 HWP3 파서가 char_shapes 위치를 upstream과 다른 단위 체계로
  계산**한다는, diff에 드러나지 않는 더 깊은 전제 불일치가 있었다(HWP3 단순 문자
  인덱스 vs HWPX/HWP5 PARA_TEXT 확장 단위). 이 경우 패치를 강행하면 왕복 정합이
  오히려 새로 깨진다 — 통합 테스트가 실패하면 억지로 임계값을 맞추지 말고, 근본
  전제를 재검토하고 필요하면 그 라운드에서는 보류한다.

## 반영 방법 원칙

- 구조가 갈라진 영역(대부분의 대형 리팩터 대상 파일)은 `git cherry-pick`이 아니라
  `git show <hash>`로 diff를 읽고 rhwp-code의 현재 구조 위치에 수동으로 재적용한다.
- 구조가 갈라지지 않은 영역(예: `#4318`처럼 fork 이후 독자 변경이 거의 없었던
  서브시스템)은 `git apply --check`로 클린 적용 가능 여부를 먼저 스크리닝해 난이도를
  빠르게 가늠한다.
- 게이트 도입 이력이 있는 upstream 커밋(예: 초판을 냈다가 "일반 케이스까지 영향
  범위가 넓어져 회귀"를 이유로 바로 다음 커밋에서 게이트를 추가해 되돌린 경우)은
  **반드시 게이트 포함 최종본만** 이식한다. 초판 형태로 적용하면 upstream이 이미
  겪은 회귀를 그대로 재현한다.

## 착수 순서 원칙

같은 라운드 안에서 여러 항목을 이식할 때는 다음 순서를 따른다(2026-09-01 Tier 1
도입에서 검증됨).

1. **의존성 없고 국소적인 항목** — 파서의 순수 함수/데이터 추가, 단일 파일 수정.
2. **선행 기능이 필요하지만 그 배선이 이미 갖춰진 항목** — 새 표·함수를 신설하지만
   호출부·IR 필드가 이미 있어 복사 수준인 것.
3. **새 인프라(스레드-로컬 게이트, 반복 수렴 알고리즘 등) 설계가 필요한 항목** —
   영향 범위가 가장 넓고 회귀 위험이 크므로 마지막에 배치하고 리뷰 비중을 높인다.

## 검증

`mydocs/manual/pr_review/local_validation.md` 4.3의 변경 범위별 게이트를 그대로
적용한다. parser 전용 변경은 focused test + release-test 전체 + fmt + clippy,
renderer/layout/typeset이 하나라도 걸리면 Native Skia 3종·wasm-pack build·시각 증적까지
포함한다. 신규 스레드-로컬 게이트를 도입하는 항목은 관련 provenance(HWP3-origin/
HWP5-origin) 양쪽 샘플을 섞어 회귀가 없는지 별도로 한 번 더 확인한다.

## 추적

보류·차단된 항목(선행 기능 자체가 크거나, 이식 후 테스트에서 더 깊은 전제 불일치가
드러난 경우)은 다음 라운드 보고서에 "다음 라운드 재검토 대상"으로 명시해 누락되지
않게 한다. 예: `#5251`(HWP3 char_shapes 경계)은 네이티브 HWP3 파서의 char_shapes 단위
체계 자체를 재검토해야 하는 더 큰 작업으로 재분류되어 2026-09-01 라운드에서 보류됐다.
2026-09-02 재착수에서 upstream 패치를 실제로 적용·테스트해 가설을 확정했다 —
`render-diff`(자기 라운드트립 시각 게이트)가 패치 전 PASS 였다가 패치 후
STRUCT_MISMATCH(347px)로 새로 깨졌다. 즉 upstream 패치를 그대로 들여오면 **지금
작동하는 렌더링에 실제 회귀를 낸다** — "테스트가 실패하면 근본 전제를 재검토하고
필요하면 보류한다"는 이 문서의 원칙이 그대로 들어맞은 사례다. 패치는 되돌렸다.

## 참고

- 반대 방향(우리 개선을 upstream이 거둬 가게 하는 것)은 [포크 수확 규약](fork_harvest_convention.md).
- 우리 자신의 patch 브랜치를 우리 `main` 위에 얹는 절차는
  [로컬 패치 스택 upstream 재적용 절차](patch_stack_upstream_sync.md).
- HWP3 격리·공통 IR 경계 원칙은 [포맷 파서와 공통 Document IR 경계](../tech/parser_architecture.md).
- 변경 범위별 검증 게이트는 [`local_validation.md`의 4.3](pr_review/local_validation.md#43-변경-범위별-기본-검증).
- 첫 실측 라운드 결과는 `mydocs/report/upstream_devel_sync_candidates_20260901.md` 참조.
