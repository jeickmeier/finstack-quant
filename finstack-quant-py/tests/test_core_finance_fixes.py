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
def test_future_price_quotes_interpolate_from_spot(kind: str) -> None:
    curve = PriceCurve("SPOT", "2025-01-01", [(1.0, 80.0), (2.0, 100.0)], spot_price=75.0, kind=kind)
    for reconstructed in (curve, PriceCurve.from_json(curve.to_json())):
        assert reconstructed.price(0.0) == 75.0
        assert reconstructed.price(0.5) == pytest.approx(77.5)
        assert reconstructed.price(1e-6) == pytest.approx(75.000005)
        assert reconstructed.price(1.0) == 80.0


def test_price_quote_at_zero_must_agree_with_spot() -> None:
    with pytest.raises(ValueError, match="spot_price"):
        PriceCurve("SPOT", "2025-01-01", [(0.0, 80.0), (1.0, 100.0)], spot_price=75.0)


def test_future_cpi_quotes_interpolate_from_base_cpi() -> None:
    curve = InflationCurve("CPI", "2025-01-01", 300.0, [(1.0, 330.0), (2.0, 345.0)])
    for reconstructed in (curve, InflationCurve.from_json(curve.to_json())):
        assert reconstructed.cpi(0.0) == 300.0
        assert reconstructed.cpi(0.5) == pytest.approx(math.sqrt(300.0 * 330.0))
        assert reconstructed.cpi(1e-6) == pytest.approx(300.0 * 1.1**1e-6)
        assert reconstructed.cpi(1.0) == pytest.approx(330.0)


def test_monotone_convex_flat_first_strip_stays_flat() -> None:
    curve = DiscountCurve(
        "USD-OIS", "2025-01-01", [(0.0, 1.0), (1.0, 1.0), (2.0, math.exp(-0.005)), (3.0, math.exp(-0.035))]
    )
    for t in (0.1, 1 / 3, 0.5, 0.9):
        assert curve.df(t) == pytest.approx(1.0, abs=1e-14)


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
    with pytest.raises(ValueError, match="no convergence"):
        mwr_xirr([("2025-01-01", 0.0), *flows])
