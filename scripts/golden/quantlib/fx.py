"""Native QuantLib foreign-exchange golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA_VERSION,
    VALUATION_DATE,
    central_difference,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
    tolerance,
)


def _quantlib_fx_forward_npv(
    *,
    spot: float,
    domestic_rate: float,
    foreign_rate: float,
) -> float:
    evaluation_date = ql_date(VALUATION_DATE)
    maturity = ql.Date(30, 4, 2027)
    day_count = ql.Actual365Fixed()
    ql.Settings.instance().evaluationDate = evaluation_date
    domestic_curve = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, domestic_rate, day_count, ql.Continuous)
    )
    foreign_curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, foreign_rate, day_count, ql.Continuous))
    forward = ql.FxForward(
        1_100_000.0,
        ql.USDCurrency(),
        1_000_000.0,
        ql.EURCurrency(),
        maturity,
        True,
        0,
        ql.NullCalendar(),
    )
    forward.setPricingEngine(
        ql.DiscountingFxForwardEngine(
            domestic_curve,
            foreign_curve,
            ql.QuoteHandle(ql.SimpleQuote(1.0 / spot)),
        )
    )
    return forward.NPV()


def build_fx_forward() -> dict[str, Any]:
    """Build a native QuantLib one-year EUR/USD forward fixture."""
    spot = 1.10
    domestic_rate = 0.04
    foreign_rate = 0.02

    def rate_shifted_npv(rate_shift: float) -> float:
        return _quantlib_fx_forward_npv(
            spot=spot,
            domestic_rate=domestic_rate + rate_shift,
            foreign_rate=foreign_rate + rate_shift,
        )

    def spot_shifted_npv(relative_shift: float) -> float:
        return _quantlib_fx_forward_npv(
            spot=spot * (1.0 + relative_shift),
            domestic_rate=domestic_rate,
            foreign_rate=foreign_rate,
        )

    expected = {
        "dv01": central_difference(rate_shifted_npv, 0.0),
        "fx01": central_difference(spot_shifted_npv, 0.0, 0.01),
        "npv": _quantlib_fx_forward_npv(
            spot=spot,
            domestic_rate=domestic_rate,
            foreign_rate=foreign_rate,
        ),
    }
    reason = (
        "QuantLib DiscountingFxForwardEngine and Finstack use the same "
        "collateralized no-arbitrage forward valuation with continuous flat curves."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="eurusd_1y_forward_quantlib",
            domain="fx.fx_forward",
            description="QuantLib native one-year long-EUR EUR/USD forward.",
            product="fx_forward",
        ),
        "kind": "pricing",
        "model": "discounting",
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
        ),
        "instrument": {
            "type": "fx_forward",
            "spec": {
                "id": "EURUSD-1Y-FORWARD-QUANTLIB",
                "base_currency": "EUR",
                "quote_currency": "USD",
                "maturity": "2027-04-30",
                "notional": {"amount": "1000000", "currency": "EUR"},
                "contract_rate": spot,
                "domestic_discount_curve_id": "USD-OIS",
                "foreign_discount_curve_id": "EUR-OIS",
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
            },
        },
        "expected": expected,
        "tolerances": {
            "dv01": tolerance(0.01, reason),
            "fx01": tolerance(0.01, reason),
            "npv": tolerance(0.01, reason),
        },
    }
