"""
Core financial primitives: dates, currencies, money, market data, math.

Bindings for the ``finstack-quant-core`` Rust crate.  Each submodule is
re-exported here and registered in ``sys.modules`` so that both
``from finstack_quant.core import dates`` and ``import finstack_quant.core.dates``
work transparently.

Examples
--------
>>> import finstack_quant.core as core
>>> core.__name__
'finstack_quant.core'
"""

from finstack_quant.core import config as config
from finstack_quant.core import credit as credit
from finstack_quant.core import currency as currency
from finstack_quant.core import dates as dates
from finstack_quant.core import market_data as market_data
from finstack_quant.core import math as math
from finstack_quant.core import money as money
from finstack_quant.core import rating_scales as rating_scales
from finstack_quant.core import types as types

__all__ = [
    "config",
    "credit",
    "currency",
    "dates",
    "market_data",
    "math",
    "money",
    "rating_scales",
    "types",
]
