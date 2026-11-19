"""Typed snowball note and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from decimal import Decimal
from finstack_quant.core.money import Money
from finstack_quant.core.dates import DayCount
from finstack_quant.core.dates import Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class Snowball:
    """Structured note with contractual observation and redemption rules.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Snowball
    >>> isinstance(Snowball.example(), Snowball)
    True
    """

    @staticmethod
    def builder() -> SnowballBuilder:
        """Start authoring a snowball note.

        Returns
        -------
        SnowballBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Snowball
        >>> _ = Snowball.builder().id("example-snowball")
        """
        ...

    def to_builder(self) -> SnowballBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        SnowballBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> Snowball:
        """Construct the built-in snowball note example.

        Returns
        -------
        Snowball
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Snowball
        >>> Snowball.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Snowball:
        """
        Deserialize a validated Snowball from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"snowball"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Snowball
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Snowball
        >>> future = Snowball.example()
        >>> Snowball.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`Snowball.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this snowball note as a plain Python ``dict``.

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
        Price this snowball note and return a :class:`~finstack_quant.valuations.ValuationResult`.

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
        Compute a registered scalar metric for this snowball note.

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
    def variant(self) -> dict[str, object] | str:
        """Read the contract's variant.

        Returns
        -------
        dict[str, object] | str
            Snowball or inverse floater variant.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def initial_coupon(self) -> float:
        """Read the contract's initial coupon.

        Returns
        -------
        float
            Initial coupon for snowball (c_0); ignored for inverse floater.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def fixed_rate(self) -> Decimal:
        """Read the contract's fixed rate.

        Returns
        -------
        Decimal
            Fixed rate component as a decimal annual rate (0.05 = 5%).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def gearing(self) -> float:
        """Read the contract's gearing.

        Returns
        -------
        float
            Multiplier on the floating fixing (1.0 for snowball, variable for inverse floater).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def coupon_floor(self) -> float:
        """Read the contract's coupon floor.

        Returns
        -------
        float
            Floor on each period coupon (typically 0.0).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def coupon_cap(self) -> float | None:
        """Read the contract's coupon cap.

        Returns
        -------
        float | None
            Optional cap on each period coupon.

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
            Notional amount.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def start_date(self) -> date:
        """Read the contract's start date.

        Returns
        -------
        date
            Accrual start of the first coupon period; also the first in-advance fixing date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def payment_dates(self) -> list[date]:
        """Read the contract's payment dates.

        Returns
        -------
        list[date]
            Coupon payment dates, one per period, strictly ascending and after start_date. Period i accrues from the previous payment date (or start_date) to payment_dates[i] and fixes in advance at its start.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def forward_curve_id(self) -> str:
        """Read the contract's forward curve id.

        Returns
        -------
        str
            Rates forward curve that projects the floating index (also the fixing-series key).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def index_tenor(self) -> dict[str, object] | Tenor:
        """Read the contract's index tenor.

        Returns
        -------
        dict[str, object] | Tenor
            Contractual tenor of the observed floating index (must match the forward curve tenor).

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
            Discount curve ID.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def vol_surface_id(self) -> str | None:
        """Read the contract's vol surface id.

        Returns
        -------
        str | None
            Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def call_provision(self) -> dict[str, object] | str | None:
        """Read the contract's call provision.

        Returns
        -------
        dict[str, object] | str | None
            Optional Bermudan call provision.

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
            Day count convention.

        Notes
        -----
        This property does not raise.
        """
        ...

class SnowballBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Snowball
    >>> builder = Snowball.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> Snowball:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        Snowball
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> SnowballBuilder:
        """Set the snowball note identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        SnowballBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def variant(self, value: dict[str, object] | str) -> SnowballBuilder:
        """Set the contract's variant.

        Parameters
        ----------
        value : dict[str, object] | str
            Snowball or inverse floater variant.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def initial_coupon(self, value: float) -> SnowballBuilder:
        """Set the contract's initial coupon.

        Parameters
        ----------
        value : float
            Initial coupon for snowball (c_0); ignored for inverse floater.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fixed_rate(self, value: Decimal | str | int) -> SnowballBuilder:
        """Set the contract's fixed rate.

        Parameters
        ----------
        value : Decimal | str | int
            Fixed rate component as a decimal annual rate (0.05 = 5%).

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def gearing(self, value: float) -> SnowballBuilder:
        """Set the contract's gearing.

        Parameters
        ----------
        value : float
            Multiplier on the floating fixing (1.0 for snowball, variable for inverse floater).

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def coupon_floor(self, value: float) -> SnowballBuilder:
        """Set the contract's coupon floor.

        Parameters
        ----------
        value : float
            Floor on each period coupon (typically 0.0).

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def coupon_cap(self, value: float | None) -> SnowballBuilder:
        """Set the contract's coupon cap.

        Parameters
        ----------
        value : float | None
            Optional cap on each period coupon.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> SnowballBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def start_date(self, value: date | str) -> SnowballBuilder:
        """Set the contract's start date.

        Parameters
        ----------
        value : date | str
            Accrual start of the first coupon period; also the first in-advance fixing date.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payment_dates(self, value: list[date | str]) -> SnowballBuilder:
        """Set the contract's payment dates.

        Parameters
        ----------
        value : list[date | str]
            Coupon payment dates, one per period, strictly ascending and after start_date. Period i accrues from the previous payment date (or start_date) to payment_dates[i] and fixes in advance at its start.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def forward_curve_id(self, value: str) -> SnowballBuilder:
        """Set the contract's forward curve id.

        Parameters
        ----------
        value : str
            Rates forward curve that projects the floating index (also the fixing-series key).

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def index_tenor(self, value: dict[str, object] | Tenor) -> SnowballBuilder:
        """Set the contract's index tenor.

        Parameters
        ----------
        value : dict[str, object] | Tenor
            Contractual tenor of the observed floating index (must match the forward curve tenor).

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> SnowballBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str | None) -> SnowballBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str | None
            Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def call_provision(self, value: dict[str, object] | str | None) -> SnowballBuilder:
        """Set the contract's call provision.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Optional Bermudan call provision.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> SnowballBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention.

        Returns
        -------
        SnowballBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> SnowballBuilder:
        """Set the snowball note grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        SnowballBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> SnowballBuilder:
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
        SnowballBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> SnowballBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        SnowballBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> SnowballBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        SnowballBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
