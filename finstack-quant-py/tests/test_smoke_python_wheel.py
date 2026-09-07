"""Keep the packaged-wheel smoke contract executable in ordinary Python CI."""

from __future__ import annotations

import importlib.util
from pathlib import Path

_SCRIPT_PATH = Path(__file__).resolve().parents[2] / "scripts" / "smoke_python_wheel.py"


def test_packaged_wheel_smoke_uses_public_pricing_surface() -> None:
    """The release smoke script must run against the live extension."""
    spec = importlib.util.spec_from_file_location("smoke_python_wheel", _SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.main()
