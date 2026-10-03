"""Financial validation and lifecycle contracts at the Python boundary."""

from datetime import date
import json
import math

import pytest

from finstack_quant.core.dates import Schedule
from finstack_quant.core.market_data import HazardCurve, InflationCurve, MarketContext, PriceCurve


def cpi_curve() -> InflationCurve:
    return InflationCurve("CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])


@pytest.mark.parametrize(("t1", "t2"), [(1.0, 1.0), (1.0, 0.0), (math.nan, 1.0), (0.0, math.inf)])
def test_invalid_inflation_interval_raises_value_error(t1: float, t2: float) -> None:
    with pytest.raises(ValueError, match="finite t1 < t2"):
        cpi_curve().inflation_rate(t1, t2)


@pytest.mark.parametrize("base_cpi", [math.nan, math.inf, 0.0, -300.0, 301.0])
def test_invalid_base_cpi_is_rejected(base_cpi: float) -> None:
    with pytest.raises(ValueError, match="base_cpi"):
        InflationCurve("CPI", "2025-01-01", base_cpi, [(0.0, 300.0), (1.0, 306.0)])


def test_curve_deserialization_preserves_validation() -> None:
    state = json.loads(cpi_curve().to_json())
    state["base_cpi"] = -300.0
    with pytest.raises(ValueError, match="base_cpi"):
        InflationCurve.from_json(json.dumps(state))


def test_hazard_interpolation_rejects_inconsistent_survival() -> None:
    with pytest.raises(TypeError, match="interp"):
        HazardCurve("HZ", "2025-01-01", [(1.0, 0.02), (2.0, 1.0)], recovery_rate=0.4, interp="linear")


@pytest.mark.parametrize("rule", ["weekly", "cds", "imm"])
def test_eom_rejects_incompatible_schedules(rule: str) -> None:
    builder = Schedule.builder("2025-01-01", "2025-12-20").end_of_month(True)
    if rule == "weekly":
        builder = builder.frequency("1W")
    elif rule == "cds":
        builder = builder.cds_imm()
    else:
        builder = builder.imm()
    with pytest.raises(ValueError, match="end-of-month"):
        builder.build()


def test_market_roll_preserves_price_and_cpi_near_origin() -> None:
    cpi = cpi_curve()
    price = PriceCurve("PRICE", "2025-01-01", [(0.0, 100.0), (1.0, 110.0), (2.0, 120.0)])
    market = MarketContext()
    market.insert(cpi)
    market.insert(price)
    rolled = market.roll_forward(183)
    dt = (date(2025, 7, 3) - date(2025, 1, 1)).days / 365.0
    assert rolled.get_inflation_curve("CPI").cpi(1e-8) == pytest.approx(cpi.cpi(dt + 1e-8))
    assert rolled.get_price_curve("PRICE").price(1e-8) == pytest.approx(price.price(dt + 1e-8))


# CORE-004: one day-count parser (DayCount | str through DayCount::parse).
@pytest.mark.parametrize("day_count", ["act_360", "ACT/360", "Act/360"])
def test_curve_constructors_accept_daycount_objects_and_lenient_names(day_count: str) -> None:
    from finstack_quant.core.dates import DayCount
    from finstack_quant.core.market_data import DiscountCurve, ForwardCurve
    from finstack_quant.models.credit import MertonModel

    for dc in (day_count, DayCount.ACT_360):
        knots = [(0.0, 1.0), (1.0, 0.95)]
        assert DiscountCurve("D", "2025-01-01", knots, day_count=dc).day_count == "act_360"
        assert DiscountCurve.from_dates("D", "2025-01-01", [("2026-01-01", 0.95)], dc).day_count == "act_360"
        fwd = ForwardCurve("F", 0.25, "2025-01-01", [(0.0, 0.03), (1.0, 0.035)], day_count=dc)
        assert fwd.day_count == "act_360"
        hz = HazardCurve("H", "2025-01-01", [(1.0, 0.02)], recovery_rate=0.4, day_count=dc)
        assert hz.day_count == "act_360"
        cpi = InflationCurve("CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0)], day_count=dc)
        assert cpi.day_count == "act_360"
        price = PriceCurve("P", "2025-01-01", [(0.0, 100.0), (1.0, 101.0)], day_count=dc)
        assert price.day_count == "act_360"
        merton = MertonModel(100.0, 0.2, 80.0, 0.03)
        assert merton.to_hazard_curve("M", "2025-01-01", [1.0, 2.0], 0.4, dc).day_count == "act_360"


def test_curve_day_count_rejects_non_daycount_types() -> None:
    from finstack_quant.core.market_data import DiscountCurve

    with pytest.raises(TypeError, match="DayCount or str"):
        DiscountCurve("D", "2025-01-01", [(0.0, 1.0), (1.0, 0.95)], day_count=360)


# CORE-005: ScheduleBuilder IMM setters follow Rust ScheduleBuilder semantics.
def test_schedule_builder_cds_imm_later_frequency_wins_and_to_spec_matches_build() -> None:
    builder = Schedule.builder("2025-01-15", "2026-01-15").cds_imm().frequency("1M")
    spec = builder.to_spec()
    assert spec["frequency"] == {"count": 1, "unit": "months"}
    assert spec["stub"] == "short_back"
    dates = [d.isoformat() for d in builder.build().dates]
    assert dates[:3] == ["2024-12-20", "2025-01-20", "2025-02-20"]
    assert [d.isoformat() for d in Schedule.from_spec(spec).dates] == dates


@pytest.mark.parametrize("mode", ["cds_imm", "imm"])
def test_schedule_builder_imm_modes_report_quarterly_short_back(mode: str) -> None:
    builder = getattr(Schedule.builder("2025-01-15", "2026-01-15").frequency("1M"), mode)()
    spec = builder.to_spec()
    assert spec["frequency"] == {"count": 3, "unit": "months"}
    assert spec["stub"] == "short_back"
    assert [d.isoformat() for d in Schedule.from_spec(spec).dates] == [d.isoformat() for d in builder.build().dates]


# CORE-007: MarketContext len/bool come from Rust MarketContext::len/is_empty.
def test_market_context_len_and_bool_count_fx_collateral_and_dividends() -> None:
    from finstack_quant.core.currency import Currency
    from finstack_quant.core.market_data import FxMatrix

    empty = MarketContext()
    assert (len(empty), empty.is_empty(), bool(empty)) == (0, True, False)

    fx = FxMatrix()
    fx.set_quote(Currency("EUR"), Currency("USD"), 1.1)
    fx_only = MarketContext().insert_fx(fx)
    assert (len(fx_only), fx_only.is_empty(), bool(fx_only)) == (1, False, True)
    assert (fx_only or MarketContext()) is fx_only

    collateral_only = MarketContext().map_collateral("USD-CSA", "USD-OIS")
    assert (len(collateral_only), collateral_only.is_empty(), bool(collateral_only)) == (1, False, True)

    state = json.loads(MarketContext().to_json())
    state["dividends"] = [{"id": "AAPL-DIV", "underlying": None, "events": [], "currency": None}]
    dividends_only = MarketContext.from_json(json.dumps(state))
    assert (len(dividends_only), dividends_only.is_empty(), bool(dividends_only)) == (1, False, True)


# CORE-008: CreditIndexData issuer-level inputs and accessors.
def test_credit_index_data_issuer_curves_recoveries_and_weights() -> None:
    from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData

    index_curve = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
    issuer_a = HazardCurve.flat("A-HZD", "2025-01-01", 0.02, 0.3)
    base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])

    homogeneous = CreditIndexData(2, 0.4, index_curve, base_corr)
    assert not homogeneous.has_issuer_curves()
    assert homogeneous.issuer_ids() == []
    assert homogeneous.get_issuer_weight("A") == 0.5
    assert homogeneous.get_issuer_curve("A").id == "CDX-IG"

    data = CreditIndexData(
        2,
        0.4,
        index_curve,
        base_corr,
        issuer_curves={"A": issuer_a, "B": index_curve},
        issuer_recovery_rates={"A": 0.3},
        issuer_weights={"A": 0.25, "B": 0.75},
    )
    assert data.has_issuer_curves()
    assert data.issuer_ids() == ["A", "B"]
    assert data.get_issuer_curve("A").id == "A-HZD"
    assert data.get_issuer_curve("unknown").id == "CDX-IG"
    assert (data.get_issuer_recovery("A"), data.get_issuer_recovery("B")) == (0.3, 0.4)
    assert (data.get_issuer_weight("A"), data.get_issuer_weight("B")) == (0.25, 0.75)

    ctx = MarketContext().insert(index_curve).insert(issuer_a).insert(base_corr)
    ctx = ctx.insert_credit_index("CDX-IG", data)
    stored = json.loads(ctx.to_json())["credit_indices"][0]
    assert stored["issuer_credit_curve_ids"] == {"A": "A-HZD", "B": "CDX-IG"}
    assert ctx.get_credit_index("CDX-IG").issuer_ids() == ["A", "B"]

    with pytest.raises(ValueError, match="complete coverage requires exactly num_constituents=3"):
        CreditIndexData(3, 0.4, index_curve, base_corr, issuer_curves={"A": issuer_a})
    with pytest.raises(ValueError, match="issuer_weights requires issuer_credit_curves"):
        CreditIndexData(2, 0.4, index_curve, base_corr, issuer_weights={"A": 1.0})
