"""Direct rates valuation instrument wrappers.

Mirrors ``finstack_quant_valuations::instruments::rates``.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

InterestRateSwap = _valuations.instruments.rates.InterestRateSwap
BasisSwap = _valuations.instruments.rates.BasisSwap
XccySwap = _valuations.instruments.rates.XccySwap
InflationSwap = _valuations.instruments.rates.InflationSwap
YoYInflationSwap = _valuations.instruments.rates.YoYInflationSwap
InflationCapFloor = _valuations.instruments.rates.InflationCapFloor
ForwardRateAgreement = _valuations.instruments.rates.ForwardRateAgreement
Swaption = _valuations.instruments.rates.Swaption
BermudanSwaption = _valuations.instruments.rates.BermudanSwaption
InterestRateFuture = _valuations.instruments.rates.InterestRateFuture
CapFloor = _valuations.instruments.rates.CapFloor
CmsSwap = _valuations.instruments.rates.CmsSwap
CmsOption = _valuations.instruments.rates.CmsOption
IrFutureOption = _valuations.instruments.rates.IrFutureOption
Deposit = _valuations.instruments.rates.Deposit
Repo = _valuations.instruments.rates.Repo
RangeAccrual = _valuations.instruments.rates.RangeAccrual
Tarn = _valuations.instruments.rates.Tarn
Snowball = _valuations.instruments.rates.Snowball
CmsSpreadOption = _valuations.instruments.rates.CmsSpreadOption
CallableRangeAccrual = _valuations.instruments.rates.CallableRangeAccrual

__all__: list[str] = [
    "BasisSwap",
    "BermudanSwaption",
    "CallableRangeAccrual",
    "CapFloor",
    "CmsOption",
    "CmsSpreadOption",
    "CmsSwap",
    "Deposit",
    "ForwardRateAgreement",
    "InflationCapFloor",
    "InflationSwap",
    "InterestRateFuture",
    "InterestRateSwap",
    "IrFutureOption",
    "RangeAccrual",
    "Repo",
    "Snowball",
    "Swaption",
    "Tarn",
    "XccySwap",
    "YoYInflationSwap",
]
