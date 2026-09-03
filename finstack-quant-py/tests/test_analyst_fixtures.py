"""Native end-to-end gates for the analyst curriculum's shared fixtures."""

from __future__ import annotations

from datetime import date, timedelta
from decimal import Decimal
import importlib
import json
import math
from pathlib import Path
from types import SimpleNamespace

import numpy as np
import pandas as pd
import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio import (
    Portfolio,
    aggregate_full_cashflows,
    aggregate_metrics,
    mwr_xirr,
    twrr_linked_json,
    twrr_modified_dietz,
    value_portfolio,
)
from finstack_quant.statements import Evaluator


@pytest.fixture(scope="module")
def fixtures() -> SimpleNamespace:
    with pytest.MonkeyPatch.context() as patch:
        patch.syspath_prepend(str(Path(__file__).parents[1] / "examples" / "notebooks"))
        return SimpleNamespace(
            book=importlib.import_module("_shared.analyst_book"),
            history=importlib.import_module("_shared.analyst_history"),
            borrower=importlib.import_module("_shared.borrower_model"),
        )


def test_cumulative_stages_preserve_previous_snapshots(fixtures: SimpleNamespace) -> None:
    previous = {}
    snapshots = []
    for stage, count in zip(fixtures.book.STAGES, (4, 5, 6, 7, 8), strict=True):
        definitions = fixtures.book.instruments(stage)
        assert len(fixtures.book.build_book(stage)) == count
        assert all(definitions[key] == value for key, value in previous.items())
        snapshots.append((definitions, json.dumps(definitions, sort_keys=True)))
        previous = definitions
    mutated = fixtures.book.book_spec("common")
    mutated["positions"][0]["instrument_spec"]["spec"]["notional"]["amount"] = "1"
    for definitions, before in snapshots:
        assert json.dumps(definitions, sort_keys=True) == before
    assert "BORROWER-TL" not in fixtures.book.instruments("base")


@pytest.mark.parametrize("stage", ["base", "common"])
def test_strict_mixed_book_risk_and_currency_reconciliation(fixtures: SimpleNamespace, stage: str) -> None:
    market = fixtures.book.build_market(stage)
    book = fixtures.book.build_book(stage)
    result = value_portfolio(book, market, strict_risk=True, metrics=list(fixtures.book.RISK_METRICS))
    wire = json.loads(result.to_json())
    assert not wire.get("degraded_positions", [])
    for iid, expected in fixtures.book.expected_metrics(stage).items():
        position = wire["position_values"][iid]
        assert position["risk_metrics_complete"]
        assert not position.get("risk_error")
        measures = position["valuation_result"]["measures"]
        assert all(math.isfinite(value) for value in measures.values())
        for family in expected:
            assert any(key == family or key.startswith(f"{family}::") for key in measures), (iid, family)
    assert not any(
        key.startswith("cs01") for key in wire["position_values"]["USD-CASH"]["valuation_result"]["measures"]
    )
    summed = sum(Decimal(position["value_base"]["amount"]) for position in wire["position_values"].values())
    assert abs(summed - Decimal(wire["total_base_currency"]["amount"])) <= Decimal("0.01")
    eur = wire["position_values"]["EUR-GOVT"]
    assert Decimal(eur["value_base"]["amount"]) == pytest.approx(
        Decimal(eur["value_native"]["amount"]) * Decimal("1.08"), abs=Decimal("0.01")
    )
    metrics = aggregate_metrics(result, "USD", market, fixtures.book.AS_OF)
    assert not metrics.skipped_metrics
    assert not metrics.degraded_positions
    assert "cs01::ACME-HZD" in metrics.aggregated
    assert any(key.startswith("bucketed_dv01::EUR-OIS::") for key in metrics.aggregated)
    assert any(key.startswith("pv01::USD-SOFR-3M") for key in metrics.aggregated)


def test_calibration_replay_survives_market_serialization(fixtures: SimpleNamespace) -> None:
    market = fixtures.book.build_market("base")
    before = market.to_json()
    restored = MarketContext.from_json(before)
    spec = fixtures.book.book_spec("base")
    spec["positions"] = [position for position in spec["positions"] if position["instrument_id"] == "ACME-CDS"]
    cds = Portfolio.from_spec(json.dumps(spec))
    first = value_portfolio(cds, market, strict_risk=True, metrics=["cs01", "bucketed_cs01"])
    second = value_portfolio(cds, restored, strict_risk=True, metrics=["cs01", "bucketed_cs01"])
    measures = json.loads(first.to_json())["position_values"]["ACME-CDS"]["valuation_result"]["measures"]
    replayed = json.loads(second.to_json())["position_values"]["ACME-CDS"]["valuation_result"]["measures"]
    assert measures == replayed
    assert measures["cs01::ACME-HZD"] > 0
    assert market.to_json() == before


