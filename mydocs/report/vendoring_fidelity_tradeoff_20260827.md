# hwpx-template-engine 벤더링 트레이드오프 — 완결성 우선 정책과 테스트 분류 (2026-08-27)

**이 문서는 이 패치 브랜치(`patch/v0.8.4-cjk-rewrap-threshold`)에만 존재한다.
`table-debugging`/`main`에는 절대 병합하지 않는다** — 아래 정책 자체가
`rhwp-code`의 기본 설계 목표(한/글 오라클 충실도 우선)를 이 패치 하나에 한해
의도적으로 뒤집기 때문이다.

## 배경

이 브랜치는 `tcrlic.hwpx`의 `신청인_상호` 필드가 오른쪽 끝에서 잘리는 결함을
고친다(`composer.rs`의 `recompose_stored_single_line_if_overflowing` ×1.8 임계를
CJK 포함 줄에 한해 ×1.15로 낮춤 — 커밋 `f9d08eb7c`, 전체 기술 상세는
`hwpx-template-engine/vendor/rhwp/PATCHES.md`의 "CJK-only cell rewrap threshold"
항목 참고). 이 수정은 `tcrlic`/`scslic`의 clip은 고치지만, `rhwp-code` 자체의
한/글 오라클 고정 테스트(`tests/issue_1921_59043_pagination_pin.rs`)와
`hwpx-template-engine`의 `pipelineSmokeTest`(`appccr1dlm`의
`footerFitsWhenRoomExistsAndNeverOverflowsWhenItDoesnt`)를 모두 실패시킨다 —
이전에 잘리거나 겹치던 콘텐츠가 이제 줄바꿈되어 행이 커지고 쪽배치가
한/글의 실제 출력과 달라지기 때문이다.

## 채택된 정책 (프로젝트 소유자 결정, 2026-08-27)

**`hwpx-template-engine`는 텍스트가 잘려 사라지는 것을 절대 허용하지 않는다 —
한/글의 실제 렌더링과 시각적으로 달라지는 대가를 치르더라도.** 근거: `rhwp`는
한컴의 공식 SDK가 아니라 제3자 비공식 구현이므로, 한/글과 픽셀 단위로 동일한
결과를 내는 것은애초에 이 프로젝트가 지킬 수 있는 약속이 아니었다. 실제
목표는 "채워진 서식의 그럭저럭 좋은 PDF/SVG"를 만드는 것 — 한/글과 똑같은
배치보다 데이터가 하나도 누락되지 않는 것이 더 중요하다.

전체 정책 원문은 `hwpx-template-engine/docs/design/RENDERING_FIDELITY_POLICY.md`.
이 문서는 그 정책을 **`rhwp-code` 쪽에서** 어떻게 읽어야 하는지 — 특히 이
저장소의 테스트 스위트를 어떻게 분류해 판단할지 — 를 다룬다.

## `rhwp-code`가 이 패치를 병합하지 않는 이유

`rhwp-code` 자체는 두 가지 역할을 겸한다: (1) `hwpx-template-engine`에 벤더링되는
렌더링 엔진, (2) 픽셀 비교 검증 오라클. 이 패치는 (1)의 한 소비자만을 위한
트레이드오프이지 `rhwp-code` 전체의 새 기본값이 아니다 — 오라클 역할(2)은
여전히 한/글과의 충실도를 요구하며, 이 패치는 그 요구와 정면으로 배치된다.
그래서 격리된 패치 브랜치에만 존재하고 `main`으로는 절대 병합하지 않는다.

## 테스트 분류 프레임워크 (권고안 — 이번 라운드에는 코드/CI에 적용하지 않음)

단순 "필수 vs 선택"의 이분법보다, 실제로 무엇을 검증하는지에 따라 3단계로
나누는 것이 더 정확하다:

### Tier 1 — 무조건 필수 (정합성/안전성 불변식, 충실도 의견이 아님)

크래시, 깨진 산출물, 잘못된 필드 값, 손상된 텍스트 등 — 어느 철학을 택하든
실패하면 실제로 뭔가 고장난 것이다.

