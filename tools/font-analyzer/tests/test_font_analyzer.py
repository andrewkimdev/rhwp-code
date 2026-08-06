#!/usr/bin/env python3
"""font-analyzer 실측 회귀 테스트.

실제 저장소 fixture(samples/field-01.hwp, samples/hwp3-sample5-hwpx.hwpx)에
대해 도구를 서브프로세스로 실행해 `rhwp info --json` 계약 위의 동작을 검증한다.

실행:
    RHWP_BIN=target/debug/rhwp python tools/font-analyzer/tests/test_font_analyzer.py

RHWP_BIN을 지정하지 않으면 PATH의 rhwp 또는 저장소 target/ 빌드를 자동 탐색한다.
둘 다 없으면 명확한 오류로 실패한다 (빌드: cargo build --bin rhwp).
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TESTS_DIR = Path(__file__).resolve().parent
TOOL_DIR = TESTS_DIR.parent
REPO = TOOL_DIR.parents[1]
TOOL = TOOL_DIR / "font_analyzer.py"

HWP_FIXTURE = REPO / "samples" / "field-01.hwp"
HWPX_FIXTURE = REPO / "samples" / "hwp3-sample5-hwpx.hwpx"

sys.path.insert(0, str(TOOL_DIR))
import font_analyzer  # noqa: E402

RHWP_BIN: str = ""


def setUpModule() -> None:  # noqa: N802 (unittest 규약)
    global RHWP_BIN
    try:
        RHWP_BIN = font_analyzer.resolve_rhwp_bin(None)
    except font_analyzer.ToolError as exc:
        raise RuntimeError(
            f"rhwp 실행 파일이 필요합니다: {exc}\n"
            "RHWP_BIN 환경변수로 지정하거나 cargo build --bin rhwp 후 재실행하세요."
        ) from exc
    for fixture in (HWP_FIXTURE, HWPX_FIXTURE):
        if not fixture.is_file():
            raise RuntimeError(f"fixture가 없습니다: {fixture}")


def run_tool(*args: str, rhwp_bin: str | None = None) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["RHWP_BIN"] = rhwp_bin if rhwp_bin is not None else RHWP_BIN
    return subprocess.run(
        [sys.executable, str(TOOL), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
        cwd=str(REPO),
    )


class SingleFileTests(unittest.TestCase):
    def test_hwp_fixture_contains_expected_fonts(self) -> None:
        proc = run_tool(str(HWP_FIXTURE), "--format", "json")
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        result = json.loads(proc.stdout)
        self.assertEqual(result["format"], "hwp5")
        self.assertIn("함초롬돋움", result["fonts"])
        self.assertIn("함초롬바탕", result["fonts"])
        self.assertEqual(result["fontCount"], len(result["fonts"]))

    def test_hwpx_fixture_has_at_least_one_font(self) -> None:
        proc = run_tool(str(HWPX_FIXTURE), "--format", "json")
        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        result = json.loads(proc.stdout)
        self.assertEqual(result["format"], "hwpx")
        self.assertGreaterEqual(result["fontCount"], 1)
        self.assertTrue(all(isinstance(f, str) and f for f in result["fonts"]))


class FailureTests(unittest.TestCase):
    def test_missing_file_exits_nonzero(self) -> None:
        proc = run_tool(str(REPO / "samples" / "no-such-file.hwp"))
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("오류", proc.stderr)

    def test_invalid_rhwp_bin_exits_nonzero(self) -> None:
        proc = run_tool(str(HWP_FIXTURE), rhwp_bin=str(REPO / "no-such-rhwp.exe"))
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("RHWP_BIN", proc.stderr)

    def test_symbolic_link_input_exits_nonzero(self) -> None:
        if os.name == "nt":
            self.skipTest("Windows의 심볼릭 링크 생성 권한은 개발 환경마다 다릅니다")
        with tempfile.TemporaryDirectory() as tmp:
            alias = Path(tmp) / "linked.hwp"
            alias.symlink_to(HWP_FIXTURE)
            proc = run_tool(str(alias))
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("심볼릭 링크", proc.stderr)


class DirectoryTests(unittest.TestCase):
    def test_directory_aggregation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_dir = Path(tmp)
            (tmp_dir / HWP_FIXTURE.name).write_bytes(HWP_FIXTURE.read_bytes())
            (tmp_dir / HWPX_FIXTURE.name).write_bytes(HWPX_FIXTURE.read_bytes())

            proc = run_tool(str(tmp_dir), "--format", "json")
            self.assertEqual(proc.returncode, 0, msg=proc.stderr)
            result = json.loads(proc.stdout)

        self.assertEqual(result["fileCount"], 2)
        self.assertEqual(result["okCount"], 2)
        self.assertEqual(result["errorCount"], 0)
        self.assertGreaterEqual(result["uniqueFontCount"], 2)
        by_name = {font["name"]: font for font in result["fonts"]}
        self.assertIn("함초롬바탕", by_name)
        self.assertIn("함초롬돋움", by_name)
        self.assertGreaterEqual(by_name["함초롬바탕"]["fileCount"], 1)

    def test_directory_without_documents_exits_nonzero(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            proc = run_tool(tmp)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("오류", proc.stderr)

    def test_max_files_limit_stops_directory_collection(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "a.hwp").touch()
            (root / "b.hwpx").touch()
            with self.assertRaisesRegex(font_analyzer.ToolError, "--max-files 상한"):
                font_analyzer.collect_targets(root, recursive=False, max_files=1)


class OutputSafetyTests(unittest.TestCase):
    def test_source_file_cannot_be_output_file(self) -> None:
        original = HWP_FIXTURE.read_bytes()
        proc = run_tool(str(HWP_FIXTURE), "--output", str(HWP_FIXTURE))
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("분석 원본", proc.stderr)
        self.assertEqual(original, HWP_FIXTURE.read_bytes())

    def test_existing_output_requires_explicit_overwrite(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "fonts.txt"
            output.write_text("keep", encoding="utf-8")
            denied = run_tool(str(HWP_FIXTURE), "--output", str(output))
            self.assertNotEqual(denied.returncode, 0)
            self.assertIn("--overwrite", denied.stderr)
            self.assertEqual("keep", output.read_text(encoding="utf-8"))

            allowed = run_tool(
                str(HWP_FIXTURE), "--output", str(output), "--overwrite"
            )
            self.assertEqual(allowed.returncode, 0, msg=allowed.stderr)
            self.assertIn("글꼴", output.read_text(encoding="utf-8"))


class TimeoutTests(unittest.TestCase):
    def test_rhwp_timeout_is_reported_as_tool_error(self) -> None:
        with mock.patch.object(
            font_analyzer.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(["rhwp"], 1),
        ):
            with self.assertRaisesRegex(font_analyzer.ToolError, "끝나지 않았습니다"):
                font_analyzer.rhwp_info("rhwp", Path("sample.hwp"), 1)


class FormattingTests(unittest.TestCase):
    """바이너리가 필요 없는 순수 포매팅 검증."""

    def test_markdown_aggregate_table(self) -> None:
        result = {
            "root": "samples",
            "recursive": False,
            "fileCount": 2,
            "okCount": 1,
            "errorCount": 1,
            "uniqueFontCount": 1,
            "fonts": [{"name": "함초롬바탕", "fileCount": 1, "files": ["a.hwp"]}],
            "files": [],
            "errors": [{"source": "b.hwp", "error": "깨진 | 파일"}],
        }
        rendered = font_analyzer.format_markdown(result)
        self.assertIn("| 함초롬바탕 | 1 |", rendered)
        self.assertIn("깨진 \\| 파일", rendered)


if __name__ == "__main__":
    unittest.main(verbosity=2)
