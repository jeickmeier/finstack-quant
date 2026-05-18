"""SABR smile goldens (closed-form vol generation, not instrument pricing)."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("volatility/sabr"))
def test_volatility_sabr_smile(fixture: str) -> None:
    """Run every SABR smile fixture through the Python bindings."""
    run_golden(fixture)
