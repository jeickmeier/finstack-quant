"""Direct exotic valuation instrument wrappers."""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

AsianOption = _valuations.exotics.AsianOption
BarrierOption = _valuations.exotics.BarrierOption
LookbackOption = _valuations.exotics.LookbackOption
Basket = _valuations.exotics.Basket

__all__: list[str] = [
    "AsianOption",
    "BarrierOption",
    "Basket",
    "LookbackOption",
]
