"""Typed commodity swap and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.dates import Tenor
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class CommoditySwap:
    """Fixed-for-floating commodity price swap.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CommoditySwap
    >>> isinstance(CommoditySwap.example(), CommoditySwap)
    True
    """

    @staticmethod
    def builder() -> CommoditySwapBuilder:
        """Start authoring a commodity swap.

        Returns
        -------
        CommoditySwapBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommoditySwap
        >>> _ = CommoditySwap.builder().id("example-commodity-swap")
        """
        ...

    def to_builder(self) -> CommoditySwapBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        CommoditySwapBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> CommoditySwap:
        """Construct the built-in commodity swap example.

        Returns
        -------
        CommoditySwap
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommoditySwap
        >>> CommoditySwap.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CommoditySwap:
        """
        Deserialize a validated CommoditySwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"commodity_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CommoditySwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommoditySwap
        >>> future = CommoditySwap.example()
        >>> CommoditySwap.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`CommoditySwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this commodity swap as a plain Python ``dict``.

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
        Canonical model key used when ``model="default"`` is passed to ``price_instrument``.

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
    def underlying(self) -> dict[str, object] | str:
        """Read the contract's underlying.

        Returns
        -------
        dict[str, object] | str
            Underlying commodity parameters (type, ticker, unit, currency).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quantity(self) -> float:
        """Read the contract's quantity.

        Returns
        -------
        float
            Notional quantity per period.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def fixed_price(self) -> float:
        """Read the contract's fixed price.

        Returns
        -------
        float
            Fixed price per commodity unit, in the notional currency (finite).

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
            Commodity forward PriceCurve that projects the floating-leg price observations.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def side(self) -> dict[str, object] | str:
        """Read the contract's side.

        Returns
        -------
        dict[str, object] | str
            Direction of the swap: Pay means paying the fixed price leg, Receive means receiving the fixed price leg.

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
            Start date of the swap.

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
            End date of the swap.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def frequency(self) -> dict[str, object] | Tenor:
        """Read the contract's frequency.

        Returns
        -------
        dict[str, object] | Tenor
            Payment frequency as a Tenor.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def calendar_id(self) -> str | None:
        """Read the contract's calendar id.

        Returns
        -------
        str | None
            Optional calendar ID for date adjustments.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def business_day_convention(self) -> str:
        """Read the contract's business day convention.

        Returns
        -------
        str
            Business day convention for payment-schedule date adjustments. Only applied when calendar_id is set and resolves to a registered holiday calendar; without a calendar the schedule dates are left unadjusted.

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
    def index_lag_days(self) -> int | None:
        """Read the contract's index lag days.

        Returns
        -------
        int | None
            Optional index lag in **calendar days**: the floating-leg averaging window is shifted back by exactly this many calendar days (no business-day adjustment of the shifted window endpoints).

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
            Realized floating-index fixings as (date, price) pairs. Floating-leg observations with date strictly before the valuation date read from this store; a missing past fixing is an error — no silent substitution of today's spot . Observations on or after the valuation date project from the price curve.

        Notes
        -----
        This property does not raise.
        """
        ...

class CommoditySwapBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CommoditySwap
    >>> builder = CommoditySwap.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> CommoditySwap:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        CommoditySwap
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> CommoditySwapBuilder:
        """Set the commodity swap identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        CommoditySwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying(self, value: dict[str, object] | str) -> CommoditySwapBuilder:
        """Set the contract's underlying.

        Parameters
        ----------
        value : dict[str, object] | str
            Underlying commodity parameters (type, ticker, unit, currency).

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quantity(self, value: float) -> CommoditySwapBuilder:
        """Set the contract's quantity.

        Parameters
        ----------
        value : float
            Notional quantity per period.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fixed_price(self, value: float) -> CommoditySwapBuilder:
        """Set the contract's fixed price.

        Parameters
        ----------
        value : float
            Fixed price per commodity unit, in the notional currency (finite).

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def forward_curve_id(self, value: str) -> CommoditySwapBuilder:
        """Set the contract's forward curve id.

        Parameters
        ----------
        value : str
            Commodity forward PriceCurve that projects the floating-leg price observations.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def side(self, value: dict[str, object] | str) -> CommoditySwapBuilder:
        """Set the contract's side.

        Parameters
        ----------
        value : dict[str, object] | str
            Direction of the swap: Pay means paying the fixed price leg, Receive means receiving the fixed price leg.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def start_date(self, value: date | str) -> CommoditySwapBuilder:
        """Set the contract's start date.

        Parameters
        ----------
        value : date | str
            Start date of the swap.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def maturity(self, value: date | str) -> CommoditySwapBuilder:
        """Set the contract's maturity.

        Parameters
        ----------
        value : date | str
            End date of the swap.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def frequency(self, value: dict[str, object] | Tenor) -> CommoditySwapBuilder:
        """Set the contract's frequency.

        Parameters
        ----------
        value : dict[str, object] | Tenor
            Payment frequency as a Tenor.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def calendar_id(self, value: str | None) -> CommoditySwapBuilder:
        """Set the contract's calendar id.

        Parameters
        ----------
        value : str | None
            Optional calendar ID for date adjustments.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def business_day_convention(self, value: str) -> CommoditySwapBuilder:
        """Set the contract's business day convention.

        Parameters
        ----------
        value : str
            Business day convention for payment-schedule date adjustments. Only applied when calendar_id is set and resolves to a registered holiday calendar; without a calendar the schedule dates are left unadjusted.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> CommoditySwapBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def index_lag_days(self, value: int | None) -> CommoditySwapBuilder:
        """Set the contract's index lag days.

        Parameters
        ----------
        value : int | None
            Optional index lag in **calendar days**: the floating-leg averaging window is shifted back by exactly this many calendar days (no business-day adjustment of the shifted window endpoints).

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def past_fixings(self, value: list[tuple[date | str, float]]) -> CommoditySwapBuilder:
        """Set the contract's past fixings.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Realized floating-index fixings as (date, price) pairs. Floating-leg observations with date strictly before the valuation date read from this store; a missing past fixing is an error — no silent substitution of today's spot . Observations on or after the valuation date project from the price curve.

        Returns
        -------
        CommoditySwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> CommoditySwapBuilder:
        """Set the commodity swap grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        CommoditySwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CommoditySwapBuilder:
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
        CommoditySwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> CommoditySwapBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CommoditySwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CommoditySwapBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CommoditySwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
