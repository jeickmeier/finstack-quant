"""Inflation cap/floor pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/inflation_cap_floor"))
def test_pricing_inflation_cap_floor(fixture: str) -> None:
    """Run every inflation cap/floor pricing fixture through the Python bindings."""
    run_golden(fixture)
