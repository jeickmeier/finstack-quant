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


def usd_ois_curve(as_of: date) -> DiscountCurve:
    """Return the 6-knot USD-OIS curve used by the instrument notebooks.

    This is a deliberately coarse teaching curve, distinct from the richer
    8-knot USD-OIS curve inside :func:`build_demo_market`; the two carry
    DIFFERENT discount factors. This helper exists solely to dedupe the curve
    that the ``02_pricing/instruments`` notebooks build inline, so their printed
    numbers stay stable. Use :func:`build_demo_market` when you want a full
    cross-asset market instead.
    """
    return DiscountCurve(
        "USD-OIS",
        as_of,
        [(0.0, 1.0), (0.5, 0.985), (1.0, 0.97), (3.0, 0.90), (5.0, 0.82), (10.0, 0.65)],
        day_count="act_365f",
    )


def usd_sofr_curve(as_of: date) -> ForwardCurve:
    """Return the 5-knot downward-sloping USD-SOFR-3M forward curve."""
    return ForwardCurve(
        "USD-SOFR-3M",
        0.25,
        knots=[(0.0, 0.052), (1.0, 0.048), (3.0, 0.045), (5.0, 0.043), (10.0, 0.041)],
        base_date=as_of,
        day_count="act_360",
    )


def usd_sofr_fixings(as_of: date) -> ScalarTimeSeries:
    """Return flat 5% daily USD-SOFR-3M fixings from 2024-01-01 through *as_of*."""
    fixing_start = date(2024, 1, 1)
    return ScalarTimeSeries(
        "FIXING:USD-SOFR-3M",
        [(fixing_start + timedelta(days=offset), 0.05) for offset in range((as_of - fixing_start).days + 1)],
    )


def usd_ois_2026(as_of: date = date(2026, 6, 19), shift: float = 0.0) -> DiscountCurve:
    """Return the 7-knot USD-OIS curve used by the reporting tear sheets.

    Args:
        as_of: Curve base date. Defaults to 2026-06-19, the instrument tear
            sheet's as-of date; the attribution tear sheet passes its own
            snapshot dates.
        shift: Subtracted from every non-unit discount factor, which lifts
            implied rates. Use a small positive value to model a sell-off.
    """
    return DiscountCurve(
        "USD-OIS",
        as_of,
        [
            (0.0, 1.0),
            (0.5, 0.98 - shift),
            (1.0, 0.96 - shift),
            (2.0, 0.92 - shift),
            (3.0, 0.88 - shift),
            (5.0, 0.80 - shift),
            (10.0, 0.65 - shift),
        ],
        day_count="act_365f",
    )
