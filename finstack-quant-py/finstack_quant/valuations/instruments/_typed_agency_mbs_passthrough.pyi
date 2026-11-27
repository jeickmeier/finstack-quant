"""Typed agency MBS passthrough and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.core.dates import DayCount
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class AgencyMbsPassthrough:
    """Agency mortgage pass-through with a generic or specified pool.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AgencyMbsPassthrough
    >>> isinstance(AgencyMbsPassthrough.example(), AgencyMbsPassthrough)
    True
    """

    @staticmethod
    def builder() -> AgencyMbsPassthroughBuilder:
        """Start authoring an agency MBS passthrough.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AgencyMbsPassthrough
        >>> _ = AgencyMbsPassthrough.builder().id("example-agency-mbs-passthrough")
        """
        ...

    def to_builder(self) -> AgencyMbsPassthroughBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> AgencyMbsPassthrough:
        """Construct the built-in agency MBS passthrough example.

        Returns
        -------
        AgencyMbsPassthrough
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AgencyMbsPassthrough
        >>> AgencyMbsPassthrough.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> AgencyMbsPassthrough:
        """
        Deserialize a validated AgencyMbsPassthrough from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"agency_mbs_passthrough"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        AgencyMbsPassthrough
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AgencyMbsPassthrough
        >>> future = AgencyMbsPassthrough.example()
        >>> AgencyMbsPassthrough.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`AgencyMbsPassthrough.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this agency MBS passthrough as a plain Python ``dict``.

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
            The registered pricing model key selected for these contract terms.

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
        """Return the contractual expiry reported by the native instrument.

        Returns
        -------
        datetime.date or None
            Contract expiry where the instrument defines an expiry date.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct the exact contract through its canonical instrument JSON.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and the serialized contract argument.

        Raises
        ------
        ValueError
            If serialization of the contract fails.
        """
        ...

    @property
    def pool_id(self) -> str:
        """Read the contract's pool id.

        Returns
        -------
        str
            Pool identifier (CUSIP or internal pool ID).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def agency(self) -> dict[str, object] | str:
        """Read the contract's agency.

        Returns
        -------
        dict[str, object] | str
            Agency program (FNMA, FHLMC, GNMA).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def pool_type(self) -> dict[str, object] | str:
        """Read the contract's pool type.

        Returns
        -------
        dict[str, object] | str
            Pool type (generic or specified).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def original_face(self) -> Money:
        """Read the contract's original face.

        Returns
        -------
        Money
            Original face amount (initial principal balance).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def current_face(self) -> Money:
        """Read the contract's current face.

        Returns
        -------
        Money
            Current face amount (remaining principal balance).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def wac(self) -> float:
        """Read the contract's wac.

        Returns
        -------
        float
            Weighted average coupon (gross rate on underlying mortgages).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def coupon(self) -> float:
        """Read the contract's coupon.

        Returns
        -------
        float
            Net pass-through coupon paid to the investor, as an annual decimal (0.04 = 4%): wac less the servicing and guarantee fees.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def servicing_fee_bp(self) -> float:
        """Read the contract's servicing fee bp.

        Returns
        -------
        float
            Annual servicing fee in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def guarantee_fee_bp(self) -> float:
        """Read the contract's guarantee fee bp.

        Returns
        -------
        float
            Annual agency guarantee fee (g-fee) in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def wam_months(self) -> int:
        """Read the contract's wam months.

        Returns
        -------
        int
            Remaining weighted average maturity in months as of the valuation date (current WAM, not the original term). Pool age (WALA) for seasoning ramps is derived separately from issue_date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def issue_date(self) -> date:
        """Read the contract's issue date.

        Returns
        -------
        date
            Issue date of the pool.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def last_paid_accrual_end(self) -> date | None:
        """Read the contract's last paid accrual end.

        Returns
        -------
        date | None
            End date of the latest accrual period whose delayed P&I payment has settled and is already reflected in current_face. When omitted, pricing infers the latest paid period from the agency payment-delay rule and as_of.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def maturity(self) -> date:
        """Read the contract's maturity.

        Returns
        -------
        date
            Legal maturity date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def stated_delay_days(self) -> int | None:
        """Read the contract's stated delay days.

        Returns
        -------
        int | None
            Optional custom stated payment delay in days (overrides the agency rule). A delay D pays on day D − 30k of the month k = (D − 1)/30 months after the accrual month, rolled Following on the usny calendar (55 → 25th of the next month, 75 → 15th two months later). Must be at least 1.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def prepayment_spec(self) -> dict[str, object] | str:
        """Read the contract's prepayment spec.

        Returns
        -------
        dict[str, object] | str
            Prepayment model specification.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """Read the contract's discount curve id.

        Returns
        -------
        str
            Discount curve identifier for pricing.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """Read the contract's day count.

        Returns
        -------
        DayCount
            Day count convention for accrual.

        Notes
        -----
        This property does not raise.
        """
        ...

class AgencyMbsPassthroughBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AgencyMbsPassthrough
    >>> builder = AgencyMbsPassthrough.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> AgencyMbsPassthrough:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        AgencyMbsPassthrough
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> AgencyMbsPassthroughBuilder:
        """Set the agency MBS passthrough identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def pool_id(self, value: str) -> AgencyMbsPassthroughBuilder:
        """Set the contract's pool id.

        Parameters
        ----------
        value : str
            Pool identifier (CUSIP or internal pool ID).

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def agency(self, value: dict[str, object] | str) -> AgencyMbsPassthroughBuilder:
        """Set the contract's agency.

        Parameters
        ----------
        value : dict[str, object] | str
            Agency program (FNMA, FHLMC, GNMA).

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def pool_type(self, value: dict[str, object] | str) -> AgencyMbsPassthroughBuilder:
        """Set the contract's pool type.

        Parameters
        ----------
        value : dict[str, object] | str
            Pool type (generic or specified).

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def original_face(self, value: Money) -> AgencyMbsPassthroughBuilder:
        """Set the contract's original face.

        Parameters
        ----------
        value : Money
            Original face amount (initial principal balance).

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def current_face(self, value: Money) -> AgencyMbsPassthroughBuilder:
        """Set the contract's current face.

        Parameters
        ----------
        value : Money
            Current face amount (remaining principal balance).

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def wac(self, value: float) -> AgencyMbsPassthroughBuilder:
        """Set the contract's wac.

        Parameters
        ----------
        value : float
            Weighted average coupon (gross rate on underlying mortgages).

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def coupon(self, value: float) -> AgencyMbsPassthroughBuilder:
        """Set the contract's coupon.

        Parameters
        ----------
        value : float
            Net pass-through coupon paid to the investor, as an annual decimal (0.04 = 4%): wac less the servicing and guarantee fees.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def servicing_fee_bp(self, value: float) -> AgencyMbsPassthroughBuilder:
        """Set the contract's servicing fee bp.

        Parameters
        ----------
        value : float
            Annual servicing fee in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def guarantee_fee_bp(self, value: float) -> AgencyMbsPassthroughBuilder:
        """Set the contract's guarantee fee bp.

        Parameters
        ----------
        value : float
            Annual agency guarantee fee (g-fee) in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def wam_months(self, value: int) -> AgencyMbsPassthroughBuilder:
        """Set the contract's wam months.

        Parameters
        ----------
        value : int
            Remaining weighted average maturity in months as of the valuation date (current WAM, not the original term). Pool age (WALA) for seasoning ramps is derived separately from issue_date.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def issue_date(self, value: date | str) -> AgencyMbsPassthroughBuilder:
        """Set the contract's issue date.

        Parameters
        ----------
        value : date | str
            Issue date of the pool.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def last_paid_accrual_end(self, value: date | str | None) -> AgencyMbsPassthroughBuilder:
        """Set the contract's last paid accrual end.

        Parameters
        ----------
        value : date | str | None
            End date of the latest accrual period whose delayed P&I payment has settled and is already reflected in current_face. When omitted, pricing infers the latest paid period from the agency payment-delay rule and as_of.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def maturity(self, value: date | str) -> AgencyMbsPassthroughBuilder:
        """Set the contract's maturity.

        Parameters
        ----------
        value : date | str
            Legal maturity date.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def stated_delay_days(self, value: int | None) -> AgencyMbsPassthroughBuilder:
        """Set the contract's stated delay days.

        Parameters
        ----------
        value : int | None
            Optional custom stated payment delay in days (overrides the agency rule). A delay D pays on day D − 30k of the month k = (D − 1)/30 months after the accrual month, rolled Following on the usny calendar (55 → 25th of the next month, 75 → 15th two months later). Must be at least 1.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def prepayment_spec(self, value: dict[str, object]) -> AgencyMbsPassthroughBuilder:
        """Set the contract's prepayment spec.

        Parameters
        ----------
        value : dict[str, object]
            Prepayment model specification.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> AgencyMbsPassthroughBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve identifier for pricing.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> AgencyMbsPassthroughBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention for accrual.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> AgencyMbsPassthroughBuilder:
        """Set the agency MBS passthrough grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> AgencyMbsPassthroughBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(
        self, value: MetricPricingOverrides | dict[str, object]
    ) -> AgencyMbsPassthroughBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> AgencyMbsPassthroughBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        AgencyMbsPassthroughBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
