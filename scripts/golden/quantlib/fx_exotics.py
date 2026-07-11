"""Native QuantLib FX exotic option golden builders."""

from __future__ import annotations

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


def _garman_kohlhagen_process(
    spot: float,
    domestic_rate: float,
    foreign_rate: float,
    volatility: float,
) -> ql.GarmanKohlagenProcess:
    evaluation_date = ql_date(VALUATION_DATE)
    day_count = ql.Actual365Fixed()
    return ql.GarmanKohlagenProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, foreign_rate, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, domestic_rate, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )


def _fx_market(
    *,
    spot: float,
    domestic_rate: float,
    foreign_rate: float,
    surface_id: str,
    volatility: float,
    strikes: list[float],
) -> dict[str, Any]:
    return market_snapshot(
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
        surfaces=[constant_vol_surface(surface_id, volatility, strikes=strikes)],
    )


def build_fx_digital_option() -> dict[str, Any]:
    """Build a native QuantLib cash-or-nothing EUR/USD call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 7, 2026)
    spot = 1.10
    strike = 1.10
    payout = 100_000.0
    domestic_rate = 0.04
    foreign_rate = 0.025
    volatility = 0.10
    ql.Settings.instance().evaluationDate = evaluation_date

    option = ql.VanillaOption(
        ql.CashOrNothingPayoff(ql.Option.Call, strike, payout),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(
        ql.AnalyticEuropeanEngine(
            _garman_kohlhagen_process(
                spot,
                domestic_rate,
                foreign_rate,
                volatility,
            )
        )
    )
    reason = (
        "QuantLib CashOrNothingPayoff with AnalyticEuropeanEngine and Finstack "
        "use the same Garman-Kohlhagen digital closed form."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="eurusd_cash_digital_call_3m_quantlib",
            domain="fx.fx_digital_option",
            description="QuantLib native three-month EUR/USD cash-or-nothing call.",
            product="fx_digital_option",
        ),
        "kind": "pricing",
        "model": "black76",
        "market": _fx_market(
            spot=spot,
            domestic_rate=domestic_rate,
            foreign_rate=foreign_rate,
            surface_id="EURUSD-DIGITAL-VOL-QL",
            volatility=volatility,
            strikes=[0.8, strike, 1.4],
        ),
        "instrument": {
            "type": "fx_digital_option",
            "spec": {
                "id": "EURUSD-CASH-DIGITAL-CALL-3M-QUANTLIB",
                "base_currency": "EUR",
                "quote_currency": "USD",
                "strike": strike,
                "option_type": "call",
                "payout_type": "cash_or_nothing",
                "payout_amount": {"amount": str(payout), "currency": "USD"},
                "expiry": "2026-07-30",
                "day_count": "Act365F",
                "notional": {"amount": "1000000", "currency": "EUR"},
                "domestic_discount_curve_id": "USD-OIS",
                "foreign_discount_curve_id": "EUR-OIS",
                "vol_surface_id": "EURUSD-DIGITAL-VOL-QL",
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV()},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }


def build_fx_barrier_option() -> dict[str, Any]:
    """Build a native QuantLib continuous EUR/USD up-and-out call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 7, 2026)
    spot = 1.10
    strike = 1.10
    barrier = 1.25
    notional = 1_000_000.0
    domestic_rate = 0.04
    foreign_rate = 0.025
    volatility = 0.10
    ql.Settings.instance().evaluationDate = evaluation_date

    option = ql.BarrierOption(
        ql.Barrier.UpOut,
        barrier,
        0.0,
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(
        ql.AnalyticBarrierEngine(
            _garman_kohlhagen_process(
                spot,
                domestic_rate,
                foreign_rate,
                volatility,
            )
        )
    )
    reason = (
        "QuantLib AnalyticBarrierEngine and Finstack use the continuous-monitoring "
        "Reiner-Rubinstein Garman-Kohlhagen closed form with zero rebate."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="eurusd_up_out_call_3m_quantlib",
            domain="fx.fx_barrier_option",
            description="QuantLib native three-month continuous EUR/USD up-and-out call.",
            product="fx_barrier_option",
        ),
        "kind": "pricing",
        "model": "fx_barrier_bs_continuous",
        "market": _fx_market(
            spot=spot,
            domestic_rate=domestic_rate,
            foreign_rate=foreign_rate,
            surface_id="EURUSD-BARRIER-VOL-QL",
            volatility=volatility,
            strikes=[0.8, strike, 1.4],
        ),
        "instrument": {
            "type": "fx_barrier_option",
            "spec": {
                "id": "EURUSD-UP-OUT-CALL-3M-QUANTLIB",
                "strike": strike,
                "barrier": barrier,
                "rebate": None,
                "option_type": "call",
                "barrier_type": "up_and_out",
                "expiry": "2026-07-30",
                "notional": {"amount": str(int(notional)), "currency": "EUR"},
                "base_currency": "EUR",
                "quote_currency": "USD",
                "day_count": "Act365F",
                "use_gobet_miri": False,
                "domestic_discount_curve_id": "USD-OIS",
                "foreign_discount_curve_id": "EUR-OIS",
                "vol_surface_id": "EURUSD-BARRIER-VOL-QL",
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV() * notional},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }


def build_quanto_option() -> dict[str, Any]:
    """Build a native QuantLib fixed-conversion quanto equity call fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    expiry = ql.Date(30, 4, 2027)
    spot = 100.0
    strike = 100.0
    domestic_rate = 0.04
    foreign_rate = 0.01
    dividend_yield = 0.005
    equity_volatility = 0.20
    fx_volatility = 0.10
    correlation = -0.25
    quantity = 1_000.0
    payoff_fx_rate = 0.01
    fx_spot = 0.01
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date

    domestic_curve = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, domestic_rate, day_count, ql.Continuous)
    )
    foreign_curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, foreign_rate, day_count, ql.Continuous))
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)),
        domestic_curve,
        ql.BlackVolTermStructureHandle(
            ql.BlackConstantVol(
                evaluation_date,
                ql.NullCalendar(),
                equity_volatility,
                day_count,
            )
        ),
    )
    option = ql.QuantoVanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(expiry),
    )
    option.setPricingEngine(
        ql.QuantoEuropeanEngine(
            process,
            foreign_curve,
            ql.BlackVolTermStructureHandle(
                ql.BlackConstantVol(
                    evaluation_date,
                    ql.NullCalendar(),
                    fx_volatility,
                    day_count,
                )
            ),
            ql.QuoteHandle(ql.SimpleQuote(correlation)),
        )
    )
    reason = (
        "QuantLib QuantoEuropeanEngine and Finstack use the same fixed-conversion "
        "Black-Scholes quanto drift adjustment under matched FX direction and correlation."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="nky_usd_quanto_call_1y_quantlib",
            domain="fx.quanto_option",
            description="QuantLib native one-year fixed-conversion Nikkei USD quanto call.",
            product="quanto_option",
        ),
        "kind": "pricing",
        "model": "quanto_bs",
        "market": market_snapshot(
            [
                flat_discount_curve("USD-OIS", domestic_rate),
                flat_discount_curve("JPY-OIS", foreign_rate),
            ],
            fx={
                "config": {
                    "pivot_currency": "USD",
                    "enable_triangulation": True,
                    "cache_capacity": 256,
                },
                "quotes": [["JPY", "USD", fx_spot]],
                "pinned_quotes": [],
            },
            prices={
                "NKY-DIVYIELD": {"unitless": dividend_yield},
                "NKY-SPOT": {"price": {"amount": str(spot), "currency": "JPY"}},
                "JPYUSD-SPOT": {"unitless": fx_spot},
            },
            surfaces=[
                constant_vol_surface(
                    "NKY-VOL-QL",
                    equity_volatility,
                    strikes=[50.0, strike, 150.0],
                ),
                constant_vol_surface(
                    "JPYUSD-VOL-QL",
                    fx_volatility,
                    strikes=[0.005, fx_spot, 0.02],
                ),
            ],
        ),
        "instrument": {
            "type": "quanto_option",
            "spec": {
                "id": "NKY-USD-QUANTO-CALL-1Y-QUANTLIB",
                "underlying_ticker": "NKY",
                "equity_strike": {"amount": str(strike), "currency": "JPY"},
                "option_type": "call",
                "expiry": "2027-04-30",
                "notional": {"amount": "1000", "currency": "USD"},
                "underlying_quantity": quantity,
                "payoff_fx_rate": payoff_fx_rate,
                "base_currency": "JPY",
                "quote_currency": "USD",
                "correlation": correlation,
                "day_count": "Act365F",
                "domestic_discount_curve_id": "USD-OIS",
                "foreign_discount_curve_id": "JPY-OIS",
                "spot_id": "NKY-SPOT",
                "vol_surface_id": "NKY-VOL-QL",
                "div_yield_id": "NKY-DIVYIELD",
                "fx_rate_id": "JPYUSD-SPOT",
                "fx_vol_id": "JPYUSD-VOL-QL",
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": {"npv": option.NPV() * quantity * payoff_fx_rate},
        "tolerances": {"npv": tolerance(1e-7, reason)},
    }
