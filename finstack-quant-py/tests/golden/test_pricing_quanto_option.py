"""Quanto option pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/quanto_option"))
def test_pricing_quanto_option(fixture: str) -> None:
    """Run every quanto option pricing fixture through the Python bindings."""
    run_golden(fixture)
