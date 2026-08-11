"""릴리스 태그 정렬은 패키지 메타데이터와 런타임 보고값을 함께 바꾼다."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
from types import ModuleType

import pytest


def _load_version_tool() -> ModuleType:
    root = Path(__file__).resolve().parents[3]
    path = root / "tools" / "set_package_version.py"
    spec = importlib.util.spec_from_file_location("set_package_version", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_release_alignment_updates_metadata_and_runtime_versions(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    tool = _load_version_tool()
    pyproject = tmp_path / "pyproject.toml"
    py_init = tmp_path / "__init__.py"
    node_pkg = tmp_path / "package.json"
    node_index = tmp_path / "index.ts"
    pyproject.write_text('[project]\nversion = "0.1.0"\n', encoding="utf-8")
    py_init.write_text('__version__ = "0.1.0"\n', encoding="utf-8")
    node_pkg.write_text('{"name":"@rhwp/node","version":"0.1.0"}\n', encoding="utf-8")
    node_index.write_text("export const VERSION = '0.1.0';\n", encoding="utf-8")

    monkeypatch.setattr(tool, "PYPROJECT", pyproject)
    monkeypatch.setattr(tool, "PY_INIT", py_init)
    monkeypatch.setattr(tool, "NODE_PKG", node_pkg)
    monkeypatch.setattr(tool, "NODE_INDEX", node_index)

    tool.set_python("0.8.3")
    tool.set_node("0.8.3")

    assert 'version = "0.8.3"' in pyproject.read_text(encoding="utf-8")
    assert '__version__ = "0.8.3"' in py_init.read_text(encoding="utf-8")
    assert json.loads(node_pkg.read_text(encoding="utf-8"))["version"] == "0.8.3"
    assert "VERSION = '0.8.3'" in node_index.read_text(encoding="utf-8")
