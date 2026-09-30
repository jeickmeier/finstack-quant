"""Regression tests for portfolio binding performance paths."""

from __future__ import annotations

from datetime import date
import json

import numpy as np
import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.models.factor.risk import (
    build_stress_attribution,
    historical_var_decomposition,
    parametric_es_decomposition,
    parametric_var_decomposition,
)
from finstack_quant.portfolio import (
    Portfolio,
    PortfolioValuation,
    scenario_pnl,
    scenario_pnl_batch,
    scenario_pnl_batch_json,
    value_portfolio,
)
from finstack_quant.scenarios import ScenarioSpec

AS_OF = "2025-01-15"


def _portfolio_json() -> str:
    return json.dumps({
        "id": "PERF-PATHS",
        "as_of": AS_OF,
        "base_currency": "USD",
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
                        "notional": {"amount": "1000000", "currency": "USD"},
                        "start_date": AS_OF,
                        "maturity": "2025-07-15",
                        "day_count": "act_360",
                        "fixed_rate": "0.04",
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


def _scenario_batch_json() -> str:
    scenarios = []
    for scenario_id, bp in (("up_10bp", 10.0), ("down_15bp", -15.0)):
        operations = [
            {
                "kind": "curve_parallel_bp",
                "curve_kind": "discount",
                "curve_id": "USD-OIS",
                "discount_curve_id": None,
                "bp": bp,
            }
        ]
        spec = ScenarioSpec.from_json(json.dumps({"id": scenario_id, "operations": operations}))
        scenarios.append(json.loads(spec.to_json()))
    return json.dumps(scenarios)


def test_value_portfolio_returns_typed_roundtrippable_result() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market = _market()

    valuation = value_portfolio(portfolio, market)
    round_tripped = PortfolioValuation.from_json(valuation.to_json())

    assert isinstance(valuation, PortfolioValuation)
    assert valuation.total_value == pytest.approx(round_tripped.total_value)
    assert valuation.base_currency == round_tripped.base_currency
    assert valuation.as_of == round_tripped.as_of
    assert len(valuation) == len(round_tripped)


def test_value_portfolio_metrics_select_pv_only_or_explicit_risk() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market = _market()

    pv_only = json.loads(value_portfolio(portfolio, market, metrics=[]).to_json())
    dv01_only = json.loads(value_portfolio(portfolio, market, metrics=["dv01"]).to_json())

    pv_measures = pv_only["position_values"]["USD-POS"]["valuation_result"]["measures"]
    dv01_measures = dv01_only["position_values"]["USD-POS"]["valuation_result"]["measures"]
    assert pv_measures == {}
    assert "dv01" in dv01_measures
    assert "theta" not in dv01_measures


def test_scenario_pnl_batch_matches_standalone_and_accepts_json_or_typed_inputs() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market = _market()
    scenarios_json = _scenario_batch_json()
    scenarios = json.loads(scenarios_json)

    # `scenario_pnl_batch_json` is the JSON-wire twin, so the typed
    # standalone results are compared through their canonical JSON.
    batch = json.loads(scenario_pnl_batch_json(portfolio, scenarios_json, market))
    expected = []
    for scenario in scenarios:
        pnl, report = scenario_pnl(portfolio, json.dumps(scenario), market)
        expected.append({
            "scenario_id": scenario["id"],
            "pnl": json.loads(pnl.to_json()),
            "report": json.loads(report.to_json()),
        })

    assert batch == expected
    assert [item["scenario_id"] for item in batch] == ["up_10bp", "down_15bp"]
    assert json.loads(scenario_pnl_batch_json(_portfolio_json(), scenarios_json, market.to_json())) == batch
    assert json.loads(scenario_pnl_batch_json(portfolio, "[]", market)) == []
    # The typed entry point returns the same payloads as wrapper objects.
    typed = scenario_pnl_batch(portfolio, scenarios_json, market)
    assert [json.loads(item.to_json()) for item in typed] == batch
    assert scenario_pnl_batch(portfolio, "[]", market) == []
    with pytest.raises(ValueError, match="invalid type"):
        scenario_pnl_batch_json(portfolio, "{}", market)


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

    numpy_result = parametric_var_decomposition(position_ids, weights, covariance)
    list_result = parametric_var_decomposition(position_ids, weights, nested)

    assert numpy_result.to_json() == list_result.to_json()
    assert parametric_es_decomposition(position_ids, weights, covariance).to_json() == (
        parametric_es_decomposition(position_ids, weights, nested).to_json()
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

    numpy_result = historical_var_decomposition(position_ids, position_pnls)
    list_result = historical_var_decomposition(position_ids, nested)
    stress_numpy = build_stress_attribution(position_ids, position_pnls)
    stress_list = build_stress_attribution(position_ids, nested)

    assert numpy_result.to_json() == list_result.to_json()
    assert stress_numpy.to_json() == stress_list.to_json()


def test_numpy_inputs_reject_wrong_shapes() -> None:
    with pytest.raises(ValueError, match="covariance must have 2 rows"):
        parametric_var_decomposition(
            ["A", "B"],
            [0.4, 0.6],
            np.ones((3, 3), dtype=np.float64),
        )

    with pytest.raises(ValueError, match="position_pnls must have 2 rows, got 3"):
        historical_var_decomposition(
            ["A", "B"],
            np.ones((3, 10), dtype=np.float64),
        )
