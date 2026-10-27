"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_equity.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides

class EquityOption:
    """
    Vanilla equity option (typed wrapper for the canonical Rust
    ``EquityOption``). European options price with Black–Scholes–Merton
    (``"black76"`` on the forward); American and Bermudan styles use the tree
    pricer. ``quantity`` (underlying units) scales the per-share value and
    ``currency`` is the premium/PV currency; discrete dividends and a
    continuous ``div_yield_id`` are both supported.

    Construct via :meth:`EquityOption.builder`,
    :meth:`EquityOption.european`, :meth:`EquityOption.example` or
    :meth:`EquityOption.from_json`. Every public Rust field is readable as a
    property; ``greeks`` / ``delta`` / ``gamma`` / ``vega`` / ``theta`` /
    ``rho`` / ``implied_vol`` mirror the Rust accessors and
    :meth:`EquityOption.price` / :meth:`EquityOption.metric` run the same
    pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityOption
    >>> opt = EquityOption.example()
    >>> (opt.underlying_ticker, opt.strike, opt.option_type, opt.exercise_style)
    ('SPX', 4500.0, 'call', 'european')
    """

    @staticmethod
    def builder() -> EquityOptionBuilder:
        """
        Create a fluent builder (mirrors Rust ``EquityOption::builder()``).

        Returns
        -------
        EquityOptionBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> builder = EquityOption.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> EquityOption:
        """
        Deserialize a validated EquityOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"equity_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        EquityOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> EquityOption.from_json(EquityOption.example().to_json()).id
        'SPX-CALL-4500'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`EquityOption.from_json`.

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
            Model key (``"black76"`` (European) or ``"tree"``).
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
    def example() -> EquityOption:
        """
        Canonical SPX 4500 European call expiring 2024-06-21 (mirrors Rust
        ``EquityOption::example``): 100 units in USD, curve ``USD-OIS``, spot
        ``EQUITY-SPOT``, surface ``EQUITY-VOL``, dividend yield ``EQUITY-DIVYIELD``.

        Returns
        -------
        EquityOption
            The example option.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> EquityOption.example().vol_surface_id
        'EQUITY-VOL'
        """
        ...
    @staticmethod
    def european(
        id: str,
        ticker: str,
        strike: float,
        expiry: datetime.date | datetime.datetime | pd.Timestamp | str,
        quantity: float,
        currency: Currency | str,
        option_type: str,
        *,
        discount_curve_id: str = "USD-OIS",
        spot_id: str = "EQUITY-SPOT",
        vol_surface_id: str = "EQUITY-VOL",
        div_yield_id: str | None = "EQUITY-DIVYIELD",
    ) -> EquityOption:
        """
        Build a cash-settled European call or put (mirrors Rust
        ``EquityOption::european`` / ``european_with_market_data``).
        The market-data identifiers default to the same generic ids the Rust
        constructor uses; pass your own to bind the option to real market
        objects.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        ticker : str
            Underlying equity ticker.
        strike : float
            Strike price; must be finite and positive.
        expiry : datetime.date | datetime.datetime | pd.Timestamp | str
            Expiry date.
        quantity : float
            Number of underlying units; PV and Greeks scale linearly with it.
        currency : Currency | str
            Currency of the strike, premium and present value (e.g. ``"USD"``).
        option_type : str
            ``"call"`` or ``"put"``.
        discount_curve_id : str, default "USD-OIS"
            Discount curve identifier.
        spot_id : str, default "EQUITY-SPOT"
            Equity spot price identifier.
        vol_surface_id : str, default "EQUITY-VOL"
            Volatility surface identifier.
        div_yield_id : str | None, default "EQUITY-DIVYIELD"
            Continuous dividend yield identifier; ``None`` for no yield.

        Returns
        -------
        EquityOption
            The validated option.

        Raises
        ------
        ValueError
            If ``strike`` is not positive, ``quantity`` is zero, ``currency``
            is not a recognized code, ``option_type`` is not ``"call"`` or
            ``"put"``, or ``expiry`` cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> opt = EquityOption.european("AAPL-C-200", "AAPL", 200.0, "2025-06-20", 100.0, "USD", "call", spot_id="AAPL")
        >>> (opt.option_type, opt.settlement, opt.spot_id)
        ('call', 'cash', 'AAPL')
        """
        ...
    def implied_vol(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        target_price: float,
    ) -> float:
        """
        Recover decimal volatility with the configured exercise engine,
        dividend schedule and model day count. Trial volatility replaces the
        active market surface or override. American and Bermudan exercise use
        the configured lattice and remaining exercise dates.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount curve, spot and (optional) dividend yield.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date strictly before expiry and observed exercise.
        target_price : float
            Observed option premium: finite non-negative total trade PV in the
            option currency, including the contract multiplier.

        Returns
        -------
        float
            Annualized lognormal volatility as a decimal (``0.20`` = 20%).

        Raises
        ------
        ValueError
            If PV or ``quantity`` is invalid, the option has expired or exercised,
            or the target is indistinguishable from the deterministic price.
        KeyError
            If required market data is missing from ``market``.
        RuntimeError
            If the root search does not converge.
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
    def underlying_ticker(self) -> str:
        """
        Identifier of the underlying equity referenced by the option.

        Returns
        -------
        str
            Ticker string exactly as supplied at construction; it is the key
            used to look up the spot price and volatility surface in the
            market context, so it must match the market-data identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def strike(self) -> float:
        """
        Contractual exercise price of the option.

        Returns
        -------
        float
            Strike expressed in the same price units and currency as the
            underlying spot quote, not as a percentage of spot or as
            moneyness.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def option_type(self) -> str:
        """
        Payoff direction of the option contract.

        Returns
        -------
        str
            Serde string, either ``"call"`` (payoff ``max(S - K, 0)``) or
            ``"put"`` (payoff ``max(K - S, 0)``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_style(self) -> str:
        """
        Exercise rights attached to the option, which select the pricing
        engine used.

        Returns
        -------
        str
            Serde string: ``"european"`` (exercise only at expiry),
            ``"american"`` (any time up to expiry) or ``"bermudan"``
            (on a discrete set of scheduled exercise dates).

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
    def quantity(self) -> float:
        """
        Number of underlying units; PV and Greeks scale linearly with it.

        Returns
        -------
        float
            Underlying unit count (contract size).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def currency(self) -> str:
        """
        Currency of the strike, premium and present value.

        Returns
        -------
        str
            ISO-4217 currency code, e.g. ``"USD"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Model day count for volatility, carry and exercise times.
        Discount factors use the discount curve's own date convention.

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
    def exercise(self) -> dict[str, object] | None:
        """
        Observed exercise state (``date``, ``spot``, ``settlement_date``, ``exercised``).

        Returns
        -------
        dict[str, object] | None
            The lifecycle dict, or ``None``.

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
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spot_id(self) -> str:
        """
        Equity spot price identifier.

        Returns
        -------
        str
            The price id.

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
            The surface id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def div_yield_id(self) -> str | None:
        """
        Continuous dividend yield identifier.

        Returns
        -------
        str | None
            The id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discrete_dividends(self) -> list[tuple[datetime.date, float]]:
        """
        Discrete dividend schedule.

        Returns
        -------
        list[tuple[datetime.date, float]]
            ``(ex_date, amount)`` pairs in date order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_dates(self) -> list[datetime.date] | None:
        """
        Bermudan exercise dates.

        Returns
        -------
        list[datetime.date] | None
            The dates, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``EquityOption(id='SPX-CALL-4500', underlying_ticker='SPX', option_type='call', strike=4500.0, expiry=datetime.date(2024, 6, 21), ...)``.

        Returns
        -------
        str
            ``EquityOption(<field>=<value>, ...)``.

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

class EquityOptionBuilder:
    """
    Fluent builder for :class:`EquityOption`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``underlying_ticker``, ``strike``,
    ``option_type``, ``expiry``, ``quantity``, ``currency``, ``discount_curve_id``,
    ``spot_id``, ``vol_surface_id`` (``exercise_style`` defaults to
    ``"european"``, ``day_count`` to ACT/365F, ``settlement`` to ``"cash"``).

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityOption
    >>> option = (
    ...     EquityOption
    ...     .builder()
    ...     .id("AAPL-C-200")
    ...     .underlying_ticker("AAPL")
    ...     .strike(200.0)
    ...     .option_type("call")
    ...     .expiry("2025-06-20")
    ...     .quantity(100.0)
    ...     .currency("USD")
    ...     .discount_curve_id("USD-OIS")
    ...     .spot_id("AAPL")
    ...     .vol_surface_id("AAPL-VOL")
    ...     .build()
    ... )
    >>> option.exercise_style
    'european'
    """

    def id(self, value: str) -> EquityOptionBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the equity option.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def underlying_ticker(self, value: str) -> EquityOptionBuilder:
        """
        Set the underlying equity ticker symbol.

        Parameters
        ----------
        value : str
            Underlying equity ticker symbol.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def strike(self, value: float) -> EquityOptionBuilder:
        """
        Set the strike price.

        Parameters
        ----------
        value : float
            Strike price; must be finite and positive.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def option_type(self, value: Literal["call", "put"]) -> EquityOptionBuilder:
        """
        Set the option type.

        Parameters
        ----------
        value : Literal["call", "put"]
            Option type of the equity option.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized option type.
        """
        ...
    def exercise_style(self, value: Literal["european", "american", "bermudan"]) -> EquityOptionBuilder:
        """
        Set the exercise style.

        Parameters
        ----------
        value : Literal["european", "american", "bermudan"]
            Exercise style; defaults to ``"european"`` when never set.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized exercise style.
        """
        ...
    def expiry(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> EquityOptionBuilder:
        """
        Set the option expiry date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Option expiry date (ISO 8601 strings accepted).

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def day_count(self, value: DayCount | str) -> EquityOptionBuilder:
        """
        Set the model day count for volatility, dividend carry and exercise times.

        Parameters
        ----------
        value : DayCount | str
            Day count convention; ACT/365F when never set.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized day count.
        """
        ...
    def settlement(self, value: Literal["physical", "cash"]) -> EquityOptionBuilder:
        """
        Set the settlement method.

        Parameters
        ----------
        value : Literal["physical", "cash"]
            Physical delivery or fixed cash settlement.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized settlement method.
        """
        ...
    def exercise(
        self,
        date: datetime.date | datetime.datetime | pd.Timestamp | str,
        spot: float,
        settlement_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        exercised: bool,
    ) -> EquityOptionBuilder:
        """
        Set the observed exercise or expiry lifecycle state.

        Parameters
        ----------
        date : datetime.date | datetime.datetime | pd.Timestamp | str
            Exercise date, or expiry date for an unexercised observation.
        spot : float
            Positive observed underlying level in strike-price units.
        settlement_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Contractual cash-payment or physical-delivery date.
        exercised : bool
            Whether exercise or assignment occurred.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a date cannot be interpreted.
        """
        ...
    def quantity(self, value: float) -> EquityOptionBuilder:
        """
        Set the number of underlying units the option is written on.

        Parameters
        ----------
        value : float
            Number of underlying units; PV and Greeks scale linearly with it.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def currency(self, value: Currency | str) -> EquityOptionBuilder:
        """
        Set the currency of the strike, premium and present value.

        Parameters
        ----------
        value : Currency | str
            ISO-4217 currency code (e.g. ``"USD"``) or ``Currency``.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is
            not a recognized currency code.
        TypeError
            If ``value`` is neither a ``Currency`` nor a string.
        """
        ...
    def discount_curve_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the discount curve identifier for present value calculations.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def spot_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the equity spot price identifier.

        Parameters
        ----------
        value : str
            Equity spot price identifier.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_surface_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the equity volatility surface identifier.

        Parameters
        ----------
        value : str
            Equity volatility surface identifier.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def div_yield_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the continuous dividend yield identifier.

        Parameters
        ----------
        value : str
            Continuous dividend yield identifier; zero yield when never set.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def discrete_dividends(
        self, value: list[tuple[datetime.date | datetime.datetime | pd.Timestamp | str, float]]
    ) -> EquityOptionBuilder:
        """
        Set the discrete dividend schedule.

        Parameters
        ----------
        value : list[tuple[datetime.date | datetime.datetime | pd.Timestamp | str, float]]
            Positive ``(ex_date, dividend_amount)`` pairs in strictly increasing date order. European pricing uses escrowed spot adjustment; tree pricing restores remaining dividend value at exercise nodes.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a date cannot be interpreted.
        """
        ...
    def exercise_dates(
        self, value: list[datetime.date | datetime.datetime | pd.Timestamp | str]
    ) -> EquityOptionBuilder:
        """
        Set the exercise schedule for Bermudan options.

        Parameters
        ----------
        value : list[datetime.date | datetime.datetime | pd.Timestamp | str]
            Dates on which early exercise is permitted; required when ``exercise_style`` is ``"bermudan"``.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a date cannot be interpreted.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> EquityOptionBuilder:
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
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> EquityOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> EquityOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> EquityOptionBuilder:
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
        EquityOptionBuilder
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
        ``EquityOptionBuilder(id='X', quantity=100.0, currency="USD")``.

        Returns
        -------
        str
            ``EquityOptionBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> EquityOption:
        """
        Build the validated equity option.

        Runs only the Rust ``EquityOptionBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`EquityOption.price`.

        Returns
        -------
        EquityOption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``EquityOptionBuilder: missing required field 'id'``), or the instrument
            fails validation (non-positive strike, zero notional, unsorted dividends, inconsistent exercise state).
        """
        ...
