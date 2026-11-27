"""Typed lookback option and its builder."""

from __future__ import annotations

from datetime import date
from typing import Any
from finstack_quant.core.dates import DayCount
from finstack_quant.core.currency import Currency
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class LookbackOption:
    """Option whose payoff depends on an observed minimum or maximum of the underlying price.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import LookbackOption
    >>> isinstance(LookbackOption.example(), LookbackOption)
    True
    """

    @staticmethod
    def builder() -> LookbackOptionBuilder:
        """Start authoring a lookback option.

        Returns
        -------
        LookbackOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import LookbackOption
        >>> _ = LookbackOption.builder().id("example-lookback-option")
        """
        ...

    def to_builder(self) -> LookbackOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        LookbackOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> LookbackOption:
        """Construct the built-in lookback option example.

        Returns
        -------
        LookbackOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import LookbackOption
        >>> LookbackOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> LookbackOption:
        """
        Deserialize a validated LookbackOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"lookback_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        LookbackOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import LookbackOption
        >>> future = LookbackOption.example()
        >>> LookbackOption.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`LookbackOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this lookback option as a plain Python ``dict``.

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
    def strike(self) -> dict[str, object] | str | None:
        """Read the contract's strike.

        Returns
        -------
        dict[str, object] | str | None
            Strike price (None for floating strike lookbacks)

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
    def lookback_type(self) -> dict[str, object] | str:
        """Read the contract's lookback type.

        Returns
        -------
        dict[str, object] | str
            Lookback type (fixed or floating strike)

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
    def expiry_fixing(self) -> float | None:
        """Read the contract's expiry fixing.

        Returns
        -------
        float | None
            Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized payoff cannot move with a later market spot snapshot. At expiry itself, the current market spot is treated as the terminal fixing when this field is absent.

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
    def monitoring(self) -> dict[str, object] | str:
        """Read the contract's monitoring.

        Returns
        -------
        dict[str, object] | str
            Contractual monitoring of the path extremum. continuous (default) prices with the Goldman-Sosin-Gatto closed form. discrete observes the extremum only on the strictly increasing observation_dates (no later than expiry) and prices by Monte Carlo.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observed_min(self) -> float | None:
        """Read the contract's observed min.

        Returns
        -------
        float | None
            Observed minimum underlying level since inception, in the same quote units as strike (required for Floating Call / Fixed Put once seasoned).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observed_max(self) -> float | None:
        """Read the contract's observed max.

        Returns
        -------
        float | None
            Observed maximum underlying level since inception, in the same quote units as strike (required for Floating Put / Fixed Call once seasoned).

        Notes
        -----
        This property does not raise.
        """
        ...

class LookbackOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import LookbackOption
    >>> builder = LookbackOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> LookbackOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        LookbackOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> LookbackOptionBuilder:
        """Set the lookback option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        LookbackOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying_ticker(self, value: str) -> LookbackOptionBuilder:
        """Set the contract's underlying ticker.

        Parameters
        ----------
        value : str
            Underlying asset ticker symbol

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: dict[str, object] | None) -> LookbackOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : dict[str, object] | None
            Strike price (None for floating strike lookbacks)

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: dict[str, object] | str) -> LookbackOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : dict[str, object] | str
            Option type (call or put)

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def lookback_type(self, value: dict[str, object] | str) -> LookbackOptionBuilder:
        """Set the contract's lookback type.

        Parameters
        ----------
        value : dict[str, object] | str
            Lookback type (fixed or floating strike)

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> LookbackOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Option expiry date

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry_fixing(self, value: float | None) -> LookbackOptionBuilder:
        """Set the contract's expiry fixing.

        Parameters
        ----------
        value : float | None
            Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized payoff cannot move with a later market spot snapshot. At expiry itself, the current market spot is treated as the terminal fixing when this field is absent.

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quantity(self, value: float) -> LookbackOptionBuilder:
        """Set the contract's quantity.

        Parameters
        ----------
        value : float
            Number of underlying units the option is written on; PV and Greeks scale linearly with it.

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def currency(self, value: Currency | str) -> LookbackOptionBuilder:
        """Set the contract's currency.

        Parameters
        ----------
        value : Currency | str
            Currency of the strike, premium and present value.

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> LookbackOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> LookbackOptionBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID for present value calculations

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str) -> LookbackOptionBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str
            Spot price identifier

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> LookbackOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface ID

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def div_yield_id(self, value: str | None) -> LookbackOptionBuilder:
        """Set the contract's div yield id.

        Parameters
        ----------
        value : str | None
            Optional dividend-yield scalar ID

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def monitoring(self, value: dict[str, object]) -> LookbackOptionBuilder:
        """Set the contract's monitoring.

        Parameters
        ----------
        value : dict[str, object]
            Contractual monitoring of the path extremum. continuous (default) prices with the Goldman-Sosin-Gatto closed form. discrete observes the extremum only on the strictly increasing observation_dates (no later than expiry) and prices by Monte Carlo.

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observed_min(self, value: float | None) -> LookbackOptionBuilder:
        """Set the contract's observed min.

        Parameters
        ----------
        value : float | None
            Observed minimum underlying level since inception, in the same quote units as strike (required for Floating Call / Fixed Put once seasoned).

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observed_max(self, value: float | None) -> LookbackOptionBuilder:
        """Set the contract's observed max.

        Parameters
        ----------
        value : float | None
            Observed maximum underlying level since inception, in the same quote units as strike (required for Floating Put / Fixed Call once seasoned).

        Returns
        -------
        LookbackOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> LookbackOptionBuilder:
        """Set the lookback option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        LookbackOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> LookbackOptionBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        LookbackOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object]) -> LookbackOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        LookbackOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> LookbackOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        LookbackOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
