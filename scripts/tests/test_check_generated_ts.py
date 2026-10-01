"""Tests for generated TypeScript inventory comparison."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import shutil
import subprocess

_SCRIPT_PATH = Path(__file__).parents[1] / "check_generated_ts.py"
_SPEC = importlib.util.spec_from_file_location("check_generated_ts", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)


def test_inventory_drift_detects_case_only_rename() -> None:
    """Linux TypeScript resolution fails when git casing disagrees with imports."""
    expected = {"CDSTrancheQuote.ts": b"export type CdsTrancheQuote = {};\n"}
    actual = {"CdsTrancheQuote.ts": b"export type CdsTrancheQuote = {};\n"}

    missing, extra, changed = _MODULE.inventory_drift(expected, actual)

    assert missing == ["CDSTrancheQuote.ts"]
    assert extra == ["CdsTrancheQuote.ts"]
    assert changed == []


def test_worktree_inventory_handles_added_and_deleted_declarations(tmp_path: Path) -> None:
    """An unstaged generation update is checked against the working files."""
    git = shutil.which("git")
    assert git is not None
    subprocess.run([git, "init", "--quiet", str(tmp_path)], check=True)  # noqa: S603
    directory = tmp_path / "generated"
    directory.mkdir()
    kept = directory / "Kept.ts"
    removed = directory / "Removed.ts"
    kept.write_bytes(b"export type Kept = number;\n")
    removed.write_bytes(b"export type Removed = number;\n")
    subprocess.run([git, "-C", str(tmp_path), "add", "generated"], check=True)  # noqa: S603
    removed.unlink()
    added = directory / "Added.ts"
    added.write_bytes(b"export type Added = string;\n")

    expected = _MODULE.files_by_git_path(directory, root=tmp_path)

    assert expected == {"Added.ts": added.read_bytes(), "Kept.ts": kept.read_bytes()}
    # If the exporter still requires a deleted declaration, it remains drift.
    regenerated = {**expected, "Removed.ts": b"export type Removed = number;\n"}
    assert _MODULE.inventory_drift(expected, regenerated) == ([], ["Removed.ts"], [])
