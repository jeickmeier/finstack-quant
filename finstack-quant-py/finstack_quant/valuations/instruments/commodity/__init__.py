"""Direct commodity valuation instrument wrappers.

Mirrors ``finstack_quant_valuations::instruments::commodity``.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

CommodityOption = _valuations.instruments.commodity.CommodityOption
CommodityAsianOption = _valuations.instruments.commodity.CommodityAsianOption
CommodityForward = _valuations.instruments.commodity.CommodityForward
CommoditySwap = _valuations.instruments.commodity.CommoditySwap
CommoditySwaption = _valuations.instruments.commodity.CommoditySwaption
CommoditySpreadOption = _valuations.instruments.commodity.CommoditySpreadOption

__all__: list[str] = [
    "CommodityAsianOption",
    "CommodityForward",
    "CommodityOption",
    "CommoditySpreadOption",
    "CommoditySwap",
    "CommoditySwaption",
]
