"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_credit/cds.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides
from finstack_quant.valuations.instruments._typed_legs import PremiumLegSpec, ProtectionLegSpec

class CreditDefaultSwap:
    """
    Single-name credit default swap (typed wrapper for the canonical Rust
    ``CreditDefaultSwap``).

    Follows the ISDA CDS Standard Model conventions: quarterly IMM premium
    dates, ACT/360, accrual-on-default and points-upfront quoting via
    :meth:`CreditDefaultSwapBuilder.upfront`. ``convention="isda_na"`` is the
    SNAC / post-Big-Bang standard (and the Rust default);
    ``valuation_convention`` defaults to Bloomberg CDSW clean principal.
    Construct via :meth:`CreditDefaultSwap.builder`,
    :meth:`CreditDefaultSwap.example` or :meth:`CreditDefaultSwap.from_json`.
    Every public Rust field is readable as a property and
    :meth:`CreditDefaultSwap.price` / :meth:`CreditDefaultSwap.metric` run the
    same pricer as :func:`price_instrument`. The ``"cs01"`` and
    ``"bucketed_cs01"`` metrics rebootstrap a quote-backed hazard curve from
    its stored calibration recipe.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
    >>> cds = CreditDefaultSwap.example()
    >>> (cds.id, cds.side, cds.convention, cds.doc_clause_effective)
    ('CDS-CORP-5Y', 'pay', 'isda_na', 'xr14')
    """

    @staticmethod
    def builder() -> CreditDefaultSwapBuilder:
        """
        Create a fluent builder (mirrors Rust ``CreditDefaultSwap::builder()``).

        Returns
        -------
        CreditDefaultSwapBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
        >>> builder = CreditDefaultSwap.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CreditDefaultSwap:
        """
        Deserialize a validated CreditDefaultSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"credit_default_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CreditDefaultSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
        >>> CreditDefaultSwap.from_json(CreditDefaultSwap.example().to_json()).id
        'CDS-CORP-5Y'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CreditDefaultSwap.from_json`.

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
        Compute one scalar metric for this instrument (e.g. ``"cs01"`` or ``"par_spread"``).

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
    def example() -> CreditDefaultSwap:
        """
        Canonical 5-year USD 10,000,000 investment-grade payer CDS (mirrors Rust
        ``CreditDefaultSwap::example``): ``isda_na`` convention, 100bp running
        spread, 40% recovery, curves ``USD-OIS`` / ``CORP-HAZARD``, premium
        2024-03-20 to 2029-03-20.

        Returns
        -------
        CreditDefaultSwap
            The example CDS.

        Raises
        ------
        ValueError
            If the example instrument fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
        >>> CreditDefaultSwap.example().protection_leg.credit_curve_id
        'CORP-HAZARD'
        """
        ...
    def par_spread(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Par spread implied by the market, in basis points (mirrors Rust
        ``CreditDefaultSwap::par_spread``): the running spread at which the
        contract is worth zero under this CDS's valuation convention, premium
        schedule, discount curve, hazard curve and recovery assumption.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves named by the CDS.
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
            If the curve recovery metadata conflicts with the contract recovery.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional amount of protection.

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
        ISDA regional convention (serde name).

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
        Premium (fixed coupon) leg specification.

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
        Protection (default-contingent) leg specification.

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
    def valuation_convention(self) -> str:
        """
        Valuation presentation convention (serde name).

        Returns
        -------
        str
            ``"bloomberg_cdsw_clean"`` (default), ``"bloomberg_cdsw_clean_full_premium"``, ``"isda_dirty"`` or ``"quant_lib_isda_parity"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def upfront(self) -> tuple[datetime.date, Money] | None:
        """
        Points-upfront payment as ``(payment_date, amount)``; positive means the protection buyer pays.

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
    def doc_clause(self) -> str | None:
        """
        Explicit ISDA documentation clause (serde name).

        Returns
        -------
        str | None
            The clause, or ``None`` when derived from the convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def doc_clause_effective(self) -> str:
        """
        Effective documentation clause after convention-based resolution (mirrors Rust ``doc_clause_effective``).

        Returns
        -------
        str
            ``"xr14"`` for ``isda_na`` / ``isda_as`` / ``custom``, ``"mm14"`` for ``isda_eu``, or the explicit clause resolved to its 2014 variant.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection_effective_date(self) -> datetime.date | None:
        """
        Protection effective date for a forward-starting CDS.

        Returns
        -------
        datetime.date | None
            The date, or ``None`` when protection starts with the premium leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection_start(self) -> datetime.date:
        """
        Date protection starts (mirrors Rust ``protection_start``).

        Returns
        -------
        datetime.date
            ``protection_effective_date`` when set, else the premium start date.

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
        Python-style rendering of the key economics, e.g. ``CreditDefaultSwap(id='CDS-CORP-5Y', side='pay', notional=Money(10000000.0, 'USD'), coupon_bp=100, ...)``.

        Returns
        -------
        str
            ``CreditDefaultSwap(<field>=<value>, ...)``.

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

class CreditDefaultSwapBuilder:
    """
    Fluent builder for :class:`CreditDefaultSwap`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``side``,
    ``convention``, ``premium``, ``protection``. ``valuation_convention``
    defaults to ``"bloomberg_cdsw_clean"``; ``upfront``, ``doc_clause``,
    ``protection_effective_date`` and ``margin_spec`` are optional.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     CreditDefaultSwap,
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
    >>> protection = ProtectionLegSpec("ACME-CDS", 0.4, 3)
    >>> cds = (
    ...     CreditDefaultSwap
    ...     .builder()
    ...     .id("CDS-1")
    ...     .notional(Money(10_000_000.0, Currency("USD")))
    ...     .side("pay")
    ...     .convention("isda_na")
    ...     .premium_leg(premium)
    ...     .protection_leg(protection)
    ...     .upfront((datetime.date(2024, 6, 25), Money(-250_000.0, Currency("USD"))))
    ...     .build()
    ... )
    >>> cds.upfront[1].amount
    -250000.0
    """

    def id(self, value: str) -> CreditDefaultSwapBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the CDS.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> CreditDefaultSwapBuilder:
        """
        Set the notional amount.

        Parameters
        ----------
        value : Money
            Notional amount of protection.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def side(self, value: Literal["pay", "receive"]) -> CreditDefaultSwapBuilder:
        """
        Set the protection buyer/seller perspective.

        Parameters
        ----------
        value : Literal["pay", "receive"]
            ``"pay"`` to buy protection (pay premium), ``"receive"`` to sell protection (receive premium).

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized side.
        """
        ...
    def convention(self, value: Literal["isda_na", "isda_eu", "isda_as", "custom"]) -> CreditDefaultSwapBuilder:
        """
        Set the ISDA regional convention.

        Parameters
        ----------
        value : Literal["isda_na", "isda_eu", "isda_as", "custom"]
            ``"isda_na"`` is the SNAC / post-Big-Bang North American standard (ACT/360, quarterly IMM, T+3); ``"isda_eu"`` the European standard (T+1, TARGET2); ``"isda_as"`` Asian (ACT/365F, Tokyo); ``"custom"`` for a manually configured convention.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not one of the accepted strings (the message lists them).
        """
        ...
    def premium_leg(self, value: PremiumLegSpec) -> CreditDefaultSwapBuilder:
        """
        Set the premium leg specification.

        Parameters
        ----------
        value : PremiumLegSpec
            Premium leg specification.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def protection_leg(self, value: ProtectionLegSpec) -> CreditDefaultSwapBuilder:
        """
        Set the protection leg specification.

        Parameters
        ----------
        value : ProtectionLegSpec
            Protection leg specification.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def valuation_convention(
        self,
        value: Literal[
            "bloomberg_cdsw_clean", "bloomberg_cdsw_clean_full_premium", "isda_dirty", "quant_lib_isda_parity"
        ],
    ) -> CreditDefaultSwapBuilder:
        """
        Set the valuation presentation convention.

        Parameters
        ----------
        value : Literal["bloomberg_cdsw_clean", "bloomberg_cdsw_clean_full_premium", "isda_dirty", "quant_lib_isda_parity"]
            ``"bloomberg_cdsw_clean"`` (default) reports Bloomberg CDSW clean principal; ``"isda_dirty"`` the academic ISDA dirty PV; ``"quant_lib_isda_parity"`` reproduces QuantLib ``IsdaCdsEngine``.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized convention.
        """
        ...
    def upfront(
        self, value: tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]
    ) -> CreditDefaultSwapBuilder:
        """
        Set the points-upfront payment (the standard post-Big-Bang quote).

        Parameters
        ----------
        value : tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]
            ``(payment_date, amount)``; a payment from protection buyer to seller (positive: buyer pays, negative: seller pays). The currency must match the notional.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``(date, Money)`` pair.
        """
        ...
    def doc_clause(
        self,
        value: Literal["cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz", "custom"],
    ) -> CreditDefaultSwapBuilder:
        """
        Set the ISDA documentation clause for restructuring credit events.

        Parameters
        ----------
        value : Literal["cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz", "custom"]
            One of the four 2014 ISDA restructuring elections, a regional ISDA corporate default, or ``"custom"``. If never set, the effective clause is derived from the CDS convention (see :attr:`CreditDefaultSwap.doc_clause_effective`).

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized documentation clause.
        """
        ...
    def protection_effective_date(
        self, value: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> CreditDefaultSwapBuilder:
        """
        Set the protection effective date for a forward-starting CDS.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Date on which credit protection begins; must satisfy ``premium.start <= value <= premium.end`` (ISO 8601 strings accepted).

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def margin_spec(self, value: dict[str, object] | str) -> CreditDefaultSwapBuilder:
        """
        Set the OTC margin (CSA / initial-margin) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OtcMarginSpec`` in serde form (``dict`` or JSON string); cleared CDS use the ``cleared`` form, bilateral CDS need a SIMM credit classification.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as an ``OtcMarginSpec``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CreditDefaultSwapBuilder:
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
        CreditDefaultSwapBuilder
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
    ) -> CreditDefaultSwapBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CreditDefaultSwapBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> CreditDefaultSwapBuilder:
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
        CreditDefaultSwapBuilder
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
        ``CreditDefaultSwapBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CreditDefaultSwapBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CreditDefaultSwap:
        """
        Build the validated CDS.

        Runs only the Rust ``CreditDefaultSwapBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`CreditDefaultSwap.price`.

        Returns
        -------
        CreditDefaultSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CreditDefaultSwapBuilder: missing required field 'id'``), or the instrument
            fails validation (recovery outside ``[0, 1]``, upfront currency mismatch, protection date outside the premium period).
        """
        ...
