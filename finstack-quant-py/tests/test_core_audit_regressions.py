"""Financial validation and lifecycle contracts at the Python boundary."""

from datetime import date
import json
import math

import pytest

from finstack_quant.core.dates import Schedule
from finstack_quant.core.market_data import DiscountCurve, HazardCurve, InflationCurve, MarketContext, PriceCurve


def cpi_curve() -> InflationCurve:
    return InflationCurve(
        "CPI",
        "2025-01-01",
        [(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)],
        day_count="act_365f",
        interp="log_linear",
        extrapolation="error",
    )


@pytest.mark.parametrize(("t1", "t2"), [(1.0, 1.0), (1.0, 0.0), (math.nan, 1.0), (0.0, math.inf)])
def test_invalid_inflation_interval_raises_value_error(t1: float, t2: float) -> None:
    with pytest.raises(ValueError, match=r"finite 0\.0 <= t1 < t2"):
        cpi_curve().inflation_rate(t1, t2)


@pytest.mark.parametrize("base_cpi", [math.nan, math.inf, 0.0, -300.0])
def test_invalid_base_cpi_is_rejected(base_cpi: float) -> None:
    with pytest.raises(ValueError, match=r"positive|finite"):
        InflationCurve(
            "CPI",
            "2025-01-01",
            [(0.0, base_cpi), (1.0, 306.0)],
            day_count="act_365f",
            interp="log_linear",
            extrapolation="error",
        )


def test_curve_deserialization_preserves_validation() -> None:
    state = json.loads(cpi_curve().to_json())
    state["knot_points"][0][1] = -300.0
    with pytest.raises(ValueError, match="positive"):
        InflationCurve.from_json(json.dumps(state))


def test_hazard_interpolation_rejects_inconsistent_survival() -> None:
    with pytest.raises(TypeError, match="interp"):
        HazardCurve(
            "HZ",
            "2025-01-01",
            [(1.0, 0.02), (2.0, 1.0)],
            recovery_rate=0.4,
            day_count="act_365f",
            interp="linear",
        )


def test_eom_rejects_incompatible_schedules() -> None:
    builder = (
        Schedule.builder("2025-01-31", "2025-12-31").frequency("1M").stub_rule("none").grid("end_of_month").unadjusted()
    )
    with pytest.raises(ValueError, match="end-of-month"):
        builder.frequency("1W").build()


def test_market_condition_keeps_price_and_cpi_frozen_on_original_clock() -> None:
    cpi = cpi_curve()
    price = PriceCurve("PRICE", "2025-01-01", [(0.0, 100.0), (1.0, 110.0), (2.0, 120.0)])
    market = MarketContext()
    market.insert(cpi)
    market.insert(price)
    conditioned = market.condition("2025-07-03")
    assert conditioned.get_inflation_curve("CPI").base_date == date(2025, 1, 1)
    assert conditioned.get_inflation_curve("CPI").cpi(1e-8) == pytest.approx(cpi.cpi(1e-8))
    assert conditioned.get_price_curve("PRICE").price(1e-8) == pytest.approx(price.price(1e-8))


# CORE-004: one day-count parser (DayCount | str through the canonical DayCount::from_str).
@pytest.mark.parametrize("day_count", ["act_360", " ACT_360 ", "Act_360"])
def test_curve_constructors_accept_daycount_objects_and_normalized_names(day_count: str) -> None:
    from finstack_quant.core.dates import DayCount
    from finstack_quant.core.market_data import DiscountCurve, ForwardCurve
    from finstack_quant.models.credit import MertonModel

    for dc in (day_count, DayCount.ACT_360):
        knots = [(0.0, 1.0), (1.0, 0.95)]
        assert (
            DiscountCurve(
                "D",
                "2025-01-01",
                knots,
                interp="monotone_convex",
                extrapolation="extend",
                day_count=dc,
            ).day_count
            == "act_360"
        )
        assert (
            DiscountCurve.from_dates(
                "D",
                "2025-01-01",
                [("2026-01-01", 0.95)],
                dc,
                "monotone_convex",
                "extend",
            ).day_count
            == "act_360"
        )
        fwd = ForwardCurve("F", "2025-01-01", [(1.0, 0.965)], interp="log_linear", extrapolation="extend", day_count=dc)
        assert fwd.day_count == "act_360"
        hz = HazardCurve("H", "2025-01-01", [(1.0, 0.02)], recovery_rate=0.4, day_count=dc)
        assert hz.day_count == "act_360"
        cpi = InflationCurve(
            "CPI", "2025-01-01", [(0.0, 300.0), (1.0, 306.0)], day_count=dc, interp="log_linear", extrapolation="error"
        )
        assert cpi.day_count == "act_360"
        price = PriceCurve("P", "2025-01-01", [(0.0, 100.0), (1.0, 101.0)], day_count=dc)
        assert price.day_count == "act_360"
        merton = MertonModel(100.0, 0.2, 80.0, 0.03)
        assert merton.to_hazard_curve("M", "2025-01-01", [1.0, 2.0], 0.4, dc).day_count == "act_360"


