"""Inflation-linked bond pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/inflation_linked_bond"))
def test_pricing_inflation_linked_bond(fixture: str) -> None:
    """Run every inflation-linked bond pricing fixture through the Python bindings."""
    run_golden(fixture)
