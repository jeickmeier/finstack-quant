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


def test_canonical_nested_sensitivity_json_roundtrips_and_reconciles_risk() -> None:
    payload = {
        "base_currency": "EUR",
        "position_ids": ["A", "B"],
        "factor_ids": ["F"],
        "data": [[2.0], [3.0]],
    }
    matrix = SensitivityMatrix.from_json(json.dumps(payload))
    assert json.loads(matrix.to_json()) == payload
    assert (matrix.n_positions, matrix.n_factors) == (2, 1)
    assert matrix.delta(1, 0) == 3.0
    assert matrix.position_deltas(0) == [2.0]
    assert matrix.factor_deltas(0) == [2.0, 3.0]
    assert json.loads(pickle.loads(pickle.dumps(matrix)).to_json()) == payload  # noqa: S301
    decomposition = decompose_factor_risk(matrix, '{"factor_ids":["F"],"n":1,"data":[0.04]}')
    assert decomposition.total_risk == pytest.approx(1.0)
    contributions = decomposition.position_factor_contributions()
    assert sum(row["risk_contribution"] for row in contributions) == pytest.approx(1.0)
    assert decomposition.to_position_factor_dataframe()["position_id"].tolist() == ["A", "B"]
    assert decomposition.to_factor_dataframe()["factor_id"].tolist() == ["F"]


@pytest.mark.parametrize("data", [[], [[], []], [[]], [[2.0, 3.0]], [2.0], [[None]]])
def test_malformed_sensitivity_storage_is_rejected_before_access_or_risk(data: object) -> None:
    payload = {"base_currency": "USD", "position_ids": ["P"], "factor_ids": ["F"], "data": data}
    with pytest.raises(ValueError, match=r"sensitivity data|invalid type"):
        SensitivityMatrix.from_json(json.dumps(payload))


@pytest.mark.parametrize("extra", [{"n_factors": 1}, {"unexpected": True}])
def test_sensitivity_json_rejects_obsolete_and_unknown_fields(extra: dict[str, object]) -> None:
    payload = {
        "base_currency": "USD",
        "position_ids": ["P"],
        "factor_ids": ["F"],
        "data": [[2.0]],
        **extra,
    }
    with pytest.raises(ValueError, match="unknown field"):
        SensitivityMatrix.from_json(json.dumps(payload))


def test_sensitivity_json_requires_currency_and_preserves_zero_factor_rows() -> None:
    payload = {"position_ids": ["P"], "factor_ids": [], "data": [[]]}
    with pytest.raises(ValueError, match="base_currency"):
        SensitivityMatrix.from_json(json.dumps(payload))
    payload["base_currency"] = "USD"
    matrix = SensitivityMatrix.from_json(json.dumps(payload))
    assert matrix.position_deltas(0) == []
    assert json.loads(matrix.to_json()) == payload
