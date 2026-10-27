"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_rates.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides
from finstack_quant.valuations.instruments._typed_legs import FixedLegSpec, FloatLegSpec

class InterestRateSwap:
    """
    Typed wrapper for the canonical Rust ``InterestRateSwap`` instrument.

    Construct via :meth:`InterestRateSwap.from_conventions` (market
    conventions resolved from the rate-index registry, the preferred way
    to build standard swaps), :meth:`InterestRateSwap.builder` with explicit
    :class:`FixedLegSpec` / :class:`FloatLegSpec` legs,
    :meth:`InterestRateSwap.example` or
    :meth:`InterestRateSwap.from_json`. Every public Rust field is readable
    as a property; :meth:`InterestRateSwap.price` /
    :meth:`InterestRateSwap.metric` run the same pricer as
    :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import InterestRateSwap
    >>> swap = InterestRateSwap.from_conventions(
    ...     "IRS-5Y",
    ...     10_000_000.0,
    ...     "pay",
    ...     0.035,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ...     "USD-SOFR",
    ...     "USD-OIS",
    ...     "USD-SOFR",
    ...     currency="USD",
    ... )
    >>> (swap.side, swap.float_leg.reset_lag_days)
    ('pay', 0)
    """

    @staticmethod
    def builder() -> InterestRateSwapBuilder:
        """
        Create a fluent builder (mirrors Rust ``InterestRateSwap::builder()``).

        Returns
        -------
        InterestRateSwapBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> builder = InterestRateSwap.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def from_conventions(
        id: str,
        notional: Money | float,
        side: Literal["pay", "receive"],
        fixed_rate: float | Rate,
        start_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        index_id: str,
        discount_curve_id: str,
        forward_curve_id: str,
        *,
        currency: str | None = None,
    ) -> InterestRateSwap:
        """
        Create a vanilla swap from registered rate-index conventions.

        Mirrors Rust ``InterestRateSwap::from_conventions`` (QuantLib
        ``MakeVanillaSwap`` ergonomics): day counts, frequencies, calendars,
        reset/payment lags and overnight compounding are resolved from the
        convention registry entry for ``index_id``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Notional shared by both legs; a bare number is tagged with ``currency``.
        side : str
            ``"pay"`` pays fixed / receives floating; ``"receive"`` the opposite.
        fixed_rate : float | Rate
            Fixed coupon as a decimal (``0.03`` = 3%) or a ``Rate``.
        start_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Effective date (ISO 8601 strings accepted).
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        index_id : str
            Registered rate index (e.g. ``"USD-SOFR"``, ``"USD-SOFR-3M"``,
            ``"EUR-EURIBOR-6M"``).
        discount_curve_id : str
            Discount curve identifier for both legs.
        forward_curve_id : str
            Projection curve identifier for the floating leg.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        InterestRateSwap
            The validated swap.

        Raises
        ------
        ValueError
            If ``side`` is unknown, ``index_id`` is not registered, a bare
            ``notional`` has no ``currency``, or validation fails.
        TypeError
            If ``fixed_rate``/``notional`` has an unsupported type or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> swap = InterestRateSwap.from_conventions(
        ...     "IRS-5Y",
        ...     10_000_000.0,
        ...     "pay",
        ...     0.035,
        ...     "2025-01-15",
        ...     "2030-01-15",
        ...     "USD-SOFR",
        ...     "USD-OIS",
        ...     "USD-SOFR",
        ...     currency="USD",
        ... )
        >>> (swap.side, swap.float_leg.reset_lag_days)
        ('pay', 0)
        """
        ...
    @staticmethod
    def example() -> InterestRateSwap:
        """
        Canonical 5-year USD pay-fixed swap (mirrors Rust
        ``InterestRateSwap::example``): semi-annual 30/360 fixed vs
        quarterly ACT/360 ``USD-SOFR-3M``, T-2 reset lag, ``usny`` calendar.

        Returns
        -------
        InterestRateSwap
            The example swap.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> InterestRateSwap.example().float_leg.reset_lag_days
        2
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> InterestRateSwap:
        """
        Deserialize a validated InterestRateSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"interest_rate_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        InterestRateSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> swap = InterestRateSwap.example()
        >>> InterestRateSwap.from_json(swap.to_json()).id == swap.id
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
            and :meth:`InterestRateSwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the swap spec as a plain Python ``dict``.

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
        metric_pricing_overrides: MetricPricingOverrides | dict[str, Any] | str | None = None,
        market_history: MarketHistory | dict[str, Any] | str | None = None,
        explain: bool = False,
    ) -> ValuationResult:
        """
        Price this swap and return a :class:`~finstack_quant.valuations.ValuationResult`.

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
        metric_pricing_overrides : MetricPricingOverrides | dict[str, Any] | str, optional
            Metric-time overrides merged into
            ``instrument.spec.metric_pricing_overrides`` before pricing
            (e.g. ``{"theta_period": {"count": 1, "unit": "days"}}``).
        market_history : MarketHistory | dict[str, Any] | str, optional
            ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.
        explain : bool, default False
            When ``True``, attach a step trace as ``result.explanation``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
            A seasoned floating period whose fixing date precedes ``as_of`` needs a
            ``ScalarTimeSeries("FIXING:<forward_curve_id>", ...)`` in ``market``; the
            message names the series id.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
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
        Compute one scalar metric for this swap (e.g. ``"dv01"``).

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
            If a required curve or surface is missing from ``market``.
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
    def notional(self) -> Money:
        """
        Notional shared by both legs.

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
    def side(self) -> str:
        """
        Swap direction for the fixed leg.

        Returns
        -------
        str
            ``"pay"`` or ``"receive"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fixed_leg(self) -> FixedLegSpec:
        """
        Fixed leg specification.

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
    def float_leg(self) -> FloatLegSpec:
        """
        Floating leg specification.

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
    def margin_spec(self) -> dict[str, object] | None:
        """
        OTC margin (CSA / initial-margin) specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The spec dict, or ``None``.

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
            ``metric_pricing_overrides`` argument to ``price`` is merged over it
            per call.

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
        """
        Expiry date exposed by the Rust ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry/maturity date, or ``None`` when the instrument type reports none.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def adjust_fixed_accrual_dates(self) -> bool:
        """Whether fixed coupon accrual boundaries are adjusted.

        Returns
        -------
        bool
            True uses the fixed leg's calendar and business-day convention;
            false (default) retains contractual accrual dates.

        Notes
        -----
        This accessor does not raise. Payment-date adjustment remains independent.
        """
        ...

    @property
    def adjust_float_accrual_dates(self) -> bool:
        """Whether floating coupon accrual boundaries are adjusted.

        Returns
        -------
        bool
            True uses the floating leg's calendar and business-day convention;
            false (default) retains contractual accrual dates.

        Notes
        -----
        This accessor does not raise. Payment-date adjustment remains independent.
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

class InterestRateSwapBuilder:
    """
    Fluent builder for :class:`InterestRateSwap`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``side``, ``fixed``,
    ``float``.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import FixedLegSpec, FloatLegSpec, InterestRateSwap
    >>> fixed = FixedLegSpec(
    ...     "USD-OIS",
    ...     0.04,
    ...     Tenor.semi_annual(),
    ...     DayCount.THIRTY_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ... )
    >>> floating = FloatLegSpec(
    ...     "USD-OIS",
    ...     "USD-SOFR-3M",
    ...     0.0,
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ... )
    >>> swap = (
    ...     InterestRateSwap
    ...     .builder()
    ...     .id("IRS-1")
    ...     .notional(10_000_000.0, currency="USD")
    ...     .side("pay")
    ...     .fixed_leg(fixed)
    ...     .float_leg(floating)
    ...     .build()
    ... )
    >>> swap.notional.amount
    10000000.0
    """

    def id(self, value: str) -> InterestRateSwapBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money | float, currency: str | None = None) -> InterestRateSwapBuilder:
        """
        Set the notional shared by both legs.

        Parameters
        ----------
        value : Money | float
            Notional shared by both legs; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def side(self, value: Literal["pay", "receive"]) -> InterestRateSwapBuilder:
        """
        Set the swap direction for the fixed leg.

        Parameters
        ----------
        value : Literal["pay", "receive"]
            Swap direction for the fixed leg (serde string). ``"pay"`` pays fixed / receives floating.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def fixed_leg(self, value: FixedLegSpec) -> InterestRateSwapBuilder:
        """
        Set the fixed leg specification.

        Parameters
        ----------
        value : FixedLegSpec
            Fixed leg specification.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def float_leg(self, value: FloatLegSpec) -> InterestRateSwapBuilder:
        """
        Set the floating leg specification.

        Parameters
        ----------
        value : FloatLegSpec
            Floating leg specification.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def margin_spec(self, value: dict[str, object] | str) -> InterestRateSwapBuilder:
        """
        Set the OTC margin (CSA / initial-margin) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OtcMarginSpec`` in serde form (``dict`` or JSON string), e.g. the ``margin_spec`` value of a margined swap's ``to_dict()``.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``OtcMarginSpec``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> InterestRateSwapBuilder:
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
        InterestRateSwapBuilder
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
    ) -> InterestRateSwapBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> InterestRateSwapBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> InterestRateSwapBuilder:
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
        InterestRateSwapBuilder
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
        ``InterestRateSwapBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``InterestRateSwapBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> InterestRateSwap:
        """
        Build the validated swap.

        Runs the same validation as the Rust ``InterestRateSwapBuilder::build`` (structural
        invariants only); pricing-time checks run in ``InterestRateSwap.price``.

        Returns
        -------
        InterestRateSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``InterestRateSwapBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

    def adjust_fixed_accrual_dates(self, value: bool) -> InterestRateSwapBuilder:
        """Select fixed coupon accrual-date adjustment.

        Parameters
        ----------
        value : bool
            True uses the fixed leg's calendar and business-day convention;
            false (default) retains contractual dates independently of payment dates.

        Returns
        -------
        InterestRateSwapBuilder
            This builder for chaining.

        Raises
        ------
        ValueError
            If this builder has already been consumed by build.
        TypeError
            If value is not a boolean.
        """
        ...

    def adjust_float_accrual_dates(self, value: bool) -> InterestRateSwapBuilder:
        """Select floating coupon accrual-date adjustment.

        Parameters
        ----------
        value : bool
            True uses the floating leg's calendar and business-day convention;
            false (default) retains contractual dates independently of payment dates.

        Returns
        -------
        InterestRateSwapBuilder
            This builder for chaining.

        Raises
        ------
        ValueError
            If this builder has already been consumed by build.
        TypeError
            If value is not a boolean.
        """
        ...

class Swaption:
    """
    Typed wrapper for the canonical Rust ``Swaption`` instrument.

    Construct via :meth:`Swaption.builder`, :meth:`Swaption.example` /
    :meth:`Swaption.example` or :meth:`Swaption.from_json`. Every
    public Rust field is readable as a property; ``get_strike`` /
    ``get_underlying_start_date`` / ``get_underlying_maturity`` / ``forward_swap_rate`` mirror the
    Rust accessors and :meth:`Swaption.price` / :meth:`Swaption.metric` run
    the same pricer as :func:`price_instrument`.

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
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        metric_pricing_overrides: MetricPricingOverrides | dict[str, Any] | str | None = None,
        market_history: MarketHistory | dict[str, Any] | str | None = None,
        explain: bool = False,
    ) -> ValuationResult:
        """
        Price this swaption and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"black76"``, ``"normal"``, ``"hull_white_1f"``, ...).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        metric_pricing_overrides : MetricPricingOverrides | dict[str, Any] | str, optional
            Metric-time overrides merged into
            ``instrument.spec.metric_pricing_overrides`` before pricing
            (e.g. ``{"theta_period": {"count": 1, "unit": "days"}}``).
        market_history : MarketHistory | dict[str, Any] | str, optional
            ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.
        explain : bool, default False
            When ``True``, attach a step trace as ``result.explanation``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
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
        Compute one scalar metric for this swaption (e.g. ``"vega"``).

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
            If a required curve or surface is missing from ``market``.
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
    def forward_swap_rate(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Forward swap rate of the underlying (mirrors Rust ``Swaption::forward_swap_rate``).

        Parameters
        ----------
        market : MarketContext | str
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
        Canonical model key used when ``model="default"`` is passed to ``price``.

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
            ``metric_pricing_overrides`` argument to ``price`` is merged over it
            per call.

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
    >>> from finstack_quant.valuations.instruments import FixedLegSpec, FloatLegSpec, Swaption
    >>> fixed = FixedLegSpec(
    ...     "USD-OIS",
    ...     0.04,
    ...     Tenor.semi_annual(),
    ...     DayCount.THIRTY_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ... )
    >>> floating = FloatLegSpec(
    ...     "USD-OIS",
    ...     "USD-SOFR-3M",
    ...     0.0,
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ... )
    >>> swaption = (
    ...     Swaption
    ...     .builder()
    ...     .id("SWPT-1")
    ...     .option_type("call")
    ...     .notional(10_000_000.0, currency="USD")
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
    def notional(self, value: Money | float, currency: str | None = None) -> SwaptionBuilder:
        """
        Set the notional of the underlying swap.

        Parameters
        ----------
        value : Money | float
            Notional of the underlying swap; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
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
        invariants only); pricing-time checks run in ``Swaption.price``.

        Returns
        -------
        Swaption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``SwaptionBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

