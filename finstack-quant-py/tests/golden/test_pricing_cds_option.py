"""CDS option pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures_with_marks, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures_with_marks("pricing/cds_option"))
def test_pricing_cds_option(fixture: str) -> None:
    """Run every CDS option pricing fixture through the Python bindings.

    Known non-executable Bloomberg fixtures are marked xfail from the shared
    `known_non_executable.json` allowlist (see conftest).
    """
    run_golden(fixture)
