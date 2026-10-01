"""Core financial primitives: dates, currencies, money, market data, math.

Bindings for the ``finstack-quant-core`` Rust crate.

Examples:
>>> from finstack_quant.core import dates
>>> dates.Tenor.parse("3M").months
3

"""

from finstack_quant.finstack_quant import core as _core

currency = _core.currency
money = _core.money
config = _core.config
types = _core.types
dates = _core.dates
math = _core.math
market_data = _core.market_data
rating_scales = _core.rating_scales
table = _core.table

# Canonical home of the shared exception base. Every named exception inherits
# from it except `calibration.CalibrationEnvelopeError`, which derives from
# `RuntimeError` instead — `pyo3::create_exception!` accepts one base type, and
# reparenting it would break existing `except RuntimeError` handlers. The full
# rationale lives beside the declarations in the binding crate's `src/errors.rs`.
FinstackError = _core.FinstackError
schema = _core.schema

__all__: list[str] = [
    "FinstackError",
    "config",
    "currency",
    "dates",
    "market_data",
    "math",
    "money",
    "rating_scales",
    "schema",
    "table",
    "types",
]
