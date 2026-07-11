"""FX forward pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/fx_forward"))
def test_pricing_fx_forward(fixture: str) -> None:
    """Run every FX forward pricing fixture through the Python bindings."""
    run_golden(fixture)
