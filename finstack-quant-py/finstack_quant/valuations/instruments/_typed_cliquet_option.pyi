"""Typed cliquet option and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class CliquetOption:
    """Option that accumulates local returns over specified reset periods.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CliquetOption
    >>> isinstance(CliquetOption.example(), CliquetOption)
    True
    """

    @staticmethod
    def builder() -> CliquetOptionBuilder:
        """Start authoring a cliquet option.

        Returns
        -------
        CliquetOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CliquetOption
        >>> _ = CliquetOption.builder().id("example-cliquet-option")
        """
        ...

    def to_builder(self) -> CliquetOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        CliquetOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> CliquetOption:
        """Construct the built-in cliquet option example.

        Returns
        -------
        CliquetOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CliquetOption
        >>> CliquetOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CliquetOption:
        """
        Deserialize a validated CliquetOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cliquet_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CliquetOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CliquetOption
        >>> future = CliquetOption.example()
        >>> CliquetOption.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`CliquetOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this cliquet option as a plain Python ``dict``.

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
        Price this cliquet option and return a :class:`~finstack_quant.valuations.ValuationResult`.

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

    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute a registered scalar metric for this cliquet option.

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
            Underlying asset ticker symbol

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def reset_dates(self) -> list[date]:
        """Read the contract's reset dates.

        Returns
        -------
        list[date]
            Reset dates for periodic return locking

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
            Explicit terminal expiry date for the structure.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def local_cap(self) -> float:
        """Read the contract's local cap.

        Returns
        -------
        float
            Local cap on individual period returns

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def local_floor(self) -> float:
        """Read the contract's local floor.

        Returns
        -------
        float
            Local floor on individual period returns (default 0.0)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def global_cap(self) -> float:
        """Read the contract's global cap.

        Returns
        -------
        float
            Global cap on sum of all period returns

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def global_floor(self) -> float:
        """Read the contract's global floor.

        Returns
        -------
        float
            Global floor on sum of all period returns (default 0.0)

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
            Notional amount

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
            Day count convention

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """Read the contract's discount curve id.

        Returns
        -------
        str
            Discount curve ID for present value calculations

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
            Spot price identifier

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
            Volatility surface ID

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def path_model(self) -> dict[str, object] | str:
        """Read the contract's path model.

        Returns
        -------
        dict[str, object] | str
            Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.

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
            Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def initial_level(self) -> float | None:
        """Read the contract's initial level.

        Returns
        -------
        float | None
            Strike-set underlying level anchoring the first period's return. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any reset date strictly before as_of): pricing errors otherwise. Do not duplicate the strike-set date inside reset_dates when providing this — reset dates are period-end observations.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def past_fixings(self) -> list[tuple[date, float]]:
        """Read the contract's past fixings.

        Returns
        -------
        list[tuple[date, float]]
            Observed underlying fixings for seasoned trades (date, level pairs). Every reset date strictly before the valuation date must have a matching fixing here; pricing errors otherwise. Locked-in period returns are computed deterministically from these fixings (with local cap/floor applied) and only the remaining future periods are simulated.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def payoff_type(self) -> dict[str, object] | str:
        """Read the contract's payoff type.

        Returns
        -------
        dict[str, object] | str
            Payoff aggregation type (default: Additive)

        Notes
        -----
        This property does not raise.
        """
        ...

class CliquetOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CliquetOption
    >>> builder = CliquetOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> CliquetOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        CliquetOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> CliquetOptionBuilder:
        """Set the cliquet option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        CliquetOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying_ticker(self, value: str) -> CliquetOptionBuilder:
        """Set the contract's underlying ticker.

        Parameters
        ----------
        value : str
            Underlying asset ticker symbol

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def reset_dates(self, value: list[date | str]) -> CliquetOptionBuilder:
        """Set the contract's reset dates.

        Parameters
        ----------
        value : list[date | str]
            Reset dates for periodic return locking

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> CliquetOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Explicit terminal expiry date for the structure.

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def local_cap(self, value: float) -> CliquetOptionBuilder:
        """Set the contract's local cap.

        Parameters
        ----------
        value : float
            Local cap on individual period returns

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def local_floor(self, value: float) -> CliquetOptionBuilder:
        """Set the contract's local floor.

        Parameters
        ----------
        value : float
            Local floor on individual period returns (default 0.0)

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def global_cap(self, value: float) -> CliquetOptionBuilder:
        """Set the contract's global cap.

        Parameters
        ----------
        value : float
            Global cap on sum of all period returns

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def global_floor(self, value: float) -> CliquetOptionBuilder:
        """Set the contract's global floor.

        Parameters
        ----------
        value : float
            Global floor on sum of all period returns (default 0.0)

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> CliquetOptionBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> CliquetOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> CliquetOptionBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID for present value calculations

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str) -> CliquetOptionBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str
            Spot price identifier

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> CliquetOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface ID

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def path_model(self, value: dict[str, object] | str) -> CliquetOptionBuilder:
        """Set the contract's path model.

        Parameters
        ----------
        value : dict[str, object] | str
            Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def div_yield_id(self, value: str | None) -> CliquetOptionBuilder:
        """Set the contract's div yield id.

        Parameters
        ----------
        value : str | None
            Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def initial_level(self, value: float | None) -> CliquetOptionBuilder:
        """Set the contract's initial level.

        Parameters
        ----------
        value : float | None
            Strike-set underlying level anchoring the first period's return. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any reset date strictly before as_of): pricing errors otherwise. Do not duplicate the strike-set date inside reset_dates when providing this — reset dates are period-end observations.

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def past_fixings(self, value: list[tuple[date | str, float]]) -> CliquetOptionBuilder:
        """Set the contract's past fixings.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Observed underlying fixings for seasoned trades (date, level pairs). Every reset date strictly before the valuation date must have a matching fixing here; pricing errors otherwise. Locked-in period returns are computed deterministically from these fixings (with local cap/floor applied) and only the remaining future periods are simulated.

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payoff_type(self, value: dict[str, object] | str) -> CliquetOptionBuilder:
        """Set the contract's payoff type.

        Parameters
        ----------
        value : dict[str, object] | str
            Payoff aggregation type (default: Additive)

        Returns
        -------
        CliquetOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> CliquetOptionBuilder:
        """Set the cliquet option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        CliquetOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CliquetOptionBuilder:
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
        CliquetOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> CliquetOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CliquetOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CliquetOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CliquetOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