class CapFloor:
    """
    Typed wrapper for the canonical Rust ``CapFloor`` instrument.

    Construct via :meth:`CapFloor.builder`, :meth:`CapFloor.example` or
    :meth:`CapFloor.from_json`. Every public Rust field is readable as a
    property; :meth:`CapFloor.price` / :meth:`CapFloor.metric` run the same
    pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CapFloor
    >>> cap = CapFloor.example()
    >>> (cap.rate_option_type, cap.strike, cap.vol_type)
    ('cap', 0.03, 'auto')
    """

    @staticmethod
    def builder() -> CapFloorBuilder:
        """
        Create a fluent builder (mirrors Rust ``CapFloor::builder()``).

        Returns
        -------
        CapFloorBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented
        defaults. Unset ``vol_type`` defaults to ``"auto"``: the surface is treated
        as a lognormal quote and each caplet uses Black-76 when forward and strike
        are positive, otherwise an equivalent Bachelier price. A normal-vol surface
        must set ``vol_type`` to ``"normal"``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CapFloor
        >>> builder = CapFloor.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def example() -> CapFloor:
        """
        Canonical 5-year USD 3% cap (mirrors Rust ``CapFloor::example``): quarterly
        ACT/360 on ``USD-SOFR-3M`` discounted on ``USD-OIS`` with vol surface
        ``USD-CAPFLOOR-VOL``.

        Returns
        -------
        CapFloor
            The example cap.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CapFloor
        >>> CapFloor.example().forward_curve_id
        'USD-SOFR-3M'
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CapFloor:
        """
        Deserialize a validated CapFloor from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cap_floor"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CapFloor
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CapFloor
        >>> CapFloor.from_json(CapFloor.example().to_json()).id
        'IRCAP-USD-5Y-3PCT'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CapFloor.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the cap/floor spec as a plain Python ``dict``.

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
        metric_pricing_overrides: MetricPricingOverrides | dict[str, Any] | str | None = None,
        market_history: MarketHistory | dict[str, Any] | str | None = None,
        explain: bool = False,
    ) -> ValuationResult:
        """
        Price this cap/floor and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"black76"``, ``"normal"``, ``"hull_white_1f"``, ...).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        metric_pricing_overrides : MetricPricingOverrides | dict[str, Any] | str, optional
            Metric-time overrides merged into
            ``instrument.spec.metric_pricing_overrides`` before pricing
            (e.g. ``{"theta_period": {"count": 1, "unit": "days"}}``).
        market_history : MarketHistory | dict[str, Any] | str, optional
            ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.
        explain : bool, default False
            When ``True``, attach a step trace as ``result.explanation``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
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
        Compute one scalar metric for this cap/floor (e.g. ``"vega"``).

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
            If a required curve or surface is missing from ``market``.
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
    def rate_option_type(self) -> str:
        """
        Option type of the cap/floor.

        Returns
        -------
        str
            ``"cap"``, ``"floor"``, ``"caplet"`` or ``"floorlet"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional amount.

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
    def strike(self) -> float:
        """
        Strike as a decimal rate.

        Returns
        -------
        float
            ``0.03`` for 3%.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spread_bp(self) -> float:
        """
        Contractual margin added to the index, in basis points.

        Returns
        -------
        float
            ``0.0`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start_date(self) -> datetime.date:
        """
        Start date of the underlying period.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        End date of the underlying period.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Holiday calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_style(self) -> str:
        """
        Exercise style (serde string).

        Returns
        -------
        str
            ``"european"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement(self) -> str:
        """
        Settlement type (serde string).

        Returns
        -------
        str
            ``"cash"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used for discounting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def forward_curve_id(self) -> str:
        """
        Forward curve identifier.

        Returns
        -------
        str
            Curve id used to project the index.

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
    def vol_type(self) -> str:
        """
        Volatility convention.

        Returns
        -------
        str
            ``"lognormal"``, ``"shifted_lognormal"``, ``"normal"`` or ``"auto"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_shift(self) -> float:
        """
        Displacement shift for shifted-lognormal pricing.

        Returns
        -------
        float
            Non-negative shift; ``0.0`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def overnight_coupon(self) -> dict[str, object] | None:
        """
        Overnight (RFR) coupon convention in serde form.

        Returns
        -------
        dict[str, object] | None
            The convention dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def premium(self) -> tuple[datetime.date, Money] | None:
        """
        Dated premium paid by the holder.

        Returns
        -------
        tuple[datetime.date, Money] | None
            ``(payment_date, amount)`` or ``None``.

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
            ``metric_pricing_overrides`` argument to ``price`` is merged over it
            per call.

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
        """
        Expiry date exposed by the Rust ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry/maturity date, or ``None`` when the instrument type reports none.

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

