"""Native QuantLib commodity option golden builders."""

from __future__ import annotations

import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    constant_vol_surface,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
)

CAPTURED_ON = "2026-11-23"
EXPIRY = "2027-04-30"
RATE = 0.04
QUANTITY = 10_000.0
# Both sides evaluate Black-76 on the same forward, discount factor and variance; the bound is
# the crate's adjudicated 5e-9 relative normal-CDF agreement with QuantLib on the premium.
CLOSED_FORM_REL = 5e-9


def _captured(meta: dict[str, Any]) -> dict[str, Any]:
    return {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON}


def _price_curve(curve_id: str, level: float) -> dict[str, Any]:
    return {
        "type": "price",
        "id": curve_id,
        "base": VALUATION_DATE,
        "day_count": "act_365f",
        "level_kind": "price",
        "knot_points": [[0.0, level], [1.0, level], [20.0, level]],
        "interp_style": "linear",
        "extrapolation": "flat",
    }


def _time_to_expiry() -> float:
    return ql.Actual365Fixed().yearFraction(ql_date(VALUATION_DATE), ql_date(EXPIRY))


def build_commodity_spot_option() -> dict[str, Any]:
    """Build a one-year Black-76 WTI call on a flat forward curve."""
    forward, strike, volatility = 80.0, 75.0, 0.30
    time = _time_to_expiry()
    npv = QUANTITY * ql.blackFormula(
        ql.Option.Call, strike, forward, volatility * math.sqrt(time), math.exp(-RATE * time)
    )
    reason = "QuantLib blackFormula and Finstack evaluate the same Black-76 formula on the curve forward."
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name="wti_black76_call_1y_quantlib",
                domain="commodity.commodity_spot_option",
                description="QuantLib Black-76 one-year European WTI call on the forward price.",
                product="commodity_spot_option",
                source_detail=(
                    f"QuantLib {ql.__version__} blackFormula: forward {forward}, strike {strike}, lognormal "
                    f"volatility {volatility}, Act/365F expiry time, discount exp(-{RATE} T), quantity {QUANTITY:.0f} BBL."
                ),
            )
        ),
        "kind": "pricing",
        "model": "black76",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", RATE), _price_curve("WTI-FORWARD", forward)],
            surfaces=[constant_vol_surface("WTI-VOL", volatility, strikes=[40.0, 80.0, 160.0])],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "commodity_spot_option",
                "spec": {
                    "id": "WTI-BLACK76-CALL-1Y-QUANTLIB",
                    "commodity_type": "Energy",
                    "underlying_ticker": "CL",
                    "unit": "BBL",
                    "currency": "USD",
                    "option_type": "call",
                    "exercise_style": "european",
                    "settlement": "cash",
                    "strike": strike,
                    "expiry": EXPIRY,
                    "quantity": QUANTITY,
                    "multiplier": 1.0,
                    "day_count": "act_365f",
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "WTI-FORWARD",
                    "vol_surface_id": "WTI-VOL",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"npv": npv},
        "tolerances": {"npv": {"rel": CLOSED_FORM_REL, "tolerance_reason": reason}},
    }


def _kirk_value(
    option_type: int, *, f1: float, f2: float, strike: float, vol1: float, vol2: float, rho: float
) -> float:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    discount = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, RATE, day_count, ql.Continuous))

    def process(forward: float, volatility: float) -> ql.BlackProcess:
        vol = ql.BlackVolTermStructureHandle(
            ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)
        )
        return ql.BlackProcess(ql.QuoteHandle(ql.SimpleQuote(forward)), discount, vol)

    option = ql.BasketOption(
        ql.SpreadBasketPayoff(ql.PlainVanillaPayoff(option_type, strike)), ql.EuropeanExercise(ql_date(EXPIRY))
    )
    option.setPricingEngine(ql.KirkEngine(process(f1, vol1), process(f2, vol2), rho))
    return option.NPV()


