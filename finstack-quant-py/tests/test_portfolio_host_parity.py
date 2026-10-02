"""Cross-host goldens for the portfolio entry points bound in both hosts.

``finstack-quant/portfolio/tests/fixtures/host_parity.json`` holds one set of
inputs and the expected outputs. This module asserts the Python results; the
WASM facade test ``tests/facade/portfolio_parity.test.mjs`` asserts the same
file, so both hosts are pinned to the same numbers.

Regenerate the fixture (inputs and expected outputs) from the Python host with
``UPDATE_HOST_PARITY=1 uv run --no-sync pytest finstack-quant-py/tests/test_portfolio_host_parity.py``.
"""

from __future__ import annotations

from datetime import date
import json
import math
import os
from pathlib import Path
import re
from typing import Any

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.models.factor.credit import CreditFactorModel
from finstack_quant.models.factor.risk import RiskDecomposition
from finstack_quant.portfolio import (
    Portfolio,
    PortfolioError,
    aggregate_metrics,
    allocate_weights,
    attribute_portfolio_pnl,
    build_credit_vol_report,
    factor_stress,
    position_what_if,
    scenario_pnl_batch_json,
    validate_allocation_json,
    value_portfolio,
)

FIXTURE = Path(__file__).parents[2] / "finstack-quant/portfolio/tests/fixtures/host_parity.json"
AS_OF_T0 = "2025-01-15"
AS_OF_T1 = "2025-01-16"
_DECIMAL = re.compile(r"-?\d+(\.\d+)?")


