"""Convertible bond pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/convertible"))
def test_pricing_convertible(fixture: str) -> None:
    """Run every convertible bond pricing fixture through the Python bindings."""
    run_golden(fixture)
