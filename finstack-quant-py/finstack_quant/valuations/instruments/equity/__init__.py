"""Direct equity valuation instrument wrappers.

Mirrors ``finstack_quant_valuations::instruments::equity``.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

Equity = _valuations.instruments.equity.Equity
EquityOption = _valuations.instruments.equity.EquityOption
VarianceSwap = _valuations.instruments.equity.VarianceSwap
EquityIndexFuture = _valuations.instruments.equity.EquityIndexFuture
VolatilityIndexFuture = _valuations.instruments.equity.VolatilityIndexFuture
VolatilityIndexOption = _valuations.instruments.equity.VolatilityIndexOption
Autocallable = _valuations.instruments.equity.Autocallable
CliquetOption = _valuations.instruments.equity.CliquetOption
EquityTotalReturnSwap = _valuations.instruments.equity.EquityTotalReturnSwap
PrivateMarketsFund = _valuations.instruments.equity.PrivateMarketsFund
RealEstateAsset = _valuations.instruments.equity.RealEstateAsset
LeveredRealEstateEquity = _valuations.instruments.equity.LeveredRealEstateEquity
DiscountedCashFlow = _valuations.instruments.equity.DiscountedCashFlow

__all__: list[str] = [
    "Autocallable",
    "CliquetOption",
    "DiscountedCashFlow",
    "Equity",
    "EquityIndexFuture",
    "EquityOption",
    "EquityTotalReturnSwap",
    "LeveredRealEstateEquity",
    "PrivateMarketsFund",
    "RealEstateAsset",
    "VarianceSwap",
    "VolatilityIndexFuture",
    "VolatilityIndexOption",
]
