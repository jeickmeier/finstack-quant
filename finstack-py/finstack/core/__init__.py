"""Core financial primitives: dates, currencies, money, market data, math.

Bindings for the ``finstack-core`` Rust crate.
"""

from __future__ import annotations

import sys

from finstack.finstack import core as _core

currency = _core.currency
money = _core.money
config = _core.config
types = _core.types
dates = _core.dates
math = _core.math
market_data = _core.market_data
credit = _core.credit

_submodules = {
    "currency": currency,
    "money": money,
    "config": config,
    "types": types,
    "dates": dates,
    "math": math,
    "market_data": market_data,
    "credit": credit,
    "credit.scoring": credit.scoring,
    "credit.pd": credit.pd,
    "credit.lgd": credit.lgd,
    "credit.migration": credit.migration,
}

for _name, _mod in _submodules.items():
    _key = f"finstack.core.{_name}"
    if _key not in sys.modules:
        sys.modules[_key] = _mod

__all__: list[str] = [
    "config",
    "credit",
    "currency",
    "dates",
    "market_data",
    "math",
    "money",
    "types",
]
