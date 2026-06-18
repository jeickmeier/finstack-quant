from __future__ import annotations

from typing import Any

from finstack_quant.core.market_data import MarketContext

class _RatesInstrument:
    """Base API for direct rates valuation instrument wrappers.

    Concrete classes wrap swaps, cross-currency swaps, inflation products, FRAs,
    futures, caps/floors, swaptions, CMS products, deposits, repos, and rates
    exotics. Specs are converted to canonical Rust tagged ``InstrumentJson``.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments.rates import InterestRateSwap
    >>> swap = InterestRateSwap.from_json(json_str)  # doctest: +SKIP
    >>> result_json = swap.price(market, "2025-06-30")  # doctest: +SKIP
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build a rates instrument from a spec dict, JSON string, or fields.

        Parameters
        ----------
        spec : dict or str, optional
            Instrument specification matching the Rust serde shape, or tagged
            ``InstrumentJson`` with the concrete class' type tag.
        **kwargs
            Keyword fields for the Rust spec, such as fixed/floating leg specs,
            notional, pay/receive side, accrual schedule, discount curve ID,
            projection curve ID, inflation curve ID, collateral fields, option
            expiry, volatility surface ID, or exercise schedule.

        Raises
        ------
        ValueError
            If both ``spec`` and keyword fields are supplied, the type tag
            mismatches, required fields are missing, or validation fails.
        """
        ...

    @staticmethod
    def from_json(json: str) -> _RatesInstrument:
        """Deserialize and validate this rates instrument from tagged JSON.

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
        """Serialize this rates instrument to canonical tagged JSON.

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
            If schedule, curve, collateral, fixing, optionality, or convention
            fields violate Rust instrument invariants.
        """
        ...

    def price(self, market: MarketContext | str, as_of: str, model: str = "default") -> str:
        """Price this rates instrument and return ``ValuationResult`` JSON.

        Parameters
        ----------
        market : MarketContext or str
            Typed market context or serialized JSON. Rates products typically
            require OIS discount curves, projection curves, fixing histories,
            inflation curves, basis curves, collateral data, and volatility
            surfaces referenced by the spec.
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
        See ``docs/REFERENCES.md#isda-2006-definitions`` and
        ``docs/REFERENCES.md#hull-white-1990`` for rate-convention and
        short-rate model references.
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
        """Price this rates instrument and compute requested metrics.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector. Default ``"default"``.
        metrics : list[str]
            Metric IDs, such as ``"dv01"``, ``"convexity"``, ``"theta"``, or
            bucketed curve risk keys supported by the Rust registry.
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

class InterestRateSwap(_RatesInstrument):
    """Plain-vanilla fixed/floating interest-rate swap wrapper."""

class BasisSwap(_RatesInstrument):
    """Floating-for-floating basis swap wrapper."""

class XccySwap(_RatesInstrument):
    """Cross-currency swap wrapper."""

class InflationSwap(_RatesInstrument):
    """Zero-coupon or inflation-linked swap wrapper."""

class YoYInflationSwap(_RatesInstrument):
    """Year-on-year inflation swap wrapper."""

class InflationCapFloor(_RatesInstrument):
    """Inflation cap, floor, or collar wrapper."""

class ForwardRateAgreement(_RatesInstrument):
    """Forward rate agreement wrapper."""

class Swaption(_RatesInstrument):
    """European option on an interest-rate swap."""

class BermudanSwaption(_RatesInstrument):
    """Bermudan option on an interest-rate swap."""

class InterestRateFuture(_RatesInstrument):
    """Interest-rate futures contract wrapper."""

class CapFloor(_RatesInstrument):
    """Interest-rate cap, floor, or collar wrapper."""

class CmsSwap(_RatesInstrument):
    """Constant-maturity swap wrapper."""

class CmsOption(_RatesInstrument):
    """Option on a constant-maturity swap rate."""

class IrFutureOption(_RatesInstrument):
    """Option on an interest-rate future."""

class Deposit(_RatesInstrument):
    """Money-market deposit wrapper."""

class Repo(_RatesInstrument):
    """Repurchase-agreement financing wrapper."""

class RangeAccrual(_RatesInstrument):
    """Range accrual note or coupon wrapper."""

class Tarn(_RatesInstrument):
    """Target redemption note wrapper."""

class Snowball(_RatesInstrument):
    """Snowball or inverse-floater structured note wrapper."""

class CmsSpreadOption(_RatesInstrument):
    """Option on the spread between two CMS rates."""

class CallableRangeAccrual(_RatesInstrument):
    """Callable range accrual wrapper."""

__all__: list[str] = [
    "InterestRateSwap",
    "BasisSwap",
    "XccySwap",
    "InflationSwap",
    "YoYInflationSwap",
    "InflationCapFloor",
    "ForwardRateAgreement",
    "Swaption",
    "BermudanSwaption",
    "InterestRateFuture",
    "CapFloor",
    "CmsSwap",
    "CmsOption",
    "IrFutureOption",
    "Deposit",
    "Repo",
    "RangeAccrual",
    "Tarn",
    "Snowball",
    "CmsSpreadOption",
    "CallableRangeAccrual",
]
