"""Direct exotic valuation instrument wrappers.

Mirrors ``finstack_quant_valuations::instruments::exotics``.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

AsianOption = _valuations.instruments.exotics.AsianOption
BarrierOption = _valuations.instruments.exotics.BarrierOption
LookbackOption = _valuations.instruments.exotics.LookbackOption
Basket = _valuations.instruments.exotics.Basket

__all__: list[str] = [
    "AsianOption",
    "BarrierOption",
    "Basket",
    "LookbackOption",
]
