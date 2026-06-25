from __future__ import annotations

from typing import Any

from finstack_quant.core.market_data import MarketContext

class _CommodityInstrument:
    """Base API for direct commodity valuation instrument wrappers.

    Concrete classes wrap Rust commodity forwards, swaps, swaptions, vanilla
    options, Asian options, and spread options. Specs are converted to canonical
    tagged ``InstrumentJson`` and validated by the Rust valuation engine.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments.commodity import CommodityForward
    >>> forward = CommodityForward.from_json(json_str)  # doctest: +SKIP
    >>> result_json = forward.price(market, "2025-06-30")  # doctest: +SKIP
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build a commodity instrument from a spec dict, JSON string, or fields.

        Parameters
        ----------
        spec : dict or str, optional
            Instrument specification matching the Rust serde shape, or tagged
            ``InstrumentJson`` with the concrete class' type tag. When omitted,
            ``kwargs`` must supply all required spec fields.
        **kwargs
            Keyword fields for the Rust spec, such as commodity identifiers,
            quantity, maturity, forward curve IDs, discount curve IDs, strike,
            or averaging schedule fields depending on instrument type.

        Raises
        ------
        ValueError
            If both ``spec`` and keyword fields are supplied, required fields
            are missing, the type tag mismatches, or validation fails.
        """
        ...

    @staticmethod
    def from_json(json: str) -> _CommodityInstrument:
        """Deserialize and validate this commodity instrument from tagged JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` string with a type tag matching the
            concrete class.

        Returns
        -------
        Self
            Validated direct wrapper.

        Raises
        ------
        ValueError
            If JSON is malformed, the tag mismatches, or Rust validation fails.
        """
        ...

    def to_json(self) -> str:
        """Serialize this commodity instrument to canonical tagged JSON.

        Returns
        -------
        str
            Pretty-printed tagged ``InstrumentJson``.
        """
        ...

    def validate(self) -> None:
        """Validate the instrument spec without pricing it.

        Raises
        ------
        ValueError
            If the payload violates instrument invariants or market-convention
            requirements enforced by Rust.
        """
        ...

    def price(self, market: MarketContext | str, as_of: str, model: str = "default") -> str:
        """Price this commodity instrument and return ``ValuationResult`` JSON.

        Parameters
        ----------
        market : MarketContext or str
            Typed market context or serialized market-context JSON. Commodity
            products typically require a discount curve plus commodity forward
            curves and volatility surfaces referenced by the spec.
        as_of : str
            Valuation date in ISO 8601 ``YYYY-MM-DD`` form.
        model : str, optional
            Pricing model selector. ``"default"`` resolves to the instrument's
            registered Rust default model.

        Returns
        -------
        str
            JSON-encoded ``ValuationResult``.

        Raises
        ------
        ValueError
            If required market data is missing or pricing fails.

        Sources
        -------
        See ``docs/REFERENCES.md#black-1976`` for commodity-forward option
        pricing conventions.
        """
        ...

    def price_with_metrics(
        self,
        market: MarketContext | str,
        as_of: str,
        model: str = "default",
        metrics: list[str] = ...,
        pricing_options: str | None = None,
        market_history: str | None = None,
    ) -> str:
        """Price this commodity instrument and compute requested metrics.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector. Default ``"default"``.
        metrics : list[str]
            Metric IDs, such as Greeks, PV01-style sensitivities, or historical
            risk metrics supported by the Rust registry.
        pricing_options : str, optional
            JSON-encoded pricing overrides for bumps, models, or scenarios.
        market_history : str, optional
            JSON-encoded market history for metrics requiring time series.

        Returns
        -------
        str
            JSON-encoded ``ValuationResult`` with requested metrics populated.

        Raises
        ------
        ValueError
            If a metric is unsupported or required market data is missing.
        """
        ...

class CommodityOption(_CommodityInstrument):
    """European option on a commodity forward or futures-style underlying."""

class CommodityAsianOption(_CommodityInstrument):
    """Average-price or average-strike commodity option."""

class CommodityForward(_CommodityInstrument):
    """Forward or futures-style commodity delivery contract."""

class CommoditySwap(_CommodityInstrument):
    """Fixed-for-floating commodity price swap."""

class CommoditySwaption(_CommodityInstrument):
    """Option to enter a commodity swap."""

class CommoditySpreadOption(_CommodityInstrument):
    """Option on the spread between two commodity underlyings."""

__all__ = [
    "CommodityOption",
    "CommodityAsianOption",
    "CommodityForward",
    "CommoditySwap",
    "CommoditySwaption",
    "CommoditySpreadOption",
]
