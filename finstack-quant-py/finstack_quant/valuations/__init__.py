"""Instrument pricing and risk metrics.

Bindings for the ``finstack-quant-valuations`` Rust crate. Where things live:

- Market data (``DiscountCurve``, ``ForwardCurve``, ``HazardCurve``,
  ``MarketContext``, ``FxMatrix``): :mod:`finstack_quant.core.market_data`;
  curve bootstrapping and quote ingestion: :mod:`finstack_quant.calibration`.
- Instruments, builders and :func:`~finstack_quant.valuations.instruments.price_instrument`:
  :mod:`finstack_quant.valuations.instruments`.
- Results: :class:`ValuationResult` (here) and :func:`instrument_cashflows`
  for per-flow tables.
- Composite instruments, credit-derivative examples, the listed-market
  catalog and JSON schemas: :mod:`~finstack_quant.valuations.composite`,
  :mod:`~finstack_quant.valuations.credit_derivatives`,
  :mod:`~finstack_quant.valuations.market`, :mod:`~finstack_quant.valuations.schema`.

The module-level ``*_coupon_profile``, ``cms_spread_option_intrinsic`` and
``callable_range_accrual_accrued`` functions are deterministic exotic-rates
helpers that need no market data.

Examples:
--------
>>> from finstack_quant.valuations import instruments
>>> hasattr(instruments, "price_instrument")
True

"""

import json as _json
import sys as _sys
from typing import TYPE_CHECKING as _TYPE_CHECKING, Any as _Any

from finstack_quant.finstack_quant import valuations as _valuations
from finstack_quant.valuations import (
    composite as composite,
    credit_derivatives as credit_derivatives,
    instruments as instruments,
    market as market,
)

if _TYPE_CHECKING:
    import pandas as pd

ValuationResult = _valuations.ValuationResult
tarn_coupon_profile = _valuations.tarn_coupon_profile
snowball_coupon_profile = _valuations.snowball_coupon_profile
inverse_floater_coupon_profile = _valuations.inverse_floater_coupon_profile
cms_spread_option_intrinsic = _valuations.cms_spread_option_intrinsic
callable_range_accrual_accrued = _valuations.callable_range_accrual_accrued
# `schema` is a compiled submodule with no pure-Python shim package, so alias it
# onto the public dotted path that `import finstack_quant.valuations.schema` uses.
schema = _valuations.schema
_sys.modules.setdefault("finstack_quant.valuations.schema", schema)


def instrument_cashflows(
    instrument: _Any,
    market: _Any,
    as_of: _Any,
    *,
    model: str,
) -> tuple[dict, "pd.DataFrame"]:
    """Per-flow DF / survival / PV DataFrame for a discountable instrument.

    Supports ``model in {"discounting", "hazard_rate"}``. Hazard-rate export
    rejects bonds with call, put, or return-floor rights because static rows
    cannot represent exercise-contingent value. For supported static-flow
    pairs, ``envelope["total_pv"]`` reconciles with ``base_value``.

    Args:
        instrument: Typed instrument (``Bond``, ``InterestRateSwap``, ...) or a
            canonical ``finstack_quant.instrument/1`` JSON envelope.
        market: ``MarketContext`` instance or JSON string.
        as_of: Valuation date (``datetime.date``, ``datetime.datetime``,
            ``pandas.Timestamp`` or ISO 8601 string).
        model: ``"discounting"`` (DF only) or ``"hazard_rate"`` (adds survival
            probability, conditional default probability, and recovery-adjusted
            principal PV). ``"default"`` is not accepted.

    Returns:
        ``(envelope, df)`` where ``envelope`` is the parsed JSON dict and
        ``df`` is a ``pandas.DataFrame`` of the per-flow rows with ``date``
        / ``reset_date`` parsed as ``datetime64``.

    Raises:
        KeyError: If a curve or fixing series the instrument depends on is
            missing from ``market``.
        ValueError: If ``model`` is unsupported, the instrument type isn't
            priced under that model, or a bond with embedded exercise rights
            is requested under a static cashflow model.
        RuntimeError: If the pricer fails numerically.

    Examples:
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import Bond
    >>> as_of = datetime.date(2024, 1, 1)
    >>> bond = Bond.fixed(
    ...     "B", Money(1000.0, Currency("USD")), Rate(0.05), as_of, datetime.date(2026, 1, 1), StubKind.NONE, "USD-OIS"
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> from finstack_quant.valuations import instrument_cashflows
    >>> header, frame = instrument_cashflows(bond, market, as_of, model="discounting")
    >>> (header["instrument_id"], len(frame))
    ('B', 6)

    """
    import pandas as pd

    payload = instruments.instrument_cashflows_json(instrument, market, as_of, model)
    envelope = _json.loads(payload)
    df = pd.DataFrame(envelope["flows"])
    if not df.empty:
        df["date"] = pd.to_datetime(df["date"])
        if "reset_date" in df.columns:
            df["reset_date"] = pd.to_datetime(df["reset_date"])
    return envelope, df


__all__: list[str] = [
    "ValuationResult",
    "callable_range_accrual_accrued",
    "cms_spread_option_intrinsic",
    "composite",
    "credit_derivatives",
    "instrument_cashflows",
    "instruments",
    "inverse_floater_coupon_profile",
    "market",
    "schema",
    "snowball_coupon_profile",
    "tarn_coupon_profile",
]
