"""Options on futures golden builders priced with QuantLib's Black and Bachelier formulas.

Each option is European on a quoted futures price. QuantLib's free functions
`blackFormula` and `bachelierBlackFormula` give the undiscounted-forward price
with an explicit discount factor, independently of Finstack.
"""

from __future__ import annotations

from collections.abc import Callable
import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
)

CAPTURED_ON = "2026-11-24"
# Both sides evaluate the same closed form on the same inputs; the residual is
# the difference between two normal-distribution implementations.
CLOSED_FORM_REL = 1e-9
RATE = 0.04
# Finstack defines these sensitivities as central differences in the futures
# price: delta at its default 1% relative bump, and the delta inside DV01 at a
# fixed 0.01% relative bump. The references difference the QuantLib price over
# the same bumps, so only the closed-form residual remains.
DELTA_BUMP = 0.01
DV01_DELTA_BUMP = 1e-4


def _discount(date: str) -> float:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    return ql.FlatForward(evaluation_date, RATE, ql.Actual365Fixed(), ql.Continuous).discount(ql_date(date))


def _build(
    *,
    name: str,
    domain: str,
    instrument_type: str,
    description: str,
    product: str,
    futures_price: float,
    strike: float,
    volatility: float,
    expiry: str,
    option_type: str,
    model: str,
    contracts: float,
    multiplier: float,
    reference_price: float | None = None,
    price_change_per_bp: float | None = None,
) -> dict[str, Any]:
    time = ql.Actual365Fixed().yearFraction(ql_date(VALUATION_DATE), ql_date(expiry))
    std_dev = volatility * math.sqrt(time)
    ql_type = ql.Option.Call if option_type == "call" else ql.Option.Put
    futures_style = reference_price is not None
    discount = 1.0 if futures_style else _discount(expiry)

    def unit_price(forward: float) -> float:
        if model == "black76":
            return ql.blackFormula(ql_type, strike, forward, std_dev, discount)
        return ql.bachelierBlackFormula(ql_type, strike, forward, std_dev, discount)

    scale = contracts * multiplier
    npv = scale * (unit_price(futures_price) - (reference_price or 0.0))

    def delta(relative_bump: float) -> float:
        bump = futures_price * relative_bump
        return scale * (unit_price(futures_price + bump) - unit_price(futures_price - bump)) / (2.0 * bump)

    expected = {"delta": delta(DELTA_BUMP), "npv": npv}
    if price_change_per_bp is not None:
        # One basis point moves the futures price by `price_change_per_bp` and,
        # for a premium paid at expiry, the discount factor by -time * 1bp.
        discounting = 0.0 if futures_style else -time * 1e-4 * npv
        expected["dv01"] = delta(DV01_DELTA_BUMP) * price_change_per_bp + discounting

    formula = "blackFormula" if model == "black76" else "bachelierBlackFormula"
    premium = (
        f"futures-style margining against reference price {reference_price}"
        if futures_style
        else f"premium discounted from {expiry} on a FlatForward curve (continuous Act/365F, {RATE})"
    )
    reason = (
        f"QuantLib {formula} and Finstack evaluate the same closed form on the same futures price, strike, "
        "volatility and Act/365F expiry time; the residual is the normal-distribution implementation."
    )
    derivative_reason = (
        "Central difference of the QuantLib price over the futures-price bump Finstack defines for this "
        "sensitivity (1% relative for delta, 0.01% for the delta inside DV01); only the closed-form residual remains."
    )
    terms: dict[str, Any] = {
        "underlying": f"{name.upper()}-UNDERLYING",
        "futures_price": futures_price,
        "strike": strike,
        "contracts": contracts,
        "multiplier": multiplier,
        "currency": "USD",
        "option_type": option_type,
        "position": "long",
        "exercise_style": "european",
        "expiry": expiry,
        "settlement": {"type": "cash", "payment_date": expiry},
        "model": model,
        "premium_style": "futures_style" if futures_style else "premium_paid",
        "day_count": "act_365f",
        "discount_curve_id": "USD-OIS",
    }
    if reference_price is not None:
        terms["option_reference_price"] = reference_price
    if price_change_per_bp is not None:
        terms["underlying_price_change_per_bp"] = price_change_per_bp

    meta = metadata(
        name=name,
        domain=domain,
        description=description,
        product=product,
        source_detail=(
            f"QuantLib {ql.__version__} {formula}: F = {futures_price}, K = {strike}, volatility {volatility}, "
            f"Act/365F time {time!r} to {expiry}; {premium}; long {contracts:.0f} contracts, "
            f"multiplier {multiplier:.0f}."
        ),
    )
    return {
        "schema": SCHEMA,
        "metadata": {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON},
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([flat_discount_curve("USD-OIS", RATE)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": instrument_type,
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "terms": terms,
                    "instrument_pricing_overrides": {"market_quotes": {"implied_volatility": volatility}},
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {
            metric: {
                "rel": CLOSED_FORM_REL,
                "tolerance_reason": reason if metric == "npv" else derivative_reason,
            }
            for metric in expected
        },
    }


def build_commodity_future_option() -> dict[str, Any]:
    """Build a Black-76 call on a crude oil future."""
    return _build(
        name="wti_future_call_dec26_quantlib",
        domain="commodity.commodity_future_option",
        instrument_type="commodity_future_option",
        description="European Black-76 call on a crude oil future, premium paid at expiry.",
        product="commodity_future_option",
        futures_price=82.0,
        strike=85.0,
        volatility=0.32,
        expiry="2026-11-17",
        option_type="call",
        model="black76",
        contracts=50.0,
        multiplier=1000.0,
    )


def build_equity_future_option() -> dict[str, Any]:
    """Build a Black-76 put on an equity index future."""
    return _build(
        name="spx_future_put_sep26_quantlib",
        domain="listed.equity_future_option",
        instrument_type="equity_future_option",
        description="European Black-76 put on an equity index future, premium paid at expiry.",
        product="equity_future_option",
        futures_price=5050.0,
        strike=4900.0,
        volatility=0.18,
        expiry="2026-09-18",
        option_type="put",
        model="black76",
        contracts=20.0,
        multiplier=50.0,
    )


def build_fx_future_option() -> dict[str, Any]:
    """Build a futures-style Black-76 call on a currency future."""
    return _build(
        name="eurusd_future_call_dec26_quantlib",
        domain="listed.fx_future_option",
        instrument_type="fx_future_option",
        description="European Black-76 call on a EUR/USD future with futures-style premium margining.",
        product="fx_future_option",
        futures_price=1.1100,
        strike=1.1250,
        volatility=0.075,
        expiry="2026-12-04",
        option_type="call",
        model="black76",
        contracts=40.0,
        multiplier=125_000.0,
        reference_price=0.0125,
    )


def build_interest_rate_future_option() -> dict[str, Any]:
    """Build a Bachelier put on a three-month rate future."""
    return _build(
        name="sofr_future_put_dec26_quantlib",
        domain="rates.interest_rate_future_option",
        instrument_type="interest_rate_future_option",
        description="European Bachelier put on a three-month rate future, premium paid at expiry.",
        product="interest_rate_future_option",
        futures_price=96.25,
        strike=96.00,
        volatility=0.85,
        expiry="2026-12-11",
        option_type="put",
        model="normal",
        contracts=100.0,
        multiplier=2500.0,
        price_change_per_bp=-0.01,
    )


def build_volatility_index_future_option() -> dict[str, Any]:
    """Build a Black-76 call on a volatility index future."""
    return _build(
        name="vix_future_call_aug26_quantlib",
        domain="listed.volatility_index_future_option",
        instrument_type="volatility_index_future_option",
        description="European Black-76 call on a volatility index future, premium paid at expiry.",
        product="volatility_index_future_option",
        futures_price=20.5,
        strike=24.0,
        volatility=0.95,
        expiry="2026-08-19",
        option_type="call",
        model="black76",
        contracts=200.0,
        multiplier=100.0,
    )


FUTURE_OPTION_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "commodity_future_option": (
        "commodity_future_option/wti_future_call_dec26_quantlib.json",
        build_commodity_future_option,
    ),
    "equity_future_option": (
        "equity_future_option/spx_future_put_sep26_quantlib.json",
        build_equity_future_option,
    ),
    "fx_future_option": (
        "fx_future_option/eurusd_future_call_dec26_quantlib.json",
        build_fx_future_option,
    ),
    "interest_rate_future_option": (
        "interest_rate_future_option/sofr_future_put_dec26_quantlib.json",
        build_interest_rate_future_option,
    ),
    "volatility_index_future_option": (
        "volatility_index_future_option/vix_future_call_aug26_quantlib.json",
        build_volatility_index_future_option,
    ),
}