def test_curve_day_count_rejects_non_daycount_types() -> None:
    from finstack_quant.core.market_data import DiscountCurve

    with pytest.raises(TypeError, match="DayCount or str"):
        DiscountCurve(
            "D",
            "2025-01-01",
            [(0.0, 1.0), (1.0, 0.95)],
            interp="monotone_convex",
            extrapolation="extend",
            day_count=360,
        )


def test_schedule_builder_grid_selection_keeps_frequency_and_stub() -> None:
    # Record CORE-CONVENTIONS-008 #2: a grid selector never resets other choices.
    builder = Schedule.builder("2025-03-19", "2026-03-18").frequency("6M").stub_rule("short_back").unadjusted()
    spec = builder.grid("quarterly_imm").to_spec()
    assert (spec["frequency"], spec["stub"], spec["grid"]) == (
        {"count": 6, "unit": "months"},
        "short_back",
        "quarterly_imm",
    )
    assert [d.isoformat() for d in builder.build().dates] == ["2025-03-19", "2025-09-17", "2026-03-18"]


def test_schedule_builder_requires_every_choice() -> None:
    with pytest.raises(ValueError, match="frequency, stub, grid, payment convention"):
        Schedule.builder("2025-01-15", "2026-01-15").build()
    spec = Schedule.builder("2025-01-15", "2026-01-15").frequency("3M").stub_rule("none").grid("natural").unadjusted()
    payload = spec.to_spec()
    payload["business_day_convention"] = "following"
    with pytest.raises(ValueError, match="needs a calendar"):
        Schedule.from_spec(payload)


# CORE-007: MarketContext len/bool come from Rust MarketContext::len/is_empty.
def test_market_context_len_and_bool_count_fx_collateral_and_dividends() -> None:
    from finstack_quant.core.currency import Currency
    from finstack_quant.core.market_data import FxMatrix

    empty = MarketContext()
    assert (len(empty), empty.is_empty(), bool(empty)) == (0, True, False)

    fx = FxMatrix(None)
    fx.set_quotes([(Currency("EUR"), Currency("USD"), 1.1)])
    fx_only = MarketContext().insert_fx(fx)
    assert (len(fx_only), fx_only.is_empty(), bool(fx_only)) == (1, False, True)
    assert (fx_only or MarketContext()) is fx_only

    state = json.loads(MarketContext().to_json())
    state["dividends"] = [
        {"id": "AAPL-DIV", "underlying": "AAPL", "share_basis": "AAPL", "events": [], "currency": "USD"}
    ]
    dividends_only = MarketContext.from_json(json.dumps(state))
    assert (len(dividends_only), dividends_only.is_empty(), bool(dividends_only)) == (1, False, True)


