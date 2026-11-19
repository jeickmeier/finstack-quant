"""Native QuantLib interest-rate option golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    central_difference,
    chained_simple_discounts,
    constant_vol_surface,
    flat_discount_curve,
    forward_curve,
    market_snapshot,
    metadata,
    projection_knots,
    ql_date,
    ql_projection_curve,
    tolerance,
)

ANALYTICAL_RATE_TOLERANCE = 1e-7


def _optionlet_projection(forward_rate: float) -> tuple[list[ql.Date], list[float]]:
    dates = [ql_date(VALUATION_DATE), ql.Date(30, 7, 2026), ql.Date(30, 10, 2026)]
    return dates, chained_simple_discounts(dates, forward_rate, ql.Actual360())


def _quantlib_optionlet(
    *,
    rate_option_type: str,
    forward_rate: float,
    discount_rate: float,
    rate_shift: float,
    strike: float,
    volatility: float,
    normal: bool,
) -> ql.CapFloor:
    evaluation_date = ql_date(VALUATION_DATE)
    projection_dates, projection_discounts = _optionlet_projection(forward_rate)
    start, maturity = projection_dates[1:]
    option_day_count = ql.Actual365Fixed()
    accrual_day_count = ql.Actual360()
    ql.Settings.instance().evaluationDate = evaluation_date

    projection = ql_projection_curve(projection_dates, projection_discounts, shift=rate_shift)
    discount = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, discount_rate + rate_shift, option_day_count, ql.Continuous)
    )
    index = ql.IborIndex(
        "USD-TERM-3M",
        ql.Period(3, ql.Months),
        0,
        ql.USDCurrency(),
        ql.NullCalendar(),
        ql.Unadjusted,
        False,
        accrual_day_count,
        projection,
    )
    schedule = ql.Schedule(
        start,
        maturity,
        ql.Period(3, ql.Months),
        ql.NullCalendar(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Forward,
        False,
    )
    leg = ql.IborLeg(
        [10_000_000.0],
        schedule,
        index,
        accrual_day_count,
        ql.Unadjusted,
        [0],
    )
    optionlet: ql.CapFloor = ql.Cap(leg, [strike]) if rate_option_type == "caplet" else ql.Floor(leg, [strike])
    volatility_quote = ql.QuoteHandle(ql.SimpleQuote(volatility))
    if normal:
        engine = ql.BachelierCapFloorEngine(discount, volatility_quote)
    else:
        engine = ql.BlackCapFloorEngine(discount, volatility_quote, option_day_count)
    optionlet.setPricingEngine(engine)
    return optionlet


def _build_optionlet_fixture(
    *,
    product: str,
    name: str,
    rate_option_type: str,
    forward_rate: float,
    discount_rate: float,
    strike: float,
    volatility: float,
    normal: bool,
) -> dict[str, Any]:
    def price_with_rates(rate_shift: float) -> float:
        return _quantlib_optionlet(
            rate_option_type=rate_option_type,
            forward_rate=forward_rate,
            discount_rate=discount_rate,
            rate_shift=rate_shift,
            strike=strike,
            volatility=volatility,
            normal=normal,
        ).NPV()

    optionlet = _quantlib_optionlet(
        rate_option_type=rate_option_type,
        forward_rate=forward_rate,
        discount_rate=discount_rate,
        rate_shift=0.0,
        strike=strike,
        volatility=volatility,
        normal=normal,
    )
    expected = {
        "dv01": central_difference(price_with_rates, 0.0),
        "npv": optionlet.NPV(),
        "vega": optionlet.vega() * 0.01,
    }
    model_name = "Bachelier" if normal else "Black-76"
    reason = (
        f"QuantLib and Finstack use the same {model_name} closed form on an "
        "exactly aligned one-period schedule and simple forward."
    )
    vol_type = "normal" if normal else "lognormal"
    vol_quote_type = "normal" if normal else "black_lognormal"
    return {
        "schema": SCHEMA,
        "metadata": metadata(
            name=name,
            domain="rates.cap_floor",
            description=f"QuantLib native one-period {model_name} {rate_option_type}.",
            product=product,
        ),
        "kind": "pricing",
        "model": "black76",
        "market": market_snapshot(
            [
                flat_discount_curve("USD-OIS", discount_rate),
                forward_curve(
                    "USD-TERM-3M",
                    projection_knots(*_optionlet_projection(forward_rate)),
                    index_months=3,
                ),
            ],
            surfaces=[
                constant_vol_surface(
                    "USD-CAPFLOOR-VOL-QL",
                    volatility,
                    quote_type=vol_quote_type,
                    strikes=[-0.01, strike, 0.10] if normal else [0.01, strike, 0.10],
                )
            ],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "cap_floor",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "rate_option_type": rate_option_type,
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "strike": str(strike),
                    "start_date": "2026-07-30",
                    "maturity": "2026-10-30",
                    "frequency": {"count": 3, "unit": "months"},
                    "day_count": "act_360",
                    "calendar_id": "weekends_only",
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "USD-TERM-3M",
                    "vol_surface_id": "USD-CAPFLOOR-VOL-QL",
                    "vol_type": vol_type,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {
            "dv01": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
            "npv": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
            "vega": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
        },
    }


def build_black_caplet() -> dict[str, Any]:
    """Build a positive-rate Black-76 caplet fixture."""
    return _build_optionlet_fixture(
        product="black_caplet",
        name="usd_black_caplet_quantlib",
        rate_option_type="caplet",
        forward_rate=0.04,
        discount_rate=0.03,
        strike=0.04,
        volatility=0.20,
        normal=False,
    )


def build_bachelier_floorlet() -> dict[str, Any]:
    """Build a negative-rate Bachelier floorlet fixture."""
    return _build_optionlet_fixture(
        product="bachelier_floorlet",
        name="usd_bachelier_floorlet_quantlib",
        rate_option_type="floorlet",
        forward_rate=-0.002,
        discount_rate=0.005,
        strike=-0.001,
        volatility=0.01,
        normal=True,
    )


def _cap_schedule() -> ql.Schedule:
    return ql.Schedule(
        ql.Date(30, 7, 2026),
        ql.Date(30, 7, 2027),
        ql.Period(3, ql.Months),
        ql.WeekendsOnly(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Forward,
        False,
    )


def _cap_projection(forward_rate: float) -> tuple[list[ql.Date], list[float]]:
    dates = [ql_date(VALUATION_DATE), *_cap_schedule()]
    return dates, chained_simple_discounts(dates, forward_rate, ql.Actual360())


def _quantlib_multi_period_cap(
    *,
    forward_rate: float,
    discount_rate: float,
    rate_shift: float,
    strike: float,
    volatility: float,
) -> ql.CapFloor:
    evaluation_date = ql_date(VALUATION_DATE)
    option_day_count = ql.Actual365Fixed()
    accrual_day_count = ql.Actual360()
    ql.Settings.instance().evaluationDate = evaluation_date

    schedule = _cap_schedule()
    projection = ql_projection_curve(*_cap_projection(forward_rate), shift=rate_shift)
    discount = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, discount_rate + rate_shift, option_day_count, ql.Continuous)
    )
    index = ql.IborIndex(
        "USD-TERM-3M",
        ql.Period(3, ql.Months),
        0,
        ql.USDCurrency(),
        ql.NullCalendar(),
        ql.Unadjusted,
        False,
        accrual_day_count,
        projection,
    )
    leg = ql.IborLeg(
        [10_000_000.0],
        schedule,
        index,
        accrual_day_count,
        ql.ModifiedFollowing,
        [0],
    )
    cap = ql.Cap(leg, [strike])
    cap.setPricingEngine(
        ql.BlackCapFloorEngine(
            discount,
            ql.QuoteHandle(ql.SimpleQuote(volatility)),
            option_day_count,
        )
    )
    return cap


def build_black_cap() -> dict[str, Any]:
    """Build a native QuantLib four-period Black-76 cap fixture."""
    forward_rate = 0.04
    discount_rate = 0.03
    strike = 0.04
    volatility = 0.20

    def price_with_rates(rate_shift: float) -> float:
        return _quantlib_multi_period_cap(
            forward_rate=forward_rate,
            discount_rate=discount_rate,
            rate_shift=rate_shift,
            strike=strike,
            volatility=volatility,
        ).NPV()

    cap = _quantlib_multi_period_cap(
        forward_rate=forward_rate,
        discount_rate=discount_rate,
        rate_shift=0.0,
        strike=strike,
        volatility=volatility,
    )
    reason = (
        "QuantLib and Finstack sum the same Black-76 caplet formula over four "
        "exactly aligned quarterly periods with flat simple forwards."
    )
    return {
        "schema": SCHEMA,
        "metadata": metadata(
            name="usd_black_cap_1y_quantlib",
            domain="rates.cap_floor",
            description="QuantLib native four-period one-year Black-76 cap.",
            product="black_cap",
        ),
        "kind": "pricing",
        "model": "black76",
        "market": market_snapshot(
            [
                flat_discount_curve("USD-OIS", discount_rate),
                forward_curve(
                    "USD-TERM-3M",
                    projection_knots(*_cap_projection(forward_rate)),
                    index_months=3,
                ),
            ],
            surfaces=[
                constant_vol_surface(
                    "USD-CAPFLOOR-VOL-QL",
                    volatility,
                    quote_type="black_lognormal",
                    strikes=[0.01, strike, 0.10],
                )
            ],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "cap_floor",
                "spec": {
                    "id": "USD-BLACK-CAP-1Y-QUANTLIB",
                    "rate_option_type": "cap",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "strike": str(strike),
                    "start_date": "2026-07-30",
                    "maturity": "2027-07-30",
                    "frequency": {"count": 3, "unit": "months"},
                    "day_count": "act_360",
                    "calendar_id": "weekends_only",
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "USD-TERM-3M",
                    "vol_surface_id": "USD-CAPFLOOR-VOL-QL",
                    "vol_type": "lognormal",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {
            "dv01": central_difference(price_with_rates, 0.0),
            "npv": cap.NPV(),
            "vega": cap.vega() * 0.01,
        },
        "tolerances": {
            "dv01": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
            "npv": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
            "vega": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
        },
    }


def _swaption_projection(forward_rate: float) -> tuple[list[ql.Date], list[float]]:
    dates = [ql_date(VALUATION_DATE), ql.Date(30, 4, 2027), ql.Date(30, 4, 2028)]
    return dates, chained_simple_discounts(dates, forward_rate, ql.Actual365Fixed())


def _quantlib_swaption(
    *,
    forward_rate: float,
    discount_rate: float,
    rate_shift: float,
    strike: float,
    volatility: float,
    normal: bool,
) -> ql.Swaption:
    evaluation_date = ql_date(VALUATION_DATE)
    projection_dates, projection_discounts = _swaption_projection(forward_rate)
    swap_start, swap_end = projection_dates[1:]
    day_count = ql.Actual365Fixed()
    calendar = ql.WeekendsOnly()
    ql.Settings.instance().evaluationDate = evaluation_date

    discount = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, discount_rate + rate_shift, day_count, ql.Continuous)
    )
    projection = ql_projection_curve(projection_dates, projection_discounts, shift=rate_shift)
    index = ql.IborIndex(
        "USD-TERM-1Y",
        ql.Period(1, ql.Years),
        0,
        ql.USDCurrency(),
        ql.NullCalendar(),
        ql.Unadjusted,
        False,
        day_count,
        projection,
    )
    schedule = ql.Schedule(
        swap_start,
        swap_end,
        ql.Period(1, ql.Years),
        calendar,
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Forward,
        False,
    )
    swap = ql.VanillaSwap(
        ql.VanillaSwap.Payer,
        10_000_000.0,
        schedule,
        strike,
        day_count,
        schedule,
        index,
        0.0,
        day_count,
        ql.ModifiedFollowing,
    )
    swaption = ql.Swaption(swap, ql.EuropeanExercise(swap_start))
    volatility_quote = ql.QuoteHandle(ql.SimpleQuote(volatility))
    if normal:
        engine = ql.BachelierSwaptionEngine(discount, volatility_quote, day_count)
    else:
        engine = ql.BlackSwaptionEngine(discount, volatility_quote, day_count)
    swaption.setPricingEngine(engine)
    return swaption


def _build_swaption_fixture(
    *,
    product: str,
    name: str,
    forward_rate: float,
    discount_rate: float,
    strike: float,
    volatility: float,
    normal: bool,
) -> dict[str, Any]:
    def price_with_rates(rate_shift: float) -> float:
        return _quantlib_swaption(
            forward_rate=forward_rate,
            discount_rate=discount_rate,
            rate_shift=rate_shift,
            strike=strike,
            volatility=volatility,
            normal=normal,
        ).NPV()

    swaption = _quantlib_swaption(
        forward_rate=forward_rate,
        discount_rate=discount_rate,
        rate_shift=0.0,
        strike=strike,
        volatility=volatility,
        normal=normal,
    )
    expected = {
        "dv01": central_difference(price_with_rates, 0.0),
        "npv": swaption.NPV(),
        "vega": swaption.vega() * 0.01,
    }
    model_name = "Bachelier" if normal else "Black-76"
    reason = (
        f"QuantLib and Finstack use the same {model_name} European swaption "
        "formula on a one-payment vanilla underlying swap."
    )
    vol_model = "normal" if normal else "black"
    quote_type = "normal" if normal else "black_lognormal"
    return {
        "schema": SCHEMA,
        "metadata": metadata(
            name=name,
            domain="rates.swaption",
            description=f"QuantLib native 1Y into 1Y {model_name} payer swaption.",
            product=product,
        ),
        "kind": "pricing",
        "model": "normal" if normal else "black76",
        "market": market_snapshot(
            [
                flat_discount_curve("USD-OIS", discount_rate),
                forward_curve(
                    "USD-TERM-1Y",
                    projection_knots(*_swaption_projection(forward_rate)),
                    index_months=12,
                ),
            ],
            surfaces=[
                {
                    "id": "USD-SWAPTION-VOL-QL",
                    "expiries": [0.5, 1.0, 2.0],
                    "secondary_values": [0.5, 1.0, 2.0],
                    "secondary_axis": "tenor",
                    "quote_type": quote_type,
                    "interpolation_mode": "vol",
                    "vols_row_major": [volatility] * 9,
                }
            ],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "swaption",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "option_type": "call",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "expiry": "2027-04-30",
                    "settlement": "physical",
                    "cash_settlement_method": "par_yield",
                    "vol_model": vol_model,
                    "vol_surface_id": "USD-SWAPTION-VOL-QL",
                    "underlying_fixed_leg": {
                        "discount_curve_id": "USD-OIS",
                        "rate": str(strike),
                        "frequency": {"count": 12, "unit": "months"},
                        "day_count": "act_365f",
                        "calendar_id": "weekends_only",
                        "stub": "none",
                        "start": "2027-04-30",
                        "end": "2028-04-30",
                        "payment_lag_days": 0,
                    },
                    "underlying_float_leg": {
                        "discount_curve_id": "USD-OIS",
                        "index_id": "USD-TERM-1Y",
                        "forward_curve_id": "USD-TERM-1Y",
                        "spread_bp": "0",
                        "frequency": {"count": 12, "unit": "months"},
                        "day_count": "act_365f",
                        "calendar_id": "weekends_only",
                        "stub": "none",
                        "reset_lag_days": 0,
                        "fixing_calendar_id": "weekends_only",
                        "start": "2027-04-30",
                        "end": "2028-04-30",
                        "compounding": "simple",
                        "payment_lag_days": 0,
                    },
                    "sabr_params": None,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {
            "dv01": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
            "npv": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
            "vega": tolerance(ANALYTICAL_RATE_TOLERANCE, reason),
        },
    }


def build_black_swaption() -> dict[str, Any]:
    """Build a Black-76 European payer swaption fixture."""
    return _build_swaption_fixture(
        product="black_swaption",
        name="usd_black_1y1y_payer_swaption_quantlib",
        forward_rate=0.04,
        discount_rate=0.03,
        strike=0.04,
        volatility=0.20,
        normal=False,
    )


def build_bachelier_swaption() -> dict[str, Any]:
    """Build a Bachelier European payer swaption fixture."""
    return _build_swaption_fixture(
        product="bachelier_swaption",
        name="usd_bachelier_1y1y_payer_swaption_quantlib",
        forward_rate=0.001,
        discount_rate=0.005,
        strike=0.001,
        volatility=0.01,
        normal=True,
    )
