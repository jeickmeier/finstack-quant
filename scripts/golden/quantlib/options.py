"""Native QuantLib option golden builders."""

from __future__ import annotations

import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA_VERSION,
    VALUATION_DATE,
    constant_vol_surface,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
    tolerance,
)


def build_european_equity_option() -> dict[str, Any]:
    """Build a native QuantLib one-year European equity call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 4, 2027)
    spot = 100.0
    strike = 100.0
    rate = 0.04
    dividend_yield = 0.02
    volatility = 0.20
    quantity = 100.0
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    spot_quote = ql.QuoteHandle(ql.SimpleQuote(spot))
    discount_curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous))
    dividend_curve = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)
    )
    vol_surface = ql.BlackVolTermStructureHandle(
        ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)
    )
    process = ql.BlackScholesMertonProcess(
        spot_quote,
        dividend_curve,
        discount_curve,
        vol_surface,
    )
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(ql.AnalyticEuropeanEngine(process))

    expected = {
        "delta": option.delta() * quantity,
        "gamma": option.gamma() * quantity,
        "npv": option.NPV() * quantity,
        "rho": option.rho() * quantity * 1e-4,
        "vega": option.vega() * quantity * 0.01,
    }
    reason = (
        "QuantLib AnalyticEuropeanEngine and Finstack use the same Black-Scholes-Merton "
        "closed form; tolerance permits only floating-point and date-clock residuals."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="spx_atm_call_1y_quantlib",
            domain="equity.equity_option",
            description="QuantLib native one-year ATM European equity call.",
            product="european_equity_option",
        ),
        "kind": "pricing",
        "model": "black76",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
            surfaces=[constant_vol_surface("SPX-VOL-QL", volatility)],
        ),
        "instrument": {
            "type": "equity_option",
            "spec": {
                "id": "SPX-ATM-CALL-1Y-QUANTLIB",
                "underlying_ticker": "SPX",
                "strike": strike,
                "option_type": "call",
                "expiry": "2027-04-30",
                "notional": {"amount": str(quantity), "currency": "USD"},
                "discount_curve_id": "USD-OIS",
                "spot_id": "SPX-SPOT",
                "vol_surface_id": "SPX-VOL-QL",
                "div_yield_id": "SPX-DIVYIELD",
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": expected,
        "tolerances": {metric: tolerance(1e-7, reason) for metric in expected},
    }


def build_european_fx_option() -> dict[str, Any]:
    """Build a native QuantLib three-month EUR/USD European call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 7, 2026)
    spot = 1.10
    strike = 1.10
    domestic_rate = 0.04
    foreign_rate = 0.025
    volatility = 0.10
    notional = 1_000_000.0
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    domestic_curve = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, domestic_rate, day_count, ql.Continuous)
    )
    foreign_curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, foreign_rate, day_count, ql.Continuous))
    process = ql.GarmanKohlagenProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        foreign_curve,
        domestic_curve,
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(ql.AnalyticEuropeanEngine(process))

    time_to_expiry = day_count.yearFraction(evaluation_date, expiry)
    forward_delta = option.delta() * math.exp(foreign_rate * time_to_expiry)
    premium_adjusted_delta = (
        (strike / spot)
        * math.exp(-domestic_rate * time_to_expiry)
        * ql.CumulativeNormalDistribution()(
            (math.log(spot / strike) + (domestic_rate - foreign_rate - volatility**2 / 2.0) * time_to_expiry)
            / (volatility * math.sqrt(time_to_expiry))
        )
    )
    expected = {
        "delta": option.delta() * notional,
        "delta_forward": forward_delta * notional,
        "delta_premium_adjusted": premium_adjusted_delta * notional,
        "foreign_rho": option.dividendRho() * notional * 1e-4,
        "gamma": option.gamma() * notional,
        "npv": option.NPV() * notional,
        "rho": option.rho() * notional * 1e-4,
        "vega": option.vega() * notional * 0.01,
    }
    reason = (
        "QuantLib AnalyticEuropeanEngine and Finstack use the same "
        "Garman-Kohlhagen closed form; tolerance permits only floating-point "
        "and date-clock residuals."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="eurusd_atm_call_3m_quantlib",
            domain="fx.fx_option",
            description="QuantLib native three-month ATM EUR/USD European call.",
            product="european_fx_option",
        ),
        "kind": "pricing",
        "model": "black76",
        "market": market_snapshot(
            [
                flat_discount_curve("USD-OIS", domestic_rate),
                flat_discount_curve("EUR-OIS", foreign_rate),
            ],
            fx={
                "config": {
                    "pivot_currency": "USD",
                    "enable_triangulation": True,
                    "cache_capacity": 256,
                },
                "quotes": [["EUR", "USD", spot]],
                "pinned_quotes": [],
            },
            surfaces=[constant_vol_surface("EURUSD-VOL-QL", volatility)],
        ),
        "instrument": {
            "type": "fx_option",
            "spec": {
                "id": "EURUSD-ATM-CALL-3M-QUANTLIB",
                "base_currency": "EUR",
                "quote_currency": "USD",
                "strike": strike,
                "option_type": "call",
                "expiry": "2026-07-30",
                "notional": {"amount": str(int(notional)), "currency": "EUR"},
                "domestic_discount_curve_id": "USD-OIS",
                "foreign_discount_curve_id": "EUR-OIS",
                "vol_surface_id": "EURUSD-VOL-QL",
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": expected,
        "tolerances": {metric: tolerance(1e-7, reason) for metric in expected},
    }


def build_barrier_option() -> dict[str, Any]:
    """Build a native QuantLib continuous down-and-out equity call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 4, 2027)
    spot = 100.0
    strike = 100.0
    barrier = 80.0
    rate = 0.04
    dividend_yield = 0.01
    volatility = 0.20
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )
    option = ql.BarrierOption(
        ql.Barrier.DownOut,
        barrier,
        0.0,
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(ql.AnalyticBarrierEngine(process))
    reason = (
        "QuantLib AnalyticBarrierEngine and Finstack use the continuous-monitoring "
        "Reiner-Rubinstein closed form with zero rebate."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="spx_down_out_call_1y_quantlib",
            domain="exotics.barrier_option",
            description="QuantLib native one-year continuous down-and-out equity call.",
            product="barrier_option",
        ),
        "kind": "pricing",
        "model": "barrier_bs_continuous",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
            surfaces=[
                constant_vol_surface(
                    "SPX-BARRIER-VOL-QL",
                    volatility,
                    strikes=[50.0, strike, 150.0],
                )
            ],
        ),
        "instrument": {
            "type": "barrier_option",
            "spec": {
                "id": "SPX-DOWN-OUT-CALL-1Y-QUANTLIB",
                "underlying_ticker": "SPX",
                "strike": strike,
                "barrier": {"amount": str(barrier), "currency": "USD"},
                "rebate": None,
                "option_type": "call",
                "barrier_type": "down_and_out",
                "expiry": "2027-04-30",
                "notional": {"amount": "1", "currency": "USD"},
                "day_count": "Act365F",
                "use_gobet_miri": False,
                "discount_curve_id": "USD-OIS",
                "spot_id": "SPX-SPOT",
                "vol_surface_id": "SPX-BARRIER-VOL-QL",
                "div_yield_id": "SPX-DIVYIELD",
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV()},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }


def build_geometric_asian_option() -> dict[str, Any]:
    """Build a native QuantLib discrete geometric-average equity call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 4, 2027)
    fixing_dates = [
        ql.Date(30, 7, 2026),
        ql.Date(30, 10, 2026),
        ql.Date(30, 1, 2027),
        expiry,
    ]
    spot = 100.0
    strike = 100.0
    rate = 0.04
    dividend_yield = 0.01
    volatility = 0.20
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )
    option = ql.DiscreteAveragingAsianOption(
        ql.Average.Geometric,
        fixing_dates,
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(ql.AnalyticDiscreteGeometricAveragePriceAsianEngine(process))
    reason = (
        "QuantLib AnalyticDiscreteGeometricAveragePriceAsianEngine and Finstack "
        "use the exact discrete geometric-average closed form on identical fixing dates."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="spx_geometric_asian_call_1y_quantlib",
            domain="exotics.asian_option",
            description="QuantLib native one-year discrete geometric-average equity call.",
            product="geometric_asian_option",
        ),
        "kind": "pricing",
        "model": "asian_geometric_bs",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
            surfaces=[
                constant_vol_surface(
                    "SPX-ASIAN-VOL-QL",
                    volatility,
                    strikes=[50.0, strike, 150.0],
                )
            ],
        ),
        "instrument": {
            "type": "asian_option",
            "spec": {
                "id": "SPX-GEOMETRIC-ASIAN-CALL-1Y-QUANTLIB",
                "underlying_ticker": "SPX",
                "strike": strike,
                "option_type": "call",
                "averaging_method": "geometric",
                "expiry": "2027-04-30",
                "fixing_dates": [
                    "2026-07-30",
                    "2026-10-30",
                    "2027-01-30",
                    "2027-04-30",
                ],
                "notional": {"amount": "1", "currency": "USD"},
                "day_count": "Act365F",
                "discount_curve_id": "USD-OIS",
                "spot_id": "SPX-SPOT",
                "vol_surface_id": "SPX-ASIAN-VOL-QL",
                "div_yield_id": "SPX-DIVYIELD",
                "past_fixings": [],
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV()},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }


