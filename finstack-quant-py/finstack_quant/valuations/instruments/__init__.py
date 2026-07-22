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

Bond = _valuations.instruments.Bond
TermLoan = _valuations.instruments.TermLoan
validate_instrument_json = _valuations.instruments.validate_instrument_json
bond_from_cashflows_json = _valuations.instruments.bond_from_cashflows_json
price_instrument = _valuations.instruments.price_instrument
price_instrument_with_metrics = _valuations.instruments.price_instrument_with_metrics
instrument_cashflows_json = _valuations.instruments.instrument_cashflows_json
list_models = _valuations.instruments.list_models
list_models_grouped = _valuations.instruments.list_models_grouped
list_standard_metrics = _valuations.instruments.list_standard_metrics
list_standard_metrics_grouped = _valuations.instruments.list_standard_metrics_grouped

# Structured-credit tranche analytics (mirrors WASM; takes a tranche id).
structured_credit_tranche_discount_margin = _valuations.instruments.structured_credit_tranche_discount_margin
structured_credit_tranche_breakeven_cdr = _valuations.instruments.structured_credit_tranche_breakeven_cdr
structured_credit_tranche_oas = _valuations.instruments.structured_credit_tranche_oas
structured_credit_tranche_metrics = _valuations.instruments.structured_credit_tranche_metrics
structured_credit_tranche_scenario_table = _valuations.instruments.structured_credit_tranche_scenario_table

__all__: list[str] = [
    "Bond",
    "TermLoan",
    "bond_from_cashflows_json",
    "instrument_cashflows_json",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "price_instrument",
    "price_instrument_with_metrics",
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
    "validate_instrument_json",
]
