"""Explicit synthetic accounting history and aligned statistical teaching data.

These datasets are invented, not observed investment performance. Dated books,
markets and trade/flow instructions are inputs to native pricing and performance
APIs; no NAV or return formula is hidden here. The longer daily panel supplies aligned
observations for risk estimation, regression and optimization exercises.

>>> len(performance_history())
3
>>> risk_panel().shape[0]
504
"""

from __future__ import annotations

from datetime import date
from itertools import pairwise
import json
from typing import Any

import numpy as np
import pandas as pd

from finstack_quant.calibration import calibrate
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import FxMatrix, MarketContext

from .analyst_book import AS_OF, book_spec, build_market, calibration_envelope

RISK_SEED = 20260824
ASSET_COLUMNS = ("government", "corporate", "equity", "cash")
FACTOR_COLUMNS = ("market", "rates", "credit")
HISTORY_DATES = (AS_OF, date(2025, 4, 15), date(2025, 7, 16), date(2025, 10, 15))


def performance_history() -> list[dict[str, Any]]:
    """Return three periods whose NAVs must be obtained from native repricing.

    Returns:
        Fresh start/end ISO dates, reporting currency and illustrative
        benchmark simple returns. Obtain positions from ``history_spec`` and
        the market from ``history_market``. Carry settled contractual payments
        and trades in an explicit USD cash ledger; external flows change that
        ledger at their actual dates. No NAV or portfolio return is invented.

    Notes:
        This fixed data factory does not raise exceptions.

    >>> history = performance_history()
    >>> history[0]["end"] == history[1]["start"]
    True
    """
    return [
        {"start": start.isoformat(), "end": end.isoformat(), "currency": "USD", "benchmark_return": benchmark}
        for start, end, benchmark in zip(HISTORY_DATES[:-1], HISTORY_DATES[1:], (0.012, -0.004, 0.010), strict=True)
    ]


def history_events() -> list[dict[str, Any]]:
    """Return dated external cash and executed-unit trade instructions.

    Returns:
        Fresh event records. ``external_flow.amount`` is USD cash into the
        account (negative for withdrawal). ``trade.quantity_change`` is in
        native instrument units and executes at that date's computed per-unit
        PV, with the opposite amount booked to cash and no transaction costs.
        Contractual payments are extracted from the native cashflow API rather
        than copied into this ledger. Settled USD cash earns zero interest.

    Notes:
        This fixed data factory does not raise exceptions.

    >>> history_events()[1]["instrument_id"]
    'SPX-CALL'
    """
    return [
        {"date": "2025-02-28", "type": "external_flow", "amount": 250_000.0},
        {"date": "2025-04-30", "type": "trade", "instrument_id": "SPX-CALL", "quantity_change": 0.5},
        {"date": "2025-05-30", "type": "external_flow", "amount": -100_000.0},
        {"date": "2025-08-29", "type": "external_flow", "amount": 150_000.0},
        {"date": "2025-09-15", "type": "trade", "instrument_id": "SPX-CALL", "quantity_change": -0.25},
    ]


def history_spec(as_of: date) -> dict[str, Any]:
    """Return a fresh dated base book after trades and maturity settlements.

    Args:
        as_of: Snapshot date within ``HISTORY_DATES``; same-day trades have
            already executed. Matured deposits are retired after transferring
            their redemption to cash. Use previous-day held positions, with
            ``as_of`` advanced, when extracting same-day contractual payments.

    Returns:
        Native portfolio specification. External flows and contractual paid
        cash remain in the separately visible cash ledger, not position PV.

    Raises:
        ValueError: If the date is outside the history window.

    >>> history_spec(date(2025, 5, 1))["positions"][5]["quantity"]
    1.5
    """
    _history_fraction(as_of)
    spec = book_spec("base")
    spec["as_of"] = as_of.isoformat()
    for event in history_events():
        if event["type"] == "trade" and date.fromisoformat(event["date"]) <= as_of:
            for position in spec["positions"]:
                if position["instrument_id"] == event["instrument_id"]:
                    position["quantity"] += event["quantity_change"]
    spec["positions"] = [
        position
        for position in spec["positions"]
        if not (
            position["instrument_spec"]["type"] == "deposit"
            and date.fromisoformat(position["instrument_spec"]["spec"]["maturity"]) <= as_of
        )
    ]
    return spec


