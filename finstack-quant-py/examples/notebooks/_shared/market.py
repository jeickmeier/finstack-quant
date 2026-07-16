"""Canonical typed market data used by notebook examples."""

from __future__ import annotations

from datetime import date, timedelta

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import (
    DiscountCurve,
    ForwardCurve,
    FxMatrix,
    HazardCurve,
    MarketContext,
    ScalarTimeSeries,
)

DEMO_AS_OF = date(2025, 1, 15)


def build_demo_market(as_of: date = DEMO_AS_OF) -> MarketContext:
    """Build the deterministic cross-asset market used by shared fixtures."""
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            as_of,
            [
                (0.0, 1.0),
                (0.25, 0.9888),
                (0.5, 0.9775),
                (1.0, 0.955),
                (2.0, 0.91),
                (3.0, 0.87),
                (5.0, 0.80),
                (10.0, 0.65),
            ],
            day_count="act_365f",
        )
    )
    market.insert(
        DiscountCurve(
            "EUR-OIS",
            as_of,
            [(0.0, 1.0), (1.0, 0.97), (3.0, 0.91), (5.0, 0.85)],
            day_count="act_365f",
        )
    )
    market.insert(
        ForwardCurve(
            "USD-SOFR-3M",
            0.25,
            knots=[(0.0, 0.045), (1.0, 0.047), (3.0, 0.049), (10.0, 0.052)],
            base_date=as_of,
            day_count="act_360",
        )
    )
    market.insert(
        HazardCurve(
            "CORP-HAZARD",
            as_of,
            [(1.0, 0.02), (3.0, 0.024), (5.0, 0.028), (10.0, 0.032)],
            recovery_rate=0.40,
            par_spreads=[
                (1.0, 116.639125),
                (3.0, 134.717567),
                (5.0, 147.417265),
                (10.0, 166.063391),
            ],
        )
    )

    fixing_start = date(2023, 12, 1)
    fixings: list[tuple[date, float]] = []
    current = fixing_start
    while current <= as_of:
        elapsed = max((current - date(2024, 1, 1)).days, 0) / 365.0
        fixings.append((current, 0.045 + 0.003 * min(elapsed, 1.0)))
        current += timedelta(days=1)
    market.insert_series(ScalarTimeSeries("FIXING:USD-SOFR-3M", fixings))

    market.insert_price("AAPL-SPOT", 185.0, "USD")
    market.insert_price("AAPL-DIV", 0.005)
    market.insert_price("SPX-SPOT", 5200.0, "USD")
    market.insert_price("SPX-DIV", 0.015)

    fx = FxMatrix()
    fx.set_quote(Currency("EUR"), Currency("USD"), 1.08)
    market.insert_fx(fx)
    return market