def build_arithmetic_asian_option() -> dict[str, Any]:
    """Build a native QuantLib discrete arithmetic-average equity call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 4, 2027)
    fixing_dates = [
        ql.Date(30, 7, 2026),
        ql.Date(30, 10, 2026),
        ql.Date(30, 1, 2027),
        expiry,
    ]
    spot = 100.0
    strike = 100.0
    rate = 0.04
    dividend_yield = 0.01
    volatility = 0.20
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )
    option = ql.DiscreteAveragingAsianOption(
        ql.Average.Arithmetic,
        fixing_dates,
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(ql.TurnbullWakemanAsianEngine(process))
    reason = (
        "QuantLib TurnbullWakemanAsianEngine and Finstack use the same "
        "Turnbull-Wakeman moment-matching approximation on identical fixing dates."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="spx_arithmetic_asian_call_1y_quantlib",
            domain="exotics.asian_option",
            description="QuantLib native one-year discrete arithmetic-average equity call.",
            product="arithmetic_asian_option",
        ),
        "kind": "pricing",
        "model": "asian_turnbull_wakeman",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
            surfaces=[
                constant_vol_surface(
                    "SPX-ASIAN-VOL-QL",
                    volatility,
                    strikes=[50.0, strike, 150.0],
                )
            ],
        ),
        "instrument": {
            "type": "asian_option",
            "spec": {
                "id": "SPX-ARITHMETIC-ASIAN-CALL-1Y-QUANTLIB",
                "underlying_ticker": "SPX",
                "strike": strike,
                "option_type": "call",
                "averaging_method": "arithmetic",
                "expiry": "2027-04-30",
                "fixing_dates": [
                    "2026-07-30",
                    "2026-10-30",
                    "2027-01-30",
                    "2027-04-30",
                ],
                "notional": {"amount": "1", "currency": "USD"},
                "day_count": "Act365F",
                "discount_curve_id": "USD-OIS",
                "spot_id": "SPX-SPOT",
                "vol_surface_id": "SPX-ASIAN-VOL-QL",
                "div_yield_id": "SPX-DIVYIELD",
                "past_fixings": [],
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV()},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }


def build_fixed_lookback_option() -> dict[str, Any]:
    """Build a native QuantLib continuous fixed-strike lookback call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 4, 2027)
    spot = 100.0
    strike = 105.0
    observed_max = spot
    rate = 0.04
    dividend_yield = 0.01
    volatility = 0.20
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )
    option = ql.ContinuousFixedLookbackOption(
        observed_max,
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(ql.AnalyticContinuousFixedLookbackEngine(process))
    reason = (
        "QuantLib AnalyticContinuousFixedLookbackEngine and Finstack use the "
        "continuous-monitoring Goldman-Sosin-Gatto closed form with the same "
        "observed maximum."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="spx_fixed_lookback_call_1y_quantlib",
            domain="exotics.lookback_option",
            description="QuantLib native one-year continuous fixed-strike lookback call.",
            product="fixed_lookback_option",
        ),
        "kind": "pricing",
        "model": "lookback_bs_continuous",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
            surfaces=[
                constant_vol_surface(
                    "SPX-LOOKBACK-VOL-QL",
                    volatility,
                    strikes=[50.0, strike, 150.0],
                )
            ],
        ),
        "instrument": {
            "type": "lookback_option",
            "spec": {
                "id": "SPX-FIXED-LOOKBACK-CALL-1Y-QUANTLIB",
                "underlying_ticker": "SPX",
                "strike": strike,
                "option_type": "call",
                "lookback_type": "fixed_strike",
                "expiry": "2027-04-30",
                "notional": {"amount": "1", "currency": "USD"},
                "day_count": "Act365F",
                "discount_curve_id": "USD-OIS",
                "spot_id": "SPX-SPOT",
                "vol_surface_id": "SPX-LOOKBACK-VOL-QL",
                "div_yield_id": "SPX-DIVYIELD",
                "use_gobet_miri": False,
                "observed_min": None,
                "observed_max": {"amount": str(observed_max), "currency": "USD"},
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV()},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }


