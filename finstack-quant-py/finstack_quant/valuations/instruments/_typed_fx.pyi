"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_fx.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import BusinessDayConvention, DayCount, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides

class FxForward:
    """
    Outright FX forward on a currency pair (typed wrapper for the canonical
    Rust ``FxForward``). The notional is denominated in ``base_currency``; PV
    is reported in ``quote_currency`` via covered interest parity. A missing
    ``contract_rate`` values the forward at-market (zero PV at inception).

    Construct via :meth:`FxForward.builder`, :meth:`FxForward.from_trade_date`
    (spot-lag and tenor roll from a trade date), :meth:`FxForward.example` or
    :meth:`FxForward.from_json`; fix the rate with
    :meth:`FxForward.with_forward_points` / :meth:`FxForward.with_forward_pips`.
    Every public Rust field is readable as a property and
    :meth:`FxForward.price` / :meth:`FxForward.metric` run the same pricer as
    :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxForward
    >>> fwd = FxForward.example()
    >>> (str(fwd.base_currency), str(fwd.quote_currency), fwd.contract_rate)
    ('EUR', 'USD', 1.12)
    """

    @staticmethod
    def builder() -> FxForwardBuilder:
        """
        Create a fluent builder (mirrors Rust ``FxForward::builder()``).

        Returns
        -------
        FxForwardBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> builder = FxForward.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> FxForward:
        """
        Deserialize a validated FxForward from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_forward"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxForward
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> FxForward.from_json(FxForward.example().to_json()).id
        'EURUSD-FWD-6M'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`FxForward.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

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
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"`` is the native model).
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
        Compute one scalar metric for this instrument (e.g. ``"dv01"`` or ``"fx01"``).

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
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

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
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

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
    @staticmethod
    def example() -> FxForward:
        """
        Canonical 6-month EUR/USD forward, EUR 1,000,000 at 1.12 (mirrors Rust
        ``FxForward::example``): curves ``USD-OIS`` / ``EUR-OIS``, maturity
        2025-06-15.

        Returns
        -------
        FxForward
            The example forward.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> FxForward.example().maturity
        datetime.date(2025, 6, 15)
        """
        ...
    @staticmethod
    def from_trade_date(
        id: str,
        base_currency: Currency | str,
        quote_currency: Currency | str,
        trade_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        tenor: Tenor | str,
        notional: Money | float,
        domestic_discount_curve_id: str,
        foreign_discount_curve_id: str,
        *,
        base_calendar_id: str | None = None,
        quote_calendar_id: str | None = None,
        settlement_days: int | None = None,
        business_day_convention: BusinessDayConvention | str | None = None,
        end_of_month: bool = False,
    ) -> FxForward:
        """
        Build a forward from a trade date and a standard FX tenor (mirrors Rust
        ``FxForward::from_trade_date``): the spot date is rolled from
        ``trade_date`` by ``settlement_days`` business days (CLS-consistent pair
        roll), then ``tenor`` is added with the FX end-of-month rule and
        ``business_day_convention``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        base_currency : Currency | str
            Base (foreign) currency; notional currency.
        quote_currency : Currency | str
            Quote (domestic) currency; PV currency.
        trade_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Trade date from which spot is rolled.
        tenor : Tenor | str
            Standard FX tenor from spot, e.g. ``"3M"``.
        notional : Money | float
            Notional in ``base_currency``; a bare float is tagged with that currency.
        domestic_discount_curve_id : str
            Quote-currency discount curve identifier.
        foreign_discount_curve_id : str
            Base-currency discount curve identifier.
        base_calendar_id : str | None
            Base-currency holiday calendar; ``None`` counts weekdays only, except
            that a USD leg uses ``"usny"``.
        quote_calendar_id : str | None
            Quote-currency holiday calendar; ``None`` counts weekdays only, except
            that a USD leg uses ``"usny"``.
        settlement_days : int | None
            Contractual T+N spot lag in business days (non-negative), honored as
            given (for example ``1`` for tom-next); ``None`` uses
            :func:`finstack_quant.core.dates.fx_standard_settlement_days` for the pair.
        business_day_convention : BusinessDayConvention | str | None
            Roll rule applied to the maturity: a ``BusinessDayConvention`` or a
            canonical name accepted by ``adjust``, ignoring surrounding whitespace
            and ASCII case. ``None`` selects the Rust ``FxForward::from_trade_date``
            default, Modified Following.
        end_of_month : bool, default False
            Apply the FX end-of-month rule when spot falls on month end.

        Returns
        -------
        FxForward
            Validated at-market forward (no ``contract_rate``).

        Raises
        ------
        ValueError
            If the currencies coincide, the tenor/date is invalid, or the
            notional currency differs from ``base_currency``.
        KeyError
            If a calendar identifier is unknown.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> fwd = FxForward.from_trade_date(
        ...     "EURUSD-3M", "EUR", "USD", "2025-01-15", "3M", 1_000_000.0, "USD-OIS", "EUR-OIS"
        ... )
        >>> fwd.contract_rate is None
        True
        """
        ...
    def with_forward_points(self, spot_rate: float, forward_points: float) -> FxForward:
        """
        Return a copy whose contract rate is ``spot_rate + forward_points``
        (mirrors Rust ``FxForward::with_forward_points``). Forward points are in
        rate units (``0.0025`` for 25 pips on EUR/USD).

        Parameters
        ----------
        spot_rate : float
            Spot rate, quote currency per unit of base currency; must be positive.
        forward_points : float
            Forward points in rate units, added to ``spot_rate``.

        Returns
        -------
        FxForward
            New forward with ``contract_rate`` set.

        Raises
        ------
        ValueError
            If ``spot_rate`` is not positive/finite, or the resulting contract
            rate is not positive.
        """
        ...
    def with_forward_pips(self, spot_rate: float, pips: float) -> FxForward:
        """
        Return a copy whose contract rate is ``spot_rate + pips * pip_size``
        (mirrors Rust ``FxForward::with_forward_pips``); the pip size follows
        market convention (``0.01`` for JPY/KRW/HUF pairs, ``0.0001`` otherwise).

        Parameters
        ----------
        spot_rate : float
            Spot rate, quote currency per unit of base currency; must be positive.
        pips : float
            Forward points quoted in pips.

        Returns
        -------
        FxForward
            New forward with ``contract_rate`` set.

        Raises
        ------
        ValueError
            If ``pips`` or ``spot_rate`` is not finite, or the resulting
            contract rate is not positive.
        """
        ...
    def market_forward_rate(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Covered-interest-parity forward rate implied by the market (mirrors Rust
        ``FxForward::market_forward_rate``): ``F = S * DF_foreign(T) / DF_domestic(T)``.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying both discount curves and the FX matrix (or an
            explicit ``quoted_spot`` on the instrument).
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Forward rate, quote currency per unit of base currency.

        Raises
        ------
        KeyError
            If a discount curve or the FX spot is missing from ``market``.
        ValueError
            If the market JSON or ``as_of`` is invalid.
        """
        ...
    @property
    def base_currency(self) -> Currency:
        """
        Base (foreign) currency; the notional currency.

        Returns
        -------
        Currency
            The base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def quote_currency(self) -> Currency:
        """
        Quote (domestic) currency; the PV currency.

        Returns
        -------
        Currency
            The quote currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Maturity / settlement date.

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
    def notional(self) -> Money:
        """
        Notional amount in the base currency.

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
    def contract_rate(self) -> float | None:
        """
        Contract forward rate (quote per base).

        Returns
        -------
        float | None
            The rate, or ``None`` when at-market.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def domestic_discount_curve_id(self) -> str:
        """
        Domestic (quote-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def foreign_discount_curve_id(self) -> str:
        """
        Foreign (base-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def quoted_spot(self) -> float | None:
        """
        Explicit spot override (quote per base).

        Returns
        -------
        float | None
            The override, or ``None`` to use the FX matrix.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def base_calendar_id(self) -> str | None:
        """
        Base-currency holiday calendar identifier.

        Returns
        -------
        str | None
            The calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def quote_calendar_id(self) -> str | None:
        """
        Quote-currency holiday calendar identifier.

        Returns
        -------
        str | None
            The calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry as seen by the pricer.

        Returns
        -------
        datetime.date | None
            ``None``: FX forwards carry no option expiry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``FxForward(id='EURUSD-FWD-6M', pair='EURUSD', notional=Money(1000000.0, 'EUR'), maturity=datetime.date(2025, 6, 15), contract_rate=1.12)``.

        Returns
        -------
        str
            ``FxForward(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
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

class FxForwardBuilder:
    """
    Fluent builder for :class:`FxForward`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``base_currency``,
    ``quote_currency``, ``maturity``, ``notional``,
    ``domestic_discount_curve_id``, ``foreign_discount_curve_id``.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import FxForward
    >>> fwd = (
    ...     FxForward
    ...     .builder()
    ...     .id("EURUSD-FWD")
    ...     .base_currency("EUR")
    ...     .quote_currency(Currency("USD"))
    ...     .maturity("2025-06-15")
    ...     .notional(Money(1_000_000.0, Currency("EUR")))
    ...     .contract_rate(1.12)
    ...     .domestic_discount_curve_id("USD-OIS")
    ...     .foreign_discount_curve_id("EUR-OIS")
    ...     .build()
    ... )
    >>> fwd.contract_rate
    1.12
    """

    def id(self, value: str) -> FxForwardBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the FX forward.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def base_currency(self, value: Currency | str) -> FxForwardBuilder:
        """
        Set the base currency (foreign currency, numerator of the pair).

        Parameters
        ----------
        value : Currency | str
            Base (foreign) currency, as a ``Currency`` or ISO-4217 code.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def quote_currency(self, value: Currency | str) -> FxForwardBuilder:
        """
        Set the quote currency (domestic currency, denominator of the pair).

        Parameters
        ----------
        value : Currency | str
            Quote (domestic) currency; also the PV currency.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> FxForwardBuilder:
        """
        Set the maturity/settlement date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity/settlement date (ISO 8601 strings accepted).

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def notional(self, value: Money) -> FxForwardBuilder:
        """
        Set the notional amount in base currency.

        Parameters
        ----------
        value : Money
            Notional amount, denominated in the base currency.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def contract_rate(self, value: float) -> FxForwardBuilder:
        """
        Set the contract forward rate (quote per base).

        Parameters
        ----------
        value : float
            Contract forward rate; when never set the forward is valued at-market (zero PV at inception).

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def domestic_discount_curve_id(self, value: str) -> FxForwardBuilder:
        """
        Set the domestic (quote currency) discount curve identifier.

        Parameters
        ----------
        value : str
            Domestic (quote currency) discount curve identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def foreign_discount_curve_id(self, value: str) -> FxForwardBuilder:
        """
        Set the foreign (base currency) discount curve identifier.

        Parameters
        ----------
        value : str
            Foreign (base currency) discount curve identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def quoted_spot(self, value: float) -> FxForwardBuilder:
        """
        Set an explicit spot rate override (quote per base).

        Parameters
        ----------
        value : float
            Spot FX rate; when never set the spot is sourced from the market's FX matrix.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def base_calendar_id(self, value: str) -> FxForwardBuilder:
        """
        Set the base currency calendar identifier for business day adjustment.

        Parameters
        ----------
        value : str
            Base currency holiday calendar identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def quote_calendar_id(self, value: str) -> FxForwardBuilder:
        """
        Set the quote currency calendar identifier for business day adjustment.

        Parameters
        ----------
        value : str
            Quote currency holiday calendar identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> FxForwardBuilder:
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
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> FxForwardBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> FxForwardBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> FxForwardBuilder:
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
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``FxForwardBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``FxForwardBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> FxForward:
        """
        Build the validated FX forward.

        Runs only the Rust ``FxForwardBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`FxForward.price`.

        Returns
        -------
        FxForward
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``FxForwardBuilder: missing required field 'id'``), or the instrument
            fails validation (``base_currency`` equals ``quote_currency``, notional not in the base currency).
        """
        ...

class FxOption:
    """
    Vanilla FX option priced with Garman–Kohlhagen (typed wrapper for the
    canonical Rust ``FxOption``). ``strike`` is quoted as quote currency per
    unit of base currency; the notional is in ``base_currency``. The option
    carries its pair/venue delta convention so Greeks are reported the way
    the desk quotes them.

    Construct via :meth:`FxOption.builder`, :meth:`FxOption.european`,
    :meth:`FxOption.example` or :meth:`FxOption.from_json`. Every public Rust
    field is readable as a property; ``greeks`` / ``delta`` / ``gamma`` /
    ``vega`` / ``theta`` / ``rho`` / ``foreign_rho`` / ``vanna`` / ``volga`` /
    ``implied_vol`` mirror the Rust accessors and :meth:`FxOption.price` /
    :meth:`FxOption.metric` run the same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxOption
    >>> opt = FxOption.example()
    >>> (opt.option_type, opt.strike, opt.delta_convention["kind"])
    ('call', 1.12, 'forward')
    """

    @staticmethod
    def builder() -> FxOptionBuilder:
        """
        Create a fluent builder (mirrors Rust ``FxOption::builder()``).

        Returns
        -------
        FxOptionBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> builder = FxOption.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> FxOption:
        """
        Deserialize a validated FxOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> FxOption.from_json(FxOption.example().to_json()).id
        'FXOPT-EURUSD-CALL'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`FxOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

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
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"black76"`` (Garman–Kohlhagen) is the native model).
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
        Compute one scalar metric for this instrument (e.g. ``"delta"`` or ``"vega"``).

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
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

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
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

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
    @staticmethod
    def example() -> FxOption:
        """
        Canonical EUR/USD call, strike 1.12, EUR 1,000,000 (mirrors Rust
        ``FxOption::example``): forward-delta convention, premium in USD, curves
        ``USD-OIS`` / ``EUR-OIS``, surface ``EURUSD-VOL``.

        Returns
        -------
        FxOption
            The example option.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> FxOption.example().vol_surface_id
        'EURUSD-VOL'
        """
        ...
    @staticmethod
    def european(
        id: str,
        base_currency: Currency | str,
        quote_currency: Currency | str,
        strike: float,
        expiry: datetime.date | datetime.datetime | pd.Timestamp | str,
        notional: Money | float,
        vol_surface_id: str,
        option_type: Literal["call", "put"],
        delta_convention_kind: Literal["spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"],
        premium_currency: Currency | str,
        venue: str,
    ) -> FxOption:
        """
        Build a European FX option with currency-derived OIS curves (mirrors Rust
        ``FxOption::european``): discount curves default to ``"<QUOTE>-OIS"``
        (domestic) and ``"<BASE>-OIS"`` (foreign), with the pre-configured
        EUR/USD and GBP/USD underlying presets when applicable.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        base_currency : Currency | str
            Base (foreign) currency; notional currency.
        quote_currency : Currency | str
            Quote (domestic) currency.
        strike : float
            Strike, quote currency per unit of base currency.
        expiry : datetime.date | datetime.datetime | pd.Timestamp | str
            Expiry date.
        notional : Money | float
            Notional in ``base_currency``; a bare float is tagged with that currency.
        vol_surface_id : str
            FX volatility surface identifier.
        option_type : {"call", "put"}
            Call or put on the base currency.
        delta_convention_kind : {"spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"}
            Delta convention quoted by the venue.
        premium_currency : Currency | str
            Currency in which the premium is paid (base or quote).
        venue : str
            Non-empty market venue / quoting-source identifier.

        Returns
        -------
        FxOption
            The validated option.

        Raises
        ------
        ValueError
            If the currencies coincide, ``premium_currency`` is neither leg,
            ``venue`` is blank, or the notional is not positive.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> opt = FxOption.european(
        ...     "EURUSD-CALL",
        ...     "EUR",
        ...     "USD",
        ...     1.12,
        ...     "2025-06-15",
        ...     1_000_000.0,
        ...     "EURUSD-VOL",
        ...     "call",
        ...     "spot",
        ...     "USD",
        ...     "desk",
        ... )
        >>> (opt.domestic_discount_curve_id, opt.foreign_discount_curve_id)
        ('USD-OIS', 'EUR-OIS')
        """
        ...
    def implied_vol(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        target_price: float,
    ) -> float:
        """
        Implied volatility that reproduces ``target_price`` (mirrors Rust
        ``FxOption::implied_vol``, Garman–Kohlhagen inversion).

        Parameters
        ----------
        market : MarketContext | str
            Market carrying both discount curves and the FX spot.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        target_price : float
            Observed option PV in quote currency (same scaling as ``price``).

        Returns
        -------
        float
            Annualized lognormal volatility as a decimal (``0.10`` = 10%).

        Raises
        ------
        KeyError
            If a curve or the spot is missing from ``market``.
        RuntimeError
            If the root search does not converge (price outside no-arbitrage bounds).
        """
        ...

    def greeks(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> dict[str, float]:
        """
        Compute the standard option Greek set as a dict (mirrors the WASM
        ``greeks`` method): Greeks the selected model cannot produce are
        omitted, and any non-finite Greek raises rather than being returned.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        dict[str, float]
            Mapping of Greek name to value for every Greek the model produced.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or a returned Greek is non-finite.
        """
        ...
    @property
    def base_currency(self) -> Currency:
        """
        Base (foreign) currency; the notional currency.

        Returns
        -------
        Currency
            The base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def quote_currency(self) -> Currency:
        """
        Quote (domestic) currency.

        Returns
        -------
        Currency
            The quote currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def strike(self) -> float:
        """
        Strike, quote currency per unit of base currency.

        Returns
        -------
        float
            The strike rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def option_type(self) -> str:
        """
        Option type on the base currency.

        Returns
        -------
        str
            ``"call"`` or ``"put"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def delta_convention(self) -> dict[str, str]:
        """
        Delta convention.

        Returns
        -------
        dict[str, str]
            ``{"kind", "premium_currency", "venue"}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date:
        """
        Last date on which the option may be exercised.

        Returns
        -------
        datetime.date
            Unadjusted calendar expiry date. Time to expiry used in pricing
            is measured from the valuation date to this date under the
            pricing day-count convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Day count for the time-to-expiry year fraction.

        Returns
        -------
        DayCount
            ``DayCount.ACT_365F`` unless set otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional amount in the base currency.

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
    def domestic_discount_curve_id(self) -> str:
        """
        Domestic (quote-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def foreign_discount_curve_id(self) -> str:
        """
        Foreign (base-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        FX volatility surface identifier.

        Returns
        -------
        str
            The surface id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``FxOption(id='FXOPT-EURUSD-CALL', pair='EURUSD', option_type='call', strike=1.12, expiry=datetime.date(2030, 1, 15), notional=Money(1000000.0, 'EUR'))``.

        Returns
        -------
        str
            ``FxOption(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
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

class FxOptionBuilder:
    """
    Fluent builder for :class:`FxOption`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``base_currency``,
    ``quote_currency``, ``strike``, ``option_type``, ``delta_convention``,
    ``expiry``, ``notional``, ``domestic_discount_curve_id``,
    ``foreign_discount_curve_id``, ``vol_surface_id`` (``day_count``
    defaults to ACT/365F).

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import FxOption
    >>> opt = (
    ...     FxOption
    ...     .builder()
    ...     .id("EURUSD-CALL")
    ...     .base_currency("EUR")
    ...     .quote_currency("USD")
    ...     .strike(1.12)
    ...     .option_type("call")
    ...     .delta_convention("spot", "USD", "desk")
    ...     .expiry("2025-06-15")
    ...     .notional(Money(1_000_000.0, Currency("EUR")))
    ...     .domestic_discount_curve_id("USD-OIS")
    ...     .foreign_discount_curve_id("EUR-OIS")
    ...     .vol_surface_id("EURUSD-VOL")
    ...     .build()
    ... )
    >>> opt.delta_convention["venue"]
    'desk'
    """

    def id(self, value: str) -> FxOptionBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the FX option.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def base_currency(self, value: Currency | str) -> FxOptionBuilder:
        """
        Set the base currency (foreign currency).

        Parameters
        ----------
        value : Currency | str
            Base (foreign) currency, as a ``Currency`` or ISO-4217 code.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def quote_currency(self, value: Currency | str) -> FxOptionBuilder:
        """
        Set the quote currency (domestic currency).

        Parameters
        ----------
        value : Currency | str
            Quote (domestic) currency, as a ``Currency`` or ISO-4217 code.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def strike(self, value: float) -> FxOptionBuilder:
        """
        Set the strike exchange rate (quote per base).

        Parameters
        ----------
        value : float
            Strike exchange rate, quote currency per unit of base currency.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def option_type(self, value: Literal["call", "put"]) -> FxOptionBuilder:
        """
        Set the option type: ``"call"`` or ``"put"`` on base currency.

        Parameters
        ----------
        value : Literal["call", "put"]
            Option type of the FX option.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized option type.
        """
        ...
    def delta_convention(
        self,
        kind: Literal["spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"],
        premium_currency: Currency | str,
        venue: str,
    ) -> FxOptionBuilder:
        """
        Set the pair/venue delta convention and premium currency.

        Parameters
        ----------
        kind : {"spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"}
            Delta convention quoted by the venue.
        premium_currency : Currency | str
            Currency in which the FX option premium is paid.
        venue : str
            Non-empty market venue or quoting-source identifier.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, ``kind`` is unknown or ``venue`` is blank.
        """
        ...
    def expiry(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> FxOptionBuilder:
        """
        Set the option expiry date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Option expiry date (ISO 8601 strings accepted).

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def day_count(self, value: DayCount | str) -> FxOptionBuilder:
        """
        Set the model day count for volatility, dividend carry and exercise times.

        Parameters
        ----------
        value : DayCount | str
            Day count convention; ACT/365F when never set.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized day count.
        """
        ...
    def notional(self, value: Money) -> FxOptionBuilder:
        """
        Set the notional amount in base currency.

        Parameters
        ----------
        value : Money
            Notional amount, denominated in the base currency.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def domestic_discount_curve_id(self, value: str) -> FxOptionBuilder:
        """
        Set the domestic currency discount curve identifier.

        Parameters
        ----------
        value : str
            Domestic currency discount curve identifier.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def foreign_discount_curve_id(self, value: str) -> FxOptionBuilder:
        """
        Set the foreign currency discount curve identifier.

        Parameters
        ----------
        value : str
            Foreign currency discount curve identifier.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_surface_id(self, value: str) -> FxOptionBuilder:
        """
        Set the FX volatility surface identifier.

        Parameters
        ----------
        value : str
            FX volatility surface identifier for option pricing.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> FxOptionBuilder:
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
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> FxOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> FxOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> FxOptionBuilder:
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
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``FxOptionBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``FxOptionBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> FxOption:
        """
        Build the validated FX option.

        Runs only the Rust ``FxOptionBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`FxOption.price`.

        Returns
        -------
        FxOption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``FxOptionBuilder: missing required field 'id'``), or the instrument
            fails validation (``base_currency`` equals ``quote_currency``, premium currency is neither leg, notional not in the base currency).
        """
        ...
