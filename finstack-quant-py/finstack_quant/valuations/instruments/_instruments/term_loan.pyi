"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/instruments/term_loan.rs``."""

from __future__ import annotations

from finstack_quant.valuations.instruments._pricing import PriceExtras
from finstack_quant.cashflows.builder import CashFlowSchedule
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
        extras: PriceExtras | dict[str, Any] | str | None = None,
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
    def quote_compounding(self) -> str:
        """
        Quote compounding of the discount margin and z-spread.

        Shared by pricing, quote inversion and CS01; stated independently of
        the payment frequency, never derived from it.

        Returns
        -------
        str
            ``"continuous"``, ``"simple"``, ``"semi_annual"``, ``"quarterly"``,
            ``"monthly"`` or ``"periodic(n)"``.

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
    def yield_basis(self) -> str:
        """
        Annual basis of the yield quotes and effective-interest-rate accretion.

        Separate from the accrual ``day_count`` (resolution O11): yields to
        maturity, call and worst are effective annual rates on this basis.

        Returns
        -------
        str
            ``"act_365f"`` or ``"act_360"``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
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
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> loan = (
    ...     TermLoan
    ...     .builder()
    ...     .id("TL-1")
    ...     .currency("USD")
    ...     .notional_limit(Money(10_000_000.0, "USD"))
    ...     .issue_date("2024-01-01")
    ...     .maturity("2029-01-01")
    ...     .rate(0.06)
    ...     .frequency(Tenor.quarterly())
    ...     .day_count(DayCount.ACT_360)
    ...     .yield_basis("act_360")
    ...     .discount_curve_id("USD-OIS")
    ...     .amortization({"percent_of_remaining_per_period": {"pct": 0.025}})
    ...     .quote_compounding("quarterly")
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
    def notional_limit(self, value: Money) -> TermLoanBuilder:
        """
        Set the committed notional (facility limit).

        Parameters
        ----------
        value : Money
            Committed notional (facility limit), in the loan currency.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``finstack_quant.core.money.Money``.
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
    def quote_compounding(self, value: str) -> TermLoanBuilder:
        """
        Set the quote compounding of the discount margin and z-spread (required).

        Parameters
        ----------
        value : str
            ``"continuous"``, ``"simple"``, ``"semi_annual"``, ``"quarterly"``,
            ``"monthly"`` or ``"periodic(n)"``; stated independently of the
            payment frequency.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the label is unknown or the builder was already consumed by ``build()``.
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
    def yield_basis(self, value: str) -> TermLoanBuilder:
        """
        Set the required annual basis of yield quotes and EIR accretion.

        The basis is independent of the accrual day count (resolution O11);
        there is no default, so ``build()`` raises until it is set.

        Parameters
        ----------
        value : str
            ``"act_365f"`` or ``"act_360"``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a supported basis or the builder was already
            consumed by ``build()``.
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
    def upfront_fee(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the upfront (arrangement or OID) fee paid on the issue date.

        Parameters
        ----------
        value : dict[str, object] | str
            The ``UpfrontFee`` serde form, as a ``dict`` or JSON string:
            ``{"amount": Money}`` for an absolute fee (a ``Money`` or its
            ``{"amount": "20000", "currency": "USD"}`` dict) or
            ``{"fraction_of_commitment": 0.02}`` for a decimal fraction of the
            DDTL commitment, else ``notional_limit``. The ``upfront_fee``
            getter returns this shape.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``UpfrontFee`` shape.
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
            Rust ``OidEirSpec`` in serde form (``dict`` or JSON string), e.g. ``{"include_fees": false}``.

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
