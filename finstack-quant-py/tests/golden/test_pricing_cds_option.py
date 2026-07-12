"""CDS option pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/cds_option"))
def test_pricing_cds_option(fixture: str) -> None:
    """Run every CDS option pricing fixture through the Python bindings.

    Known unresolved metrics are handled at comparison level from the shared
    `known_non_executable.json` allowlist.
    """
    run_golden(fixture)
