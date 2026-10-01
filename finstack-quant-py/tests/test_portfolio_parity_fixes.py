"""Behavioural pins for the portfolio binding-parity fixes.

Covers typed portfolio construction (``Portfolio.builder``), typed scenario
inputs on the pipeline functions, the ``PV + dv01`` standard metric plan,
``PortfolioResult`` assembly, ``PortfolioCashflows`` typed accessors, the
``ComparisonOp`` / ``Inequality`` string spellings, the serde wire shape of
what-if position changes, and the optimization trade universe.
"""

from __future__ import annotations

import datetime as dt
import json
import pickle

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.portfolio import (
    CandidatePosition,
    Constraint,
    Inequality,
    MetricExpr,
    Objective,
    PerPositionMetric,
    Portfolio,
    PortfolioBuilder,
    PortfolioCashflows,
    PortfolioMetrics,
    PortfolioResult,
    PortfolioValuation,
    PositionFilter,
    PositionValue,
    ReconciliationReport,
    TradeUniverse,
    aggregate_full_cashflows,
    aggregate_metrics,
    apply_scenario_and_revalue,
    mwr_xirr,
    net_in_currency_by_date,
    scenario_pnl,
    scenario_pnl_batch,
    twrr_modified_dietz,
    value_portfolio,
)
from finstack_quant.scenarios import ScenarioSpec
from finstack_quant.valuations.instruments import Bond

AS_OF = dt.date(2025, 1, 15)


def _bond(bond_id: str = "B1") -> Bond:
    return Bond.fixed(
        bond_id,
        1_000_000.0,
        0.05,
        dt.date(2024, 1, 15),
        dt.date(2034, 1, 15),
        "none",
        "USD-OIS",
        currency="USD",
    )


def _market() -> MarketContext:
    return MarketContext().insert(DiscountCurve.flat("USD-OIS", AS_OF, 0.04))


def _portfolio() -> Portfolio:
    return (
        Portfolio
        .builder("book", "USD", AS_OF)
        .name("Desk book")
        .entity("ACME")
        .position("P1", _bond(), 1.0, entity_id="ACME", unit="face_value")
        .tag("desk", "rates")
        .build()
    )


class TestPortfolioBuilder:
    def test_builder_returns_typed_builder_and_portfolio(self) -> None:
        builder = Portfolio.builder("book", "USD", "2025-01-15")
        assert isinstance(builder, PortfolioBuilder)
        pf = builder.build()
        assert (pf.id, pf.base_currency, pf.as_of, len(pf)) == ("book", "USD", AS_OF, 0)
        assert pf.name is None
        assert pf.tags == {}
        assert pf.meta == {}

    def test_builder_with_position_and_metadata(self) -> None:
        pf = _portfolio()
        assert pf.name == "Desk book"
        assert pf.entity_ids == ["ACME"]
        assert pf.position_ids == ["P1"]
        assert pf.tags == {"desk": "rates"}
        frame = pf.positions_to_dataframe()
        assert list(frame.columns) == [
            "position_id",
            "entity_id",
            "instrument_id",
            "instrument_type",
            "quantity",
            "unit",
            "book_id",
        ]
        assert frame.iloc[0]["unit"] == "face_value"
        assert frame.iloc[0]["instrument_id"] == "B1"

    def test_builder_is_consumed_by_build(self) -> None:
        builder = Portfolio.builder("book", "USD", AS_OF)
        builder.build()
        with pytest.raises(ValueError, match=r"already been consumed"):
            builder.build()

    def test_position_without_entity_uses_standalone_entity(self) -> None:
        pf = Portfolio.builder("book", "USD", AS_OF).position("P1", _bond(), 1.0).build()
        assert len(pf.entity_ids) == 1

    def test_invalid_currency_and_unit_raise(self) -> None:
        with pytest.raises(ValueError, match=r"Invalid currency code"):
            Portfolio.builder("book", "XXX", AS_OF)
        with pytest.raises(ValueError, match=r"unknown position unit"):
            Portfolio.builder("book", "USD", AS_OF).position("P1", _bond(), 1.0, unit="lots")

    def test_portfolio_equality_and_pickle(self) -> None:
        pf = _portfolio()
        assert pf == Portfolio.from_spec(pf.to_json())
        assert pf != Portfolio.builder("other", "USD", AS_OF).build()
        assert pickle.loads(pickle.dumps(pf)) == pf  # noqa: S301


class TestValuationAndResult:
    def test_default_plan_is_pv_plus_dv01(self) -> None:
        valuation = value_portfolio(_portfolio(), _market())
        assert isinstance(valuation, PortfolioValuation)
        assert valuation.as_of == AS_OF
        assert not valuation.has_degraded_risk
        metrics = aggregate_metrics(valuation, "USD", _market(), AS_OF)
        assert metrics.get_total("dv01") is not None
        assert metrics.get_total("theta") is None
        assert metrics.get_metric("dv01")["metric_id"] == "dv01"
        assert metrics.get_position_metrics("P1")["currency"] == "USD"

    def test_position_values_and_lookups(self) -> None:
        valuation = value_portfolio(_portfolio(), _market())
        values = valuation.position_values
        assert set(values) == {"P1"}
        assert isinstance(values["P1"], PositionValue)
        assert valuation.get_position_value("P1").risk_metrics_complete
        assert valuation.get_entity_value("ACME").currency == "USD"
        assert valuation.fx_collapse_policy == "cashflow_date"
        with pytest.raises(KeyError):
            valuation.get_position_value("nope")
        frame = valuation.to_dataframe()
        assert "risk_metrics_complete" in frame.columns
        assert "risk_error" in frame.columns

    def test_portfolio_result_assembly(self) -> None:
        valuation = value_portfolio(_portfolio(), _market())
        metrics = aggregate_metrics(valuation, "USD", _market(), AS_OF)
        result = PortfolioResult(valuation, metrics)
        assert result.total_value == pytest.approx(valuation.total_value)
        assert isinstance(result.valuation, PortfolioValuation)
        assert isinstance(result.metrics, PortfolioMetrics)
        assert "rounding" in result.meta
        assert result.get_metric("dv01") == pytest.approx(metrics.get_total("dv01"))


