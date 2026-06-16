"""Bond future pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/bond_future"))
def test_pricing_bond_future(fixture: str) -> None:
    """Run every bond future pricing fixture through the Python bindings."""
    run_golden(fixture)
