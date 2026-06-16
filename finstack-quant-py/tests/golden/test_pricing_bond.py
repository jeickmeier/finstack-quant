"""Bond pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/bond"))
def test_pricing_bond(fixture: str) -> None:
    """Run every bond pricing fixture through the Python bindings."""
    run_golden(fixture)
