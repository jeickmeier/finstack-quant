"""Direct FX valuation instrument wrappers.

Mirrors ``finstack_quant_valuations::instruments::fx``.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

FxSpot = _valuations.instruments.fx.FxSpot
FxForward = _valuations.instruments.fx.FxForward
FxSwap = _valuations.instruments.fx.FxSwap
Ndf = _valuations.instruments.fx.Ndf
FxOption = _valuations.instruments.fx.FxOption
FxDigitalOption = _valuations.instruments.fx.FxDigitalOption
FxTouchOption = _valuations.instruments.fx.FxTouchOption
FxBarrierOption = _valuations.instruments.fx.FxBarrierOption
FxVarianceSwap = _valuations.instruments.fx.FxVarianceSwap
QuantoOption = _valuations.instruments.fx.QuantoOption

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
