"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_credit/cds_tranche.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.cashflows.builder import RollRule
from finstack_quant.core.dates import BusinessDayConvention, DayCount, StubKind, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides

class CdsTranche:
    """
    Synthetic CDO / index tranche (typed wrapper for the canonical Rust
    ``CdsTranche``): protection on portfolio losses between ``attach_pct``
    and ``detach_pct`` (percent points), paying ``coupon_bp`` on the
    surviving tranche notional, priced with the one-factor Gaussian copula
    against the ``credit_index_id`` loss distribution.

    Construct via :meth:`CdsTranche.standard` (standard quarterly ACT/360
    schedule), :meth:`CdsTranche.builder`, :meth:`CdsTranche.example` or
    :meth:`CdsTranche.from_json`. Every public Rust field is readable as a
    property; ``expected_loss`` / ``jump_to_default`` mirror the Rust
    accessors and :meth:`CdsTranche.price` / :meth:`CdsTranche.metric` run
    the same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CdsTranche
    >>> tranche = CdsTranche.example()
    >>> (tranche.attach_pct, tranche.detach_pct, tranche.side)
    (0.0, 3.0, 'pay')
    """

    @staticmethod
    def builder() -> CdsTrancheBuilder:
        """
        Create a fluent builder (mirrors Rust ``CdsTranche::builder()``).

        Returns
        -------
        CdsTrancheBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsTranche
        >>> builder = CdsTranche.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CdsTranche:
        """
        Deserialize a validated CdsTranche from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cds_tranche"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CdsTranche
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsTranche
        >>> CdsTranche.from_json(CdsTranche.example().to_json()).id
        'CDXIG-42-0X3'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CdsTranche.from_json`.

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
            Model key (the copula tranche pricer is the native model).
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
        Compute one scalar metric for this instrument (e.g. ``"expected_loss"``).

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
    def example() -> CdsTranche:
        """
        Canonical CDX.NA.IG 42 equity (0–3%) tranche, USD 10,000,000 (mirrors
        Rust ``CdsTranche::example``): buy protection, 100bp running, maturity
        2029-12-20, curves ``USD-OIS`` / ``CDX.NA.IG.HAZARD``.

        Returns
        -------
        CdsTranche
            The example tranche.

        Raises
        ------
        ValueError
            If the example instrument fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsTranche
        >>> CdsTranche.example().credit_index_id
        'CDX.NA.IG.HAZARD'
        """
        ...
    @staticmethod
    def standard(
        id: str,
        params: CdsTrancheParams,
        discount_curve_id: str,
        credit_index_id: str,
        side: Literal["pay", "receive"],
    ) -> CdsTranche:
        """
        Build a tranche on the standard schedule (mirrors Rust
        ``CdsTranche::standard``): quarterly, ACT/360, Following, weekends-only
        calendar, short-front stub.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        params : CdsTrancheParams
            Economic terms (attach/detach, notional, maturity, coupon).
        discount_curve_id : str
            Discount curve identifier.
        credit_index_id : str
            Credit index identifier for the loss distribution.
        side : {"pay", "receive"}
            Tranche side: ``"pay"`` buys protection, ``"receive"`` sells it.

        Returns
        -------
        CdsTranche
            The validated tranche.

        Raises
        ------
        ValueError
            If ``side`` is unknown or the parameters fail validation
            (``attach_pct >= detach_pct``, fractional attach/detach, ...).

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CdsTranche, CdsTrancheParams
        >>> params = CdsTrancheParams.mezzanine_tranche(
        ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 100.0
        ... )
        >>> tranche = CdsTranche.standard("CDX-42-3X7", params, "USD-OIS", "CDX.NA.IG.HAZARD", "pay")
        >>> (tranche.day_count, tranche.business_day_convention)
        (DayCount.ACT_360, 'following')
        """
        ...

    @property
    def index_name(self) -> str:
        """
        Underlying index name.

        Returns
        -------
        str
            The index name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Index series number.

        Returns
        -------
        int
            The series number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attach_pct(self) -> float:
        """
        Attachment point in percent.

        Returns
        -------
        float
            Attachment (``3.0`` = 3%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def detach_pct(self) -> float:
        """
        Detachment point in percent.

        Returns
        -------
        float
            Detachment (``7.0`` = 7%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Tranche notional.

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
    def maturity(self) -> datetime.date:
        """
        Scheduled maturity.

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
    def coupon_bp(self) -> float:
        """
        Contractual running coupon (bp) paid on the tranche premium leg.

        Returns
        -------
        float
            Coupon quoted in basis points per annum on the outstanding
            tranche notional (for example ``100.0`` for a 100 bp coupon),
            not as a decimal rate. Accrues on the premium-leg day count.

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
            The coupon tenor (typically quarterly).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Day count convention.

        Returns
        -------
        DayCount
            ``DayCount.ACT_360`` for standard tranches.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde name).

        Returns
        -------
        str
            ``"modified_following"`` unless set otherwise.

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
            The calendar id, or ``None``.

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
    def credit_index_id(self) -> str:
        """
        Credit index identifier for the loss distribution.

        Returns
        -------
        str
            The credit index id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def side(self) -> str:
        """
        Direction of the tranche position from the holder's perspective.

        Returns
        -------
        str
            Serde string, either ``"pay"`` (buys protection: pays the running
            coupon and receives tranche loss payments) or
            ``"receive"`` (sells protection: receives the coupon and pays
            losses), matching the CDS and CDS index ``side``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start_date(self) -> datetime.date | None:
        """
        Explicit contract start (effective) date for schedule anchoring.

        Returns
        -------
        datetime.date | None
            The date, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def realized_loss(self) -> float:
        """
        Realized portfolio loss so far.

        Returns
        -------
        float
            Fraction of the original portfolio notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def roll_rule(self) -> RollRule:
        """
        Coupon roll-date grid.

        Returns
        -------
        RollRule
            ``RollRule.CDS_IMM`` for the CDS roll dates (20th of Mar/Jun/Sep/Dec);
            ``RollRule.NONE`` for a bespoke ``frequency``/``stub`` schedule.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub convention for a bespoke (``RollRule.NONE``) coupon schedule.

        Returns
        -------
        StubKind
            The stub rule (``short_front`` by default).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def upfront(self) -> tuple[datetime.date, Money] | None:
        """
        Upfront payment as ``(payment_date, amount)``.

        Returns
        -------
        tuple[datetime.date, Money] | None
            The pair, or ``None``.

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
        Python-style rendering of the key economics, e.g. ``CdsTranche(id='CDXIG-42-0X3', index_name='CDX.NA.IG', series=42, attach_pct=0.0, detach_pct=3.0, side='pay', ...)``.

        Returns
        -------
        str
            ``CdsTranche(<field>=<value>, ...)``.

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

class CdsTrancheBuilder:
    """
    Fluent builder for :class:`CdsTranche`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    The builder pre-seeds ``realized_loss(0.0)``; ``roll_rule`` defaults to
    ``RollRule.NONE``, ``stub`` to ``"short_front"`` and
    ``business_day_convention`` to
    ``"modified_following"``. Builders are consumed by ``build()``; create a
    new builder per instrument. Required fields: ``id``, ``index_name``,
    ``series``, ``attach_pct``, ``detach_pct``, ``notional``, ``maturity``,
    ``coupon_bp``, ``frequency``, ``day_count``,
    ``discount_curve_id``, ``credit_index_id``, ``side``.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import CdsTranche
    >>> tranche = (
    ...     CdsTranche
    ...     .builder()
    ...     .id("CDX-IG-42-3-7")
    ...     .index_name("CDX.NA.IG")
    ...     .series(42)
    ...     .attach_pct(3.0)
    ...     .detach_pct(7.0)
    ...     .notional(Money(10_000_000.0, Currency("USD")))
    ...     .maturity("2029-06-20")
    ...     .coupon_bp(100.0)
    ...     .frequency(Tenor.quarterly())
    ...     .day_count(DayCount.ACT_360)
    ...     .discount_curve_id("USD-OIS")
    ...     .credit_index_id("CDX-IG-42-CURVE")
    ...     .side("pay")
    ...     .build()
    ... )
    >>> tranche.coupon_bp
    100.0
    """

    def id(self, value: str) -> CdsTrancheBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the tranche trade.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def index_name(self, value: str) -> CdsTrancheBuilder:
        """
        Set the underlying index name.

        Parameters
        ----------
        value : str
            Index name, e.g. ``"CDX.NA.IG"``, ``"CDX.NA.HY"``, ``"iTraxx EUR"``.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def series(self, value: int) -> CdsTrancheBuilder:
        """
        Set the series number.

        Parameters
        ----------
        value : int
            Series number, e.g. ``42``.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def attach_pct(self, value: float) -> CdsTrancheBuilder:
        """
        Set the attachment point.

        Parameters
        ----------
        value : float
            Attachment point quoted in percent (``0.0`` for equity; ``3.0`` for a tranche attaching at 3%).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def detach_pct(self, value: float) -> CdsTrancheBuilder:
        """
        Set the detachment point.

        Parameters
        ----------
        value : float
            Detachment point quoted in percent (``3.0`` for a 0-3% tranche).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> CdsTrancheBuilder:
        """
        Set the notional amount of the tranche.

        Parameters
        ----------
        value : Money
            Notional amount of the tranche.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CdsTrancheBuilder:
        """
        Set the maturity date of the tranche.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def coupon_bp(self, value: float | Bps) -> CdsTrancheBuilder:
        """
        Set the running coupon.

        Parameters
        ----------
        value : float | Bps
            Running coupon in basis points (``100.0`` = 1.00%).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither a number nor ``Bps``.
        """
        ...
    def frequency(self, value: Tenor | str) -> CdsTrancheBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency (typically quarterly, ``"3M"``).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a tenor string cannot be parsed.
        """
        ...
    def day_count(self, value: DayCount | str) -> CdsTrancheBuilder:
        """
        Set the day count convention.

        Parameters
        ----------
        value : DayCount | str
            Day count convention (typically ``DayCount.ACT_360`` / ``"act_360"``).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized day count.
        """
        ...
    def business_day_convention(self, value: BusinessDayConvention | str) -> CdsTrancheBuilder:
        """
        Set the business day convention for coupon dates.

        Parameters
        ----------
        value : BusinessDayConvention | str
            Roll rule (``"modified_following"`` when never set).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized convention.
        """
        ...
    def calendar_id(self, value: str) -> CdsTrancheBuilder:
        """
        Set the holiday calendar identifier.

        Parameters
        ----------
        value : str
            Holiday calendar identifier.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def discount_curve_id(self, value: str) -> CdsTrancheBuilder:
        """
        Set the discount curve identifier (by quote currency).

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_index_id(self, value: str) -> CdsTrancheBuilder:
        """
        Set the credit index identifier for survival/loss modeling.

        Parameters
        ----------
        value : str
            Credit index identifier.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def side(self, value: Literal["pay", "receive"]) -> CdsTrancheBuilder:
        """
        Set the tranche side (``"pay"`` buys protection, ``"receive"`` sells it).

        Parameters
        ----------
        value : Literal["pay", "receive"]
            Tranche side.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized side.
        """
        ...
    def start_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CdsTrancheBuilder:
        """
        Set the contract start (effective) date for schedule anchoring.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Effective date; if never set, uses the as-of date (or the prior CDS roll date when ``roll_rule`` is ``RollRule.CDS_IMM``).

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def realized_loss(self, value: float) -> CdsTrancheBuilder:
        """
        Set the realized (settled) loss.

        Parameters
        ----------
        value : float
            Realized loss as a fraction of the original portfolio notional; ``0.0`` when never set.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def roll_rule(self, value: RollRule) -> CdsTrancheBuilder:
        """
        Set the coupon roll-date grid.

        Parameters
        ----------
        value : RollRule
            ``RollRule.CDS_IMM`` for the CDS roll dates (20th of Mar, Jun, Sep,
            Dec) or ``RollRule.NONE`` (the default) for a schedule from
            ``frequency`` and ``stub``; ``RollRule.IMM`` is rejected by ``build()``.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def stub(
        self, value: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
    ) -> CdsTrancheBuilder:
        """
        Set the stub convention for a bespoke coupon schedule.

        Parameters
        ----------
        value : StubKind | str
            Stub rule used when ``roll_rule`` is ``RollRule.NONE``; ``"short_front"`` when never set.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the stub is unknown.
        """
        ...
    def upfront(self, value: tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]) -> CdsTrancheBuilder:
        """
        Set the upfront payment.

        Parameters
        ----------
        value : tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]
            ``(payment_date, amount)``; the amount currency must match the tranche notional.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``(date, Money)`` pair.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CdsTrancheBuilder:
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
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> CdsTrancheBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CdsTrancheBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CdsTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> CdsTrancheBuilder:
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
        CdsTrancheBuilder
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
        ``CdsTrancheBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CdsTrancheBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CdsTranche:
        """
        Build the validated CDS tranche.

        Runs only the Rust ``CdsTrancheBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`CdsTranche.price`.

        Returns
        -------
        CdsTranche
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CdsTrancheBuilder: missing required field 'id'``), or the instrument
            fails validation (``attach_pct >= detach_pct``, fractional attach/detach, loss outside ``[0, 1]``).
        """
        ...

class CdsTrancheParams:
    """
    Economic terms of an index tranche (typed wrapper for the Rust
    ``CdsTrancheParams``). Attachment and detachment are quoted in percent
    points (``3.0`` = 3%), the running coupon in basis points. Pass to
    :meth:`CdsTranche.standard` for a tranche on the standard quarterly
    ACT/360 schedule.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import CdsTrancheParams
    >>> params = CdsTrancheParams.mezzanine_tranche(
    ...     "CDX.NA.IG", 42, Money(10_000_000.0, Currency("USD")), datetime.date(2029, 12, 20), 100.0
    ... )
    >>> (params.attach_pct, params.detach_pct)
    (3.0, 7.0)
    """

    def __init__(
        self,
        index_name: str,
        series: int,
        attach_pct: float,
        detach_pct: float,
        notional: Money,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        coupon_bp: float | Bps,
        realized_loss: float = 0.0,
    ) -> None:
        """
        Describe a tranche on a credit index.

        Parameters
        ----------
        index_name : str
            Underlying index name, e.g. ``"CDX.NA.IG"``.
        series : int
            Index series number.
        attach_pct : float
            Attachment point in percent (``3.0`` = 3%).
        detach_pct : float
            Detachment point in percent (``7.0`` = 7%); must exceed ``attach_pct``.
        notional : Money
            Tranche notional.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity (an IMM date for standard tranches).
        coupon_bp : float | Bps
            Running coupon in basis points (``100.0`` = 1%).
        realized_loss : float, default 0.0
            Realized portfolio loss so far as a fraction of the original
            portfolio notional, in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``realized_loss`` is outside ``[0, 1]`` or a date cannot be interpreted.
        TypeError
            If ``coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CdsTrancheParams
        >>> CdsTrancheParams(
        ...     "CDX.NA.IG", 42, 7.0, 15.0, Money(5_000_000.0, Currency("USD")), "2029-12-20", 100.0
        ... ).coupon_bp
        100.0
        """
        ...
    @staticmethod
    def equity_tranche(
        index_name: str,
        series: int,
        notional: Money,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        coupon_bp: float | Bps,
    ) -> CdsTrancheParams:
        """
        Standard equity tranche (0%–3%).

        Parameters
        ----------
        index_name : str
            Underlying index name.
        series : int
            Index series number.
        notional : Money
            Tranche notional.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity.
        coupon_bp : float | Bps
            Running coupon in basis points.

        Returns
        -------
        CdsTrancheParams
            Tranche terms with ``attach_pct=0.0`` and ``detach_pct=3.0``.

        Raises
        ------
        ValueError
            If ``maturity`` cannot be interpreted.
        TypeError
            If ``coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CdsTrancheParams
        >>> CdsTrancheParams.equity_tranche(
        ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 500.0
        ... ).detach_pct
        3.0
        """
        ...
    @staticmethod
    def mezzanine_tranche(
        index_name: str,
        series: int,
        notional: Money,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        coupon_bp: float | Bps,
    ) -> CdsTrancheParams:
        """
        Standard mezzanine tranche (3%–7%).

        Parameters
        ----------
        index_name : str
            Underlying index name.
        series : int
            Index series number.
        notional : Money
            Tranche notional.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity.
        coupon_bp : float | Bps
            Running coupon in basis points.

        Returns
        -------
        CdsTrancheParams
            Tranche terms with ``attach_pct=3.0`` and ``detach_pct=7.0``.

        Raises
        ------
        ValueError
            If ``maturity`` cannot be interpreted.
        TypeError
            If ``coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CdsTrancheParams
        >>> CdsTrancheParams.mezzanine_tranche(
        ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 100.0
        ... ).attach_pct
        3.0
        """
        ...
    @property
    def index_name(self) -> str:
        """
        Underlying index name.

        Returns
        -------
        str
            The index name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Index series number.

        Returns
        -------
        int
            The series number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attach_pct(self) -> float:
        """
        Attachment point in percent.

        Returns
        -------
        float
            Attachment (``3.0`` = 3%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def detach_pct(self) -> float:
        """
        Detachment point in percent.

        Returns
        -------
        float
            Detachment (``7.0`` = 7%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Tranche notional.

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
    def maturity(self) -> datetime.date:
        """
        Scheduled maturity.

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
    def coupon_bp(self) -> float:
        """
        Contractual running coupon (bp) paid on the tranche premium leg.

        Returns
        -------
        float
            Coupon quoted in basis points per annum on the outstanding
            tranche notional (for example ``100.0`` for a 100 bp coupon),
            not as a decimal rate. Accrues on the premium-leg day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def realized_loss(self) -> float:
        """
        Realized portfolio loss so far.

        Returns
        -------
        float
            Fraction of the original portfolio notional in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CdsTrancheParams(index_name='CDX.NA.IG', series=42, attach_pct=3.0, detach_pct=7.0, ...)``.

        Returns
        -------
        str
            ``CdsTrancheParams(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...
