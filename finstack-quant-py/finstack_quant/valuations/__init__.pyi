"""Instrument pricing, risk metrics, cashflow inspection, and composites.

Where things live:

- Market data (``DiscountCurve``, ``ForwardCurve``, ``HazardCurve``,
  ``MarketContext``, ``FxMatrix``): :mod:`finstack_quant.core.market_data`;
  curve bootstrapping and quote ingestion: :mod:`finstack_quant.calibration`.
- Instruments, builders and :func:`~finstack_quant.valuations.instruments.price_instrument`:
  :mod:`finstack_quant.valuations.instruments`.
- Results: :class:`ValuationResult` (here); per-flow cashflow tables come from
  :func:`~finstack_quant.valuations.instruments.instrument_cashflows`.
- Composites, credit-derivative examples, listed-market catalog and JSON
  schemas: ``.composite``, ``.market``, ``.schema``.

Examples
--------
>>> from finstack_quant.valuations import instruments
>>> hasattr(instruments, "price_instrument")
True

"""

from __future__ import annotations

from collections.abc import Sequence

import datetime
from typing import Any

import pandas as pd
import polars as pl

from finstack_quant.core.dates import StubKind
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.valuations import composite as composite
from finstack_quant.valuations import instruments as instruments
from finstack_quant.valuations import market as market
from finstack_quant.valuations import schema as schema

from finstack_quant.valuations._margin import calculate_simm as calculate_simm

from finstack_quant.valuations._result import (
    ValuationResult as ValuationResult,
)

__all__ = [
    "composite",
    "instruments",
    "market",
    "schema",
    "ValuationResult",
    "calculate_simm",
]
