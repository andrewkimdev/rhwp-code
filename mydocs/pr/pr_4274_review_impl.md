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

## 3차 결과 및 보정 (Windows package gate, 2026-08-09)

- 실제 `npm --prefix npm/hwpctrl-ocx run gate`에서 `p4-setmutate` 뒤 Hwp 종료가 비동기여서,
  `taskkill` 직후의 즉시 PID 재조회가 `OK/LEFTOVER`를 만들고 다음 일곱 시나리오를
  `OCCUPIED`로 건너뛰는 것을 재현했다.
- 정리 경로도 `wait_for_hwp_exit`로 재확인하도록 고친다. 정리 대상은 실행 전 baseline에 없던
  PID로 한정하며, 새 단위 계약은 `taskkill` 뒤 대기 호출을 고정한다.

## 4차 결과 및 보정 (CI shard 2, 2026-08-09)

- 최신 contributor head의 CI shard 2는 `issue_2027_picture_wrap_toggle_loss::
  tac_roundtrip_preserves_anchor_line_segs`에서 실패했다. 새 본문 그림 경로는 `char_offsets`만
  8-unit 이동시키고 기존 `line_segs.text_start`는 그대로 뒀다. floating 상태의 저장 줄 좌표는
  83인데 TAC off 재리플로우는 이동된 문자 좌표로 91을 산출해 왕복 정합이 깨졌다.
- 컨트롤은 문단 스트림에서 8칸을 차지하므로 `char_count` 증분은 유지한다. 대신 공용
  `shift_for_inline_control_insert`가 글자모양·범위 태그와 같이 `line_segs.text_start`도
  이동하도록 보정한다. 새 단위 테스트가 이 공용 계약을 고정하며, 그림 앵커 회귀는 기존 갭과
  control position 검증으로 계속 보호한다.
- 보정 후 `issue_2027_picture_wrap_toggle_loss::tac_roundtrip_preserves_anchor_line_segs`,
  `issue_4347_insert_leaves_coordinate_trace`의 본문·글상자 2건, 그리고
  `test_inline_control_insert_line_segs_shift`가 모두 통과했다.

## 5차 계획 (Windows Hancom 종료 대화상자, 2026-08-09)

- 실제 full gate에서 수정된 `2026_oss_rst.hwp`를 직접 `Quit()`해 “저장할까요?” 대화상자가
  활성 RDP 세션을 막는 것을 확인했다. 이 실행은 중단했고 결과를 사용하지 않는다.
- 설치된 `pyhwpx` API 문서에 따르면 `clear(option=1)`은 변경을 버리고, `option=0`만 저장
  질문을 띄운다. Oracle runner가 모든 종료 갈래에서 명시적으로 discard한 뒤 `Quit()`하도록
  helper를 추가한다.
- mock 계약은 discard와 Quit의 순서 및 종료 예외 격리를 고정한다. Windows에서는 먼저 수정
  시나리오 한 건을 prompt 없이 끝내는지 확인하고, 그 뒤 전체 package gate를 새 로그로 재실행한다.

## 5차 결과 (Windows Hancom 종료 대화상자, 2026-08-10)

- `runner_ocx.py`가 종료할 때 `pyhwpx.Hwp.clear(option=1)`로 활성 문서 변경을 버린 뒤
  `HwpObject.Quit()`하도록 공용 helper를 적용했다. discard가 예외를 내더라도 Quit은 계속
  시도한다.
- Linux mock 계약 24건, npm 공개 패키지 계약 1건, 원장 검사 308/484가 모두 통과했다.
- Windows 한컴 2022 `12, 0, 0, 535`에서 원본을 실제 수정하는 `p3-action-autonum` 138개 COM
  호출이 exit 0으로 끝났다. 실행 후 `Hwp.exe`는 0개였고 저장 확인 대화상자도 나타나지 않았으며,
  원본 표본의 변경 감지 가드도 통과했다.
- 같은 보정으로 당시 81개 live Oracle/rhwp 시나리오가 끝까지 실행됐다. 마지막 L3 비교만
  `target/release/rhwp.exe` 부재로 `WinError 2`가 나서 전체 exit 1이었으며, 이는 한컴 종료
  모달과 무관한 검증 선행조건 누락이다. 이후 Windows release CLI를 생성했고, 최신 contributor
  head의 확장 시나리오까지 반영한 full gate에서 최종 판정을 다시 고정한다.

## 경계와 rollback

- HwpCtrl API 구현·원장 완료 수·Oracle 반환 계약은 수정하지 않는다.
- 보정이 Windows Oracle과 다르면 scenario path만 contributor 경로로 되돌리지 않고, 반환값에
  경로 의존성이 있는지 probe로 확인해 portable fixture 규약을 다시 정한다.
- 보정 commit과 검증 결과는 같은 review branch에 고정한다. remote push·GitHub comment는
  작업지시자의 다음 승인 전에는 수행하지 않는다.
