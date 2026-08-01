from __future__ import annotations

import contextlib
import importlib.util
import io
import subprocess
import sys
import types
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).resolve().parents[2] / "tools" / "hwp3_convert_openable.py"
SPEC = importlib.util.spec_from_file_location("hwp3_convert_openable", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"hwp3_convert_openable 모듈을 불러올 수 없습니다: {MODULE_PATH}")
OPENABLE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = OPENABLE
SPEC.loader.exec_module(OPENABLE)


class IsolatedHangulProcessTests(unittest.TestCase):
    def test_child_creates_and_quits_only_a_new_hidden_instance(self) -> None:
        created: list[tuple[bool, bool]] = []
        cleared: list[int] = []
        quit_called: list[bool] = []

        class FakeHwp:
            PageCount = 2

            def __init__(self, *, new: bool, visible: bool) -> None:
                created.append((new, visible))

            def open(self, _path: str) -> bool:
                return True

            def clear(self, *, option: int) -> None:
                cleared.append(option)

            def quit(self) -> None:
                quit_called.append(True)

        fake_pyhwpx = types.ModuleType("pyhwpx")
        fake_pyhwpx.Hwp = FakeHwp  # type: ignore[attr-defined]
        with mock.patch.dict(sys.modules, {"pyhwpx": fake_pyhwpx}):
            with contextlib.redirect_stdout(io.StringIO()):
                OPENABLE.child("source.hwp", "converted.hwp")

        self.assertEqual(created, [(True, False)])
        self.assertEqual(cleared, [1])
        self.assertEqual(quit_called, [True])

    def test_timeout_runs_no_global_hangul_process_kill(self) -> None:
        timeout = subprocess.TimeoutExpired(["python", "worker.py"], 1)
        with mock.patch.object(OPENABLE.subprocess, "run", side_effect=timeout) as run:
            self.assertIsNone(OPENABLE.run_child("source.hwp", "converted.hwp", 1))

        self.assertEqual(run.call_count, 1)
        command = run.call_args.args[0]
        self.assertEqual(command[:3], [sys.executable, str(MODULE_PATH), "--child"])
        self.assertNotIn("taskkill", MODULE_PATH.read_text(encoding="utf-8").lower())


if __name__ == "__main__":
    unittest.main()
