"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_rates/swap.rs``."""

from __future__ import annotations

from finstack_quant.margin import OtcMarginSpec
from finstack_quant.valuations.instruments._pricing import PriceExtras
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
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import InterestRateSwap
    >>> swap = InterestRateSwap.from_conventions({
    ...     "id": "IRS-5Y",
    ...     "notional": Money(10_000_000.0, "USD"),
    ...     "side": "pay",
    ...     "fixed_rate": 0.035,
    ...     "start_date": "2025-01-15",
    ...     "maturity": "2030-01-15",
    ...     "index_id": "USD-SOFR",
    ...     "discount_curve_id": "USD-OIS",
    ...     "forward_curve_id": "USD-SOFR",
    ... })
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
    def from_conventions(params: dict[str, Any] | str) -> InterestRateSwap:
        """
        Create a vanilla swap from registered rate-index conventions.

        Mirrors Rust ``InterestRateSwap::from_conventions`` (QuantLib
        ``MakeVanillaSwap`` ergonomics): day counts, frequencies, calendars,
        reset/payment lags and overnight compounding are resolved from the
        convention registry entry for ``index_id``.

        Parameters
        ----------
        params : dict[str, Any] | str
            ``ConventionSwapParams`` as a dict or JSON text. Every key is
            required:

            - ``id`` (str): unique instrument identifier.
            - ``notional`` (Money | dict): notional shared by both legs, a
              ``Money`` or its wire dict ``{"amount": "1000000", "currency": "USD"}``.
            - ``side`` (str): ``"pay"`` pays fixed / receives floating;
              ``"receive"`` the opposite.
            - ``fixed_rate`` (float): fixed coupon as a decimal (``0.03`` = 3%).
            - ``start_date`` / ``maturity`` (datetime.date | str): ISO 8601 dates.
            - ``index_id`` (str): registered rate index (e.g. ``"USD-SOFR"``,
              ``"USD-SOFR-3M"``, ``"EUR-EURIBOR-6M"``).
            - ``discount_curve_id`` (str): discount curve for both legs.
            - ``forward_curve_id`` (str): projection curve of the floating leg.

        Returns
        -------
        InterestRateSwap
            The validated swap.

        Raises
        ------
        ValueError
            If ``params`` has a missing, unknown or malformed key, ``index_id``
            is not registered, or validation fails.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> swap = InterestRateSwap.from_conventions({
        ...     "id": "IRS-5Y",
        ...     "notional": Money(10_000_000.0, "USD"),
        ...     "side": "pay",
        ...     "fixed_rate": 0.035,
        ...     "start_date": "2025-01-15",
        ...     "maturity": "2030-01-15",
        ...     "index_id": "USD-SOFR",
        ...     "discount_curve_id": "USD-OIS",
        ...     "forward_curve_id": "USD-SOFR",
        ... })
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
        extras: PriceExtras | dict[str, Any] | str | None = None,
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
        extras: PriceExtras | dict[str, Any] | str | None = None,
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
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import FixedLegSpec, FloatLegSpec, InterestRateSwap
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
    >>> swap = (
    ...     InterestRateSwap
    ...     .builder()
    ...     .id("IRS-1")
    ...     .notional(Money(10_000_000.0, "USD"))
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
    def notional(self, value: Money) -> InterestRateSwapBuilder:
        """
        Set the notional shared by both legs.

        Parameters
        ----------
        value : Money
            Notional shared by both legs.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``finstack_quant.core.money.Money``.
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
    def margin_spec(self, value: OtcMarginSpec | dict[str, object] | str) -> InterestRateSwapBuilder:
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
