"""FX future pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/fx_future"))
def test_pricing_fx_future(fixture: str) -> None:
    """Run every FX future pricing fixture through the Python bindings."""
    run_golden(fixture)
