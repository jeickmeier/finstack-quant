"""Core financial primitives: dates, currencies, money, market data, math.

Bindings for the ``finstack-core`` Rust crate.  Each submodule is
re-exported here and registered in ``sys.modules`` so that both
``from finstack.core import dates`` and ``import finstack.core.dates``
work transparently.
"""

from finstack.core import config as config
from finstack.core import credit as credit
from finstack.core import currency as currency
from finstack.core import dates as dates
from finstack.core import market_data as market_data
from finstack.core import math as math
from finstack.core import money as money
from finstack.core import rating_scales as rating_scales
from finstack.core import types as types

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
