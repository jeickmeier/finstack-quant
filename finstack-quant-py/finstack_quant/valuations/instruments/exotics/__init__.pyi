"""Direct exotic valuation instrument wrappers.

Python-facing wrappers for Asian, barrier, and lookback options plus multi-underlier
baskets. Specs convert to canonical Rust tagged ``InstrumentJson`` before
validation and pricing.
"""

from __future__ import annotations

from typing import Any

class _ExoticInstrument:
    """Base API for direct exotic valuation instrument wrappers.

    Build from a spec dict, JSON string, or keyword fields; validate and price
    via the Rust valuation engine.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments.exotics import AsianOption
    >>> asian = AsianOption.from_json(json_str)  # doctest: +SKIP
    >>> result_json = asian.price(market, "2025-06-30")  # doctest: +SKIP
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build an exotic instrument from a spec dict, JSON string, or keyword fields.

        Parameters
        ----------
        spec : dict or str, optional
            Instrument specification dict or tagged JSON string.
        **kwargs
            Keyword fields merged into the spec when ``spec`` is omitted.

        Raises
        ------
        ValueError
            If the constructor inputs are insufficient or invalid.
        """
        ...

    @staticmethod
    def from_json(json: str) -> "_ExoticInstrument":
        """Deserialize and validate this exotic instrument from tagged JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` string.

        Returns
        -------
        _ExoticInstrument
            Validated instrument wrapper.

        Raises
        ------
        ValueError
            If JSON is malformed, the type tag mismatches, or validation fails.
        """
        ...

    def to_json(self) -> str:
        """Serialize this exotic instrument to canonical tagged JSON.

        Returns
        -------
        str
            Pretty-printed tagged instrument JSON.
        """
        ...

    def validate(self) -> None:
        """Validate the instrument spec without pricing it.

        Raises
        ------
        ValueError
            If the spec is malformed or violates instrument invariants.
        """
        ...

    def price(self, market: Any, as_of: str, model: str = "default") -> str:
        """Price this exotic instrument and return ``ValuationResult`` JSON.

        Parameters
        ----------
        market : MarketContext or str
            Typed market context or serialized JSON with discount curves, forwards,
            and vol surfaces required by the instrument.
        as_of : str
            Valuation date in ISO 8601 ``YYYY-MM-DD`` form.
        model : str, optional
            Pricing model selector. ``"default"`` uses the instrument's registered
            default model (often Monte Carlo or analytic per product).

        Returns
        -------
        str
            JSON-encoded ``ValuationResult``.

        Raises
        ------
        ValueError
            If required market data is missing or pricing fails.
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
        """Price this exotic instrument and compute requested valuation metrics.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.
        metrics : list[str]
            Fully qualified metric IDs to compute alongside PV.
        pricing_options : str, optional
            JSON-encoded pricing options (bump sizes, MC path counts).
        market_history : str, optional
            JSON-encoded market history for path-dependent metrics.

        Returns
        -------
        str
            JSON-encoded ``ValuationResult`` with metrics populated.

        Raises
        ------
        ValueError
            If metrics are unknown or pricing fails.
        """
        ...

class _ExoticOptionInstrument(_ExoticInstrument):
    """Exotic option wrapper with direct scalar Greek helpers."""

    def delta(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option delta under the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        float
            Delta with respect to the primary underlier.

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def gamma(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option gamma under the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        float
            Gamma (second derivative of PV w.r.t. spot).

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def vega(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option vega under the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        float
            Vega per 1% absolute move in implied volatility.

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def theta(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option theta under the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        float
            Theta (PV sensitivity to one calendar day).

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def rho(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option rho under the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        float
            Rho per 1% absolute move in the discount curve.

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def vanna(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option vanna under the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        float
            Cross sensitivity of delta to volatility.

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def volga(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return option volga under the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        float
            Volga (second derivative of PV w.r.t. implied volatility).

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def greeks(self, market: Any, as_of: str, model: str = "default") -> dict[str, float]:
        """Return all supported exotic option Greeks as a dict.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector.

        Returns
        -------
        dict[str, float]
            Map of Greek name to value.

        Raises
        ------
        ValueError
            If pricing or any metric computation fails.
        """
        ...

class AsianOption(_ExoticOptionInstrument):
    """Asian (average-price) option on arithmetic or geometric averaging."""

class BarrierOption(_ExoticOptionInstrument):
    """Barrier option (knock-in/knock-out on a single underlier)."""

class LookbackOption(_ExoticOptionInstrument):
    """Lookback option on the running extremum of the underlier."""

class Basket(_ExoticInstrument):
    """Basket instrument for multi-underlier valuation."""

__all__ = [
    "AsianOption",
    "BarrierOption",
    "Basket",
    "LookbackOption",
]
