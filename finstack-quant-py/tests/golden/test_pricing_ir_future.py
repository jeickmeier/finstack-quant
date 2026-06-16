"""Interest-rate future pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/ir_future"))
def test_pricing_ir_future(fixture: str) -> None:
    """Run every IR future pricing fixture through the Python bindings."""
    run_golden(fixture)