def test_cashflow_ladder_is_complete_and_reconciles_native_currency(fixtures: SimpleNamespace) -> None:
    cashflows = aggregate_full_cashflows(fixtures.book.build_book("common"), fixtures.book.build_market("common"))
    wire = json.loads(cashflows.to_json())
    assert not wire["issues"]
    assert set(wire["position_summaries"]) == set(fixtures.book.instruments("common"))
    for currency in ("USD", "EUR"):
        raw_sum = sum(
            Decimal(event["amount"]["amount"]) for event in wire["events"] if event["amount"]["currency"] == currency
        )
        ladder_sum = sum(amount for _, amount in cashflows.net_in_currency_by_date(currency))
        assert float(raw_sum) == pytest.approx(ladder_sum, abs=0.01)


def test_dated_flow_history_drives_native_performance(fixtures: SimpleNamespace) -> None:
    """Price dated holdings, settle payments and trades, then remove external flows."""
    history = fixtures.history.performance_history()
    events = fixtures.history.history_events()
    start_date, end_date = fixtures.history.HISTORY_DATES[0], fixtures.history.HISTORY_DATES[-1]
    initial_market = fixtures.history.history_market(start_date)
    initial_spec = fixtures.history.history_spec(start_date)
    initial_book = Portfolio.from_spec(json.dumps(initial_spec))
    initial_nav = value_portfolio(initial_book, initial_market, metrics=[]).total_value
    initial_cashflows = json.loads(aggregate_full_cashflows(initial_book, initial_market).to_json())["events"]
    payment_dates = {
        date.fromisoformat(event["date"])
        for event in initial_cashflows
        if start_date < date.fromisoformat(event["date"]) <= end_date
    }
    timeline = sorted(
        payment_dates
        | set(fixtures.history.HISTORY_DATES[1:])
        | {date.fromisoformat(event["date"]) for event in events}
    )
    cash, segment_start, factor = 0.0, initial_nav, 1.0
    period_begin, period_flows, returns, dietz_returns = initial_nav, [], [], []
    investor_flows = [{"date": start_date.isoformat(), "amount": -initial_nav}]
    settled_positions = set()
    for when in timeline:
        market = fixtures.history.history_market(when)
        if when in payment_dates:
            held = fixtures.history.history_spec(when - timedelta(days=1))
            held["as_of"] = when.isoformat()
            payments = json.loads(aggregate_full_cashflows(Portfolio.from_spec(json.dumps(held)), market).to_json())
            for payment in payments["events"]:
                if payment["date"] == when.isoformat():
                    amount = float(payment["amount"]["amount"])
                    if payment["amount"]["currency"] == "EUR":
                        amount *= fixtures.history.history_quotes(when)["eur_usd"]
                    cash += amount
                    settled_positions.add(payment["position_id"])
        spec = fixtures.history.history_spec(when)
        valuation = value_portfolio(Portfolio.from_spec(json.dumps(spec)), market, metrics=[])
        wire = json.loads(valuation.to_json())
        for event in (event for event in events if event["date"] == when.isoformat()):
            if event["type"] == "trade":
                position = next(
                    position for position in spec["positions"] if position["instrument_id"] == event["instrument_id"]
                )
                unit_value = float(wire["position_values"][event["instrument_id"]]["value_base"]["amount"])
                unit_value /= position["quantity"]
                trade_cost = event["quantity_change"] * unit_value
                before_trade = valuation.total_value - trade_cost + cash
                cash -= trade_cost
                assert valuation.total_value + cash == pytest.approx(before_trade, abs=1e-8)
            else:
                before_flow = valuation.total_value + cash
                factor *= before_flow / segment_start
                cash += event["amount"]
                segment_start = valuation.total_value + cash
                assert segment_start - before_flow == pytest.approx(event["amount"], abs=1e-8)
                period_flows.append(event)
                investor_flows.append({"date": event["date"], "amount": -event["amount"]})
        nav = valuation.total_value + cash
        if when in fixtures.history.HISTORY_DATES[1:]:
            returns.append(factor * nav / segment_start - 1.0)
            period = history[len(returns) - 1]
            start, end = date.fromisoformat(period["start"]), date.fromisoformat(period["end"])
            dietz_returns.append(
                twrr_modified_dietz(
                    json.dumps({
                        "beginning_market_value": period_begin,
                        "ending_market_value": nav,
                        "cashflows": [
                            {
                                "amount": flow["amount"],
                                "fraction_of_period_remaining": (end - date.fromisoformat(flow["date"])).days
                                / (end - start).days,
                            }
                            for flow in period_flows
                        ],
                    })
                )
            )
            period_begin, segment_start, factor, period_flows = nav, nav, 1.0, []
    investor_flows.append({"date": end_date.isoformat(), "amount": nav})
    years = (date.fromisoformat(history[-1]["end"]) - date.fromisoformat(history[0]["start"])).days / 365.0
    linked = json.loads(twrr_linked_json(json.dumps(returns), years))
    assert len(returns) == len(dietz_returns) == 3
    assert linked["cumulative"] == pytest.approx(math.prod(1 + value for value in returns) - 1)
    assert all(math.isfinite(value) for value in returns + dietz_returns)
    assert {"USD-CORP", "EUR-GOVT", "USD-CASH", "EUR-CASH", "ACME-CDS", "USD-PAYER-IRS"} <= settled_positions
    assert cash > 0
    assert fixtures.history.history_spec(start_date) == initial_spec
    xirr = mwr_xirr(json.dumps(investor_flows))
    assert math.isfinite(xirr)
    anchor = date.fromisoformat(investor_flows[0]["date"])
    npv = sum(
        flow["amount"] / (1 + xirr) ** ((date.fromisoformat(flow["date"]) - anchor).days / 365.0)
        for flow in investor_flows
    )
    # Native XIRR normalizes cash amounts before applying its 1e-8 tolerance.
    assert abs(npv) / max(abs(flow["amount"]) for flow in investor_flows) < 1e-8


