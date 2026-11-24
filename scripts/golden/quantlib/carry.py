"""Cost-of-carry golden builders on QuantLib discount curves.

QuantLib has no FX swap or equity futures instrument, so these fixtures take
their discount factors from QuantLib flat curves and evaluate the contractual
cash flows and the cost-of-carry forward here, independently of Finstack.
"""

from __future__ import annotations

import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    central_difference,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
)

CAPTURED_ON = "2026-11-23"
# Both sides discount the same dated cash flows on the same flat curves; only
# double-precision evaluation order differs.
ARITHMETIC_REL = 1e-9


def _captured(meta: dict[str, Any]) -> dict[str, Any]:
    return {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON}


def _discount(rate: float, date: str) -> float:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    curve = ql.FlatForward(evaluation_date, rate, ql.Actual365Fixed(), ql.Continuous)
    return curve.discount(ql_date(date))


def _tolerances(expected: dict[str, float], reason: str) -> dict[str, Any]:
    return {metric: {"rel": ARITHMETIC_REL, "tolerance_reason": reason} for metric in expected}


def build_fx_swap() -> dict[str, Any]:
    """Build a three-month off-market EUR/USD FX swap fixture."""
    spot, near_rate, far_rate = 1.10, 1.1005, 1.1070
    near, far = "2026-05-05", "2026-08-05"
    notional = 10_000_000.0
    domestic_rate, foreign_rate = 0.04, 0.025

    def npv(domestic_shift: float = 0.0, foreign_shift: float = 0.0) -> float:
        domestic = domestic_rate + domestic_shift
        foreign = foreign_rate + foreign_shift
        # Receive EUR and pay USD on the near date, the reverse on the far date.
        base_leg = notional * (_discount(foreign, near) - _discount(foreign, far))
        quote_leg = notional * (far_rate * _discount(domestic, far) - near_rate * _discount(domestic, near))
        return spot * base_leg + quote_leg

    expected = {
        "dv01_domestic": central_difference(lambda shift: npv(domestic_shift=shift), 0.0),
        "dv01_foreign": central_difference(lambda shift: npv(foreign_shift=shift), 0.0),
        "npv": npv(),
    }
    reason = (
        "Four dated cash flows discounted on QuantLib flat curves and converted at spot, against the same "
        "covered-interest-parity valuation in Finstack; only floating-point evaluation order differs."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name="eurusd_fx_swap_3m_quantlib",
                domain="fx.fx_swap",
                description="Three-month off-market EUR/USD FX swap valued on QuantLib discount curves.",
                product="fx_swap",
                source_detail=(
                    f"QuantLib {ql.__version__} FlatForward discount factors (continuous Act/365F, USD {domestic_rate}, "
                    f"EUR {foreign_rate}); cash flows +N EUR and -N*{near_rate} USD on {near}, -N EUR and "
                    f"+N*{far_rate} USD on {far}, N = {notional:.0f}; EUR leg converted at spot {spot}."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", domestic_rate), flat_discount_curve("EUR-OIS", foreign_rate)],
            fx={
                "config": {"pivot_currency": "USD", "cache_capacity": 256},
                "quotes": [["EUR", "USD", spot]],
                "pinned_quotes": [],
                "provider_quotes": [],
                "provider_pinned_quotes": [],
            },
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "fx_swap",
                "spec": {
                    "id": "EURUSD-FXSWAP-3M-QUANTLIB",
                    "base_currency": "EUR",
                    "quote_currency": "USD",
                    "near_date": near,
                    "far_date": far,
                    "notional": {"amount": f"{notional:.0f}", "currency": "EUR"},
                    "domestic_discount_curve_id": "USD-OIS",
                    "foreign_discount_curve_id": "EUR-OIS",
                    "near_rate": near_rate,
                    "far_rate": far_rate,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": _tolerances(expected, reason),
    }


def build_equity_future() -> dict[str, Any]:
    """Build a three-month equity index future fixture."""
    spot, entry_price, dividend_yield, rate = 5000.0, 5010.0, 0.015, 0.04
    contracts, multiplier = 10.0, 50.0
    settlement = "2026-07-30"
    time = ql.Actual365Fixed().yearFraction(ql_date(VALUATION_DATE), ql_date(settlement))

    def fair_price(spot_level: float = spot, rate_shift: float = 0.0) -> float:
        return spot_level * math.exp(-dividend_yield * time) / _discount(rate + rate_shift, settlement)

    def npv(spot_level: float = spot, rate_shift: float = 0.0) -> float:
        return contracts * multiplier * (fair_price(spot_level, rate_shift) - entry_price)

    expected = {
        "basis": fair_price() - spot,
        "delta": contracts * multiplier * fair_price() / spot,
        "dv01": central_difference(lambda shift: npv(rate_shift=shift), 0.0),
        "futures_price": fair_price(),
        "npv": npv(),
    }
    reason = (
        "Cost-of-carry fair price S*exp(-q*T)/DF(T) with a QuantLib flat-curve discount factor, against the "
        "same formula in Finstack; only floating-point evaluation order differs."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name="spx_future_3m_quantlib",
                domain="listed.equity_future",
                description="Three-month equity index future at cost-of-carry fair value on a QuantLib discount curve.",
                product="equity_future",
                source_detail=(
                    f"QuantLib {ql.__version__} FlatForward discount factor (continuous Act/365F, {rate}) to settlement "
                    f"{settlement}; fair price S*exp(-q*T)/DF with S = {spot}, q = {dividend_yield}, T on Act/365F; "
                    f"long {contracts:.0f} contracts, multiplier {multiplier:.0f}, entry {entry_price}."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "equity_future",
                "spec": {
                    "id": "SPX-FUTURE-3M-QUANTLIB",
                    "underlying_ticker": "SPX",
                    "underlying_currency": "USD",
                    "terms": {
                        "contracts": contracts,
                        "multiplier": multiplier,
                        "currency": "USD",
                        "entry_price": entry_price,
                        "last_trading_date": "2026-07-29",
                        "settlement_date": settlement,
                        "position": "long",
                        "settlement": {"type": "cash"},
                    },
                    "discount_curve_id": "USD-OIS",
                    "spot_id": "SPX-SPOT",
                    "div_yield_id": "SPX-DIVYIELD",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": _tolerances(expected, reason),
    }
