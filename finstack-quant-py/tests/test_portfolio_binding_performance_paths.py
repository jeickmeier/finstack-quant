"""Regression tests for portfolio binding performance paths."""

from __future__ import annotations

from datetime import date
import json

import numpy as np
import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.portfolio import (
    Portfolio,
    PortfolioValuation,
    build_stress_attribution,
    historical_var_decomposition,
    historical_var_decomposition_typed,
    parametric_es_decomposition,
    parametric_var_decomposition,
    parametric_var_decomposition_typed,
    value_portfolio,
    value_portfolio_typed,
)

AS_OF = "2025-01-15"


def _portfolio_json() -> str:
    return json.dumps({
        "id": "PERF-PATHS",
        "as_of": AS_OF,
        "base_ccy": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            {
                "position_id": "USD-POS",
                "entity_id": "FUND",
                "instrument_id": "USD-DEP",
                "instrument_spec": {
                    "type": "deposit",
                    "spec": {
                        "id": "USD-DEP",
                        "notional": {"amount": 1_000_000.0, "currency": "USD"},
                        "start_date": AS_OF,
                        "maturity": "2025-07-15",
                        "day_count": "Act360",
                        "quote_rate": 0.04,
                        "discount_curve_id": "USD-OIS",
                        "attributes": {},
                    },
                },
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    })


def _market() -> MarketContext:
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            date.fromisoformat(AS_OF),
            [(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)],
            day_count="act_365f",
        )
    )
    return market


def test_value_portfolio_typed_matches_legacy_json_result() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market = _market()

    typed = value_portfolio_typed(portfolio, market)
    legacy = PortfolioValuation.from_json(value_portfolio(portfolio, market))

    assert isinstance(typed, PortfolioValuation)
    assert typed.total_value == pytest.approx(legacy.total_value)
    assert typed.base_ccy == legacy.base_ccy
    assert typed.as_of == legacy.as_of
    assert len(typed) == len(legacy)


def test_value_portfolio_metrics_select_pv_only_or_explicit_risk() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market = _market()

    pv_only = json.loads(value_portfolio_typed(portfolio, market, metrics=[]).to_json())
    dv01_only = json.loads(
        value_portfolio_typed(portfolio, market, metrics=["dv01"]).to_json()
    )

    pv_measures = pv_only["position_values"]["USD-POS"]["valuation_result"]["measures"]
    dv01_measures = dv01_only["position_values"]["USD-POS"]["valuation_result"][
        "measures"
    ]
    assert pv_measures == {}
    assert "dv01" in dv01_measures
    assert "theta" not in dv01_measures


@pytest.mark.parametrize(
    "covariance",
    [
        np.asarray([[0.04, 0.01], [0.01, 0.09]], dtype=np.float64),
        np.asfortranarray([[0.04, 0.01], [0.01, 0.09]], dtype=np.float64),
    ],
)
def test_numpy_covariance_matches_nested_lists(covariance: np.ndarray) -> None:
    position_ids = ["A", "B"]
    weights = [0.4, 0.6]
    nested = covariance.tolist()

    legacy_numpy = parametric_var_decomposition(position_ids, weights, covariance)
    legacy_list = parametric_var_decomposition(position_ids, weights, nested)
    typed_numpy = parametric_var_decomposition_typed(position_ids, weights, covariance)
    typed_list = parametric_var_decomposition_typed(position_ids, weights, nested)

    assert legacy_numpy == legacy_list
    assert typed_numpy.to_json() == typed_list.to_json()
    assert parametric_es_decomposition(position_ids, weights, covariance) == (
        parametric_es_decomposition(position_ids, weights, nested)
    )


def test_numpy_position_pnls_match_nested_lists() -> None:
    position_ids = ["A", "B"]
    position_pnls = np.asarray(
        [
            [-8.0, -2.0] + [0.5] * 38,
            [-2.0, -4.0] + [0.5] * 38,
        ],
        dtype=np.float64,
    )
    nested = position_pnls.tolist()

    legacy_numpy = historical_var_decomposition(position_ids, position_pnls)
    legacy_list = historical_var_decomposition(position_ids, nested)
    typed_numpy = historical_var_decomposition_typed(position_ids, position_pnls)
    typed_list = historical_var_decomposition_typed(position_ids, nested)
    stress_numpy = build_stress_attribution(position_ids, position_pnls)
    stress_list = build_stress_attribution(position_ids, nested)

    assert legacy_numpy == legacy_list
    assert typed_numpy.to_json() == typed_list.to_json()
    assert stress_numpy.to_json() == stress_list.to_json()


def test_numpy_inputs_reject_wrong_shapes() -> None:
    with pytest.raises(ValueError, match="covariance must have 2 rows"):
        parametric_var_decomposition(
            ["A", "B"],
            [0.4, 0.6],
            np.ones((3, 3), dtype=np.float64),
        )

    with pytest.raises(ValueError, match="position_pnls must have 2 rows"):
        historical_var_decomposition(
            ["A", "B"],
            np.ones((3, 10), dtype=np.float64),
        )
