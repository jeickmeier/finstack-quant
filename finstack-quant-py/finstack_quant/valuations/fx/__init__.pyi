"""Direct FX valuation instrument wrappers."""

from __future__ import annotations

from typing import Any

class _FxInstrument:
    """Base API for direct FX valuation instrument wrappers.

    Concrete classes include spot, forward, swap, NDF, vanilla/digital/touch/
    barrier options, variance swaps, and quanto options. Inputs are converted
    to the canonical Rust tagged instrument JSON; pricing delegates to the Rust
    valuation engine.
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build from a spec dict, JSON string, or keyword fields."""
        ...

    @staticmethod
    def from_json(json: str) -> "_FxInstrument":
        """Deserialize and validate this FX instrument from tagged JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this FX instrument to canonical tagged JSON."""
        ...

    def validate(self) -> None:
        """Validate the instrument spec without pricing it.

        Raises:
            ValueError: If the spec is malformed or violates instrument
                invariants.
        """
        ...

    def price(self, market: Any, as_of: str, model: str = "default") -> str:
        """Price this FX instrument and return ``ValuationResult`` JSON.

        Args:
            market: ``MarketContext`` or serialized market-context JSON.
            as_of: ISO 8601 valuation date.
            model: Pricing model selector, defaulting to the instrument's Rust
                default model.
        """
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
        """Price this FX instrument and compute requested valuation metrics."""
        ...

class _FxOptionInstrument(_FxInstrument):
    """FX option wrapper with direct scalar Greek helpers."""

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
        """Return domestic-rate rho under the selected model."""
        ...

    def foreign_rho(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return foreign-rate rho under the selected model."""
        ...

    def vanna(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option vanna under the selected model."""
        ...

    def volga(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option volga under the selected model."""
        ...

    def greeks(self, market: Any, as_of: str, model: str = "default") -> dict[str, float]:
        """Return all supported FX option Greeks as a dict."""
        ...

class FxSpot(_FxInstrument):
    """FX spot instrument."""

class FxForward(_FxInstrument):
    """Deliverable FX forward instrument."""

class FxSwap(_FxInstrument):
    """FX swap instrument."""

class Ndf(_FxInstrument):
    """Non-deliverable FX forward instrument."""

class FxOption(_FxOptionInstrument):
    """Vanilla FX option instrument."""

class FxDigitalOption(_FxOptionInstrument):
    """Digital FX option instrument."""

class FxTouchOption(_FxOptionInstrument):
    """Touch/no-touch FX option instrument."""

class FxBarrierOption(_FxOptionInstrument):
    """Barrier FX option instrument."""

class FxVarianceSwap(_FxInstrument):
    """FX variance swap instrument."""

class QuantoOption(_FxOptionInstrument):
    """Quanto option instrument with FX-adjusted payoff."""

__all__: list[str]
