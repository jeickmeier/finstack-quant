"""Direct FX valuation instrument wrappers."""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

FxSpot = _valuations.fx.FxSpot
FxForward = _valuations.fx.FxForward
FxSwap = _valuations.fx.FxSwap
Ndf = _valuations.fx.Ndf
FxOption = _valuations.fx.FxOption
FxDigitalOption = _valuations.fx.FxDigitalOption
FxTouchOption = _valuations.fx.FxTouchOption
FxBarrierOption = _valuations.fx.FxBarrierOption
FxVarianceSwap = _valuations.fx.FxVarianceSwap
QuantoOption = _valuations.fx.QuantoOption

__all__: list[str] = [
    "FxBarrierOption",
    "FxDigitalOption",
    "FxForward",
    "FxOption",
    "FxSpot",
    "FxSwap",
    "FxTouchOption",
    "FxVarianceSwap",
    "Ndf",
    "QuantoOption",
]
