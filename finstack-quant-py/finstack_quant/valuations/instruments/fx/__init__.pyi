"""Direct FX valuation instrument wrappers.

Python-facing wrappers for FX spot, forwards, swaps, NDFs, vanilla and exotic
options, variance swaps, and quanto options. Specs are converted to canonical
Rust tagged ``InstrumentJson`` before validation and pricing.

Note
----
FX vol surfaces typically quote delta-based strikes; ensure the selected
``model`` matches the instrument's registered default when using scalar Greek
helpers.
"""

from __future__ import annotations

from typing import Any

class _FxInstrument:
    """Base API for direct FX valuation instrument wrappers.

    Concrete classes include spot, forward, swap, NDF, vanilla/digital/touch/
    barrier options, variance swaps, and quanto options. Build from a spec dict,
    JSON string, or keyword fields; price via the Rust valuation engine.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments.fx import FxForward
    >>> fwd = FxForward.from_json(json_str)  # doctest: +SKIP
    >>> result_json = fwd.price(market, "2025-06-30")  # doctest: +SKIP
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build an FX instrument from a spec dict, JSON string, or keyword fields.

        Parameters
        ----------
        spec : dict or str, optional
            Instrument specification as a dict matching the Rust serde shape, or
            a tagged JSON string. When omitted, ``kwargs`` must supply required
            fields.
        **kwargs
            Keyword fields merged into the spec (e.g. ``pair``, ``notional``,
            ``maturity``).

        Raises
        ------
        ValueError
            If neither ``spec`` nor sufficient ``kwargs`` are provided, or the
            payload fails validation.
        """
        ...

    @staticmethod
    def from_json(json: str) -> "_FxInstrument":
        """Deserialize and validate this FX instrument from tagged JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` string with a type tag matching the concrete
            class.

        Returns
        -------
        _FxInstrument
            Validated instrument wrapper.

        Raises
        ------
        ValueError
            If JSON is malformed, the type tag mismatches, or validation fails.
        """
        ...

    def to_json(self) -> str:
        """Serialize this FX instrument to canonical tagged JSON.

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
        """Price this FX instrument and return ``ValuationResult`` JSON.

        Parameters
        ----------
        market : MarketContext or str
            Typed :class:`~finstack_quant.core.market_data.MarketContext` or
            serialized market-context JSON. Must contain discount curves, FX
            spots/forwards, and vol surfaces referenced by the instrument.
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

        Examples
        --------
        >>> result_json = instrument.price(market, "2025-06-30")  # doctest: +SKIP
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
        """Price this FX instrument and compute requested valuation metrics.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector. Default ``"default"``.
        metrics : list[str]
            Fully qualified metric IDs (e.g. ``"delta"``, ``"vega"``,
            ``"bucketed_dv01::USD-OIS::5y"``).
        pricing_options : str, optional
            JSON-encoded pricing options (bump sizes, finite-difference policy).
        market_history : str, optional
            JSON-encoded market history for metrics requiring a time series.

        Returns
        -------
        str
            JSON-encoded ``ValuationResult`` with requested metrics populated.

        Raises
        ------
        ValueError
            If metrics are unknown or required market data is missing.
        """
        ...

class _FxOptionInstrument(_FxInstrument):
    """FX option wrapper with direct scalar Greek helpers."""

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
            Delta with respect to spot (domestic-currency PV sensitivity convention).

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
            Theta (PV sensitivity to passage of one calendar day).

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def rho(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return domestic-rate rho under the selected model.

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
            Rho per 1% absolute move in the domestic discount curve.

        Raises
        ------
        ValueError
            If pricing or the metric computation fails.
        """
        ...

    def foreign_rho(self, market: Any, as_of: str, model: str = "default") -> float:
        """Return foreign-rate rho under the selected model.

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
            Rho per 1% absolute move in the foreign discount curve.

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
            Cross sensitivity of delta to volatility (or vega to spot).

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
        """Return all supported FX option Greeks as a dict.

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
            Map of Greek name to value (includes delta, gamma, vega, theta, rho,
            foreign_rho, vanna, volga when supported).

        Raises
        ------
        ValueError
            If pricing or any metric computation fails.
        """
        ...

class FxSpot(_FxInstrument):
    """FX spot instrument for immediate exchange at the spot rate."""

class FxForward(_FxInstrument):
    """Deliverable FX forward instrument."""

class FxSwap(_FxInstrument):
    """FX swap combining near and far leg exchanges."""

class Ndf(_FxInstrument):
    """Non-deliverable FX forward settled in domestic currency."""

class FxOption(_FxOptionInstrument):
    """Vanilla FX option (European or American per spec)."""

class FxDigitalOption(_FxOptionInstrument):
    """Digital (binary) FX option."""

class FxTouchOption(_FxOptionInstrument):
    """One-touch or no-touch FX option."""

class FxBarrierOption(_FxOptionInstrument):
    """Barrier FX option (knock-in/knock-out)."""

class FxVarianceSwap(_FxInstrument):
    """FX variance swap on realized variance of the FX rate."""

class QuantoOption(_FxOptionInstrument):
    """Quanto option with FX-adjusted equity or rate payoff."""

__all__ = [
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
