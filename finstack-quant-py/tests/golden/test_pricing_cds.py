"""CDS pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/cds"))
def test_pricing_cds(fixture: str) -> None:
    """Run every CDS pricing fixture through the Python bindings."""
    run_golden(fixture)
