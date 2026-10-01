"""Typed cashflows surface: Python results against the cross-host goldens.

``finstack-quant-wasm/tests/facade/cashflows_parity.test.mjs`` computes every
case below through the WASM facade and pins the values in
``finstack-quant-wasm/tests/facade/golden/cashflows_parity.json``. The same
inputs go through the Python bindings here and must give the same JSON, so
both hosts are held to one set of numbers. Decimal strings (``Money``) compare
exactly; floats compare to 1e-12 relative, the native-vs-wasm32 allowance of
INVARIANTS.md §2.1.
"""

from __future__ import annotations

from collections.abc import Callable
import datetime as dt
import json
import math
from pathlib import Path
import re
from typing import Any

import pytest

from finstack_quant import cashflows
from finstack_quant.cashflows.accrual import AccrualConfig, AccrualIndex, ExCouponRule, accrued_interest_amount
from finstack_quant.cashflows.aggregation import (
    aggregate_by_period,
    aggregate_cashflows_checked,
    calendar_year_ladder,
)
from finstack_quant.cashflows.builder import (
    AmortizationSpec,
    CashFlowMeta,
    CashFlowSchedule,
    CouponType,
    DefaultModelSpec,
    FeeBase,
    FeeSpec,
    FixedCouponSpec,
    FloatingCouponSpec,
    FloatingLegCompounding,
    FloatingRateFallback,
    FloatingRateSpec,
    Notional,
    PrepaymentModelSpec,
    PrincipalExchange,
    RecoveryModelSpec,
    ScheduleParams,
    StepUpCouponSpec,
    merge_cashflow_schedules,
)
from finstack_quant.cashflows.fixings import materialize_fixings
from finstack_quant.cashflows.primitives import CashFlow, CashFlowAccrual, CFKind, is_cash_settlement_kind
from finstack_quant.core.dates import DayCount, Tenor, build_periods
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money

GOLDEN_DIR = Path(__file__).parents[2] / "finstack-quant-wasm/tests/facade/golden"
GOLDEN: dict[str, Any] = json.loads((GOLDEN_DIR / "cashflows_parity.json").read_text())
MARKET_JSON = (GOLDEN_DIR / "parity_market.json").read_text()

ISSUE = dt.date(2025, 1, 15)
MATURITY = dt.date(2027, 1, 15)
FIXED_SPEC = {
    "coupon_type": "cash",
    "rate": "0.06",
    "frequency": {"count": 6, "unit": "months"},
    "day_count": "30_360",
    "business_day_convention": "following",
    "calendar_id": "weekends_only",
    "stub": "none",
    "end_of_month": False,
    "payment_lag_days": 0,
}
FLOATING_SPEC = {
    "rate_spec": {
        "forward_curve_id": "USD-SOFR-3M",
        "spread_bp": "150",
        "reset_frequency": {"count": 3, "unit": "months"},
        "reset_lag_days": 0,
    },
    "coupon_type": "cash",
    "frequency": {"count": 3, "unit": "months"},
    "day_count": "act_360",
    "business_day_convention": "following",
    "calendar_id": "weekends_only",
    "stub": "none",
    "end_of_month": False,
    "payment_lag_days": 0,
}


def usd(amount: float) -> Money:
    return Money(amount, "USD")


def fixed(**overrides: Any) -> FixedCouponSpec:
    return FixedCouponSpec.from_json(json.dumps({**FIXED_SPEC, **overrides}))


def floating() -> FloatingCouponSpec:
    return FloatingCouponSpec.from_json(json.dumps(FLOATING_SPEC))


def market() -> MarketContext:
    return MarketContext.from_json(MARKET_JSON)


def builder() -> Any:
    return CashFlowSchedule.builder().principal(usd(1_000_000), ISSUE, MATURITY)


def bond() -> CashFlowSchedule:
    return builder().fixed_cf(fixed()).build()


def periods() -> list[Any]:
    return list(build_periods("2025Q1..2027Q1").periods)


