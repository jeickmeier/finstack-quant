"""QuantLib Black-76 commodity swaption golden builders.

QuantLib has no commodity swaption instrument. The swap annuity is the sum of
QuantLib flat-curve discount factors on the underlying payment dates, and the
option on the forward swap price is QuantLib's Black-76 calculator with that
annuity as its discount, evaluated here independently of Finstack.
"""

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
EXPIRY = "2026-10-30"
SWAP_START = "2026-11-03"
SWAP_END = "2027-11-03"
# Quarterly unadjusted payment dates of the underlying swap; all are weekdays.
PAYMENT_DATES = ["2027-02-03", "2027-05-03", "2027-08-03", "2027-11-03"]
RATE = 0.04
FORWARD = 80.0
STRIKE = 78.0
VOLATILITY = 0.30
QUANTITY = 10_000.0
# Both sides evaluate Black-76 on the same forward, annuity and variance; the bound is
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


def _black_calculator(option_type: int) -> tuple[ql.BlackCalculator, float]:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    curve = ql.FlatForward(evaluation_date, RATE, day_count, ql.Continuous)
    # The swap pays quantity x price per period with no accrual factor, so the
    # annuity is a plain sum of discount factors. On a flat price curve every
    # period's average forward, and hence the forward swap price, is the level.
    annuity = sum(curve.discount(ql_date(date)) for date in PAYMENT_DATES)
    time = day_count.yearFraction(evaluation_date, ql_date(EXPIRY))
    calculator = ql.BlackCalculator(
        ql.PlainVanillaPayoff(option_type, STRIKE), FORWARD, VOLATILITY * math.sqrt(time), annuity
    )
    return calculator, time


def _build_swaption(*, product: str, name: str, call: bool) -> dict[str, Any]:
    side = "call" if call else "put"
    calculator, time = _black_calculator(ql.Option.Call if call else ql.Option.Put)
    expected = {
        "delta": QUANTITY * calculator.deltaForward(),
        "npv": QUANTITY * calculator.value(),
        # Finstack reports vega per one volatility point.
        "vega": QUANTITY * calculator.vega(time) * 0.01,
    }
    reason = (
        "QuantLib BlackCalculator and Finstack evaluate the same Black-76 formula on the flat forward swap "
        "price with the sum of payment-date discount factors as annuity."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="commodity.commodity_swaption",
                description=f"QuantLib Black-76 six-month WTI {side} swaption on a one-year quarterly swap.",
                product=product,
                source_detail=(
                    f"QuantLib {ql.__version__} BlackCalculator: flat forward swap price {FORWARD}, strike {STRIKE}, "
                    f"lognormal volatility {VOLATILITY}, Act/365F time to expiry {EXPIRY}; discount = annuity = sum "
                    f"of FlatForward (continuous Act/365F, {RATE}) discount factors on {', '.join(PAYMENT_DATES)}; "
                    f"quantity {QUANTITY:.0f} BBL per period. Delta is per unit forward swap price, vega per "
                    "volatility point."
                ),
            )
        ),
        "kind": "pricing",
        "model": "black76",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", RATE), _price_curve("WTI-FORWARD", FORWARD)],
            surfaces=[constant_vol_surface("WTI-VOL", VOLATILITY, strikes=[40.0, 80.0, 160.0])],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "commodity_swaption",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "commodity_type": "Energy",
                    "underlying_ticker": "CL",
                    "unit": "BBL",
                    "currency": "USD",
                    "option_type": side,
                    "expiry": EXPIRY,
                    "underlying_start_date": SWAP_START,
                    "underlying_maturity": SWAP_END,
                    "swap_frequency": {"count": 3, "unit": "months"},
                    "fixed_price": STRIKE,
                    "quantity": QUANTITY,
                    "business_day_convention": "modified_following",
                    "day_count": "act_365f",
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "WTI-FORWARD",
                    "vol_surface_id": "WTI-VOL",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {metric: {"rel": CLOSED_FORM_REL, "tolerance_reason": reason} for metric in expected},
    }


def build_commodity_swaption_call() -> dict[str, Any]:
    """Build a six-month Black-76 call on a one-year quarterly WTI swap."""
    return _build_swaption(product="commodity_swaption_call", name="wti_swaption_black76_call_6m_quantlib", call=True)


def build_commodity_swaption_put() -> dict[str, Any]:
    """Build a six-month Black-76 put on a one-year quarterly WTI swap."""
    return _build_swaption(product="commodity_swaption_put", name="wti_swaption_black76_put_6m_quantlib", call=False)
