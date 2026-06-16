"""Term-loan pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/term_loan"))
def test_pricing_term_loan(fixture: str) -> None:
    """Run every term-loan pricing fixture through the Python bindings."""
    run_golden(fixture)
