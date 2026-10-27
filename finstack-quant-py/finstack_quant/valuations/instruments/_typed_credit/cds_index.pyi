"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_credit/cds_index.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides
from finstack_quant.valuations.instruments._typed_legs import PremiumLegSpec, ProtectionLegSpec

class CdsIndex:
    """
    Credit index (CDX / iTraxx) trade (typed wrapper for the canonical Rust
    ``CdsIndex``).

    Priced against a single index hazard curve (``pricing="single_curve"``, a
    synthetic CDS) or by expanding into weighted constituents
    (``pricing="constituents"``); ``index_factor`` scales the surviving
    notional after defaults. Construct via :meth:`CdsIndex.from_preset` (the
    preferred way for standardized indices), :meth:`CdsIndex.builder`,
    :meth:`CdsIndex.example` or :meth:`CdsIndex.from_json`. Every public Rust
    field is readable as a property; ``par_spread`` / ``risky_pv01`` / ``cs01``
    mirror the Rust accessors and :meth:`CdsIndex.price` /
    :meth:`CdsIndex.metric` run the same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CdsIndex
    >>> idx = CdsIndex.example()
    >>> (idx.index_name, idx.series, idx.pricing, idx.num_constituents)
    ('CDX.NA.IG', 42, 'single_curve', 125)
    """

    @staticmethod
    def builder() -> CdsIndexBuilder:
        """
        Create a fluent builder (mirrors Rust ``CdsIndex::builder()``).

        Returns
        -------
        CdsIndexBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndex
        >>> builder = CdsIndex.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CdsIndex:
        """
        Deserialize a validated CdsIndex from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cds_index"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CdsIndex
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndex
        >>> CdsIndex.from_json(CdsIndex.example().to_json()).id
        'CDX-IG-42'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CdsIndex.from_json`.

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
            Model key (``"hazard_rate"`` is the native model).
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
        Compute one scalar metric for this instrument (e.g. ``"cs01"``).

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
    def example() -> CdsIndex:
        """
        Canonical CDX.NA.IG series 42 USD 10,000,000 payer (mirrors Rust
        ``CdsIndex::example``): 60bp running coupon, ``single_curve`` pricing off
        ``CDX.NA.IG.HAZARD`` discounted on ``USD-OIS``, premium 2024-03-20 to
        2029-12-20, 125 names.

        Returns
        -------
        CdsIndex
            The example index trade.

        Raises
        ------
        ValueError
            If the example instrument fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndex
        >>> CdsIndex.example().protection_leg.credit_curve_id
        'CDX.NA.IG.HAZARD'
        """
        ...
    @staticmethod
    def from_preset(
        preset: CdsIndexParams,
        id: str,
        notional: Money,
        side: Literal["pay", "receive"],
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        recovery_rate: float,
        discount_curve_id: str,
        credit_curve_id: str,
    ) -> CdsIndex:
        """
        Build an index trade from a standardized preset (mirrors Rust
        ``CdsIndex::from_preset``): the premium leg takes the preset's fixed
        coupon and regional convention (day count, frequency, business-day rule,
        calendar, stub), pricing is ``"single_curve"``, ``index_factor`` is
        ``1.0`` and the constituent list is empty.

        Parameters
        ----------
        preset : CdsIndexParams
            Index identity, coupon and convention (e.g. :meth:`CdsIndexParams.cdx_na_ig`).
        id : str
            Unique instrument identifier for the trade.
        notional : Money
            Index notional.
        side : {"pay", "receive"}
            ``"pay"`` buys protection, ``"receive"`` sells protection.
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Premium accrual start (typically the last IMM roll).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity (an IMM date).
        recovery_rate : float
            Assumed recovery as a fraction (``0.4`` = 40%).
        discount_curve_id : str
            Discount curve identifier.
        credit_curve_id : str
            Index hazard curve identifier.

        Returns
        -------
        CdsIndex
            The index trade.

        Raises
        ------
        ValueError
            If ``side`` is unknown, a date cannot be interpreted, or the preset
            coupon is not representable.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CdsIndex, CdsIndexParams
        >>> idx = CdsIndex.from_preset(
        ...     CdsIndexParams.cdx_na_ig(42, 1, 100.0),
        ...     "CDX-42-5Y",
        ...     Money(10_000_000.0, Currency("USD")),
        ...     "pay",
        ...     "2024-03-20",
        ...     "2029-06-20",
        ...     0.4,
        ...     "USD-OIS",
        ...     "CDX.NA.IG.HAZARD",
        ... )
        >>> (idx.convention, idx.num_constituents, idx.index_factor)
        ('isda_na', 125, 1.0)
        """
        ...
    def par_spread(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Par spread of the index in basis points (mirrors Rust
        ``CdsIndex::par_spread``; risky-annuity denominator in ``single_curve``
        mode, weighted constituents otherwise).

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves the index names.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Par spread in basis points.

        Raises
        ------
        KeyError
            If a curve is missing from ``market``.
        ValueError
            If ``as_of`` or the market JSON is invalid.
        """
        ...
    def risky_pv01(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Risky PV01 (risky annuity) of the premium leg (mirrors Rust
        ``CdsIndex::risky_pv01``): PV of 1bp running on the surviving notional.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves the index names.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Risky PV01 in notional currency units per basis point.

        Raises
        ------
        KeyError
            If a curve is missing from ``market``.
        ValueError
            If ``as_of`` or the market JSON is invalid.
        """
        ...
    def cs01(self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str) -> float:
        """
        Credit spread sensitivity (mirrors Rust ``CdsIndex::cs01`` with the cached
        recalibration provider): the hazard curve(s) are rebootstrapped after a
        1bp parallel spread bump. Hazard curves built by hand without a lossless
        calibration recipe raise.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves the index names.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            PV change for a +1bp spread move, in notional currency units.

        Raises
        ------
        KeyError
            If a curve is missing from ``market``.
        ValueError
            If the hazard curve carries no lossless calibration recipe.
        RuntimeError
            If the recalibration fails.
        """
        ...
    @property
    def index_name(self) -> str:
        """
        Ticker of the credit index family this contract references.

        Returns
        -------
        str
            Index family ticker as supplied at construction, for example
            ``"CDX.NA.IG"`` or ``"iTraxx Europe"``. The value is stored
            verbatim and is not normalised or validated against a registry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Roll series of the credit index, incremented each semi-annual roll.

        Returns
        -------
        int
            Series number as an unsigned integer (for example ``41`` for
            CDX.NA.IG series 41). Higher numbers denote more recent
            on-the-run rolls.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def version(self) -> int:
        """
        Version within the series.

        Returns
        -------
        int
            The version number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Traded notional of the index position, carrying its own currency.

        Returns
        -------
        Money
            Currency-tagged notional in the index deal currency (USD for
            CDX, EUR for iTraxx). It is the full original notional and is
            not scaled by the index factor; apply
            :attr:`index_factor` to obtain the current outstanding amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def index_factor(self) -> float:
        """
        Fraction of surviving notional.

        Returns
        -------
        float
            ``1.0`` when no constituent has defaulted since inception.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def side(self) -> str:
        """
        Protection perspective.

        Returns
        -------
        str
            ``"pay"`` (buy protection) or ``"receive"`` (sell protection).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def convention(self) -> str:
        """
        Regional ISDA convention (serde name).

        Returns
        -------
        str
            ``"isda_na"``, ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def premium_leg(self) -> PremiumLegSpec:
        """
        Premium leg specification.

        Returns
        -------
        PremiumLegSpec
            The premium leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection_leg(self) -> ProtectionLegSpec:
        """
        Protection leg specification.

        Returns
        -------
        ProtectionLegSpec
            The protection leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pricing(self) -> str:
        """
        Pricing aggregation mode.

        Returns
        -------
        str
            ``"single_curve"`` or ``"constituents"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def constituents(self) -> list[CdsIndexConstituent]:
        """
        Constituent rows.

        Returns
        -------
        list[CdsIndexConstituent]
            Typed rows; empty in ``single_curve`` mode.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_constituents(self) -> int | None:
        """
        Number of names in the pool.

        Returns
        -------
        int | None
            The count, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def upfront(self) -> tuple[datetime.date, Money] | None:
        """
        Contractual upfront payment as ``(payment_date, amount)``; positive means the protection buyer pays.

        Returns
        -------
        tuple[datetime.date, Money] | None
            The upfront pair, or ``None`` when the trade has no upfront.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def margin_spec(self) -> dict[str, object] | None:
        """
        OTC margin specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The ``OtcMarginSpec`` dict, or ``None`` for unmargined trades.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Premium-leg end date as seen by the pricer.

        Returns
        -------
        datetime.date | None
            The scheduled maturity, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CdsIndex(id='CDX-IG-42', index_name='CDX.NA.IG', series=42, side='pay', notional=Money(10000000.0, 'USD'), coupon_bp=60, ...)``.

        Returns
        -------
        str
            ``CdsIndex(<field>=<value>, ...)``.

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

class CdsIndexBuilder:
    """
    Fluent builder for :class:`CdsIndex`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    The builder pre-seeds an empty ``constituents`` list so ``build()``
    succeeds without calling :meth:`constituents` in ``"single_curve"`` mode.
    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``index_name``, ``series``,
    ``version``, ``notional``, ``index_factor``, ``side``, ``convention``,
    ``premium``, ``protection``, ``pricing``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     CdsIndex,
    ...     PremiumLegSpec,
    ...     ProtectionLegSpec,
    ... )
    >>> from finstack_quant.cashflows.builder import RollRule
    >>> premium = PremiumLegSpec(
    ...     datetime.date(2024, 3, 20),
    ...     datetime.date(2029, 6, 20),
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     100.0,
    ...     "USD-OIS",
    ...     roll_rule=RollRule.CDS_IMM,
    ... )
    >>> index = (
    ...     CdsIndex
    ...     .builder()
    ...     .id("CDX-IG-42")
    ...     .index_name("CDX.NA.IG")
    ...     .series(42)
    ...     .version(1)
    ...     .notional(Money(10_000_000.0, Currency("USD")))
    ...     .index_factor(1.0)
    ...     .side("pay")
    ...     .convention("isda_na")
    ...     .premium_leg(premium)
    ...     .protection_leg(ProtectionLegSpec("CDX-IG-42-HZD", 0.4, 3))
    ...     .pricing("single_curve")
    ...     .num_constituents(125)
    ...     .build()
    ... )
    >>> index.pricing
    'single_curve'
    """

    def id(self, value: str) -> CdsIndexBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the index trade.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def index_name(self, value: str) -> CdsIndexBuilder:
        """
        Set the index name.

        Parameters
        ----------
        value : str
            Index name, e.g. ``"CDX.NA.IG"``, ``"CDX.NA.HY"``, ``"iTraxx Europe"``.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def series(self, value: int) -> CdsIndexBuilder:
        """
        Set the series number.

        Parameters
        ----------
        value : int
            Series number, e.g. ``42``.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def version(self, value: int) -> CdsIndexBuilder:
        """
        Set the version number within the series.

        Parameters
        ----------
        value : int
            Version number, e.g. ``1``.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> CdsIndexBuilder:
        """
        Set the notional amount of the index.

        Parameters
        ----------
        value : Money
            Notional amount of the index.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def index_factor(self, value: float) -> CdsIndexBuilder:
        """
        Set the index factor (fraction of surviving notional).

        Parameters
        ----------
        value : float
            Index factor in ``[0.0, 1.0]``; ``1.0`` means no constituent has defaulted since series inception.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def side(self, value: Literal["pay", "receive"]) -> CdsIndexBuilder:
        """
        Set the protection buyer/seller perspective.

        Parameters
        ----------
        value : Literal["pay", "receive"]
            ``"pay"`` to buy protection (pay premium), ``"receive"`` to sell protection (receive premium).

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized side.
        """
        ...
    def convention(self, value: Literal["isda_na", "isda_eu", "isda_as", "custom"]) -> CdsIndexBuilder:
        """
        Set the ISDA regional convention.

        Parameters
        ----------
        value : Literal["isda_na", "isda_eu", "isda_as", "custom"]
            ``"isda_na"`` is the SNAC / post-Big-Bang North American standard; ``"isda_eu"`` European; ``"isda_as"`` Asian; ``"custom"`` for a manually configured convention.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not one of the accepted strings (the message lists them).
        """
        ...
    def premium_leg(self, value: PremiumLegSpec) -> CdsIndexBuilder:
        """
        Set the premium leg specification.

        Parameters
        ----------
        value : PremiumLegSpec
            Premium leg specification (coupon schedule and discounting).

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def protection_leg(self, value: ProtectionLegSpec) -> CdsIndexBuilder:
        """
        Set the protection leg specification.

        Parameters
        ----------
        value : ProtectionLegSpec
            Protection leg specification (credit curve and settlement).

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def pricing(self, value: Literal["single_curve", "constituents"]) -> CdsIndexBuilder:
        """
        Set the pricing aggregation mode.

        Parameters
        ----------
        value : Literal["single_curve", "constituents"]
            ``"single_curve"`` prices the index against a single index hazard curve (synthetic CDS). ``"constituents"`` prices each issuer separately and aggregates by weight; requires :meth:`CdsIndexBuilder.constituents` to be set.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized pricing mode.
        """
        ...
    def constituents(self, value: list[CdsIndexConstituent | dict[str, object]] | str) -> CdsIndexBuilder:
        """
        Set the index constituents.

        Parameters
        ----------
        value : list[CdsIndexConstituent | dict[str, object]] | str
            Constituent rows as typed :class:`CdsIndexConstituent` objects, dicts with ``credit`` (``reference_entity``, ``recovery_rate``, ``credit_curve_id``), ``weight`` and optional ``defaulted``, or a JSON array of the same shape.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, ``value`` is neither a sequence nor a string, or a dict/JSON entry does not match the constituent shape.
        """
        ...
    def num_constituents(self, value: int) -> CdsIndexBuilder:
        """
        Set the number of reference entities in the index pool.

        Parameters
        ----------
        value : int
            Number of names in the index pool, e.g. ``125`` for CDX.NA.IG; required for portfolio-level analytics (e.g. jump-to-default) when ``constituents`` is empty.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def upfront(self, value: tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]) -> CdsIndexBuilder:
        """
        Set the contractual upfront payment.

        Parameters
        ----------
        value : tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]
            ``(payment_date, amount)`` on the index notional; positive means the protection buyer pays. Discounted from
            the payment date and dropped once that date is before the valuation date.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is not a ``(date, Money)`` pair.
        """
        ...
    def margin_spec(self, value: dict[str, object] | str) -> CdsIndexBuilder:
        """
        Set the OTC margin (CSA / initial-margin) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OtcMarginSpec`` in serde form (``dict`` or JSON string).

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as an ``OtcMarginSpec``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CdsIndexBuilder:
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
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> CdsIndexBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CdsIndexBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CdsIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> CdsIndexBuilder:
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
        CdsIndexBuilder
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
        ``CdsIndexBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CdsIndexBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CdsIndex:
        """
        Build the validated CDS index.

        Runs only the Rust ``CdsIndexBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`CdsIndex.price`.

        Returns
        -------
        CdsIndex
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CdsIndexBuilder: missing required field 'id'``), or the instrument
            fails validation (index factor outside ``[0, 1]``, empty constituents in ``constituents`` mode).
        """
        ...

class CdsIndexParams:
    """
    Preset descriptor for a standardized CDS index (typed wrapper for the Rust
    ``CdsIndexParams``): index identity (name, series, version), fixed running
    coupon and regional convention. Trade state lives on the :class:`CdsIndex`
    built with :meth:`CdsIndex.from_preset`. Instances compare by value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CdsIndexParams
    >>> preset = CdsIndexParams.cdx_na_ig(42, 1, 100.0)
    >>> (preset.index_name, preset.convention, preset.num_constituents)
    ('CDX.NA.IG', 'isda_na', 125)
    """

    def __init__(
        self,
        index_name: str,
        series: int,
        version: int,
        coupon_bp: float | Bps,
        convention: Literal["isda_na", "isda_eu", "isda_as", "custom"] = "isda_na",
        num_constituents: int | None = None,
    ) -> None:
        """
        Describe a standardized CDS index.

        Parameters
        ----------
        index_name : str
            Index name, e.g. ``"CDX.NA.IG"`` or ``"iTraxx Europe"``.
        series : int
            Series number (e.g. ``42``).
        version : int
            Version within the series (e.g. ``1``).
        coupon_bp : float | Bps
            Fixed running coupon in basis points (``100.0`` = 1%).
        convention : {"isda_na", "isda_eu", "isda_as", "custom"}, default "isda_na"
            Regional ISDA convention (``"isda_na"`` is the SNAC standard).
        num_constituents : int | None
            Number of names in the pool, used by portfolio analytics when the
            constituent list is empty.

        Raises
        ------
        ValueError
            If ``convention`` is not an accepted string.
        TypeError
            If ``coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndexParams
        >>> CdsIndexParams("CDX.NA.HY", 42, 1, 500.0).coupon_bp
        500.0
        """
        ...
    @staticmethod
    def cdx_na_ig(series: int, version: int, coupon_bp: float | Bps) -> CdsIndexParams:
        """
        CDX North American Investment Grade preset (125 names, ``isda_na``).

        Parameters
        ----------
        series : int
            Series number.
        version : int
            Version within the series.
        coupon_bp : float | Bps
            Fixed running coupon in basis points (``100.0`` for CDX.NA.IG).

        Returns
        -------
        CdsIndexParams
            The preset.

        Raises
        ------
        TypeError
            If ``coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndexParams
        >>> CdsIndexParams.cdx_na_ig(42, 1, 100.0).num_constituents
        125
        """
        ...
    @staticmethod
    def cdx_na_hy(series: int, version: int, coupon_bp: float | Bps) -> CdsIndexParams:
        """
        CDX North American High Yield preset (100 names, ``isda_na``).

        Parameters
        ----------
        series : int
            Series number.
        version : int
            Version within the series.
        coupon_bp : float | Bps
            Fixed running coupon in basis points (``500.0`` for CDX.NA.HY).

        Returns
        -------
        CdsIndexParams
            The preset.

        Raises
        ------
        TypeError
            If ``coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndexParams
        >>> CdsIndexParams.cdx_na_hy(42, 1, 500.0).num_constituents
        100
        """
        ...
    @staticmethod
    def itraxx_europe(series: int, version: int, coupon_bp: float | Bps) -> CdsIndexParams:
        """
        iTraxx Europe Main preset (125 names, ``isda_eu``).

        Parameters
        ----------
        series : int
            Series number.
        version : int
            Version within the series.
        coupon_bp : float | Bps
            Fixed running coupon in basis points (``100.0`` for iTraxx Europe).

        Returns
        -------
        CdsIndexParams
            The preset.

        Raises
        ------
        TypeError
            If ``coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndexParams
        >>> CdsIndexParams.itraxx_europe(41, 1, 100.0).convention
        'isda_eu'
        """
        ...
    @property
    def index_name(self) -> str:
        """
        Ticker of the credit index family this contract references.

        Returns
        -------
        str
            Index family ticker as supplied at construction, for example
            ``"CDX.NA.IG"`` or ``"iTraxx Europe"``. The value is stored
            verbatim and is not normalised or validated against a registry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Roll series of the credit index, incremented each semi-annual roll.

        Returns
        -------
        int
            Series number as an unsigned integer (for example ``41`` for
            CDX.NA.IG series 41). Higher numbers denote more recent
            on-the-run rolls.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def version(self) -> int:
        """
        Version within the series.

        Returns
        -------
        int
            The version number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def coupon_bp(self) -> float:
        """
        Fixed running coupon.

        Returns
        -------
        float
            Coupon in basis points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def convention(self) -> str:
        """
        Regional ISDA convention (serde name).

        Returns
        -------
        str
            ``"isda_na"``, ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_constituents(self) -> int | None:
        """
        Number of names in the pool.

        Returns
        -------
        int | None
            The count, or ``None`` when unknown.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __eq__(self, other: object) -> bool:
        """
        Value equality (mirrors Rust ``PartialEq``).

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when every field matches.

        Notes
        -----
        This method does not raise; unrelated types compare unequal.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CdsIndexParams(index_name='CDX.NA.IG', series=42, version=1, coupon_bp=100.0, convention='isda_na', num_constituents=125)``.

        Returns
        -------
        str
            ``CdsIndexParams(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class CdsIndexConstituent:
    """
    One reference entity in a CDS index (typed wrapper for the Rust
    ``CdsIndexConstituent``): issuer credit parameters, index weight and
    default flag. Accepted by :meth:`CdsIndexBuilder.constituents` alongside
    dicts / JSON of the same shape; picklable.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CdsIndexConstituent
    >>> row = CdsIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 1 / 125)
    >>> (row.reference_entity, row.defaulted)
    ('ACME-CORP', False)
    """

    def __init__(
        self,
        reference_entity: str,
        recovery_rate: float,
        credit_curve_id: str,
        weight: float,
        defaulted: bool = False,
    ) -> None:
        """
        Describe one index constituent.

        Parameters
        ----------
        reference_entity : str
            Issuer / reference-entity name.
        recovery_rate : float
            Assumed recovery as a fraction (``0.4`` = 40%).
        credit_curve_id : str
            Hazard curve identifier for the issuer.
        weight : float
            Weight of the issuer in the index notional (``1/125`` for CDX IG).
        defaulted : bool, default False
            Whether the name has defaulted; defaulted names drop out of the
            premium leg (their settlement is reflected in ``index_factor``).

        Notes
        -----
        This constructor does not raise; validation happens when the index is priced.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndexConstituent
        >>> CdsIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 0.008).weight
        0.008
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CdsIndexConstituent:
        """
        Deserialize from the canonical JSON shape.

        Parameters
        ----------
        json : str
            JSON object with ``credit`` (``reference_entity``, ``recovery_rate``,
            ``credit_curve_id``), ``weight`` and optional ``defaulted``.

        Returns
        -------
        CdsIndexConstituent
            The parsed constituent.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsIndexConstituent
        >>> row = CdsIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 0.008)
        >>> CdsIndexConstituent.from_json(row.to_json()).credit_curve_id
        'ACME-HZD'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical JSON shape.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json` and :meth:`CdsIndexBuilder.constituents`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def reference_entity(self) -> str:
        """
        Issuer / reference-entity name.

        Returns
        -------
        str
            The issuer name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Assumed recovery as a fraction.

        Returns
        -------
        float
            Recovery in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str:
        """
        Hazard curve identifier for the issuer.

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
    def weight(self) -> float:
        """
        Weight of the issuer in the index notional.

        Returns
        -------
        float
            The weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def defaulted(self) -> bool:
        """
        Whether the name has defaulted.

        Returns
        -------
        bool
            ``True`` for defaulted names.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CdsIndexConstituent(reference_entity='ACME-CORP', recovery_rate=0.4, credit_curve_id='ACME-HZD', weight=0.008, defaulted=False)``.

        Returns
        -------
        str
            ``CdsIndexConstituent(<field>=<value>, ...)``.

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
