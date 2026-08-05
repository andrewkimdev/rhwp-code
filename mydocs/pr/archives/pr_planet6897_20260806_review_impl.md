---
kind: review_plan
status: active
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-06
---

# planet6897 PR #4041·#4047·#4053·#4057·#4060·#4066·#4067·#4077 통합 기록

## 기준과 누적 방식

최종 기준은 `upstream/devel` `0b2e1c7e87132c51840bf7d8b79a04635f5b2cbb`다. 원 PR head는
검토 시작 시점과 동일했고, contributor branch에는 push·rewrite하지 않았다. 최신 devel 위에서
원 변경을 누적한 뒤, `typeset.rs`의 겹치는 두 수정에만 메인터너 보정 `c278b3871706fd33548ecb52c1c6010ae4e05f06`을
추가했다.

| 원 PR | source head | 통합 적용 commit | 처리 |
| --- | --- | --- | --- |
| #4041 | `44daea010019c0e7602a95cd90e844bebae432af` | `c506c949a` | 저장 꼬리줄 상한 적용 |
| #4047 | `b1ee94e9990de53a9a72613376dc9e1d55f45637` | `88c68492b`~`26d8fc5bd` | WASM 측정기·패리티 하네스 적용 |
| #4053 | `cd8c0b4ac74b14a1279a96bda971af01405eb89f` | `51c558f72` | r31 조사 문서 적용 |
| #4057 | `d3592c5582439bf4bd3d57d1952e368e50308c52` | `a4b1c79fb`~`6de709902` | WMF DOM SVG 경로 적용 |
| #4060 | `9c9e67eb57ef1d5191345fa3f782b4cc1430d008` | `03a0de0a0`~`da2ea6479` | 중복 WMF는 제외하고 EMF 고유분만 적용 |
| #4066 | `f0631023a2dee6545fc7bc757edb86046a108bd3` | `126127997`~`8776da74b` | 선행 stack은 제외하고 잔여 이미지 고유분만 적용 |
| #4067 | `7a6b8003875ace30013b57f9656dc651e9c806c1` | `a67ee894a` | Square 그림 anchor·실 fixture 적용 |
| #4077 | `fc70872cbbaf365871455a27c276499be8f388a3` | `e21ffecdd` | 각주 안전마진 조건부 예외 적용 |

## 충돌 및 보정

- `tests/fixtures/ir_field_sweep_baseline.tsv`는 #4067 새 fixture 행과 기존 fixture 행을 이름순으로
  모두 보존했다. 후속 위치 이동 commit은 같은 결과를 만들므로 적용하지 않았다.
- #4041과 #4077은 같은 줄 분할 루프를 바꾸므로 한쪽을 덮어쓰지 않았다. 상태 전이와 각주 영역
  판정을 순수 함수으로 추출하고 경계값 회귀 테스트 3개를 추가해, 꼬리 연쇄 상한·HWP 권위 경계·
  native HWP5 경계·각주 조건부 예외가 함께 유지됨을 고정했다.

## 완료 검증

- 최신 rebase head에서 `cargo test --profile release-test --tests`: 467개 test binary 성공.
- Stage 1의 focused typeset 4개, 이미지 resolver 14개, WMF/EMF 2개, 표 셀 anchor·WMF flow fixture,
  Native Skia 58/2/4, clippy, fmt, diff, wasm-pack build를 순차 완료했다.
- #4067 fixture는 HWP 2020 MCP PDF 기준 5쪽과 rhwp 첫 페이지를 수동 시각 대조했다. Square 그림은
  셀 밖으로 잘리지 않았고, 전체 폰트 fidelity는 이 통합의 수용 조건이 아니다.

## 후속 절차

1. review 문서·오늘할일·기준 PDF·대표 PNG를 code 보정과 함께 통합 PR에 포함한다.
2. 원본 저장소의 임시 head branch로 push하고 `devel` 대상 PR을 만든다.
3. 최신 integration head의 required GitHub Actions와 mergeability를 확인한 뒤 merge한다.
4. merge 뒤 원 PR 8개를 통합 PR로 superseded 처리하고, 관련 issue 상태·devel 동기화·review target과
   branch 정리를 merge 후속 절차에 따라 수행한다.
