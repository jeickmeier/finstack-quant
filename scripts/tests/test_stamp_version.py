"""Tests for the build-version stamper."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "stamp_version.py"
_SPEC = importlib.util.spec_from_file_location("stamp_version", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)
stamp_version = _MODULE.stamp_version
workspace_version = _MODULE.workspace_version


def _workspace(root: Path) -> Path:
    (root / "Cargo.toml").write_text(
        '[workspace.package]\nversion = "0.12.0"\n\n'
        "[workspace.dependencies]\n"
        'sha2 = { version = "0.12.0", default-features = false }\n',
        encoding="utf-8",
    )
    member = root / "crates" / "app"
    member.mkdir(parents=True)
    (member / "Cargo.toml").write_text(
        '[package]\nname = "app"\nversion.workspace = true\n\n'
        "[dependencies]\n"
        'core = { path = "../core", version = "0.12.0", default-features = false }\n'
        'serde = { version = "0.12.0" }\n',
        encoding="utf-8",
    )
    wasm = root / "finstack-quant-wasm"
    wasm.mkdir()
    (wasm / "package.json").write_text('{\n  "name": "w",\n  "version": "0.12.0"\n}\n', encoding="utf-8")
    (wasm / "package-lock.json").write_text(
        json.dumps({"name": "w", "version": "0.12.0", "packages": {"": {"version": "0.12.0"}}}, indent=2) + "\n",
        encoding="utf-8",
    )
    return member / "Cargo.toml"


@pytest.mark.parametrize("version", ["0.12.0-dev.202610210600", "0.12.0-rc.1"])
def test_stamps_workspace_path_requirements_and_npm(tmp_path: Path, version: str) -> None:
    """A pre-release reaches the workspace, path requirements, and npm manifests."""
    member = _workspace(tmp_path)

    assert stamp_version(tmp_path, version)

    assert workspace_version(tmp_path) == version
    member_text = member.read_text(encoding="utf-8")
    assert f'core = {{ path = "../core", version = "{version}"' in member_text
    assert 'serde = { version = "0.12.0" }' in member_text
    root_text = (tmp_path / "Cargo.toml").read_text(encoding="utf-8")
    assert 'sha2 = { version = "0.12.0"' in root_text
    package = json.loads((tmp_path / "finstack-quant-wasm" / "package.json").read_text(encoding="utf-8"))
    lock = json.loads((tmp_path / "finstack-quant-wasm" / "package-lock.json").read_text(encoding="utf-8"))
    assert package["version"] == version
    assert lock["version"] == version
    assert lock["packages"][""]["version"] == version


def test_matching_version_is_a_no_op(tmp_path: Path) -> None:
    """A final tag whose version is already committed writes nothing."""
    member = _workspace(tmp_path)
    before = member.read_bytes()

    assert not stamp_version(tmp_path, "0.12.0")

    assert member.read_bytes() == before


@pytest.mark.parametrize("version", ["v0.12.0", "0.12", "0.12.0+build.1", "0.12.0rc1"])
def test_rejects_non_semver(tmp_path: Path, version: str) -> None:
    """Tags, partial versions, build metadata, and PEP 440 forms are rejected."""
    _workspace(tmp_path)

    with pytest.raises(SystemExit):
        stamp_version(tmp_path, version)