def wire(value: Any) -> Any:
    """The JSON a WASM caller sees for a Python binding value."""
    if hasattr(value, "to_json"):
        return json.loads(value.to_json())
    if isinstance(value, dt.date):
        return value.isoformat()
    if isinstance(value, (CFKind, DayCount)):
        return str(value)
    if isinstance(value, dict):
        return {key: wire(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [wire(item) for item in value]
    return value


def dated(pairs: list[tuple[dt.date, Money]]) -> list[dict[str, Any]]:
    """``(date, Money)`` tuples as the ``{date, amount}`` rows WASM returns."""
    return [{"date": date.isoformat(), "amount": wire(amount)} for date, amount in pairs]


DECIMAL = re.compile(r"-?\d+(\.\d+)?")


def assert_same(actual: Any, expected: Any, path: str = "$") -> None:
    """Structural equality with a 1e-12 relative allowance on floats."""
    if isinstance(expected, dict):
        assert isinstance(actual, dict), path
        assert sorted(actual) == sorted(expected), path
        for key in expected:
            assert_same(actual[key], expected[key], f"{path}.{key}")
    elif isinstance(expected, list):
        assert isinstance(actual, list), path
        assert len(actual) == len(expected), path
        for index, (left, right) in enumerate(zip(actual, expected, strict=True)):
            assert_same(left, right, f"{path}[{index}]")
    elif isinstance(expected, str) and isinstance(actual, str) and DECIMAL.fullmatch(expected):
        # Exact-decimal text of an f64 result (``Money.amount``): same allowance as a float.
        assert DECIMAL.fullmatch(actual), path
        assert math.isclose(float(actual), float(expected), rel_tol=1e-12, abs_tol=1e-12), (
            f"{path}: {actual} != {expected}"
        )
    elif isinstance(expected, bool) or expected is None or isinstance(expected, str):
        assert actual == expected, path
    else:
        assert isinstance(actual, (int, float)), path
        assert not isinstance(actual, bool), path
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), f"{path}: {actual} != {expected}"


def _amortizing() -> Any:
    return wire(
        builder()
        .principal_exchange(PrincipalExchange.INITIAL_AND_FINAL)
        .amortization(AmortizationSpec.linear_to(usd(400_000)))
        .fixed_cf(fixed())
        .fee(FeeSpec.fixed(dt.date(2025, 7, 15), usd(2500)))
        .add_principal_event(dt.date(2025, 10, 15), dt.date(2025, 10, 17), usd(-100_000), "amortization")
        .build()
    )


def _step_up() -> Any:
    spec = {key: value for key, value in FIXED_SPEC.items() if key != "rate"}
    spec.update({"initial_rate": "0.05", "step_schedule": [["2026-01-15", "0.07"]]})
    return wire(builder().step_up_cf(StepUpCouponSpec.from_json(json.dumps(spec))).build())


def _windows() -> Any:
    switch = dt.date(2026, 1, 15)
    return wire(
        builder()
        .add_fixed_window(ISSUE, switch, fixed(rate="0.04"))
        .add_fixed_window(switch, MATURITY, fixed(rate="0.08"))
        .payment_split_program([(switch, CouponType.PIK), (MATURITY, CouponType.split("0.5", "0.5"))])
        .build()
    )


def _margin_steps() -> Any:
    return [
        wire(builder().float_margin_steps([(dt.date(2026, 1, 15), "250")], floating()).build(market())),
        wire(builder().add_floating_window(ISSUE, MATURITY, floating()).build(market())),
    ]


def _accessors() -> Any:
    schedule = bond()
    return {
        "flows": wire(schedule.get_flows()),
        "coupons": wire(schedule.coupons()),
        "dates": wire(schedule.dates()),
        "notional": wire(schedule.get_notional()),
        "day_count": wire(schedule.get_day_count()),
        "meta": wire(schedule.get_meta()),
    }


def _pv_by_period() -> Any:
    schedule = bond()
    return {
        "default_day_count": wire(schedule.pv_by_period(periods(), market(), "USD-OIS", ISSUE)),
        "act_360": wire(schedule.pv_by_period(periods(), market(), "USD-OIS", ISSUE, DayCount.ACT_360)),
    }


def _ladder() -> Any:
    schedule = bond()
    frame = schedule.calendar_year_ladder([index + 0.5 for index in range(len(schedule.get_flows()))])
    return frame.to_dict("records")


def _from_parts() -> Any:
    schedule = bond()
    flows = schedule.get_flows()
    return [
        wire(CashFlowSchedule.from_parts(flows, schedule.get_notional(), DayCount.THIRTY_360, schedule.get_meta())),
        wire(CashFlowSchedule.from_flows(flows[1:3], Notional.par(1000, "USD"), DayCount.ACT_365F)),
    ]


def _build_cashflow_schedule() -> Any:
    spec = {
        "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
        "issue_date": "2025-01-15",
        "maturity": "2027-01-15",
        "coupon_program": [{"kind": "floating", "spec": FLOATING_SPEC}],
    }
    return wire(cashflows.build_cashflow_schedule(spec, market()))


def _schedule_from_flows() -> Any:
    flows = [(dt.date(2025, 6, 30), usd(1500)), (dt.date(2025, 3, 31), usd(1000))]
    opts = cashflows.ScheduleBuildOpts(notional_hint=usd(100_000))
    return [
        wire(cashflows.schedule_from_dated_flows(flows, "fee", DayCount.ACT_360, opts)),
        wire(cashflows.schedule_from_dated_flows(flows, CFKind.FIXED, DayCount.ACT_360)),
        wire(cashflows.schedule_from_classified_flows(bond().get_flows()[:3], DayCount.THIRTY_360, opts)),
    ]


def _merge() -> Any:
    second = cashflows.schedule_from_dated_flows([(dt.date(2025, 6, 30), usd(1500))], "fee", DayCount.ACT_360)
    return wire(merge_cashflow_schedules([bond(), second], Notional.par(1_000_000, "USD"), DayCount.THIRTY_360))


def _accrual() -> Any:
    schedule = bond()
    compounded = AccrualConfig.from_json(
        json.dumps({"method": "compounded", "ex_coupon": None, "include_pik": True, "frequency": None})
    )
    ex_coupon = AccrualConfig.from_json(
        json.dumps({
            "method": "compounded",
            "ex_coupon": {"days_before_coupon": 7, "calendar_id": None},
            "include_pik": True,
            "frequency": None,
        })
    )
    linear = AccrualIndex.build(schedule)
    with_ex_coupon = AccrualIndex.build(schedule, ex_coupon)
    payment = dt.date(2025, 7, 15)
    return {
        "amount": wire(accrued_interest_amount(schedule, dt.date(2025, 4, 15))),
        "amount_compounded": wire(accrued_interest_amount(schedule, dt.date(2025, 4, 15), compounded)),
        "index": [wire(linear.accrued_at(date)) for date in ("2025-01-15", "2025-04-15", "2025-07-10", "2026-12-31")],
        "index_ex_coupon": [wire(with_ex_coupon.accrued_at(date)) for date in ("2025-04-15", "2025-07-10")],
        "ex_date_calendar_days": wire(ExCouponRule(7).ex_date(payment)),
        "ex_date_business_days": wire(ExCouponRule(7, "usny").ex_date(payment)),
    }


def _aggregation() -> Any:
    flows = [
        (dt.date(2025, 3, 15), usd(100.25)),
        (dt.date(2025, 9, 15), usd(50)),
        (dt.date(2026, 2, 1), Money(75, "EUR")),
        (dt.date(2026, 2, 2), usd(0.1)),
    ]
    aggregation = aggregate_by_period(flows, periods())
    ladder = calendar_year_ladder(
        [dt.date(2025, 3, 15), dt.date(2025, 9, 15), dt.date(2026, 2, 1)],
        ["fixed", "amortization", "fee"],
        [100.25, 50, 75],
        [99, 48.5, 70.125],
    )
    return {
        "by_period": wire(aggregation),
        "get_amount": [
            wire(aggregation.get_amount("2026Q1", "EUR")),
            wire(aggregation.get_amount("2025Q1", "EUR")),
            wire(aggregation.get_amount("nope", "USD")),
        ],
        "checked": wire(aggregate_cashflows_checked(flows[:2], "USD")),
        "ladder": [
            {"year": year, "non_principal": non_principal, "principal": principal, "pv": pv}
            for year, non_principal, principal, pv in ladder
        ],
    }


def _primitives() -> Any:
    flow = CashFlow(dt.date(2025, 7, 15), usd(30_000), "fixed", accrual_factor=0.5, rate=0.06)
    accrual = CashFlowAccrual(dt.date(2025, 1, 15), dt.date(2025, 7, 15), DayCount.THIRTY_360)
    return {
        # WASM returns the serde wire string (``pre_payment``); read Python's from a serialized flow.
        "parse": [
            wire(CashFlow(dt.date(2025, 7, 15), usd(0), CFKind.parse(name)))["kind"]
            for name in ("fixed", "prepayment", "collateral_substitution_out")
        ],
        "interest_like": [
            CFKind.parse(kind).is_interest_like() for kind in ("fixed", "float_reset", "fee", "notional")
        ],
        "principal_like": [
            CFKind.parse(kind).is_principal_like() for kind in ("fixed", "pik", "amortization", "recovery")
        ],
        "cash_settlement": [
            is_cash_settlement_kind(kind) for kind in ("fixed", "pik", "defaulted_notional", "notional")
        ],
        "balance_date": [
            wire(flow.get_balance_date()),
            wire(flow.with_principal_date(dt.date(2025, 7, 11)).get_balance_date()),
        ],
        "with_accrual": wire(flow.with_accrual(accrual)),
        "with_principal_delta": wire(flow.with_principal_delta(usd(-1000))),
    }


def _coupon_type_wire(coupon_type: CouponType) -> Any:
    """``CouponType`` has no ``to_json``; read it back from a spec that embeds it."""
    return json.loads(FixedCouponSpec("0.05", ScheduleParams.semiannual_30360(), coupon_type).to_json())["coupon_type"]


def _rate_spec_field(field: str, value: Any) -> Any:
    """Wire form of a floating-rate enum without ``to_json``, read back from a spec that embeds it."""
    spec = FloatingRateSpec("USD-SOFR", "0", Tenor.parse("3M"), **{field: value})
    return json.loads(spec.to_json())[field]


def _coupon_fee_floating() -> Any:
    undrawn = FeeBase.undrawn(usd(5_000_000))
    return [
        _coupon_type_wire(CouponType.split("0.6", "0.4")),
        wire(undrawn),
        wire(FeeSpec.fixed(dt.date(2025, 3, 1), usd(25_000))),
        wire(FeeSpec.periodic_bp(undrawn, "37.5", Tenor.parse("3M"), DayCount.ACT_360, "usny", "modified_following")),
        _rate_spec_field("fallback", FloatingRateFallback.fixed_rate("0.03")),
        _rate_spec_field("compounding", FloatingLegCompounding.compounded_in_arrears(5)),
        _rate_spec_field("compounding", FloatingLegCompounding.compounded_with_observation_shift(2)),
        _rate_spec_field("compounding", FloatingLegCompounding.compounded_with_rate_cutoff(3)),
        wire(FloatingRateSpec.sofr("150")),
        wire(FloatingRateSpec.sonia("25.5")),
        wire(FloatingRateSpec.euribor_3m("-10")),
    ]


def _default_models() -> Any:
    models = [
        DefaultModelSpec.constant_cdr(0.03),
        DefaultModelSpec.sda(1.5),
        DefaultModelSpec.cdr_2pct(),
        DefaultModelSpec.vector([0.01, 0.02, 0.04]),
        DefaultModelSpec.cumulative_loss([0.5, 1.5, 2.0], 0.4),
        DefaultModelSpec.timing(0.1, [15, 30, 30, 15, 10]),
    ]
    return {"models": wire(models), "mdr": [[model.mdr(month) for month in (1, 12, 31, 90)] for model in models]}


def _prepayment_models() -> Any:
    models = [
        PrepaymentModelSpec.constant_cpr(0.06),
        PrepaymentModelSpec.psa(1.5),
        PrepaymentModelSpec.psa_100(),
        PrepaymentModelSpec.cmbs_with_lockout(60, 0.1),
        PrepaymentModelSpec.abs(0.015),
        PrepaymentModelSpec.vector([0.02, 0.05, 0.08]),
    ]
    return {"models": wire(models), "smm": [[model.smm(month) for month in (1, 12, 31, 90)] for model in models]}


def _recovery_models() -> Any:
    flat = RecoveryModelSpec.from_json(json.dumps({"rate": 0.4, "recovery_lag": 6}))
    vector = flat.with_severity_vector([0.7, 0.6, 0.55])
    return {
        "vector": wire(vector),
        "recovery_rate": [[model.recovery_rate(month) for month in (0, 1, 2, 12)] for model in (flat, vector)],
    }


def _materialize_fixings() -> Any:
    meta = CashFlowMeta.from_json(
        json.dumps({
            "projected_fixings": [{"series_id": "FIXING:USD-SOFR", "date": "2025-01-03", "value": 0.03}],
            "representation": "contractual",
            "calendar_ids": [],
            "commitment": None,
        })
    )
    schedule = CashFlowSchedule.from_flows([], Notional.par(1, "USD"), DayCount.ACT_360, meta)
    return wire(materialize_fixings(market(), [schedule], "2025-01-02", "2025-01-06"))["series"]


CASES: dict[str, Callable[[], Any]] = {
    "builder.fixed_bond": lambda: wire(bond()),
    "builder.amortizing_with_fee_and_event": _amortizing,
    "builder.step_up": _step_up,
    "builder.windows_and_pik_program": _windows,
    "builder.payment_window": lambda: wire(
        builder().fixed_cf(fixed()).add_payment_window(ISSUE, dt.date(2026, 1, 15), CouponType.PIK).build()
    ),
    "builder.floating_with_market": lambda: wire(builder().floating_cf(floating()).build(market())),
    "builder.fixed_to_float": lambda: wire(
        builder()
        .fixed_to_float(dt.date(2026, 1, 15), fixed(frequency={"count": 3, "unit": "months"}), floating())
        .build(market())
    ),
    "builder.float_margin_steps_and_floating_window": _margin_steps,
    "schedule.accessors": _accessors,
    "schedule.wal": lambda: bond().wal(ISSUE),
    "schedule.outstanding_by_date": lambda: dated(bond().outstanding_by_date()),
    "schedule.scale_amounts": lambda: wire(bond().scale_amounts(-0.25)),
    "schedule.with_representation": lambda: wire(bond().with_representation("projected")),
    "schedule.with_notional": lambda: wire(bond().with_notional(Notional.par(2_000_000, "USD"))),
    "schedule.pv_by_period": _pv_by_period,
    "schedule.calendar_year_ladder": _ladder,
    "schedule.from_parts_and_from_flows": _from_parts,
    "build_cashflow_schedule": _build_cashflow_schedule,
    "dated_flows": lambda: dated(cashflows.dated_flows(bond())),
    "schedule_from_flows": _schedule_from_flows,
    "merge_cashflow_schedules": _merge,
    "accrual": _accrual,
    "aggregation": _aggregation,
    "primitives": _primitives,
    "specs.amortization": lambda: wire([
        AmortizationSpec.linear_to(usd(250_000)),
        AmortizationSpec.step_remaining([(dt.date(2026, 1, 15), usd(750_000)), (dt.date(2027, 1, 15), usd(0))]),
        AmortizationSpec.percent_of_original_per_period(0.05),
        AmortizationSpec.percent_of_remaining_per_period(0.1),
        AmortizationSpec.linear_between(dt.date(2025, 7, 15), dt.date(2027, 1, 15)),
        AmortizationSpec.custom_principal([(dt.date(2026, 1, 15), usd(250_000))]),
    ]),
    "specs.coupon_fee_floating": _coupon_fee_floating,
    "specs.notional": lambda: [wire(Notional.par(1_000_000.005, "USD")), Notional.par(5, "JPY").currency().code],
    "specs.default_model": _default_models,
    "specs.prepayment_model": _prepayment_models,
    "specs.recovery_model": _recovery_models,
    "specs.schedule_params": lambda: wire([
        ScheduleParams.quarterly_act360(),
        ScheduleParams.semiannual_30360(),
        ScheduleParams.annual_actact(),
        ScheduleParams.usd_sofr_swap(),
        ScheduleParams.usd_corporate_bond(),
        ScheduleParams.usd_treasury(),
        ScheduleParams.eur_estr_swap(),
        ScheduleParams.eur_gov_bond(),
        ScheduleParams.gbp_sonia_swap(),
        ScheduleParams.jpy_tona_swap(),
    ]),
    "materialize_fixings": _materialize_fixings,
}


def test_python_cases_cover_the_golden_file() -> None:
    assert sorted(CASES) == sorted(GOLDEN)


@pytest.mark.parametrize("name", sorted(CASES))
def test_python_matches_the_wasm_golden(name: str) -> None:
    assert_same(CASES[name](), GOLDEN[name])


def test_period_aggregation_get_amount_is_the_rust_lookup() -> None:
    aggregation = aggregate_by_period([(dt.date(2025, 3, 15), usd(100.25))], periods())
    assert aggregation.get_amount("2025Q1", "USD").amount == 100.25
    assert aggregation.get_amount("2025Q1", "EUR") is None
    assert aggregation.get_amount("2025Q2", "USD") is None
    assert not hasattr(aggregation, "get")
