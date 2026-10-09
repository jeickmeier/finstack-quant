"""Tests for the notebook import-path installer."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

_SCRIPT_PATH = Path(__file__).parents[1] / "install_notebook_path.py"
_SPEC = importlib.util.spec_from_file_location("install_notebook_path", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)
install_notebook_path = _MODULE.install_notebook_path
notebooks_root = _MODULE.notebooks_root


def test_notebooks_root_contains_shared_package() -> None:
    """The installed path must be the tree that owns ``_shared``."""
    root = notebooks_root()

    assert (root / "_shared" / "__init__.py").is_file()
    assert root.name == "notebooks"


def test_install_writes_absolute_notebooks_root(tmp_path: Path) -> None:
    """A site-packages path file should point at the notebooks tree."""
    site_packages = tmp_path / "site-packages"

    written = install_notebook_path(site_packages)

    assert written == site_packages / "finstack_notebooks.pth"
    assert written.read_text(encoding="utf-8") == f"{notebooks_root()}\n"
    assert Path(written.read_text(encoding="utf-8").strip()).is_absolute()


def test_install_keeps_an_unchanged_path_file(tmp_path: Path) -> None:
    """Repeating the install should leave an already-correct path file in place."""
    site_packages = tmp_path / "site-packages"
    written = install_notebook_path(site_packages)
    written.write_text(written.read_text(encoding="utf-8"), encoding="utf-8")
    before = written.read_bytes()

    again = install_notebook_path(site_packages)

    assert again.read_bytes() == before