# CORE-008: CreditIndexData issuer-level inputs and accessors.
def test_credit_index_data_issuer_curves_recoveries_and_weights() -> None:
    from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData

    index_curve = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4, "act_365f")
    issuer_a = HazardCurve.flat("A-HZD", "2025-01-01", 0.02, 0.3, "act_365f")
    base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])

    homogeneous = CreditIndexData(2, 0.4, index_curve, base_corr)
    assert not homogeneous.has_issuer_curves()
    assert homogeneous.issuer_ids() == []
    assert homogeneous.num_constituents == 2
    assert homogeneous.recovery_rate == 0.4
    with pytest.raises(KeyError, match="A"):
        homogeneous.get_issuer_curve("A")

    data = CreditIndexData(
        2,
        0.4,
        index_curve,
        base_corr,
        issuer_curves={"A": issuer_a, "B": index_curve},
        issuer_recovery_rates={"A": 0.3},
        issuer_weights={"A": 0.9, "B": 0.1},
    )
    assert data.has_issuer_curves()
    assert data.issuer_ids() == ["A", "B"]
    assert data.get_issuer_curve("A").id == "A-HZD"
    for lookup in (data.get_issuer_curve, data.get_issuer_weight, data.get_issuer_recovery):
        with pytest.raises(KeyError, match="C"):
            lookup("C")
    assert (data.get_issuer_recovery("A"), data.get_issuer_recovery("B")) == (0.3, 0.4)
    assert (data.get_issuer_weight("A"), data.get_issuer_weight("B")) == (0.9, 0.1)

    ctx = MarketContext().insert(index_curve).insert(issuer_a).insert(base_corr)
    ctx = ctx.insert_credit_index("CDX-IG", data)
    stored = json.loads(ctx.to_json())["credit_indices"][0]
    assert stored["issuer_credit_curve_ids"] == {"A": "A-HZD", "B": "CDX-IG"}
    assert ctx.get_credit_index("CDX-IG").issuer_ids() == ["A", "B"]

    with pytest.raises(ValueError, match="complete coverage requires exactly num_constituents=3"):
        CreditIndexData(3, 0.4, index_curve, base_corr, issuer_curves={"A": issuer_a})
    with pytest.raises(ValueError, match="issuer_weights requires issuer_credit_curves"):
        CreditIndexData(2, 0.4, index_curve, base_corr, issuer_weights={"A": 1.0})


def test_slice9_curve_contract_regressions() -> None:
    pre_base = date(2024, 12, 31)
    negative_rate = DiscountCurve(
        "CHF-OIS",
        "2025-01-01",
        [(0.0, 1.0), (1.0, 1.002)],
        interp="linear",
        extrapolation="extend",
        day_count="act_365f",
    )
    assert negative_rate.forward(0.0, 1.0) < 0.0
    with pytest.raises(ValueError, match="zero_rate_on_date requires mapped time t > 0"):
        negative_rate.zero_rate(pre_base, "continuous")

    with pytest.raises(ValueError, match="t > 0"):
        HazardCurve("HZ-ZERO", "2025-01-01", [(0.0, 0.02)], recovery_rate=0.4, day_count="act_365f")

    hz = HazardCurve("HZ-MULTI", "2025-01-01", [(1.0, 0.02), (3.0, 0.03)], recovery_rate=0.4, day_count="act_365f")
    with pytest.raises(ValueError, match="hazard_rate requires finite non-negative time"):
        hz.hazard_rate(float("nan"))

    conditioned = MarketContext().insert(hz).condition(date(2030, 1, 1))
    assert conditioned.get_hazard("HZ-MULTI").hazard_rate(0.5) == pytest.approx(0.03, rel=1e-12)


def test_dividend_snapshot_preserves_entitlement_payment_and_currency() -> None:
    state = json.loads(MarketContext().to_json())
    event = {
        "event_id": "Q1",
        "ex_date": "2025-04-01",
        "pay_date": "2025-07-01",
        "amount": {"amount": "10", "currency": "USD"},
    }
    state["dividends"] = [
        {"id": "EQ-DIV", "underlying": "EQ", "share_basis": "EQ", "currency": "USD", "events": [event]}
    ]
    restored = json.loads(MarketContext.from_json(json.dumps(state)).to_json())
    assert restored["dividends"] == state["dividends"]
    event["amount"]["currency"] = "JPY"
    with pytest.raises(ValueError, match="validation failed"):
        MarketContext.from_json(json.dumps(state))


def test_one_one_is_rejected_for_annualized_market_clocks() -> None:
    from finstack_quant.core.dates import VolatilityClock

    with pytest.raises(ValueError, match="annualized"):
        DiscountCurve.flat("USD", "2025-01-01", 0.05, "one_one")
    with pytest.raises(ValueError, match="annualized"):
        VolatilityClock.from_json('{"reference_date":"2025-01-01","day_count":"one_one"}')

    curve = DiscountCurve.flat("USD", "2025-01-01", 0.05, "act_365f")
    for target in [date(2025, 1, 2), date(2026, 1, 1), date(2035, 1, 1)]:
        years = (target - date(2025, 1, 1)).days / 365
        assert curve.df(target) == pytest.approx(math.exp(-0.05 * years))
