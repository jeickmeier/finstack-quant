from __future__ import annotations

from typing import Any

from finstack_quant.core.market_data import MarketContext

class _EquityInstrument:
    """Base API for direct equity valuation instrument wrappers.

    Concrete classes wrap equity spot positions, listed derivatives, structured
    equity products, equity TRS, private-markets funds, real estate, and DCF
    valuation specs. Specs are converted to canonical Rust ``InstrumentJson``.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments.equity import EquityOption
    >>> option = EquityOption.from_json(json_str)  # doctest: +SKIP
    >>> result_json = option.price(market, "2025-06-30")  # doctest: +SKIP
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build an equity instrument from a spec dict, JSON string, or fields.

        Parameters
        ----------
        spec : dict or str, optional
            Instrument specification matching the Rust serde shape, or tagged
            ``InstrumentJson`` for the concrete class.
        **kwargs
            Keyword fields for the Rust spec, such as underlying identifiers,
            notional, strike, maturity, discount/dividend curve IDs, volatility
            surface IDs, payoff schedules, waterfall terms, or DCF assumptions.

        Raises
        ------
        ValueError
            If both ``spec`` and keyword fields are supplied, the type tag
            mismatches, required fields are missing, or validation fails.
        """
        ...

    @staticmethod
    def from_json(json: str) -> _EquityInstrument:
        """Deserialize and validate this equity instrument from tagged JSON.

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
        """Serialize this equity instrument to canonical tagged JSON.

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
        """Price this equity instrument and return ``ValuationResult`` JSON.

        Parameters
        ----------
        market : MarketContext or str
            Typed market context or serialized JSON. Equity products typically
            require spot data, discount curves, dividend curves, volatility
            surfaces, and scenario paths referenced by the instrument.
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
        See ``docs/REFERENCES.md#black-scholes-1973`` and
        ``docs/REFERENCES.md#heston-1993`` for equity option model references.
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
        """Price this equity instrument and compute requested metrics.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector. Default ``"default"``.
        metrics : list[str]
            Metric IDs, such as ``"delta"``, ``"vega"``, ``"hvar"``, or
            ``"expected_shortfall"`` where supported by the Rust registry.
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

class Equity(_EquityInstrument):
    """Equity spot or cash-equity exposure wrapper."""

class EquityOption(_EquityInstrument):
    """Vanilla equity option wrapper."""

class VarianceSwap(_EquityInstrument):
    """Equity variance or volatility swap wrapper."""

class EquityIndexFuture(_EquityInstrument):
    """Equity index futures contract wrapper."""

class VolatilityIndexFuture(_EquityInstrument):
    """Volatility index futures contract wrapper."""

class VolatilityIndexOption(_EquityInstrument):
    """Option on a volatility index future or contract."""

class Autocallable(_EquityInstrument):
    """Autocallable structured equity note wrapper."""

class CliquetOption(_EquityInstrument):
    """Cliquet or ratchet option wrapper."""

class EquityTotalReturnSwap(_EquityInstrument):
    """Equity total return swap wrapper."""

class PrivateMarketsFund(_EquityInstrument):
    """Private-markets fund valuation wrapper."""

class RealEstateAsset(_EquityInstrument):
    """Unlevered real-estate asset valuation wrapper."""

class LeveredRealEstateEquity(_EquityInstrument):
    """Levered real-estate equity waterfall wrapper."""

class DiscountedCashFlow(_EquityInstrument):
    """Discounted-cash-flow equity valuation wrapper."""

__all__ = [
    "Equity",
    "EquityOption",
    "VarianceSwap",
    "EquityIndexFuture",
    "VolatilityIndexFuture",
    "VolatilityIndexOption",
    "Autocallable",
    "CliquetOption",
    "EquityTotalReturnSwap",
    "PrivateMarketsFund",
    "RealEstateAsset",
    "LeveredRealEstateEquity",
    "DiscountedCashFlow",
]
