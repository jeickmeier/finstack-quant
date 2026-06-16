"""FX swap pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/fx_swap"))
def test_pricing_fx_swap(fixture: str) -> None:
    """Run every FX swap pricing fixture through the Python bindings."""
    run_golden(fixture)
