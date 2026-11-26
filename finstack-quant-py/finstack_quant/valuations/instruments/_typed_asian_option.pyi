"""Typed Asian option and its builder."""

from __future__ import annotations

from datetime import date
from typing import Any
from finstack_quant.core.dates import DayCount
from finstack_quant.core.currency import Currency
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class AsianOption:
    """Option on an average of observations of the underlying asset.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AsianOption
    >>> isinstance(AsianOption.example(), AsianOption)
    True
    """

    @staticmethod
    def builder() -> AsianOptionBuilder:
        """Start authoring an Asian option.

        Returns
        -------
        AsianOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AsianOption
        >>> _ = AsianOption.builder().id("example-asian-option")
        """
        ...

    def to_builder(self) -> AsianOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        AsianOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> AsianOption:
        """Construct the built-in Asian option example.

        Returns
        -------
        AsianOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AsianOption
        >>> AsianOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> AsianOption:
        """
        Deserialize a validated AsianOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"asian_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        AsianOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AsianOption
        >>> future = AsianOption.example()
        >>> AsianOption.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`AsianOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this Asian option as a plain Python ``dict``.

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
    def strike(self) -> float:
        """Read the contract's strike.

        Returns
        -------
        float
            Strike price

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def option_type(self) -> dict[str, object] | str:
        """Read the contract's option type.

        Returns
        -------
        dict[str, object] | str
            Option type (call or put)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def averaging_method(self) -> dict[str, object] | str:
        """Read the contract's averaging method.

        Returns
        -------
        dict[str, object] | str
            Averaging method (arithmetic or geometric)

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
    def fixing_dates(self) -> list[date]:
        """Read the contract's fixing dates.

        Returns
        -------
        list[date]
            Dates on which underlying is observed for averaging. **Note**: These dates should be pre-adjusted for business day conventions. The pricer uses these dates directly without further adjustment. See struct-level documentation for business day convention guidance.

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
            Number of underlying units the option is written on; PV and Greeks scale linearly with it.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def currency(self) -> str:
        """Read the contract's currency.

        Returns
        -------
        str
            Currency of the strike, premium and present value.

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
    def past_fixings(self) -> list[tuple[date, float]]:
        """Read the contract's past fixings.

        Returns
        -------
        list[tuple[date, float]]
            Past fixings for seasoned options (date, observed price pairs). For seasoned options where some averaging observations have already occurred, provide the historical fixings here. Only fixings that match dates in fixing_dates and are on or before the valuation date are considered.

        Notes
        -----
        This property does not raise.
        """
        ...

class AsianOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AsianOption
    >>> builder = AsianOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> AsianOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        AsianOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> AsianOptionBuilder:
        """Set the Asian option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        AsianOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying_ticker(self, value: str) -> AsianOptionBuilder:
        """Set the contract's underlying ticker.

        Parameters
        ----------
        value : str
            Underlying asset ticker symbol

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: float) -> AsianOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : float
            Strike price

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: dict[str, object] | str) -> AsianOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : dict[str, object] | str
            Option type (call or put)

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def averaging_method(self, value: dict[str, object] | str) -> AsianOptionBuilder:
        """Set the contract's averaging method.

        Parameters
        ----------
        value : dict[str, object] | str
            Averaging method (arithmetic or geometric)

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> AsianOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Option expiry date

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fixing_dates(self, value: list[date | str]) -> AsianOptionBuilder:
        """Set the contract's fixing dates.

        Parameters
        ----------
        value : list[date | str]
            Dates on which underlying is observed for averaging. **Note**: These dates should be pre-adjusted for business day conventions. The pricer uses these dates directly without further adjustment. See struct-level documentation for business day convention guidance.

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quantity(self, value: float) -> AsianOptionBuilder:
        """Set the contract's quantity.

        Parameters
        ----------
        value : float
            Number of underlying units the option is written on; PV and Greeks scale linearly with it.

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def currency(self, value: Currency | str) -> AsianOptionBuilder:
        """Set the contract's currency.

        Parameters
        ----------
        value : Currency | str
            Currency of the strike, premium and present value.

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> AsianOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> AsianOptionBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID for present value calculations

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str) -> AsianOptionBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str
            Spot price identifier

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> AsianOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface ID

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def div_yield_id(self, value: str | None) -> AsianOptionBuilder:
        """Set the contract's div yield id.

        Parameters
        ----------
        value : str | None
            Optional dividend-yield scalar ID

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def past_fixings(self, value: list[tuple[date | str, float]]) -> AsianOptionBuilder:
        """Set the contract's past fixings.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Past fixings for seasoned options (date, observed price pairs). For seasoned options where some averaging observations have already occurred, provide the historical fixings here. Only fixings that match dates in fixing_dates and are on or before the valuation date are considered.

        Returns
        -------
        AsianOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> AsianOptionBuilder:
        """Set the Asian option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        AsianOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> AsianOptionBuilder:
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
        AsianOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> AsianOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        AsianOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> AsianOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        AsianOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
