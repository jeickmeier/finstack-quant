"""Commodity spot option pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/commodity_spot_option"))
def test_pricing_commodity_spot_option(fixture: str) -> None:
    """Run every commodity spot option pricing fixture through the Python bindings."""
    run_golden(fixture)
