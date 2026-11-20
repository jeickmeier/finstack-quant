"""Typed equity total-return swap and its builder."""

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

class EquityTotalReturnSwap:
    """Swap exchanging equity price and net dividend return for floating-rate financing and a spread.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityTotalReturnSwap
    >>> isinstance(EquityTotalReturnSwap.example(), EquityTotalReturnSwap)
    True
    """

    @staticmethod
    def builder() -> EquityTotalReturnSwapBuilder:
        """Start authoring an equity total-return swap.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityTotalReturnSwap
        >>> _ = EquityTotalReturnSwap.builder().id("example-equity-total-return-swap")
        """
        ...

    def to_builder(self) -> EquityTotalReturnSwapBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> EquityTotalReturnSwap:
        """Construct the built-in equity total-return swap example.

        Returns
        -------
        EquityTotalReturnSwap
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityTotalReturnSwap
        >>> EquityTotalReturnSwap.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> EquityTotalReturnSwap:
        """
        Deserialize a validated EquityTotalReturnSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"equity_total_return_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        EquityTotalReturnSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityTotalReturnSwap
        >>> future = EquityTotalReturnSwap.example()
        >>> EquityTotalReturnSwap.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`EquityTotalReturnSwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this equity total-return swap as a plain Python ``dict``.

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
        Price this equity total-return swap and return a :class:`~finstack_quant.valuations.ValuationResult`.

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
        Compute a registered scalar metric for this equity total-return swap.

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
    def notional(self) -> Money:
        """Read the contract's notional.

        Returns
        -------
        Money
            Notional amount for the swap.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def underlying(self) -> dict[str, object] | str:
        """Read the contract's underlying.

        Returns
        -------
        dict[str, object] | str
            Underlying equity parameters (spot ID, dividend yield, contract size).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def financing_leg(self) -> dict[str, object] | str:
        """Read the contract's financing leg.

        Returns
        -------
        dict[str, object] | str
            Financing leg specification (curves, spread, day count).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def schedule(self) -> dict[str, object] | str:
        """Read the contract's schedule.

        Returns
        -------
        dict[str, object] | str
            Schedule specification (payment dates and frequency).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def side(self) -> dict[str, object] | str:
        """Read the contract's side.

        Returns
        -------
        dict[str, object] | str
            Trade side: receive receives the total-return leg and pays financing; pay pays the total return and receives financing.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def initial_level(self) -> float | None:
        """Read the contract's initial level.

        Returns
        -------
        float | None
            Optional first-period start level. Pricing uses this only when the first period is in progress and no matching entry exists in past_fixings. Future periods use live spot.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def past_fixings(self) -> list[tuple[date, float]]:
        """Read the contract's past fixings.

        Returns
        -------
        list[tuple[date, float]]
            Observed underlying levels at contractual reset boundaries. Provide (reset_date, level) for each boundary on or before the valuation date that is needed by an unpaid return period. This includes the final period end when payment lag leaves its cashflow outstanding. The first boundary may use initial_level; a boundary equal to as_of may use live spot. Pricing errors when any other required fixing is unavailable.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def margin_spec(self) -> dict[str, object] | str | None:
        """Read the contract's margin spec.

        Returns
        -------
        dict[str, object] | str | None
            Optional OTC margin specification for VM/IM. Equity TRS use SIMM equity bucket for margin calculation.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def dividend_tax_rate(self) -> float:
        """Read the contract's dividend tax rate.

        Returns
        -------
        float
            Dividend withholding tax rate for net return calculation. Specifies the fraction of dividends withheld for tax (e.g., 0.15 for 15% withholding). When set to 0.0 (default), the TRS passes through 100% of dividends (gross return). When set to a positive value, the dividend return component is reduced: text net_dividend_return = gross_dividend_return × (1 - dividend_tax_rate) # Market Context Withholding tax varies by jurisdiction and investor domicile: - US qualified dividends: typically 0% for domestic investors - US non-qualified: up to 30% for foreign investors (varies by treaty) - European: varies by country (15-30% typical)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def dividend_settlement(self) -> dict[str, object] | str:
        """Read the contract's dividend settlement.

        Returns
        -------
        dict[str, object] | str
            Settlement timing for explicit manufactured dividends. This field is mandatory because dividend-date and period-end settlement differ by the funding carry between the two dates.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discrete_dividends(self) -> list[tuple[date, float]]:
        """Read the contract's discrete dividends.

        Returns
        -------
        list[tuple[date, float]]
            Optional discrete cash dividends (ex_date, amount) for the underlying. When non-empty, pricing uses explicit period dividend pass-through and does not add continuous-yield dividend return to avoid double counting.

        Notes
        -----
        This property does not raise.
        """
        ...

class EquityTotalReturnSwapBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityTotalReturnSwap
    >>> builder = EquityTotalReturnSwap.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> EquityTotalReturnSwap:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        EquityTotalReturnSwap
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> EquityTotalReturnSwapBuilder:
        """Set the equity total-return swap identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def notional(self, value: Money) -> EquityTotalReturnSwapBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount for the swap.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def underlying(self, value: dict[str, object] | str) -> EquityTotalReturnSwapBuilder:
        """Set the contract's underlying.

        Parameters
        ----------
        value : dict[str, object] | str
            Underlying equity parameters (spot ID, dividend yield, contract size).

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def financing_leg(self, value: dict[str, object] | str) -> EquityTotalReturnSwapBuilder:
        """Set the contract's financing leg.

        Parameters
        ----------
        value : dict[str, object] | str
            Financing leg specification (curves, spread, day count).

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def schedule(self, value: dict[str, object] | str) -> EquityTotalReturnSwapBuilder:
        """Set the contract's schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Schedule specification (payment dates and frequency).

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def side(self, value: dict[str, object] | str) -> EquityTotalReturnSwapBuilder:
        """Set the contract's side.

        Parameters
        ----------
        value : dict[str, object] | str
            Trade side: receive receives the total-return leg and pays financing; pay pays the total return and receives financing.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def initial_level(self, value: float | None) -> EquityTotalReturnSwapBuilder:
        """Set the contract's initial level.

        Parameters
        ----------
        value : float | None
            Optional first-period start level. Pricing uses this only when the first period is in progress and no matching entry exists in past_fixings. Future periods use live spot.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def past_fixings(self, value: list[tuple[date | str, float]]) -> EquityTotalReturnSwapBuilder:
        """Set the contract's past fixings.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Observed underlying levels at contractual reset boundaries. Provide (reset_date, level) for each boundary on or before the valuation date that is needed by an unpaid return period. This includes the final period end when payment lag leaves its cashflow outstanding. The first boundary may use initial_level; a boundary equal to as_of may use live spot. Pricing errors when any other required fixing is unavailable.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def margin_spec(self, value: dict[str, object] | str | None) -> EquityTotalReturnSwapBuilder:
        """Set the contract's margin spec.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Optional OTC margin specification for VM/IM. Equity TRS use SIMM equity bucket for margin calculation.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def dividend_tax_rate(self, value: float) -> EquityTotalReturnSwapBuilder:
        """Set the contract's dividend tax rate.

        Parameters
        ----------
        value : float
            Dividend withholding tax rate for net return calculation. Specifies the fraction of dividends withheld for tax (e.g., 0.15 for 15% withholding). When set to 0.0 (default), the TRS passes through 100% of dividends (gross return). When set to a positive value, the dividend return component is reduced: text net_dividend_return = gross_dividend_return × (1 - dividend_tax_rate) # Market Context Withholding tax varies by jurisdiction and investor domicile: - US qualified dividends: typically 0% for domestic investors - US non-qualified: up to 30% for foreign investors (varies by treaty) - European: varies by country (15-30% typical)

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def dividend_settlement(self, value: dict[str, object] | str) -> EquityTotalReturnSwapBuilder:
        """Set the contract's dividend settlement.

        Parameters
        ----------
        value : dict[str, object] | str
            Settlement timing for explicit manufactured dividends. This field is mandatory because dividend-date and period-end settlement differ by the funding carry between the two dates.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discrete_dividends(self, value: list[tuple[date | str, float]]) -> EquityTotalReturnSwapBuilder:
        """Set the contract's discrete dividends.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Optional discrete cash dividends (ex_date, amount) for the underlying. When non-empty, pricing uses explicit period dividend pass-through and does not add continuous-yield dividend return to avoid double counting.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> EquityTotalReturnSwapBuilder:
        """Set the equity total-return swap grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> EquityTotalReturnSwapBuilder:
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
        EquityTotalReturnSwapBuilder
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
    ) -> EquityTotalReturnSwapBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> EquityTotalReturnSwapBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        EquityTotalReturnSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
