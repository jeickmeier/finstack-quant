"""Typed money-market deposit declarations."""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Any

from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class Deposit:
    """Fixed-rate money-market deposit using explicit accrual dates.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Deposit
    >>> Deposit.example().discount_curve_id
    'USD-OIS'
    """

    @staticmethod
    def builder() -> DepositBuilder:
        """Create a deposit builder with no required fields populated.

        Returns
        -------
        DepositBuilder
            Builder for specifying notional, dates and accrual conventions.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Deposit
        >>> from finstack_quant.core.money import Money
        >>> builder = Deposit.builder().id("DEP").notional(Money(100, "USD"))
        >>> builder = builder.start_date("2025-01-15").maturity("2025-04-15").day_count("act_360")
        >>> builder.discount_curve_id("USD-OIS").build().id
        'DEP'
        """
        ...

    def to_builder(self) -> DepositBuilder:
        """Copy the deposit's terms into a new editable builder.

        Returns
        -------
        DepositBuilder
            A builder retaining exact rates, dates, conventions, attributes,
            and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> Deposit:
        """Create the canonical six-month USD deposit fixture.

        Returns
        -------
        Deposit
            A USD 100,000 deposit paying a 4.5 percent contractual simple rate.

        Raises
        ------
        ValueError
            If the crate's example instrument is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Deposit
        >>> Deposit.example().id
        'DEP-USD-6M'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Deposit:
        """
        Deserialize a validated Deposit from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"interest_rate_deposit"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Deposit
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Deposit
        >>> deposit = Deposit.example()
        >>> Deposit.from_json(deposit.to_json()).id == deposit.id
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
            and :meth:`Deposit.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the deposit spec as a plain Python ``dict``.

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
        Canonical model key used when ``model="default"`` is passed to ``price_instrument``.

        Returns
        -------
        str
            The registered deposit model key ``"discounting"``.

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
    def notional(self) -> Money:
        """Contractual principal.

        Returns
        -------
        Money
            Positive principal and deposit currency.

        Notes
        -----
        This accessor does not raise for a valid stored deposit.
        """
        ...

    @property
    def start_date(self) -> datetime.date:
        """Contractual interest accrual start.

        Returns
        -------
        datetime.date
            Accrual start before applying calendar adjustments.

        Raises
        ------
        ValueError
            If the Rust date is outside Python's supported date range.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """Contractual date at maturity.

        Returns
        -------
        datetime.date
            Maturity before applying calendar adjustments.

        Raises
        ------
        ValueError
            If the Rust date is outside Python's supported date range.
        """
        ...

    @property
    def fixed_rate(self) -> Decimal | None:
        """Exact contractual deposit rate.

        Returns
        -------
        Decimal | None
            Annualized simple decimal rate (0.045 for 4.5 percent), or None when unset.

        Notes
        -----
        This accessor does not raise for a valid stored deposit.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """Identifier used to find the discount curve.

        Returns
        -------
        str
            Market curve identifier used for valuation and par-rate retrieval.

        Notes
        -----
        This accessor does not raise for a valid stored deposit.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """Interest accrual day-count convention.

        Returns
        -------
        DayCount
            Day-count convention used to determine simple interest.

        Notes
        -----
        This getter does not raise.
        """
        ...

    @property
    def business_day_convention(self) -> str:
        """Business-day adjustment convention.

        Returns
        -------
        str
            Wire label such as modified_following.

        Raises
        ------
        ValueError
            If the convention cannot be serialized.
        """
        ...

    @property
    def calendar_id(self) -> str | None:
        """Calendar used for business-day adjustment.

        Returns
        -------
        str | None
            Registered calendar identifier or None when dates are unadjusted.

        Notes
        -----
        This getter does not raise.
        """
        ...

    @property
    def expiry(self) -> datetime.date | None:
        """Effective maturity under the deposit's business-day convention.

        Returns
        -------
        datetime.date or None
            Adjusted maturity, or None if the effective date cannot be resolved.

        Raises
        ------
        ValueError
            If the effective date cannot be represented by Python.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Describe how pickle reconstructs the deposit from canonical JSON.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and the serialized deposit argument.

        Raises
        ------
        ValueError
            If the stored deposit cannot be serialized.
        """
        ...

class DepositBuilder:
    """Fluent authoring of a deposit using explicit financial terms.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import Deposit
    >>> builder = Deposit.builder().id("DEP").notional(Money(100_000, "USD"))
    >>> builder = builder.start_date("2025-01-15").maturity("2025-04-15")
    >>> builder = builder.day_count("act_360").fixed_rate(Decimal("0.0525")).discount_curve_id("USD-OIS")
    >>> builder.build().id
    'DEP'
    """

    def id(self, value: str) -> DepositBuilder:
        """Set the deposit's id.

        Parameters
        ----------
        value : str
            Unique deposit identifier used for pricing results and positions.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def notional(self, value: Money) -> DepositBuilder:
        """Set the deposit's notional.

        Parameters
        ----------
        value : Money
            Strictly positive principal in the deposit currency.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def start_date(self, value: datetime.date | str) -> DepositBuilder:
        """Set the deposit's start date.

        Parameters
        ----------
        value : datetime.date | str
            Accrual start, normally the settlement date, as a Python date or ISO text.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def maturity(self, value: datetime.date | str) -> DepositBuilder:
        """Set the deposit's maturity.

        Parameters
        ----------
        value : datetime.date | str
            Contractual maturity after the accrual start, as a Python date or ISO text.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def fixed_rate(self, value: Decimal) -> DepositBuilder:
        """Set the deposit's fixed rate.

        Parameters
        ----------
        value : Decimal
            Exact annualized contractual simple rate in decimal form, such as 0.045 for 4.5 percent. Required for cash flows and pricing.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def day_count(self, value: DayCount | str) -> DepositBuilder:
        """Set the deposit's day count.

        Parameters
        ----------
        value : DayCount | str
            Explicit day-count convention used to calculate the interest accrual fraction.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def discount_curve_id(self, value: str) -> DepositBuilder:
        """Set the deposit's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve identifier looked up in the market at pricing time.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def business_day_convention(self, value: str) -> DepositBuilder:
        """Set the deposit's business day convention.

        Parameters
        ----------
        value : str
            Business-day adjustment such as modified_following, the default, or following.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def calendar_id(self, value: str) -> DepositBuilder:
        """Set the deposit's calendar id.

        Parameters
        ----------
        value : str
            Registered holiday calendar identifier used for business-day adjustments.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def attributes(self, value: Attributes | dict[str, object]) -> DepositBuilder:
        """Set the deposit's attributes.

        Parameters
        ----------
        value : Attributes | dict[str, object]
            Scenario selection and aggregation attributes stored with the deposit.

        Returns
        -------
        DepositBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def build(self) -> Deposit:
        """
        Build the validated deposit.

        Runs the same validation as the Rust ``DepositBuilder::build`` (structural
        invariants only); pricing-time checks run in :func:`price_instrument`.

        Returns
        -------
        Deposit
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and every missing field, e.g.
            ``DepositBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> DepositBuilder:
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
        DepositBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> DepositBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        DepositBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> DepositBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        DepositBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