class CapFloorBuilder:
    """
    Fluent builder for :class:`CapFloor`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``rate_option_type``, ``notional``,
    ``strike``, ``start_date``, ``maturity``, ``frequency``, ``day_count``,
    ``discount_curve_id``, ``forward_curve_id``, ``vol_surface_id``.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import CapFloor
    >>> cap = (
    ...     CapFloor
    ...     .builder()
    ...     .id("CAP-1")
    ...     .rate_option_type("cap")
    ...     .notional(5_000_000.0, currency="USD")
    ...     .strike(0.05)
    ...     .start_date("2024-01-15")
    ...     .maturity("2027-01-15")
    ...     .frequency(Tenor.quarterly())
    ...     .day_count(DayCount.ACT_360)
    ...     .discount_curve_id("USD-OIS")
    ...     .forward_curve_id("USD-SOFR-3M")
    ...     .vol_surface_id("USD-CAP-VOL")
    ...     .build()
    ... )
    >>> cap.vol_type
    'auto'
    """

    def id(self, value: str) -> CapFloorBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def rate_option_type(self, value: Literal["cap", "floor", "caplet", "floorlet"]) -> CapFloorBuilder:
        """
        Set the option type.

        Parameters
        ----------
        value : Literal["cap", "floor", "caplet", "floorlet"]
            Option type (serde string). ``"cap"``/``"floor"`` price a series of caplets/floorlets, ``"caplet"``/``"floorlet"`` a single period.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def notional(self, value: Money | float, currency: str | None = None) -> CapFloorBuilder:
        """
        Set the notional amount.

        Parameters
        ----------
        value : Money | float
            Notional amount; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def strike(self, value: float | Rate) -> CapFloorBuilder:
        """
        Set the strike rate of every caplet/floorlet.

        Parameters
        ----------
        value : float | Rate
            Strike as a decimal (``0.05`` = 5%) or a ``Rate``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not finite.
        """
        ...
    def spread_bp(self, value: float | Bps) -> CapFloorBuilder:
        """
        Set the contractual margin added to the referenced rate.

        Parameters
        ----------
        value : float | Bps
            Margin in basis points (``10`` = 10bp) or a ``Bps``, added after projecting the index.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not finite.
        """
        ...
    def premium(
        self,
        payment_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        amount: Money | float,
        currency: str | None = None,
    ) -> CapFloorBuilder:
        """
        Set the dated premium paid by the cap/floor holder.

        Parameters
        ----------
        payment_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Contractual premium payment date. Payments on or before the valuation
            date are treated as settled and excluded from NPV.
        amount : Money | float
            Non-negative premium outflow in the notional currency; a bare number is
            tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``amount`` is a bare number.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``payment_date`` cannot be interpreted, a bare amount has no
            ``currency``, or the builder was already consumed. Premium amount and
            currency validation occurs in ``build``.
        """
        ...
    def start_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CapFloorBuilder:
        """
        Set the start date of the underlying period.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of the underlying period (ISO 8601 strings accepted).

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CapFloorBuilder:
        """
        Set the end date of the underlying period.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of the underlying period (ISO 8601 strings accepted).

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def frequency(self, value: Tenor | str) -> CapFloorBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency for caps/floors.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, or a string ``value`` is not a recognized tenor string.
        """
        ...
    def day_count(self, value: DayCount | str) -> CapFloorBuilder:
        """
        Set the day count convention.

        Parameters
        ----------
        value : DayCount | str
            Day count convention.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, or a string ``value`` is not a recognized day-count name.
        """
        ...
    def stub(
        self, value: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
    ) -> CapFloorBuilder:
        """
        Set the stub rule.

        Parameters
        ----------
        value : StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
            Stub rule (serde string). Default ``"short_front"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def business_day_convention(self, value: str) -> CapFloorBuilder:
        """
        Set the business day convention.

        Parameters
        ----------
        value : str
            Business day convention (serde string). Default ``"modified_following"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def calendar_id(self, value: str) -> CapFloorBuilder:
        """
        Set the holiday calendar identifier for schedule and roll conventions.

        Parameters
        ----------
        value : str
            Holiday calendar identifier for schedule and roll conventions.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def exercise_style(self, value: str) -> CapFloorBuilder:
        """
        Set the exercise style.

        Parameters
        ----------
        value : str
            Exercise style (serde string). Default ``"european"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def settlement(self, value: str) -> CapFloorBuilder:
        """
        Set the settlement type.

        Parameters
        ----------
        value : str
            Settlement type (serde string). Default ``"cash"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def discount_curve_id(self, value: str) -> CapFloorBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def forward_curve_id(self, value: str) -> CapFloorBuilder:
        """
        Set the forward curve identifier.

        Parameters
        ----------
        value : str
            Forward curve identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_surface_id(self, value: str) -> CapFloorBuilder:
        """
        Set the volatility surface identifier.

        Parameters
        ----------
        value : str
            Volatility surface identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_type(self, value: Literal["lognormal", "shifted_lognormal", "normal", "auto"]) -> CapFloorBuilder:
        """
        Set the volatility type convention.

        Parameters
        ----------
        value : Literal["lognormal", "shifted_lognormal", "normal", "auto"]
            Volatility type convention (serde string). Must match the configured surface; ``"auto"`` (the default when unset) follows source convention and displacement metadata. Normal quotes use decimal rate units; Black quotes use dimensionless annual volatility. Incompatible model/source conventions raise ``ValueError``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def vol_shift(self, value: float) -> CapFloorBuilder:
        """
        Set the displacement shift used for shifted-lognormal pricing.

        Parameters
        ----------
        value : float
            Displacement added to forward and strike; must be non-negative.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def overnight_coupon(self, value: dict[str, object] | str) -> CapFloorBuilder:
        """
        Set the overnight (RFR) coupon convention for compounded caplets.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OvernightCouponConvention`` in serde form (``dict`` or JSON string), e.g. ``{"compounding": {"compounded_in_arrears": {"lookback_days": 0}}, "payment_lag_days": 2}``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``OvernightCouponConvention``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CapFloorBuilder:
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
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> CapFloorBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CapFloorBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> CapFloorBuilder:
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
        CapFloorBuilder
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
        ``CapFloorBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CapFloorBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CapFloor:
        """
        Build the validated cap/floor.

        Runs the same validation as the Rust ``CapFloorBuilder::build`` (structural
        invariants only); pricing-time checks run in ``CapFloor.price``.

        Returns
        -------
        CapFloor
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CapFloorBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...
