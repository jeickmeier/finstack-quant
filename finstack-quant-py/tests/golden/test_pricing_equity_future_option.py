"""Equity future option pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/equity_future_option"))
def test_pricing_equity_future_option(fixture: str) -> None:
    """Run every equity future option pricing fixture through the Python bindings."""
    run_golden(fixture)
