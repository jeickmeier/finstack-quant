"""
Core financial primitives: dates, currencies, money, market data, math.

Bindings for the ``finstack-quant-core`` Rust crate.  Each submodule is
re-exported here and registered in ``sys.modules`` so that both
``from finstack_quant.core import dates`` and ``import finstack_quant.core.dates``
work transparently.

Examples
--------
>>> from finstack_quant.core import dates
>>> dates.Tenor.parse("3M").months
3

"""

from finstack_quant.core import cashflow as cashflow
from finstack_quant.core import config as config
from finstack_quant.core import currency as currency
from finstack_quant.core import dates as dates
from finstack_quant.core import market_data as market_data
from finstack_quant.core import math as math
from finstack_quant.core import money as money
from finstack_quant.core import rating_scales as rating_scales
from finstack_quant.core import schema as schema
from finstack_quant.core import table as table
from finstack_quant.core import types as types

class FinstackError(ValueError):
    """
    Base class for every named exception this library defines.

    ``except FinstackError`` catches ``AnalyticsError``, ``PortfolioError``
    and its subclasses, and ``ContractValidationError`` and its subclasses.
    It inherits ``ValueError`` so that code written before this base existed
    keeps working unchanged.

    Note
    ----
    Errors mapped from core failures are still raised as plain ``ValueError``
    or ``KeyError``, so ``except FinstackError`` is narrower than "anything
    this library can raise".

    Examples
    --------
    >>> from finstack_quant.core import FinstackError
    >>> issubclass(FinstackError, ValueError)
    True
    """

__all__ = [
    "FinstackError",
    "cashflow",
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
