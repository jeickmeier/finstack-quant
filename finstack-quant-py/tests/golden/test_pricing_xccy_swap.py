"""Cross-currency swap pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/xccy_swap"))
def test_pricing_xccy_swap(fixture: str) -> None:
    """Run every cross-currency swap pricing fixture through the Python bindings."""
    run_golden(fixture)