def _position(index: int) -> dict[str, Any]:
    instrument_id = f"DEP-{index}"
    return {
        "position_id": f"POS-{index}",
        "entity_id": "FUND",
        "instrument_id": instrument_id,
        "instrument_spec": {
            "type": "deposit",
            "spec": {
                "id": instrument_id,
                "notional": {"amount": str(1_000_000 * (index + 1)), "currency": "USD"},
                "start_date": AS_OF_T0,
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


def _market_json(as_of: str, rate: float) -> dict[str, Any]:
    knots = [(year, math.exp(-rate * year)) for year in (0.0, 0.5, 1.0, 2.0)]
    market = MarketContext().insert(DiscountCurve("USD-OIS", date.fromisoformat(as_of), knots))
    return json.loads(market.to_json())


def _inputs() -> dict[str, Any]:
    return {
        "as_of_t0": AS_OF_T0,
        "as_of_t1": AS_OF_T1,
        "portfolio_spec": {
            "id": "HOST-PARITY",
            "as_of": AS_OF_T0,
            "base_currency": "USD",
            "entities": {"FUND": {"id": "FUND"}},
            "positions": [_position(0), _position(1)],
        },
        "market_t0": _market_json(AS_OF_T0, 0.04),
        "market_t1": _market_json(AS_OF_T1, 0.041),
        "attribution_method": "parallel",
        "reconciliation_tolerance": 1.0e-6,
        "factor_model_config": {
            "factors": [
                {
                    "id": "usd_rates",
                    "factor_type": "rates",
                    "market_mapping": {"curve_parallel": {"curve_ids": ["USD-OIS"], "units": "rate_bp"}},
                    "description": "Parallel USD rates shift",
                }
            ],
            "covariance": {"factor_ids": ["usd_rates"], "n": 1, "data": [0.0001]},
            "matching": {
                "mapping_table": [{"dependency_filter": {}, "attribute_filter": {}, "factor_id": "usd_rates"}]
            },
            "pricing_mode": "full_repricing",
            "risk_measure": "variance",
        },
        "stresses": [["usd_rates", 1.0]],
        "changes": [{"kind": "remove", "position_id": "POS-1"}],
        "scenarios": [
            {
                "id": scenario_id,
                "operations": [
                    {
                        "kind": "curve_parallel_bp",
                        "curve_kind": "discount",
                        "curve_id": "USD-OIS",
                        "discount_curve_id": None,
                        "bp": bp,
                    }
                ],
            }
            for scenario_id, bp in (("up_10bp", 10.0), ("down_15bp", -15.0))
        ],
        "allocation_spec": {
            "scheme": "inverse_volatility",
            "total_capital": 1_000_000.0,
            "strategies": [
                {"id": "S1", "fixed_weight": None, "returns": [0.01, -0.02, 0.015, 0.005], "risk_budget": None},
                {"id": "S2", "fixed_weight": None, "returns": [0.002, -0.001, 0.003, 0.001], "risk_budget": None},
            ],
            "covariance": None,
        },
        "credit_model": {
            "schema": "finstack_quant.credit_factor_model/1",
            "as_of": "2024-03-29",
            "calibration_window": {"start": "2022-03-29", "end": "2024-03-29"},
            "policy": "globally_off",
            "generic_factor": {"name": "CDX IG", "series_id": "cdx.ig.5y"},
            "hierarchy": {"levels": ["rating", "region"]},
            "panel_frequency": "monthly",
            "use_returns_or_levels": "returns",
            "bucket_weighting": "equal",
            "config": {
                "factors": [],
                "covariance": {"n": 0, "factor_ids": [], "data": []},
                "matching": {"mapping_table": []},
                "pricing_mode": "delta_based",
            },
            "issuer_betas": [],
            "anchor_state": {"pc": 0.0, "by_level": []},
            "static_correlation": {"factor_ids": [], "data": []},
            "vol_state": {"factors": {}, "idiosyncratic": {}},
            "factor_histories": None,
            "diagnostics": {
                "mode_counts": {},
                "bucket_sizes_per_level": [],
                "fold_ups": [],
                "r_squared_histogram": None,
                "tag_taxonomy": {},
            },
        },
        "credit_decomposition": {
            "total_risk": 1.0,
            "measure": "variance",
            "factor_contributions": [
                {"factor_id": "credit::generic", "absolute_risk": 0.10, "relative_risk": 0.10, "marginal_risk": 0.0},
                {
                    "factor_id": "credit::level0::rating::IG",
                    "absolute_risk": 0.20,
                    "relative_risk": 0.20,
                    "marginal_risk": 0.0,
                },
                {
                    "factor_id": "credit::level1::rating.region::IG.EU",
                    "absolute_risk": 0.30,
                    "relative_risk": 0.30,
                    "marginal_risk": 0.0,
                },
            ],
            "residual_risk": 0.40,
            "position_factor_contributions": [
                {"position_id": "POS1", "factor_id": "credit::generic", "risk_contribution": 0.10},
                {"position_id": "POS1", "factor_id": "credit::level0::rating::IG", "risk_contribution": 0.20},
            ],
            "position_residual_contributions": [],
        },
    }


def _compute(inputs: dict[str, Any]) -> dict[str, Any]:
    """Every host-shared portfolio computation, as JSON-shaped values."""
    portfolio = Portfolio.from_spec(json.dumps(inputs["portfolio_spec"]))
    market_t0 = MarketContext.from_json(json.dumps(inputs["market_t0"]))
    market_t1 = MarketContext.from_json(json.dumps(inputs["market_t1"]))
    config = json.dumps(inputs["factor_model_config"])
    allocation = json.dumps(inputs["allocation_spec"])

    attribution = attribute_portfolio_pnl(
        portfolio, market_t0, market_t1, inputs["as_of_t0"], inputs["as_of_t1"], inputs["attribution_method"]
    )
    stress = factor_stress(
        portfolio, market_t0, config, inputs["as_of_t0"], [tuple(pair) for pair in inputs["stresses"]]
    )
    what_if = position_what_if(portfolio, market_t0, config, inputs["as_of_t0"], inputs["changes"])
    report = build_credit_vol_report(
        RiskDecomposition.from_json(json.dumps(inputs["credit_decomposition"])),
        CreditFactorModel.from_json(json.dumps(inputs["credit_model"])),
        by_position=True,
    )
    valuation = value_portfolio(portfolio, market_t0, metrics=["dv01"])
    metrics = aggregate_metrics(valuation, "USD", market_t0, inputs["as_of_t0"])
    position_metrics = metrics.get_position_metrics("POS-1")
    return {
        "allocate_weights": json.loads(allocate_weights(allocation).to_json()),
        "validate_allocation_json": json.loads(validate_allocation_json(allocation)),
        "scenario_pnl_batch": json.loads(
            scenario_pnl_batch_json(portfolio, json.dumps(inputs["scenarios"]), market_t0)
        ),
        "attribute_portfolio_pnl": json.loads(attribution.to_json()),
        "attribution_explain": attribution.explain(),
        "attribution_reconciliation_check": json.loads(
            attribution.reconciliation_check(inputs["reconciliation_tolerance"]).to_json()
        ),
        "factor_stress": json.loads(stress.to_json()),
        "position_what_if": json.loads(what_if.to_json()),
        "build_credit_vol_report": json.loads(report.to_json()),
        "valuation_position_value_base": valuation.get_position_value("POS-1").value_base.amount,
        "valuation_entity_value": valuation.get_entity_value("FUND").amount,
        "metrics_total_dv01": metrics.get_total("dv01"),
        "metrics_position_dv01": None if position_metrics is None else position_metrics["metrics"]["dv01"],
        "builder_spec": json.loads(_built_portfolio(inputs).to_json()),
    }


def _built_portfolio(inputs: dict[str, Any]) -> Portfolio:
    """The fixture portfolio assembled through the fluent builder."""
    spec = inputs["portfolio_spec"]
    builder = Portfolio.builder(spec["id"], spec["base_currency"], spec["as_of"]).name("Desk book").entity("FUND")
    for position in spec["positions"]:
        builder = builder.position(
            position["position_id"],
            json.dumps({"schema": "finstack_quant.instrument/1", "instrument": position["instrument_spec"]}),
            position["quantity"],
            entity_id=position["entity_id"],
            unit=position["unit"],
        )
    return builder.tag("desk", "rates").meta("owner", {"team": "rates"}).build()


def _assert_close(actual: Any, expected: Any, path: str = "$") -> None:
    """Structural equality with a tolerance for native-vs-wasm32 float drift."""
    if isinstance(expected, str) and _DECIMAL.fullmatch(expected) and isinstance(actual, str):
        # Money amounts are decimal strings; compare them as numbers.
        assert float(actual) == pytest.approx(float(expected), rel=1e-9, abs=1e-9), path
    elif isinstance(expected, bool) or expected is None or isinstance(expected, str):
        assert actual == expected, path
    elif isinstance(expected, (int, float)):
        assert isinstance(actual, (int, float)), path
        assert actual == pytest.approx(expected, rel=1e-9, abs=1e-9), path
    elif isinstance(expected, list):
        assert isinstance(actual, list), path
        assert len(actual) == len(expected), path
        for index, (left, right) in enumerate(zip(actual, expected, strict=True)):
            _assert_close(left, right, f"{path}[{index}]")
    else:
        assert isinstance(actual, dict), path
        assert sorted(actual) == sorted(expected), path
        for key in expected:
            _assert_close(actual[key], expected[key], f"{path}.{key}")


def test_python_matches_the_shared_host_parity_fixture() -> None:
    if os.environ.get("UPDATE_HOST_PARITY"):
        inputs = _inputs()
        FIXTURE.parent.mkdir(parents=True, exist_ok=True)
        FIXTURE.write_text(json.dumps({"inputs": inputs, "expected": _compute(inputs)}, indent=2) + "\n")
    fixture = json.loads(FIXTURE.read_text())
    _assert_close(_compute(fixture["inputs"]), fixture["expected"])


def test_require_metric_and_what_if_errors_come_from_rust() -> None:
    fixture = json.loads(FIXTURE.read_text())["inputs"]
    portfolio = Portfolio.from_spec(json.dumps(fixture["portfolio_spec"]))
    market = MarketContext.from_json(json.dumps(fixture["market_t0"]))
    with pytest.raises(PortfolioError):
        position_what_if(
            portfolio,
            market,
            json.dumps(fixture["factor_model_config"]),
            fixture["as_of_t0"],
            [{"kind": "remove", "position_id": "NOPE"}],
        )
