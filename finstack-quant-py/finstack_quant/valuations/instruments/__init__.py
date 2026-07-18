"""Instrument wrappers and JSON helpers for ``finstack_quant.valuations``.

This mirrors ``finstack_quant_valuations::instruments``: category-specific
wrappers live in submodules, while JSON pricing helpers are exported here.

Examples:
--------
>>> import finstack_quant.valuations.instruments as instruments
>>> instruments.__name__
'finstack_quant.valuations.instruments'
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

validate_instrument_json = _valuations.instruments.validate_instrument_json
bond_from_cashflows_json = _valuations.instruments.bond_from_cashflows_json
price_instrument = _valuations.instruments.price_instrument
price_instrument_with_metrics = _valuations.instruments.price_instrument_with_metrics
instrument_cashflows_json = _valuations.instruments.instrument_cashflows_json
list_models = _valuations.instruments.list_models
list_models_grouped = _valuations.instruments.list_models_grouped
list_standard_metrics = _valuations.instruments.list_standard_metrics
list_standard_metrics_grouped = _valuations.instruments.list_standard_metrics_grouped

__all__: list[str] = [
    "bond_from_cashflows_json",
    "instrument_cashflows_json",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "price_instrument",
    "price_instrument_with_metrics",
    "validate_instrument_json",
]
