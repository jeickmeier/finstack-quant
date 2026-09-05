"""Fresh synthetic liquidity observations and aligned position-factor panels.

Capacity is an explicit teaching assumption, especially for OTC instruments.
It is never inferred from outstanding notional or represented as market data.

>>> liquidity_panel().loc["USD-CORP", "quantity"]
1.0
"""

from __future__ import annotations

import numpy as np
import pandas as pd

from .analyst_history import risk_panel


def liquidity_panel() -> pd.DataFrame:
    """Return common-book liquidity inputs in consistent position-unit terms.

    Returns:
        Fresh frame keyed by native instrument ID. Quantity and daily capacity
        both use full-contract position units; ``notional_per_unit`` states the
        currency amount in one unit. Spread is bid-ask width as a decimal
        fraction of mid price. Cash has no liquidation spread; the unquoted
        term loan has unavailable capacity, represented by NaN rather than zero.

    >>> len(liquidity_panel())
    8
    """
    observations = [
        ("USD-CORP", "USD", "corporate", 2_000_000, 0.0040, 0.10, "synthetic OTC"),
        ("EUR-GOVT", "EUR", "government", 1_500_000, 0.0008, 1.00, "synthetic OTC"),
        ("USD-CASH", "USD", "cash", 500_000, 0.0, np.nan, "cash deposit; contractual maturity"),
        ("EUR-CASH", "EUR", "cash", 250_000, 0.0, np.nan, "cash deposit; contractual maturity"),
        ("USD-PAYER-IRS", "USD", "rates", 2_000_000, 0.0020, 0.25, "synthetic OTC"),
        ("SPX-CALL", "USD", "equity", 100, 0.0150, 0.50, "synthetic option position capacity"),
        ("ACME-CDS", "USD", "credit", 1_000_000, 0.0075, 0.20, "synthetic OTC"),
        ("BORROWER-TL", "USD", "private credit", 1_000_000, 0.0200, np.nan, "unavailable capacity; no quote"),
    ]
    frame = pd.DataFrame(
        observations,
        columns=[
            "position_id",
            "currency",
            "sector",
            "notional_per_unit",
            "spread_fraction",
            "daily_capacity",
            "source",
        ],
    ).set_index("position_id")
    frame["quantity"] = 1.0
    frame["quantity_unit"] = "full-contract position units"
    frame["capacity_unit"] = "full-contract position units/day"
    frame["spread_unit"] = "fraction of mid value"
    frame.attrs["assumption"] = "Invented executable teaching data; not observed liquidity or a firm trading limit."
    return frame


def position_factor_panel() -> pd.DataFrame:
    """Return aligned synthetic normalized P&L, factors and benchmark observations.

    Returns:
        Fresh 504-day frame with native position IDs, market/rates/credit factors,
        benchmark and risk-free columns on identical dates. Position values are
        modeled P&L divided by fixed risk capital, not actual investment returns
        or the three-period book history. For derivatives this denominator avoids
        division by a near-zero PV.         Factor loadings are disclosed teaching inputs.

    >>> position_factor_panel().index.is_unique
    True
    """
    panel = risk_panel()
    aligned = panel[["market", "rates", "credit", "benchmark", "risk_free"]].copy()
    for position, proxy, sign in [
        ("USD-CORP", "corporate", 1),
        ("EUR-GOVT", "government", 1),
        ("USD-CASH", "cash", 1),
        ("EUR-CASH", "cash", 1),
        ("USD-PAYER-IRS", "rates", -1),
        ("SPX-CALL", "equity", 1.5),
        ("ACME-CDS", "credit", -1),
        ("BORROWER-TL", "corporate", 1.2),
    ]:
        aligned[position] = sign * panel[proxy]
    aligned.attrs["units"] = "daily modeled P&L / fixed risk capital; factor returns are decimal fractions"
    aligned.attrs["method"] = "Known synthetic proxy loadings; use dated native repricing for actual book performance."
    return aligned
