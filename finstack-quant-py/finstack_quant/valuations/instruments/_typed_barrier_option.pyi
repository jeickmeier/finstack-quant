"""Typed barrier option and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class BarrierOption:
    """Option with barrier monitoring and a contractual rebate. The default pricing method follows the selected monitoring convention.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BarrierOption
    >>> isinstance(BarrierOption.example(), BarrierOption)
    True
    """

    @staticmethod
    def builder() -> BarrierOptionBuilder:
        """Start authoring a barrier option.

        Returns
        -------
        BarrierOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierOption
        >>> _ = BarrierOption.builder().id("example-barrier-option")
        """
        ...

    def to_builder(self) -> BarrierOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        BarrierOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> BarrierOption:
        """Construct the built-in barrier option example.

        Returns
        -------
        BarrierOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierOption
        >>> BarrierOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> BarrierOption:
        """
        Deserialize a validated BarrierOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"barrier_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        BarrierOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierOption
        >>> future = BarrierOption.example()
        >>> BarrierOption.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`BarrierOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this barrier option as a plain Python ``dict``.

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
        Price this barrier option and return a :class:`~finstack_quant.valuations.ValuationResult`.

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
    ) -> float:
        """
        Compute a registered scalar metric for this barrier option.

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
    def strike(self) -> float:
        """Read the contract's strike.

        Returns
        -------
        float
            Strike price

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def barrier(self) -> float:
        """Read the contract's barrier.

        Returns
        -------
        float
            Barrier level: the absolute underlying price that triggers the knock-in/out, in the same quote units as strike.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def rebate(self) -> Money | None:
        """Read the contract's rebate.

        Returns
        -------
        Money | None
            Total contractual trade rebate in the payoff currency, independent of notional. Knock-outs pay on a hit according to rebate_timing; knock-ins pay at expiry only if no hit occurred.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def rebate_timing(self) -> dict[str, object] | str | None:
        """Read the contract's rebate timing.

        Returns
        -------
        dict[str, object] | str | None
            Payment timing of the rebate. Required exactly when rebate is set. For a knock-out, at_hit pays the rebate at the first qualifying hit and at_expiry defers it to expiry. A knock-in (no-hit) rebate is only determined at expiry, so it must be at_expiry; at_hit is rejected. The analytical pricer values at-hit knock-out rebates via the discounted first-passage closed form; Monte Carlo applies at-hit timing via with_rebate_at_hit.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def option_type(self) -> dict[str, object] | str:
        """Read the contract's option type.

        Returns
        -------
        dict[str, object] | str
            Option type (call or put)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def barrier_type(self) -> dict[str, object] | str:
        """Read the contract's barrier type.

        Returns
        -------
        dict[str, object] | str
            Barrier type (up/down, in/out)

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
            Option expiry date

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def expiry_fixing(self) -> float | None:
        """Read the contract's expiry fixing.

        Returns
        -------
        float | None
            Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized intrinsic value is invariant to later market spot updates. At expiry, the current market spot is used when this field is absent.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observed_barrier_breached(self) -> bool | None:
        """Read the contract's observed barrier breached.

        Returns
        -------
        bool | None
            Observed barrier state for expired options. Historical barrier monitoring must be supplied explicitly for expired options because terminal spot alone does not reveal whether the barrier was breached intralife and then reversed.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quantity(self) -> float:
        """Read the contract's quantity.

        Returns
        -------
        float
            Number of underlying units the option is written on; PV and Greeks scale linearly with it.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def currency(self) -> str:
        """Read the contract's currency.

        Returns
        -------
        str
            Currency of the strike, premium and present value.

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
            Day count convention

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def monitoring(self) -> dict[str, object] | str:
        """Read the contract's monitoring.

        Returns
        -------
        dict[str, object] | str
            Contractual monitoring: continuous or an explicit, strictly increasing set of observation dates no later than expiry. Discrete pricing does not interpolate hits between those dates.

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
            Spot price identifier

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
            Volatility surface ID

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
            Optional dividend-yield scalar ID

        Notes
        -----
        This property does not raise.
        """
        ...

class BarrierOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BarrierOption
    >>> builder = BarrierOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> BarrierOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        BarrierOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> BarrierOptionBuilder:
        """Set the barrier option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        BarrierOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying_ticker(self, value: str) -> BarrierOptionBuilder:
        """Set the contract's underlying ticker.

        Parameters
        ----------
        value : str
            Underlying asset ticker symbol

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: float) -> BarrierOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : float
            Strike price

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def barrier(self, value: float) -> BarrierOptionBuilder:
        """Set the contract's barrier.

        Parameters
        ----------
        value : float
            Barrier level: the absolute underlying price that triggers the knock-in/out, in the same quote units as strike.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def rebate(self, value: Money | None) -> BarrierOptionBuilder:
        """Set the contract's rebate.

        Parameters
        ----------
        value : Money | None
            Total contractual trade rebate in the payoff currency, independent of notional. Knock-outs pay on a hit according to rebate_timing; knock-ins pay at expiry only if no hit occurred.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def rebate_timing(self, value: dict[str, object] | str | None) -> BarrierOptionBuilder:
        """Set the contract's rebate timing.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Payment timing of the rebate. Required exactly when rebate is set. For a knock-out, at_hit pays the rebate at the first qualifying hit and at_expiry defers it to expiry. A knock-in (no-hit) rebate is only determined at expiry, so it must be at_expiry; at_hit is rejected. The analytical pricer values at-hit knock-out rebates via the discounted first-passage closed form; Monte Carlo applies at-hit timing via with_rebate_at_hit.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: dict[str, object] | str) -> BarrierOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : dict[str, object] | str
            Option type (call or put)

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def barrier_type(self, value: dict[str, object] | str) -> BarrierOptionBuilder:
        """Set the contract's barrier type.

        Parameters
        ----------
        value : dict[str, object] | str
            Barrier type (up/down, in/out)

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> BarrierOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Option expiry date

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry_fixing(self, value: float | None) -> BarrierOptionBuilder:
        """Set the contract's expiry fixing.

        Parameters
        ----------
        value : float | None
            Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized intrinsic value is invariant to later market spot updates. At expiry, the current market spot is used when this field is absent.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observed_barrier_breached(self, value: bool | None) -> BarrierOptionBuilder:
        """Set the contract's observed barrier breached.

        Parameters
        ----------
        value : bool | None
            Observed barrier state for expired options. Historical barrier monitoring must be supplied explicitly for expired options because terminal spot alone does not reveal whether the barrier was breached intralife and then reversed.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quantity(self, value: float) -> BarrierOptionBuilder:
        """Set the contract's quantity.

        Parameters
        ----------
        value : float
            Number of underlying units the option is written on; PV and Greeks scale linearly with it.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def currency(self, value: Currency | str) -> BarrierOptionBuilder:
        """Set the contract's currency.

        Parameters
        ----------
        value : Currency | str
            Currency of the strike, premium and present value.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> BarrierOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def monitoring(self, value: dict[str, object] | str) -> BarrierOptionBuilder:
        """Set the contract's monitoring.

        Parameters
        ----------
        value : dict[str, object] | str
            Contractual monitoring: continuous or an explicit, strictly increasing set of observation dates no later than expiry. Discrete pricing does not interpolate hits between those dates.

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> BarrierOptionBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID for present value calculations

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str) -> BarrierOptionBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str
            Spot price identifier

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> BarrierOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface ID

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def div_yield_id(self, value: str | None) -> BarrierOptionBuilder:
        """Set the contract's div yield id.

        Parameters
        ----------
        value : str | None
            Optional dividend-yield scalar ID

        Returns
        -------
        BarrierOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> BarrierOptionBuilder:
        """Set the barrier option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        BarrierOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> BarrierOptionBuilder:
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
        BarrierOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> BarrierOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        BarrierOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> BarrierOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        BarrierOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
