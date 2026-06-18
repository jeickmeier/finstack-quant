"""Instrument wrappers and JSON helpers for ``finstack_quant.valuations``.

This mirrors ``finstack_quant_valuations::instruments``: category-specific
wrappers live in submodules, while JSON pricing helpers are exported here.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations
from finstack_quant.valuations.instruments import (
    commodity as commodity,
    credit_derivatives as credit_derivatives,
    equity as equity,
    exotics as exotics,
    fixed_income as fixed_income,
    fx as fx,
    rates as rates,
)

validate_instrument_json = _valuations.instruments.validate_instrument_json
price_instrument = _valuations.instruments.price_instrument
price_instrument_with_metrics = _valuations.instruments.price_instrument_with_metrics
instrument_cashflows_json = _valuations.instruments.instrument_cashflows_json
list_standard_metrics = _valuations.instruments.list_standard_metrics
list_standard_metrics_grouped = _valuations.instruments.list_standard_metrics_grouped

__all__: list[str] = [
    "commodity",
    "credit_derivatives",
    "equity",
    "exotics",
    "fixed_income",
    "fx",
    "instrument_cashflows_json",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "price_instrument",
    "price_instrument_with_metrics",
    "rates",
    "validate_instrument_json",
]
