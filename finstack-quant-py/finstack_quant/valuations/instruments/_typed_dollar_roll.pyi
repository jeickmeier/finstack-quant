"""Typed dollar roll and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from finstack_quant.core.money import Money
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class DollarRoll:
    """Paired agency TBA sale and purchase with different settlement months.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import DollarRoll
    >>> isinstance(DollarRoll.example(), DollarRoll)
    True
    """

    @staticmethod
    def builder() -> DollarRollBuilder:
        """Start authoring a dollar roll.

        Returns
        -------
        DollarRollBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import DollarRoll
        >>> _ = DollarRoll.builder().id("example-dollar-roll")
        """
        ...

    def to_builder(self) -> DollarRollBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        DollarRollBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> DollarRoll:
        """Construct the built-in dollar roll example.

        Returns
        -------
        DollarRoll
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import DollarRoll
        >>> DollarRoll.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> DollarRoll:
        """
        Deserialize a validated DollarRoll from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"dollar_roll"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        DollarRoll
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import DollarRoll
        >>> future = DollarRoll.example()
        >>> DollarRoll.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`DollarRoll.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this dollar roll as a plain Python ``dict``.

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
        Price this dollar roll and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Registered model key for these terms. The default uses default_model.
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
            If a required market observation or requested metric key is missing.
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
        Compute a registered scalar metric for this dollar roll.

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
            If an observation required by the selected metric is missing.
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
    def agency(self) -> dict[str, object] | str:
        """Read the contract's agency.

        Returns
        -------
        dict[str, object] | str
            Agency program.

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
            Pass-through coupon rate.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def term(self) -> dict[str, object] | str:
        """Read the contract's term.

        Returns
        -------
        dict[str, object] | str
            Original loan term.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def notional(self) -> Money:
        """Read the contract's notional.

        Returns
        -------
        Money
            Trade notional (par amount).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def front_settlement_year(self) -> int:
        """Read the contract's front settlement year.

        Returns
        -------
        int
            Front-month settlement year.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def front_settlement_month(self) -> int:
        """Read the contract's front settlement month.

        Returns
        -------
        int
            Front-month settlement month (1-12).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def back_settlement_year(self) -> int:
        """Read the contract's back settlement year.

        Returns
        -------
        int
            Back-month settlement year.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def back_settlement_month(self) -> int:
        """Read the contract's back settlement month.

        Returns
        -------
        int
            Back-month settlement month (1-12).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement_class(self) -> dict[str, object] | str:
        """Read the contract's settlement class.

        Returns
        -------
        dict[str, object] | str
            SIFMA settlement class of the deliverable pools (A/B/C/D), used for both legs. Required: never inferred from agency or term.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def front_settlement_date(self) -> date | None:
        """Read the contract's front settlement date.

        Returns
        -------
        date | None
            Explicit front-month settlement date override. When set, bypasses the SIFMA calendar lookup for the front leg.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def back_settlement_date(self) -> date | None:
        """Read the contract's back settlement date.

        Returns
        -------
        date | None
            Explicit back-month settlement date override. When set, bypasses the SIFMA calendar lookup for the back leg.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def front_price(self) -> float:
        """Read the contract's front price.

        Returns
        -------
        float
            Front-month price (sell price).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def back_price(self) -> float:
        """Read the contract's back price.

        Returns
        -------
        float
            Back-month price (buy price).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def trade_date(self) -> date | None:
        """Read the contract's trade date.

        Returns
        -------
        date | None
            Trade date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def prepayment_spec(self) -> dict[str, object] | str | None:
        """Read the contract's prepayment spec.

        Returns
        -------
        dict[str, object] | str | None
            Prepayment model of the generic pool both legs deliver. None uses the embedded generic PSA assumption of the TBA legs.

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
            Discount curve identifier.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def repo_curve_id(self) -> str | None:
        """Read the contract's repo curve id.

        Returns
        -------
        str | None
            Optional repo/financing curve identifier (carry-only). Discount curve used by roll specialness to calculate the simple financing rate (DF(front)/DF(back) − 1)/τ over the front/back settlement interval, with τ on ACT/360. The implied financing rate itself is determined by prices and carry. See the carry calculations (see crate::dollar_roll::carry module). Does **not** affect the mark-to-market PV, which always discounts both legs at discount_curve_id. When None, the discount curve rate is used as the reference financing rate for carry analytics.

        Notes
        -----
        This property does not raise.
        """
        ...

class DollarRollBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import DollarRoll
    >>> builder = DollarRoll.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> DollarRoll:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        DollarRoll
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> DollarRollBuilder:
        """Set the dollar roll identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        DollarRollBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def agency(self, value: dict[str, object] | str) -> DollarRollBuilder:
        """Set the contract's agency.

        Parameters
        ----------
        value : dict[str, object] | str
            Agency program.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def coupon(self, value: float) -> DollarRollBuilder:
        """Set the contract's coupon.

        Parameters
        ----------
        value : float
            Pass-through coupon rate.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def term(self, value: dict[str, object] | str) -> DollarRollBuilder:
        """Set the contract's term.

        Parameters
        ----------
        value : dict[str, object] | str
            Original loan term.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> DollarRollBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Trade notional (par amount).

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def front_settlement_year(self, value: int) -> DollarRollBuilder:
        """Set the contract's front settlement year.

        Parameters
        ----------
        value : int
            Front-month settlement year.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def front_settlement_month(self, value: int) -> DollarRollBuilder:
        """Set the contract's front settlement month.

        Parameters
        ----------
        value : int
            Front-month settlement month (1-12).

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def back_settlement_year(self, value: int) -> DollarRollBuilder:
        """Set the contract's back settlement year.

        Parameters
        ----------
        value : int
            Back-month settlement year.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def back_settlement_month(self, value: int) -> DollarRollBuilder:
        """Set the contract's back settlement month.

        Parameters
        ----------
        value : int
            Back-month settlement month (1-12).

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_class(self, value: dict[str, object] | str) -> DollarRollBuilder:
        """Set the contract's settlement class.

        Parameters
        ----------
        value : dict[str, object] | str
            SIFMA settlement class of the deliverable pools (A/B/C/D), used for both legs. Required: never inferred from agency or term.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def front_settlement_date(self, value: date | str | None) -> DollarRollBuilder:
        """Set the contract's front settlement date.

        Parameters
        ----------
        value : date | str | None
            Explicit front-month settlement date override. When set, bypasses the SIFMA calendar lookup for the front leg.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def back_settlement_date(self, value: date | str | None) -> DollarRollBuilder:
        """Set the contract's back settlement date.

        Parameters
        ----------
        value : date | str | None
            Explicit back-month settlement date override. When set, bypasses the SIFMA calendar lookup for the back leg.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def front_price(self, value: float) -> DollarRollBuilder:
        """Set the contract's front price.

        Parameters
        ----------
        value : float
            Front-month price (sell price).

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def back_price(self, value: float) -> DollarRollBuilder:
        """Set the contract's back price.

        Parameters
        ----------
        value : float
            Back-month price (buy price).

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def trade_date(self, value: date | str | None) -> DollarRollBuilder:
        """Set the contract's trade date.

        Parameters
        ----------
        value : date | str | None
            Trade date.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def prepayment_spec(self, value: dict[str, object] | str | None) -> DollarRollBuilder:
        """Set the contract's prepayment spec.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Prepayment model of the generic pool both legs deliver. None uses the embedded generic PSA assumption of the TBA legs.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> DollarRollBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def repo_curve_id(self, value: str | None) -> DollarRollBuilder:
        """Set the contract's repo curve id.

        Parameters
        ----------
        value : str | None
            Optional repo/financing curve identifier (carry-only). Discount curve used by roll specialness to calculate the simple financing rate (DF(front)/DF(back) − 1)/τ over the front/back settlement interval, with τ on ACT/360. The implied financing rate itself is determined by prices and carry. See the carry calculations (see crate::dollar_roll::carry module). Does **not** affect the mark-to-market PV, which always discounts both legs at discount_curve_id. When None, the discount curve rate is used as the reference financing rate for carry analytics.

        Returns
        -------
        DollarRollBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> DollarRollBuilder:
        """Set the dollar roll grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        DollarRollBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> DollarRollBuilder:
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
        DollarRollBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> DollarRollBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        DollarRollBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> DollarRollBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        DollarRollBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