def build_floating_lookback_option() -> dict[str, Any]:
    """Build a native QuantLib continuous floating-strike lookback call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 4, 2027)
    spot = 100.0
    observed_min = spot
    rate = 0.04
    dividend_yield = 0.01
    volatility = 0.20
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )
    option = ql.ContinuousFloatingLookbackOption(
        observed_min,
        ql.FloatingTypePayoff(ql.Option.Call),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(ql.AnalyticContinuousFloatingLookbackEngine(process))
    reason = (
        "QuantLib AnalyticContinuousFloatingLookbackEngine and Finstack use the "
        "continuous-monitoring Goldman-Sosin-Gatto closed form with the same "
        "observed minimum."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="spx_floating_lookback_call_1y_quantlib",
            domain="exotics.lookback_option",
            description="QuantLib native one-year continuous floating-strike lookback call.",
            product="floating_lookback_option",
        ),
        "kind": "pricing",
        "model": "lookback_bs_continuous",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
            surfaces=[
                constant_vol_surface(
                    "SPX-LOOKBACK-VOL-QL",
                    volatility,
                    strikes=[50.0, spot, 150.0],
                )
            ],
        ),
        "instrument": {
            "type": "lookback_option",
            "spec": {
                "id": "SPX-FLOATING-LOOKBACK-CALL-1Y-QUANTLIB",
                "underlying_ticker": "SPX",
                "strike": None,
                "option_type": "call",
                "lookback_type": "floating_strike",
                "expiry": "2027-04-30",
                "notional": {"amount": "1", "currency": "USD"},
                "day_count": "Act365F",
                "discount_curve_id": "USD-OIS",
                "spot_id": "SPX-SPOT",
                "vol_surface_id": "SPX-LOOKBACK-VOL-QL",
                "div_yield_id": "SPX-DIVYIELD",
                "use_gobet_miri": False,
                "observed_min": {"amount": str(observed_min), "currency": "USD"},
                "observed_max": None,
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV()},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }
