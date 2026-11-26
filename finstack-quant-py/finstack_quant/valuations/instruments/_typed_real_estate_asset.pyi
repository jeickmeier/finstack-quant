"""Typed real estate asset and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class RealEstateAsset:
    """Unlevered valuation of property cashflows and terminal proceeds.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import RealEstateAsset
    >>> isinstance(RealEstateAsset.example(), RealEstateAsset)
    True
    """

    @staticmethod
    def builder() -> RealEstateAssetBuilder:
        """Start authoring a real estate asset.

        Returns
        -------
        RealEstateAssetBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RealEstateAsset
        >>> _ = RealEstateAsset.builder().id("example-real-estate-asset")
        """
        ...

    def to_builder(self) -> RealEstateAssetBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        RealEstateAssetBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> RealEstateAsset:
        """Construct the built-in real estate asset example.

        Returns
        -------
        RealEstateAsset
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RealEstateAsset
        >>> RealEstateAsset.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> RealEstateAsset:
        """
        Deserialize a validated RealEstateAsset from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"real_estate_asset"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        RealEstateAsset
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RealEstateAsset
        >>> future = RealEstateAsset.example()
        >>> RealEstateAsset.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`RealEstateAsset.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this real estate asset as a plain Python ``dict``.

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
    def currency(self) -> str:
        """Read the contract's currency.

        Returns
        -------
        str
            Currency for valuation.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def valuation_method(self) -> dict[str, object] | str:
        """Read the contract's valuation method.

        Returns
        -------
        dict[str, object] | str
            Valuation method (DCF or DirectCap).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def property_type(self) -> dict[str, object] | str | None:
        """Read the contract's property type.

        Returns
        -------
        dict[str, object] | str | None
            Optional property type classification (for reporting).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def noi_schedule(self) -> list[tuple[date, float]]:
        """Read the contract's noi schedule.

        Returns
        -------
        list[tuple[date, float]]
            Net operating income schedule (date, amount). Entries must be **annual** NOI amounts: cap-rate formulas (direct cap, exit-cap terminal value, going-in cap rate) apply a quoted annual cap rate to a single schedule entry, so a monthly or quarterly schedule would understate those values by 12x/4x. Flows dated exactly on the valuation as_of are included in PV undiscounted (t = 0).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def capex_schedule(self) -> list[tuple[date, float]] | None:
        """Read the contract's capex schedule.

        Returns
        -------
        list[tuple[date, float]] | None
            Capital expenditure schedule (date, amount). Values are treated as **positive outflows**. When present, cashflows are valued as NOI - CapEx (unlevered net cash flow).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discount_rate(self) -> float | None:
        """Read the contract's discount rate.

        Returns
        -------
        float | None
            Discount rate for DCF (annualized).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def cap_rate(self) -> float | None:
        """Read the contract's cap rate.

        Returns
        -------
        float | None
            Capitalization rate for direct cap (annualized).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def stabilized_noi(self) -> float | None:
        """Read the contract's stabilized noi.

        Returns
        -------
        float | None
            Optional stabilized NOI override for direct cap.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def terminal_cap_rate(self) -> float | None:
        """Read the contract's terminal cap rate.

        Returns
        -------
        float | None
            Optional terminal cap rate for DCF (uses last NOI).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def terminal_growth_rate(self) -> float | None:
        """Read the contract's terminal growth rate.

        Returns
        -------
        float | None
            Optional terminal growth rate used to project NOI_{N+1} for exit valuation. Market convention for exit-cap terminal value is annual next-period NOI divided by the exit cap rate. When not provided, defaults to 0 (uses last NOI as-is, i.e. a trailing-NOI terminal value). Exactly **one** period of growth is applied to the last NOI on/before the exit date — it is not compounded over any gap between the last scheduled NOI and a later sale_date. Validation range is [-100%, 20%] to guard against configuration errors.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def sale_date(self) -> date | None:
        """Read the contract's sale date.

        Returns
        -------
        date | None
            Optional sale/exit date that truncates the DCF horizon. When set, DCF only values unlevered flows up to and including sale_date. Terminal proceeds (if configured) are realized on sale_date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def sale_price(self) -> Money | None:
        """Read the contract's sale price.

        Returns
        -------
        Money | None
            Optional explicit gross sale price (terminal proceeds), before disposition costs. When set, this takes precedence over terminal_cap_rate for terminal proceeds.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def purchase_price(self) -> Money | None:
        """Read the contract's purchase price.

        Returns
        -------
        Money | None
            Optional purchase price (useful for IRR / cap rate metrics).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def acquisition_costs(self) -> list[Money]:
        """Read the contract's acquisition costs.

        Returns
        -------
        list[Money]
            Acquisition (closing) cost line items in instrument currency, as positive outflow magnitudes deducted at as_of in DCF valuation.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def disposition_cost_decimal(self) -> float | None:
        """Read the contract's disposition cost decimal.

        Returns
        -------
        float | None
            Optional disposition cost percentage applied to terminal value. A value of 0.02 represents 2% selling costs. Must be in [0, 1).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def disposition_costs(self) -> list[Money]:
        """Read the contract's disposition costs.

        Returns
        -------
        list[Money]
            Optional detailed disposition cost line items (positive outflows) deducted from terminal proceeds.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def appraisal_value(self) -> Money | None:
        """Read the contract's appraisal value.

        Returns
        -------
        Money | None
            Optional appraisal override value.

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
            Day count convention for year fractions.

        Notes
        -----
        This property does not raise.
        """
        ...

class RealEstateAssetBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import RealEstateAsset
    >>> builder = RealEstateAsset.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> RealEstateAsset:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        RealEstateAsset
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> RealEstateAssetBuilder:
        """Set the real estate asset identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        RealEstateAssetBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def currency(self, value: Currency | str) -> RealEstateAssetBuilder:
        """Set the contract's currency.

        Parameters
        ----------
        value : Currency | str
            Currency for valuation.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def valuation_method(self, value: dict[str, object] | str) -> RealEstateAssetBuilder:
        """Set the contract's valuation method.

        Parameters
        ----------
        value : dict[str, object] | str
            Valuation method (DCF or DirectCap).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def property_type(self, value: dict[str, object] | str | None) -> RealEstateAssetBuilder:
        """Set the contract's property type.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Optional property type classification (for reporting).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def noi_schedule(self, value: list[tuple[date | str, float]]) -> RealEstateAssetBuilder:
        """Set the contract's noi schedule.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Net operating income schedule (date, amount). Entries must be **annual** NOI amounts: cap-rate formulas (direct cap, exit-cap terminal value, going-in cap rate) apply a quoted annual cap rate to a single schedule entry, so a monthly or quarterly schedule would understate those values by 12x/4x. Flows dated exactly on the valuation as_of are included in PV undiscounted (t = 0).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def capex_schedule(self, value: list[tuple[date | str, float]] | None) -> RealEstateAssetBuilder:
        """Set the contract's capex schedule.

        Parameters
        ----------
        value : list[tuple[date | str, float]] | None
            Capital expenditure schedule (date, amount). Values are treated as **positive outflows**. When present, cashflows are valued as NOI - CapEx (unlevered net cash flow).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_rate(self, value: float | None) -> RealEstateAssetBuilder:
        """Set the contract's discount rate.

        Parameters
        ----------
        value : float | None
            Discount rate for DCF (annualized).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def cap_rate(self, value: float | None) -> RealEstateAssetBuilder:
        """Set the contract's cap rate.

        Parameters
        ----------
        value : float | None
            Capitalization rate for direct cap (annualized).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def stabilized_noi(self, value: float | None) -> RealEstateAssetBuilder:
        """Set the contract's stabilized noi.

        Parameters
        ----------
        value : float | None
            Optional stabilized NOI override for direct cap.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def terminal_cap_rate(self, value: float | None) -> RealEstateAssetBuilder:
        """Set the contract's terminal cap rate.

        Parameters
        ----------
        value : float | None
            Optional terminal cap rate for DCF (uses last NOI).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def terminal_growth_rate(self, value: float | None) -> RealEstateAssetBuilder:
        """Set the contract's terminal growth rate.

        Parameters
        ----------
        value : float | None
            Optional terminal growth rate used to project NOI_{N+1} for exit valuation. Market convention for exit-cap terminal value is annual next-period NOI divided by the exit cap rate. When not provided, defaults to 0 (uses last NOI as-is, i.e. a trailing-NOI terminal value). Exactly **one** period of growth is applied to the last NOI on/before the exit date — it is not compounded over any gap between the last scheduled NOI and a later sale_date. Validation range is [-100%, 20%] to guard against configuration errors.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def sale_date(self, value: date | str | None) -> RealEstateAssetBuilder:
        """Set the contract's sale date.

        Parameters
        ----------
        value : date | str | None
            Optional sale/exit date that truncates the DCF horizon. When set, DCF only values unlevered flows up to and including sale_date. Terminal proceeds (if configured) are realized on sale_date.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def sale_price(self, value: Money | None) -> RealEstateAssetBuilder:
        """Set the contract's sale price.

        Parameters
        ----------
        value : Money | None
            Optional explicit gross sale price (terminal proceeds), before disposition costs. When set, this takes precedence over terminal_cap_rate for terminal proceeds.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def purchase_price(self, value: Money | None) -> RealEstateAssetBuilder:
        """Set the contract's purchase price.

        Parameters
        ----------
        value : Money | None
            Optional purchase price (useful for IRR / cap rate metrics).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def acquisition_costs(self, value: list[Money]) -> RealEstateAssetBuilder:
        """Set the contract's acquisition costs.

        Parameters
        ----------
        value : list[Money]
            Acquisition (closing) cost line items in instrument currency, as positive outflow magnitudes deducted at as_of in DCF valuation.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def disposition_cost_decimal(self, value: float | None) -> RealEstateAssetBuilder:
        """Set the contract's disposition cost decimal.

        Parameters
        ----------
        value : float | None
            Optional disposition cost percentage applied to terminal value. A value of 0.02 represents 2% selling costs. Must be in [0, 1).

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def disposition_costs(self, value: list[Money]) -> RealEstateAssetBuilder:
        """Set the contract's disposition costs.

        Parameters
        ----------
        value : list[Money]
            Optional detailed disposition cost line items (positive outflows) deducted from terminal proceeds.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def appraisal_value(self, value: Money | None) -> RealEstateAssetBuilder:
        """Set the contract's appraisal value.

        Parameters
        ----------
        value : Money | None
            Optional appraisal override value.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> RealEstateAssetBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention for year fractions.

        Returns
        -------
        RealEstateAssetBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> RealEstateAssetBuilder:
        """Set the real estate asset grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        RealEstateAssetBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> RealEstateAssetBuilder:
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
        RealEstateAssetBuilder
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
    ) -> RealEstateAssetBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        RealEstateAssetBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> RealEstateAssetBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        RealEstateAssetBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
