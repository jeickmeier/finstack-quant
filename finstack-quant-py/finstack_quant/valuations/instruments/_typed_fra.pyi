"""Typed forward rate agreement declarations."""

from __future__ import annotations
from finstack_quant.valuations.instruments._pricing import PriceExtras

import datetime
import pandas as pd
from decimal import Decimal
from typing import Any

from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class ForwardRateAgreement:
    """Forward rate agreement paying or receiving a contractual fixed rate.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ForwardRateAgreement
    >>> ForwardRateAgreement.example().discount_curve_id
    'USD-OIS'
    """

    @staticmethod
    def builder() -> ForwardRateAgreementBuilder:
        """Create a forward rate agreement builder with no required fields populated.

        Returns
        -------
        ForwardRateAgreementBuilder
            Builder for specifying notional, dates and accrual conventions.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ForwardRateAgreement
        >>> from finstack_quant.core.money import Money
        >>> from decimal import Decimal
        >>> builder = ForwardRateAgreement.builder().id("FRA").notional(Money(100, "USD"))
        >>> builder = builder.start_date("2025-01-15").maturity("2025-04-15").day_count("act_360")
        >>> builder = builder.fixed_rate(Decimal("0.045")).discount_curve_id("USD-OIS").forward_curve_id("USD-SOFR-3M")
        >>> builder.side("receive").reset_lag_days(2).build().id
        'FRA'
        """
        ...

    def to_builder(self) -> ForwardRateAgreementBuilder:
        """Copy the forward rate agreement's terms into a new editable builder.

        Returns
        -------
        ForwardRateAgreementBuilder
            A builder retaining exact rates, dates, conventions, attributes,
            and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> ForwardRateAgreement:
        """Create the canonical USD 3x6 forward rate agreement fixture.

        Returns
        -------
        ForwardRateAgreement
            A USD 10 million 3x6 contract receiving 4.5 percent fixed.

        Raises
        ------
        ValueError
            If the crate's example instrument is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ForwardRateAgreement
        >>> ForwardRateAgreement.example().id
        'FRA-3X6-USD'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> ForwardRateAgreement:
        """
        Deserialize a validated ForwardRateAgreement from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"interest_rate_forward rate agreement"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        ForwardRateAgreement
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ForwardRateAgreement
        >>> forward rate agreement = ForwardRateAgreement.example()
        >>> ForwardRateAgreement.from_json(forward rate agreement.to_json()).id == forward rate agreement.id
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
            and :meth:`ForwardRateAgreement.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the forward rate agreement spec as a plain Python ``dict``.

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
        Price this forward rate agreement and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hull_white_1f"``, ...).
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
            Cash flow valuation requires the forward rate agreement’s contractual fixed rate.
        KeyError
            If the discount curve or requested metric is missing from ``market``.
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
        Compute one scalar metric for this forward rate agreement (e.g. ``"dv01"``).

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
            If the discount curve is missing from ``market``.
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
            The registered forward rate agreement model key ``"discounting"``.

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
            Positive principal and forward rate agreement currency.

        Notes
        -----
        This accessor does not raise for a valid stored forward rate agreement.
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
    def fixed_rate(self) -> Decimal:
        """Exact contractual forward rate agreement rate.

        Returns
        -------
        Decimal
            Exact annualized simple decimal rate, e.g. 0.045 for 4.5 percent.

        Notes
        -----
        This accessor does not raise for a valid stored forward rate agreement.
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
        This accessor does not raise for a valid stored forward rate agreement.
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
    def fixing_business_day_convention(self) -> str | None:
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
    def fixing_calendar_id(self) -> str | None:
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
    def fixing_date(self) -> datetime.date | None:
        """Read the contract's fixing date.

        Returns
        -------
        datetime.date | None
            The rate observation date. When absent the engine subtracts the reset lag from the start date.

        Notes
        -----
        This accessor does not raise for a valid contract.
        """
        ...

    @property
    def reset_lag_days(self) -> int:
        """Read the contract's reset lag days.

        Returns
        -------
        int
            Signed business-day lag used to resolve a missing fixing date.

        Notes
        -----
        This accessor does not raise for a valid contract.
        """
        ...

    @property
    def observed_fixing(self) -> float | None:
        """Read the contract's observed fixing.

        Returns
        -------
        float | None
            Locked annualized fixing as a decimal rate once known, or None.

        Notes
        -----
        This accessor does not raise for a valid contract.
        """
        ...

    @property
    def forward_curve_id(self) -> str:
        """Read the contract's forward curve id.

        Returns
        -------
        str
            Market curve used for the projected fixing.

        Notes
        -----
        This accessor does not raise for a valid contract.
        """
        ...

    @property
    def side(self) -> str:
        """Read the contract's side.

        Returns
        -------
        str
            Fixed leg direction, pay or receive.

        Notes
        -----
        This accessor does not raise for a valid contract.
        """
        ...

    @property
    def expiry(self) -> datetime.date | None:
        """Effective maturity under the forward rate agreement's business-day convention.

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
        """Describe how pickle reconstructs the forward rate agreement from canonical JSON.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and the serialized forward rate agreement argument.

        Raises
        ------
        ValueError
            If the stored forward rate agreement cannot be serialized.
        """
        ...

class ForwardRateAgreementBuilder:
    """Fluent authoring of a forward rate agreement using explicit financial terms.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import ForwardRateAgreement
    >>> builder = ForwardRateAgreement.builder().id("FRA").notional(Money(100_000, "USD"))
    >>> builder = builder.start_date("2025-01-15").maturity("2025-04-15")
    >>> builder = builder.day_count("act_360").fixed_rate(Decimal("0.0525")).discount_curve_id("USD-OIS")
    >>> builder = builder.forward_curve_id("USD-SOFR-3M").side("receive").reset_lag_days(2)
    >>> builder.build().id
    'FRA'
    """

    def id(self, value: str) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's id.

        Parameters
        ----------
        value : str
            Unique forward rate agreement identifier used for pricing results and positions.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def notional(self, value: Money) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's notional.

        Parameters
        ----------
        value : Money
            Strictly positive principal in the forward rate agreement currency.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def start_date(self, value: datetime.date | str) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's start date.

        Parameters
        ----------
        value : datetime.date | str
            Accrual start, normally the settlement date, as a Python date or ISO text.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def maturity(self, value: datetime.date | str) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's maturity.

        Parameters
        ----------
        value : datetime.date | str
            Contractual maturity after the accrual start, as a Python date or ISO text.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def fixed_rate(self, value: Decimal) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's fixed rate.

        Parameters
        ----------
        value : Decimal
            Exact annualized contractual simple rate in decimal form, such as 0.045 for 4.5 percent. Required for cash flows and pricing.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def day_count(self, value: DayCount | str) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's day count.

        Parameters
        ----------
        value : DayCount | str
            Explicit day-count convention used to calculate the interest accrual fraction.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def discount_curve_id(self, value: str) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve identifier looked up in the market at pricing time.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def fixing_business_day_convention(self, value: str) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's business day convention.

        Parameters
        ----------
        value : str
            Business-day adjustment such as modified_following, the default, or following.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def fixing_calendar_id(self, value: str) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's calendar id.

        Parameters
        ----------
        value : str
            Registered holiday calendar identifier used for business-day adjustments.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def fixing_date(self, value: datetime.date | str) -> ForwardRateAgreementBuilder:
        """Set the contract's fixing date.

        Parameters
        ----------
        value : datetime.date | str
            ISO-8601 fixing date on or before the accrual start.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining.

        Raises
        ------
        TypeError
            If the argument has an incompatible type.
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        """
        ...

    def reset_lag_days(self, value: int) -> ForwardRateAgreementBuilder:
        """Set the contract's reset lag days.

        Parameters
        ----------
        value : int
            Signed business-day lag from fixing to accrual start, between -31 and 31.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining.

        Raises
        ------
        TypeError
            If the argument has an incompatible type.
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        """
        ...

    def observed_fixing(self, value: float) -> ForwardRateAgreementBuilder:
        """Set the contract's observed fixing.

        Parameters
        ----------
        value : float
            Finite annualized decimal rate already observed for the contract, e.g. 0.05 for 5 percent.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining.

        Raises
        ------
        TypeError
            If the argument has an incompatible type.
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        """
        ...

    def forward_curve_id(self, value: str) -> ForwardRateAgreementBuilder:
        """Set the contract's forward curve id.

        Parameters
        ----------
        value : str
            Identifier of the projection curve used before an observed fixing.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining.

        Raises
        ------
        TypeError
            If the argument has an incompatible type.
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        """
        ...

    def side(self, value: str) -> ForwardRateAgreementBuilder:
        """Set the contract's side.

        Parameters
        ----------
        value : str
            Wire label pay for paying fixed and receiving floating, or receive for the opposite cash flow.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining.

        Raises
        ------
        TypeError
            If the argument has an incompatible type.
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, object]) -> ForwardRateAgreementBuilder:
        """Set the forward rate agreement's attributes.

        Parameters
        ----------
        value : Attributes | dict[str, object]
            Scenario selection and aggregation attributes stored with the forward rate agreement.

        Returns
        -------
        ForwardRateAgreementBuilder
            The same builder for chaining additional terms.

        Raises
        ------
        TypeError
            If value has an incompatible Python type.
        ValueError
            If the builder was consumed or a supplied date, label or decimal is invalid.
        """
        ...

    def build(self) -> ForwardRateAgreement:
        """
        Build the validated forward rate agreement.

        Runs the same validation as the Rust ``ForwardRateAgreementBuilder::build`` (structural
        invariants only); pricing-time checks run in ``ForwardRateAgreement.price``.

        Returns
        -------
        ForwardRateAgreement
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``ForwardRateAgreementBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> ForwardRateAgreementBuilder:
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
        ForwardRateAgreementBuilder
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
    ) -> ForwardRateAgreementBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        ForwardRateAgreementBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> ForwardRateAgreementBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        ForwardRateAgreementBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
