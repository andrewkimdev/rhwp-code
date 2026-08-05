#!/usr/bin/env python
"""HWP/HWPX document text-diff tool.

Compares the extracted text of two documents (HWP, HWPX, or any format the
`rhwp` CLI can read) page by page and reports the differences. Extraction is
delegated to `rhwp export-text --json` rather than re-implemented here, so
this tool stays correct across format/parser changes instead of drifting
from the real parser.

Exit codes follow the repo-wide CLI contract (mydocs/manual/cli_commands.md):
  0  no differences
  1  runtime failure (rhwp binary not found, export failed, file missing)
  2  usage error (bad arguments)
  3  differences detected
"""

import argparse
import difflib
import json
import os
import subprocess
import sys
from pathlib import Path


def find_rhwp_binary(explicit):
    if explicit:
        return explicit
    from shutil import which

    on_path = which("rhwp")
    if on_path:
        return on_path

    repo_root = Path(__file__).resolve().parents[2]
    for profile in ("release", "debug"):
        for name in ("rhwp.exe", "rhwp"):
            candidate = repo_root / "target" / profile / name
            if candidate.is_file():
                return str(candidate)

    return None


def export_text(rhwp_bin, path, max_chars=None):
    args = [rhwp_bin, "export-text", str(path), "--json"]
    if max_chars is not None:
        args += ["--max-chars", str(max_chars)]

    try:
        result = subprocess.run(
            args, capture_output=True, text=True, encoding="utf-8", check=False
        )
    except OSError as exc:
        raise RuntimeError(f"rhwp 실행 실패: {exc}") from exc

    if result.returncode != 0:
        raise RuntimeError(
            f"'{path}' export-text 실패 (exit {result.returncode}): {result.stderr.strip()}"
        )

    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"'{path}' export-text --json 출력 파싱 실패: {exc}") from exc


def diff_pages(pages_a, pages_b):
    """Diff two documents' pages by index, returning a list of per-page diffs."""
    page_count = max(len(pages_a), len(pages_b))
    reports = []

    for i in range(page_count):
        text_a = pages_a[i]["text"] if i < len(pages_a) else None
        text_b = pages_b[i]["text"] if i < len(pages_b) else None

        if text_a == text_b:
            continue

        if text_a is None:
            reports.append({"page": i, "kind": "added", "lines": text_b.splitlines()})
            continue
        if text_b is None:
            reports.append({"page": i, "kind": "removed", "lines": text_a.splitlines()})
            continue

        lines_a = text_a.splitlines()
        lines_b = text_b.splitlines()
        unified = list(
            difflib.unified_diff(lines_a, lines_b, lineterm="", n=1)
        )
        reports.append({"page": i, "kind": "changed", "diff": unified})

    return reports


def print_human_report(source_a, source_b, page_count_a, page_count_b, reports):
    print(f"--- {source_a}")
    print(f"+++ {source_b}")

    if page_count_a != page_count_b:
        print(f"페이지 수 다름: {page_count_a} -> {page_count_b}")

    if not reports:
        print("차이 없음")
        return

    for report in reports:
        page = report["page"]
        if report["kind"] == "added":
            print(f"\n[page {page}] 새 문서에만 존재")
            for line in report["lines"]:
                print(f"+{line}")
        elif report["kind"] == "removed":
            print(f"\n[page {page}] 원본에만 존재 (새 문서에서 제거됨)")
            for line in report["lines"]:
                print(f"-{line}")
        else:
            print(f"\n[page {page}]")
            for line in report["diff"]:
                print(line)


def main():
    parser = argparse.ArgumentParser(
        prog="doc-diff",
        description="두 HWP/HWPX 문서의 텍스트 내용을 페이지 단위로 비교한다",
    )
    parser.add_argument("file_a", help="원본 문서")
    parser.add_argument("file_b", help="비교 대상 문서")
    parser.add_argument("--json", action="store_true", help="JSON 결과 출력")
    parser.add_argument(
        "--max-chars",
        type=int,
        default=None,
        help="문서당 추출 문자 상한 (export-text --max-chars 로 전달)",
    )
    parser.add_argument(
        "--rhwp-bin",
        default=None,
        help="rhwp 바이너리 경로 (기본: PATH 또는 target/{release,debug}/rhwp)",
    )
    args = parser.parse_args()

    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")

    if not os.path.isfile(args.file_a):
        print(f"파일 없음: {args.file_a}", file=sys.stderr)
        return 2
    if not os.path.isfile(args.file_b):
        print(f"파일 없음: {args.file_b}", file=sys.stderr)
        return 2

    rhwp_bin = find_rhwp_binary(args.rhwp_bin)
    if rhwp_bin is None:
        print(
            "rhwp 바이너리를 찾을 수 없음 — --rhwp-bin 로 경로를 지정하거나 "
            "PATH 또는 target/{release,debug}/ 에 빌드하세요.",
            file=sys.stderr,
        )
        return 1

    try:
        doc_a = export_text(rhwp_bin, args.file_a, args.max_chars)
        doc_b = export_text(rhwp_bin, args.file_b, args.max_chars)
    except RuntimeError as exc:
        print(str(exc), file=sys.stderr)
        return 1

    reports = diff_pages(doc_a["pages"], doc_b["pages"])
    has_diff = bool(reports) or doc_a["pageCount"] != doc_b["pageCount"]

    if args.json:
        print(
            json.dumps(
                {
                    "schemaVersion": "1.0",
                    "sourceA": args.file_a,
                    "sourceB": args.file_b,
                    "pageCountA": doc_a["pageCount"],
                    "pageCountB": doc_b["pageCount"],
                    "hasDiff": has_diff,
                    "pages": reports,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    else:
        print_human_report(
            args.file_a, args.file_b, doc_a["pageCount"], doc_b["pageCount"], reports
        )

    return 3 if has_diff else 0


if __name__ == "__main__":
    sys.exit(main())
