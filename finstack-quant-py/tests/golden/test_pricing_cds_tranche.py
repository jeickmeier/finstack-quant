"""CDS tranche pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/cds_tranche"))
def test_pricing_cds_tranche(fixture: str) -> None:
    """Run every CDS tranche pricing fixture through the Python bindings."""
    run_golden(fixture)
