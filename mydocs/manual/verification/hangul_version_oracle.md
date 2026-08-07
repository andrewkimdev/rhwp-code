---
kind: reference
status: active
canonical: mydocs/manual/verification/visual_verification_governance.md
last_verified: 2026-08-07
---

# 한글 버전별 페이지네이션 대조 가이드

`tools/hangul_version_oracle/` — 같은 문서를 **한글 버전별로 열어 페이지네이션을 대조**하는 하니스다.
rhwp의 1차 정답지는 한컴 편집기인데 편집기 버전마다 조판이 갈리므로, **어느 문서에서 정답지가
갈라지는지**를 먼저 확정해야 한다.

첫 측정 결과는 [한글 버전 오라클 대조 r1](../../report/hangul_version_oracle_r1_20260807.md)이다 —
10,000건 중 247건(2.47%)이 한글 2022와 2024에서 다르게 페이지네이션됐다.
정답지 버전 결정은 [Discussion #4137](https://github.com/edwardkim/rhwp/discussions/4137)에서 논의 중이다.

## 1. 무엇을 재는가

쪽수만 비교하면 "쪽수는 같은데 쪽 나눔 위치가 다른" 경우를 놓친다. 문서마다 **페이지네이션 지문**을
뽑아 통째로 비교한다.

```text
문단 0..N 을 SetPos 로 훑으며 XHwpDocumentInfo.CurrentPage 를 읽고,
쪽이 바뀌는 지점만 남긴 런렝스 목록  =  "쪽@시작문단, 쪽@시작문단, ..."
```

[`tools/verify_pi_page_vs_hangul.py`](../../../tools/verify_pi_page_vs_hangul.py)의 per-PI 쪽위치와 같은
계약이다. 쪽·문단 인덱스는 모두 0-based다.

판정 분류:

| 분류 | 뜻 |
| --- | --- |
| `PAGE_DELTA` | 총 쪽수가 다름 |
| `BREAK_DIFF` | 쪽수는 같으나 쪽 나눔 위치가 다름 |
| `PARA_DIFF` | 문단 수 자체가 다름(문서를 다르게 읽음) |
| `ERR` | 한쪽 이상에서 지문을 못 만듦 |
| `MISSING` | 한쪽 패스에만 있음 |

## 2. 요구사항

- **Windows**, 한컴오피스 한글 **2종 이상 설치**(2018·2020·2022·2024 등 병렬 설치).
- Windows PowerShell 5.1(기본 `powershell.exe`). 추가 설치 불필요 — Python·pyhwpx 필요 없다.
- 관리자 권한 **불필요**. 버전 전환은 HKCU만 건드린다.
- 한글 UI를 쓰지 않는 시간대에 돌린다. 측정 중 사용자의 한글 창이 뜨고, 남은 `Hwp.exe`가 있으면
  측정이 무효가 된다(4절).

## 3. 코퍼스

r1과 같은 표본을 쓰려면 공유본을 받는다. 두 링크를 합쳐 **10,000건**이며 구성은
**`.hwp` 6,582 / `.hwpx` 3,418**이다. 받은 뒤 개수로 확인한다.

- https://naver.me/5yh8sh7K
- https://naver.me/GCJhfJw2

```powershell
$Root = 'D:\hwpdocs_10k_share'   # 압축을 푼 위치
$files = Get-ChildItem $Root -Recurse -File -Include *.hwp,*.hwpx | Sort-Object FullName
$files.Count                      # 10000 이어야 한다
$files | Group-Object Extension | Select-Object Name, Count
[System.IO.File]::WriteAllLines("$Root\..\corpus_10k.txt", ($files | ForEach-Object { $_.FullName }), (New-Object System.Text.UTF8Encoding($false)))
```

목록 파일은 **UTF-8(BOM 없음)** 이어야 한다. 한글 파일명이 깨지면 전부 `ERR`이 된다.
다른 코퍼스를 써도 되며, 절대경로 목록과 `-Root`만 맞추면 된다.

## 4. 반드시 지킬 것 — 병렬 실행 금지

**한글 COM 인스턴스를 동시에 여러 개 띄우면 같은 문서에 다른 페이지네이션이 나온다.** r1에서 실측한
근거다.

| 검증 | 결과 |
| --- | --- |
| 같은 버전·같은 200문서를 **워커 1개 vs 5개**로 측정 | **36건(18%) 불일치** |
| 문서마다 새 인스턴스 vs 인스턴스 1개 유지(워커 1개) | **200/200 완전 일치** |
| 병렬 실행에서 인스턴스 내 처리 순서 구간별 차이율 | 16.7% ↔ 62.0% 로 요동 |

정확도에 필요한 것은 "문서마다 새 인스턴스"가 아니라 **동시 실행 금지**다. 인스턴스는 오래 유지해도
되므로 기본값은 `-Workers 1 -RecycleEvery 0`이다.

**함정** — 같은 조건을 두 번 돌리는 자기일관성 시험은 이 오염을 **잡지 못한다.** 오염이 처리 순서에
대해 결정적이라 두 실행이 똑같이 틀린다. 병렬도를 바꿔 대조해야 드러난다.

`-Workers`를 1보다 크게 주면 스크립트가 경고를 낸다. 처리량 실험 외에는 쓰지 않는다.

## 5. 절차

### 5.1 설치된 버전 확인

```powershell
powershell -File tools\hangul_version_oracle\list_hangul_versions.ps1
```

설치된 릴리스, 각 `Hwp.exe`의 ProductVersion, 현재 COM이 실제로 넘겨주는 버전을 보여준다.
`-HwpVersion`에 넣을 값은 여기 `Version` 열(설치 폴더명 `Hnc\Office <값>`)이다.

버전 major는 바이너리에서 직접 읽으므로 **설치만 되어 있으면 어느 릴리스든 쓸 수 있다.**
실측 대응: **2022 = 12.x**, **2024 = 13.x** (r1에서 확인). 2018 = 10.x, 2020 = 11.x는 같은 계열의 역산이며,
스크립트는 하드코딩된 표를 쓰지 않으므로 실제 값과 어긋날 일이 없다.

### 5.2 버전별 패스

버전마다 한 번씩, **순차로** 돌린다.

```powershell
powershell -File tools\hangul_version_oracle\page_oracle_run.ps1 `
  -HwpVersion 2022 -ListPath corpus_10k.txt -OutDir pass2022 -Root 'D:\hwpdocs_10k_share'

powershell -File tools\hangul_version_oracle\page_oracle_run.ps1 `
  -HwpVersion 2024 -ListPath corpus_10k.txt -OutDir pass2024 -Root 'D:\hwpdocs_10k_share'
```

표준 경로가 아니면 `-HwpVersion` 대신 `-HwpExe 'C:\...\Hwp.exe'`를 쓴다.

각 패스는 시작할 때 **오버라이드가 실제로 먹었는지 COM을 한 번 띄워 확인**하고, 어긋나면 즉시 중단한다
(패스 하나를 통째로 버리는 대신 몇 초 만에 알려준다). 워커도 인스턴스를 만들 때마다 major를 다시 검증한다.

산출물은 `OutDir/result_0.tsv`이며 컬럼은 `relpath / status / pages / paras / breakCount / fingerprint`다.
중단해도 같은 명령을 다시 주면 남은 문서부터 이어서 한다.

**실측 소요**(10,000건, 워커 1개): 2022 약 22분, 2024 약 28분.

### 5.3 비교

```powershell
powershell -File tools\hangul_version_oracle\compare_passes.ps1 `
  -DirA pass2022 -DirB pass2024 -LabelA 2022 -LabelB 2024 -OutPath diff.tsv
```

`diff.tsv`에는 **다른 문서만** 남는다(`kind / path / detail`). `detail`은 쪽수 차이 또는 최초 분기 지점이다.
`BREAK_DIFF`의 `first divergence #1: 2022=1@7 2024=1@8`은 2쪽이 2022에서는 7번 문단, 2024에서는 8번
문단에서 시작한다는 뜻이다.

### 5.4 재현 검증 — 생략하지 말 것

차이 목록을 그대로 믿지 않는다. **다르다고 나온 문서 + 같다고 나온 대조군을 섞어 순서를 바꿔 독립
재실행**하고, 같은 분류로 재현된 것만 확정한다. r1에서는 이 절차로 254건 중 247건(98.8%)이 재현됐고
대조군 100건에서 새 차이는 0건이었다.

```powershell
# diff.tsv 의 경로 + 무작위 대조군을 섞어 verify_list.txt 를 만든 뒤
powershell -File tools\hangul_version_oracle\page_oracle_run.ps1 -HwpVersion 2022 -ListPath verify_list.txt -OutDir verify2022 -Root 'D:\hwpdocs_10k_share'
powershell -File tools\hangul_version_oracle\page_oracle_run.ps1 -HwpVersion 2024 -ListPath verify_list.txt -OutDir verify2024 -Root 'D:\hwpdocs_10k_share'
powershell -File tools\hangul_version_oracle\compare_passes.ps1 -DirA verify2022 -DirB verify2024 -OutPath verify_diff.tsv
```

### 5.5 복원

```powershell
powershell -File tools\hangul_version_oracle\restore_com_default.ps1
```

HKCU 오버라이드 값을 **기계 기본값으로 되돌린다.** 키를 지우지 않는다 — 이유는 6절.

## 6. 버전 전환은 어떻게 동작하나, 그리고 절대 하지 말 것

한글은 릴리스가 달라도 **같은 CLSID** `{2291CF00-64A1-4877-A9B4-68CFE89612D6}`를 쓴다. HKLM 등록은
**마지막에 설치한 버전**을 가리키므로, 그대로 두면 어떤 버전을 원하든 그 하나만 잡힌다. 하니스는 HKCU에
같은 CLSID의 `LocalServer32`를 덮어써서 원하는 `Hwp.exe`를 고른다. COM 활성화에서 HKCU가 HKLM보다
우선하고, 사용자 단위라 관리자 권한이 필요 없다.

```text
HKCU\Software\Classes\CLSID\{2291CF00-...}\LocalServer32
HKCU\Software\Classes\Wow6432Node\CLSID\{2291CF00-...}\LocalServer32
  = "<Hwp.exe 경로> -Automation"
```

전환을 무효로 만드는 것이 **둘** 있다. 둘 다 조용히 다른 버전을 재게 만들므로 반드시 알아야 한다.

1. **남아 있는 `Hwp.exe`.** 이미 떠 있는 인스턴스가 있으면 COM은 오버라이드와 무관하게 **그 인스턴스에
   붙는다.** `page_oracle_run.ps1`이 시작할 때 잔여 프로세스를 정리하지만, 사용자가 한글 UI를 띄우면
   다시 생긴다.
2. **HKCU CLSID 키 삭제.** `HKCU\Software\Classes\[Wow6432Node\]CLSID\{2291CF00-...}`를 **지우면 그
   로그인 세션 동안 COM이 HKCU를 아예 무시한다.** 값을 다시 써도 소용없고, 이후 모든 활성화가 HKLM
   기본값으로 간다. **로그오프 후 다시 로그인해야 복구된다.** (Windows 11 + 한글 2022·2024 실측)

   그래서 `restore_com_default.ps1`은 키를 지우지 않고 **값을 기계 기본값으로 되돌린다.** 정리하려고
   `Remove-Item`으로 키를 날리지 말 것. 정말 필요하면 `-Purge`가 있지만 로그오프 직전에만 쓴다.

증상은 명확하다 — 패스 시작 시 `COM did not honour the HKCU override.`로 즉시 중단되고 위 두 항목을
안내한다.

## 7. 작성 앱 버전 분포

"어느 버전을 정답지로 둘 것인가"는 그 문서들이 **어느 편집기에서 나왔는가**와 떨어져 있지 않다.

```powershell
powershell -File tools\hangul_version_oracle\scan_appversion.ps1 `
  -ListPath corpus_10k.txt -OutPath appversion.tsv -Root 'D:\hwpdocs_10k_share'
```

- HWPX는 `version.xml`의 `appVersion`에 **저장한 한글의 버전이 그대로** 기록된다.
- HWP5에는 작성 앱 버전이 없다. FileHeader의 포맷 버전만 얻을 수 있고, 세대의 약한 지표일 뿐이다.

r1 실측(HWPX 3,418건): 한글 2020이 70.7%로 최빈, 2024 저장분은 1.0%, 2022 이하가 98.4%였다.

## 8. 알려진 함정

- **`ShowWindow(SW_HIDE)`로 한글 창을 숨기지 말 것.** 자동화가 교착된다 — r1에서 워커 전원이 첫 문서에서
  정지했고 숨김을 중단하자 즉시 재개됐다. 창은 COM `XHwpWindows.Visible = false`로만 다룬다.
- **`GetPos()`는 PowerShell에서 호출할 수 없다**(`[out]` 파라미터). `GetPosBySet()`을 쓴다.
- **FilePathCheckerModule은 없어도 된다.** `RegisterModule`이 `False`를 반환해도 r1의 10,000건 × 2버전
  전 구간에서 파일 접근 보안 대화상자는 한 번도 뜨지 않았다. `SetMessageBoxMode(0x00020000)`이면 충분하다.
- **PowerShell 스크립트에 한글 주석을 넣지 말 것.** Windows PowerShell 5.1은 BOM 없는 `.ps1`을 ANSI로
  읽어 파싱이 깨진다. `tools/hangul_version_oracle/`의 스크립트는 전부 ASCII다.
- 문서 하나가 한글을 멈춰 세우는 일이 있다. 감시자가 `-StallSeconds`(기본 300) 초과 시 해당 `Hwp.exe`만
  종료하고 워커가 복구한다. 그 문서는 `ERR`로 남으므로 마지막에 따로 재시도한다.

## 9. 관련

- 측정 보고서: [한글 버전 오라클 대조 r1](../../report/hangul_version_oracle_r1_20260807.md)
- 정답지 버전 논의: [Discussion #4137](https://github.com/edwardkim/rhwp/discussions/4137)
- 원본↔저장본 쪽수 오라클: [한글 페이지 충실도 오라클](hangul_page_oracle.md)
- 시각 검증 지도: [verification/README](README.md)
