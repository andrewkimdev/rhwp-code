---
kind: guide
status: active
canonical: mydocs/manual/agent_troubleshooting_guide.md
last_verified: 2026-07-30
---

# 에이전트 실패 사전 — 증상으로 찾는 원인과 처방

AI 에이전트·스크립트가 rhwp 를 부릴 때 반복해서 밟는 실패를 **오류 문자열 그대로
검색되는 표제**로 정리한다. 각 항목은 증상 → 원인 → 처방 → 근거 순이다.
명령·옵션의 canonical reference 는 [CLI 명령어 매뉴얼](cli_commands.md).

판정의 대원칙은 [종료 코드 계약(#2707)](cli_commands.md#종료-코드-2707)이다:

- **exit 2 = 호출을 조립한 쪽(에이전트)의 버그.** 재시도하지 말고 인자를 고친다.
- **exit 1 = 실행 환경/입력 파일의 문제.** 파일 존재·형식·권한부터 본다.
- **exit 3/4 = 오류가 아니라 검증 판정.** 차이가 "발견"된 것이다.

## 입력·인코딩

### "stream did not contain valid UTF-8" (exit 1)

- **증상**: `edit fill-fields --data @row.json` 이 즉시 실패.
- **원인**: 데이터 파일이 UTF-8 이 아니다. Windows 에서 기본 인코딩(cp949)으로 저장한
  JSON 파일이 전형이다 — 메모장·PowerShell `>` 리다이렉트·Python `open(..., 'w')`
  전부 기본값이 cp949 다.
- **처방**: 데이터 파일은 항상 UTF-8(BOM 없이)로 쓴다. Python 은
  `open(path, 'w', encoding='utf-8')`, PowerShell 은 `Set-Content -Encoding utf8NoBOM`.
- **근거**: [규제영향분석서 사례](../report/edit_demo_regulatory/README.md)의 함정 기록.

### 한글 파일명이 ??? 로 깨지거나 파일을 못 찾음

- **원인**: 셸의 코드페이지/로케일. rhwp 자체는 경로를 그대로 받는다.
- **처방**: Git Bash·PowerShell 7+ 를 쓰고, 경로에 공백이 있으면 따옴표로 감싼다.
  자동화에서는 경로를 ASCII 임시 사본으로 복사해 처리하는 것이 가장 견고하다.

## 사용법 (exit 2 계열)

### "알 수 없는 옵션: -o" — 명령마다 옵션 표면이 다르다

- **증상**: 다른 명령에서 쓰던 플래그가 특정 명령에서 exit 2.
- **원인**: 옵션 표면은 명령별 계약이다. 예: `export-hwpx` 는 출력을 **positional**
  (`export-hwpx <입력> [출력.hwpx]`)로 받고 `-o` 가 없다.
- **처방**: 추측하지 말고 `rhwp capabilities` 에서 해당 명령의 `flags` 를 읽는다.
  에이전트라면 온보딩 시 `capabilities` 한 번 호출로 전 명령 표면을 캐시한다.

### "페이지 번호가 범위를 벗어났습니다 (0~N)" (exit 2)

- **원인**: 페이지는 **0 기준**이다. 사람용 "5쪽"은 `-p 4` 다.
- **처방**: `search --json` 의 `matches[].page` 값은 이미 0 기준이므로 그대로
  `-p` 에 넣으면 된다. 사람에게 보여줄 때만 +1 한다.

### 파일 positional 을 두 번 줌 (exit 2)

- **원인**: 옵션 값을 빠뜨리면 다음 인자가 파일로 해석된다
  (예: `--profile` 뒤에 값 없이 파일명). #3359 이후 조용히 삼키지 않고 즉시 2 로 끝난다.
- **처방**: exit 2 는 조립 버그 신호다 — stderr 의 사용법 안내를 읽고 인자를 고친다.

## 편집 응답의 오독

### `filledCount` 는 성공했는데 서식이 덜 채워짐

- **원인**: ① 문서에 없는 이름은 `notFound` 로 보고되고 건너뛴다. ② 같은 이름이
  여러 번 나오면 첫 번째만 채워지고 `ambiguous` 로 보고된다.
- **처방**: `notFound == [] && ambiguous == []` 를 게이트로 건다. 반복 필드는
  `이름[N]`(0 기준, `fields --json` 목록 순서) 으로 재지목한다.
- **근거**: [CLI 매뉴얼 — edit fill-fields](cli_commands.md) 절, #3476 (심화 가이드는 #3574).

### set-cell 이 "병합으로 덮인 칸" 이라며 실패

- **원인**: 그 좌표는 병합(rowSpan/colSpan) 아래 숨은 칸이다. 값을 넣으면 렌더에
  안 보이는 유령 데이터가 된다 — 실패가 보호 동작이다.
- **처방**: 실패 메시지가 안내하는 **앵커 좌표**로 다시 쓴다.

### 치환했는데 출력 파일이 없음

- **원인**: `replace-text` 는 치환 0건이면 출력 파일을 만들지 않는다(의도된 동작).
- **처방**: `replacedCount` 를 먼저 본다. 0 이면 `--find` 문자열이 문서 표기와
  다른 것이다(전각/반각, 공백, 줄바꿈). `search` 로 실제 표기를 먼저 확인한다.

## 검증 판정 (exit 3/4)

### `--verify` 가 exit 3 — 변환이 실패했나?

- **원인 아님**: 변환 산출물은 이미 저장됐다. exit 3 은 "재파싱한 IR 이 원본과
  다르다"는 **판정**이다.
- **처방**: `ir-diff <원본> <산출물> --json` 으로 `categories` 를 본다.
  편집을 거친 산출물이면 의도한 변경(텍스트·셀)만 있는지, 순수 변환이면
  차이 카테고리를 이슈로 보고한다(형식별 알려진 잔여 결함이 있을 수 있다).
- 배치 게이트를 세울 때는 exit 0/3 을 분기하고, 3 을 "불합격"이 아니라
  "검토 대상" 큐로 보낸다.

## 환경·빌드

### `export-png` 가 exit 2 — "기능 부재"

- **원인**: `native-skia` feature 없이 빌드된 바이너리다.
- **처방**: 호출 전에 `capabilities` 의 해당 명령 `available` 필드를 본다(#3357).
  `false` 면 PNG 대신 `export-svg` 로 대체하거나 feature 포함 빌드를 쓴다.

### 보호 문서: exit 2 와 exit 1 의 구분

- **비밀번호를 안 줬다** → exit 2 (사용법). `--password-stdin < pw.txt` 로 준다.
- **비밀번호가 틀렸거나, 지원하지 않는 암호화** (HWP5 EncryptVersion 1~3,
  비압축 HWP3 암호 본문, DRM) → exit 1 (런타임).
- 상세 지원 매트릭스: [CLI 매뉴얼 — 비밀번호 보호 HWP](cli_commands.md#비밀번호-보호-hwp).
  `--password` 값은 프로세스 목록에 노출된다 — 자동화에서는 `--password-stdin` 을 쓴다.

### 렌더 산출물의 글꼴이 문서와 다름

- **원인**: 문서가 쓰는 폰트가 실행 환경에 없어 번들 대체 폰트로 떨어졌다.
- **처방**: 필요한 폰트를 설치하고 `--font-path` 또는 환경변수 `RHWP_FONT_PATH` 로
  명시한다. 서버·컨테이너 대량 변환에서 특히 필수다.

## 배치·파이프라인

### batch 가 exit 1 인데 결과는 다 나온 것 같음

- **원인**: exit 1 은 **부분 실패**다 — NDJSON 레코드는 입력 순서대로 전부 나오고,
  실패한 파일만 `error`/`exitClass` 필드를 가진 레코드로 나온다.
- **처방**: exit 코드로 전체를 버리지 말고 레코드 단위로 분류한다:
  `jq -c 'select(.error != null)'` 로 실패분만 추려 재시도/보고한다.

### `--json` 출력에 로그가 섞여 파싱 실패

- **원인 아님(설계)**: `--json` 모드의 stdout 은 순수 JSON 하나(배치는 NDJSON)다.
  진단·진행 메시지는 전부 stderr 로 나간다.
- **처방**: stdout 만 파이프에 태우고 stderr 는 로그로 보존한다
  (`2>err.log`). stdout 파싱이 실제로 깨졌다면 그 자체가 버그이므로 이슈로 보고한다.

## 그래도 안 풀리면

1. `rhwp capabilities` 로 명령 표면·계약을 재확인한다 (추측 금지).
2. 같은 입력으로 사람용 모드(`--json` 없이)를 실행해 stderr 안내를 읽는다.
3. `info` → `dump`/`diag` 순으로 입력 문서 자체의 이상을 좁힌다
   ([문서 진단 도구](document_diagnostics_tool_manual.md)).
4. 재현 명령·stderr·샘플(공유 가능한 것)로 이슈를 연다 — 증상 문자열을 제목에
   그대로 넣으면 다음 사람이 이 사전에서 찾는다.
