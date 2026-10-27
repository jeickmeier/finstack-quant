"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_credit/cds_option.rs``."""

from __future__ import annotations

import datetime
import decimal
from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides

class CdsOption:
    """
    European option on a single-name or index CDS (typed wrapper for the
    canonical Rust ``CdsOption``).

    The strike is a forward spread (``{"spread": "0.0325"}``, decimal rate)
    or a clean index price in percentage points
    (``{"clean_price_pct": "107.0"}``). A call is the right to buy protection
    (payer), a put the right to sell it (receiver). Pricing uses the
    Bloomberg CDSO numerical-quadrature model; ``delta``, ``gamma``,
    ``vega``, ``theta`` and ``implied_vol`` are metric ids of
    :meth:`CdsOption.price` / :meth:`CdsOption.metric`. Construct via
    :meth:`CdsOption.builder`, :meth:`CdsOption.example` or
    :meth:`CdsOption.from_json`; instances are accepted directly by
    :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CdsOption
    >>> option = CdsOption.example()
    >>> (option.id, option.option_type, option.strike, option.vol_surface_id)
    ('CDSOPT-CALL-CORP-5Y', 'call', {'spread': '0.01'}, 'CDSOPT-VOL')
    """

    @staticmethod
    def builder() -> CdsOptionBuilder:
        """
        Create a fluent builder (mirrors Rust ``CdsOption::builder()``).

        Returns
        -------
        CdsOptionBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsOption
        >>> builder = CdsOption.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CdsOption:
        """
        Deserialize a validated CdsOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cds_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CdsOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsOption
        >>> CdsOption.from_json(CdsOption.example().to_json()).id
        'CDSOPT-CALL-CORP-5Y'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CdsOption.from_json`.

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
            Model key (``"bloomberg_cdso"`` style native model).
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
        Compute one scalar metric for this instrument (e.g. ``"delta"`` or ``"vega"``).

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
    def example() -> CdsOption:
        """
        Canonical 100bp-strike call on a 5-year USD 10,000,000 corporate CDS
        (mirrors Rust ``CdsOption::example``): expiry 2025-06-20, CDS maturity
        2030-06-20, cash settlement, 40% recovery, curves ``USD-OIS`` /
        ``CORP-HAZARD`` and vol surface ``CDSOPT-VOL``.

        Returns
        -------
        CdsOption
            The example option.

        Raises
        ------
        ValueError
            If the example option fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CdsOption
        >>> CdsOption.example().settlement
        'cash'
        """
        ...
    @property
    def strike(self) -> dict[str, str]:
        """
        Option strike in its serde form.

        Returns
        -------
        dict[str, str]
            ``{"spread": "<decimal rate>"}`` or ``{"clean_price_pct": "<price points>"}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def option_type(self) -> str:
        """
        Option type of the CDS option.

        Returns
        -------
        str
            ``"call"`` (right to buy protection) or ``"put"`` (right to sell protection).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date:
        """
        Legal option expiry date.

        Returns
        -------
        datetime.date
            The expiry date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_maturity(self) -> datetime.date:
        """
        Maturity of the underlying CDS.

        Returns
        -------
        datetime.date
            The underlying CDS maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Option notional.

        Returns
        -------
        Money
            Notional of the underlying CDS.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement(self) -> str:
        """
        Settlement type of the exercise proceeds.

        Returns
        -------
        str
            ``"cash"`` or ``"physical"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_settlement_date(self) -> datetime.date | None:
        """
        Explicit exercise proceeds payment date.

        Returns
        -------
        datetime.date | None
            The date, or ``None`` for legal expiry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_start_date(self) -> datetime.date | None:
        """
        Explicit accrual-effective date of the underlying CDS.

        Returns
        -------
        datetime.date | None
            The date, or ``None`` when ``protection_start_convention`` selects it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection_start_convention(self) -> str:
        """
        Accrual-start convention of the synthetic underlying CDS.

        Returns
        -------
        str
            ``"spot"`` or ``"forward"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def knockout(self) -> bool:
        """
        Whether the option knocks out on default before expiry.

        Returns
        -------
        bool
            ``True`` for knock-out options.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Recovery rate assumption.

        Returns
        -------
        float
            Recovery as a decimal (``0.4`` = 40%).

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
            Discount curve id in the market context.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str:
        """
        Hazard (credit) curve identifier.

        Returns
        -------
        str
            Hazard curve id in the market context.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        Volatility surface identifier.

        Returns
        -------
        str
            Vol surface id in the market context.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_convention(self) -> str:
        """
        ISDA convention of the underlying CDS.

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
    def underlying_is_index(self) -> bool:
        """
        Whether the underlying is a CDS index.

        Returns
        -------
        bool
            ``True`` for an index option, ``False`` for a single name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def index_factor(self) -> float:
        """
        Current index factor ``f``.

        Returns
        -------
        float
            Surviving fraction of original index notional, in ``(0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def strike_index_factor(self) -> float | None:
        """
        Original index factor ``f0`` of a clean-price strike.

        Returns
        -------
        float | None
            The factor, or ``None`` for spread strikes.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def realized_loss(self) -> float:
        """
        Settled cumulative index loss since option inception.

        Returns
        -------
        float
            Decimal fraction of original index notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def coupon_bp(self) -> decimal.Decimal | None:
        """
        Running coupon of the underlying CDS in basis points.

        Returns
        -------
        decimal.Decimal | None
            The coupon, or ``None`` when the strike spread is the coupon.

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

class CdsOptionBuilder:
    """
    Fluent builder for :class:`CdsOption`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Required: ``id``, ``strike``, ``option_type``, ``expiry``,
    ``underlying_maturity``, ``notional``, ``settlement``,
    ``recovery_rate``, ``discount_curve_id``, ``credit_curve_id``,
    ``vol_surface_id`` and ``underlying_is_index``. Builders are consumed by
    ``build()``; create a new builder per instrument.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import CdsOption
    >>> option = (
    ...     CdsOption
    ...     .builder()
    ...     .id("CDSO-1")
    ...     .strike({"spread": "0.0125"})
    ...     .option_type("put")
    ...     .expiry(datetime.date(2025, 6, 20))
    ...     .underlying_maturity(datetime.date(2030, 6, 20))
    ...     .notional(Money(5_000_000, Currency("USD")))
    ...     .settlement("physical")
    ...     .recovery_rate(0.4)
    ...     .discount_curve_id("USD-OIS")
    ...     .credit_curve_id("CORP-HAZARD")
    ...     .vol_surface_id("CDSOPT-VOL")
    ...     .underlying_is_index(False)
    ...     .build()
    ... )
    >>> (option.option_type, option.settlement, option.strike)
    ('put', 'physical', {'spread': '0.0125'})
    """

    def id(self, value: str) -> CdsOptionBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the option.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def strike(self, value: dict[str, str] | str) -> CdsOptionBuilder:
        """
        Set the option strike as a forward spread or a clean index price.

        Parameters
        ----------
        value : dict[str, str] | str
            ``{"spread": "0.0325"}`` (decimal forward spread) or ``{"clean_price_pct": "107.0"}`` (clean price points), as a dict or JSON text.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not one of the two strike forms.
        """
        ...
    def option_type(self, value: str) -> CdsOptionBuilder:
        """
        Set the option type.

        Parameters
        ----------
        value : str
            ``"call"`` buys protection at expiry, ``"put"`` sells it.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized option type.
        """
        ...
    def expiry(self, value: datetime.date | str) -> CdsOptionBuilder:
        """
        Set the option expiry date.

        Parameters
        ----------
        value : datetime.date | str
            Legal expiry; must precede ``underlying_maturity``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date.
        """
        ...
    def underlying_maturity(self, value: datetime.date | str) -> CdsOptionBuilder:
        """
        Set the underlying CDS maturity date.

        Parameters
        ----------
        value : datetime.date | str
            Maturity of the CDS delivered or cash-settled at exercise.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date.
        """
        ...
    def notional(self, value: Money) -> CdsOptionBuilder:
        """
        Set the option notional.

        Parameters
        ----------
        value : Money
            Positive notional of the underlying CDS.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def settlement(self, value: str) -> CdsOptionBuilder:
        """
        Set the settlement type.

        Parameters
        ----------
        value : str
            ``"cash"`` or ``"physical"``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized settlement type.
        """
        ...
    def exercise_settlement_date(self, value: datetime.date | str) -> CdsOptionBuilder:
        """
        Set the exercise proceeds payment date.

        Parameters
        ----------
        value : datetime.date | str
            On or after expiry and before CDS maturity; when never set legal expiry is used.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date.
        """
        ...
    def underlying_start_date(self, value: datetime.date | str) -> CdsOptionBuilder:
        """
        Set the underlying CDS accrual-effective date.

        Parameters
        ----------
        value : datetime.date | str
            Accrual start for the forward spread and risky annuity.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date.
        """
        ...
    def protection_start_convention(self, value: str) -> CdsOptionBuilder:
        """
        Set the underlying accrual-start convention.

        Parameters
        ----------
        value : str
            ``"spot"`` (default, prior CDS roll) or ``"forward"`` (option expiry).

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized convention.
        """
        ...
    def knockout(self, value: bool) -> CdsOptionBuilder:
        """
        Set whether the option knocks out on default before expiry.

        Parameters
        ----------
        value : bool
            ``True`` for knock-out single-name options; default ``False``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def recovery_rate(self, value: float) -> CdsOptionBuilder:
        """
        Set the recovery rate assumption.

        Parameters
        ----------
        value : float
            Recovery as a decimal in ``[0, 1]``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def discount_curve_id(self, value: str) -> CdsOptionBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve id in the market context.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def credit_curve_id(self, value: str) -> CdsOptionBuilder:
        """
        Set the hazard (credit) curve identifier.

        Parameters
        ----------
        value : str
            Hazard curve id in the market context.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def vol_surface_id(self, value: str) -> CdsOptionBuilder:
        """
        Set the volatility surface identifier.

        Parameters
        ----------
        value : str
            Vol surface id in the market context.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def underlying_convention(self, value: str) -> CdsOptionBuilder:
        """
        Set the ISDA convention of the underlying CDS.

        Parameters
        ----------
        value : str
            ``"isda_na"`` (default), ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized convention.
        """
        ...
    def underlying_is_index(self, value: bool) -> CdsOptionBuilder:
        """
        Set whether the underlying is a CDS index.

        Parameters
        ----------
        value : bool
            ``True`` for an index option, ``False`` for a single name.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def index_factor(self, value: float) -> CdsOptionBuilder:
        """
        Set the current index factor.

        Parameters
        ----------
        value : float
            Surviving fraction of the original index notional, in ``(0, 1]``; default ``1.0``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def strike_index_factor(self, value: float) -> CdsOptionBuilder:
        """
        Set the original index factor of a clean-price strike.

        Parameters
        ----------
        value : float
            ``f0`` the clean-price strike is quoted on.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def realized_loss(self, value: float) -> CdsOptionBuilder:
        """
        Set the settled cumulative index loss since option inception.

        Parameters
        ----------
        value : float
            Decimal fraction of original index notional in ``[0, 1]``; default ``0.0``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`CdsOptionBuilder.build`.
        """
        ...
    def coupon_bp(self, value: decimal.Decimal | str | int | float | Bps | Rate) -> CdsOptionBuilder:
        """
        Set the running coupon of the underlying CDS.

        Parameters
        ----------
        value : decimal.Decimal | str | int | float | Bps | Rate
            Coupon in basis points (``100`` or ``Bps(100)`` for CDX.NA.IG).

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a decimal number.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CdsOptionBuilder:
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
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> CdsOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CdsOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> CdsOptionBuilder:
        """
        Set free-form instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict | None
            Attribute bag: an ``Attributes`` or its serde ``dict`` form
            (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
            ``None`` clears it.

        Returns
        -------
        CdsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
        """
        ...
    def build(self) -> CdsOption:
        """
        Build the validated CDS option.

        Runs only the Rust ``CdsOptionBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`CdsOption.price`.

        Returns
        -------
        CdsOption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (e.g. ``CdsOptionBuilder: missing required field 'id'``), or the
            option fails validation (expiry not before CDS maturity,
            non-positive notional, inconsistent strike state).
        """
        ...
