"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_revolving_credit/revolving_credit.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.cashflows.builder import CashFlowSchedule
from finstack_quant.core.dates import BusinessDayConvention, DayCount, StubKind, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides
from finstack_quant.valuations.instruments._typed_revolving_credit.results import EnhancedMonteCarloResult

class RevolvingCredit:
    """
    Typed wrapper for the canonical Rust ``RevolvingCredit`` instrument.

    Construct via :meth:`RevolvingCredit.builder`, the
    :meth:`RevolvingCredit.example` preset or :meth:`RevolvingCredit.from_json`.
    Every public Rust field is readable as a property; :meth:`price` /
    :meth:`metric` run the same pricer as :func:`price_instrument`. Stochastic
    facilities (a ``draw_repay_spec`` of kind ``"stochastic"``) additionally
    expose :meth:`price_with_paths` and their path-averaged
    :meth:`expected_cashflows`. Instances are accepted directly by
    :func:`price_instrument` and :meth:`AssetPool.with_instruments`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import RevolvingCredit
    >>> facility = RevolvingCredit.example()
    >>> (facility.id, facility.is_stochastic)
    ('RCF-USD-3Y', False)
    """

    @staticmethod
    def builder() -> RevolvingCreditBuilder:
        """
        Create a fluent builder (mirrors Rust ``RevolvingCredit::builder()``).

        Returns
        -------
        RevolvingCreditBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RevolvingCredit
        >>> builder = RevolvingCredit.builder()
        >>> builder.id("RCF-1") is builder
        True
        """
        ...
    @staticmethod
    def example() -> RevolvingCredit:
        """
        Canonical example: USD 50M three-year SOFR + 250bp facility with USD 10M drawn and a scheduled draw and repayment (mirrors Rust ``RevolvingCredit::example``).

        Returns
        -------
        RevolvingCredit
            The example facility.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RevolvingCredit
        >>> RevolvingCredit.example().id
        'RCF-USD-3Y'
        """
        ...
    @staticmethod
    def from_json(json: str) -> RevolvingCredit:
        """
        Deserialize a validated facility from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"revolving_credit"`` payload. The UTF-8 input must not exceed
            16 MiB. Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        RevolvingCredit
            The validated facility represented by the payload.

        Raises
        ------
        ValueError
            If the input exceeds 16 MiB, is malformed, has an unsupported
            envelope schema, carries a type other than ``"revolving_credit"``,
            or fails facility validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RevolvingCredit
        >>> facility = RevolvingCredit.example()
        >>> RevolvingCredit.from_json(facility.to_json()).id == facility.id
        True
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

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
            Canonical serde shape of the Rust ``RevolvingCredit``.

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
            ``(RevolvingCredit.from_json, (json,))``.

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
        Price this facility and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`; a
        stochastic ``draw_repay_spec`` prices by Monte Carlo, a deterministic
        one on its contractual schedule.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key.
        metrics : list[str], optional
            Metric identifiers to compute (for example ``"dv01"`` or
            ``"draw_option_cost"``).
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
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve or metric is missing from ``market``.
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
        Compute one scalar metric for this facility (e.g. ``"dv01"`` or ``"draw_option_cost"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value.

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def price_with_paths(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> EnhancedMonteCarloResult:
        """
        Run the stochastic Monte Carlo valuation and keep every path.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the discount curve, the index forward curve and
            fixings for floating facilities, and the hazard curve for
            market-anchored spread processes.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date; the simulation starts here with the current drawn
            amount.

        Returns
        -------
        EnhancedMonteCarloResult
            Present value and draw option cost estimates plus every path.

        Raises
        ------
        ValueError
            If the facility has a deterministic ``draw_repay_spec`` or fails
            validation.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the simulation fails.
        """
        ...
    def expected_cashflows(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> CashFlowSchedule:
        """
        Cashflow schedule of the facility: contractual for a deterministic ``draw_repay_spec``, path-averaged for a stochastic one.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the curves the schedule projects from.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date; flows are projected from this date.

        Returns
        -------
        CashFlowSchedule
            Dated interest, fee and principal flows from the lender's
            perspective (draws negative, repayments positive).

        Raises
        ------
        ValueError
            If the facility fails validation or the schedule cannot be built.
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
            If the instrument cannot enumerate its dependencies.
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
    def commitment(self) -> Money:
        """
        Total committed amount.

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
        Drawn balance at the simulation anchor (the later of the commitment and valuation dates), in both deterministic and stochastic mode.

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
    def issue_date(self) -> datetime.date:
        """
        Date the facility becomes available.

        Returns
        -------
        datetime.date
            The commitment date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Expiry of the commitment.

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
    def rate(self) -> dict[str, Any]:
        """
        Coupon specification as its ``RateSpec`` serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``{"fixed": {"rate": r}}`` with a decimal rate, or ``{"floating": {...}}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Interest accrual day count.

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
    def frequency(self) -> Tenor:
        """
        Payment frequency for interest and fees.

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
    def fees(self) -> dict[str, Any]:
        """
        Fee structure as its serde ``dict`` (including dated ``steps``).

        Returns
        -------
        dict[str, Any]
            ``upfront_fee``, ``commitment_fee_tiers``, ``usage_fee_tiers``,
            ``facility_fee_bp`` and ``steps``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def commitment_steps(self) -> list[dict[str, Any]]:
        """
        Scheduled commitment changes as serde ``dict`` rows (``date``, ``amount``, ``reduction_fee_bp``); empty when the commitment is flat.

        Returns
        -------
        list[dict[str, Any]]
            One row per commitment step, in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def margin_steps(self) -> list[dict[str, Any]]:
        """
        Dated margin steps as serde ``dict`` rows (``date``, ``delta_bp``); empty when the margin is flat.

        Returns
        -------
        list[dict[str, Any]]
            One row per margin step, in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def scheduled_fees(self) -> list[dict[str, Any]]:
        """
        Dated fixed fees as serde ``dict`` rows (``date``, ``amount``); empty when none are scheduled.

        Returns
        -------
        list[dict[str, Any]]
            One row per scheduled fee, in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def oid_eir(self) -> dict[str, Any] | None:
        """
        Effective-interest-rate reporting switch (``{"include_fees": bool}``), or ``None`` for the default (fees included).

        Returns
        -------
        dict[str, Any] | None
            The switch, or ``None``.

        Raises
        ------
        ValueError
            If the spec cannot be serialized.
        """
        ...
    @property
    def lc(self) -> dict[str, Any] | None:
        """
        Letter-of-credit sub-facility as its serde ``dict`` (``sublimit``, ``outstanding``, ``events``, ``fee_bp``, ``fronting_fee_bp``, ``leq``), or ``None`` without an LC sublimit.

        Returns
        -------
        dict[str, Any] | None
            The LC sub-facility, or ``None``.

        Raises
        ------
        ValueError
            If the spec cannot be serialized.
        """
        ...
    @property
    def draw_repay_spec(self) -> dict[str, Any]:
        """
        Draw/repay specification as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``{"deterministic": [...]}`` or ``{"stochastic": {...}}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def is_stochastic(self) -> bool:
        """
        Whether utilization is simulated (stochastic ``draw_repay_spec``).

        Returns
        -------
        bool
            ``True`` for a stochastic facility.

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
    def credit_curve_id(self) -> str | None:
        """
        Credit (hazard) curve identifier, or ``None``.

        Returns
        -------
        str | None
            The curve id when the facility carries credit risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Recovery rate on default, as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            The recovery fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def leq(self) -> float:
        """
        Loan-equivalent exposure: fraction of the undrawn commitment drawn at default, as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            The loan-equivalent exposure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub rule for schedule generation.

        Returns
        -------
        StubKind
            The stub kind.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business-day convention applied to payment dates (serde string, e.g. ``"modified_following"``).

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
        Holiday calendar identifier used for payment, fixing and settlement rolls, or ``None`` for weekends only.

        Returns
        -------
        str | None
            The calendar identifier, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def payment_lag_days(self) -> int:
        """
        Business days between an accrual end and its payment date.

        Returns
        -------
        int
            The payment lag; ``0`` pays on the adjusted accrual end.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int:
        """
        Business days from the valuation date to the settlement date used by quote metrics.

        Returns
        -------
        int
            The settlement lag; ``0`` settles on the valuation date.

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
        Default pricing model key from the ``Instrument`` trait.

        Returns
        -------
        str
            The model key.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry date exposed by the ``Instrument`` trait, or ``None``.

        Returns
        -------
        datetime.date | None
            The expiry date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class RevolvingCreditBuilder:
    """
    Fluent builder for :class:`RevolvingCredit`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per facility.
    Required fields: ``id``, ``commitment``, ``drawn``,
    ``issue_date``, ``maturity``, ``rate``, ``day_count``,
    ``frequency``, ``fees``, ``draw_repay_spec``,
    ``discount_curve_id`` and ``recovery_rate``. Nested specs accept a
    ``dict`` or JSON ``str`` in the Rust serde shape.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import RevolvingCredit
    >>> facility = (
    ...     RevolvingCredit
    ...     .builder()
    ...     .id("RCF-1")
    ...     .commitment(Money(50_000_000.0, Currency("USD")))
    ...     .drawn(Money(10_000_000.0, Currency("USD")))
    ...     .issue_date(datetime.date(2024, 1, 15))
    ...     .maturity(datetime.date(2027, 1, 15))
    ...     .rate(0.06)
    ...     .day_count("act_360")
    ...     .frequency("3M")
    ...     .fees({
    ...         "commitment_fee_tiers": [{"threshold": "0", "bp": "25"}],
    ...         "usage_fee_tiers": [{"threshold": "0", "bp": "10"}],
    ...         "facility_fee_bp": 5.0,
    ...     })
    ...     .draw_repay_spec({"deterministic": []})
    ...     .discount_curve_id("USD-OIS")
    ...     .recovery_rate(0.4)
    ...     .build()
    ... )
    >>> facility.is_stochastic
    False
    """

    def id(self, value: str) -> RevolvingCreditBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the facility.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def commitment(self, value: Money | float, currency: str | None = None) -> RevolvingCreditBuilder:
        """
        Set the total commitment.

        Parameters
        ----------
        value : Money | float
            Commitment; a bare number needs ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the amount is not finite, a bare number has no currency, or the
            builder was already consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def drawn(self, value: Money | float, currency: str | None = None) -> RevolvingCreditBuilder:
        """
        Set the drawn balance at the simulation anchor, the later of the commitment date and the valuation date, in both modes. Deterministic draw/repay events must be dated after that anchor.

        Parameters
        ----------
        value : Money | float
            Drawn balance; a bare number needs ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the amount is not finite, a bare number has no currency, or the
            builder was already consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> RevolvingCreditBuilder:
        """
        Set the date the facility becomes available.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Commitment date (ISO 8601 strings accepted).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date or the builder was already consumed.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> RevolvingCreditBuilder:
        """
        Set the expiry of the commitment.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date or the builder was already consumed.
        """
        ...
    def rate(self, value: float | Rate | Bps | dict[str, Any] | str) -> RevolvingCreditBuilder:
        """
        Set the facility coupon.

        Parameters
        ----------
        value : float | Rate | Bps | dict[str, Any] | str
            A bare decimal, ``Rate`` or ``Bps`` builds a fixed rate (``0.06`` = 6%); a ``dict`` or
            JSON ``str`` in the ``RateSpec`` serde shape
            (``{"fixed": {"rate": 0.06}}`` or ``{"floating": {...}}`` with a
            ``FloatingRateSpec``) is used verbatim.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def day_count(self, value: DayCount | str) -> RevolvingCreditBuilder:
        """
        Set the interest accrual day count.

        Parameters
        ----------
        value : DayCount | str
            Day count object or serde name (``"act_360"``, ``"act_365f"``,
            ``"30_360"``, ...).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the name is unknown or the builder was already consumed.
        TypeError
            If ``value`` is neither ``DayCount`` nor ``str``.
        """
        ...
    def frequency(self, value: Tenor | str) -> RevolvingCreditBuilder:
        """
        Set the payment frequency for interest and fees.

        Parameters
        ----------
        value : Tenor | str
            Tenor object or string such as ``"3M"``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the tenor cannot be parsed or the builder was already consumed.
        TypeError
            If ``value`` is neither ``Tenor`` nor ``str``.
        """
        ...
    def fees(self, value: dict[str, Any] | str) -> RevolvingCreditBuilder:
        """
        Set the fee structure from its serde shape.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``RevolvingCreditFees`` as a ``dict`` or JSON ``str``
            (``upfront_fee`` as ``None``, ``{"amount": Money-dict}`` or
            ``{"fraction_of_commitment": 0.02}``; ``commitment_fee_tiers``,
            ``usage_fee_tiers``, ``facility_fee_bp`` and the dated ``steps`` list of
            ``{"date", "commitment_delta_bp", "usage_delta_bp",
            "facility_delta_bp"}`` rows).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def commitment_steps(self, value: list[dict[str, Any]] | str) -> RevolvingCreditBuilder:
        """
        Set the scheduled commitment changes.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            Rows of ``{"date": "YYYY-MM-DD", "amount": Money-dict,
            "reduction_fee_bp": str}`` (a ``list`` of dicts or a JSON ``str``),
            each the commitment in force from its date; ``reduction_fee_bp`` is
            the one-off reduction fee on a step down, a decimal string in basis
            points of the reduced amount (default ``"0"``). Dates must
            be strictly increasing, after the commitment date and on or before
            maturity.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a row does not match the serde shape or the builder was
            already consumed; ordering and feasibility fail at ``build()``.
        """
        ...
    def scheduled_fees(self, value: list[dict[str, Any]] | str) -> RevolvingCreditBuilder:
        """
        Set the dated fixed fees (amendment, waiver, extension, consent).

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            Rows of ``{"date": "YYYY-MM-DD", "amount": Money-dict}`` (a
            ``list`` of dicts or a JSON ``str``), each paid on its date. Dates
            must lie after the commitment date and on or before maturity.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a row does not match the serde shape or the builder was
            already consumed; date and sign checks fail at ``build()``.
        """
        ...
    def oid_eir(self, value: dict[str, Any] | str | None) -> RevolvingCreditBuilder:
        """
        Set the effective-interest-rate reporting switch.

        Parameters
        ----------
        value : dict[str, Any] | str | None
            ``{"include_fees": bool}`` as a ``dict`` or JSON ``str``; ``None``
            (the default) includes fees in the effective yield.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def lc(self, value: dict[str, Any] | str | None) -> RevolvingCreditBuilder:
        """
        Set the letter-of-credit sub-facility.

        Parameters
        ----------
        value : dict[str, Any] | str | None
            ``LetterOfCreditSpec`` as a ``dict`` or JSON ``str`` with
            ``sublimit`` and ``outstanding`` (Money dicts), ``events`` (rows
            of ``{"date", "amount", "is_issue"}``), ``fee_bp`` (``None``
            accrues the floating margin), ``fronting_fee_bp`` and ``leq`` (the
            fraction of the LC face drawn at default). ``None`` removes the
            sublimit.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed; sublimit and capacity checks fail at ``build()``.
        """
        ...
    def margin_steps(self, value: list[dict[str, Any]] | str) -> RevolvingCreditBuilder:
        """
        Set the dated margin steps.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            Rows of ``{"date": "YYYY-MM-DD", "delta_bp": str}`` (a decimal
            string in basis points; a ``list`` of
            dicts or a JSON ``str``); each delta shifts the floating spread or
            the fixed rate from its date, cumulatively. Dates must be strictly
            increasing and strictly inside the facility life.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a row does not match the serde shape or the builder was
            already consumed; ordering fails at ``build()``.
        """
        ...
    def draw_repay_spec(self, value: dict[str, Any] | str) -> RevolvingCreditBuilder:
        """
        Set the draw/repay specification from its serde shape.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``DrawRepaySpec`` as a ``dict`` or JSON ``str``:
            ``{"deterministic": [{"date": ..., "amount": Money, "is_draw": bool}, ...]}``
            or ``{"stochastic": {"utilization_process": {...}, "mc_config": ...}}``.
            The estimator count, antithetic flag and seed label come from
            ``instrument_pricing_overrides.model_config`` (``mc_paths``,
            ``mc_antithetic``, ``mc_seed_scenario``).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def discount_curve_id(self, value: str) -> RevolvingCreditBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve id in the market context.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def credit_curve_id(self, value: str | None) -> RevolvingCreditBuilder:
        """
        Set (or clear) the credit curve identifier.

        Parameters
        ----------
        value : str | None
            Hazard curve id used for survival weighting; ``None`` prices
            without credit risk. A stochastic facility with a credit curve must
            use a market-anchored spread process on the same curve.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def recovery_rate(self, value: float) -> RevolvingCreditBuilder:
        """
        Set the recovery rate on default.

        Parameters
        ----------
        value : float
            Recovery fraction as a decimal in ``[0, 1]``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def leq(self, value: float) -> RevolvingCreditBuilder:
        """
        Set the loan-equivalent exposure drawn at default.

        Parameters
        ----------
        value : float
            Fraction of the undrawn commitment assumed drawn at default, as a
            decimal in ``[0, 1]`` (default ``0.0``).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def stub(self, value: StubKind | str) -> RevolvingCreditBuilder:
        """
        Set the stub rule for schedule generation.

        Parameters
        ----------
        value : StubKind | str
            Stub kind object or serde name (``"short_front"``, ``"short_back"``,
            ``"long_front"``, ``"long_back"``, ``"none"``); default
            ``"short_front"``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the name is unknown or the builder was already consumed.
        TypeError
            If ``value`` is neither ``StubKind`` nor ``str``.
        """
        ...
    def business_day_convention(self, value: BusinessDayConvention | str) -> RevolvingCreditBuilder:
        """
        Set the business-day convention for payment dates.

        Parameters
        ----------
        value : BusinessDayConvention | str
            Convention object or serde name (``"modified_following"``,
            ``"following"``, ``"preceding"``, ...); default
            ``"modified_following"``. Accrual boundaries stay unadjusted.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the name is unknown or the builder was already consumed.
        TypeError
            If ``value`` is neither ``BusinessDayConvention`` nor ``str``.
        """
        ...
    def calendar_id(self, value: str | None) -> RevolvingCreditBuilder:
        """
        Set the holiday calendar used for payment, fixing and settlement rolls.

        Parameters
        ----------
        value : str | None
            Calendar identifier such as ``"usny"``; ``None`` (the default)
            adjusts for weekends only. An unknown identifier fails at
            ``build()``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def payment_lag_days(self, value: int) -> RevolvingCreditBuilder:
        """
        Set the payment lag in business days after each accrual end.

        Parameters
        ----------
        value : int
            Business days on the facility calendar; ``0`` (the default) pays
            on the adjusted accrual end.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def settlement_days(self, value: int) -> RevolvingCreditBuilder:
        """
        Set the settlement lag used by quote metrics.

        Parameters
        ----------
        value : int
            Business days from the valuation date to settlement; ``0`` (the
            default) settles on the valuation date. The base present value is
            always anchored at the valuation date.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> RevolvingCreditBuilder:
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
        RevolvingCreditBuilder
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
    ) -> RevolvingCreditBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> RevolvingCreditBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> RevolvingCreditBuilder:
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
        RevolvingCreditBuilder
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
    def build(self) -> RevolvingCredit:
        """
        Consume the builder and validate the facility.

        Returns
        -------
        RevolvingCredit
            The validated facility.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the field), or the facility fails validation.
        """
        ...
