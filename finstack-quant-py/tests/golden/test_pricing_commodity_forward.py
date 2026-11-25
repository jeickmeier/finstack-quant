"""Commodity forward pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/commodity_forward"))
def test_pricing_commodity_forward(fixture: str) -> None:
    """Run every commodity forward pricing fixture through the Python bindings."""
    run_golden(fixture)
