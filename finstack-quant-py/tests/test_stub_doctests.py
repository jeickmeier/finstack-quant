"""Execute public stub and module doctest examples against the live package."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

# Run by `mise run python-test --doctest`; every other python-test mode excludes it.
pytestmark = pytest.mark.doctest

_SCRIPT_PATH = Path(__file__).resolve().parents[2] / "scripts" / "run_python_stub_doctests.py"
_SPEC = importlib.util.spec_from_file_location("run_python_stub_doctests", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)

_API_PATHS = _MODULE.api_paths()


@pytest.mark.parametrize(
    "path",
    _API_PATHS,
    ids=[str(path.relative_to(_MODULE.PACKAGE_ROOT)) for path in _API_PATHS],
)
def test_stub_doctests(path: Path) -> None:
    """Examples in one stub or pure-Python module execute against the extension."""
    failures = _MODULE.run_file(path)
    assert failures == [], "\n".join(failures)
