"""Cashflow currency and typed/JSON validation boundary regressions."""

import datetime as dt
import json
import pickle

import pytest

from finstack_quant.cashflows import dated_flows, dated_flows_json
from finstack_quant.cashflows.accrual import AccrualConfig, accrued_interest_amount
from finstack_quant.cashflows.aggregation import PeriodAggregation
from finstack_quant.cashflows.builder import CashFlowMeta, CashFlowSchedule, Notional
from finstack_quant.cashflows.primitives import CashFlow, CashFlowAccrual
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money


def test_aggregation_rejects_mislabeled_money_currency() -> None:
    wire = '{"2025Q1":{"USD":{"amount":"100","currency":"EUR"}}}'
    with pytest.raises(ValueError, match=r"USD.*EUR"):
        PeriodAggregation.from_json(wire)


def test_aggregation_round_trip_preserves_currency_labels() -> None:
    wire = json.dumps({
        "2025Q1": {
            "USD": {"amount": "100", "currency": "USD"},
            "EUR": {"amount": "200", "currency": "EUR"},
        }
    })
    totals = PeriodAggregation.from_json(wire)
    restored = PeriodAggregation.from_json(totals.to_json())
    assert restored.get("2025Q1", "USD") == Money(100, "USD")
    assert restored.get("2025Q1", "EUR") == Money(200, "EUR")
    assert restored.to_dataframe().to_dict("records") == [
        {"period": "2025Q1", "currency": "USD", "amount": 100.0},
        {"period": "2025Q1", "currency": "EUR", "amount": 200.0},
    ]


@pytest.mark.parametrize("as_json", [False, True])
def test_dated_flows_rejects_same_invalid_economics_as_json(as_json: bool) -> None:
    schedule = CashFlowSchedule.from_flows(
        [CashFlow(dt.date(2025, 6, 15), Money(150, "USD"), "amortization")],
        Notional.par(100, "USD"),
        DayCount.ACT_360,
    )
    wire = schedule.to_json()
    with pytest.raises(ValueError, match="repayments exceed outstanding"):
        dated_flows(wire if as_json else schedule)
    with pytest.raises(ValueError, match="repayments exceed outstanding"):
        dated_flows_json(wire)


def test_dated_flows_preserves_cash_and_omits_state_rows() -> None:
    date = dt.date(2025, 6, 15)
    schedule = CashFlowSchedule.from_flows(
        [
            CashFlow(date, Money(10, "USD"), "fixed"),
            CashFlow(date, Money(5, "USD"), "pik"),
            CashFlow(date, Money(20, "USD"), "defaulted_notional"),
        ],
        Notional.par(100, "USD"),
        DayCount.ACT_360,
    )
    assert dated_flows(schedule) == [(date, Money(10, "USD"))]
    assert dated_flows(schedule.to_json()) == dated_flows(schedule)
    assert json.loads(dated_flows_json(schedule.to_json())) == [
        {"date": "2025-06-15", "amount": {"amount": "10", "currency": "USD"}}
    ]


def _coupon_schedule(flow: CashFlow) -> CashFlowSchedule:
    return CashFlowSchedule.from_parts(
        [flow],
        Notional.par(1_000, "USD"),
        DayCount.ACT_360,
        CashFlowMeta(issue_date=dt.date(2025, 1, 1), maturity=dt.date(2025, 7, 1)),
    )


@pytest.mark.parametrize("invalid", [float("nan"), float("inf"), float("-inf")])
@pytest.mark.parametrize("field", ["rate", "projected_index_rate"])
def test_nonfinite_coupon_rates_cannot_be_erased_by_serialization(invalid: float, field: str) -> None:
    flow = CashFlow(
        dt.date(2025, 7, 1),
        Money(25, "USD"),
        "fixed",
        accrual_factor=181 / 360,
        rate=invalid if field == "rate" else 0.05,
    ).with_accrual(
        CashFlowAccrual(
            dt.date(2025, 1, 1),
            dt.date(2025, 7, 1),
            DayCount.ACT_360,
            projected_index_rate=invalid if field == "projected_index_rate" else 0.045,
        )
    )
    schedule = _coupon_schedule(flow)

    for value in (flow, schedule):
        with pytest.raises(ValueError, match="finite"):
            value.validate()
        with pytest.raises(ValueError, match="finite"):
            value.to_json()
        with pytest.raises(ValueError, match="finite"):
            pickle.dumps(value)

    with pytest.raises(ValueError, match="finite"):
        accrued_interest_amount(schedule, dt.date(2025, 4, 1), AccrualConfig())


@pytest.mark.parametrize("wire_format", ["json", "pickle"])
@pytest.mark.parametrize(("rate", "projected_rate"), [(None, None), (-0.05, -0.055), (0.05, 0.045)])
def test_coupon_rate_round_trip_preserves_metadata_and_accrual(
    wire_format: str, rate: float | None, projected_rate: float | None
) -> None:
    amount = -25 if rate is not None and rate < 0 else 25
    flow = CashFlow(
        dt.date(2025, 7, 1),
        Money(amount, "USD"),
        "fixed",
        accrual_factor=181 / 360,
        rate=rate,
    ).with_accrual(
        CashFlowAccrual(
            dt.date(2025, 1, 1),
            dt.date(2025, 7, 1),
            DayCount.ACT_360,
            projected_index_rate=projected_rate,
        )
    )
    schedule = _coupon_schedule(flow)
    flow.validate()
    schedule.validate()
    if wire_format == "json":
        restored_flow = CashFlow.from_json(flow.to_json())
        restored_schedule = CashFlowSchedule.from_json(schedule.to_json())
    else:
        restored_flow = pickle.loads(pickle.dumps(flow))  # noqa: S301 - trusted in-process round trip
        restored_schedule = pickle.loads(pickle.dumps(schedule))  # noqa: S301 - trusted in-process round trip

    for restored in (restored_flow, restored_schedule.get_flows()[0]):
        restored.validate()
        assert restored.to_json() == flow.to_json()
        assert restored.rate == rate
        assert restored.accrual is not None
        assert restored.accrual.projected_index_rate == projected_rate
    restored_schedule.validate()
    assert restored_schedule.to_json() == schedule.to_json()
    for value in (schedule, restored_schedule):
        accrued = accrued_interest_amount(value, dt.date(2025, 4, 1), AccrualConfig())
        assert accrued.currency.code == "USD"
        assert accrued.amount == pytest.approx(amount * 90 / 181)
