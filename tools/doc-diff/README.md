# doc-diff

두 HWP/HWPX 문서의 텍스트 내용을 페이지 단위로 비교하는 CLI 도구다. 텍스트 추출은
직접 파싱하지 않고 `rhwp export-text --json` 에 위임한다 — 파서/포맷 변경에 따라
이 도구가 별도로 낡는 것을 막기 위해서다.

## 사용법

```bash
python doc_diff.py 원본.hwp 수정본.hwpx
python doc_diff.py 원본.hwp 수정본.hwpx --json
python doc_diff.py 원본.hwp 수정본.hwpx --rhwp-bin /path/to/rhwp
```

`rhwp` 바이너리는 기본적으로 PATH, 그다음 `target/release/`, `target/debug/` 순으로 찾는다.

## 종료 코드

`mydocs/manual/cli_commands.md` 의 종료 코드 계약을 따른다.

| 코드 | 의미 |
|---:|---|
| 0 | 차이 없음 |
| 1 | 런타임 실패 (rhwp 바이너리 없음, export-text 실패) |
| 2 | 사용법 오류 (파일 없음 등) |
| 3 | 차이 검출됨 |

## JSON 출력

```json
{
  "schemaVersion": "1.0",
  "sourceA": "...", "sourceB": "...",
  "pageCountA": 10, "pageCountB": 11,
  "hasDiff": true,
  "pages": [
    {"page": 3, "kind": "changed", "diff": ["--- ", "+++ ", "..."]},
    {"page": 10, "kind": "added", "lines": ["..."]}
  ]
}
```
