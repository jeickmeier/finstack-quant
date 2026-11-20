"""Typed non-deliverable forward and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class Ndf:
    """Cash-settled currency forward with separate fixing and settlement terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Ndf
    >>> isinstance(Ndf.example(), Ndf)
    True
    """

    @staticmethod
    def builder() -> NdfBuilder:
        """Start authoring a non-deliverable forward.

        Returns
        -------
        NdfBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Ndf
        >>> _ = Ndf.builder().id("example-ndf")
        """
        ...

    def to_builder(self) -> NdfBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        NdfBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> Ndf:
        """Construct the built-in non-deliverable forward example.

        Returns
        -------
        Ndf
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Ndf
        >>> Ndf.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Ndf:
        """
        Deserialize a validated Ndf from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"ndf"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Ndf
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Ndf
        >>> contract = Ndf.example()
        >>> Ndf.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`Ndf.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this non-deliverable forward as a plain Python ``dict``.

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
        Price this non-deliverable forward and return a :class:`~finstack_quant.valuations.ValuationResult`.

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
        Compute a registered scalar metric for this non-deliverable forward.

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

    @property
    def expiry(self) -> datetime.date | None:
        """Return the contractual expiry reported by the native instrument.

        Returns
        -------
        datetime.date or None
            Contract expiry where the instrument defines an expiry date.

        Notes
        -----
        This accessor does not raise.
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
    def base_currency(self) -> str:
        """Read the contract's base currency.

        Returns
        -------
        str
            Base currency (restricted/non-deliverable currency, numerator).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement_currency(self) -> str:
        """Read the contract's settlement currency.

        Returns
        -------
        str
            Settlement currency (freely convertible, typically USD, denominator and PV currency).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def fixing_date(self) -> date:
        """Read the contract's fixing date.

        Returns
        -------
        date
            Fixing date (rate observation date, typically T-2 before maturity).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def maturity(self) -> date:
        """Read the contract's maturity.

        Returns
        -------
        date
            Maturity/settlement date.

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
            Notional amount in base currency.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def contract_rate(self) -> float | None:
        """Read the contract's contract rate.

        Returns
        -------
        float | None
            Contract forward rate. Interpretation depends on quote_convention: - BasePerSettlement: base per settlement (e.g., 7.25 CNY per USD) - SettlementPerBase: settlement per base (e.g., 0.138 USD per CNY) None values the NDF at-market: the contract rate equals the market forward, so the settlement amount and PV are zero.

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
            Settlement currency discount curve ID.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quote_convention(self) -> object:
        """Read the contract's quote convention.

        Returns
        -------
        object
            Quote convention for contract_rate and observed_fixing.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def foreign_discount_curve_id(self) -> str | None:
        """Read the contract's foreign discount curve id.

        Returns
        -------
        str | None
            Optional foreign (base) currency discount curve ID. Required for pre-fixing forward estimation unless quoted_forward is supplied.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observed_fixing(self) -> float | None:
        """Read the contract's observed fixing.

        Returns
        -------
        float | None
            Observed fixing rate. Interpretation depends on quote_convention. If Some, NDF is post-fixing.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def fixing_source(self) -> object | None:
        """Read the contract's fixing source.

        Returns
        -------
        object | None
            Official fixing source/benchmark enum for type-safe specification. Use this field for validated fixing sources. See NdfFixingSource for supported benchmarks and their typical currencies.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quoted_spot(self) -> float | None:
        """Read the contract's quoted spot.

        Returns
        -------
        float | None
            Optional spot rate override for forward rate calculation. Interpretation depends on quote_convention.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quoted_forward(self) -> float | None:
        """Read the contract's quoted forward.

        Returns
        -------
        float | None
            Explicit pre-fixing forward rate in quote_convention units. Use this for NDF market quotes or basis-adjusted forwards when a base currency discount curve is unavailable.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def base_calendar_id(self) -> str | None:
        """Read the contract's base calendar id.

        Returns
        -------
        str | None
            Optional base currency calendar.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement_calendar_id(self) -> str | None:
        """Read the contract's settlement calendar id.

        Returns
        -------
        str | None
            Optional settlement currency calendar.

        Notes
        -----
        This property does not raise.
        """
        ...

class NdfBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Ndf
    >>> builder = Ndf.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> Ndf:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        Ndf
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> NdfBuilder:
        """Set the non-deliverable forward identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        NdfBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def base_currency(self, value: Currency | str) -> NdfBuilder:
        """Set the contract's base currency.

        Parameters
        ----------
        value : Currency | str
            Base currency (restricted/non-deliverable currency, numerator).

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_currency(self, value: Currency | str) -> NdfBuilder:
        """Set the contract's settlement currency.

        Parameters
        ----------
        value : Currency | str
            Settlement currency (freely convertible, typically USD, denominator and PV currency).

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fixing_date(self, value: date | str) -> NdfBuilder:
        """Set the contract's fixing date.

        Parameters
        ----------
        value : date | str
            Fixing date (rate observation date, typically T-2 before maturity).

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def maturity(self, value: date | str) -> NdfBuilder:
        """Set the contract's maturity.

        Parameters
        ----------
        value : date | str
            Maturity/settlement date.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> NdfBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount in base currency.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def contract_rate(self, value: float | None) -> NdfBuilder:
        """Set the contract's contract rate.

        Parameters
        ----------
        value : float | None
            Contract forward rate. Interpretation depends on quote_convention: - BasePerSettlement: base per settlement (e.g., 7.25 CNY per USD) - SettlementPerBase: settlement per base (e.g., 0.138 USD per CNY) None values the NDF at-market: the contract rate equals the market forward, so the settlement amount and PV are zero.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def domestic_discount_curve_id(self, value: str) -> NdfBuilder:
        """Set the contract's domestic discount curve id.

        Parameters
        ----------
        value : str
            Settlement currency discount curve ID.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_convention(self, value: object) -> NdfBuilder:
        """Set the contract's quote convention.

        Parameters
        ----------
        value : object
            Quote convention for contract_rate and observed_fixing.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def foreign_discount_curve_id(self, value: str | None) -> NdfBuilder:
        """Set the contract's foreign discount curve id.

        Parameters
        ----------
        value : str | None
            Optional foreign (base) currency discount curve ID. Required for pre-fixing forward estimation unless quoted_forward is supplied.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observed_fixing(self, value: float | None) -> NdfBuilder:
        """Set the contract's observed fixing.

        Parameters
        ----------
        value : float | None
            Observed fixing rate. Interpretation depends on quote_convention. If Some, NDF is post-fixing.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fixing_source(self, value: object | None) -> NdfBuilder:
        """Set the contract's fixing source.

        Parameters
        ----------
        value : object | None
            Official fixing source/benchmark enum for type-safe specification. Use this field for validated fixing sources. See NdfFixingSource for supported benchmarks and their typical currencies.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quoted_spot(self, value: float | None) -> NdfBuilder:
        """Set the contract's quoted spot.

        Parameters
        ----------
        value : float | None
            Optional spot rate override for forward rate calculation. Interpretation depends on quote_convention.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quoted_forward(self, value: float | None) -> NdfBuilder:
        """Set the contract's quoted forward.

        Parameters
        ----------
        value : float | None
            Explicit pre-fixing forward rate in quote_convention units. Use this for NDF market quotes or basis-adjusted forwards when a base currency discount curve is unavailable.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def base_calendar_id(self, value: str | None) -> NdfBuilder:
        """Set the contract's base calendar id.

        Parameters
        ----------
        value : str | None
            Optional base currency calendar.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_calendar_id(self, value: str | None) -> NdfBuilder:
        """Set the contract's settlement calendar id.

        Parameters
        ----------
        value : str | None
            Optional settlement currency calendar.

        Returns
        -------
        NdfBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> NdfBuilder:
        """Set the non-deliverable forward grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        NdfBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> NdfBuilder:
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
        NdfBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> NdfBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        NdfBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> NdfBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        NdfBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