def _history_fraction(as_of: date) -> tuple[int, float]:
    if as_of < HISTORY_DATES[0] or as_of > HISTORY_DATES[-1]:
        raise ValueError(f"history date must lie in {HISTORY_DATES[0]}..{HISTORY_DATES[-1]}")
    for index, (start, end) in enumerate(pairwise(HISTORY_DATES)):
        if as_of <= end:
            return index, (as_of - start).days / (end - start).days
    raise AssertionError("validated history date has no enclosing interval")


def history_quotes(as_of: date) -> dict[str, float]:
    """Return the deterministic market observations for a history date.

    Args:
        as_of: Date in the inclusive history window.

    Returns:
        Interpolated synthetic observations: USD rate shift in decimals, CDS
        quote shift in bp, SPX level, and USD per EUR. Intermediate event dates
        interpolate between the four explicit endpoint observations.

    Raises:
        ValueError: If the date lies outside the history window.

    >>> history_quotes(AS_OF)["spx"]
    5200.0
    """
    index, fraction = _history_fraction(as_of)
    observations = {
        "rate_shift": (0.0, -0.0025, 0.0010, 0.0005),
        "credit_shift_bp": (0.0, 20.0, 40.0, 10.0),
        "spx": (5200.0, 5350.0, 5000.0, 5500.0),
        "eur_usd": (1.08, 1.07, 1.10, 1.09),
    }
    return {key: values[index] + fraction * (values[index + 1] - values[index]) for key, values in observations.items()}


def history_market(as_of: date) -> MarketContext:
    """Return a fresh calibrated market snapshot for historical book repricing.

    Args:
        as_of: Date in the inclusive history window.

    Returns:
        Rolled base market with USD rates and ACME hazard jointly recalibrated
        from date-specific quotes, and dated SPX and EUR/USD observations.
        Forward tenor quotes, fixings, EUR discount factors and option vols
        follow the disclosed base fixture conventions.

    Raises:
        ValueError: If the date is outside the window or market data is invalid.
        RuntimeError: If native calibration fails.
        OSError: If the canonical calibration example cannot be read.

    >>> history_market(AS_OF).get_discount("USD-OIS").id
    'USD-OIS'
    """
    quotes = history_quotes(as_of)
    market = build_market("base", as_of)
    envelope = calibration_envelope("base", as_of)
    for quote in envelope["market_data"]:
        if quote["kind"] == "rate_quote":
            quote["rate"] += quotes["rate_shift"]
        elif quote["kind"] == "cds_quote":
            quote["spread_bp"] += quotes["credit_shift_bp"]
    calibrated = calibrate(json.dumps(envelope)).market
    market.insert(calibrated.get_discount("USD-OIS"))
    market.insert(calibrated.get_hazard("ACME-HZD"))
    market.insert_price("SPX-SPOT", quotes["spx"], "USD")
    fx = FxMatrix()
    fx.set_quote(Currency("EUR"), Currency("USD"), quotes["eur_usd"])
    market.insert_fx(fx)
    return market


def risk_panel() -> pd.DataFrame:
    """Return 504 aligned synthetic daily simple returns with a fixed seed.

    Returns:
        Fresh date-indexed frame, all returns in decimal fractions per day.
        Columns include four investable asset proxies, three factors, a
        rebalanced portfolio (20/35/35/10 percent), its benchmark
        (30/40/20/10 percent), and a daily risk-free return. Factor exposures
        generate the proxy returns; these are not repriced instrument returns.
        Dates are weekdays, not an exchange trading calendar.

    Notes:
        This fixed data factory does not raise exceptions.

    >>> panel = risk_panel()
    >>> panel.index.is_unique and not panel.isna().any().any()
    True
    """
    rng = np.random.default_rng(RISK_SEED)
    factors = rng.normal(size=(504, 3)) * np.array([0.009, 0.0025, 0.0035])
    factors += np.array([0.0003, 0.00005, 0.00008])
    residual = rng.normal(size=(504, 4)) * np.array([0.0003, 0.0006, 0.002, 0.00001])
    loadings = np.array([[0.02, 1.0, 0.0], [0.10, 0.75, 0.8], [1.05, -0.10, 0.05], [0.0, 0.0, 0.0]])
    asset_returns = factors @ loadings.T + residual + 0.04 / 252.0
    panel = pd.DataFrame(asset_returns, index=pd.bdate_range("2023-02-08", periods=504), columns=ASSET_COLUMNS)
    panel.index.name = "date"
    panel[list(FACTOR_COLUMNS)] = factors
    panel["risk_free"] = 0.04 / 252.0
    panel["portfolio"] = asset_returns @ np.array([0.20, 0.35, 0.35, 0.10])
    panel["benchmark"] = asset_returns @ np.array([0.30, 0.40, 0.20, 0.10])
    return panel
