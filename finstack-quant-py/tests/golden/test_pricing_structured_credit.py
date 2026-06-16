"""Structured credit pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/structured_credit"))
def test_pricing_structured_credit(fixture: str) -> None:
    """Run every structured credit pricing fixture through the Python bindings."""
    run_golden(fixture)
