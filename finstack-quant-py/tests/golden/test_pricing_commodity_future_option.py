"""Commodity future option pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/commodity_future_option"))
def test_pricing_commodity_future_option(fixture: str) -> None:
    """Run every commodity future option pricing fixture through the Python bindings."""
    run_golden(fixture)
