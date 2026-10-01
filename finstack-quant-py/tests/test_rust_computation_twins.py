"""Cross-host goldens for the Rust computations bound in both hosts by slice S21.

The expected values are asserted identically by
``finstack-quant-wasm/tests/facade/rust_computation_twins.test.mjs``.
"""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.cashflows import abs_to_smm, builder
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.portfolio import (
    Constraint,
    MetricExpr,
    Objective,
    PerPositionMetric,
    PortfolioError,
    PortfolioOptimizationResult,
    PortfolioOptimizationSpec,
    WeightingScheme,
    optimize_portfolio,
    rebalance_from_spec,
)
from finstack_quant.statements import (
    Evaluator,
    FinancialModelSpec,
    ForecastSpec,
    ModelBuilder,
    MonteCarloConfig,
    MonteCarloResults,
)
from finstack_quant.valuations.instruments import Bond, FxForward

MODEL = {
    "id": "facade-model",
    "periods": [{"id": "2025Q1", "start": "2025-01-01", "end": "2025-04-01", "is_actual": False}],
    "nodes": {"revenue": {"node_id": "revenue", "node_type": "value", "values": {"2025Q1": 100000.0}}},
    "schema_version": 1,
}


def test_abs_to_smm_is_bound_flat_and_on_builder() -> None:
    assert abs_to_smm(0.015, 1) == 0.015
    assert abs_to_smm(0.015, 11) == 0.01764705882352941
    assert builder.abs_to_smm(0.015, 11) == abs_to_smm(0.015, 11)
    with pytest.raises(ValueError, match="speed"):
        abs_to_smm(1.5, 1)


def test_dated_schedule_convention_and_missing_node_come_from_rust() -> None:
    model = FinancialModelSpec.from_json(json.dumps(MODEL))
    result = Evaluator().evaluate(model)
    assert result.to_dated_schedule(model, "revenue") == [(datetime.date(2025, 3, 31), 100000.0)]
    assert result.to_dated_schedule(model, "revenue", "start") == [(datetime.date(2025, 1, 1), 100000.0)]
    with pytest.raises(KeyError):
        result.to_dated_schedule(model, "ebitda")
    with pytest.raises(ValueError, match="start"):
        result.to_dated_schedule(model, "revenue", "middle")


def test_financial_model_content_hash_matches_wasm() -> None:
    model = FinancialModelSpec.from_json(json.dumps(MODEL))
    assert model.content_hash() == "sha256:43c8428179a2f9bb332af91d87fc7c91ea922286ec7a42b3e5ace45b85a0f17e"


def test_breach_probability_survives_from_json_with_path_data() -> None:
    builder_ = ModelBuilder("mc")
    builder_.periods("2025Q1..Q4", "2025Q1")
    model = (
        builder_
        .mixed("revenue")
        .values([("2025Q1", 100.0)])
        .forecast(ForecastSpec.normal(100.0, 10.0, 7))
        .build()
        .build()
    )
    live = Evaluator().evaluate_monte_carlo(model, MonteCarloConfig(200, 42, include_path_data=True))
    assert live.breach_probability("revenue", 410.0) == 0.22
    restored = MonteCarloResults.from_json(live.to_json())
    assert restored.breach_probability("revenue", 410.0) == 0.22
    without_paths = Evaluator().evaluate_monte_carlo(model, MonteCarloConfig(200, 42))
    assert MonteCarloResults.from_json(without_paths.to_json()).breach_probability("revenue", 410.0) is None


def test_fx_forward_from_trade_date_defaults_spot_lag_in_rust() -> None:
    args = ("EURUSD-3M", "EUR", "USD", "2025-01-15", "3M", 1_000_000.0, "USD-OIS", "EUR-OIS")
    defaulted = FxForward.from_trade_date(*args)
    explicit = FxForward.from_trade_date(*args, settlement_days=2)
    assert defaulted.maturity == datetime.date(2025, 4, 17)
    assert defaulted.to_json() == explicit.to_json()


def _book() -> dict:
    def zero(instrument_id: str, notional: float, maturity: str) -> dict:
        bond = Bond.zero_coupon(instrument_id, Money(notional, "USD"), "2024-01-15", maturity, "USD-OIS")
        return json.loads(bond.to_json())["instrument"]

    return {
        "id": "book",
        "name": "Book",
        "base_currency": "USD",
        "as_of": "2024-01-15",
        "entities": {"E": {"id": "E", "name": None}},
        "positions": [
            {
                "position_id": "A",
                "entity_id": "E",
                "instrument_id": "ZC-A",
                "instrument_spec": zero("ZC-A", 1_000_000.0, "2029-01-15"),
                "quantity": 1.0,
                "unit": "units",
            },
            {
                "position_id": "B",
                "entity_id": "E",
                "instrument_id": "ZC-B",
                "instrument_spec": zero("ZC-B", 500_000.0, "2027-01-15"),
                "quantity": 1.0,
                "unit": "units",
            },
        ],
    }


def test_rebalance_from_spec_works_on_a_result_rebuilt_from_json() -> None:
    market = MarketContext().insert(DiscountCurve.flat("USD-OIS", datetime.date(2024, 1, 15), 0.04))
    spec = (
        PortfolioOptimizationSpec
        .new(
            json.dumps(_book()),
            Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.constant(1.0))),
        )
        .with_weighting(WeightingScheme.notional_weight())
        .with_constraint(Constraint.budget(1.0))
    )
    result = PortfolioOptimizationResult.from_json(optimize_portfolio(spec, market).to_json())
    rebalanced = rebalance_from_spec(spec, result)
    quantities = [p["quantity"] for p in json.loads(rebalanced.to_spec_json())["positions"]]
    assert quantities == [0.0, 3.0]

    foreign = json.loads(result.to_json())
    foreign["implied_quantities"]["ZZZ"] = 1.0
    with pytest.raises(PortfolioError, match="ZZZ"):
        rebalance_from_spec(spec, PortfolioOptimizationResult.from_json(json.dumps(foreign)))
