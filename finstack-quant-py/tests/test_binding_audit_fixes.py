"""Regression tests for the 2026-08 binding-audit fixes.

Covers strict metric-name parsing, core validation taxonomy for liquidity
entry points, the ``measure`` getter wire form, ``OptimizationStatus``
equality, residual-contribution parity on ``decompose_factor_risk``, new
result fields (``specific_return``, ``degraded_positions``,
``unaggregated_metrics``), non-finite risk-budget utilization, and the
``Portfolio.validate_materialization`` twin.
"""

from __future__ import annotations

from datetime import date
import json
import math

import pytest

from finstack_quant.attribution import ReturnContributionResult
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.models.factor.risk import evaluate_risk_budget
from finstack_quant.models.liquidity import lvar_bangia
from finstack_quant.portfolio import (
    OptimizationStatus,
    PerPositionMetric,
    Portfolio,
    PortfolioMetrics,
    compute_factor_sensitivities,
    decompose_factor_risk,
    value_portfolio,
)

AS_OF = "2025-01-15"


def _portfolio_json() -> str:
    return json.dumps({
        "id": "AUDIT-FIXES",
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
                        "quote_rate": "0.04",
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


# M3: strict metric parsing in value_portfolio


def test_value_portfolio_rejects_unknown_metric_name() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market = _market()

    with pytest.raises(ValueError, match="dv011"):
        value_portfolio(portfolio, market, metrics=["dv011"])


def test_value_portfolio_still_accepts_standard_metric_names() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market = _market()

    valuation = json.loads(value_portfolio(portfolio, market, metrics=["dv01"]).to_json())
    measures = valuation["position_values"]["USD-POS"]["valuation_result"]["measures"]
    assert "dv01" in measures


# M4: strict metric parsing in PerPositionMetric.metric


def test_per_position_metric_rejects_unknown_metric_name() -> None:
    with pytest.raises(ValueError, match="dvo1"):
        PerPositionMetric.metric("dvo1")


def test_per_position_metric_accepts_standard_and_custom_paths() -> None:
    assert PerPositionMetric.metric("dv01").kind == "metric"
    assert PerPositionMetric.custom_key("my_measure").kind == "custom_key"


# M5: measure getter returns the bare snake_case serde tag


def _variance_decomposition():  # noqa: ANN202
    matrix = compute_factor_sensitivities("[]", "[]", MarketContext(), AS_OF, "USD")
    return decompose_factor_risk(matrix, '{"factor_ids":[],"n":0,"data":[]}')


def test_decompose_factor_risk_measure_is_bare_snake_case_tag() -> None:
    decomp = _variance_decomposition()
    assert decomp.measure == "variance"


# MD5: position_residual_contributions exposed on FactorRiskDecomposition


def test_decompose_factor_risk_exposes_position_residual_contributions() -> None:
    decomp = _variance_decomposition()
    assert decomp.position_residual_contributions() == []


# MD9: models liquidity errors use the canonical validation taxonomy


def test_lvar_bangia_validation_failure_is_value_error() -> None:
    with pytest.raises(ValueError, match="confidence"):
        lvar_bangia(-100.0, 0.01, 0.005, 0.3, 1_000_000.0)


# MD10: OptimizationStatus equality and hashing


def test_optimization_status_equality_and_hash() -> None:
    assert OptimizationStatus.optimal() == OptimizationStatus.optimal()
    assert hash(OptimizationStatus.optimal()) == hash(OptimizationStatus.optimal())
    assert OptimizationStatus.optimal() != OptimizationStatus.unbounded()
    assert OptimizationStatus.infeasible(["a"]) == OptimizationStatus.infeasible(["a"])
    assert OptimizationStatus.infeasible(["a"]) != OptimizationStatus.infeasible(["b"])


# MD11: Portfolio.validate_materialization Python twin


def _empty_bundle() -> dict[str, object]:
    return {
        "schema": "finstack_quant.portfolio_materialization/1",
        "portfolio": {
            "id": "AUDIT-EMPTY",
            "as_of": AS_OF,
            "base_currency": "USD",
            "entities": {},
        },
        "instruments": [],
        "positions": [],
    }


def test_validate_materialization_returns_report_for_valid_bundle() -> None:
    report = Portfolio.validate_materialization(json.dumps(_empty_bundle()))
    assert report.positions == 0
    assert report.unique_instruments == 0
    assert report.truncated is False


def test_validate_materialization_returns_diagnostics_for_invalid_bundle() -> None:
    bundle = _empty_bundle()
    bundle["positions"] = [
        {
            "id": "P1",
            "entity_id": "MISSING",
            "instrument_id": "I1",
            "artifact_id": "sha256:deadbeef",
            "quantity": 1.0,
            "unit": "units",
        }
    ]
    outcome = Portfolio.validate_materialization(json.dumps(bundle))
    assert isinstance(outcome, dict)
    assert outcome["diagnostics"], "expected at least one diagnostic"
    assert all("code" in item for item in outcome["diagnostics"])


# Item 13a: ReturnContributionResult.specific_return


def test_return_contribution_result_exposes_specific_return() -> None:
    doc = json.dumps({
        "portfolio_return": 0.01,
        "instrument_contribution": [],
        "group_contribution": {},
        "factor_contribution": [],
        "specific_return": 0.004,
        "benchmark_relative": None,
    })
    result = ReturnContributionResult.from_json(doc)
    assert result.specific_return == pytest.approx(0.004)

    no_factors = json.dumps({
        "portfolio_return": 0.01,
        "instrument_contribution": [],
        "group_contribution": {},
        "factor_contribution": [],
        "benchmark_relative": None,
    })
    assert ReturnContributionResult.from_json(no_factors).specific_return is None


# Item 13c: PortfolioMetrics.degraded_positions / unaggregated_metrics


def test_portfolio_metrics_exposes_degradation_fields() -> None:
    doc = json.dumps({
        "aggregated": {},
        "by_position": {},
        "degraded_positions": ["P-1"],
        "unaggregated_metrics": ["ytm"],
    })
    metrics = PortfolioMetrics.from_json(doc)
    assert metrics.degraded_positions == ["P-1"]
    assert metrics.unaggregated_metrics == ["ytm"]


# Item 13e: non-finite risk-budget utilization survives every exit


def test_zero_target_breach_reports_infinite_utilization_everywhere() -> None:
    result = evaluate_risk_budget(
        position_ids=["A", "B"],
        actual_var=[-50.0, -50.0],
        target_var_pct=[0.0, 1.0],
        portfolio_var=-100.0,
    )
    entry = result.positions[0]
    assert math.isinf(entry.utilization)

    single_row = entry.to_dataframe()
    cell = single_row["utilization"].iloc[0]
    assert isinstance(cell, float), f"expected numeric utilization, got {type(cell)}"
    assert math.isinf(cell)

    frame = result.to_dataframe()
    assert math.isinf(float(frame["utilization"].iloc[0]))

    payload = json.loads(result.to_json())
    wire_value = payload["positions"][0]["utilization"]
    assert wire_value in ("inf", "-inf")


# Minor (b): days_to_liquidate keywords follow the Rust share-space contract


def test_days_to_liquidate_uses_rust_share_space_keywords() -> None:
    from finstack_quant.models.liquidity import days_to_liquidate

    assert days_to_liquidate(position_quantity=1000.0, adv=100.0, participation_rate=0.1) == 100.0


# Minor (g): NumPy column-mismatch message no longer claims "row 0"


def test_square_matrix_column_mismatch_message_has_no_fake_row_index() -> None:
    np = pytest.importorskip("numpy")
    from finstack_quant.models.factor.risk import parametric_var_decomposition

    with pytest.raises(ValueError, match="columns") as excinfo:
        parametric_var_decomposition(
            ["A", "B"],
            [1.0, 2.0],
            np.zeros((2, 3), dtype=np.float64),
        )
    message = str(excinfo.value)
    assert "row 0" not in message
    assert "columns" in message
