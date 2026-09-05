"""Regression tests for factor-model risk binding edge-cases.

Covers:
- decompose_factor_risk returns a clean zero-risk decomposition for zero factors.
"""

from __future__ import annotations

from datetime import date
import json
import pickle

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.portfolio import (
    SensitivityMatrix,
    compute_factor_sensitivities,
    compute_pnl_profiles,
    decompose_factor_risk,
)

# Shared helpers


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
            "notional": {"amount": "1000000", "currency": "USD"},
            "issue_date": "2025-01-15",
            "maturity": "2030-01-15",
            "discount_curve_id": "USD-OIS",
            "cashflow_spec": {
                "fixed": {
                    "coupon_type": "cash",
                    "rate": "0.05",
                    "frequency": {"count": 6, "unit": "months"},
                    "day_count": "30_360",
                    "business_day_convention": "following",
                    "calendar_id": "weekends_only",
                    "stub": "none",
                    "end_of_month": False,
                    "payment_lag_days": 0,
                }
            },
            "attributes": {},
        },
    }
    positions_json = json.dumps([
        {
            "id": "bond_5y",
            "instrument": {
                "schema": "finstack_quant.instrument/1",
                "instrument": bond,
            },
            "weight": 1.0,
        }
    ])
    return mc, positions_json


# C20 — zero-factor matrix must not abort


def test_decompose_factor_risk_zero_factors_returns_zero_risk() -> None:
    """Regression: decompose_factor_risk must handle zero factors cleanly.

    when the sensitivity matrix has n_factors == 0.

    Before the fix, chunks_exact(0) panicked across the PyO3 boundary. The
    canonical Rust/WASM behavior is now an empty zero-risk decomposition.
    """
    market, positions_json = _market_and_positions()

    # Build a zero-factor sensitivity matrix via the public API.
    zero_factor_matrix = compute_factor_sensitivities(
        positions_json,
        "[]",  # empty factor list → n_factors == 0
        market,
        "2025-01-15",
        "USD",
    )
    assert zero_factor_matrix.n_factors == 0, "Precondition: matrix must have n_factors == 0 for this regression test"

    # Dummy covariance for a zero-factor model.
    cov_json = json.dumps({"factor_ids": [], "n": 0, "data": []})

    decomposition = decompose_factor_risk(zero_factor_matrix, cov_json)

    assert decomposition.total_risk == 0.0
    assert decomposition.residual_risk == 0.0
    assert decomposition.factor_contributions() == []
    assert decomposition.position_factor_contributions() == []


def test_sensitivity_reporting_currency_is_required_and_survives_round_trip() -> None:
    market = MarketContext()
    with pytest.raises(TypeError):
        compute_factor_sensitivities("[]", "[]", market, "2025-01-15")
    matrix = compute_factor_sensitivities("[]", "[]", market, "2025-01-15", "EUR")
    assert matrix.base_currency == "EUR"
    assert json.loads(matrix.to_json())["base_currency"] == "EUR"
    assert SensitivityMatrix.from_json(matrix.to_json()).base_currency == "EUR"
    assert pickle.loads(pickle.dumps(matrix)).base_currency == "EUR"  # noqa: S301 - trusted local roundtrip
    assert compute_pnl_profiles("[]", "[]", market, "2025-01-15", "EUR") == []
    with pytest.raises(ValueError, match="Matching variant not found"):
        compute_factor_sensitivities("[]", "[]", market, "2025-01-15", "INVALID")
