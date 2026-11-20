"""Typed quanto option and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class QuantoOption:
    """Option on a foreign underlier with a contractual payout conversion into the settlement currency.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import QuantoOption
    >>> isinstance(QuantoOption.example(), QuantoOption)
    True
    """

    @staticmethod
    def builder() -> QuantoOptionBuilder:
        """Start authoring a quanto option.

        Returns
        -------
        QuantoOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import QuantoOption
        >>> _ = QuantoOption.builder().id("example-quanto-option")
        """
        ...

    def to_builder(self) -> QuantoOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        QuantoOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> QuantoOption:
        """Construct the built-in quanto option example.

        Returns
        -------
        QuantoOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import QuantoOption
        >>> QuantoOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> QuantoOption:
        """
        Deserialize a validated QuantoOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"quanto_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        QuantoOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import QuantoOption
        >>> contract = QuantoOption.example()
        >>> QuantoOption.from_json(future.to_json()).id == future.id != ""
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`QuantoOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this quanto option as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        extras: PriceExtras | dict[str, Any] | str | None = None,
    ) -> ValuationResult:
        """
        Price this quanto option and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Registered model key for these terms. The default uses default_model.
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        extras : PriceExtras or dict[str, Any] or str, optional
            The rarer pricing inputs, as a dict or JSON text in the shape of the
            Rust ``PriceExtras``; every key is optional and an unknown key raises
            ``ValueError``. ``metric_pricing_overrides`` (a ``MetricPricingOverrides``
            or dict) is merged into ``instrument.spec.metric_pricing_overrides``
            before pricing (e.g. ``{"theta_period": {"count": 1, "unit": "days"}}``):
            only the keys present replace stored controls, ``bump_config`` keys
            merge into the stored bump configuration and an explicit ``None``
            clears a control; omitted keeps the instrument's overrides. ``market_history`` (a ``MarketHistory`` or dict)
            holds the historical scenarios ``hvar`` and ``expected_shortfall``
            require; omitted prices without history. ``explain`` (bool, default
            ``False``) attaches a step trace as ``result.explanation`` when
            ``True``. ``None`` (the default) means no extras.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required market observation or requested metric key is missing.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...

    def delta(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute spot delta.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Cash PV change per unit of spot, including contract notional.

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def gamma(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute spot gamma.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Cash PV curvature per squared unit of spot, including contract notional.

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def vega(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute option vega.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Cash PV change per volatility point (0.01 absolute implied volatility).

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def theta(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute option theta.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Cash P&L over the configured theta horizon, capped at expiry, with the current spot held fixed.

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def rho(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute domestic rate rho.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Cash PV change per upward basis point of domestic yields.

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def foreign_rho(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute foreign rate rho.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Cash PV change per upward basis point of foreign yields.

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def vanna(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute spot-volatility cross sensitivity.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Cash delta change per volatility point (0.01 absolute implied volatility).

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def volga(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> float:
        """Compute option volatility curvature.

        Parameters
        ----------
        market : MarketContext or str
            Market state providing the configured spot, yield curves and volatility.
        as_of : date or str
            Valuation date for the current contract and observation state.
        model : str
            Registered model to use. The default selects the instrument's canonical model.

        Returns
        -------
        float
            Sensitivity of cash vega to one volatility point (0.01 absolute implied volatility).

        Raises
        ------
        ValueError
            If the date or selected engine is invalid.
        KeyError
            If a required market input or sensitivity is absent.
        RuntimeError
            If pricing or the sensitivity calculation fails.
        """
        ...

    def greeks(self, market: MarketContext | str, as_of: date | str, model: str = "default") -> dict[str, float]:
        """Compute the first- and second-order price sensitivities supported by the selected model.

        Parameters
        ----------
        market : MarketContext or str
            Snapshot containing the configured FX, carry and volatility inputs.
        as_of : date or str
            Valuation date used to determine the remaining option life and observed state.
        model : str
            Registered engine name. The default uses the instrument's standard model.

        Returns
        -------
        dict[str, float]
            Applicable Greeks keyed by their native metric identifiers and in their native units.

        Raises
        ------
        ValueError
            If the date or engine is invalid.
        KeyError
            If a required market input is missing.
        RuntimeError
            If the selected pricing or sensitivity calculation fails.
        """
        ...

    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute a registered scalar metric for this quanto option.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01-style sensitivities, basis points for
            spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If an observation required by the selected metric is missing.
        RuntimeError
            If the metric computation fails.
        """
        ...

    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...

    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            The registered pricing model key selected for these contract terms.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_pricing_overrides(self) -> dict[str, object]:
        """
        Instrument-owned pricing inputs (``InstrumentPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            ``market_quotes`` (quoted clean price, yield, spreads, implied
            volatility, premium) and ``model_config`` (tree model and steps, Monte
            Carlo and Hull-White parameters). Empty sections are omitted; ``{}``
            when none is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metric_pricing_overrides(self) -> dict[str, object]:
        """
        Metric-time pricing configuration (``MetricPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            Bump sizes, theta period and the other metric settings stored on
            the instrument; unset fields are ``None`` or omitted. A
            ``price(..., extras={"metric_pricing_overrides": ...})`` patch is
            merged over it per call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scenario_pricing_overrides(self) -> dict[str, object]:
        """
        Scenario-only pricing adjustments (``ScenarioPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            ``scenario_price_shock_decimal`` and ``scenario_spread_shock_bp``;
            ``{}`` when none is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct the exact contract through its canonical instrument JSON.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and the serialized contract argument.

        Raises
        ------
        ValueError
            If serialization of the contract fails.
        """
        ...

    @property
    def underlying_ticker(self) -> str:
        """Read the contract's underlying ticker.

        Returns
        -------
        str
            Underlying equity ticker symbol

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def strike(self) -> float:
        """Read the contract's strike.

        Returns
        -------
        float
            Per-unit strike of the equity underlying, in base_currency price units.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def option_type(self) -> object:
        """Read the contract's option type.

        Returns
        -------
        object
            Option type (call or put)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def expiry(self) -> date:
        """Read the contract's expiry.

        Returns
        -------
        date
            Option expiry date

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def notional(self) -> Money:
        """Read the contract's notional.

        Returns
        -------
        Money
            Strike-equivalent domestic reference notional. When quantity and payoff_fx_rate are supplied, this must equal quantity * payoff_fx_rate * strike.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quantity(self) -> float | None:
        """Read the contract's quantity.

        Returns
        -------
        float | None
            Number of underlying units covered by the option payoff. Quanto pricing is performed per unit of the underlying in the base currency. This quantity converts the per-unit price into contract-level exposure.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def payoff_fx_rate(self) -> float | None:
        """Read the contract's payoff fx rate.

        Returns
        -------
        float | None
            Fixed payoff FX conversion rate from base-currency payoff into quote currency. Example: for a JPY-underlying option settled in USD at a fixed 140 JPY/USD conversion, payoff_fx_rate is 1.0 / 140.0 USD per JPY.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def base_currency(self) -> str:
        """Read the contract's base currency.

        Returns
        -------
        str
            Base currency (equity denomination)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quote_currency(self) -> str:
        """Read the contract's quote currency.

        Returns
        -------
        str
            Quote currency (payment/settlement currency)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def correlation(self) -> object:
        """Read the contract's correlation.

        Returns
        -------
        object
            Correlation between equity price and FX rate

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """Read the contract's day count.

        Returns
        -------
        DayCount
            Day count convention (defaults to ACT/365F).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def domestic_discount_curve_id(self) -> str:
        """Read the contract's domestic discount curve id.

        Returns
        -------
        str
            Discount curve ID (domestic currency)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def foreign_discount_curve_id(self) -> str:
        """Read the contract's foreign discount curve id.

        Returns
        -------
        str
            Discount curve ID (foreign currency)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def spot_id(self) -> str:
        """Read the contract's spot id.

        Returns
        -------
        str
            Equity spot price identifier

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def vol_surface_id(self) -> str:
        """Read the contract's vol surface id.

        Returns
        -------
        str
            Equity volatility surface ID

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def div_yield_id(self) -> str | None:
        """Read the contract's div yield id.

        Returns
        -------
        str | None
            Optional dividend-yield scalar ID

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def fx_spot_id(self) -> str | None:
        """Read the contract's fx spot id.

        Returns
        -------
        str | None
            Optional FX rate identifier. The referenced scalar must be quoted as **quote currency per unit of base currency** (e.g. for base JPY / quote USD, USD-per-JPY ≈ 0.0071, conventionally named JPYUSD). The pricer compounds it with exp((r_quote − r_base)·t) to obtain the ATM forward for the FX vol lookup, so an inverted quote (JPY-per-USD) silently mislocates the smile. validate() rejects ids whose embedded pair name contradicts this direction.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def fx_vol_surface_id(self) -> str | None:
        """Read the contract's fx vol surface id.

        Returns
        -------
        str | None
            Optional FX volatility surface ID. Must be calibrated in the same **quote-per-base** direction as fx_spot_id so absolute-strike lookups at the CIRP forward land on the correct smile. validate() rejects ids whose embedded pair name contradicts this direction.

        Notes
        -----
        This property does not raise.
        """
        ...

class QuantoOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import QuantoOption
    >>> builder = QuantoOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> QuantoOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        QuantoOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> QuantoOptionBuilder:
        """Set the quanto option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        QuantoOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying_ticker(self, value: str) -> QuantoOptionBuilder:
        """Set the contract's underlying ticker.

        Parameters
        ----------
        value : str
            Underlying equity ticker symbol

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: float) -> QuantoOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : float
            Per-unit strike of the equity underlying, in base_currency price units.

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: object) -> QuantoOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : object
            Option type (call or put)

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> QuantoOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Option expiry date

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> QuantoOptionBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Strike-equivalent domestic reference notional. When quantity and payoff_fx_rate are supplied, this must equal quantity * payoff_fx_rate * strike.

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quantity(self, value: float | None) -> QuantoOptionBuilder:
        """Set the contract's quantity.

        Parameters
        ----------
        value : float | None
            Number of underlying units covered by the option payoff. Quanto pricing is performed per unit of the underlying in the base currency. This quantity converts the per-unit price into contract-level exposure.

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payoff_fx_rate(self, value: float | None) -> QuantoOptionBuilder:
        """Set the contract's payoff fx rate.

        Parameters
        ----------
        value : float | None
            Fixed payoff FX conversion rate from base-currency payoff into quote currency. Example: for a JPY-underlying option settled in USD at a fixed 140 JPY/USD conversion, payoff_fx_rate is 1.0 / 140.0 USD per JPY.

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def base_currency(self, value: Currency | str) -> QuantoOptionBuilder:
        """Set the contract's base currency.

        Parameters
        ----------
        value : Currency | str
            Base currency (equity denomination)

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_currency(self, value: Currency | str) -> QuantoOptionBuilder:
        """Set the contract's quote currency.

        Parameters
        ----------
        value : Currency | str
            Quote currency (payment/settlement currency)

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def correlation(self, value: object) -> QuantoOptionBuilder:
        """Set the contract's correlation.

        Parameters
        ----------
        value : object
            Correlation between equity price and FX rate

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> QuantoOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention (defaults to ACT/365F).

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def domestic_discount_curve_id(self, value: str) -> QuantoOptionBuilder:
        """Set the contract's domestic discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID (domestic currency)

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def foreign_discount_curve_id(self, value: str) -> QuantoOptionBuilder:
        """Set the contract's foreign discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID (foreign currency)

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str) -> QuantoOptionBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str
            Equity spot price identifier

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> QuantoOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Equity volatility surface ID

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def div_yield_id(self, value: str | None) -> QuantoOptionBuilder:
        """Set the contract's div yield id.

        Parameters
        ----------
        value : str | None
            Optional dividend-yield scalar ID

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fx_spot_id(self, value: str | None) -> QuantoOptionBuilder:
        """Set the contract's fx spot id.

        Parameters
        ----------
        value : str | None
            Optional FX rate identifier. The referenced scalar must be quoted as **quote currency per unit of base currency** (e.g. for base JPY / quote USD, USD-per-JPY ≈ 0.0071, conventionally named JPYUSD). The pricer compounds it with exp((r_quote − r_base)·t) to obtain the ATM forward for the FX vol lookup, so an inverted quote (JPY-per-USD) silently mislocates the smile. validate() rejects ids whose embedded pair name contradicts this direction.

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fx_vol_surface_id(self, value: str | None) -> QuantoOptionBuilder:
        """Set the contract's fx vol surface id.

        Parameters
        ----------
        value : str | None
            Optional FX volatility surface ID. Must be calibrated in the same **quote-per-base** direction as fx_spot_id so absolute-strike lookups at the CIRP forward land on the correct smile. validate() rejects ids whose embedded pair name contradicts this direction.

        Returns
        -------
        QuantoOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> QuantoOptionBuilder:
        """Set the quanto option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        QuantoOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> QuantoOptionBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``InstrumentPricingOverrides`` in serde form (``dict`` or JSON
            string): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        QuantoOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> QuantoOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        QuantoOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> QuantoOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        QuantoOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
