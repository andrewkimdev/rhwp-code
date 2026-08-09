---
kind: pr-review-implementation
status: in-progress
canonical: mydocs/manual/pr_review_workflow.md
last_verified: 2026-08-09
---

# PR #4274 Windows 시나리오 이식성 메인터너 보정 계획

## 근거

최신 contributor head `27161a67b`의 `pF-insertpicture`, `pG-pageimage`,
`pH-insertrepeat`은 `paths.win`에 `C:\\Users\\planet\\...`를 고정했다. `runner_ocx.py`는
Windows에서 이 값을 그대로 COM Oracle에 전달하므로, 다른 Windows 작업자에서는 그림 입력과
쪽 그림 출력이 실패한다. 실제 `win10-ted`에는 contributor 경로가 없고, review worktree의
`samples/s1.jpg`만 존재함을 확인했다.

## 보정 순서

1. 세 시나리오의 Windows 갈래를 `{repo}` 또는 `{out}` 토큰으로 전환한다. Windows에서만
   역슬래시 경로가 되도록 작성하되, POSIX 갈래와 호출 계약·기대 반환값은 바꾸지 않는다.
2. `scenario_spec.py`의 예시와 `test_harness_contract.py`를 함께 고친다. Windows 경로를
   `PureWindowsPath`, POSIX 경로를 `PurePosixPath`로 각각 검증하고, tracked scenario에
   사용자 홈 절대경로가 다시 들어오지 못하게 한다.
3. Linux 계약·원장 검사를 다시 실행한다. Windows 별도 worktree에서 fresh WASM을 만든 뒤
   `pF`, `pG`, `pH` Oracle 대조와 80개 전체 gate를 순차 실행한다.

## 1차 결과 (Linux, 2026-08-09)

- `test_harness_contract.py`: 20건 통과. Windows·POSIX 토큰 해석과 contributor 홈 경로 금지
  계약을 포함한다.
- `npm --prefix npm/hwpctrl-ocx run ledger:check`: `308/484 완료`로 기존 원장 수를 유지했다.
- `npm --prefix npm/hwpctrl-ocx run test:contract`, 문서 메타데이터·상대 링크, JSON 문법,
  `git diff --check`: 모두 통과했다.
- Windows `win10-ted` review worktree에서 fresh WASM 빌드를 시작했다. 완료 후 실제 Hancom
  Oracle의 `pF`, `pG`, `pH`와 전체 gate 결과를 이 문서에 추가한다.

## 2차 결과 및 보정 (Windows Hancom 2022, 2026-08-09)

- fresh WASM(`pkg/rhwp_bg.wasm`, 9,195,123 bytes)과 active RDP 세션의 Hancom 2022 Oracle로
  portable path 시나리오를 재실행했다. `pF-insertpicture` 14/14, `pG-pageimage` 10/10,
  `pH-insertrepeat` 20/20 호출이 모두 `MATCH`였다.
- 기본 `run_gate.py`는 `Quit()` 뒤 10초 동안 남은 새 `Hwp.exe`를 안전상 자동 종료하지 않아,
  live Windows에서 첫 시나리오가 `OK/LEFTOVER`가 되고 뒤 시나리오가 `OCCUPIED`가 됐다.
  이는 시나리오 계약 불일치가 아니라 npm 공개 gate의 실행 경로 결함이다.
- `run_package_gate.py`를 추가해 Windows package gate에만 기존의 명시적
  `--cleanup-spawned`를 전달한다. baseline에 있던 사용자 Hancom 프로세스는 기존처럼
  `OCCUPIED`로 거부하며 종료하지 않는다. Linux/macOS는 이전과 동일한 WASM self-check다.

## 경계와 rollback

- HwpCtrl API 구현·원장 완료 수·Oracle 반환 계약은 수정하지 않는다.
- 보정이 Windows Oracle과 다르면 scenario path만 contributor 경로로 되돌리지 않고, 반환값에
  경로 의존성이 있는지 probe로 확인해 portable fixture 규약을 다시 정한다.
- 보정 commit과 검증 결과는 같은 review branch에 고정한다. remote push·GitHub comment는
  작업지시자의 다음 승인 전에는 수행하지 않는다.
