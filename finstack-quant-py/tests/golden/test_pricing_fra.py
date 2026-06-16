"""FRA pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/fra"))
def test_pricing_fra(fixture: str) -> None:
    """Run every FRA pricing fixture through the Python bindings."""
    run_golden(fixture)
