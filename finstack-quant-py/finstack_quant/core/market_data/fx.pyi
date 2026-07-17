"""
FX types exposed by ``core.market_data.fx``.

Examples
--------
>>> import finstack_quant.core.market_data.fx as fx
>>> fx.__name__
'finstack_quant.core.market_data.fx'
"""

from finstack_quant.core.market_data import FxConversionPolicy as FxConversionPolicy
from finstack_quant.core.market_data import FxMatrix as FxMatrix
from finstack_quant.core.market_data import FxRateResult as FxRateResult

__all__ = ["FxConversionPolicy", "FxRateResult", "FxMatrix"]