**특히 명시적으로 짚어둘 것**: `tests/overflow_cell_baseline.rs`. 이 래칫의
`LAYOUT_OVERFLOW_CELL` 신호(셀 안 줄의 윗변이 쪽 하단 밖에 그려져 "그 줄은
어느 부분도 보이지 않음")는 한/글 충실도 검사가 아니다 — 이 프로젝트가 방금
채택한 "아무것도 조용히 사라지면 안 된다"는 원칙을 렌더러 쪽에서 그대로
검증하는 것이다. 새 정책 아래서도 이 게이트는 그대로 유지해야 한다 — 여기서
회귀가 나면 그건 정책이 막으려는 바로 그 실패 양상(실제 데이터 소실)이다.

### Tier 2 — 이 소비 경로에 한해 권고/선택 (순수 한/글 픽셀 충실도, 데이터 소실과 무관)

한/글 오라클 대비 정확한 쪽수 고정(`tests/issue_2430_cell_rewrap_threshold.rs`,
`tests/issue_1921_59043_pagination_pin.rs`의 쪽수 검증), 표 정렬/폭/간격
정확성 검사(`everyTableSharesTheSameLeftEdgeInAnyTemplate` 등),
`pipelineSmokeTest`의 "피할 수 있었던/낭비된 쪽나눔" 검사
(`repeatBlockOverflowKeepsTopBorderOnFirstTableOfNewPage`,
`repeatBlockTwoToThreePageTransitionNeverWastesRoomInAnyTemplate`) — 모두
"한/글 배치와 일치하는가"를 재는 것으로, 새 정책이 명시적으로 협상 가능하다고
선언한 축이다.

### Tier 3 — 시각 검토 필요, 자동 분류하지 않음

개념상으로는 Tier 1(잠재적 데이터 소실)처럼 들리지만, 실제 판정은 픽셀 단위
가시성이 아니라 bounding-box 계산(기하 근사치)이라 보수적이거나 부정확할 수
있는 것들: `pipelineSmokeTest`의
`footerFitsWhenRoomExistsAndNeverOverflowsWhenItDoesnt`,
`sealBottomGapIsCorrectedToDesiredGapInAnyTemplate`,
`noFloatingPictureOverlapsPrecedingTextInAnyTemplate`. "콘텐츠가 쪽 경계를
넘었다"거나 "bbox가 겹쳤다"는 계산은 실제로 글자가 안 보이는 경우부터, 겹쳤지만
완전히 읽을 수 있는 경우까지 다 포함할 수 있다 — 렌더링된 이미지를 직접 봐야만
구별된다. 이번 세션에서 `appccr1dlm`을 실제 전후 PDF로 직접 확인해 판단한 것과
정확히 같은 과정이다. **권고**: 이 티어를 규칙으로 자동 해소하려 하지 말 것 —
Tier 3 실패는 앞으로도 "양쪽을 렌더링해서 직접 보고 판단"으로 다루는 게 맞다.

### 이번 세션의 실측 사례

- `overflow_cell_baseline.rs`, `issue_2430_cell_rewrap_threshold.rs` — 이 패치와
  무관하게 그대로 통과(해소할 충돌 없음).
- `issue_1921_59043_pagination_pin.rs`의 쪽수 고정(Tier 2) — 회귀했고, 정책에
  따라 수용.
- `pipelineSmokeTest`의 `footerFitsWhenRoomExistsAndNeverOverflowsWhenItDoesnt`
  (Tier 3) — 회귀했고, 실제 전후 PDF를 시각 검토한 뒤 개선(겹침 대신 줄바꿈)으로
  수용. 다만 이는 `appccr1dlm`에 대한 `RandomSampleDataGenerator`의 무작위 추첨
  1건에 대한 판단이라는 점은 유의 — 예정된 전 템플릿 수동 검토 때 다른 추첨
  결과도 한 번 더 확인할 가치가 있다.

## 이번 라운드에는 하지 않은 것

`rhwp-code`의 실제 테스트 파일은 건드리지 않았다 — `#[ignore]` 추가, 태그, CI
설정 변경 전혀 없음. 이번 라운드는 문서화 + 권고 프레임워크뿐이다. 코드로
적용(예: feature-gated 테스트 분리)하는 것은 사용자의 전 템플릿 수동 시각
검토가 끝난 뒤, 그 결과로 Tier 2/3 분류 자체가 달라질 수 있으므로 별도
후속으로 미룬다.
