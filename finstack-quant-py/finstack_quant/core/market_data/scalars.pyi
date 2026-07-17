"""
Scalar market types exposed by ``core.market_data.scalars``.

Examples
--------
>>> import finstack_quant.core.market_data.scalars as scalars
>>> scalars.__name__
'finstack_quant.core.market_data.scalars'
"""

from finstack_quant.core.market_data import InflationIndex as InflationIndex
from finstack_quant.core.market_data import ScalarTimeSeries as ScalarTimeSeries

__all__ = ["ScalarTimeSeries", "InflationIndex"]