class TestScenarioInputs:
    def test_scenario_functions_accept_typed_spec(self) -> None:
        spec = ScenarioSpec.from_json('{"id":"s","name":"S","operations":[]}')
        pf, mkt = _portfolio(), _market()
        _valuation, report = apply_scenario_and_revalue(pf, spec, mkt)
        assert report.operations_applied == 0
        pnl, _ = scenario_pnl(pf, spec, mkt)
        assert pnl.total == pytest.approx(0.0)
        batch = scenario_pnl_batch(pf, [spec, spec.to_json()], mkt)
        assert [item.scenario_id for item in batch] == ["s", "s"]
        assert scenario_pnl_batch(pf, "[]", mkt) == []

    def test_scenario_rejects_non_spec(self) -> None:
        with pytest.raises(TypeError):
            scenario_pnl(_portfolio(), 42, _market())


class TestCashflows:
    def test_typed_accessors_and_collapse_frame(self) -> None:
        cfs = aggregate_full_cashflows(_portfolio(), _market())
        assert isinstance(cfs, PortfolioCashflows)
        assert cfs.num_positions == 1
        assert cfs.num_issues == 0
        assert cfs.events
        assert cfs.by_position["P1"]
        assert cfs.issues == []
        assert cfs.to_issues_dataframe().shape[0] == 0
        net = cfs.net_in_currency_by_date("USD")
        assert net == net_in_currency_by_date(cfs, "USD") == net_in_currency_by_date(cfs.to_json(), "USD")
        frame = cfs.collapse_to_base_by_date_kind(_market(), "USD", AS_OF)
        assert list(frame.columns) == ["date", "kind", "amount", "currency"]
        ladder = json.loads(cfs.collapse_to_base_by_date_kind_json(_market(), "USD", AS_OF))
        assert len(ladder) == frame["date"].nunique()


class TestOptimizationInputs:
    def test_inequality_strings(self) -> None:
        assert Inequality("<=") == Inequality.le()
        assert Inequality("ge") == Inequality.ge()
        with pytest.raises(ValueError, match=r"Unknown inequality"):
            Inequality("=>")
        expr = MetricExpr.weighted_sum(PerPositionMetric.pv_base())
        constraint = Constraint.metric_bound(expr, "<=", 1.0)
        assert constraint.to_json() == Constraint.metric_bound(expr, Inequality.le(), 1.0).to_json()

    def test_comparison_op_symbols(self) -> None:
        by_symbol = PositionFilter.by_attribute("rating", ">=", number=3.0)
        by_name = PositionFilter.by_attribute("rating", "ge", number=3.0)
        assert by_symbol.to_json() == by_name.to_json()

    def test_trade_universe_round_trip(self) -> None:
        candidate = CandidatePosition("C1", "ACME", _bond("B2"), unit="face_value", max_weight=0.5)
        assert candidate.instrument_id == "B2"
        universe = TradeUniverse.filtered(PositionFilter.all()).with_candidate(candidate).allow_shorting_candidates()
        rebuilt = TradeUniverse.from_json(universe.to_json())
        assert [c.id for c in rebuilt.candidates] == ["C1"]
        assert rebuilt.allow_short_candidates
        spec = (
            __import__("finstack_quant.portfolio", fromlist=["PortfolioOptimizationSpec"])
            .PortfolioOptimizationSpec.new(
                _portfolio(), Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
            )
            .with_trade_universe(universe)
        )
        assert spec.trade_universe is not None
        assert json.loads(spec.to_json())["trade_universe"]["allow_short_candidates"] is True


class TestPerformanceInputs:
    def test_twrr_modified_dietz_takes_one_rust_owned_period(self) -> None:
        # Rust owns the omitted-cashflows meaning (no flows) and rejects
        # unknown keys; the same JSON is accepted by WASM twrrModifiedDietz.
        bare = {"beginning_market_value": 100.0, "ending_market_value": 110.0}
        assert twrr_modified_dietz(bare) == pytest.approx(0.1)
        assert twrr_modified_dietz({**bare, "cashflows": []}) == twrr_modified_dietz(bare)
        with pytest.raises(ValueError, match=r"unknown field `bogus`"):
            twrr_modified_dietz({**bare, "bogus": 1})
        with pytest.raises(TypeError):
            twrr_modified_dietz(beginning_market_value=100.0, ending_market_value=110.0)  # type: ignore[call-arg]

    def test_mwr_xirr_accepts_tuples(self) -> None:
        pairs = [(dt.date(2025, 1, 1), -100.0), ("2026-01-01", 110.0)]
        assert mwr_xirr(pairs) == pytest.approx(0.1, abs=1e-6)


class TestReconciliationReport:
    def test_from_json(self) -> None:
        report = ReconciliationReport.from_json('{"total_residual":0.0,"is_reconciled":true,"tolerance":0.01}')
        assert report.is_reconciled
        assert report.tolerance == 0.01
        assert pickle.loads(pickle.dumps(report)).to_json() == report.to_json()  # noqa: S301
