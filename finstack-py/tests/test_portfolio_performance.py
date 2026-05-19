"""Portfolio performance binding tests."""

from __future__ import annotations

import json

import pytest


def test_twrr_modified_dietz_binding_matches_gips_example() -> None:
    """The Python binding exposes Modified-Dietz TWRR as JSON input."""
    from finstack.portfolio import twrr_modified_dietz

    period = {
        "beginning_market_value": 10_000_000.0,
        "ending_market_value": 10_500_000.0,
        "cashflows": [
            {
                "amount": 1_000_000.0,
                "fraction_of_period_remaining": 0.60,
            }
        ],
    }

    assert twrr_modified_dietz(json.dumps(period)) == pytest.approx(
        -500_000.0 / 10_600_000.0,
        abs=1e-12,
    )


def test_twrr_linked_binding_geometrically_links_returns() -> None:
    """The Python binding exposes geometric TWRR linking."""
    from finstack.portfolio import twrr_linked

    result = json.loads(twrr_linked(json.dumps([0.05, 0.03]), 1.0))

    assert result["cumulative"] == pytest.approx(0.0815, abs=1e-12)
    assert result["annualised"] == pytest.approx(0.0815, abs=1e-12)
    assert result["num_periods"] == 2


def test_mwr_xirr_binding_solves_money_weighted_return() -> None:
    """The Python binding exposes money-weighted return from dated cashflows."""
    from finstack.portfolio import mwr_xirr

    cashflows = [
        {"date": "2025-01-01", "amount": -100.0},
        {"date": "2026-01-01", "amount": 110.0},
    ]

    assert mwr_xirr(json.dumps(cashflows)) == pytest.approx(0.10, abs=1e-6)
