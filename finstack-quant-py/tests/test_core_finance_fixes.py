"""Public Python regressions for the core quantitative finance fixes."""

from datetime import date
import math

import pytest

from finstack_quant.core.dates import DayCount, DayCountContext, HolidayCalendar, add_business_days
from finstack_quant.core.market_data import DiscountCurve, InflationCurve, PriceCurve
from finstack_quant.portfolio import mwr_xirr


def test_icma_rejects_an_adjusted_reference_that_changes_the_nominal_roll() -> None:
    context = DayCountContext(frequency="6M", coupon_period=("2025-01-15", "2025-07-16"))
    with pytest.raises(ValueError, match="unadjusted"):
        DayCount.ACT_ACT_ISMA.year_fraction("2024-07-15", "2025-01-15", context)


@pytest.mark.parametrize("kind", ["price", "vol_index"])
def test_future_price_quotes_preserve_supplied_pillars(kind: str) -> None:
    curve = PriceCurve({
        "id": "SPOT",
        "base_date": "2025-01-01",
        "knots": [(1.0, 80.0), (2.0, 100.0)],
        "kind": kind,
        "day_count": "act_365f",
        "interp": "linear",
        "extrapolation": "error",
    })
    for reconstructed in (curve, PriceCurve.from_json(curve.to_json())):
        assert reconstructed.knots == [1.0, 2.0]
        assert reconstructed.price(1.5) == 90.0
        with pytest.raises(ValueError, match="below minimum knot"):
            reconstructed.price(0.0)


def test_business_day_price_curve_uses_registered_calendar() -> None:
    curve = PriceCurve({
        "id": "P",
        "base_date": "2025-01-03",
        "knots": [(0.0, 0.0), (1.0, 252.0)],
        "kind": "price",
        "day_count": "bus_252",
        "calendar_id": "weekends_only",
        "interp": "linear",
        "extrapolation": "error",
    })
    assert curve.calendar_id == "weekends_only"
    assert curve.price("2025-01-06") == pytest.approx(1.0)


def test_future_cpi_quotes_interpolate_from_explicit_origin() -> None:
    curve = InflationCurve(
        "CPI",
        "2025-01-01",
        [(0.0, 300.0), (1.0, 330.0), (2.0, 345.0)],
        day_count="act_365f",
        interp="log_linear",
        extrapolation="error",
    )
    for reconstructed in (curve, InflationCurve.from_json(curve.to_json())):
        assert reconstructed.cpi(0.0) == 300.0
        assert reconstructed.cpi(0.5) == pytest.approx(math.sqrt(300.0 * 330.0))
        assert reconstructed.cpi(1e-6) == pytest.approx(300.0 * 1.1**1e-6)
        assert reconstructed.cpi(1.0) == pytest.approx(330.0)


@pytest.mark.parametrize("t", [-0.1, math.nan, math.inf, 3.0])
def test_inflation_queries_reject_invalid_coordinates(t: float) -> None:
    curve = InflationCurve(
        "CPI",
        "2025-01-01",
        [(0.0, 300.0), (1.0, 330.0)],
        day_count="act_365f",
        interp="log_linear",
        extrapolation="error",
    )
    with pytest.raises(ValueError, match=r"non-negative|exceeds maximum knot"):
        curve.cpi(t)


def test_inflation_curve_requires_explicit_source_conventions_and_origin() -> None:
    with pytest.raises(TypeError):
        InflationCurve("CPI", "2025-01-01", [(0.0, 300.0), (1.0, 330.0)])  # type: ignore[call-arg]
    with pytest.raises(ValueError, match="origin"):
        InflationCurve(
            "CPI",
            "2025-01-01",
            [(1.0, 330.0), (2.0, 345.0)],
            day_count="act_365f",
            interp="log_linear",
            extrapolation="error",
        )


def test_monotone_convex_flat_first_strip_stays_flat() -> None:
    curve = DiscountCurve(
        "USD-OIS",
        "2025-01-01",
        [(0.0, 1.0), (1.0, 1.0), (2.0, math.exp(-0.005)), (3.0, math.exp(-0.035))],
        interp="monotone_convex",
        extrapolation="extend",
        day_count="act_365f",
    )
    assert curve.df(0.0) == 1.0
    assert curve.df(1.0) == 1.0
    for t in (0.1, 1 / 3, 0.5, 0.9):
        expected = math.exp(0.00125 * t * (1.0 - t) * (1.0 + t))
        assert curve.df(t) == pytest.approx(expected, abs=1e-14)


@pytest.mark.parametrize("code", ["hkex", "hkhk"])
def test_hong_kong_holidays_and_easter_settlement(code: str) -> None:
    calendar = HolidayCalendar(code)
    for holiday in ("2026-04-03", "2026-04-06", "2026-04-07", "2026-05-25", "2026-06-19", "2026-10-19"):
        assert not calendar.is_business_day(holiday)
    assert add_business_days("2026-04-02", 1, calendar) == date(2026, 4, 8)


@pytest.mark.parametrize("code", ["jpx", "jpto"])
def test_japanese_substitute_and_citizen_holidays(code: str) -> None:
    calendar = HolidayCalendar(code)
    assert not calendar.is_business_day("2026-05-06")
    assert not calendar.is_business_day("2026-09-22")
    assert add_business_days("2026-05-05", 1, calendar) == date(2026, 5, 7)


def test_zero_dated_cashflows_do_not_change_xirr() -> None:
    flows = [("2030-01-01", -100.0), ("2031-01-01", 110.0)]
    assert mwr_xirr([("2025-01-01", 0.0), *flows]) == pytest.approx(mwr_xirr(flows), abs=1e-12)


@pytest.mark.parametrize("amounts", [(-100.0, 100.0, -100.0), (-1.0, 10000.0, -100000000.0)])
def test_rootless_xirr_is_rejected_with_earlier_zero(amounts: tuple[float, float, float]) -> None:
    flows = [("2030-01-01", amounts[0]), ("2031-01-01", amounts[1]), ("2032-01-01", amounts[2])]
    with pytest.raises(ValueError, match="exactly one sign change"):
        mwr_xirr([("2025-01-01", 0.0), *flows])


@pytest.mark.parametrize("policy", ["extend", "error"])
def test_vol_index_price_rejects_invalid_tail(policy: str) -> None:
    knots = [(0.0, 30.0), (0.25, 20.0)]
    curve = PriceCurve({
        "id": "VIX",
        "base_date": "2025-01-01",
        "knots": knots,
        "kind": "vol_index",
        "extrapolation": policy,
        "day_count": "act_365f",
        "interp": "linear",
    })
    assert [curve.price(0), curve.price(0.25)] == [30, 20]
    for query in (1.0, "2026-01-01"):
        with pytest.raises(ValueError, match="Invalid vol_index level" if policy == "extend" else "maximum knot"):
            curve.price(query)
    commodity = PriceCurve({
        "id": "OIL",
        "base_date": "2025-01-01",
        "knots": knots,
        "extrapolation": "extend",
        "kind": "price",
        "day_count": "act_365f",
        "interp": "linear",
    })
    assert commodity.price(1) == pytest.approx(-10)