def _build_spread_option(*, product: str, name: str, call: bool) -> dict[str, Any]:
    f1, f2, strike, vol1, vol2, rho = 95.0, 80.0, 10.0, 0.35, 0.30, 0.85
    npv = QUANTITY * _kirk_value(
        ql.Option.Call if call else ql.Option.Put, f1=f1, f2=f2, strike=strike, vol1=vol1, vol2=vol2, rho=rho
    )
    side = "call" if call else "put"
    reason = "QuantLib KirkEngine and Finstack evaluate the same Kirk (1995) approximation."
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="commodity.commodity_spread_option",
                description=f"QuantLib Kirk one-year crack-spread {side}.",
                product=product,
                source_detail=(
                    f"QuantLib {ql.__version__} KirkEngine on two BlackProcess futures: F1 {f1}, F2 {f2}, strike "
                    f"{strike}, volatilities {vol1} and {vol2}, correlation {rho}, flat continuous Act/365F "
                    f"rate {RATE}, quantity {QUANTITY:.0f}."
                ),
            )
        ),
        "kind": "pricing",
        "model": "black76",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", RATE), _price_curve("RBOB-FORWARD", f1), _price_curve("WTI-FORWARD", f2)],
            surfaces=[
                constant_vol_surface("RBOB-VOL", vol1, strikes=[40.0, 80.0, 160.0]),
                constant_vol_surface("WTI-VOL", vol2, strikes=[40.0, 80.0, 160.0]),
            ],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "commodity_spread_option",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "currency": "USD",
                    "option_type": side,
                    "strike": strike,
                    "correlation": rho,
                    "expiry": EXPIRY,
                    "quantity": QUANTITY,
                    "day_count": "act_365f",
                    "discount_curve_id": "USD-OIS",
                    "leg1_forward_curve_id": "RBOB-FORWARD",
                    "leg1_vol_surface_id": "RBOB-VOL",
                    "leg2_forward_curve_id": "WTI-FORWARD",
                    "leg2_vol_surface_id": "WTI-VOL",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"npv": npv},
        "tolerances": {"npv": {"rel": CLOSED_FORM_REL, "tolerance_reason": reason}},
    }


def build_commodity_spread_call() -> dict[str, Any]:
    """Build a one-year Kirk crack-spread call."""
    return _build_spread_option(product="commodity_spread_call", name="crack_spread_kirk_call_1y_quantlib", call=True)


def build_commodity_spread_put() -> dict[str, Any]:
    """Build a one-year Kirk crack-spread put."""
    return _build_spread_option(product="commodity_spread_put", name="crack_spread_kirk_put_1y_quantlib", call=False)


def build_commodity_asian_option() -> dict[str, Any]:
    """Build a one-year quarterly arithmetic-average WTI call on a flat forward curve."""
    forward, strike, volatility = 80.0, 78.0, 0.30
    fixings = ["2026-07-30", "2026-10-30", "2027-01-30", EXPIRY]
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    flat_rate = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, RATE, day_count, ql.Continuous))
    # A dividend yield equal to the rate gives zero cost of carry, so every
    # fixing's forward equals the flat curve level.
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(forward)),
        flat_rate,
        flat_rate,
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), volatility, day_count)),
    )
    option = ql.DiscreteAveragingAsianOption(
        ql.Average.Arithmetic,
        [ql_date(date) for date in fixings],
        ql.PlainVanillaPayoff(ql.Option.Call, strike),
        ql.EuropeanExercise(ql_date(EXPIRY)),
    )
    option.setPricingEngine(ql.TurnbullWakemanAsianEngine(process))
    reason = (
        "QuantLib TurnbullWakemanAsianEngine and Finstack use the same Turnbull-Wakeman moment matching on "
        "identical fixing dates and forwards; the bound matches the equity Asian fixture's 1e-7 on a price of 6.28."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name="wti_arithmetic_asian_call_1y_quantlib",
                domain="commodity.commodity_asian_option",
                description="QuantLib Turnbull-Wakeman one-year quarterly arithmetic-average WTI call.",
                product="commodity_asian_option",
                source_detail=(
                    f"QuantLib {ql.__version__} TurnbullWakemanAsianEngine with zero cost of carry: flat forward "
                    f"{forward}, strike {strike}, volatility {volatility}, four quarterly fixings ending {EXPIRY}, "
                    f"flat continuous Act/365F rate {RATE}, quantity {QUANTITY:.0f} BBL."
                ),
            )
        ),
        "kind": "pricing",
        "model": "asian_turnbull_wakeman",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", RATE), _price_curve("WTI-FORWARD", forward)],
            surfaces=[constant_vol_surface("WTI-VOL", volatility, strikes=[40.0, 80.0, 160.0])],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "commodity_asian_option",
                "spec": {
                    "id": "WTI-ARITHMETIC-ASIAN-CALL-1Y-QUANTLIB",
                    "commodity_type": "Energy",
                    "underlying_ticker": "CL",
                    "unit": "BBL",
                    "currency": "USD",
                    "option_type": "call",
                    "averaging_method": "arithmetic",
                    "strike": strike,
                    "fixing_dates": fixings,
                    "past_fixings": [],
                    "expiry": EXPIRY,
                    "quantity": QUANTITY,
                    "day_count": "act_365f",
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "WTI-FORWARD",
                    "vol_surface_id": "WTI-VOL",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"npv": QUANTITY * option.NPV()},
        "tolerances": {"npv": {"rel": 2e-8, "tolerance_reason": reason}},
    }
