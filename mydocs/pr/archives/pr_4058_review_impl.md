# PR #4058 구현·통합 기록

## 대상

- PR: [#4058](https://github.com/edwardkim/rhwp/pull/4058)
- 관련 이슈: [#3930](https://github.com/edwardkim/rhwp/issues/3930)
- 후속 fidelity: [#3820](https://github.com/edwardkim/rhwp/issues/3820)
- base: `upstream/devel` `aebfcaa33`
- 구현 head: `043a2e339d537cb068287160e89434be87e14a69`

## 커밋 순서와 책임

| 순서 | 커밋 | 책임 |
| ---: | --- | --- |
| 1 | `1b799b893` | HWPX 표 흐름과 희소 바탕쪽 상속 보정, 실제 편람 회귀 |
| 2 | `47216ca6b` | 단일 바탕쪽 HWP5 저장 계약 보존 |
| 3 | `928cb282e` | CHAR_SHAPE sentinel fail-closed probe |
| 4 | `c93e861f6` | CharShape 직렬화 모듈 분리와 byte-identical 복원 |
| 5 | `043a2e339` | source provenance audit CLI와 사용법 |
| 6 | 현재 PR 보정 커밋 | HWPX->HWP `imgDim=(0,0)` sentinel 비교 범위화, form-002 SVG 기준 갱신, p30/p144/p145 직접 회귀 |

## 처리 단계

1. `#3930`의 직접 증상 두 건은 저장·재열기 회귀와 focused release-test로 고정한다.
2. Hancom PDF 전수 비교에 남은 차이는 oracle 의존 probe를 production으로 승격하지 않고
   source-derived 판별 가능성을 audit으로 검증한다.
3. 실제 source HWPX signature가 raw 상태를 구별하지 못함을 확인했으므로 serializer 변경 없이
   diagnostic과 근거만 유지한다.
4. #3930에는 해결 범위와 fixture/PDF 위치를 기록하고, 동일 기준 세트의 후속 fidelity 분석은
   #3820으로 이관한다.
5. 최신 PR head의 CI·mergeability와 작업지시자 승인을 확인한 뒤 병합한다. #3930만 닫고 #3820은
   계속 열린 상태로 둔다.
6. 최초 최신 head에서 Clippy가 테스트 코드의 불필요한 clone과 단일 원소 루프를 거부하면,
   저장 로직을 건드리지 않고 해당 테스트 표현만 보정한다. workspace Clippy와 focused
   release-test를 다시 통과시킨 head의 CI를 병합 기준으로 사용한다.
7. HWPX 그림의 `imgDim=(0,0)`은 Hancom HWP5 저장 계약에서만 발생하는 원본 이미지 메타
   sentinel이므로 HWPX->HWP 자기검증에서만 비교 불능으로 취급한다. `curSz` 또는 `imgRect`가
   함께 달라지면 검증 실패를 유지한다. p30/p144/p145는 페이지 수 대신 render tree 직접 비교로
   수용 기준을 고정한다.

## Rollback 범위

- p144/145와 p30 보정에 회귀가 발견되면 `1b799b893`와 `47216ca6b`의 저장·조판 변경을 함께
  되돌린다. fixture 회귀는 유지해 재발을 드러낸다.
- diagnostic이 문제가 되면 `928cb282e`부터 `043a2e339`까지를 별도로 되돌릴 수 있으며,
  production serializer의 Stage 2 byte-identical 계약에는 영향을 주지 않는다.
- #3820의 후속 보정은 이 PR을 수정하지 않고 새 branch/PR로 진행한다.