def test_risk_panel_alignment_and_reproducibility(fixtures: SimpleNamespace) -> None:
    panel = fixtures.history.risk_panel()
    pd.testing.assert_frame_equal(panel, fixtures.history.risk_panel())
    assert panel.index.is_unique
    assert panel.index.is_monotonic_increasing
    assert len(panel) == 504
    assert not panel.isna().any().any()
    assets = panel[list(fixtures.history.ASSET_COLUMNS)].to_numpy()
    assert np.allclose(panel["portfolio"], assets @ np.array([0.2, 0.35, 0.35, 0.1]))
    assert np.linalg.eigvalsh(np.cov(assets, rowvar=False)).min() > 0
    assert panel["portfolio"].corr(panel["benchmark"]) > 0.9


def test_borrower_forecasts_and_three_statement_reconciliation(fixtures: SimpleNamespace) -> None:
    model = fixtures.borrower.borrower_model()
    before = model.to_json()
    result = Evaluator().evaluate(model)
    for period in fixtures.borrower.PERIODS:
        assert abs(result.get("balance_check", period)) < 1e-6
        assert result.get("cash_end", period) > 0
        assert result.get("net_leverage", period) > 0
        assert math.isfinite(result.get("dscr", period))
    assert result.get("revenue", "2025Q1") == pytest.approx(24_000_000 * 1.02)
    assert result.get("debt_end", "2024Q4") == 100_000_000
    assert result.get("debt_end", "2025Q4") == 90_000_000
    assert result.get("debt_begin", "2025Q1") == 100_000_000
    assert result.get("cash_interest", "2025Q1") == 1_875_000
    assert model.to_json() == before
    assert fixtures.borrower.borrower_model().to_json() == before


def test_borrower_vintages_exclude_future_actuals_and_preserve_loan_units(fixtures: SimpleNamespace) -> None:
    prior = fixtures.borrower.borrower_vintage("2024Q3")
    current = fixtures.borrower.borrower_vintage("2024Q4")
    prior_wire = json.loads(prior.to_json())
    assert "2024Q4" not in prior_wire["nodes"]["revenue"]["values"]
    assert not prior_wire["periods"][3]["is_actual"]
    evaluator = Evaluator()
    prior_result, current_result = evaluator.evaluate(prior), evaluator.evaluate(current)
    assert prior_result.get("revenue", "2024Q4") == pytest.approx(23_000_000 * 1.01)
    assert current_result.get("revenue", "2024Q4") == 24_000_000
    assert prior_result.get("revenue", "2024Q3") == current_result.get("revenue", "2024Q3")
    data = fixtures.borrower.borrower_data()
    loan = fixtures.book.instruments("common")["BORROWER-TL"]["instrument"]["spec"]
    assert data["loan"]["holding_notional"] == float(loan["notional_limit"]["amount"])
    assert data["loan"]["spread_bp"] == float(loan["rate"]["floating"]["spread_bp"])
    assert data["loan"]["holding_notional"] / data["loan"]["facility_notional"] == data["loan"]["holding_fraction"]
    assert data["credit"]["measure"] == "physical"
    assert data["credit"]["base"]["pd"] < data["credit"]["downside"]["pd"] < data["credit"]["severe"]["pd"]
    data["loan"]["holding_notional"] = 0.0
    assert fixtures.borrower.borrower_data()["loan"]["holding_notional"] > 0
