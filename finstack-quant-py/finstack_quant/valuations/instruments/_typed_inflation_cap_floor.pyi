"""Typed inflation cap or floor and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from decimal import Decimal
from finstack_quant.core.money import Money
from finstack_quant.core.dates import DayCount
from finstack_quant.core.dates import Tenor
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class InflationCapFloor:
    """Cap or floor on year-on-year inflation observations.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import InflationCapFloor
    >>> isinstance(InflationCapFloor.example(), InflationCapFloor)
    True
    """

    @staticmethod
    def builder() -> InflationCapFloorBuilder:
        """Start authoring an inflation cap or floor.

        Returns
        -------
        InflationCapFloorBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InflationCapFloor
        >>> _ = InflationCapFloor.builder().id("example-inflation-cap-floor")
        """
        ...

    def to_builder(self) -> InflationCapFloorBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        InflationCapFloorBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> InflationCapFloor:
        """Construct the built-in inflation cap or floor example.

        Returns
        -------
        InflationCapFloor
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InflationCapFloor
        >>> InflationCapFloor.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> InflationCapFloor:
        """
        Deserialize a validated InflationCapFloor from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"inflation_cap_floor"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        InflationCapFloor
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InflationCapFloor
        >>> future = InflationCapFloor.example()
        >>> InflationCapFloor.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`InflationCapFloor.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this inflation cap or floor as a plain Python ``dict``.

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
    def rate_option_type(self) -> dict[str, object] | str:
        """Read the contract's rate option type.

        Returns
        -------
        dict[str, object] | str
            Cap/floor type (cap, floor, caplet, floorlet). Caplet and floorlet price a single period.

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
            Notional amount in quote currency.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def strike(self) -> Decimal:
        """Read the contract's strike.

        Returns
        -------
        Decimal
            Strike (annualized, decimal).

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
            Start date of the first inflation period.

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
            End date of the final inflation period.

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
            Payment frequency (ignored for caplet/floorlet).

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
            Day count convention for accrual. Option quote time always uses ACT/365F.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def stub(self) -> dict[str, object] | str:
        """Read the contract's stub.

        Returns
        -------
        dict[str, object] | str
            Schedule stub convention.

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
            Business day convention for schedule and payments.

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
            Optional holiday calendar identifier.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def inflation_index_id(self) -> str:
        """Read the contract's inflation index id.

        Returns
        -------
        str
            Inflation index/curve identifier (e.g., US-CPI-U).

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
            Discount curve identifier.

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
            Volatility surface identifier.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def lag(self) -> dict[str, object] | str | None:
        """Read the contract's lag.

        Returns
        -------
        dict[str, object] | str | None
            Explicit contractual CPI observation lag; missing terms fail reference resolution.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def interpolation(self) -> dict[str, object] | str | None:
        """Read the contract's interpolation.

        Returns
        -------
        dict[str, object] | str | None
            Explicit contractual monthly CPI interpolation; no source default is used. Defaults to monthly step interpolation when neither source supplies it.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def volatility_expiry(self) -> dict[str, object] | str:
        """Read the contract's volatility expiry.

        Returns
        -------
        dict[str, object] | str
            Clock used by the volatility surface's annualized quotes. This is independent of CPI publication policy; changing it requires matching quotes, not merely relabelling the old surface.

        Notes
        -----
        This property does not raise.
        """
        ...

class InflationCapFloorBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import InflationCapFloor
    >>> builder = InflationCapFloor.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> InflationCapFloor:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        InflationCapFloor
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> InflationCapFloorBuilder:
        """Set the inflation cap or floor identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        InflationCapFloorBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def rate_option_type(self, value: dict[str, object] | str) -> InflationCapFloorBuilder:
        """Set the contract's rate option type.

        Parameters
        ----------
        value : dict[str, object] | str
            Cap/floor type (cap, floor, caplet, floorlet). Caplet and floorlet price a single period.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> InflationCapFloorBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount in quote currency.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: Decimal | str | int) -> InflationCapFloorBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : Decimal | str | int
            Strike (annualized, decimal).

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def start_date(self, value: date | str) -> InflationCapFloorBuilder:
        """Set the contract's start date.

        Parameters
        ----------
        value : date | str
            Start date of the first inflation period.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def maturity(self, value: date | str) -> InflationCapFloorBuilder:
        """Set the contract's maturity.

        Parameters
        ----------
        value : date | str
            End date of the final inflation period.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def frequency(self, value: dict[str, object] | Tenor) -> InflationCapFloorBuilder:
        """Set the contract's frequency.

        Parameters
        ----------
        value : dict[str, object] | Tenor
            Payment frequency (ignored for caplet/floorlet).

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> InflationCapFloorBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention for accrual. Option quote time always uses ACT/365F.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def stub(self, value: dict[str, object] | str) -> InflationCapFloorBuilder:
        """Set the contract's stub.

        Parameters
        ----------
        value : dict[str, object] | str
            Schedule stub convention.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def business_day_convention(self, value: str) -> InflationCapFloorBuilder:
        """Set the contract's business day convention.

        Parameters
        ----------
        value : str
            Business day convention for schedule and payments.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def calendar_id(self, value: str | None) -> InflationCapFloorBuilder:
        """Set the contract's calendar id.

        Parameters
        ----------
        value : str | None
            Optional holiday calendar identifier.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def inflation_index_id(self, value: str) -> InflationCapFloorBuilder:
        """Set the contract's inflation index id.

        Parameters
        ----------
        value : str
            Inflation index/curve identifier (e.g., US-CPI-U).

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> InflationCapFloorBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> InflationCapFloorBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface identifier.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def lag(self, value: dict[str, object] | str | None) -> InflationCapFloorBuilder:
        """Set the contract's lag.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Explicit contractual CPI observation lag; missing terms fail reference resolution.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def interpolation(self, value: dict[str, object] | str | None) -> InflationCapFloorBuilder:
        """Set the contract's interpolation.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Explicit contractual monthly CPI interpolation; no source default is used. Defaults to monthly step interpolation when neither source supplies it.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def volatility_expiry(self, value: dict[str, object] | str) -> InflationCapFloorBuilder:
        """Set the contract's volatility expiry.

        Parameters
        ----------
        value : dict[str, object] | str
            Clock used by the volatility surface's annualized quotes. This is independent of CPI publication policy; changing it requires matching quotes, not merely relabelling the old surface.

        Returns
        -------
        InflationCapFloorBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> InflationCapFloorBuilder:
        """Set the inflation cap or floor grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        InflationCapFloorBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> InflationCapFloorBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        InflationCapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object]) -> InflationCapFloorBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        InflationCapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> InflationCapFloorBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        InflationCapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
