---
kind: review
status: maintainer-correction-pending-ci
canonical: mydocs/pr/archives/pr_3685_review.md
last_verified: 2026-08-01
---

# PR #3685 검토·메인터너 보정 기록 — HWP3 변환본 한컴 열기 저장 계약

## 라우팅과 범위

```text
base route: collaborator external PR
modifiers: intake_and_review.md, local_validation.md, rework_and_exceptions.md
current source head: 2f81e673308b5f253528541c3963e452e1cf2e41 (작성 시점 참고)
```

이 기록은 [PR #3685](https://github.com/edwardkim/rhwp/pull/3685)의 HWP3→HWP5 저장 계약 보정과
Windows 한글 열기 검사 도구를 검토한 결과다. 작성자 `@planet6897`은 재기여자이며, 관련 이슈는
[#3676](https://github.com/edwardkim/rhwp/issues/3676)이다.

시각 sweep/PDF 증적은 적용하지 않는다. 변경은 renderer·typesetter·페이지 layout 또는 기준 fixture의
시각 결과가 아니라 HWP 바이너리 저장 계약과 Windows 외부 오라클 검사 도구에 관한 것이다. 이 경우의
정답지는 PDF 겹침 비교가 아니라 실제 한글 열기이며, 그 Windows 실물 검증은 작업지시자가 완료했다고
확인했다. 같은 검증을 중복 실행하지 않았다.

## PR metadata

| 항목 | 값 |
| --- | --- |
| PR / 작성자 | [#3685](https://github.com/edwardkim/rhwp/pull/3685) / `@planet6897` |
| base / head | `devel` `f80b910aabeda5939972752719b0916129eb3a53` / `fix/3676-hwp3-convert-hancom-openable` `2f81e673308b5f253528541c3963e452e1cf2e41` |
| 원 PR 변경 규모 | 3 files, +475 / -0 |
| 메인터너 code tail | `fcf101e68` (Windows 격리), `af00f9245` (HWP3 PBF 범위), `4c7082710`·`b23de3a5` (중첩 개체·Chart/OLE caption walker), `ccae1d016` (Python 안전 회귀 CI gate) |
| 변경 파일 | 원 PR 3개와 `scripts/tests/test_hwp3_convert_openable.py`, review·오늘할일 |
| reviewer | `@edwardkim` 요청 완료 |
| merge 상태 | `CLEAN` (작성 시점 참고) |

## 확인한 근거

| 항목 | 결과 |
| --- | --- |
| 원 source head CI | `2f81e673`의 CI preflight, lint, test archive, Native Skia, default-feature 8 shards, `Build & Test`, CodeQL 모두 success; WASM·frontend gate의 skipped는 preflight 판정에 따른 정상 생략 |
| Windows 기능 오라클 | 작업지시자가 `win10-ted`에서 한글 실물 열기 검증 완료를 확인. 이 검토에서는 중복 실행하지 않음 |
| Windows tool regression | `python3 -m py_compile`, `python3 -m unittest scripts/tests/test_hwp3_convert_openable.py` 2 passed, `--help`, `git diff --check` 성공. CI lint job에도 같은 Python test를 추가했고 Rust release-test는 실행하지 않음 |
| HWP3 PBF regression | `cargo test --profile release-test --test issue_3676_hwp3_convert_hancom_openable -- --nocapture` 4 passed, 0 failed. 단일 BOTH HWPX의 HWP→HWPX 보존과 기존 HWP3 3-record 계약을 함께 확인 |
| 중첩 walker regression | `cargo test --profile release-test --lib document_core::converters::hwpx_to_hwp::tests::hwp3_nested_caption_hidden_comment_and_master_page_pictures_are_normalized -- --exact --nocapture` 1 passed, 0 failed |
| 추가 전체 Cargo | 최신 CI와 중복되며 실행하지 않음. 검토 초기에 시작된 중복 전체 suite는 완료 결과로 사용하지 않고 범위 변경 직후 종료 |
| 시각 검증 | N/A. renderer/layout·PDF 기준 문서 변경이 아니며, 이 PR의 외부 정답지는 한글 열기 계약 |
| LFS 사전 판독 | review Markdown과 오늘할일은 `filter`/LFS attribute 모두 `unspecified`, `git lfs status` 대상 없음 |

## 발견한 P1과 메인터너 보정 범위

### P1 — HWPX 입력의 단일 BOTH `pageBorderFill`을 EVEN/ODD로 만들어 버린다

새 `convert_hwpx_to_hwp_ir()`는 `normalize_page_border_fills_for_hwp()`를 무조건 호출하고,
그 함수는 `extra_page_border_fills`가 두 개가 될 때까지 BOTH를 복제한다
(`src/document_core/converters/hwpx_to_hwp.rs:181-186`, `734-750`). 그러나 이 adapter는
`FileFormat::Hwpx`와 `FileFormat::Hwp3` 모두에 적용되고(`1933-1948`),
`export_hwp_with_adapter()`는 같은 `DocumentCore`의 IR을 in-place로 바꾼다
(`src/document_core/commands/document.rs:1154-1164`).

따라서 단일 BOTH가 정상인 실제 HWPX를 HWP로 저장한 뒤 이어서 HWPX로 저장하면, 원래 없던 EVEN/ODD가
재방출된다. 현재 HWPX serializer는 단일 BOTH 문서에서 그 복제를 명시적으로 금지한다. 복제하면
원본에 없던 요소가 생기고 #2896 IR field sweep baseline이 발산하기 때문이다
(`src/serializer/hwpx/section.rs:2544-2553`). 이는 이 PR이 기존 HWPX→HWP→HWPX 보존 계약에 도입하는
회귀다.

**적용한 보정:** `af00f9245`가 공통 HWPX adapter와 HWP3 materialization을 분리했다. HWPX source는
`extra_page_border_fills`를 보존하고, `convert_if_hwpx_source(..., Hwp3)`에서만 세 record를 채운다.
실제 단일-BOTH HWPX의 HWP→HWPX 재저장 XML에 EVEN/ODD가 생기지 않는 회귀를 추가했으며, 기존 HWP3
3-record 검사도 유지했다.

### P1 — HWP3가 실제 만드는 caption·숨은 주석 내부 그림을 보정 walker가 빠뜨린다

새 geometry/local-file-version walker는 본문, 표 셀, 일부 text box, 머리말/꼬리말, 각주/미주만 순회한다
(`src/document_core/converters/hwpx_to_hwp.rs:651-731`). PR의 전제처럼 개체 하나라도 0 값이 남으면
한글이 문서를 거부할 수 있는데, HWP3 parser가 실제 만드는 다음 경로를 방문하지 않는다.

- Picture caption (`src/parser/hwp3/mod.rs:1455-1488`): `Control::Picture` arm은 caption을 재귀하지 않는다.
- Table caption (`1272-1297`): Table arm은 셀만 순회하고 `table.caption`을 건너뛴다.
- Group/classic drawing caption (`1490-1544`): group child와 일부 text box만 처리하고 caption을 건너뛴다.
- `HiddenComment` (`1941-1946`)와 HWP3 OLE로 옮겨진 caption (`4539-4572`): walker에 해당 재귀가 없다.

공통 adapter 경로에서는 Chart/OLE text box와 `section_def.master_pages`도 같은 이유로 누락된다. 반면 기존
bin-order/remap/adapt walker는 caption·HiddenComment·drawing text box·master page를 모두 방문한다
(`hwpx_to_hwp.rs:390-461`, `469-554`, `1098-1141`, `1468-1487`, `1723-1738`).

**적용한 보정:** `4c7082710`이 paragraph/control/shape 공통 재귀 walker로 교체해 Picture/Table/Group/
drawing/OLE caption, HiddenComment, 모든 `drawing_mut()` text box, field memo, SectionDef와 master page를
함께 방문한다. HWP3 실제 caption·HiddenComment 경로와 공통 adapter의 master-page OLE text box 안 nested
Picture에 대해 geometry·crop·`local_file_version == 1`을 정확히 고정하는 library regression을 추가했다.

### P1 — Windows batch 도구가 사용 중인 한글 프로세스를 강제 종료할 수 있다

`tools/hwp3_convert_openable.py`의 child는 `Hwp(visible=False)`를 사용한다(28행). 설치된
`pyhwpx 1.7.2`의 기본 `new=False`는 ROT의 기존 `!HwpObject.*`에 attach할 수 있으며, finally의
`hwp.quit()`(36-40행)은 attach한 인스턴스에도 `Quit()`을 호출한다. 더 직접적으로 `kill_hangul()`은
`taskkill /F /IM Hwp.exe`와 `HwpApp.exe`를 모든 문서 전(80행)과 timeout 뒤(90행)에 실행한다
(43-46행). 저장하지 않은 사용자의 한글 작업까지 잃을 수 있다.

**적용한 보정:** `fcf101e68`이 child를 `Hwp(new=True, visible=False)`로 명시하고 이미지명 전역
`taskkill`과 timeout 뒤 전역 종료를 제거했다. timeout은 검사 Python 자식만 끝내며, COM 서버가 남을 수
있는 환경은 전용 Windows 검증 호스트에서 정리해야 한다는 precondition을 명시했다. Python mock regression은
새 hidden instance 생성, 자기 instance의 `quit()`, timeout 경로의 전역 kill 부재를 고정한다.

## 현재 상태와 권고

**권고: 메인터너 보정 완료, 최신 CI 대기 — merge 보류.** 세 P1은 focused regression으로 보정했으며
full suite는 원 source CI와 중복돼 실행하지 않았다. 이 tail을 contributor source PR head에 push한 뒤 새
latest head의 required CI success·`CLEAN`을 다시 확인한다. 그 뒤에만 리뷰 승인과 merge 판단을 재개한다.

이 maintainer tail은 위 P1을 해소하는 source/test와 review 기록·오늘할일만 바꾼다. 별도 PR은 만들지 않는다.
