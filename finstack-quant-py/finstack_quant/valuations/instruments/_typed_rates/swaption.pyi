"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_rates/swaption.rs``."""

from __future__ import annotations

from finstack_quant.margin import OtcMarginSpec
import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides
from finstack_quant.valuations.instruments._typed_legs import FixedLegSpec, FloatLegSpec

class Swaption:
    """
    Typed wrapper for the canonical Rust ``Swaption`` instrument.

    Construct via :meth:`Swaption.builder`, :meth:`Swaption.example` /
    :meth:`Swaption.example` or :meth:`Swaption.from_json`. Every
    public Rust field is readable as a property; ``get_strike`` /
    ``get_underlying_start_date`` / ``get_underlying_maturity`` / ``forward_swap_rate`` mirror the
    Rust accessors. Price it with :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Swaption
    >>> swpn = Swaption.example()
    >>> (swpn.option_type, swpn.get_strike(), swpn.vol_surface_id)
    ('call', 0.03, 'USD-SWPNVOL')
    """

    @staticmethod
    def builder() -> SwaptionBuilder:
        """
        Create a fluent builder (mirrors Rust ``Swaption::builder()``).

        Returns
        -------
        SwaptionBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> builder = Swaption.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def example() -> Swaption:
        """
        Canonical European 1Yx5Y USD payer swaption (mirrors Rust
        ``Swaption::example``): cash-settled, Black vol, 3% strike on a 5-year
        swap, vol surface ``USD-SWPNVOL``.

        Returns
        -------
        Swaption
            The example swaption.

        Raises
        ------
        ValueError
            If the example instrument fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> Swaption.example().settlement
        'cash'
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> Swaption:
        """
        Deserialize a validated Swaption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"swaption"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Swaption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> Swaption.from_json(Swaption.example().to_json()).id
        'SWPN-1Yx5Y-USD'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`Swaption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the swaption spec as a plain Python ``dict``.

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
    def to_builder(self) -> SwaptionBuilder:
        """
        Convert this swaption into an editable :class:`SwaptionBuilder`.

        Returns
        -------
        SwaptionBuilder
            Builder pre-populated with this swaption's fields for incremental editing.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> sw = Swaption.example().to_builder().id("SW-EDIT").build()
        >>> sw.id
        'SW-EDIT'
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
    def forward_swap_rate(
        self, market: MarketContext, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Forward swap rate of the underlying (mirrors Rust ``Swaption::forward_swap_rate``).

        Parameters
        ----------
        market : MarketContext
            Market context holding the discount and forward curves.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Par swap rate of the underlying as a decimal.

        Raises
        ------
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the annuity or floating PV cannot be computed.
        """
        ...
    def get_strike(self) -> float:
        """
        Fixed strike of the underlying swap (mirrors Rust ``get_strike``).

        Returns
        -------
        float
            Strike as a decimal rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def get_underlying_start_date(self) -> datetime.date:
        """
        Effective date of the underlying swap (mirrors Rust ``get_underlying_start_date``).

        Returns
        -------
        datetime.date
            The underlying start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def get_underlying_maturity(self) -> datetime.date:
        """
        Maturity of the underlying swap (mirrors Rust ``get_underlying_maturity``).

        Returns
        -------
        datetime.date
            The underlying end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
    def option_type(self) -> str:
        """
        Option type of the swaption.

        Returns
        -------
        str
            ``"call"`` (payer) or ``"put"`` (receiver).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional of the underlying swap.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date:
        """
        Option expiry date.

        Returns
        -------
        datetime.date
            The expiry date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement(self) -> str:
        """
        Settlement method.

        Returns
        -------
        str
            ``"physical"`` or ``"cash"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def cash_settlement_method(self) -> str:
        """
        Cash settlement annuity method (serde string).

        Returns
        -------
        str
            ``"collateralized_cash_price"``, ``"par_yield"``, ``"isda_par_par"`` or ``"zero_coupon"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_model(self) -> str:
        """
        Volatility model.

        Returns
        -------
        str
            ``"black"`` or ``"normal"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        Volatility surface identifier.

        Returns
        -------
        str
            Surface id looked up in the market context.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_fixed_leg(self) -> FixedLegSpec:
        """
        Fixed leg of the underlying swap.

        Returns
        -------
        FixedLegSpec
            The fixed leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_float_leg(self) -> FloatLegSpec:
        """
        Floating leg of the underlying swap.

        Returns
        -------
        FloatLegSpec
            The floating leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def sabr_params(self) -> dict[str, object] | None:
        """
        SABR parameters (``alpha``, ``beta``, ``nu``, ``rho``, ``shift``).

        Returns
        -------
        dict[str, object] | None
            The parameter dict, or ``None``.

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
            Registered model key such as ``"discounting"`` or ``"black76"``.

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
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical instrument JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored instrument fails.
        """
        ...

class SwaptionBuilder:
    """
    Fluent builder for :class:`Swaption`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``option_type``, ``notional``,
    ``expiry``, ``settlement``, ``cash_settlement_method``, ``vol_model``,
    ``vol_surface_id``, ``underlying_fixed_leg``, ``underlying_float_leg``.
    European exercise is the contract supported by this type.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import FixedLegSpec, FloatLegSpec, Swaption
    >>> fixed = FixedLegSpec(
    ...     discount_curve_id="USD-OIS",
    ...     rate="0.04",
    ...     frequency=Tenor.semi_annual(),
    ...     day_count=DayCount.THIRTY_360,
    ...     start="2025-01-15",
    ...     end="2030-01-15",
    ... )
    >>> floating = FloatLegSpec(
    ...     index_id="USD-SOFR-3M",
    ...     discount_curve_id="USD-OIS",
    ...     forward_curve_id="USD-SOFR-3M",
    ...     spread_bp="0",
    ...     frequency=Tenor.quarterly(),
    ...     day_count=DayCount.ACT_360,
    ...     start="2025-01-15",
    ...     end="2030-01-15",
    ... )
    >>> swaption = (
    ...     Swaption
    ...     .builder()
    ...     .id("SWPT-1")
    ...     .option_type("call")
    ...     .notional(Money(10_000_000.0, "USD"))
    ...     .expiry("2025-01-13")
    ...     .settlement("cash")
    ...     .cash_settlement_method("collateralized_cash_price")
    ...     .vol_model("black")
    ...     .vol_surface_id("USD-SWPT-VOL")
    ...     .underlying_fixed_leg(fixed)
    ...     .underlying_float_leg(floating)
    ...     .build()
    ... )
    >>> swaption.get_strike()
    0.04
    """

    def id(self, value: str) -> SwaptionBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def option_type(self, value: Literal["call", "put"]) -> SwaptionBuilder:
        """
        Set the option type.

        Parameters
        ----------
        value : Literal["call", "put"]
            Option type (serde string). ``"call"`` is a payer, ``"put"`` a receiver swaption.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def notional(self, value: Money) -> SwaptionBuilder:
        """
        Set the notional of the underlying swap.

        Parameters
        ----------
        value : Money
            Notional of the underlying swap.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``finstack_quant.core.money.Money``.
        """
        ...
    def expiry(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> SwaptionBuilder:
        """
        Set the option expiry date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Option expiry date (ISO 8601 strings accepted).

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def settlement(self, value: Literal["physical", "cash"]) -> SwaptionBuilder:
        """
        Set the settlement method.

        Parameters
        ----------
        value : Literal["physical", "cash"]
            Settlement method (serde string).

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def cash_settlement_method(
        self, value: Literal["collateralized_cash_price", "par_yield", "isda_par_par", "zero_coupon"]
    ) -> SwaptionBuilder:
        """
        Set the cash settlement annuity method (only used when ``settlement`` is ``"cash"``).

        Parameters
        ----------
        value : Literal["collateralized_cash_price", "par_yield", "isda_par_par", "zero_coupon"]
            Cash settlement annuity method (only used when ``settlement`` is ``"cash"``) (serde string). ``"collateralized_cash_price"`` discounts the physical fixed-leg annuity.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def vol_model(self, value: Literal["black", "normal"]) -> SwaptionBuilder:
        """
        Set the volatility model used for pricing.

        Parameters
        ----------
        value : Literal["black", "normal"]
            Volatility model used for pricing (serde string).

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def vol_surface_id(self, value: str) -> SwaptionBuilder:
        """
        Set the volatility surface identifier.

        Parameters
        ----------
        value : str
            Volatility surface identifier.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def underlying_fixed_leg(self, value: FixedLegSpec) -> SwaptionBuilder:
        """
        Set the complete fixed leg of the underlying swap.

        Parameters
        ----------
        value : FixedLegSpec
            Fixed leg of the underlying swap.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def underlying_float_leg(self, value: FloatLegSpec) -> SwaptionBuilder:
        """
        Set the complete floating leg of the underlying swap.

        Parameters
        ----------
        value : FloatLegSpec
            Floating leg of the underlying swap.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def sabr_params(self, value: dict[str, object] | str) -> SwaptionBuilder:
        """
        Set the SABR volatility model parameters.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``SabrParameters`` in serde form (``dict`` or JSON string), e.g. ``{"alpha": 0.025, "beta": 0.5, "nu": 0.4, "rho": -0.3, "shift": None}``.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``SabrParameters``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> SwaptionBuilder:
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
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> SwaptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> SwaptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> SwaptionBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict | None
            Attribute bag: an ``Attributes`` or its serde ``dict`` form
            (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
            ``None`` clears it.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``SwaptionBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``SwaptionBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> Swaption:
        """
        Build the validated swaption.

        Runs the same validation as the Rust ``SwaptionBuilder::build`` (structural
        invariants only); pricing-time checks run in :func:`price_instrument`.

        Returns
        -------
        Swaption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and every missing field, e.g.
            ``SwaptionBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...
