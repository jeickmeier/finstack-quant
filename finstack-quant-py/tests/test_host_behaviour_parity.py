"""Behaviour the Python host shares with WASM.

Covers the fluent mutators (``ScheduleBuilder``, ``MarketContext``) and the
portfolio members that compute or validate in Rust.

The same cases are asserted in
``finstack-quant-wasm/tests/facade/host_behaviour_parity.test.mjs``.
"""

from __future__ import annotations

import datetime as dt
import json

import pytest

from finstack_quant.core.dates import Schedule
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.portfolio import (
    Constraint,
    Portfolio,
    PortfolioMetrics,
    PositionFilter,
    SensitivityMatrix,
)

# Shared with host_behaviour_parity.test.mjs.
MATRIX = {
    "base_currency": "USD",
    "position_ids": ["A", "B"],
    "factor_ids": ["F1", "F2", "F3"],
    "data": [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
}


def test_schedule_builder_setters_update_the_builder_in_place_and_return_it() -> None:
    builder = Schedule.builder("2025-01-15", "2025-07-15")
    assert builder.frequency("3M") is builder
    # Statement style (no chaining) applies the setting, as in WASM.
    builder.end_of_month(True)
    assert builder.stub_rule("short_front").adjust_with("following", "nyse") is builder
    assert builder.payment_lag_days(2).fixing_lag_business_days(1) is builder
    assert builder.cds_imm().imm().error_policy("strict") is builder
    spec = builder.to_spec()
    assert spec["frequency"] == {"count": 3, "unit": "months"}
    assert spec["end_of_month"] is True
    assert spec["stub"] == "short_front"
    assert spec["calendar_id"] == "nyse"
    assert spec["payment_lag_days"] == 2
    assert spec["fixing_lag_business_days"] == 1
    assert spec["imm_mode"] is True
    assert spec["cds_imm_mode"] is False

    with pytest.raises(ValueError, match="nope"):
        builder.frequency("nope")
    assert builder.to_spec() == spec


def test_market_context_inserts_update_the_context_in_place_and_return_it() -> None:
    market = MarketContext()
    curve = DiscountCurve.flat("USD-OIS", dt.date(2025, 1, 2), 0.04)
    assert market.insert(curve) is market
    assert market.insert_price("SPX", 5900.0).insert_price("SPOT", 185.25, "USD") is market
    assert market.map_collateral("USD-CSA", "USD-OIS") is market
    assert "USD-OIS" in market.curve_ids()


def test_sensitivity_matrix_accessors_reject_out_of_range_indices() -> None:
    matrix = SensitivityMatrix.from_json(json.dumps(MATRIX))
    assert matrix.delta(1, 2) == 6.0
    assert matrix.position_deltas(1) == [4.0, 5.0, 6.0]
    assert matrix.factor_deltas(1) == [2.0, 5.0]
    # Row-major storage: (0, 3) would read (1, 0) without the Rust bounds check.
    with pytest.raises(ValueError, match="factor_idx 3 out of bounds for 3 factors"):
        matrix.delta(0, 3)
    with pytest.raises(ValueError, match="position_idx 2 out of bounds for 2 positions"):
        matrix.delta(2, 0)
    with pytest.raises(ValueError, match="position_idx 2 out of bounds"):
        matrix.position_deltas(2)
    with pytest.raises(ValueError, match="factor_idx 3 out of bounds"):
        matrix.factor_deltas(3)


def test_constraint_constructors_validate_in_rust_and_serialize_to_the_wire_object() -> None:
    assert json.loads(Constraint.budget(1.0).to_json()) == {"budget": {"rhs": 1.0}}
    assert json.loads(Constraint.max_turnover(0.25, label="turnover").to_json()) == {
        "max_turnover": {"label": "turnover", "max_turnover": 0.25}
    }
    assert json.loads(Constraint.weight_bounds(PositionFilter.all(), 0.0, 0.1).to_json()) == {
        "weight_bounds": {"label": None, "filter": "all", "min": 0.0, "max": 0.1}
    }
    limit = json.loads(Constraint.exposure_limit("rating", "CCC", 0.1, label="ccc cap").to_json())
    minimum = json.loads(Constraint.exposure_minimum("rating", "CCC", 0.1, label="ccc cap").to_json())
    assert limit["metric_bound"]["op"] == "le"
    assert limit["metric_bound"]["rhs"] == 0.1
    assert limit["metric_bound"]["label"] == "ccc cap"
    assert {**minimum["metric_bound"], "op": "le"} == limit["metric_bound"]

    with pytest.raises(ValueError, match="budget rhs"):
        Constraint.budget(-1.0)
    with pytest.raises(ValueError, match="max_turnover"):
        Constraint.max_turnover(-0.1)
    with pytest.raises(ValueError, match="min"):
        Constraint.weight_bounds(PositionFilter.all(), 0.2, 0.1)
    with pytest.raises(ValueError, match="max_share must be in"):
        Constraint.exposure_limit("rating", "CCC", 1.5)
    with pytest.raises(ValueError, match="min_share must be in"):
        Constraint.exposure_minimum("rating", "CCC", -0.5)


def test_portfolio_metrics_total_lookup_matches_the_wasm_twin() -> None:
    metrics = PortfolioMetrics.from_json(
        json.dumps({
            "aggregated": {"dv01": {"metric_id": "dv01", "total": 12.5, "by_entity": {}}},
            "by_position": {},
        })
    )
    assert metrics.get_total("dv01") == 12.5
    assert metrics.get_total("cs01") is None


def test_portfolio_to_spec_is_the_plain_form_of_to_json() -> None:
    portfolio = Portfolio.builder("book", "USD", dt.date(2025, 1, 15)).build()
    spec = portfolio.to_spec()
    assert spec == json.loads(portfolio.to_json())
    assert spec["id"] == "book"
    assert Portfolio.from_spec(portfolio.to_json()) == portfolio
    assert not hasattr(portfolio, "to_spec_json")
