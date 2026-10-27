"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/instruments.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._merton_mc import MertonMcConfig, MertonMcResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides

class Bond:
    """
    Typed wrapper for the canonical Rust ``Bond`` instrument.

    Construct via :meth:`Bond.fixed` (US corporate), :meth:`Bond.with_convention`
    (a named preset; override its stub with :meth:`Bond.with_stub`), :meth:`Bond.floating` /
    :meth:`Bond.floating_with_convention`, :meth:`Bond.zero_coupon`, the
    :meth:`Bond.builder` fluent builder (callable schedules, credit curve,
    custom cashflow specs and settlement conventions), the ``Bond.example*``
    presets or :meth:`Bond.from_json`. Every public Rust field is readable
    as a property; :meth:`Bond.price` / :meth:`Bond.metric` run the same
    pricer as :func:`price_instrument`. Instances are accepted directly by
    :func:`price_instrument` and :func:`instrument_cashflows`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Bond
    >>> bond = Bond.fixed(
    ...     "BOND-1",
    ...     1_000_000.0,
    ...     0.05,
    ...     "2024-01-01",
    ...     "2034-01-01",
    ...     "none",
    ...     "USD-OIS",
    ...     currency="USD",
    ... )
    >>> bond.id
    'BOND-1'
    >>> bond.notional.amount
    1000000.0
    """

    @staticmethod
    def builder() -> BondBuilder:
        """
        Create a fluent builder (mirrors Rust ``Bond::builder()``).

        Returns
        -------
        BondBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> builder = Bond.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def fixed(
        id: str,
        notional: Money | float,
        coupon_rate: float | Rate,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"],
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a US corporate fixed-rate bond (semi-annual, 30/360, T+1).

        Mirrors Rust ``Bond::fixed``. For another market's conventions use
        :meth:`Bond.with_convention` and, to override the preset's stub rule,
        chain :meth:`Bond.with_stub`.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        coupon_rate : float | Rate
            Annual coupon as a decimal (``0.05`` = 5%) or a ``Rate``.
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date (ISO 8601 strings accepted).
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        stub : StubKind | str
            Placement and length policy for an irregular coupon period, as a
            ``StubKind`` or its serde name (``"none"``, ``"short_front"``, ...).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated fixed-rate bond.

        Raises
        ------
        ValueError
            If ``stub`` is not a recognized name, a bare ``notional`` has no
            ``currency``, or validation fails (e.g. maturity not after
            issue_date).
        TypeError
            If ``coupon_rate`` or ``notional`` has an unsupported type or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> bond = Bond.fixed(
        ...     "BOND-1", 1_000_000.0, 0.05, "2024-01-01", "2034-01-01", "none", "USD-OIS", currency="USD"
        ... )
        >>> bond.settlement_days
        1
        """
        ...
    def with_stub(
        self, stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
    ) -> Bond:
        """
        Return a copy of this bond with a different coupon-schedule stub rule.

        Mirrors Rust ``Bond::with_stub``; the receiver is not modified.

        Parameters
        ----------
        stub : StubKind | str
            Placement and length policy for an irregular coupon period, as a
            ``StubKind`` or its serde name (``"none"``, ``"short_front"``, ...).

        Returns
        -------
        Bond
            A new bond whose coupon schedule uses ``stub``.

        Raises
        ------
        ValueError
            If ``stub`` is not a recognized stub name.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> bund = Bond.with_convention(
        ...     "BUND", 1_000_000.0, 0.025, "2024-01-15", "2034-01-15", "german_bund", "EUR-OIS", currency="EUR"
        ... ).with_stub("long_back")
        >>> bund.to_dict()["cashflow_spec"]["fixed"]["stub"]
        'long_back'
        """
        ...
    @staticmethod
    def with_convention(
        id: str,
        notional: Money | float,
        coupon_rate: float | Rate,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        convention: Literal[
            "us_treasury", "us_agency", "german_bund", "uk_gilt", "french_oat", "jgb", "us_corporate", "eur_corporate"
        ],
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a fixed-rate bond from a named market convention.

        Mirrors Rust ``Bond::with_convention``; the stub rule is the preset's own
        (chain :meth:`Bond.with_stub` to override it).

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        coupon_rate : float | Rate
            Annual coupon as a decimal (``0.05`` = 5%) or a ``Rate``.
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        convention : str
            Bond convention preset (``"us_treasury"``, ``"us_agency"``,
            ``"german_bund"``, ``"uk_gilt"``, ``"french_oat"``, ``"jgb"``,
            ``"us_corporate"``, ``"eur_corporate"``).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated fixed-rate bond.

        Raises
        ------
        ValueError
            If ``convention`` is unknown, a bare ``notional`` has no ``currency``,
            or validation fails.
        TypeError
            If ``coupon_rate``/``notional`` has an unsupported type or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> gilt = Bond.with_convention(
        ...     "GILT",
        ...     1_000_000.0,
        ...     0.04,
        ...     "2024-01-01",
        ...     "2034-01-01",
        ...     "uk_gilt",
        ...     "GBP-OIS",
        ...     currency="GBP",
        ... )
        >>> gilt.settlement_days
        1
        """
        ...
    @staticmethod
    def floating(
        id: str,
        notional: Money | float,
        forward_curve_id: str,
        spread_bp: float | Bps,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        frequency: Tenor,
        day_count: DayCount,
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a floating-rate bond (FRN) linked to a forward index.

        Mirrors Rust ``Bond::floating``. Settlement, calendar, and business-day
        convention come from the notional currency: USD ``us_corporate`` (T+1,
        ``usny``), EUR ``eur_corporate`` (T+2, ``target2``), GBP ``uk_gilt``
        (T+1), JPY ``jgb`` (T+2). Other currencies raise ``ValueError``; use
        :meth:`Bond.floating_with_convention` to name the preset explicitly.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        forward_curve_id : str
            Forward curve identifier (e.g. ``"USD-SOFR-3M"``).
        spread_bp : float | Bps
            Spread over the index in whole basis points; fractional values raise
            ``ValueError`` (use :meth:`Bond.from_json` for sub-bp margins).
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        frequency : Tenor
            Payment frequency (e.g. ``Tenor.quarterly()``).
        day_count : DayCount
            Day count convention (e.g. ``DayCount.ACT_360``).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated floating-rate note.

        Raises
        ------
        ValueError
            If the notional currency has no mapped settlement convention,
            ``notional`` is not finite and positive, ``spread_bp`` is not a finite
            whole number of basis points, or ``issue_date`` is not strictly before
            ``maturity``.
        TypeError
            If ``spread_bp``/``notional`` has an unsupported type or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import Bond
        >>> frn = Bond.floating(
        ...     "FRN",
        ...     1000.0,
        ...     "USD-SOFR-3M",
        ...     125.0,
        ...     "2024-01-01",
        ...     "2029-01-01",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     "USD-OIS",
        ...     currency="USD",
        ... )
        >>> frn.has_floating_coupons
        True
        """
        ...
    @staticmethod
    def floating_with_convention(
        id: str,
        notional: Money | float,
        forward_curve_id: str,
        spread_bp: float | Bps,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        frequency: Tenor,
        day_count: DayCount,
        convention: Literal[
            "us_treasury", "us_agency", "german_bund", "uk_gilt", "french_oat", "jgb", "us_corporate", "eur_corporate"
        ],
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a floating-rate bond with an explicit convention preset.

        Mirrors Rust ``Bond::floating_with_convention``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        forward_curve_id : str
            Forward curve identifier (e.g. ``"EUR-EURIBOR-3M"``).
        spread_bp : float | Bps
            Spread over the index in whole basis points; fractional values raise
            ``ValueError`` (use :meth:`Bond.from_json` for sub-bp margins).
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention.
        convention : str
            Bond convention preset (see :meth:`Bond.with_convention`).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated floating-rate note.

        Raises
        ------
        ValueError
            If ``convention`` is unknown, a bare ``notional`` has no ``currency``,
            ``spread_bp`` is not a finite whole number of basis points, or
            validation fails.
        TypeError
            If ``spread_bp``/``notional`` has an unsupported type or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import Bond
        >>> frn = Bond.floating_with_convention(
        ...     "FRN-EUR",
        ...     1000.0,
        ...     "EUR-EURIBOR-3M",
        ...     80.0,
        ...     "2024-01-01",
        ...     "2029-01-01",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     "eur_corporate",
        ...     "EUR-OIS",
        ...     currency="EUR",
        ... )
        >>> frn.settlement_days
        2
        """
        ...
    @staticmethod
    def zero_coupon(
        id: str,
        notional: Money | float,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a zero-coupon bond (single principal redemption at maturity).

        Mirrors Rust ``Bond::zero_coupon``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Redemption amount; a bare number is tagged with ``currency``.
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity (redemption) date.
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated zero-coupon bond.

        Raises
        ------
        ValueError
            If a bare ``notional`` has no ``currency`` or ``maturity`` is not after
            ``issue_date``.
        TypeError
            If ``notional`` has an unsupported type or a date cannot be
            interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> zc = Bond.zero_coupon("ZC", 1_000_000.0, "2024-01-01", "2029-01-01", "USD-OIS", currency="USD")
        >>> zc.has_floating_coupons
        False
        """
        ...
    @staticmethod
    def example() -> Bond:
        """
        Canonical example: 5-year USD 5% semi-annual fixed-rate bond discounted on ``USD-OIS`` (mirrors Rust ``Bond::example``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example().discount_curve_id
        'USD-TREASURY'
        """
        ...
    @staticmethod
    def example_floating() -> Bond:
        """
        Canonical example: USD SOFR-linked floating-rate note (mirrors Rust ``Bond::example_floating``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_floating().has_floating_coupons
        True
        """
        ...
    @staticmethod
    def example_callable() -> Bond:
        """
        Canonical example: fixed-rate bond carrying a call schedule (mirrors Rust ``Bond::example_callable``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_callable().call_put is not None
        True
        """
        ...
    @staticmethod
    def example_amortizing() -> Bond:
        """
        Canonical example: fixed-rate bond with a principal amortization schedule (mirrors Rust ``Bond::example_amortizing``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_amortizing().cashflow_spec.keys() >= {"amortizing"}
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> Bond:
        """
        Deserialize a validated Bond from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"bond"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Bond
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.from_json(Bond.example().to_json()).id == Bond.example().id
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
            and :meth:`Bond.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the bond spec as a plain Python ``dict``.

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
        Price this bond and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hazard_rate"``, ``"tree"``,
            ``"rates_credit"``, ...). For bonds, ``"discounting"`` and
            ``"hazard_rate"`` are non-callable rates-only and fractional-
            recovery-of-par models;
            ``"tree"`` values rates-only rights; ``"rates_credit"`` values
            call, put, and return-floor rights jointly with credit risk.
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
            Stochastic ``"rates_credit"`` pricing includes Monte Carlo
            convergence and reproducibility diagnostics in ``details``.

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
        Compute one scalar metric for this bond (e.g. ``"ytm"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            ``"default"`` uses the bond-native selection. Explicit keys are
            ``"discounting"`` for non-callable rates-only PV,
            ``"hazard_rate"`` for non-callable fractional recovery of par,
            ``"tree"`` for rates-only exercise rights, and ``"rates_credit"``
            for joint rates-credit valuation including call, put, and
            return-floor rights.

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
    def min_moic(self, multiple: float) -> Bond:
        """
        Return a copy with a minimum MOIC return floor on early redemption
        (mirrors Rust ``Bond::min_moic``).

        Parameters
        ----------
        multiple : float
            Minimum multiple of invested capital (e.g. ``1.25``).

        Returns
        -------
        Bond
            A new bond with ``return_floor`` set; ``self`` is unchanged.

        Notes
        -----
        This method does not raise; the floor is validated at pricing time.
        """
        ...
    def min_xirr(self, rate: float | Rate) -> Bond:
        """
        Return a copy with a minimum XIRR return floor on early redemption
        (mirrors Rust ``Bond::min_xirr``).

        Parameters
        ----------
        rate : float | Rate
            Target annualized IRR as a decimal (``0.12`` = 12%) or a ``Rate``.

        Returns
        -------
        Bond
            A new bond with ``return_floor`` set; ``self`` is unchanged.

        Raises
        ------
        TypeError
            If ``rate`` is neither a number nor a ``Rate``.
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
        Principal amount of the bond.

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
        Issue date of the bond.

        Returns
        -------
        datetime.date
            The contractual issue date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Maturity (final redemption) date.

        Returns
        -------
        datetime.date
            The contractual maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def cashflow_spec(self) -> dict[str, object]:
        """
        Coupon/cashflow specification in serde form.

        Returns
        -------
        dict[str, object]
            One-key dict: ``{"fixed": {...}}``, ``{"floating": {...}}``, ``{"step_up": {...}}`` or ``{"amortizing": {...}}``.

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
    def credit_curve_id(self) -> str | None:
        """
        Hazard curve identifier for ``hazard_rate`` and ``rates_credit`` pricing.

        Returns
        -------
        str | None
            Curve id, or ``None`` when no credit-consuming model is configured.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def repo_curve_id(self) -> str | None:
        """
        Repo (financing) discount curve identifier.

        Returns
        -------
        str | None
            Curve id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def call_put(self) -> dict[str, object] | None:
        """
        Call/put schedule in serde form.

        Returns
        -------
        dict[str, object] | None
            ``{"calls": [...], "puts": [...]}`` or ``None`` for a bullet bond.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def return_floor(self) -> dict[str, object] | None:
        """
        Return-floor specification (minimum MOIC / XIRR) in serde form.

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
    def custom_cashflows(self) -> dict[str, object] | None:
        """
        Explicit cashflow schedule overriding generated coupons, in serde form.

        Returns
        -------
        dict[str, object] | None
            The schedule dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def accrual_method(self) -> str:
        """
        Accrual method (serde string).

        Returns
        -------
        str
            ``"linear"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_convention(self) -> dict[str, object] | None:
        """
        Settlement convention (settlement lag, ex-coupon period) in serde form.

        Returns
        -------
        dict[str, object] | None
            ``{"settlement_days": ..., "ex_coupon_days": ..., "ex_coupon_calendar_id": ...}`` or ``None``.

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
            The lag, or ``None`` when no settlement convention is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def ex_coupon_days(self) -> int | None:
        """
        Ex-coupon period in business days.

        Returns
        -------
        int | None
            The period, or ``None`` when no settlement convention is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def has_floating_coupons(self) -> bool:
        """
        Whether coupons depend on forward-curve projection (FRNs).

        Returns
        -------
        bool
            ``True`` for floating (or amortizing-floating) cashflow specs.

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
    def price_merton_mc(
        self,
        config: MertonMcConfig,
        discount_rate: float,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> MertonMcResult:
        """
        Price this bond with the Merton Monte Carlo structural credit engine.

        Uses geometric Brownian motion asset dynamics only. Floating-rate and
        amortizing cashflow specs raise ``ValueError``. When the config's PIK
        schedule is the default uniform cash mode, the bond's ``CouponType``
        overrides the schedule; otherwise the config schedule takes precedence.
        The independent-estimator count, antithetic flag and seed label come
        from this bond's ``instrument_pricing_overrides.model_config``
        (``mc_paths``, ``mc_antithetic``, ``mc_seed_scenario``), defaulting to
        5,000 antithetic estimators seeded from the bond id.

        Parameters
        ----------
        config : MertonMcConfig
            Structural model, PIK schedule, recovery and time grid.
        discount_rate : float
            Flat continuously compounded risk-free rate as a decimal used to
            discount simulated cashflows (unless term-structure discount factors
            are set on the config).
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        MertonMcResult
            Monte Carlo pricing result with clean/dirty prices and path stats.

        Raises
        ------
        ValueError
            If ``as_of`` is invalid, the bond has floating or amortizing
            cashflows, or simulation parameters fail validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond, MertonMcConfig, PikMode, PikSchedule
        >>> from finstack_quant.models.credit import MertonModel
        >>> bond = Bond.fixed(
        ...     "BOND-MC",
        ...     1_000_000.0,
        ...     0.08,
        ...     "2024-01-01",
        ...     "2029-01-01",
        ...     "none",
        ...     "USD-OIS",
        ...     currency="USD",
        ... )
        >>> merton = MertonModel(100.0, 0.25, 80.0, 0.04)
        >>> config = MertonMcConfig(merton, 0.40).pik_schedule(PikSchedule.uniform(PikMode.pik()))
        >>> bond.price_merton_mc(config, 0.04, "2024-01-01").num_paths
        5000
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

class BondBuilder:
    """
    Fluent builder for :class:`Bond`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``issue_date``, ``maturity``,
    ``cashflow_spec``, ``discount_curve_id``. Nested specs accept a ``dict`` or JSON ``str``
    in the Rust serde shape; ``Bond.example().to_dict()`` shows the exact
    field names.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Bond
    >>> base = Bond.example().to_dict()
    >>> callable_bond = (
    ...     Bond
    ...     .builder()
    ...     .id("CALLABLE")
    ...     .notional(1_000_000.0, currency="USD")
    ...     .issue_date("2024-01-15")
    ...     .maturity("2034-01-15")
    ...     .cashflow_spec(base["cashflow_spec"])
    ...     .discount_curve_id("USD-OIS")
    ...     .credit_curve_id("ACME-HZD")
    ...     .call_put({
    ...         "calls": [{"start": "2029-01-15", "end": "2034-01-15", "price_pct_of_par": 100.0}],
    ...         "puts": [],
    ...     })
    ...     .build()
    ... )
    >>> callable_bond.credit_curve_id
    'ACME-HZD'
    """

    def id(self, value: str) -> BondBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money | float, currency: str | None = None) -> BondBuilder:
        """
        Set the principal amount.

        Parameters
        ----------
        value : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> BondBuilder:
        """
        Set the required contractual issue date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Required contractual issue date (ISO 8601 strings accepted). Omission raises ValueError on build().

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> BondBuilder:
        """
        Set the maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def cashflow_spec(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the coupon/cashflow specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CashflowSpec`` in serde form (``dict`` or JSON string), e.g. ``{"fixed": {"coupon_type": "cash", "rate": "0.05", "schedule": {...}}}`` (copy ``Bond.example().cashflow_spec``).

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CashflowSpec``.
        """
        ...
    def discount_curve_id(self, value: str) -> BondBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_curve_id(self, value: str) -> BondBuilder:
        """
        Set the hazard curve identifier for ``hazard_rate`` and ``rates_credit`` pricing.

        Parameters
        ----------
        value : str
            Hazard curve identifier for scalar or joint rates-credit pricing.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def repo_curve_id(self, value: str) -> BondBuilder:
        """
        Set the repo (financing) discount curve identifier.

        Parameters
        ----------
        value : str
            Repo (financing) discount curve identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def call_put(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the call/put schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CallPutSchedule`` in serde form (``dict`` or JSON string), e.g. ``{"calls": [{"start": "2027-01-15", "end": "2029-01-15", "price_pct_of_par": 100.0}], "puts": []}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CallPutSchedule``.
        """
        ...
    def return_floor(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the return-floor specification (minimum MOIC / XIRR on early redemption).

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ReturnFloorSpec`` in serde form (``dict`` or JSON string), e.g. ``Bond.example().min_moic(1.25).return_floor``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``ReturnFloorSpec``.
        """
        ...
    def custom_cashflows(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the explicit cashflow schedule that overrides generated coupons.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CashFlowSchedule`` in serde form (``dict`` or JSON string), e.g. the ``custom_cashflows`` value of a bond built from cashflows.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CashFlowSchedule``.
        """
        ...
    def accrual_method(self, value: str) -> BondBuilder:
        """
        Set the accrual method.

        Parameters
        ----------
        value : str
            Accrual method (serde string). ``"linear"`` is the default.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def settlement_convention(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the settlement convention (settlement lag and ex-coupon period).

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``BondSettlementConvention`` in serde form (``dict`` or JSON string), e.g. ``{"settlement_days": 2, "ex_coupon_days": 0, "ex_coupon_calendar_id": None}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``BondSettlementConvention``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> BondBuilder:
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
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> BondBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> BondBuilder:
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
        BondBuilder
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
        ``BondBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``BondBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> Bond:
        """
        Build the validated bond.

        Runs the same validation as the Rust ``BondBuilder::build`` (structural
        invariants only); pricing-time checks run in ``Bond.price``.

        Returns
        -------
        Bond
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``BondBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

class TermLoan:
    """
    Typed wrapper for the canonical Rust ``TermLoan`` instrument.

    Construct via :meth:`TermLoan.builder` (a bare decimal ``rate`` builds a
    fixed-rate loan; a serde ``dict`` builds a floating one), the
    ``TermLoan.example*`` presets or :meth:`TermLoan.from_json`. Every
    public Rust field is readable as a property; :meth:`TermLoan.price` /
    :meth:`TermLoan.metric` run the same pricer as :func:`price_instrument`.
    Instances are accepted directly by :func:`price_instrument` and
    :func:`instrument_cashflows`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> loan = TermLoan.example()
    >>> (loan.id, loan.rate)
    ('TERM-LOAN-USD-5Y', {'fixed': {'rate': 0.06}})
    """

    @staticmethod
    def builder() -> TermLoanBuilder:
        """
        Create a fluent builder (mirrors Rust ``TermLoan::builder()``).

        Returns
        -------
        TermLoanBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> builder = TermLoan.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def example() -> TermLoan:
        """
        Canonical example: 5-year USD 6% quarterly Act/360 loan with 2.5% per-period amortization (mirrors Rust ``TermLoan::example``).

        Returns
        -------
        TermLoan
            The example loan.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.example().id
        'TERM-LOAN-USD-5Y'
        """
        ...
    @staticmethod
    def example_floating_with_ddtl() -> TermLoan:
        """
        Canonical example: 7-year USD SOFR + 400bp leveraged loan with a delayed-draw commitment and a 0% floor (mirrors Rust ``TermLoan::example_floating_with_ddtl``).

        Returns
        -------
        TermLoan
            The example loan.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.example_floating_with_ddtl().ddtl is not None
        True
        """
        ...
    @staticmethod
    def example_callable() -> TermLoan:
        """
        Canonical example: loan carrying a prepayment (call) schedule (mirrors Rust ``TermLoan::example_callable``).

        Returns
        -------
        TermLoan
            The example loan.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.example_callable().call_schedule is not None
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> TermLoan:
        """
        Deserialize a validated TermLoan from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"term_loan"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        TermLoan
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.from_json(TermLoan.example().to_json()).id
        'TERM-LOAN-USD-5Y'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`TermLoan.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the loan spec as a plain Python ``dict``.

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
        Price this term loan and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hazard_rate"``, ``"tree"``).
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
        Compute one scalar metric for this term loan (e.g. ``"dv01"``).

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
    def currency(self) -> str:
        """
        Currency the loan is denominated in.

        Returns
        -------
        str
            ISO-4217 currency code such as ``"USD"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional_limit(self) -> Money:
        """
        Committed notional (facility limit).

        Returns
        -------
        Money
            Currency-tagged commitment.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def issue_date(self) -> datetime.date:
        """
        Issue / funding date.

        Returns
        -------
        datetime.date
            The issue date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Maturity (final redemption) date.

        Returns
        -------
        datetime.date
            The contractual maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rate(self) -> dict[str, object]:
        """
        Rate specification in serde form.

        Returns
        -------
        dict[str, object]
            ``{"fixed": {"rate": 0.06}}`` (decimal rate) or ``{"floating": {...}}``.

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
    def credit_curve_id(self) -> str | None:
        """
        Hazard curve identifier.

        Returns
        -------
        str | None
            Curve id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def amortization(self) -> str | dict[str, object]:
        """
        Amortization specification in serde form.

        Returns
        -------
        str | dict[str, object]
            ``"none"``, ``{"percent_of_remaining_per_period": {"pct": 0.025}}``, ``{"linear_between": {...}}``, ...

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def coupon_type(self) -> str:
        """
        Coupon type (serde string).

        Returns
        -------
        str
            ``"cash"``, ``"pik"``, ...

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def upfront_fee(self) -> dict[str, object] | None:
        """
        Upfront (arrangement or OID) fee paid on the issue date.

        Returns
        -------
        dict[str, object] | None
            ``UpfrontFee`` serde dict (``{"amount": Money-dict}`` or
            ``{"fraction_of_commitment": 0.02}``), or ``None`` when the loan
            has no upfront fee.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def ddtl(self) -> dict[str, object] | None:
        """
        Delayed-draw term loan specification in serde form.

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
    def covenants(self) -> dict[str, object] | None:
        """
        Covenant event schedule in serde form.

        Returns
        -------
        dict[str, object] | None
            The events dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def oid_eir(self) -> dict[str, object] | None:
        """
        OID / effective-interest-rate specification in serde form.

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
    def call_schedule(self) -> dict[str, object] | None:
        """
        Prepayment (call) schedule in serde form.

        Returns
        -------
        dict[str, object] | None
            The schedule dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int:
        """
        Settlement lag in business days.

        Returns
        -------
        int
            The lag (Rust default 2).

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

class TermLoanBuilder:
    """
    Fluent builder for :class:`TermLoan`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``currency``, ``notional_limit``,
    ``maturity``, ``rate``, ``frequency``, ``day_count``,
    ``discount_curve_id``, ``amortization``. Nested specs accept a ``dict``
    or JSON ``str`` in the Rust serde shape.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> loan = (
    ...     TermLoan
    ...     .builder()
    ...     .id("TL-1")
    ...     .currency("USD")
    ...     .notional_limit(10_000_000.0, currency="USD")
    ...     .issue_date("2024-01-01")
    ...     .maturity("2029-01-01")
    ...     .rate(0.06)
    ...     .frequency(Tenor.quarterly())
    ...     .day_count(DayCount.ACT_360)
    ...     .discount_curve_id("USD-OIS")
    ...     .amortization({"percent_of_remaining_per_period": {"pct": 0.025}})
    ...     .build()
    ... )
    >>> loan.rate
    {'fixed': {'rate': 0.06}}
    """

    def id(self, value: str) -> TermLoanBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def currency(self, value: str) -> TermLoanBuilder:
        """
        Set the loan currency.

        Parameters
        ----------
        value : str
            ISO-4217 currency code (e.g. ``"USD"``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized currency code.
        """
        ...
    def notional_limit(self, value: Money | float, currency: str | None = None) -> TermLoanBuilder:
        """
        Set the committed notional (facility limit).

        Parameters
        ----------
        value : Money | float
            Committed notional (facility limit); a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> TermLoanBuilder:
        """
        Set the required contractual issue / funding date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Required contractual issue / funding date (ISO 8601 strings accepted). Omission raises ValueError on build().

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> TermLoanBuilder:
        """
        Set the maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def rate(self, value: float | Rate | dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the interest rate specification.

        Parameters
        ----------
        value : float | Rate | dict[str, object] | str
            A bare decimal (``0.06`` = 6%) or ``Rate`` sets a fixed rate (mirrors
            Rust ``RateSpec::fixed_rate``). A
            ``dict`` / JSON ``str`` is the Rust ``RateSpec`` in serde form, e.g.
            ``{"floating": {"forward_curve_id": "USD-SOFR-3M", "spread_bp": 400, ...}}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed or a dict/str does not
            deserialize as a ``RateSpec``.
        TypeError
            If ``value`` has an unsupported type.
        """
        ...
    def frequency(self, value: Tenor | str) -> TermLoanBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency (e.g. ``Tenor.quarterly()``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, or a string ``value`` is not a recognized tenor string.
        """
        ...
    def day_count(self, value: DayCount | str) -> TermLoanBuilder:
        """
        Set the accrual day-count convention.

        Parameters
        ----------
        value : DayCount | str
            Day count convention (e.g. ``DayCount.ACT_360``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, or a string ``value`` is not a recognized day-count name.
        """
        ...
    def business_day_convention(self, value: str) -> TermLoanBuilder:
        """
        Set the business day convention.

        Parameters
        ----------
        value : str
            Business day convention (serde string). Default ``"modified_following"``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def calendar_id(self, value: str) -> TermLoanBuilder:
        """
        Set the holiday calendar identifier (e.g. ``"usny"``).

        Parameters
        ----------
        value : str
            Holiday calendar identifier (e.g. ``"usny"``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def stub(
        self, value: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
    ) -> TermLoanBuilder:
        """
        Set the stub rule.

        Parameters
        ----------
        value : StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
            Stub rule (serde string). Default ``"short_front"``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def discount_curve_id(self, value: str) -> TermLoanBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_curve_id(self, value: str) -> TermLoanBuilder:
        """
        Set the hazard curve identifier for credit-risky pricing.

        Parameters
        ----------
        value : str
            Hazard curve identifier for credit-risky pricing.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def amortization(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the amortization schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``AmortizationSpec`` in serde form (``dict`` or JSON string), e.g. ``"none"``, ``{"percent_of_remaining_per_period": {"pct": 0.025}}`` (declining balance), ``{"percent_of_original_per_period": {"pct": 0.01}}``, ``{"linear_between": {"start": "2025-01-01", "end": "2029-01-01"}}`` or ``{"custom_principal": {"items": [...]}}``; ``linear_to`` and ``step_remaining`` are rejected at ``build()``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``AmortizationSpec``.
        """
        ...
    def coupon_type(self, value: str) -> TermLoanBuilder:
        """
        Set the coupon type.

        Parameters
        ----------
        value : str
            Coupon type (serde string). ``"cash"`` (default), ``"pik"``, ...

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def upfront_fee(
        self, value: Money | float | dict[str, object] | str, currency: str | None = None
    ) -> TermLoanBuilder:
        """
        Set the upfront (arrangement or OID) fee paid on the issue date.

        Parameters
        ----------
        value : Money | float | dict[str, object] | str
            A ``Money`` or bare number (tagged with ``currency``) is an absolute
            amount; a ``dict`` or JSON string is the ``UpfrontFee`` serde form
            (``{"amount": Money-dict}`` or ``{"fraction_of_commitment": 0.02}``,
            a decimal fraction of the DDTL commitment, else ``notional_limit``).
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, a bare number is given without ``currency``, or a ``dict``/string does not match the ``UpfrontFee`` shape.
        """
        ...
    def ddtl(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the delayed-draw (DDTL) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``DdtlSpec`` in serde form (``dict`` or JSON string), e.g. ``TermLoan.example_floating_with_ddtl().ddtl``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``DdtlSpec``.
        """
        ...
    def covenants(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the covenant event schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``TermLoanCovenantEvents`` in serde form (``dict`` or JSON string), e.g. ``{"events": [...]}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``TermLoanCovenantEvents``.
        """
        ...
    def oid_eir(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the OID / effective-interest-rate specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OidEirSpec`` in serde form (``dict`` or JSON string), e.g. ``{"issue_price_pct": 99.0, ...}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``OidEirSpec``.
        """
        ...
    def call_schedule(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the prepayment (call) schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``LoanCallSchedule`` in serde form (``dict`` or JSON string), e.g. ``TermLoan.example_callable().call_schedule``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``LoanCallSchedule``.
        """
        ...
    def settlement_days(self, value: int) -> TermLoanBuilder:
        """
        Set the settlement lag in business days (default 2).

        Parameters
        ----------
        value : int
            Settlement lag.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> TermLoanBuilder:
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
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> TermLoanBuilder:
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
        TermLoanBuilder
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
        ``TermLoanBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``TermLoanBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> TermLoan:
        """
        Build the validated term loan.

        Runs the same validation as the Rust ``TermLoanBuilder::build`` (structural
        invariants only); pricing-time checks run in ``TermLoan.price``.

        Returns
        -------
        TermLoan
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``TermLoanBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...
