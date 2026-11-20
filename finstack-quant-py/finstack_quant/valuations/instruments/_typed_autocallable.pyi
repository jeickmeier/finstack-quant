"""Typed autocallable note and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class Autocallable:
    """Structured note with observed call levels and contractual early redemption.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Autocallable
    >>> isinstance(Autocallable.example(), Autocallable)
    True
    """

    @staticmethod
    def builder() -> AutocallableBuilder:
        """Start authoring an autocallable note.

        Returns
        -------
        AutocallableBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Autocallable
        >>> _ = Autocallable.builder().id("example-autocallable")
        """
        ...

    def to_builder(self) -> AutocallableBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        AutocallableBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> Autocallable:
        """Construct the built-in autocallable note example.

        Returns
        -------
        Autocallable
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Autocallable
        >>> Autocallable.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Autocallable:
        """
        Deserialize a validated Autocallable from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"autocallable"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Autocallable
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Autocallable
        >>> future = Autocallable.example()
        >>> Autocallable.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`Autocallable.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this autocallable note as a plain Python ``dict``.

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
        Price this autocallable note and return a :class:`~finstack_quant.valuations.ValuationResult`.

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
        Compute a registered scalar metric for this autocallable note.

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
    def underlying_ticker(self) -> str:
        """Read the contract's underlying ticker.

        Returns
        -------
        str
            Underlying asset ticker symbol

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observation_dates(self) -> list[date]:
        """Read the contract's observation dates.

        Returns
        -------
        list[date]
            Observation dates for autocall and coupon checks. Barriers are monitored **discretely** at these exact dates only. The Monte Carlo time grid is constructed to include these dates precisely.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def payment_dates(self) -> list[date]:
        """Read the contract's payment dates.

        Returns
        -------
        list[date]
            Contractual payment dates for coupons and early redemption. Each entry corresponds to observation_dates at the same index.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def expiry(self) -> date:
        """Read the contract's expiry.

        Returns
        -------
        date
            Explicit terminal expiry date for the structure.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def autocall_barriers(self) -> list[float]:
        """Read the contract's autocall barriers.

        Returns
        -------
        list[float]
            Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%). Each barrier corresponds to the observation date at the same index. If spot ≥ barrier × initial_spot on the observation date, the product autocalls.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def coupon_barriers(self) -> list[float]:
        """Read the contract's coupon barriers.

        Returns
        -------
        list[float]
            Coupon barriers as ratios of initial spot. Coupon eligibility is independent of the autocall barrier. Each entry corresponds to the observation date at the same index.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def coupons(self) -> list[float]:
        """Read the contract's coupons.

        Returns
        -------
        list[float]
            Coupon return amounts for each observation date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def memory_coupons(self) -> bool:
        """Read the contract's memory coupons.

        Returns
        -------
        bool
            Memory ("Phoenix") coupon feature. When enabled, a coupon whose coupon barrier is missed accrues until a later observation meets its coupon barrier. The current and all accrued coupons are then paid on that observation's payment date. Coupon eligibility remains independent of autocall eligibility.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def final_barrier(self) -> float:
        """Read the contract's final barrier.

        Returns
        -------
        float
            Final barrier level for final payoff determination

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def final_payoff_type(self) -> dict[str, object] | str:
        """Read the contract's final payoff type.

        Returns
        -------
        dict[str, object] | str
            Type of final payoff (capital protection, participation, knock-in put)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def cap_level(self) -> float:
        """Read the contract's cap level.

        Returns
        -------
        float
            Cap level for final payoff (maximum return)

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
            Notional amount

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
            Day count convention for interest calculations

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
            Discount curve ID for present value calculations

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def spot_id(self) -> str:
        """Read the contract's spot id.

        Returns
        -------
        str
            Spot price identifier for underlying asset

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def vol_surface_id(self) -> str:
        """Read the contract's vol surface id.

        Returns
        -------
        str
            Volatility surface ID for option pricing

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def path_model(self) -> dict[str, object] | str:
        """Read the contract's path model.

        Returns
        -------
        dict[str, object] | str
            Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def div_yield_id(self) -> str | None:
        """Read the contract's div yield id.

        Returns
        -------
        str | None
            Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.

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
            Initial (strike-set) underlying level S_0 used as the reference for barrier and payoff ratios. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any observation date on or before as_of): pricing errors if past observation dates exist without it.

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
            Observed underlying fixings for seasoned trades (date, level pairs). For a mid-life autocallable, every observation date on or before the valuation date must have a matching fixing here; pricing errors otherwise. Past fixings are evaluated deterministically (autocall, missed memory coupons, discrete knock-in monitoring) and only the remaining future observation dates are simulated.

        Notes
        -----
        This property does not raise.
        """
        ...

class AutocallableBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Autocallable
    >>> builder = Autocallable.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> Autocallable:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        Autocallable
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> AutocallableBuilder:
        """Set the autocallable note identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        AutocallableBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying_ticker(self, value: str) -> AutocallableBuilder:
        """Set the contract's underlying ticker.

        Parameters
        ----------
        value : str
            Underlying asset ticker symbol

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observation_dates(self, value: list[date | str]) -> AutocallableBuilder:
        """Set the contract's observation dates.

        Parameters
        ----------
        value : list[date | str]
            Observation dates for autocall and coupon checks. Barriers are monitored **discretely** at these exact dates only. The Monte Carlo time grid is constructed to include these dates precisely.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payment_dates(self, value: list[date | str]) -> AutocallableBuilder:
        """Set the contract's payment dates.

        Parameters
        ----------
        value : list[date | str]
            Contractual payment dates for coupons and early redemption. Each entry corresponds to observation_dates at the same index.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> AutocallableBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Explicit terminal expiry date for the structure.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def autocall_barriers(self, value: list[float]) -> AutocallableBuilder:
        """Set the contract's autocall barriers.

        Parameters
        ----------
        value : list[float]
            Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%). Each barrier corresponds to the observation date at the same index. If spot ≥ barrier × initial_spot on the observation date, the product autocalls.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def coupon_barriers(self, value: list[float]) -> AutocallableBuilder:
        """Set the contract's coupon barriers.

        Parameters
        ----------
        value : list[float]
            Coupon barriers as ratios of initial spot. Coupon eligibility is independent of the autocall barrier. Each entry corresponds to the observation date at the same index.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def coupons(self, value: list[float]) -> AutocallableBuilder:
        """Set the contract's coupons.

        Parameters
        ----------
        value : list[float]
            Coupon return amounts for each observation date.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def memory_coupons(self, value: bool) -> AutocallableBuilder:
        """Set the contract's memory coupons.

        Parameters
        ----------
        value : bool
            Memory ("Phoenix") coupon feature. When enabled, a coupon whose coupon barrier is missed accrues until a later observation meets its coupon barrier. The current and all accrued coupons are then paid on that observation's payment date. Coupon eligibility remains independent of autocall eligibility.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def final_barrier(self, value: float) -> AutocallableBuilder:
        """Set the contract's final barrier.

        Parameters
        ----------
        value : float
            Final barrier level for final payoff determination

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def final_payoff_type(self, value: dict[str, object] | str) -> AutocallableBuilder:
        """Set the contract's final payoff type.

        Parameters
        ----------
        value : dict[str, object] | str
            Type of final payoff (capital protection, participation, knock-in put)

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def cap_level(self, value: float) -> AutocallableBuilder:
        """Set the contract's cap level.

        Parameters
        ----------
        value : float
            Cap level for final payoff (maximum return)

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> AutocallableBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> AutocallableBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention for interest calculations

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> AutocallableBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID for present value calculations

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str) -> AutocallableBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str
            Spot price identifier for underlying asset

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> AutocallableBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface ID for option pricing

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def path_model(self, value: dict[str, object] | str) -> AutocallableBuilder:
        """Set the contract's path model.

        Parameters
        ----------
        value : dict[str, object] | str
            Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def div_yield_id(self, value: str | None) -> AutocallableBuilder:
        """Set the contract's div yield id.

        Parameters
        ----------
        value : str | None
            Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def initial_level(self, value: float | None) -> AutocallableBuilder:
        """Set the contract's initial level.

        Parameters
        ----------
        value : float | None
            Initial (strike-set) underlying level S_0 used as the reference for barrier and payoff ratios. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any observation date on or before as_of): pricing errors if past observation dates exist without it.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def past_fixings(self, value: list[tuple[date | str, float]]) -> AutocallableBuilder:
        """Set the contract's past fixings.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Observed underlying fixings for seasoned trades (date, level pairs). For a mid-life autocallable, every observation date on or before the valuation date must have a matching fixing here; pricing errors otherwise. Past fixings are evaluated deterministically (autocall, missed memory coupons, discrete knock-in monitoring) and only the remaining future observation dates are simulated.

        Returns
        -------
        AutocallableBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> AutocallableBuilder:
        """Set the autocallable note grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        AutocallableBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> AutocallableBuilder:
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
        AutocallableBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> AutocallableBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        AutocallableBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> AutocallableBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        AutocallableBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
