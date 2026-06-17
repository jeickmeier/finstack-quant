"""Direct exotic valuation instrument wrappers."""

from __future__ import annotations

from typing import Any

class _ExoticInstrument:
    """Base API for direct exotic valuation instrument wrappers.

    Concrete classes convert Python dicts, JSON strings, or keyword arguments
    into canonical Rust tagged instrument JSON before validation and pricing.
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build from a spec dict, JSON string, or keyword fields."""
        ...

    @staticmethod
    def from_json(json: str) -> "_ExoticInstrument":
        """Deserialize and validate this exotic instrument from tagged JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this exotic instrument to canonical tagged JSON."""
        ...

    def validate(self) -> None:
        """Validate the instrument spec without pricing it."""
        ...

    def price(self, market: Any, as_of: str, model: str = "default") -> str:
        """Price this exotic instrument and return ``ValuationResult`` JSON."""
        ...

    def price_with_metrics(
        self,
        market: Any,
        as_of: str,
        model: str = "default",
        metrics: list[str] = ...,
        pricing_options: str | None = None,
        market_history: str | None = None,
    ) -> str:
        """Price this exotic instrument and compute requested metrics."""
        ...

class _ExoticOptionInstrument(_ExoticInstrument):
    """Exotic option wrapper with direct scalar Greek helpers."""

    def delta(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option delta under the selected model."""
        ...

    def gamma(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option gamma under the selected model."""
        ...

    def vega(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option vega under the selected model."""
        ...

    def theta(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option theta under the selected model."""
        ...

    def rho(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option rho under the selected model."""
        ...

    def vanna(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option vanna under the selected model."""
        ...

    def volga(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option volga under the selected model."""
        ...

    def greeks(self, market: Any, as_of: str, model: str = "default") -> dict[str, float]:
        """Return all supported exotic option Greeks as a dict."""
        ...

class AsianOption(_ExoticOptionInstrument):
    """Asian option instrument."""

class BarrierOption(_ExoticOptionInstrument):
    """Barrier option instrument."""

class LookbackOption(_ExoticOptionInstrument):
    """Lookback option instrument."""

class Basket(_ExoticInstrument):
    """Basket instrument for multi-underlier valuation."""

__all__: list[str]
