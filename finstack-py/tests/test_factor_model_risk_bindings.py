"""Regression tests for factor-model risk binding edge-cases.

Covers:
- decompose_factor_risk raises ValueError (not abort) on zero-factor matrix.
"""

from __future__ import annotations

from datetime import date
import json

from finstack.core.market_data import DiscountCurve, MarketContext
import pytest

from finstack.portfolio import (
    compute_factor_sensitivities,
    decompose_factor_risk,
)

# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------


def _market_and_positions() -> tuple[MarketContext, str]:
    """Minimal single-position portfolio against a flat USD-OIS curve."""
    mc = MarketContext()
    mc.insert(
        DiscountCurve(
            "USD-OIS",
            date(2025, 1, 15),
            [(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (5.0, 0.75), (10.0, 0.55)],
            day_count="act_365f",
        )
    )
    bond = {
        "type": "bond",
        "spec": {
            "id": "BOND-5Y",
            "notional": {"amount": 1_000_000.0, "currency": "USD"},
            "issue_date": "2025-01-15",
            "maturity": "2030-01-15",
            "discount_curve_id": "USD-OIS",
            "cashflow_spec": {
                "Fixed": {
                    "coupon_type": "Cash",
                    "rate": 0.05,
                    "freq": {"count": 6, "unit": "months"},
                    "dc": "Thirty360",
                    "bdc": "following",
                    "calendar_id": "weekends_only",
                    "stub": "None",
                    "end_of_month": False,
                    "payment_lag_days": 0,
                }
            },
            "attributes": {},
        },
    }
    positions_json = json.dumps([{"id": "bond_5y", "instrument": bond, "weight": 1.0}])
    return mc, positions_json


# ---------------------------------------------------------------------------
# C20 — zero-factor matrix must raise ValueError, not abort
# ---------------------------------------------------------------------------


def test_decompose_factor_risk_zero_factors_raises_value_error() -> None:
    """Regression: decompose_factor_risk must raise ValueError (not abort/panic).

    when the sensitivity matrix has n_factors == 0.

    Before the fix, chunks_exact(0) panicked across the PyO3 boundary, causing
    an abort that cannot be caught by Python. After the fix, a clean ValueError
    is raised.
    """
    market, positions_json = _market_and_positions()

    # Build a zero-factor sensitivity matrix via the public API.
    zero_factor_matrix = compute_factor_sensitivities(
        positions_json,
        "[]",  # empty factor list → n_factors == 0
        market,
        "2025-01-15",
    )
    assert zero_factor_matrix.n_factors == 0, "Precondition: matrix must have n_factors == 0 for this regression test"

    # Dummy covariance for a zero-factor model.
    cov_json = json.dumps({"factor_ids": [], "n": 0, "data": []})

    with pytest.raises(ValueError, match="no factors"):
        decompose_factor_risk(zero_factor_matrix, cov_json)
