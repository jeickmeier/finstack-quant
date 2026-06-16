"""Deposit pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/deposit"))
def test_pricing_deposit(fixture: str) -> None:
    """Run every deposit pricing fixture through the Python bindings."""
    run_golden(fixture)


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/market_envelope_smoke"))
def test_pricing_market_envelope_smoke(fixture: str) -> None:
    """Run market_envelope_smoke fixtures through the Python bindings."""
    run_golden(fixture)
