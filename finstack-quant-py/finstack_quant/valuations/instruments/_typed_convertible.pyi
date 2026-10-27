"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_convertible.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides

class ConvertibleBond:
    """
    Convertible bond (typed wrapper for the canonical Rust
    ``ConvertibleBond``): debt with an embedded equity conversion option,
    priced on a defaultable-equity tree with fractional recovery of live
    principal. Default extinguishes equity and pays ``recovery_rate`` times
    the recovery basis. Survival-conditioned equity drift compensates for
    jump-to-zero risk. ``credit_curve_id`` supplies issuer hazard; without it
    default is absent.

    Construct via :meth:`ConvertibleBond.builder`,
    :meth:`ConvertibleBond.example` / :meth:`ConvertibleBond.example_mandatory`
    or :meth:`ConvertibleBond.from_json`. Every public Rust field is readable
    as a property (typed :class:`ConversionSpec` / :class:`CallPutSchedule`
    where Rust has a struct); ``conversion_ratio`` / ``parity`` /
    ``conversion_premium`` / ``greeks`` mirror the Rust accessors and
    :meth:`ConvertibleBond.price` / :meth:`ConvertibleBond.metric` run the
    same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConvertibleBond
    >>> cb = ConvertibleBond.example()
    >>> (cb.id, cb.conversion_ratio, cb.spot_id)
    ('CB-TECH-5Y', 25.0, 'TECH')
    """

    @staticmethod
    def builder() -> ConvertibleBondBuilder:
        """
        Create a fluent builder (mirrors Rust ``ConvertibleBond::builder()``).

        Returns
        -------
        ConvertibleBondBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> builder = ConvertibleBond.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> ConvertibleBond:
        """
        Deserialize a validated ConvertibleBond from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"convertible_bond"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        ConvertibleBond
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> ConvertibleBond.from_json(ConvertibleBond.example().to_json()).id
        'CB-TECH-5Y'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`ConvertibleBond.from_json`.

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
            Model key (the convertible tree pricer is the native model).
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
        Compute one scalar metric for this instrument (e.g. ``"delta"`` or ``"bond_floor"``).

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
    def example() -> ConvertibleBond:
        """
        Canonical 5-year USD 1,000,000 2% semi-annual convertible (mirrors Rust
        ``ConvertibleBond::example``): ratio 25 shares per bond, voluntary
        conversion, underlying ``"TECH"``, curves ``USD-IG`` / ``USD-CREDIT-BBB``,
        issue 2024-01-15, maturity 2029-01-15.

        Returns
        -------
        ConvertibleBond
            The example bond.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> ConvertibleBond.example().credit_curve_id
        'USD-CREDIT-BBB'
        """
        ...
    @staticmethod
    def example_mandatory() -> ConvertibleBond:
        """
        Mandatory (PERCS/DECS-style) convertible example (mirrors Rust
        ``ConvertibleBond::example_mandatory``): 3-year 5% semi-annual,
        mandatory-variable conversion at maturity (upper conversion price 60,
        lower 40), 130% soft call, call at 101% after year 2 and put at 100%
        after year 1.

        Returns
        -------
        ConvertibleBond
            The example bond.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> ConvertibleBond.example_mandatory().soft_call_trigger["threshold_pct"]
        130.0
        """
        ...
    def parity(self, market: MarketContext | str) -> float:
        """
        Conversion value divided by notional (mirrors Rust
        ``ConvertibleBond::parity``). Ordinary conversion uses the effective
        ratio times spot; mandatory-variable conversion uses the contractual
        lower-price, variable-share and upper-price regimes.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the underlying equity price.

        Returns
        -------
        float
            Dimensionless conversion-value-to-notional ratio; 1.0 means par.

        Raises
        ------
        KeyError
            If the ``spot_id`` price is missing from ``market``.
        ValueError
            If conversion terms are invalid or the equity price is negative
            or non-finite.
        """
        ...
    def conversion_premium(self, market: MarketContext | str, bond_price: float) -> float:
        """
        Conversion premium over parity (mirrors Rust
        ``ConvertibleBond::conversion_premium``):
        ``bond_price / conversion_value - 1``. Conversion value follows the
        policy, including mandatory-variable share delivery.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the underlying equity price.
        bond_price : float
            Observed bond price in notional currency units per bond.

        Returns
        -------
        float
            Conversion premium as a decimal fraction (``0.15`` = 15%).

        Raises
        ------
        KeyError
            If the ``spot_id`` price is missing from ``market``.
        ValueError
            If conversion value is nonpositive or ``bond_price`` is negative
            or non-finite.
        """
        ...
    def greeks(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> dict[str, float]:
        """
        Tree Greeks of the convertible (mirrors Rust ``ConvertibleBond::greeks``
        with the default tree).

        Bump sizes come from the bond's ``metric_pricing_overrides.bump_config``
        (``spot_bump_decimal``, ``vol_bump_decimal``, ``rate_bump_bp``), defaulting to
        1% of spot, 1 vol point and 1bp.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the curves, equity price and volatility. An active
            instrument volatility override takes precedence; otherwise surface
            volatility is sampled at the contractual conversion strike. Floating
            coupons require their forward curve and realized historical fixings.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        dict[str, float]
            ``price``, ``delta``, ``gamma``, ``vega``, ``theta``, ``rho``.

        Raises
        ------
        KeyError
            If required market data is missing.
        RuntimeError
            If the tree pricer fails.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Principal amount.

        Returns
        -------
        Money
            Currency-tagged principal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def issue_date(self) -> datetime.date:
        """
        Dated date from which the bond starts accruing interest.

        Returns
        -------
        datetime.date
            Calendar date, unadjusted for business days. It anchors the
            coupon schedule and the first accrual period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Scheduled redemption date on which principal is repaid.

        Returns
        -------
        datetime.date
            Unadjusted calendar maturity; payment dates derived from it are
            rolled by the instrument's business-day convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier for the debt component.

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
    def credit_curve_id(self) -> str | None:
        """
        Issuer hazard curve identifier.

        Returns
        -------
        str | None
            The hazard curve id, or ``None`` (the cash component discounts at the risk-free curve).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def conversion(self) -> ConversionSpec:
        """
        Conversion terms.

        Returns
        -------
        ConversionSpec
            The typed conversion spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def conversion_ratio(self) -> float | None:
        """
        Base conversion ratio (shares per bond), derived from ratio or price (mirrors Rust ``conversion_ratio``).

        Returns
        -------
        float | None
            The ratio, or ``None`` when neither ratio nor price is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def effective_conversion_ratio(self) -> float | None:
        """
        Conversion ratio after anti-dilution adjustments (mirrors Rust ``effective_conversion_ratio``).

        Returns
        -------
        float | None
            The adjusted ratio, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spot_id(self) -> str:
        """
        Market-scalar id of the underlying share price.

        Returns
        -------
        str
            The ``MarketContext`` price id read for the equity spot.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        Equity volatility id: a volatility surface, or a unitless scalar
        holding a flat volatility.

        Returns
        -------
        str
            The volatility id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def div_yield_id(self) -> str | None:
        """
        Unitless continuous dividend-yield scalar id.

        Returns
        -------
        str | None
            The id, or ``None`` for a zero dividend yield.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def call_put(self) -> CallPutSchedule | None:
        """
        Call/put schedule.

        Returns
        -------
        CallPutSchedule | None
            The typed schedule, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def soft_call_trigger(self) -> dict[str, object] | None:
        """
        Soft-call trigger (``threshold_pct``, ``observation_days``, ``required_days_above``).

        Returns
        -------
        dict[str, object] | None
            The trigger dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int | None:
        """
        Settlement lag in business days.

        Returns
        -------
        int | None
            The lag, or ``None`` for same-day.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float | None:
        """
        Assumed recovery rate on default as a fraction.

        Returns
        -------
        float | None
            The recovery, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def resolved_settlement(self) -> dict[str, object] | None:
        """Recorded exercise and delayed delivery replacing debt and coupons.

        Returns
        -------
        dict[str, object] | None
            ISO exercise/delivery dates, shares, currency-tagged cash and recovery
            basis; ``None`` while the debt remains unresolved.

        Notes
        -----
        This accessor does not raise; it returns the stored claim without repricing.
        """
        ...
    @property
    def cashflow_spec(self) -> dict[str, object]:
        """
        Coupon/cashflow specification in serde form, the same shape as
        ``Bond.cashflow_spec``.

        Returns
        -------
        dict[str, object]
            ``{"fixed": {...}}``, ``{"floating": {...}}``, ``{"step_up": {...}}``
            or ``{"amortizing": {...}}``; a zero-coupon convertible is a fixed
            spec with rate ``"0"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Maturity as seen by the pricer.

        Returns
        -------
        datetime.date | None
            The maturity, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``ConvertibleBond(id='CB-TECH-5Y', notional=Money(1000000.0, 'USD'), issue_date=datetime.date(2024, 1, 15), ...)``.

        Returns
        -------
        str
            ``ConvertibleBond(<field>=<value>, ...)``.

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

class ConvertibleBondBuilder:
    """
    Fluent builder for :class:`ConvertibleBond`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``issue_date``,
    ``maturity``, ``discount_curve_id``, ``conversion``, ``cashflow_spec``.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import ConversionSpec, ConvertibleBond
    >>> bond = (
    ...     ConvertibleBond
    ...     .builder()
    ...     .id("CONV-1")
    ...     .notional(Money(1_000.0, Currency("USD")))
    ...     .issue_date("2024-01-15")
    ...     .maturity("2029-01-15")
    ...     .discount_curve_id("USD-OIS")
    ...     .conversion(
    ...         ConversionSpec(ratio=20.0, policy="voluntary", anti_dilution="full_ratchet", dividend_adjustment="none")
    ...     )
    ...     .spot_id("ACME")
    ...     .vol_surface_id("ACME-VOL")
    ...     .cashflow_spec(ConvertibleBond.example().cashflow_spec)
    ...     .build()
    ... )
    >>> bond.conversion_ratio
    20.0
    """

    def id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the convertible bond.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> ConvertibleBondBuilder:
        """
        Set the principal amount.

        Parameters
        ----------
        value : Money
            Principal amount.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> ConvertibleBondBuilder:
        """
        Set the issue date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date (ISO 8601 strings accepted).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> ConvertibleBondBuilder:
        """
        Set the maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def discount_curve_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the discount curve identifier for the debt component.

        Parameters
        ----------
        value : str
            Discount curve identifier (risk-free or funding).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_curve_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the issuer hazard curve identifier.

        Parameters
        ----------
        value : str
            ``HazardCurve`` identifier; the zero-recovery risky discount factor is the risk-free discount factor times its
            survival probability. If never set, the cash component discounts at the risk-free curve (no credit spread).
            Requires an explicit ``recovery_rate``.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def conversion(self, value: ConversionSpec | dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the conversion terms.

        Parameters
        ----------
        value : ConversionSpec | dict[str, object] | str
            Typed :class:`ConversionSpec`, a dict, or a JSON object string with ``ratio``, ``price``, ``policy``, ``anti_dilution``, ``dividend_adjustment`` and ``dilution_events``; at least one of ``ratio`` / ``price`` must be set.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``ConversionSpec`` shape.
        """
        ...
    def spot_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the market-scalar id of the underlying share price (required).

        Parameters
        ----------
        value : str
            ``MarketContext`` price id holding the share price in the bond's
            currency, or a unitless level.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_surface_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the equity volatility id (required).

        Parameters
        ----------
        value : str
            Volatility surface id read at the conversion strike and maturity,
            or the id of a unitless scalar holding a flat volatility.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def div_yield_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the continuous dividend-yield scalar id.

        Parameters
        ----------
        value : str
            Id of a unitless decimal dividend yield (``0.02`` = 2%). When never
            set the dividend yield is zero; a set id must exist in the market.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def call_put(self, value: CallPutSchedule | dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the call/put schedule.

        Parameters
        ----------
        value : CallPutSchedule | dict[str, object] | str
            Typed :class:`CallPutSchedule`, a dict, or a JSON object string with ``calls`` and ``puts`` arrays of windows.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``CallPutSchedule`` shape.
        """
        ...
    def soft_call_trigger(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the soft-call trigger condition.

        Parameters
        ----------
        value : dict[str, object] | str
            ``PriceTrigger`` as a dict or JSON object string with ``threshold_pct`` (percent of conversion price, e.g. ``130.0``), ``observation_days`` and ``required_days_above``.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``PriceTrigger`` shape.
        """
        ...
    def settlement_days(self, value: int) -> ConvertibleBondBuilder:
        """
        Set the settlement lag.

        Parameters
        ----------
        value : int
            Business days from trade date to settlement (e.g. ``2`` for US corporate convertibles); same-day when never set.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def recovery_rate(self, value: float) -> ConvertibleBondBuilder:
        """
        Set the assumed recovery rate on default.

        Parameters
        ----------
        value : float
            Recovery rate as a fraction (``0.40`` = 40%); only relevant when ``credit_curve_id`` is set.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def resolved_settlement(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
        """Record the claim remaining after exercise extinguishes the debt.

        Parameters
        ----------
        value : dict[str, object] | str
            Canonical ConvertibleSettlement dict or JSON: ISO ``exercise_date``
            and ``delivery_date``, nonnegative ``shares``, currency-tagged
            ``cash`` and ``recovery_basis``. Cash includes retained coupons;
            recovery basis excludes uncapitalized accrual. Omission leaves debt
            unresolved. Rust ``build()`` validates dates, amounts and currencies.

        Returns
        -------
        ConvertibleBondBuilder
            This builder for chaining.

        Raises
        ------
        ValueError
            If the builder was consumed or the record cannot be deserialized.
        """
        ...
    def cashflow_spec(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the coupon/cashflow specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CashflowSpec`` as a dict or JSON object string (the same
            shape as ``Bond.cashflow_spec``, e.g. ``{"fixed": {"coupon_type":
            "cash", "rate": "0.05", ...schedule}}``); a zero-coupon convertible
            uses a fixed spec with rate ``"0"``.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CashflowSpec``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
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
        ConvertibleBondBuilder
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
    ) -> ConvertibleBondBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> ConvertibleBondBuilder:
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
        ConvertibleBondBuilder
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
        ``ConvertibleBondBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``ConvertibleBondBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> ConvertibleBond:
        """
        Build the validated convertible bond.

        Runs only the Rust ``ConvertibleBondBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`ConvertibleBond.price`.

        Returns
        -------
        ConvertibleBond
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``ConvertibleBondBuilder: missing required field 'id'``), or the instrument
            fails validation (conversion terms set neither ``ratio`` nor ``price``, maturity not after issue).
        """
        ...

class ConversionSpec:
    """
    Conversion terms of a convertible bond (typed wrapper for the Rust
    ``ConversionSpec``). At least one of ``ratio`` (shares per bond) and
    ``price`` (conversion price per share) must be given; when both are,
    they must agree with ``notional / price``. Accepted by
    :meth:`ConvertibleBondBuilder.conversion` alongside dicts / JSON of the
    same shape; picklable.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConversionSpec
    >>> spec = ConversionSpec(ratio=25.0, policy="voluntary", anti_dilution="none", dividend_adjustment="none")
    >>> (spec.ratio, spec.policy, spec.anti_dilution)
    (25.0, 'voluntary', 'none')
    """

    def __init__(
        self,
        ratio: float | None = None,
        price: float | None = None,
        *,
        policy: str | dict[str, object],
        anti_dilution: Literal["none", "full_ratchet", "weighted_average"],
        dividend_adjustment: Literal["none", "adjust_price", "adjust_ratio"],
        dilution_events: list[dict[str, object]] | str | None = None,
    ) -> None:
        """
        Describe the conversion terms.

        Parameters
        ----------
        ratio : float | None
            Conversion ratio (shares per bond); derived from ``price`` when ``None``.
        price : float | None
            Conversion price per share; derived from ``ratio`` when ``None``.
        policy : str | dict[str, object]
            Conversion policy (keyword-only, required as in Rust): the bare label
            ``"voluntary"`` (not JSON text),
            or a tagged dict such as ``{"mandatory_on": "2027-03-15"}``,
            ``{"window": {"start": "2025-01-15", "end": "2028-01-15"}}``,
            ``{"upon_event": "qualified_ipo"}`` or ``{"mandatory_variable":
            {"conversion_date": ..., "upper_conversion_price": ...,
            "lower_conversion_price": ...}}`` (dates as ISO strings).
        anti_dilution : {"none", "full_ratchet", "weighted_average"}
            Anti-dilution protection (keyword-only, required as in Rust).
        dividend_adjustment : {"none", "adjust_price", "adjust_ratio"}
            Dividend protection (keyword-only, required as in Rust).
        dilution_events : list[dict[str, object]] | str | None
            Dilution events (``date``, ``new_issue_price``, ``new_shares_issued``,
            ``shares_outstanding_before``) in chronological order; pricing raises
            ``ValueError`` for out-of-order events. A string must be a JSON array. Default empty.

        Raises
        ------
        ValueError
            If ``policy`` / ``anti_dilution`` / ``dividend_adjustment`` are not
            recognized, a label is JSON-encoded, or a dilution event does not match the schema.
        TypeError
            If ``policy`` is neither a bare label string nor a tagged dict.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConversionSpec
        >>> spec = ConversionSpec(
        ...     price=50.0, policy="voluntary", anti_dilution="none", dividend_adjustment="adjust_ratio"
        ... )
        >>> spec.dividend_adjustment
        'adjust_ratio'
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> ConversionSpec:
        """
        Deserialize from the canonical JSON shape.

        Parameters
        ----------
        json : str
            JSON object with ``ratio``, ``price``, ``policy``, ``anti_dilution``,
            ``dividend_adjustment`` and optional ``dilution_events``.

        Returns
        -------
        ConversionSpec
            The parsed terms.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConversionSpec
        >>> ConversionSpec.from_json(
        ...     ConversionSpec(
        ...         ratio=20.0, policy="voluntary", anti_dilution="none", dividend_adjustment="none"
        ...     ).to_json()
        ... ).ratio
        20.0
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical JSON shape.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json` and :meth:`ConvertibleBondBuilder.conversion`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def ratio(self) -> float | None:
        """
        Conversion ratio (shares per bond).

        Returns
        -------
        float | None
            The explicit ratio, or ``None`` when derived from ``price``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def price(self) -> float | None:
        """
        Conversion price per share.

        Returns
        -------
        float | None
            The explicit price, or ``None`` when derived from ``ratio``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def policy(self) -> str | dict[str, object]:
        """
        Conversion policy in serde form.

        Returns
        -------
        str | dict[str, object]
            ``"voluntary"`` or the tagged dict form.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def anti_dilution(self) -> str:
        """
        Anti-dilution policy (serde name).

        Returns
        -------
        str
            ``"none"``, ``"full_ratchet"`` or ``"weighted_average"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def dividend_adjustment(self) -> str:
        """
        Dividend adjustment policy (serde name).

        Returns
        -------
        str
            ``"none"``, ``"adjust_price"`` or ``"adjust_ratio"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def dilution_events(self) -> list[dict[str, object]]:
        """
        Dilution events.

        Returns
        -------
        list[dict[str, object]]
            Serde dicts, in order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``ConversionSpec(ratio=25.0, price=None, policy='voluntary', anti_dilution='none', dividend_adjustment='none', dilution_events=<0>)``.

        Returns
        -------
        str
            ``ConversionSpec(<field>=<value>, ...)``.

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
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class CallPutSchedule:
    """
    Issuer call and holder put windows (typed wrapper for the Rust
    ``CallPutSchedule``). Each window is a dict ``{"start", "end",
    "price_pct_of_par", "make_whole"?}`` with dates as ISO strings and prices
    in percent of par (``101.0`` = 101%). Accepted by
    :meth:`ConvertibleBondBuilder.call_put` alongside dicts / JSON of the
    same shape; picklable.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CallPutSchedule
    >>> sched = CallPutSchedule(calls=[{"start": "2026-03-15", "end": "2027-03-15", "price_pct_of_par": 101.0}])
    >>> (len(sched.calls), len(sched.puts))
    (1, 0)
    """

    def __init__(
        self,
        calls: list[dict[str, object]] | str | None = None,
        puts: list[dict[str, object]] | str | None = None,
    ) -> None:
        """
        Describe the call and put windows.

        Parameters
        ----------
        calls : list[dict[str, object]] | str | None
            Issuer call windows (``start``, ``end``,
            ``price_pct_of_par``, optional ``make_whole``); default none.
        puts : list[dict[str, object]] | str | None
            Holder put windows of the same shape; default none.

        Raises
        ------
        ValueError
            If a window does not match the schema.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CallPutSchedule
        >>> CallPutSchedule().calls
        []
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CallPutSchedule:
        """
        Deserialize from the canonical JSON shape (``{"calls": [...], "puts": [...]}``).

        Parameters
        ----------
        json : str
            JSON object with ``calls`` and ``puts`` arrays.

        Returns
        -------
        CallPutSchedule
            The parsed schedule.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CallPutSchedule
        >>> CallPutSchedule.from_json('{"calls": [], "puts": []}').puts
        []
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical JSON shape.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json` and :meth:`ConvertibleBondBuilder.call_put`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def calls(self) -> list[dict[str, object]]:
        """
        Issuer call windows.

        Returns
        -------
        list[dict[str, object]]
            Serde dicts, in order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def puts(self) -> list[dict[str, object]]:
        """
        Holder put windows.

        Returns
        -------
        list[dict[str, object]]
            Serde dicts, in order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CallPutSchedule(calls=<1>, puts=<0>)``.

        Returns
        -------
        str
            ``CallPutSchedule(<field>=<value>, ...)``.

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
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
