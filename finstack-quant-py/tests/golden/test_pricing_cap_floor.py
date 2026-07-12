"""Cap/floor pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden


@pytest.mark.parametrize("fixture", discover_fixtures("pricing/cap_floor"))
def test_pricing_cap_floor(fixture: str) -> None:
    """Run every cap/floor pricing fixture through the Python bindings.

    Known unresolved metrics are handled at comparison level from the shared
    `known_non_executable.json` allowlist.
    """
    run_golden(fixture)
