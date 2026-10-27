"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_asset_backed_facility/facility.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.cashflows.builder import DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides
from finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms import TermOutSpec
from finstack_quant.valuations.instruments._typed_structured_credit.asset_pool import AssetPool
from finstack_quant.valuations.instruments._typed_structured_credit.diagnostics import SimulationDiagnostics
from finstack_quant.valuations.instruments._typed_structured_credit.structured_credit import StructuredCredit
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows import TrancheCashflows

class AssetBackedFacility:
    """
    Typed wrapper for the canonical Rust ``AssetBackedFacility`` instrument:
    a committed warehouse line against a collateral pool.

    Advance rates, eligibility and concentration limits define the borrowing
    base; a borrowing-base coverage test diverts collateral cash to repay the
    facility when the drawn balance exceeds it. Collateral principal recycles
    while revolving and repays the facility sequentially afterwards, the
    undrawn commitment accrues a fee, and a term-out ends in a collateral
    liquidation. The engine runs a synthetic two-class structured-credit deal
    (:meth:`synthesized_deal`); :meth:`project` returns the lender's and the
    residual's flows. Construct via :meth:`builder`, :meth:`example` or
    :meth:`from_json`; instances are accepted directly by
    :func:`price_instrument`. Rates are decimals or basis points (``*_bp``),
    percentages are percent values.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    >>> facility = AssetBackedFacility.example()
    >>> (facility.id, facility.drawn.amount, facility.commitment.amount)
    ('ABF-EXAMPLE', 70000000.0, 80000000.0)
    >>> facility.borrowing_base()["borrowing_base"]["currency"]
    'USD'
    """

    @staticmethod
    def builder() -> AssetBackedFacilityBuilder:
        """
        Create a fluent builder (mirrors Rust ``AssetBackedFacility::builder()``).

        Returns
        -------
        AssetBackedFacilityBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetBackedFacility
        >>> builder = AssetBackedFacility.builder()
        >>> builder.id("WH-1") is builder
        True
        """
        ...
    @staticmethod
    def example() -> AssetBackedFacility:
        """
        Canonical example: the example CLO pool financed by a USD 80M commitment drawn USD 70M at a fixed 6%, 80% advance rate, 20% obligor limit, two-year revolving period and a 24-month term-out (mirrors Rust ``AssetBackedFacility::example``).

        Returns
        -------
        AssetBackedFacility
            The example facility.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetBackedFacility
        >>> AssetBackedFacility.example().rate
        {'fixed': {'rate': 0.06}}
        """
        ...
    @staticmethod
    def from_json(json: str) -> AssetBackedFacility:
        """
        Deserialize from a canonical ``finstack_quant.instrument/1`` envelope.

        Parameters
        ----------
        json : str
            Envelope JSON whose instrument type is ``asset_backed_facility``.

        Returns
        -------
        AssetBackedFacility
            The decoded facility.

        Raises
        ------
        ValueError
            If the JSON is malformed, carries another instrument type or
            fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetBackedFacility
        >>> facility = AssetBackedFacility.example()
        >>> AssetBackedFacility.from_json(facility.to_json()).id
        'ABF-EXAMPLE'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical instrument envelope.

        Returns
        -------
        str
            Envelope accepted by :func:`price_instrument` and :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Serde form of the facility as a Python ``dict``.

        Returns
        -------
        dict[str, Any]
            Canonical serde shape of the Rust ``AssetBackedFacility``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(AssetBackedFacility.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
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
        Price the lender's projected flows (interest, principal and commitment fees) and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context (discount and index curves plus fixings for the
            note and the collateral) or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key; ``"default"`` selects discounting.
        metrics : list[str], optional
            Metric identifiers to compute alongside the value (for example
            ``"abf_borrowing_base_cushion"``, ``"abf_facility_irr"`` or
            ``"dv01"``).
        metric_pricing_overrides : MetricPricingOverrides | dict[str, Any] | str, optional
            Metric-time overrides merged into
            ``instrument.spec.metric_pricing_overrides`` before pricing.
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
            If an input cannot be interpreted, the facility fails validation
            or a metric cannot be computed.
        KeyError
            If a required curve or metric is missing from ``market``.
        RuntimeError
            If pricing fails.
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
        Compute one scalar metric for this facility (e.g. ``"abf_borrowing_base"``, ``"abf_advance_rate_utilization"`` or ``"dv01"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (``"abf_borrowing_base"``,
            ``"abf_borrowing_base_cushion"``, ``"abf_advance_rate_utilization"``,
            ``"abf_facility_irr"``, ``"abf_residual_irr"``, ``"dv01"``,
            ``"cs01"`` ...).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value.

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown for this instrument or an input cannot
            be interpreted.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def borrowing_base(self) -> dict[str, Any]:
        """
        Borrowing base on the closing collateral.

        Returns
        -------
        dict[str, Any]
            ``BorrowingBaseReport`` serde dict with ``eligible_collateral``,
            ``concentration_excess`` and ``borrowing_base`` Money values.

        Raises
        ------
        ValueError
            If the borrowing-base rules are malformed.
        """
        ...
    def synthesized_deal(self) -> StructuredCredit:
        """
        The two-class structured-credit deal the engine runs for this facility.

        The deal carries the facility note and the residual class, the
        borrowing-base coverage test, the reinvestment window to the
        effective revolving end, the early-amortization rules and the
        term-out call.

        Returns
        -------
        StructuredCredit
            The synthetic deal.

        Raises
        ------
        ValueError
            If the facility or the synthetic deal fails validation.
        """
        ...
    def project(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> FacilityProjection:
        """
        Project the facility through the engine.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the curves and fixings the note and the
            collateral need.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date the projection starts from.

        Returns
        -------
        FacilityProjection
            Facility and residual flows, commitment fees and the period record.

        Raises
        ------
        ValueError
            If the facility fails validation or ``as_of`` is invalid.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the simulation fails.
        """
        ...
    def facility_irr(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> float:
        """
        Lender IRR: XIRR of ``-drawn`` on ``as_of`` against every projected interest, principal and fee receipt.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the curves the projection needs.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date the investment is dated on.

        Returns
        -------
        float
            Annual internal rate of return as a decimal.

        Raises
        ------
        ValueError
            If the projection fails or no rate solves.
        KeyError
            If a required curve is missing from ``market``.
        """
        ...
    def market_dependencies(self) -> dict[str, Any]:
        """
        Market-data dependencies (curves, fixings) as a dict.

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust ``MarketDependencies``.

        Raises
        ------
        ValueError
            If the collateral cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Instrument identifier.

        Returns
        -------
        str
            Stable identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def collateral(self) -> AssetPool:
        """
        Collateral pool the facility lends against.

        Returns
        -------
        AssetPool
            The pool (asset rows, rep lines or instrument collateral).

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored pool.
        """
        ...
    @property
    def borrowing_base_rules(self) -> dict[str, Any]:
        """
        Advance rates and concentration limits as their serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``BorrowingBaseRules`` with ``advance_rates`` and
            ``concentration_limits``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def commitment(self) -> Money:
        """
        Total commitment.

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
    def drawn(self) -> Money:
        """
        Amount drawn at closing.

        Returns
        -------
        Money
            Currency-tagged drawn balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def undrawn(self) -> Money:
        """
        Undrawn commitment at closing (``commitment - drawn``).

        Returns
        -------
        Money
            Currency-tagged undrawn amount.

        Raises
        ------
        ValueError
            If the commitment and the drawn amount differ in currency.
        """
        ...
    @property
    def rate(self) -> dict[str, Any]:
        """
        Facility coupon as its ``RateSpec`` serde dict.

        Returns
        -------
        dict[str, Any]
            ``{"fixed": {"rate": r}}`` with a decimal all-in rate, or
            ``{"floating": {...}}`` with a ``FloatingRateSpec``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def commitment_fee_bp(self) -> float:
        """
        Commitment fee on the undrawn commitment in basis points per annum.

        Returns
        -------
        float
            Basis points per annum.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def closing_date(self) -> datetime.date:
        """
        Closing date; the first payment date is one frequency later.

        Returns
        -------
        datetime.date
            The closing date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def revolving_end(self) -> datetime.date:
        """
        Scheduled end of the revolving period.

        Returns
        -------
        datetime.date
            The scheduled revolving end.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def effective_revolving_end(self) -> datetime.date:
        """
        Effective revolving end: the scheduled end or the earliest dated amortization event, whichever is first.

        Returns
        -------
        datetime.date
            The effective revolving end.

        Notes
        -----
        This accessor does not raise; it is derived from stored terms.
        """
        ...
    @property
    def repayment_date(self) -> datetime.date:
        """
        Final repayment date: the revolving end plus the term-out window, capped at maturity.

        Returns
        -------
        datetime.date
            The repayment date.

        Raises
        ------
        ValueError
            If the term-out window exceeds the supported calendar range.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Legal final maturity.

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
        Payment frequency of interest, fees and the borrowing-base test.

        Returns
        -------
        Tenor
            The payment frequency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day count of the facility interest and the commitment fee.

        Returns
        -------
        DayCount
            The accrual convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Payment calendar identifier, or ``None``.

        Returns
        -------
        str | None
            Holiday calendar id (e.g. ``"nyse"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def amortization_events(self) -> list[dict[str, Any]]:
        """
        Events that end revolving early, as ``AmortizationEvent`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            Each ``{"kind": "date", "date": ...}``,
            ``{"kind": "cumulative_loss", "max_cumulative_loss": ...}`` or
            ``{"kind": "excess_spread", "min_excess_spread_3m": ...}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def term_out(self) -> dict[str, Any] | None:
        """
        Term-out window as its serde dict (``{"months": ...}``), or ``None``.

        Returns
        -------
        dict[str, Any] | None
            Months after the revolving end at which the collateral is
            liquidated.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def liquidation_price_pct(self) -> float | None:
        """
        Collateral liquidation price at the term-out end in percent of par, or ``None`` for par.

        Returns
        -------
        float | None
            Percent of par.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fees(self) -> dict[str, Any] | None:
        """
        Transaction fees paid ahead of the facility's interest as the ``DealFees`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when the facility carries no fees.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def draws(self) -> list[dict[str, Any]]:
        """
        Scheduled draws after closing as ``DrawEvent`` serde dicts (``date``, ``amount``).

        Returns
        -------
        list[dict[str, Any]]
            Empty when the line is fully funded at closing.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def readvance_to_borrowing_base(self) -> bool:
        """
        Whether the line is re-advanced up to the borrowing base each revolving period.

        Returns
        -------
        bool
            ``True`` when re-advances apply.

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
    def credit_model(self) -> dict[str, Any]:
        """
        Collateral behavior as its ``CreditModelConfig`` serde ``dict``.

        Returns
        -------
        dict[str, Any]
            Prepayment, default, recovery and delinquency assumptions.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def prepayment_spec(self) -> PrepaymentModelSpec:
        """
        Deterministic prepayment model of the collateral.

        Returns
        -------
        PrepaymentModelSpec
            The typed spec.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored spec.
        """
        ...
    @property
    def default_spec(self) -> DefaultModelSpec:
        """
        Deterministic default model of the collateral.

        Returns
        -------
        DefaultModelSpec
            The typed spec.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored spec.
        """
        ...
    @property
    def recovery_spec(self) -> RecoveryModelSpec:
        """
        Recovery model of the collateral.

        Returns
        -------
        RecoveryModelSpec
            The typed spec.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored spec.
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
        Scenario-selection attributes.

        Returns
        -------
        Attributes
            The attribute map.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Default pricing model key from the ``Instrument`` trait (``"discounting"``).

        Returns
        -------
        str
            The model key.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AssetBackedFacilityBuilder:
    """
    Fluent builder for :class:`AssetBackedFacility`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per facility.
    Required fields: ``id``, ``collateral``, ``borrowing_base_rules``,
    ``commitment``, ``drawn``, ``rate``, ``closing_date``,
    ``revolving_end``, ``maturity``, ``frequency`` and ``discount_curve_id``.
    ``day_count`` defaults to Act/360 and ``commitment_fee_bp`` to zero. Nested
    specs accept a ``dict`` or JSON ``str`` in the Rust serde shape.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    >>> pool = AssetBackedFacility.example().collateral
    >>> facility = (
    ...     AssetBackedFacility
    ...     .builder()
    ...     .id("WH-1")
    ...     .collateral(pool)
    ...     .borrowing_base_rules({"advance_rates": [{"asset_class": "*", "rate": 0.75}]})
    ...     .commitment(Money(80_000_000.0, Currency("USD")))
    ...     .drawn(60_000_000.0, currency="USD")
    ...     .rate(0.055)
    ...     .commitment_fee_bp(50.0)
    ...     .closing_date(datetime.date(2024, 1, 15))
    ...     .revolving_end(datetime.date(2026, 1, 15))
    ...     .maturity(datetime.date(2030, 1, 15))
    ...     .frequency("3M")
    ...     .calendar_id("nyse")
    ...     .term_out(24)
    ...     .discount_curve_id("USD-OIS")
    ...     .build()
    ... )
    >>> (facility.id, facility.undrawn.amount, facility.repayment_date)
    ('WH-1', 20000000.0, datetime.date(2028, 1, 15))
    """

    def id(self, value: str) -> AssetBackedFacilityBuilder:
        """
        Set the facility identifier.

        Parameters
        ----------
        value : str
            Stable facility identifier.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def collateral(self, value: AssetPool) -> AssetBackedFacilityBuilder:
        """
        Set the collateral pool.

        Parameters
        ----------
        value : AssetPool
            Collateral pool (asset rows, rep lines or instrument collateral).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def borrowing_base_rules(self, value: dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the advance rates, eligibility and concentration limits.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``BorrowingBaseRules`` serde object: ``advance_rates`` (each with
            an ``asset_class`` wire name or ``"*"``, a decimal ``rate`` and an
            optional ``eligibility``) and ``concentration_limits`` (each with
            ``scope`` ``"obligor"`` / ``"industry"`` / ``"asset_class"`` and a
            percent ``max_pct``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``BorrowingBaseRules`` shape or
            the builder was already consumed.
        """
        ...
    def commitment(self, value: Money | float, currency: str | None = None) -> AssetBackedFacilityBuilder:
        """
        Set the total commitment.

        Parameters
        ----------
        value : Money | float
            Commitment; a bare amount is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code for a bare amount (ignored for ``Money``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a bare amount has no currency or the builder was already
            consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def drawn(self, value: Money | float, currency: str | None = None) -> AssetBackedFacilityBuilder:
        """
        Set the amount drawn at closing.

        Parameters
        ----------
        value : Money | float
            Drawn balance (at most the commitment and below the collateral);
            a bare amount is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code for a bare amount (ignored for ``Money``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a bare amount has no currency or the builder was already
            consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def rate(self, value: float | Rate | Bps | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the facility coupon.

        Parameters
        ----------
        value : float | Rate | Bps | dict[str, Any] | str
            A bare decimal, ``Rate`` or ``Bps`` builds a fixed all-in rate (``0.06`` = 6%); a
            ``dict`` or JSON ``str`` in the ``RateSpec`` serde shape
            (``{"fixed": {"rate": 0.06}}`` or ``{"floating": {...}}`` with a
            ``FloatingRateSpec``) is used verbatim.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def commitment_fee_bp(self, value: float) -> AssetBackedFacilityBuilder:
        """
        Set the commitment fee on the undrawn commitment.

        Parameters
        ----------
        value : float
            Basis points per annum (``50.0`` = 0.50%).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not finite or the builder was already consumed.
        """
        ...
    def closing_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> AssetBackedFacilityBuilder:
        """
        Set the closing date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Closing date; the first payment date is one frequency later.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the date is invalid or the builder was already consumed.
        """
        ...
    def revolving_end(
        self, value: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> AssetBackedFacilityBuilder:
        """
        Set the scheduled end of the revolving period.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Revolving end; after it, collateral principal repays the facility.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the date is invalid or the builder was already consumed.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> AssetBackedFacilityBuilder:
        """
        Set the legal final maturity.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Legal final maturity.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the date is invalid or the builder was already consumed.
        """
        ...
    def frequency(self, value: Tenor | str) -> AssetBackedFacilityBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency of interest, fees and the borrowing-base test
            (``Tenor`` or a string such as ``"3M"``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the string is not a tenor or the builder was already consumed.
        """
        ...
    def day_count(self, value: DayCount | str) -> AssetBackedFacilityBuilder:
        """
        Set the accrual day count.

        Parameters
        ----------
        value : DayCount | str
            Accrual convention (``DayCount`` or a name such as
            ``"act_360"``); Act/360 when never set.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the string is not a day count or the builder was already
            consumed.
        """
        ...
    def calendar_id(self, value: str) -> AssetBackedFacilityBuilder:
        """
        Set the payment calendar.

        Parameters
        ----------
        value : str
            Holiday calendar identifier (e.g. ``"nyse"``); required for
            pricing.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def amortization_events(self, value: list[dict[str, Any]] | str) -> AssetBackedFacilityBuilder:
        """
        Set the events that end revolving early.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            ``AmortizationEvent`` objects: ``{"kind": "date", "date": ...}``,
            ``{"kind": "cumulative_loss", "max_cumulative_loss": ...}`` or
            ``{"kind": "excess_spread", "min_excess_spread_3m": ...}``.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the event shape or the builder was
            already consumed.
        """
        ...
    def term_out(self, value: int | TermOutSpec | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the term-out window after revolving.

        Parameters
        ----------
        value : int | TermOutSpec | dict[str, Any] | str
            Months after the revolving end at which the remaining collateral
            is liquidated to repay the facility, or a ``TermOutSpec`` object / dict.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...

    def fees(self, value: dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the transaction fees paid through the waterfall ahead of the
        facility's interest.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``DealFees`` serde object (``trustee_fee`` Money per annum,
            ``senior_mgmt_fee_bp``, ``subordinated_mgmt_fee_bp``,
            ``servicing_fee_bp``, optional ``master_servicer_fee_bp`` /
            ``workout_fee_pct`` / ``liquidation_fee_pct`` /
            ``special_servicer_fee_bp`` / ``incentive_fee``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``DealFees`` shape or this builder
            was already consumed by ``build``.
        """
        ...

    def draws(self, value: list[dict[str, Any]] | str) -> AssetBackedFacilityBuilder:
        """
        Set the scheduled draws after closing.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            ``DrawEvent`` serde objects ``{"date": "2025-01-01", "amount":
            {"amount": "10000000", "currency": "USD"}}``, ascending by date;
            each is applied on the first payment date at or after its date.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``DrawEvent`` shape or this
            builder was already consumed by ``build``.
        """
        ...

    def readvance_to_borrowing_base(self, value: bool) -> AssetBackedFacilityBuilder:
        """
        Set whether the line is re-advanced up to the borrowing base each
        revolving period.

        Parameters
        ----------
        value : bool
            ``True`` draws ``min(commitment, borrowing base) − balance`` every
            revolving period.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by ``build``.
        """
        ...
    def liquidation_price_pct(self, value: float) -> AssetBackedFacilityBuilder:
        """
        Set the collateral liquidation price at the term-out end.

        Parameters
        ----------
        value : float
            Percent of par (``100.0`` = par).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def discount_curve_id(self, value: str) -> AssetBackedFacilityBuilder:
        """
        Set the discount curve.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def credit_model(self, value: dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Replace the whole collateral behavior model.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CreditModelConfig`` serde object; the per-field setters
            (:meth:`prepayment_spec` ...) then modify it.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def prepayment_spec(self, value: PrepaymentModelSpec | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the deterministic prepayment model of the collateral.

        Parameters
        ----------
        value : PrepaymentModelSpec | dict[str, Any] | str
            Typed spec or its serde form.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def default_spec(self, value: DefaultModelSpec | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the deterministic default model of the collateral.

        Parameters
        ----------
        value : DefaultModelSpec | dict[str, Any] | str
            Typed spec or its serde form.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def recovery_spec(self, value: RecoveryModelSpec | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the recovery model of the collateral.

        Parameters
        ----------
        value : RecoveryModelSpec | dict[str, Any] | str
            Typed spec or its serde form.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> AssetBackedFacilityBuilder:
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
        AssetBackedFacilityBuilder
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
    ) -> AssetBackedFacilityBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> AssetBackedFacilityBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> AssetBackedFacilityBuilder:
        """
        Set scenario-selection attributes.

        Parameters
        ----------
        value : Attributes | dict | None
            Attribute bag: an ``Attributes`` or its serde ``dict`` form
            (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
            ``None`` clears it.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
        """
        ...
    def build(self) -> AssetBackedFacility:
        """
        Consume the builder and validate the facility.

        Returns
        -------
        AssetBackedFacility
            The validated facility.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the field), or the facility fails validation.
        """
        ...

class FacilityProjection:
    """
    Facility and residual projection (:meth:`AssetBackedFacility.project`'s
    return value): the lender's interest and principal, the commitment
    fees, the residual class's flows and the synthetic deal's period record.

    Examples
    --------
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    >>> facility = AssetBackedFacility.example()
    >>> as_of = facility.closing_date
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> projection = facility.project(market, as_of)
    >>> list(projection.to_dataframe().columns)
    ['date', 'interest', 'principal', 'commitment_fee', 'draw', 'lender_total', 'residual']
    """

    @staticmethod
    def from_json(json: str) -> FacilityProjection:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``FacilityProjection`` (the shape ``to_json`` writes).

        Returns
        -------
        FacilityProjection
            The decoded projection.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FacilityProjection
        >>> try:
        ...     FacilityProjection.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded projection.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the projection.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(FacilityProjection.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    @property
    def facility(self) -> TrancheCashflows:
        """
        Interest and principal paid to the facility note.

        Returns
        -------
        TrancheCashflows
            The note's flows.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored flows.
        """
        ...
    @property
    def residual(self) -> TrancheCashflows:
        """
        Cash paid to the residual class.

        Returns
        -------
        TrancheCashflows
            The residual's flows.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored flows.
        """
        ...
    @property
    def commitment_fees(self) -> list[tuple[datetime.date, Money]]:
        """
        Commitment fee per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(payment date, fee)`` pairs in date order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def draws(self) -> list[tuple[datetime.date, Money]]:
        """
        Lender draws applied (scheduled draws and re-advances) per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs; outflows in :attr:`lender_cashflows`.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
    @property
    def lender_cashflows(self) -> list[tuple[datetime.date, Money]]:
        """
        Every cashflow to the lender per date: interest, principal and fees less draws.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(payment date, amount)`` pairs in date order; a draw is a negative amount.

        Raises
        ------
        ValueError
            If a date cannot be converted or two flows on one date differ in currency.
        """
        ...
    @property
    def diagnostics(self) -> SimulationDiagnostics:
        """
        Per-period record of the synthetic deal.

        Returns
        -------
        SimulationDiagnostics
            Pool, collections, cash-account and coverage-test record.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored record.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per payment date as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``interest``, ``principal``,
        ``commitment_fee``, ``draw`` (lender advances), ``lender_total`` and
        ``residual`` (cash to the residual class), all in currency units. The
        component columns pivot the flow lists by date; ``lender_total`` is the
        Rust ``lender_cashflows`` amount on that date (the flows behind the
        ``lender_cashflows`` property and the facility IRR).

        Returns
        -------
        pandas.DataFrame
            The projection in date order.

        Raises
        ------
        ValueError
            If any facility, residual, fee or draw amount has a different
            currency from the facility. Empty results preserve all columns.
        """
        ...
