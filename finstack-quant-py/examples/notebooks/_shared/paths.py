"""Stable repository paths for example notebooks."""

from __future__ import annotations

from pathlib import Path
from typing import Final

NOTEBOOKS_ROOT: Final = Path(__file__).resolve().parents[1]
PYTHON_PACKAGE_ROOT: Final = NOTEBOOKS_ROOT.parents[1]
REPOSITORY_ROOT: Final = PYTHON_PACKAGE_ROOT.parent
FIXTURE_ROOT: Final = Path(__file__).resolve().parent / "data"


def fixture_path(name: str, *, version: str = "v1") -> Path:
    """Return a versioned shared-fixture path."""
    if not name or Path(name).name != name:
        msg = f"Fixture name must be a plain file name, got {name!r}"
        raise ValueError(msg)
    path = FIXTURE_ROOT / version / name
    if not path.is_file():
        msg = f"Unknown notebook fixture: {version}/{name}"
        raise FileNotFoundError(msg)
    return path
