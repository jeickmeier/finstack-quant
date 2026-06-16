"""IRS pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/irs"))
def test_pricing_irs(fixture: str) -> None:
    """Run every IRS pricing fixture through the Python bindings."""
    run_golden(fixture)
