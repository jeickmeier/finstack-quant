"""CMS option pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/cms_option"))
def test_pricing_cms_option(fixture: str) -> None:
    """Run every CMS option pricing fixture through the Python bindings."""
    run_golden(fixture)
