"""Volatility index future pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/volatility_index_future"))
def test_pricing_volatility_index_future(fixture: str) -> None:
    """Run every volatility index future pricing fixture through the Python bindings."""
    run_golden(fixture)
