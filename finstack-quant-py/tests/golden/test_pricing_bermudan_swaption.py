"""Bermudan swaption pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/bermudan_swaption"))
def test_pricing_bermudan_swaption(fixture: str) -> None:
    """Run every Bermudan swaption pricing fixture through the Python bindings."""
    run_golden(fixture)
