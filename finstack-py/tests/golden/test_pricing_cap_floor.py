"""Cap/floor pricing goldens."""

from __future__ import annotations

import pytest

from .conftest import discover_fixtures, run_golden

_USD_CAP_5Y_ATM_BLACK_FIXTURE = "pricing/cap_floor/usd_cap_5y_atm_black.json"
_USD_CAP_5Y_ATM_BLACK_XFAIL_REASON = (
    "Bloomberg cap/floor source-validation fixture is marked non-executable; "
    "preserve screen expected_outputs until quote-basis cap/floor risk support lands."
)


def _cap_floor_fixture_marks(fixture: str) -> pytest.MarkDecorator | None:
    if fixture == _USD_CAP_5Y_ATM_BLACK_FIXTURE:
        return pytest.mark.xfail(reason=_USD_CAP_5Y_ATM_BLACK_XFAIL_REASON, strict=False)
    return None


@pytest.mark.parametrize(
    "fixture",
    [
        pytest.param(fixture, marks=mark) if (mark := _cap_floor_fixture_marks(fixture)) else fixture
        for fixture in discover_fixtures("pricing/cap_floor")
    ],
)
def test_pricing_cap_floor(fixture: str) -> None:
    """Run every cap/floor pricing fixture through the Python bindings."""
    run_golden(fixture)
