"""Typed inflation-linked bond declarations."""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Any
import pandas as pd
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class InflationLinkedBond:
    """Bond whose nominal cashflows reflect the contractual CPI indexation rules.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import InflationLinkedBond
    >>> isinstance(InflationLinkedBond.example(), InflationLinkedBond)
    True
    """

    @staticmethod
    def builder() -> InflationLinkedBondBuilder:
        """Start specifying an inflation-linked bond.

        Returns
        -------
        InflationLinkedBondBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InflationLinkedBond
        >>> _ = InflationLinkedBond.builder().id("TIPS")
        """
        ...

    def to_builder(self) -> InflationLinkedBondBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        InflationLinkedBondBuilder
            Builder retaining the exact coupon, conventions, and overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> InflationLinkedBond:
        """Construct the canonical inflation-linked bond fixture.

        Returns
        -------
        InflationLinkedBond
            Validated US Treasury inflation-protected bond example.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InflationLinkedBond
        >>> InflationLinkedBond.example().real_coupon > 0
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> InflationLinkedBond:
        """
        Deserialize a validated InflationLinkedBond from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"inflation_linked_bond"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        InflationLinkedBond
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InflationLinkedBond
        >>> inflation-linked bond = InflationLinkedBond.example()
        >>> InflationLinkedBond.from_json(inflation-linked bond.to_json()).id == inflation-linked bond.id != ""
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
            and :meth:`InflationLinkedBond.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the inflation-linked bond spec as a plain Python ``dict``.

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
        Price this inflation-linked bond and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hull_white_1f"``, ...).
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
            Cash flow valuation requires the inflation-linked bond’s contractual fixed rate.
        KeyError
            If the discount curve or requested metric is missing from ``market``.
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
        extras: PriceExtras | dict[str, Any] | str | None = None,
    ) -> float:
        """
        Compute one scalar metric for this inflation-linked bond (e.g. ``"dv01"``).

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
        extras : PriceExtras or dict or str, optional
            Request configuration and overrides accepted by ``price``.
            Omit to use the stored instrument controls.

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
            If the discount curve is missing from ``market``.
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
            The registered inflation-linked bond model key ``"discounting"``.

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
    def notional(self) -> Money:
        """Read the bond's notional.

        Returns
        -------
        Money
            Positive face amount in real terms, with its nominal payment currency.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def issue_date(self) -> datetime.date:
        """Read the bond's issue date.

        Returns
        -------
        datetime.date
            Contractual issue date before payment-date adjustments.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """Read the bond's maturity.

        Returns
        -------
        datetime.date
            Contractual maturity before payment-date adjustments.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def real_coupon(self) -> Decimal:
        """Read the bond's real coupon.

        Returns
        -------
        Decimal
            Exact annual real coupon as a decimal rate, such as 0.025 for 2.5 percent.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def frequency(self) -> Tenor:
        """Read the bond's frequency.

        Returns
        -------
        Tenor
            Positive coupon interval, such as 6M for semiannual payments.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """Read the bond's day count.

        Returns
        -------
        DayCount
            Accrual year-fraction convention, such as 30_360.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """Read the bond's base date.

        Returns
        -------
        datetime.date
            Reference CPI date from the issue terms, which may differ from the issue date.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def base_cpi(self) -> float:
        """Read the bond's base cpi.

        Returns
        -------
        float
            Positive contractual base index level in CPI points.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def indexation_method(self) -> str:
        """Read the bond's indexation method.

        Returns
        -------
        str
            Indexation convention: tips, canadian, uk, french or japanese.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def lag(self) -> str:
        """Read the bond's lag.

        Returns
        -------
        str
            Contractual observation shift, using 3M for three calendar months, 90D for 90 days or none.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def deflation_protection(self) -> str:
        """Read the bond's deflation protection.

        Returns
        -------
        str
            Protected payments: none, maturity_only or all_payments.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def floor_vol_surface_id(self) -> str | None:
        """Read the bond's floor vol surface id.

        Returns
        -------
        str | None
            Black strike surface ID for unpublished protected CPI ratios.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def business_day_convention(self) -> str:
        """Read the bond's business day convention.

        Returns
        -------
        str
            Payment-date adjustment. Omit the setter to use modified_following.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def stub(self) -> str:
        """Read the bond's stub.

        Returns
        -------
        str
            Coupon stub convention. Omit the setter to use short_front.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def calendar_id(self) -> str | None:
        """Read the bond's calendar id.

        Returns
        -------
        str | None
            Holiday calendar driving contractual payment adjustment.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """Read the bond's discount curve id.

        Returns
        -------
        str
            Nominal discount curve ID used for projected nominal cash.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def inflation_index_id(self) -> str:
        """Read the bond's inflation index id.

        Returns
        -------
        str
            ID shared by the realized CPI series and projected CPI curve.

        Notes
        -----
        This method does not raise.
        """
        ...

class InflationLinkedBondBuilder:
    """Fluent builder for inflation-indexed principal and coupon terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import InflationLinkedBond
    >>> builder = InflationLinkedBond.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> InflationLinkedBond:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        InflationLinkedBond
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's id.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def notional(self, value: Money) -> InflationLinkedBondBuilder:
        """Set the bond's notional.

        Parameters
        ----------
        value : Money
            Positive face amount in real terms, with its nominal payment currency.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def issue_date(self, value: datetime.date | str) -> InflationLinkedBondBuilder:
        """Set the bond's issue date.

        Parameters
        ----------
        value : datetime.date | str
            Contractual issue date before payment-date adjustments.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def maturity(self, value: datetime.date | str) -> InflationLinkedBondBuilder:
        """Set the bond's maturity.

        Parameters
        ----------
        value : datetime.date | str
            Contractual maturity before payment-date adjustments.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def real_coupon(self, value: Decimal | str) -> InflationLinkedBondBuilder:
        """Set the bond's real coupon.

        Parameters
        ----------
        value : Decimal | str
            Exact annual real coupon as a decimal rate, such as 0.025 for 2.5 percent.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def frequency(self, value: Tenor | str) -> InflationLinkedBondBuilder:
        """Set the bond's frequency.

        Parameters
        ----------
        value : Tenor | str
            Positive coupon interval, such as 6M for semiannual payments.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def day_count(self, value: DayCount | str) -> InflationLinkedBondBuilder:
        """Set the bond's day count.

        Parameters
        ----------
        value : DayCount | str
            Accrual year-fraction convention, such as 30_360.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def base_date(self, value: datetime.date | str) -> InflationLinkedBondBuilder:
        """Set the bond's base date.

        Parameters
        ----------
        value : datetime.date | str
            Reference CPI date from the issue terms, which may differ from the issue date.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def base_cpi(self, value: float) -> InflationLinkedBondBuilder:
        """Set the bond's base cpi.

        Parameters
        ----------
        value : float
            Positive contractual base index level in CPI points.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def indexation_method(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's indexation method.

        Parameters
        ----------
        value : str
            Indexation convention: tips, canadian, uk, french or japanese.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def lag(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's lag.

        Parameters
        ----------
        value : str
            Contractual observation shift, using 3M for three calendar months, 90D for 90 days or none.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def deflation_protection(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's deflation protection.

        Parameters
        ----------
        value : str
            Protected payments: none, maturity_only or all_payments.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def floor_vol_surface_id(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's floor vol surface id.

        Parameters
        ----------
        value : str
            Black strike surface ID for unpublished protected CPI ratios.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def business_day_convention(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's business day convention.

        Parameters
        ----------
        value : str
            Payment-date adjustment. Omit the setter to use modified_following.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def stub(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's stub.

        Parameters
        ----------
        value : str
            Coupon stub convention. Omit the setter to use short_front.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def calendar_id(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's calendar id.

        Parameters
        ----------
        value : str
            Holiday calendar driving contractual payment adjustment.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def discount_curve_id(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's discount curve id.

        Parameters
        ----------
        value : str
            Nominal discount curve ID used for projected nominal cash.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def inflation_index_id(self, value: str) -> InflationLinkedBondBuilder:
        """Set the bond's inflation index id.

        Parameters
        ----------
        value : str
            ID shared by the realized CPI series and projected CPI curve.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> InflationLinkedBondBuilder:
        """Set the bond's attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        InflationLinkedBondBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> InflationLinkedBondBuilder:
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
        InflationLinkedBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(
        self, value: MetricPricingOverrides | dict[str, object] | str
    ) -> InflationLinkedBondBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        InflationLinkedBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> InflationLinkedBondBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        InflationLinkedBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
