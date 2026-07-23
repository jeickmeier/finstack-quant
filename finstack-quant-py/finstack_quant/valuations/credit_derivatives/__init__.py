"""Canonical example payloads for CDS-family instruments.

Mirrors ``finstack_quant_valuations::instruments::credit_derivatives`` example
constructors; the WASM facade exposes the same four factories under
``valuations.creditDerivatives``. Pricing and validation for CDS instruments
go through the generic JSON entry points in
``finstack_quant.valuations.instruments``.

Examples:
--------
>>> import finstack_quant.valuations.credit_derivatives as credit_derivatives
>>> credit_derivatives.__name__
'finstack_quant.valuations.credit_derivatives'
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

cds_index_example_json = _valuations.credit_derivatives.cds_index_example_json
cds_option_example_json = _valuations.credit_derivatives.cds_option_example_json
cds_tranche_example_json = _valuations.credit_derivatives.cds_tranche_example_json
credit_default_swap_example_json = _valuations.credit_derivatives.credit_default_swap_example_json

__all__: list[str] = [
    "cds_index_example_json",
    "cds_option_example_json",
    "cds_tranche_example_json",
    "credit_default_swap_example_json",
]
