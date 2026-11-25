"""Equity total return swap pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/equity_total_return_swap"))
def test_pricing_equity_total_return_swap(fixture: str) -> None:
    """Run every equity total return swap pricing fixture through the Python bindings."""
    run_golden(fixture)
